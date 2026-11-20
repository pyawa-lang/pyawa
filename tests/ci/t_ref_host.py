#!/usr/bin/env python3
"""**通用参考宿主的可执行验收**（PLAN 收口后下一阶段 ④；2026-11-13）。

判据：`examples/reference_host.c` —— **与宿主身份无关**的 C 宿主示例 —— 必须

1. 真编译（`cc`，缺编译器即红，不跳过）、真链接（`pyawa-abi` 的 `staticlib`）；
2. 真运行且退出码 0、**stdout 逐行等于** `EXPECTED`（示例里每一步的断言都由示例自己完成；
   这里只钉"整串可复现"，防止文档漂移）；
3. `pa.h` 里确实声明了本示例用到的配额两函数（`AB-65`／`AB-66`）——头文件与实现同步，
   下游宿主才能"照抄"。

示例覆盖（对应 `DESIGN.md` §7.5）：创建／宿主脚本 API 回调／沙箱拒绝越界（宿主 API 白名单）／clock 能力域 vtable（§12.17 A2）／
超时（协作式中断）／每实例内存配额（`MemoryError` 可捕获）／实例隔离／销毁／
`clock` 能力域 vtable（§12.17 A2）。

用法::

    python3 tests/ci/t_ref_host.py [--release]
"""

from __future__ import annotations

import os
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
clock=ok
memory-used=positive
instances=isolated
timeout=interrupted
done"""


def fail(message: str) -> int:
    print(f"[T-REF-HOST] 红：{message}")
    return 1


def main(argv: list[str]) -> int:
    # **Windows 控制台默认 cp1252**（GitHub `windows-latest` 实测）：中文输出会
    # `UnicodeEncodeError` ⇒ 本脚本显式 UTF-8（先于任何 print）。
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="replace")

    release = "--release" in argv
    profile = "release" if release else "debug"

    if not EXAMPLE.is_file():
        return fail(f"参考宿主不在：{EXAMPLE}")
    if not HEADER.is_file():
        return fail(f"头文件不在：{HEADER}")
    header = HEADER.read_text(encoding="utf-8")
    capability_header = INCLUDE / "pa_capabilities.h"
    if not capability_header.is_file():
        return fail("能力域头文件不在：crates/pyawa-abi/include/pa_capabilities.h")
    if "pa_clock_vtable" not in capability_header.read_text(encoding="utf-8"):
        return fail("pa_capabilities.h 缺 pa_clock_vtable")
    for declaration in ("pa_set_memory_limit", "pa_memory_used"):
        if declaration not in header:
            return fail(f"pa.h 缺 `{declaration}` 声明（AB-65／AB-66；下游无法照抄）")
    for label in ("AB-65", "AB-66"):
        if label not in header:
            return fail(f"pa.h 缺 `{label}` 注释（ABI-adjacent 的出处）")

    windows = os.name == "nt"
    if windows:
        # Windows：MSVC `cl`（CI 用 `ilammy/msvc-dev-cmd` 把它放上 PATH）；缺编译器同样**不跳过**。
        if shutil.which("cl") is None:
            return fail("Windows 上需要 MSVC 的 `cl.exe` 在 PATH（CI 用 `msvc-dev-cmd` 设置）")
    elif shutil.which("cc") is None:
        return fail("本机没有 `cc`：参考宿主必须真编译真运行，缺编译器**不跳过**")

    build = ["cargo", "build", "-p", "pyawa-abi"] + (["--release"] if release else [])
    # **Windows：链接器必须显式钉到 MSVC 的 `link.exe`**——GitHub runner 的 Git Bash 里
    # `C:\Program Files\Git\usr\bin\link.exe` 是 coreutils 的 `link`，Rust 解析会先撞上它
    # （实测报 "extra operand … symbols.o"）。`VCToolsInstallDir` 的布局随 VS 版本变（VS18
    # 实测那格存在但候选目录对不上）⇒ 用 `glob` 在 `VSINSTALLDIR\VC\Tools\MSVC\*\bin\Hostx64\x64`
    # 里找，并用 `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER` 把结论显式交给 cargo。
    build_env = dict(os.environ)
    cl_path = None
    if windows:
        candidates = []
        vc = build_env.get("VCToolsInstallDir", "")
        if vc:
            vc_root = pathlib.Path(vc)
            candidates += [vc_root / "bin" / "Hostx64" / "x64", vc_root / "bin" / "Hostx86" / "x64"]
        vs = build_env.get("VSINSTALLDIR", "")
        if vs:
            candidates += sorted(
                (pathlib.Path(vs) / "VC" / "Tools" / "MSVC").glob("*/bin/Hostx64/x64"), reverse=True
            )
        tool_dir = next((c for c in candidates if (c / "link.exe").is_file()), None)
        if tool_dir is None:
            return fail("Windows 上找不到 MSVC 的 link.exe；候选目录：" + "；".join(str(c) for c in candidates))
        build_env["PATH"] = str(tool_dir) + os.pathsep + build_env.get("PATH", "")
        build_env["CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"] = str(tool_dir / "link.exe")
        cl_path = tool_dir / "cl.exe"
    built = subprocess.run(build, cwd=ROOT, capture_output=True, text=True, env=build_env)
    if built.returncode != 0:
        print(built.stdout)
        print(built.stderr)
        return fail(f"`{' '.join(build)}` 失败")

    # Windows 的 staticlib 产物是 `.lib`（MSVC）；其余平台是 `.a`。
    archive = ROOT / "target" / profile / ("pyawa_abi.lib" if windows else "libpyawa_abi.a")
    if not archive.is_file():
        return fail(f"静态库不在：{archive}（`crate-type` 里应当有 `staticlib`）")

    with tempfile.TemporaryDirectory(prefix="pyawa-ref-host-") as directory:
        binary = pathlib.Path(directory) / ("reference_host.exe" if windows else "reference_host")
        if windows:
            # MSVC：`cl /Fe:…`；Rust 的 staticlib 自带系统库 link 指令（kernel32 等）。
            # 编译器用上面定位到的 MSVC 目录里的绝对路径（Git Bash 的 `/usr/bin` 在 PATH 前面）。
            compile_command = [
                str(cl_path) if cl_path is not None and cl_path.is_file() else "cl",
                "/nologo",
                f"/I{INCLUDE}",
                str(EXAMPLE),
                str(archive),
                # Rust 的 staticlib 从 C 链接时要**显式**带 Windows 系统库（首跑实测缺 33 个
                # `__imp_*`：ws2_32／ntdll／userenv 等）；顺序无关，链接器会自动去重。
                "ws2_32.lib",
                "ntdll.lib",
                "userenv.lib",
                "advapi32.lib",
                "bcrypt.lib",
                f"/Fe:{binary}",
            ]
        else:
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
        compiled = subprocess.run(
            compile_command,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if compiled.returncode != 0:
            # **两端都打**：MSVC `cl` 把错误写 stdout（实测先前只打 stderr ⇒ 空输出、看不出病因）。
            print("--- 编译器 stdout ---")
            print(compiled.stdout)
            print("--- 编译器 stderr ---")
            print(compiled.stderr)
            print("--- 命令 ---")
            print(" ".join(compile_command))
            return fail("编译／链接失败（MSVC `cl`；详见上方两端输出）")
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

    print("[T-REF-HOST] 绿：参考宿主真编译真运行，七项演示逐行符合期望")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
