# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> 第 579–720 轮的长复盘已**收口归档**进 `docs/rounds/03-m3-progress-03.md` ✓（本笔同笔提交 ✓）。

## 现在

- **HEAD**：`eae7f2c`（`dev`）—— 本笔（**task-20 第 1 轮**：**`type.__annotations__` getset 描述符** ✓
  （类对象／模块对象两条路 ＋ 3.14「没有就建空 dict 并缓存」✓）＋ **`operator.attrgetter`** ✓ ＋
  **`annotationlib` 同步入手** ✓）✓（提交前已核对 `git rev-parse --short HEAD` ✓）
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓ —— **以本笔收口提交为准** ✓；本单页随其后一笔 docs 提交落地 ✓）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，本笔实测 ✓）：**通过 257 ＋ 参照口径 17 ＝ 274 ÷ 628 ⇒ 43.6%** ✗（阈值 67%）
  - 本笔前 **273 ÷ 628 ＝ 43.5%** ⇒ 本笔 **＋1** ✓（`annotationlib` 同步后能 import ✓；**链没通完** ✗ ——
    `import inspect` 现卡**两处新真 bug** ✓：① `'...' % <映射>` 取键时报 `KeyError: __mro__` ✓
    （`runtime.rs` 的 `%` 映射支 ✓）、② 另一处 `str + tuple` ✓（`PYAWA_BINOP_DEBUG=1` 实测 ✓）——
    见下条 ✓，如实报 ✓）
- 判据② **已达成** ✓：`target/recon/reparity.py` 的 **13 行**与参照（本机 `python3` 3.14）逐字一致 ✓
  - 进度指标（**不作判据** ✓，`CM-15`）：`Lib/` 已同步子集 **370 个文件 ⇒ 能 import 257 个 ⇒ 69.5%** ✓

## 下一条命令（Lead 队列：task-14 内建容器子类布局枢纽（tuple/list/dict/str/int 子类）；`logging` 一族已越过 re 引擎 ⇒ 现卡 `tokenize` 前置）

**用户已拍板路线 2** ✓：按"**阻塞最多模块的报错签名**"队列抬判据① 分子，目标 **≥ 421/628 ＝ 67%** ✓
（`enum.py` 换回支降为**低优先** ✓，不再主攻 ✓）。**排活口径＝全分母 628** ✓。

**task-20 第 1 轮落地** ✓（**`type.__annotations__` 描述符 ＋ `attrgetter` ＋ `annotationlib`** ✓）：
- **`type.__annotations__`** ✓（core `instance.rs::install_annotations_descriptor` ✓）：一个
  **getset 描述符**对象 ✓（`getset_descriptor` 类型 ＋ `__get__` native ✓，与 stdlib 的 `_tuplegetter`
  同一手法 ✓）装进 **`type` 类型的命名空间** ✓ ⇒ `type.__dict__["__annotations__"].__get__` 可用 ✓
  （`annotationlib:1141` 正是它 ✓）。**3.14 口径** ✓（`Instance::annotations_of` **一处真相** ✓）：
  **自有就给** ✓、**没有就建空 `dict` 并缓存** ✓ —— **类对象**写命名空间 ✓、**模块对象**写实例字典 ✓
  （两条路都实测 ✓）；`attribute.rs` 侧也走同一处 ✓（`C.__annotations__` ✓）。
- **`operator.attrgetter`** ✓（`operator_module.rs` ✓）：与 `itemgetter` 同一套绑定方法机制 ✓；
  一个 attr ⇒ 单值 ✓、多个 ⇒ 元组 ✓、**点分路径**（`a.b` ✓）逐层 `getattr` ✓ —— `inspect` 的
  `from operator import attrgetter` 不再是 `ImportError` ✓。
- **`annotationlib`** ✓：`Lib/annotationlib.py` 同步 ✓、**import 通过** ✓（判据① ＋1 ✓）。
- **验收** ✓：新增 `crates/pyawa-runtime/tests/annotations_shapes.rs` **2 例**（描述符两条路 ＋ `__get__` ＋
  `get_annotations`（类／模块 ✓）＋ `attrgetter` 单值／多值／property ✓；`import annotationlib` ＋
  `operator.attrgetter` ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓；`quickcheck` ✓、
  `slowcheck` 十项全绿 ✓、0 警告 ✓；判据① **273 ⇒ 274 ÷ 628 ＝ 43.6%（＋1）** ✓。
- **如实范围 ✗／下一轮** ✓：**类体注解声明**（`x: int`）我方编译器还不落 `__annotations__`／`__annotate__` ✓
  （故抽例只钉描述符那半边 ✓）；`import inspect` 现卡**两处新真 bug** ✓：
  ① `%` 映射取键 `KeyError: __mro__` ✓（`runtime.rs` 的 `%` 映射支 ✓ —— 触发点是某个
  `'%(…)' % <映射>` ✓）、② 另一处 `str + tuple` ✓（`PYAWA_BINOP_DEBUG=1` 实测站点 ✓，左值＝
  `__author__[0]` ✓）—— 第 2 轮按序拆 ✓。

**task-19 第 3 轮落地** ✓（**`ascii()` ＋ `_io.TextIOWrapper` 读路径 ＋ 两处编译器真 bug** ✓）：
- **`ascii()`** ✓（`builtins_module.rs` ✓）：口径 ＝ `repr(obj)` **再把非 ASCII 转义** ✓
  （`\xNN`／`\uNNNN`／`\U…` ✓）—— 不可打印那半边 `repr` 已转好 ✓（第 2 轮 ✓）。
