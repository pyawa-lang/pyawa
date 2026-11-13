# NEXT（会话 13 · §12 需求单收口后）

> **本轮落地（2026-11-14）**：`PLAN` §12 需求单三项全部落地 ✓ —— 12.1 `x = yield`（编译器生成器判定）、
> 12.2 `_signal` 记账式最小面＋`asyncio` 解锁、12.3 `_csv`＋`Lib/csv.py`。判据与如实边界见 `PLAN` §12 各状态注。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`079b1685`（本轮前；§12 三项的提交见 `git log`）
- **工作区**：干净 ✓（`git status --porcelain` 空）
- **下一条命令**：`tools/slowcheck.sh --release`（本轮已全绿；下一轮先复跑确认基线未漂）
- **主线**：`PLAN` §9.4 —— **内存安全 ＋ 执行效率**已收口（效率线关闭 ✓）；其余**慢慢补** ✗。
- **本轮发现（既有、非本轮引入，已记档待另修；判据即"给定脚本 exit 0／结果正确"）**：
  - **`StopIteration` 回收崩溃**：`yield from` 耗尽后，模块层先 catch 一次 `StopIteration`、随后出现
    未捕获 `StopIteration` ⇒ `帧操作失败：StackUnderflow`（确定性）。最小形态：`def g():\n yield from [1]`
    ＋ 模块层 `while True: try: z.send(None) except StopIteration: break`，再另起生成器 `send` 到耗尽且不捕获。
  - **`asyncio.gather`／`wait_for` 红**：`Task` 读 `_Future__asyncio_awaited_by` 失败——`pyawa-core` 缺
    **类体私有名改写**（`__x` → `_C__x`）。判据：`asyncio.gather(coro())` 返回结果。
  - **内建函数 `__module__` 硬编码 `"builtins"`**：`test.support.check__all__` 认不出 `_csv` 的 7 个名字
    （`test_csv.test__all__` 1 失败）。判据：`_csv.reader.__module__ == "_csv"`。
  - **`_io` 的 `TextIOWrapper(newline='')` 不在孤立 `\r` 断行**（`test_csv` 2 错误根因；实测
    `['a,b\rc,d\r']` vs 参照两行）。判据：该行源切分与参照一致。
  - **编译期缺"yield 位置非法"检查**（既有）：函数内 class 体／推导式内部／annotation／PEP 695
    type alias 里的 `yield`，参照报 `SyntaxError`、本层接受（class 体在运行期 abort）。
    判据：上述四类在 `pyawa check` 出语法类诊断。
- **未修 bug ＋ 判据**：`iter_value` 丢异常（round 171 撤回，三种修法均致 test_dict 间歇失败）·
  traceback 帧保活 · dict 哈希化（test_dict 慢）… 详见 `docs/rounds/04-post-m6-01.md` 与 `PLAN` §9.2。
- **口径**：套件体检 `tools/libtests_survey.py --release`（报告级）；改编译器后清缓存
  `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋ `find target/upstream -name '*.pyac' -delete`；
  判据① `tools/lib_import_ratio.py`；ABI `PA_ABI_VERSION`＝**1.9**、指令集 **2**；
  编号现最大 `BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／`P3-42`。
- **闸门**：`&&` 只看 exit code；`git commit -F - -- <显式路径>`；slowcheck 十一项全绿才提交
