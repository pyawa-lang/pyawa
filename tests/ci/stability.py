#!/usr/bin/env python3
"""**`MS-25`**：同一提交连跑 ≥3 次，测试数与结果**必须**完全一致。

为什么值得单独一个脚本：`cargo test` 对 `SIGABRT` 一类信号**不会**打 `FAILED`——它表现为
"**总数变少**"（本仓库真实发生过一次：类体帧少一次 `incref` 导致随机堆损坏）。所以
只看"有没有 FAILED"会漏，必须比对**每次运行的计数**。

比对粒度是**每个测试二进制**（不是总数）：总数会掩盖"这个少 2、那个多 2"。

用法::

    python3 tests/ci/stability.py            # 默认连跑 3 次
    python3 tests/ci/stability.py --runs 5

退出码：任何一次运行失败（非零退出／出现 FAILED／出现 error）或**计数不一致** ⇒ 1。
计数抖动必须按**崩溃／内存损坏**排查，不要当成偶发。
"""

from __future__ import annotations

import argparse
import collections
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

#: `Running tests/xxx.rs (target/…/deps/xxx-hash)` 或 `Running unittests src/lib.rs (…)`
RUNNING = re.compile(r"^\s*Running\s+(.+?)\s+\((.+)\)\s*$")
#: `   Doc-tests pyawa_core`——**单独成组**：不然它会覆盖前一个二进制的计数（少算）
DOCTEST = re.compile(r"^\s*Doc-tests\s+(\S+)\s*$")
#: `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s`
RESULT = re.compile(
    r"^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured;"
)
#: `Doc-tests …` 那一组的名字在别处；用 target 路径的最后一段做标识即可。


def run_once() -> tuple[dict[str, tuple[int, int, str]], bool, str]:
    """跑一次 `cargo test --workspace --no-fail-fast`，返回 `(每个二进制的计数, 是否正常, 原始尾部)`。"""
    # **注意**：cargo 把 `Running …` 写 **stderr**、把 `test result:` 写 **stdout**。
    # 分别捕获再拼接会丢掉真实顺序（结果全在前、Running 全在后），归组就全错了 ⇒
    # 必须把 stderr 重定向进 stdout，保持"Running → 该二进制的结果"这一对。
    completed = subprocess.run(
        ["cargo", "test", "--workspace", "--no-fail-fast"],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    output = completed.stdout
    counts: dict[str, tuple[int, int, str]] = {}
    current = "（未归组）"
    for line in output.splitlines():
        doctest = DOCTEST.match(line)
        if doctest:
            current = f"doc:{doctest.group(1)}"
            continue
        running = RUNNING.match(line)
        if running:
            # 用 target 路径里的文件名做主键（`…/deps/executor-1a2b3c` ⇒ `executor`）
            target = running.group(2)
            name = pathlib.Path(target).name.split("-")[0]
            current = name
            continue
        result = RESULT.match(line.strip())
        if result:
            status, passed, failed = result.group(1), int(result.group(2)), int(result.group(3))
            counts[current] = (passed, failed, status)
    healthy = completed.returncode == 0 and "FAILED" not in output and "\nerror" not in output
    tail = "\n".join(output.splitlines()[-15:])
    return counts, healthy, tail


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="MS-25：测试计数稳定性")
    parser.add_argument("--runs", type=int, default=3, help="连跑次数（默认 3，MS-25 要求 ≥3）")
    args = parser.parse_args(argv)
    if args.runs < 3:
        print("MS-25 要求至少连跑 3 次（--runs >= 3）")
        return 1

    runs: list[dict[str, tuple[int, int, str]]] = []
    for index in range(args.runs):
        counts, healthy, tail = run_once()
        total = sum(passed for passed, _, _ in counts.values())
        print(f"第 {index + 1} 次：{len(counts)} 个二进制，{total} 项通过"
              f"{'' if healthy else '  ⚠ 本次运行不正常'}")
        if not healthy:
            print(tail)
            return 1
        runs.append(counts)

    # 比对：每个二进制都要在每次运行里出现，且 (passed, failed, status) 完全一致
    failures: list[str] = []
    names = sorted({name for counts in runs for name in counts})
    for name in names:
        seen = [counts.get(name) for counts in runs]
        if any(entry is None for entry in seen):
            failures.append(f"{name}: 并非每次运行都出现（{seen}）")
            continue
        if len(set(seen)) != 1:
            failures.append(f"{name}: 计数在多次运行间不一致 {seen}")
    totals = collections.Counter(
        sum(passed for passed, _, _ in counts.values()) for counts in runs
    )
    if len(totals) != 1:
        failures.append(f"总通过数不一致：{dict(totals)}")

    if failures:
        print("\n[FAIL] MS-25：测试计数在多次运行间不一致——按崩溃／内存损坏排查：")
        for failure in failures:
            print(f"       - {failure}")
        return 1
    total = sum(passed for passed, _, _ in runs[0].values())
    print(f"\n[PASS] MS-25：连跑 {args.runs} 次，{len(runs[0])} 个二进制、{total} 项通过，计数完全一致")
    return 0


if __name__ == "__main__":
    sys.exit(main())
