#!/usr/bin/env python3
"""**把判据① 的失败按"缺什么"归族并排序**（新目标第 0 步的仪器 ✓）。

动因 ✓：`docs/DESIGN.md` §「解锁曲线」给的是**设计时的估计顺序**（前 5／10 个 C 模块 ✓）；
实测的族大小与它**不一致** ✗（例如 `_multibytecodec` 一族实测 21 ✓、`re`／`_sre` 17 ✓）
⇒ 排序要**按实测**，不照抄曲线假设 ✓（新目标原文 ✓）。

口径 ✓：跑 `tools/lib_import_ratio.py`，把它的**失败行**按
`No module named 'X'` 与 `cannot import name 'A' from 'B'` 归并计数 ✓；
计数＝**该族卡住多少个上游文件**（同一文件可有多个原因 ⇒ 各族计数之和 ≥ 失败数 ✓，不是划分 ✓）。

用法::
    python3 tools/lib_import_families.py            # 打印排序后的族
    python3 tools/lib_import_families.py --json      # 机器可读
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

MISSING_MODULE = re.compile(r"No module named '([^']+)'")
MISSING_NAME = re.compile(r"cannot import name '([^']+)' from '([^']+)'")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    run = subprocess.run(
        [sys.executable, str(ROOT / "tools" / "lib_import_ratio.py")],
        capture_output=True,
        text=True,
    )
    out = run.stdout + run.stderr
    modules: collections.Counter[str] = collections.Counter()
    names: collections.Counter[str] = collections.Counter()
    for target in MISSING_MODULE.findall(out):
        modules[target] += 1
    for name, holder in MISSING_NAME.findall(out):
        names[f"{holder}.{name}"] += 1

    summary = [line for line in out.splitlines() if "判据①" in line]
    if args.json:
        print(json.dumps({
            "summary": summary[-1] if summary else "",
            "missing_modules": modules.most_common(),
            "missing_names": names.most_common(),
        }, ensure_ascii=False, indent=2))
        return 0

    if summary:
        print(summary[-1])
    print("\n**缺模块族**（计数＝卡住多少个上游文件 ✓）")
    for target, count in modules.most_common(20):
        print(f"  {count:>3}  {target}")
    print("\n**缺名字族**")
    for target, count in names.most_common(12):
        print(f"  {count:>3}  {target}")
    print(
        "\n提示 ✓：上表就是**按实测的开工顺序** —— 计数大的族先做 ✓；"
        "补完一族再跑 `tools/find_syncable.py` 收同步批次 ✓。"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
