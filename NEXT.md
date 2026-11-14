# NEXT（会话 13 · §12.13 #2/#3/#4 已实现，1.13）

> **本轮落地（2026-11-14，代码＋docs）**：按用户授权实现 **§12.13 #2/#3/#4**（#1 路线 **A** 已拍板）：
> - **#2**：`pa_newtype` 的 `dealloc`／`traverse` 改收 `Option<fn>`（C 的 `NULL` niche 映射）
>   ⇒ NULL 当场 `PA_ERR_INVALID`＋`pa_errmsg`；`pa_newhandle` 的空消息路径同步补消息。
> - **#3**：`pa_getglobal` 缺失名 ⇒ `PA_ERR_INVALID` **且不压栈**（原"压 nil"作废）。
> - **#4**：新增 `AB-79` `pa_pushmodule(st, module)`（栈契约 +1；非法句柄 `PA_ERR_INVALID`）
>   ⇒ C 宿主可把模块对象上栈并 `pa_set_object_ops`（`AB-75` 模块态）。
> - `PA_ABI_MINOR` **12→13**，`pa.h` 同步；测试 `crates/pyawa-abi/tests/host_usability.rs`
>   3 passed（`T-AB-15/16/17`）。**历史**：§12.13 原提交经两次 amend（仅"复跑方式"两行），
>   `origin/dev` 仍 `f7f041f0`；普通 push 即可（无需 force）。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`3e1dfbff`（本轮前；本轮提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **#1（路线 A，待实现）**：C 宿主链 `pyawa-runtime` 的 cdylib／静态库获得 `paL_newstate`／
  `paL_openlibs`（电池版；`pyawa-abi` 裸实例保持语言骨架、失败带 `pa_errmsg`）；
  验收 `T-AB-18`：组合根实例 builtins／`import` 可用＋**仓库内不含 numpy 的最小探针**可复跑。
  落点：`pyawa-runtime` 导出面／`Cargo.toml` crate-type／`pa.h` 注释／`tests/ci/` 探针；
  实现前先定"单一产物"细节（ABI 符号能否从 runtime cdylib 再导出）。
- **已修（可复核）**：`pa_newtype` NULL hooks ⇒ `host_usability.rs`；`pa_getglobal` 缺失 ⇒
  同文件；`pa_pushmodule` ⇒ 同文件（`import pushmod; pushmod.answer == 42`）。
- **其他待办（承接）**：`ABI #2` `pa_exec_file`（等 `IM-15`）· ABI **发布**＝用户决定
  （`DESIGN` §13-22）· 官方测试面（`array.array`）· 性能主线（dict 哈希化）· `CX-30` 白名单 ·
  本地提交 push/合并需授权。
- **口径**：`PA_ABI_VERSION`＝**1.13**、指令集 **2**；编号现最大 `TS-57`／`BC-67`／`AB-79`／
  `CP-39`／`CX-30`／`P3-42`／`T-AB-18`；`pa_` 核心函数 **80**（§15.3 已含 `pa_pushmodule`）。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
