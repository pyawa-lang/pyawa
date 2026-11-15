# NEXT（会话 13 · §12.19 traceback 规格缺口已校正；待立规格→实现）

> **本轮（2026-11-15，docs）**：按用户授权修补 `§12.19`：① 承认 `BC-60`／`T-BC-22` 已有
> **可观察链语义**（`__context__`／`__cause__`／`__traceback__` 追加等），缺口改述为
> **对象模型数据面＋`paL_where` 完整链**，禁止另造第二套链语义；② 取号依据改为
> `SPEC-INDEX.md §2`（本文件前缀内连续、`CX-19`/`T-CX-10`）；③ 验收改为可执行 grep，
> 并列出当前 5 处代码残留＋1 处旧 NEXT 句；④ C6 复核并入 §12.18 C6，不重复列。
> `SPEC-c-abi` §15.4／`RELEASE.md` 的 `paL_where` 行同步引用 `BC-60`／`T-BC-22`／§12.19。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`946ca6cd`（本轮前；docs 提交见 `git log`）· **工作区**：干净后提交
- **下一条命令**：立 §12.19 item 1 的 traceback 规格（取下一个连续 `OM-` 号＋`T-OM`），
  再按规格实现 item 2；随后做 item 3 错引清理。
- **§12.19 现状**：item 1（对象模型数据面）**待做**；item 2（`paL_where` 完整链＋
  `__traceback__` 最小面）**待做**；item 3（错引）**待做**；item 4 已并入 C6 ✓。
- **错引残留（item 3 的可执行判据）**：
  `grep -rn 'OM-28' crates/pyawa-abi/include crates/pyawa-abi/src docs/RELEASE.md docs/SPEC-c-abi.md NEXT.md | grep -i traceback`
  → 当前命中：`pa.h:456`、`helpers.rs:9/378/401/407`（共 5 处代码）；修后应为空。
- **已有可复核**：`BC-60`（`SPEC-bytecode`）＋`T-BC-22` 链语义；`paL_where` 当前实现
  `level=0`／`f_back` 链的 `code:line`（`helpers_wiring` 覆盖）；`paL_where(0)` 无帧 ⇒
  `NOTIMPLEMENTED`（如实）。
- **不在本单**：`traceback` 模块完整 Python 面（`format_exc` 一族）。
- **口径**：`PA_ABI_VERSION`＝**1.14**、指令集 **2**；两个公开头；`pa_` 核心函数 **81**；
  `PROTOCOL_CASES` **75**；编号最大 `TS-57`／`BC-67`／`AB-81`／`CP-39`／`CX-30`／`P3-42`／`T-AB-20`
  （traceback 新号待 item 1 分配）。
- **其他待办（承接）**：B3 稳定发行 tag/日期；ABI 发布决定；第二平台；`Lib/test` 通过率；
  push/合并（普通 push 即可）。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；纯 docs 过 `check.py`；
  动代码须 slowcheck 十二项全绿。
