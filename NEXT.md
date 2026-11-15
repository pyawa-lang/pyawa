# NEXT（会话 13 · alpha 发行文档＋AB-81 已落地；ABI 冻结随稳定版）

> **状态（2026-11-15）**：`v0.1.0-alpha.1` 的发行说明/变更日志/冻结口径已入库（`3de520d5`，
> 已推送）；`IM-15 已落地、pa_exec_file 属产品边界`的更正（`3cdce6a6`）；本笔按复核清掉 6 处
> 文档不一致：`AB-81` 移回规格 AB 列表并澄清与 SemVer 的关系；`DESIGN §13` 的 22 正文/计数
> 改为已决 0 未决；`PLAN §12.4` 去掉“等 IM-15”；`CHANGELOG` 协议数改 **75**（以
> `PROTOCOL_CASES` 为准）；`REFERENCE-HOST.md` 更新为 A2 的 C 侧 clock vtable／七项／
> `paL_*` 由组合根提供；`PLAN §12.18` B1/B2 ✓、B3 待 tag。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`3cdce6a6`（本轮前；docs 提交见 `git log`）· **工作区**：干净后提交
- **下一条命令**：`python3 tests/ci/check.py`（纯 docs；实现轮才跑 slowcheck 十二项）
- **ABI 状态**：**1.14**，**alpha 期未冻结**（`AB-81`：冻结随首个稳定发行版；alpha 破坏须在
  `docs/RELEASE.md` 逐条列）；指令集 **2**；两个公开头 `pa.h`＋`pa_capabilities.h`。
- **已完成（可复核）**：`docs/RELEASE.md`／`CHANGELOG.md`；`AB-81`；`REQUIREMENTS` 的
  「ABI 冻结时机」；`DESIGN §13-22` 已决；`REFERENCE-HOST.md` 与 `t_ref_host.py` 七项一致
  （含 `clock=ok`）；`CHANGELOG` 75 条与 `PROTOCOL_CASES` 一致。
- **B 组剩余**：**B3**——稳定发行版时打 tag（如 `abi-1.14`/发行 tag）＋冻结生效日期写进规格；
  当前 `RELEASE.md` 仍写“打 tag 前可改”。
- **C 组（如实未提供，不阻塞 alpha）**：`pa_exec_file`（非 `IM-15`：最小产物无能力提供者；
  电池版未实现该函数）；`paL_error` 变参；`paL_dostring`/`paL_dofile`；`paL_where`/`paL_requiref`；
  `TS-45` 大整数 ABI 复核；`pa.h`/`lib.rs` 里残留“能力层尚未接线”注释随单修。
- **其他（承接）**：第二平台未验证；`Lib/test` 通过率未作承诺；官方测试面/性能主线；
  push/合并需授权（本地领先，普通 push 即可）。
- **口径**：编号最大 `TS-57`／`BC-67`／`AB-81`／`CP-39`／`CX-30`／`P3-42`／`T-AB-20`；
  `pa_` 核心函数 **81**；`PROTOCOL_CASES` **75** 条。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；纯 docs 过 `check.py`；
  动代码须 slowcheck 十二项全绿才提交。