- **真 bug ✗①：普通串 ＋ 两枚以上 f-string** ✓（`parser.rs` ✓）：`parse_atom` 的 `Str` 臂只吃
  **一枚**后随 f-string ✗ ⇒ `('Field(' f'name={…!r},' f'type={…!r},' ')')` 的**第二枚**留在原地 ✓
  ⇒ 括号那一组报「括号没有闭合，实际 Some(FStr …)」✗（`Lib/dataclasses.py:314` 正是这个形状 ✓）。
  修法 ✓：把「继续吃后续相邻字面量」抽成 `consume_adjacent_string_parts` **一处真相** ✓（`Str` 臂与
  `FStr` 臂都走它 ✓）、段合并抽成 `merge_adjacent_literal_parts` ✓；纯字面量链仍折成**一个常量** ✓。
- **真 bug ✗②：词法器减法溢出 panic** ✓（`lexer.rs` ✓）：单字符前缀（`f'…'` ⇒ `index＝2` ✓）在求
  `r` 前缀时走 `characters.get(index - 3)` ✗ ⇒ **调试构建直接 panic** ✓（`f"{'a'}"` 的子词法当场炸 ✓，
  `dataclasses` 就是这样撞上的 ✓）⇒ 改 `checked_sub` ✓（语义不变：位次不够＝没有那个字符 ✓）。
- **`_io.TextIOWrapper` 读路径** ✓（`io_module.rs` ✓）：占位类型**构造不出**（`new` 槽拒实参 ✓）⇒
  建**真类** ✓（`__init__(buffer, encoding=None, …)` ✓、`read`／`readline`／`readlines`／
  `__iter__`／`__next__`／`__enter__`／`__exit__`／`close` ✓、`encoding` ✓、`closed` 实例镜像 ✓）；
  解码走 `decode_with_encoding` **一处真相** ✓；**写路径／`newline=`／`buffering` 如实立支** ✗；
  `type(sys.stdout) is _io.TextIOWrapper` 不再为真 ✓（`sys.stdout` 仍是那个占位文本流类型 ✓）。
- **`tokenize.open` 端到端** ✓：`open(name,'rb')` ⇒ `detect_encoding` ⇒ `seek(0)` ⇒
  `TextIOWrapper(buffer, encoding, line_buffering=True)` ⇒ 迭代／`with` ✓ —— 与参照逐行一致 ✓。
- **验收** ✓：新增 `fstring_io_shapes.rs` **3 例**（`ascii` ✓／f-string 相邻链 ＋ panic 回归 ✓／
  `TextIOWrapper` ＋ `tokenize.open` ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓；
  `quickcheck` ✓、`slowcheck` **十项全绿** ✓（330s ✓）、0 警告 ✓；判据① **273 ÷ 628 ＝ 43.5%（＋0）** ✓（如实 ✓）。
- **新墙（如实立支 ✓）**：`_colorize` **不是**一个纯 Python 小模块的单文件同步 ✗ —— 实测它
  `from dataclasses import …` ✓、`dataclasses` ⇒ `inspect`（**未同步** ✓）、`inspect` ⇒ `annotationlib`
  ⇒ **`type.__dict__["__annotations__"]` 取不到**（`KeyError` ✓ —— 我们的 `type` 类型字典里没有那个
  getset 描述符 ✓）⇒ `Lib/dataclasses.py`／`annotationlib.py`／`_colorize.py` 三份**都 import 不过** ✓
  ⇒ 已**撤出** `Lib/` ✓（不把"同步了但不能 import"的文件混进已同步子集 ✓ —— 实测试着带上会把进度指标
  从 69.5% 压到 68.9% ✓）。要收 `traceback`／`logging`／`concurrent.futures`，得先做
  **`type.__annotations__` getset ＋ `annotationlib`＋`inspect`＋`dataclasses` 链** ✓（独立支 ✓）。

**task-19 第 2 轮落地** ✓（**f-string 三段拆分 ＋ `repr` 转义 ＋ `len(str)` 按字符** ✓）：
- **f-string** ✓（`tokenize_module.rs` 的 `FStringScanner` ✓）：一个 `Lexeme::FStr` ⇒ `FSTRING_START`
  （前缀字母 ＋ 开引号 ✓）＋ `FSTRING_MIDDLE` 段 ＋ 字段 token ＋ `FSTRING_END`（闭引号 ✓）。口径全按
  本机 `python3` 3.14 实测 ✓：`{{`／`}}` **只算一个花括号**且**断开 `MIDDLE` 段** ✓
  （`f"a{{b}}c"` ⇒ `'a{'`／`'b}'`／`'c'` 三段 ✓）；字段＝`OP '{'` ＋ **表达式逐 token**（**只把表达式那一段**
  交给同一个 `python_tokens` ✓ —— 带 `!` 的整段丢进去会词法失败 ✗）＋ `OP '='`（调试 `=` ✓）／
  `OP '!'`＋`NAME`（转换 ✓）／`OP ':'` ＋ **格式说明**（`MIDDLE` 段 ＋ 嵌套字段 ✓、转义 ✓、
  **收尾必发一个 `MIDDLE`**（空串也算 ✓）⇒ `f"{x:{w}}"` 的 `MIDDLE ''` (12,12) ✓）＋ `OP '}'` ✓；
  子词法里的**虚拟 token**（`Indent`／`Dedent`／`Newline`／`End` ✓）不进字段 ✓。
- **`repr` 不可打印字符** ✓（`instance.rs::quote_str` ✓）：先前只转 `\\ \n \r \t` ＋ 引号 ✗ ⇒
  `repr('a\x0bb')` 吐原始 VT ✓；现在按参照转 ✓ —— **ASCII 控制字符**（`\x00`／`\x7f` ✓）也走 `\xNN` ✓、
  `< 0x100` ⇒ `\xNN` ✓、`< 0x10000` ⇒ `\uNNNN` ✓（`\u200b`／`\u2028`／`\u3000`／`\uffff` ✓）、
  其余 ⇒ `\U…` ✓；**可打印非 ASCII 原样** ✓（`é`／`中`／`😀` ✓）；含 Cf／Zs（空格除外 ✓）／Zl／Zp／
  Co（私用区 ✓）／非字符 ✓；`ascii=true` 仍把可打印非 ASCII 也转掉 ✓。
