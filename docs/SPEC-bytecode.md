# SPEC — 字节码与执行模型

> 规范性文件 · 状态：M1 前置规格 v0
> 读者：实现编译器、VM、`_opcode`／`_opcode_metadata` 的开发者
> **冲突时的优先级**：`REQUIREMENTS.md`（决策）> 本文件（执行细则）> `DESIGN.md`（架构论证）
> 依赖：`SPEC-object-model.md`（帧、code object、容器都建立在它之上）

规范用语：**必须** / **禁止** / **建议** / **可选**。硬约束编号 **`BC-n`**。

---

## 1. 范围与依赖

规定：code object、指令集契约、编译管线、帧与执行、异常表、生成器／协程、`sys.monitoring` 事件点。

**不**规定：`.pyac` 的命名与失效判定、模式开关与 import 钩子（→ `SPEC-imports-and-modes.md`）；
对象的引用计数协议（→ `SPEC-object-model.md`）。

---

## 2. 上游硬契约（**这一节不是设计，是逐字同步 `Lib/` 推导出的义务**）

`Lib/opcode.py` 与 `Lib/dis.py` 逐字同步、**禁止修改**。它们在 **import 时**就会索引特定名字，
因此下面的符号**必须**存在且类型正确；缺一个就是 import 失败。

### 2.1 `_opcode` 必须导出

| 符号 | 形态 | 依据 |
|---|---|---|
| `stack_effect(opcode, oparg=None, *, jump=None)` | 函数 → int | `opcode.py:14`、`dis.py` |
| `has_arg(op)` | 函数 → bool | `opcode.py:28` |
| `has_const(op)` | 函数 → bool | `opcode.py:29` |
| `has_name(op)` | 函数 → bool | `opcode.py:30` |
| `has_jump(op)` | 函数 → bool | `opcode.py:31` |
| `has_free(op)` | 函数 → bool | `opcode.py:34` |
| `has_local(op)` | 函数 → bool | `opcode.py:35` |
| `has_exc(op)` | 函数 → bool | `opcode.py:36` |
| `get_intrinsic1_descs()` | 函数 → 序列 | `opcode.py:39` |
| `get_intrinsic2_descs()` | 函数 → 序列 | `opcode.py:40` |
| `get_special_method_names()` | 函数 → 序列 | `opcode.py:41` |
| `get_nb_ops()` | 函数 → 序列 | `opcode.py:44` |
| `get_executor(code, offset)` | 函数 | `dis.py:22`；Pyawa 无 JIT，**允许**恒返回 `None` |

### 2.2 `_opcode_metadata` 必须导出

`opmap`（dict：名字 → 编号）、`_specializations`、`_specialized_opmap`、
`HAVE_ARGUMENT`、`MIN_INSTRUMENTED_OPCODE`（`opcode.py:16–17`）。

`_specializations` 与 `_specialized_opmap` **必须**为空 dict（`BC-32`：Pyawa 不做 CPython 式特化），
`opname` 的构造对空值安全（`opcode.py:21–23` 遍历两者）。

⚠ **别把参照实现的值当成规格的期望**：CPython 3.14.4 的这两个表**非空**（实测 **17**／**84** 项）。
验收的期望值是"**Pyawa 的为空**"；参照实现的值只可另存备查。二者混用会把 CPython 的特化
当成 Pyawa 的期望——这正是实现时踩到过的坑。

### 2.3 `opmap` 必须包含的指令名（**27 个，缺一即 import 崩**）

`opcode.py` 直接索引：
**`EXTENDED_ARG`**（`:18`）、**`COMPARE_OP`**（`:46`）

`dis.py` 直接索引：
`BINARY_OP`、`CALL_INTRINSIC_1`、`CALL_INTRINSIC_2`、`CONTAINS_OP`、`CONVERT_VALUE`、
`END_ASYNC_FOR`、`ENTER_EXECUTOR`、`FOR_ITER`、`IMPORT_NAME`、`IS_OP`、`JUMP_BACKWARD`、
`LOAD_ATTR`、`LOAD_COMMON_CONSTANT`、`LOAD_FAST_BORROW_LOAD_FAST_BORROW`、
`LOAD_FAST_LOAD_FAST`、`LOAD_GLOBAL`、`LOAD_SMALL_INT`、`LOAD_SPECIAL`、`LOAD_SUPER_ATTR`、
`SEND`、`SET_FUNCTION_ATTRIBUTE`、`STORE_FAST_LOAD_FAST`、`STORE_FAST_STORE_FAST`、
`STORE_GLOBAL`、`STORE_NAME`

- **BC-1** 上述 27 个名字**必须**全部出现在 `opmap` 中。
  **建议**直接采用 CPython 的整张指令名表：`dis.py` 的分支逻辑按名字格式化输出，
  名字缺语义会让反汇编结果误导使用者。
- **BC-2** 编号**以 CPython 3.14 为基线**（`BC-30`）——**禁止**为"紧凑"或"看起来自有"而重新编号：
  `opname` 由 `opmap` 派生（`opcode.py:20`），而 `_cache_format` 与 `dis` 的分支都**按名字写死**，
  重编号只有错位风险、没有收益。Pyawa 专有指令取**空闲编号**（`BC-31`）。
- **BC-3** `EXTENDED_ARG` 的语义仍须为"扩展下一个指令的 oparg"（`dis` 依赖它拼装长参数）。

### 2.4 code object 必须暴露的属性与方法

`Lib/types.py` 是纯 Python，其中 `CodeType = type((lambda: 0).__code__)`，
而 `dis.py` 从 code object 读取以下内容。**这些名字与含义都必须与 CPython 一致**：

