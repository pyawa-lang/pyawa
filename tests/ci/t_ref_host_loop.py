#!/usr/bin/env python3
"""P0-3 参考宿主 loop 验收：native poll(2) event provider＋任务表＋超时。"""
from __future__ import annotations
import pathlib, subprocess, sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = ROOT / "examples" / "reference_host_loop.c"
INCLUDE = ROOT / "crates" / "pyawa-abi" / "include"
LIB = ROOT / "target" / "release"
BIN = ROOT / "target" / "reference_host_loop"
EXPECTED = """ref-loop: tasks=2
ref-loop: ready=2
ref-loop: f=42
ref-loop: g=44
ref-loop: timeout=0
ref-loop: OK"""

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
    print("ref-host-loop PASS")
    return 0

if __name__ == "__main__":
    sys.exit(main())