- **`len(str)` 按字符** ✓（`instance/containers.rs::length_of` ✓）：先前用 `value().len()` ⇒
  **UTF-8 字节数** ✗（`len('中')` 给 **3** ✓、参照 **1** ✓；`len('é')` 给 2 ✓）⇒ 改 `chars().count()` ✓。
- **验收** ✓：`tokenize_shapes.rs` **7 例**（第 1 轮 5 例 ＋ **f-string 16 抽例** ✓ ＋
  **`repr` 21 抽例** ✓ ＋ **`len`** ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓；
  `quickcheck` ✓、`slowcheck` **十项全绿** ✓（322s ✓ —— `len`／`repr` 是**广面**改动 ✓，
  全套护栏照过 ✓）、0 警告 ✓；判据① **273 ÷ 628 ＝ 43.5%（＋0）** ✓（语义面 ✓，如实报 ✓）。
- **新发现的缺口** ✗：内建 `ascii()` 未接线 ✓（`ascii('é')` ⇒ `NameError` ✓ —— 参照 `'\\xe9'` ✓）；
  **制表符缩进**仍照底层报未接线 ✓。

**task-19 第 1 轮落地** ✓（**`_tokenize` 真词法器** ✓；recon 结论＝「借 core 词法器」这一口径可行 ✓）：
- **一处真相** ✓：新公开 `pyawa_core::python_tokens`（`compile/lexer.rs` ✓）—— **底层就是编译器词法器** ✓
  （`lex` ✓），只把词法单元投影成 `(类别 ＋ 起止 (行,列))` ✓；**文本**由 `_tokenize` 侧按 span 从源码切 ✓
  （编译器的 `Str`／`Int` **已解码** ✗ ⇒ tokenize 要的**源文本**取不出来 ✓）。
- **`_tokenize.TokenizerIter`** ✓（`crates/pyawa-stdlib/src/tokenize_module.rs` ✓）：`__init__` 逐行读
  readline ✓（`str` ✓／`bytes` ＋ `encoding` ✓、`StopIteration` 收尾 ✓）、`__iter__`／`__next__` ✓；
  **参照投影** ✓：类别 ⇒ `token` 数字 ✓、五元组 `(type, string, start, end, line)` ✓、
  物理行层（`COMMENT`／`NL` ✓）、`NEWLINE` **一位宽尾** ✓、`INDENT` 空白文本 ✓、
  `DEDENT` **位置＝触发行缩进宽度 ＋ `line`＝那一行** ✓、多行 token 的 `line`＝**它自己吃掉的物理行** ✓、
  行尾 `COMMENT` **不含 `\n`** ✓、隐式续行的 `NL` ✓、`ENDMARKER` ✓。
- **同轮真 bug ✗**：`str.splitlines` **整条 `keepends` 没读** ✗、还用 Rust `lines()` 把行尾丢掉 ✓
  ⇒ `'a\nb\n'.splitlines(True)` 给 `['a','b']` ✗（参照 `['a\n','b\n']` ✓；`\r\n` 算**一个**界 ✓、
  `keepends` 为真连界一起切 ✓、末尾有界**不**多空项 ✓）。
- **同步** ✓：`Lib/tokenize.py`（上游逐字节 ✓）。
- **验收** ✓：`tokenize_shapes.rs` **4 例**（隐式续行 ✓／运算符・关键字・行尾注释・`DEDENT` 位置 ✓／
  注释・空行・缩进・多行字符串・全角 ✓／`splitlines` 四档 ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓。
- **如实范围 ✗（第 2 轮 ✓）**：**f-string 整段按 `STRING` 出** ✗ —— 参照给
  `FSTRING_START`／`FSTRING_MIDDLE`／`FSTRING_END` ✓（含 `!r`／`:` 格式说明／嵌套 `{}` 与 `{{` 转义 ✓）；
  **制表符缩进**照底层如实报未接线 ✓（编译器词法器不收 `\t` ✓）。
- **判据①** ✓：**＋1**（272 ⇒ **273 ÷ 628 ＝ 43.5%** ✓ —— `import tokenize` 翻绿 ✓）。

**task-14 重定焦落地** ✓（lead 裁决 (i) ✓，owner 续做 ✓）：**类型字典一致性 ＋ tuple 子类三层** ✓
- **真 bug ✗①（根因）**：`builtins_module.rs` 里那条 `enum.py` 兼容做法把**同一个 `object.__new__`**
  写进 int／str／float／**tuple**／bytes／list／dict／set 的命名空间 ✓ ⇒ **盖掉** core 注册的
  `tuple.__new__` ✓（实测 `tuple.__dict__["__new__"] is object.__dict__["__new__"]` 为 **True** ✓、
  `tuple.__new__(P, (3,4))` 落到 `object.__new__` ✓）⇒ 改成**只补自己没有的** ✓
  （`enum.py` 的 `'__new__' in base.__dict__` 照旧成立 ✓）。
- **真 bug ✗②（`tuple_new_native` 两条派发）**：`bound` 是类型对象而 `args[0]` **不是**类型 ⇒ 用 `bound`
  当 `cls` ✓，否则用 `args[0]` ✓ ⇒ 属性通道绑法与构造分派**都认** ✓ ⇒ `tuple.__new__(cls, it)` ✓、`P(...)` ✓。
- **真 bug ✗③（闭包 cell 被丢）**：`classes.rs` 的 `requalified_method` 换 `co_qualname` 时给的是
  **空 cell 表** ✗ ⇒ `type()`／类体造的**每个闭包函数**都"自由变量未绑定" ✓（最小复现：
  `def f(): x=1; def g(): return x; return {'g': g}` ＋ `type('K', (), ns)` ✓）⇒ 照 `defaults` 那条
  **逐 cell `incref` 接过来** ✓。
