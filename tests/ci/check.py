#!/usr/bin/env python3
"""tests/ci —— 不变量与 CI 强制项的检查实现。

约束的**定义**（编号、出处、现在能否落地）唯一出处是 `docs/CONSTRAINTS.md`（`CX-`），
本脚本只负责**实现**检查，不重述约束内容。

用法::

    python3 tests/ci/check.py           # 跑全部检查；有失败则退出码 1
    python3 tests/ci/check.py --list    # 列出检查项、对应的 CX 编号与扫描范围

扫描范围由 `CONSTRAINTS.md` §3.1 规定（`T-CX-3`／`T-CX-4`／`T-CX-5` 只扫各 crate 的 `src/`；
`T-CX-9` 只扫 `pyawa-core` 的 `src/`）。`T-CX-8` 扫 `docs/*.md`、各 `README.md` 与根 `Cargo.toml`。
"""

from __future__ import annotations

import argparse
import collections
import dataclasses
import pathlib
import re
import sys
from collections.abc import Callable

ROOT = pathlib.Path(__file__).resolve().parents[2]

#: 编号族与"哪份规格还没写"**都从 `docs/SPEC-INDEX.md` §1 派生**（见 `id_families`）。
#: 在这里另立一张常量表会造出第二个真相源——规格状态一变，它不跟着动。
ID_TOKEN = r"(?:T-)?[A-Z]{2}-\d+"
REFERENCE = re.compile(rf"(?<![\w-])({ID_TOKEN}|§13-\d+)\b")
ANY_ID = re.compile(rf"(?<![\w-])({ID_TOKEN})\b")

#: 引用尚未写出的规格时，所在行必须显式标注临时假设（`SPEC-INDEX.md` §2／§3）。
ANNOTATION_MARKERS = ("临时假设", "未定", "待写", "暂假设", "依赖")

#: `CX-3`：VM 核心 crate。
VM_CORE_CRATES = ("crates/pyawa-core",)
#: `CX-4`：还要加上能力接口 crate 与 C 层模块 crate（`CONSTRAINTS.md` §3.1）。
INTERFACE_CRATES = ("crates/pyawa-capabilities",)
#: `CX-4`：`pyawa-stdlib` 从第一个模块起就纳入——它的模块一旦自行 `use std::fs`，
#: 能力层就不再是唯一 I/O 出口（`DESIGN.md` §3 不变量 1）。
STDLIB_CRATES = ("crates/pyawa-stdlib",)

FORBIDDEN_GLOBAL_STATE = (r"\bstatic\s+mut\b", r"\bthread_local\b")
FORBIDDEN_PLATFORM = (
    r"\bstd::fs\b",
    r"\bstd::net\b",
    r"\blibc\b",
    r"#\[\s*cfg\s*\(\s*target_os\s*\)\s*\]",
)
FORBIDDEN_CYCLE_REF = (r"\bRc\s*<", r"\bArc\s*<", r"\bRc\s*::", r"\bArc\s*::")

#: `CX-22`：值**载荷**的布局类型（`*Object` 一族）。名单**必须**都能在核心里找到声明——
#: 改名的类型会让本检查自己变红，而不是悄悄放过（见 `check_stdlib_layout_isolation`）。
LAYOUT_TYPES = (
    "BoolObject",
    "IntObject",
    "FloatObject",
    "StrObject",
    "BytesObject",
    "ListObject",
    "TupleObject",
    "DictObject",
    "SetObject",
    "SliceObject",
)
#: `CX-22`：允许出现在 `pyawa-stdlib/src/` 的两个名字（句柄与安全 API 的类型）。
LAYOUT_ALLOWED = ("Header", "Instance")

#: `CX-2` ②：任一 `README.md` 里的规格状态标注，形如 `` `docs/SPEC-*.md`（`XX-`，<状态>） ``。
SPEC_STATUS_MENTION = re.compile(r"`(docs/[A-Za-z0-9._-]+\.md)`（`([A-Z]{2})-`，([^）]+)）")

#: `CX-20`／`T-CX-11`：规格「未决」「尚未写出」节里提到 `§13-N` 的判据
GAP_SECTION = re.compile(r"^## .*(未决|尚未写出)")
SECTION_HEADING = re.compile(r"^## ")
DECIDED_HEADING = "### 已决（备查）"
DECIDED_ITEM = re.compile(r"^- \*\*(\d+)\.\*\*")
DECISION_MENTION = re.compile(r"§13-(\d+)")
#: 「历史说明」白名单：出现在该行或其**前 3 行**内即放行（`CX-20`）
DECISION_HISTORY_MARKERS = ("已决", "原先", "关闭", "不再是")
#: `CX-18`：反引号包裹的仓库内路径（只查带这些后缀的；目录与通配写法不在此列）。
DOC_PATH = re.compile(r"`(crates/[^`\s]+\.(?:rs|json|toml))`")
#: `CX-7`：`flags.rs` 的位常量声明。
FLAGS_CONST = re.compile(r"^\s*pub const ([A-Z][A-Z0-9_]*): u32\s*=\s*(.+?);\s*$")
#: `CX-7` 规则 ②：`RESERVED_MASK` 的引用只允许出现在这两个文件里。
RESERVED_MASK_ALLOWED = (
    "crates/pyawa-core/src/flags.rs",
    "crates/pyawa-core/src/header.rs",
)

