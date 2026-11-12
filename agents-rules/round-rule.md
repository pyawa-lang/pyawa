# 轮次规则

> 轮次是资源，落地才是产出。本文件规定"一轮允许怎么用、什么算违规、谁能判"。
> 反面形态（真实发生过）：连续几十轮只做"定位一格 + 写一段交接"，判据① 180 轮不动。

## 1. 一轮一落地（硬边界）

每轮**只能**以下面三者之一结束：

| 结束形态 | 判据（谁都能查） |
|---|---|
| **(a) 提交** | 该轮结束时 `git log --oneline -1` 相对轮首**前进** |
| **(b) 撤回 ＋ 一条改变计划的事实** | `git status` 从脏回到干净，且有一条**会改变下一步选活**的新事实被记录（不是"又确认了一次"） |
| **(c) 不收工** | 继续做，直到 (a) 或 (b) |

- **违规轮**：只写交接、只写台账、只把"定位到行号"当产出、只重述已知事实。
- 定位**不是**产出：定位只在**同轮内**作为修复的中间态存在；当轮不修，就必须按 (b) 记一条改变计划的事实。

## 2. 开工先定验收

- 每轮开始就写死两件事：**本轮要落什么** ＋ **用哪条命令验收**。
- **没有验收命令的活不开**。验收命令必须能在轮内跑完（秒级～分钟级）。

## 3. 快慢两档验收

- **快档**（轮内反复用）：`tools/quickcheck.sh`（编译 ＋ 编译期用例 ＋ 关键护栏 ＋ 可选探针）。
- **提交前快档**（`tools/fastcheck.sh`）：`cargo check --all-targets` 0 警告 ＋ 逐字节 ＋ `tests/ci/check.py` ＋ 夹具 ＋ 语料下限 ＋ 少量"必须保持绿"的官方套件烟测（含基线计数断言，不放松判定）。
- **慢档**（`tools/roundcheck.sh`）：十一项 `&&` 闸门（`cargo check --workspace --all-targets` 0 警告／逐字节／`tests/ci/check.py`／`stability.py`／`heap_and_concurrency.py`／对拍普通与 `PYAWA_DANGLING=1`／语料下限 182／夹具 490）＋ 官方全量 survey，**两者并行**跑（闸门内 ③ 由 ⑤ 覆盖、⑨⑩ 并行；survey 默认 `--jobs 4` 让核）。
- **分档节奏（2026-11-09 用户批准提速）**：
  - **行为变更轮**（改动默认路径／运行时语义）：提交前必须 `tools/roundcheck.sh` 全绿（十一项 ＋ survey）；
  - **纯诊断轮／默认零输出轮**：`tools/fastcheck.sh` 全绿即可提交；但**每两轮至少一次** `tools/roundcheck.sh`，且下一次行为变更轮必须补；
  - survey 同节奏：行为变更轮必跑；诊断轮可并到"每两轮一次"的整轮验收里。
- **环境上限下的等价分块**（round 132 起；DSH 会话单条命令约 **600 s** 被 `SIGTERM` 时）：
  `tools/roundcheck.sh` 整体（gate 800+ s）与完整 survey 都跑不完 ⇒ 用
  `tools/gate_chunk.sh small|stability|heap|conformance` **四块**分别跑（合起来＝十一项，各块日志
  `target/roundcheck/chunk-*.log`），survey 用 `python3 tools/libtests_survey.py --release --resume --seconds 420`
  **多次续跑**直至跑满 397 文件（进度 `target/libtests_survey.progress.json`，`--timeout` 口径不变）。
  四块全绿 ＋ survey 跑满 ⇒ 视为**慢档等价通过** ✓（台账里写明分块数字）。
  survey 分片跑法：每片 `--start/--end` 跑完把 `target/libtests_survey.tsv` cp 到
  `target/recon/<名字>.tsv`；全部分片完成后用
  `python3 tools/libtests_survey_merge.py target/recon/<名字>_*.tsv [--extra 名字:kind:理由]` 合并并打印统计 ✓。
- **禁止**用慢档做轮内迭代；**禁止**用快档替代**行为变更轮**的提交前闸门。

## 4. 时间盒与升级

- 同一处 bug **最多 3 轮**；3 轮不落地 ⇒ **停手升级**：把选项（换环境／改口径／弃该支／加预算）交用户决策，不得继续烧轮次。
- 判据①（`tools/lib_import_ratio.py`）**连续 5 轮不动** ⇒ 视为**停滞**，直接报告并升级（不得原地继续推进定位）。

## 5. 交接瘦身

- 续做状态只写单页 `NEXT.md`：**HEAD**／**工作区状态**／**下一条命令**／**未修 bug ＋ 判据**。
- **`NEXT.md` 必须与当轮同笔提交**（与台账同理：记录随实质改动走）。提交前**核对两行**：
  **HEAD** 行等于 `git rev-parse --short HEAD`、**工作区状态**行与 `git status --porcelain` 一致
  ⇒ 否则"只看单页"这条约定**当场失效**。（真实发生过：单页写 `HEAD=3e5c70f`，实际已到 `90b4040`，
  差 10 笔，且"已修待提交"的两条其实早已提交。）
- 长复盘进随代码提交的台账；**不得**只写台账（`agents-rules/commit-rule.md` §1：禁止纯台账提交）。

- **本页有硬上限**（`CX-23`／`CX-24`，机检 `tests/ci/check.py` 的 `T-CX-14`／`T-CX-15`）：**≤ 40 行**；**禁止**写逐轮复盘（历史只进 `docs/rounds/`）；**HEAD 行必须是短哈希**（真实提交、能 `git cat-file -e`），**禁止**写指针。超限或写指针 ⇒ 闸门红 ⇒ 不得提交。

## 6. 先易后难（取活顺序）

1. **能过十一项闸门的增量**优先（解析器／语义修复、收账）；
2. **需要新载荷类型／新环境**的活**整体推迟**，不拿轮次去磨；
3. 取活按"**阻塞最多模块的报错签名**"队列，而不是按 fan-in 想象；
4. 收账后**必须**两道必检：**逐个导入** ＋ **无关脚本 `startup ok`**（`tools/find_syncable.py` 的"通过"不足以证明可入 `Lib/`）。

## 7. 目标轮次上限

- 目标设 `maxGoalRounds` 上限；**达到上限即停**（`round-limit`），不自动续跑。
- 加预算 ≠ 放宽本规则：**预算增加时，规则照旧**（一轮一落地）。
