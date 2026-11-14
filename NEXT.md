# NEXT（会话 13 · AB-80 `pa_writetobytes` 已落地；1.14）

> **本轮落地（2026-11-14，代码＋docs）**：按用户授权完成 `AB-80` 的规格与实现：
> `pa_writetobytes(st, idx, offset, p, len)`——**唯一接受恰好 `bytearray`**（`bytes`/`memoryview`/
> 子类/非字节/非法 idx 一律 `PA_ERR_INVALID`，带 `pa_errmsg`）；只写数据区、不扩容/不重分配；
> `offset.checked_add(len)` 溢出/越界拒绝且**不部分写**；`p==NULL && len>0` 拒绝；合法范围
> `len==0` ⇒ no-op `PA_OK`；不借出 VM 内存。`PA_ABI_MINOR` **13→14**；`pa.h` 原型/版本注同步；
> `SPEC-c-abi` §15.3 库存 **81**、§16 清零、版本 **1.14**；`PLAN §12.16` ✓。
> **验收（T-AB-20）**：`crates/pyawa-abi/tests/write_tobytes.rs` **4 passed**（成功写回＋guest 读到、
> 栈不变；越界/usize 溢出且不部分写；`bytes`/子类/非字节/非法 idx；NULL 指针/len==0 规则；
> 失败消息）；valgrind 3.26 `--error-exitcode=99` **0 errors**。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`2b1bde41`（本轮前；实现提交见 `git log`）· **工作区**：干净后提交
- **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **本轮验收**：`check.py` 20/20；`cargo check --workspace --all-targets` 0 警告；
  `PYAWA_DANGLING=1` 全量测试；`slowcheck --release` 十二项；未 push、ABI 不发布。
- **其他待办（承接）**：ABI 发布（用户决定；当前 **1.14**）· `pa_exec_file`（等 `IM-15`）·
  `TS-45` 大整数 ABI · 官方测试面/性能主线 · push/合并需授权。
- **口径**：`PA_ABI_VERSION`＝**1.14**、指令集 **2**；编号现最大 `TS-57`／`BC-67`／`AB-80`／
  `CP-39`／`CX-30`／`P3-42`／`T-AB-20`；`pa_` 核心函数 **81**。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；动代码须 slowcheck 全绿才提交。
