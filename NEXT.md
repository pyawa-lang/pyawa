# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> 第 579–720 轮的长复盘已**收口归档**进 `docs/rounds/03-m3-progress-03.md` ✓（本笔同笔提交 ✓）。

## 现在

- **HEAD**：`85071d6`（`dev`）—— `ds-lib-sync` 的**task-15**（`reversed` 泛化：`__reversed__` 优先＋序列协议＋惰性迭代器）那笔 ✓（提交前已核对 `git rev-parse --short HEAD` ✓）
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓ —— **以本笔收口提交为准** ✓；本单页随其后一笔 docs 提交落地 ✓）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，本笔实测 ✓）：**通过 249 ＋ 参照口径 17 ＝ 266 ÷ 628 ⇒ 42.4%** ✗（阈值 67%）
  - 上一笔 **270 ÷ 628 ⇒ 43.0%** ⇒ 本笔 **＋0** ✗（**运行期**能力：`codecs`／`zipfile` 本来就 import 得动 ⇒ 判据① 不动，如实报 ✓）
  - 判据② **已达成** ✓：`target/recon/reparity.py` 的 **13 行**与参照（本机 `python3` 3.14）逐字一致 ✓
  - 进度指标（**不作判据** ✓，`CM-15`）：`Lib/` 已同步子集 **368 个文件 ⇒ 能 import 250 个 ⇒ 67.9%** ✓

## 下一条命令（Lead 队列：task-14 内建容器子类布局枢纽；`dis.py` 放回前的引用计数根因；`logging` 撞 re 引擎另立枢纽）

**用户已拍板路线 2** ✓：按"**阻塞最多模块的报错签名**"队列抬判据① 分子，目标 **≥ 421/628 ＝ 67%** ✓
（`enum.py` 换回支降为**低优先** ✓，不再主攻 ✓）。**排活口径＝全分母 628** ✓。

**枢纽⑨本笔落地** ✓：`_struct` **真实现** ✓（`crates/pyawa-stdlib/src/struct_module.rs`）——
`Struct` 类 ＋ 模块级 `pack`／`pack_into`／`unpack`／`unpack_from`／`iter_unpack`／`calcsize`／`_clearcache` ✓、
格式字符与字节序／对齐按参照 ✓、`str` 与 `bytes` 格式串都收 ✓、core 新增
`Instance::write_bytearray_at`（`pack_into` 只写可变的 `bytearray` ✓）。
**逐例与参照对** ✓：`calcsize` 八个形态 ✓、`pack`／`unpack`／`4s`／混合格式／`Struct`／`pack_into`／
三类报错文案（`bad char`／`unpack requires`／`argument out of range`）✓ —— 详见本笔提交正文 ✓。

**task-15 落地** ✓：**`reversed` 泛化** ✓ —— `__reversed__` 优先 ✓、序列协议（`length_with_protocol` ＋
新 `reversed` 迭代器类型 ＋ `advance` 走 `subscript_read` ✓）⇒ **惰性** ✓、错误文案照参照 ✓；
实测 tuple／str／range／bytes／list／自定义 `len+getitem`／自定义 `__reversed__` 全对 ✓。
**`import dis` 已通但未放回 `Lib/`** ✗：放回后 `slowcheck` 的
`file_backed_import_needs_the_fs_domain` 连续两次报 `对已释放对象 incref`（单跑通过、不带它全绿 ✓）
⇒ 归因未钉死，文件先留 `target/dis_hold.py` ✓。

**枢纽⑪选项②落地** ✓：**内建编码表**（`ascii`／`latin-1`／`utf-8`／`cp437` ✓，cp437 表由参照本机
`bytes(range(256)).decode('cp437')` 生成 ✓）＋ **Rust 造 `CodecInfo` 类** ✓ ⇒
`codecs.lookup` 四档都回真 `CodecInfo` ✓、`bytes.decode('cp437')`／`str.encode('cp437')` 对参照 ✓、
**zipfile 端到端读写收口** ✓（缓冲 113 字节＝参照长度 ✓，只在时间戳 4 字节上不同 ✓；
`namelist()`／`read()` 全对 ✓）。**一处真相**：`Instance::decode_with_encoding`／`encode_with_encoding`
被 `bytes.decode`／`str.encode`／`_codecs` 三方共用 ✓。
**如实口径** ✗：`lookup` **还不查 `encodings` 包**（要 namedtuple ✓）、`CodecInfo` 也不是 namedtuple ✓。
**`import dis` 未通** ✗：停在 **`reversed` 只接 list** ✓（task-7 旧账 ✓，与 namedtuple 是两格 ✓）。

