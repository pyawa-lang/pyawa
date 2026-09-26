//! Pyawa 稳定 C ABI：栈式线格式、不透明句柄、`catch_unwind` 边界。
//!
//! 函数清单归属 `docs/SPEC-c-abi.md`（`AB-`），见本 crate 的 `README.md`。
//!
//! 本 crate 是工作区内唯一预期需要 `unsafe` 的地方（FFI 边界）。
