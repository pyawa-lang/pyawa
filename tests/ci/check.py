#!/usr/bin/env python3
"""tests/ci —— 不变量与 CI 强制项的检查实现。

约束的**定义**（编号、出处、现在能否落地）唯一出处是 `docs/CONSTRAINTS.md`（`CX-`），
本脚本只负责**实现**检查，不重述约束内容。

用法::

    python3 tests/ci/check.py           # 跑全部检查；有失败则退出码 1
    python3 tests/ci/check.py --list    # 列出检查项、对应的 CX 编号与扫描范围

扫描范围由 `CONSTRAINTS.md` §3.1 规定（`T-CX-3`／`T-CX-4`／`T-CX-5` 只扫各 crate 的 `src/`）。
"""

from __future__ import annotations

import argparse
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
#: `CX-4`：还要加上能力接口 crate。
INTERFACE_CRATES = ("crates/pyawa-capabilities",)

FORBIDDEN_GLOBAL_STATE = (r"\bstatic\s+mut\b", r"\bthread_local\b")
FORBIDDEN_PLATFORM = (
    r"\bstd::fs\b",
    r"\bstd::net\b",
    r"\blibc\b",
    r"#\[\s*cfg\s*\(\s*target_os\s*\)\s*\]",
)
FORBIDDEN_CYCLE_REF = (r"\bRc\s*<", r"\bArc\s*<", r"\bRc\s*::", r"\bArc\s*::")

TABLE_SEPARATOR = re.compile(r"^\|[\s:|-]+\|$")
BULLET_DEFINITION = r"^\s*(?:[-*+]|\d+[.)])\s+\*\*(?:~~)?\s*`?{identifier}`?\s*(?:~~)?\s*\*\*"
STRUCK_DEFINITION = r"~~\s*`?{identifier}`?\s*~~"


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
    - 删除线（作废但保留空号，`SPEC-INDEX.md` §2）：`~~OM-12~~`、`` ~~`OM-12`~~ ``
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
    return scan_crates(VM_CORE_CRATES + INTERFACE_CRATES, FORBIDDEN_PLATFORM)


def check_cycle_reference_types() -> list[str]:
    return scan_crates(VM_CORE_CRATES, FORBIDDEN_CYCLE_REF)


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


def check_placeholder_ledger(implemented: set[str]) -> list[str]:
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
            failures.append(f"{cx} 账本写「已实现」，但脚本里没有对应检查（CX-15）")
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
        Check("T-CX-4", ("CX-4",), "无平台依赖（VM 核心 ＋ 能力接口 crate 的 src/）", check_platform_dependencies),
        Check("T-CX-5", ("CX-6",), "禁 Rc／Arc 作对象引用（VM 核心 crate 的 src/）", check_cycle_reference_types),
        Check("T-CX-7", ("CX-17",), "每份已写规格都有「尚未写出」节", check_gap_sections),
    ]
    implemented = {cx for check in checks for cx in check.cx}
    checks.append(
        Check(
            "T-CX-6",
            ("CX-14",),
            "占位账本与 CONSTRAINTS §3 的落地状态一致",
            lambda: check_placeholder_ledger(implemented),
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
