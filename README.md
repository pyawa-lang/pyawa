# Pyawa

用 Rust 从零实现的、语义对标 **CPython 3.14** 的 Python 方言。
核心是**像 Lua 一样易嵌入**：宿主直接实现沙箱接口、脚本不可信且不可逃逸、纯 Python 库逐字可用。
另提供**程序级可选的安全谱**（渐进类型，非强制）。**目标不是更快的 Python。**

当前状态：**M0 · M1 · M2 · M3 · M4 · M5 · M6 均已成立**。

- **M0**（文档集）：14 份全部写出（状态列见 `docs/SPEC-INDEX.md` §1）；两条判据（`CX-1` 编号零悬空
  ＋重复定义、`CX-2` 文档集状态一致）由 `tests/ci/check.py` 承载，当前 **14/14 绿**
- **M1**（最小可嵌入内核，`docs/PLAN-milestones.md` §6 ①）：判据本件是
  [`examples/m1.c`](examples/m1.c)——**45 行** ≤ 50（`MS-21` 要求入库），真编译、真链接、真运行，
  输出 `42`、退出码 0；验收脚本 [`tests/ci/t_ab_1.py`](tests/ci/t_ab_1.py)（**缺 C 编译器即红，
  不降级**）。M1 ② 的启动延迟与常驻内存按 `§13-17` 是**提示项、不作判据**，报告**已出**：
  `tools/measure_footprint.py` 实测（release）VM 引导 **48 µs**、宿主整程 **1.05 ms**、峰值 RSS
  **3.4 MB**，数值与口径的唯一出处是 `docs/DESIGN.md` 的"Pyawa（M1 最小内核）实测基线"
- **M2**（核心语义，`docs/PLAN-milestones.md` §6）：判据是"对拍 harness 在 `MS-13` 的语料上**无新差异**"。
  实测（`cargo test -p pyawa-abi --test conformance`）：**407/407 通过 · 0 已知差异 · 0 新差异**；
  `MS-8` 的比对项**齐**（退出码／stdout／stderr／未捕获异常 ＋ 探针）；`MS-12` 自检 **407/407**；
  `MS-13` 的语料下限**逐面满足**（可执行判据 `tools/check_corpus_floor.py`）；`MS-25` 连跑三次数值一致。
  在册已知差异 3 条（`DIV-1`／`DIV-2`／`DIV-8`）**全属 `MS-19` 的合法类**，且当前**无语料命中**
- **M4**（类型系统，`docs/PLAN-milestones.md` §6）：扩展模式（`.pyawa`）的渐进类型 **A 批（类型链）**——
  `TS-46`（`T?`）→ `TS-47`＋`TS-48`（变量标注即静态类型／平台类型）→ `TS-50`（收束）。两层检查：
  **编译期静态检查器**（`crates/pyawa-core/src/compile/typecheck.rs`，`CompileError::TypeAssignment`）＋
  **运行期** `CHECK_ASSIGNMENT`（`BC-65`）／`TypeAssignmentError`。判据①（`T-BC-8`／`T-BC-9` 可观测）的
  可执行归档见 `crates/pyawa-core/tests/m4_acceptance.rs`；判据②（**渐进保证** `T-TS-1`）见
  `crates/pyawa-core/tests/gradual_guarantee.rs`。`.py` 下同源码**合法且惰性**（`BC-15`／`TS-5`，编译产物逐字节不变）。
  如实缺口：`BC-25`①（"两侧皆标注处禁止发射"，需跨模块静态信息）未做；`TS-50` 部分形态与类体/嵌套函数边界未覆盖
  （台账卷 05 的 round 4–7 节）
- **M5**（能力层正式化，`docs/PLAN-milestones.md` §6）：判据① `SPEC-capabilities.md` §11 的 `T-CP-*` **13/13 全绿**——
  可执行归档 `crates/pyawa-abi/tests/capability_acceptance.rs`（11 例）／`crates/pyawa-runtime/tests/two_providers.rs`（7 例）／
  `t_cp10_cancel.rs`（4 例）／`select_dup2_shapes.rs`（1 例）／`capability_registration.rs`（6 例），静态两条由
  `tests/ci/check.py` 的 `T-CX-4` 承担；判据② **两 provider 写法不变**（同一脚本逐字节相同，真实 `PosixFs` vs 受限
  `RestrictedFs`）；判据③ 启动延迟与常驻内存**升为阻塞项**（`§13-17` 已升），**阻塞阈值经用户拍板**——三档基线数字与
  阈值的唯一出处是 `docs/DESIGN.md` §9 的「Pyawa（M5 实测基线）」末节（判据① Lib import ratio **477/628 ＝ 76.0%**）。
  如实缺口（不阻塞判据）：`CP-22` 的 `os.getcwd/chdir` stdlib 面未接；`CP-21` 的逐段 Dir 能力链为 v1 简化档；
  `T-CP-10` 不含无 timeout 的阻塞 `recv/send` 与 Python 层 `KeyboardInterrupt` 投递；`AB-43` 消费方有界读取待做；
  `dup/dup2` 到 0/1/2 未接；C 面 `_codecs_hk`／iso2022／gb18030／hz 未做（台账卷 05 的 round 12–13 节）
