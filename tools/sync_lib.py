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
    # **task-1 收账**：形参 cell 快路修好（`Lib/functools.py::singledispatch.register` 的
    # `argname, cls = next(...)` 不再把 cell 对象当值 ✓）＋ `posix.remove` 补成 `unlink` 别名 ✓
    # ⇒ `importlib.resources` 一族 10 个从"同步了却 import 失败"转"通过" ✓ ⇒ 按上游逐字节列进 `SLICE` ✓。
    # **用 `*.py` 的 glob** ✓：`importlib/resources/**` 会把上游的 `__pycache__/*.pyc` 也匹配进来 ✗
    # ⇒ 既违反 `CX-8`（字节码必然不同 ✗）又污染 `Lib/` ✗。
    "importlib/readers.py",
    "importlib/simple.py",
    "importlib/resources/**/*.py",
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
    # **task-24 收账**（第 412 轮 ✓）：`itertools.chain` 的引用计数 UAF 修好后，find_syncable
    # 又量出这些整包/文件现在能 import ✓ —— `multiprocessing`（整包 ✓）、`unittest`（整包 ✓）、
    # `wsgiref.handlers`／`wsgiref.util`（`wsgiref.simple_server` 仍卡 `platform` 解析
    # `sys.version`，其文件早已在表里 ✓）。包用 `**/*.py`（同 `tomllib` 先例 ✓；只吃源文件 ✓）。
    "multiprocessing/**/*.py",
    "unittest/**/*.py",
    "wsgiref/handlers.py",
    "wsgiref/util.py",
    # **task-26 收账**（第 413 轮 ✓）：`tuple.index`（asyncio 链的最后一格）＋ `from M import *`
    # 尊重 `__all__`（`asyncio/__init__.py` 靠它把各子模块的 `__all__` 拼起来 ✓）落地后
    # **`import asyncio` 通** ✓ ⇒ 整包同步（包用 `**/*.py` 同 `tomllib` 先例 ✓；逐个 import 过 ✓）。
    "asyncio/**/*.py",
    "xmlrpc/__init__.py",
    "xmlrpc/client.py",
    "xmlrpc/server.py",
    # **第 292 轮**：类关键字转交元类（`enum` 那一族的头）＋ `for x in a, b:` ＋ `frozenset` 的
    # 迭代面修好之后，`find_syncable` 又量出 4 个模块（`email`／`xml`／`getopt`／`test`）——
    # 仍是**逐文件**（`lib_compile` 那道闸门要求 `Lib/` 里每个文件都过编译期不变量；
    # 编不过的已被 `--prune` 删掉）。
    "email/__init__.py",
    "email/_encoded_words.py",
    "email/_policybase.py",
    "email/base64mime.py",
    "email/charset.py",
    "email/contentmanager.py",
    "email/encoders.py",
    "email/errors.py",
    "email/feedparser.py",
    "email/generator.py",
    "email/header.py",
    "email/headerregistry.py",
    "email/iterators.py",
    "email/message.py",
    "email/mime/__init__.py",
    "email/mime/application.py",
    "email/mime/audio.py",
    "email/mime/base.py",
    "email/mime/image.py",
    "email/mime/message.py",
    "email/mime/multipart.py",
    "email/mime/nonmultipart.py",
    "email/mime/text.py",
    "email/parser.py",
    "email/policy.py",
    "xml/__init__.py",
    "xml/dom/NodeFilter.py",
    "xml/dom/__init__.py",
    "xml/dom/domreg.py",
    "xml/dom/minicompat.py",
    "xml/dom/minidom.py",
    "xml/dom/pulldom.py",
    "xml/dom/xmlbuilder.py",
    "xml/etree/ElementInclude.py",
    "xml/etree/ElementPath.py",
    "xml/etree/__init__.py",
    "xml/etree/cElementTree.py",
    "xml/parsers/__init__.py",
    "xml/parsers/expat.py",
    "xml/sax/__init__.py",
    "xml/sax/_exceptions.py",
    "xml/sax/expatreader.py",
    "xml/sax/handler.py",
    "xml/sax/xmlreader.py",
    "getopt.py",
    "test/__init__.py",
    "test/__main__.py",
    "test/libregrtest/__init__.py",
    "test/libregrtest/filter.py",
    "test/libregrtest/parallel_case.py",
    "test/libregrtest/pgo.py",
    "test/libregrtest/tsan.py",
    "test/libregrtest/win_utils.py",
    "test/regrtest.py",
    "test/support/_hypothesis_stubs/__init__.py",
    "test/support/_hypothesis_stubs/_helpers.py",
    "test/support/_hypothesis_stubs/strategies.py",
    "test/support/ast_helper.py",
    "test/support/asynchat.py",
    "test/support/asyncore.py",
    "test/support/bytecode_helper.py",
    "test/support/hashlib_helper.py",
    "test/support/hypothesis_helper.py",
    "test/support/i18n_helper.py",
    "test/support/import_helper.py",
    "test/support/logging_helper.py",
    "test/support/numbers.py",
    "test/support/refleak_helper.py",
    "test/support/script_helper.py",
    "test/support/smtpd.py",
    "test/support/testcase.py",
    "test/support/threading_helper.py",
    "test/support/venv.py",
    "test/support/warnings_helper.py",
    "test/test_support.py",
    "test/typinganndata/__init__.py",
    "test/typinganndata/ann_module7.py",
    "test/typinganndata/ann_module9.py",
    "test/typinganndata/fwdref_module.py",
    "test/typinganndata/partialexecution/__init__.py",
    "test/typinganndata/partialexecution/b.py",
    # **第 303 轮**：容器的 `__contains__` 属性 ＋ `staticmethod`／`classmethod` 取用之后，
    # `find_syncable` 又量出 1 个模块 ✓。
    "keyword.py",
    # **第 309 轮**：`_weakref` 的面扩上 ＋ `object.__hash__` 之后，
    # `find_syncable` 又量出 `weakref` ✓。
    "weakref.py",
    # **第 313 轮**：`maketrans` 一族之后，`find_syncable` 量出 `collections` ✓。
    "collections/__init__.py",
    # **第 317 轮**：`time`／`clock` 之后，`find_syncable` 量出 `profile` ✓。
    "profile.py",    # **第 405 轮修**：这 11 条原先只写了**模块名**（`_compat_pickle`／`ast`／…）⇒ `--sync`
    # 在 `prefix / name` 上找不到文件而**整体退出** ✗。SLICE 的元素一律是"相对上游 stdlib 的
    # **文件路径**"（包写 `x/__init__.py`，不用裸目录名 ✗ —— 裸目录的 glob 还会把
    # `__pycache__/*.pyc` 拉进来 ✗）⇒ 这里补成真实 `.py` 路径 ✓。
    "_compat_pickle.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "_py_warnings.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "_pydatetime.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "ast.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "contextvars.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "datetime.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "ntpath.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "shlex.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "test/typinganndata/ann_module4.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "test/typinganndata/ann_module8.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）
    "_pyrepl/main.py",  # 第 318 轮由 find_syncable 收账（只增不改 ✓）

    # **task-24 尾件**：`typing.py` 逐字节同步 ✓ —— 它要靠 PEP 695 全套（`class C[T]`／`def f[T]`／
    # `[T: bound]`／`type X = …`／方法注解两层闭包 ✓）＋ `_collections.defaultdict` ✓；
    # 本条同时是 `pathlib.types`／`importlib.metadata`／`importlib.resources` 那一串的前置 ✓。
    "typing.py",

    # **task-4 收账**（2026-10-17 ✓）：`set`／`dict` 的**就地位运算族**（`|=`／`&=`／`-=`／`^=`）接线
    # （＋共享树里形参 cell 修复）之后，`tools/find_syncable.py` 量出这一批现在就能 import ⇒
    # 按上游**逐字节**同步 ✓。两道必检 ✓（**逐个 import** ✓ ＋ 无关脚本 `startup ok` ✓）；
    # `pyawa-core --test lib_compile` 闸门 ✓（`Lib/` 里每个文件都过编译期不变量 ✓）。
    # **逐文件列**（不用 glob ✗）：`dbm` 包里 `gnu.py`／`sqlite3.py` 缺 C 基座
    # （`_gdbm`／`sqlite3`）⇒ 它们**不在**本清单 ✓（§6.4：只同步能 import 的文件 ✓）。
    "_android_support.py",
    "_ast_unparse.py",
    "_markupbase.py",
    "_osx_support.py",
    "_pyio.py",
    "_pyrepl/_threading_handler.py",
    "_pyrepl/commands.py",
    "_pyrepl/console.py",
    "_pyrepl/trace.py",
    "_pyrepl/types.py",
    "cmd.py",
    "code.py",
    "codeop.py",
    "dbm/__init__.py",
    "dbm/dumb.py",
    "dbm/ndbm.py",
    "locale.py",
    "modulefinder.py",
    "numbers.py",
    "optparse.py",
    "pkgutil.py",
    "pprint.py",
    "py_compile.py",
    "pyclbr.py",
    "rlcompleter.py",
    "runpy.py",
    "sched.py",
    "tabnanny.py",
    "tarfile.py",
    "test/typinganndata/ann_module5.py",
    "test/typinganndata/ann_module6.py",
    "uuid.py",
    "wave.py",
    "zipapp.py",

    # **P3-31 解析器缺口批收账**（第 405 轮 ✓）：嵌套元组目标／`with … as (a, b)`／推导式模式目标／
    # 调用基底下标赋值／前导点浮点／元组 `*`／行内 if 分号表＋次行 else／带注解的非名字目标／
    # 制表符缩进／`*` 后置关键字实参 —— 十格修完之后 `tools/find_syncable.py`（3 轮）量出这 11 个
    # 模块现在就能 import ✓（逐个 import 过 ✓）；按上游**逐字节**同步 ✓。
    # **包用 `**/*.py` 的 glob** ✓（`tomllib/**/*.py` 只吃源文件，不会把 `__pycache__` 拉进来 ✓；
    # 上游 `tomllib/mypy.ini` 不是模块 ⇒ 不在同步面 ✓）。
    "_pyrepl/base_eventqueue.py",
    "_pyrepl/keymap.py",
    "_pyrepl/windows_eventqueue.py",
    "argparse.py",
    "difflib.py",
    "gettext.py",
    "platform.py",
    "pstats.py",
    "test/typinganndata/ann_module3.py",
    "tomllib/**/*.py",
    "xml/etree/ElementTree.py",

    # **task-10 收账**（2026-10-17 ✓）：`posix.getcwd`／`getcwdb`／`readlink` 接线（走 `fs` 能力域的
    # `readlink` 槽 ＋ `std::env::current_dir()` 最小路径）之后，`tools/find_syncable.py` 量出
    # `sysconfig` 与 `_aix_support` 现在就能 import ✓（逐个 import 过 ✓、`lib_compile` 闸门 ✓、
    # 无关脚本 `startup ok` ✓）；按上游**逐字节**同步 ✓。
    # **包逐文件列** ✓（不用 glob ✗ —— `Lib/` 里每个文件都要过编译期不变量）。
    # **如实记** ✗：`sysconfig.get_paths()` 仍会撞 `sys.abiflags`（`sys` 面未接，见台账）——
    # 那不影响"能 import"这个口径（判据①只看 import ✓）。
    # **enum 换真货清债点**（第 411 轮，task-22 B6 ✓）：假货 `Lib/enum.py` 的
    # `_convert_`／`global_enum` 两笔过渡面随整文件消失；CX-8 的 enum 偏差从此**归零** ✓。
    # 前置（B1–B3 ＋ B5 四格）已让**上游逐字节真本**整文件 import（53/53）＋ http 一族可导入 ✓。
    "enum.py",
    # **B6 顺带收账**（第 411 轮，task-22 ✓）：`mimetypes` 的链式 `global` 赋值真 bug 修完后可 import ✓
    # （它是 `http.server` 的前置；`http.server` 本轮实测**段错误** ⇒ 未收、已立据 ✓）。
    "mimetypes.py",
    # **task-24 收账**（第 412 轮 ✓）：`itertools.chain` 的引用计数真 bug 修好后
    # `import http.server` 不再 SIGSEGV ⇒ 收（`wsgiref.simple_server` 仍卡 `posix.environ`
    # 之外的 `platform`/`sys.version` 解析面，`xmlrpc.server` 卡 `pyexpat` C 模块 ⇒ 均未收）。
    "http/server.py",
    "sysconfig/__init__.py",
    "sysconfig/__main__.py",
    "_aix_support.py",

    # **task-11 小格批收账**（第 406 轮 ✓）：`dir` 内建／`_operator._compare_digest`／
    # 生成器表达式自由变量（先前**编译期 panic**）／类体 `__classdict__` cell 槽错位 四格落地后
    # `tools/find_syncable.py`（4 轮、**先重建 conformance 二进制** ✓）量出这 11 个模块现在就能
    # import ✓（逐个 import 过 ✓；`_operator` 在 Rust 侧、不在这张文件表里 ✓）。
    "_pydecimal.py",
    "_pylong.py",
    "calendar.py",
    "decimal.py",
    "fractions.py",
    "hmac.py",
    "pickle.py",
    "pickletools.py",
    "secrets.py",
    "shelve.py",
    "statistics.py",

    # **task-12 收账（P3-29 socket 第一轮，2026-10-17 ✓）**：Rust 侧 `_socket` 面落地
    # （常量/`has_ipv6`/类与异常面为真；I/O 函数面如实报 `NotImplementedError`）
    # ＋ 假货 `Lib/enum.py` 补 `_convert_` 过渡面之后，`import socket` 通 ✓（逐个 import 过 ✓）。
    # 按上游**逐字节**同步 ✓（`socket.py` 是 `Lib/` 里的新文件，不改旧文件）。
    "socket.py",

    # **task-13 收账（`_signal` 最小面，2026-10-17 ✓）**：Rust 侧 `_signal` 面落地
    # （45 常量真值；`signal`/`getsignal` 参数校验照参照；调用如实报 `NotImplementedError`）
    # ⇒ `import signal` 通 ✓（逐个 import 过 ✓；census 最大族 asyncio 的第一道 C 模块闸 ✓）。
    # 按上游**逐字节**同步 ✓（`signal.py` 是 `Lib/` 里的新文件，不改旧文件）。
    "signal.py",

    # **task-15 收账（`_posixsubprocess` 最小面 ＋ subprocess 导入链，2026-10-17 ✓）**：
    # Rust 侧 `_posixsubprocess`（`fork_exec` 名字/元数）＋ `select`（收窄面）＋ `posix` 的
    # `waitpid` 一族（`WNOHANG` 常量与 `WIFSTOPPED`/`WSTOPSIG`/`waitstatus_to_exitcode` 真实现、
    # `waitpid` 如实未接线）之后，`import selectors`／`import subprocess` 逐个通 ✓。
    # 按上游**逐字节**同步 ✓（两份都是 `Lib/` 里的新文件，不改旧文件）。
    "selectors.py",
    "subprocess.py",

    # **task-14 收账顺带（`__file__` 落地＋subprocess 链解锁，2026-10-17 ✓）**：
    # `find_syncable` 量出并逐个 import 通过后同步 ✓（socket/subprocess 链下游）。
    "ftplib.py",
    "mailbox.py",
    "poplib.py",

    # **task-16 收账**（第 408 轮 ✓）：两个 core import 真 bug（循环相对导入的 `sys.modules` 回退、
    # 失败模块从 `sys.modules` 清理）修完之后，`tools/find_syncable.py` 量出这 2 个模块现在就能
    # import ✓（逐个 import 过 ✓；`socketserver` 来自前者、`partialexecution.a` 是那个包里
    # `from . import a` 的循环形状 ✓）。
    "socketserver.py",
    "test/typinganndata/partialexecution/a.py",

    # **task-3 收账（官方测试套件包骨架，2026-10-24 ✓）**：`Lib/test` 此前只有 42 个 `.py` ✓，
    # 而上游 `test` **包骨架**（`support/__init__.py`／`libregrtest` 的 runner 一族／
    # `typinganndata` 的注解夹具）**不在** deb 数据包里 ✓、只在本机 stdlib（`upstream_prefix()`，
    # 即 `CX-8` 的上游口径）里 ✓ ⇒ `import test.support` 直接 `No module named 'test.support'` ✗。
    # 这里**逐文件列 19 个**（不用目录 glob ✗）：其余 7 个（`libregrtest/result.py`／
    # `libregrtest/utils.py`／`support/channels.py`／`support/os_helper.py`／
    # `typinganndata/ann_module.py`／`ann_module2.py`／`ann_module695.py`）当时过不了
    # `tools/cargo.sh test -p pyawa-core --test lib_compile` 的编译期不变量 ✗（`P3-35` 的
    # **合法 Python 被拒**语法缺口：`for *x` 目标／surrogate escape `\uD800`／链式赋值非名字目标／
    # 注解赋值的括号名字与属性目标／PEP 695 `*Ts`）⇒ **暂不同步** ✓（`P3-35` 修完再收）；
    # 按 `MS-19` 判据补充：**不登记 KNOWN**、也不改编译器凑绿 ✗。**后续**：其中 4 个已由
    # **task-28 批 A** 解锁收进 ✓（见下）；剩 3 个仍不同步 ✓。
    "test/libregrtest/cmdline.py",
    "test/libregrtest/findtests.py",
    "test/libregrtest/logger.py",
    "test/libregrtest/main.py",
    "test/libregrtest/refleak.py",
    "test/libregrtest/results.py",
    "test/libregrtest/run_workers.py",
    "test/libregrtest/runtests.py",
    "test/libregrtest/save_env.py",
    "test/libregrtest/setup.py",
    "test/libregrtest/single.py",
    "test/libregrtest/testresult.py",
    "test/libregrtest/worker.py",
    "test/support/__init__.py",
    "test/support/pty_helper.py",
    "test/support/socket_helper.py",
    "test/support/strace_helper.py",
    "test/typinganndata/_typed_dict_helper.py",
    "test/typinganndata/mod_generics_cache.py",
    # **task-28 批 A 收账（`P3-35` 批 A 解锁，2026-10-24 ✓）**：parser 批（for-star 目标／链式元组
    # 目标／注解赋值的括号名字与属性目标）落地后，这 4 个现在过得了 `lib_compile` 的编译期不变量 ✓
    #（Lead 实测：`ann_module`／`ann_module2` 跑完无错；`channels.py` 编译过、运行期还缺
    # `_interpchannels`（另一格）；`result.py` 编译过、运行期 `from .utils import …` 还卡在
    # `utils.py` 的 surrogate escape（另一格）——`lib_compile` **只看编译** ✓ ⇒ 都可同步 ✓）。
    # 三个目录的 `__init__.py` 早已在表里／已同步 ✓ ⇒ 本批只补这 4 个文件（不重复、不覆盖 ✓）。
    "test/libregrtest/result.py",
    "test/support/channels.py",
    "test/typinganndata/ann_module.py",
    "test/typinganndata/ann_module2.py",
    # **`P3-35` 剩余 3 个仍不同步** ✓：`test/libregrtest/utils.py`（surrogate `\uD800-\uDFFF`
    # 字面量 ⇒ 要**string repr 级**改造，不是纯解析器格）、`test/support/os_helper.py`
    # （`\uDC80` 的运行时 surrogateescape 口径）、`test/typinganndata/ann_module695.py`
    # （PEP 695 `*Ts` 未接线）⇒ 修完再收；按 `MS-19` 判据补充：**不登记 KNOWN**、不改编译器 ✗。
    # **`sysconfig` 的 `_sysconfigdata` 数据模块**（task-3 顺带 ✓）：`sysconfig.get_config_var()`
    # 走 `importlib.import_module(_get_sysconfigdata_name())` ✓，官方 `test.support` 导入期的
    # `check_sanitizer()` 就要 `CFLAGS` ⇒ 这两个名字按本机 `sys.platform`／multiarch 口径列出 ✓
    #（后一个是前一个的符号链接，`shutil.copyfile` 落成逐字节相同的普通文件 ⇒ `CX-8` 仍成立 ✓）。
    "_sysconfigdata__linux_x86_64-linux-gnu.py",
    "_sysconfigdata__x86_64-linux-gnu.py",
)

