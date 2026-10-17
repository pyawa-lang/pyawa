# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-17 ✓）：本会话（Lead＋队员团队模式）已收 **round 1–3**；
> 逐轮复盘见 `docs/rounds/03-m5-progress-05.md` ✓。

## 现在

- **HEAD**：`9d5b40b`（`dev`，**已推送** origin ✓）—— round 6（`_socket` 最小面 ⇒ import socket 通）＋ docs 收口
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：
  **通过 357 ＋ 参照口径 17 ＝ 374 ÷ 628 ⇒ 59.6%**（区间 373–375；阈值 67% ＝ 421，还差 47）
  - 战役累计：208 → 296 → 306 → 341 → 358 → 361 → 372 → **374**（本会话 +78）；进度指标：`Lib/` **440** 文件 ⇒ 358 能 import（81.4%）
  - 判据② **推进中**：语料 **234**（conformance 234 通过 · 0 已知差异 · 0 新差异 ✓）⇒ **尚未能裁定**
    （P3-27 未完，如实 ✓）

## 下一手（round 7：**enum 决策已升级给用户**（影响 100+ 格）；并行可收 select／CJK）

round 3 后 overlay 复测（`target/recon/ceiling_overlay2.txt`）：**341/628**；前三大族＝`_signal`（52）／`os.getcwd`（49，round 4 已收首格 sysconfig 族）／`_socket`（42）。round 5（task-11，进行中）＝内建 `dir`（6）＋`_operator` 别名（2）＋genexp 自由变量 panic（decimal 一族）＋`__classdict__` cellvars（calendar/mailbox）。之后按依赖序：
1. **`posix.getcwd`／`posix.readlink`**（sysconfig 族直通 ✅ 实测）：`readlink` 能力域槽已存在只是填
   `None`（`pyawa-capabilities/src/fs.rs:117` ⇒ `instance/fs.rs` + `runtime/fs_posix.rs` + 
   `posix_module.rs` 三处照 `fs_stat` 范本）；`getcwd` 无槽 ⇒ 最小路径 `std::env::current_dir()`
   （不在 T-CX-4 禁止面，`posix_module.rs` 有 `std::env::vars` 先例）。**语料要比它 ⇒ conformance
   harness 的 fs provider 也要补 readlink**（否则被测侧恒 OSError＝假阳性）。收 sysconfig＋
   test.support（`os_helper.py:115` 的模块级 `os.getcwd()`）一族。
2. **enum 决策（用户没拍板前先问）**：`signal.py` 的硬前置是 `Lib/enum.py` 假货缺 `_convert_`
   （上游真 enum.py 现在也跑不动：`TypeError: 'NoneType' object is not iterable`）⇒ 二选一：
   假货补 `_convert_`（要 `setattr` 出 `Signals`/`Handlers`）／先修 VM 换真 enum。
3. **`_signal` 最小面**：导入期必需恰好 `signal`＋`getsignal`（带 `__doc__`）；其余 4 个由
   `from _signal import *` 决定 ⇒ 落 Rust 侧 `_signal` 模块。
4. 之后：内建 `dir`（pickle 族）→ `sys._getframe` 非 0 depth（urllib.request）→ CJK `_codecs_*`
   （24）→ socket（P3-29，41，用户已拍板立项）。
- 挂账真 bug（均已配 10 行复现，见台账卷 05）：`global X`＋函数内 `import X` ⇒ UnboundLocalError
  （受害者 `Counter.most_common`）；`warnings.catch_warnings(record=True)` StackUnderflow；
  `ChainMap.keys()` 静默空；`KeyError` 消息少 repr；`functools.wraps` 不搬 `__name__`；PEP 604
  union 注解 dataclass 的 `'function' object has no attribute '__builtins__'`。
- 既有发射偏差（无夹具覆盖，parse_batch_shapes 头注已记）：`with …: pass` 少一条 `NOP`；方法调用
  `LOAD_ATTR` 的 NULL 位形态；常量条件 `if True:` 不折叠。

## 团队模式（本会话的工作方式，建议沿用）

- **Lead 不写代码**：拆任务上板（`team_task_create`，写死写域）→ 队员执行（认领→落地→汇报）→
  Lead 审查 diff、跑终审闸门（slowcheck 十项）、统一提交（`git commit -F - -- <显式路径>`）与推送。
- **队员用 `ds-` 前缀命名**（⇒ deepseek 路由，实测 ✓）。**pyawa-core 同时至多两名写手且必须
  不同子目录**（round 2 实测：`compile/emitter.rs`＋`executor/protocol.rs` 并行成功 ✓）；非 Rust 面
  （语料／工具）可并行。
- **侦察先行**：每轮开工先让非 Rust 队员把缺口钉到最小复现（round 2 的 multiprocessing 26 格被
  改写成 10 行 boolop 复现 ✓；round 4 的 C 模块面已由 task-9 钉好 ✓）。
- **队员上下文会耗尽**：配方写进本页／台账后换新队员接棒；**注意成员上限 8**（现 7：ds-core-cells／
  ds-abi-boundary／ds-corpus-libtest／ds-inplace-ops／ds-parse-batch＋lead）。
- **同墙 3 轮上限**；**红灯不提交**；解析器／编译器改动**不与语义分离落地**；**写域要动允许面外文件
  先报 Lead**（round 3 的 task-8 事后补报，放行但记了账 ✓）。

## 本战役的关键口径（新会话必读）

- **分子测量**：`lib_import_ratio.py` 跑 **conformance 测试二进制** ⇒ 改 core/stdlib 后必须先
  `tools/cargo.sh test -p pyawa-abi --test conformance --no-run` **再量**（踩过两次 ✓）。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）✓。**并发测量
  无效**：`Lib/` 被同步进程增删时读的数不作数（round 2 实测 ⇒ 树稳定后复核 ✓）。
- **上限诊断 overlay 口径**：`--ceiling` 的 `sys.path[0]` 插上游原版会遮住本仓适配版 ⇒ 用 overlay
  （上游全量 ⊕ 本仓 `Lib/` 覆盖）。round 3 后复测脚本已备好：`target/recon/ceiling_rerun.sh`
  （先查工作区干净再跑 ✓）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **收账两道必检**：逐个导入＋无关脚本 `startup ok`；只同步**能 import** 的文件进 `Lib/`。
- **conformance 会补末尾换行** ⇒ 「源末无换行」回归必须 Rust 侧直测（范本 `eof_inline_suite.rs` ✓）。
  **字面量会被常量折叠** ⇒ boolop 回归必须用名字（范本 `boolop_nest.rs` ✓）。
- **语料要的新内建面，conformance harness 的 provider 也要有**（否则被测侧恒错＝假阳性——task-9
  对 `readlink` 的提醒 ✓）。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` 同轮提交 ✓；闸门**只看 exit code** ✓；
- 判据① **5 轮不动**即报停滞 ✓；判据① 单次读数 ±1 ⇒ 报"区间" ✓；
  `slowcheck` 十项全绿（含 stability 连跑口径 MS-25）才许提交 ✓；
- 新增 `P*-` 条目号前先重算当前最大号（墓碑算定义，`CX-19`；现最大 **P3-31**（已解除留墓碑）✓）。