| 属性 | 用途（`dis` 中出现次数） |
|---|---|
| `co_code` | 原始字节码（13） |
| `co_consts` | 常量表（22） |
| `co_names` | 全局／属性名表（8） |
| `co_varnames` | 局部名表（2） |
| `co_cellvars` / `co_freevars` | 闭包（2／2） |
| `co_positions()` | 方法 → 迭代 `(lineno, end_lineno, col, end_col)`（18） |
| `co_lines()` | 方法 → 迭代 `(start, end, lineno)`（1） |
| `co_exceptiontable` | 异常表字节串（1） |
| `co_flags` | 标志（2） |
| `co_stacksize`、`co_nlocals`、`co_argcount`、`co_posonlyargcount`、`co_kwonlyargcount` | 元信息（各 1） |
| `co_name`、`co_qualname`、`co_filename`、`co_firstlineno` | 元信息（1／–／1／3） |
| `_varname_from_oparg()` | `dis` 用（1） |

- **BC-4** 上表**必须**全部提供，且 `co_positions()`／`co_lines()` 的元组形状**必须**如上。
  这是 traceback 的列信息与 `dis` 输出的前提。
- **BC-5** `co_code` 是 **Pyawa 自己的字节码字节串**（不是 CPython 的）。
  `dis` 会按 `opmap` 解释它——因此 BC-1 与 BC-2 的取值直接决定反汇编是否可读。
- **BC-6** `co_exceptiontable` 是 Pyawa 自己的异常表编码；`dis` 只按字节串展示，
  不要求与 CPython 的编码相同。

> **这段是本文件最重要的一节**：它说明"Pyawa 自有字节码"的自由度**只在于编号与编码**，
> **不在于指令名与 code object 的可见属性**。

---

## 3. 帧与执行

- **BC-7** 帧**必须**是对象（有 `SPEC-object-model.md` 的头部），因为 `sys._getframe`、
  `frame.f_locals`、traceback 都是语义的一部分。
- **BC-8** 值栈与局部变量槽**必须**是帧的一部分；每个栈项**持有**一个引用
  （协议见 `SPEC-object-model.md` OM-16／17）。
- **BC-9** 局部变量的读写**必须**经槽位索引（编译期解析名字），
  **禁止**运行时按名字查字典——否则 `LOAD_FAST` 类指令的语义无法成立。
- **BC-10** 闭包用 cell 对象实现；cell **必须**是 `GC_TRACKED`
  （cell 是经典成环来源：递归函数经 cell 引用自身）。
- **BC-11** 帧**必须**可挂起：生成器、`async`／`await`、`yield from` 共用同一套可挂起帧机制，
  **禁止**为协程另建一套执行栈。
- **BC-12** 异常处理**必须**用异常表（区间查询）模型，**禁止**用"每条指令的 handler 链"，
  以支持 `except*`（异常组）与零开销的 try。
- **BC-13** 求值顺序、`finally`、`with`、`try` 的语义**必须**与 CPython 3.14 一致，
  包括 `return` 途中经过 `finally` 的细节。

---

## 4. 编译管线

- **BC-14** 编译器**必须**以"模式"为显式输入参数（`compile(source, filename, mode)`，
  mode ∈ {纯 Python 模式, 扩展模式}）；**禁止**从全局或环境推断模式。
  模式由文件后缀决定（`SPEC-imports-and-modes.md`）。
- **BC-15** 纯 Python 模式下，任何扩展语法**必须**报 `SyntaxError`；
  **禁止**"先解析再忽略"——否则无法保证 `.py` 的纯净性（`DESIGN.md` §2.1）。
- **BC-16** 编译**必须**是纯函数：相同（源码、文件名、模式、优化级别）→ 相同字节码。
  这条是 `.pyac` 可缓存与"对拍 harness 可复现"的前提。
- **BC-17** 作用域规则（`global`／`nonlocal`、类体、推导式作用域、`except as` 的解绑时机）
  **必须**与 CPython 一致；这些是历史上最容易被漏掉的语义角落。
- **BC-18** 编译期**必须**产出与源码行／列对齐的位置表，以支撑 BC-4 的
  `co_positions()`／`co_lines()`。

### 4.1 边界检查指令（Pyawa 专有）

`DESIGN.md` §13-9 已拍板**方案 B**：边界检查用**专用指令**，不由类型系统降级为普通调用。
归属本文件（`SPEC-INDEX.md` §4.1）。

- **BC-23** 指令集**必须**含边界检查指令（两个名字，或一个带方向位）：
  - `CHECK_BOUNDARY_IN`——进入标注函数时校验入参，**归责给调用方**
  - `CHECK_BOUNDARY_OUT`——标注函数返回时校验返回值，**归责给被调用方**

  **禁止**把它们实现为对普通可调用对象的调用（那等于退回方案 A）。
- **BC-24** oparg **必须**足以定位签名条目（**建议**：常量表中的签名索引，或类型元数据表的偏移）。
- **BC-25** 发射规则两条**都**要满足：
  ① **只在标注／未标注的交界处发射**；两侧都已标注（静态可查）时**禁止**发射——
     否则就是把静态检查重复成运行期开销，违背"安全度由程序自己选择"的成本模型（`DESIGN.md` §6）；
  ② **只出现在扩展模式编译出的代码里**（`TS-5`）——纯 Python 模式**禁止**产出任何 Pyawa 专有指令。
     因此 `.py` 调用 `.pyawa` 的**已标注**函数时**仍会**被检查（检查在**被调用方**的序言里），
     而 `.pyawa` 调用纯 `.py` 函数时**不会**。
- **BC-26** 检查失败**必须**抛归责异常，且**必须**携带：方向（入／出）、期望类型、实际类型、
  边界位置（文件名与行号）。**异常的具体形态属 `SPEC-type-system.md`**，本文件不定义。
- **BC-27** `stack_effect` **必须**正确处理这两个指令；`has_arg` **必须**对它们返回真。
- **BC-28** 它们是 **Pyawa 专有指令**，不在 §2.3 的 27 个必含名字之内；编号自定（BC-2）。
  `dis` 会按 `opname` 原样显示——**这正是选 B 的收益：检查点在反汇编里可见**。
- **BC-29** 指令集**必须**有一个版本常量；**`.pyac` 头部必须携带它**，版本不符即判陈旧
  （`.pyac` 布局由 `SPEC-imports-and-modes.md` 定义）。指令集任何增删**必须**升版本。

