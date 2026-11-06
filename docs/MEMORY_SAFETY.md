# 内存安全专项（验收口径与清零路线）

> **一处真相**：本文件定义"内存安全 bug 清零"的**口径**与**当前清单／基线**；逐轮的实测证据写进
> `docs/rounds/04-post-m6-01.md`（随代码提交）。比例与数字**不写死**在别处，漂了就更新这里。

## 1. 口径：什么算"内存安全 bug"

一条**内存安全 bug** ＝ 下列任一现象在**官方套件或语料**里出现（与语义错误、性能超时分开计）：

| 类别 | 现象（可机检的签名） | 检测方式 |
|---|---|---|
| UAF／悬垂读 | `[悬垂] … 不在活表`、`对已释放对象 incref/decref` | `PYAWA_DANGLING=1` |
| 堆损坏 | `tcache_thread_shutdown(): unaligned tcache chunk`、`malloc():`、`double free`、`corrupted` | 同上（glibc 自检）＋未来 ASan／valgrind |
| 崩溃 | `SIGSEGV`／`SIGABRT`（非断言、非 RecursionError） | 退出信号 |
| 释放点错乱 | `PYAWA_ZOMBIE_TRACE=1` 报出的"谁会再放一次" | 诊断模式 |

**不算**内存安全 bug（但要另行记录）：内存泄漏/增长（按 §4 的基线考核）、语义差异（conformance 管）、
超时（性能面）、`NotImplementedError`（未接线面）。

## 2. 验收：清零的三条硬门

1. **悬垂全量**：`python3 tools/memdangle_survey.py`（`PYAWA_DANGLING=1` 跑全部 397 个顶层测试）⇒
   `memory` 类计数为 **0**、`timeout` 不计入本门但需记录；
2. **检测器**：同一全量套件在 **ASan 或 valgrind** 下零 memory error（工具链先落地，见 §5）；
3. **所有权断言**：debug build 默认开启"按指针记 incref/decref"的断言（`PYAWA_OWNERSHIP_CHECK=1`
   或 debug 默认），`ownership_shapes`＋套件抽样下零不平衡。

## 3. 当前清单（按根因族，证据在 round 台账）

### A 族：调用/返回的所有权模型不统一（最核心，是 `test_weakref` 超时的根因）

> **round 90 新事实（改变顺序）**：把 `frame_clear` 改成「拥有并释放形参槽」后，实参泄漏 **+100 ⇒ 0**、
> conformance 双模式全绿，但 **`__del__` 帧的释放晚于 `python_level_finalize` 返回** ⇒ 复活判定被
> 延迟 decref 误判 ⇒ `__del__` 无限重复（最小脚本 6.5M 次后超时）⇒ **实验撤回**。
> **新顺序**：① 先查清 `__del__` 帧为何被多持一份、释放被推迟（候选：`f_back`／`pending_exception`／
> `traceback.tb_frame`／异常状态栈）→ ② 再统一形参所有权 → ③ 最后删补偿性释放（`call.rs` 的
> `__init__` 两处）。诊断开关：`PYAWA_DEL_TRACE=1`（终结前后计数／类型）。
- 位置实参泄漏：`f(x)`×100 ⇒ refcount **+100**（参照 0）；
- `dispatch_raise` 的异常引用**双放**（conformance 的 `pyawa_side_runner` 可稳定复现 `tcache` SIGABRT；
  `PYAWA_ZOMBIE_TRACE` 显示被双放的是一枚 `str`）；
- 三种"只改 Python 分支"的修法全部**证伪**（round 83/84）⇒ 必须先做「按指针记账＋逐调用点断言」，
  再按 `CALL`/`CALL_KW`/`CALL_FUNCTION_EX` → `call_value` → 内部直接 `call_callable` **三批迁移**。
- 估计：**3–5 轮**。

### B 族：生命周期/重入
- `test_calendar`：`PYAWA_DANGLING=1` 下 `对已释放对象 incref`（`TextIOWrapper`/stdout 捕获场景）—— 1–2 轮；
- `test_statistics`：named-import + doctest 收集路径 **segfault**（direct 跑不崩）—— 1–2 轮；
- 终结器＋`gc.collect()` 重入（`ownership_shapes` EVIL 用例曾 SIGSEGV）。

### C 族：泄漏（不是 UAF，但与内存增长/超时相关）
- 被吞异常/回溯/帧泄漏、生成器形参归属（补丁验证有效但单独落地曾使 `ok 38⇒37`，等 A 族一起落）、
  ABI 未捕获异常（round 81 已修）。估计：**1–2 轮**。

## 3b. 基线（首测，round 88）

工具：`python3 tools/memdangle_survey.py --timeout 90 --jobs 8`（release CLI，`PYAWA_DANGLING=1`
＋`PYAWA_ZOMBIE_TRACE=1`，全部 397 个顶层测试）。

| 日期／提交 | memory 类 | timeout | ok | runtime | unimplemented |
|---|---|---|---|---|---|
| round 88 首测（修 `filter` 前） | **5**（`test_itertools`／`test_platform`／`test_random`／`test_urlparse`／`test_webbrowser`） | 3 | 35 | 271 | 83 |
| round 88 修 `filter`＋`getrandbits` 后 | **1**（只剩 `test_itertools.test_issue30347_2` 的 groupby 可重入 ✗） | 3 | 35 | 273 | 85 |
| round 89 修 `groupby` 键双持有后 | **0** ✅（硬门①首次达成） | 3 | 35 | 273 | 86 |

> memory 类判定与"清零"口径见 §1／§2；**只统计签名命中，不把语义失败算进来**。

## 4. 泄漏基线（不是"零泄漏"）

固定脚本（`target/recon/p341slow/*.py` 与套件抽样）记录 `gc` 的 live 峰值与 RSS；新提交不得让基线**回退**
超过阈值（阈值随首测确定，写进本表）。

## 5. 工具链现状与待办

- 已有：`PYAWA_DANGLING=1`（悬垂检测）、`PYAWA_ZOMBIE_TRACE=1`（最早释放点）、`tools/memdangle_survey.py`
  （本文件 §2 第 1 条的可执行基线）。
- **缺**：本机**没有 valgrind**、rust 工具链**只有 stable**（无 `-Zsanitizer`）⇒ ASan/valgrind 需先装工具链
  或换 CI 环境；落地前 §2 第 2 条只能标记为"未测"。

## 6. 清零路线（按轮次，不给日历日期）

1. 第 1 轮：本文件＋`memdangle_survey` 基线（**本轮**）；
2. 2–5 轮：A 族三批迁移（每批：`ownership_shapes`＋conformance＋`DANGLING` conformance 红即回退）；
3. 6–7 轮：B 族 `test_calendar` UAF＋`test_statistics` segfault（先最小复现再修）；
4. 8–9 轮：C 族收尾＋把"悬垂全量 memory=0"纳入闸门候选；
5. 10–12 轮（缓冲）：工具链（ASan/valgrind）落地并把 §2 第 2 条转为阻断。
- **估计**：乐观 6–8 轮，保守 12–15 轮；按当前每轮强度折算约 **3–6 周**（不给承诺日期）。
