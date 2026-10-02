# SPEC — C 模块（C 实现层）

> 规范性文件 · 状态：v0
> 读者：实现 `pyawa-stdlib` 的人
> **冲突时的优先级**：`REQUIREMENTS.md`（决策）> `SPEC-*.md`（执行细则）> `DESIGN.md`（架构论证）
> 依赖：`SPEC-capabilities.md`（`CP-`，能力域与提号规则）、`SPEC-bytecode.md` §2（`_opcode` 一类）、
> `SPEC-object-model.md`（`OM-`，记账与载荷）

规范用语：**必须** / **禁止** / **建议** / **可选**。硬约束编号 **`CM-n`**，测试编号 **`T-CM-n`**。

---

## 1. 范围与依赖

**规定**：113 个 C 实现层模块的**分类、实现顺序依据、通用契约**、`errno` → 异常的映射归属、
"未实现"的形态、Unicode 子项目的排期边界、以及能力槽位的提号规则。

**不规定**：

| 内容 | 归属 |
|---|---|
| 能力域接口的**形状**与新槽位的定义 | `SPEC-capabilities.md`（`CP-`）——本文件只能**提号**（`CP-` §12） |
| `.pyac`／模式／import 钩子 | `SPEC-imports-and-modes.md`（`IM-`） |
| 指令表与 `_opcode` 的**内容** | `SPEC-bytecode.md` §2 |
| Unicode 版本与数据表来源 | `§13-7`（**未定**） |
| C-API／ABI | **不做**（`REQUIREMENTS.md` C 兼容范围） |

---

## 2. 上游硬契约（实测清单，**非设计可改**）

本节的数字是「逐字同步 `Lib/`」的直接后果：**C 模块的 Python 层行为由纯 Python 代码的用法定义**。

| 指标 | 实测值 | 出处 |
|---|---|---|
| 纯 Python stdlib | **557 文件 / 288,254 行** | `DESIGN.md` §9 |
| C 实现层模块 | **113**（64 内建 ＋ 49 `lib-dynload`） | 同上 |
| 依赖至少一个 C 模块的纯 Python 文件 | **304（54%）** | 同上 |
| 实现前 5 个模块可解锁 | **67%** 的 `Lib/` 可 import | 同上 |
| `encodings` | **32,612 行**（stdlib 最大一块） | 同上 |

按依赖顺序的前 10 个（括号内为被纯 Python 文件引用的次数）：
`sys`(191)、`itertools`(47)、`time`(46)、`errno`(29)、`builtins`(26)、
`_multibytecodec`(24)、`binascii`(12)、`math`(11)、`pwd`(8)、`atexit`(8)。

---

## 3. 术语

- **C 实现层模块**：CPython 里用 C 实现、在 Python 层可见的那些模块（`_io`／`posix`／`_sre`…）。
- **能力域模块**：需要外部世界权威的模块（`posix`／`_io`／`_socket`…）——I/O **必须**经能力层。
- **纯计算模块**：不碰外部世界的模块（`_sre`／`_struct`／`_json`／`math`…）。
- **未提供**：整个模块不存在。**未实现**：模块在，但某能力槽位不存在（`CP-5`）。

---

## 4. 分类

- **CM-1** 每个模块**必须**归入下列三类之一，且分类决定它受哪条约束：

  | 类别 | 例子 | 关键约束 |
  |---|---|---|
  | **核心 VM 必需** | `sys`、`builtins`、`_imp`、`marshal`、`_opcode`／`_opcode_metadata` | 属 VM 的启动路径；`_opcode` 一类另受 `SPEC-bytecode.md` §2 约束 |
  | **能力域模块** | `posix`、`_io`、`_socket`、`select`、`time`、`_thread`、`mmap` | **必须**走能力层（`CM-8`） |
  | **纯计算** | `_sre`、`_struct`、`_json`、`_csv`、`math`、`binascii`、`_decimal` | 不碰外部世界 |

- **CM-2** 分类**必须**记录在每个模块的契约里；一个模块若既需能力又是纯计算，**必须**拆开——能力部分走 `CP-`，计算部分留下。
- **CM-3** 分类**禁止**由"实现难度"决定；难易不改变它属于哪一类。

---

## 5. 通用契约（每个模块都必须满足）

- **CM-4** 契约定**从 Python 看到的 API 与语义**；**禁止**以 C-API 或 ABI 表述（`DESIGN.md` §9 第二类）。
- **CM-5** **错误映射**：机器错误 → 对应的 Python 异常，**映射表归本文件**（`CP-33` 移交）。
  映射**必须**使纯 Python 层能照常 `except OSError`／`except FileNotFoundError`。
- **CM-6** **"未提供" vs "未实现"**（`CP-34` 移交，形态由本条定）：
  - 整个**模块未提供** ⇒ **必须**抛 `ImportError`（`import` 语义要求如此）
  - 模块在、**某槽位未实现** ⇒ 调用时抛 **`NotImplementedError`**（Pyawa 特有情形；
    **禁止**复用 `OSError` 家族——那会与"已实现但拒绝"混淆，违反 `DESIGN.md` §2）
- **CM-7** 上述两者的消息**必须**可区分，且**必须**点明缺的是哪个模块或哪个槽位。
- **CM-8** 需外部世界权威的模块**必须**经能力层调用；**禁止**直接依赖 libc／OS
  （`DESIGN.md` §3 不变量 1；静态检查见 `CX-4`）。
- **CM-9** 内存记账**必须**由 VM 侧按实例进行（`OM-3`）；模块**禁止**自行维护全局预算。
- **CM-10** 需要**新的能力槽位**时，**必须**回到 `SPEC-capabilities.md` 领号
  （`CP-` §12：**禁止**在别处定义能力函数）。
- **CM-11** 模块的 Rust 实现**禁止**把 OS 句柄泄漏到 Python 层——`fd` 一类**必须**是
  `int`，即实例能力表的下标（`CP-14`／`CP-31`）。