---

## 5. `sys.monitoring` 事件点（架构约束，不是后加功能）

`DESIGN.md` 后果 8：调试器与 profiler 是已确认的工具链需求，事件点**必须**在主循环预留。

- **BC-19** 主循环**必须**有可挂事件点的位置，至少覆盖：
  `PY_START`／`PY_RETURN`／`PY_YIELD`、`CALL`／`C_RETURN`、`LINE`、
  `JUMP`／`BRANCH`、`INSTRUCTION`、`RAISE`／`EXCEPTION_HANDLED`、
  `STOP_ITERATION`。
- **BC-20** 事件点**必须**可整体关闭且关闭时**不改变语义**；
  开关状态**必须**按实例存放（无全局状态）。
- **BC-21** M1 **只需预留事件点结构，禁止实现**完整的 `sys.monitoring` 工具 API。
- **BC-22** `sys.setprofile`／`sys.settrace` 的可见行为属于语义，但**允许**排在
  `sys.monitoring` 之后实现（`DESIGN.md` §13 未列此项为 M1 阻塞）。

---

## 6. 验收（可测性质）

| 编号 | 测试 |
|---|---|
| T-BC-1 | `import opcode` 与 `import dis` 成功（无 `KeyError`）——直接验证 BC-1 |
| T-BC-2 | 对一段固定脚本 `dis.dis(f)` 输出可读且无异常（验证 BC-4／BC-5） |
| T-BC-3 | 纯 Python 模式下扩展语法报 `SyntaxError`；扩展模式下通过（BC-15） |
| T-BC-4 | 同一源码编译两次得到逐字节相同的 `co_code` 与位置表（BC-16） |
| T-BC-5 | 异常组（`except*`）、`finally` 中 `return`、`with` 嵌套、推导式作用域
与 CPython 对拍一致（BC-12／BC-13／BC-17） |
| T-BC-6 | 生成器／协程的挂起与恢复在递归、嵌套 `yield from` 下正确（BC-11） |
| T-BC-7 | 关闭全部事件点前后，同一脚本行为一致且性能特征无观测差异（BC-20） |
| T-BC-8 | 标注／未标注交界处能观察到检查指令；两侧皆标注处**没有**该指令（BC-25） |
| T-BC-9 | 检查失败时异常携带方向、期望类型、实际类型、文件名与行号（BC-26） |
| T-BC-10 | 改动指令集后旧 `.pyac` 被判定为陈旧而非被加载（`BC-29`／`BC-40`） |
| T-BC-11 | **基线 ⊆ `opmap`**，且额外项**仅为** Pyawa 专有指令；专有指令只占空闲编号（`BC-30`／`BC-31`） |
| T-BC-12 | 逐个发射 §10 中**带 cache** 的指令后，`dis` 能正确反汇编且偏移对齐（`BC-35`） |
| T-BC-13 | `_specializations`／`_specialized_opmap` 为空；无 instrumented／executor 指令被发射（`BC-32`） |
| T-BC-14 | 值栈越界触发错误而非 UB；`co_stacksize` 被遵守（`BC-43`） |
| T-BC-15 | §10 的每条指令都能被 `stack_effect` 给出值（`BC-38`／`BC-49`） |
| T-BC-16 | §11 的每条构造都有对拍用例，且**求值顺序与可见副作用**与 CPython 一致（`BC-52`） |
| T-BC-17 | **跳转目标**按 `BC-55` 的公式计算，且与 `dis` 给出的 `argval` 逐条一致（含后向与带 cache 的跳转） |
| T-BC-18 | 参数绑定按 `BC-56`：四类错误都报 `TypeError`，且**消息与参照实现一致**（以探测为准） |
| T-BC-19 | 名类／属性类指令的 oparg 解码按 `BC-57`，与参照实现的 `dis` 输出**逐条一致**（含方法位与 `NULL` 位） |
| T-BC-20 | `BC-58` 表内每条指令的 oparg 解码与参照实现的 `dis` 输出**逐条一致**（含 `COMPARE_OP` 的 `>>5` 与 `bool` 位） |
| T-BC-21 | **编码对拍完备性**：语料里出现的**每一条**指令的 `argval`／`argrepr` 都与 `dis` 一致——含 `BC-57`／`BC-58` **未单独列出**的指令（`BC-59`） |

---

## 7. 未决（引用 `DESIGN.md` §13）

- **§13-12** 扩展特性清单为空 ⇒ **BC-14／BC-15 的"扩展模式"目前只是空壳**，
  语法层没有需要拦截或启用的东西。这是本规格最大的悬空点。
- **§13-13** REPL／`-c`／stdin／`compile()` 的默认模式（影响 BC-14 的调用方）
- **§13-7** Unicode 版本与数据表来源（影响源码解码与 `\N{...}` 转义）
- **§13-9 的剩余部分**：落点已决（方案 B，见 §4.1）；**检查粒度**（是否深入容器内部、深入几层）
  仍开放——它会改变 `CHECK_BOUNDARY_*` 携带的签名内容，但不改变指令集本身

---

## 8. 指令编码与元数据

### 8.1 基线：采用 CPython 3.14 的指令名与编号空间

- **BC-30** Pyawa 的指令集**以 CPython 3.14 的 `opmap` 为基线**——**基线的名字与编号一个不改**
  （实测：154 个名字，编号最大 266，`HAVE_ARGUMENT = 43`，`MIN_INSTRUMENTED_OPCODE = 234`）。
  **导出给 Python 的 `opmap` 必须是「基线 ∪ Pyawa 专有指令」的并集**（专有指令按 `BC-31` 追加）；
  否则 `opcode.py` 的 `opname` 只从 `opmap` ∪ `_specialized_opmap` 填名（`opcode.py:20–23`），
  `CHECK_BOUNDARY_*` 会在 `dis` 里显示成 `<232>` 一类占位符——`BC-28` 说的"检查点在反汇编里可见"就落空了。
  一句话：**基线 ⊆ Pyawa 的 `opmap`，且额外项只允许是专有指令。**
  理由：(a) `BC-1` 已强制 27 个名字必须存在；(b) `dis`／`opcode` 的分支逻辑与 `_cache_format`
  都**按名字写死**；(c) 语义级兼容目标下，指令语义与 CPython 一致可省掉一整类偏差。
  `BC-2` **允许**改编号，但改只会带来错位风险而无收益，**不建议**。
  **"自有字节码"指的是自有 VM 与自有实现，不是必须发明 ISA。**
