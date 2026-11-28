# PLAN — 里程碑与验收

> 规范性文件 · 状态：v0
> 读者：决定"能不能往下走"的人，与实现对拍 harness 的开发者
> **冲突时的优先级**：`REQUIREMENTS.md`（决策）> `SPEC-*.md`（执行细则）> `DESIGN.md`（架构论证）
> 本文件是 `DESIGN.md` §13-10（兼容性验收标准）的落地；里程碑**列表**的出处仍是 `DESIGN.md` §12

规范用语：**必须** / **禁止** / **建议** / **可选**。硬约束编号 **`MS-n`**，测试编号 **`T-MS-n`**。

---

## 1. 范围与依赖

**规定**：里程碑的**判据形式**、对拍 harness 的契约、基线语料、以及"完成"如何判定。

**不规定**：各里程碑要实现的**内容**（在 `SPEC-object-model.md`／`SPEC-bytecode.md`／
`SPEC-capabilities.md` 等规格里）；里程碑的**列表与顺序**（`DESIGN.md` §12）。

| 依赖 | 用途 |
|---|---|
| `DESIGN.md` §12 | 里程碑列表（本文只给它加判据，不重述它） |
| `DESIGN.md` §2.1 | `.py`／`.pyawa` 两种模式——语料**必须**标明模式 |
| `DESIGN.md` §9 | 实现观测面（对拍的不可比项） |
| `DESIGN.md` §13 | 唯一的未决项归口 |

---

## 2. 对拍的参照系与不可比项（本文件唯一"非设计可改"的部分）

- **MS-16** 参照实现**必须**是**具体到补丁版本**的 CPython 3.14.x，并写进每份对拍报告。
  当前基线：**3.14.4**（本机实测基线见 `DESIGN.md` §9）。升版本**必须**是一次显式改动。
- **MS-17** 下列项**必然不同**，**禁止**纳入比对（`DESIGN.md` §9 的"实现观测面"）：
  `sys.implementation`、`sys.version`、`platform.*`、`sysconfig`、`dis`／`opcode` 的输出、
  `gc` 统计、字节码产物（`.pyac`）与启动／常驻内存；
  **`co_positions()` 的列跨度与参照编译器的位置传播细节**（粘性 loc、跳转线程化——`is` 与 `in`
  的收尾跨度甚至互不相同）。
  **边界（不得含糊）**：**行号级必须一致**——`co_lines()` 的行映射、`frame.f_lineno` 与
  `traceback` 的行号是**可观察语义**，**必须**比对；只有**列跨度与传播过程**属本条观测面。
  语料里因此不覆盖的用例**必须逐条写明理由**（夹具按惯例标 `positions_covered = false`）。
- **MS-18** `sys.getrefcount` 的比对**必须**按 §13-5 的裁决调整严格度；
  在 §13-5 定案之前，**禁止**把它列入阻塞性比对（可作为提示项）。
- **MS-19** 上述差异**必须**维护成一份**显式清单**——落点固定为 **`tests/conformance/divergences.md`**；
  每条一行，**必须**含：**编号**｜**差异**｜**依据**（哪条规格条款或哪个 §13 项）｜
  **归一规则**（harness 可机械实现）｜**首次记录**（提交／版本）。**禁止**用"忽略整个 stderr"
  这类宽泛规则代替。
  两处**必须分清**（最常混）：
  - **`dict` 的迭代序是规定序**（3.7+ 为插入序）⇒ **必须**一致，**不得**归一化
  - **`set`／`frozenset` 的迭代序未规定**（依实现哈希布局）⇒ **必须**归一化后比，且**必须**登记
- **`MS-19` 的判据补充（2026-10-18）** ✓：**合法 `.py` 被拒**（词法／语法／核心方法缺失）＝ **新差异** ✗、
  **必须修** ✓；**禁止**归入"已知差异"或"未实现路径" ✗ —— "未实现"只适用于**新特性／未提供模块**（`CM-6`），
  不适用于"**参照能跑而本层跑不了的合法 Python**" ✓。**验收**：`Lib/test` 调查（`tools/libtests_survey.py`）
  里"语法错／核心方法缺失"类必须逐条**修或立据** ✓，不得只记数 ✗。
  **适用范围（不划清就会把 `MS-10` 的三分类架空）**——清单**只**登记两类：
  1. **参照实现未规定**的行为差异（`set` 迭代序；依赖 locale 的 `format` spec 一类）
  2. `MS-17` 已列的**实现观测面**
  **禁止**把"**尚未实现**"登记为差异（功能没做、属性还没有、协议没接）——否则
  "把没做的都登记一遍"就能让对拍全绿，`MS-10` 的"新差异 ＝ 失败"随之失效。
  未实现的项**必须**留在 `PLAN-milestones.md` §9 的待做清单里，且**不进语料**
  （`MS-13` 的语料只覆盖**已实现**的语义）⇒ 对拍该红就红。
  **可观察语义**的缺口（如 `() is ()` 的身份、异常 `__traceback__` 的存在性）**必须修**，
  **不得**登记——`OM-23` 已立此先例（"`is` 可观测 ⇒ 不是实现自由"）。

> `Lib/` 与上游的哈希零差异是**另一条**不变量（`CX-8`），
> **不属于**对拍，二者不要混。

---

### 2.1 升级参照实现的流程（`MS-16` 的落地）

升补丁版本**必须**按顺序做，且**必须**在**同一次改动**里完成全部四步：

1. **重跑探测**：`tools/gen_opcode_tables.py` 与 `tools/gen_opcode_fixture.py` 对新版本重跑；
   产物差异**必须**在提交说明里逐项说明（`BC-38`：数值的唯一出处在实现）。
2. **更新 pin**：本文件 §2 的基线版本、`DESIGN.md` §9 的对照基线、`tools/README.md` 三者同步。
3. **重跑对拍**：`MS-13` 的语料在**新**参照实现上重跑，差异清单（`MS-19`）按结果更新。
4. **复核依据**：差异清单里每项的「依据」**必须**重新核对——参照实现的行为变了，
   原先的归一规则可能已不再成立。

- **MS-22** 上述四步**必须**全做；**禁止**只改版本号。
- **MS-23** 若发现**不兼容差异**（语义不同，且不属于 `MS-17` 的"实现观测面"），
  **必须**先立条目进差异清单，**禁止**静默归入容差。

---

## 3. 术语

- **判据**：一个"通过／不通过"的可执行检查，对应某个里程碑。
- **对拍**：同一段源码在参照实现与 Pyawa 上各跑一次，比对结果。
- **基线语料**：用于对拍的源码集合。
- **差异清单**：MS-19 要求的显式不可比项清单。
- **假阳性**：因参照系差异而非 Pyawa 缺陷导致的比对失败。

---

## 4. 判据的形式规则

- **MS-1** 每个里程碑的"完成"**必须**由一条**可执行**的判据决定，**禁止**只写描述。
- **MS-2** 判据未通过时**禁止**把该里程碑标为完成（`AGENTS.md`「完成度如实」）。
- **MS-3** 判据**必须**能在 CI 或一条命令内跑完；需要人工判断的项**必须**显式标为人工项。
- **MS-4** 里程碑的**降级或挪动**（把某项推迟到后续里程碑）**必须**改 `DESIGN.md` §12 并留下记录，
  **禁止**只在讨论里生效。
- **MS-5** **文档完成度与实现里程碑是两套进度**（前者见 `SPEC-INDEX.md` §1），**禁止**互相替代：
  规格齐全不代表 M1 成立，骨架落地也不代表 M0 完成。

---

## 5. 对拍 harness 契约

- **MS-6** 输入：一段源码或一个文件，**外加模式**（纯 Python 模式／扩展模式）。
  模式缺失时**禁止**猜测——见 §13-13。
- **MS-7** 两侧各跑一次：参照实现（MS-16）与 Pyawa（独立运行时）。
- **MS-8** 比对项**至少**含：退出码、stdout、stderr 的**规范化后**内容、未捕获异常的类型与消息。
- **MS-9** 比对前**必须**规范化：绝对路径、`0x…` 形式的内存地址、耗时数字、临时目录名、
  **裁定（第 270 轮，用户）** ✓：规范化的**范围**就定在这几项 —— ① **路径**（`__file__`／`sys.path` 一类的前缀 ✓）；② **内存地址**（`0x…` ✓）；③ **行尾与末尾换行** ✓。**其余逐字比** ✓；这不算"加宽比对范围"（`MS-11` ✓）—— 它是**同一段脚本**在两侧的表示差异 ✓，不是语义差异 ✓。
  缓存目录名、行尾与末尾换行。
- **MS-10** 结果**必须**分三类，**禁止**合并：**通过** / **已知差异**（在清单里） / **新差异**（失败）。
- **MS-11** 出现假阳性时**必须**修 harness 或把该项进差异清单（附依据，MS-19）；
  **禁止**加宽比对范围到失去意义。
- **MS-12** harness **必须**支持"自检"：两侧都指向参照实现时，结果**必须**全绿。
  自检不过说明 harness 本身坏了，此时任何对拍结论**禁止**采信。
- **MS-13** 基线语料**至少**含：
  ① 自建的最小语义用例（覆盖各规格的验收编号）；
  ② `Lib/` 与 `Lib/test/` 中**以纯 Python 模式可编译**的部分。
  语料**必须**标明模式；扩展模式**没有参照实现** ⇒ 扩展语料**可以**为空，**也可以**放——
  用自己的**对照物**验收（每个特性的对照物写在它自己的条款里）。
  **裁定（第 270 轮，用户）** ✓：**下限**就按 ① 的**自建用例**定 ✓ —— 总数 **≥ 112** ✓ （现 **408** ✓），且 **M2 内容面各设下限**：类 **≥ 15** ✓（现 16 ✓）／异常 **≥ 11** ✓（现 13 ✓）／import **≥ 14** ✓（现 15 ✓）／生成器 **≥ 4** ✓／描述符 **≥ 3** ✓（现 4 ✓）／元类 **≥ 2** ✓（现测值即下限 ✓ ⇒ 只许涨、不许落 ✓）。② 的 `Lib/` 语料**仍暂空** ✓（依赖 M3 ✓）⇒ 已登记 ✓。
**`MS-13` 的边界规则（2026-10-18）** ✓：② 的"**可编译**"边界**不得**用来排除**合法 Python 被拒**的情形 ✗ ——
凡是**合法 `.py`**（词法／语法／核心方法）在 Pyawa 下**被拒或报错**，一律按 `MS-10` 的**新差异**处理、
**必须修** ✓；**不属于**"未实现"豁免（`CM-6` 只管**未提供模块／未接线的扩展面** ✓）。

- **MS-14** 报告**必须**可发布：含参照版本（MS-16）、语料清单与模式、通过／已知差异／新差异的计数、
  以及差异清单的快照。
- **MS-15** 每次对拍**必须**有**超时**；超时**必须**计为**失败**（`MS-10` 的"新差异"），
  **禁止**靠跳过或重试来掩盖。
- **MS-24** **两级对拍，禁止混用**：
  - **程序级**＝`MS-8` 的比对项（退出码／规范化 stdout／stderr／未捕获异常的类型与消息）
    ——这是**唯一**的**语义**对拍手段
  - **编码级**＝`BC-59`（逐指令 `argval`／`argrepr` 对 `dis`）——这是**指令集**的对拍手段
  - **禁止**把"逐指令 trace"当语义对拍手段——指令序列属 `MS-17` 的**实现观测面**，**必然不同**
- **MS-25** **测试总数必须稳定**（约束的是**本仓自己的套件**，与 §5 的对拍 harness 同理）：
  同一提交连跑多次（≥3）测试数与结果**必须**完全一致。数字抖动**必须**按**崩溃／内存损坏**排查
  ——`cargo test` 对 `SIGABRT` 一类信号**不会**打 `FAILED`，它表现为"**总数变少**"而不是失败
  （实例：类体帧少一次 `incref` 导致的随机堆损坏）。

---

## 6. 各里程碑的判据

里程碑的**名称与顺序**见 `DESIGN.md` §12，此处只给判据。编号沿用 `DESIGN.md` 的 `M0`…`M6`。

| 里程碑 | 判据（可执行） |
|---|---|
| **M0** 设计文档 | ① `CX-1`（编号零悬空）；② `CX-2`（状态一致）。两项由 `tests/ci/check.py` 实现，见 §7 |
| **M1** 最小可嵌入内核 | ① 一个 **≤50 行**的 C 程序能：创建实例 → 执行一段脚本 → 注入一个宿主函数 → 取回一个值；② 启动延迟与常驻内存**测量并报告**（`§13-17` 已决为**提示项**，**不作判据**） |
| **M2** 核心语义 | 对拍 harness 在 MS-13 的语料上**无新差异**（已知差异只允许 MS-19 清单内的项） |
| **M3** 标准库第一批 | ① 解锁曲线达到**本里程碑声明的档位**：`CM-14` fan-in 的**前 5 个模块**（`sys`／`itertools`／`time`／`errno`／`builtins`）⇒ `Lib/` 中**可 import 的比例 ≥ 67%**（**分母 ＝ 上游 CPython 3.14 的 `Lib/**/*.py` 全量**（现 **628** 个）——**同步进度不改变分母**，否则比例可被"少同步"抬高；仪器须按上游清单统计（`tools/lib_import_ratio.py`）；`DESIGN.md` §9 的曲线；`CM-15`：该比例**只衡量"能 import"**，**不作验收**）；② **对拍无新差异**（含 `MS-13` ② 的 `Lib/` 语料） |
| **M4** 类型系统 | ① `T-BC-8`／`T-BC-9` 可观测（检查点可见、归责信息完整）；② **渐进保证**：对同一语料增删标注，除类型错误外行为不变（`DESIGN.md` §6） |
| **M5** 能力层正式化 | ① `SPEC-capabilities.md` §11 的 `T-CP-*` 全绿；② 同一程序在真实机器实现与受限实现下**写法不变**；③ 启动延迟与常驻内存**升为阻塞项**（`§13-17`） |
| **M6** 兼容性验收 | MS-14 的报告可发布，且"新差异"为零 |

- **MS-20** 上表任一行判据**未通过**时，该里程碑**禁止**标记为完成（重申 MS-2，因为这是最常被绕过的一处）。
- **MS-21** `M1` 的 C 程序判据**必须**入库为可编译、可运行的示例，而**不是**文档里的代码片段。

