# NEXT（会话 13 · §12.13 宿主可用性：规格与编号已补）

> **本轮落地（2026-11-14，只动 docs）**：按用户授权「先补 §12.13 的规格与编号」补齐：
> **`SPEC-c-abi`**——`AB-36` 明确 NULL `dealloc`/`traverse` ⇒ `pa_newtype` **当场** `PA_ERR_INVALID`
> ＋`pa_errmsg`（`AB-22`/`AB-48`）；`§15.3` 定 `pa_getglobal` **缺失名 ⇒ `PA_ERR_INVALID`（不压栈）**；
> 新增 **`AB-79`** `int pa_pushmodule(pa_state*, pa_module)`（+1，模块句柄上栈，`AB-75` 模块态）；
> `§15.4` 注明 `paL_openlibs` 的**提供方＝组合根**（`pyawa-runtime` 产物）且占位失败必须带消息；
> `§13` 新增 **`T-AB-15`**（模块上栈）、**`T-AB-16`**（global 缺失语义）、**`T-AB-17`**（NULL hooks
> 拒绝）、**`T-AB-18`**（组合根可用性＋仓库内无 numpy 最小探针）；`§16` 登记 `AB-79` 未实现；
> 窗口 `PA_ABI_MINOR` **1.12→1.13**。**`PLAN §12.13`** 补编号/优先级/依赖/落点与“待用户拍板路线”
> （#1 架构：A 链 `pyawa-runtime` 产物（推荐）／B `pa_host` 加安装器／C 宿主自链 stdlib C-ABI）。
> **历史说明**：原 §12.13 提交经两次 `--amend`（`55025e42→…→3e1dfbff`），仅改“复跑方式”两行；
> `origin/dev` 仍在 `f7f041f0`，重写未外推（普通 push 即可，无需 force）。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`3e1dfbff`（本轮前；本轮 docs 提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（本轮纯 docs，`check.py` 20/20 即可提交；实现轮再跑）
- **实现顺序（§12.13）**：**#2／#3**（小修，互不依赖）→ **#4**（`AB-79`＋`pa.h`＋`host_module.rs`
  C 宿主路径，1.13）→ **#1**（待拍板：推荐 A；实现 `pyawa-runtime` 产物导出 `paL_newstate`/
  `paL_openlibs`，ABI 裸实例保持最小）。
- **#2 判据**：`pa_newtype(..., NULL, NULL, sig)` ⇒ `PA_ERR_INVALID`，`pa_errmsg` 非空；
  合法注册零回归（`T-AB-17`）。
- **#3 判据**：`pa_getglobal("__不存在__")` ⇒ `PA_ERR_INVALID` 且**不压栈**；存在名照旧 `PA_OK`
  （`T-AB-16`；核对既有测试无“缺失名依赖 nil”）。
- **#4 判据**：`pa_pushmodule(st, mod)`（+1）后再 `pa_set_object_ops` 可为模块挂 `getattr_fn`；
  C/ctypes 宿主能取未枚举的 `np.<x>`；非法句柄 `PA_ERR_INVALID`（`T-AB-15`）。
- **#1 判据**：组合根产物实例上 builtins/`import` 可用、失败带 `pa_errmsg`；仓库内**不含 numpy**
  的最小探针可复跑（真 numpy 桥只作宿主侧压测、不入库）（`T-AB-18`）。
- **ABI 面**：`AB-79` 追加式 → `PA_ABI_MINOR` **1.13**；`pa_getglobal` 语义变更与 `AB-36` 拒绝
  同属该未发布窗口。ABI **不发布**（用户 2026-11-13 口径）。
- **其他待办（承接）**：`ABI #2` `pa_exec_file`（等 `IM-15`）· ABI 发布＝用户决定 · 官方测试面
  （`array.array`）· 性能主线（dict 哈希化）· `CX-30` 白名单 · push/合并需授权。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；实现轮须 `slowcheck` 十二项全绿。
