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
  **裁定（第 270 轮，用户）** ✓：**下限**就按 ① 的**自建用例**定 ✓ —— 总数 **≥ 112** ✓ （现 **407** ✓），且 **M2 内容面各设下限**：类 **≥ 15** ✓（现 16 ✓）／异常 **≥ 11** ✓（现 13 ✓）／import **≥ 14** ✓（现 15 ✓）／生成器 **≥ 4** ✓／描述符 **≥ 3** ✓（现 4 ✓）／元类 **≥ 2** ✓（现测值即下限 ✓ ⇒ 只许涨、不许落 ✓）。② 的 `Lib/` 语料**仍暂空** ✓（依赖 M3 ✓）⇒ 已登记 ✓。
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
>   实测 **407/407 通过、已知差异 0、新差异 0**，四要素齐；语料 **407** 条与在册差异 **3** 条（`DIV-1`／`DIV-2`／`DIV-8`）未变；
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
>   （现 **407** 例，含 `Lib/` 165＋`Lib/test/` 50；来源列格式 A 见 `tests/conformance/corpus/manifest.tsv`）＋ `MS-19` 的在册差异（现 **3** 条：`DIV-1`／`DIV-2`／`DIV-8`）。
> - **已兼容面（当前实况）**：`Lib/` **逐字节同步 739** 个文件（`--check` 全一致 ✓；判据① 的分母仍是上游顶层 **628**）；
>   可 import 比例 **477/628（76.0%）** ⇒ **判据① 已达标** ✓（阈值 421／67%；复核用
>   `tools/lib_import_ratio.py`——该仪器**单次读数有 ±1 噪声**，判涨跌要多跑取众数 ✓）；C ABI `AB-1`…`AB-64` 可用；C 模块面按 `CM-14` 的 fan-in 增长，
>   未提供的走 `CM-6`（`ImportError`，如实）。**能力层**已正式化（M5）：`T-CP-*` 13/13、两 provider 写法不变、体量阻塞阈值已拍板
>   （`DESIGN.md` §9 的 M5 小节）。**兼容性验收**已达成（M6）：`MS-14` 报告 407/407、新差异 0；`MS-19` 补充（合法 Python 被拒）已消除
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
>   语料 **407** 条与在册差异 **3** 条未变；如实缺口见 §6 的「M6 的要求」。

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
| **P3-27** | **`Lib/test` 语料化**（**主体已落，转回归扩张**）（**判据② 的前提**）：三批共落 **407 条**（自建 192 ＋ `Lib/` 165 ＋ `Lib/test/` 50；覆盖 **150** 个不同可 import 模块），来源列格式 A（manifest 第 5 列）＋出处守卫；**机检仪器** `tools/corpus_coverage.py`（五项：总量≥400／Lib 覆盖≥150／Lib.test≥40／对拍 0 新差异·自检绿／MS-14 四要素）已接进 `tests/ci/check.py` 成 **M3-COVERAGE 硬门** ⇒ **判据② 已裁定达成** ✓。此后按回归需要继续扩覆盖（不阻塞判据） | `MS-13` ②、`MS-14` | 无（机检仪器已就位 ✓） |
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

### 9.4 下一阶段的开工指示（**非规范**）

> **取活口径（2026-11-12 用户，覆盖此前「磨套件」口径）** ✓：**主目标＝内存安全 ＋ 执行效率**——**解释器必须满足的** ✓；**其余全能慢慢补** ✗。
> ① **内存硬门** ✓：`python3 tools/memdangle_survey.py`（`PYAWA_DANGLING=1` 全量套件）**计数归零** ✓ ＋ 全库**无 abort／segfault／堆损坏** ✓（`AB-29`／不变量 3）。
> ② **执行效率** ✓（姿态见 `REQUIREMENTS` 的「解释器硬要求」行 ✓）：判据**三轴**——(a) **纯计算/调度**：`tools/bench_hot.py` **基线入库** ＋ **CI 禁回退**（>10% 红 ✓）；与 CPython 的**比值必须报告** ✓，目标 **≤5×** ✗（**当前 135–226×** ✗ 属病态，先把病态成本找出来）；(b) **能力路径**：同一工作负载下**不得慢于「CPython ＋ Python 层代理」** ✓（这是卖点 ✓）；(c) **启动/常驻**：保持 `DESIGN` §9 基线的优势 ✓。**禁止超线性**热点 ✓。**与 CPython 的比值必须报告** ✓（目标比值待用户拍 ✗）；**禁止超线性**热点（单用例耗时上限 ✓）。
> ③ **其余排后** ✗：日常库缺口（`json.load(f)`／`defaultdict`／`match` 同行 suite／`urllib.request`）· 冷门语义（`asyncgen`／`dis`／`traceback`／`unicode_identifiers`／PEP 646·695）· **官方套件百分比** —— **都不是判据** ✗，有空再补 ✓。

> **A→B→C 次序（2026-11-12 用户批准 ✓）**：
> **A 效率（主线 ✓）**：A1 热点定位（**已做 ✓**：`type_named` 按名字查类型 ≈20% ✗ ＋ libc `getenv` 2.18% ✗）→ A2 最小改动（**一次一处** ✓，每处一笔提交）→ A3 每轮复测 `bench_hot` 倍数 → A4 立回退闸（基线入库 ＋ 回退 >10% 红 ✓）→ A5 把 GC 保活（`test_weakref` 169 s ✗）并入 A 线。
> **B 内存（守成 ✓）**：B1 `tools/memcheck.sh` 子集进每轮闸门 ✓ · B2 全量 valgrind ＋ 全量套件 memdangle 做「阶段边界 ＋ 棘轮（不得更差）」✓ · B3 形参所有权限时 **3 轮**探针 ✓（成→估得出；败→升级记档）。
> **C 其余（明确不做 ✗）**：日常库四项／冷门语义（`asyncgen`／`dis`／PEP 646·695）／官方套件百分比／ABI 发布（用户决定 ✗）。
> **估时口径**：只报**可测单位**（倍数／errors 数／ok/day ✓），**不报「几周」** ✗（人月神话 ✓）。

> **完整工单（2026-11-12 一次性下达 ✓；此后逐轮自走，不再逐步请示 ✓）**——总口径：硬要求＝内存安全＋执行效率（**效率 214× → 22× ✓，已由 slowcheck 第 11 项防回退 ✓ ⇒ 效率线关闭** ✗，只剩「结构特化」大工程，不做 ✗）；其余（C 线）慢慢补 ✗。
> 1. **B3-2 诊断**：`PYAWA_REFCOUNT_CANARY=1` 跑 memdangle／套件 ⇒ 产出「借用计数下降／上升」站点清单，修掉其中便宜的 ≤3 处；清单入台账。
> 2. **B3-3 收口**：`dunder_call_native` 的双重释放族（round 224／227 同类）逐项归还 ＋ 异常路径整改。
> 3. **形参所有权老账（13 处）**：只修**有最小复现**的；无复现记档 ✗（不做无靶子重构）。
> 4. **`test_weakref` 的 GC 保活**（169 s ✗ → <120 s）：根因＝traceback 帧把被测局部保活 ✓。
> 5. **C1 日常面四连**：`json.load(open(f))` ✗／`defaultdict` ✗／`match` 同行 suite ✗／`urllib.request` ✗。**（2026-11-13 已全部转绿 ✓）**
> 5b. **真实脚本冒烟抓到的 3 个库缺口**（2026-11-13，10 个真实脚本 7/10 ✗）：① `pathlib.Path.write_text/read_text` ✗（走 `io.open`，`_io` 真文件面未接线）；② `csv` 模块缺 ✗（`CM-` 尾）；③ `subprocess` 的真执行 ✗（`_posixsubprocess.fork_exec` 未接线、`capture_output` 的 fd 处理报 `open: path should be string`）。
>   —— 语言面已无问题 ✓（`match`／`except*`／`\N{…}`／socketpair／argparse／re 全过 ✓）；**这些是库面长尾** ✗，按 `CM-14` fan-in 取。
>   **三者的定位要分开 ✗（2026-11-13，按 `DESIGN` §1 的定位对齐）**：① `io.open`／`pathlib` 文件面 = **脚本相关，必修** ✓（脚本读写文件走宿主授予的 `fs` 能力 ✓）；② `csv` = **低优先**（宿主可提供／脚本可手写 ✓）；③ `subprocess` = **非目标，不修** ✗（沙箱禁 fork ✓；`proc` 域须宿主显式授予，`DESIGN` §1 已列为非目标 ✓ —— **不是缺陷** ✓）。
>   ⇒ **「用真实脚本 7/10」是 CPython 替代品的尺子** ✗；本项目**没有**「必须能替代 CPython」这个尺子 ✓（`DESIGN` §1 非目标 ✓）。**但也不能反向把项目定成「脚本运行时」** ✗ —— **业务用途是用户当下的用途** ✓，不是项目的排他定位 ✗（项目定位的唯一出处＝`DESIGN` §1）。


> 6. **C2 冷门语义**：`\N{…}`／`except*`／async 推导式／PEP 695／`import *`／下标删除 —— 按 `MS-19` 判据补充**必须修** ✗。
> 7. **全量内存棘轮**（阶段边界做一次）：全量 valgrind（分批）＋ 全量套件 memdangle ⇒ 记基线 ＋ 「不得更差」。
> 8. **ABI 发布** ＝ **用户决定** ✗（到点先报，不自行发布）。
> **测量纪律（2026-11-13）** ✓：**bench／profile／slowcheck／全量 survey 只在机器空载时跑** ✓；观察 `cat /proc/loadavg`，负载 > 核数×0.7 一律**跳过并明说** ✗（并发测量无效，硬判会误红 ✗；`bench_gate.py` 已内置该守门 ✓）。
> **每轮固定动作**：仪器（`profile_hot`／`memcheck --limit 20`／`bench_gate`）＋ 闸门（`check.py` 16 项 ＋ `slowcheck --release` **12 项**）＋ 同笔提交（代码＋台账＋`NEXT`，`NEXT` ≤40 行）＋ 报告三行（做了什么／可测单位前→后／未修与下一步）。
> **只需请示四种** ✗：设计决策（ABI 发布、immortal 之类）／突破安全边界／需要用户权限或工具／同一处 3 轮不成。**其余一律自走** ✓（选题顺序、实现方案、拆几笔提交，只要满足「一轮一落地」）。
> **停止条件**：同墙 3 轮停手升级 ✗；每 2–3 轮重跑 survey／bench／memcheck 并报三数 ✓。

