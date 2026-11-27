#!/usr/bin/env python3
"""P0-3 参考宿主 loop 验收：native poll(2) event provider＋任务表＋超时，
以及宿主 loop 驱动含 `await` 协程的场景（正常完成／多次 resume／错误与 THROW 传播／
INTERRUPT 取消与恢复／销毁派发）。"""
from __future__ import annotations
import pathlib, subprocess, sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = ROOT / "examples" / "reference_host_loop.c"
INCLUDE = ROOT / "crates" / "pyawa-abi" / "include"
LIB = ROOT / "target" / "release"
BIN = ROOT / "target" / "reference_host_loop"
EXPECTED = """ref-loop: tasks=3
ref-loop: ready=3
ref-loop: f=42
ref-loop: g=44
ref-loop: multi-yields=2
ref-loop: multi=4
ref-loop: error-status=1
ref-loop: error-msg=LookupError: after
ref-loop: throw-status=1
ref-loop: throw-reason=2
ref-loop: interrupt-status=4
ref-loop: interrupt-reason=0
ref-loop: after-cancel=6
ref-loop: recovered=44
ref-loop: timeout=0
ref-loop: destroy-reason=1
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
