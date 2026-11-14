#!/usr/bin/env python3
"""**§12.17 A3**：能力域 vtable 的 C ↔ Rust 布局一致性机检（需 `cc`）。

判据（任一不满足即红）：
1. `tools/gen_pa_capabilities.py --check`：提交的 `pa_capabilities.h`／`capability_layout.rs`
   与 Rust `repr(C)` 源一致（字段/类型漂移 ⇒ 红）；
2. C 探针（由生成器 `--emit-c-probe` 产出）在 `cc -std=c99` 下编译、运行，打印 3 域每个字段的
   `offset/size` 与结构体尺寸；
3. Rust 测试 `crates/pyawa-abi/tests/capability_layout.rs` 打印同一口径；
4. 两侧逐行一致（字段名/顺序/offset/尺寸任一漂移 ⇒ 红）；
5. `pa_capabilities.h` 能被 **C 与 C++ 独立 include**（AB-46；不依赖先 include `pa.h`）。
"""

from __future__ import annotations

import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
HEADER_DIR = ROOT / "crates" / "pyawa-abi" / "include"
HEADER = HEADER_DIR / "pa_capabilities.h"
GENERATOR = ROOT / "tools" / "gen_pa_capabilities.py"
RUST_TEST = ROOT / "crates" / "pyawa-abi" / "tests" / "capability_layout.rs"
LINE = re.compile(r"^layout\|([a-z_]+)\|([A-Za-z0-9_@]+)\|(\d+)\|(\d+)$")


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, **kwargs)


def parse_layout(text: str) -> dict[tuple[str, str], tuple[int, int]]:
    out: dict[tuple[str, str], tuple[int, int]] = {}
    for line in text.splitlines():
        match = LINE.match(line.strip())
        if match:
            key = (match.group(1), match.group(2))
            if key in out:
                raise SystemExit(f"[FAIL] 重复的布局行：{key}")
            out[key] = (int(match.group(3)), int(match.group(4)))
    return out


def main() -> int:
    failures: list[str] = []
    # 1) 生成物与 Rust 源一致。
    check = run([sys.executable, str(GENERATOR), "--check"])
    if check.returncode != 0:
        failures.append(check.stderr.strip() or check.stdout.strip())

    # 2) C 探针（生成 → 编译 → 运行）。
    if shutil.which("cc") is None:
        failures.append("找不到 `cc`（本机检必须真编译；不跳过）")
    if shutil.which("c++") is None:
        failures.append("找不到 `c++`（AB-46 要求 C/C++ 都能 include）")
    c_layout: dict[tuple[str, str], tuple[int, int]] = {}
    with tempfile.TemporaryDirectory() as td:
        td_path = pathlib.Path(td)
        probe_c = td_path / "capability_layout_probe.c"
        emit = run([sys.executable, str(GENERATOR), "--emit-c-probe", str(probe_c)])
        if emit.returncode != 0:
            failures.append(f"生成 C 探针失败：{emit.stderr.strip()}")
        else:
            probe_bin = td_path / "probe"
            compile_probe = run(
                ["cc", "-std=c99", "-Wall", "-Wextra", "-Werror", f"-I{HEADER_DIR}", str(probe_c), "-o", str(probe_bin)]
            )
            if compile_probe.returncode != 0:
                failures.append(f"C 探针编译失败：{compile_probe.stderr.strip()}")
            else:
                executed = run([str(probe_bin)])
                if executed.returncode != 0:
                    failures.append(f"C 探针运行失败：{executed.stderr.strip()}")
                c_layout = parse_layout(executed.stdout)

        # 5) 两个头独立 include（C 与 C++）。
        for language, compiler, suffix in (("C", "cc", ".c"), ("C++", "c++", ".cc")):
            source = td_path / f"include_only{suffix}"
            source.write_text('#include "pa_capabilities.h"\nint main(void) { return 0; }\n', encoding="utf-8")
            included = run([compiler, "-std=c99" if language == "C" else "-std=c++17",
                            "-Wall", "-Wextra", "-Werror", f"-I{HEADER_DIR}", str(source), "-o", str(td_path / f"inc_{language}")])
            if included.returncode != 0:
                failures.append(f"`pa_capabilities.h` 不能被 {language} 独立 include：{included.stderr.strip()}")

    # 3) Rust 侧布局。
    rust = run(["tools/cargo.sh", "test", "-p", "pyawa-abi", "--test", "capability_layout", "--", "--nocapture"])
    if rust.returncode != 0:
        failures.append(f"Rust 布局探针测试失败：{(rust.stderr or rust.stdout).strip()}")
    rust_layout = parse_layout(rust.stdout)

    # 4) 逐行一致。
    if c_layout and rust_layout:
        missing = sorted(set(rust_layout) - set(c_layout))
        extra = sorted(set(c_layout) - set(rust_layout))
        mismatch = sorted(
            key for key in set(c_layout) & set(rust_layout) if c_layout[key] != rust_layout[key]
        )
        if missing:
            failures.append(f"C 探针缺 Rust 侧字段：{missing[:8]}")
        if extra:
            failures.append(f"C 探针多出字段：{extra[:8]}")
        for key in mismatch[:8]:
            failures.append(f"布局不一致 {key}：C={c_layout[key]} Rust={rust_layout[key]}")
        if len(c_layout) != len(rust_layout):
            failures.append(f"行数不一致：C={len(c_layout)} Rust={len(rust_layout)}")

    if failures:
        for failure in failures:
            print(f"[FAIL] {failure}", file=sys.stderr)
        return 1
    print(f"[PASS] capability layout：C↔Rust 逐行一致（{len(c_layout)} 行，含 3 域结构/全部字段）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