> **工单收口后的下一阶段（2026-11-13；用户确认 8 项全部完成 ✓ —— 全量 valgrind 410 例 0 error ✓、memdangle 基线 2（既有、A/B 证）✓、bench 空载复测 +0.8~5.6% 在容差内 ✓）**：
> ① **每实例内存配额** ✓（**round 266 落地**：`OM-41`／`AB-65`／`AB-66`；超限抛**可捕获的** `MemoryError`；宿主占用上界 ≈ `limit+slack`；CLI `PYAWA_MEMORY_LIMIT` 示例接缝；验收 `crates/pyawa-core/tests/memory_quota.rs` 5 例 ＋ `crates/pyawa-runtime/tests/memory_quota.rs` 5 例 ＋ `crates/pyawa-abi/tests/memory_quota.rs` 4 例 ✓）—— 沙箱**硬需求** ✓；**注意**：配额是**宿主侧的 ABI 钩子** ⇒ 属 ABI-adjacent ✓，应与 ⑤ 的钩子**一起定** ✓（ABI 仍暂不发布 ✓）。
> ② **威胁模型与宿主责任** ✓（**round 267 落地**：`DESIGN` §7.5 —— 四目标（不逃逸／不越界／不拖垮宿主／不串实例）、VM 已保证（无 FFI 出口＋静态 `T-CX-*`、内省只碰 VM 对象、能力面依赖倒置、栈溢出干净异常、内存安全基线＋配额）、宿主必须承担 7 条（壁钟超时／配额／同实例串行／provider 根夹紧／资源策略／有界输入／实例隔离）、残余风险 6 条如实记 ✗）；
> ③ **`io.open`／`pathlib` 文件面** ✓（**round 268 落地**：`_io.text_encoding` 真接线、`FileIO.__del__` 落盘、`symlink` 三参、errno 文案扩充、`with (expr).attr` 解析修复（`MS-19`）；验收 `crates/pyawa-runtime/tests/io_pathlib_face.rs`（与 CPython 27 行逐行一致）＋`crates/pyawa-core/tests/with_parenthesized.rs` 4 例 ✓；如实缺口：`open_code` 占位、绝对路径输出不进对拍）；
> ④ **通用参考宿主** ✓（**round 269 落地**：`examples/reference_host.c` ＋ `docs/REFERENCE-HOST.md` ＋ 可执行验收 `tests/ci/t_ref_host.py`（真编译真运行、stdout 逐行对拍、`pa.h` 的 `AB-65/66` 同步检查）。演示：宿主定义脚本 API → 脚本调回宿主 → 沙箱拒绝越界 → 协作式超时 → 每实例配额／指标 → 实例隔离 → 销毁；**与宿主身份无关**（无业务逻辑、无命名假设）✓；如实边界：C 宿主暂无标准库（`paL_openlibs` 未接线，P3-14），沙箱用宿主 API 白名单演示，能力域 vtable 样例见 CLI provider）；
**钩子族清单（一处真相＝`SPEC-c-abi`）**：`AB-69` 宿主函数签名（含生命周期：setup 阶段注入 ✓）／：`AB-67` 宿主模块注入 **✓ round 271 落地**（`pa_add_host_module`／`pa_module_addfunction`／`pa_module_setconst`／`pa_module_addsubmodule`；保留名／隔离／回调守卫／不透明句柄；验收 `crates/pyawa-abi/tests/host_module.rs` 8 例）／`AB-68` 宿主设每实例 `sys.path` **✓ round 270 落地**（`pa_setpath`；最小 `sys` 骨架；验收 `crates/pyawa-abi/tests/host_path.rs` 5 例）—— `AB-69` **✓ round 273 收口**（round 272：`pa_type_expr` 数据模型＋`pa_func_set_signature`＋边界运行期检查；round 273：**检查期调用点检查**（`compile_with_host_functions`＋`TS-47` 第二插入点）✓；验收 `crates/pyawa-abi/tests/host_signature.rs` 10 例）—— 钩子族三项全部落地 ✓。
> **钩子族实现需求（给开发；2026-11-13 用户批准 ✓）**——语义细节以 `SPEC-c-abi` 的 `AB-67`／`AB-68`／`AB-69` 为**一处真相** ✓；> 此处只给**顺序／验收／禁区** ✓。**顺序**：`AB-68`（最小，热身）→ `AB-67`（核心）→ `AB-69`（依赖 67 的 host_fn）；一次一件 ✓。
> **`AB-68` 验收**：① 宿主设两个目录 ⇒ 脚本能 import 两处模块 ✓；② `sys.path` 语义与 `IM-*` 完全一致（不得另立一套 ✗）；③ 读文件仍被 `fs` 能力夹根（`../..` 拒 ✓）；④ 脚本自己改 `sys.path` 语义不变 ✓；⑤ 两实例互不影响 ✓。
> **`AB-67` 验收**：① `import myapp`／`from myapp.api import f`／`myapp.api.f()` 三形态一致 ✓；② 宿主函数／常量／子模块都能装 ✓；③ 两实例拿到的模块互不相同（`AB-64`）✓；④ guest 拿不到任何 `pa_*` ✓；⑤ 保留名（`sys`／`builtins`／`io`…）注入 ⇒ `PA_ERR_INVALID` ✓；⑥ 宿主返回对象在脚本里只能是**不透明句柄**（`CP-10`，不得内省 ✗）；⑦ 宿主函数抛错 ⇒ 脚本侧可捕获异常、panic 不穿 FFI（`AB-29`）✓；⑧ **回调内调 `pa_add_host_module` ⇒ 返回错误码而非 UB** ✓（实现加 in-callback 守卫 ✓）；⑨ 生命周期：setup 注入 ✓、两次调用间可加 ✓。
> **`AB-69` 验收**：① 声明签名后，有标注插件调用点**检查期**抓错 ✓；② 无标注插件在**边界运行期**抓错并归责插件 ✓；③ 不声明 ⇒ 与今天完全一致（不检查）✓；④ 签名含宿主不透明类型时仍不暴露内部 ✓；⑤ 可选／变参 flags 至少各一个用例 ✓；⑥ **宿主类型可进签名**（`pa_type_expr_named` 同一性＝类型对象 ✓、不暴露内部 ✓、子类型按注册声明 ✓）；⑦ **泛型擦除口径**（身份擦 `TS-35` ✓／检查留 `TS-47` ✓，`isinstance(x, list[int])` 同 CPython 报 `TypeError` ✓）。
> **禁区** ✗：不做通用 import 钩子（`sys.meta_path` 式，无需求）；不冻结 ABI（用户口径）；不实现宿主库语义（宿主的事）；不做 `importlib.reload` 语义（宿主模块以重新注入代替）。
> **硬约束**：无第三方依赖（`CX-`）✓；`unsafe` 最小化＋`SAFETY` ✓；`T-CX-3/4/5` 不得违反 ✓；`bench_gate` 不回退 >10% ✓；`memcheck --limit 20` 0 errors ✓；conformance 全绿 ✓；每笔提交按本节「每轮固定动作」✓。
> ⑤ `ABI 发布` ＝ **暂不发布** ✗（2026-11-13 用户：**可能还要加几个钩子** ✓）⇒ 等钩子稳定再发 ✓；且 ①（配额）与钩子族同属 ABI-adjacent ⇒ **应在冻结前** ✓。





> **已拍板 ✓（2026-10-18，A1–A4 按建议）**：`P3-29` socket 的四个拍板点 —— **A1** 新增 `CP-38`（`socketpair` 槽 ✓；
> vtable 加槽，ABI 未发布 ⇒ 现在最便宜 ✓）· **A2** `getaddrinfo` **推迟** ✓ · **A3** 地址线 v1＝
> **IPv4 4B ＋ 端口 2B 大端**，v1 只 **`AF_INET`／`AF_UNIX`** ✓（`CP-39`）· **A4** `fd` 沿用 `fs` 先例（`CP-31`）✓。
> 决策记录的唯一出处：`REQUIREMENTS.md` 的「socket 首片」行 ✓ —— **可以开工** ✓（`SPEC-capabilities` 的槽位表与
> `CP-38`／`CP-39` 已补 ✓）。

> 规则**不在这里复述**——每条都给出处，避免第二个真相源。本节只给**顺序、边界与复核清单**。

> **surrogate epic（`P3-38`）开工指示（2026-10-25，**用户已拍板** ✓）**：`str` 表示层由"仅合法 UTF-8"
> 迁到 **WTF-8**（`str` 可持**合法孤立代理**）。**顺序固定**：S1 `wtf8.rs`（编解码基础，零风险）→
> S2 `StrObject` 存储 ＋ `text_of` 语义 → S3 API 面**逐片**迁移（按审计报告分片，**禁止**一次性大改）→
> S4 lexer 转义 ＋ `surrogateescape`／`surrogatepass` → S5 `Lib/test` 重试同步 ＋ survey 回归。
> **每阶段都是可提交的独立切片**；S2 之后每片必须保持既有 `text_of` 调用点全绿再进下一片。
> **开工前先出 API 审计报告**（S3 片单的来源：`text_of`／`repr`／`print`／`encode`／`decode`／比较／
> 哈希／切片／格式化／属性名／字典键／fs 路径／正则／`unicodedata` 的调用点清单）；它**不单独成一轮**。
> 阶段细节与逐阶段验收**只在 §9.2 的 `P3-38` 行**（一处真相，本处不重述）；本条完成是 **M6 收口 gate
> 的一部分** ✓（见该行的 gate 注）。

> **取活优先级（M5 期，2026-10-18）** ✓：**M5 判据③ 的体量（`P3-28`）→ 判据①②（`T-CP-*` 核对 ＋
> 两 provider 写法不变）→ 并行线（B 批 `BC-62`／`BC-63`＋`TS-49` · 类型级描述符专项 · C 面）→
> socket 首片（**已拍板 ✓**：`socketpair`，按 `CP-38`／`CP-39`）**；同一处 bug 按 `round-rule.md` §4 的
> 三轮上限**停手升级**；**弃支不烧轮次**。
> （M3 期的"C 模块缺口 ＋ `next_work` 队列"**已完成使命** ✓ —— 判据① 已达标 ✓、判据② 已裁定 ✓）

> **提交**：逐轮记录、台账与进度注记**必须随当轮的实质改动一起提交**，**禁止**纯台账轮次
> （`agents-rules/commit-rule.md` §1）✓

**M5 开局顺序（2026-10-18；已于 round 10–13 全部完成 ✓ 归档，仅供追溯 ✗）** ✓

> **真实脚本冒烟清单（每轮抽一个真跑）** ✓：日志分析（`re`＋`json`＋`Counter`＋`with open` ✓）· BFS（`pop(i)` ✓）·
> JSON 数据处理（`dataclasses.asdict` ✓）· 命令行小工具（`sys.exit` ✓）· 目录遍历（`deque`／`pathlib` ✓）。
> **红字即记一条缺口** ✓（并按 `MS-19` 的新增补充：**合法 .py 被拒 ⇒ 必须修** ✗）。
> 当前已知必用面缺口（2026-10-31 复测 ✓）：**`json.load(f)`** ✗ · **`deque` 迭代** ✗ · **`defaultdict`** ✗ ·
> **异常 `errno` 属性** ✗ · **`match` 的同行 suite**（`case [1,*r]:` ⇒ "冒号后面要换行"）✗ ——
> 其余旧缺口（`sys.exit`／`list.pop(i)`／同行 `try:`／`dataclasses.asdict`／`complex`／下标删除）**已修** ✓。

1. **`P3-28` 实例体量三档实测**：① 裸实例 ② 导入 `site` ＋常用模块的实例 ③ **N 个实例并行**
   （每实例增量 ＋ 总常驻 ＋ 并行度）；落点 `tools/measure_footprint.py`。**已完成（round 10）** ✓：
   三档数字见 `DESIGN.md` §9 的 M5 小节（一处真相，本处不重述）。
   ⚠ M1 的 48 µs／1.05 ms／3.4 MB 是"**无 site／无导入系统**"时量的（`DESIGN.md` §9 的注 ✓）⇒ **已重量** ✓。
2. **判据①**：`SPEC-capabilities` §11 的 **`T-CP-*` 逐条核对**。**round 10 进度：10 绿／3 红**
   （红＝`T-CP-3` 的 `os.dup2`＋`select` 面、`T-CP-7` 后半的实例销毁关句柄、`T-CP-10` 取消令牌/worker；
   另有 `CP-21`／`CP-32` 的 VM 侧路径解析机制债）⇒ **round 11 收 T-CP-3／7（12 绿／1 红）**；
   **round 12：`T-CP-10` 收口 ⇒ 13 绿／0 红 ✓**（证据：`capability_acceptance.rs` 11 passed／
   `two_providers.rs` 7 passed／`select_dup2_shapes.rs` 1 passed／`t_cp10_cancel.rs` 4 passed／
   `capability_acceptance.rs` 的 T-CP-10 可取消等待测试；`CP-21`／`CP-32` 机制债 round 12 另落，见台账）。
3. **判据②**：写"**同一脚本、两种 provider**"的**可执行验收**（真实 provider vs 受限 provider ⇒ **写法不变** ✓；
   受限档按 `T-CP-4`／`T-CP-5`／`T-CP-7` 的形状造 ✓）。**已落地（round 10）** ✓：
   `crates/pyawa-runtime/tests/two_providers.rs`（同一脚本逐字节相同、归一后观察完全一致）。
4. **并行线**（不同写域 ✓）：**B 批** ✓（`BC-62` 多语句 `lambda` · `BC-63`＋`TS-49` 空安全操作符——round 10 落地）·
   **类型级描述符专项** ✓（`functools.wraps.__name__` 正路 ＋ `P3-17` 同族——round 10 主体落地）·
   **C 面**（CJK `_codecs_*`——round 10 落 `_codecs_jp` 一族；`unittest.mock`／`multiprocessing.heap`／`xmlrpc` 未做）。
5. **socket 首片（已拍板 ✓）**：`socketpair`，按 **`CP-38`**（槽）／**`CP-39`**（地址线 v1）；`CpNetVtable` 填槽 ＋
   接口版本随 `CP-30` 升（`PA_ABI_VERSION` 1.0 ⇒ 1.1 ✓）。**首片已落地（round 10）** ✓：`socket.socketpair()`
   端到端真 I/O；`getaddrinfo`／DNS **仍在首片外** ✗（A2 推迟 ✓）；`connect`／`bind`／`poll` 一族如实未接线。

