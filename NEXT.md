# NEXT（会话 13 · AB-80 `pa_writetobytes`：规格已钉死，实现待做）

> **本轮（2026-11-14，docs）**：按用户授权先把 `AB-80` 的规格/库存/状态钉死：
> `SPEC-c-abi` `AB-80` 明确——**唯一接受恰好 `bytearray`**（`bytes`/`memoryview`/子类/非字节/
> 非法 idx 一律 `PA_ERR_INVALID`）；只写数据区、不扩容；`offset.checked_add(len)` 溢出/越界拒绝、
> 不部分写；校验顺序 state→idx→类型→NULL/len→范围；合法 bytearray 上 `len==0`（`offset<=长度`）
> 为 `PA_OK` no-op；失败写 `pa_errmsg`；窗口 **`PA_ABI_MINOR` 13→14**。`§15.3` 库存改 **81**
> 并加 `pa_writetobytes` 行；`§16` 登记 AB-80 为唯一未实现项；`T-AB-20` 精确化（含 usize 溢出/
> 子类拒绝/消息/栈不变/valgrind）；`PLAN §12.16` 状态=规格已定、实现待做。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`0c926f90`（本轮前；docs 提交见 `git log`）· **工作区**：干净后提交
- **下一条命令**：实现 `pa_writetobytes` → `tools/slowcheck.sh --release`
- **实现判据（T-AB-20）**：`crates/pyawa-abi/tests/write_tobytes.rs`：
  ①恰好 `bytearray` 写回成功＋guest 读到新内容＋栈不变；②`offset+len` 越界/**usize 溢出** ⇒
  `PA_ERR_INVALID` 且不部分写；③`bytes`/`memoryview`/`bytearray` 子类/非字节/非法 idx ⇒
  `PA_ERR_INVALID`；④`p==NULL && len>0` 拒绝、合法 bytearray 上 `len==0`（`offset<=len`）⇒
  `PA_OK` no-op；⑤失败 `pa_errmsg` 非空；⑥尽量 valgrind 跑该测试（`--error-exitcode=99`）0 错误。
- **实现落点**：`crates/pyawa-abi/src/lib.rs`（新函数）＋`include/pa.h`（原型/版本注）＋
  `PA_ABI_MINOR=14`；`SPEC-c-abi` §15.3 行去掉"（待实现）"、§16 清零、版本头 1.14、
  `PLAN §12.16` 状态 ✓、`NEXT` 收口。
- **门禁**：`check.py` 20/20；`cargo check --workspace --all-targets` 0 警告；
  `PYAWA_DANGLING=1` 全量测试；`slowcheck --release` 十二项；未 push、ABI 不发布。
- **其他待办（承接）**：ABI 发布（用户决定；1.14 窗口）· `pa_exec_file`（等 `IM-15`）·
  `TS-45` 大整数 ABI · 官方测试面/性能主线 · push/合并需授权。
- **口径**：`PA_ABI_VERSION`＝**1.13**（AB-80 落地后 **1.14**）、指令集 **2**；编号现最大
  `TS-57`／`BC-67`／`AB-80`／`CP-39`／`CX-30`／`P3-42`／`T-AB-20`；`pa_` 核心函数 **81**（含
  AB-80，实现前如实标待实现）。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；动代码须 slowcheck 全绿才提交。