> **M6 的要求**（判据在上表、唯一出处；本节只记**落地口径**与**如实缺口**）：
> - **判据（`MS-14`）**：报告可发布——`cargo test -p pyawa-abi --test conformance`（`target/conformance/report.md`，参照 Python 3.14.4）
>   实测 **408/408 通过、已知差异 0、新差异 0**，四要素齐；语料 **408** 条与在册差异 **3** 条（`DIV-1`／`DIV-2`／`DIV-8`）未变；
>   机检硬门 `check.py` 的 `M3-COVERAGE` 同步绿；`PYAWA_DANGLING=1` 双档均绿。
> - **`MS-19` 补充（合法 Python 被拒必须修）**：`P3-38` surrogate epic 收口后，`Lib/test` 体检 **语法错 0／可编译 397/397＝100%**；
>   `support/os_helper.py`（PEP 383 面）`import test.support.os_helper` 通过、`multibytecodec_support.py` 同步且可导入；
>   `\ud800`/`\uDC80` 字面量、`surrogateescape`／`surrogatepass`、fs 路径/argv/readlink/输出边界均落地（证据见台账 round 22–30）。
> - **兼容边界声明刷新（M6 收口实测）**：判据① **477/628＝76.0%**（≥67% 阈值 ✓）；`Lib/` 逐字节同步 **739** 个文件（CX-8 `--check` 全一致，
>   其中 169 个按第二来源根）；`Lib/test` 体检 **可编译 100%／跑通 18/397／语法错 0／未实现 96／运行期错 271／超时 12**；
>   判据③ 体量沿用 M5 拍板阈值（本轮未重测，如实记）。
> - **如实缺口（不阻塞 M6 三条判据，留队列）**：未实现形态 `\N{…}` 15／`except*` 7／async 推导式 4／PEP 695 `*Ts`·`**P` 4／下标删除 5／
>   `import *` 4；`socket` I/O 17；缺模块（`zipimport`／`urllib.request`／`pyexpat`／`fork_wait` 等）；3 个编译器 cell/free 表 panic 用例；
>   `FAILED(errors=1)` 泛化 16；`tb_next` 链；fs/io 其余（`os.pipe`／标准流 `dup`／`sys.prefix`）；`P3-38` 的 4 条有损输出通道边界
>   （`%s` 嵌入/宽度、混合 `str.format`、`str.__str__` 直调、`str(e)`）。以上均**未登记为差异**、**未用 KNOWN 绕过**。

> **M3 的要求**（本节声明——判据的唯一出处）：
> - **交付物**：**引入 `Lib/`**（逐字同步，`DESIGN.md` §9）＋ **`import` 运行期可用**（`P3-12`：
>   finder／loader 落在 Python 层、I/O 走能力层、`.pyac` 容器与陈旧判定）；第一批模块按 `CM-14`
>   的 fan-in 顺序（**前 5 个**：`sys`／`itertools`／`time`／`errno`／`builtins`）——**我们自己实现**
>   的那部分**必须**各有逐模块合约（`CM-4`；逐字同步的 `Lib/` 模块**不需要**）。
> - **判据 ①（覆盖面）**：`DESIGN.md` §9 的解锁曲线 ⇒ **可 import 的比例 ≥ 67%**（**分母＝上游 `Lib/**/*.py` 全量**，现 **628** 个；**同步进度不改变分母**。**已同步子集**（现 16 个文件）的通过率**另作进度指标**，**不作判据**）；该比例
>   **只衡量"能 import"**、**不衡量语义正确**，且**禁止**当作完成度或验收（`CM-15`）。
> - **判据 ②（语义）**：**对拍无新差异**——语料含 `MS-13` ② 的 **`Lib/` 语料**（扩展模式语料仍**可以**为空）。
> - **前置**：`§9.4` 第 4 项（`fs` 域／`print`／最小 CLI）与第 5 项（`P3-12`）收口；编译器对 `Lib/`
>   的覆盖（`BC-` 面：`_bootstrap.py`／`site.py` 一类的语句面）。
> - **不算完成的三种情形**：`Lib/` 语料进不了对拍；解锁比例靠"未实现也报能 import"凑数（`CM-15`）；
>   把能力缺口登记成差异（`MS-19` 的适用范围：**能力缺口必须修**）。

> **M4 的要求**（判据在上表、唯一出处；本节只记**落地口径**）：
> - **范围**：扩展特性 **A 批（类型链）**——`TS-46`（`T?`）→ `TS-47`＋`TS-48`（变量标注即静态类型／平台类型）
>   → `TS-50`（收束）；**不改语法**的走 `TS-5`（`.py` 下合法且惰性），改语法的 `?` 只在扩展模式（`BC-15`）。
> - **两层检查**：**编译期尽力**（`crates/pyawa-core/src/compile/typecheck.rs`；静态可判定 ⇒
>   `CompileError::TypeAssignment`，ABI 侧 `PA_ERR_SYNTAX`）＋**运行期兜住**（`CHECK_ASSIGNMENT`／`BC-65`
>   ⇒ `TypeAssignmentError`；边界 `CHECK_BOUNDARY_*`／`BC-23` ⇒ `TypeBoundaryError`）。
> - **判据 ①（`T-BC-8`／`T-BC-9` 可观测）**：可执行归档 `crates/pyawa-core/tests/m4_acceptance.rs`；
>   **判据 ②（渐进保证 `T-TS-1`）**：`crates/pyawa-core/tests/gradual_guarantee.rs`。
> - **如实缺口（不阻塞判据）**：`BC-25`①"两侧皆标注处禁止发射"需跨模块静态信息 ⇒ **未做**（现按"该函数带标注
>   保守发射"）；`TS-50` 部分形态（`None is x` 反序／`== None`／`and`/`or` 复合守卫／`not (is None)`／
>   `isinstance`／`match`）未覆盖；类体不查、嵌套函数读外层局部不建模、形参注解不登记（形参体内重赋值不检）。

> **M5 的要求**（判据在上表、唯一出处；本节只记**落地口径**）：
> - **判据①（`T-CP-*` 全绿）**：可执行归档 `crates/pyawa-abi/tests/capability_acceptance.rs`（11 例）／
>   `crates/pyawa-runtime/tests/two_providers.rs`（7 例）／`t_cp10_cancel.rs`（4 例）／`select_dup2_shapes.rs`（1 例）／
>   `crates/pyawa-abi/tests/capability_registration.rs`（6 例）；静态两条（T-CP-6／13）由 `tests/ci/check.py` 的 `T-CX-4` 承担。
> - **判据②（两 provider 写法不变）**：`two_providers.rs`——同一脚本**逐字节相同**，真实 `PosixFs` vs 受限
>   `RestrictedFs`，输出按 `MS-9` 归一后完全一致。
> - **判据③（升为阻塞项，阈值经用户拍板 2026-10-18）**：阻塞阈值表与三档基线数字的**唯一出处**＝
>   `DESIGN.md` §9 的「Pyawa（M5 实测基线）」末节「判据③ 的阻塞阈值」；收口时实测全部在阈值内。
> - **如实缺口（不阻塞判据，留队列）**：`CP-22` 的 `os.getcwd/chdir` stdlib 面未接（cwd 核心状态已落、初值是过渡口径）；
>   `CP-21` 的"逐段 Dir 能力链"v1 为简化档（guest 绝对名＋provider 根映射）；`T-CP-10` 未覆盖无 timeout 的阻塞
>   `recv/send` 与 Python 层 `KeyboardInterrupt` 投递（本层 `pa_interrupt` 是宿主级中断，如实差异）；
>   `AB-43` 的消费方有界读取待做；`dup/dup2` 到 0/1/2 未接；C 面 `_codecs_hk`／iso2022／gb18030／hz 未做。

> **兼容边界声明**（**随时效更新**；修订点记在末行）：
> - **`Lib/test` 体检（报告级，**非判据** ✗）**：`tools/libtests_survey.py` 对上游 3.14.4 的 `Lib/test` 给出
>   **可编译率／跑通率／头号缺口榜**；**两个比例不得当验收或完成度** ✗（`CM-15` 同理 ✓），只作**趋势与缺口发现** ✓；
>   **阶段边界时把这两个数记入本块** ✓（否则"兼容性验收"名不副实 ✗）。
>   **实测（2026-10-31，release 档，397 个顶层 `test_*.py`）**：可编译 **397/397＝100%** ✓、跑通 **18/397＝4.5%** ✗；
>   语法错 **0** ✓；未实现 **96** ✗／运行期错 **271** ✗（其中 **~89 为断言失败＝真语义差异** ✗）／超时 **12** ✗
>   （`test_dict`／`test_set`／`test_sort` 等基础测试超时 ⇒ **疑似 hang**，待单独钉 ✗）。
> - **保证层（语义零差异）**：**单线程** ＋ 纯 Python 源 ＋ 只用**已声明的 C 模块面** ⇒ 以 `MS-13` 的语料为准
>   （现 **408** 例，含 `Lib/` 215（其中 `Lib/test/` 50）＋自建 193；来源列格式 A 见 `tests/conformance/corpus/manifest.tsv`）＋ `MS-19` 的在册差异（现 **3** 条：`DIV-1`／`DIV-2`／`DIV-8`）。
> - **已兼容面（当前实况）**：`Lib/` **逐字节同步 739** 个文件（`--check` 全一致 ✓；判据① 的分母仍是上游顶层 **628**）；
>   可 import 比例 **477/628（76.0%）** ⇒ **判据① 已达标** ✓（阈值 421／67%；复核用
>   `tools/lib_import_ratio.py`——该仪器**单次读数有 ±1 噪声**，判涨跌要多跑取众数 ✓）；C ABI `AB-1`…`AB-64` 可用；C 模块面按 `CM-14` 的 fan-in 增长，
>   未提供的走 `CM-6`（`ImportError`，如实）。**能力层**已正式化（M5）：`T-CP-*` 13/13、两 provider 写法不变、体量阻塞阈值已拍板
>   （`DESIGN.md` §9 的 M5 小节）。**兼容性验收**已达成（M6）：`MS-14` 报告 408/408、新差异 0；`MS-19` 补充（合法 Python 被拒）已消除
>   （`Lib/test` 语法错 0／可编译 100%，surrogate epic `P3-38` 收口）。
> - **未兼容面（明确不承诺）**：**多线程程序的语义**（含 GIL 原子性／线程调度；`threading` **允许跑**）·
>   **C 扩展加载** · free-threading · `.pyc`／`.pyac` 跨实现（自有格式）· `_interpreters` ·
>   `Lib/` 未同步的 **84** 个文件 · **扩展模式**（`.pyawa` 无参照 ⇒ 只按各特性条款自对照）·
>   **未被语料覆盖、也未列入 `MS-19` 的边角语义**（一律**无保证**）。
> - **修订规则**：**阶段边界**（`--no-ff` 收录进 `main`）**必须**更新本块；**判据① 或语料数变动时也应刷新** ✓；
>   每次修订**追加一行**（保留历史）。
> - **修订**：`dev` @ `92494e7`（2026-10-06）—— 首次成文。
> - **修订**：`dev` @ `90b4040`（2026-10-07）—— 刷新同步数与判据①（`Lib/` 283→294、可 import 145→184）；
> - **修订**：`dev` @ `3e4e7a4`（2026-10-18）—— **M3 阶段边界**（tag `m3`）：判据① 451/628＝71.8% 达标、判据② 裁定达成（语料 407 条／Lib 覆盖 150 模块／M3-COVERAGE 硬门）；补 CM-4 合约 23 节（§5.2.10–§5.2.32）；
>   语料数与在册差异未变 ✓。
> - **修订**：`dev` @ `59dfbb3`（2026-10-18）—— **M4 阶段边界**（tag `m4`）：扩展特性 A 批（类型链
>   `TS-46`／`TS-47`＋`TS-48`／`TS-50`）落地；判据① `T-BC-8`／`T-BC-9` 可观测（归档
>   `crates/pyawa-core/tests/m4_acceptance.rs`）、判据② 渐进保证 `T-TS-1`（`crates/pyawa-core/tests/gradual_guarantee.rs`）；
>   语料 **407** 条与在册差异 **3** 条未变；`BC-25`①"两侧皆标注处禁止发射"（需跨模块静态信息）**未做**、如实记。
> - **修订**：`dev` @ `4e57e32`（2026-10-17）—— **判据① 达标**（451/628 ＝ 71.8%，阈值 421）；`Lib/` 同步 523/628；
>   语料 235（**判据② 仍未裁定** ✗）；在册差异仍 3 条 ✓。
> - **修订**：`dev` @ `42cadcb`（2026-10-18）—— **M5 阶段边界**（tag `m5`）：能力层正式化——`T-CP-*` **13/13 全绿**、
>   两 provider **写法不变**（`crates/pyawa-runtime/tests/two_providers.rs`）、判据③ 的**阻塞阈值经用户拍板**
>   （三档实测数字与阈值的唯一出处＝`DESIGN.md` §9 的「Pyawa（M5 实测基线）」末节）；可 import 比例刷新为
>   **466/628（74.2%）**；`Lib/` 同步 523/628；语料 **407** 条与在册差异 **3** 条未变；如实缺口见 §6 的「M5 的要求」。
> - **修订**：`dev` @ `d6be4ea`（2026-10-24）—— **M6 开局（round 14）**：`P3-32`／`P3-33` 落地；`Lib/test` 体检**首次非零**（可编译 311/397＝78.3%、**跑通 5/397＝1.3%**）；判据① **466/628（74.2%）未变**；`Lib/` 同步 **544/628**（同步子集 450/544＝82.7%）；语料 **407** 条与在册差异 **3** 条未变。
> - **修订**：`dev` @ `faf685a`（2026-10-31）—— **M6 阶段边界**（tag `m6`）：兼容性验收达成——`MS-14` 报告 **407/407 通过、已知差异 0、新差异 0**；
>   `MS-19` 判据补充随 `P3-38` surrogate epic 收口（S1…S3e 全落地；`Lib/test` 语法错 **0**／可编译 **397/397＝100%**、
>   `support.os_helper` 可导入、PEP 383 codecs/fs/argv/readlink/输出边界落地）；判据① **477/628（76.0%）**；`Lib/` 同步 **739** 文件（`--check` 全一致）；
>   语料 **408** 条与在册差异 **3** 条未变；如实缺口见 §6 的「M6 的要求」。
> - **修订**：`dev` @ `0369d101`（2026-11-15）—— 语料 **407 → 408**（`MS-14` 报告 **408/408**、已知/新差异 0；
>   组成＝自建 **193** ＋ `Lib/` **215**（含 `Lib/test/` **50**））；其余数字未变。

---

## 7. 验收（本文件自身的可测性质）

| 编号 | 测试 |
|---|---|
| ~~`T-MS-1`~~ | 已作废 → **见 `T-CX-1`**（编号一致性；唯一定义处在 `docs/CONSTRAINTS.md` §5，编号保留不复用） |
| `T-MS-2` | 每个被标为完成的里程碑，都能指出对应的可执行判据且**当前**通过 |
| `T-MS-3` | harness 自检：两侧同为参照实现时全绿（`MS-12`） |
| `T-MS-4` | 差异清单中每一项都有依据与归一规则（`MS-19`） |
| ~~`T-MS-5`~~ | 已作废 → **见 `T-CX-2`**（状态一致性；唯一定义处在 `docs/CONSTRAINTS.md` §5，编号保留不复用） |

> ⚠ `T-CX-1`／`T-CX-2` 是**现在就能落地**的（不需要 `Lib/`，不需要 VM），
> 与 `CX-3`／`CX-4` 同属"不该往后拖"的一类。
> 它们已由 `tests/ci/check.py` 实现（一条命令跑完，`--list` 列出检查项）——
> M0 判据①／② 到此可执行；`T-CX-3`…`T-CX-6` 的实现状态见 `tests/ci/README.md` 的状态账本。

---

## 8. 未决（引用 `DESIGN.md` §13）

- **§13-10** —— 本文档是它的落地。**容差细节已裁定**（第 270 轮 ✓：见 `MS-9`）；仍开放的只剩实现侧未接的部分 ✓
  与基线语料的**范围下限**（MS-13 的②要覆盖到哪一档）
