//! **Pyawa LSP 服务**（PLAN §10 LSP 线②：`publishDiagnostics` ＋ `hover`）。
//!
//! 口径（一处真相＝`crates/pyawa-lsp/README.md` 的"范围与如实边界"＋ `PLAN` §10）：
//! * 传输：stdio，`Content-Length` 帧的 JSON-RPC 2.0（**手写实现**，本 crate **零第三方依赖** ✓）；
//! * 同步：`textDocument/didOpen`／`didChange`（**全量**）／`didClose`（`textDocumentSync.change = 1`）；
//! * 诊断：`pyawa_core::compile::check_source` 的批量诊断投影成 `publishDiagnostics`；
//!   `.py` 只语法类（`MS-19`）／`.pyawa` 语法＋类型；
//! * `hover`：对**已标注的名字**显示声明类型 markdown；找不到给 `null`（v1 如实边界 ✗）；
//! * 只编译不执行：本 crate **不建 VM 实例、不跑字节码** ✓。
//!
//! 依赖隔离：本 crate 在 `CX-27` 的白名单里（第三方只许在此），但这里**一个第三方也没用**；
//! `pyawa-core` 等核心 crate **不反向依赖**本 crate ✓（`T-CX-17`）。

pub mod json;
mod server;

pub use server::run_stdio;
