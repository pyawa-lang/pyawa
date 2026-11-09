#!/usr/bin/env python3
"""**单文件官方测试套件的逐用例计时**（round 141，找解释器热点用）。

本层解释器每次 Python 调用开销仍是最大的系统性热点（`test_userstring`／`test_statistics`／
`test_weakref` 一类"大循环"套件全压在这）⇒ 先用"逐用例墙钟"定位到具体用例，再决定修哪里 ✓。

用法：
    python3 tools/test_durations.py <test_xxx.py> [--top 15] [--jobs 1]

做法：把测试文件所在目录加进 `sys.path`、按模块名 import，再用 `unittest.defaultTestLoader`
展开成单个用例、逐条 `test.run(TestResult())` 计时（**只计时、不判定**；失败也算耗时 ✓）。
输出按耗时降序的前 N 条 ✓。（本工具本身要在 `pyawa` 里跑：`./target/release/pyawa tools/test_durations.py …`）
"""
from __future__ import annotations

import argparse
import os
import pathlib
import sys
import time
import unittest


def flatten(suite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from flatten(item)
        else:
            yield item


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("test_file")
    ap.add_argument("--top", type=int, default=15)
    ap.add_argument("--sort", choices=["slow", "fast"], default="slow")
    args = ap.parse_args()

    path = pathlib.Path(args.test_file).resolve()
    if not path.is_file():
        sys.exit(f"找不到测试文件：{path}")
    sys.path.insert(0, str(path.parent))
    module = __import__(path.stem)
    suite = unittest.defaultTestLoader.loadTestsFromModule(module)
    times = []
    for test in flatten(suite):
        started = time.monotonic()
        test.run(unittest.TestResult())
        times.append((time.monotonic() - started, test.id()))
    times.sort(reverse=args.sort == "slow")
    for elapsed, test_id in times[: args.top]:
        print(f"{elapsed:9.3f}s  {test_id}")
    print(f"合计 {len(times)} 个用例，墙钟合计 {sum(item[0] for item in times):.2f}s")


if __name__ == "__main__":
    main()