- **§13-5** —— immortal 与 `sys.getrefcount` 对齐程度 ⇒ 决定 MS-18 何时从"提示项"升为"阻塞项"
- **§13-13** —— REPL／`-c`／stdin／`compile()` 的默认模式 ⇒ 决定 MS-6 缺模式时能否有默认
- ~~**§13-17**~~ —— **已决**：M1 为**提示项**、**M5 起升为阻塞项**（阈值经用户拍板 2026-10-18；
  阈值表与基线数字的唯一出处＝`DESIGN.md` §9 的「Pyawa（M5 实测基线）」末节）
- ~~**§13-12**~~ —— **已决**：机制已定、特性逐个准入（入口见 `DESIGN.md` §2.1 的扩展特性索引）
- **§13-7** —— Unicode 版本与数据表来源 ⇒ 决定编码类语料的边界

---

## 9. 任务队列（当前一批）

> **本节非规范**：它是**工作队列**，不是判据。每条的依据都在各自规格里，这里只写"做什么、卡在谁身上"。
> 随进度更新；做完的条目移入 §9.1，**不要留旧条目**。**续做状态（HEAD／工作区／下一条命令）看单页
> `NEXT.md`** ✓ —— 本节只留队列与指示，**不留快照**（`AGENTS.md` 的"一处真相"）。

### 9.1 已完成（留档，便于判断进度）

> **已完成项的留档已移到 `docs/ROUNDS.md`（索引）与 `docs/rounds/` 各卷**（非规范台账）；本节保留节号供引用 ✓

### 9.2 待做（按依赖排序）

| # | 事项 | 规范依据 | 卡在哪 |
|---|---|---|---|
| **P1-6** | 字节码 **§10 起步指令集与执行器** ＋ §11 下降规则（帧布局与码元解码**已完成**）。**大部分已收口**（第 215 轮按实情标注：下面只剩列出的那一项）。**已收口（第 276 轮复验）**：本节原先只剩的那一项**类对象上的属性读**已接线——`class C: v = 5` 之后 `C.v` 与参照同为 `5`（`class_attr_read.py` 在语料清单里且为绿） | `BC-33`…`BC-36`、`BC-42`…`BC-54` | 无 |
| ~~**P1-7**~~ | ~~实例生命周期的对外出口（create／destroy／interrupt）~~ **已完成**（第 215 轮核实：`crates/pyawa-abi/tests/abi.rs` 的 `create_and_destroy_a_compatible_instance` 覆盖建／销与 `pa_interrupt` 的三态（活状态 `PA_OK`、销毁后 `PA_ERR_ABI`、`NULL` ⇒ `PA_ERR_INVALID`）；中断后"执行类函数随即返回"在 `crates/pyawa-core/tests/lifecycle.rs` 覆盖） | `T-OM-4` | 无 |
| ~~**P1-8**~~ | ~~**C ABI 最小闭环**（含 `MS-21` 的 ≤50 行示例与 `pa_sig` 内联签名，`AB-51`…`AB-54`）~~ **已完成**（第 215 轮核实：`T-AB-1` 绿＝真编译真运行、`examples/m1.c` **45 行**（判据 ≤50）、`pa_sig` 在 `pa.h` 与 `pa_register`／宿主类型注册里在位） | `AB-1`…`AB-54` 与 §15 函数清单 | **无**（`§13-2`／`§13-3`／`§13-16` 均已决） |
| ~~**P1-9**~~ | ~~宿主类型与继承：`OM-13` C3（含宿主类型参与 MRO）＋ `OM-14` 子类分派槽位与实例字典~~ **已完成**（实例字典挂载点＝`Header` 的 `dict` 那一格，取舍见 `crates/pyawa-core/src/header.rs`） | `OM-13`／`OM-14`、`AB-37` | 无 |
| **P1-10** | **源码 → 字节码的编译器**（`BC-14`…`BC-18` 的契约；§11 的族级映射）——**大部分已收口**（第 215 轮按实情标注）：**`P3-12` 的 `IM-31` 与 `P3-13` 都依赖它**。**第 212 轮**：**二元／一元运算符面**（`+ - * / // % **`、`& | ^ << >>`、一元 `+ - ~`）；**第 213 轮**：**括号（分组）**、**元组字面量**（`()`／`(a, b)`／裸 `a, b`）、**下标读**（`a[i]`，含链式与 `f()[0]`）与**下标写**（`a[i] = v`）——都按参照**指令流＋常量池逐字节**对拍。**第 214 轮**：**切片**（常量界进常量池、两段非常量走 `BINARY_SLICE`、三段走 `BUILD_SLICE 3`；执行器与 **list 切片写**一并接线）；**第 217 轮**：**`not`／`is`／`is not`／`in`／`not in`**（`not` 在值／条件两种上下文有四种发射形态；`IS_OP`／`CONTAINS_OP` 的 0/1）；**第 218 轮**：**`and`／`or`**（值上下文＋条件上下文＋嵌套融合；折叠取「决定结果的那个操作数」的位置）；**第 219 轮**：**增强赋值 `+=` 一族**（名字／属性／下标三种目标的栈序，`list` 的 `+=` 就地 extend，下标目标只求值一次）。**未接线**：**第 220 轮**：**`elif`**（与「`else:` 里套 `if`」同形；顺手修掉链尾隐式 return 多补一条的 bug）；**第 221 轮**：**统一后缀链**（`.`／`(`／`[` 任意串联 ⇒ `a[0].b` 一族）与**矩阵乘 `@`／`@=`**（运行期对未支持类型如实报参照实测的 `TypeError`）；**第 222 轮**：**链式赋值目标**（`items[0].v = …`，目标链统一成一个解析器）＋ **`pass`**（实测**不产生指令**，但位置要留给收尾）＋**空模块**可编。**第 223 轮**：**`break`／`continue`**——`continue` 的指令流已**逐字节对上**；`break` 因参照的**块结构**（把循环后的代码复制到 break 路径）**语义一致、布局不同**（那两条形状不进编译夹具，由语料守着）；顺带补两处流分析（终止语句之后的死代码不发、体必然终止时循环尾回跳不发）。**第 225 轮**：修**夹具生成器的位置配对错位**（自认的测量 bug：`co_positions()` 按码元含 cache）
＋ 覆盖标志改为**实测驱动**（`tools/compile-positions-census.tsv`）＋ 循环回跳位置改取循环体最后一条。
**第 224 轮**：**`try`／`except`**（裸／带类型／多处理块／`as` 名）——编译器发射 ＋ `BC-54` 异常表**第一次真正到达运行期**（`CompiledUnit` 此前没有该字段、`instantiate` 硬编码空表）；`else`／`finally` 仍未接线。同 `break` 的口径：**语义一致、布局不同**（不进编译夹具，由语料 `try_except.py` 与 `exceptions.rs` 的端到端测试守）。**第 232 轮**：**`lambda`**（嵌套单元 `<lambda>`；与 `def` 共用 `emit_function_object`；顺带补 `CO_NESTED`）；
**第 233 轮**：布尔嵌套的**骨架跨度**按「拥有该操作数的那个节点」逐层取（`a and b or c` 等已转绿）⇒ 位置可比 233 → 234、位置差异 139 → 19。
**第 276 轮逐条复验**（走 ABI 路径；探针只打印**字符串**，以免撞上"`print` 只收 `str`"这条**另外**的缺口）：`import` ✓／推导式 ✓／f-string ✓／`try` 的 `else`＋`finally` ✓／`with` 的**多项**形式 ✓／循环体内 `return` 的清理 ✓（运行结果与参照一致；布局是否逐字节一致另属夹具）⇒ **本节原先那一串只剩一处真未接线**：`set`／`dict`／`bytearray` 的**就地**运算（`|=` 一族；执行器如实报"尚未接线"）。**`try`／`except` 的侦察结论（第 222 轮）**：**运行期已就绪**（`PUSH_EXC_INFO`／`CHECK_EXC_MATCH`／`POP_EXCEPT`／`RERAISE` 都在，异常表也已被调度使用）⇒ 基本是**纯编译器**工作：`try:` 先发 `NOP`、体、`PUSH_EXC_INFO`、处理块（裸 `except` 是 `POP_TOP`；带类型是 `LOAD_NAME 类型; CHECK_EXC_MATCH; POP_JUMP_IF_FALSE; NOT_TAKEN; POP_TOP`）、`POP_EXCEPT`、收尾，最后是清理块（`RERAISE 0`／`COPY 3; POP_EXCEPT; RERAISE 1`）＋**异常表**（每条 4 个 6-bit varint：起点、长度、目标、`depth<<1|lasti`）**`break` 的已知难点**（第 221 轮实测量清）：参照把**循环后的代码复制到 break 路径**（`for` 的 break ＝ `POP_TOP` ＋ 落进那份副本；`while` 的 break ＝ `NOP` ＋ 落进副本），循环正常退出那条路径另有自己的一份 ⇒ 要逐字节对上，必须先有**块结构模型**（与上一条粘性 loc 同源）；`continue` 简单（`JUMP_BACKWARD` 回循环起点）
| **P1-11** | **任意精度整数**（`TS-45`）：大整数对象 ＋ 四则／整除／取模／幂 ＋ 比较 ＋ `hash` ＋ `repr`／`str`（含 `int`↔`str` 的 **4300 位上限**）＋ 与 `float` 互转。**进行中**：第 199 轮落**纯算术核心**（`src/bigint.rs`，参照夹具对拍）；第 200 轮**接线**（载荷两态／四则／比较／真值／`repr`）；第 201 轮落 **`TS-45` ①的输入方向**（`sys.get_/set_int_max_str_digits`）；第 202 轮落 **`OM-11` 扩**（`repr`／`str` 槽能表达失败，顺带修掉"覆写抛异常被吞"）＋ **①的输出方向**；第 203 轮落**位运算／位移**与 **`int`↔`float`**；第 204 轮落**大整数的 `__format__`**（整数码走任意精度，位数上限**管**十进制码、**不管**十六进制码——两条实测），顺带修掉格式化模块两处早先偏差（`0` 配显式 `=` 的零填充、`g`／`G` 的精度口径）。顺带修掉 `//`／`%` 用 `div_euclid` 的**负除数**真 bug。**第 276 轮复验**：**ABI 侧的大整数通道已落地** ✓（`AB-62`：`pa_pushintstring`（`lib.rs:1106`）＋ `pa_tointstring`（`lib.rs:1328`）；`crates/pyawa-abi/tests/abi.rs` 的 `the_integer_string_bridge_covers_all_integers`／`the_integer_string_bridge_reports_failures_like_the_spec` 两条绿；harness 的整数探针已改走它）、**编译器已能发** `*`／`//`／`%`／`**` ✓（见 §9.4"编译器覆盖面"注）⇒ **仍真未接线**：`float('<串>')` 的解析（实测报 `float_new：这个实参形态还没接线`）；**另记一条相邻缺口**：内建 `hash` 未接线（`hash(...)` ⇒ `NameError`） | `TS-45`、`OM-23` | **无**（已裁：**现在就做**） |
| **P1-12** | **`bytes` 类型面**（`TS-42` 的"M2 之后、M3 之前"档）：载荷 ＋ 类型对象 ＋ 构造路径（含**字面量**）＋ 索引／切片／迭代／哈希／比较 ＋ 方法面（以 oracle 逐批）。**进行中**：第 205 轮落**第一刀**（`BytesObject`／类型对象／构造／`len`／整数索引／迭代／等值／字典序，七条失败消息照实测）；第 206 轮落**字面量**——编译器认 `b'…'`（词法层解转义、非 ASCII 与坏 `\x` 照实测报错、AST／常量池／`.pyac` 常量编码同步）＋ `b'ab' + b'cd'` 的编译期折叠与运行期相加，**指令流与常量池逐字节对拍**；第 207 轮落**方法面第一批** 12 个（`hex`／`decode`／`startswith`／`endswith`／`find`／`count`／`replace`／`upper`／`lower`／`strip`／`split`／`join`，20 条实测用例；四条易静默写错的逐条去量过）；第 208 轮落 **`slice` 类型 ＋ 切片**（`bytes`／`str`／`list`／`tuple` 四族共用 CPython 的 `slice.indices()` 口径，16 种切法 × 四族逐条对拍；切片语义**一处真相**，夹具 `tools/gen_slice_fixture.py`）。第 210 轮再落**方法面第二批** 12 个（`rfind`／`index`／`rindex`／`removeprefix`／`removesuffix`／`lstrip`／`rstrip`／`zfill`／`splitlines`／`isdigit`／`isspace`／`__contains__`）＋ **`in` 的两条路**（`CONTAINS_OP` 与 `__contains__` 都接）。**第 276 轮复验**：切片**写** ✓（`l[1:3] = [9]` 与参照一致）、bytes 作 `dict`／`set` 的**键** ✓（走哈希槽，成员判定与参照一致）⇒ **仍真未接线**：`bytes.__hash__` 这个**显式 dunder**（`b"a".__hash__()` ⇒ `AttributeError`；通用 `hash` 内建也没接）、方法面剩余（`translate`／`center`／`title`／`swapcase`／`casefold`…）、~~**ABI 的 bytes 通道**（对拍 harness 现在只能把它标成 `<unrenderable>`）~~ **已完成**（标签 `PA_TBYTES` ＋ `pa_tobytes` ✓；harness 渲染成 `<bytes:十六进制>`，跨语言可比改由**探针**承担，例如 `x.hex()`） | `TS-41`／`TS-42`、`CM-27`、`BC-4` | **无**（已裁：做，**排在 `P1-11` 之后**） |
| ~~**P1-13**~~ | **`pa_options` 过界通道**（`AB-61`）：`pa.h` 定尺寸标记结构（`size` ＋ **检查档位** ＋ **优化级**）；`pa_exec_string`／`pa_exec_file` 收 `const pa_options *`（**可 `NULL`**）；档位／优化级进产物决定要素（`IM-21`）与 `.pyac` 头部（`IM-19`）。**已落地**：`pa_options` ＋ 两条执行签名；有界读（`AB-43` 惯例，`NULL` ⇒ 浅层＋默认优化级）；档位进发射（深层 ⇒ `BC-25`② 的边界检查，验收 `crates/pyawa-abi/tests/abi.rs`）；优化级进 `compile` 的显式输入集（本层没有优化器 ⇒ 暂不改发射）。**归 `P3-12` 的那一半**：从源码到 `.pyac` 的驱动链（头部字段早已就位） | `AB-7`／`AB-61`、`TS-31`、`IM-19`／`IM-21` | **无**（已裁：改法 **A**） |
| **P3-12** | `.pyac` 与 import 钩子 | `IM-18`…`IM-34` | **无**（`§13-15` 已决）；**编译器已就位**（`P1-10`）。**已落地**：产物容器（`IM-18`…`IM-21`，含 `TS-31` 的**检查档位**字段与陈旧判定）＋ **代码段＝编译产物的确定性序列化**（`IM-31` 的 Rust 层这一半：编译、编解码、两步陈旧判定）＋ **运行期的 `fromlist` 装载**（第 279 轮：照 `_handle_fromlist` 的口径把 `<模块>.<名字>` 当子模块载入并挂成父包属性 ✓）＋ **`_imp` 的引导面**（第 280 轮：`is_builtin` **按模块表**三态 ✓、`is_frozen`（没有冻结模块 ⇒ `False` ✓）、`extension_suffixes`（不支持扩展 ⇒ **空表** ✓））＋ **`_thread` 最小面**（第 280 轮：`RLock`／`allocate_lock`／`get_ident` ✓；`sys.builtin_module_names` 据实列全 ✓；用户裁定 A＝**VM 侧** ✓）⇒ **`import importlib` 已经跑通** ✓（`_bootstrap` 与 `_bootstrap_external` 都能加载 ✓）。**未落地**：finder（`IM-30`，落在 Python 层 —— `_bootstrap_external` 已加载，**让它去找模块**是下一件 ✓）、`_imp` 的 `create_builtin`／`exec_builtin`／`acquire_lock` 一族、能力层的 I/O（`IM-15`）、冻结（`IM-27`／`IM-33`／`IM-34`） |
| **P3-13** | 类型检查器与边界检查 | `TS-12`／`TS-13`、`TS-28`…`TS-39` | **无**（`§13-9` 已决）。**已落地**：边界检查的两条专有指令 `BC-23`（`CHECK_BOUNDARY_IN`／`OUT`：表内 232／233、`has_arg` 真、`stack_effect` 0、执行语义 ＋ `TypeBoundaryError` ＋ `TS-28`…`TS-30` 的相容判定 ＋ `TS-13` 的浅层；验收 `tests/boundary.rs`）。**已落地（`TS-31` 的输入通道）**：检查档位＝**编译期参数**（`CheckTier`：浅层默认／深层）⇒ `.pyac` 头部新增"检查档位"（`IM-19`，排优化级之后，`HEADER_LEN` 38→39）、陈旧判定比档位（`IM-20` ②）、产物五要素（`IM-21`）；`pyac` 的黄金字节用例随之更新。**已落地（本阶段新增）**：**注解的解析与边界检查发射**——`def f(x: int) -> int:` 可解析，扩展模式＋深层档位＋带注解时发 `CHECK_BOUNDARY_IN`／`OUT`（`BC-25`②＋`TS-31`），`list[int]` ⇒ 复合标签 `(list, int)`；端到端验收（编译→实例化→调用，坏实参 ⇒ `TypeBoundaryError`）。**已落地（本阶段新增）**：`SET_FUNCTION_ATTRIBUTE` 的 bit4 `annotate`（PEP 649 的执行器那半） ＋ 编译器的 `__annotate__` 发射（带注解的 `def` 与参照**逐字节**一致，端到端可调用） ＋ `LOAD_COMMON_CONSTANT`（实测的固定表 0…4）＋ **文档字符串**（函数／模块，逐字节一致；顺带支持双引号字符串）＋ **形参全族**（位置默认值／`*args`／`**kw`／仅关键字／**仅位置 `/`**，都与参照逐字节一致）＋ `co_stacksize` 的**保守上界**（此前是占位常数， 带注解的 `def` 一跑就 `StackOverflow`；差异登记为 `DIV-8`）。 **已落地（本轮）**：函数对象属性通道的**数据属性**（`__name__`／`__qualname__`／`__code__`／ `__defaults__`／`__kwdefaults__`／`__globals__`／`__annotate__`）。 **已落地（本阶段新增，续）**：`f.__annotations__`（惰性调用 `__annotate__(1)` ＋ 缓存 ⇒ 同一对象）、`f.__doc__`（函数文档串进常量 0 ＋ `co_flags` 的 `0x4000000` 判据）。**未落地**：合成注解单元的**位置表**（要給注解记 span；语料里如实标成「位置表未对齐」）、 `f.__annotations__` 的**可写**（本层只读）、`BC-25`①的"只在标注／未标注交界处发射"（要跨模块静态信息 ⇒ 现按带标注保守发射）、**PEP 649** 的注解对象一族（`__annotate__`／`SET_FUNCTION_ATTRIBUTE`）、`TS-32`…`TS-39` 的编译期检查器与覆盖率报告。**深层档位的执行器那半已落地**：标签带内层（`(list, int)`）⇒ 对 `list`／`tuple` 元素**递归**比（`tests/boundary.rs`），裸标签仍是浅层 ⇒ `TS-13` 的"默认浅层"是代码生成的结果。**已落地（M4 阶段）**：扩展特性的类型链——`TS-46`（`T?`）／`TS-47`＋`TS-48`（变量标注即静态类型／平台类型）／`TS-50`（收束）：编译期静态检查器 `crates/pyawa-core/src/compile/typecheck.rs`（`CompileError::TypeAssignment`）＋运行期赋值检查 `CHECK_ASSIGNMENT`（`BC-65`）与 union 标签（`BC-24`）；验收 `tests/m4_acceptance.rs`／`tests/gradual_guarantee.rs` 等。**仍未落地**：`BC-25`①（跨模块静态信息）、`TS-32`…`TS-39` 的覆盖率报告 |
| **P3-14** | 逐模块合约与 stdlib 实现（分批）；**`errno` 优先**（fan-in 第 4）。**`CompiledUnit` 补 `co_cellvars`／`co_freevars`**（`BC-45`）＋ `.pyac` 的代码段编解码同步
（两个文本表接在 `names`／`varnames` 之后）；**注意**：这是代码段的**格式变化**，头部里没有
"产物格式版本"字段 ⇒ 旧构建写的 `.pyac` 会被新构建按新布局解释 ⇒ 产物必须重建（`IM-20` 的
陈旧判定管不到这一层，已如实记在这里）。