> **round 10 状态小结（2026-10-18）** ✓：1 ✓ · 2 **部分**（10/13 绿）· 3 ✓ · 4 ✓（B 批／描述符主体／C 面 jp 族）·
> 5 ✓（socketpair 首片）。⇒ **M5 判据①②③ 尚未全绿**（③ 的阈值仍待用户拍板）；细节与未修清单见
> `NEXT.md` 与 `docs/rounds/03-m3-progress-05.md` 的 round 10 节。
>
> **round 11 状态小结（2026-10-18）** ✓：**`CP-31` 实例能力表落地** ⇒ `T-CP-3`／`T-CP-7` 收口
> （矩阵 **12 绿／1 红**，仅剩 `T-CP-10`）；C 面 core 编码委派＋`_codecs_cn` ⇒ 判据① 461/628＝73.4%。
> ⇒ M5 判据① 还差 `T-CP-10`（取消令牌/worker）＋`CP-21`／`CP-32` 机制债；细节见台账 round 11 节。
>
> **round 12 状态小结（2026-10-18）** ✓：**`T-CP-10` 收口 ⇒ `T-CP-*` 矩阵 13 绿／0 红 ✓**（M5 判据① 达成）；
> `CP-21`／`CP-32` 的 VM 侧路径解析落地（`instance/path.rs`＋NaiveFs 反证；`RestrictedFs` 夹根逻辑删除）；
> `CP-30` 注册时版本校验＋fs/clock/net vtable `version` 对齐 `PA_ABI_VERSION`（1.1）；
> C 面 `_codecs_tw`／`_codecs_kr` ⇒ 判据①（Lib import ratio）466/628＝74.2%。
> ⇒ **M5 判据② 早已 ✓；判据③ 的阈值仍待用户拍板**（基线数字见 `DESIGN.md` §9）；细节见台账 round 12 节。
>
> **round 13 状态小结（2026-10-18）** ✓ —— **M5 收口**：判据③ 的阻塞阈值经**用户拍板**（三档阈值表＝
> `DESIGN.md` §9 的 M5 小节末节）；判据①② 已有可执行证据（见 §6 的「M5 的要求」）⇒ **M5 成立**；
> 本笔更新 `README`／`PLAN`／`NEXT` 状态，`dev` 以 `--no-ff` 收录 `main` ＋ **tag `m5`**（合并提交见 `git log --merges -1`）。
> 如实缺口与后续队列见 `NEXT.md` 与台账 round 12–13 节（不阻塞 M5 判据）。

> **M1–M4 期（历史，仅供追溯）**：下面的 1–9 保留原样，**不再作为取活依据** ✗。

**顺序**（队列里所有待做的"卡在哪"都是"无"）

> **逐轮日志已移到 `docs/ROUNDS.md`（索引）与 `docs/rounds/` 各卷** ✓（本节只留指示：顺序／复核清单／每族两件事／上报／提交）

> 前提：`main`／`m1` ＝ `14b2764`（**M1 阶段边界，已打标签**）；工作继续在 `dev`。
> **已完成项见 §9.1／§9.2 的台账，本节不重复。**

1. ~~**M1 ② 的足迹报告**（启动延迟 ＋ 常驻内存）~~ **已完成**（`tools/footprint_host.c` ＋
   `tools/measure_footprint.py`；数值与口径见 `DESIGN.md` 的"Pyawa（M1 最小内核）实测基线"——
   release：VM 引导 48 µs、宿主整程 1.05 ms、峰值 RSS 3.4 MB）。`§13-19` 与 M5 需要的那个**基线**有了
2. ~~**`P1-13`（`pa_options` 过界）**：`pa.h` 定**尺寸标记**结构（`size` ＋ 检查档位 ＋ 优化级）；
   `pa_exec_string`／`pa_exec_file` 收 `const pa_options *`（**可 `NULL`**）；档位／优化级进
   `IM-19`／`IM-21`~~ **已完成**——收尾标志达成：`SPEC-c-abi.md` §15.3 的注已改为"**已满足**"
   （`AB-7`）；档位真的改发射（深层 ⇒ 边界检查），优化级进了 `compile` 的显式输入集
   （本层没有优化器 ⇒ 暂不改发射）。
3. ~~**M2 harness 的"减配首版"**（**杠杆最大的一件**）：今天**没有 CLI**（无 `[[bin]]`），但
   `pa_exec_string` 已能跑源码 ⇒ 驱动**不必等 CLI**。首版比对 **退出码 ＋ 指定全局的值 ＋
   未捕获异常的类型／消息**；**stdout/stderr 做不到**（`MS-8` 要求它们，而 `print` 要
   `sys.stdout` → `_io` → **`fs` 域**，`CM-26`）⇒ **如实标为减配版**。
   收益：此后每一步语义进展都有"**通过／已知差异／新差异**"的数字（M2 判据的形态）~~
   **已完成**：`crates/pyawa-abi/tests/conformance.rs`（`cargo test -p pyawa-abi --test conformance`；
   语料 `tests/conformance/corpus/`）——首轮 **9/9 通过 · 0 已知差异 · 0 新差异**，自检（`MS-12`）
   全绿。它当轮就抓到三处**可观察缺口**：`__name__` 未绑（**已修**：`pa_exec_string` 补
   `__main__`）、下标表达式未解析、类对象属性读未接线（后两条**第 276 轮核实均已接线** ✓：下标一族 46 条夹具全绿、`C.v` 见 §9.2 的 `P1-6` 行）。
4. **基础设施第一批**（**紧接着上面三项**；开发实测：今天跑程序的入口**藏在测试二进制里**——全仓无 `fn main`）：
   ① **最小 CLI（≈1 轮）**：跑单文件／传参／给退出码／报未捕获异常。
      **如实范围**：**还不能 `print`**（要 `sys.stdout` → `_io` → **`fs` 域**，`CM-26` **禁止** sink）
      ⇒ **禁止**为"手感"临时绕开 `CM-26`。**它是 `MS-8` 合规版的前置**——退出码／stdout／stderr
      本质上都是**进程级**比对。
   ② **闭包运行期（1–2 轮）**：闭合"**编译面领先运行面**"——编译器已产 `co_cellvars`／`co_freevars`／
      `MAKE_CELL` 一类，运行期**必须**能跑（实测信号：`COPY_FREE_VARS` 在执行器里看不出对应处理）。
   ③ **`fs` 域的真实实现 ＋ `print`**：**CLI 与 `fs` 域是同一工作流**（`DESIGN.md` §7：`pyawa-runtime`
      ＝真实机器实现）；它同时是**三处共用的前置**：`print`（`CM-26`）· `site.py` 建 `sys.path`
      （`IM-24`）· import 的 I/O（`IM-15`）。
5. **主线（大件，按依赖）**：`P1-11`（任意精度整数，**已授权"现在就做"**；注意 `int`↔`str` 的
   **4300 位上限**与 `hash` 两处**易漏的可观察连带**）→ `P1-12`（`bytes`）→ ~~`marshal`（`CM-27`）~~
   **已完成**（第 209 轮：`crates/pyawa-stdlib/src/marshal_module.rs`，自有格式版本 1、
   `loads(dumps(x))` 往返、循环引用支持、`dump`／`load` 如实报未实现；夹具
   `tools/gen_marshal_fixture.py`）→
   **`sys`／`_io` ＋ `fs` 域的真实机器实现** ⇒ **`print`** → 合规 harness（stdout/stderr 齐备）→
   **`P3-12`（import 系统）** → **M3（引入 `Lib/`）**。链条不可跳的依据：
   `Lib/importlib/_bootstrap_external.py` 第 30 行就是 `import marshal`。
6. **允许并行做 stdlib 的头部**：`CM-14` 顺序里**不依赖能力域**的模块
   （`errno`、`builtins` 的纯计算面如 `len`／`abs`／`min`／`max`／`sorted`、`__build_class__`）**可以**
   与上面并行。条件三条：① 按 `CM-14` 的 fan-in 顺序取；② 每做一个模块**就补它那段逐模块合约**
   （`CM-4`；这也是 `SPEC-c-modules.md` §12 缺口的**分批补法**）；③ **不得**降低
   `CM-5`／`CM-6` 的契约。**依赖能力域**的（`posix`／`_io`／`time` 一类）仍等 `P3-14`；
   ⚠ **`print` 不在"纯计算面"里**：它要 `sys.stdout`（`_io` ⇒ **`fs` 域**），排在其后（`CM-26`）。
   **下一份逐模块合约写 `sys`**（`CM-14` 的 fan-in 第 1）；先写**不依赖能力域**的部分
   （`argv`／`path`／`modules`／`version_info` 一类），`stdout`／`stderr` 归 `_io` 之后再补——
   `sys` 就位正是 `print`（`CM-26`）被解锁的前半步。
7. ~~**台账纪律**：`§9.2` 里已完成的项按既有惯例**就地加删除线**（`P1-9` 就是这样）。…~~
   **第 276 轮已办** ✓：`P1-6` 标收口（最后一项 `C.v` 已接线）；`P1-7`／`P1-8`／`P1-9`／`P1-13` 早已划线；
   `P1-10` 的"未接线"清单按**逐条复验**改到只剩 `set`／`dict`／`bytearray` 的就地运算；
   `P1-11`／`P1-12` 的"进行中"注据实更新（`AB-62` 已落地、切片写已接线、`float('<串>')` 与
   `bytes.__hash__` 仍缺）⇒ 口径见 §9.2 各自行 ✓。
8. **再往后**：表达式面按"**`Lib/` 需要**"的顺序补——**逻辑运算符（`and`／`or`／`not`）＋增强赋值
   （`+=` 一族）先做**（两者是 M2 语料最常见的缺口；`and`／`or` **返回操作数**且短路；
   增强赋值对**不可变**目标退化为 `a = a + b`、**目标只求值一次**，对**可变**目标
   （`set`／`dict`／`bytearray`）要走 `__i*__` **就地**语义——**未接线前如实报未接线**，
   **禁止**静默退化成重新绑定）。接着：**`break`／`continue`／`elif` ＋ `a[0].b`（下标后接属性）**
   （都小、能立刻扩语料）→ **`try`**（`Lib/` 用得极多）→ `with` → `import`（`P3-12`）→
   `lambda` → 推导式 → f-string。`**` 与 `@`（矩阵乘）都只是 `BINARY_OP` 的一个下标
   （`BC-39` 的 27 项）⇒ **补运算符时顺手带上**，不必单列。再往后 `P3-12`（`.pyac` 与 import）→
   `P3-13`（类型检查与边界的剩余：`TS-32`…`TS-39`）→`P3-14`（stdlib 分批）／
   `P3-15`（Unicode 数据表的剩余面）。**WASM 目标**（`§13-11` 已决：**需要**）——**待定三件**：
   ① 能力层的 **provider 形态**（`SPEC-capabilities`）② ABI 面（同一 `pa_*` 面在 wasm 上怎么落）
   ③ 构建目标；**时机**排在主线（第 5 项）与 M3 之后 ✓
9. **扩展特性的实现**（**A 批已落地（M4）**、B 批未做——入口见 `DESIGN.md` §2.1 的扩展特性索引）。
   排期：A 批已随 **M4** 落地 ✓；**B 批可随时开**（不阻塞 M5 ✗）。分两批——
   - **A 批（类型链）**：`TS-46`（`T?`）→ `TS-47`＋`TS-48`（变量标注即静态类型／平台类型）→
     `TS-50`（收束）。与 `P3-13`（类型检查器与边界检查）**同一处地基**，一起做省一遍。
     **已落地（M4，round 4–6）**：编译期静态检查器（`compile/typecheck.rs`）＋运行期 `CHECK_ASSIGNMENT`
     （`BC-65`）＋union 标签（`BC-24`）；验收 `tests/m4_acceptance.rs`／`tests/gradual_guarantee.rs`／
     `tests/type_check.rs`／`tests/assignment_emission.rs`／`tests/type_nullable.rs`；落地口径与如实缺口见
     §6 的「M4 的要求」。
   - **B 批（表达式／词法链）**：`BC-62`（多语句 `lambda`）· `BC-63`＋`TS-49`（空安全操作符）。
     —— **已落地（round 10）** ✓（规格已定 ✓；词法／发射器／类型检查一起改；未覆盖形态见台账 round 10）。
   - **验收**（各特性的条款里各自写了）：`BC-62` 对拍 `def` 版 · `TS-46` 对拍 `T | None` ·
     `BC-63` 对拍它条款里的等价写法表 · `TS-47`／`TS-50` 走"**编译期尽力 ＋ 运行期兜住**"。
   - **并列的两项**（同样"规格已定、实现未做"，第 276 轮补记）：
     - **`IM-35`（`sys.modules` 四条语义）**（`SPEC-imports-and-modes.md` §…；验收 `T-IM-10`）：
       **第 276 轮实测**——① **命中不重执行** ✓（`sys.modules` 里是**同一个**模块对象）、
       ③ **失败移除** ✓（导入失败后表里不留名字）⇒ **④ 删除后重导 ✗ 未成立**（`del sys.modules["errno"]`
       之后 `import errno` ⇒ `ModuleNotFoundError`；原生模块只注册一次）· ② **执行前入表**未逐条复验
       ⇒ 与 `P3-12` 的 finder 一起收口最省。
     - **`BC-61`（语法的唯一对照物）**：**必须**把 `Grammar/python.gram` **纳入同步范围**
       （`SPEC-bytecode.md:152-153` 已如此要求 ✓）——仓库现在**没有** `Grammar/` ✗ ⇒ 待办：把该文件
       **逐字同步**进来并在同步清单／pin（§2.1 第 2 步那三处）里登记。**与 `C1` 不冲突** ✓：
       `C1` 记的是"词法/语法**手写**、暂不切换 `python.gram`"，而 `BC-61` 约束的是**对照物**、
       **不约束实现手段**（`SPEC-bytecode.md` 同处写明）✓。

> **编译面与运行面不得脱节**：编译器**已经产出**的构造，运行期**必须**能跑；否则编译面的覆盖面是
> **虚的**——这与"运行期已实现、源码层写不出来"是同一件事的**正反两面**。夹具里出现"能编译、
> 跑不了"的用例时，**按缺口处理**（`MS-19` 的"尚未实现"），**不得**记成"已支持"。

