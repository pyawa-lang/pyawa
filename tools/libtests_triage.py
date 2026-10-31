#!/usr/bin/env python3
"""对 `libtests_survey.py` 的失败项做**分诊**：把"跑得起来但断言失败"的测试归类成可执行清单。

输入：survey 的 tsv（默认 `target/libtests_survey.tsv`，由 `tools/libtests_survey.py` 产出）
范围：默认只挑 `kind == "runtime"` 且 reason 以 `FAILED` 开头的行 —— 即**测试跑起来了、
      断言/用例失败**的那些（这是"真语义差异"的信号）；`--all` 可放开到全部失败项。
做法：逐一用 CLI 独立重跑（临时 cwd、超时可控），从 `unittest` 输出里抽取：
      `ERROR: <test>`／`FAIL: <test>` 行 ＋ 其后首个异常行（`XxxError: msg`），归一化签名。
输出：`target/libtests_triage.tsv`（file / signature / detail）＋ 终端 Top 签名（按出现次数）。
口径：签名去掉测试名、路径与数字，只用于**排序与分组**；它不是判据（`CM-15` 同理）。
      未修项按 `MS-19` 判据补充处理：合法 Python 被拒/报错＝新差异，必须修。

用法::

    python3 tools/libtests_triage.py                 # 默认：只看 FAILED 类
    python3 tools/libtests_triage.py --all           # 全部失败项
    python3 tools/libtests_triage.py --limit 20 --release
"""
import argparse
import collections
import concurrent.futures
import os
import pathlib
import re
import subprocess
import sys
import tempfile

WORKSPACE = pathlib.Path(__file__).resolve().parent.parent
SURVEY = WORKSPACE / "target/libtests_survey.tsv"
DEFAULT_DIR = WORKSPACE / "target/upstream/extracted/usr/lib/python3.14/test"
DEFAULT_BIN = WORKSPACE / "target/debug/pyawa"
RELEASE_BIN = WORKSPACE / "target/release/pyawa"
OUT = WORKSPACE / "target/libtests_triage.tsv"

NOISE = re.compile(r"\b0x[0-9a-fA-F]+\b")
PATHS = re.compile(r"(/[\w./-]+)+")
DIGITS = re.compile(r"\d+")


def normalize(line: str) -> str:
    line = NOISE.sub("0xADDR", line)
    line = PATHS.sub("PATH", line)
    line = DIGITS.sub("N", line)
    return line.strip()[:160]


def extract_signature(text: str):
    """从 unittest 输出里取（首个 ERROR/FAIL 的测试名, 归一化后的异常行）。"""
    lines = text.splitlines()
    first_test = ""
    for i, line in enumerate(lines):
        if line.startswith(("ERROR:", "FAIL:")):
            first_test = line.strip()
            for follow in lines[i + 1 : i + 40]:
                m = re.search(r"^(\w*(?:Error|Exception|Failure|SkipTest))\b:?(.*)$", follow.strip())
                if m:
                    return first_test, normalize(m.group(0))
            return first_test, "(no exception line)"
    # 没跑到 unittest 汇总（崩溃/超时前退出）：退回最后一行
    for line in reversed(lines):
        if line.strip():
            return "", normalize(line)
    return "", "(empty output)"


def run_one(binary: pathlib.Path, path: pathlib.Path, timeout: float):
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
        except subprocess.TimeoutExpired:
            return path.name, "(timeout)", "超时"
        return (path.name,) + extract_signature((proc.stderr or "") + (proc.stdout or ""))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--survey", default=str(SURVEY))
    ap.add_argument("--dir", default=str(DEFAULT_DIR))
    ap.add_argument("--bin", default=None)
    ap.add_argument("--release", action="store_true")
    ap.add_argument("--all", action="store_true", help="包含非 FAILED 的失败项")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--jobs", type=int, default=max(4, (os.cpu_count() or 4) - 1))
    ap.add_argument("--timeout", type=float, default=60.0)
    args = ap.parse_args()

    survey = pathlib.Path(args.survey)
    if not survey.is_file():
        sys.exit(f"找不到 survey 结果：{survey}（先跑 tools/libtests_survey.py）")
    root = pathlib.Path(args.dir).resolve()
    binary = pathlib.Path(args.bin).resolve() if args.bin else None
    if binary is None:
        binary = (RELEASE_BIN if args.release else DEFAULT_BIN).resolve()

    rows = []
    for line in survey.read_text(encoding="utf-8").splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        name, kind, reason = parts[0], parts[1], parts[2]
        if kind == "ok":
            continue
        if not args.all and not reason.startswith("FAILED"):
            continue
        rows.append((name, kind, reason))
    if args.limit:
        rows = rows[: args.limit]

    results = []
    groups = collections.Counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for name, kind, reason in rows:
            _file, test, sig = pool.submit(
                run_one, binary, (root / name).resolve(), args.timeout
            ).result()
            results.append((name, test, sig))
            groups[sig] += 1

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8") as fh:
        fh.write("file\ttest\tsignature\n")
        for row in results:
            fh.write("\t".join(row) + "\n")

    print(f"分诊 {len(results)} 个失败项（来源 {survey.name}）")
    if groups:
        print("  头号签名（按出现次数）：")
        for sig, count in groups.most_common(12):
            print(f"    {count:4d} × {sig}")
    print(f"  明细 → {OUT}")


if __name__ == "__main__":
    main()