- **CM-25** **格式化迷你语言**（`FORMAT_WITH_SPEC` 的目标）**以参照实现为 oracle**：
  - **禁止**用"本规格未列出该 spec"为由跳过——与 `BC-59` 同一精神
  - 尚未实现的 spec 命中时**必须如实报未实现**（`CM-6`），**禁止**静默给出不同结果
  - 完成判据 ＝ **受测语料里的 spec 全过**；语料**必须**随覆盖增长
  - `'n'` 等**依赖 locale**（或其它实现定义行为）的 spec，与参照实现的差异**必须**记入
    差异清单（`MS-19`），**禁止**沉默——但**不得**把它当作"可以不实现"的借口
- **CM-26** **`print` 的输出通道**：`print` **不是** I/O 本身——它写的是 `sys.stdout`，
  而真正的 I/O 是那个 `_io` 对象，**归 `fs` 域**（`DESIGN.md` §7.2 的域表已把 `_io` 归 `fs`）。
  - **禁止**为输出新增能力域；**禁止**照"平台常量注入"的先例注入 output sink——
    常量是**只读数据**、不是 I/O，而 sink 会造出**第二条通往同一个 `fd` 的路径**（两个真相），
    并绕过实例能力表：`fd` 是能力表下标（`CP-31`），宿主将**无法按实例重定向输出**
  - 因此 **`print` 排在 `sys` ＋ `_io` 之后**，**不属于** `PLAN-milestones.md` §9.4 第 4 条
    所说的"纯计算面"（`builtins` 的纯计算面是 `len`／`abs`／`min`／`max`／`sorted` 一类）

---

### 5.1 `errno` → 异常映射表（`CM-5` 的落地）

映射**按 errno 名字**定义（**不是**按数字：数字随宿主平台变化，且存在
`EAGAIN == EWOULDBLOCK` 这类别名，必须一并处理）。下表由**本机参照实现探测导出**（`CM-19`）：

| 异常类（均为 `OSError` 子类） | errno 名 |
|---|---|
| `BlockingIOError` | `EAGAIN`／`EWOULDBLOCK`／`EALREADY`／`EINPROGRESS` |
| `BrokenPipeError` | `EPIPE`／`ESHUTDOWN` |
| `ChildProcessError` | `ECHILD` |
| `ConnectionAbortedError` | `ECONNABORTED` |
| `ConnectionRefusedError` | `ECONNREFUSED` |
| `ConnectionResetError` | `ECONNRESET` |
| `FileExistsError` | `EEXIST` |
| `FileNotFoundError` | `ENOENT` |
| `InterruptedError` | `EINTR` |
| `IsADirectoryError` | `EISDIR` |
| `NotADirectoryError` | `ENOTDIR` |
| `PermissionError` | `EACCES`／`EPERM` |
| `ProcessLookupError` | `ESRCH` |
| `TimeoutError` | `ETIMEDOUT` |
| **`OSError`（兜底）** | **其余全部** |

- **CM-19** 该表**必须**由**本机参照实现探测导出**（导出脚本入库），**禁止**凭记忆手写；
  表外的 errno 一律落 `OSError`。
- **CM-20** `errno` 模块**必须**暴露**宿主平台**的 errno 数字（与 CPython 同源），
  而映射**按名字**匹配——**禁止**把某个平台的数字硬编码进映射。
- **CM-21** 非 `OSError` 家族的错误（如 `_sre`／`_struct` 的解析失败）**必须**映射到该模块在
  CPython 里的**原生异常类型**（`re.error`、`struct.error`…），**禁止**一律套 `OSError`。
- 该表与 `errno.errorcode` 的**权威形态**是生成物 `crates/pyawa-stdlib/src/errno_map.rs`
  （脚本 `tools/gen_errno.py`）；上表是供人读的形式，二者不一致时以生成物为准。

### 5.2 逐模块合约（分批补，`CM-14`／`PLAN-milestones.md` §9.4 条件②）

本节按 `CM-4` 的形式（**从 Python 看到的 API 与语义**）逐个模块写；每落地一个模块补一段，
这也是 §12 那条缺口的**分批补法**。

#### 5.2.1 `errno`

- **能 import**：`errno` 始终可 import（`CM-6`：模块**未提供**才抛 `ImportError`；
  宿主没注入平台常量时模块仍在，只是没有常量）
- **属性**：宿主平台的全部 `E*` 大写常量（整数，`CM-20`：与 CPython 同源、数字随平台）、
  `errorcode`（`dict`，**整数键** → 规范名；`EAGAIN == EWOULDBLOCK` 这类**别名只留一个名字**）、
  `__name__`（`"errno"`）、`__doc__`
- **没有** `__all__`（参照实现也没有；`errno` 的公开面就是 `dir()` 里那些常量 ＋ `errorcode`）
- **语义**：常量是**只读**的整数；模块**不持有**任何平台状态（`CM-8`：不直连 libc／OS；
  常量由 `pyawa-runtime` 启动时注入，来源见 `DESIGN.md` §9 第 20 条）
- **错误映射**：`errno` 名字 → `OSError` 子类按 §5.1；数字先经 `errorcode` 翻成名字再查表
  （`CM-20`：**禁止**把平台数字写进映射）
- 落地：`crates/pyawa-stdlib/src/errno_module.rs`（实现）＋ `crates/pyawa-stdlib/src/errno_map.rs`
  （生成物）＋ `crates/pyawa-runtime/src/platform_errno.rs`（生成物，启动时注入）

#### 5.2.2 `builtins`（**纯计算面**）

- **能 import**：`builtins` 始终可 import（它是解释器自带的名字空间）
- **已落地的函数**（本层只做**不需要能力域、也不需要输出通道**的）：
  `abs`、`all`、`any`、`bin`、`callable`、`chr`、`hex`、`isinstance`、`issubclass`、`len`、
  `max`、`min`、`oct`、`ord`、`repr`、`sorted`、`sum`
  ＋ **`__build_class__`**（由核心在引导期建好，`OM-14`）
