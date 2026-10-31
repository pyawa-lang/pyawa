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
| ~~**P3-29**~~ **首片已落地（round 10：`socketpair` 真 I/O；其余如实未接线）** | **`socket` 支**（**用户已拍板立项**，会话 05，排在 typing 收口之后）：`_socket`/`socket` 的最小面——`http.client`／`urllib.request`／`socketserver`／`asyncio` 的共同前置（上限诊断 overlay 口径 **41 个模块**压在这上面）；先 POSIX 域的无连接面（`socketpair`/`create_connection` 一族随后），逐条对参照 | `CM-14`、`DESIGN.md` §9 | 无（工作量大，单独立项 ✓） |
| **P3-30** | **`random.Random(<seed>)` 的序列与参照不一致**（会话 05 语料侦察实测：`Random(42).random()` ⇒ 0.0838… vs 参照 0.6394…，参照文档保证同种子同序列 ⇒ **真差异**）：生成算法/种子混合口径要对齐 CPython 的 MT19937 init_by_array | `MS-19`、`MS-10` | 无 |
| ~~**P3-31**~~ **已解除（round 3）** | **解析器缺口批**：**10 格全落**（嵌套元组目标／`.05` 前导点浮点／行内 if 分号表＋次行 else／`*` 后置关键字实参／`with … as (a,b)`／推导式模式目标／元组里的 `*`／带注解赋值非名字目标／制表符缩进／调用基底下标赋值）⇒ `difflib`／`gettext`／`xml.etree.ElementTree`／`tomllib`／`argparse` 等 11 个模块同步 ⇒ 判据① 341→358 ✓。剩余同族缺口（PEP 604 union 注解 dataclass 字段的 `__builtins__` 面）挂账后续。清单全文＝台账卷 `03-m3-progress-05.md` 的 task-7 表 ✓（一处真相，不重述） | `BC-14`、`CM-14` | 无 |
| ~~**P3-32**~~ **已完成（round 14）** | **官方 `Lib/test` 的框架门槛**（`tools/libtests_survey.py` 实测 397 模块 **0 通过** ✗）：根因＝**`sys.exit` 缺失**（`unittest.main()` 结尾必用）＋ **`unittest.main()` 直接崩 `KeyError: 'dest'`**（选项解析路）⇒ 修好这两处，官方套件才**开始**跑。**落地**：`sys.exit`（抛 `SystemExit`；退出码口径见 `SPEC-c-modules.md` §5.2.3）＋ CLI 把未捕获 `SystemExit` 解释成进程退出码；`unittest.main()` 全路打通（`dict` kwargs／`None` 键、argparse 的 `os.terminal_size`、`sys` 流 `write`、富比较协议、类内方法闭包转发等前置一并落地）⇒ **官方套件首次跑通**（≥1 个模块；全量调查与台账见 `NEXT.md` 与 `docs/rounds/`） | `REQUIREMENTS` 的兼容承诺（保证层）＋ `MS-19` 判据补充 | 无 |
| ~~**P3-33**~~ **已完成（round 14）** | **`sys.path` 不认 `PYTHONPATH`**（参照认 ✓、本层只有"脚本目录 ＋ `Lib`" ✗）⇒ `import test.support` 一类外部路径全灭；与 `IM-24`（`sys.path` 由 `site.py` 构造）的 C 层那半需补齐。**落地**：`PYTHONPATH` 进 `sys.path`（组合根过渡半，口径见 `SPEC-c-modules.md` §5.2.3 的 `path` 条目）＋ `test/support` 包骨架补同步 **21** 个上游文件（19 个骨架 ＋ `_sysconfigdata*` 2 个数据模块）⇒ `Lib/test` 的 .py **42→61**、`Lib/` 同步 **544/628**、`sync_lib.py --check` **548** 个文件逐字节一致（例外清单空）。**缺口如实记**：另有 **7** 个 `Lib/test/*` 骨架因 `P3-35` 的合法 Python 被拒（surrogate escape／链式赋值非名字目标／for-star／PEP 695 等）**暂不同步**（不登记 `KNOWN`，列进 `P3-35` 队列）；`SYNC_EXCLUDE` 另排除 `multiprocessing/heap.py`／`unittest/mock.py`（同因）。细节见 `NEXT.md` 与 `docs/rounds/` | `IM-24`／`IM-26`、`MS-19` 判据补充 | 无 |
| ~~**P3-34**~~ **已完成（M6 期实测 `1j` ✓）** | **`complex` 类型面**：`1j` 字面量解析不了（状态 5）⇒ **52 个官方测试直接阻塞**（调查实测） | `TS-42`、`CM-27` | 无 |
| ~~**P3-35**~~ **主体已完成（M6 期；剩余形态见 `NEXT` 的未修清单）** | **合法 Python 被拒的语法缺口**（`MS-19` 判据补充：**必须修、不得登记为差异** ✗）：surrogate escape `\ud800` 类 **20 个**／同行 `try:`·`for:` **8 个**／解析器瑕疵（`语句结尾多出了 Some(Str(…))` 5、`async` 在列表里 1、`要冒号却得 Some(Dot)` 1） | `MS-19` 判据补充、`BC-15` | 无 |
| **P3-36** | **`.pyac` `BadCodeSection`**（6 个官方测试触发 ✗）：代码段读不出来 ⇒ 缓存/序列化真 bug，按 `IM-18`…`IM-21` 的容器契约查 | `IM-18`…`IM-21` | 无 |
| ~~**P3-37**~~ **主体已完成（M6 期实测：`doctest`／`gc`／`hashlib`／`unicodedata`／`unittest.mock` 均已可导入 ✓；`pyexpat` 仍缺 ✗）** | **官方套件的缺模块**：`doctest`(15)／`unittest.mock`(12)／`gc`(10)／`unicodedata`(4)／`hashlib`(4) —— 按 `CM-14` 的 fan-in 取；`unittest.mock` 与 `P3-32` 同族（先修框架） | `CM-14`、`P3-14` | 无 |
| **P3-38** | **`str` 表示层支持合法孤立代理**（**WTF-8 级改造；surrogate epic**，**用户已拍板 2026-10-25**）：动机＝`MS-19` 判据补充「**合法 Python 被拒必须修、不得登记差异**」——当前 `Lib/test/support/os_helper.py`（`'\udeee'`／`'\uDC80'` 等）与 `Lib/test/multibytecodec_support.py`（`'\udeee'`）是**唯一阻塞**；survey 里 **30** 个 `test_*.py` 因 surrogate 转义报**语法错**、`os_helper` 缺模块 **×106**；另 `surrogateescape`／`surrogatepass` 是 **PEP 383** 的语义面。**分阶段（每阶段＝可提交的独立切片）**：**S1 `wtf8.rs` 编解码基础**（本轮 super-fix 落地中：`encode`／`decode`／`validate`／往返／错误用例；**零风险**，不动既有表示）；**S2 `StrObject` 存储迁 WTF-8 字节 ＋ `text_of` 语义**（`Some(&str)` **仅对合法 UTF-8**，新增 `wtf8_bytes()` 逃生口）；**S3 各 API 面迁移**（按审计报告分小片：repr／print／encode／decode／比较／哈希／切片／格式化／属性名／字典键／fs 路径／正则／`unicodedata`）；**S4 lexer 接受合法孤立代理转义** ＋ `str.encode`／`decode` 的 `surrogateescape`／`surrogatepass`；**S5 `Lib/test/{support/os_helper,multibytecodec_support}.py` 等重试同步 ＋ 回归**。**逐阶段验收**：S1 ＝ WTF-8 合法／非法样例的往返与错误分类对拍夹具；S2 ＝ 既有 `text_of` 调用点全绿 ＋ 合法 UTF-8 路径零行为变化（`.py` 编译产物逐字节不变）；S3 ＝ 每片单独提交、该片含代理样本的对拍用例通过且不回归；S4 ＝ survey 的 **30** 个 surrogate 语法错族归零 ＋ PEP 383 两套 codec 语义对拍；S5 ＝ `tools/libtests_survey.py` 的 `os_helper` 缺模块 **×106** 与对应两文件相关格归零、语料无新差异（`MS-19`）。**M6 收口 gate 含本条完成**（"新差异为零" ＋ `MS-19` 补充；`MS-14` 的报告口径不变）。 | `MS-19` 判据补充、`CM-13`（不变量）、`BC-15`；`str` 的既有条目只引用（`TS-41`／`TS-42`／`TS-43`、`BC-4`） | 无（用户已拍板；S1→S5 顺序、每阶段独立提交） |
| ~~**P3-39**~~ **已完成（round 33）** | **`io.StringIO` 不可实例化**（`TypeError: cannot create 'StringIO' instances`）⇒ **15 个官方测试阻塞**（分诊实测）；`io`／`_io` 面按 `CM-14` 补。**落地**：`_io.StringIO` 由占位类型换**真类**（基类 `_TextIOBase`；21 个 native 处理器：`__init__`／`write`／`getvalue`／`close`／`read`／`readline`／`readlines`／`writelines`／`seek`／`tell`／`truncate`／`flush`／`readable`／`writable`／`seekable`／`isatty`／`detach`／`__iter__`／`__next__`／`__enter__`／`__exit__`）；位置是**码点**下标、越界 `write` 用 `'\x00'` 补齐、关闭态文案两档、文本流 `seek` 规则、`writelines`／`detach`／文本流属性面全部照本机 3.14.4 实测；顺修 `BytesIO`／`StringIO` 共有的 `readlines` 到 EOF 多推空串 ＋ `sizehint` 一处真 bug。**验收**：对拍探针 pyawa 与 `python3` **逐字节 0 diff**；`test_memoryio` 跑通 **20⇒43**（errors 148⇒111）；`test_io`／分诊里 `cannot create 'StringIO' instances` **归零**。**如实未接** ✗：`newline` 换行翻译、`__getstate__`／`__setstate__`／pickle、`IntLike`（`__index__`）实参（属 `P3-42`） | `CM-14`、`P3-37` | `crates/pyawa-stdlib/src/`、`Lib/io.py` |
| ~~**P3-40**~~ **已完成（round 34）** | **`os` 面缺口**：`os.mkdir` 缺失（**8** 个测试 ✗）／`os.pipe` 缺失（**6** ✗）／`os.unregister` 一类（2 ✗）—— 分诊实测。**落地**：`posix.mkdir(path, mode=0o777)` 走 `fs` 的 `mkdir` 槽（provider 改 **`mkdir(2)`** ⇒ `mode` 真生效、umask 由内核施加、errno 原样带回 ⇒ `FileExistsError` 等子类 ✓）＋ `posix.pipe()`（`CpFsVtable` **表尾追加** `pipe` 槽，`AB-41` ⇒ `PA_ABI_MINOR` **3→4**；两端不透明句柄 → 实例能力表下标，`CP-31` 动态段从 3 起、与 fs 共用一张表）＋ `codecs.unregister`（**更正分诊标签**：那 2 个测试要的是 `codecs.unregister`，CPython 的 `os` 没有 `unregister`）＋ `SPEC-capabilities` §9.1／`SPEC-c-abi` §12／`pa.h` 同步。**验收**：对拍探针与 python 3.14.4 **逐字节 0 diff**；分诊里 `mkdir`(8)／`pipe`(6)／`unregister`(2) **归零**。**如实未接** ✗：`os.close` 已关 fd 仍静默、`get/set_inheritable`、`os.chmod`／`scandir`（下一层缺口）。 | `CM-14`、`P3-37` | `crates/pyawa-stdlib/src/`、`Lib/os.py` |
| **P3-41** **主体已落地（round 32）** | **官方套件里的 hang 与崩溃**（12 个"超时"逐条复跑后的裁决）：**1 个崩溃** ✗✗（`test_frame` exit **134** ＝ SIGABRT —— 属 `AB-29`／不变量 3 类，**最高优先** ✓）＋ **1 个真 hang** ✗（`test_tempfile`：`map()` 急求值消费无限 `_RandomNameSequence`，无界内存）＋ **5 个只是慢** ✓（round 32 **逐条复核推翻"6 个真 hang"的原判**：`test_dict`／`test_weakref`＝dict 线性扫描 O(n²)；`test_long`／`test_peepholer`＝`unittest.subTest` 机器开销＋不释放；`test_userstring`≈15 min，次因 `startswith/endswith` 忽略 `start`／`end`）。**落地**：① `test_frame` 不再 abort（`repr` 递归守卫加深度计 ⇒ `RecursionError` ＋ RAII 摘守卫；`set`／`frozenset` 补守卫）⇒ **exit 134 ⇒ 状态 5（如实"未实现"）**；② round 32 顺带修新暴露的崩溃族：`for`／`while` 体末尾 `if …: break` 前面夹 `try` ⇒ 回边丢失 ⇒ `StackUnderflow`（列表断言失败会中止整个官方测试文件 ✗）；③ 6 个 hang 全部给出**最小复现**（`target/recon/hang1/`、`hang2/`，细节见 `docs/rounds/04-post-m6-01.md`）。**未修（如实）**：`map`／`filter`／`zip` 惰性化（真 hang 的根因）、dict 哈希化、`subTest` 释放、`startswith` 的 start/end。 | `AB-29`、`CX-` 崩溃面、`MS-19` 判据补充 | `crates/pyawa-core/src/`、`crates/pyawa-stdlib/src/` |
| **P3-42** **主体已落地（round 35，6 组里 5 组）** | **断言失败签名批次**（分诊 `target/libtests_triage.tsv` ✓）：`int.from_bytes` 缺 `byteorder`(2) · `time.struct_time` 缺(2) · `compile()` 不认 flags(2) · `Exception.__setstate__` 缺(2) · `_opcode.is_valid` 缺(1) · `KeyError: 'closed'`(3) · `KeyError: N`(7) —— 逐条按 `MS-19` 判据补充处理（合法 Python 报错＝新差异、必须修 ✗）。**落地（round 35）**：前五组签名**分诊归零**——`from_bytes` 缺省 `byteorder='big'`；`time.struct_time` 真类型（可序列／9 个 `tm_*` 属性／`tm_zone`／构造与 repr 照本机 3.14.4；`localtime`／`gmtime` 返回它、`mktime` 接受它）；`compile()` 收 `Lib/codeop.py` 的 `PyCF_*` 与 `__future__` 位（barry／AST／顶层 await 诚实未实现）；`Exception.__setstate__`；`_opcode.is_valid`。跑通 18⇒**20/397**（`test_baseexception`／`test_strftime` 转 ok）。**未清（如实）**：`KeyError` 族 12 行——根因两层：(a) **特殊方法派发缺口**（`==`／`!=` 不走 `__eq__`、`__add__` 等不派发；`test_binop`／`test_opcodes` 卡它），(b) `unittest` 的 contextmanager 生成器里 `KeyError` 在测试体执行期间仍处"正在处理"⇒ 断言异常的 `__context__` 被污染（`test_keyword` 等显示成 `KeyError: N`）。另：`time.struct_time` **不是 tuple 子类**（`isinstance(t, tuple)` 假 ⇒ `test_structseq.test_tuple` 仍 FAIL）。 | `MS-19` 判据补充、`P3-35` | 随修复点 |

### 9.3 文档侧待办

**当前无待办。** 原先列的三条（`§13-3`／`§13-7` 的候选、`CP-` 领号、`errno` 常量来源）
**已全部落定**；剩余 6 项规格缺口（`BC` 超出 §10 的指令、`CM` 逐模块合约表、`MS` 四项）
**全部等实现**——按 `SPEC-INDEX.md` §5.1 对 `v0` 的定义，文档侧已成立。

### 9.4 下一阶段的开工指示（**非规范**）

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