> **到 `import` 能为标准库所用**的前置链（**不是新里程碑，是 M3 的前置清单**）：
> CLI／宿主 ⇒ **`fs` 域** ⇒ `print` ⇒ **`builtins` 映射** ⇒ `bytes` ⇒ `marshal` ⇒ `importlib` 能在
> VM 里跑 ⇒ `site.py` ⇒ **`P3-12`** ⇒ 引入 `Lib/`。**规格侧已齐**（`IM-11`…`IM-35`；§11／§12 无未写小节
> 与无关项）⇒ **缺的全是实现**。

> **M2 的门**：语料下限（`MS-13`）与规范化容差（`MS-9`）**已于第 270 轮裁定** ✓（见各自条目 ✓），**它们决定
> "M2 何时能声称通过"**（现在 9 条语料绿**不等于** M2）。两者**都建议等表达式面推进到一定程度
> 再定**——语料下限与"语言面能写什么"强相关，现在定会定出一个很快过期的数字。

> **编译器覆盖面的准确表述**（免得被引用错，也免得像我那样误撤一次）：`def` 的**完整签名**（含 `/`、
> `*args`、`**kw`、默认值）／**注解**／**类体两档**／**文档串**已进编译器，且与参照**逐字节一致**。
> **第 212 轮起，算术与位运算的表达式面也进了**：`+ - * / // % **`、`& | ^ << >>`、一元 `+ - ~`
> （优先级、`**` 右结合、常量折叠都逐字节对拍）；**第 213 轮再补括号（分组）、元组字面量、
> 下标读与下标写**⇒ `x = a[1] + a[2]`、`a[1] = 9`、`t = (4, 5)` 这些**都写得出来了**。
> **第 276 轮更新**：这一段原先列的"仍**发不出**"（`break`／`continue`、`try`／`except`、`with`、`import`、
> `lambda`、推导式、f-string、`a[0].b`）**已全部进齐** ✓（逐条复验见 §9.2 的 `P1-10` 行）；
> **当前已知仍发不出／未接线的**只有 `set`／`dict`／`bytearray` 的**就地**运算（`|=` 一族）——其余以 §9.2 各行为准。
>
> **`MS-17` 的现状（第 228 轮实测）**：夹具 **244** 条里 **行号可比 244 条（100%）**、**位置可比 221 条**。
> 行号缺口 **0**；列跨度缺口 **20** 条，逐条在 `tools/compile-positions-census.tsv`：
> 11 条是「参照给合成指令（类体 `MAKE_CELL` 等）的位置是 `None`，本层的位点表表达不了『缺失』」、
> 4 条布尔嵌套（`(a and b) or (c and d)` 一族）、2 条 `not not`、2 条链式目标（`a[0].b = v`）、
> 1 条嵌套注解子项跨度（`list[int]` 的 `list` 与 `int` 各取自己的跨度）。
>
> 列跨度属观测面（`MS-17` 允许"未覆盖但必须写明理由"）；第 226～228 轮共修 **十三条**传播规则，
> 其中**四条**（`LOAD_ATTR` 取对象、`RETURN_VALUE` 取值、赋值"复合右值取整段"、增强赋值存入取整条语句）
> 是**错位测量**留下来的错规则——详见各轮小结。度"，其余是逐族
> 列跨度传播细节（观测面，`MS-17` 允许未覆盖但必须写明理由）。
>
> ⚠️ 第 220／225 轮那批"粘性 loc 让 `if` 体沿用条件的行"的结论是**错位测量**的产物，已在第 225／226 轮更正。ONST slice(…)`）、两段非常量走
> `BINARY_SLICE`、三段走 `BUILD_SLICE 3`；`list` 的切片写一并接线。
>
> **位置表的一处已知边界**（第 212 轮）：单个运算符与"一元套简单操作数"的位置已与参照一致；
> **嵌套复合表达式**（如 `a * b + c`、`-a ** b`、`a[1][2]`）的后继加载与收尾跨度取**内层**那段，
> 属参照**内部传播细节**——**未推规则**，夹具里按惯例标 `positions_covered = false`（那条用例的
> **指令流与常量池仍逐字节比**）。
>
> **第 211 轮补注（两处容易看错的地方）**：
> ① **运行期不等于编译器**：`BINARY_OP` 一族（`+`／`-`／`*`／`//`／`%`／`**`／位运算／一元）**已经**
>    走任意精度公开入口（`P1-11`），所以"源码写不出 `a * b`"是**编译器**的缺口，不是语义缺口。
>    此前执行器里还残留 i64／单例假设（结果不在单例区间就报 `IntOutOfRange`）——已由新对拍语料
>    `big_int_add.py` 抓出并作废。
> ② **常量折叠差异**：常量池的整数只有 `Constant::Int(i64)` ⇒ `i64::MAX + 1` 这类字面量加法
>    **不折**（参照会折成大整数常量）——**语义等价、指令流不同**；若把这类源码放进
>    `gen_compile_fixture.py` 的用例集，会因指令流不同而红，故**不放进**，在此登记。
>
> **AB-62 已落地**（第 211 轮）：`pa_tointstring`（任意整数 → 十进制借用视图）＋
> `pa_pushintstring`（十进制 → 整数压栈）；`bytes` 按裁定用**既有** `pa_pushbytes`／`pa_tobytes`
> （从桩改为真实现，不新增函数）。对拍 harness 的整数渲染改走桥，语料扩到 **11 条**
> （新增 `big_int_add.py`／`bytes_concat.py`），**11/11 通过 · 0 已知差异 · 0 新差异**。
> **待裁**：`pa_type` 对 `bytes` 目前报 `PA_THANDLE`（`pa_tag` 里没有 bytes）——要不要加
> `PA_TBYTES`：**已决** —— `pa_type` 的标签取值域由 **`AB-63`** 定，`bytes` 取 **`PA_TBYTES = 8`**
> （**末尾追加**，`AB-44`）；并明确"判类型用 `pa_type`／`pa_is*`、取值用 `pa_to*`"。

> **`m1` 之后的 ABI 注意**：`m1` 是**里程碑标记**，**不是 ABI 冻结声明**（标签说明里写明了）
> ⇒ `P1-13` 改执行签名**不需要**走 `AB-44` 的版本矩阵。将来要"对外声明 ABI 可依赖"时，那才是
> `AB-44` 的起点；**建议在 `P1-13` 落地之后**立那个标志。**进 `main` ≠ ABI 冻结**。

**复核清单**（依据在右列）

| **P3-22** | **`dict` 子类实例不被当字典**（第 298 轮量出）：本层判"是不是字典"用的是**类型恰好相等**，不是 `is_subtype` ⇒ `dict` 子类的实例既不能下标赋值、也不能当类命名空间（最小复现：`class D(dict): pass; D()["x"] = 1` ⇒ "下标赋值只接线了 list／dict"）。**这是 `enum` 要的那一格**：`EnumDict` 是 `dict` 子类且**重写了 `__setitem__`**（成员名就那样收的）⇒ 修法不能只放宽判据，还要走槽位／重写 | `OM-11`、`AB-37`、`AB-58` | 第 299 轮查清：根因是本层的「宿主布局继承」只对 **host 类型**生效，**VM 内部**内建类型（`dict`／`list`／`frozenset`）不在那条路上 ⇒ 子类拿到的是普通对象布局；修法＝内建类型的**布局继承** ＋ 内建方法**进类型字典**（放宽类型判据只会引入 UB）|
| **P3-23** | **t-string**（PEP 750，3.14 新语法）：`t"…"` 现在被拆成名字 `t` ＋ 字符串 ⇒ `Lib/annotationlib.py:327` 的 `type(t"")` 报「实参表里出现 `Some(Str("")`」；要连**新内建类型** `Template`／`Interpolation` 一起做（不是小缺口）。挡着 `annotationlib` ⇒ `test.support` 那一族 26 个模块 | `BC-14`、`TS-*` | 无 |
| ~~**P3-24**~~ **已解除（第 302 轮）** | **方法的默认值元组装的是类名** —— 根因是 `compile_class_scope` 收尾只调 `flush_jumps`，漏了模块／函数那四样冲刷 ⇒ `deferred` 的默认值元组既没入常量池、占位字节也没回填（`Q.__init__.__defaults__` 成了 `('Q',)`）。已修 ＋ 语料 `method_defaults.py` | `BC-4`、`OM-14` | 无 |
| **P3-25**（一半已解除，第 303 轮） | **在类型对象上取属性不做描述符绑定** —— **已修**：`staticmethod`／`classmethod` 的取用（`Q.s` 不再拿到包装对象本身；`classmethod` 绑到那个类；第 298 轮 `__prepare__` 的绕行已撤）＋ **容器的 `__contains__` 属性**（通用实现，与 `in` 同一处；`bytes` 也换过来）。**还剩**：其余**内建方法**作为属性仍未普遍接线（`[].append` 一类走的是逐类型方法表；`list.__len__` 等仍报 `AttributeError`）| `OM-11`、`AB-37` | 逐张方法表补（`keyword` 已验证可同步 ✓）|
| ~~**P3-21**~~ **已解除（第 297 轮）** | **`iter_value` 拿到已释放对象 ⇒ `SIGSEGV`** —— 根因是 `dict.setdefault` 插入那条路把**借来的实参**当返回值交出 ⇒ 字典里那一份被放掉 ⇒ use-after-free（实测链条：`Lib/enum.py` 的 `classdict.setdefault('_ignore_', []).append(...)`）。已修（返回值那一份自己 retain）＋ 语料 `dict_setdefault_reference.py` ＋ `PYAWA_QUARANTINE=1` 闸门守住 | `OM-11`、`OM-16` | 无 |
| 不要做 | 依据 |
|---|---|
| 手写内建类型表 | `TS-41`（必须探测导出） |
| 手写绑定错误消息 | `BC-56`（消息以探测为准） |
| 把容器载荷布局写进文档 | `TS-43`（实现自选） |
| 在文档里复述逐指令动作 | `SPEC-bytecode.md` §10 的注 |
| 为内建类型旁路属性通道 | 同上（必须走 `getattr` 槽，`OM-11`） |
| 用"自己跟自己对"的夹具 | 夹具必须是参照实现的真产物——否则 `MS-12` 的自检无意义 |
| 只对拍 `BC-57`／`BC-58` 名单里的指令 | `BC-59`：编码对拍**必须完备**（语料里出现的每条指令都对 `dis`） |
| 把临时内部错误类型／消息当成**可登记的差异** | 异常类型与消息是**可观察语义**（`MS-8`／`MS-10`／`MS-17`）⇒ 接线后**必须**换成真异常，**不得**进差异清单 |
| 因为"本层没实现"而在解码时对 `NOT_TAKEN` 一类返回 `Err` | `SPEC-bytecode.md` §10 的三分类②：**参照实现会发射** ⇒ VM **必须容受**（我们的编译器可以不发） |
| 为"表里有、编译器不发"的指令做 parity | 同上③（`BINARY_SLICE`／`STORE_SLICE`／`RESERVED`／编号 ≥ 256 的伪指令） |
| 给**无 oparg** 的指令编出含义 | `BC-58`：`_opcode.has_arg` 是唯一权威（`STORE_SUBSCR`／`MAKE_FUNCTION`／`NOT_TAKEN`… 的 oparg 必须为 0，`BC-33`） |
| 把异常状态放成**进程级全局** | `BC-60`：必须按**实例**存放（`OM-1`／`CX-3`）；将来按实例 × 线程 |
| 让**宿主自己分配**载荷、只把指针交进来 | `AB-58`：载荷由 **VM 分配**（否则 `OM-3` 记不了账、`AB-18` 管不到） |
| 为输出新增能力域或注入 output sink | `CM-26`：`print` 归 `fs` 域（`sys.stdout` → `_io`），sink 会绕过实例能力表 |
| 只看某一轮的测试总数 | `MS-25`：连跑 ≥3 次总数必须一致；抖动按崩溃／内存损坏排查 |
| 留下未解释的**编号缺号** | `CX-19`：同前缀编号必须连续（墓碑算定义） |
| 让**夹具探针有副作用**（先跑会改被测物的赋值，再取 `value`） | 探针**必须先取 `value`、再跑任何有副作用的探测**；夹具生成后**逐条读**，不能只看条数——否则期望值失真，且 `MS-12` 的自检也照不出来 |
| 让 stdlib 的**安全入口**把容器布局漏成公共 API | `OM-6`：载荷表示**不进 ABI** ⇒ `pyawa-stdlib` 为 `forbid(unsafe_code)` 新增的安全入口**不得**暴露布局细节 |
| 把 `m1` 当成"ABI 已冻结／已发布" | 标签说明 ＋ `README.md`：**里程碑标记 ≠ ABI 发布标志**；`AB-44` 的起点另立（建议在 `P1-13` 之后） |
| 让 harness 首版去比 stdout/stderr | `print` 未落地（`CM-26`：`sys.stdout` → `_io` → `fs` 域）⇒ 必须**如实标成减配版** |
| 在 harness 里"缺前置就跳过" | 与 `T-AB-1` 同一原则：**缺即红、不降级** |
| 改 `Lib/` | `CX-8`（例外清单为空） |