**枢纽⑩第三轮落地** ✓：补内建 **`__import__`** ✓（绝对名＋顶层／叶子口径，走 `import_through_bridge`
一处真相 ✓）—— `encodings/__init__.py:100` 那条链上缺的第一格 ✓。**zipfile 读回仍未通** ✗（如实立据）：
`encodings.search_function('cp437')` 返回 `None` ✓，下一格是 `encodings/cp437.py` 的
`getregentry()` 造 `codecs.CodecInfo(...)` 报 `TypeError: object.__new__() takes exactly one argument` ✓
—— 与 `dis.py` 那处**同一格**：**`collections.namedtuple` 的运行期缺口** ✗
（`CodecInfo` 是 namedtuple ✓）⇒ 建议立"namedtuple"枢纽（`dis`／`inspect`／`typing`／`encodings`
的共同前置 ✓）。

**枢纽⑩第二轮落地** ✓：`_struct` 两处**真 bug** ✗ —— ① 带计数标量的**总宽**（`2B4HL2L2H` 一族只加单宽 ✗
⇒ `calcsize('<4s2B4HL2L2H')` 给 17、参照 30 ✓）；② `pack` 用**全局 `cursor`** 当字段内下标 ✗
（第二字段起偏移全错，实测越界 panic ✓）。修后 `zipfile` **写出 113 字节＝参照长度** ✓（此前 65 ✓），
既有 `struct` 抽例全绿 ✓。**读回仍差一格** ✗：`LookupError: unknown encoding: cp437` ✓ ——
`Lib/encodings/cp437.py` 在 `Lib/` 里 ✓，但我们的 `_codecs` 注册表**不查 `encodings` 包** ✓
（`codecs.lookup("cp437")` 实测报 unknown encoding ✓）⇒ 第三轮从这条查 ✓。

**枢纽⑩第一轮落地** ✓：`_io.BytesIO` **真实现** ✓（逐例与参照对：write／read／seek／tell／getvalue／
覆盖写／truncate／close 后报错／readline ✓，并**按参照去掉 `__len__`** —— 空缓冲的假值 bug 正是
`zipfile` 报 `Attempt to write to ZIP archive that was already closed` 的来源 ✓）；
`IOBase` 一族补 `closed`／`__enter__`／`__exit__`／`close`／`flush` ✓；`time.localtime`／`gmtime`
（UTC 口径，Hinnant 历 ✓）。**zipfile 端到端仍差一格** ✗：写阶段已通（缓冲 65 字节 ✓），但读回报
`BadZipFile` ✓ —— 65 字节里**没有中央目录** ⇒ 下一轮查 `ZipFile.close()` 的 `_write_end_record()`
为何没落笔（同墙第 1 轮 ✓，还剩 2 轮 ✓）。

**枢纽⑨第三轮收口** ✓：zipfile 那堵墙的根因是**名字下标 >255 的 `as u8` 截断** ✗
（`Lib/zipfile/__init__.py` **557** 个名字 ✓）—— 三轮证据链：① 探针夹到 `ZipExtFile`／`ZipFile`
之间且属性名随探针位置变 ✓；② `_ZipWriteFile` 整段单独跑通过 ✓、致命 miss 是 `name=os type=module` ✓；
③ 诊断补打接收者（`module=io`）与帧（`frame_module=zipfile`）⇒ 实际在取 **`io.os`** ✗，
再用 300 个全局名的小样本当场复现（`'module' object has no attribute 'g44'` ✓）。
修法：`emit_shifted`（高位到低位补 `EXTENDED_ARG` ✓）＋ **8 处**截断改扩展路径 ✓
（`LOAD_ATTR`×3／`LOAD_GLOBAL`×2／`DELETE_ATTR`／`LOAD_CONST None`）✓。收 **4** 个模块 ✓。
**仍缺** ✗：zipfile 端到端读写要 **`io.BytesIO`** 一族真实现 ✓（`_io` 现为占位 ✓，另行立据 ✓）。

