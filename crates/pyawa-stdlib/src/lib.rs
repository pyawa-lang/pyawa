//! Pyawa 的 C 层模块实现：从 Python 看到的 API 与语义。
//!
//! 逐模块合约归属 `docs/SPEC-c-modules.md`（`CM-`），见本 crate 的 `README.md`。
//!
//! **已落地**：`_opcode` 与 `_opcode_metadata` 的数据表与纯函数
//! （`opcode_metadata` 由 `tools/gen_opcode_tables.py` 从本机 CPython 3.14 探测生成）。
//! 其余模块仍是占位。

#![forbid(unsafe_code)]

pub mod opcode;
pub mod opcode_metadata;