- **真 bug ✗④（"恰好类型"一族）**：`percent_format`（右操作数只认恰好 tuple／dict ✗ ⇒ `'%r' % self`
  里 `%r` 去 `repr(self)` ⇒ `__repr__` **自递归** ✓）、`iterator_type_for`／`iterable_length`／
  `iterable_item`（tuple／list／str／bytes 的**子类**报 `not iterable` ✗）、`values_equal`
  （`Point(1,2) == (1,2)` 给 **False** ✗）⇒ 全部改**子类也算** ✓（与 `dict` 那条**同一把尺子** ✓）。
- **真 bug ✗⑤（描述符）**：内建族实例的"自有字典"快路把**数据描述符**（`property` ✓）原样交回 ✗
  ⇒ `Point(1,2).x` 给 `<property object>` ✓；改**只对可调用短路** ✓、非可调用落描述符分支 ✓；
  另 `property_descriptor_get` 把 `fget` 当 `bound_self` 交出去 ✗ ⇒ 已绑定的 `fget`
  （namedtuple 回退用的 `property(_itemgetter(index))` ✓）收不到实参 ✓ ⇒ 改走
  `call_value(fget, &[target], &[])` ✓。
- **验收** ✓：新增 `crates/pyawa-runtime/tests/tuple_subclass_shapes.rs` **2 例**（tuple 子类布局／
  `tuple.__new__(cls, it)`／`property` 字段／迭代／`repr`／相等 ✓ ＋ `codecs.lookup('cp437')` 回真
  `CodecInfo` ✓）与参照（本机 `python3` 3.14）**逐字一致** ✓；`quickcheck` ✓、`slowcheck` 十项全绿 ✓、
  0 警告 ✓；判据① **＋0** ✓（如实 ✓）。
- ~~**仍差一格（如实 ✗，下一轮）**：`namedtuple._asdict`／`_make`／`_replace` 还红 ✓ —— 根因是
  **编译器 `cellvars` 顺序与运行期帧的 cell 槽位不一致** ✓（最小复现：`def m(): a,b,c,d,e=1,2,3,4,5;
  def f(): return (a,e)` ⇒ 参照 `(1,5)`／我们 `(4,1)` ✓；矩阵 `a`→4、`e`→1、`(a,b)`→(4,5)、
  `(c,d,e)`→(3,2,1) ✓）⇒ 归**符号表／帧布局**那一层 ✓ 独立成支 ✓。另 `_collections._tuplegetter` 缺 ✓
  ⇒ 字段类型名是 `property` 而非 `_tuplegetter` ✓（值语义一致 ✓）。~~
  ✓ **第 2 轮已收** ✓：**`cellvars` 嫌疑排除** ✓（实测两端**一致** ✓：`co_cellvars=('a','e')` ✓、
  闭包元组槽 `4,5` ✓、内层 `co_freevars=('a','e')` ✓）—— 真根因是**另一个** ✓（见下条 ✓）。

**task-14 第 2 轮落地** ✓（**`AssignTuple` 的等长元组窥孔 ＋ `_tuplegetter`** ✓）：
- **真 bug ✗（根因更正 ✓）**：`Statement::AssignTuple` 的"等长元组窥孔"**不看长度**就发 `SWAP 2` ✗ ——
  而它只在 **2 项**时成立 ✓（CPython 实测：`a,b=1,2` ⇒ `SWAP 2` ✓；`a,b,c=x,y,z` ⇒ `SWAP 3` ✓；
  **4 项及以上** ⇒ `值 ＋ UNPACK_SEQUENCE n` ✓）⇒ `a,b,c,d,e = 1,2,3,4,5` 实测得 `(4,5,3,2,1)` ✗
  （`a,b=1,2` 恰好对 ✓ ⇒ 一直没被发现 ✓）。**影响面** ✓：**任何 ≥4 项的解包** ✓ —— 含 `namedtuple`
  生成的 `_asdict`／`_replace`／`_make` 里那些解包 ✓、也含**外层函数自己的局部** ✓（那条"闭包矩阵"
  其实是解包先歪、再被闭包读出去 ✓）。修法：窥孔只对 **≤3 项** ✓、`SWAP <目标数>` ✓（不再硬写 2 ✓），
  4 项及以上走 `UNPACK_SEQUENCE` ✓。
- **`_collections._tuplegetter`** ✓：`collections_module.rs` 补**只读下标描述符** ✓（`__get__` ⇒
  `obj[index]` ✓，类访问回描述符本身 ✓；`index`／`__doc__` 放实例字典 ✓，不新增载荷类型 ✓）。
- **验收** ✓：`tuple_subclass_shapes.rs` 扩到 **4 例**（新增 `namedtuple` 全套 ✓：`_fields`／`_asdict`／
  `_replace`／`_make`／`repr`／相等／`_tuplegetter` 类型名／`codecs.lookup('cp437')` ✓ ＋ ≥4 项解包 ✓）
  与参照（本机 `python3` 3.14）**逐字一致** ✓；`cargo test -p pyawa-core --test compile`（**逐字节** ✓、
  编译器指令级对拍 ✓）全绿 ✓ —— `SWAP <n>` 改法与参照字节码**同形** ✓；`quickcheck` ✓、
  `slowcheck` 十项全绿 ✓、0 警告 ✓；判据① **＋0** ✓（`namedtuple` 通了但没有模块因此翻绿 ✓，如实报 ✓）。


**task-17 第 1 轮落地** ✓（**引擎枢纽：两档引擎** ✓，用户 2026-10-16 拍板引入 `fancy-regex` ✓）：
- **依赖边**：`crates/pyawa-stdlib/Cargo.toml` 加 `fancy-regex = "0.19.2"` ✓，与 `regex = "1"` **同位** ✓
  （用途与归属**只写该文件注释一处** ✓；网络通 ⇒ `tools/cargo.sh add` ＋ `cargo fetch` 拉齐 ✓）。
