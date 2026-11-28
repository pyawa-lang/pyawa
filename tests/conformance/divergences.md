# 已知差异清单（`MS-19` 的落点）

`docs/PLAN-milestones.md` 的 **`MS-19`** 规定：本文件是差异清单的**唯一落点**。
`MS-17` 列出的"必然不同"项**不进本表**（它们压根不参与比对）；本表登记的是
**"会参与比对、但我们已经知道会不同"**的项。

## 为什么必须有它

没有它，差异就只能留在代码注释与提交说明里 —— 那样 harness 无法机械消费，
`MS-10` 的"已知差异"分类也就无从判定，结果是**每次对拍都淹没在假阳性里**。

## 条目格式（**必须**五列齐全）

| 编号 | 差异 | 依据 | 归一规则 | 首次记录 |
|---|---|---|---|---|
| `DIV-n` | 一句话说清"什么和什么不同" | 哪条规格条款或哪个 `DESIGN.md` §13 项 | **harness 可机械实现**的规则 | 提交哈希／参照版本 |

规则：

- **依据必填**：写不出依据的差异，说明它**不是**已知差异，而是 **bug**（`MS-10` 的"新差异"）
- **归一规则必须可实现**：**禁止**"忽略整个 stderr"这类宽泛规则（`MS-19`）
- **`dict` 的迭代序不进本表**：它是**规定序**（3.7+ 为插入序）⇒ **必须**一致
- **`set`／`frozenset` 的迭代序必须进本表**：它**未规定**（依实现哈希布局）⇒ 归一化后比
- 条目**只增不删**：修好后把归一规则改为"已一致"并保留编号（与编号纪律一致）

## 当前条目

> 五列格式（`MS-19`）：编号／差异／依据／归一规则／首次记录。
> `~~ID~~` 是**墓碑**（已修或已移出，`CX-19` 要求保留编号）。

| 编号 | 差异 | 依据 | 归一规则 | 首次记录 |
|---|---|---|---|---|
| `DIV-1` | `set`／`frozenset` 的**迭代序**与参照实现不同 | 迭代序**未规定**（依实现哈希布局）⇒ 本文件规定的必录项 | harness 对 `set`／`frozenset` 的迭代输出**排序后**再比对（或按集合语义比对，不比对顺序） | `425f1a7` · 3.14.4 |
| `DIV-2` | `format(x, 'n')` 的输出与参照实现不同（Pyawa 把 `'n'` 当 `'d'`，不做 locale 分组） | `CM-25`（locale 类差异进差异清单） | harness 把语料里 `'n'` 规格的**期望值**替换为同一对象 `'d'` 规格的结果再比对 | `425f1a7` · 3.14.4 |
| `DIV-8` | `code.co_stacksize` 的**值**与参照实现不同（本层给**保守上界**，参照给精确值） | 规格只说「值栈深度以它为上界、越界必须报错」（`BC-43`）；`SPEC-bytecode.md` §2.4 把它列为**元信息**，**未规定**具体值 | harness 不比对 `co_stacksize` 的值，只比对「被遵守」（`T-BC-14` 的越界报错） | `2395bad` · 3.14.4 |
| `DIV-10` | `signal.signal()` 注册的 handler **永不触发**（记账式、不投递）；`_signal.getsignal` 初值取**记账表**（`SIGINT`→`default_int_handler`、32／33→`None`、其余 `SIG_DFL`），不反映宿主进程继承的信号处置 | `docs/REQUIREMENTS.md`「信号面（2026-11-14）」行（方案 A：不做真实信号投递）／`docs/PLAN-milestones.md` §12.2 | harness 对「注册 handler 后是否被触发」的探针**归一为固定值「不触发」**；对依赖宿主继承处置的 `getsignal`／`signal` 前值探针（如 `SIGPIPE`／`SIGXFSZ` 初值）归一为记账表值（`SIG_DFL`＝0）；其余 `getsignal`／`signal` 返回值照参照比对、不归一 | `9af71464` · 3.14.4 |
| `DIV-11` | **原生递归的两层上限**：默认递归上限对齐参照 **1000**、`setrecursionlimit` **可升**（安全上限 **10000**；参照无此夹）。①「内建→脚本」回调链受 **`PYAWA_NATIVE_DEPTH`** 守卫——**默认 64**，CLI（512 MiB 线程）与对拍 harness（64 MiB 线程）**声明 1000 ⇒ 与参照对齐**，ABI／嵌入／wasm 未声明仍 64；② **round 288（A-0）起**，不经过 `execute` 的内部原生递归（环状/深容器 `==`、深 `hash()`、内建迭代器链）超限 ⇒ **可捕获 `RecursionError`（参照同），不再 abort**（此前 debug/release 都 `stack overflow`） | `PLAN §12.22` S2／S3；`D1`（round 289，`crates/pyawa-core/src/instance.rs` 的 `native_depth_limit`）；内部守卫 round 288（A-0） | harness **不比对**深度阈值；固定用例断言「CLI／对拍 200 层用户 `__eq__`／`__repr__` 成功、1500 层可捕获 `RecursionError`；ABI 默认 64 下无限回调可捕获不崩；环状容器 `==`／深 `hash()`／迭代器链超限同报可捕获 `RecursionError`」；超 `MAX_SAFE_RECURSION_LIMIT` 如实夹住 | 2026-11-19（S2/S3/S4）；2026-11-28 round 288（A-0）／round 289（D1 收窄） |
| `DIV-15` | **编译期 parser/emitter 的栈预算**：本层用**实际栈用量预算**（`PYAWA_PARSE_STACK`；默认 debug 4 MiB／release 512 KiB，CLI／对拍 harness 声明 64 MiB）；参照用堆上的 parser stack（上限约 5k–8k 层）⇒ **可接受的嵌套深度阈值不同**（本层不再撞穿原生栈；超限报参照同文案的 `MemoryError: Parser stack overflowed - Python source too complex to parse`） | `PLAN §12.22`（round 290）；`PYAWA_PARSE_STACK` | harness 不比对嵌套深度阈值；固定用例断言「10 万层嵌套一元／括号 ⇒ 报 parser 溢出消息且不 abort；5000 层一元可正常编译」 | 2026-11-28（round 290） |