- **语义口径**（期望值全部取自参照实现，见 `crates/pyawa-stdlib/tests/builtins.rs`）：
  - `abs(True)` ⇒ `1`，且结果是 **`int` 不是 `bool`**；`abs("x")` ⇒
    `bad operand type for abs(): 'str'`
  - `len(x)` 收 `str`／`list`／`tuple`／`dict`／`set`；`len(5)` ⇒ `object of type 'int' has no len()`
  - `ord` 按**字符数**判长度（`ord("ab")` ⇒ `ord() expected a character, but string of length 2 found`）；
    `ord(s)` 给码点、`chr(i)` 给单字符，`chr` 越界 ⇒ `ValueError: chr() arg not in range(0x110000)`
  - `bin`／`oct`／`hex` 走整数（`bin(True)` ⇒ `'0b1'`、`bin(-5)` ⇒ `'-0b101'`），
    非整数 ⇒ `'float' object cannot be interpreted as an integer`
  - `callable(x)` 的判据是**核心那一处** `Instance::is_callable`（`OM-11`）
  - `isinstance`／`issubclass` 的第二个参数可以是类型或**类型的元组**；
    `isinstance(1, 5)` ⇒ `isinstance() arg 2 must be a type, a tuple of types, or a union`；
    `issubclass(1, int)` ⇒ `issubclass() arg 1 must be a class`
  - `min`／`max`：**单实参**要可迭代（`min(1)` ⇒ `'int' object is not iterable`）、
    **多实参**是候选本身；没有实参 ⇒ `min expected at least 1 argument, got 0`；
    空可迭代 ⇒ `ValueError: min() iterable argument is empty`（`max` 同形）；
    `default=` 只在空的时候生效（`min([1, 2], default=9)` ⇒ `1`）
  - `sorted(iterable, /, *, key=None, reverse=False)`：**新列表**；`sorted()` ⇒
    `sorted expected 1 argument, got 0`；`reverse=True` 是**排完再反转**（等值元素保持原序）；
    `key=` 每个元素只算**一次**
  - 序比较走核心那一处 `Instance::order_of`：**数值塔**（`int`／`bool`／`float` 混着比）与
    两个 `str`（字典序）。比不了就报参照实现那条
    `TypeError: '<' not supported between instances of 'str' and 'int'`
    （**注意**：消息里"正在比的那个"在前）；迭代走 `Instance::iterable_items`：
    `list`／`tuple`／`str`（逐字符）／`dict`（逐**键**）／`set`
  - 这三条有**能力边界**（如实记）：`key=` 只能是本层认得的可调用（原生／函数／类型）；
    迭代器对象（生成器等）与自定义类的 `__lt__` **还没接**——那要等 `§10` 的迭代器族与
    `OM-11` 的 `richcompare` 槽位
  - `sum(iterable, /, start=0)`：数值塔内累加（`sum([True, True])` ⇒ `2`、`sum([1.5, 1.5])` ⇒ `3.0`）；
    `sum()` ⇒ `sum() takes at least 1 positional argument (0 given)`；`sum(5)` ⇒
    `'int' object is not iterable`；累加不了 ⇒
    `unsupported operand type(s) for +: 'int' and 'str'`（**累加器**的在前）
  - `all`／`any`：空可迭代 ⇒ `True`／`False`；真值走核心那一处 `Instance::truth_of`
    （`None`／假／数值零／空串／空容器为假，其余为真）；`all()` ⇒
    `all() takes exactly one argument (0 given)`；`all(5)` ⇒ `'int' object is not iterable`
- **未落地、且不是"忘了"**：
  - **`print` 与任何需要输出通道的内建**：九域里**没有"输出"域**，
    `DESIGN.md` §9 第 20 条的先例是"由 `pyawa-runtime` 启动时注入"。**口径未裁**（见下）
  - 需要能力域的（`open`、`input`、`exec`／`compile`…）：等 `P3-14`
  - 需要**迭代器对象**（不是容器）／富比较／哈希协议的（`map`、`filter`、`zip`、`enumerate`、
    `hash`、`id`…）：等 `§10` 的迭代器族与 `OM-11` 的 `richcompare`／`hash` 槽位接线
    （`min`／`max`／`sorted`／`sum`／`all`／`any` 已按上一段的边界落地）
- **待裁（不自行决定）**：`print` 的输出通道走哪儿——见 `README.md` 的"待裁"一节
- 落地：`crates/pyawa-stdlib/src/builtins_module.rs`
- **对拍夹具**：`tools/gen_builtins_fixture.py` ⇒ `crates/pyawa-stdlib/tests/fixtures/builtins.rs`
  （40 个用例；那份夹具是**生成的 Rust 源码**而不是 JSON——消费方 crate 里没有 JSON 解析器，
  生成源码省掉解析，也省掉一份重复的解析器）

---

#### 5.2.3 `sys`（先写**不依赖能力域**的部分）

- **能 import**：`sys` 始终可 import（解释器自带）
- **本段范围**：只写"不依赖能力域、输出通道与 import 机制"就已能确定的那部分。以下**不在本段**，
  随各自前置补齐（`CM-14` 的分批）：
  - `stdout`／`stderr`／`__stdout__` 一族（归 `_io` ⇒ **`fs` 域**，`CM-26`）
  - `executable`／`prefix`／`base_prefix`／`exec_prefix`／`platlibdir` 一族（要真机路径 ⇒
    能力层 `IM-15`／`CP-21`）
  - `meta_path`／`path_hooks`／`path_importer_cache`（要 importlib，`IM-30`…`IM-32`）
  - `float_info`／`int_info`／`hash_info`／`stdlib_module_names`／`builtin_module_names`
    （绑定本层尚未定的实现参数——哈希布局、整数表示——或要模块系统）
- **身份的硬约束**（`CX-13`，`DESIGN.md` §9 的载荷决策）：
  - **`implementation.name` 必须报 `pyawa`**——谎报 `cpython` 会让库去加载**不存在**的 C 扩展，
    而库自带的纯 Python 回退路径才是"生态可用"能成立的原因
  - `implementation.cache_tag` 用自己的值：本层取 **`pyawa-<指令集版本>`**（`BC-29`／`BC-40`
    的常量）；**禁止**冒用 `cpython-3xx`
  - `implementation.version` 是**本实现自己的**版本元组（工作区版本，现为 `0.0.0`），
    不是语言版本
