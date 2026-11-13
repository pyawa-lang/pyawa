# NEXT（会话 13 · ABI 收口完成）

> **本轮落地（2026-11-14）**：`PLAN-milestones.md` §12.4 的 ABI 收口收尾 —— **#4** `AB-70`
> `pa_tohandle`（宿主对象载荷读回）· **#8** `AB-71` `pa_set_program_filename` 进 `pa.h`／规格
> （影子导出清零）· **#3** `AB-72` `pa_call`／`pa_pcall` 的 `nresults` 精确语义（0/N，多截少补
> `None`）· **#5** §15.4 的 18 个 `paL_*` 签名补齐 · **#6** 栈契约记法跟随实现（`setfield` −1、
> `settable`／`rawset` −2）· **#7** `pa.h` 过时注释修正。**ABI 次版本 9→10**（追加式 ＋ 未发布前
> 的语义补齐）；**ABI 仍不发布**（用户 2026-11-13：等钩子族定稿）。`#1`／`#2` 维持合规现状。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`9a028e85`（本轮前；ABI 收口的三笔提交见 `git log`）
- **工作区**：干净 ✓（`git status --porcelain` 空）
- **下一条命令**：`tools/slowcheck.sh --release`
- **主线**：`PLAN` §9.4 —— **内存安全 ＋ 执行效率**已收口；其余**慢慢补** ✗。
- **ABI 剩余**：**#1** `AB-52` 的 `type_expr` 求值（等 `TS` 注解解析器；现"原样保留"合规）·
  **#2** `pa_exec_file`（等 `IM-15`；现如实 `PA_ERR_NOTIMPLEMENTED` 合规）· **发布**＝用户决定
  （`DESIGN` §13-22；发布前须确认钩子族是否还要加）。`T-CX-19`（`CX-29`）过渡清单为空，棘轮在。
- **本轮发现（既有、非本轮引入，已记档待修；判据即"给定脚本 exit 0／结果正确"）**：
  - **`StopIteration` 回收崩溃**：模块层先 catch 一次、再出现未捕获 `StopIteration` ⇒
    `帧操作失败：StackUnderflow`（确定性）。
  - **`asyncio.gather`／`wait_for` 红**：`Task` 读 `_Future__asyncio_awaited_by` 失败——`pyawa-core`
    缺**类体私有名改写**（`__x` → `_C__x`）。
  - **内建函数 `__module__` 硬编码 `"builtins"`**（`_csv.reader.__module__` 应为 `"_csv"`）。
  - **`_io` 的 `TextIOWrapper(newline='')` 不在孤立 `\r` 断行**（`test_csv` 2 错误的根因）。
  - **`inspect.getattr_static` 对类查找恒 `MISSING`**（`types.GetSetDescriptorType` 由
    `type(FunctionType.__code__)` 算出、本层把函数 `__code__` 暴露成原生函数 ⇒ 上游 inspect 的
    `_shadowed_dict` 判定失效；`AB-54` 的宿主函数自描述已绕开它）。
  - **`tests/ci/selftest.py` 基线在临时副本里不干净**（既有）：副本不含 `NEXT.md`／`target/conformance`
    且无 `.git` ⇒ `T-CX-14/15`／`M3-COVERAGE ④⑤` 恒红；需另修 harness（不影响 `check.py`）。
- **未修 bug ＋ 判据**：`iter_value` 丢异常（round 171 撤回）· traceback 帧保活 · dict 哈希化；
  详见 `docs/rounds/04-post-m6-01.md` 与 `PLAN` §9.2。
- **口径**：改编译器后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +`；
  判据① `tools/lib_import_ratio.py`；ABI `PA_ABI_VERSION`＝**1.10**、指令集 **2**；
  编号现最大 `AB-72`／`BC-65`／`TS-50`／`CP-39`／`CX-29`／`P3-42`。
- **闸门**：`&&` 只看 exit code；`git commit -F - -- <显式路径>`；slowcheck 十一项全绿才提交