- **一处真相** ✓：`enum Engine { Fast, Fancy }` ＋ **唯一**编译入口 `build_engine` ✓ —— `regex` **先试** ✓、
  失败**才**退回溯引擎 ✓（快路径失败**不**直接报 ✗：`\1`／`(?<=…)` 正是要退的那一类 ✓）。
- **同一条产出管线** ✓：`Engine::captures`／`captures_iter` 交回共用的 `GroupSpans`（字节跨度 ✓）⇒
  `span`／`group`／`findall`／`finditer`／`split`／`sub`／`subn`／`_sre.match_raw` 两条引擎**天然同口径** ✓
  （不新增载荷类型 ✓；`REGISTRY` 与 `PatternData.built` 一并改成 `Engine` ✓）。
- **验收** ✓：探针 **21 行**与参照（本机 `python3` 3.14）**逐字一致** ✓（look-behind／look-ahead 正负向 ✓、
  `\1`／`(?P=x)` ✓、`findall`／`sub`／`split`／`finditer` ✓、VERBOSE ✓、bytes 的 `search`／`sub` ✓、
  空匹配 `(?=a)` 的 `findall` ✓）；`_sre_shapes.rs` 既有 **15** 例 ＋ 新增 **1** 例（14 行）⇒ **16 passed** ✓
  ⇒ 既有路径**零回退** ✓；`import textwrap` ✓／`wsgiref.validate` ✓。
- **判据① ＋1** ✓（**272 ÷ 628 ＝ 43.3%** ✓）；`quickcheck` ✓、`slowcheck` 十项全绿 ✓、0 警告 ✓；
  收账两道必检 ✓（逐个导入 ✓ ＋ 无关脚本 `startup ok` ✓）。
- **墙换了一格（如实 ✗）**：`logging`／`concurrent.futures`／`traceback` **仍红** ✗ —— 已越过 look-around ✓，
  现停 **`ModuleNotFoundError: No module named 'tokenize'`** ✓（独立缺口：`builtins.open` ✗ ＋ `_tokenize` ✗，
  见「未修 bug」✓）；验收归 **task-18** ✓。

**task-17 第 2 轮落地** ✓（加固轮 ✓，lead 裁决：这轮过了即**收口** ✓）：
- **真 bug ✗①**：`pos > 0` 时旧实现把 `pos` **之前的前缀一起切掉** ✗ ⇒ `^`／`\b`／look-behind 看错上下文 ✓
  （`(?<=a)b` 对 `"xabxa"` 的 `search(…, 2)` ⇒ 参照 `(2,3)` ✓ 而切片法 `None` ✗；`^b` 的 `match(…, 1)`
  参照 `None` ✓ 而切片法**误报** `(1,2)` ✗）⇒ 改为**只按 `endpos` 截尾** ✓、起点交引擎自己的 pos 接口 ✓
  （`regex::captures_at` ✓／`fancy_regex::captures_from_pos` ✓，`\A` 仍只认串首 ✓）；空匹配改由**共用游标**推进 ✓
  （前进一个字符 ✓、句尾只产出一次 ✓）⇒ `findall`／`finditer`／`split`／`sub` 两档同一条扫描路径 ✓、跨度不再平移 ✓。
- **真 bug ✗②**（core）：`str(object, encoding[, errors])` 的两／三实参形态被**整个忽略** ✗
  （`str(b'abc', 'latin1')` 回 `"b'abc'"`）⇒ 补解码路径（`Instance::decode_with_encoding` 一处真相 ✓）＋
  参照三条 `TypeError` 文案 ✓ —— 这条正是 `Lib/re/_parser.py:235` 的 **bytes tokenizer** ✓：修前 bytes 正则的
  解析器读的是 **`repr(bytes)`** ✗（`rb'(?s)a'` 报 `global flags not at the start of the expression` ✗）。
- **验收** ✓：`target/recon/re_engine_edges.py` **58 行** ✓、`re_engine_probe.py` **24 行** ✓、
  `iso_str2.py`／`iso_flags.py` 全与参照**逐字一致** ✓；新增 `crates/pyawa-runtime/tests/re_engine_shapes.rs`
  （pos/endpos 上下文 ✓、bytes 回退产出 ✓、空匹配前进 ✓ 共 3 例）＋ `text_shapes.rs` 的 `str` 解码 1 例 ✓；
  `_sre_shapes.rs` **16 例零回退** ✓；`quickcheck` ✓、`slowcheck` 十项全绿 ✓、0 警告 ✓；判据① **＋0** ✓。
- **同轮探到的其余缺口（如实 ✗，另立据）**：① `str(bytearray, …)` 仍 `TypeError` ✗、`errors` 非 `strict`
  未实现 ✗（同 `bytes.decode` 口径 ✓）；② `Pattern.sub(repl, s, count=1)` 的**关键字** `count` 被原生派发丢掉 ✗
  （位置实参那条对 ✓）；③ 模块级 `re.sub(pat, repl, s, count)` 的位置实参弃用告警走 `sys._getframe(N)` ✗
  ⇒ 现报「`sys._getframe` 目前只接 depth＝0」✓（见「未修 bug」✓）。

**task-18 第 1 轮 (a) 落地** ✓（前置枢纽第一步 ✓，lead 拍板拆分 ✓）：**`builtins.open` ＋ 文件对象** ✓
- **依赖边** ✓：`crates/pyawa-stdlib/Cargo.toml` 加 `pyawa-capabilities` ✓ —— **唯一**理由是"域的
  `open_flag` 编码"✓（它是能力契约的一部分 ✓，**不是** POSIX 位 ✗；见下面那处真 bug ✓）。
