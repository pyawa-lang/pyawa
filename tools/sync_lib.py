#!/usr/bin/env python3
"""`Lib/` 与上游 CPython 3.14.x 的**同步**与**零差异校验**（`CX-8`）。

约束定义在 `docs/CONSTRAINTS.md` 的 `CX-8`（"`Lib/` 与上游 CPython 3.14.x **文件哈希零差异**；
例外清单**必须为空**"），本脚本只**实现**检查，不重述约束内容。

用法::

    python3 tools/sync_lib.py --check     # 校验 Lib/ 里每个文件与上游逐字节一致；不一致 ⇒ 退出码 1
    python3 tools/sync_lib.py --sync      # 按脚本里的 SLICE 从上游复制（**只增不改**）

上游前缀默认取**本机 python3 的 stdlib 路径**（必须报 3.14.x；`CX-8` 只对 3.14.x 成立）。
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
LIB = ROOT / "Lib"

#: 初始切片：`site.py` 直接导入的那些 ＋ 它们最短的依赖链（**只增不改** ✓）。名字带目录时按原样拷贝。
SLICE = (
    "os.py",
    "stat.py",
    "posixpath.py",
    "genericpath.py",
    "_collections_abc.py",
    "_sitebuiltins.py",
    "warnings.py",
    # 目标 ① 点名的两份（逐字放进 Lib/ ⇒ 天然满足 CX-8 ✓）＋ 包的 __init__ 与 _abc
    "importlib/__init__.py",
    "importlib/_abc.py",
    "importlib/_bootstrap.py",
    "importlib/_bootstrap_external.py",
)


def upstream_prefix() -> pathlib.Path:
    out = subprocess.run(
        [sys.executable, "-c", "import sysconfig; print(sysconfig.get_paths()['stdlib'])"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    prefix = pathlib.Path(out)
    if not (prefix / "os.py").exists():
        raise SystemExit(f"上游前缀不像 stdlib：{prefix}")
    return prefix


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check(prefix: pathlib.Path) -> int:
    """逐字节比对 `Lib/` 里的**每个**文件（例外清单必须为空 ⇒ 不设白名单 ✓）。"""
    if not LIB.exists():
        print("Lib/ 尚未引入（CX-8 无从谈起）")
        return 0
    problems: list[str] = []
    files = sorted(item for item in LIB.rglob("*") if item.is_file())
    for item in files:
        relative = item.relative_to(LIB)
        upstream = prefix / relative
        if not upstream.exists():
            problems.append(f"上游没有这个文件：{relative}")
            continue
        if digest(item) != digest(upstream):
            problems.append(f"与上游不一致：{relative}")
    print(f"CX-8：Lib/ 共 {len(files)} 个文件，与上游 {prefix} 比对完毕")
    for problem in problems:
        print(f"  ✗ {problem}")
    if problems:
        return 1
    print("  ✓ 全部逐字节一致（例外清单为空）")
    return 0


def sync(prefix: pathlib.Path) -> int:
    LIB.mkdir(exist_ok=True)
    copied = 0
    for name in SLICE:
        source = prefix / name
        if not source.exists():
            print(f"  ✗ 上游没有 {name}")
            return 1
        target = LIB / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        copied += 1
    print(f"已从 {prefix} 复制 {copied} 个文件到 Lib/")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Lib/ 与上游 CPython 的同步与零差异校验（CX-8）")
    parser.add_argument("--check", action="store_true", help="校验零差异")
    parser.add_argument("--sync", action="store_true", help="按 SLICE 复制（只增不改）")
    args = parser.parse_args()
    if not args.check and not args.sync:
        parser.print_help()
        return 2
    prefix = upstream_prefix()
    return check(prefix) if args.check else sync(prefix)


if __name__ == "__main__":
    raise SystemExit(main())