- **M6**（兼容性验收，`docs/PLAN-milestones.md` §6）：判据 **`MS-14` 报告可发布、新差异为零**——
  `cargo test -p pyawa-abi --test conformance` 实测（参照 Python 3.14.4）：**407/407 通过 · 已知差异 0 · 新差异 0**，
  四要素齐，`PYAWA_DANGLING=1` 双档均绿；语料 **407** 条与在册差异 **3** 条（`DIV-1`／`DIV-2`／`DIV-8`）未变。
  `MS-19` 判据补充（合法 Python 被拒必须修）随 `P3-38` surrogate epic 收口：`Lib/test` 体检 **语法错 0／可编译 397/397＝100%**，
  `import test.support.os_helper` 通过、`multibytecodec_support.py` 同步且可导入，`\ud800` 字面量／PEP 383
  `surrogateescape`·`surrogatepass`／fs 路径·argv·readlink·输出边界均落地；判据① **477/628＝76.0%**、
  `Lib/` 逐字节同步 **739** 文件（`--check` 全一致）。如实缺口（不阻塞判据、留队列）：未实现形态（`\N{…}`／`except*`／
  async 推导式／PEP 695／下标删除／`import *`）、`socket` I/O、若干缺模块、3 个编译器 cell/free panic 用例、`tb_next` 链、
  fs/io 其余面（台账卷 05 的 round 22–30 节；技术落点见 `docs/PLAN-milestones.md` §6「M6 的要求」）
- **缺口**：**规格内已无未写小节**（原先两处——`SPEC-bytecode.md` 的"超出 §10 的指令"与
  `SPEC-c-modules.md` 的"逐模块合约表"——**均已落地**：前者为按需流程、后者为维护中的表）；
  `PLAN-milestones.md` §10 里的 **harness 实现**与**基线语料下限**也均已落地，
  **M0–M6 的可执行判据均已成立** ✓（M6 判据已由 `cargo test -p pyawa-abi --test conformance` 承担；M3 判据① **477/628 ＝ 76.0%** ✓ 仪器 `tools/lib_import_ratio.py`；判据② **已裁定达成** ✓ 仪器 `tools/corpus_coverage.py`）。这是 `v0` 的既定含义

## 文档（`docs/`，全部平铺）

**冲突时的优先级**：`REQUIREMENTS.md`（决策）> `SPEC-*.md`（执行细则）> `DESIGN.md`（架构论证）。
跨文档引用**只写编号、禁止重述内容**，规则见 `SPEC-INDEX.md` §2。

**文档清单（文件、范围、ID 前缀、状态）唯一出处为 `docs/SPEC-INDEX.md` §1**，本文件不重述。
本文件只报完成度：**共 14 份，已写 14 份 / 待写 0 份**（含非规范的逐轮台账 `ROUNDS.md`）。

## 目录

```
AGENTS.md      给 AI agent 的边界（每次任务都守）＋ 细则索引
agents-rules/  不常发生的细则：commit-rule.md / branch-rule.md / docs-rule.md
docs/          文档集（14 份，全部平铺：引用以文件名 + 编号为准）
docs/rounds/   逐轮台账分卷（**非规范**、只增不改；索引见 `docs/ROUNDS.md`）
crates/        Rust 工作区：pyawa-core / capabilities / abi / stdlib / runtime
tests/         conformance/（与 CPython 对拍）· ci/（不变量静态检查）
tools/         LSP、调试器、profiler（M6 后）
Lib/           CPython 3.14 纯 Python 标准库，逐字同步、禁止本地修改（M3 起引入）
```

## 分支与合并约定

- **`main` 是主干，只接受合并**：改动先提交在工作分支 `dev`（需要并行时再从 `dev` 开 `dev/<主题>`）
- **合并一律 `--no-ff`，必须留下合并提交**——合并提交是"这批改动是什么、何时进来"的唯一痕迹；
  快进合并会让工作分支白建，也让 `main` 的历史失去阶段边界。仓库已设 `merge.ff = false`，
  因此 `git merge dev` 默认即留痕
- 合并提交的说明写清这批改动的**范围与落点**（工作分支上的提交号），便于日后定位

```sh
git checkout main && git merge --no-ff dev -m "Merge branch 'dev': <这批改动是什么>"
git checkout dev
```

## 架构不变量

四条架构不变量**唯一出处为 `docs/DESIGN.md` §3**（不变量 1–4）：违反其中任何一条都需要推翻重来。
本文件不重述其内容。

## 当前未决

未决项按"什么时候必须定"分档，**唯一出处为 `docs/DESIGN.md` §13**。

- **A 档（动手前必须定）**：只剩 `§13-10` 的两个细项（`MS-9` 规范化容差、`MS-13` 基线语料下限），
  要等**首次对拍**才有事实
- **B 档（公开第一个 ABI 之前拍板）**：**已清空**——原 3 项（`§13-1`／`§13-4`／`§13-17`）全部已决
- **C 档（可边做边定）**：`§13-11`（是否需要 WASM 目标）、`§13-12`（扩展特性清单为空——
  机制先于特性，**有意如此**）

⇒ **目前没有卡开工的未决项。**
