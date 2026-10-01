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

---

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

- **§13-7** —— Unicode 版本与数据表来源 ⇒ `CM-13`
- **§13-16** —— C ABI 函数数量预算 ⇒ 能力槽位总数上限，影响 `CM-10` 的拆分粒度
- **§13-4** —— 宿主自定义能力的异步分类声明 ⇒ 能力域模块里哪些调用可异步化
- **§13-17** —— 启动延迟与常驻内存是否列为 M1 指标 ⇒ 前五个模块的实现取舍
- **§13-2** —— ABI 版本策略 ⇒ `_imp`／`sys` 一类启动路径模块的版本字段

---

## 12. 尚未写出（本规格自己缺的节）

`SPEC-INDEX.md` §5 第 6 条要求 `v0` 规格显式列出缺口。本规格缺：

| 缺的节 | 内容 | 为什么现在没有 |
|---|---|---|
| **逐模块合约表** | 113 个模块**逐个**的 Python 层 API 与语义契约（`CM-4` 要求的形式） | 体量所限；**按 `CM-14` 的顺序分批补**，每批随该批实现一同落地 |

**不是缺口、而是按需流程**（与 `CM-14` 的分批同理）：

- **能力槽位增量清单**：`CM-10` 已规定"需要新槽位时**必须**回 `SPEC-capabilities.md` 领号"。
  它是**流程**，不是待写的节——每批 stdlib 实现若需要新槽位，就在当批回 `CP-` 领号并落地。
  **禁止**预先把它写成一张清单（那等于凭空推测各域会缺什么）。

> 原先的「`errno` → 异常映射表」缺口已由 §5.1 的 `CM-19`…`CM-21` 关闭；
> 「Unicode 数据表来源」已由 `CM-13`／`CM-22`…`CM-24` 关闭。