**cell 族指令**（`BC-45`：`MAKE_CELL`／`LOAD_LOCALS`／`STORE_DEREF`／`LOAD_DEREF`）**已落地**，
`cell` 类型显式注册（`Ladder::Later` 让它引导期不自动建）⇒ 这是"带 `def` 的类体"的前置。

**`__set_name__` 已落地**（第 109 轮；实测口径如下）：对本类命名空间**按插入序**逐项调
`__set_name__(类对象, 属性名)`；继承项不再调；无该方法的项跳过；抛错**原样传播**。
落点在类创建钩子（`pyawa-core/src/lib.rs` 里有完整实测记录）。

**待裁定（第 105 轮上报）**：`chain.from_iterable` 需要 `chain` 是**类型对象**且类型调用要能报错
（`chain(5)` ⇒ `'int' object is not iterable`），而本层 `NewFn` 槽**没有异常通道** ⇒ 要么改槽位签名、
要么让 `chain` 继续当函数并记一条 `DIV-`。两条路的代价写在 `pyawa-core` 的 `lib.rs` 里。

**`__static_attributes__` 的静态收集已落地**（只收 `self.X = …` 赋值、字母序去重、嵌套函数与 `if`／`while`／`for` 体里也算）。

**函数的隐式返回已落地**（`LOAD_CONST None; RETURN_VALUE`，只在语句体能落到末尾时发射）；
顺带修好 `tests/compile.rs` 的一处**夹具盲区**（原先逐条 `zip` 比指令，少了指令也不红 ⇒ 现在先比条数）。

**类体编译两档都已落地**（含 `def` 的那一档：`__classdict__` cell ＋ `LOAD_FAST_BORROW` 越界回落
到同号 cell 槽），端到端通过。

**类体编译**（`class C[(B)]: …`，体里不含 `def` 那一支）**已落地**：与参照逐字节一致 ＋ 端到端；
带 `def` 的类体（要 `__classdict__` cell）如实报未接线。

**`OM-40`／`OM-20` ② 机械化**：`tests/gc_field_coverage.rs`（持引用字段必须在 traverse／clear 里）。

**§5.2.6 `itertools` 已落地 `count`／`repeat`／`islice`／`chain`／`takewhile`／`dropwhile`／
`filterfalse`／`accumulate`／`starmap`／`cycle`／`pairwise`／`batched`／`zip_longest`／`compress`／`combinations`／`permutations`／`combinations_with_replacement`／`product`**（`islice` 的消耗语义与 `chain` 的
惰性都照实测；两条边界如实报未接线：`i64` 宽度、浮点；`chain.from_iterable` 等随后补）。
**迭代协议前置已落地**（`GET_ITER` 走 `__iter__`、推进走 `__next__`，实测消息一致）⇒ `itertools`
一类"要真迭代器"的模块不再卡在前置上。**已补合约**：§5.2.1 `errno`／§5.2.2 `builtins`／§5.2.3 `sys`／§5.2.4 `_imp`／§5.2.5 `_opcode`＋`_opcode_metadata`／**§5.2.6 `itertools`（`count`／`repeat`／`islice`／`chain`／`takewhile`／`dropwhile`／`filterfalse`／`accumulate`／`starmap`／`cycle`／`pairwise`／`batched`／`zip_longest`／`compress`／`combinations`／`permutations`／`combinations_with_replacement`／`product` 共 **18** 个；参照 20 个公开名，剩 `groupby`／`tee`／`chain.from_iterable`——后两者要「子迭代器」或「类型对象带方法」，是单独的、更大的形状）**；**§5.2.7 `operator`** 已落地 **36** 个函数（比较 12 ＋ 算术 7 ＋ 一元 4 ＋ 位运算 5 ＋ `inv`／`index`／`contains`／`concat`／`call`／`length_hint`／`itemgetter`／`attrgetter` 等）；其余（`matmul`／下标族要协议槽位、`methodcaller`、`i*` 原地族）各自有前置，逐条记在 §5.2.7；`sys` 的不依赖能力域面已落地（`argv`／`path`／`modules`／`version`／`version_info`／`hexversion`／`maxsize`／`maxunicode`／`byteorder`／`implementation`／`getrefcount`）＋ **`_imp` 的可落地子集**（`pyc_magic_number_token` 自定值、`is_builtin` 并入模块表前一律 `0`；其余逐条记在 §5.2.4 的"未落地"）＋ **`_opcode`／`_opcode_metadata` 的 Python 层包装**（转发核心的探测表，特化两表按 `BC-32` 必须为空），验收＋探测夹具入库 | `CM-4`、`CM-14`、`CM-19`…`CM-21` | Unicode 部分已解（`§13-7` 已决） |
| **P3-15** | **导出 Unicode 数据表**（`unidata_version` 锁定参照实现 ＋ 探测导出） | `CM-13`、`CM-22`…`CM-24` | 无。**已落地**：`tools/gen_unicode.py` 探测导出 → `unicode_tables.rs`（`unidata_version` ＝ **16.0.0**，与参照一致）＋ **四张区段表**（通用类别 4099 段／双向类别 2300 段／组合类 590 段／东亚宽度 600 段，共用一个二分查找）＋ Rust 夹具（抽样：类别 7190、双向 4095、组合 1059、东亚宽度 1151，均含每段首尾）＋ 三张**稀疏**表（`decimal` 760 项／`digit` 888 项／`numeric` 2002 项，值记成**约分对**——`numeric` 的内部表示就是分数，分母可以大到 `2^59` 故用 `u64`）＋ 第四张稀疏表 `decomposition`（**17085** 项，值是十六进制序列，兼容分解带 `<tag>` 前缀——标记是值的一部分）＋ 验收九条（版本锁定、四张区段表抽样对拍、四表覆盖全域 ＋ 区段连续、类别集合一致、越界给 `None`、**四张稀疏表穷尽对拍**〔`numeric` 逐位重现参照浮点〕、稀疏表结构不变量、分解表的标记集合）。生成器另加一条**规模哨兵**（稀疏表项数 ≥ 10 万即报错）。**未落地**：`name` 一族数据表、`unicodedata` 模块面本身（它挂在 `locale` 能力域下） |

| **P3-16** | **WASM 目标**（`§13-11` 已决：**需要**）：待定三件——能力层的 **provider 形态**、**ABI 面**（同一 `pa_*` 面在 wasm 上怎么落）、**构建目标** | `§13-11`、`SPEC-capabilities.md`、`SPEC-c-abi.md` | 无（已决"要"；**时机与待定项见 §9.4 第 8 项**） |
| ~~**P3-17**~~ **主体已落地（round 10；未覆盖形态见台账 round 10）** | **描述符协议剩余**：`property.__set__`／`__delete__`（**数据描述符写** —— `@x.setter` 现在仍写进**实例字典** ✗）＋ `staticmethod.__get__`／`classmethod.__get__`（`Lib/abc.py` 的 `class abstractclassmethod(classmethod)` 要它；`Lib/` 里 `classmethod` 遍地都是） | `OM-11`、`TS-41` | 无（第 279 轮已接线 `property.__get__` ✓；这三条是同一处的**续** ✓） |
| **P3-18** | **容器类型的 `__contains__` 面（dunder 面）＋ 先修它的前置**：`y.__contains__(x)` 与 `_collections_abc._check_methods(C, "__contains__")` 查的是**类型字典** ⇒ 内建容器类型要有这一条，且**在实例上取用要绑定 `self`**（`collections` 那一族 **12** 个模块压在上面）；**前置**：第 283 轮实测 「类体帧的**帧槽悬垂**」——`PYAWA_DANGLING=1` 在 `HEAD` 上就能复现（`frame_clear` ← `build_class_native` ← `load_module`；另有 `method_clear` 一支），普通模式只是**没被踩到** ⇒ 一旦多挂几条类型字典项就会**变致命**（实测 3/4 次 SIGSEGV）⇒ **先修悬垂、再落 dunder 面** | `OM-11`、`TS-41`、`MS-25` | 无（第 283 轮已定位到具体调用链 ＋ 逐条排除法；见 `ROUNDS.md` 第 283／284 轮）|
| **P3-19** | **类型对象的引用计数欠账**：谁会**多放一份**（把注册表里的类型对象减到 0 ✓）？第 284 轮已把**症状**挡住 ✓（通用释放路径不释放注册类型 ✓）并让两种诊断口全绿 ✓；抓手：`PYAWA_LEAK_MODE`／`PYAWA_RULER` 对照实验 ✓，再给类型对象加**最小计数断言**（归零即报地点 ✓）；嫌疑面：类体帧（`__class__` 那一节／`LOAD_NAME` 的命名空间 ✓）、`run_class_body` 的帧建立与清理 ✓ | `OM-15`、`OM-16`、`MS-25` | 无（第 284 轮已把「归零即释放」这条挡住 ✓，欠计数本身待查 ✓）|
| **P3-20** | **嵌套 `try` 会截断外层块**（第 294 轮已把病灶钉到指令级：嵌套那档的**处理块**没有体那条路那条「只发余部、不发作用域收尾」的规矩 ⇒ 处理块里也发了 `LOAD_CONST None; RETURN_VALUE`；两次修法（跳回余部起点／同口径跳块尾）分别撞上 `StackUnderflow` 与 `SIGSEGV × 57`，均已撤回）（第 293 轮定位、**未修**）：`try` 那一臂在发处理块**之前**就 `emit_rest_and_tail(余部)`，而处理块跑完之后的落点与余部的相对位置不成立 ⇒ **余部被跳过**。最小复现见 `ROUNDS.md` 第 293 轮；**受害者 `Lib/types.py`**（`except ImportError:` 里嵌 `try/except TypeError as exc:` ⇒ 其后 30+ 个定义全丢）⇒ 也是 `enum` 一族当前卡点的根因 | `BC-23`、`MS-25` | 无（形状与诊断都已钉死）|

> 原先标"卡 `§13-16`／`§13-2`／`§13-15`／`§13-9`"的四行现已全部解除；
> **现在没有任何待做项卡在未决项上**（`P3-14` 的 Unicode 部分除外）。

- **`MS-19` 从差异清单移入的四条**（"尚未实现"不是差异 ⇒ 落在本清单、**不进语料**）：
  `~~DIV-4~~` PEP 695（`Lib/typing` 可用时）、`~~DIV-5~~` 映射模式的映射协议
  （与 `MATCH_MAPPING`／`MATCH_KEYS` 的协议面同批）、`~~DIV-6~~` `__traceback__`
  （`BC-60` 的 traceback 面落地时）、`~~DIV-7~~` 空元组单例（**已修**：`OM-23` ＋ `tests/singletons.rs`）。
