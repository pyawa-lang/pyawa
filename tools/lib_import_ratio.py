#!/usr/bin/env python3
"""`M3` 判据① 的仪器：**`Lib/` 里"能 import"的文件比例**（`DESIGN.md` §9 的解锁曲线）。

判据原文（`docs/PLAN-milestones.md` §6 的 `M3` 行）：`CM-14` fan-in 前 5 个 C 模块
（`sys`／`itertools`／`time`／`errno`／`builtins`）⇒ **可 import 的比例 ≥ 67%**。
`CM-15`：**它只衡量"能 import"、不衡量语义正确**，且禁止当作完成度或验收。

量法（与对拍**同一条路** ✓）：每个 `Lib/**/*.py` 造一段 `import <模块名>` 的脚本，
丢给对拍用的那个**子进程入口**（`pyawa_side_runner`，ABI 路径 ✓）跑一遍；
**顶层执行跑完、没有未捕获异常** ⇒ 记"能 import" ✓。

用法：
    python3 tools/lib_import_ratio.py            # 全部 `Lib/**/*.py`
    python3 tools/lib_import_ratio.py --verbose  # 逐个打印结果与首个异常

退出码：比例 **< 67%** 时非零（便于当门用 ✓）。
"""

import argparse
import os
import pathlib
import re
import subprocess
import sys

WORKSPACE = pathlib.Path(__file__).resolve().parent.parent
LIB = WORKSPACE / "Lib"
THRESHOLD = 67.0


def module_name(path: pathlib.Path) -> str:
    relative = path.relative_to(LIB).with_suffix("")
    parts = list(relative.parts)
    if parts[-1] == "__init__":
        parts.pop()
    return ".".join(parts)


def find_runner() -> pathlib.Path:
    candidates = [
        entry
        for entry in (WORKSPACE / "target" / "debug" / "deps").glob("conformance-*")
        if entry.is_file() and not entry.name.endswith(".d")
    ]
    if not candidates:
        sys.exit("✗ 没有测试二进制：先跑一次 `cargo test -p pyawa-abi --test conformance`")
    return max(candidates, key=lambda entry: entry.stat().st_mtime)


def try_import(runner: pathlib.Path, module: str, scratch: pathlib.Path) -> str | None:
    """返回 `None` 表示能 import ✓；否则返回**首个异常的说明** ✓。"""
    source = scratch / f"import_{module.replace('.', '_')}.py"
    source.write_text(f"import {module}\nprint('ok')\n")
    env = dict(
        os.environ,
        PYAWA_CONFORMANCE_SOURCE=str(source.resolve()),
        PYAWA_CONFORMANCE_PROBES="0",
    )
    try:
        child = subprocess.run(
            [str(runner), "--exact", "pyawa_side_runner", "--nocapture"],
            env=env,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        return "超时（120 s）"
    out = child.stdout
    i, j = out.find("---BEGIN---"), out.find("---END---")
    block = out[i:j] if i >= 0 and j > i else out
    kinds = [l.split("=", 1)[1] for l in block.splitlines() if l.startswith("exception_type=")]
    kinds = [k for k in kinds if k]
    if kinds:
        messages = [l.split("=", 1)[1] for l in block.splitlines() if l.startswith("exception_message=")]
        return f"{kinds[0]}: {messages[0] if messages else ''}".strip()
    if child.returncode != 0:
        return f"子进程退出码 {child.returncode}"
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--verbose", action="store_true", help="逐个打印")
    args = parser.parse_args()

    runner = find_runner()
    scratch = WORKSPACE / "target" / "lib-import-ratio"
    scratch.mkdir(parents=True, exist_ok=True)

    files = sorted(path for path in LIB.rglob("*.py") if "__pycache__" not in path.parts)
    ok, bad = 0, []
    for path in files:
        module = module_name(path)
        failure = try_import(runner, module, scratch)
        if failure is None:
            ok += 1
            if args.verbose:
                print(f"✓ {module}")
        else:
            bad.append((module, failure))
            if args.verbose:
                print(f"✗ {module} ⇒ {failure}")
    total = len(files)
    ratio = (ok / total * 100) if total else 0.0
    print(f"`Lib/` 文件 {total} 个 ⇒ **能 import {ok} 个 ⇒ {ratio:.1f}%**（判据① 阈值 {THRESHOLD:.0f}%）")
    for module, failure in bad:
        print(f"  ✗ {module} ⇒ {failure[:100]}")
    return 0 if ratio >= THRESHOLD else 1


if __name__ == "__main__":
    sys.exit(main())