TABLE_SEPARATOR = re.compile(r"^\|[\s:|-]+\|$")
BULLET_DEFINITION = r"^\s*(?:[-*+]|\d+[.)])\s+\*\*(?:~~)?\s*`?{identifier}`?\s*(?:~~)?\s*\*\*"
STRUCK_DEFINITION = r"~~\s*`?{identifier}`?\s*~~"
#: 作废但保留空号的另一种写法：删除线**套着**加粗（`~~**OM-12**~~`）
STRUCK_BOLD_DEFINITION = r"~~\s*\*\*\s*`?{identifier}`?\s*\*\*\s*~~"


# --------------------------------------------------------------------------- #
# 编号族：定义处与"尚未写出"的规格，来自 SPEC-INDEX.md §1
# --------------------------------------------------------------------------- #


def spec_index_rows() -> list[list[str]]:
    rows: list[list[str]] = []
    text = (ROOT / "docs/SPEC-INDEX.md").read_text(encoding="utf-8")
    for line in text.splitlines():
        if re.match(r"^\|\s*(?:—|\d+)\s*\|", line):
            rows.append([cell.strip().strip("`") for cell in line.strip("|").split("|")])
    return rows


def id_families() -> tuple[dict[str, str], dict[str, str]]:
    """返回 `(已写规格的编号族 → 文件, 待写规格的编号族 → 文件)`，**唯一来源是 §1 表**。

    编号族＝§1 的「ID 前缀」列去掉尾部连字符（`OM-` → `OM`）；状态为「待写」的进后者。
    """
    written: dict[str, str] = {}
    pending: dict[str, str] = {}
    for row in spec_index_rows():
        prefix = row[3].strip()
        family = prefix.rstrip("-")
        if not prefix.endswith("-") or not re.fullmatch(r"[A-Z]{2}", family):
            continue
        path = f"docs/{row[1].strip()}"
        (pending if row[-1].strip() == "待写" else written)[family] = path
    return written, pending


# --------------------------------------------------------------------------- #
# 定义位置：只有这几种写法才算"在这里立一个编号"
# --------------------------------------------------------------------------- #


def id_column_rows(text: str) -> set[int]:
    """表头**首列含「编号」**的表格，其数据行的行号（0 基）。

    只有这种表行的首列才算定义：作用域表（`CONSTRAINTS.md` §3.1）的首列也写着编号，
    但那是在说"这条约束扫哪里"，不是"在这里立这个编号"。
    """
    lines = text.splitlines()
    rows: set[int] = set()
    active = False
    for index, line in enumerate(lines):
        stripped = line.strip()
        if not stripped.startswith("|"):
            # 单元格换行（行尾带 `|` 的续行）不算表格结束——`SPEC-bytecode.md` 的
            # `T-BC-5` 就是这么折行的，若在这里把状态清掉，它后面所有行都不再算定义。
            if stripped.endswith("|"):
                continue
            active = False
            continue
        following = lines[index + 1].strip() if index + 1 < len(lines) else ""
        if TABLE_SEPARATOR.match(following):
            active = "编号" in stripped.strip("|").split("|")[0]
            continue
        if TABLE_SEPARATOR.match(stripped):
            continue
        if active:
            rows.add(index)
    return rows


def definitions_in(text: str) -> dict[str, list[int]]:
    """编号 → 定义处行号（1 基）。只认定义位置；**引用不算定义**。

    认三种写法：

    - 列表项里的加粗条目：`- **OM-7** …`、`1. **MS-20** …`
    - 删除线（作废但保留空号，`SPEC-INDEX.md` §2）：`~~OM-12~~`、`` ~~`OM-12`~~ ``、
      `~~**OM-12**~~`（删除线套加粗，两种顺序都认）
    - 表头首列含「编号」的表格，其数据行**首列**：`| T-OM-1 | …`、`` | `T-CX-1` | … ``

    不认"编号出现在 owner 文件里"——那样 owner 文件里打错一个编号，它反而成了定义，
    永远不会有检查报错。
    """
    lines = text.splitlines()
    id_rows = id_column_rows(text)
    found: dict[str, list[int]] = {}
    for index, line in enumerate(lines):
        stripped = line.strip()
        first_cell = stripped.strip("|").split("|")[0] if stripped.startswith("|") else ""
        for identifier in ANY_ID.findall(line):
            escaped = re.escape(identifier)
            defined = (
                re.match(BULLET_DEFINITION.format(identifier=escaped), line) is not None
                or re.search(STRUCK_DEFINITION.format(identifier=escaped), line) is not None
                or re.search(STRUCK_BOLD_DEFINITION.format(identifier=escaped), line) is not None
                or (
                    index in id_rows
                    and re.search(rf"(?<![\w-])`?{escaped}`?(?![\w-])", first_cell) is not None
                )
            )
            if defined:
                found.setdefault(identifier, []).append(index + 1)
    return found