| ~~**P3-28**~~ **已完成（round 10；数字见 `DESIGN.md` §9）** | **实例体量的三档实测**（回答"有多像 Lua"：能不能开多个实例）：① 裸实例 ② 导入 `site` ＋ 常用模块的实例 ③ **N 个实例并行**（每实例增量 ＋ 总常驻 ＋ 并行度）；落点 `tools/footprint_*`（现只有裸档） | `DESIGN.md` §9 的实测基线 ＋ `M5` 的体量阻塞项 | 无（**现在可开工** ✓ —— 它是 **`M5` 判据③（体量升为阻塞项）的阻塞前置** ✓） |
| **P3-26** | **判据① 的驱动量＝C 模块缺口，不是批量搬运**（**改口径**：整包批量同步实测 **0 个可搬** ✗ ——`find_syncable` 判的是"逐字节拷进来后能否 import"，而包的导入链要**整套 C 模块**就位）。可导入面由 `CM-14` 的 fan-in 顺序决定（`DESIGN.md` §9 的曲线：前 5 个 ⇒ 67%、20 个 ⇒ 80%）；**同步是解锁的结果，不得当手段**。当前靶子＝把前 5 个（`sys`／`itertools`／`time`／`errno`／`builtins`）的面补到能支撑 `Lib/` 导入，并继续按 `next_work.py` 的报错签名队列取活 | `CM-14`／`CM-15`、`DESIGN.md` §9 | 无 |
| **P3-27** | **`Lib/test` 语料化**（**主体已落，转回归扩张**）（**判据② 的前提**）：三批共落 **408 条**（自建 193 ＋ `Lib/` 215（含 `Lib/test/` 50）；覆盖 **150** 个不同可 import 模块），来源列格式 A（manifest 第 5 列）＋出处守卫；**机检仪器** `tools/corpus_coverage.py`（五项：总量≥400／Lib 覆盖≥150／Lib.test≥40／对拍 0 新差异·自检绿／MS-14 四要素）已接进 `tests/ci/check.py` 成 **M3-COVERAGE 硬门** ⇒ **判据② 已裁定达成** ✓。此后按回归需要继续扩覆盖（不阻塞判据） | `MS-13` ②、`MS-14` | 无（机检仪器已就位 ✓） |
| ~~**P3-29**~~ **已落地（round 10 首片 `socketpair` ＋ round 37 主体：AF_INET TCP 面向全链）** | **`socket` 支**（**用户已拍板立项**，会话 05）：`_socket`/`socket` 的最小面——`http.client`／`urllib.request`／`socketserver`／`asyncio` 的共同前置。**round 37 落地**：`net` 能力域 vtable 的 8 个空槽补真 syscall（`bind`／`connect`／`listen`／`accept`／`getsockname`／`getpeername`／`shutdown`／`setsockopt`；`CP-39` v1 的 AF_INET 线格式、两段式出参、`EINTR` 重试）＋ `Instance::net_*` 包装 ＋ `_socket.socket` 方法面（`_accept`／`sendall`／`connect_ex`／超时值面／`family·type·proto` 只读属性）＋ 模块级 `get/setdefaulttimeout`；**连带修** `super()` 的属性查找补描述符协议。**验收**：TCP 回环对拍与 python3 3.14.4 逐字节 0 diff；官方套件 20⇒**21/397**（`test_xml_dom_xmlbuilder` 转 ok）、socket I/O 签名 18⇒**0**。**如实未接** ✗：IPv6／AF_UNIX 地址线、真超时语义、`_socket.poll`／`select` 分流、`sendto`／`recvfrom`／`getsockopt`、`getaddrinfo` 一族（A2 推迟）。 | `CM-14`、`DESIGN.md` §9 | 无（工作量大，单独立项 ✓） |
| **P3-30** | **`random.Random(<seed>)` 的序列与参照不一致**（会话 05 语料侦察实测：`Random(42).random()` ⇒ 0.0838… vs 参照 0.6394…，参照文档保证同种子同序列 ⇒ **真差异**）：生成算法/种子混合口径要对齐 CPython 的 MT19937 init_by_array | `MS-19`、`MS-10` | 无 |
| ~~**P3-31**~~ **已解除（round 3）** | **解析器缺口批**：**10 格全落**（嵌套元组目标／`.05` 前导点浮点／行内 if 分号表＋次行 else／`*` 后置关键字实参／`with … as (a,b)`／推导式模式目标／元组里的 `*`／带注解赋值非名字目标／制表符缩进／调用基底下标赋值）⇒ `difflib`／`gettext`／`xml.etree.ElementTree`／`tomllib`／`argparse` 等 11 个模块同步 ⇒ 判据① 341→358 ✓。剩余同族缺口（PEP 604 union 注解 dataclass 字段的 `__builtins__` 面）挂账后续。清单全文＝台账卷 `03-m3-progress-05.md` 的 task-7 表 ✓（一处真相，不重述） | `BC-14`、`CM-14` | 无 |
| ~~**P3-32**~~ **已完成（round 14）** | **官方 `Lib/test` 的框架门槛**（`tools/libtests_survey.py` 实测 397 模块 **0 通过** ✗）：根因＝**`sys.exit` 缺失**（`unittest.main()` 结尾必用）＋ **`unittest.main()` 直接崩 `KeyError: 'dest'`**（选项解析路）⇒ 修好这两处，官方套件才**开始**跑。**落地**：`sys.exit`（抛 `SystemExit`；退出码口径见 `SPEC-c-modules.md` §5.2.3）＋ CLI 把未捕获 `SystemExit` 解释成进程退出码；`unittest.main()` 全路打通（`dict` kwargs／`None` 键、argparse 的 `os.terminal_size`、`sys` 流 `write`、富比较协议、类内方法闭包转发等前置一并落地）⇒ **官方套件首次跑通**（≥1 个模块；全量调查与台账见 `NEXT.md` 与 `docs/rounds/`） | `REQUIREMENTS` 的兼容承诺（保证层）＋ `MS-19` 判据补充 | 无 |
| ~~**P3-33**~~ **已完成（round 14）** | **`sys.path` 不认 `PYTHONPATH`**（参照认 ✓、本层只有"脚本目录 ＋ `Lib`" ✗）⇒ `import test.support` 一类外部路径全灭；与 `IM-24`（`sys.path` 由 `site.py` 构造）的 C 层那半需补齐。**落地**：`PYTHONPATH` 进 `sys.path`（组合根过渡半，口径见 `SPEC-c-modules.md` §5.2.3 的 `path` 条目）＋ `test/support` 包骨架补同步 **21** 个上游文件（19 个骨架 ＋ `_sysconfigdata*` 2 个数据模块）⇒ `Lib/test` 的 .py **42→61**、`Lib/` 同步 **544/628**、`sync_lib.py --check` **548** 个文件逐字节一致（例外清单空）。**缺口如实记**：另有 **7** 个 `Lib/test/*` 骨架因 `P3-35` 的合法 Python 被拒（surrogate escape／链式赋值非名字目标／for-star／PEP 695 等）**暂不同步**（不登记 `KNOWN`，列进 `P3-35` 队列）；`SYNC_EXCLUDE` 另排除 `multiprocessing/heap.py`／`unittest/mock.py`（同因）。细节见 `NEXT.md` 与 `docs/rounds/` | `IM-24`／`IM-26`、`MS-19` 判据补充 | 无 |
| ~~**P3-34**~~ **已完成（M6 期实测 `1j` ✓）** | **`complex` 类型面**：`1j` 字面量解析不了（状态 5）⇒ **52 个官方测试直接阻塞**（调查实测） | `TS-42`、`CM-27` | 无 |
| ~~**P3-35**~~ **主体已完成（M6 期）＋ round 36 补 `DELETE_SUBSCR`（1/6）** | **合法 Python 被拒的语法缺口**（`MS-19` 判据补充：**必须修、不得登记为差异** ✗）：surrogate escape `\ud800` 类 **20 个**／同行 `try:`·`for:` **8 个**／解析器瑕疵（`语句结尾多出了 Some(Str(…))` 5、`async` 在列表里 1、`要冒号却得 Some(Dot)` 1）。**round 36 落地**：`DELETE_SUBSCR` 协议补齐——`__delitem__` 派发（用户类／容器子类）＋ `bytearray` 整数下标／切片（含步长）删除 ＋ 不可删内建文案两档照参照（整数下标 `doesn't`、切片 `does not`；语料 `MS-14` 407/407 不变）⇒ 分诊"下标删除"签名归零、官方套件未实现 100⇒87、跑通 20⇒20。**仍剩（如实）**：`\N{…}` 15（要 Unicode 名字表生成进 core）／`except*` 7（PEP 654）／`import *` 4（模块帧失败导入静默成 None 的机制缺口，证据在 round 36 台账）／async 推导式 4／PEP 695 `*Ts` 2。 | `MS-19` 判据补充、`BC-15` | 无 |
| **P3-36** | **`.pyac` `BadCodeSection`**（6 个官方测试触发 ✗）：代码段读不出来 ⇒ 缓存/序列化真 bug，按 `IM-18`…`IM-21` 的容器契约查 | `IM-18`…`IM-21` | 无 |
| ~~**P3-37**~~ **已完成（round 38 收口 `pyexpat` 形状面；解析本体如实未移植 ✗）** | **官方套件的缺模块**：`doctest`(15)／`unittest.mock`(12)／`gc`(10)／`unicodedata`(4)／`hashlib`(4) —— 按 `CM-14` 的 fan-in 取；`unittest.mock` 与 `P3-32` 同族（先修框架）。**round 38 落地 `pyexpat` 形状面**：模块级常量（`EXPAT_VERSION`／`native_encoding`／`version_info`／`features`／三个 `XML_PARAM_ENTITY_PARSING_*`）＋ `ErrorString` 44 条**码序**表 ＋ `errors`／`model` 两个**子模块**（模块对象，`xml.parsers.expat` 重导出要）＋ `ExpatError`／`error` ＋ `XMLParserType`（类名 `xmlparser`）＋ `ParserCreate`（处理器属性／`SetParamEntityParsing`；参数校验文案照参照）；**`Parse`／`ParseFile` 如实 `NotImplementedError`**（整块 expat C 扩展：XML/DTD/实体/命名空间/错误码语义，属独立评估）。验收：形状面对拍 python3 3.14.4 逐字节 0 diff；官方套件 `No module named 'pyexpat'` 签名 4⇒0（21⇒21）；`test_pyexpat` 仍到不了断言层（如实）。 | `CM-14`、`P3-14` | 无 |
| **P3-38** | **`str` 表示层支持合法孤立代理**（**WTF-8 级改造；surrogate epic**，**用户已拍板 2026-10-25**）：动机＝`MS-19` 判据补充「**合法 Python 被拒必须修、不得登记差异**」——当前 `Lib/test/support/os_helper.py`（`'\udeee'`／`'\uDC80'` 等）与 `Lib/test/multibytecodec_support.py`（`'\udeee'`）是**唯一阻塞**；survey 里 **30** 个 `test_*.py` 因 surrogate 转义报**语法错**、`os_helper` 缺模块 **×106**；另 `surrogateescape`／`surrogatepass` 是 **PEP 383** 的语义面。**分阶段（每阶段＝可提交的独立切片）**：**S1 `wtf8.rs` 编解码基础**（本轮 super-fix 落地中：`encode`／`decode`／`validate`／往返／错误用例；**零风险**，不动既有表示）；**S2 `StrObject` 存储迁 WTF-8 字节 ＋ `text_of` 语义**（`Some(&str)` **仅对合法 UTF-8**，新增 `wtf8_bytes()` 逃生口）；**S3 各 API 面迁移**（按审计报告分小片：repr／print／encode／decode／比较／哈希／切片／格式化／属性名／字典键／fs 路径／正则／`unicodedata`）；**S4 lexer 接受合法孤立代理转义** ＋ `str.encode`／`decode` 的 `surrogateescape`／`surrogatepass`；**S5 `Lib/test/{support/os_helper,multibytecodec_support}.py` 等重试同步 ＋ 回归**。**逐阶段验收**：S1 ＝ WTF-8 合法／非法样例的往返与错误分类对拍夹具；S2 ＝ 既有 `text_of` 调用点全绿 ＋ 合法 UTF-8 路径零行为变化（`.py` 编译产物逐字节不变）；S3 ＝ 每片单独提交、该片含代理样本的对拍用例通过且不回归；S4 ＝ survey 的 **30** 个 surrogate 语法错族归零 ＋ PEP 383 两套 codec 语义对拍；S5 ＝ `tools/libtests_survey.py` 的 `os_helper` 缺模块 **×106** 与对应两文件相关格归零、语料无新差异（`MS-19`）。**M6 收口 gate 含本条完成**（"新差异为零" ＋ `MS-19` 补充；`MS-14` 的报告口径不变）。 | `MS-19` 判据补充、`CM-13`（不变量）、`BC-15`；`str` 的既有条目只引用（`TS-41`／`TS-42`／`TS-43`、`BC-4`） | 无（用户已拍板；S1→S5 顺序、每阶段独立提交） |
| ~~**P3-39**~~ **已完成（round 33）** | **`io.StringIO` 不可实例化**（`TypeError: cannot create 'StringIO' instances`）⇒ **15 个官方测试阻塞**（分诊实测）；`io`／`_io` 面按 `CM-14` 补。**落地**：`_io.StringIO` 由占位类型换**真类**（基类 `_TextIOBase`；21 个 native 处理器：`__init__`／`write`／`getvalue`／`close`／`read`／`readline`／`readlines`／`writelines`／`seek`／`tell`／`truncate`／`flush`／`readable`／`writable`／`seekable`／`isatty`／`detach`／`__iter__`／`__next__`／`__enter__`／`__exit__`）；位置是**码点**下标、越界 `write` 用 `'\x00'` 补齐、关闭态文案两档、文本流 `seek` 规则、`writelines`／`detach`／文本流属性面全部照本机 3.14.4 实测；顺修 `BytesIO`／`StringIO` 共有的 `readlines` 到 EOF 多推空串 ＋ `sizehint` 一处真 bug。**验收**：对拍探针 pyawa 与 `python3` **逐字节 0 diff**；`test_memoryio` 跑通 **20⇒43**（errors 148⇒111）；`test_io`／分诊里 `cannot create 'StringIO' instances` **归零**。**如实未接** ✗：`newline` 的读/写翻译均已接（2026-11-14，`NewlineMode`；见 §12.5／§12.6）、`__getstate__`／`__setstate__`／pickle、`IntLike`（`__index__`）实参（属 `P3-42`） | `CM-14`、`P3-37` | `crates/pyawa-stdlib/src/`、`Lib/io.py` |
| ~~**P3-40**~~ **已完成（round 34）** | **`os` 面缺口**：`os.mkdir` 缺失（**8** 个测试 ✗）／`os.pipe` 缺失（**6** ✗）／`os.unregister` 一类（2 ✗）—— 分诊实测。**落地**：`posix.mkdir(path, mode=0o777)` 走 `fs` 的 `mkdir` 槽（provider 改 **`mkdir(2)`** ⇒ `mode` 真生效、umask 由内核施加、errno 原样带回 ⇒ `FileExistsError` 等子类 ✓）＋ `posix.pipe()`（`CpFsVtable` **表尾追加** `pipe` 槽，`AB-41` ⇒ `PA_ABI_MINOR` **3→4**；两端不透明句柄 → 实例能力表下标，`CP-31` 动态段从 3 起、与 fs 共用一张表）＋ `codecs.unregister`（**更正分诊标签**：那 2 个测试要的是 `codecs.unregister`，CPython 的 `os` 没有 `unregister`）＋ `SPEC-capabilities` §9.1／`SPEC-c-abi` §12／`pa.h` 同步。**验收**：对拍探针与 python 3.14.4 **逐字节 0 diff**；分诊里 `mkdir`(8)／`pipe`(6)／`unregister`(2) **归零**。**如实未接** ✗：`os.close` 已关 fd 仍静默、`get/set_inheritable`、`os.chmod`／`scandir`（下一层缺口）。 | `CM-14`、`P3-37` | `crates/pyawa-stdlib/src/`、`Lib/os.py` |
| **P3-41** **主体已落地（round 32）** | **官方套件里的 hang 与崩溃**（12 个"超时"逐条复跑后的裁决）：**1 个崩溃** ✗✗（`test_frame` exit **134** ＝ SIGABRT —— 属 `AB-29`／不变量 3 类，**最高优先** ✓）＋ **1 个真 hang** ✗（`test_tempfile`：`map()` 急求值消费无限 `_RandomNameSequence`，无界内存）＋ **5 个只是慢** ✓（round 32 **逐条复核推翻"6 个真 hang"的原判**：`test_dict`／`test_weakref`＝dict 线性扫描 O(n²)；`test_long`／`test_peepholer`＝`unittest.subTest` 机器开销＋不释放；`test_userstring`≈15 min，次因 `startswith/endswith` 忽略 `start`／`end`）。**落地**：① `test_frame` 不再 abort（`repr` 递归守卫加深度计 ⇒ `RecursionError` ＋ RAII 摘守卫；`set`／`frozenset` 补守卫）⇒ **exit 134 ⇒ 状态 5（如实"未实现"）**；② round 32 顺带修新暴露的崩溃族：`for`／`while` 体末尾 `if …: break` 前面夹 `try` ⇒ 回边丢失 ⇒ `StackUnderflow`（列表断言失败会中止整个官方测试文件 ✗）；③ 6 个 hang 全部给出**最小复现**（`target/recon/hang1/`、`hang2/`，细节见 `docs/rounds/04-post-m6-01.md`）。**round 39 续落（慢因）**：`test_frame` 的 26.9 s 单测＝15 万层嵌套 mapping 被**单代 GC 固定阈值**拖成 **O(n²)**（每 2000 次分配做一次 O(存活) 全堆扫描）⇒ `Instance::alloc` 改**自适应水位**（每次回收后至少再等 `max(阈值, 存活数)` 次分配；`sys.get_threshold()` 口径不变）＋ `gc_collections()` 诊断计数；效果 `test_frame` release **32.46⇒4.10 s**／debug **196⇒41.22 s**、该形状 O(n²)⇒线性；官方套件**超时 9⇒3**（`test_long`／`test_peepholer`／`test_userstring`／`test_cmath` 等 6 个转为正常跑完）。**round 40 续落（`test_userstring`）**：① `str.startswith／endswith／count` 的 `start`／`end` 真生效 ＋ `find`／`count` 收 `None` 边界 ＋ `split`／`rsplit` 收 `None`／`maxsplit` ＋ `bytes` 同口径（对拍 0 diff；清掉 `test_find_periodic_pattern` 的 904 个假失败）② **`weakref.ref` 相等／哈希按目标**（`values_equal` ＋ `hash_object`，core 加安全访问器 `weakref_target`）⇒ `_py_abc` 的 `_abc_cache` 不再"一次 `isinstance` 长一条"：`isinstance` **5.4 ms ⇒ 12.3 µs 恒定**、`test_count` **>900 s ⇒ 6.9 s**、`test_userstring` **941 failures／9 errors／>900 s ⇒ 38 failures／4 errors／42.3 s**（分类 runtime、不超时）。**round 41 续落（6 hang 的剩余账）**：`map`／`filter` 改**真惰性迭代器**（`ItStateKind::Map`／`Filter` ＋ 类型注册 ＋ `advance_map`／`advance_filter`；`type(map(...))` 与 `iter(m) is m` 照参照 ✓）＋ `dict.fromkeys` 收通用可迭代（顺修重复键不合并、多键 value 计数欠账）⇒ **`test_tempfile` 的唯一真 hang 解除**（无限挂死 ⇒ 118 测试／0.75 s 跑完）；官方套件**超时 3⇒2**。**未修（如实）**：dict 哈希化（`test_dict` 的 `test_copy_fuzz` 153.6 s：`d[i]=i` 建 10 万项与 `d == d2` 都是线性扫 ⇒ O(n²)；`test_weakref` 超时同源）、`subTest` 释放、`test_userstring` 剩余 38 failures／4 errors、`weakref` 仍是强引用。 | `AB-29`、`CX-` 崩溃面、`MS-19` 判据补充 | `crates/pyawa-core/src/`、`crates/pyawa-stdlib/src/` |
| **P3-42** **主体已落地（round 35，6 组里 5 组）** | **断言失败签名批次**（分诊 `target/libtests_triage.tsv` ✓）：`int.from_bytes` 缺 `byteorder`(2) · `time.struct_time` 缺(2) · `compile()` 不认 flags(2) · `Exception.__setstate__` 缺(2) · `_opcode.is_valid` 缺(1) · `KeyError: 'closed'`(3) · `KeyError: N`(7) —— 逐条按 `MS-19` 判据补充处理（合法 Python 报错＝新差异、必须修 ✗）。**落地（round 35）**：前五组签名**分诊归零**——`from_bytes` 缺省 `byteorder='big'`；`time.struct_time` 真类型（可序列／9 个 `tm_*` 属性／`tm_zone`／构造与 repr 照本机 3.14.4；`localtime`／`gmtime` 返回它、`mktime` 接受它）；`compile()` 收 `Lib/codeop.py` 的 `PyCF_*` 与 `__future__` 位（barry／AST／顶层 await 诚实未实现）；`Exception.__setstate__`；`_opcode.is_valid`。跑通 18⇒**20/397**（`test_baseexception`／`test_strftime` 转 ok）。**未清（如实）**：`KeyError` 族 12 行——根因两层：(a) **特殊方法派发缺口**（`==`／`!=` 不走 `__eq__`、`__add__` 等不派发；`test_binop`／`test_opcodes` 卡它），(b) `unittest` 的 contextmanager 生成器里 `KeyError` 在测试体执行期间仍处"正在处理"⇒ 断言异常的 `__context__` 被污染（`test_keyword` 等显示成 `KeyError: N`）。另：`time.struct_time` **不是 tuple 子类**（`isinstance(t, tuple)` 假 ⇒ `test_structseq.test_tuple` 仍 FAIL）。**round 44**：`==`／`!=` **特殊方法派发**已落地（用户类 `__eq__`／`__ne__`、容器逐元素、`NotImplemented` 下一跳、`object` 默认实现跳过防递归）＋ `str` 一族口径 ⇒ `test_userstring` **38F／4E ⇒ 全绿**、官方套件跑通 **21 → 22**（未实现 90 → 89）；`KeyError` 族的 (b) 上下文污染仍在。**round 45**：嵌套 `with` 退出路径（内层正常出口不跳循环续点、`break`／`continue` 退出循环体内各层 `with`）⇒ `test_frame` 状态 5 解除、官方套件「未实现」90 → 83。**round 46**：真值协议（`__bool__` 恰返回 `bool`／`__len__` 整数且 `>=0`／类型对象恒真）⇒ `test_bool` 的 Rust 栈溢出 abort 消除（6F／2E 跑完）。**round 47**：`bool` 不可继承／`int`·`bool` 属性面／一元·位运算的 bool→int／`bool.from_bytes`／`bool(**kw)`／`BytesIO.getbuffer`／`eval` 结果查找 ⇒ `test_bool` 6F／2E ⇒ **2F**、官方套件跑通 22 → 23。**round 48**：`int.__new__`／`bool.__new__` 显式登记（`type.__new__` 的 new 槽桥接收口）＋ 核心侧 `DeprecationWarning` 通道（`~bool`）⇒ `test_bool` **全绿**、官方套件跑通 **23 → 24**。**round 49**：doctest 导入前置面（`sys.gettrace`／`stdin`／`displayhook`、隐式异常 `__traceback__`、`backslashreplace`）⇒ 4 个 doctest 文件从导入即死推进到真实用例（剩余 `DICT_MERGE`／生成器 `throw` 的 `StackUnderflow`／断言差异）。 | `MS-19` 判据补充、`P3-35` | 随修复点 |