**枢纽⑧剩下的（附根因）** ✗：
- `zipfile` 一族 ⇒ 卡在 **`_struct` 的真实现** ✓（`Lib/zipfile/__init__.py` **模块级**就
  `struct.Struct('<H')` 一族 ✓；我们的 `_struct` 是"名字齐、调用报未接线"的占位 ✓）；
- `http/cookiejar.py` 已同步 ✓（parser 单行 `for` 体上一轮解掉 ✓）⇒ 导入仍缺 **`urllib.request`** ✗；
- `logging`／`concurrent.futures` ⇒ `Lib/textwrap.py:82` 的 **look-around** ✗（引擎级 ⇒ Lead 另立枢纽 ✓）。

**用户已拍板路线 2** ✓：按"**阻塞最多模块的报错签名**"队列抬判据① 分子，目标 **≥ 421/628 ＝ 67%** ✓
（`enum.py` 换回支降为**低优先** ✓，不再主攻 ✓）。**排活口径＝全分母 628** ✓。

**枢纽⑧本笔（threading 第一轮）** ✓：`_thread` 按 `Lib/threading.py` **实际要的**最小面补了四处 ✓
（`itertools.count.__next__` ✓、`_thread._local` ✓、`_excepthook`／`_ExceptHookArgs` ✓、
`_ThreadHandle` 的 `tp_new`（按本对象布局分配 ✓，通用 `plain_new` 会撞 `instance_size 40≠48` ✗）✓），
并同步 `Lib/threading.py`／`Lib/logger`／`Lib/logging/`（3）／`Lib/_threading_local.py` ✓（逐字节一致 ✓）。
**threading 仍未 import 通** ✗ ⇒ 本轮 **＋0** ✓（如实报 ✓，**无回退** ✓）。

**还剩的墙** ✗：① `threading` 执行到 **71 个名字**时
`AttributeError: 'module' object has no attribute 'excepthook'` ✓（`_make_invoke_excepthook()` 读全局
`excepthook` ✓ —— 下一轮按四条探针规矩定位到具体行 ✓）；② `logging`／`concurrent.futures` ⇒
`Lib/textwrap.py:82` 的 `(?<=…)` **look-around** 不在我们的 `regex` 引擎里 ✗（与 `traceback` 同一条 ✓，
枢纽④已立据 ✓）。

**枢纽⑦本笔落地** ✓：parser 的**单行 `for`／`while` 体**（与 `def`／`class` 同口径 ✓）、
**`_opcode` 真模块**（7 个 `has_*` 收编号 ✓、4 个 descs／nb_ops ✓、`stack_effect` ✓，全部转发
`pyawa-core::opcode` 的表 ✓）＋ `Lib/opcode.py` 逐字节同步 ✓ ⇒ `import opcode` ✓。
**同轮真 bug** ✗：自定义内建模块命名空间漏 `__name__` ⇒ `importlib._bootstrap._setup` 遍历
`sys.modules` 时 `AttributeError` ⇒ importlib 一族 7 ＋ `profile` 共 **8** 个模块转红（实测 ✓，已修 ✓）。

**枢纽⑦的两道墙（探明 ✓，已立据 ✗，见「未修 bug」）**：
- **`typing`** ⇒ ① **PEP 695 语法**（`class SupportsAbs[T](Protocol)` ⇒ 现报「`class` 后面要冒号」）；
  ② **`_typing` C 模块**（`Lib/typing.py:32` 裸 import）＝ `Generic`／`TypeVar`／`ParamSpec`／
  `TypeVarTuple`／`ParamSpecArgs/Kwargs`／`TypeAliasType`／`Union`／`NoDefault`／`_idfunc` 十个面。
  本轮同步过 `Lib/typing.py`，**`lib_compile` 直接不过** ✗ ⇒ 按 `cookiejar` 先例先不放 ✓。
- **`inspect`** ⇒ 三块前置，各自还有独立缺口 ✓：`annotationlib`（`type.__dict__['__annotations__']`
  描述符缺失 ✗）、`dis`（`reversed` 只接 list ✗ ＋ `collections.namedtuple` 运行期 ✗）、
  `tokenize`（`builtins.open` 缺失 ✗ ＋ `_tokenize` ✗）。
