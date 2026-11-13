//! `pyawa-lsp` 可执行入口：在 stdio 上跑 LSP（`Content-Length` 帧的 JSON-RPC 2.0）。
//!
//! 用法：编辑器按 LSP 约定启动本程序（stdio 传输）；日志走 stderr（stdout 只放协议帧 ✓）。

fn main() {
    if let Err(error) = pyawa_lsp::run_stdio() {
        eprintln!("pyawa-lsp: {error}");
        std::process::exit(1);
    }
}
