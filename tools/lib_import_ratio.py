#!/usr/bin/env python3
"""`M3` 判据① 的仪器：**上游 `Lib/**/*.py` 全量**里"能 import"的比例。

判据原文（`docs/PLAN-milestones.md` §6 的 `M3` 行）：

- **分母 ＝ 上游 CPython 3.14 的 `Lib/**/*.py` 全量**（现 **628**）——**同步进度不改变分母**
  （否则比例可被"少同步"抬高）；仪器**须按上游清单统计**；
- **已同步子集**（本仓 `Lib/`）的通过率**另作进度指标**，**不作判据**（`CM-15`：只衡量"能 import"、
  不衡量语义正确）。

**两侧都跑**（用户裁定 A，第 279 轮 ✓）：

- **Pyawa 侧**：与对拍**同一条 ABI 路径**（`pyawa_side_runner` 子进程；`sys.path` ＝ 语料目录 ＋
  `Lib/` ✓）；
- **参照侧**：`python3 -S -c "import <模块>"` —— `-S` 是**与 Pyawa 侧对齐** ✓（本层不跑 `site.py`；
  带 `site` 的参照会先把 `os` 装好 ⇒ 像 `genericpath` 这类**循环导入**就"看起来能 import"了 ✗）。

分类（照 `MS-10` 的三分类口径 ✓）：

- **通过**：两侧都能 import ✓
- **参照口径**：参照侧 import 不了 ⇒ **不计我们失败** ✓（显式列出，逐条可查 ✓）
- **失败**：参照能、我们不能 ⇒ **这才是判据要看的缺口** ✓

判据① 的比值 ＝ (通过 ＋ 参照口径) ÷ 上游全量；低于 67% 时非零退出（便于当门用 ✓）。

用法::

    python3 tools/lib_import_ratio.py                 # 全量（628 个模块，两侧都跑）
    python3 tools/lib_import_ratio.py --verbose       # 逐个打印
    python3 tools/lib_import_ratio.py --limit 20      # 只跑前 20 个（冒烟）
    python3 tools/lib_import_ratio.py --jobs 8        # 并发度（默认 8）
    python3 tools/lib_import_ratio.py --pyawa-only    # 只跑 Pyawa 侧（开发时省时间；**不作判据**）
"""

from __future__ import annotations

import argparse
import concurrent.futures
import os
import pathlib
import subprocess
import sys

WORKSPACE = pathlib.Path(__file__).resolve().parent.parent
LIB = WORKSPACE / "Lib"
THRESHOLD = 67.0

# 上游前缀这**一处真相**在 `tools/sync_lib.py`（`CX-8` 的同步与校验用它）✓ ⇒ 这里**复用**它 ✓。
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sync_lib import upstream_prefix  # noqa: E402


def module_name(path: pathlib.Path, root: pathlib.Path) -> str:
    relative = path.relative_to(root).with_suffix("")
    parts = list(relative.parts)
    if parts[-1] == "__init__":
        parts.pop()
    return ".".join(parts)


def upstream_modules() -> list[str]:
    """上游 `Lib/**/*.py` 的**全量**模块名（判据① 的分母 ✓）。"""
    root = upstream_prefix()
    names = {
        module_name(path, root) for path in root.rglob("*.py") if "__pycache__" not in path.parts
    }
    return sorted(names)


def find_runner() -> pathlib.Path:
    candidates = [
        entry
        for entry in (WORKSPACE / "target" / "debug" / "deps").glob("conformance-*")
        if entry.is_file() and not entry.name.endswith(".d")
    ]
    if not candidates:
        sys.exit("✗ 没有测试二进制：先跑一次 `cargo test -p pyawa-abi --test conformance`")
    return max(candidates, key=lambda entry: entry.stat().st_mtime)


def pyawa_import_failure(runner: pathlib.Path, module: str, scratch: pathlib.Path) -> str | None:
    """返回 `None` 表示 Pyawa 侧能 import ✓；否则返回**首个异常的说明** ✓。"""
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
    kinds = [line.split("=", 1)[1] for line in block.splitlines() if line.startswith("exception_type=")]
    kinds = [kind for kind in kinds if kind]
    if kinds:
        messages = [
            line.split("=", 1)[1] for line in block.splitlines() if line.startswith("exception_message=")
        ]
        return f"{kinds[0]}: {messages[0] if messages else ''}".strip()
    if child.returncode != 0:
        return f"子进程退出码 {child.returncode}"
    return None