#: `--sync` 要**跳过**的已知坏文件（相对上游 stdlib 的路径 ⇒ 原由）。**只影响 `--sync`** ✓；
#: `--check` 的语义不变 ✓（它只校验 `Lib/` 里**已有**的文件 ⇒ 不引白名单、例外清单仍为空 ✓）。
#: 这些都是**合法 Python 却被 pyawa 的编译期不变量拒**的（`P3-35` 一族）⇒ 按 `MS-19` 判据补充
#: **不登记 KNOWN**、也不改编译器凑绿；暂不同步 ✓。
#: 先例：`36c450b` 把 `multiprocessing/heap.py`／`unittest/mock.py` 两处**按纪律剔除** ✓
#: （链式赋值非名字目标／虚数字面量 `1j`）——但 `multiprocessing/**/*.py` 与 `unittest/**/*.py`
#: 两条既有 glob 会把它们重新拉回来 ✗ ⇒ 这里显式排除 ✓（否则"跑一次 `--sync` 就红闸门" ✗）。
SYNC_EXCLUDE = frozenset(
    {
        "multiprocessing/heap.py",
        "unittest/mock.py",
    }
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
    # **跳过字节码缓存** ✓（第 703 轮 ✗ 修）：`Lib/**/__pycache__/*.pyc` 是**本机跑出来的产物** ✗，
    # 与上游必然不同 ✓ ⇒ 先前它们把 `--check` 淹成一片红 ✗（实测报错**全是** `__pycache__` ✓，
    # 反而把"真正不一致的 `.py`"埋掉了 ✓）。`CX-8` 管的是**源文件** ✓。
    files = sorted(
        item
        for item in LIB.rglob("*")
        if item.is_file() and "__pycache__" not in item.parts
    )
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
    skipped = 0
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
                # **显式排除**（task-3 ✓）：既有 glob 会重新匹配上 `36c450b` 按纪律剔除的
                # 已知坏文件（`SYNC_EXCLUDE`，`P3-35` 语法缺口）⇒ 跳过 ✓（`--check` 不受影响 ✓）。
                if relative.as_posix() in SYNC_EXCLUDE:
                    skipped += 1
                    continue
                target = LIB / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
                copied += 1
            continue
        # 逐文件条目也过同一张排除表（**一处真相** ✓：排除只在 `SYNC_EXCLUDE` 判 ✓）。
        if name in SYNC_EXCLUDE:
            skipped += 1
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
    if skipped:
        print(
            f"  按 SYNC_EXCLUDE 跳过 {skipped} 个已知坏文件"
            "（`P3-35` 语法缺口，暂不同步；不登记 KNOWN）"
        )
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