### 已修 / 已移出（留墓碑，`CX-19`：墓碑算定义）

> `MS-19` 的适用范围只收两类：① **参照实现未规定**；② `MS-17` 的**实现观测面**。
> 下表是"**已修**"与"**尚未实现（已移出）**"的留档：前者写修法，后者写去哪、何时修。
> 一律保留 `~~ID~~` 墓碑（`CX-19`：墓碑算定义）。

| 编号 | 差异 | 依据 | 处置（怎么修 / 移到哪、何时修） | 首次记录 |
|---|---|---|---|---|
| `~~DIV-3~~` | 绑定方法的 `repr` 缺**限定名**：Pyawa 给 `<bound method m of …>`，参照给 `<bound method C.m of …>` | `BC-4`（`co_qualname`） | **已修**：① 编译器产出 `co_qualname`（模块 ⇒ `<module>`、模块级 `def f` ⇒ `f`、函数里的函数 ⇒ `f.<locals>.g`；48 段语料与参照逐项对拍）；② `method_repr` 改用它；③ 类体方法的 `C.m` 由**类创建钩子**在建类时补写（换一份 code ＋ 新函数，`tests/classes.rs` 验收） | `425f1a7` · 3.14.4 |
| `~~DIV-4~~` | PEP 695 语法当前阶段不可用（`type X = …`／`def f[T]` 要 `typing.TypeAliasType`／`TypeVar`） | 阶段归属：排在 `Lib/typing` 之后（`SPEC-bytecode.md` §10） | 移入 `PLAN-milestones.md` §9.2（P3-14 的 `typing` 批次）；**修的时间点＝`Lib/typing` 可用时** | `425f1a7` · 3.14.4 |
| `~~DIV-5~~` | 映射模式只认 `dict`（未接映射协议） | `SPEC-bytecode.md` §10 的模式匹配族 | 移入 §9.2：**接映射协议时修**（与 `MATCH_MAPPING`／`MATCH_KEYS` 的协议面同批） | `425f1a7` · 3.14.4 |
| `~~DIV-6~~` | 异常对象的 `__traceback__` 是 `None`（参照给 traceback 对象） | `BC-60` 末条要求 traceback 对象；帧／traceback 的补齐排在后面的族 | 移入 §9.2：**`BC-60` 的 traceback 面落地时修** | `a3d0c27` · 3.14.4 |
| `~~DIV-7~~` | 空元组不是单例：`() is ()` 为假（参照为真） | `OM-23` 新增条款：**空元组必须是单例**（`is` 可观测，与"小整数区间不是实现自由"同一理由） | **已修（本笔）**：`new_tuple` 空表回单例表那一份；`BUILD_TUPLE 0`／`*args` 空收集槽／`tuple()` 全部改走它；验收 `tests/singletons.rs` | `a3d0c27` · 3.14.4 |