**第 215 轮复核记录**（本节复核清单的落地情况；方法写在括号里，便于下轮复跑）

| 复核项 | 做法 | 结果 |
|---|---|---|
| 夹具探针不得有副作用 | 扫 `tools/gen_*.py` 全部 22 个生成器：先按"就地改"特征（`__setitem__`／`del`／`sort`／`append`／`update`／`= ` 等）机扫，再**逐个读**主体可变的那几个（切片／集合／迭代器／模式匹配）的探针顺序 | **一处违规**：`gen_slice_fixture.py` 的 `__setitem__` 探针排在取 `value` 之前（第 213 轮已修，源码里留了原因注释）；其余生成器的主体**每个用例都重建**（如 `gen_match_fixture.py` 的 `build(description)` 在循环内），迭代器类（`gen_itertools_fixture.py`）每条探针各建一个 ⇒ 无跨探针状态 |
| stdlib 安全入口不得漏布局（`OM-6`） | ① 列 `Instance` 的全部 `pub fn` 签名，找除 `NonNull<Header>` 之外还带布局类型的；② 扫 `crates/pyawa-stdlib/src/` 是否出现布局类型名、`refcount` 字段、`.items.borrow`；③ 查有无 `-> &mut` 别名返回与容量暴露 | **合规**：安全入口只交**不透明句柄**（`NonNull<Header>`）与拷贝（`Vec<NonNull<Header>>`）、标量；stdlib 只直接构造 core 公开的 `BuiltinFunctionObject`／`AttributeObject`（原生函数与属性机制，**不是**容器布局），不碰任何容器载荷字段；无 `&mut` 别名返回、无容量 API |

> 附一条**不是违规**但值得记住的事实：`pyawa-abi`（C 层 crate）**内部**直接用 `DictObject`／`ListObject`／`StrObject`／`IntObject`／`FloatObject` 等核心布局类型——`OM-6` 禁的是"头部出现在 **C ABI 签名**里"，而 `pa.h` 只有不透明句柄与标量，故合规；但若将来要求 C 层也布局无关，需再补核心访问器。

**每族的两件事**：① 夹具对拍参照实现的**真产物**（真字节 ＋ 真 `dis` 输出）；
② 发射器产出**必须**过 `validate`（`BC-32`…`BC-36`）。
**① 的完备性**：语料里出现的**每一条**指令都要与 `dis` 对拍，**不是**只测 `BC-57`／`BC-58`
名单里的那几条（`BC-59`／`T-BC-21`）——`dis` 是完备 oracle，靠它兜住"文档还没追到的编码"。

**撞上"没规格"时**：停下来问，并带上 ① 要决定什么；② **实测到的参照行为**；
③ 倾向哪种口径（含代价）。

**提交**：纪律见 `agents-rules/commit-rule.md`。本轮相关的两条——
**生成脚本与夹具一起入库**（否则数据不可复现）；**新增约束必须领新号**
（下一个空号 ＝ `CONSTRAINTS.md` §3 表尾编号 ＋ 1——**这里不写具体数字**，写了就会过期）。

---

#### 暂时违约清单（**非规范**；逐条带「回填条件」，防日后淡忘）

> 本节是**工作台账**，不是规范：规格本身（`IM-`／`CM-`／`CX-`）未改一字。任何一条回填后，请就地划掉并注明轮次。

**A. 对硬约束的暂时违约**（必须回填；违反的编号逐条写明）

| # | 违约（vs 谁） | 现在的做法 | 为什么暂时这样 | 回填条件 |
|---|---|---|---|---|
| A1 | `IM-30`／`IM-31` ＋ 本文件「不能用 Rust 私写顶替」 | `pyawa-core` 的 `load_module` 在 **Rust** 里做目录扫描＋编译＋执行＋登记 | 它是**过渡桥**：让 `import` 今天能跑，并**守住** `IM-15`（I/O 全经 `fs` 域） | `Lib/` 引入（M3）后，Python 层 finder（**继承** `_bootstrap_external.FileFinder`）顶替 |
| A2 | `IM-24`／`IM-26`（`sys.path` 归 `site.py`，且不得跳过它） | `sys.path` 由**组合根**播种：CLI 放脚本目录、语料 harness 放语料目录 | `site.py` 还跑不起来 | `site.py` 能在 VM 里跑 |
| A3 | `IM-31` 的"loader 的编译与 `.pyac` 读写落 Rust 且经能力层" | 编译在 Rust ✓ 经能力层 ✓，但**没有产物容器**：每次 import 现编现跑、不写缓存 | `P3-12` 的 `.pyac` 未落地；`IM-16`（只读根 ⇒ 不写缓存、不报错）**现在恰好满足** ✓ | `.pyac` 容器与陈旧判定（`IM-18`…`IM-21`）落地 |

## 10. 尚未写出（本规格自己缺的节）

`SPEC-INDEX.md` §5 第 6 条要求 `v0` 规格显式列出缺口。本规格缺：

| 缺的节 | 内容 | 为什么现在没有 |
|---|---|---|
| **对拍 harness 的实现** | `MS-6`…`MS-14` 的可执行实现 | **已落地**：runner ＝ `crates/pyawa-abi/tests/conformance.rs`，语料 ＝ `tests/conformance/corpus/`，报告 ＝ `target/conformance/report.md`（`MS-14`）——口径与边界逐条见 `tests/conformance/README.md`。<br>~~**未落地**：`MS-8` 的 **stdout／stderr** 比对（等 `print`，`CM-26`）~~ **已完成**（第 93–95 轮）．**仍欠**：`MS-13` ②（`Lib/` 语料，等 M3）；`MS-9` 已按 `§13-10` 的裁定归一（行尾／末尾换行 ＋ `0x` 地址 ＋ 路径前缀） |
| **基线语料清单** | `MS-13` ①（自建最小语义用例）与②（`Lib/` 中可编译部分）的具体清单、及其**模式标注** | ① **已有首版**（纯 Python 模式；清单与边界见 `tests/conformance/README.md`）；**但 `MS-13` 的"范围下限"已裁定**（`§13-10`）：总数 ≥ 112 ＋ 各内容面下限，判据 `tools/check_corpus_floor.py` ✓ —— **M2 已按它成立** ✓。<br>② **已裁定达成** ✓（2026-10-18：语料 **407** 条＝自建 192＋`Lib/` 165＋`Lib/test/` 50，覆盖 **150** 个可 import 模块；机检仪器 `tools/corpus_coverage.py` 五项全 PASS 并已接进 `tests/ci/check.py` 成 **M3-COVERAGE 硬门**）；扩展模式语料**没有参照**，用**自身对照**验收（见各特性条款） |
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
① 407 语料（`.py`）跑 `check` **0 诊断** ✓；
② 一个故意写错的 `.pyawa` 能**一次报出 ≥2 条**类型诊断 ✓（证批量）；
③ `--format=json` 含 `file`／`line`／`col`／`code`／`severity`／`message` ✓；
④ CI `fast` 档在 `check.py` 之后加一步对语料跑 `pyawa check` ✓；
⑤ 同一文件「`pyawa` 执行」与「`pyawa check`」的**语法**诊断一致 ✓。

**禁区** ✗：不对 `.py` 做类型诊断（破坏 `MS-19`）；不执行任何代码；不做 autofix（留 ③）；不实现 LSP 协议（那是 ②）。

**依赖** ✓：`M4` 检查器（`TS-*`）＋ `compile/` 解析器（0 unsafe）；**不引** LSP 框架 ✓（那是 `pyawa-lsp` 的事）。

**状态** ✓：① `pyawa check` **已落地 ✓（round 275）**——`pyawa check [--format=json] <文件…>`：只编译不执行、批量诊断（不遇首错即止）、`.py` 只语法类（`MS-19`）／`.pyawa` 语法＋类型、`--format=json`（`file`/`line`/`col`/`code`/`severity`/`message`）、退出码 0／1／2；验收 `crates/pyawa-runtime/tests/check_command.rs` 6 例（含 **407 语料 0 诊断**、JSON 字段、执行 vs check 语法诊断一致、退出码）；CI fast 在 `check.py` 后已加语料步骤（PLAN §10.1 ④）。② LSP MVP **已落地 ✓（round 276）**——`crates/pyawa-lsp`（**零第三方依赖**的手写 JSON-RPC／LSP 帧）：`initialize`／`initialized`／全量 `didOpen·didChange·didClose`／`publishDiagnostics`（与 `check_source` 同一处真相）／`hover`（已标注名字的声明类型）；集成验收 `crates/pyawa-lsp/tests/lsp_stdio.rs` 3 例（真起子进程走 Content-Length 帧）；CI fast 已加 `-p pyawa-lsp` 测试。③ **按需梯的最小项 已落地 ✓（round 277）**——`textDocument/completion`（文档内符号）／`textDocument/definition`（注解/def/class/顶层赋值首个定义处）／`textDocument/rename`（**单文件整词** `WorkspaceEdit`）；验收 `crates/pyawa-lsp/tests/lsp_stdio.rs` 第 4 例。**如实边界**：不做类型推断／成员补全／标准库符号／跨文件跳转／作用域分析（PLAN 原文即"按需"，这些留待需求出现 ✗）。**位置口径收口 ✓（round 279）**：LSP `character` 已严格按 **UTF-16 code unit** 换算（入参/出参、非 BMP 字符；验收 `lsp_stdio.rs` 第 5 例）；其余已知边界在 `crates/pyawa-lsp/README.md` 的「范围外」逐条写明理由 ✗。

## 12. 需求单：语言与库缺口（2026-11-13）

三项均由文档侧实测定位（非推测），证据与验收如下。语义仍以各 `SPEC-*` 为**一处真相**，此处只给范围与判据。

**状态 ✓（2026-11-14）**：三项**全部落地**（逐项状态见各小节末尾状态注；`REQUIREMENTS.md` 的「信号面」「工作边界」等决策已按本轮实测收口）。

### 12.1 `x = yield`（`SEND` 取值形态）—— 语言语义缺口

**证据**：
- `SPEC-bytecode.md:533` 把 `RETURN_GENERATOR`／`YIELD_VALUE`／`SEND`／`GET_AWAITABLE`／`GET_YIELD_FROM_ITER` 列入「生成器与协程」族。
- 实测（release）：
  - `def g(): yield 1` ⇒ 正常；`def g():\n while True: yield` 的 `send()` ⇒ 正常。
  - **`def g():\n v = yield\n print(v)`** ⇒ `未捕获（状态 5）：指令 52 的这个形态尚未接线：非生成器函数不该让出（码元被改坏了？）`。
- 即：**yield 不接值时可用，接值（`x = yield`）即崩**。

**范围**：让 `x = yield`（`SEND` 后取值）按 `YIELD_VALUE`／`SEND` 的规格工作；`send(v)` 的 `v` 必须成为该表达式的值；`throw`／`close` 的既有行为不得回退。
**验收**：
① `v = yield` ＋ `gi.send(7)` ⇒ 生成器内 `v == 7`；
② `try/except GeneratorExit` 包住的 `x = yield` 形态在 `close()` 时不泄漏、不崩；
③ `yield from`／`await` 现有用例（含 `pyawa-lsp` 之外的核心测试）全绿；
④ 语料新增一条该形态（`tests/conformance/corpus/`），对拍参照一致；
⑤ `check.py` 全绿 ＋ `slowcheck` 十一项全绿。
**禁区**：不改 `YIELD_VALUE` 的既有语义；不为"看起来能跑"在解释器层特判（须按码元路径修）。

**状态 ✓（2026-11-14 落地）**：修因在**编译器符号分析**（按码元路径，无解释器特判）：`statement_has_yield` 覆盖全部语句变体（赋值族／`assert`／`for` 可迭代／`while` 条件／`with` 项与目标／`match` 主语与守卫／`raise`／`delete`／`def` 装饰器与形参默认值／`class` 基类·关键字·装饰器），`expression_has_yield` 全面下探同作用域表达式（`lambda` 形参默认值、推导式**最外层可迭代**等作用域边界逐条实测；`lambda`／`def`／`class` 的**体**不下探）。独立复核：72 例判定矩阵 66/72 一致（6 例外全部是"参照 SyntaxError、本层接受"的**既有** parser 缺口，非本笔引入）；参照判为生成器的 55 例**全部**一致；`compile` 夹具逐指令与 conformance **408/408** 证明既有产物未变。验收：①`send(7)` ⇒ `v == 7` ✓ ②`GeneratorExit`＋`close()` 不崩、`PYAWA_DANGLING=1` 干净 ✓ ③`compile`／`generators`／`coroutines`／`generator_and_range_shapes` 全绿 ✓ ④语料 `yield_expression_assignment.py`（manifest **408** 条）对拍一致 ✓ ⑤`check.py`＋`slowcheck` 全绿 ✓（收口轮）。**顺带收口**：同族两处漏标（`except (yield):` 的类型表达式；`yield (yield from …)` 的操作数）已一并修并加回归测试（`tests/yield_expression_shapes.rs` 6 例）。

