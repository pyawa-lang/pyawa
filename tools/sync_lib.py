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
    # **`site.py` 本体**（第 157 轮补 ✓）：`SLICE` 的注释一直说以它为起点 ✓，但列表里漏了它 ✗。
    "site.py",
    # **`abc.py` 的下一跳**（第 172 轮 ✓）：`_py_abc` 会连带要它 ✓
    "_weakrefset.py",
    # **`abc.py` 的再下一跳**（第 173 轮 ✓）
    "types.py",
    "warnings.py",
    # `Lib/os.py` 导入期就要的（第 137 轮）
    "abc.py",
    "_py_abc.py",
    # 目标 ① 点名的两份（逐字放进 Lib/ ⇒ 天然满足 CX-8 ✓）＋ 包的 __init__ 与 _abc
    "importlib/__init__.py",
    "importlib/_abc.py",
    "importlib/_bootstrap.py",
    "importlib/_bootstrap_external.py",
    # **`codecs` 与 `encodings` 那一族**（第 282 轮 ✓）：`_codecs` 落地之后它们才有底座 ✓；
    # 一条 glob 收全 123 个编解码模块 ✓（`encodings/*.py` 在 `Lib/` 里的路径原样保有 ✓）。
    "codecs.py",
    "encodings/*.py",
    # **第 288 轮**：`_io` 的面补齐（`io.DEFAULT_BUFFER_SIZE` ✓）＋ `IMPORT_FROM` 缺名改报
    # `ImportError` ＋ 类型的 `__doc__` 兜底之后，`tools/find_syncable.py` 量出**现在就能同步**
    # 的模块（判据① 的分子就是这么长的 ✓ —— 它只同步"按上游逐字节放进来、且 `Lib/` ＋ 内建就能
    # import 成功"的那些 ✓）。
    # **包逐文件列** ✓（不用 glob ✗）：`lib_compile` 那道闸门要求"`Lib/` 里**每个**文件都过
    # 编译期不变量" ✓ ⇒ 编不过的（`_pyrepl` 20 个、`urllib/parse.py`、`urllib/request.py`、
    # `wsgiref` 3 个 ✓）**没有**同步进来 ✓，glob 会把它们又拉回来 ✗。
    "__future__.py",
    "__hello__.py",
    "__phello__/__init__.py",
    "__phello__/spam.py",
    "_apple_support.py",
    "_ios_support.py",
    "_opcode_metadata.py",
    "_pyrepl/__init__.py",
    "_pyrepl/__main__.py",
    "_pyrepl/input.py",
    "_pyrepl/pager.py",
    "_pyrepl/unix_eventqueue.py",
    "bisect.py",
    "colorsys.py",
    "compression/__init__.py",
    "compression/_common/__init__.py",
    "compression/_common/_streams.py",
    "compression/bz2.py",
    "compression/gzip.py",
    "compression/lzma.py",
    "compression/zlib.py",
    "compression/zstd/__init__.py",
    "compression/zstd/_zstdfile.py",
    "concurrent/__init__.py",
    "concurrent/futures/__init__.py",
    "concurrent/futures/_base.py",
    "concurrent/futures/interpreter.py",
    "concurrent/futures/process.py",
    "concurrent/futures/thread.py",
    "concurrent/interpreters/__init__.py",
    "concurrent/interpreters/_crossinterp.py",
    "concurrent/interpreters/_queues.py",
    "filecmp.py",
    "graphlib.py",
    "importlib/machinery.py",
    "io.py",
    "linecache.py",
    "netrc.py",
    "operator.py",
    "pydoc_data/__init__.py",
    "pydoc_data/module_docs.py",
    "pydoc_data/topics.py",
    "quopri.py",
    "reprlib.py",
    "sitecustomize.py",
    "this.py",
    "token.py",
    "urllib/__init__.py",
    "urllib/error.py",
    "urllib/response.py",
    "urllib/robotparser.py",
    "wsgiref/__init__.py",
    "wsgiref/headers.py",
    "wsgiref/simple_server.py",
    "wsgiref/validate.py",
    "xmlrpc/__init__.py",
    "xmlrpc/client.py",
    "xmlrpc/server.py",
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
        # **支持 glob**（第 282 轮）：`encodings/*.py` 那一族 123 个文件按一条写 ✓
        #（一条一条列既啰嗦又容易漏 ✓）。一个都没匹配到 ⇒ **报错**（不是静默跳过 ✓）。
        if any(character in name for character in "*?["):
            matches = sorted(prefix.glob(name))
            if not matches:
                print(f"  ✗ 上游没有匹配 {name} 的文件")
                return 1
            for source in matches:
                if not source.is_file():
                    continue
                relative = source.relative_to(prefix)
                target = LIB / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
                copied += 1
            continue
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