- **语言版本 vs 实现版本**（两者**必须**分开报，`DESIGN.md` §9 的"实现观测面"）：
  - `version_info` 报**本实现所实现的语言级别** ⇒ `(3, 14, 4, 'final', 0)`（对拍参照是 3.14.4）：
    库用 `sys.version_info >= (3, 11)` 一类做**特性检测**，报实现自己的版本号会让它们走错分支
    （PyPy 等替代实现的惯例同此）
  - `hexversion` 是 `version_info` 的整数编码（与参照同式：主 `<<24`｜次 `<<16`｜微 `<<8`｜
    发布级 `<<4`｜序号）
  - `version` 是**构建串**，**必须**含 `pyawa`（**禁止**伪装成 CPython 的构建串）
- **`argv`**：启动参数列表。REPL／嵌入式默认 `[""]`；`-c` 入口给 `["-c"]`（照参照的可见形态）
- **`path`**：**由 `site.py` 构建**（`IM-24`）——本层只暴露这个列表，**禁止**另立路径逻辑
- **`modules`**：import 系统的模块表（`IM-6` 一族的落点）；import 未接之前只保证这个键存在
- **与实现无关的常量**（值由探测参照导出、逐项对拍，见 `crates/pyawa-stdlib/tests/sys.rs`）：
  - `maxunicode` ＝ `0x10FFFF`
  - `byteorder` ＝ **宿主**字节序（本机 `'little'`；由目标端序决定，与参照同源）
  - `maxsize` ＝ `isize::MAX`
- **`getrefcount`**（`OM-22`）：返回**真实计数加一**（借用参数那一份），与参照的可见语义一致；
  单例与 interned 字符串的具体数字**不进对照**（`MS-18` 与差异清单的口径）
- **已落地**（`crates/pyawa-stdlib/src/sys_module.rs`）：`argv`／`path`／`modules`／`version`／
  `version_info`／`hexversion`／`maxsize`／`maxunicode`／`byteorder`／`implementation`
  （点号可访问的命名空间，用核心的安全面搭：`new_attribute_type` ＋ `set_type_attribute`）／
  **`getrefcount`**（`OM-22`：真实计数加一；三种用法的消息逐条实测）；`__name__`／`__doc__`。
  **未落地**：上面"不在本段"的各项
- **验收**：`crates/pyawa-stdlib/tests/sys.rs`——身份三条（`name`／`cache_tag`／`version`）、
  语言版本两条（`version_info`／`hexversion` 与参照一致）、常量三条对拍、`argv`／`path`／
  `modules` 的形态；期望值由 `tools/gen_sys_fixture.py` **探测参照实现**导出
  （`crates/pyawa-stdlib/tests/fixtures/sys.rs`，生成物、禁止手改）

#### 5.2.4 `_imp`

`CM-14` 顺序里紧接着的那一个（`SPEC-c-modules.md` §6 已写明它的硬义务）。

- **能 import**：`_imp` 是解释器自带 —— `CM-6` 的"模块未提供才抛 `ImportError`"不适用
- **`pyc_magic_number_token`**（§6 的硬义务）：**必须**是**整数**；`_bootstrap_external.py` 用它算
  `MAGIC_NUMBER`（取**低 16 位**）。**其值由 Pyawa 自定**（§6 原文）——参照实现是 168627755
  （`0xa0d0e2b`），本层**不冒用**：取 `.pyac` 自己那套的标识（`BC-29` 的指令集版本参与进来亦可），
  只要满足"是整数、低 16 位稳定"即可
- **`is_builtin(name)`**：三态 —— 参照实现里 `-1` ＝ 是内建模块、`0` ＝ 不是、
  `1` ＝ "本应内建却不在表里"（`sys` ⇒ `-1`、未知 ⇒ `0`，实测）。
  本层**并入模块表之前一律 `0`**（还没有可导入的模块 ⇒ `CM-6` 的"未提供"口径）；
  等 `P3-12` 的模块表落地后再照表给 `-1`／`1`
- **本段未落地**（等各自前置，**不伪造**）：
  - `acquire_lock`／`release_lock`／`lock_held`：导入锁。本层每实例单线程，**没有**真的跨调用锁 ⇒
    若做成空操作会让 `lock_held()` 与参照的**可观测**行为不一致（参照在 `acquire_lock()` 之后为真）
    ⇒ **不实现**，留给 `P3-12` 的 import 流程一起定
  - `source_hash`／`check_hash_based_pycs`：绑定 `.pyc` 的源码哈希方案；`IM-17` 决定
    `__pycache__` 一律忽略 ⇒ 本层不做
  - `extension_suffixes`／`create_dynamic`／`create_builtin`／`exec_builtin`／`exec_dynamic`：
    扩展模块与 import 机制（`P3-12`）；`extension_suffixes` 还要平台（能力层）
  - `find_frozen`／`get_frozen_object`／`init_frozen`／`is_frozen`／`is_frozen_package`／
    `_frozen_module_names`：冻结表（`IM-27`／`IM-33`／`IM-34` 的落点）
  - `_fix_co_filename`：要 code 对象改写（本层 code 不可变，`code_with_qualname` 那种"造副本"可复用）
- **验收**：`crates/pyawa-stdlib/tests/imp.rs`——token 是整数且低 16 位稳定、与参照**必须不同**
  （自定值）；`is_builtin` 在**并入模块表之前**一律 `0`（含 `sys`／`_imp` 自己）；
  参照的三态取值由探测夹具记下（`tools/gen_imp_fixture.py`）

#### 5.2.5 `_opcode` 与 `_opcode_metadata`

`SPEC-bytecode.md` §2.1／§2.2 定的是**必须导出**的符号表；本节记**本层怎么落地**它。

- **归属**：指令表的数据与纯函数在 **`pyawa-core`**（`BC-38`：指令集是 VM 的一部分）；
  `pyawa-stdlib` 只是 Python 层的**包装**，**只转发不复制**——复制会造出第二个真相源
  （`crates/pyawa-stdlib/src/opcode.rs` 的文件头写着这条）