- **落** ✓：`_io.open` 真实现 ✓（**一处真相** ✓：`builtins.open` 指向**同一个** native ✓）＝模式解析
  （`r`／`w`／`a`／`x`／`+`／`b`／`t` ✓，三条报错文案照参照 ✓）＋ `_io.FileIO` 真类 ✓（`read`／`readline`／
  `readlines`／`write`／`seek`／`tell`／`flush`／`close`／`closed`／`readable`／`writable`／`seekable`／
  `isatty`／`fileno`／`__enter__`／`__exit__`／`__iter__`／`__next__` ✓）；读模式在 `open()` 时**整读** ✓、
  写模式**内存改＋`flush`／`close` 落盘** ✓（如实口径 ✓，见 `io_module.rs` 节首 ✓）；core 新增
  `Instance::fs_seek`／`fs_tell` 两个薄包装 ✓（`seek`／`tell` 槽位**早就在** vtable 里 ✓、规格 §9.1 有 ✓）。
- **同轮四处真 bug ✗**（`open` 一用就撞上 ✓）：
  ① **`fs` 句柄号段从 1 起** ✗ ⇒ 进程**头两个**文件拿到的正是 `stdout`／`stderr` 的号 ✓
  （实测 `with open(…, 'w')` 写的数据**跑到终端**、文件是空的 ✓）⇒ 真文件改从 **3** 起 ✓；
  ② **`FOR_ITER` 对非内建迭代器直接报"未接线"** ✗ ⇒ 一切 `__iter__`／`__next__` 用户类与文件对象
  **进不了 `for`** ✓（`advance_iterator` 本就带协议分支 ✓ ⇒ 那格判断**本来就是多余的** ✓）；
  ③ **`os.open` 把 POSIX 的 `O_*` 原样交给能力域** ✗（域位是 `APPEND=8`／`TRUNC=16`／`CREAT=32` ✓）
  ⇒ `O_WRONLY|O_CREAT` 实测"打不开" ✓ ⇒ 在 `posix_module` 加**翻译** ✓（`O_EXCL` 域里没槽 ⇒ `fs_stat` 先探 ✓）；
  ④ `readlines()` 把句尾那次空读也当一行 ✗（参照不 ✓）。
- **验收** ✓：`crates/pyawa-runtime/tests/file_shapes.rs` **2 例**（`open` 全模式／错误文案／编码／`with`／
  迭代 ✓ ＋ `os.open` 标志翻译 ✓）与参照（本机 `python3` 3.14）**逐字一致** ✓；`target/recon/t18_file_accept.py`、
  `t18_osopen.py`、`t18_open.py`、`t18_iter.py` 全对 ✓；`quickcheck` ✓、`slowcheck` 十项全绿 ✓、0 警告 ✓；
  收账两道必检 ✓。判据① **＋0** ✓（`open` 本身不挡任何模块的 import ✓，如实报 ✓）。
- **下一手** ✓：`linecache.updatecache`／`tokenize.open` **本轮做不到** ✗ —— 两者都 `import tokenize` ✓，
  而 `tokenize` 要 `_tokenize` 真词法器 ✓（**task-19** ✓）；本笔已把 `tokenize.open` 需要的那一半
  （`rb` ＋ `readline` ＋ `seek(0)` ＋ 读回 ✓）钉进验收脚本 ✓。

**枢纽⑨本笔落地** ✓：`_struct` **真实现** ✓（`crates/pyawa-stdlib/src/struct_module.rs`）——
`Struct` 类 ＋ 模块级 `pack`／`pack_into`／`unpack`／`unpack_from`／`iter_unpack`／`calcsize`／`_clearcache` ✓、
格式字符与字节序／对齐按参照 ✓、`str` 与 `bytes` 格式串都收 ✓、core 新增
`Instance::write_bytearray_at`（`pack_into` 只写可变的 `bytearray` ✓）。
**逐例与参照对** ✓：`calcsize` 八个形态 ✓、`pack`／`unpack`／`4s`／混合格式／`Struct`／`pack_into`／
三类报错文案（`bad char`／`unpack requires`／`argument out of range`）✓ —— 详见本笔提交正文 ✓。

**task-16 落地** ✓（内存安全支）：
- **偶发 UAF 钉死并修掉** ✗：80 次加压（第 9 次复现 ✓）抓到
  `install → posix_module::build → dict_set → incref_object` ⇒ `os.stat_result` 的类型对象用了
  **进程级静态缓存** ✗（新 `Instance` 复用**已释放**的类型指针 ✓）⇒ 改为**每实例**在类型注册表里查 ✓；
  修后同一加压 **80/80 全过** ✓（与 `Lib/dis.py` 无关 ✓，上一轮归因已更正 ✓）。
- **`dict.__reversed__`** ✓（键列表＋惰性反向 ✓）⇒ `list(reversed({1:2,3:4}))`⇒`[3,1]` ✓。
- **异常 `args` 缓存双重释放** ✗：同一份引用交给缓存与调用方 ⇒ 修法：入缓存后再 `incref` 一份 ✓。
- **收回 `Lib/dis.py`** ✓（`import dis` 通 ✓）；验收：`quickcheck && slowcheck` **连跑 3 次全绿** ✓，
  判据① **271 ÷ 628 ＝ 43.2%（＋1）** ✓。

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
- ~~**`Lib/dis.py` 放回会掀引用计数**~~ ✓ **task-16 已修** ✓：真因不在 `dis.py`，而是
  `posix_module` 的 `os.stat_result` **跨实例进程级静态缓存** ✗（新实例复用已释放的类型指针 ✓）；
  改为每实例查表 ✓ ⇒ 加压 80/80 全过 ✓、`dis.py` 已收回 ✓。
- **容器子类布局（内建容器子类）** ✗（task-14 待办）：`class P(tuple)` 的实例分配与 `__new__` 槽位可达 ✓。
- **`_codecs.lookup` 不查 `encodings` 包** ✗（枢纽⑪如实口径）：内建表（四档）已通 ✓，
  其余编码要走已注册 search function ⇒ 仍缺 namedtuple（`CodecInfo`）✓ —— 等 ① 后放开 ✓。
