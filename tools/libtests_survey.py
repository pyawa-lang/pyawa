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
import signal
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



def _run_capture(cmd, cwd, env, timeout):
    """跑一个子进程并捕获输出——**用临时文件而非管道**（round 136）：
    子进程若再派生子进程（`test_os`／`test_popen` 一类），管道会被孙进程继承；超时杀父进程后
    `communicate()` 仍等管道的 EOF ⇒ **survey 驱动被挂死** ✗（实测 207-237 片被平台 SIGTERM）。
    换成临时文件：超时杀掉子进程即可立刻返回 ✓（文件描述符随进程组消亡 ✓）。"""
    with tempfile.TemporaryFile() as out_file, tempfile.TemporaryFile() as err_file:
        # **独立会话 + 超时杀整组**（round 201）：有的套件会派生子进程／在内存压力下卡在不可中断 IO，
        # 只杀直接子进程的话 `subprocess.run` 仍可能在 `communicate()` 里等（实测 `test_ntpath` 在
        # 高负载机器上把 survey 驱动挂到平台 600 s 上限 ✗）。`start_new_session=True` ＋ `killpg`
        # ⇒ 连子进程组一起杀，驱动一定能拿回控制权 ✓。
        try:
            proc = subprocess.Popen(
                cmd,
                cwd=cwd,
                env=env,
                stdout=out_file,
                stderr=err_file,
                start_new_session=True,
            )
        except OSError:
            return None, ""
        try:
            returncode = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                pass
            return None, ""
        proc.returncode = returncode
        out_file.seek(0)
        err_file.seek(0)
        text = (err_file.read() + out_file.read()).decode("utf-8", "replace")
        return proc.returncode, text

def run_named(binary: pathlib.Path, path: pathlib.Path, timeout: float):
    """**以导入名再跑一次**（官方 regrtest 的派发形状；`tools/libtests_named_runner.py` ✓）。

    直接跑脚本时模块名是 `__main__`；官方套件的 doctest 期望文本带**模块限定名**
    （`test.test_extcall.f()` 一类）⇒ 会出现"上下文差异"的假失败 ✗。这里复核一次：
    通过就升级为 `ok`（理由写明"named-import harness" ✓ —— 口径更接近官方跑法 ✓）。
    """
    runner = WORKSPACE / "tools/libtests_named_runner.py"
    env = dict(os.environ)
    parent = str(path.parent.parent)
    env["PYTHONPATH"] = parent + (os.pathsep + env["PYTHONPATH"] if env.get("PYTHONPATH") else "")
    with tempfile.TemporaryDirectory() as td:
        code, _text = _run_capture(
            [str(binary), str(runner), str(path), "test." + path.stem],
            td,
            env,
            timeout,
        )
        return code == 0


def run_one(binary: pathlib.Path, path: pathlib.Path, timeout: float):
    # `test_*.py` 需要 `import test.support` ⇒ 把 **test/ 的父目录**放进 PYTHONPATH
    # （否则会误报 ModuleNotFoundError: No module named 'test.support' —— 首版踩过 ✓）。
    env = dict(os.environ)
    parent = str(path.parent.parent)
    env["PYTHONPATH"] = parent + (os.pathsep + env["PYTHONPATH"] if env.get("PYTHONPATH") else "")
    with tempfile.TemporaryDirectory() as td:
        code, out = _run_capture([str(binary), str(path)], td, env, timeout)
        if code is None:
            return path.name, "timeout", "超时"
        lines = out.strip().splitlines()
        msg = lines[-1] if lines else ""
        returncode = code
        # 分类按 CLI 的结构化消息（"未捕获（状态 N）"）而非进程退出码：
        # 退出码在"打不开文件/语法错"等路径上并不唯一，状态码才稳定。
        if "状态 2" in out:
            kind = "syntax"
        elif "状态 5" in out:
            kind = "unimplemented"
        elif "未捕获" in out or returncode != 0:
            kind = "runtime"
        else:
            kind = "ok"
        # **上下文复核**（round 63）：直接跑失败、但按导入名跑通过 ⇒ 记为 ok（官方跑法 ✓）。
        if kind in ("runtime", "unimplemented") and run_named(binary, path, timeout):
            return path.name, "ok", "OK (named-import harness)"
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
    # **分片**（round 131：DSH 单条命令墙钟上限 10 分钟时用 ✓；两片各自跑完再合并 tsv ✓）。
    ap.add_argument("--start", type=int, default=0)
    ap.add_argument("--end", type=int, default=0)
    ap.add_argument("--timeout", type=float, default=120.0)
    # **按文件名跳过**（round 201）：`test_ntpath` 在这台机器的高负载下会卡在不可中断 IO（连
    # SIGKILL 都杀不掉，实测把 survey 驱动挂到平台 600 s 上限 ✗）⇒ 允许显式跳过，结果由
    # `libtests_survey_merge.py --extra` 按既有口径补成 timeout ✓（口径不变 ✓）。
    ap.add_argument("--skip", action="append", default=[], metavar="FILE")
    # **可续跑**（round 132）：每轮只跑 `--seconds` 秒、进度存 `*.progress.json`；跑满 397 个文件后再汇报总数 ✓
    # （本会话单条命令 ~600 s 上限 ⇒ 一次跑不完整个 survey ✓）。
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--seconds", type=float, default=420.0)
    ap.add_argument("--reset-progress", action="store_true")
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
    if args.start or args.end:
        files = files[args.start : (args.end or None)]
    # **`--skip` 在切片之后**：`--start/--end` 的**序号口径**因此不受跳过影响 ✓（与既有分片命令兼容 ✓）。
    if args.skip:
        skipped = set(args.skip)
        files = [p for p in files if p.name not in skipped]

    rows = []
    reasons = collections.Counter()
    progress_path = OUT.with_suffix(".progress.json")
    if args.resume:
        import json
        import time as _time

        done = {}
        if progress_path.is_file() and not args.reset_progress:
            try:
                done = json.loads(progress_path.read_text(encoding="utf-8"))
            except (json.JSONDecodeError, OSError):
                done = {}
        pending = [p for p in files if p.name not in done]
        started = _time.monotonic()
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            for offset in range(0, len(pending), args.jobs):
                batch = pending[offset : offset + args.jobs]
                for name, kind, msg in pool.map(
                    lambda p: run_one(binary, p, args.timeout), batch
                ):
                    done[name] = (kind, msg)
                    if kind == "syntax":
                        reasons[msg] += 1
                progress_path.write_text(
                    json.dumps(done, ensure_ascii=False), encoding="utf-8"
                )
                # 每批都刷新 tsv（排序后写）⇒ 中途被上限截断也不丢结果 ✓
                with OUT.open("w", encoding="utf-8") as fh:
                    fh.write("file\tkind\treason\n")
                    for name in sorted(done):
                        kind, msg = done[name]
                        fh.write(f"{name}\t{kind}\t{msg}\n")
                if _time.monotonic() - started >= args.seconds:
                    break
        rows = [(name, *done[name]) for name in sorted(done)]
        if len(done) < len(files):
            print(
                f"survey 可续跑：本轮累计 {len(done)}/{len(files)} 个文件"
                f"（预算 {args.seconds:.0f}s）"
            )
    else:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            for name, kind, msg in pool.map(
                lambda p: run_one(binary, p, args.timeout), files
            ):
                rows.append((name, kind, msg))
                if kind == "syntax":
                    reasons[msg] += 1

    OUT.parent.mkdir(parents=True, exist_ok=True)
    if not args.resume:
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
