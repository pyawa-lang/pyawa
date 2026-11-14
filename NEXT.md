# NEXT（会话 13 · 引用泄漏结构性收口）

> **本轮落地（2026-11-14，第三轮）**：把「`Err(Raised)` 那份异常引用被静默丢掉」从**纪律**变成
> **类型上不可能**——
> **阶段 0**：① **B 引用账本**（`core/src/error_ledger.rs`，`PYAWA_GC_LOG=4`，每实例无全局态；
> `#[track_caller]` 点名生成点＋示例栈；恒等式 `generated == pending`、
> `generated == 丢弃＋移交＋隐式＋泄漏＋…`）② **D 失败路径探针集**（`tests/ci/failure_path_leaks.py`
> 11 探针；修前 **6/11 RED**）③ **C 静态规则**（`CX-30`／`T-CX-20`：禁止裸丢弃 `Result`＋白名单棘轮；
> 顺带修好 `selftest.py` 既有假红基线）。
> **阶段 1**：`CX-30` 白名单 **55→18**（A 类 33 处、B 类 4 处清空）；6 处已知泄漏全修（`int.rs`
> 探针／`set_new`／排序探针／惰性注解／终结器旧 pending／`truthiness`）＋4 处风险 `.ok()` ⇒
> D 探针 **11/11 PASS**（`f(*Plain())` N=20000：tb 净漏 20002⇒2、live 207295⇒7348）。
> **阶段 2 / A（RAII）**：`RaiseRef`（Drop 只还 `Err` 那份、保留 `pending` 槽；`discard()`／
> `into_pending()`／`release_keeping_pending()` 均幂等）＋`PendingRef`（`replace_pending` 交回的旧
> pending 自动释放）；`ExecError::Raised` 携带守卫（**删 `Clone`**），迁移 **83 构造／45 解构**
> 与 **13 个 pending 调用点**；新增 `tests/raise_guard.rs`（4 passed）。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`82257fc1`（本轮前；本轮代码与文档提交见 `git log`）
- **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`
- **闸门**：`check.py` 20/20（含 `T-CX-20`）；`selftest.py` 24 项断言全绿；`cargo check --workspace
  --all-targets` 0 警告；`PYAWA_DANGLING=1` 下 `cargo test --workspace --release` 全绿；conformance
  **408×2**（普通＋悬垂）绿；`failure_path_leaks.py` 11/11；`bench_gate` 四档 ✓；`slowcheck` 十一项全绿。
- **下一批建议**：①把 `tests/ci/failure_path_leaks.py` **接进 slowcheck** 作为固定子项（现在要手动跑；
  需改 `tools/slowcheck.sh` 的 11 项清单，属下一轮）②`CX-30` 白名单 C 类 18 处可逐条转显式（低价值）
  ③ABI 剩余：`#1` `type_expr` 求值（等 TS 注解解析器）、`#2` `pa_exec_file`（等 IM-15）、发布＝用户
  决定（`DESIGN` §13-22）④官方测试面（`array.array` 元素存储等）与性能主线（dict 哈希化）⑤24 笔
  本地提交待定 push/合并。
- **本轮边界（如实）**：账本是**诊断近似**（"对象存活＋连续两次 flush"判未消费；ptr 复用留
  `overwritten` 账）；`RaiseRef` 存 `*const Instance`（不变量：错误只在本实例执行栈内传播/消费，
  与改动前"裸指针＋实例存活"同契约）；**`CX-30` 在 A 之后不再是内存正确性的必需**，保留为可审计
  风格规则（防复制"交回裸指针"旧模式）；`discard_raised` 已由 `RaiseRef::discard` 取代（core 导出
  已移除，ABI/注释已同步）；ABI 面未动（`AB-21` 边界仍按状态码映射）。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；编号现最大
  `BC-67`／`AB-72`／`TS-50`／`CP-39`／`CX-30`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十一项全绿才提交。
