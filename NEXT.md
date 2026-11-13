# NEXT（会话 13 · §12 收口后 · ABI 打磨轮）

> **本轮落地（2026-11-14）**：① `pa_ty` → **`pa_type_expr`**（签名类型表达式句柄改名，发布前最便宜）；
> ② 签名**权威归注册信息**：`AB-29`／`AB-53`／`AB-54`／`TS-19`／`T-TS-6` 明确 `.pyi` 是**单向导出投影**、
> 不在依赖链（运行期与进程内检查器都不读它）；③ `AB-54` 运行期自描述落地——宿主函数的
> `__text_signature__` 由 `pa_sig` 投影（`inspect.signature` 可用，不依赖 `.pyi`）；
> ④ 新机检 **`T-CX-19`**（约束 **`CX-29`**）：ABI 导出 ⊆ `pa.h` ⊆ 规格 ＋ 总数 ≤ 120。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`18d313b2`（本轮前；ABI 打磨的三笔提交见 `git log`）
- **工作区**：干净 ✓（`git status --porcelain` 空）
- **下一条命令**：`tools/slowcheck.sh --release`（本轮已全绿；下一轮先复跑确认基线未漂）
- **主线**：`PLAN` §9.4 —— **内存安全 ＋ 执行效率**已收口；其余**慢慢补** ✗。**ABI 仍不发布**（用户决定）。
- **ABI 收口待办**（`PLAN` §12.4）：**#3** `pa_call` 多返回值形状、**#6** 栈契约记法、**#8** 影子导出
  （`pa_set_program_filename`，现列在 `T-CX-19` 的过渡清单里）三者**待用户拍板**；**#4** `pa_tohandle`
  与 **#5** §15.4 签名、**#7** `pa.h` 注释未开工；**#1/#2** 等 `TS` 注解解析器／`IM-15`（现状合规）。
- **本轮发现（既有、非本轮引入，已记档待修；判据即"给定脚本 exit 0／结果正确"）**：
  - **`StopIteration` 回收崩溃**：模块层先 catch 一次、再出现未捕获 `StopIteration` ⇒
    `帧操作失败：StackUnderflow`（确定性；最小形态见 `docs/rounds/04-post-m6-01.md` 之外的复现记录）。
  - **`asyncio.gather`／`wait_for` 红**：`Task` 读 `_Future__asyncio_awaited_by` 失败——`pyawa-core` 缺
    **类体私有名改写**（`__x` → `_C__x`）。
  - **内建函数 `__module__` 硬编码 `"builtins"`**（`_csv.reader.__module__` 应为 `"_csv"`）。
  - **`_io` 的 `TextIOWrapper(newline='')` 不在孤立 `\r` 断行**（`test_csv` 2 错误的根因）。
  - **`inspect.getattr_static` 对类查找恒 `MISSING`**：`types.GetSetDescriptorType` 由
    `type(FunctionType.__code__)` 算出、本层把函数 `__code__` 暴露成原生函数 ⇒ 上游 inspect 的
    `_shadowed_dict` 判定失效（`T-CX-19` 之外的既有缺口；`AB-54` 的宿主函数自描述已绕开它）。
  - **`tests/ci/selftest.py` 基线在临时副本里不干净**（既有）：副本不含 `NEXT.md`／`target/conformance`
    且无 `.git` ⇒ `T-CX-14/15`／`M3-COVERAGE ④⑤` 恒红；需另修 harness（不影响 `check.py`）。
- **未修 bug ＋ 判据**：`iter_value` 丢异常（round 171 撤回）· traceback 帧保活 · dict 哈希化；
  详见 `docs/rounds/04-post-m6-01.md` 与 `PLAN` §9.2。
- **口径**：改编译器后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +`；
  判据① `tools/lib_import_ratio.py`；ABI `PA_ABI_VERSION`＝**1.9**（本轮无版本事件：改名发生在发布前）、
  指令集 **2**；编号现最大 `AB-69`／`BC-65`／`TS-50`／`CP-39`／`CX-29`／`P3-42`。
- **闸门**：`&&` 只看 exit code；`git commit -F - -- <显式路径>`；slowcheck 十一项全绿才提交