- **BC-31** Pyawa **专有指令**（如 `CHECK_BOUNDARY_IN`／`CHECK_BOUNDARY_OUT`，`BC-23`）
  **必须**取**空闲编号**：**禁止**复用已占编号，也**禁止**占用 `MIN_INSTRUMENTED_OPCODE` 以上区段。
- **BC-32** **禁止发射**：编号 ≥ `MIN_INSTRUMENTED_OPCODE` 的 instrumented 一族、
  `ENTER_EXECUTOR`、以及 `_specialized_opmap` 里的特化名。Pyawa **不实现** CPython 的特化与 executor，
  故 `_specializations` 与 `_specialized_opmap` **必须**为空 dict（`BC-1` 的注）。

### 8.2 编码

- **BC-33** 指令流是**码元序列**，每码元 2 字节：`opcode: u8` ＋ `oparg: u8`；
  无参指令的 oparg **必须**为 0。
  ⚠ **`opmap` 里有 11 项编号 ≥ 256**（本机实测：`ANNOTATIONS_PLACEHOLDER`=256、`JUMP`=257、
  `JUMP_IF_FALSE`=258、`JUMP_IF_TRUE`=259、`JUMP_NO_INTERRUPT`=260、`LOAD_CLOSURE`=261、
  `POP_BLOCK`=262、`SETUP_CLEANUP`=263、`SETUP_FINALLY`=264、`SETUP_WITH`=265、
  `STORE_FAST_MAYBE_NULL`=266）。**它们不可能出现在 `co_code` 里**——一个字节表示不了 ≥256 的值，
  这是**构造性事实**，故它们是**编译期内部标记**，不是可发射指令。
  Pyawa 的编译器**同样禁止**把它们写进 `co_code`；因此 `opcode: u8` 与 `BC-30` 的
  "`opmap` 与基线一致"**不冲突**（表里有、码元里没有，两者都对）。
- **BC-34** `EXTENDED_ARG` 展开：oparg **必须**按**大端**拼接——每个 `EXTENDED_ARG` 贡献 8 位高位，
  直到最后一个非 `EXTENDED_ARG` 指令贡献低 8 位。**禁止**其他拼接顺序（`dis` 依赖它还原长参数）。
- **BC-55** **跳转的基准与单位**（**上游硬契约**：`dis.py:567–572` 用的就是这个公式，
  算错则反汇编里**所有**跳转目标都是错的）：

  ```
  目标字节偏移 = offset + 2 + signed_arg*2 + 2*caches
  ```

  - `offset` ＝ 该指令自身的**字节**偏移；`2` ＝ 它自身（opcode ＋ oparg 各 1 字节）
  - `caches` ＝ 它**自己的** inline cache 槽数（`BC-35`），**必须**计入
  - `signed_arg` ＝ **前向取 `+arg`，后向取 `−arg`**；后向判定**只看名字**属于
    `{JUMP_BACKWARD, JUMP_BACKWARD_NO_INTERRUPT, END_ASYNC_FOR}`（`dis._is_backward_jump`；
    注意 `END_ASYNC_FOR` 也带目标）
  - **以码元计**：`目标码元 = offset_cu + 1 + signed_arg + caches`

  **3.14 实测**：`hasjabs` **为空**——16 条跳转**全部是相对**，故无绝对跳转分支。
  **禁止**以"下一条指令之后"或"不含自身 cache"的方式解释 oparg。
- **BC-56** **参数绑定规则**（`BC-44` 只定了**槽位顺序**，这里定"**实参怎么填进槽位**"）：
  - 顺序：仅位置参数 → 位置或关键字参数 → `*args`（收集多余位置实参）→ 仅关键字参数 →
    `**kwargs`（收集未知关键字）
  - 多余位置实参、缺少必填实参、同一参数被位置与关键字重复给、未知关键字 ⇒ **必须**报 `TypeError`
  - **错误消息必须与参照实现一致**（`MS-8` 要比对未捕获异常的消息，`MS-9` 只做路径／地址／耗时归一）
    ⇒ 消息文本**必须**以**探测参照实现**为准，**禁止**手写近似文本
  - 绑定**必须**在**进入函数体之前**完成，**禁止**进入后再重试或部分赋值
- **BC-57** **名类／属性类指令的 oparg 解码**（**上游硬契约**：`dis.py` 的 `hasname` 分支只对
  `LOAD_GLOBAL`／`LOAD_ATTR`／`LOAD_SUPER_ATTR` 做移位，**其余一律直接用 `arg`**）：

  | 指令 | 下标 | 标志位（**实测 3.14.4**） |
  |---|---|---|
  | `LOAD_GLOBAL` | `oparg >> 1` | bit 0 ＝ 压 `NULL`（`dis` 显示 `+ NULL`） |
  | `LOAD_ATTR` | `oparg >> 1` | bit 0 ＝ **取方法**（`dis` 显示 `+ NULL|self`） |
  | `LOAD_SUPER_ATTR` | `oparg >> 2` | bit 0 ＝**取方法**；bit 1 ＝**`super()` 带两个实参**（即 `super(C, self)` 而非零参形式）。实测：`4`＝`super().m`、`5`＝`super().m()`、`6`＝`super(B,self).m`、`7`＝`super(B,self).m()` |
  | `STORE_ATTR`／`DELETE_ATTR` | **`oparg`（不移位）** | — |
  | `LOAD_NAME`／`STORE_NAME`／`DELETE_NAME`／`STORE_GLOBAL`／`DELETE_GLOBAL`／`IMPORT_NAME`／`IMPORT_FROM` | **`oparg`（不移位）** | — |

  实测样例：`obj.alpha`／`obj.beta`／`obj.gamma`（名字下标 0／1／2）的 `LOAD_ATTR` oparg
  为 `0／2／4`，而 `obj.epsilon`／`obj.zeta`／`obj.eta`（下标 4／5／6）的 `STORE_ATTR` oparg
  为 `4／5／6`——**同为属性指令却一个移位、一个不移位**，这是最容易写错的一处。
  **`LOAD_ATTR` 的方法位与 `CALL` 的 `self/NULL` 槽是一对**：置位时压入的是
  **`(方法, self)` 两个值**，不是绑定方法对象（故 `LOAD_ATTR` 必须在调用协议接线后实现）。