- `dis`／`tokenize`／`annotationlib`／`inspect` **都过了 `lib_compile`** ✓，但运行期会打断既有导入者
  （`tokenize` ← `linecache`／`traceback`／`importlib._bootstrap_external`；`inspect` ← `ast`／`importlib.metadata`）
  ⇒ **暂不放进来** ✗（文件留在 `target/` ✓，前置一落随即收 ✓）。

**下一手** ✓（Lead 已排定）：⑧ `logging`＋`threading`（`zipfile`／`concurrent.futures`／`http.cookiejar` 在等 ✓）→
**socket 立支评估** → ⑨ `asyncio` 探墙 → ⑩ CJK `_codecs_*` 专支 → ⑪ `multiprocessing` → ⑫ `test.support` 殿后 ✓；
`typing`（PEP 695 ＋ `_typing`）与 `inspect`（annotationlib／dis／tokenize）两块**建议单独成枢纽** ✓
（一落即可收 `pathlib.types` 与 `importlib.resources`／`metadata` 17 下游 ✓）。

**四条探针规矩**（四次踩坑换的 ✓，务必照办）：
① **按文本锚定** ✓（**不用行号** ✗）；
② 标记文本**只用 ASCII** ✓（全角字符会撞我们自己的**词法缺口** ✗ —— 那本身是一处待接线的小缺口 ✓）；
③ **躲开装饰器** ✓（插在 `def` 行**之后** ✓，别插在 `@…` 与 `def` 之间 ✗）；
④ 插进 `for`／`if`／`def` 的**体**里要**＋4 缩进** ✓（同缩进会把体写空 ✗）；且 `for …:` 这类**别匹配到括号内的推导式** ✗。

**探针收尾也要清缓存** ✓：跑完还原后**必须** `rm -rf Lib/__pyawa__` ✓，否则旧的坏产物会被复用 ✗（第 718–720 轮踩到一次 ✓）。

## 仍等你拍

- （暂无 ✓）路线 2 已拍板（目标分子 ≥ 421/628 ＝ 67% ✓）；`enum.py` 换回支保留为低优先 ✓。

## 未修 bug ＋ 判据

- **终结器／`super` 那条 UAF** ✗：**可观测失败已消失** ✓（重压 6×6：修前 27/36 红 ⇒ 修后 0/36 绿 ✓）；
  底层"**字典是否被重复释放**"**仍未证明** ✗（见归档卷第 578 轮 ✓）。
- **判据②** ✓：`re` 的 **13 行**逐字对账**已达成** ✓（见「现在」✓）。
- **判据①** ✗：**264 ÷ 628 ＝ 42.0%**（阈值 67%）—— 路线 2 推进中，见「下一条命令」✓。
- ~~**`threading` 未 import 通**~~ ✓ **枢纽⑧第二轮已修** ✓：那格墙在 `sys` 缺 `excepthook` ＋ `exc_info` ✓
  （`_make_invoke_excepthook()` 读它们 ✗）⇒ 补上后 `import threading` ✓、`_threading_local` ✓（判据① ＋2 ✓）。
  同轮已补：`count.__next__` ✓、`_thread._local` ✓、`_ThreadHandle()` 的 `tp_new` ✓、`_excepthook`／`_ExceptHookArgs` ✓。
- ~~**`_struct` 只有占位**~~ ✓ **枢纽⑨已修** ✓：`Struct`／`pack`／`unpack`／`pack_into`／
  `unpack_from`／`iter_unpack`／`calcsize` 全部真实现 ✓（逐例与参照对 ✓，见「下一条命令」✓）。
- ~~**zipfile 的类体级 attr 缺口**~~ ✓ **枢纽⑨第三轮已修** ✓：根因是**名字下标 >255 被 `as u8`
  截断** ✗（大模块现场：zipfile 557 个名字 ⇒ `LOAD_ATTR` 取到名字表邻居项 ✓）⇒
  `emit_shifted` 补 `EXTENDED_ARG` ✓、8 处截断改扩展路径 ✓ ⇒ zipfile 4 个模块全通 ✓
  （三轮证据链见「下一条命令」✓）。
- ~~**`io.BytesIO` 一族无真实现**~~ ✓ **枢纽⑩第一轮已落 `BytesIO`** ✓（`StringIO` 仍是占位 ✗）。
- ~~**zipfile 读回报 `BadZipFile`**~~ ✓ **枢纽⑩第二轮已修** ✓：真因是 `_struct` 带计数标量**总宽**与
  **逐字段下标**两处 bug ✗（头短写 ⇒ 中央目录读到一半报 BadZipFile ✓）⇒ 现在写出 113 字节对参照 ✓。
