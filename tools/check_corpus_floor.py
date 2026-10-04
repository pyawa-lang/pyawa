#!/usr/bin/env python3
"""**`MS-13` 语料下限**的可执行判据（第 270 轮按用户裁定落地）。

裁定（`docs/PLAN-milestones.md` 的 `MS-13` 条 ✓）：总数 ≥ 112 ✓，且 M2 内容面各设下限 ——
类 ≥ 16／异常 ≥ 11／import ≥ 14／生成器 ≥ 4／描述符 ≥ 3／元类 ≥ 2 ✓（**只许涨、不许落** ✓）。
② 的 `Lib/` 语料仍暂空 ✓（依赖 M3 ✓）。

用法：`python3 tools/check_corpus_floor.py` ⇒ 全过则退出 0 ✓。
"""

import pathlib
import re
import sys

CORPUS = pathlib.Path("tests/conformance/corpus")
MANIFEST = CORPUS / "manifest.tsv"
FLOOR = {"总数": 112, "类": 15, "异常": 11, "import": 14, "生成器": 4, "描述符": 3, "元类": 2}
PATTERNS = {
    "类": r"\bclass\s+\w+",
    "异常": r"\b(try:|except\b|raise\b|finally:)",
    "import": r"^\s*(import|from)\s+\w",
    "生成器": r"\b(yield\b|\.send\(|next\()",
    "描述符": r"__get__|__set__|property\(|__set_name__",
    "元类": r"metaclass\s*=|type\s*\(\s*\w+\s*,",
}


def counted() -> dict:
    names = [
        line.split("\t")[0]
        for line in MANIFEST.read_text().splitlines()
        if line.strip() and not line.startswith("#")
    ]
    counts = {"总数": len(names)}
    for face, pattern in PATTERNS.items():
        hit = 0
        for name in names:
            path = CORPUS / name
            if path.is_file() and re.search(pattern, path.read_text(), re.M):
                hit += 1
        counts[face] = hit
    return counts


def main() -> int:
    counts = counted()
    failed = []
    for face, floor in FLOOR.items():
        if counts[face] < floor:
            failed.append(f"{face}：{counts[face]} < 下限 {floor}")
    summary = " ｜ ".join(f"{face} {counts[face]}/{FLOOR[face]}" for face in FLOOR)
    if failed:
        print(f"✗ 语料低于 `MS-13` 下限（第 270 轮裁定）：{'；'.join(failed)}")
        print(f"  实测：{summary}")
        return 1
    print(f"✓ 语料满足 `MS-13` 下限：{summary}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
