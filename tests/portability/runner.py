#!/usr/bin/env python3
"""**P0-2 可移植门：在单个平台上跑语料并产出"归一化后"的 canonical JSON**（`PLAN §12.26`）。

设计（契约见 `REQUIREMENTS.md`「脚本可移植性承诺」／`DESIGN.md` §14）：

1. **每个平台各跑一遍**同一批 `corpus/*.py`（子进程 `pyawa probe.py`，cwd 是独立临时目录）；
2. **归一化**只在**这里**发生（把平台事实替换成占位符），**不改变运行时行为**：运行时对
   `sys.platform`／`os.name`／路径等**如实回答**；
3. 产出 canonical JSON，交给 `compare.py` 在**另一个平台**的产物上做**同脚本同结果**比对。

归一化白名单（与契约一致）：
- `PLATFORM=` → `<platform>`；`OSNAME=` → `<osname>`；`SEP=` → `<sep>`；
- `PATH=` → 分隔符统一成 `/`；`CWD=`／`TEMP=` → `<cwd>`／`<temp>`；
- `ERRNO=` → `<errno>`；`WINERROR=` → `<winerror>`；`TIME=` → `<time>`；`ADDRESS=` → `<addr>`。
其余行必须逐字一致（输出本身要写成确定性的：排序集合、固定浮点格式、不打地址/时间）。
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
CORPUS = pathlib.Path(__file__).resolve().parent / "corpus"

#: 平台事实标签（**只在这里归一化**；运行时仍如实回答）。
_TAGGED = {
    "PLATFORM": "<platform>",
    "OSNAME": "<osname>",
    "SEP": "<sep>",
    "CWD": "<cwd>",
    "TEMP": "<temp>",
    "ERRNO": "<errno>",
    "WINERROR": "<winerror>",
    "TIME": "<time>",
    "ADDRESS": "<addr>",
    "PATHMODULE": "<pathmodule>",
}
_TAG_RE = re.compile(r"^([A-Z]+)=(.*)$")


def normalize_line(line: str) -> str:
    """归一化**一行**输出（白名单之外的字节必须原样保留）。"""
    match = _TAG_RE.match(line)
    if match:
        tag, value = match.group(1), match.group(2)
        if tag == "PATH":
            # 路径分隔符是契约白名单：统一成 `/`（其余部分必须一致）。
            return "PATH=" + value.replace("\\", "/")
        if tag in _TAGGED:
            return f"{tag}={_TAGGED[tag]}"
    return line


def normalize_text(text: str) -> list[str]:
    return [normalize_line(line) for line in text.splitlines()]


def binary_path(explicit: str | None) -> pathlib.Path:
    if explicit:
        return pathlib.Path(explicit)
    name = "pyawa.exe" if os.name == "nt" else "pyawa"
    return ROOT / "target" / "debug" / name


def run_one(binary: pathlib.Path, script: pathlib.Path, timeout: int) -> dict:
    directory = tempfile.mkdtemp(prefix="pyawa-portability-")
    try:
        (pathlib.Path(directory) / "probe.py").write_bytes(script.read_bytes())
        try:
            completed = subprocess.run(
                [str(binary), "probe.py"],
                cwd=directory,
                capture_output=True,
                timeout=timeout,
                check=False,
            )
            exit_code = completed.returncode
            stdout = completed.stdout.decode("utf-8", errors="replace")
            stderr = completed.stderr.decode("utf-8", errors="replace")
        except subprocess.TimeoutExpired:
            exit_code = -1
            stdout = ""
            stderr = f"TIMEOUT after {timeout}s"
        return {
            "exit": exit_code,
            "stdout": normalize_text(stdout),
            "stderr": normalize_text(stderr),
        }
    finally:
        shutil.rmtree(directory, ignore_errors=True)


def discover() -> list[pathlib.Path]:
    return sorted(CORPUS.glob("*.py"))


def self_test() -> None:
    cases = [
        ("PLATFORM=linux", "PLATFORM=<platform>"),
        ("OSNAME=nt", "OSNAME=<osname>"),
        ("SEP=\\", "SEP=<sep>"),
        ("PATH=a\\b\\c.txt", "PATH=a/b/c.txt"),
        ("CWD=C:\\tmp\\x", "CWD=<cwd>"),
        ("ERRNO=ENOENT", "ERRNO=<errno>"),
        ("WINERROR=2", "WINERROR=<winerror>"),
        ("TIME=123.5", "TIME=<time>"),
        ("ADDRESS=0x7f00", "ADDRESS=<addr>"),
        ("PATHMODULE=ntpath", "PATHMODULE=<pathmodule>"),
        ("plain output", "plain output"),
        ("x=1", "x=1"),
    ]
    for source, expected in cases:
        actual = normalize_line(source)
        assert actual == expected, f"{source!r}: {actual!r} != {expected!r}"
    assert normalize_text("a\nPLATFORM=x\n") == ["a", "PLATFORM=<platform>"]
    print(f"portability normalize self-test OK（{len(cases)} 例）")


def main() -> int:
    # Windows 控制台默认 cp1252：中文汇总行会 UnicodeEncodeError（CI 实测）⇒ 固定 UTF-8。
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", default=None, help="pyawa 可执行文件（缺省 target/debug/pyawa[.exe]）")
    parser.add_argument("--out", default=None, help="canonical JSON 输出路径（缺省 stdout）")
    parser.add_argument("--corpus", default=None, help="语料目录（缺省 tests/portability/corpus）")
    parser.add_argument("--timeout", type=int, default=60)
    parser.add_argument("--self-test", action="store_true", help="只跑归一化自检")
    parser.add_argument("--raw-json", default=None,
                        help="把别的 runner（如 wasm 的 e8）产出的原始 JSON 归一化成 canonical JSON")
    parser.add_argument("--platform-label", default=None,
                        help="--raw-json 模式写入 meta.platform 的标签（缺省 raw 里的 meta）")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    if args.raw_json:
        raw = json.loads(pathlib.Path(args.raw_json).read_text(encoding="utf-8"))
        results = {}
        for name, record in sorted(raw.get("results", {}).items()):
            results[name] = {
                "exit": record["exit"],
                "stdout": normalize_text("\n".join(record.get("stdout", []))),
                "stderr": normalize_text("\n".join(record.get("stderr", []))),
            }
        document = {
            "meta": {
                "platform": args.platform_label or raw.get("meta", {}).get("platform", "raw"),
                "os_name": raw.get("meta", {}).get("os_name", "raw"),
                "corpus": len(results),
                "binary": raw.get("meta", {}).get("binary", "raw"),
            },
            "results": results,
        }
        text = json.dumps(document, ensure_ascii=False, indent=2, sort_keys=True)
        if args.out:
            pathlib.Path(args.out).write_text(text + "\n", encoding="utf-8")
            print(f"portability runner（raw 模式）OK：{len(results)} 个脚本 → {args.out}")
        else:
            print(text)
        return 0

    binary = binary_path(args.bin)
    if not binary.is_file():
        print(f"找不到 pyawa 二进制：{binary}（先 cargo build -p pyawa-runtime --bin pyawa）", file=sys.stderr)
        return 2

    global CORPUS
    if args.corpus:
        CORPUS = pathlib.Path(args.corpus)

    scripts = discover()
    if not scripts:
        print(f"语料为空：{CORPUS}", file=sys.stderr)
        return 2

    results = {}
    for script in scripts:
        results[script.name] = run_one(binary, script, args.timeout)

    document = {
        "meta": {
            "platform": sys.platform,
            "os_name": os.name,
            "corpus": len(scripts),
            "binary": str(binary),
        },
        "results": results,
    }
    text = json.dumps(document, ensure_ascii=False, indent=2, sort_keys=True)
    if args.out:
        pathlib.Path(args.out).write_text(text + "\n", encoding="utf-8")
        print(f"portability runner OK：{len(scripts)} 个脚本 → {args.out}")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
