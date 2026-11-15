#!/usr/bin/env python3
"""**失败路径的 GC 收支探针集（task-16 D）** —— 引用泄漏的可复跑判据。

背景（`docs/rounds/04-post-m6-01.md` round 171 起、`NEXT` 的"同族泄漏（已量化）"）：
core 的若干**失败路径**会漏掉一份引用（`Err(Raised)` bundle、或 `__new__`／`__del__` 等
路径上"本次调用持有"的实参/实例）。它们的共同症状是：
- 泄漏的异常带着 traceback → **`tb_created − tb_freed` 随迭代次数线性增长**；
- 泄漏的普通对象 → **`live_after` 随迭代次数线性增长**。

本脚本给每个失败路径生成一段探针，用 `PYAWA_GC_LOG=1` 的 `[gc]` 收支做判据：

1. **异常收支**：最后一次回收的 `tb_created − tb_freed ≤ 8`（泄漏时 ≈ N）；
2. **对象收支（缩放法）**：同一探针跑 N=10000 与 N=20000，最后一次回收的 `live_after`
   差值 ∈ [−512, +512]（泄漏时 ≈ N）。

因此本脚本**不需要**知道"应该有几颗"——它只判"随 N 线性增长"。当前已知会 RED 的路径
（修前基线）：`set`／`frozenset`／`int`／`sorted` 比较失败／坏 `__del__`／惰性注解；
已验证 GREEN 的（task-14 修过的回归面）：`f(*Plain())`／`tuple(Plain())`／坏 `__new__`／
坏 `__init__`／坏 `__exit__`。

用法（**已接闸**：`tools/slowcheck.sh` 第 12 项 / `tools/gate_chunk.sh small`，2026-11-14）：

    python3 tests/ci/failure_path_leaks.py                  # 默认 ./target/release/pyawa
    python3 tests/ci/failure_path_leaks.py --binary target/debug/pyawa
    python3 tests/ci/failure_path_leaks.py --ledger         # 另跑 PYAWA_GC_LOG=4 的书（B 账本）

退出码：0 ＝ 全部判据绿；1 ＝ 有 RED（逐条打印数字与判据）。

**判据出处**：`PYAWA_GC_LOG` 的 `[gc] … tb_created=… tb_freed=…` 行（`Instance::collect`）；
缩放法的两张 N 各自是**独立进程**（同一脚本、`sys.argv[1]` 给 N），避免进程内缓存互相污染。
"""

from __future__ import annotations

import argparse
import os
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]

#: 判据阈值：`tb` 在飞常数 ≤ 8（修复后实测 2）；`live_after` 缩放差 ≤ 512。
TB_INFLIGHT_LIMIT = 8
LIVE_SCALING_LIMIT = 512
SMALL_N = 10_000
LARGE_N = 20_000

#: 失败路径探针。每段 `BODY` 是探针脚本主体：`call()` 每次触发一次失败路径（异常都在内部接住）。
#: 末尾由模板统一做 `gc.collect()`（**两次**：第一次确认在途项，第二次让账本把 suspects 清零）。
PROBES: list[tuple[str, str, bool]] = [
    (
        "noniterable_fstar",
        """\
class Plain:
    pass

def f(*a, **k):
    return a

def call():
    try:
        f(*Plain())
    except TypeError:
        pass
""",
        False,
    ),
    (
        "noniterable_tuple",
        """\
class Plain:
    pass

def call():
    try:
        tuple(Plain())
    except TypeError:
        pass
""",
        False,
    ),
    (
        "noniterable_set",
        """\
class Plain:
    pass

def call():
    try:
        set(Plain())
    except TypeError:
        pass
""",
        True,
    ),
    (
        "noniterable_frozenset",
        """\
class Plain:
    pass

def call():
    try:
        frozenset(Plain())
    except TypeError:
        pass
""",
        True,
    ),
    (
        "noniterable_int",
        """\
class Plain:
    pass

def call():
    try:
        int(Plain())
    except TypeError:
        pass
""",
        True,
    ),
    (
        "sorted_compare",
        """\
class NoCompare:
    pass

def call():
    try:
        sorted([NoCompare(), NoCompare()])
    except TypeError:
        pass
""",
        True,
    ),
    (
        "bad_new",
        """\
class Boom:
    def __new__(cls):
        raise ValueError("new boom")

def call():
    try:
        Boom()
    except ValueError:
        pass
""",
        False,
    ),
    (
        "bad_init",
        """\
class Boom:
    def __init__(self):
        raise ValueError("init boom")

def call():
    try:
        Boom()
    except ValueError:
        pass
""",
        False,
    ),
    (
        "bad_exit",
        """\
class CM:
    def __enter__(self):
        return self

    def __exit__(self, *args):
        raise ValueError("exit boom")

def call():
    try:
        with CM():
            pass
    except ValueError:
        pass
""",
        False,
    ),
    (
        "bad_del",
        """\
class D:
    def __del__(self):
        raise ValueError("del boom")

def call():
    D()
""",
        True,
    ),
    (
        "lazy_annotations",
        """\
import sys

def bad_annotate(*args, **kwargs):
    raise ValueError("annot boom")

sys.modules[__name__].__annotate_func__ = bad_annotate

def call():
    try:
        sys.modules[__name__].__annotations__
    except BaseException:
        pass
""",
        True,
    ),
]

TEMPLATE = """\
import gc
import sys

N = int(sys.argv[1])

__BODY__
gc.collect()
for _ in range(N):
    call()
gc.collect()
gc.collect()
print("done", N)
"""