- **BC-58** **其余 oparg 的移位／标志位／掩码**（**上游硬契约**，`dis.py:612–650` 的判决；
  与 `BC-57` 同类，**逐条实测 3.14.4**）。这张表是"oparg 不是裸下标"的完整清单：

  | 指令 | oparg 的确切含义 |
  |---|---|
  | `COMPARE_OP` | `cmp_index = oparg >> 5`（`cmp_op` 六元组）；**bit 4（`& 16`）＝ `bool(...)` 标志**（判定上下文置位，`dis` 显示 `bool(<)`）。**低 4 位是参照实现的编译期信息**：`dis` 不读、Pyawa **不解释**，发射时可填 0，但**解码必须容受** |
  | `IS_OP`／`CONTAINS_OP` | **0／1 标志**：`0` ＝ `is`／`in`，`1` ＝ `is not`／`not in` |
  | `CONVERT_VALUE` | `1` ＝ `str`、`2` ＝ `repr`、`3` ＝ `ascii` |
  | `SET_FUNCTION_ATTRIBUTE` | **位掩码**，按 `dis.FUNCTION_ATTR_FLAGS` 的位序：bit0 `defaults`／bit1 `kwdefaults`／bit2 `annotations`／bit3 `closure`／**bit4 `annotate`**（3.14 的延迟注解协议） |
  | `MAKE_FUNCTION` | **无 oparg**（**实测 `hasarg` 不含它**）——附件一律由随后的 `SET_FUNCTION_ATTRIBUTE` 逐个施加。**禁止**照 3.12 及以前把标志塞进它的 oparg |
  | `LOAD_FAST_LOAD_FAST`／`LOAD_FAST_BORROW_LOAD_FAST_BORROW`／`STORE_FAST_LOAD_FAST`／`STORE_FAST_STORE_FAST` | **两个槽位打包**：`arg1 = oparg >> 4`、`arg2 = oparg & 15`（**高 4 位是第一个**） |
  | `LOAD_SMALL_INT` | oparg 是**字面值本身**，**不**走常量表 |
  | `LOAD_COMMON_CONSTANT`／`LOAD_SPECIAL` | oparg 索引**固定表**（`dis._common_constants`／`_special_method_names`），**不是** `co_consts`／`co_names` |
  | `CALL_KW` | `oparg` ＝ 实参总数；**关键字名表以常量元组形式在调用前压栈**（3.14 **无** `KW_NAMES`） |
  | `UNPACK_EX` | **低字节 ＝ 前置个数，高字节 ＝ 后置个数**（实测 `x, *y, z = a` → `257` ＝ `0x0101`） |
- **BC-59** **逐指令编码一律以夹具对拍 `dis` 为准**——这是 `BC-57`／`BC-58` 的**完备收口**，
  也是唯一能防"没踩到就没发现"的办法：
  - 取一份语料 → 用**参照实现**编译 → 对**语料里出现的每一条指令**断言 `argval`／`argrepr`
    与 `dis` 的逐条一致
  - **禁止**以"本规格没列出这条指令的编码"为理由跳过对拍——**`dis` 就是完备的 oracle**
  - 语料**必须**随 §10 的族增长而扩充，**至少**覆盖 §10 起步指令集的全部族
  - 理由：oparg 的解释有一百多条，**文档只能追已发现的**（`BC-57`／`BC-58` 都是被实测抓出来的）；
    本条把验收从"文档穷举"改成"**oracle 对拍**"

### 8.3 inline cache 槽（**错位隐患，必须遵守**）

- **BC-35** 若某指令被**发射**，且其名字出现在 `opcode._inline_cache_entries` 里，
  则其后**必须**留出**等宽**的 cache 码元（零填充）。
  **禁止**"用了名字却不留 cache 槽"——`dis` 会按 `_cache_format` 跳过对应宽度，不留就**整体错位**。
  实测宽度：`LOAD_ATTR`=9、`BINARY_OP`=5、`LOAD_GLOBAL`=4、`STORE_ATTR`=4、`CALL`=3、`CALL_KW`=3、
  `TO_BOOL`=3；其余 12 个带 cache 的指令各 1（`COMPARE_OP`／`CONTAINS_OP`／`FOR_ITER`／
  `JUMP_BACKWARD`／`LOAD_SUPER_ATTR`／`SEND`／`UNPACK_SEQUENCE`／`STORE_SUBSCR`／`POP_JUMP_IF_*`）。
- **BC-36** **禁止**用 cache 槽做自己的优化（`dis` 会显示垃圾）；cache 槽**必须**是零填充占位。

### 8.4 元数据接口

- **BC-37** `has_arg`／`has_const`／`has_name`／`has_jump`／`has_free`／`has_local`／`has_exc`
  **必须**对每个指令返回与 CPython 语义一致的分类——`opcode.py` 用它们构造
  `hasarg`／`hasconst`／… 这些**公开**列表。