- **`_opcode` 已落地**（导出§2.1 的全部 13 项，另加参照有的 `is_valid`）：
  - `stack_effect(opcode, oparg=None, *, jump=None)`：转发核心的坑位效应；`jump` 是**仅关键字**
  - `has_arg`／`has_const`／`has_name`／`has_jump`／`has_free`／`has_local`／`has_exc`：转发核心的谓词表
  - `get_intrinsic1_descs()`／`get_intrinsic2_descs()`／`get_special_method_names()`：返回 `list`
    （实测形态）；`get_nb_ops()`：返回 `list`，元素是 `(NB_名字, 符号)` **二元组**
  - `get_executor(code, offset)`：Pyawa 无 JIT ⇒ **恒 `None`**（§2.1 明文允许）
- **`_opcode_metadata` 已落地**：`opmap`（名字 → 编号，转发 `OPMAP`）、`HAVE_ARGUMENT`、
  `MIN_INSTRUMENTED_OPCODE`；**`_specializations` 与 `_specialized_opmap` 必须为空 dict**
  （`BC-32`）——⚠ 参照实现的这两个表**非空**（实测 17／84 项），**别把它当期望**（§2.2 的警示）
- **用法错误的几条实测消息**（`tests/opcode_module.rs` 逐字对上）：
  `has_arg() missing required argument 'opcode' (pos 1)`、
  `has_const() takes at most 1 argument (2 given)`、
  `'str' object cannot be interpreted as an integer`、
  `stack_effect() takes at least 1 positional argument (0 given)`、
  `stack_effect() takes at most 2 positional arguments (3 given)`、
  `ValueError: invalid opcode or oparg`
- **未实测的错误路径**（如实标注）：0 实参调用 `get_*` 那四个、`stack_effect` 的未知关键字与
  `jump` 重复传值——本层按 CPython 的通用措辞实现，**尚未**逐条对拍参照
- **验收**：`crates/pyawa-stdlib/tests/opcode_module.rs`——必须导出的符号都在；
  七个谓词对 8 个编号与 `pyawa-core` 的表逐项一致；特化两表为空且 `opmap` 与核心同规模；
  getter 的返回形态；六条实测消息；`get_executor` 恒 `None`

#### 5.2.6 `itertools`

`CM-14` 的 fan-in 表里排第三（`itertools`(47)，"属前五个，解锁 67% 的关键路径"）。**已落地 `count`／`repeat`／`islice`／`chain`**（其余待补）。

- **已落地**：`count(start=0, step=1)`（无限迭代器）
  - 语义照参照**实测**：`count()` ⇒ 0、1、2…；`count(1, 2)` ⇒ 1、3、5、7、9；`count(5, 3)` ⇒ 5、8、11…；
    `count(0, -1)` ⇒ 0、−1、−2…；`count(-7, 4)` ⇒ −7、−3、1…（全部进夹具 `tests/fixtures/itertools.rs`）
  - **`iter(c) is c`**（迭代器是它自己的迭代器）⇒ 已在 `ITERATOR_TYPE_NAMES` 里登记，`GET_ITER`
    对它原样返回
  - 用法错误的消息**逐条实测**：`count() takes at most 2 arguments (3 given)`、
    `count() got an unexpected keyword argument 'x'`、非数值 ⇒ `TypeError: a number is required`
  - `__name__` ＝ `itertools`；`__doc__` **照参照原文**（`tools/gen_itertools_fixture.py` 探测导出到
    `crates/pyawa-stdlib/src/itertools_doc.txt`，模块用 `include_str!`）
- **两条已知边界**（**如实报未接线，不静默凑**）：
  1. **整数宽度**：本层 `int` 是 `i64`，参照实现是**任意精度**（实测 `count(2**70, 2**70)` 给出 2^70 级别的值）
     ⇒ 越过 `i64` 时 `advance` 报 `ExecError::Unsupported`（**不回绕、不截断**）。
     ⚠ **这条要裁**：任意精度 `int` 的口径在 `docs/` 里**还没有**（`TS-40` 只讲子类型关系与数值塔提升，
     没讲宽度／溢出）——见本轮报告的 ①②③
  2. **浮点**：参照接受浮点（实测 `count(0.5, 0.5)` ⇒ 0.5、1.0、1.5…），本层 `count` 的载荷是整数
     ⇒ 浮点实参报 `ExecError::Unsupported`（不静默取整）
- **已落地**：`repeat(object, times=None)`——`times` 缺省 ⇒ 无限；`times` 为**负数** ⇒ **空**
  （实测 `repeat(5, -1)` ⇒ `[]`）；消息逐条实测：`repeat() missing required argument 'object' (pos 1)`、
  `repeat() takes at most 2 arguments (3 given)`、非整数 ⇒ `'str' object cannot be interpreted as an integer`
- **已落地**：`islice(iterable, stop)` ／ `islice(iterable, start, stop[, step])`。
  **实测的状态机语义**（探测夹具记着全部这些）：
  - 让出下标 `i` 满足 `start <= i < stop` 且 `(i - start) % step == 0`（`islice(range(10), 2, 5, 2)` ⇒ `[2, 4]`）
  - **消耗到 `max(start, stop)`**：`islice(count(), 5, 2)` 一个都不让出，但**仍消耗 5 个**
    （之后再取内层是 5，不是 0）；`islice(count(), 10, 20, 5)` ⇒ `[10, 15]` 且消耗 **20** 个
  - 内层先耗尽 ⇒ 正常结束（`islice([1, 2], 10)` ⇒ `[1, 2]`）
  - `stop`／`step` 可以是 `None`（`None` ⇒ 无上界／步长 1）
  - 消息逐条实测：`islice expected at least 2 arguments, got 1`、
    `ValueError: Step for islice() must be a positive integer or None.`、
    内层不是可迭代 ⇒ `'int' object is not iterable`（与 `GET_ITER` **同一处实现** `executor::iter_value`，
    消息不会分叉）
