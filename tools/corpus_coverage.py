#!/usr/bin/env python3
"""**判据② 可裁定**的覆盖率仪器（`MS-13` ② ＋ `MS-14`；`P3-27` 的靶子由它给出）。

判据②（`docs/PLAN-milestones.md` §6 的 `M3` 行）＝**对拍无新差异**，且语料必须含
`MS-13` ② 的 **`Lib/` 语料**。本脚本把"够不够"变成**可机械检查**的五个计数/要素
（逐项 `[PASS|FAIL]`；任一 FAIL ⇒ 退出码 1）：

  ① **语料总量 ≥ 400**（读 `tests/conformance/corpus/manifest.tsv` 的数据行）
  ② **`Lib/` 模块覆盖 ≥ 150 个不同模块**（模块名由**来源**推出；`lib_<module>.py` 命名启发式
     不可靠——`collections_*` 四份只覆盖一个模块、`datetime_*` 两份一个模块，见 `--verbose`）
  ③ **`Lib/test/` 来源用例 ≥ 40 条**（来源落在 `Lib/test/` 下的语料条数）
  ④ **两侧对拍口径体检**：`target/conformance/self-check.md` 的 `MS-12` 自检**全绿** ＋
     `target/conformance/report.md` 的「新差异」计数为 **0**
  ⑤ **报告可发布（`MS-14`）四要素**：参照版本／语料清单与模式／通过-已知差异-新差异计数／
     差异清单快照

## 「来源」的解析（**一处真相**：manifest 第 5 列，Lead 已裁定 **格式 A**）

"这条语料断言哪个 `Lib/` 模块"**不能**靠用例名猜。Lead 裁定的唯一来源格式是
**manifest 第 5 列「来源」**（harness 侧已放宽为「4 或 5 列」，该列**不参与对拍比对**）：

- 取值：`Lib/<相对路径>.py`（`__init__` 归包名；可带 `#L12-L30`）；
- **自建语料**（`MS-13` ①）写 `-`；
- **每一行都必须有第 5 列**（迁移期缺列 ⇒ ②③ 判**不达标**，输出里给出待补条数）。

只有一种**兼容路径**（**不是**格式承诺、不计入 PASS）：老语料首行是散文
（`# MS-13 ② 的 Lib/ 语料：逐字同步的 Lib/...`），本仪器仍能**识别**它来量"现在到哪了"，
但 ②③ 只认第 5 列 ⇒ 缺列时计为不达标。补上第 5 列后 legacy 路径自然失效。

## 用法

    python3 tools/corpus_coverage.py            # 机检（非零退出＝不达标）；顺带刷新覆盖基线
    python3 tools/corpus_coverage.py --verbose  # 加：逐模块覆盖表（哪些有语料／哪些没有）

每次运行都会把"当前覆盖表"刷新到 `target/recon/corpus_coverage_baseline.txt`（给第三批写手当靶子）。
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CORPUS = ROOT / "tests" / "conformance" / "corpus"
MANIFEST = CORPUS / "manifest.tsv"
REPORT = ROOT / "target" / "conformance" / "report.md"
SELF_CHECK = ROOT / "target" / "conformance" / "self-check.md"
BASELINE = ROOT / "target" / "recon" / "corpus_coverage_baseline.txt"

#: 判据② 的三个阈值（`PLAN-milestones.md` §6 的 `M3` 行；Lead 口径）。
TOTAL_TARGET = 400
MODULE_TARGET = 150
LIBTEST_TARGET = 40

#: 来源标记：`Lib/<路径>` ＋ 可选行区间 `#L12-L30`。
LIB_SOURCE = re.compile(r"^Lib/(?P<path>[^#]+?)(?P<lines>#L\d+(?:-L?\d+)?)?$")
#: legacy 散文惯例里的 `Lib/...`（**仅**基线估算；不是格式承诺，见文件头）。
LEGACY_PATH = re.compile(r"`(Lib/[^`]+)`")

#: `MS-14` 报告的四要素（conformance.rs 的落盘格式，见 `target/conformance/report.md`）。
REFERENCE_LINE = re.compile(r"^- 参照实现：\*\*(?P<version>[^*]+)\*\*", re.MULTILINE)
SELF_CHECK_LINE = re.compile(r"^- 被测侧：\*\*自检\*\*", re.MULTILINE)
SNAPSHOT_LINE = re.compile(r"^- 差异清单快照：(?P<snapshot>.+)$", re.MULTILINE)
COUNT_LINE = re.compile(
    r"^\*\*计数\*\*：共 (?P<total>\d+) ⇒ 通过 \*\*(?P<pass>\d+)\*\*"
    r" · 已知差异 \*\*(?P<known>\d+)\*\* · 新差异 \*\*(?P<new>\d+)\*\*",
    re.MULTILINE,
)
TABLE_ROW = re.compile(r"^\| `(?P<name>[^`]+)` \| (?P<mode>[^|]*?) \|", re.MULTILINE)


# --------------------------------------------------------------------------- #
# 清单与来源
# --------------------------------------------------------------------------- #


@dataclasses.dataclass
class ManifestRow:
    """manifest 的一行数据（4 列表头 ＋ 可选的**第 5 列「来源」**）。"""

    line: int
    name: str
    mode: str
    #: 第 5 列原文（`Lib/<路径>.py` 或 `-`）；4 列行（迁移期）为 None。
    source_column: str | None
    width: int


@dataclasses.dataclass
class CorpusCase:
    """一条语料的来源解析结果。"""

    name: str
    file: str
    mode: str
    #: `explicit`（manifest 第 5 列）／`legacy`（散文惯例，仅基线）／`self`（自建）／`missing`
    source_kind: str
    #: 显式来源的原文（自建为 `-`）；legacy 给解析到的路径；`missing` 为 None。
    source: str | None
    #: 从来源推出的模块名（`Lib/test/...` 也给出来，由调用方按前缀分流）。
    module: str | None


def read_comment_block(path: pathlib.Path) -> list[str]:
    """文件开头的连续注释块（跳过空行；遇到第一行代码即停）。"""
    block: list[str] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return block
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("#"):
            block.append(line)
        elif stripped == "":
            continue
        else:
            break
    return block


def legacy_source(path: pathlib.Path) -> str | None:
    """legacy 散文惯例：首个注释块里第一个**像来源路径**的 `` `Lib/...` ``（**仅** `lib_*`／`libtest_*`）。

    散文里常先出现裸的 `` `Lib/` ``（"`Lib/` 语料"）⇒ 必须跳过它，只认 `.py` 结尾或
    至少两级目录的候选（`Lib/pathlib/`）。**这不是格式承诺**，只用于基线估算。
    """
    if not (path.name.startswith("lib_") or path.name.startswith("libtest_")):
        return None
    for line in read_comment_block(path):
        for match in LEGACY_PATH.finditer(line):
            candidate = match.group(1)
            if candidate == "Lib/":
                continue
            if candidate.endswith(".py") or candidate.count("/") >= 2:
                return candidate
    return None


def parse_source(raw: str) -> tuple[str, str | None]:
    """把来源原文解析成 (种类, 模块名)。

    种类：`self`（`-`）／`lib`（`Lib/` 下的非 test 模块）／`libtest`（`Lib/test/` 来源）／
    `bad`（格式不认识）。
    """
    value = raw.strip()
    if value in ("-", ""):
        return "self", None
    if "," in value:
        # 一条语料只能断言一个来源；要多来源就拆成多条语料（保持 1:1，别造第二真相）
        return "bad", None
    match = LIB_SOURCE.match(value)
    if not match:
        return "bad", None
    relative = match.group("path").strip()
    if not relative.endswith(".py"):
        if relative.endswith("/"):
            relative += "__init__.py"
        else:
            return "bad", None
    parts = relative[: -len(".py")].split("/")
    if parts and parts[-1] == "__init__":
        parts = parts[:-1]
    if not parts or any(not part for part in parts):
        return "bad", None
    module = ".".join(parts)
    if module == "test" or module.startswith("test."):
        return "libtest", module
    return "lib", module


def parse_manifest() -> tuple[list[ManifestRow], list[str]]:
    """读 manifest ⇒ (数据行, 错误)。

    允许**迁移期混排**（部分行 4 列、部分行 5 列）：行宽只接受 4 或 5；
    4 列行没有「来源」列，②③ 会把它计入"待补"（`format_ok` 不成立）。
    """
    rows: list[ManifestRow] = []
    errors: list[str] = []
    if not MANIFEST.is_file():
        return rows, [f"读不到语料清单：{MANIFEST.relative_to(ROOT)}"]
    for number, line in enumerate(MANIFEST.read_text(encoding="utf-8").splitlines(), 1):
        line = line.rstrip()
        if not line or line.startswith("#"):
            continue
        cells = line.split("\t")
        if len(cells) not in (4, 5):
            errors.append(f"manifest 第 {number} 行：{len(cells)} 列（期望 4 或 5）")
            continue
        rows.append(
            ManifestRow(
                line=number,
                name=cells[0].strip(),
                mode=cells[1].strip(),
                source_column=cells[4].strip() if len(cells) == 5 else None,
                width=len(cells),
            )
        )
    return rows, errors


def resolve_case(row: ManifestRow) -> tuple[CorpusCase, list[str]]:
    """一条 manifest 行 ⇒ 来源解析（**第 5 列是唯一真相**；legacy 仅基线；缺列标 `missing`）。"""
    path = CORPUS / row.name
    if not path.is_file():
        return CorpusCase(row.name, row.name, row.mode, "missing", None, None), [
            f"manifest 第 {row.line} 行：语料文件不存在 {row.name}"
        ]
    errors: list[str] = []
    legacy = legacy_source(path)

    if row.source_column is not None:
        if row.source_column == "":
            errors.append(f"{row.name}：manifest 的「来源」列为空（自建语料请显式写 `-`）")
            return CorpusCase(row.name, row.name, row.mode, "missing", None, None), errors
        kind, module = parse_source(row.source_column)
        if kind == "bad":
            errors.append(
                f"{row.name}：来源 {row.source_column!r} 不可机械解析"
                "（期望 `Lib/<相对路径>.py` 或 `-`）"
            )
            return CorpusCase(row.name, row.name, row.mode, "missing", row.source_column, None), errors
        # 第 5 列是真相：`-`（自建）也给 source_kind=explicit，但不计入 ②③
        return CorpusCase(
            row.name, row.name, row.mode, "explicit", row.source_column, module
        ), errors

    # 没有第 5 列（迁移期 4 列行）⇒ 只能 legacy 估算；**不计入 PASS**
    if legacy is not None:
        kind, module = parse_source(legacy)
        if kind in ("lib", "libtest"):
            return CorpusCase(row.name, row.name, row.mode, "legacy", legacy, module), errors

    if row.name.startswith(("lib_", "libtest_")):
        errors.append(
            f"{row.name}：缺第 5 列「来源」，且 legacy 首行也解析不出 `Lib/...`"
            "（格式 A：Lib 语料写 `Lib/<路径>.py`、自建写 `-`）"
        )
        return CorpusCase(row.name, row.name, row.mode, "missing", None, None), errors
    # `MS-13` ① 自建语料：迁移期允许 4 列（源格式未承诺），但 `format_ok` 仍要求补 `-`
    return CorpusCase(row.name, row.name, row.mode, "self", None, None), errors


# --------------------------------------------------------------------------- #
# 报告（MS-12 自检 ＋ MS-14 四要素）
# --------------------------------------------------------------------------- #


@dataclasses.dataclass
class Report:
    """一份 `MS-14` 报告的解析结果。"""

    path: pathlib.Path
    exists: bool
    self_check: bool
    reference: str | None
    rows: list[tuple[str, str]]
    counts: tuple[int, int, int, int] | None  # (total, pass, known, new)
    snapshot: str | None
    errors: list[str]


def parse_report(path: pathlib.Path) -> Report:
    """读 `target/conformance/{report,self-check}.md`（不存在 ⇒ exists=False）。"""
    report = Report(path, path.is_file(), False, None, [], None, None, [])
    if not report.exists:
        return report
    text = path.read_text(encoding="utf-8", errors="replace")
    reference = REFERENCE_LINE.search(text)
    report.reference = reference.group("version").strip() if reference else None
    report.self_check = bool(SELF_CHECK_LINE.search(text))
    report.rows = [
        (match.group("name"), match.group("mode").strip()) for match in TABLE_ROW.finditer(text)
    ]
    counts = COUNT_LINE.search(text)
    if counts:
        report.counts = tuple(int(counts.group(key)) for key in ("total", "pass", "known", "new"))  # type: ignore[assignment]
    snapshot = SNAPSHOT_LINE.search(text)
    report.snapshot = snapshot.group("snapshot").strip() if snapshot else None
    return report


# --------------------------------------------------------------------------- #
# 覆盖与基线
# --------------------------------------------------------------------------- #


def covered_modules(cases: list[CorpusCase], kinds: set[str]) -> dict[str, list[str]]:
    """按来源模块归并语料（只统计 `kinds` 里的来源种类；`Lib/test` 归 ③ 不算 ②）。"""
    modules: dict[str, list[str]] = {}
    for case in cases:
        if case.source_kind not in kinds or case.module is None:
            continue
        if case.module == "test" or case.module.startswith("test."):
            continue
        modules.setdefault(case.module, []).append(case.name)
    return modules


def upstream_non_test_modules() -> list[str] | None:
    """上游 `Lib/**/*.py` 全量的**非 `test.*`** 模块（与判据① 的仪器共用同一口径）。

    分母的**一处真相**在 `tools/lib_import_ratio.py`（它复用 `tools/sync_lib.py` 的
    `upstream_prefix()`）——这里只 import，不重述清单。
    """
    try:
        sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
        from lib_import_ratio import upstream_modules  # noqa: PLC0415

        return [
            module
            for module in upstream_modules()
            if module != "test" and not module.startswith("test.")
        ]
    except Exception as error:  # noqa: BLE001 —— 取不到上游只影响 --verbose 的"未覆盖"清单
        print(f"[WARN] 取不到上游 Lib 清单（逐模块覆盖表退化为只看已覆盖）：{error}")
        return None


def repository_lib_modules() -> set[str]:
    """本仓 `Lib/` 已同步的模块（给基线标 `[已同步]`，写手优先挑这些）。"""
    lib = ROOT / "Lib"
    modules: set[str] = set()
    if not lib.is_dir():
        return modules
    for path in lib.rglob("*.py"):
        if "__pycache__" in path.parts:
            continue
        parts = list(path.relative_to(lib).with_suffix("").parts)
        if parts and parts[-1] == "__init__":
            parts = parts[:-1]
        if parts:
            modules.add(".".join(parts))
    return modules


def format_stamp() -> str:
    """`2026-10-17T22:40:00+08:00` 形式的生成时间。"""
    return datetime.datetime.now().astimezone().isoformat(timespec="seconds")


def write_baseline(
    cases: list[CorpusCase],
    modules: dict[str, list[str]],
    libtest_count: int,
    universe: list[str] | None,
    format_ok: bool,
) -> None:
    """刷新 `target/recon/corpus_coverage_baseline.txt`（第三批写手的靶子）。"""
    synced = repository_lib_modules()
    covered = set(modules)
    if universe is None:
        uncovered = []
    else:
        uncovered = [module for module in universe if module not in covered]
    synced_uncovered = [module for module in uncovered if module in synced]
    unsynced_uncovered = [module for module in uncovered if module not in synced]
    foreign = sorted(covered - set(universe)) if universe else []
    filled = sum(1 for case in cases if case.source_kind == "explicit")
    legacy = sum(1 for case in cases if case.source_kind == "legacy")
    lines = [
        "# 判据② 覆盖率基线（给第三批语料的靶子）",
        f"# 生成：{format_stamp()} ｜ 仪器：tools/corpus_coverage.py",
        f"# 语料清单：tests/conformance/corpus/manifest.tsv（{len(cases)} 条）",
        f"# 阈值：语料 ≥{TOTAL_TARGET} ｜ Lib 模块 ≥{MODULE_TARGET} ｜ Lib/test 来源 ≥{LIBTEST_TARGET}",
        f"# 现状：{len(cases)} ｜ {len(modules)} ｜ {libtest_count}",
        f"# 缺口：{max(0, TOTAL_TARGET - len(cases))} 条 ｜ {max(0, MODULE_TARGET - len(modules))} 个模块"
        f" ｜ {max(0, LIBTEST_TARGET - libtest_count)} 条",
        "#",
        "# 来源格式：**A（manifest 第 5 列「来源」）** —— Lead 已裁定；仪器的 ②③ 只认这一列。",
        f"# 迁移：第 5 列已填 {filled}/{len(cases)} 条；4 列待补 {len(cases) - filled} 行（**自建写 `-`**）。",
        "# 来源取值：`Lib/<相对路径>.py`（`__init__` 归包名；可带 `#L12-L30`）／自建 `-`。",
        "# 口径：只统计非 `test.*` 模块（`Lib/test/` 来源归 ③ 单列）；模块名由来源路径推出",
        "#       （`Lib/<路径>.py`；`__init__` 归包名；`/`→`.`）。",
    ]
    if not format_ok and legacy:
        lines.append("# 现有模块覆盖暂含 legacy 散文估算（仅选靶，**不作 PASS 依据**）。")
    lib_named = sum(1 for case in cases if case.name.startswith("lib_"))
    if lib_named and lib_named != len(modules):
        lines.append(
            f"# 注意：模块覆盖按**不同模块**计 —— {lib_named} 个 `lib_*` 文件实际只覆盖 {len(modules)} 个模块"
            "（同一模块的多份语料合计算 1）。"
        )
    lines += [
        "",
        f"## 已覆盖模块（{len(modules)} 个）——同一模块的多条语料已合并",
    ]
    for module in sorted(modules):
        lines.append(f"  {module}（{len(modules[module])} 条：{'、'.join(sorted(modules[module]))}）")
    if foreign:
        lines += ["", "## [WARN] 覆盖的模块不在上游清单里（来源可能写错）"]
        lines += [f"  {module}" for module in foreign]
    lines += [
        "",
        f"## 未覆盖的上游模块（{len(uncovered) if universe is not None else '未知'} 个）——第三批靶子",
        (
            "# `[已同步]` ＝ 本仓 `Lib/` 已有这一份 ⇒ 语料写出来就能跑；其余要先同步（判据① 的 import 面）。"
            if universe is not None
            else "# [WARN] 取不到上游 Lib 清单 ⇒ 本节的「未覆盖」清单不可用（只列已覆盖）。"
        ),
    ]
    lines += [f"  [已同步] {module}" for module in synced_uncovered]
    lines += [f"  [未同步] {module}" for module in unsynced_uncovered]
    lines += [
        "",
        "# 写手提示：",
        "# - 每个新语料一条 manifest 行；**第 5 列**写来源（格式 A，别写两种）；",
        "# - `lib_<模块>.py` 命名只是惯例，**不是**来源真相 —— 模块覆盖只认第 5 列；",
        "# - `Lib/test/` 来源的语料同时计入 ③（≥40 条）；",
        "# - 旧 4 列行也要补第 5 列（自建语料补 `-`），否则 ②③ 判不达标。",
        "",
    ]
    BASELINE.parent.mkdir(parents=True, exist_ok=True)
    BASELINE.write_text("\n".join(lines), encoding="utf-8")


# --------------------------------------------------------------------------- #
# 检查
# --------------------------------------------------------------------------- #


def print_check(number: str, title: str, ok: bool, detail: str) -> bool:
    print(f"[{'PASS' if ok else 'FAIL'}] {number} {title}：{detail}")
    return ok


def main() -> int:
    parser = argparse.ArgumentParser(description="判据② 覆盖率仪器（MS-13 ②／MS-14）")
    parser.add_argument("--verbose", action="store_true", help="打印逐模块覆盖表（有语料／没语料）")
    args = parser.parse_args()

    print("== 判据② 覆盖率仪器（`MS-13` ②／`MS-14`；`tools/corpus_coverage.py`）==")
    print()

    # ---- 清单与来源解析 ----
    rows, manifest_errors = parse_manifest()
    cases: list[CorpusCase] = []
    case_errors: list[str] = []
    for row in rows:
        case, errors = resolve_case(row)
        cases.append(case)
        case_errors.extend(errors)
    errors = manifest_errors + case_errors

    legacy = [case for case in cases if case.source_kind == "legacy"]
    explicit = [case for case in cases if case.source_kind == "explicit"]
    missing = [case for case in cases if case.source_kind == "missing"]
    self_built = [case for case in cases if case.source_kind == "self"]
    # 显式来源里 `-`（自建）也是显式，但不算 Lib 模块
    explicit_lib = [case for case in explicit if case.module is not None]
    explicit_self = [case for case in explicit if case.module is None]
    rows_without_source = [row for row in rows if row.source_column is None]
    # **格式 A 达标**：无解析错误、无 legacy 估算、无缺失、且**每一行都有第 5 列**（自建写 `-`）
    format_ok = not errors and not legacy and not missing and not rows_without_source

    print(
        f"清单：{MANIFEST.relative_to(ROOT)} ⇒ {len(rows)} 条"
        f"（文件都存在：{'是' if not missing else f'否，缺 {len(missing)} 条'}）"
    )
    print(
        f"来源格式 A（manifest 第 5 列）：已填 {len(rows) - len(rows_without_source)}/{len(rows)} 条"
        f"（Lib {len(explicit_lib)} ／ 自建 {len(explicit_self)}）"
        + (f"；**4 列待补 {len(rows_without_source)} 条**" if rows_without_source else "")
    )
    if legacy:
        print(
            f"  legacy 兼容路径：{len(legacy)} 条缺第 5 列、按首行散文估算"
            "（只用来量现状，**不计入 PASS**）"
        )
    if self_built:
        print(f"  迁移期：另有 {len(self_built)} 条自建语料是 4 列（需补第 5 列 `-`）")
    if errors:
        for error in errors[:20]:
            print(f"  ✗ {error}")
        if len(errors) > 20:
            print(f"  … 另有 {len(errors) - 20} 条同类错误")
    suspicious = [
        case.name for case in explicit_self if case.name.startswith(("lib_", "libtest_"))
    ]
    if suspicious:
        print(f"  [WARN] {len(suspicious)} 条 `lib_*` 命名却声明自建来源（`-`）：{'、'.join(suspicious[:10])}")
    print()

    # ---- ① 语料总量 ----
    total = len(rows)
    bad_modes = sorted({row.mode for row in rows if row.mode != "python"})
    check1 = print_check(
        "①",
        "语料总量",
        total >= TOTAL_TARGET and not bad_modes and bool(rows),
        f"{total} / 阈值 {TOTAL_TARGET}"
        + (f"（差 {TOTAL_TARGET - total}）" if total < TOTAL_TARGET else "")
        + (f"；非 python 模式 {bad_modes}（`MS-13` ③ 要求扩展模式语料为空）" if bad_modes else ""),
    )

    # ---- ② Lib 模块覆盖（第 5 列是真相；legacy 仅基线） ----
    resolved_modules = covered_modules(cases, {"explicit", "legacy"})
    module_count = len(resolved_modules)
    if format_ok:
        format_note = "来源格式 A 已补齐"
    else:
        format_note = f"来源格式 A 未补齐（第 5 列待补 {len(rows_without_source)} 条"
        format_note += f"；legacy 估算 {len(legacy)} 条不计入 PASS）" if legacy else "）"
    check2 = print_check(
        "②",
        "Lib 模块覆盖",
        module_count >= MODULE_TARGET and format_ok,
        f"{module_count} / 阈值 {MODULE_TARGET}"
        + (f"（差 {MODULE_TARGET - module_count}）" if module_count < MODULE_TARGET else "")
        + f"；{format_note}",
    )

    # ---- ③ Lib/test 来源用例 ----
    libtest_resolved = [
        case
        for case in cases
        if case.source_kind in ("explicit", "legacy")
        and case.module is not None
        and (case.module == "test" or case.module.startswith("test."))
    ]
    libtest_count = len(libtest_resolved)
    check3 = print_check(
        "③",
        "Lib/test 来源用例",
        libtest_count >= LIBTEST_TARGET and format_ok,
        f"{libtest_count} / 阈值 {LIBTEST_TARGET}"
        + (f"（差 {LIBTEST_TARGET - libtest_count}）" if libtest_count < LIBTEST_TARGET else "")
        + f"；{format_note}",
    )

    # ---- ④ 对拍口径体检（MS-12 自检全绿 ＋ report 新差异 0） ----
    self_report = parse_report(SELF_CHECK)
    subject_report = parse_report(REPORT)
    problems4: list[str] = []
    if not self_report.exists:
        problems4.append(f"缺 {SELF_CHECK.relative_to(ROOT)}（先跑 `cargo test -p pyawa-abi --test conformance`）")
    elif not self_report.self_check:
        problems4.append("self-check.md 里没有 `MS-12` 自检标记（被测侧不是参照实现）")
    elif self_report.counts is None:
        problems4.append("self-check.md 里读不到三分类计数行")
    else:
        total_s, pass_s, known_s, new_s = self_report.counts
        if new_s != 0 or known_s != 0 or pass_s != total_s:
            problems4.append(
                f"自检**不绿**（通过 {pass_s}/{total_s} · 已知 {known_s} · 新 {new_s}；`MS-12` 要求全绿）"
            )
    if not subject_report.exists:
        problems4.append(f"缺 {REPORT.relative_to(ROOT)}")
    elif subject_report.counts is None:
        problems4.append("report.md 里读不到三分类计数行")
    elif subject_report.counts[3] != 0:
        problems4.append(f"report.md 的**新差异** = {subject_report.counts[3]}（判据② 要求 0）")
    self_counts = self_report.counts
    subject_counts = subject_report.counts
    detail4 = (
        (
            f"自检绿（通过 {self_counts[1]}/{self_counts[0]} · 已知 {self_counts[2]} · 新 {self_counts[3]}）"
            if self_counts and self_counts[1] == self_counts[0] and self_counts[2] == 0 and self_counts[3] == 0
            else "自检未绿/缺"
        )
        + "｜report "
        + (
            f"通过 {subject_counts[1]} · 已知 {subject_counts[2]} · 新 {subject_counts[3]}"
            if subject_counts
            else "缺计数"
        )
    )
    if problems4:
        detail4 = "；".join(problems4)
    elif (
        subject_report.exists
        and MANIFEST.is_file()
        and subject_report.path.stat().st_mtime < MANIFEST.stat().st_mtime
    ):
        detail4 += "；[WARN] 报告比语料清单旧（先重跑对拍再采信）"
    check4 = print_check("④", "对拍口径（MS-12／新差异 0）", not problems4, detail4)

    # ---- ⑤ MS-14 报告可发布四要素 ----
    problems5: list[str] = []
    if not subject_report.exists:
        problems5.append("缺 report.md")
    else:
        if not subject_report.reference:
            problems5.append("缺「参照实现」版本行")
        manifest_names = [row.name[: -len(".py")] for row in rows]
        report_names = [name for name, _ in subject_report.rows]
        if not report_names:
            problems5.append("缺「语料清单与模式」表格")
        elif sorted(report_names) != sorted(manifest_names):
            problems5.append(
                f"报告的语料清单（{len(report_names)} 行）与 manifest（{len(manifest_names)} 条）不一致"
            )
        elif any(not mode for _, mode in subject_report.rows):
            problems5.append("报告表格有行的「模式」为空")
        if subject_report.counts is None:
            problems5.append("缺三分类计数行")
        else:
            total_r, pass_r, known_r, new_r = subject_report.counts
            if pass_r + known_r + new_r != total_r:
                problems5.append(
                    f"三分类计数不自洽（通过 {pass_r}＋已知 {known_r}＋新 {new_r} ≠ 共 {total_r}）"
                )
        if subject_report.snapshot is None:
            problems5.append("缺「差异清单快照」行")
    if problems5:
        detail5 = "；".join(problems5)
    else:
        total_r, pass_r, known_r, new_r = subject_report.counts  # type: ignore[misc]
        detail5 = (
            f"参照版本 {subject_report.reference} ✓｜语料清单与模式 {len(subject_report.rows)} 行 ✓"
            f"｜三分类 {total_r}={pass_r}+{known_r}+{new_r} ✓｜差异快照 {subject_report.snapshot} ✓"
        )
    check5 = print_check("⑤", "MS-14 报告四要素", not problems5, detail5)

    # ---- 来源格式（已定：A＝manifest 第 5 列；迁移期提示待补条数） ----
    if not format_ok:
        print()
        print("来源格式＝**A（manifest 第 5 列「来源」）**（Lead 已裁定）：")
        print(
            "  取值：`Lib/<相对路径>.py`（`__init__` 归包名；可带 `#L12-L30`）；"
            "**自建语料写 `-`**。每一行都要有第 5 列。"
        )
        print(
            f"  迁移现状：已填 {len(rows) - len(rows_without_source)}/{len(rows)} 条；"
            f"**待补 {len(rows_without_source)} 条**（4 列行）。"
        )
        if legacy:
            print(
                f"  legacy 兼容路径：{len(legacy)} 条缺第 5 列、暂按首行散文估算（只量现状，**不计入 PASS**）。"
            )
        print(
            "  harness 侧：`crates/pyawa-abi/tests/conformance.rs` 已放宽为「4 或 5 列」，"
            "第 5 列**不参与对拍比对**。"
        )
        print("  闸门接线：暂不硬接（Lead 裁定；第三批语料落地、仪器转 PASS 的同一轮再接 `tests/ci/check.py`）。")

    # ---- 其它提示 ----
    manifest_files = {row.name for row in rows}
    orphans = sorted(path.name for path in CORPUS.glob("*.py") if path.name not in manifest_files)
    if orphans:
        print()
        print(f"[WARN] 语料目录有 {len(orphans)} 个 `.py` 不在清单里（辅助件可忽略）：{'、'.join(orphans[:10])}")

    # ---- 上游清单（① 的一处分母；--verbose 与基线都要它；只取一次） ----
    universe = upstream_non_test_modules()

    # ---- 逐模块覆盖（--verbose） ----
    if args.verbose:
        covered = set(resolved_modules)
        synced = repository_lib_modules()
        print()
        print(f"== 逐模块覆盖（②；分母＝上游非 `test.*` 模块 {len(universe) if universe else '未知'}）==")
        print(f"已覆盖（{len(covered)}）：")
        for module in sorted(covered):
            print(
                f"  [x] {module}（{len(resolved_modules[module])} 条："
                f"{'、'.join(sorted(resolved_modules[module]))}）"
            )
        if universe is not None:
            uncovered = [module for module in universe if module not in covered]
            print(f"未覆盖（{len(uncovered)}）：")
            for module in uncovered:
                print(f"  [ ] {'[已同步] ' if module in synced else '[未同步] '}{module}")

    # ---- 刷新基线 ----
    write_baseline(cases, resolved_modules, libtest_count, universe, format_ok)
    print()
    print(f"覆盖基线已刷新：{BASELINE.relative_to(ROOT)}")

    # ---- 结论 ----
    passed = sum([check1, check2, check3, check4, check5])
    print()
    if passed == 5:
        print("结论：PASS（五项全过）")
        return 0
    print(
        f"结论：FAIL（{passed}/5 项 PASS；①{'✓' if check1 else '✗'} ②{'✓' if check2 else '✗'}"
        f" ③{'✓' if check3 else '✗'} ④{'✓' if check4 else '✗'} ⑤{'✓' if check5 else '✗'}）"
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
