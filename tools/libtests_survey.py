#!/usr/bin/env python3
"""`Lib/test` 可编译／可运行面调查（对上游 CPython 3.14.4 的测试套件）。

输入：一份与参照**同版本**的 `Lib/test` 目录（默认
      `target/upstream/extracted/usr/lib/python3.14/test`，由 `libpython3.14-testsuite`
      deb 包解出 —— `target/upstream/extracted/` 就是 deb 的数据根；**不进仓库** ✓
      —— 它是"测试输入"，不是交付物）。
做法：逐个顶层 `test_*.py` 用 CLI 跑一遍（短超时、独立临时 cwd），按退出码分类：
      0        = 跑通 ✓
      2        = 编译／词法错 ✗（状态 2 是 `PA_ERR_SYNTAX`）
      1／5     = 运行期错／未实现 ✗（状态 5 = 尚未接线）
      超时     = 卡住 ✗
      编译错只取最后一条消息作"缺口签名"，聚合成"卡最多的签名"清单。
输出：`target/libtests_survey.tsv`（逐文件）＋ 终端汇总（**可编译率** / **跑通率** / 头号缺口）。
口径：**可编译率** ＝ 非语法错占比；**跑通率** ＝ 退出 0 占比。
      两个数都**不得**当"完成度"或"验收"（`CM-15` 同理：比例只排顺序，不衡量语义）。
      实测基线的**数字**不写死在这里（会漂 ✗）—— 以 `NEXT.md`「现在」段与随代码提交的
      台账为准（**一处真相**：本文件只定义口径与做法）。

用法::

    python3 tools/libtests_survey.py                      # 全量（target/debug/pyawa）
    python3 tools/libtests_survey.py --limit 50 --jobs 8
    python3 tools/libtests_survey.py --dir /path/to/Lib/test
    python3 tools/libtests_survey.py --release --limit 40  # 用 target/release/pyawa（提速抽样）
"""
import argparse
import collections
import concurrent.futures
import os
import pathlib
import subprocess
import sys
import tempfile

WORKSPACE = pathlib.Path(__file__).resolve().parent.parent
# 官方套件数据＝`libpython3.14-testsuite` deb 解到 `target/upstream/extracted/` 的 usr 数据根
# （**不进仓库** ✓）。文件名口径：deb 数据根 ＋ `/usr/lib/python3.14/test`。
DEFAULT_DIR = WORKSPACE / "target/upstream/extracted/usr/lib/python3.14/test"
DEFAULT_BIN = WORKSPACE / "target/debug/pyawa"
#: `--release` 且没显式给 `--bin` 时的默认 CLI（显式 `--bin` 仍优先）。
RELEASE_BIN = WORKSPACE / "target/release/pyawa"
OUT = WORKSPACE / "target/libtests_survey.tsv"


def run_one(binary: pathlib.Path, path: pathlib.Path, timeout: float):
    # `test_*.py` 需要 `import test.support` ⇒ 把 **test/ 的父目录**放进 PYTHONPATH
    # （否则会误报 ModuleNotFoundError: No module named 'test.support' —— 首版踩过 ✓）。
    env = dict(os.environ)
    parent = str(path.parent.parent)
    env["PYTHONPATH"] = parent + (os.pathsep + env["PYTHONPATH"] if env.get("PYTHONPATH") else "")
    with tempfile.TemporaryDirectory() as td:
        try:
            proc = subprocess.run(
                [str(binary), str(path)],
                cwd=td,
                env=env,
                capture_output=True,
                text=True,
                timeout=timeout,
            )
            out = (proc.stderr or "") + (proc.stdout or "")
            lines = out.strip().splitlines()
            msg = lines[-1] if lines else ""
        except subprocess.TimeoutExpired:
            return path.name, "timeout", "超时"
        # 分类按 CLI 的结构化消息（"未捕获（状态 N）"）而非进程退出码：
        # 退出码在"打不开文件/语法错"等路径上并不唯一，状态码才稳定。
        if "状态 2" in out:
            kind = "syntax"
        elif "状态 5" in out:
            kind = "unimplemented"
        elif "未捕获" in out or proc.returncode != 0:
            kind = "runtime"
        else:
            kind = "ok"
        return path.name, kind, msg[:200]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=str(DEFAULT_DIR))
    ap.add_argument("--bin", default=None)
    ap.add_argument(
        "--release",
        action="store_true",
        help="默认用 target/release/pyawa（显式 --bin 仍优先；仅改默认路径，口径不变）",
    )
    ap.add_argument("--jobs", type=int, default=max(4, (os.cpu_count() or 4) - 1))
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--timeout", type=float, default=30.0)
    args = ap.parse_args()

    root = pathlib.Path(args.dir).resolve()
    # 默认档＝debug（旧行为逐字不变）；`--release` 只换默认 CLI；显式 `--bin` 永远优先。
    if args.bin is not None:
        binary = pathlib.Path(args.bin).resolve()
    elif args.release:
        binary = RELEASE_BIN
    else:
        binary = DEFAULT_BIN
    if not root.is_dir():
        sys.exit(
            f"找不到 Lib/test 目录：{root}\n"
            "（先解开 `libpython3.14-testsuite` deb 到 `target/upstream/extracted/`）"
        )
    if not binary.is_file():
        build_hint = "cargo build --release --bin pyawa" if args.release else "cargo build --bin pyawa"
        sys.exit(f"找不到 CLI：{binary}（先 {build_hint}）")

    files = sorted(root.glob("test_*.py"))
    if args.limit:
        files = files[: args.limit]

    rows = []
    reasons = collections.Counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for name, kind, msg in pool.map(
            lambda p: run_one(binary, p, args.timeout), files
        ):
            rows.append((name, kind, msg))
            if kind == "syntax":
                reasons[msg] += 1

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8") as fh:
        fh.write("file\tkind\treason\n")
        for row in rows:
            fh.write("\t".join(row) + "\n")

    total = len(rows)
    ok = sum(1 for _, k, _ in rows if k == "ok")
    syntax = sum(1 for _, k, _ in rows if k == "syntax")
    unimplemented = sum(1 for _, k, _ in rows if k == "unimplemented")
    runtime = sum(1 for _, k, _ in rows if k == "runtime")
    timedout = sum(1 for _, k, _ in rows if k == "timeout")
    compilable = total - syntax
    print(f"共 {total} 个 test_*.py")
    print(f"  可编译（非语法错）：{compilable}/{total} = {100 * compilable / max(total, 1):.1f}%")
    print(f"  跑通（退出 0）：  {ok}/{total} = {100 * ok / max(total, 1):.1f}%")
    print(f"  语法错 {syntax} ｜ 未实现 {unimplemented} ｜ 运行期错 {runtime} ｜ 超时 {timedout}")
    if reasons:
        print("  头号语法缺口（按签名聚合）：")
        for reason, count in reasons.most_common(8):
            print(f"    {count:4d} × {reason}")
    print(f"  明细 → {OUT}")


if __name__ == "__main__":
    main()