- **已落地**：`chain(*iterables)`——**惰性**（`chain(count(5), [9])` 只取头 3 个 ⇒ `[5, 6, 7]`，
  不会被无限的内层卡住）；`chain()` ⇒ 空；元素不是可迭代对象时**取值那一刻**才报
  `'int' object is not iterable`（实测；构造本身不报）。实现：参数收进 `list` ⇒ `iter_value` 得外层
  迭代器 ⇒ 状态机在"外层取下一个内层"与"从当前内层取值"之间切换
  - **未落地**：`chain.from_iterable(...)`（参照的类方法；本层把 `chain` 做成**函数**，
    没有可挂类方法的类型对象）
- **已落地**：`takewhile(predicate, iterable)`／`dropwhile(predicate, iterable)`／
  `filterfalse(predicate, iterable)`——语义照实测（以 `x < 3` 与 `[1, 2, 3, 4, 1]` 为例：
  `[1, 2]`／`[3, 4, 1]`／`[3, 4]`）；消息逐条实测：
  `takewhile expected 2 arguments, got 1`（三个各报各的名字）、
  谓词不可调用 ⇒ `'int' object is not callable`（**第一次取值时**才报）、
  内层不可迭代 ⇒ `'int' object is not iterable`（与 `GET_ITER` 同一处实现）
  - 实现：三者共用一个状态形状（内层 ＋ 谓词 ＋ mode ＋ 一个标志），谓词走**普通调用**
    （`call_value`），异常照上抛（实测谓词抛 `ZeroDivisionError` 会穿透）
- **已落地**：`accumulate(iterable[, func])`——`func` 缺省是**加法**（本层只做整数，与 `BINARY_OP`
  的现状同口径）；**实测**：非可调用的 `func` 不当场报错，首次要用时才报
  （`accumulate([1], 5)` ⇒ `[1]`）；缺参消息 `accumulate() missing required argument 'iterable' (pos 1)`
- **已落地**：`starmap(function, iterable)`——每个元素**展开**成实参调用（认 `tuple` 与 `list`）；
  元素不能展开 ⇒ 实测 `'int' object is not iterable`；缺参 ⇒ `starmap expected 2 arguments, got 1`
- **已落地**：`cycle(iterable)`——惰性（**边取边缓存**，取多少消费多少），取完一直重放缓存；
  空输入立刻耗尽。消息逐条实测：`cycle expected 1 argument, got 0`、
  `cycle() takes no keyword arguments`、非可迭代 ⇒ `'int' object is not iterable`
- **已落地**：`pairwise(iterable)`（两两成对，短输入 ⇒ 空）与 `batched(iterable, n)`
  （每批最多 `n`、末批可短）。消息逐条实测：`pairwise expected 1 argument, got 0`、
  `batched() missing required argument 'n' (pos 2)`、`ValueError: n must be at least one`、
  `'str' object cannot be interpreted as an integer`、
  `batched() takes exactly 2 positional arguments (3 given)`
- **已落地**：`zip_longest(*iterables, fillvalue=None)`——多个迭代器同时走，短的一侧用 `fillvalue`
  补，**全**耗尽才停。**实测**：无参数 ⇒ `[]`（**不报错**）；`fillvalue` 只能按关键字给；
  未知关键字的消息**不带**名字（`zip_longest() got an unexpected keyword argument`）；
  非可迭代实参 ⇒ `'int' object is not iterable`
- **已落地**：`compress(data, selectors)`（按选择器真假筛；任一先尽就停）与
  `combinations(iterable, r)`（池**当场物化**；`r = 0` ⇒ 一个空元组；`r > len` ⇒ 空）。
  消息逐条实测：`compress() missing required argument 'selectors' (pos 2)`、
  `combinations() missing required argument 'r' (pos 2)`、
  `'str' object cannot be interpreted as an integer`、`ValueError: r must be non-negative`
- **已落地**：`permutations(iterable, r=None)`——池**当场物化**；顺序是"下标的**字典序**"
  （实测 `permutations([1,2,3])` 正是它）；`r` 缺省是池长；`r = 0` ⇒ 一个空元组；`r > len` ⇒ 空。
  消息逐条实测：`permutations() missing required argument 'iterable' (pos 1)`、
  **`Expected int as r`**（与 `combinations` 的 `'str' object cannot be interpreted as an integer`
  **不同**——两处都比对过了）、`ValueError: r must be non-negative`
- **已落地**：`combinations_with_replacement(iterable, r)`——与 `combinations` 同一个状态形状
  （多一个 `replace` 标志）：下标**可重复且非降序**，起点是下标全 0；`r = 0` ⇒ 一个空元组；
  `([], 1)` ⇒ 空。消息逐条实测：`combinations_with_replacement() missing required argument
  'iterable' (pos 1)`／`'r' (pos 2)`、非整数 ⇒ `'str' object cannot be interpreted as an integer`、
  负数 ⇒ `ValueError: r must be non-negative`
  - **过程里抓到一处真 bug**：可重复那一支的**首个**组合一度用了 `0..r`（那是不可重复的起点）✗，
    夹具的 `(1,1)` 当场拆穿 ⇒ 改成下标全 0
- **已落地**：`product(*iterables, repeat=1)`——每个输入**当场物化**，`repeat` **复用同一份池**；
  无输入 ⇒ 一个空元组；任一池为空 ⇒ 空。消息逐条实测：非整数 `repeat` ⇒
  `'str' object cannot be interpreted as an integer`、负数 ⇒
  `ValueError: repeat argument cannot be negative`、
  未知关键字 ⇒ `product() got an unexpected keyword argument 'nope'`（这一条**带名字**）
- **本段未落地**（各自后续）：`groupby`／`tee`／`chain.from_iterable`／`chain.from_iterable`／`cycle`／`accumulate`／
  `batched`／`compress`／`dropwhile`／`filterfalse`／`groupby`／`pairwise`／`starmap`／`takewhile`／`zip_longest`／
  `product`／`permutations`／`combinations`／`combinations_with_replacement`／`tee`／
  `chain.from_iterable`
  （参照实现一共 20 个公开名，夹具里留档；其中 `repeat`／`islice`／`chain` 会**持对象引用**
  ⇒ 要先定它们的 GC 面，不硬塞进现有 `IteratorObject`）