- **BC-38** `stack_effect(opcode, oparg=None, *, jump=None)` **必须**给出栈效应，
  且**必须**对 `BC-23` 的两个专有指令也给出一致的值（`BC-27`）。
  **数值数据的唯一出处在实现**；本规格只定"必须与语义一致"，**禁止**在文档里复制第二份数值表。
  **归属（依赖边裁决）**：指令表**属于 `pyawa-core`**——指令集是 VM 的一部分，而本规格正是
  `pyawa-core` 的归属规格。`pyawa-stdlib` 的 `_opcode`／`_opcode_metadata` 是它的 **Python 层包装**，
  依赖方向 `pyawa-stdlib → pyawa-core`（`REQUIREMENTS.md` 的 crate 依赖边行、根 `Cargo.toml`）。
  生成方式：`tools/gen_opcode_tables.py` 向运行时探测并校验后生成；期望值取自
  `tools/gen_opcode_fixture.py` 导出的夹具。
  **落地位置**：`crates/pyawa-core/src/opcode_metadata.rs`（表）与 `crates/pyawa-core/src/opcode.rs`
  （函数）；期望值夹具在 `crates/pyawa-core/tests/fixture-opcode-3.14.json`。
  `pyawa-stdlib` **只做转发**（不复制数值）——`pyawa-stdlib → pyawa-core` 已随迁移落地。
- **BC-39** `BINARY_OP` 的 oparg **必须**对应 `_opcode.get_nb_ops()` 的顺序（**实测 27 项**，
  `NB_ADD`=0 … `NB_XOR`=12，`NB_INPLACE_ADD`=13 … `NB_INPLACE_XOR`=25，**`NB_SUBSCR`=26**）；
  `COMPARE_OP` 的 **`cmp_index = oparg >> 5`**（**不是**裸下标！`dis.py:622` 就是
  `cmp_op[arg >> 5]`），另有 **bit 4 ＝ `bool(...)` 标志**——细节见 `BC-58`。
  `dis` 会据此打印运算符。

### 8.5 指令集版本常量

- **BC-40** 指令集**必须**有**单调递增的整数**版本常量；指令的增、删、语义变化、
  以及 cache 宽度变化**必须**递增它。
- **BC-41** 该常量**必须**写入 `.pyac` 头部（`BC-29`；布局由 `SPEC-imports-and-modes.md` 定），
  版本不符即判陈旧。**取值**由实现维护，**禁止**在文档里写死具体数值。

---

## 9. 帧布局

- **BC-42** 帧**必须**至少含：`code`、`locals`（长度 = `co_nlocals` 的槽数组）、`stack`（值栈）、
  指令指针、异常表游标、**可挂起状态**（`BC-11`）。
- **BC-43** 值栈深度**必须**以 `co_stacksize` 为上界；越界**必须**报错，**禁止** UB 或静默扩容。
- **BC-44** 局部槽编号**必须**与 `co_varnames` 一致，顺序为：位置参数 → 仅关键字参数 →
  `*args` → `**kwargs` → 函数体局部变量；边界由 `co_posonlyargcount`／`co_argcount`／
  `co_kwonlyargcount` 共同界定。
- **BC-45** cell 与 free 变量**必须**用**独立槽数组**（对应 `co_cellvars`／`co_freevars`），
  且 cell **必须**是 `GC_TRACKED` 对象（`OM-10`；cell 是递归函数的经典成环来源）。
- **BC-46** 帧**必须**持有值栈上每一项的一个引用（`OM-16`）；弹出时**必须**按协议释放（`OM-20`）。
- **BC-47** 可挂起帧**必须**记录恢复点（指令指针 ＋ 值栈镜像 ＋ 异常表游标）；
  生成器／协程恢复**必须**从该点继续（`BC-11`）。
- **BC-48** 帧**必须**是对象且可在 Python 层观察（`BC-7`）；`f_locals` 的**可写语义**
  按 3.14 的 `locals()` 规则（`BC-13`）。
- **BC-54** `co_exceptiontable` 的编码**必须**能被 `dis.py` 的 `_parse_exception_table` **原样解析**
  （**上游硬契约**：`dis.py:733` 会解它，`dis` 的异常条目输出依赖它）。因此：
  - **base-64 varint**：`val = b & 63`；只要 `b & 64` 为真就继续，`val <<= 6` 后 `val |= b & 63`
    ——**首个字节的 6 位是高位**
  - 每条目 **4 个 varint**：`start`、`length`、`target`（三者**以码元计**，读取方会 ×2 换成字节偏移）、
    `dl`（`depth = dl >> 1`，`lasti = dl & 1`）
  - 表**以字节耗尽为终止**，**无**条目计数

  **禁止**自行设计该编码。

---

## 10. 起步指令集（M1／M2 必须覆盖）

- **BC-49** 下列指令**必须**在 M1／M2 可用；其余按同一 schema 增量补齐。
  **数值与栈效应的数据出处在实现**（`BC-38`），本表只定**覆盖面与 oparg 约定**。
  **名字一律以 `opmap` 为准**（`BC-30`）——本表若与实测 `opmap` 不符，**以 `opmap` 为准**。

