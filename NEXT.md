# NEXT（会话 13 · §12.13 全部完成；下一批待选项）

> **§12.13 #1（路线 A）已落地（2026-11-14）**：`pyawa-runtime` 的 C 产物（cdylib／staticlib）为
> **单一库**——实测 `nm -D libpyawa_runtime.so` 导出 **80 个 `pa_*`** ＋ `paL_newstate`／
> `paL_openlibs`（`[lib] crate-type = ["rlib","cdylib","staticlib"]`）。`paL_newstate` = 建实例＋
> `fs`／`clock`／`net` 真实能力＋builtins／`sys`／stdlib；`paL_openlibs` 由组合根实现（`pyawa-abi`
> 最小产物不再导出它）。验收 `T-AB-18`：`tests/ci/host_usability_probe.py`（**无 numpy**，slowcheck
> 步骤 5 与 `gate_chunk small` 均跑）：电池版 builtins/`import`/`print`、异常带消息、裸 `pa_create`
> 实例报错带 `pa_errmsg`、`paL_openlibs` 补装可用。§12.13 #2/#3/#4 见 `df2e0680`。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`df2e0680`（本轮前；本轮提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（十二项，含宿主可用性探针）
- **本轮验收**：`check.py` 20/20；`slowcheck --release` 十二项全绿；`PYAWA_DANGLING=1` 全量测试；
  `cargo check --workspace --all-targets` 0 警告；未 push、ABI 不发布。
- **如实边界（§12.13 #1）**：provider／vtable 按**实例泄漏到进程末**（C 便捷入口没有销毁回调可挂；
  要精确回收 ⇒ 宿主用 `pa_create`＋自注册能力）；`Lib/` 定位仍走 `env!(CARGO_MANIFEST_DIR)`
  （构建期路径；部署需带 `Lib/`——`site.py`／`IM-24` 后消）；组合根 cdylib 不导出 `paL_dostring`／
  `paL_dofile` 的补齐（仍属缺编译器／文件 I/O 的既有边界）。
- **下一批候选（按优先级，均需新授权）**：
  1. **ABI 发布**（用户决定；当前版本 **1.13**＝`pa_pushmodule`＋§12.13 语义修正）。
  2. `ABI #2` `pa_exec_file`（等 `IM-15` 文件 I/O 统一）与 `TS-45` 大整数 ABI 通道
     （`pa_tointeger` 越 i64 如实 `PA_ERR_NOTIMPLEMENTED`）。
  3. 官方测试面（`array.array` 元素存储等）／性能主线（dict 哈希化）／`CX-30` 白名单。
  4. 部署化：`paL_newstate` 的 provider 生命周期回调（需 ABI 新钩子）＋ `Lib/` 定位改运行时查
     （可复用 `IM-24`／`site.py` 落地）。
- **口径**：`PA_ABI_VERSION`＝**1.13**、指令集 **2**；编号现最大 `TS-57`／`BC-67`／`AB-79`／
  `CP-39`／`CX-30`／`P3-42`／`T-AB-18`；`pa_` 核心函数 **80**；C 产物＝`pyawa-runtime`
  cdylib/staticlib（单一库；`pyawa-abi` cdylib 为最小 ABI 面）。
- **历史**：§12.13 原提交经两次 amend（仅复跑方式两行）；`origin/dev` 在 `f7f041f0`，本地领先；
  push／合并需授权（普通 push，无需 force）。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；动代码须 slowcheck 全绿才提交。