- **内建容器子类布局（tuple/list/dict/str/int 子类）未接** ✗（枢纽⑪实测，**已升级为独立枢纽 ①**）：
  `class P(tuple)` 的 `P((1,2))` 返回**普通 tuple**（`tuple_new` 里 `let _ = class;` ✗）、
  `tuple.__new__` 属性通道落到 `object.__new__` ✗ ⇒ `collections.namedtuple` 生成的类第一步就炸 ✓
  （`CodecInfo`／`dis`／`inspect`／`typing` 的共同前置 ✓）。**已用选项 ② 绕开 zipfile 验收** ✓。
- ~~**`reversed` 只接 list**~~ ✓ **task-15 已修** ✓（`__reversed__` 优先＋序列协议＋惰性 ✓）。
- **`Lib/dis.py` 放回会掀引用计数** ✗（task-15 实测）：放回后 `slowcheck` 的
  `file_backed_import_needs_the_fs_domain` 连续两次 `对已释放对象 incref` ✓（单跑该测试通过 ✗、
  不放 `dis.py` 时十项全绿 ✓）⇒ **归因未钉死**（疑并行测试顺序／引用计数交互 ✓）；
  文件在 `target/dis_hold.py` ✓，钉死后再收 ✓（`import dis` 本身已实测能过 ✓）。
- **`_codecs.lookup` 不查 `encodings` 包** ✗（枢纽⑪如实口径）：内建表（四档）已通 ✓，
  其余编码要走已注册 search function ⇒ 仍缺 namedtuple（`CodecInfo`）✓ —— 等 ① 后放开 ✓。
- ~~**`collections.namedtuple` 运行期缺口**~~（并入上面「内建容器子类布局」条 ✓）：`encodings.search_function`
  导 `encodings.cp437` 后调 `getregentry()` 造 `codecs.CodecInfo(...)` ⇒
  `TypeError: object.__new__() takes exactly one argument (the type to instantiate)，实际给了 1 个` ✓
  —— 与 `dis.py` 先前那处**同型** ✓（`Lib/collections/__init__.py` 的 namedtuple ✓）。
  挡住了：`encodings`（⇒ zipfile 读回 ✓）、`dis`、`inspect`、`typing` 一族 ✓。
- **`urllib.request` 缺失** ✗（枢纽⑧实测）：`Lib/http/cookiejar.py` 已同步且过闸门 ✓，导入仍卡在它 ✓。
- **`logging`／`concurrent.futures` 撞 `re` 引擎** ✗（枢纽⑧实测）：两者都经 `traceback` 走到
  `Lib/textwrap.py:82` 的 `(?<=…)` **look-around** ⇒ `ValueError: regex parse error` ✓
  （`textwrap`／`traceback` 同一处缺口 ✓，枢纽④已立据 ✓）。
- **`builtins.dir` 缺失** ✗（枢纽⑧顺带实测）：`dir` 不在内建面里 ✓（`dir(_thread)` 报
  `NameError: name 'dir' is not defined` ✓）—— 单独立据 ✓。
- **假货 `Lib/enum.py`** ✗：`python3 tools/sync_lib.py --check` 的**唯一一处**与上游不一致 ✓（`Lib/` 其余源文件逐字节相同 ✓）。
- **`typing` 两道墙** ✗（枢纽⑦探明，**建议单独成枢纽** ✓）：① **PEP 695 语法**
  （`Lib/typing.py:2913` 的 `class SupportsAbs[T](Protocol):` ⇒ 现报「`class` 后面要冒号」✗，
  同族还有 `def f[T]()`／`type X = …`）；② **`_typing` C 模块**（`Lib/typing.py:32` 是**裸 import** ✗）
  ＝ `Generic`／`TypeVar`／`ParamSpec`／`TypeVarTuple`／`ParamSpecArgs`／`ParamSpecKwargs`／
  `TypeAliasType`／`Union`／`NoDefault`／`_idfunc` 十个面 ✓。**同步过 `Lib/typing.py` 但 `lib_compile` 不过** ✗ ⇒ 先不放 ✓。