| 族 | 指令 | oparg 约定 |
|---|---|---|
| 常量与名 | `RESUME`、`NOP`、`LOAD_CONST`、`LOAD_SMALL_INT`、`LOAD_COMMON_CONSTANT`、`LOAD_NAME`、`STORE_NAME`、`DELETE_NAME`、`LOAD_GLOBAL`、`STORE_GLOBAL`、`DELETE_GLOBAL` | **见 `BC-57`**（`LOAD_GLOBAL` **移位**且带 NULL 位；`LOAD_CONST`／`LOAD_NAME` 一类**不移位**） |
| 局部与闭包 | `LOAD_FAST`、`LOAD_FAST_CHECK`、`LOAD_FAST_AND_CLEAR`、`STORE_FAST`、`DELETE_FAST`、`LOAD_DEREF`、`STORE_DEREF`、`DELETE_DEREF`、`MAKE_CELL`、`COPY_FREE_VARS`、`LOAD_CLOSURE` | 槽位／cell 下标 |
| 超指令 | `LOAD_FAST_LOAD_FAST`、`STORE_FAST_STORE_FAST`、`STORE_FAST_LOAD_FAST`、`LOAD_FAST_BORROW_LOAD_FAST_BORROW` | **见 `BC-58`**（高 4 位 ＝ 第一个槽位，低 4 位 ＝ 第二个） |
| 属性与下标 | `LOAD_ATTR`、`STORE_ATTR`、`DELETE_ATTR`、`LOAD_SUPER_ATTR`、`STORE_SUBSCR`、`DELETE_SUBSCR`；**下标读用 `BINARY_OP` ＋ `NB_SUBSCR`**（3.14 无 `BINARY_SUBSCR`） | **见 `BC-57`**（`LOAD_ATTR` **移位**＋方法位；`STORE_ATTR`／`DELETE_ATTR` **不**移位） |
| 运算符 | `BINARY_OP`、`UNARY_NEGATIVE`、`UNARY_NOT`、`UNARY_INVERT`、`COMPARE_OP`、`IS_OP`、`CONTAINS_OP`、`TO_BOOL` | 见 `BC-39`／`BC-58` |
| 一元加与内建 | `CALL_INTRINSIC_1`（`INTRINSIC_UNARY_POSITIVE`=5、`INTRINSIC_IMPORT_STAR`=2、`INTRINSIC_LIST_TO_TUPLE`=6、`INTRINSIC_STOPITERATION_ERROR`=3、`INTRINSIC_ASYNC_GEN_WRAP`=4）、`CALL_INTRINSIC_2`（`INTRINSIC_PREP_RERAISE_STAR`=1） | intrinsic 序号（**实测值**，见 `BC-39` 同类来源） |
| 控制流 | `JUMP_FORWARD`、`JUMP_BACKWARD`、`POP_JUMP_IF_TRUE`、`POP_JUMP_IF_FALSE`、`POP_JUMP_IF_NONE`、`POP_JUMP_IF_NOT_NONE`、`GET_ITER`、`FOR_ITER`、`END_FOR`、`GET_LEN` | 相对偏移 |
| 调用与返回 | `CALL`、`CALL_KW`、`PUSH_NULL`、`RETURN_VALUE`（3.14 **无** `KW_NAMES`／`RETURN_CONST`） | **见 `BC-58`**（`CALL_KW` 的关键字名表以**常量元组**在调用前压栈） |
| 容器与解包 | `BUILD_TUPLE`、`BUILD_LIST`、`BUILD_MAP`、`BUILD_SET`、`BUILD_SLICE`、`BUILD_STRING`、`UNPACK_SEQUENCE`、`UNPACK_EX`、`LIST_APPEND`、`SET_ADD`、`MAP_ADD`、`LIST_EXTEND`、`SET_UPDATE`、`DICT_UPDATE`、`DICT_MERGE`（3.14 **无** `BUILD_CONST_KEY_MAP`） | 元素个数（`UNPACK_EX` 的高低字节见 `BC-58`） |
| 函数与类 | `MAKE_FUNCTION`、`SET_FUNCTION_ATTRIBUTE`、`LOAD_BUILD_CLASS`、`IMPORT_NAME`、`IMPORT_FROM`（3.14 **无** `IMPORT_STAR`） | **见 `BC-58`**（`MAKE_FUNCTION` **无 oparg**；`SET_FUNCTION_ATTRIBUTE` 是**位掩码**） |
| 异常 | `PUSH_EXC_INFO`、`POP_EXCEPT`、`CHECK_EXC_MATCH`、`RERAISE`、`RAISE_VARARGS`、`CLEANUP_THROW`、`END_ASYNC_FOR` | 见 §11 |
| 生成器与协程 | `RETURN_GENERATOR`、`YIELD_VALUE`、`SEND`、`GET_AWAITABLE`、`GET_YIELD_FROM_ITER` | — |
| 格式化与 t-string | `CONVERT_VALUE`、`FORMAT_SIMPLE`、`FORMAT_WITH_SPEC`、`BUILD_TEMPLATE`、`BUILD_INTERPOLATION`（3.14 **无** `FORMAT_VALUE`） | **见 `BC-58`**（`CONVERT_VALUE`：`1`／`2`／`3` ＝ `str`／`repr`／`ascii`） |
| 模式匹配 | `MATCH_CLASS`、`MATCH_MAPPING`、`MATCH_SEQUENCE`、`MATCH_KEYS` | 见 §11 |
| PEP 695 泛型 | `CALL_INTRINSIC_1`／`_2` 的 typevar 一族（`INTRINSIC_TYPEVAR`=7、`INTRINSIC_PARAMSPEC`=8、`INTRINSIC_TYPEVARTUPLE`=9、`INTRINSIC_SUBSCRIPT_GENERIC`=10、`INTRINSIC_TYPEALIAS`=11；`INTRINSIC_TYPEVAR_WITH_BOUND`=2、`WITH_CONSTRAINTS`=3、`SET_FUNCTION_TYPE_PARAMS`=4、`SET_TYPEPARAM_DEFAULT`=5） | intrinsic 序号 |
| **Pyawa 专有** | `CHECK_BOUNDARY_IN`、`CHECK_BOUNDARY_OUT`（`BC-23`） | 签名条目索引（`BC-24`） |

- **BC-50** 上表**禁止**依赖具体编号；名字**必须**从 `opmap` 取（`BC-30`）。
- **BC-51** 某指令的**语义**若与 CPython 不同，**必须**在 §11 写明；
  **禁止**让名字的语义暗示与实际行为不符（例如让 `LOAD_ATTR` 不做属性查找）。