### 12.2 `_signal` 最小面 —— 一次解锁 `asyncio`

**证据**：
- 实测：`import signal` 成功，常量与 `Handlers` 可见；但 `signal.getsignal(2)` ⇒ `NotImplementedError: _signal.getsignal：真实信号注册／查询是能力域（CM-8）问题，本轮未接线`。
- 实测：`asyncio.run(m())` ⇒ **同一条 `_signal.getsignal` 报错**（即 `asyncio` 在事件循环初始化阶段就依赖它）。

**范围**：在**不启用真实信号**的前提下，提供 `_signal` 的最小面，使 `asyncio` 的事件循环可初始化并跑完一个协程：
`getsignal`／`signal`（**注册为 no-op 或仅记账**，不真正投递）／`set_wakeup_fd`（返回前值或 `-1`）／`SIGCHLD` 等常量的存在性。
**已定（2026-11-14，用户选 A）**：**不做真实信号投递**，只做**记账式（只记录、不投递）**。
- 理由：POSIX 信号处置是**进程级**状态，与 `CP-13`／`OM-1` 的每实例隔离直接冲突；放开真实信号等于把沙箱边界
  开在宿主进程上（脚本可改写宿主退出路径）。而 asyncio 真正吃信号的场景（子进程 `SIGCHLD`、Ctrl-C 处理）
  在本层本就非目标（`subprocess`／`fork` 不提供）。
- **代价（必须如实记录）**：`signal.signal` 注册的 handler **永不触发** ⇒ 实现时**必须**在
  `tests/conformance/divergences.md` 增一条 `DIV-`（注册不投递），并在 `_signal` 合约里写明"记账式"边界；
  无法记账的形态一律如实报未实现（`CM-6`／`AB-22`），**禁止假成功**。
- **留待**：真实信号属 `proc` 域（`SPEC-capabilities.md` §9.3），等该域真正开工时另行设计。
**验收**：
① `asyncio.run(coro())` 返回正确结果；② `await asyncio.sleep(0)` ＋ `async for` ＋ 异步推导式可用；
③ `signal.getsignal` 不再抛 `NotImplementedError`，返回记账号；
④ 若走方案 B：未授予该能力域时**如实报未实现**（`CM-6`），不得假装成功；
⑤ 不引入宿主不可控的信号处理；`check.py` ＋ `slowcheck` 全绿。
**禁区**：不得在 VM 内部直接调用宿主信号设施绕过能力面（`CP-` 口径）。

**状态 ✓（2026-11-14 落地）**：**方案 A 记账式**已落地——每实例记账表存在 `_signal` 模块命名空间的隐藏键（`__pyawa_signal_handlers__`／`__pyawa_signal_wakeup_fd__`；无全局可变状态、不调用宿主信号设施）。`getsignal`／`signal` 记账并返回前值（`SIGINT` 初值即 `default_int_handler`、32／33 → `None`、其余 `SIG_DFL`）；`default_int_handler` 导出且调用抛 `KeyboardInterrupt`；`set_wakeup_fd` 只接受 `-1`（其余如实 `NotImplementedError`）。独立复核：asyncio 面（`run` 返回值／`await sleep(0)`／`async for`／异步推导式／SIGINT 记账→恢复）与 signal 面（45 常量／初值／读回／前值／参数校验文案／不可捕获号）与参照逐行一致；`PYAWA_DANGLING=1` 下 7 个脚本零崩溃、零 stderr。验收①-⑤全过。差异与可观察偏差登记为 `DIV-10`，契约改写见 `SPEC-c-modules.md` §5.2.11。**范围外发现（非本项引入，另记）**：`asyncio.gather`／`wait_for` 红在 `pyawa-core` 缺**类体私有名改写**（`__x` → `_C__x`）；`OSError` 的 `.args`／`.errno` 与 `KeyboardInterrupt.args` 保真度属 core 既有口径（`str` 一致）。

### 12.3 `_csv` ＋ `Lib/csv.py` —— 标准库 C 底座缺口

**证据**：
- 实测：`import csv` ⇒ `ModuleNotFoundError: No module named 'csv'`。
- `Lib/csv.py` **不在仓库**（`Lib/` 同步面尚未覆盖它）；`crates/pyawa-stdlib/src` 内**无 `_csv`**。
- 排期口径见 `P3-14`（逐模块合约与 stdlib 实现）与 `P3-26`（判据① 的驱动量＝C 模块缺口）。

**范围**：用 Rust 实现 `_csv` 的最小面（`reader`／`writer`／`field_size_limit`／`Error` 及 `Dialect` 的必要方法），并同步 `Lib/csv.py`，使 `csv` 可 `import` 并完成读写回环。
**验收**：
① `csv.reader(io.StringIO(...))`／`csv.writer(...)` 对拍参照一致（含引号、转义、换行处理）；
② `csv.DictReader`／`DictWriter` 可用；
③ 上游 `Lib/test/test_csv.py` 的通过项数较接入前**上升**（记录前后数字）；
④ `CM-14` 的逐模块合约补段（`SPEC-c-modules.md` §5.x），含"未接面如实"；
⑤ `check.py` ＋ `slowcheck` 全绿。
**禁区**：不引入 C 扩展加载（C 底座用 Rust 重写）；不合规方言静默降级（要么支持、要么如实报未实现）。

**状态 ✓（2026-11-14 落地）**：`_csv` 以 Rust 逐条转写参照 `Modules/_csv.c` 状态机实现（`Error`／`Dialect`／`Reader`／`Writer`／`reader`／`writer`／`field_size_limit`／方言注册四函数／6 个 `QUOTE_*`／`_dialects`；读侧 `parse_process_char`、写侧 `join_append_data`、方言校验次序与文案逐字一致）；`Lib/csv.py` 已由 `tools/sync_lib.py --sync` **逐字节同步**（`SLICE` 只增 `csv.py`；`--check` 零差异）。验收：①`csv.reader`／`csv.writer` 全档对拍一致（120 行双侧逐行）✓ ②`DictReader`／`DictWriter` 可用（上游 `-k Dict` 25 例全过）✓ ③上游 `test_csv.py`：接入前 `ModuleNotFoundError`（**0 通过**）⇒ 接入后 **127 例：117 通过／0 失败／4 错误／6 跳过** ✓ ④`SPEC-c-modules.md` §5.2.33（工单写 §5.2.30，已被占号 ⇒ 取下一空号；含未接面如实）✓ ⑤`check.py`＋`slowcheck` 全绿 ✓。**上游 test_csv 剩余 4 条如实归因（均在 `_csv` 语义之外）**：4 错误＝`array.array` 元素存储未接线（§5.2.14 既有缺口）；先前的 1 失败（`__module__`）与 2 错误（`_io` **孤立 `\r`** 断行）已于 2026-11-14 的两个缺口工单修掉（见 §12.5）。

### 12.4 ABI 收口（2026-11-14 审计，8 项）

审计口径：规格定义 `AB-*` **68** 条（编号 1–69 无缺号）；`pa.h` 声明函数 **73**；cdylib 实际导出 **74**。
以下 8 项排期全部收掉（#3／#6／#8 已于 2026-11-14 由用户拍板；见本节末状态注）。

| # | 项 | 收法 | 验收 |
|---|---|---|---|
| 1 | `AB-52` 的 `type_expr` **不求值**（只原样保留表达式字符串） | **已落地（M4，2026-11-14）**：落 `TS` 注解解析器（`SPEC-type-system.md` §16 的 `TS-51`…`TS-57`；`pyawa_core::parse_type_expression`／`evaluate_type_expression`）；`pa_register`／`pa_module_addfunction` 注册时求值成 `AB-69` 的 `TyExpr`（**同一账本**：边界运行期＋编译期调用点检查；`Any`/`Platform` 变体；坏字符串/未知名 ⇒ `PA_ERR_INVALID`；`.pyi` 仍原串） | 注解表达式可求值 ✓（`crates/pyawa-abi/tests/annotation_strings.rs` 7 passed）；`.pyi` 导出正确 ✓ |
| 2 | `pa_exec_file` **未提供**（文件 I/O 经能力层 `IM-15`） | 接 `IM-15` 后提供；未接时如实 `PA_ERR_NOTIMPLEMENTED`（现状合规） | 提供后文件执行可用；未提供时与"已实现但拒绝"可区分（`AB-22`） |
| 3 | `pa_call` 的 `nresults != 1` **未定** | **已定**：**精确**结果数（`0`/`N`，多截、少补 `None`）⇒ 落 `AB-72` | 多返回值用例；单返回值不回退 |
| 4 | **宿主对象句柄不可读回**（复核：`pa_type` **本来就在**如实报 `PA_THANDLE`——审计此句为误记；缺的只有 `pa_tohandle`） | **已实现**：新增 `AB-70` `int pa_tohandle(pa_state*, int idx, void **payload_out)`——栈契约 `—`，仅对宿主对象成功，类型不符 `PA_ERR_INVALID`（`AB-22`）；借用照 `AB-15`／`AB-17` | 脚本把句柄当参数传给宿主函数能取回载荷；经 `list`／`dict` 往返后仍能取回；类型不符如实报错；规格 §15.3 表格同步 |
| 5 | `§15.4` 辅助层（`paL_*`）**规格无签名** | 按现状（`AB-19` 形态）把签名补进规格 | 规格与 `pa.h` 一致 |
| 6 | `getfield`／`setfield`／`gettable`／`settable` 栈契约：规格 `±1` vs 实现 `−1`／`−2` | **已决**：改规格记法跟随实现（`setfield` −1；`settable`／`rawset` −2） | 规格与实现一致；`T-AB` 用例覆盖 |
| 7 | `pa.h:140` 注释**过时**（`pa_exec_bytecode` 已落地，真的 decode 并按 `BC-29` 校验） | 改注释 | 注释与实现一致 |
| 8 | `pa_set_program_filename` **影子导出**（不在 `pa.h`、不在规格） | **已选**：补进 `pa.h` ＋ 规格（`AB-71`）；`T-CX-19` 过渡清单清空 | 导出 ⊆ 头文件 ⊆ 规格 |

**防 #8 复发**：**已落地 ✓（2026-11-14）**——机检 **`T-CX-19`**（约束 **`CX-29`**，定义见 `CONSTRAINTS.md` §3）：「`#[no_mangle] extern "C"` 导出符号集合 ⊆ `pa.h` 声明集合 ⊆ `SPEC-c-abi` 提及集合」，且导出与声明的总数都 **≤ `AB-1` 的 120**。只比对符号名（源码级扫描，不构建 cdylib），纯客观、不涉及判断。**过渡**：#8 的待裁项 `pa_set_program_filename` 显式列在检查的过渡清单里（棘轮：新增影子导出立即红；#8 修好后不删清单同样红）。

**状态 ✓（2026-11-14，ABI 收口完成）**：
- **#3** 已拍板（形状 A：`nresults` **精确**结果数，`0`/`N`，多截、少补 `None`）并落地为 **`AB-72`**；
- **#4** **已实现**：新 **`AB-70`** `int pa_tohandle(pa_state*, int idx, void **payload_out)`（栈契约 `—`、仅宿主对象成功、类型不符 ⇒ `PA_ERR_INVALID`、借用照 `AB-15`／`AB-17`）。**复核更正**：`pa_type` **本来就在**如实报 `PA_THANDLE`（`tag_of` 兜底 ＋ `host_module.rs` 已钉住；审计表原句"不报"是误记）——缺的只有读回；
- **#5** 已落地：`SPEC-c-abi` §15.4 的 18 个 `paL_*` 已补签名列（与 `pa.h` 一致）；
- **#6** 已拍板（改规格记法跟随实现）并落地：`setfield` **−1**、`settable`／`rawset` **−2**（`getfield`／`gettable`／`rawget` 维持 ±1）；
- **#7** 已落地：`pa.h` 顶部总述与执行入口注释都改为"`pa_exec_string`／`pa_exec_bytecode` 已落地；只有 `pa_exec_file` 如实报未提供（`IM-15`）"；
- **#8** 已拍板（补进头／规格）并落地：`pa_set_program_filename` 进 `pa.h` ＋ **`AB-71`**；`T-CX-19` 的过渡清单已清空（棘轮仍在：将来新增影子导出立即红）；
- **#1** **已落地（M4，2026-11-14）**：`AB-52` 的 `type_expr` 求值接通（见上表 row 1 与 `SPEC-type-system.md` §16、`PLAN` §12.9）；**#2** 维持合规现状（等 `IM-15`），边界已如实写在各处；
- **ABI 版本**：次版本 **9→10**（`AB-70`／`AB-71` 为**追加式**；`AB-72` 是**未发布前**的语义补齐，旧宿主不存在）。**ABI 仍不发布**（用户 2026-11-13 口径：等钩子族定稿）。


### 12.5 解释器与模块缺口收口（2026-11-14）

§12.4 收口时记下的"既有缺口"五项已逐项修完（各有回归测试或可复跑探针；**ABI 面不动**）：

