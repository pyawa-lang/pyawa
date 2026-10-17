#!/usr/bin/env python3
"""**`Lib/test/` 语料片段的出处守卫**（`MS-13` ②，`P3-27` 第一批）。

缘由（一处真相）：`tests/conformance/corpus/` 里那几条**逐字片段**语料，正文是从
`Lib/test/...` 拷来的 —— 拷贝**会漂移** ✗。本脚本把"哪几行来自哪个文件"写成**可执行判据**：
每条的声明片段必须**原样**出现在语料文件里 ✗（否则退出码 1，并指出漂移的那条）。

**不**管 `import` 驱动的那些条目：它们的被测代码就是 `Lib/` 里那一份（没有第二份拷贝 ✓）。

用法：`python3 tools/check_lib_corpus.py` ⇒ 全过退出 0。
"""

from __future__ import annotations

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CORPUS = ROOT / "tests" / "conformance" / "corpus"

#: 语料文件 ⇒ [(源文件, 行区间或 None＝整文件)]
FRAGMENTS: dict[str, list[tuple[str, object]]] = {
    "libtest_refleak_helper.py": [("Lib/test/support/refleak_helper.py", None)],
    "libtest_numbers_literals.py": [("Lib/test/support/numbers.py", None)],
    "libtest_stubclass.py": [("Lib/test/support/_hypothesis_stubs/_helpers.py", (2, 25))],
    "libtest_floats_identical.py": [("Lib/test/support/testcase.py", (1, 1)), ("Lib/test/support/testcase.py", (31, 53))],
    "libtest_full_match.py": [("Lib/test/libregrtest/filter.py", (20, 27))],
    "libtest_stub_strategy.py": [
        ("Lib/test/support/_hypothesis_stubs/_helpers.py", (2, 25)),
        ("Lib/test/support/_hypothesis_stubs/strategies.py", (6, 22)),
    ],
    "libtest_strategies_set.py": [("Lib/test/support/_hypothesis_stubs/strategies.py", (25, 68))],
}


def declared(source: pathlib.Path, lines: object) -> str:
    text = source.read_text(encoding="utf-8")
    if lines is None:
        return text
    low, high = lines  # type: ignore[misc]
    return "\n".join(text.splitlines()[low - 1 : high])


def main() -> int:
    if not FRAGMENTS:
        print("✗ 片段表是空的（守卫没有意义）")
        return 1
    failed = 0
    for name, pieces in FRAGMENTS.items():
        path = CORPUS / name
        if not path.is_file():
            print(f"✗ {name}：语料文件不存在")
            failed += 1
            continue
        body = path.read_text(encoding="utf-8")
        for source_name, lines in pieces:
            source = ROOT / source_name
            if not source.is_file():
                print(f"✗ {name}：源文件不存在 {source_name}")
                failed += 1
                continue
            if declared(source, lines) not in body:
                print(f"✗ {name}：来自 {source_name} 的片段不再逐字一致（漂移了）")
                failed += 1
    if failed:
        return 1
    total = sum(len(pieces) for pieces in FRAGMENTS.values())
    print(f"✓ `Lib/test/` 片段守卫：{len(FRAGMENTS)} 条语料、{total} 段正文与源文件逐字一致 ✓")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