GC_LINE = re.compile(r"^\[gc\] index=\d+ .*?live_after=(\d+) .*?tb_created=(\d+) tb_freed=(\d+)$")
LEDGER_SUMMARY = re.compile(r"^\[raise-ledger\] (generated=.*)$")
LEDGER_SITE = re.compile(r"^\[raise-ledger\] 未消费 Raised .*$")


def parse_last_gc(stderr: str) -> tuple[int, int, int] | None:
    """最后一次 `[gc]` 行的 `(live_after, tb_created, tb_freed)`。"""
    for line in reversed(stderr.splitlines()):
        match = GC_LINE.match(line)
        if match:
            return int(match.group(1)), int(match.group(2)), int(match.group(3))
    return None


def run_probe(binary: str, script: str, n: int, ledger: bool) -> tuple[subprocess.CompletedProcess, str]:
    """把 `script` 写成临时文件、跑 `binary script.py N`；返回 (进程结果, stderr)。"""
    with tempfile.TemporaryDirectory(prefix="pyawa-failprobe-") as directory:
        path = pathlib.Path(directory) / "probe.py"
        path.write_text(script, encoding="utf-8")
        env = dict(os.environ)
        env.pop("PYTHONPATH", None)
        env["PYAWA_GC_LOG"] = "4" if ledger else "1"
        completed = subprocess.run(
            [binary, str(path), str(n)],
            capture_output=True,
            text=True,
            env=env,
            timeout=600,
        )
        return completed, completed.stderr


def build_script(body: str) -> str:
    return TEMPLATE.replace("__BODY__", body)


def check_case(binary: str, name: str, body: str) -> tuple[bool, str]:
    """跑 N=10000／20000 两档，按判据判 PASS/RED；返回 (是否通过, 说明)。"""
    details: list[str] = []
    observations: list[tuple[int, int, int]] = []
    for n in (SMALL_N, LARGE_N):
        completed, stderr = run_probe(binary, build_script(body), n, ledger=False)
        if completed.returncode != 0:
            return False, f"探针进程非零退出（N={n}）：{completed.returncode}\n{completed.stderr[-800:]}"
        gc = parse_last_gc(stderr)
        if gc is None:
            return False, f"没解析到 [gc] 行（N={n}）"
        observations.append(gc)
    small_live, small_created, small_freed = observations[0]
    large_live, large_created, large_freed = observations[1]
    tb_small = small_created - small_freed
    tb_large = large_created - large_freed
    live_growth = large_live - small_live
    ok = (
        tb_small <= TB_INFLIGHT_LIMIT
        and tb_large <= TB_INFLIGHT_LIMIT
        and abs(live_growth) <= LIVE_SCALING_LIMIT
    )
    details.append(
        f"tb={tb_small}/{tb_large}（限 ≤{TB_INFLIGHT_LIMIT}） "
        f"live={small_live}→{large_live}（差 {live_growth:+d}，限 ±{LIVE_SCALING_LIMIT}）"
    )
    return ok, " ".join(details)


def ledger_report(binary: str, name: str, body: str, n: int = 10_000) -> str:
    """`PYAWA_GC_LOG=4` 下的账本摘要：summary ＋ 未消费生成点（B 引用账本）。"""
    completed, stderr = run_probe(binary, build_script(body), n, ledger=True)
    summaries = [match.group(1) for match in map(LEDGER_SUMMARY.match, stderr.splitlines()) if match]
    sites: list[str] = []
    for line in stderr.splitlines():
        if LEDGER_SITE.match(line):
            sites.append(line)
    if not summaries and not sites:
        return "（账本无输出）"
    out = []
    if sites:
        out.append("  账本未消费生成点：")
        out.extend(f"    {site}" for site in sites[-4:])
    if summaries:
        out.append(f"  账本 summary（末次）：{summaries[-1]}")
    return "\n".join(out)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", default=str(ROOT / "target" / "release" / "pyawa"))
    parser.add_argument("--ledger", action="store_true", help="另跑 PYAWA_GC_LOG=4 的 B 账本摘要")
    parser.add_argument("--only", default=None, help="只跑名字匹配该子串的探针")
    parser.add_argument(
        "--expect",
        choices=("all-green", "any"),
        default="any",
        help="all-green：所有探针必须绿（修完后的闸门口径）；any：只报结果（修前基线口径）",
    )
    args = parser.parse_args(argv)
    binary = str(pathlib.Path(args.binary).resolve())
    if not pathlib.Path(binary).is_file():
        print(f"找不到二进制：{binary}", file=sys.stderr)
        return 2
    selected = [probe for probe in PROBES if not args.only or args.only in probe[0]]
    if not selected:
        print(f"--only={args.only!r} 没匹配到探针", file=sys.stderr)
        return 2
    print(f"失败路径 GC 收支探针 · binary={binary}")
    print(
        f"判据：tb_inflight ≤ {TB_INFLIGHT_LIMIT}（N={SMALL_N}/{LARGE_N}）；"
        f"live_after 缩放差 ≤ ±{LIVE_SCALING_LIMIT}"
    )
    red: list[str] = []
    for name, body, expected_red in selected:
        ok, details = check_case(binary, name, body)
        verdict = "PASS" if ok else "RED "
        marker = "（既有已知缺口）" if expected_red else ""
        print(f"  [{verdict}] {name}{marker}：{details}")
        if not ok:
            red.append(name)
        if args.ledger and not ok:
            print(ledger_report(binary, name, body))
    print()
    if red:
        print(f"结果：{len(red)}/{len(selected)} RED —— {', '.join(red)}")
        if args.expect == "all-green":
            return 1
        return 1
    print(f"结果：{len(selected)}/{len(selected)} 全绿")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