> **`P3-41` 的复核（2026-10-31 23:36，文档侧实测 ✓）**：`test_frame` **崩溃已消除** ✓（exit 134 → **exit 1，196 s** ✓，不再 abort ✓；但 **196 s 太慢** ✗ ⇒ 仍会被 survey 的 120 s 上限计为超时 ✓）；**6 个 hang 全部仍在** ✗✗（`test_dict`／`test_long`／`test_weakref`／`test_peepholer`／`test_tempfile`／`test_userstring`，180 s 均未结束 ✓）。⇒ `P3-41` 只完成了"崩溃"这一半 ✓，**hang 面未动** ✗；另建议把 `test_frame` 的**慢**单独当一条（中等测试 196 s vs 正常秒级 ⇒ 疑似 O(n²)／重复扫描）。

### 9.3 文档侧待办

**当前无待办。** 原先列的三条（`§13-3`／`§13-7` 的候选、`CP-` 领号、`errno` 常量来源）
**已全部落定**；剩余 6 项规格缺口（`BC` 超出 §10 的指令、`CM` 逐模块合约表、`MS` 四项）
**全部等实现**——按 `SPEC-INDEX.md` §5.1 对 `v0` 的定义，文档侧已成立。

### 9.4 下一阶段的开工指示（**非规范**；正文已归档）

> 正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §9.4 归档」。本文件只留指针（非规范内容不进 `PLAN`）。

## 10. 尚未写出（本规格自己缺的节）

`SPEC-INDEX.md` §5 第 6 条要求 `v0` 规格显式列出缺口。本规格缺：

| 缺的节 | 内容 | 为什么现在没有 |
|---|---|---|
| **对拍 harness 的实现** | `MS-6`…`MS-14` 的可执行实现 | **已落地**：runner ＝ `crates/pyawa-abi/tests/conformance.rs`，语料 ＝ `tests/conformance/corpus/`，报告 ＝ `target/conformance/report.md`（`MS-14`）——口径与边界逐条见 `tests/conformance/README.md`。<br>~~**未落地**：`MS-8` 的 **stdout／stderr** 比对（等 `print`，`CM-26`）~~ **已完成**（第 93–95 轮）．**仍欠**：`MS-13` ②（`Lib/` 语料，等 M3）；`MS-9` 已按 `§13-10` 的裁定归一（行尾／末尾换行 ＋ `0x` 地址 ＋ 路径前缀） |
| **基线语料清单** | `MS-13` ①（自建最小语义用例）与②（`Lib/` 中可编译部分）的具体清单、及其**模式标注** | ① **已有首版**（纯 Python 模式；清单与边界见 `tests/conformance/README.md`）；**但 `MS-13` 的"范围下限"已裁定**（`§13-10`）：总数 ≥ 112 ＋ 各内容面下限，判据 `tools/check_corpus_floor.py` ✓ —— **M2 已按它成立** ✓。<br>② **已裁定达成** ✓（2026-10-18 裁定；当前语料 **408** 条＝自建 193＋`Lib/` 215（含 `Lib/test/` 50），覆盖 **150** 个可 import 模块；机检仪器 `tools/corpus_coverage.py` 五项全 PASS 并已接进 `tests/ci/check.py` 成 **M3-COVERAGE 硬门**）；扩展模式语料**没有参照**，用**自身对照**验收（见各特性条款） |
| ~~**差异清单的初始内容**~~ | ~~`MS-19` 要求的逐项「依据 ＋ 归一规则」~~ | **已关闭**：`tests/conformance/divergences.md` 已建立并录入条目（`69592b4`）。<br>⚠ `MS-19` 新加的**适用范围**要求的**重新分诊已完成**：主表只留两类合法差异， `DIV-3` **已修**（编译器产出 `co_qualname` ＋ `repr` 改用它 ＋ 类创建钩子补写 `C.m`），`DIV-4`…`DIV-7` 已移入 §9.2（`DIV-7` 已修） |
| **M2…M6 的可执行判据** | 各里程碑判据的脚本化（M0 已由 `tests/ci/check.py` 承担） | **部分已落地**：M0 ＝ `tests/ci/check.py`、M1 ＝ `tests/ci/t_ab_1.py`、M2 ＝ `cargo test -p pyawa-abi --test conformance`（`MS-10` 的三分类）；**M3…M6 待各自实现就位**——`MS-1` 要求可执行，故余下部分仍是本规格的欠账。<br>**M3 判据① 的仪器**：`tools/lib_import_ratio.py` **按判据口径**量 —— 分母＝**上游 `Lib/**/*.py` 全量（628）**，**两侧都跑**（Pyawa 侧同对拍那条 ABI 路径；参照侧 `python3 -S -c "import <模块>"`，`-S` 与本层"不跑 `site.py`"对齐），分三类（通过／**参照口径**——参照自己也 import 不了 ⇒ 不计我们失败／失败）。**基线（第 279 轮）**：**27 ÷ 628 ⇒ 4.3%** ✗（通过 10 ＋ 参照口径 17）。**另有一个上限诊断**（第 281 轮加 ✓，**不作判据** ✓）：`--ceiling` 把上游 `Lib/**/*.py` **全量**复制到 scratch 并放进 `sys.path` 再逐个 import ⇒ 量的是「若把 `Lib/` 全同步进来、现在能 import 多少个」，并把**卡住的族**按首个异常归并 ✓（它把「还没同步」与「同步了也跑不动」分开 ✓）。基线 **145/628 ＝ 23.1%** ✓（第 282 轮）| |

> 原先的「`MS-16` 的版本升级流程」缺口已由 §2.1 的 `MS-22`／`MS-23` 关闭。

> **历史台账已移到 `docs/ROUNDS.md`（索引）与 `docs/rounds/` 各卷**（逐轮日志、块结构模型、M3 完成度；**非规范**）✓

## 11. LSP 线（2026-11-13 用户批准开工 ✓）

**三梯，每梯可独立验收** ✓：① `pyawa check` ② LSP MVP（`publishDiagnostics` ＋ `hover`）③ 补全／跳转／重命名（按需）。

**依赖隔离** ✓：第三方依赖只许在 `pyawa-lsp` crate 内（`CX-27`／`T-CX-17`）；核心 crate 的边界见 `CONSTRAINTS` 的 `CX-27` ✓。

### 11.1 ① `pyawa check` 需求（给开发）

语义以 `SPEC-type-system.md`／`SPEC-imports-and-modes.md` 为**一处真相** ✓；此处只给**范围／验收／禁区** ✓。

