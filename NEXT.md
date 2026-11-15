# NEXT（会话 13 · §12.18 C 组已接线；ABI 1.14 alpha 未冻结）

> **本轮落地（2026-11-15，C 组）**：按用户授权「将 C 组接线」完成：
> - **C1 `pa_exec_file`**：经 `fs` 域读源→复用 `pa_exec_string`；电池版 `paL_newstate` 直接可用；
>   无 provider 的最小产物 ⇒ `PA_ERR_NOTIMPLEMENTED`（机器错误 ⇒ `PA_ERR_RUNTIME`）；过时注释已修。
> - **C2 `paL_errorf`**：C 头 `static inline`（`vsnprintf`→`paL_error`，不新增导出符号）；示例真编译使用。
> - **C3 `paL_dostring`／`paL_dofile`**：`pa_exec_string`／`pa_exec_file` ＋ `paL_execresult`；
>   `int mode`：0=python／1=pyawa（规格/头已写）。
> - **C4 `paL_where`**（当前帧/`f_back` 的 `code:line`；完整 traceback 仍未接）／
>   **`paL_requiref`**（`sys.modules` → `openf` → `import_through_bridge`；`glb` 写全局）。
> - **C6**：`TS-45` 复核关闭（`AB-62` 的 `pa_tointstring`/`pa_pushintstring` 覆盖全域；
>   `pa_tointeger` 只做 i64 投影）；C5 注释已在 A1 修。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`5a63c3c5`（本轮前；实现提交见 `git log`）· **工作区**：干净后提交
- **下一条命令**：`tools/slowcheck.sh --release`（十二项，含 C 组测试/探针）· 本轮已跑 **416s 全绿**
- **本轮验收**：`check.py` 20/20；`cargo check --workspace --all-targets` 0 警告；
  `PYAWA_DANGLING=1` 全量测试；`slowcheck --release`；`t_ref_host`／`check_capability_layout`；
  未 push、ABI 不发布。
- **新增测试**：`crates/pyawa-abi/tests/helpers_wiring.rs`（dostring/dofile/where/requiref）；
  `tests/ci/host_usability_probe.py`（电池版 exec_file/dofile/dostring＋最小产物 exec_file 边界）；
  `examples/reference_host.c` 使用 `paL_errorf`。
- **如实边界（C 组）**：`paL_where` 无完整 traceback（`OM-28`）；`paL_errorf` 无导出符号（仅头内联）；
  `pa_exec_file`/`paL_dofile` 在最小 ABI 产物需宿主注册 `fs`，否则 `NOTIMPLEMENTED`。
- **下一步（需授权）**：B3 稳定发行 tag/日期；ABI 发布决定；第二平台；`Lib/test` 通过率；
  push/合并（普通 push 即可）。
- **口径**：`PA_ABI_VERSION`＝**1.14**、指令集 **2**；两个公开头；`pa_` 核心函数 **81**；
  `PROTOCOL_CASES` **75**；编号最大 `TS-57`／`BC-67`／`AB-81`／`CP-39`／`CX-30`／`P3-42`／`T-AB-20`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；动代码须 slowcheck 十二项全绿。