# --------------------------------------------------------------------------- #
# 扫描目标
# --------------------------------------------------------------------------- #


def markdown_files() -> list[pathlib.Path]:
    files = [ROOT / "README.md", ROOT / "AGENTS.md"]
    files += sorted((ROOT / "docs").glob("*.md"))
    files += sorted((ROOT / "agents-rules").glob("*.md"))
    files += sorted((ROOT / "crates").glob("*/README.md"))
    files += sorted((ROOT / "tests").glob("*/README.md"))
    files += [ROOT / "tools" / "README.md"]
    return [f for f in files if f.is_file()]


def scanned_files() -> list[pathlib.Path]:
    """`CX-1` 的扫描面：全部人读文档 ＋ 代码里的注释与字符串。"""
    files = markdown_files()
    files += sorted((ROOT / "crates").glob("*/src/**/*.rs"))
    files += sorted((ROOT / "crates").glob("*/tests/**/*.rs"))
    files += sorted((ROOT / "tests").glob("**/*.py"))
    return [f for f in files if f.is_file()]


def readme_files() -> list[pathlib.Path]:
    """`CX-2` ② 的扫描面：**任一** `README.md`（排除构建产物与 `.git`）。"""
    return [
        path
        for path in sorted(ROOT.glob("**/README.md"))
        if "target" not in path.parts and ".git" not in path.parts
    ]


def doc_path_files() -> list[pathlib.Path]:
    """`CX-18` 的扫描面：`docs/*.md`、各 `README.md`、根 `Cargo.toml`（注释）。"""
    files = sorted((ROOT / "docs").glob("*.md")) + readme_files() + [ROOT / "Cargo.toml"]
    return [path for path in files if path.is_file()]


def relative(path: pathlib.Path) -> str:
    return path.relative_to(ROOT).as_posix()


