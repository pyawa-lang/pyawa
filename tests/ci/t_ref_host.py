#!/usr/bin/env python3
"""**通用参考宿主的可执行验收**（PLAN 收口后下一阶段 ④；2026-11-13）。

判据：`examples/reference_host.c` —— **与宿主身份无关**的 C 宿主示例 —— 必须

1. 真编译（`cc`，缺编译器即红，不跳过）、真链接（`pyawa-abi` 的 `staticlib`）；
2. 真运行且退出码 0、**stdout 逐行等于** `EXPECTED`（示例里每一步的断言都由示例自己完成；
   这里只钉"整串可复现"，防止文档漂移）；
3. `pa.h` 里确实声明了本示例用到的配额两函数（`AB-65`／`AB-66`）——头文件与实现同步，
   下游宿主才能"照抄"。

示例覆盖（对应 `DESIGN.md` §7.5）：创建／宿主脚本 API 回调／沙箱拒绝越界（宿主 API 白名单）／
超时（协作式中断）／每实例内存配额（`MemoryError` 可捕获）／实例隔离／销毁。

用法::

    python3 tests/ci/t_ref_host.py [--release]
"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
EXAMPLE = ROOT / "examples" / "reference_host.c"
HEADER = ROOT / "crates" / "pyawa-abi" / "include" / "pa.h"
INCLUDE = ROOT / "crates" / "pyawa-abi" / "include"

EXPECTED = """abi=ok
callback=42
echoed=hello
sandbox-escape=rejected
sandbox-inside=hello from host
quota=caught
memory-used=positive
instances=isolated
timeout=interrupted
done"""


def fail(message: str) -> int:
    print(f"[T-REF-HOST] 红：{message}")
    return 1


def main(argv: list[str]) -> int:
    release = "--release" in argv
    profile = "release" if release else "debug"

    if not EXAMPLE.is_file():
        return fail(f"参考宿主不在：{EXAMPLE}")
    if not HEADER.is_file():
        return fail(f"头文件不在：{HEADER}")
    header = HEADER.read_text(encoding="utf-8")
    for declaration in ("pa_set_memory_limit", "pa_memory_used"):
        if declaration not in header:
            return fail(f"pa.h 缺 `{declaration}` 声明（AB-65／AB-66；下游无法照抄）")
    for label in ("AB-65", "AB-66"):
        if label not in header:
            return fail(f"pa.h 缺 `{label}` 注释（ABI-adjacent 的出处）")

    if shutil.which("cc") is None:
        return fail("本机没有 `cc`：参考宿主必须真编译真运行，缺编译器**不跳过**")

    build = ["cargo", "build", "-p", "pyawa-abi"] + (["--release"] if release else [])
    built = subprocess.run(build, cwd=ROOT, capture_output=True, text=True)
    if built.returncode != 0:
        print(built.stdout)
        print(built.stderr)
        return fail(f"`{' '.join(build)}` 失败")

    archive = ROOT / "target" / profile / "libpyawa_abi.a"
    if not archive.is_file():
        return fail(f"静态库不在：{archive}（`crate-type` 里应当有 `staticlib`）")

    with tempfile.TemporaryDirectory(prefix="pyawa-ref-host-") as directory:
        binary = pathlib.Path(directory) / "reference_host"
        compile_command = [
            "cc",
            str(EXAMPLE),
            "-I",
            str(INCLUDE),
            str(archive),
            "-lpthread",
            "-ldl",
            "-lm",
            "-o",
            str(binary),
        ]
        compiled = subprocess.run(compile_command, capture_output=True, text=True)
        if compiled.returncode != 0:
            print(compiled.stderr)
            return fail("`cc` 编译／链接失败")
        try:
            run = subprocess.run([str(binary)], capture_output=True, text=True, timeout=60)
        except subprocess.TimeoutExpired:
            return fail("参考宿主 60 s 未结束（超时演示失效？）")
        if run.returncode != 0:
            print(run.stdout)
            print(run.stderr)
            return fail(f"参考宿主退出码 {run.returncode}（非 0）")
        if run.stdout.strip() != EXPECTED:
            print("--- 实际 stdout ---")
            print(run.stdout)
            print("--- 期望 stdout ---")
            print(EXPECTED)
            return fail("stdout 与期望不一致（示例/文档漂移？）")

    print("[T-REF-HOST] 绿：参考宿主真编译真运行，六项演示逐行符合期望")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