def reference_import_failure(module: str, scratch: pathlib.Path) -> str | None:
    """参照侧口径：`python3 -S -c "import <模块>"`（与 Pyawa 侧"不跑 `site.py`"对齐 ✓）。

    返回 `None` 表示参照能 import ✓；否则返回首个异常的**一句话**（stderr 末行 ✓）。
    """
    try:
        child = subprocess.run(
            [sys.executable, "-S", "-c", f"import {module}"],
            cwd=scratch,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        return "超时（120 s）"
    if child.returncode == 0:
        return None
    tail = [line for line in child.stderr.strip().splitlines() if line.strip()]
    return tail[-1] if tail else f"退出码 {child.returncode}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--verbose", action="store_true", help="逐个打印")
    parser.add_argument("--limit", type=int, default=0, help="只跑前 N 个（冒烟用）")
    parser.add_argument("--jobs", type=int, default=8, help="并发度（默认 8）")
    parser.add_argument("--pyawa-only", action="store_true", help="只跑 Pyawa 侧（不作判据）")
    args = parser.parse_args()

    runner = find_runner()
    scratch = WORKSPACE / "target" / "lib-import-ratio"
    scratch.mkdir(parents=True, exist_ok=True)
    reference_scratch = scratch / "reference"
    reference_scratch.mkdir(parents=True, exist_ok=True)

    modules = upstream_modules()
    total_upstream = len(modules)
    if args.limit:
        modules = modules[: args.limit]

    def probe(module: str) -> tuple[str, str | None, str | None]:
        pyawa = pyawa_import_failure(runner, module, scratch)
        reference = None if args.pyawa_only else reference_import_failure(module, reference_scratch)
        return module, pyawa, reference

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        results = list(pool.map(probe, modules))

    through: list[str] = []
    reference_scope: list[tuple[str, str | None, str]] = []
    failures: list[tuple[str, str]] = []
    for module, pyawa, reference in results:
        if reference is None and pyawa is None:
            through.append(module)
        elif reference is not None:
            reference_scope.append((module, pyawa, reference))
        else:
            failures.append((module, pyawa or ""))

    if args.verbose:
        for module, pyawa, reference in results:
            if reference is None and pyawa is None:
                print(f"✓ {module}")
            elif reference is not None:
                print(f"○ {module} ⇒ 参照口径：{reference[:100]}（本层：{(pyawa or '能 import')[:60]}）")
            else:
                print(f"✗ {module} ⇒ {pyawa[:100]}")

    if args.pyawa_only:
        # **不作判据**：只报 Pyawa 侧的成功率（开发时省时间 ✓）
        print(
            f"Pyawa 侧：{len(modules)} 个模块 ⇒ 能 import "
            f"{len(through) + len(reference_scope)} 个（`--pyawa-only`：**不作判据** ✓）"
        )
    else:
        print(f"参照侧口径：`{sys.executable} -S -c \"import <模块>\"`（与 Pyawa 侧『不跑 site.py』对齐）")
        numerator = len(through) + len(reference_scope)
        ratio = numerator / total_upstream * 100 if total_upstream else 0.0
        print(
            f"`M3` 判据①（分母＝上游 `Lib/**/*.py` 全量 **{total_upstream}**）："
            f"**通过 {len(through)} ＋ 参照口径 {len(reference_scope)} ＝ {numerator} ÷ {total_upstream}"
            f" ⇒ {ratio:.1f}%**（阈值 {THRESHOLD:.0f}%）"
        )
        print(f"  ✗ 我们失败 **{len(failures)}** 条（参照能、我们不能 —— 判据要看的缺口）")
        for module, message in failures[:15]:
            print(f"      ✗ {module} ⇒ {message[:100]}")
        if len(failures) > 15:
            print(f"      …… 另有 {len(failures) - 15} 条（`--verbose` 看全部）")
        print(f"  ○ 参照口径 **{len(reference_scope)}** 条（参照自己也 import 不了 ⇒ 不计我们失败）")
        for module, pyawa, reference in reference_scope[:15]:
            state = "本层也失败" if pyawa else "本层能 import"
            print(f"      ○ {module} ⇒ 参照：{reference[:80]}｜{state}")
        if len(reference_scope) > 15:
            print(f"      …… 另有 {len(reference_scope) - 15} 条（`--verbose` 看全部）")

    # **进度指标**（不作判据 ✓）：本仓 `Lib/` 已同步子集的通过率 ✓
    synced = sorted(path for path in LIB.rglob("*.py") if "__pycache__" not in path.parts)
    if synced:
        with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
            synced_results = list(
                pool.map(
                    lambda path: (
                        module_name(path, LIB),
                        pyawa_import_failure(runner, module_name(path, LIB), scratch),
                    ),
                    synced,
                )
            )
        good = [module for module, failure in synced_results if failure is None]
        print(
            f"  进度指标（**不作判据** ✓，`CM-15`）：`Lib/` 已同步子集 {len(synced)} 个文件 ⇒ "
            f"能 import {len(good)} 个 ⇒ {len(good) / len(synced) * 100:.1f}%"
        )
        for module, failure in synced_results:
            if failure is not None:
                print(f"      ✗ {module} ⇒ {(failure or '')[:100]}")

    if args.pyawa_only or args.limit:
        return 0
    ratio = (len(through) + len(reference_scope)) / total_upstream * 100 if total_upstream else 0.0
    return 0 if ratio >= THRESHOLD else 1


if __name__ == "__main__":
    sys.exit(main())