- **归属**：`count` 的载荷在 `pyawa-core`（`CountIteratorObject`，只含整数 ⇒ 不持引用、
  `traverse` 面为零）；`repeat`／`islice` 的载荷是 `ItStateObject`（**持对象引用** ⇒ 挂
  `traverse`／`clear`，`OM-40`／`OM-20` ②：`dealloc` 只释内存、引用由 `clear` 交出）；模块面都在
  `pyawa-stdlib`；类型对象与 `TypeBoundaryError`／`Frame` 一样走 `alloc_type_raw`（`TS-41` 探测表里没有）
- **构造器的引用约定**（**四个统一**）：`new_count_iterator`／`new_repeat_iterator`／
  `new_islice_iterator`／`new_chain_iterator` 一律**借用**入参（构造器自己加一份），调用方始终
  保留自己那份、用安全的 `Instance::release` 还——stdlib 是 `forbid(unsafe_code)`，这条约定让它
  不必碰 `unsafe`
- **验收**：`crates/pyawa-stdlib/tests/itertools.rs`（28 条：`count`／`repeat`／`islice` 的序列逐项对夹具、
  三组实测消息、浮点如实报未接线、`start >= stop` 的**消费数**、`chain` 的序列／惰性／取值时报错、三个谓词迭代器的序列与三组实测消息、`accumulate`／`starmap`／`cycle`／`pairwise`／`batched`／`zip_longest`／`compress`／`combinations`／`permutations`／`combinations_with_replacement`／`product` 的序列与
  二十六条实测消息、
`__name__`／`__doc__`）
  ＋ `crates/pyawa-core/tests/iteration_protocol.rs` 的 `an_iterator_is_its_own_iterator`

## 6. 已知义务（已取证，先写下来的那些）

| 模块 | 义务 |
|---|---|
| `sys` | fan-in **191**（最高）：`Lib/` 用到的全部属性都**必须**有；`sys.implementation.name` **必须**报 `pyawa`（`CX-13`） |
| `_opcode`／`_opcode_metadata` | 27 个指令名**必须**存在；`_opcode` 的 12 个函数**必须**有（`SPEC-bytecode.md` §2） |
| `marshal` | 只需**存在**且保 Python 层行为（`_bootstrap_external.py:30` 会 import 它） |
| `_imp` | **必须**提供 `pyc_magic_number_token`（`_bootstrap_external.py:224` 用它算 `MAGIC_NUMBER`）；其值由 Pyawa 自定 |
| `_io`／`posix` | `fd` **必须**是 `int` 且为实例能力表下标（`CP-31`）；路径解析在 VM 侧（`CP-32`） |
| `itertools`(47)／`time`(46)／`errno`(29)／`builtins`(26) | 属前五个，解锁 67% 的关键路径 |
| `_sre` | `re` **没有**纯 Python 备份，**必须**用 Rust 重写 |
| `_decimal` | **有**纯 Python 备份 `_pydecimal.py`（**6,402 行**），可先作为参照实现 |
| `_multibytecodec` ＋ `_codecs_*`(24) | 属 Unicode 子项目（§7） |
| `unicodedata` | 大小写折叠／NFC／NFKC／双向文本；数据表来源见 `§13-7` |

---

## 7. Unicode 子项目

- **CM-12** `encodings`（**32,612 行**）＋ `unicodedata` ＋ `_multibytecodec` ＋ `_codecs_*`
  **必须**作为**独立子项目**排期，**禁止**并入普通模块序列——它是 stdlib 里最大的一块。
- **CM-13** Unicode **版本必须**与**参照实现**一致：以 `unicodedata.unidata_version` 为准
  （`§13-7` **已决**）；Pyawa 侧的 `unicodedata.unidata_version` **必须**报同一字符串。
- **CM-22** 数据表**必须**由**探测参照实现导出**（与指令表同一手法，`BC-38` 的处理方式）：
  导出脚本入库，数据表是**生成产物**，**禁止手写**。
- **CM-23** 参照实现升补丁版本时，若 `unidata_version` 变化，**必须**按 `MS-22` 的四步重跑导出，
  差异**必须**在提交说明里说明。
- **CM-24** **禁止**把 Rust 生态的 Unicode crate 当权威——其版本不由我们控制，
  会造成 `unidata_version` 与行为**双重**不一致。

---

## 8. 实现顺序

- **CM-14** 顺序**必须**以 `DESIGN.md` §9 的 **fan-in 曲线**为依据（前 5 个解锁 67%、20 个解锁 80%）；
  **禁止**以"实现难度"或"个人偏好"排序。
- **CM-15** 该曲线**只衡量"能 import"**，**不衡量语义正确**（`DESIGN.md` §9）：
  **禁止**把解锁比例当作完成度或验收。
- **CM-16** 19 个模块**没有**纯 Python stdlib 依赖者（`cmath`、`faulthandler`、`resource`、
  `syslog`、`xxlimited*`、`_testcapi` 一族…）。它们的**解锁曲线优先级最低**，
  但**不能**据此判定"不必实现"——它们是**公开 API**，只是没有 stdlib 内部依赖者。

---

## 9. 走"未实现"路径的模块

- **CM-17** `ctypes`／`_ctypes`／`_testcapi`／`_testbuffer`／`xxlimited*` 一族，以及
  **`_interpreters`**（`§13-6` 已决：脚本级子解释器依赖 per-interpreter GIL，与"先 GIL／
  不承诺 free-threading"冲突）
  **必须**走"未提供"路径（`CM-6` 第一类）：前者把 C-API 概念暴露到 Python 层，
  而 Pyawa **不做 C-API**。这**不是**兼容性破坏
  （`REQUIREMENTS.md` 的 C-API 概念模块行与 `_interpreters` 行）。
- **CM-18** 这类模块**禁止**为了实现"看起来像"而伪造 C-API 语义；
  `import` 抛 `ImportError` 是**正确**行为。

---

## 10. 验收（可测性质）