def strip_rust(source: str) -> str:
    """去掉 Rust 的注释与字符串字面量内容，保留行号。

    注释里出现 `std::fs`、字符串里出现 `Rc<` 都不算违规——这是本检查唯一需要的词法处理。
    """
    out: list[str] = []
    i, n = 0, len(source)
    while i < n:
        pair = source[i : i + 2]
        if pair == "//":
            while i < n and source[i] != "\n":
                i += 1
        elif pair == "/*":
            depth = 1
            i += 2
            while i < n and depth:
                if source[i : i + 2] == "/*":
                    depth += 1
                    i += 2
                elif source[i : i + 2] == "*/":
                    depth -= 1
                    i += 2
                else:
                    if source[i] == "\n":
                        out.append("\n")
                    i += 1
        elif source[i] == '"':
            i += 1
            while i < n:
                if source[i] == "\\":
                    i += 2
                elif source[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
            out.append('""')
        else:
            out.append(source[i])
            i += 1
    return "".join(out)


# --------------------------------------------------------------------------- #
# T-CX-1 编号零悬空（CX-1）
# --------------------------------------------------------------------------- #


def collect_defined_ids() -> tuple[set[str], list[str]]:
    """返回 `(已定义的编号, 重复定义的报告)`。"""
    written, _ = id_families()
    defined: set[str] = set()
    duplicates: list[str] = []

    for family, path in written.items():
        text = (ROOT / path).read_text(encoding="utf-8")
        for identifier, lines in definitions_in(text).items():
            if not identifier.removeprefix("T-").startswith(f"{family}-"):
                continue  # 别的族出现在本文里，那是引用，不是定义
            if len(lines) > 1:
                duplicates.append(f"{path}: {identifier} 被定义了 {len(lines)} 次（行 {lines}）")
            defined.add(identifier)

    design = (ROOT / "docs/DESIGN.md").read_text(encoding="utf-8")
    section = design[design.index("## 13. 未决项汇总") :]
    defined |= {f"§13-{num}" for num in re.findall(r"^- \*\*(\d+)\.\*\*", section, re.M)}
    return defined, duplicates


def check_spec_ids() -> list[str]:
    defined, duplicates = collect_defined_ids()
    _, pending = id_families()
    failures: list[str] = list(duplicates)

    for path in scanned_files():
        for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for reference in REFERENCE.findall(line):
                if reference in defined:
                    continue
                if reference.startswith("§13-"):
                    failures.append(
                        f"{relative(path)}:{lineno}: {reference} 不在 DESIGN.md §13 的编号里"
                    )
                    continue
                family = reference.removeprefix("T-").split("-")[0]
                if family in pending:
                    if not any(marker in line for marker in ANNOTATION_MARKERS):
                        failures.append(
                            f"{relative(path)}:{lineno}: {reference} 属未写规格"
                            f"（{pending[family]}），必须标注临时假设"
                        )
                else:
                    failures.append(f"{relative(path)}:{lineno}: {reference} 无定义处")
    return failures


# --------------------------------------------------------------------------- #
# T-CX-2 文档集状态一致（CX-2）
# --------------------------------------------------------------------------- #


def check_doc_status() -> list[str]:
    rows = spec_index_rows()
    if not rows:
        return ["docs/SPEC-INDEX.md §1：一行都没解析到"]

    written = sum(1 for row in rows if row[-1] != "待写")
    pending = len(rows) - written

    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    failures: list[str] = []

    counted = re.search(r"已写\s*(\d+)\s*份\s*/\s*待写\s*(\d+)\s*份", readme)
    if counted is None:
        failures.append("README.md：找不到「已写 N 份 / 待写 M 份」计数")
    else:
        if int(counted.group(1)) != written:
            failures.append(
                f"README.md 说已写 {counted.group(1)} 份，SPEC-INDEX §1 数出来是 {written} 份"
            )
        if int(counted.group(2)) != pending:
            failures.append(
                f"README.md 说待写 {counted.group(2)} 份，SPEC-INDEX §1 数出来是 {pending} 份"
            )

    total = re.search(r"共\s*(\d+)\s*份", readme)
    if total is not None and int(total.group(1)) != len(rows):
        failures.append(f"README.md 说共 {total.group(1)} 份，SPEC-INDEX §1 有 {len(rows)} 行")

    # ② 任一 README 标的规格状态，必须与 §1 一致（CX-2）
    index = {
        row[1].split("/")[-1]: (row[3].rstrip("-"), row[-1])
        for row in rows
        if row[3].endswith("-")
    }
    for path in readme_files():
        for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for mentioned, prefix, status in SPEC_STATUS_MENTION.findall(line):
                entry = index.get(mentioned.split("/")[-1])
                if entry is None:
                    failures.append(
                        f"{relative(path)}:{lineno}: 标注了 {mentioned}，但它不在 SPEC-INDEX §1 里"
                    )
                    continue
                if entry[0] != prefix:
                    failures.append(
                        f"{relative(path)}:{lineno}: {mentioned} 的前缀写作 `{prefix}-`，"
                        f"§1 里是 `{entry[0]}-`"
                    )
                if entry[1] != status.strip():
                    failures.append(
                        f"{relative(path)}:{lineno}: {mentioned} 的状态写作 {status.strip()}，"
                        f"§1 里是 {entry[1]}"
                    )
    return failures


# --------------------------------------------------------------------------- #
# T-CX-3 / T-CX-4 / T-CX-5 静态扫描（CX-3 / CX-4 / CX-6）
# --------------------------------------------------------------------------- #


def scan_crates(crates: tuple[str, ...], patterns: tuple[str, ...]) -> list[str]:
    compiled = [re.compile(pattern) for pattern in patterns]
    failures: list[str] = []
    for crate in crates:
        source_dir = ROOT / crate / "src"
        if not source_dir.is_dir():
            failures.append(f"{crate}/src 不存在（扫描范围要跟着 crate 布局更新，见 CONSTRAINTS §3.1）")
            continue
        for path in sorted(source_dir.rglob("*.rs")):
            stripped = strip_rust(path.read_text(encoding="utf-8"))
            for lineno, line in enumerate(stripped.splitlines(), 1):
                for pattern in compiled:
                    if pattern.search(line):
                        failures.append(
                            f"{relative(path)}:{lineno}: 命中 {pattern.pattern}（{line.strip()}）"
                        )
    return failures


def check_global_mutable_state() -> list[str]:
    return scan_crates(VM_CORE_CRATES, FORBIDDEN_GLOBAL_STATE)


def check_platform_dependencies() -> list[str]:
    return scan_crates(VM_CORE_CRATES + INTERFACE_CRATES + STDLIB_CRATES, FORBIDDEN_PLATFORM)


def check_cycle_reference_types() -> list[str]:
    return scan_crates(VM_CORE_CRATES, FORBIDDEN_CYCLE_REF)


# --------------------------------------------------------------------------- #
# T-CX-13 stdlib 布局隔离（CX-22）
# --------------------------------------------------------------------------- #


def check_stdlib_layout_isolation() -> list[str]:
    """`CX-22`：`crates/pyawa-stdlib/src/` 不得出现**载荷布局类型名**。

    范围（`CONSTRAINTS.md` §3.1）：只扫 `pyawa-stdlib` 的 `src/`；`Header`／`Instance` 允许。
    名单与核心的声明**互相钉住**：名单里的每个名字都必须在 `pyawa-core/src/` 里有
    `pub struct <名字>` —— 否则报"名单腐烂"，免得改名后检查悄悄变绿。
    """
    failures: list[str] = []
    core_source = ""
    for path in sorted((ROOT / "crates/pyawa-core/src").rglob("*.rs")):
        core_source += path.read_text(encoding="utf-8")
    for name in LAYOUT_TYPES:
        if name in LAYOUT_ALLOWED:
            continue
        if not re.search(rf"pub struct {name}\b", core_source):
            failures.append(
                f"`CX-22` 名单里的 {name} 在 `crates/pyawa-core/src/` 里没有 `pub struct` 声明"
                f"（改名了？名单要跟着改，否则这条检查会悄悄放过）"
            )
    patterns = [re.compile(rf"\b{name}\b") for name in LAYOUT_TYPES]
    for crate in STDLIB_CRATES:
        source_dir = ROOT / crate / "src"
        if not source_dir.is_dir():
            failures.append(f"{crate}/src 不存在（扫描范围见 CONSTRAINTS §3.1）")
            continue
        for path in sorted(source_dir.rglob("*.rs")):
            stripped = strip_rust(path.read_text(encoding="utf-8"))
            for lineno, line in enumerate(stripped.splitlines(), 1):
                for pattern in patterns:
                    if pattern.search(line):
                        failures.append(
                            f"{relative(path)}:{lineno}: 出现载荷布局类型 {pattern.pattern} "
                            f"（`CX-22`：stdlib 只能走核心的安全入口；{line.strip()}）"
                        )
    return failures


# --------------------------------------------------------------------------- #
# T-CX-8 文档引用的仓库内路径存在（CX-18）
# --------------------------------------------------------------------------- #


def check_doc_paths() -> list[str]:
    """`CX-18`：反引号包裹的 `crates/….rs|json|toml` 路径**必须**存在。"""
    failures: list[str] = []
    for path in doc_path_files():
        for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for referenced in DOC_PATH.findall(line):
                if not (ROOT / referenced).exists():
                    failures.append(f"{relative(path)}:{lineno}: {referenced} 不存在")
    return failures


# --------------------------------------------------------------------------- #
# T-CX-9 flags 的预留位（CX-7）
# --------------------------------------------------------------------------- #


#: `CX-7` ①：位常量的取值**必须可静态求值**——字面量、字面量之间的
#: `<<`／`>>`／`|`／`&`／`^`／`!`／`~`、以及对 `flags.rs` 内其他常量的引用。
#: 这是"保持可判定写法"的要求，**不是**"只准写某几种形态"（见 `CONSTRAINTS.md` 的 `CX-7`）。
FLAGS_TOKEN = re.compile(r"\s*(<<|>>|[|&^!~()]|0[xX][0-9a-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|[0-9][0-9_]*|[A-Za-z_][A-Za-z0-9_]*)")
FLAGS_BITS = 32
FLAGS_MASK = (1 << FLAGS_BITS) - 1


def flags_literal(token: str) -> int | None:
    """Rust 风格整数字面量（含 `_` 分隔与 `0b`／`0o`／`0x` 前缀）。"""
    try:
        return int(token.replace("_", ""), 0)
    except ValueError:
        return None


class FlagsExpression:
    """`CX-7` ① 的求值器：够用的表达式子集，**求值不了就报红**。

    只认整数字面量、`flags.rs` 内已声明的常量、以及 `<< >> | & ^ ! ~ ( )`——
    这样"可判定"与"不许写不透明字面量"两头都守住。
    """

    def __init__(self, expression: str, known: dict[str, int]) -> None:
        self.tokens = self._tokenize(expression)
        self.position = 0
        self.known = known
        self.failed = False

    @staticmethod
    def _tokenize(expression: str) -> list[str] | None:
        tokens: list[str] = []
        rest = expression
        while rest.strip():
            matched = FLAGS_TOKEN.match(rest)
            if matched is None or not matched.group(0).strip():
                return None
            tokens.append(matched.group(1))
            rest = rest[matched.end():]
        return tokens

    def peek(self) -> str | None:
        return self.tokens[self.position] if self.position < len(self.tokens) else None

    def take(self) -> str | None:
        token = self.peek()
        self.position += 1
        return token

    def evaluate(self) -> int | None:
        if self.tokens is None:
            return None
        value = self.expression()
        if self.failed or self.position != len(self.tokens):
            return None
        return value

    def operand(self) -> int | None:
        token = self.take()
        if token is None:
            self.failed = True
            return None
        if token == "(":
            inner = self.expression()
            if self.take() != ")":
                self.failed = True
                return None
            return inner
        if token in ("!", "~"):
            inner = self.operand()
            return None if inner is None else (~inner) & FLAGS_MASK
        literal = flags_literal(token)
        if literal is not None:
            return literal if 0 <= literal <= FLAGS_MASK else None
        if token in self.known:
            return self.known[token]
        self.failed = True
        return None

    def binary(self, operators: tuple[str, ...], next_level) -> int | None:
        left = next_level()
        if left is None:
            return None
        while self.peek() in operators:
            operator = self.take()
            right = next_level()
            if right is None:
                return None
            if operator in ("<<", ">>") and not 0 <= right < FLAGS_BITS:
                return None
            if operator == "<<":
                left <<= right
            elif operator == ">>":
                left >>= right
                left &= FLAGS_MASK
            elif operator == "|":
                left |= right
            elif operator == "&":
                left &= right
            elif operator == "^":
                left ^= right
            left &= FLAGS_MASK
        return left

    def expression(self) -> int | None:
        def shift():
            return self.binary(("<<", ">>"), self.operand)

        def bitand():
            return self.binary(("&",), shift)

        def bitxor():
            return self.binary(("^",), bitand)

        def bitor():
            return self.binary(("|",), bitxor)

        return bitor()


def evaluate_flags_expression(expression: str, known: dict[str, int]) -> int | None:
    """能静态求值就给出 `u32` 取值，否则 `None`（`CX-7` ①：求值不了即红）。"""
    return FlagsExpression(expression, known).evaluate()


def check_flags_reserved() -> list[str]:
    flags_path = ROOT / "crates/pyawa-core/src/flags.rs"
    failures: list[str] = []
    constants: dict[str, int] = {}

    for lineno, line in enumerate(flags_path.read_text(encoding="utf-8").splitlines(), 1):
        matched = FLAGS_CONST.match(line)
        if matched is None:
            continue
        name, expression = matched.group(1), matched.group(2)
        value = evaluate_flags_expression(expression, constants)
        if value is None:
            failures.append(
                f"{relative(flags_path)}:{lineno}: {name} 的取值不可求值"
                "（CX-7 只认字面量／`1 << N`／已有常量的或）"
            )
            continue
        constants[name] = value

    reserved = constants.get("RESERVED_MASK")
    if reserved is None:
        failures.append(f"{relative(flags_path)}: 找不到可求值的 RESERVED_MASK")
    else:
        for name, value in constants.items():
            if name == "RESERVED_MASK":
                continue
            if value & reserved:
                failures.append(
                    f"{relative(flags_path)}: {name} 与 RESERVED_MASK 相交"
                    f"（{value:#010b} & {reserved:#010b}）"
                )

    # ② RESERVED_MASK 的引用只允许出现在 flags.rs 与 header.rs
    for crate in VM_CORE_CRATES:
        for path in sorted((ROOT / crate / "src").rglob("*.rs")):
            if relative(path) in RESERVED_MASK_ALLOWED:
                continue
            body = strip_rust(path.read_text(encoding="utf-8"))
            for lineno, line in enumerate(body.splitlines(), 1):
                if "RESERVED_MASK" in line:
                    failures.append(
                        f"{relative(path)}:{lineno}: RESERVED_MASK 的引用只允许出现在 "
                        + "／".join(RESERVED_MASK_ALLOWED)
                        + "（CX-7）"
                    )
    return failures


# --------------------------------------------------------------------------- #
# T-CX-7 每份已写规格都有「尚未写出」节（CX-17）
# --------------------------------------------------------------------------- #

GAP_SECTION = re.compile(r"^#{2,4}\s*\d+(?:\.\d+)?\.?\s*尚未写出", re.M)


def check_gap_sections() -> list[str]:
    """`SPEC-INDEX.md` §5 第 6 条：**没有缺口也必须显式写「无」**。"""
    written, _ = id_families()
    failures: list[str] = []
    for family, path in sorted(written.items()):
        text = (ROOT / path).read_text(encoding="utf-8")
        if not GAP_SECTION.search(text):
            failures.append(f"{path}（{family}-）缺「尚未写出」节")
    return failures


# --------------------------------------------------------------------------- #
# T-CX-6 占位账本与 README 状态节一致（CX-14）
# --------------------------------------------------------------------------- #

LANDING_TABLE_ROW = re.compile(r"^\|\s*\*\*(CX-\d+)\*\*\s*\|")
LEDGER_ROW = re.compile(r"^\|\s*`(CX-\d+)`\s*\|\s*(已实现|未实现)\s*\|\s*(.*?)\s*\|")


def parse_landing() -> dict[str, str]:
    """`CONSTRAINTS.md` §3：每条约束现在能否落地。"""
    landing: dict[str, str] = {}
    for line in (ROOT / "docs/CONSTRAINTS.md").read_text(encoding="utf-8").splitlines():
        matched = LANDING_TABLE_ROW.match(line)
        if matched is None:
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        landing[matched.group(1)] = "不能" if "不能" in cells[-1] else "能"
    return landing


def parse_ledger() -> dict[str, tuple[str, str]]:
    """`tests/ci/README.md` 的状态账本：CX 编号 → （状态，备注）。"""
    ledger: dict[str, tuple[str, str]] = {}
    for line in (ROOT / "tests/ci/README.md").read_text(encoding="utf-8").splitlines():
        matched = LEDGER_ROW.match(line)
        if matched is not None:
            ledger[matched.group(1)] = (matched.group(2), matched.group(3))
    return ledger


def check_numbering_continuity() -> list[str]:
    """**`T-CX-10`**：各前缀的**已定义**编号从 1 连续到最大值（`CX-19`）。

    - 只认**定义处**（`collect_defined_ids`），引用不算；删除线墓碑（`~~AB-42~~`）算定义
    - 族按"前缀"分：`AB`／`BC`／…／`CX` 与 `T-AB`／`T-BC`／…（`SPEC-INDEX.md` §2）
    - `DESIGN.md` §13 的未决项编号（`§13-4` 这种）同样纳入
    - 缺号必须留墓碑：没有墓碑的缺号会让读者以为丢了条目
    """
    defined, _ = collect_defined_ids()
    families: dict[str, set[int]] = collections.defaultdict(set)
    for identifier in defined:
        prefix, _, number = identifier.rpartition("-")
        if prefix and number.isdigit():
            families[prefix].add(int(number))

    failures: list[str] = []
    for prefix in sorted(families):
        numbers = families[prefix]
        highest = max(numbers)
        missing = [number for number in range(1, highest + 1) if number not in numbers]
        if missing:
            failures.append(
                f"{prefix}：已定义到 {prefix}-{highest}，但缺 "
                f"{', '.join(f'{prefix}-{number}' for number in missing)}"
                "（作废必须留 ~~ID~~ 墓碑，墓碑算定义；CX-19）"
            )
    return failures


def collect_decided_items() -> set[int]:
    """`DESIGN.md` §13 的「已决（备查）」清单里的未决项编号（`CX-20` 的判据基准）。

    清单的权威形态就是那小节的标题本身（`DESIGN.md` §13 的引言说已决项见文末"已决"），
    故只认 `### 已决（备查）` 起、到下一个二／三级标题之间的 `- **N.**` 条目。
    """
    decided: set[int] = set()
    lines = (ROOT / "docs/DESIGN.md").read_text(encoding="utf-8").splitlines()
    try:
        start = next(
            index for index, line in enumerate(lines) if line.startswith(DECIDED_HEADING)
        )
    except StopIteration:
        return decided
    for line in lines[start + 1 :]:
        if line.startswith("## ") or line.startswith("### "):
            break
        matched = DECIDED_ITEM.match(line)
        if matched is not None:
            decided.add(int(matched.group(1)))
    return decided


def check_fake_gaps() -> list[str]:
    """**`T-CX-11`**：规格的「未决」「尚未写出」节**不得**把**已决**的 `§13-N` 列成待定（`CX-20`）。

    判据（`CX-20` 的原文）：该行或其**前 3 行**内出现"已决／原先／关闭／不再是"之一，
    即视为**历史说明**（如"原先列的…均已决"），放行；否则报红。
    白名单是必需的——首次扫描的 27 处命中里大量是合法的历史说明，不带白名单会误报。
    """
    decided = collect_decided_items()
    failures: list[str] = []
    for path in sorted((ROOT / "docs").glob("SPEC-*.md")):
        lines = path.read_text(encoding="utf-8").splitlines()
        in_gap_section = False
        for index, line in enumerate(lines):
            if SECTION_HEADING.match(line):
                in_gap_section = bool(GAP_SECTION.match(line))
                continue
            if not in_gap_section:
                continue
            context = lines[max(0, index - 3) : index + 1]
            if any(marker in entry for entry in context for marker in DECISION_HISTORY_MARKERS):
                continue
            for number in DECISION_MENTION.findall(line):
                if int(number) in decided:
                    failures.append(
                        f"{path.name}:{index + 1}: `§13-{number}` 已在 `DESIGN.md` §13「已决」清单里，"
                        "不得当作待定；若是历史说明，该行或前 3 行内要有"
                        "「已决／原先／关闭／不再是」（CX-20）"
                    )
    return failures


def check_placeholder_ledger(implemented: set[str], defined: set[str]) -> list[str]:
    landing = parse_landing()
    ledger = parse_ledger()
    failures: list[str] = []

    if not landing:
        return ["docs/CONSTRAINTS.md §3：一条约束都没解析到"]
    if not ledger:
        return ["tests/ci/README.md：状态账本一条都没解析到"]

    for cx in sorted(ledger):
        if cx not in landing:
            failures.append(f"tests/ci/README.md: {cx} 在 CONSTRAINTS.md §3 里不存在")

    for cx in sorted(implemented & set(landing)):
        if landing[cx] != "能":
            failures.append(f"{cx} 被标为「{landing[cx]}」，脚本不得实现它（CX-14）")

    for cx, (status, note) in sorted(ledger.items()):
        if landing.get(cx) == "不能" and status == "已实现":
            failures.append(f"{cx} 还不能落地，账本不得写「已实现」（CX-14）")
        if status == "已实现" and cx not in implemented:
            # 允许"由别处承担"（如 Rust 侧 `T-OM-9`），但 note 必须点名一条**真实存在**的验收编号
            carried = [identifier for identifier in ANY_ID.findall(note) if identifier in defined]
            if not carried:
                failures.append(
                    f"{cx} 账本写「已实现」，但脚本里没有对应检查，"
                    "note 也没点名一条真实存在的验收编号（CX-14／CX-15）"
                )
        if status == "未实现" and not note:
            failures.append(f"{cx} 标「未实现」却没写理由")

    for cx, ability in sorted(landing.items()):
        if ability == "能" and cx not in implemented and cx not in ledger:
            failures.append(f"{cx} 标「能」却既未实现、也没在账本里写明状态（CX-14）")

    return failures


# --------------------------------------------------------------------------- #
# 驱动
# --------------------------------------------------------------------------- #


@dataclasses.dataclass(frozen=True)
class Check:
    test_id: str
    cx: tuple[str, ...]
    summary: str
    run: Callable[[], list[str]]


def build_checks() -> list[Check]:
    checks = [
        Check("T-CX-1", ("CX-1",), "编号零悬空（全库引用）＋ 重复定义", check_spec_ids),
        Check("T-CX-2", ("CX-2",), "文档集状态：SPEC-INDEX §1 ↔ README", check_doc_status),
        Check("T-CX-3", ("CX-3",), "禁全局可变状态（VM 核心 crate 的 src/）", check_global_mutable_state),
        Check("T-CX-4", ("CX-4",), "无平台依赖（VM 核心 ＋ 能力接口 ＋ C 层模块 crate 的 src/）", check_platform_dependencies),
        Check("T-CX-5", ("CX-6",), "禁 Rc／Arc 作对象引用（VM 核心 crate 的 src/）", check_cycle_reference_types),
        Check("T-CX-7", ("CX-17",), "每份已写规格都有「尚未写出」节", check_gap_sections),
        Check("T-CX-8", ("CX-18",), "文档引用的仓库内路径存在", check_doc_paths),
        Check("T-CX-9", ("CX-7",), "flags 位常量不与 RESERVED_MASK 相交 ＋ 引用白名单", check_flags_reserved),
        Check(
            "T-CX-10",
            ("CX-19",),
            "各前缀已定义编号从 1 连续到最大值（墓碑算定义），无未解释缺号",
            check_numbering_continuity,
        ),
        Check(
            "T-CX-13",
            ("CX-22",),
            "stdlib 布局隔离：pyawa-stdlib/src/ 不出现载荷布局类型名（Header/Instance 允许）",
            check_stdlib_layout_isolation,
        ),
        Check(
            "T-CX-11",
            ("CX-20",),
            "规格的「未决」「尚未写出」节不得把已决的 §13-N 列成待定（历史说明靠白名单放行）",
            check_fake_gaps,
        ),
    ]
    implemented = {cx for check in checks for cx in check.cx}
    checks.append(
        Check(
            "T-CX-6",
            ("CX-14",),
            "占位账本与 CONSTRAINTS §3 的落地状态一致",
            lambda: check_placeholder_ledger(implemented, collect_defined_ids()[0]),
        )
    )
    return checks


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="tests/ci 的静态检查")
    parser.add_argument("--list", action="store_true", help="只列出检查项，不执行")
    args = parser.parse_args(argv)

    checks = build_checks()
    if args.list:
        for check in checks:
            print(f"{check.test_id}  {','.join(check.cx):12}  {check.summary}")
        return 0

    print(f"tests/ci 检查 · 仓库根 {ROOT}")
    failed = 0
    for check in checks:
        failures = check.run()
        status = "PASS" if not failures else "FAIL"
        print(f"[{status}] {check.test_id} ({','.join(check.cx)}) {check.summary}")
        for failure in failures:
            print(f"       - {failure}")
        failed += bool(failures)

    print(f"\n共 {len(checks)} 项，失败 {failed} 项")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