| # | 缺口 | 状态 | 证据 |
|---|---|---|---|
| 1 | `asyncio.gather` 红（`pyawa-core` 缺**类体私有名改写**） | ✓ 已修 | `BC-66`；`compile/mangle.rs`（AST 一次性改写）；`private_name_mangling.rs` 矩阵与 3.14.4 逐字节；`asyncio_smoke` gather 用例（`[2,4,6]`） |
| 2 | 原生函数 `__module__` 恒 `"builtins"` | ✓ 已修 | 每实例旁表＋`Instance::register_namespace_native_modules`（由 `pyawa-stdlib::install` 集中登记）；`_csv.*` ⇒ `"_csv"`；上游 `test_csv` **0 失败** |
| 3 | `_io` 的 `newline=''` 不在孤立 `\r` 断行 | ✓ 已修（**读**面） | `NewlineMode` 五档；`io_newline_shapes.rs` 6 探针与 3.14.4 逐字节；上游两 roundtrip **转 PASS** |
| 4 | `inspect.getattr_static` 对类查找恒 `MISSING` | ✓ 已修 | `getset_descriptor`／`member_descriptor` **真描述符实例**（`types` 别名链成立）＋ `_shadowed_dict` 三条件；`inspect_static_shapes.rs`；`inspect.signature` 不回退/改善 |
| 5 | `StopIteration` 回收崩溃（`帧操作失败：StackUnderflow`） | ✓ 已修 | 处理块异常表区间止于**出口清理的 `POP_EXCEPT`**（`Emitter::handler_exit_cleanups`）；`exceptions.rs` 回归例；408 条对拍回绿 |

**边界（如实，均记 `NEXT.md`）**：① 嵌套类/函数内类的 `__qualname__` 运行期仍用 `__name__` 拼
（**既有**偏差）；② `nonlocal` 无外层绑定的编译期 `SyntaxError` 仍报运行期 `UnboundLocalError`
（既有）；③ `__static_attributes__` 里**显式预改写名** `self._S__p` 会被还原成 `'__p'`（歧义，极罕见）；
④ `__slots__` 字符串槽名不改写（既有）；⑤ `newline=` 的**写**翻译、`.newlines`、`StringIO(newline=…)`
仍未接；⑥ 文本 `tell`/`seek` 仍原文字符下标（非参照 cookie）；⑦ 类型对象自身的 `__name__` 等不在
`type.__dict__` ⇒ `inspect.getattr_static(用户类, "__name__")` 仍 `AttributeError`（类型属性面）。

**本轮新发现（先于本轮存在，未修 — 单开工单）**：`crates/pyawa-stdlib/src/_sre_module.rs` 的
**进程级** `static Mutex` 表（`PATTERNS`／`MATCHES`／`CLASSES`／`TEMPLATES`／`REGISTRY`）会跨 `Instance`
串对象对象地址 ⇒ 同进程**第二个**实例再 `import re`／`json`（首次编译正则）会拿到第一个实例的类型
⇒ `call_callable` 无限递归/栈溢出（`posix_module.rs:529-541` 有 round 727 修过的**同款**先例：改为
在本实例注册表里查）。CLI/survey 单实例不受影响；多实例的 cargo 测试会被误伤（诊断记在
`target/probe/`，`inspect_static_shapes.rs` 因此只放一个 `#[test]`）。


### 12.6 优先队列收口（2026-11-14，第二轮）

「建议优先」三项已全部落地（逐项证据见各笔提交；**ABI 面未动**）：

| # | 项 | 状态 | 证据 |
|---|---|---|---|
| ① | `_sre` 进程级静态表跨实例串对象（同进程第二实例 `import re`/`json` 栈溢出） | ✓ 已修 | 五张表改**每实例**（本实例类型查找＋实例字典隐藏纯值键）；仅留纯值有界 `ENGINE_CACHE`；`sre_multinstance`（修前 stack overflow ⇒ 1 passed） |
| ② | 性能回退复核 | ✓ 已修（A/B **转快**） | `attribute_lookup` 的 `__text_signature__`／`__module__` 两条投影下沉到类型 `getattr` 槽（新增 `host_function_getattr`）；A/B `-3.9%～-10.7%`；`bench_gate` 四档 ✓（`-1.9%/-5.0%/-2.5%/+0.4%`） |
| ③a | `nonlocal` 无外层绑定 → 编译期 `SyntaxError` | ✓ | 两趟 symtable 口径（`compile/symbols.rs`）；61 case 探针 **55 行逐字节**；`nonlocal_diagnostics`（23 文案＋14 合法） |
| ③b | `__slots__` 字符串槽名私有名改写 | ✓ | 类创建时按 `BC-66` 改写＋`member_descriptor`；slots 三形态对拍 diff 空 |
| ③c | 嵌套类/函数内类 `__qualname__` | ✓ | `N2.D`／`outer.<locals>.K`／方法内类/装饰器全矩阵对拍 diff 空；`BC-67` |
| ③d | `inspect.getattr_static(用户类, "__name__")` | ✓ | `type.__dict__` 的 `__name__/__qualname__/__module__/__bases__` 为真 getset；`task13_class_surface_shapes` |
| ③e | `newline=` **写**翻译／`.newlines`／`StringIO(newline=)` | ✓ | 9 探针逐字节 `SAME`；`io_newline_write_shapes`（3 passed）；`test_with`/`test_csv` 无回退 |
| ③f | `iter_value`／Err 分支引用泄漏（round 171 撤回项） | ✓ | GC 收支：`tb` 净漏 20002⇒2、`live_after` 207295⇒7348；`ref_leak_err_branches`（5 passed）；`test_dict` 10/10 |
| ③g | traceback 帧保活（实为 `with` 的 `__exit__` 缺 `tb`） | ✓ | 帧保活已成立并固化成对拍；`test_with` **12F/7E⇒6F/7E**；`exception_traceback_frames` |

**边界（如实）**：① `_sre` 的 `compile_raw`/`match_raw` 由 `id:int` 改 `key:str`（自有扩展，仅
`_sre_shapes.rs` 用）；`vars(pattern/match)` 多隐藏键（同 `_signal` 口径）。② 描述符暂无
`__set__`/`__delete__`（`inspect.isdatadescriptor` False；需写域外的类型/函数存储字段）；
`C.__dict__` 含私有键 `__pyawa_qualname__`（公开面与参照一致）；`__slots__` 的 `__dict__`/`__weakref__`
槽类型仍是 member_descriptor。③ `nonlocal` 的 pass-1 顺序文案族未扩（3 例，类型/时机一致）；
既有 emit bug `def g(a=(w := 1))` ＋ 内层 `nonlocal w` 进程 panic（未修）。④ `_io` 的 `.newlines`
为整块追踪、文本 `tell` 仍字符下标。⑤ **同族泄漏（已量化、未修，建议单开工单）**：
`builtin/int.rs:31`（`int(Plain())` 每次漏 2 颗 AttributeError+tb）、`builtin/set.rs::set_new`
（每次漏 1 个 set）、`instance/convert.rs` 排序比较探针、`instance/accessors.rs` 惰性注解、
`builtin_objects/callables.rs` 终结器（每颗新错误漏上一颗）；判据 `PYAWA_GC_LOG` 收支。⑥ 既有
symbols 缺口：annotation-only 外层绑定运行期缺 cell（`NameError`）；类 `__annotations__` 读取报
`AttributeError: '__annotations__' 只接类型对象与模块对象`。


### 12.7 引用泄漏结构性收口（2026-11-14，第三轮）

用户口径：「A（RAII）也要」＋「全部按阶段做（B+C+D → 6 处 → A）」。本轮把"`Err(Raised)` 那份
异常引用被静默丢掉"从**纪律**变成**类型上不可能**（不是逐分支打补丁）：

**阶段 0（诊断与防线）**
- **B 引用账本**：`crates/pyawa-core/src/error_ledger.rs`（每实例、无全局态；开关 `PYAWA_GC_LOG=4`，
  默认零成本）。`#[track_caller]` 把生成点记到真正的创建处；计数 `generated/pending/discarded/
  transferred/implicit/leaked/suspects/overwritten/untracked_consumed`；恒等式
  `generated == pending`、`generated == discarded + transferred + implicit + leaked + suspects + overwritten`；
  未消费项连打**生成点＋示例 Rust 栈**。测试 `crates/pyawa-runtime/tests/error_ledger_shapes.rs`（4 passed）。
- **D 失败路径探针集**：`tests/ci/failure_path_leaks.py`（11 探针，`tb_created−tb_freed` ＋
  N=10000/20000 缩放 `|Δlive|≤512`）＋ `crates/pyawa-runtime/tests/failure_path_leaks.rs`（10 passed）。
  修前基线 **6/11 RED**。
- **C 静态规则**：`CX-30`／`T-CX-20`（`tests/ci/check.py`）——core 四模块禁止裸丢弃 `Result`
  （`Err(_) =>`／`if let Err(_)`／`.is_ok()` 当语句／`let _ = <Result 调用>`），合法丢弃必须走
  `discard_error`／`discard_exec_result`／`discard_capability_result`；白名单逐条锚点＋**棘轮**
  （修好不删也红、只许缩）。**注意**：阶段 2/A 之后 RAII 让"裸丢弃"不再泄漏引用，本规则保留为
  **可审计风格规则**（防复制旧的"交回裸指针"模式），不再是内存正确性的必需。
- 顺带修好 `tests/ci/selftest.py` 的既有基线不干净（副本缺 `NEXT.md`／`Cargo.toml`／`.git`／
  conformance 报告 ⇒ 5 项假红）。

**阶段 1（6 处已知泄漏 + 同族扫描）**：`CX-30` 白名单 **55 → 18**（A 类 33 处全清为显式
`discard_exec_result`，B 类 4 处能力错误显式化；C 类 18 处无引用载荷保留），并修掉：
`builtin/int.rs` 的 `__int__/__index__` 探针、`builtin/set.rs::set_new`（set/frozenset）、
`instance/convert.rs` 排序比较探针、`instance/accessors.rs` 惰性注解、`builtin_objects/callables.rs`
终结器（旧 pending 未还）、`executor/iter.rs` 的 `truthiness` 绑定方法泄漏，以及 4 处风险 `.ok()`
（`attach_traceback`／`object_repr`／`str.translate`／`namespace`）。**D 探针 6/11 RED → 11/11 PASS**
（`f(*Plain())` N=20000：tb 净漏 20002⇒2、live 207295⇒7348）。

**阶段 2 / A（RAII `RaiseRef`／`PendingRef`）**
- `crates/pyawa-core/src/raise_guard.rs`：`RaiseRef`（持 `Err` 那份 owned 引用；**Drop 只还 `Err`
  那份、不动 `pending` 槽**；`discard()`＝旧 `discard_raised`；`into_pending()` 移交槽；
  `release_keeping_pending()` 供 `SystemExit` 等"宿主后读槽"路径；全部幂等）；`PendingRef`
  （`replace_pending` 交回的旧 pending，Drop 自动释放）。
- `ExecError::Raised { exception: RaiseRef }`（**删除 `Clone`**：裸克隆复制所有权；`PartialEq`
  按异常指针）；迁移 **83 构造点／45 解构点**；`Instance::set_pending_exception`（返回裸指针、
  旧值由调用方释放）→ `replace_pending`（返回自动释放的 `PendingRef`），迁移 13 调用点。
- 结构性收益：任何"忘记处理 `Err(Raised)`"的路径在 Drop 时自动还引用（不再靠 `discard_error`
  纪律）；终结器/旧 pending 一类同型泄漏在类型上消失。新增 `crates/pyawa-core/tests/raise_guard.rs`
  （4 passed：Drop 保留槽、`discard` 清槽、`into_pending` 移交、`PendingRef` Drop 释放旧值）。

**验收**：`PYAWA_DANGLING=1` 下 `cargo test --workspace --release` 全绿；conformance **408×2**
（普通＋悬垂）绿；`check.py` 20/20；`selftest.py` 24 项断言全绿；`cargo check --workspace
--all-targets` 0 警告；`bench_gate` 四档 ✓；`slowcheck --release` 十二项全绿。

**边界（如实）**：账本是诊断近似（"对象存活＋连续两次 flush"判未消费；ptr 复用留 `overwritten` 账）；
`RaiseRef` 存 `*const Instance`（不变量：错误只在本实例执行栈内传播/消费，与改动前"裸指针＋实例
存活"同契约）；副作用上 A 不动 ABI（`pa_*` 边界仍按状态码映射，`AB-21` 不变）。


### 12.8 失败路径 GC 探针接闸（2026-11-14）

`tests/ci/failure_path_leaks.py`（阶段 0 的 D 探针集，11 条失败路径 × N=10000/20000 缩放判据）
**接进 `tools/slowcheck.sh` 作为第 12 项**：步骤内先 `cargo build` CLI（release 档是 no-op），再跑
`--binary target/<profile>/pyawa`；任何 RED ⇒ 闸门红。`tools/gate_chunk.sh small` 同步纳入（分块
命令也能跑到）。计数口径从「十一项」更新为「十二项」（`AGENTS.md` 硬线、`agents-rules/round-rule.md`、
`docs/REQUIREMENTS.md`、`tests/ci/README.md`、`NEXT.md`）。验收：`slowcheck --release` **十二项全绿**
（第 12 项实跑 11/11 绿）；`check.py` 全绿。