> **本表只定覆盖面与 oparg 约定，不定"每条指令做什么"**：指令的**动作**以**参照实现的可观察语义**
> 为准（`REQUIREMENTS.md` 兼容深度 ＝ 语义级）；**禁止**在文档里逐条复述——那会变成第二个真相源
> （与 `BC-38` 同一处理）。`§11` 只写**构造 → 指令族**的映射，不写逐指令语义。
>
> **oparg 列的细节一律见 `BC-57`／`BC-58`**——本表只给粗粒度提示。
> **"oparg 是裸下标"是错的最常见来源**：已实测出 `LOAD_ATTR` 移位而 `STORE_ATTR` 不移位（`BC-57`）、
> `COMPARE_OP` 要 `>>5`（`BC-39`／`BC-58`）、超指令把两个槽位打包在高低 4 位（`BC-58`）等。
>
> 三条**已授权但容易被忽略**的：
> - **容器载荷布局由实现自选**（`TS-43`／`OM-38`／`OM-39`，不进 ABI）
> - **槽位的 Rust 签名由实现自选**（`TS` §15：语义契约已由 `TS` §8 给全）
> - `co_*` 一类 **Python 可见属性**（`BC-4`）的访问**必须**走 `getattr` 槽位（`OM-11`）；
>   **禁止**为内建类型旁路一条属性通道

---

## 11. 编译下降规则

- **BC-52** 下降的**可观察顺序**（求值顺序、副作用顺序、异常抛出点）**必须**与 CPython 3.14 一致
  （`BC-13`／`BC-17`）；**禁止**为减少指令数而改变可见顺序。
- **BC-53** 每条构造**必须**使用 §10 的指令族；具体序列由实现决定，但**必须**满足 `BC-52`，
  且名字解析**必须**在编译期完成（`BC-9`）。

| 构造 | 必须使用的指令族 | 必须保持的顺序／语义 |
|---|---|---|
| 常量／名／局部 | 常量与名、局部与闭包 | 名字查找发生在**运行到该点**时（`LOAD_GLOBAL` 不得提前） |
| 属性 | 属性与下标 | 先求对象、再查属性 |
| 下标读／写／删 | `BINARY_OP`＋`NB_SUBSCR` ／ `STORE_SUBSCR` ／ `DELETE_SUBSCR` | 先对象后键 |
| 二元／一元运算 | 运算符（`BINARY_OP` 见 `BC-39`） | 先左后右；增广运算用 `NB_INPLACE_*` |
| 一元加 | `CALL_INTRINSIC_1`＋`INTRINSIC_UNARY_POSITIVE` | 与 `UNARY_NEGATIVE` 对称 |
| 比较链 `a < b < c` | 运算符 ＋ 控制流 | `b` **只求值一次** |
| 布尔短路 `and`／`or` | `TO_BOOL` ＋ 控制流 | 右侧**按需**求值 |
| 条件表达式 | 控制流 | 只求值被选中一支 |
| 赋值（多目标、解包、增广） | 局部与闭包／属性与下标／容器与解包 | 左侧**从左到右**；解包失败**不**部分赋值 |
| `del` | `DELETE_*` | 与 CPython 同（含 `UnboundLocalError` 时机） |
| `if`／`while`／`for`／`break`／`continue` | 控制流 | 迭代器协议与 `for…else` 的触发条件 |
| `try`／`except`／`except*`／`else`／`finally` | 异常 ＋ 异常表（`BC-12`）；`except*` 用 `CALL_INTRINSIC_2`＋`INTRINSIC_PREP_RERAISE_STAR` | `finally` 在 `return`／`break`／异常路径上**都**执行 |
| `with`／异步 `with` | 异常 ＋ 调用 | `__exit__` 的返回语义与异常抑制 |
| 函数定义 | 函数与类 ＋ 局部与闭包 | 默认值**在 def 时**求值；装饰器**自下而上**；注解按 3.14 协议**延迟**（§2） |
| 类体与元类 | 函数与类 | 类体执行后按 `__mro_entries__`／元类解析 |
| PEP 695 泛型 | PEP 695 泛型族 | 类型参数作用域与 `SET_FUNCTION_TYPE_PARAMS` |
| `import` | `IMPORT_NAME`／`IMPORT_FROM`；星号导入用 `CALL_INTRINSIC_1`＋`INTRINSIC_IMPORT_STAR` | 走 `IM-` 的钩子；**禁止**直连文件系统（`IM-15`） |
| 推导式与生成器表达式 | 容器与解包 ＋ 控制流 ＋ 生成器与协程 | 推导式有**独立作用域**（`BC-17`） |
| `match` | 模式匹配族 | 模式**顺序**与守卫求值时机 |
| f-string／t-string | 格式化与 t-string | `!r`／`!s`／`!a` 与格式规范的求值时机 |
| **函数序言与返回** | **Pyawa 专有** | 入参检查在序言、返回值检查在返回前；发射条件见 `BC-25` |

---

## 12. 尚未写出（本规格自己缺的节）

`SPEC-INDEX.md` §5 第 6 条要求 `v0` 规格显式列出缺口。本规格缺：

| 缺的节 | 内容 | 为什么现在没有 |
|---|---|---|
| **超出 §10 的指令** | §10 只列 M1／M2 必须覆盖的族；其余按同一 schema 增量补齐 | 依赖各构造的实际落地顺序 |

**不是缺口、而是刻意不进文档的两项**（与 `BC-38` 同一处理方式）：

- **§11 的逐构造细目**：具体指令序列属**实现细节**，写死会与后续优化冲突；§11 的**族级**约束才是规范。
- **帧的 Rust 结构**：`Frame` 的字段类型依赖值的具体表示（`OM-38`，实现自选、不跨 ABI）；
  §9 的语义约束才是规范。

> 原先的「异常表的字节编码」缺口已由 `BC-54` 关闭——它不是自由设计项，而是由
> `dis.py` 的解析器**派生**出来的上游硬契约。

> **不是缺口的一项**：**指令表的数值与栈效应刻意不进文档**（`BC-38`）——它是**数据**，
> **归属 `pyawa-core`**（`pyawa-stdlib` 的 `_opcode`／`_opcode_metadata` 是它的 Python 层包装），
> 由 `tools/gen_opcode_tables.py` 向运行时探测并校验后生成。
> 它**已落地**，因此不列为缺口；详细归属、路径与迁移状态见 `BC-38`。
