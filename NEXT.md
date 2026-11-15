# NEXT（会话 13 · §12.19 traceback 数据面已立；ABI 1.14 alpha 未冻结）

> **本轮（2026-11-15，docs＋验证）**：按用户授权完成 `§12.19`：
> 1. **规格**：`SPEC-object-model` 新增 **`OM-42`**（§12.1 traceback 数据面：`tb_frame`／
>    `tb_lineno`／`tb_lasti`／`tb_next`、帧保活、`AB-74` 边界、`SkipTest` 偏差 `DIV-6`）＋
>    **`T-OM-11`**；链语义**只引用 `BC-60`／`T-BC-22`**，不重述。
> 2. **实现核对**：core 的 `new_traceback`／`attach_traceback` 已存在，本轮按 `OM-42` 核对并
>    用测试钉住（`pyawa-runtime/tests/exception_traceback_frames.rs` 1 passed、
>    `pyawa-core/tests/exceptions.rs` 18 passed）；`paL_where(level)` 的调用语义在
>    `SPEC-c-abi` §15.4 精确化（`level<0`/`out NULL` ⇒ INVALID；无帧/code/链不足 ⇒
>    NOTIMPLEMENTED；行号未知 ⇒ 0），`helpers_wiring.rs` 加 level=1／链不足用例（5 passed）。
> 3. **错引**：`pa.h`＋`helpers.rs` 的 5 处 `OM-28` 已改 `OM-42`／`BC-60`。
> 4. C6 复核并入 `§12.18 C6` ✓。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`946ca6cd`（本轮前；提交见 `git log`）· **工作区**：干净后提交
- **下一条命令**：`tools/slowcheck.sh --release`（十二项，含 traceback/helpers 测试）
- **本轮验收**：`check.py` 20/20；`cargo check --workspace --all-targets` 0 警告；
  `PYAWA_DANGLING=1` 全量测试；`slowcheck --release`；`t_ref_host`／`check_capability_layout`。
- **如实边界**：`SkipTest.__traceback__ = None`（`DIV-6`）；`tb_lasti` 数值坐标与 CPython
  不保证一致；`traceback` 模块完整 Python 面（`format_exc` 一族）不在本单。
- **其他待办（承接）**：B3 稳定发行 tag/日期；ABI 发布决定；第二平台；`Lib/test` 通过率；
  push/合并（普通 push 即可）。
- **口径**：`PA_ABI_VERSION`＝**1.14**、指令集 **2**；两个公开头；`pa_` 核心函数 **81**；
  `PROTOCOL_CASES` **75**；编号最大 `TS-57`／`BC-67`／`AB-81`／`CP-39`／`CX-30`／`P3-42`／
  `T-AB-20`／`OM-42`／`T-OM-11`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；纯 docs 过 `check.py`；
  动代码须 slowcheck 十二项全绿。