### 12.9 全口径 TS 注解解析器（2026-11-14 起，分里程碑）

用户口径：「全口径（分多轮）」——把 `AB-52` 的"由 Pyawa 自己的注解解析器求值"与 `TS` 的注解
表达式语义收口（`SPEC-type-system.md` §16 的 `TS-51`…`TS-57`；一轮一笔提交）。

| 里程碑 | 内容 | 状态 |
|---|---|---|
| **M1 规格与模型** | `TS-51`…`TS-57`（语法范围/两阶段/`BC-24` 映射/失败语义/可逆投影/`AB-52`↔`AB-69` 同一性/验收）；`AB-52` 文本补解析器两阶段与 §16 引用 | ✓（2026-11-14） |
| **M2 语法层** | `pyawa_core::type_expr::TypeExpr` 语法树＋`compile::parse_type_expression(&str)`（复用 `compile/parser/types.rs` 的 `parse_type_at`；`?` 降级/联合摊平/单实参下标摊平/可逆投影）；`crates/pyawa-core/tests/type_expression.rs`（`T-TS-12`：语法矩阵/往返/错误） | ✓（2026-11-14） |
| **M3 求值层** | 名称解析（模块 globals→builtins→宿主注册类型、点号属性链、前向引用延迟）＋`typing` 构造（`Callable`/`Optional`/`Union`/`Annotated`/`List`/`Dict`/`Set`/`Tuple`）→ `SemanticType`／`BC-24` 静态标签（`to_static_label`，与 `AB-69` 同口径）；`Deferred`/`Unknown`/`Invalid` 如实分类；`crates/pyawa-core/tests/type_expression_resolve.rs`（7 passed） | ✓（2026-11-14） |
| **M4 ABI 接入** | `AB-52` `type_expr` 在 `pa_register`/`pa_module_addfunction` 注册时求值成 `AB-69` 的 `TyExpr`（新增 `Any`/`Platform` 变体），登记进 `host_typed_signatures` ⇒ 边界运行期＋编译期调用点检查对字符串通道生效；同一函数已有句柄签名不覆盖；坏字符串/未知名 ⇒ `PA_ERR_INVALID`，显式前向引用 ⇒ 平台类型；`.pyi` 仍原串；`crates/pyawa-abi/tests/annotation_strings.rs`（6 passed，含字符串↔句柄同行为、`.pyi` 原串） | ✓（2026-11-14） |
| **M5 编译期扩展** | **已落地**：①`TS-23`/`TS-24`/`TS-25`/`TS-39` 的**标注覆盖率与档位报告**（`compile::annotation_coverage`＋`AnnotationCoverage::to_report`；分列位置/仅关键字/`*args`/`**kwargs`/返回/变量，`x: T = v` 只算一格；`crates/pyawa-core/tests/annotation_coverage.rs` 4 passed）②`pa_newtype` 的 `sig` 字符串注册时**校验**（坏表达式 ⇒ `PA_ERR_INVALID`；`annotation_strings.rs` 7 passed）。**边界如实登记（未做，理由）**：③`BC-25`① 需**全程序信息**——`BC-23` 在**被调用方序言**发射，而"两侧都已标注 ⇒ 禁止发射"要求知道全部调用点（跨模块），单个编译单元不可判定；当前按"带标注保守发射"（`P3-13` 已登记）④`TS-37` 批量诊断：**已落地**（既有 `compile::check_source`＋`typecheck::check_diagnostics`，PLAN §10.1；执行路径 `compile` 仍首错即止属"不能继续执行"的合理口径）⑤`TS-38` 产物内嵌/`TS-39` 独立文件导出：需改 `IM-18`/`IM-19` 产物要素与黄金字节（另一窗口；运行期仍不读 fs） | 部分 ✓（2026-11-14），余项按上述边界登记 |
| **M6 收口** | `PLAN` §12.4 #1 状态更新（`type_expr` 求值不再"待"：已落地 M4）、`AB-52`/`§12.4` 交叉引用清理、`NEXT`/文档/机检同步；M1…M5 边界清单复核 | ✓（2026-11-14） |

**边界**：M2 只做**语法层**（不做实例相关判断/名称解析/求值）；`AB-52` 的字符串求值在 M4 才接；
`.pyi` 导出始终是**原字符串**（`AB-53`/`AB-54`，不得成为第二签名源）。


### 12.10 TS 注解解析器收口（2026-11-14）

用户口径「全口径（分多轮）」；**M1…M6 全部收口**（`SPEC-type-system.md` §16 的 `TS-51`…`TS-57`
是规格；`AB-52` 的"由 Pyawa 自己的注解解析器求值"已成可执行义务）：

| 里程碑 | 交付 | 验收 |
|---|---|---|
| M1 规格与模型 | `TS-51`…`TS-57`；`AB-52` 文本 | `check.py`（`T-CX-1` 编号/`T-CX-10` 连续） |
| M2 语法层 | `TypeExpr`＋`parse_type_expression` | `tests/type_expression.rs` 3 passed |
| M3 求值层 | `SemanticType`＋名称解析/typing 构造/标签投影 | `tests/type_expression_resolve.rs` 7 passed |
| M4 ABI 接入 | `AB-52` 注册时求值 ＝ `AB-69` 同一账本 | `tests/annotation_strings.rs` 7 passed（两通道同行为/`.pyi` 原串） |
| M5 编译期扩展 | 标注覆盖率/档位报告（`TS-23`/`24`/`25`/`39`）；`pa_newtype` sig 校验 | `tests/annotation_coverage.rs` 4 passed |
| M6 收口 | `§12.4 #1` 状态清零、交叉引用/`NEXT` 同步 | 本文档＋`git log` |

**未收进本工程的项（范围外＋诊断证据，均不影响"注解解析器"本身）**：
- `BC-25`①（"两侧都已标注 ⇒ 禁止发射"）：需**全程序**调用点信息（`BC-23` 在**被调用方序言**
  发射），单编译单元不可判定；现维持"带标注保守发射"（`P3-13` 已登记）——不是"默认关闭"。
- `TS-38`／`TS-39` 的**产物内嵌/独立文件导出**：需改 `IM-18`／`IM-19` 的 `.pyac` 产物要素与
  黄金字节（跨格式变更窗口）；运行期仍**不读**伴随文件（`TS-38` 的运行时约束保持），报告由
  `compile::annotation_coverage` 给出。
- `TS-34` 诊断缓存：`check_source` 是纯函数（`TS-33` 成立），缓存属 LSP/工具链步骤（未落）。

### 12.11 ABI 边界不可感知（2026-11-14 用户定向）

**目标**：ABI 边界**客观存在**，但使用者**不可感知**。规格已落两条（`docs/SPEC-c-abi.md`）：
`AB-73`（宿主类型属性/方法派发）、`AB-74`（宿主异常类型映射）。

**三条判据**：
1. 可感知的痕迹只应剩两处：跨界性能、身份语义（`is`／`id`）；
2. guest 侧只用对象语法（`obj.attr`／`obj.method()`／`len(obj)`／`for x in obj`），代码里不出现"宿主/handle"字样；
3. 透明**不得**以"假成功"为代价 —— `AB-22` 的"未实现／拒绝／未找到"三态在这些路径上仍必须成立。

**验收演练**：Python 宿主把 `list`／`dict`／一个自定义类包成宿主对象注入 guest，
guest 侧在**不出现任何宿主函数名**的前提下完成增删改查与迭代。

**实施分解（2026-11-14 用户授权"先做补丁，然后实现规格"）**：
- **A 规格补丁（docs，已落）**：`AB-73` 重写（`pa_type_ops` 自带尺寸结构／回调带
  `pa_state *state`／派发与 `PA_ERR_NOTFOUND` 回退／错误映射／getattr 返回宿主函数自动包成
  绑定方法且 `pa_sig` 含 `self`／协议复用引实现位置）；`AB-74` 重写（完整签名/栈契约/类型
  要求/返回 `PA_ERR_RUNTIME`/非法 `PA_ERR_INVALID`）；`§15.2` 新增 `PA_ERR_NOTFOUND=8`；
  `§15.3` 函数清单 75→76（`pa_newtype` 加 `ops` 末参、`pa_raise_host_exception`）；
  `§15.4` 辅助层 18→19（`paL_getref`）；`§13` 新增 `T-AB-8`／`T-AB-9`／`T-AB-10`；
  `PA_ABI_VERSION` 1.10→1.11（未发布窗口内的签名改写，发布后只许追加）。
- **B 实现**：`crates/pyawa-abi/src/host.rs`（`pa_type_ops`/回调类型/`HostTypeInfo` 存 ops 与
  `*mut pa_state` 桥接）＋`lib.rs`（`pa_newtype` 收 ops 并装 `getattr`/`setattr` 槽、getattr 结果
  若为宿主函数 ⇒ `MethodObject` 绑定、`pa_raise_host_exception`、`paL_getref`、`PA_ERR_NOTFOUND`）
  ＋`crates/pyawa-abi/include/pa.h`（结构/原型/状态码/版本）＋`PA_ABI_MINOR=11`。

  **B1（2026-11-14 已落）**：`PA_ERR_NOTFOUND=8`（预留改 9–31）＋`PA_ABI_MINOR=11`；`host.rs`
  的 `pa_type_ops`/回调类型/自带尺寸界读；core `Slots.host_getattr`（**有错误通道**）＋
  `TypeObject` 不透明 `host_data`；`pa_set_type_ops`（追加式，`NULL` ⇒ 清空）＋
  `host_type_getattr_slot`（`PA_OK` 恰好一个值／`PA_ERR_NOTFOUND` 回退／其它 ⇒ `RuntimeError`）＋
  `paL_getref`；`pa.h` 结构/原型/状态码；**测试** `crates/pyawa-abi/tests/host_visibility.rs`
  （属性派发/回退/错误三态/零回归，2 passed）。
  **B2a（2026-11-14 已落，`AB-73` 收口）**：core `Slots.host_setattr`（有错误通道）＋
  `instance_attribute_set` 回退钩子（数据描述符之后、默认实例字典写之前；`Ok(false)` ⇒ 默认，
  `Err` ⇒ 上抛）；ABI `host_type_setattr_slot`（值以**借用**槽压栈、`value_index=-1`、回调不得
  消耗/压弹栈）；**绑定方法**用 ABI 内部对象 `HostBoundMethod`（函数 ＋ 接收者；call 槽前插
  接收者后走宿主函数调用路径，`pa_sig` 含 `self`），不用 core `MethodObject`（其调用路径假设
  内层是 Python 函数载荷）；`len`/`for`/`+` 协议经属性通道生效（`for` 的探测位补
  `host_getattr`；`+` 的宿主 dunder 走 `attribute_lookup`）。测试扩到 **3 passed**：
  `setattr`/`getattr` 往返、`obj.method(5)` 绑定、`len`、`for`、`+`、宿主错误三态。
  **B2b（2026-11-14 已落，`AB-74` 收口）**：`pa_raise_host_exception`（`type_idx` 栈索引指向
  `BaseException` 子类；`msg` 借用、`len=-1` 允许；VM 用 `call_value` 按普通构造语义实例化，
  构造异常也记录；宿主函数返回边界经 `raise_object_public` 重抛，guest 可 `except`）；
  **`§12.11` 验收演练**：三个宿主对象（列表式/字典式/自定义式）注入 guest，guest 只用对象语法
  完成增删改查与迭代，源码断言**不含** `host`／`handle`／`pa_` 字样；`host_visibility` 测试
  **5 passed**；`SPEC-c-abi` §16 未实现条目清零。**`§12.11` 三条判据全部成立**：
  ① 可感知痕迹只剩跨界性能与 `is`/`id`（演练里对象身份即宿主对象身份；无额外包装可见）；
  ② guest 源码对象语法（机检断言）；③ 三态（成功/未处理/出错）可区分（`T-AB-8` ⑤/`T-AB-9` ④）。
- **C 验收演练与收口**：`crates/pyawa-abi/tests/` 下新增 `host_visibility` 测试（派发→回退→协议→零回归；
  异常映射；§12.11 演练：宿主把 `list`/`dict`/自定义类包成宿主对象注入 guest，guest 源码
  **不含** "host"/"handle" 字样或宿主函数名，完成增删改查与迭代；`is`/`id` 口径登记）；
  `SPEC-c-abi` §16 未实现条目清零、`PLAN` 台账、`NEXT` 同步；`check.py`＋`slowcheck` 十二项。
- **判据映射**：判据 1 ⇒ `T-AB-10` 的 `is`/`id`/性能登记；判据 2 ⇒ `T-AB-10` 的源码扫描断言；
  判据 3 ⇒ `T-AB-8` ⑤/`T-AB-9` ④；`T-AB-8`/`T-AB-9` 覆盖两条规格的验收细则。
