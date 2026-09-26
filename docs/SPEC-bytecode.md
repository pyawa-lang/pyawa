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

`_specializations` 与 `_specialized_opmap` **允许**为空 dict（Pyawa 不做 CPython 式特化），
`opname` 的构造对空值安全（`opcode.py:21–23` 遍历两者）。

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
- **BC-2** 编号（编号空间）**允许**与 CPython 不同——`opname` 由 `opmap` 派生，
  `dis` 不假设编号连续或有特定值。但编号**必须**能容纳
  `max(opmap.values())` 的 `opname` 列表（`opcode.py:20`），故编号**建议**紧凑。
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
- **BC-25** 发射规则：**只在标注／未标注的交界处发射**。两侧都已标注（静态可查）时**禁止**发射——
  否则就是把静态检查重复成运行期开销，违背"安全度由程序自己选择"的成本模型（`DESIGN.md` §6）。
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
| T-BC-10 | 改动指令集后旧 `.pyac` 被判定为陈旧而非被加载（BC-29） |

---

## 7. 未决（引用 `DESIGN.md` §13）

- **§13-12** 扩展特性清单为空 ⇒ **BC-14／BC-15 的"扩展模式"目前只是空壳**，
  语法层没有需要拦截或启用的东西。这是本规格最大的悬空点。
- **§13-13** REPL／`-c`／stdin／`compile()` 的默认模式（影响 BC-14 的调用方）
- **§13-7** Unicode 版本与数据表来源（影响源码解码与 `\N{...}` 转义）
- **§13-9 的剩余部分**：落点已决（方案 B，见 §4.1）；**检查粒度**（是否深入容器内部、深入几层）
  仍开放——它会改变 `CHECK_BOUNDARY_*` 携带的签名内容，但不改变指令集本身