| 编号 | 测试 |
|---|---|
| `T-CM-1` | 实现前 k 个模块后，可 import 的 `Lib/` 文件比例达到 `DESIGN.md` §9 声明的值（脚本可算） |
| `T-CM-2` | 已实现模块的 Python 层行为与 CPython 对拍一致（`PLAN-milestones.md` §5 的 harness） |
| `T-CM-3` | 未提供的模块 ⇒ `ImportError`；未实现的槽位 ⇒ `NotImplementedError`，且两者可区分（`CM-6`） |
| `T-CM-4` | 宿主未实现 `fs` 域时：`import os` 成功，而操作在**调用时**报未实现（`CM-6`、`CP-5`） |
| `T-CM-5` | 能力域模块在静态检查下无 libc／OS 直连（`CM-8`、`CX-4`） |
| `T-CM-6` | `ctypes` 一族 import 抛 `ImportError`，且不被计入兼容性缺陷（`CM-17`） |

---

## 11. 未决（引用 `DESIGN.md` §13）

**本规格当前无未决项。** 原先列出的五项（`§13-7`／`§13-16`／`§13-4`／`§13-17`／`§13-2`）
**均已决**，见 `DESIGN.md` §13 的"已决"。

---

## 12. 尚未写出（本规格自己缺的节）

`SPEC-INDEX.md` §5 第 6 条要求 `v0` 规格显式列出缺口。本规格缺：

| 缺的节 | 内容 | 为什么现在没有 |
|---|---|---|
| **逐模块合约表** | 113 个模块**逐个**的 Python 层 API 与语义契约（`CM-4` 要求的形式） | 体量所限；**按 `CM-14` 的顺序分批补**（已补的见 §5.2） |

**不是缺口、而是按需流程**（与 `CM-14` 的分批同理）：

- **能力槽位增量清单**：`CM-10` 已规定"需要新槽位时**必须**回 `SPEC-capabilities.md` 领号"。
  它是**流程**，不是待写的节——每批 stdlib 实现若需要新槽位，就在当批回 `CP-` 领号并落地。
  **禁止**预先把它写成一张清单（那等于凭空推测各域会缺什么）。

> 原先的「`errno` → 异常映射表」缺口已由 §5.1 的 `CM-19`…`CM-21` 关闭；
> 「Unicode 数据表来源」已由 `CM-13`／`CM-22`…`CM-24` 关闭。

### 5.2.7 `operator`（**第一刀**：比较与真值）

**目标**：把参照实现的比较／真值一族先落地（不需要能力域，纯计算）。

- **公开面**（`tools/gen_operator_fixture.py` 从参照导出，禁手写）：参照 3.14.4 的 `dir(operator)`
  共 **57** 个非下划线名字（完整清单进夹具的 `REFERENCE_NAMES`）
- **第一刀已落地**：`eq`／`ne`／`is_`／`is_not`／`truth`／`not_`（六个）—— 相等走核心的
  `values_equal_public`、真值走核心的 `truthiness_public`（`TS-40` 口径，**不另写一份规则**）、
  身份是**指针相等**；验收：模块自带单元测试 ＋ 探测夹具的实测消息
- **第二刀已落地**：`lt`／`le`／`ge`／`gt` —— 与 `COMPARE_OP` **共用**核心新开的
  `executor::compare_public`（`int`／`bool`／`str` 按值；不可比时照实测消息
  `TypeError: '<' not supported between instances of 'int' and 'str'`）
- **已顺带接线**：容器的**值相等**——`list`／`tuple` 递归逐项比、`dict` 按键值匹配后递归比、
  `set`／`frozenset` 双向包含；**不同种类**一律不等（`[1] == (1,)` ⇒ `False`、`[1] == {1}` ⇒ `False`）
- **本段未落地**：浮点（本层浮点还没落地）；`dict`／`set` 的**序**比较（`<` 一族）仍要等
  `OM-11` 的 `richcompare` 槽位
- **第三刀已落地**：`add`／`sub`／`mul` —— 与将来的 `BINARY_OP` **共用**核心新开的
  `executor::arithmetic_public`（整数 `checked_*`、越界如实报未接线；非整数照实测消息）
  —— **仍限整数**（浮点没落地、字符串拼接与列表 `+` 未接线）
- **第四刀已落地**：`floordiv`／`mod`／`pow`（同一份 `arithmetic_public` 扩展；除零照实测报
  `ZeroDivisionError: division by zero`）—— 同样**限整数**（`truediv` 与负指数要浮点 ⇒ 未做）
- **第五刀已落地**：一元 `neg`／`pos`／`abs`／`invert`（核心新开 `unary_public`）＋ 位运算
  `and_`／`or_`／`xor`／`lshift`／`rshift`（扩进 `arithmetic_public`）；边界照实测
  （负移位 `ValueError: negative shift count`、一元非整数 `TypeError: bad operand type for unary -`）
  ⇒ 本模块累计落地 **27** 个函数（另含 3.14 新增的 `is_none`／`is_not_none`），全部与核心共用实现
- **本段未落地（第五刀之外）**：`truediv`（要浮点）、`matmul`／`getitem` 一族（要协议槽位）、
  `itemgetter`／`attrgetter`／`methodcaller`（要类体与闭包）
- **实测口径**（探测夹具逐条导出）：`eq(1, 1)` ⇒ `True`、`lt(1, 2)` ⇒ `True`、
  `truth([])` ⇒ `False`、`not_(0)` ⇒ `True`、`is_(None, None)` ⇒ `True`；
  比较不可比对象（`lt(1, 'a')` ⇒ `TypeError`）与真值不可用对象（`truth()` 缺参 ⇒ `TypeError`）
  的消息**一律照实测**
- **验收**：`pyawa-stdlib` 新增一个 `operator` 测试（序列与消息都来自探测夹具）；
  测试文件、生成脚本与夹具随**实现那一笔**一起入库（`docs-rule`）
- **本段未落地**（各自后续）：算术一族（`add`／`sub`／`mul`…，要复用核心的数值通道）、
  下标与属性一族（`getitem`／`setitem`／`attrgetter`…，要 `slice`／绑定）、`methodcaller`／
  `itemgetter` 这类**工厂**（要类体与闭包）

> 公开面计数**实测填入**（57）——由 `tools/gen_operator_fixture.py` 从参照导出；生成脚本与夹具已入库，模块实现随下一笔。
