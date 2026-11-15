#!/usr/bin/env python3
"""**调用边诊断**（B3-2）：`PYAWA_REFCOUNT_CANARY=1` 下跑 `Lib/test` 顶层文件，收
`[B3-edge]` 行 ⇒ 「借用计数下降／上升」站点清单（文件:行）＋ 被调原生函数名。

口径：
* 下降 = 借用视图被当成所有权释放（`OM-19`／`AB-16` 违约 ⇒ 危险，**必查**）；
* 上升 = 被调原生函数保留了实参（可能是合法存储，也可能是泄漏 ⇒ **逐个看**）。

用法::

    python3 tools/call_edge_survey.py                  # 全量
    python3 tools/call_edge_survey.py --limit 120      # 抽样
    python3 tools/call_edge_survey.py --timeout 60

输出：`target/call_edge_survey.tsv`（`file\\tkind\\tnative\\targ\\tbefore\\tafter\\tsite`）。
"""
import argparse
import collections
import concurrent.futures
import os
import pathlib
import resource
import subprocess
import tempfile

WORKSPACE = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_DIR = WORKSPACE / "target/upstream/extracted/usr/lib/python3.14/test"
DEFAULT_BIN = WORKSPACE / "target/release/pyawa"
OUT = WORKSPACE / "target/call_edge_survey.tsv"
MARK = "[B3-edge]"


def _limit_memory(limit_bytes: int):
    """子进程内存上限（round 253）：canary 模式会把某些**本来就慢／占内存**的用例拖爆
    （实测 `test_ntpath` 在 canary 下 >2 GB ⇒ 触发环境的内存看门狗 SIGTERM 整个采集进程 ✗）⇒
    每个 pyawa 进程单独限内存，爆了只死那一个 ✓。"""
    try:
        resource.setrlimit(resource.RLIMIT_AS, (limit_bytes, limit_bytes))
    except (ValueError, OSError):
        pass


def run_one(binary: pathlib.Path, path: pathlib.Path, timeout: float, trace: bool, mem_limit: int):
    env = dict(os.environ)
    env["PYAWA_REFCOUNT_CANARY"] = "1"
    if trace:
        # **上升**日志会把每个 `intern`／`unicode_tolower` 之类都打一行 ⇒ 全量时别开 ✗
        # （round 253 实测：全量开 trace 的输出量可达每文件十多万行，采集体会爆）。
        env["PYAWA_CALL_EDGE_TRACE"] = "1"
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
                preexec_fn=lambda: _limit_memory(mem_limit),
            )
        except subprocess.TimeoutExpired:
            return path.name, []
    out = (proc.stderr or "") + "\n" + (proc.stdout or "")
    rows = []
    for line in out.splitlines():
        if MARK not in line:
            continue
        fields = line.split("\t")
        # `[B3-edge]` \t 下降/上升 \t native \t arg \t before \t after \t site
        if len(fields) < 7:
            continue
        kind = fields[1].strip()
        rows.append(
            (path.name, kind, fields[2].strip(), fields[3].strip(), fields[4].strip(), fields[5].strip(), fields[6].strip())
        )
    return path.name, rows


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=str(DEFAULT_DIR))
    ap.add_argument("--bin", default=str(DEFAULT_BIN))
    ap.add_argument("--jobs", type=int, default=max(4, (os.cpu_count() or 4) // 2))
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--start", type=int, default=0)
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--mem-limit", type=int, default=1536, help="每个 pyawa 进程的地址空间上限（MB）")
    ap.add_argument("--timeout", type=float, default=120.0)
    ap.add_argument("--trace", action="store_true", help="同时记「上升」站点（默认只记下降；抽样时开）")
    args = ap.parse_args()

    root = pathlib.Path(args.dir).resolve()
    binary = pathlib.Path(args.bin).resolve()
    if not root.is_dir():
        raise SystemExit(f"找不到 Lib/test 目录：{root}")
    if not binary.is_file():
        raise SystemExit(f"找不到 CLI：{binary}（先 cargo build --release --bin pyawa）")

    files = sorted(root.glob("test_*.py"))[args.start :]
    if args.limit:
        files = files[: args.limit]

    per_file = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        limit = args.mem_limit * 1024 * 1024
        for name, rows in pool.map(
            lambda p: run_one(binary, p, args.timeout, args.trace, limit), files
        ):
            per_file.append((name, rows))

    all_rows = []
    seen = set()
    for name, rows in per_file:
        for row in rows:
            key = (row[0], row[1], row[2], row[3], row[6])
            if key in seen:
                continue
            seen.add(key)
            all_rows.append(row)

    out_path = pathlib.Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w", encoding="utf-8") as fh:
        fh.write("file\tkind\tnative\targ\tbefore\tafter\tsite\n")
        for row in all_rows:
            fh.write("\t".join(row) + "\n")

    down = [row for row in all_rows if row[1] == "下降"]
    down_gc = [row for row in all_rows if row[1] == "下降(GC)"]
    up = [row for row in all_rows if row[1] == "上升"]
    files_with_down = {row[0] for row in down}
    files_with_up = {row[0] for row in up}
    print(f"扫描 {len(files)} 个顶层 test_*.py（PYAWA_REFCOUNT_CANARY=1 ＋ PYAWA_CALL_EDGE_TRACE=1）")
    print(
        f"  下降站点：{len(down)} 条（{len(files_with_down)} 个文件）✗  |  "
        f"下降(GC)：{len(down_gc)} 条  |  上升站点：{len(up)} 条（{len(files_with_up)} 个文件）"
    )
    if down:
        print("  下降 —— 文件／函数：")
        for name, count in collections.Counter((row[0], row[2]) for row in down).most_common(20):
            print(f"    {name[0]}  native={name[1]}  ×{count}")
    if down_gc:
        print("  下降(GC) —— 函数分布（前 10）：")
        for native, count in collections.Counter(row[2] for row in down_gc).most_common(10):
            print(f"    native={native}  ×{count}")
    if up:
        print("  上升 —— 函数分布（前 15）：")
        for native, count in collections.Counter(row[2] for row in up).most_common(15):
            print(f"    native={native}  ×{count}")
    print(f"  明细 → {out_path}")


if __name__ == "__main__":
    main()
