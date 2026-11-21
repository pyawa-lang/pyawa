#!/usr/bin/env python3
"""S3 参考宿主（异步）验收：C 程序 + runtime cdylib，宿主用 pa_resume 驱动生成器。"""
from __future__ import annotations
import pathlib, subprocess, sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = ROOT / "examples" / "reference_host_async.c"
INCLUDE = ROOT / "crates" / "pyawa-abi" / "include"
LIB = ROOT / "target" / "release"
BIN = ROOT / "target" / "reference_host_async"
EXPECTED = """ref-async: yield1=1
ref-async: yield2=3
ref-async: done=7
ref-async: event=1
ref-async: await-token=77
ref-async: event-token=1
ref-async: await-done=42
ref-async: OK"""

def main() -> int:
    subprocess.run(["cargo", "build", "--release", "-p", "pyawa-runtime", "--lib"], cwd=ROOT, check=True)
    subprocess.run([
        "cc", str(SRC), f"-I{INCLUDE}", f"-L{LIB}", "-lpyawa_runtime",
        f"-Wl,-rpath,{LIB}", "-lpthread", "-ldl", "-lm", "-o", str(BIN),
    ], cwd=ROOT, check=True)
    out = subprocess.run([str(BIN)], capture_output=True, text=True, check=True).stdout.strip()
    if out != EXPECTED:
        print("FAIL: output mismatch")
        print(out)
        print("--- expected ---")
        print(EXPECTED)
        return 1
    print("ref-host-async PASS")
    return 0

if __name__ == "__main__":
    sys.exit(main())