- **`inspect` 三块前置（各自还有独立缺口）** ✗（枢纽⑦探明）：
  `annotationlib` ⇒ `type.__dict__['__annotations__']` **描述符缺失** ✗（实测 `KeyError: __annotations__`，
  `Lib/annotationlib.py` 末尾 `_BASE_GET_ANNOTATIONS = type.__dict__["__annotations__"].__get__` ✓）；
  `dis` ⇒ `reversed` 非 list ＋ `collections.namedtuple` 运行期 ✗；
  `tokenize` ⇒ `builtins.open` **缺失** ✗（`import builtins; hasattr(builtins,'open')` 为假 ✓）
  ＋ `_tokenize` C 模块 ✗。这四个文件都过了 `lib_compile` ✓，但会打断既有导入者
  （`tokenize` ← `linecache`／`traceback`／`importlib._bootstrap_external`；`inspect` ← `ast`／`importlib.metadata`）
  ⇒ **暂留在 `target/` 不进 `Lib/`** ✗。
- ~~**parser 缺「单行 `for` 体」**~~ ✓ **枢纽⑦已修** ✓：`for`／`while` 都接了单行体 ✓
  （`for i in range(3): total = total + i` ⇒ 3 ✓、`while n < 3: n = n + 1` ⇒ 3 ✓）；
  `Lib/http/cookiejar.py:1760` 的形态从此可编译 ✓（该文件仍缺 `urllib.request`／`threading` ✓）。
- ~~**ABC 虚拟子类注册不生效**~~ ✓ **枢纽⑥已修** ✓：`isinstance`／`issubclass` 走元类钩子、
  `object.__subclasshook__`、`type.__subclasses__()`、`__mro__`／`__bases__` 每项 retain ✓
  ⇒ `isinstance([1], Sequence)` 为真 ✓、`random.sample` 跑通 ✓（详见枢纽⑥提交正文 ✓）。
- **`reversed` 只接 list** ✗（枢纽⑤实测）：`random.shuffle` 的 `reversed(range(1, n))` 报
  「reversed 目前只接线了 list（tuple／str／range 一族随后补）」✓。
- **`int.to_bytes` 只接 `i64`** ✗（枢纽⑤实测）：`random.randbytes` 的
  `getrandbits(n*8).to_bytes(n, 'little')` 报 `descriptor needs an int` ✓（64 位以上的整数过不去 ✓）。
- **`tempfile.gettempdir()` 运行期** ✗（枢纽⑤实测）：`import tempfile` 已过 ✓，但调 `gettempdir()`
  报 `expected str, bytes or os.PathLike object, not int` ⇒ posix／`os.path` 面还要补 ✓。
- **`_random` 与参照 MT19937 的差异** ✗（枢纽⑤，**已在提交正文如实登记** ✓）：`getstate()` 是 **4** 元组
  （参照 625 ✓）、`seed(None)` 熵源是时钟＋计数（参照 `os.urandom` ✓）、同种子不同序列 ✓；
  **语义**对齐 ✓。
- **编译期 cell／free 槽位不变量** ✗（枢纽④实测）：`Lib/http/server.py` 的
  `class BaseHTTPRequestHandler` 报「MAKE_CELL 要槽 0，而那一格是 Some(Local)」
  （`nlocals=2`、`cellvars=['__classdict__']` ✓）⇒ 该文件因此**没进 `Lib/`** ✓。
- **其余已知未接的小口子** ✗（均在归档卷如实记 ✓，不假装全好 ✗）：
  `functools.lru_cache` **运行期**的 `LOAD_NAME`（函数体里发 `LOAD_NAME` ✗，第 639 轮 ✓）；
  `target/recon/bisect4.py` 变体 B 仍 `StackUnderflow` ✗（第 614 轮 ✓）；
  默认 `repr` 的 **qualname** 那半仍缺 ✗（第 599 轮 ✓）；
  全角字符的**词法缺口** ✗（第 718–720 轮探针规矩 ② ✓）。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` **同笔** ✓；闸门**只看 exit code**（不接管道 ✓）；
- 判据① **5 轮不动**即报停滞 ✓；**禁止空转**：一轮内不能只写台账 ✓；
- 判据① 单次读数 ±1 ⇒ 报"区间" ✓（第 512 轮实测 ✓）。
