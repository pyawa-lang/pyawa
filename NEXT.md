# NEXT（会话 13 · 失败路径探针接闸）

> **本轮落地（2026-11-14，第四轮）**：把阶段 0 的 **失败路径 GC 收支探针接进闸门**——
> `tests/ci/failure_path_leaks.py`（11 条失败路径 × N=10000/20000 缩放判据）成为
> **`tools/slowcheck.sh` 第 12 项**（步骤内先 `cargo build` CLI，release 档 no-op；任何 RED ⇒ 闸门红）；
> `tools/gate_chunk.sh small` 同步纳入。计数口径「十一项 → **十二项**」已同步到 `AGENTS.md` 硬线、
> `agents-rules/round-rule.md`、`docs/REQUIREMENTS.md`、`docs/PLAN-milestones.md`（新增 §12.8）
> 与 `failure_path_leaks.py` 头注。**本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；
> 只写 状态／下一条命令／未修 bug＋判据；评审记录见 `git show`，不逐轮复盘。

- **HEAD**：`e09a4a37`（本轮前；本轮提交见 `git log`）
- **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（**十二项**）
- **闸门（本轮后）**：`check.py` 20/20（含 `T-CX-20`）；`selftest.py` 24 项断言；`slowcheck --release`
  **十二项全绿**（新增第 12 项实跑 11/11）；`cargo check --workspace --all-targets` 0 警告；
  `PYAWA_DANGLING=1` 全量测试绿；conformance 408×2；`bench_gate` 四档 ✓。
- **下一批建议**：①`CX-30` 白名单 C 类 18 处可逐条转显式（低价值，收尾项）②ABI 剩余：`#1`
  `type_expr` 求值（等 TS 注解解析器）、`#2` `pa_exec_file`（等 IM-15）、**发布**＝用户决定
  （`DESIGN` §13-22）③官方测试面（`array.array` 元素存储等）与性能主线（dict 哈希化）
  ④26 笔本地提交待定 push/合并（需授权）。
- **本轮改动边界**：闸门计数变化是**流程口径**（不是语义）；第 12 项在 release 档与 debug 档都
  先构建 CLI（debug 档耗时略增）；`gate_chunk small` 也跑该探针（分块命令与慢闸口径一致）。
- **已知未修（承接上轮，按如实登记）**：账本诊断近似（`*const Instance` 不变量、ptr 复用账）；
  `CX-30` 在 RAII 后属可审计风格规则；ABI `#1`／`#2` 前置未开工；`array.array` 元素存储、
  dict 哈希化等既有缺口见 `PLAN` §9.2 与 `NEXT` 历史记录。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；编号现最大
  `BC-67`／`AB-72`／`TS-50`／`CP-39`／`CX-30`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