> 录入口径：**写不出依据的一律不录**（那是 bug，`MS-10` 的"新差异"）。
> ⚠ **适用范围（`MS-19` 新加的口径）：本表只登记两类**——① **参照实现未规定**的行为差异；
> ② `MS-17` 的**实现观测面**。**"尚未实现"不是差异**：它必须留在
> `PLAN-milestones.md` §9 的待做清单里、**不进语料**，让对拍该红就红；
> **可观察语义**的缺口（如 `() is ()` 的身份、`__traceback__` 的存在性）**必须修**。
> ⇒ **已按此重新分诊（本笔）**：主表只留 `DIV-1`／`DIV-2`／`DIV-8`（三类合法差异；`DIV-8` 是后加的，见其行）；
> `DIV-3` 转**待修**（**后来已修**：编译器产出 `co_qualname`，条目已移入下方墓碑表）；
> `DIV-4`…`DIV-7` 移出到 §9.2 的待做清单（其中 `DIV-7` 已修）。
> `dict` 的迭代序**不在**本表（规定序，必须一致）。

（同一个 `import types` 单独跑**过** ✓、在三路并行的对拍里**报** ✗）⇒ 与**模块加载顺序**相关 ✓ ⇒
头号嫌疑是 `emitter.rs` 里那条**闭包重编**路径（发现自由变量后**再编一次**内层单元 ✓） | `MS-10` 第二类 | 不归一（出现即登记 ✓） | 第 212 轮 |


> `DIV-9` 转**待修**（**后来已修**：导入路径一路的 `SlotOutOfRange`／空 cell／`sys.getfilesystemencoding`／`posix.environ`／`posix.cpu_count`／元类调用判据都清了 ✓ —— `import os` 与 `import posixpath` 现与参照**逐字一致** ✓，条目移入墓碑）。

## 平台条件差异（Windows-only，`MS-19` 口径）

这些条目**只在 Windows 上**表现为差异（Linux 上该语料通过）；依据是平台抽象／实现观测面，不是语义缺口。

| 编号 | 差异 | 依据 | 归一规则 | 首次记录 |
|---|---|---|---|---|
| `DIV-12` | **guest 路径的对外表现**：`os.getcwd()` 给 `/D:/…`（guest 名）、错误文案是 `[Errno 2] …`；Windows 参照给宿主 `D:\…`／`[WinError 2]` | `SPEC-capabilities.md`（`CP-21` guest 路径空间：宿主盘符映射成 `/D:/…`）；`DESIGN.md` §7 平台集中点 | **仅 Windows 登记**：Linux 参照无此差异（该语料在 Linux 通过）；不归一 | 2026-11-18 |
| `DIV-14` | **仅 Windows**：guest 的 `socket.AF_INET6`＝**canonical 10**（POSIX 值），Windows CPython 的 `socket.AF_INET6`＝**23**（Winsock 值）⇒ 打印族常量的探针数值不同；`libc`/`Winsock` 的 23 只在 `pyawa-runtime` provider 内映射，不进 guest／线格式 | `SPEC-capabilities.md` `CP-47`（canonical 族号＝10；Winsock 映射归 provider）；`REQUIREMENTS.md` 的 socket 首片 A5 行；Linux 参照同为 10 ⇒ 仅 Windows 有差 | **仅 Windows 登记**，不逐字节比对族常量数值：语料需要该常量时按 `socket.AF_INET6 == _socket.AF_INET6`（模块内一致）断言；IPv6 的地址**文本**／sockaddr **元组形状**照常比对（`::1`／4 元组）。Linux 参照无此差异 | `2c67f3ab`（＃87 登记）· 3.14.4 |
| `DIV-13` | **Windows 上 `asyncio` 的默认事件循环**：round 281 及以前为 selector/引擎后端；**round 282（P4）起恢复为 Proactor**，与 Windows CPython 相同 ⇒ **已一致**（保留本行历史） | `REQUIREMENTS.md`「脚本可移植性承诺」（跨平台一致 > host 平台默认值）／`DESIGN.md` §14.3；P4 依据 `docs/rounds/spikes/2026-11-22-proactor-dataplane-plan.md` | **已一致**（round 282 起默认 Proactor）；harness 仍不比对默认后端类型，固定用例断言「同一 async 脚本 Linux×Windows 同结果」 | `11da3ba9`（＃83 P4 合并）· 3.14.4 |