- ~~**`collections.namedtuple` 运行期缺口**~~（并入上面「内建容器子类布局」条 ✓）：`encodings.search_function`
  导 `encodings.cp437` 后调 `getregentry()` 造 `codecs.CodecInfo(...)` ⇒
  `TypeError: object.__new__() takes exactly one argument (the type to instantiate)，实际给了 1 个` ✓
  —— 与 `dis.py` 先前那处**同型** ✓（`Lib/collections/__init__.py` 的 namedtuple ✓）。
  挡住了：`encodings`（⇒ zipfile 读回 ✓）、`dis`、`inspect`、`typing` 一族 ✓。
- **`urllib.request` 缺失** ✗（枢纽⑧实测）：`Lib/http/cookiejar.py` 已同步且过闸门 ✓，导入仍卡在它 ✓。
- **`logging`／`concurrent.futures` 撞 `re` 引擎** ✗（枢纽⑧实测）⇒ ~~**task-17 第 1 轮已拆掉这格**~~
  ✓ **两档引擎已落** ✓（look-around／反向引用退 `fancy-regex` ✓）：`Lib/textwrap.py:82` 的 `(?<=…)` ✓、
  `import textwrap` ✓、判据① ＋1 ✓ —— 但 `logging`／`concurrent.futures`／`traceback` **仍红** ✗，
  **墙换了一格** ✓：现停 **`ModuleNotFoundError: No module named 'tokenize'`** ✓
  （`traceback` ← `linecache` ← `tokenize` ✓；`tokenize` 是既有独立缺口 ✓：`builtins.open` 缺失 ✗ ＋
  `_tokenize` C 模块 ✗ ⇒ 文件仍在 `target/` 未进 `Lib/` ✓）⇒ 下一步立 `tokenize` 前置枢纽 ✓。
  ✓ **task-19 第 1 轮已拆掉这格** ✓（`Lib/tokenize.py` 同步 ✓ ＋ `_tokenize.TokenizerIter` ✓ ⇒
  `import tokenize` 翻绿 ✓、判据① ＋1 ✓）—— `logging`／`concurrent.futures`／`traceback` 仍红 ✗，
  **墙换了一格** ✓：`traceback` 现差 **`_colorize`** ＋ **`io.TextIOWrapper`** 两块 ✓（如实立支 ✓）。
- ~~**f-string 词法（`FSTRING_START`／`MIDDLE`／`END`）** ✗（task-19 如实口径）~~ ✓ **第 2 轮已收** ✓：
  按参照拆三段 ＋ 字段 token ✓（`{{`／`}}` 只算一个花括号且断 `MIDDLE` 段 ✓；`!r`／`:` 格式说明／
  嵌套 `{}`／**收尾必发（可空）`MIDDLE`** ✓）；16 抽例逐行一致 ✓。
- **制表符缩进** ✗（task-19 如实口径）：编译器词法器对 `\t` 直接如实报"未接线" ✓ ⇒
  `_tokenize` 对制表符缩进的源码也会报错 ✓（参照按 `\t` 展开到 8 的倍数 ✓）。
- ~~**`repr` 不转义控制字符** ✗（task-19 recon 路上撞见，**独立缺口** ✓）~~ ✓ **第 2 轮已收** ✓：
  `quote_str` 按参照转不可打印字符 ✓（ASCII 控制字符也走 `\xNN` ✓、Cf／Zs（空格除外）／Zl／Zp／Co／
  非字符 ✓、可打印非 ASCII 原样 ✓）；21 抽例逐行一致 ✓。
- ~~**内建 `ascii()` 未接线** ✗（task-19 第 2 轮顺手撞见）：`ascii('é')` ⇒ `NameError` ✓（参照 `"'\\xe9'"` ✓）——
  `escape_non_ascii` 在 core 里已有 ✓（`repr` 的 `ascii=true` 档共用 ✓），只差 builtins 里那个名字 ✓。~~
  ✓ **第 3 轮已收** ✓（`builtins_module.rs::ascii_native` ✓：`repr` ＋ 非 ASCII 转义 ✓；抽例对参照 ✓）。
- **`_colorize` 那条链（`traceback` 的最后一道墙）** ✗（task-19 第 3 轮探明 ✓，task-20 第 1 轮**推进一格** ✓）：
  `_colorize` **不是**单文件同步 ✗ —— 链是 `_colorize` ⇒ `dataclasses` ⇒ `inspect` ⇒ `annotationlib` ⇒
  `type.__dict__["__annotations__"]` ✓。**第 1 轮已收**：描述符 ＋ `annotationlib` 同步 ✓（`import
  annotationlib` ✓、判据① ＋1 ✓）＋ `operator.attrgetter` ✓。**现卡** ✗：`import inspect` 撞**两处新真 bug** ✓：
  ① **`%` 映射取键** ⇒ `KeyError: __mro__` ✓（`runtime.rs` 的 `%` 映射支 ✓ 实测；触发点是某个
  `'%(…)' % <映射>` 站点 ✓）、② **一处 `str + tuple`** ✓（`PYAWA_BINOP_DEBUG=1` 站点实测 ✓、
  左值＝ `__author__[0]` ✓）。⇒ 第 2 轮按序拆这两格 ✓，再同步 `inspect`／`dataclasses`／`_colorize` ✓。
  （`import traceback`／`logging`／`concurrent.futures` 三族仍红 ✓。）
- **类体注解声明未落 `__annotations__`／`__annotate__`** ✗（task-20 第 1 轮实测）：`class C: x: int` 之后
  `C.__annotations__` 参照给 `{'x': <class 'int'>}` ✓、我们给 `{}` ✓（描述符的"建空缓存"半边对 ✓，
  **编译器没把注解写进去** ✓）。第 2／3 轮视情况补 ✓。