**范围** ✓：`pyawa check <文件…>` 子命令——**只编译不执行** ✓；**批量诊断**（`TS-41` ✓，禁止遇首错即止 ✗）；
`--format=json` ✓（LSP ② 的数据源）；`.py` **只出语法类** ✓（`TS-26`／`MS-19`），`.pyawa` 出语法＋类型 ✓；
退出码 **0＝无诊断／1＝有诊断／2＝内部错误** ✓。

**验收** ✓：
① 408 语料（`.py`）跑 `check` **0 诊断** ✓；
② 一个故意写错的 `.pyawa` 能**一次报出 ≥2 条**类型诊断 ✓（证批量）；
③ `--format=json` 含 `file`／`line`／`col`／`code`／`severity`／`message` ✓；
④ CI `fast` 档在 `check.py` 之后加一步对语料跑 `pyawa check` ✓；
⑤ 同一文件「`pyawa` 执行」与「`pyawa check`」的**语法**诊断一致 ✓。

**禁区** ✗：不对 `.py` 做类型诊断（破坏 `MS-19`）；不执行任何代码；不做 autofix（留 ③）；不实现 LSP 协议（那是 ②）。

**依赖** ✓：`M4` 检查器（`TS-*`）＋ `compile/` 解析器（0 unsafe）；**不引** LSP 框架 ✓（那是 `pyawa-lsp` 的事）。

**状态** ✓：① `pyawa check` **已落地 ✓（round 275）**——`pyawa check [--format=json] <文件…>`：只编译不执行、批量诊断（不遇首错即止）、`.py` 只语法类（`MS-19`）／`.pyawa` 语法＋类型、`--format=json`（`file`/`line`/`col`/`code`/`severity`/`message`）、退出码 0／1／2；验收 `crates/pyawa-runtime/tests/check_command.rs` 6 例（含 **407 语料 0 诊断**、JSON 字段、执行 vs check 语法诊断一致、退出码）；CI fast 在 `check.py` 后已加语料步骤（PLAN §10.1 ④）。② LSP MVP **已落地 ✓（round 276）**——`crates/pyawa-lsp`（手写 JSON-RPC／LSP 帧；**round 281** 起 JSON 编解码改用 `serde_json`，帧与分派仍手写）：`initialize`／`initialized`／全量 `didOpen·didChange·didClose`／`publishDiagnostics`（与 `check_source` 同一处真相）／`hover`（已标注名字的声明类型）；集成验收 `crates/pyawa-lsp/tests/lsp_stdio.rs` 3 例（真起子进程走 Content-Length 帧）；CI fast 已加 `-p pyawa-lsp` 测试。③ **按需梯的最小项 已落地 ✓（round 277）**——`textDocument/completion`（文档内符号）／`textDocument/definition`（注解/def/class/顶层赋值首个定义处）／`textDocument/rename`（**单文件整词** `WorkspaceEdit`）；验收 `crates/pyawa-lsp/tests/lsp_stdio.rs` 第 4 例。**如实边界**：不做类型推断／成员补全／标准库符号／跨文件跳转／作用域分析（PLAN 原文即"按需"，这些留待需求出现 ✗）。**位置口径收口 ✓（round 279）**：LSP `character` 已严格按 **UTF-16 code unit** 换算（入参/出参、非 BMP 字符；验收 `lsp_stdio.rs` 第 5 例）；其余已知边界在 `crates/pyawa-lsp/README.md` 的「范围外」逐条写明理由 ✗。

## 12. 需求单（只放**未完成**项；已收口正文归档到 `docs/rounds/`）

三项均由文档侧实测定位（非推测），证据与验收如下。语义仍以各 `SPEC-*` 为**一处真相**，此处只给范围与判据。

**状态 ✓（2026-11-14）**：三项**全部落地**（逐项状态见各小节末尾状态注；`REQUIREMENTS.md` 的「信号面」「工作边界」等决策已按本轮实测收口）。

### 12.1 `x = yield`（`SEND` 取值形态）—— 语言语义缺口

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.2 `_signal` 最小面 —— 一次解锁 `asyncio`

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.3 `_csv` ＋ `Lib/csv.py` —— 标准库 C 底座缺口

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.4 ABI 收口（2026-11-14 审计，8 项）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.5 解释器与模块缺口收口（2026-11-14）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.6 优先队列收口（2026-11-14，第二轮）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.7 引用泄漏结构性收口（2026-11-14，第三轮）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.8 失败路径 GC 探针接闸（2026-11-14）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.9 全口径 TS 注解解析器（2026-11-14 起，分里程碑）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.10 TS 注解解析器收口（2026-11-14）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.11 ABI 边界不可感知（2026-11-14 用户定向）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.12 协议派发唯一化与 ABI 收敛（2026-11-14）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.13 宿主可用性与诊断缺口（2026-11-14，真 numpy 压测所出）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.14 `getattr_fn` 必须是对象语义的单一来源（2026-11-14）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.15 宿主协议唯一入口：实现与验收（2026-11-14）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.16 `pa_writetobytes`（冻结前唯一要补的 buffer 面，2026-11-14）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.17 能力域 vtable 的 C 布局（冻结前唯一必补项，2026-11-14）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.18 ABI 剩余收口清单（2026-11-14；非 ABI 项暂缓）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.19 traceback 规格空白与收尾（2026-11-15）

> **✓ 已收口**；正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」。

### 12.20 平台矩阵：Windows（稳定版承诺，2026-11-15 用户拍板 B）

**背景**：现状只有 Linux（CI 3 个 job 全 `ubuntu-latest`；平台 provider 仅 `fs_posix`／`net_posix`／`clock_system`；
stdlib 仅 `posix_module`；规格无 Windows 提及）。用户决定**稳定版承诺 Linux x86_64 ＋ Windows x86_64**。
ABI 不受影响（`CP-4` 已把平台代码隔离在 runtime／stdlib）。

**规模参照（如实）**：`posix_module.rs` 2625 行、`fs_posix.rs` 1534 行、另有 `net_posix.rs`；
Windows 侧要写对应 provider 与 `nt` 模块 —— 这是**里程碑级**工作线，不是小项。

| 阶段 | 内容 | 验收 |
|---|---|---|
| **W1 可移植性清理** | 把通用代码里的 POSIX 假设收进 provider；CI 新增 `windows-latest` 的**最小"能编译"** job（build ＋ `check.py`） | Windows job 能编译通过；`check.py` 在 Windows 上绿 |
| **W2 `clock` 域** | Windows 时钟 provider（QPC） | 时钟测试跨平台同结果 |
| **W3 `fs` 域** | Win32 文件 provider（路径语义／只读根／错误码映射） | `fs` 形状测试跨平台；`CP-21` 越界拒绝保持 |
| **W4 `net` 域** | Winsock provider | socket 形状测试跨平台；能力域拒绝语义不变 |
| **W5 `nt` 模块 ＋ `os` 分支** | stdlib 的 `nt` 模块（`os` 在 Windows 走它）；`sys.platform`／平台字段 | `import os` 平台分支正确；`ntpath` 随 `Lib/` 同步生效 |
| **W6 测试矩阵** | Windows 上跑 `pyawa-abi` 测试 ＋ conformance ＋ 参考宿主（MSVC／clang）；**内存工具替代**（Windows 无 valgrind：用 Dr. Memory 或 ASan，或明确"Windows 上不做 memcheck"并记录） | CI 矩阵绿；内存工具口径写进 `RELEASE.md` |

**W1 状态（2026-11-15）✓（只到「能编译」；运行期属 W2–W6）**：
- **本机交叉验证（如实：只 type-check，不链接、不运行）**：装入 `rust-std` 的 `x86_64-pc-windows-msvc`
  组件后，`cargo check --workspace --target x86_64-pc-windows-msvc` **通过**（libs ＋ bins 全绿）
  ⇒ 核心／能力接口／stdlib／abi 的 POSIX 假设**确已收在 provider 内**。
- **代码收口**：CLI（`crates/pyawa-runtime/src/bin/pyawa.rs`）依赖真实能力域 provider（`fs`／`clock`／
  `net` 的 Unix 实现）⇒ CLI 实现只在 `unix` 编译；非 unix（Windows）留**显式「未接」入口**
  （打印指向 W3–W5、退出码 2），**不伪造**半可用路径。libs／bins 在 Windows 目标零改动即可编译。
- **CI**：`.github/workflows/ci.yml` 新增 **`windows` job**（`windows-latest`）：`cargo build --workspace`
  ＋ `python tests/ci/check.py`。无对拍产物（跨平台对拍属 W6）⇒ 该 job 以
  `PYAWA_CHECK_SKIP_COVERAGE=1` 把 `M3-COVERAGE` **显式标 SKIP**（不是 PASS）；Linux 的 fast／full
  不设此变量 ⇒ 仍强制。
- **CI 实跑（两次，如实）**：首跑 **run `43732891950`**——`fast`／`wasm` 绿、`windows` 红：
  Windows 控制台默认 cp1252，`check.py` 一 `print` 中文就 `UnicodeEncodeError` ⇒ 修复为
  stdout／stderr ＋ 两个子进程**显式 UTF-8**（`PYTHONIOENCODING=utf-8`）；复跑 **run `43734289101`**——
  **`fast`／`wasm`／`windows` 三 job 全绿**（`windows` 1m6s：`cargo build --workspace` ＋ `check.py`，
  `M3-COVERAGE` SKIP）。
- **如实边界（不阻塞 W1）**：本机无法运行 Windows 二进制（只交叉 type-check；实跑结果见上）；
  `cargo check --all-targets` 在 Windows 上仍红（`crates/pyawa-runtime/tests/path_resolution.rs`／
  `t_cp10_cancel.rs` 是 Unix 专用测试）⇒ **测试矩阵属 W6**，不在 W1 范围。

**W2 状态（2026-11-16 / 补齐 2026-11-19）✓**：Windows `clock` provider 落地（QPC 口径）；**日历三槽已补齐**（UCRT `_localtime64_s`／`strftime`／`_mktime64` ＋ `tm_gmtoff`／`tm_zone`；`struct_time` 11 元面）；
正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」的 12.20-W2／W3 节
（Windows runner 实跑 ✓ run `43846653765`：clock **4/4**）。

**W3 状态（2026-11-16 / 补齐 2026-11-19）✓**：Windows `fs` provider 落地（全槽位；**`pipe` 已补齐** `CreatePipe`；**`dev`／`ino` 真值** = 卷序列号／64 位文件索引）；`CP-21` 判定仍在 VM；
正文归档：同上（12.20-W2／W3 节；Windows runner 实跑 ✓ run `43846653765`：fs **8/8**）。

**W4 状态（2026-11-17 / 补齐 2026-11-19）✓**：Windows `net` provider 落地（ws2_32 FFI；**`socketpair` 已补齐** = loopback TCP 对模拟）；
正文归档：`docs/rounds/04-post-m6-01.md`「附录：PLAN §12 已收口工作单归档」的 12.20-W4／W5 节
（Windows runner 实跑 ✓ run `43911783517`：net **6/6**）。

**W5 状态（2026-11-17）✓**：Windows `nt` 模块（同一实现换名）＋`sys.platform="win32"`＋`sys.path`
guest 化；`import os` 走 nt 分支；正文归档：同上（Windows runner 实跑 ✓ run `43911783517`：
os **1/1**；首跑暴露的 CRLF 问题已由 `ee2c809e` 的 universal-newlines 修复并回归）。

**W6 状态（2026-11-18）✓ 收口**：`windows` job 全绿（run `44290668045`）——构建、abi／runtime
`--all-targets` 可编译、**conformance（必需，0 新差异＋DIV-12 Windows-only 已知差异）**、
**`pyawa-abi` 19 个测试目标真跑**、provider／os 19/19、core＋lsp、参考宿主（MSVC）、CLI 三形态冒烟、
`check.py`（M3-COVERAGE SKIP）；内存口径＝Windows 不做 memcheck（`RELEASE.md`）。
正文归档：`docs/rounds/04-post-m6-01.md`「附录」的 12.20-W6／12.20-W6-conformance-Windows／
12.20-W6 ✓ 收口 三节。

**平台面补齐第一批（2026-11-19）✓**：W2/W3/W4 的"如实未接"缺口清零——`clock` 日历三槽、
`fs.pipe`、`net.socketpair`、`fs` 的 `dev`／`ino` 全部真实现；新增 `windows_platform_gaps` ABI
端到端测试并进 CI windows job（run `44515100434` 全绿）。**仍如实未接**：`errno` 表仍 Linux 生成、
`nt` 的 `splitroot`／`normpath` 盘符语义差异、`symlink` 需特权、**Windows 不做 memcheck**、
`_overlapped` 数据面（Proactor 不能真跑）。正文归档：rounds 的 12.20-W2W3W4-平台面补齐 节。

**平台差异如实登记**：路径分隔符／驱动器号／符号链接权限／大小写敏感性等差异进 `tests/conformance/divergences.md`（`DIV-` 条目），不在代码里"假装一致"。

**不在本单**：macOS（待定）、ARM64、其它架构。

### 12.21 Web（WASM）适配（2026-11-15 用户提出；2026-11-16 压缩）

**目标**：pyawa 在浏览器里跑（宿主＝JS）。核心难点：**wasm 无运行时函数指针** ⇒ `pa_register` 需
**wasm 侧 trampoline 表**（固定槽位 wrapper，JS 按槽位填 import）；无 OS／无线程 ⇒ `fs`／`net` 门控、
`clock` 由 JS 提供；跨 JS 边界一律当次复制（借用规则天然满足 `AB-15`／`AB-17`）。

| 阶段 | 内容 | 验收 |
|---|---|---|
| **E1** | 平台门控：`wasm32-unknown-unknown` 可编译（fs／net 门控；线程与时钟禁用或由 JS 提供） | wasm 产物可编译、无 panic |
| **E2** | **trampoline 层**：wasm 侧固定槽位 wrapper ＋ JS 填充协议 | JS 注册函数、guest 可调用 |
| **E3** | ABI 导出：`pa_create`／`pa_exec_string`／`pa_*` 对 JS 可见，错误消息可取 | JS 建实例并执行脚本 |
| **E4** | 能力域：`clock` 由 JS 提供；`fs`／`net` 如实"未提供" | 取时间走 JS 时钟；文件操作如实报未实现 |
| **E5** | 测试与 CI：node／headless 跑纯计算冒烟；CI 增 wasm job | wasm job 绿；跑不了的部分如实登记 |
| **E6** | 文档：`RELEASE.md` 增 Web 行与限制 | 文档与实现一致 |
| **E7 ✓（2026-11-21 进 main）** | 库嵌入档：多实例隔离／宿主函数／stdout 经宿主 `fs_write` | `tests/wasm/e7_embedding.js` PASS（main `a8d6ad8a`） |
| **E8 ✓（2026-11-21 进 main）** | 可移植子集：10 个纯计算/Rust 模块脚本，node 运行＋canonical raw JSON；`compare.py --subset-file` 做 Linux×WASM 同结果 | E8 子集 10/10 一致（CI `portability-compare`；main `a8d6ad8a`） |

**不在本单**：`wasm32-wasi` 宿主、Web Worker 多线程、浏览器文件系统（OPFS）。

### 12.22 调用栈：脚本函数调用不吃原生栈（2026-11-19 ✓ 收口）

