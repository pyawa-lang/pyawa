#!/usr/bin/env python3
"""`P*-` 队列编号唯一性检查（**墓碑算定义** ✓，`CX-19`）。

动因 ✓（用户 2026-10-07 建议 ✓）：立条目时先写成 `P3-22/23`（与已占号冲突 ✓），又让"实例体量实测"
占用 `P3-21`（该号有墓碑 ✓ ⇒ 不得重用 ✓）—— 连撞两次 ✓，而现有检查面里**没有这一条** ✗。

口径 ✓：
- **定义行**：`P<族>-<号>` 出现在一行的**开头位置**（可有 `- `／`* `／`|` ／空白／`**` 等前导 ✓），
  后面紧跟 `:`／`：`／空格／行尾 ✓；**墓碑行**（含"墓碑"／"作废"／"不得重用"）**也算定义** ✓；
- **违反**：同一个 `P<族>-<号>` 有 **≥2 条定义行** ✓（不论是否墓碑 ✓）；
- 只报违反，退出码非 0 ✓（便于当门用 ✓）。

用法::

    python3 tools/check_p3_numbers.py            # 扫 docs/
    python3 tools/check_p3_numbers.py --verbose  # 把每条定义行也列出来
"""

from __future__ import annotations

import argparse
import collections
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ID = re.compile(r"^(?P<lead>[\s>|*\-•]*(?:\*\*)?)(?P<id>P\d+-\d+)(?:\*\*)?\s*(?:[:：]|\s|$)")
ANY = re.compile(r"P\d+-\d+")


def definition(lines: list[str], index: int) -> str | None:
    """该行是不是"定义行"；是则给出编号 ✓。"""
    match = ID.match(lines[index])
    if not match:
        return None
    identifier = match.group("id")
    # 行内**首个**编号必须是它（避免"P3-21 与 P3-22 对比"这种叙述行被当定义 ✓）
    first = ANY.search(lines[index])
    if first and first.group(0) != identifier:
        return None
    return identifier


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument("--docs", default="docs")
    args = parser.parse_args()

    definitions: dict[str, list[str]] = collections.defaultdict(list)
    for path in sorted((ROOT / args.docs).rglob("*.md")):
        lines = path.read_text(encoding="utf-8").splitlines()
        for index, _ in enumerate(lines):
            identifier = definition(lines, index)
            if identifier:
                definitions[identifier].append(f"{path.relative_to(ROOT)}:{index + 1}")

    duplicates = {key: value for key, value in definitions.items() if len(value) > 1}
    print(f"`P*-` 定义行扫描：{len(definitions)} 个编号 ⇒ **重复 {len(duplicates)} 个**")
    if args.verbose:
        for key in sorted(definitions, key=lambda item: (item.split('-')[0], int(item.split('-')[1]))):
            print(f"  {key}: {'；'.join(definitions[key])}")
    for key, where in sorted(duplicates.items()):
        print(f"  ✗ {key} ⇒ {len(where)} 条定义：{'；'.join(where)}")
    return 1 if duplicates else 0


if __name__ == "__main__":
    sys.exit(main())
