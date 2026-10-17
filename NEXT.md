# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-17 ✓）：本会话（Lead＋队员团队模式）已收 **round 1＋round 2**；
> 逐轮复盘见 `docs/rounds/03-m5-progress-05.md` ✓。

## 现在

- **HEAD**：`570cb80`（`dev`，**已推送** origin ✓）—— round 2 三笔（`|=` 就地族／嵌套 boolop／
  语料第二批 22 条）＋ docs 收口
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：
  **通过 324 ＋ 参照口径 17 ＝ 341 ÷ 628 ⇒ 54.3%**（区间 340–342；阈值 67% ＝ 421，还差 80）
  - 战役累计：208 → 296 → 306 → **341**（本会话 +45）；进度指标：`Lib/` **411** 文件 ⇒ 324 能 import
  - 判据② **推进中**：语料 **182 → 212 → 234**（conformance 234 通过 · 0 已知差异 · 0 新差异 ✓）
    ⇒ **尚未能裁定**（P3-27 未完，如实 ✓）
- **上限诊断**（overlay 口径，round 1 实测 312 ⇒ round 2 后待复测）：大族剩 `_socket`（41）／
  CJK `_codecs_*`（24）／`sys._getframe`（5）／`os.getcwd`／`_signal` 等 C 模块面。

## 下一手（round 3，按 P3-31 解析器缺口批）

弹药已全部钉好（`target/recon/parser_batch_recon.md`；清单全文＝台账卷 05 的 task-7 表 ✓）：
1. **目标/模式族**：嵌套元组目标（`difflib.py:1835` ⇒ **difflib 直接通**）＋ `with … as (a,b)`
   （`ElementTree.py:727` ⇒ **直接通**）＋推导式模式目标＋下标赋值基底是调用。
2. **表达式族**：`.05` 前导点浮点（`subprocess.py:2077`）＋元组里的 `*`（`tomllib/_parser.py`）。
3. **语句族**：行内 if 分号表＋次行 else（`urllib/request.py:1805`）＋带注解赋值非名字目标
   ＋制表符缩进（gettext 是 **tab 注释** ⇒ **gettext 直接通**）。
4. **实参族**：`*` 后置关键字实参（`pickle.py:615`）。
   修完同步能 import 的进 `Lib/`（**先重建 conformance 二进制**再量 ✓）；unittest／subprocess
   修完统一撞 `_signal`（C 模块，另立支）。
- 挂账真 bug（均已配 10 行复现，见台账卷 05）：`global X`＋函数内 `import X` ⇒ UnboundLocalError
  （受害者 `Counter.most_common`）；`warnings.catch_warnings(record=True)` StackUnderflow；
  `ChainMap.keys()` 静默空；`KeyError` 消息少 repr；`functools.wraps` 不搬 `__name__`。
- 既有小坑：`tools/sync_lib.py --sync` 有 11 条无 `.py` 的陈旧 SLICE 条目会让 `--sync` 退出（待修）。

## 团队模式（本会话的工作方式，建议沿用）

- **Lead 不写代码**：拆任务上板（`team_task_create`，写死写域）→ 队员执行（认领→落地→汇报）→
  Lead 审查 diff、跑终审闸门（slowcheck 十项）、统一提交（`git commit -F - -- <显式路径>`）与推送。
- **队员用 `ds-` 前缀命名**（⇒ deepseek 路由，实测 ✓）。**pyawa-core 同时至多两名写手且必须
  不同子目录**（本会话 round 2 实测：`compile/emitter.rs`＋`executor/protocol.rs` 并行成功 ✓，
  口径＝互不碰文件＋编辑保持树可编译＋对方文件的编译错等 60s 重试）；非 Rust 面（语料／工具）可并行。
- **侦察先行**：每轮开工先让非 Rust 队员把缺口钉到最小复现（round 2 的 multiprocessing 26 格
  就是侦察改写成 10 行 boolop 复现的 ✓），Rust 写手只带弹药上。
- **队员上下文会耗尽**：配方写进本页／台账后换新队员接棒；**注意成员上限 8**。
- **同墙 3 轮上限**：修不通就停手立据升级；**红灯不提交**；解析器／编译器改动**不与语义分离落地**
  （task-27 的教训：只修 panic 不修「合法用例编译成功」＝半成品，当场补完 ✓）。

## 本战役的关键口径（新会话必读）

- **分子测量**：`lib_import_ratio.py` 跑 **conformance 测试二进制** ⇒ 改 core/stdlib 后必须先
  `tools/cargo.sh test -p pyawa-abi --test conformance --no-run` **再量**（踩过两次 ✓）。
- **`__pyawa__` 缓存坑**：按**脚本目录**复用旧编译产物 ⇒ 量之前 `rm -rf Lib/__pyawa__`（＋
  `target/**/__pyawa__`）✓。**并发测量无效**：`Lib/` 正被同步进程增删时读的数不能作数
  （round 2 实测：377→471→413 波动 ⇒ 树稳定后复核 ✓）。
- **上限诊断 overlay 口径**：`--ceiling` 的 `sys.path[0]` 插上游原版会遮住本仓适配版
  （`Lib/enum.py` ⇒ 全红假象 ✗）⇒ 用 overlay（上游全量 ⊕ 本仓 `Lib/` 覆盖）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **收账两道必检**：逐个导入＋无关脚本 `startup ok`；只同步**能 import** 的文件进 `Lib/`。
- **conformance 会补末尾换行** ⇒ corpus 测不到「源末无换行」路径；这类回归必须 Rust 侧直测
  （范本 `crates/pyawa-core/tests/eof_inline_suite.rs` ✓）。**字面量会被常量折叠** ⇒ boolop 一族的
  回归必须用名字（范本 `boolop_nest.rs` ✓）。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` 同轮提交 ✓；闸门**只看 exit code** ✓；
- 判据① **5 轮不动**即报停滞 ✓；判据① 单次读数 ±1 ⇒ 报"区间" ✓；
  `slowcheck` 十项全绿（含 stability 连跑口径 MS-25）才许提交 ✓；
- 新增 `P*-` 条目号前先重算当前最大号（墓碑算定义，`CX-19`；现最大 **P3-31** ✓）。
