#!/usr/bin/env python3
"""**`T-AB-1`**：一个 **≤50 行**的 C 程序完成 `PLAN-milestones.md` §6 的 M1 判据（`MS-21`）。

判据：**创建实例 → 执行一段脚本 → 注入一个宿主函数 → 取回一个值**。示例是
`examples/m1.c`（45 行），本节把"**真编译、真运行**"做成一件事：

1. 数一遍示例的行数（> 50 行直接红——判据写的就是 ≤50）
2. `cargo build -p pyawa-abi`（C 侧要链的就是它的 `staticlib`，见该 crate 的 `crate-type`）
3. `cc examples/m1.c -I crates/pyawa-abi/include target/<profile>/libpyawa_abi.a …`
4. 跑起来，断言退出码 0 且打印 `42`（脚本里 `host_add(40, 2)` 的结果）

**缺 C 编译器就是红**：不跳过、不弱化（判据要么真成立、要么不成立；`cc` 不在就说明
本机没有能力**证明**它成立）。`--release` 走 release 产物，默认 debug。

用法::

    python3 tests/ci/t_ab_1.py [--release]
"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
EXAMPLE = ROOT / "examples" / "m1.c"
INCLUDE = ROOT / "crates" / "pyawa-abi" / "include"
MAX_LINES = 50
EXPECTED_STDOUT = "42"


def fail(message: str) -> int:
    print(f"[T-AB-1] 红：{message}")
    return 1


def main(argv: list[str]) -> int:
    release = "--release" in argv
    profile = "release" if release else "debug"

    if not EXAMPLE.is_file():
        return fail(f"示例不在：{EXAMPLE}（`MS-21` 要求它入库）")
    lines = len(EXAMPLE.read_text(encoding="utf-8").splitlines())
    if lines > MAX_LINES:
        return fail(f"示例 {lines} 行，判据说的是 ≤{MAX_LINES} 行")
    print(f"[T-AB-1] 示例：{EXAMPLE.relative_to(ROOT)}（{lines} 行 ≤ {MAX_LINES}）")

    if shutil.which("cc") is None:
        return fail("本机没有 `cc`：`T-AB-1` 要真编译真运行，缺编译器**不跳过**")

    build = ["cargo", "build", "-p", "pyawa-abi"] + (["--release"] if release else [])
    built = subprocess.run(build, cwd=ROOT, capture_output=True, text=True)
    if built.returncode != 0:
        print(built.stdout)
        print(built.stderr)
        return fail(f"`{' '.join(build)}` 失败")

    archive = ROOT / "target" / profile / "libpyawa_abi.a"
    if not archive.is_file():
        return fail(f"静态库不在：{archive}（`crate-type` 里应当有 `staticlib`）")

    with tempfile.TemporaryDirectory(prefix="pyawa-t-ab-1-") as directory:
        binary = pathlib.Path(directory) / "m1"
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
        run = subprocess.run([str(binary)], capture_output=True, text=True)
        if run.returncode != 0:
            print(run.stdout)
            print(run.stderr)
            return fail(f"示例退出码 {run.returncode}（非 0）")
        if run.stdout.strip() != EXPECTED_STDOUT:
            return fail(f"示例打印 {run.stdout.strip()!r}，期望 {EXPECTED_STDOUT!r}")

    print(f"[T-AB-1] 绿：真编译真运行，输出 {EXPECTED_STDOUT}（M1 判据成立）")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