- **`sys.modules["__main__"]` 缺** ✗（task-20 第 1 轮顺手撞见）：`KeyError: __main__` ✓
  （脚本模块没登记进 `sys.modules` ✓；`sys.modules["sys"]` 有 ✓）。
- **`_io.TextIOWrapper` 只有读路径** ✗（task-19 第 3 轮如实口径）：`write`／`flush`／`newline=` 的通用换行／
  `buffering` 语义**未接** ✓（`tokenize.open` 那族只读 ✓）；另 `type(sys.stdout) is _io.TextIOWrapper`
  不再为真 ✓（`sys.stdout` 仍是占位文本流类型 ✓）。
- **`len(str)` 曾数 UTF-8 字节** ✗ ⇒ ✓ **第 2 轮已收** ✓（`length_of` 改 `chars().count()` ✓；
  `len('中')` 3 ⇒ **1** ✓）—— 顺带把 `len` 抽例并进 `tokenize_shapes.rs` ✓。

- **内建容器子类支挂起** ✗ ~~（前任交接档案 ✓ `target/tuple_subclass_round3.diff` ✓ **6289 字节** ✓）：
  **布局层 ✓／属性解析层 ✓／CALL 转派层 ✗** —— 定位针 `PYAWA_TUPLE_NEW_DEBUG` ✓，**剩余 5–15 行** ✓；
  `namedtuple`／`CodecInfo`／`enum` 三族都压在它上面 ✓（与下面「内建容器子类布局」**同一格** ✓）。~~
  ✓ **task-14 两轮已收** ✓：布局 ✓／`tuple.__new__(cls, it)` ✓／`property` 字段 ✓／迭代 ✓／相等 ✓／
  `repr` ✓（第 1 轮五处真 bug ✓）＋ **`namedtuple` 全套** ✓／`_tuplegetter` ✓／`≥4 项解包` ✓（第 2 轮 ✓）。
- ~~**编译器 `cellvars` 顺序与帧的 cell 槽位不一致** ✗（task-14 第 1 轮推测）~~ ✓ **第 2 轮实测排除** ✓：
  两端**一致** ✓（`co_cellvars=('a','e')` ✓、闭包元组槽 `4,5` ✓、内层 `co_freevars=('a','e')` ✓）；
  真根因是 **`AssignTuple` 的等长元组窥孔不看长度就发 `SWAP 2`** ✗（≥4 项全歪 ✓）⇒ **已修** ✓
  （窥孔只对 ≤3 项 ✓、`SWAP <目标数>` ✓、4 项及以上走 `UNPACK_SEQUENCE` ✓ —— 与参照字节码同形 ✓，
  `--test compile` 指令级对拍全绿 ✓）。教训 ✓：那三行"矩阵"是**解包先歪**再被闭包读出来的 ✓，
  不是 cell 槽位错 ✓。
- ~~**`_collections._tuplegetter` 缺**~~ ✓ **第 2 轮已补** ✓：`collections_module.rs` 落了**只读下标描述符** ✓
  （`__get__` ⇒ `obj[index]` ✓、类访问回描述符本身 ✓；`index`／`__doc__` 放实例字典 ✓ 不新增载荷类型 ✓）
  ⇒ `type(Point.__dict__['x']).__name__` 与参照一致 ✓（`_tuplegetter` ✓）。
- **`Pattern` 方法的关键字实参被原生派发丢掉** ✗（task-17 第 2 轮实测）：`p.sub(repl, s, count=1)` 的
  `count` 被忽略 ✗（三处都不限 ⇒ 与参照 `b-aaa` 不符 ✓），而**位置实参**那条**对** ✓
  （`p.sub(repl, s, 1)` ⇒ `b-aaa` ✓、`p.subn(…, 1)` ⇒ `('-bbc', 1)` ✓）⇒ 缺口在**关键字绑定**那一层 ✓。
- **模块级 `re.sub(…, count)` 的位置实参弃用告警撞 `sys._getframe`** ✗（task-17 第 2 轮实测）：
  `re.sub(pat, repl, s, 1)` 参照发 `DeprecationWarning` ✓，我们走到
  「指令 0 的这个形态尚未接线：`sys._getframe` 目前只接 depth＝0」✗（`warnings` 取栈层要 f_back 链 ✓）。
- **`str(bytearray, enc)`／非 strict `errors`** ✗（task-17 第 2 轮如实口径）：`str()` 解码路径只收
  `bytes` ✓、`errors` 只认 `strict` 的报错面 ✓（与 `bytes.decode` **同一处真相** ✓）。
- **异常类的子类拿不到"异常布局"** ✗（task-18 实测，独立缺口）：`class X(OSError): pass`（以及
  `type.__new__` 同一条路 ✓）造出的子类 `instance_size` 仍是**普通对象**（56 ✓）⇒ 一 `raise` 就撞
  `adopt` 的 debug 断言 `128 vs 56`（`ExceptionObject` 的 Rust 布局是 128 ✓）；连 `str(MyErr("x"))`
  都给默认 repr ✓。**影响**：`io.UnsupportedOperation` 只能**别名到 `OSError`** ✗ ⇒
  `open(…, 'r').write(…)` 的**类名**对不上参照 ✓（**消息**照参照 ✓：`not writable`／`write` ✓）。
- **`linecache.updatecache`／`tokenize.open` 仍不可用** ✗（task-18 第 1 轮收口口径）：两者都
  `import tokenize` ✓（`updatecache` 拿不到就 `return []` ✓）；`tokenize` 要 `_tokenize.TokenizerIter`
  （真词法器 ✓，3.14 已无纯 Python 回退 ✓）⇒ 归 **task-19** ✓；本笔已把 `tokenize.open` 的下半截
  （`open(f,'rb')` ＋ `readline` ＋ `seek(0)` ＋ 读回 ✓）钉进 `file_shapes.rs` ✓。
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