**状态 ✓**：S1–S4 全部达成——① 回调深度守卫（统一 `call_depth` 覆盖内建→脚本路径＋专项测试）；
② **显式帧栈**（`execute` 驱动循环压帧／弹帧，脚本调用不吃原生栈；深递归 900 在 CLI／ABI／wasm
三处通过）；③ 深度语义对齐参照（默认 **1000**、`setrecursionlimit` **可升**、安全上限 **10000**、
超限可捕获 `RecursionError`）；④ wasm 小栈验证（`tests/wasm/e6_recursion.js` 进 CI）。**`D1`（round 289）**：
「内建→脚本」回调路径改为**宿主声明预算**（`PYAWA_NATIVE_DEPTH`：CLI 512 MiB 线程与对拍 harness
声明 1000 ⇒ 与参照对齐；ABI／嵌入／wasm 未声明默认 64）⇒ `DIV-11` 收窄为"未声明预算的宿主"。
可见语义见 round 289 台账与 `tests/conformance/divergences.md` 的 `DIV-11`。

**round 288（A-0，2026-11-28）补充**：S2/S3 只管"脚本帧"；**不经过 `execute` 的内部原生递归**
此前无守卫（环状容器 `a=[]; a.append(a); b=[]; b.append(b); a==b` 实测 debug/release CLI 都
`stack overflow, aborting`；参照报 `RecursionError`）。已给 `equality_public`／`values_equal`／
`key_hash_checked`／`hash_object`／`order_of`／`advance_iterator` 加**深度参数守卫**（口径用现成
`recursion_limit`，超限抛可捕获 `RecursionError`），错误经 `Result` 全链传播；**未**改
`MAX_NATIVE_DEPTH`／`MAX_CALL_DEPTH`，A（原生→脚本显式帧栈）未做。
正文归档：`docs/rounds/04-post-m6-01.md` 的 round 288／289 节。

**round 290（编译期栈守卫）**：parser/emitter 仍是 Rust 递归，深嵌套源码此前在 debug CLI 实测
`stack overflow, aborting`（参照抛 `MemoryError: Parser stack overflowed - Python source too complex
to parse`）。已加**实际栈用量预算**（`PYAWA_PARSE_STACK`；默认 512 KiB，CLI／对拍 64 MiB），
parser 递归入口（表达式/一元/原子/语句/模式）超预算即报同文案 `CompileError::Memory`（
`DIV-15`），不再撞穿原生栈。

### 12.23 Windows 原生 CLI：**已落地（2026-11-17，选项 A）**

**现状**：`crates/pyawa-runtime/src/bin/pyawa.rs` 依赖**真实能力域 provider**（`fs`／`clock`／`net` 的
Unix 实现）——读源码、`.pyac` 缓存、能力注册——因此 CLI 实现整体 `#[cfg(unix)]`；非 unix（Windows／
wasm）只留**显式「未接」入口**（退出码 2，`PLAN §12.20` W1）。W2–W5 已把三个 provider＋`nt`／
`sys.platform` 落地，端到端经 ABI（`paL_newstate`）在 Windows 通过 CI 19/19。

**阻塞点（按成本排序）**：
1. **CLI 直接依赖 `fs_posix::PosixFs`／`clock_system`／`net_posix`**：需改成平台中立入口（见选项 A）；
2. **源码读取必须走能力层**（`IM-15`：禁止 `std::fs` 直读）⇒ Windows CLI 必须先注册 `fs` 再经
   `fs` 读取；最小改动是把"组合根装配＋读源码＋执行"下沉成 `pyawa-runtime` 的公共函数；
3. **`.pyac` 缓存路径**与 CLI 的 fs 读取耦合（缓存读写、陈旧判定都在 CLI 里）；
4. `sys.platform` 已是 `win32`（W5），但 `os` 的 Windows 分支只验证到 `import os` 面；
5. 非 unix 的 `main` 现在是"未接"桩，启用后要按平台选择 provider（Unix／Windows）。

**选项与代价**：
- **A（推荐）**：在 `pyawa-runtime` 增平台中立 `install_default_capabilities(state)` ＋
  `read_source_through_fs(...)`／`eval_file` 一类公共入口；CLI 只调这些入口、不再直接 import provider。
  **估**：200–400 行重构 ＋ Windows 端到端 CLI 测试（`<file>`／`-c`／`check` 三形态、退出码、
  错误消息）；**风险中**（触 CLI 主路径，Linux 侧需逐字节不变）。
- **B**：保留 Unix CLI，另建 `pyawa-cli-windows` 薄 crate；**不推荐**（两套 CLI 行为会漂）。
- **C**：Windows CLI 用 `std::fs` 直读源码：违反 `IM-15` ⇒ **不做**。

**建议**：选 **A**，排在 W6 剩余项之后；验收＝Windows 上 `pyawa <file>`／`-c`／`<file>` 的
`check` 三形态与 Linux 同 stdout／退出码，且仍经能力层读源码（可用"不注册 fs ⇒ 如实失败"反证）。

**落地（2026-11-17）**：`pyawa-runtime` 的 `PlatformFs`／`PlatformClock`／`PlatformNet` 别名 ＋
`host_cli` 的 `not(target_family = "wasm")` 门控 ⇒ 一份 CLI 源码跨平台；`read_through_fs` 经
`guest_path_bytes`（Windows `D:\…` ⇒ `/D:/…`）仍走能力层；CI `windows` job 三形态冒烟
（`-c`／`<file>`／`check`）**首跑 run `44064955063` 即绿**。详见 rounds 的 12.23-Windows-CLI 节。

### 12.24 Windows-only C 模块的最小导入面（2026-11-18 **首批已落地**）

**状态**：`winreg`／`_winapi`／`msvcrt`／`_overlapped`／`_multiprocessing` 五个诚实桩（名字齐、
未接调用如实 `NotImplementedError`；非 Windows 零注册）＋ `windows_winstubs_shapes`；配合
`socket`／`os` 的 Windows 表面过滤与 `pa_setpath` 的 guest 转换，conformance Windows 从 198/408
收敛到 **0 新差异（必需步骤绿）**。正文归档：rounds 的 12.24-Windows-C-模块最小导入面 与
12.20-W6 ✓ 收口 节。

**状态（2026-11-19）✓ 首批收口**：① `winreg` **真实 Win32 语义**（`pyawa-runtime::win32_reg`，
便利装配路径替换 stdlib 桩；`windows_winreg` ABI 测试）；② `_winapi`／`_overlapped` **最小真实
语义**（真事件／IOCP／句柄生命周期，`pyawa-runtime::win32_overlapped`；`windows_overlapped` ABI
测试）——两者均进 CI `windows` job 且全绿。
**仍不在本单（如实）**：重叠 I/O **数据面**（`ReadFile`／`WSARecv` 真实投递 ⇒ Windows Proactor
事件循环）、注册表安全描述符与 `OSError.winerror` 属性面、长尾值类型。正文归档：rounds 的
12.24-real-winreg 与 12.24-real-overlapped 节。

### 12.25 runtime 运行矩阵（2026-11-18 **已落地**）

**状态**：`windows` job 的 `cargo test -p pyawa-runtime --no-fail-fast` 已是**必需步骤且绿**
（290 通过／0 失败）；12 个 Unix 参考面测试文件按平台 `#![cfg(unix)]` 诚实 gate；`asyncio_smoke`
依赖真实 Windows Proactor（随 §12.24 真实语义落地后解锁）。正文归档：rounds 的
12.25-runtime 运行矩阵 节。

### 12.26 脚本可移植门与异步后端（契约；asyncio 按 ADR-002 仅报告）

**判据唯一出处（本节）**：可移植子集内脚本在声明的**运行档**平台上**同脚本同结果**；CI 必需
job。异步/`asyncio` 的定位见 `ADR-002`：它是**可选的 CPython 语义 facade**，其实现架构不驱动
内核与平台承诺；相关探索只作 spike 报告。契约定义见 `REQUIREMENTS.md`「脚本可移植性承诺」与
`DESIGN.md` §14。

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0-1 文档** | 本单 ＋ REQUIREMENTS／DESIGN §14／RELEASE／`DIV-13`（用户批准后写入） | docs-only 提交，`check.py` 20/20 |
| **P0-2 portability 门（OS 档）✓（2026-11-21 已进 main）** | `tests/portability/`（24 语料）＋归一化执行器＋CI **必需** Linux×Windows 矩阵 job＋比对 job；白名单与 known-diffs 机制 | **24/24 一致**（main `7d1b0f0e`；run `44788992583` 比对 0 差异） |
| **P0-3 asyncio Windows 默认后端——不采用（仅报告）** | dev 上的 selector/引擎后端实验（导入后钩子）**不进 main** | 报告：`docs/rounds/spikes/2026-11-21-asyncio-windows.md`；依据 `ADR-002` |
| **P1 wasm 库嵌入档 ✓（2026-11-21 已进 main）** | `wasm32` 构建产物＋JS 宿主（clock/可选 fs stdout）＋E7 库嵌入＋E8 可移植子集；**asyncio-on-wasm 单列为评估** | main `4f9490cc`（E1–E5）＋`a8d6ad8a`（P1）；E2–E7 PASS、子集 10/10；CI wasm job 绿 |
| **P2 wasm async（asyncio-on-wasm）——仅报告，不采用** | JSPI/Asyncify 探索与 E9 原型 | 报告：`docs/rounds/spikes/2026-11-20-jspi.md`、`2026-11-20-asyncio-wasm.md`；依据 `ADR-002` |
| **P2 移动端库嵌入档** | `aarch64-linux-android`／`aarch64-apple-ios` 的 `cargo check`＋staticlib/cdylib 产物＋ABI 冒烟 | 构建档绿；运行档另行接入；`DIV` 如实登记 |
| **P3 平台事实** | `nt.splitroot`／`normpath`、`errno` 表等：能补则补，不能补进 harness 白名单 | portability job 绿 |

**portability 门状态 ✓（2026-11-21 进 main `7d1b0f0e`）**：`tests/portability/` 落地（24 个脚本：语言／异常／`os.path`／文件／
`os.stat`／errno／`socketpair`／TCP 回环／`time` UTC／json／collections／itertools／re／math／
dataclasses／enum／import 面）；`runner.py` 单平台执行＋**只在 harness 内**归一化平台事实（运行时
仍如实回答），`compare.py` 做 Linux×Windows 同结果比对；CI 新增 `portability` 矩阵（ubuntu／
windows 各跑 runner 上传产物）＋ `portability-compare`（必需，未登记差异即红）。**首跑修掉两个
harness 问题**：Ubuntu 26.04 无 setup-python 3.14.4 构建（改用系统 Python）＋ Windows 控制台
cp1252 打印中文失败（runner 输出固定 UTF-8）。**实测：24/24 一致、0 已知差异**（run `44788992583`）。正文归档：rounds 的 12.26-P0-2-portability 门 节。

**硬线**：每步 commit；动代码必 `check.py` 20/20 ＋ `slowcheck` 十二项全绿；最终 CI
fast／wasm／windows 全绿；同一处 3 轮不过则停手升级。**开发以本节＋REQUIREMENTS 契约为准**
（冲突时 REQUIREMENTS 是决策源）。

### 12.27 宿主事件循环与协程驱动（ADR-005，accepted 目标架构）

**判据唯一出处**：`DESIGN-event-source.md`；用户 2026-11-21 拍板长期目标＝L2（宿主拥有事件
循环），L1 为底座，L3 不承诺。**accepted 只表示目标架构**，实现前须走完 S1/S2。

| 阶段 | 内容 | 验收 |
|---|---|---|
| **S1 规格 ✓（2026-11-21）** | `event` vtable＋`pa_resume`/`pa_throw` 的 ABI 规格（状态码、栈契约、取消、GC 根）；host awaitable 不在 S1（`AB-86`） | `SPEC-capabilities.md` §9.10（`CP-40`…`CP-46`）＋`SPEC-c-abi.md` §15.6（`AB-82`…`AB-87`） |
| **S1.1 规格 ✓（2026-11-21）** | host awaitable 对象协议：`await_fn`/`await_cancel_fn`、token/恢复/异常/取消生命周期、追加式 ABI | `SPEC-c-abi.md` §11.1（`AB-88`…`AB-91`）；T-AB-24…26 |
| **S2 Spike C ✓（2026-11-21）** | 宿主 loop 驱动含 `await` 的脚本协程；yield/恢复/完成/异常/取消五态；参考 C 宿主 | 报告：`docs/rounds/spikes/2026-11-21-host-awaitable-loop.md` |
| **S3 实现 ✓（2026-11-21）** | `pa_resume`/`pa_throw`（1.15）、`event` 域（1.16）、`await_fn`＋VM await＋`await_cancel_fn` 派发（1.17）；参考 C 宿主（resume/event/await，event provider 参与 token 等待）＋测试；旧 ABI 宿主零回归；多协程同对象/按帧取消/u64 token 测试 | `crates/pyawa-abi/tests/`（coroutine_resume／event_domain／host_awaitable／host_awaitable_cancel／legacy_host_ops）；`tests/ci/t_ref_host_async.py`；check.py 20/20；slowcheck 12/12；CI 全绿 |
| **宿主 loop 参考适配器 ✓（2026-11-22；await 场景补齐 2026-11-27）** | native `poll(2)` event provider、任务表、超时，以及宿主 loop 驱动含 `await` 协程的正常完成／多次 resume／错误与 THROW 传播／INTERRUPT 取消与恢复／DESTROY 派发；`examples/reference_host_loop.c`＋`t_ref_host_loop.py` | 真编译真运行；CI fast |
| **asyncgen/嵌套 await ✓（2026-11-22）** | `asend` 内 host await token 冒泡＋外层送值路由；`host_await_asyncgen.rs` | 测试全绿 |
| **Proactor 数据面计划 ✓（P1–P4 已落地；P5 规格＋provider＋guest 面已落地）** | Windows Proactor／subprocess／pipe 数据面：P1 文件/pipe ＃69／＃72、P2 socket（AcceptEx/ConnectEx/BindLocal）＃79、P3 subprocess ＃72／＃79、**P4 默认策略已切 Proactor**（round 282）；**P5 IPv6：`CP-47` 规格已定 ＋ `R2` Windows provider 数据面已接（round 285：socket/bind/connect/accept/getsockname/getpeername、ConnectEx/BindLocal、inet_pton/ntop、IPV6_V6ONLY）＋ `R3` guest `_socket` 地址线（4 元组／18B wire）与 asyncio `::1` e2e 已接（round 286）**；边界：只接**数字** IPv6（DNS／`getaddrinfo` 名字解析不在 v1）、不带 `scope_id`／`flowinfo`（link-local 另立条款）、`inet_pton`／`ntop` 的真实实现只在 Windows CLI | 计划：`docs/rounds/spikes/2026-11-22-proactor-dataplane-plan.md` |
| **Windows asyncio 评估＋selector 落地 ✓（2026-11-22）**；**round 282 切回 Proactor 默认 ✓** | 2026-11-22 selector shim（P1–P3 未接时的临时策略；Windows CI `asyncio.run` 冒烟）；P1–P3 全绿后按 `P4` 删除 shim，CLI 走 CPython 默认 Proactor（round 282），selector 保留为脚本显式逃生口 | 评估＋代码；见 `docs/rounds/spikes/2026-11-22-windows-asyncio-assessment.md`、`docs/rounds/spikes/2026-11-22-proactor-dataplane-plan.md` |
| **S4 文档/承诺 ✓（2026-11-21）** | DESIGN/PLAN/RELEASE/NEXT 收口；CLI asyncio 模式 B 不变，L3 不承诺 | 文档一致；本行即收口记录 |


