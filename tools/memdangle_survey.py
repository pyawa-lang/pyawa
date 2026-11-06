#!/usr/bin/env python3
"""内存安全基线：用**悬垂检测模式**（`PYAWA_DANGLING=1`）跑一遍 `Lib/test` 顶层文件。

口径（`docs/DESIGN.md` §10／台账 round 88 的"内存安全验收口径"）：
* **memory 类** ＝ 输出里出现 `[悬垂]`／`不在活表`／`对已释放对象`／`tcache`／
  `Segmentation fault`／`SIGSEGV`／`SIGABRT`／`malloc():` 之一 ⇒ 记为一条**内存安全 bug**；
* **timeout** ＝ 超时（不判性质，单独计数）；
* 其余沿用 `libtests_survey.py` 的口径（状态 2＝语法错／状态 5＝未实现／未捕获＝运行期错／退出 0＝跑通）。

用法::

    python3 tools/memdangle_survey.py                 # 全量（release CLI，jobs=默认）
    python3 tools/memdangle_survey.py --limit 20      # 抽样
    python3 tools/memdangle_survey.py --timeout 60
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
DEFAULT_DIR = WORKSPACE / "target/upstream/extracted/usr/lib/python3.14/test"
DEFAULT_BIN = WORKSPACE / "target/release/pyawa"
OUT = WORKSPACE / "target/memdangle_survey.tsv"

MEMORY_MARKERS = (
    "[悬垂]",
    "不在活表",
    "对已释放对象",
    "tcache",
    "Segmentation fault",
    "SIGSEGV",
    "SIGABRT",
    "malloc():",
    "double free",
    "corrupted",
)


def classify(out: str, returncode: int) -> tuple[str, str]:
    for marker in MEMORY_MARKERS:
        if marker in out:
            lines = [line for line in out.splitlines() if marker in line]
            return "memory", (lines[0][:200] if lines else marker)
    if "状态 2" in out:
        return "syntax", "语法错"
    if "状态 5" in out:
        return "unimplemented", "未实现"
    if "未捕获" in out or returncode != 0:
        lines = out.strip().splitlines()
        return "runtime", (lines[-1][:200] if lines else "运行期错")
    return "ok", "OK"


def run_one(binary: pathlib.Path, path: pathlib.Path, timeout: float):
    env = dict(os.environ)
    env["PYAWA_DANGLING"] = "1"
    env["PYAWA_ZOMBIE_TRACE"] = "1"
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
        except subprocess.TimeoutExpired:
            return path.name, "timeout", "超时"
        out = (proc.stderr or "") + "\n" + (proc.stdout or "")
        kind, msg = classify(out, proc.returncode)
        return path.name, kind, msg


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=str(DEFAULT_DIR))
    ap.add_argument("--bin", default=str(DEFAULT_BIN))
    ap.add_argument("--jobs", type=int, default=max(4, (os.cpu_count() or 4) // 2))
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--timeout", type=float, default=120.0)
    args = ap.parse_args()

    root = pathlib.Path(args.dir).resolve()
    binary = pathlib.Path(args.bin).resolve()
    if not root.is_dir():
        sys.exit(f"找不到 Lib/test 目录：{root}")
    if not binary.is_file():
        sys.exit(f"找不到 CLI：{binary}（先 cargo build --release --bin pyawa）")

    files = sorted(root.glob("test_*.py"))
    if args.limit:
        files = files[: args.limit]

    rows = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for name, kind, msg in pool.map(lambda p: run_one(binary, p, args.timeout), files):
            rows.append((name, kind, msg))

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8") as fh:
        fh.write("file\tkind\treason\n")
        for row in rows:
            fh.write("\t".join(row) + "\n")

    counts = collections.Counter(kind for _, kind, _ in rows)
    total = len(rows)
    memory = [row for row in rows if row[1] == "memory"]
    print(f"共 {total} 个 test_*.py（PYAWA_DANGLING=1 悬垂检测模式）")
    print(
        "  memory（内存安全类）: {m} ｜ timeout: {t} ｜ ok: {o} ｜ runtime: {r} ｜ "
        "unimplemented: {u} ｜ syntax: {s}".format(
            m=counts.get("memory", 0),
            t=counts.get("timeout", 0),
            o=counts.get("ok", 0),
            r=counts.get("runtime", 0),
            u=counts.get("unimplemented", 0),
            s=counts.get("syntax", 0),
        )
    )
    if memory:
        print("  memory 类文件（前 15 条签名）：")
        for name, _, msg in memory[:15]:
            print(f"    {name}: {msg}")
    print(f"明细 → {OUT}")


if __name__ == "__main__":
    main()
