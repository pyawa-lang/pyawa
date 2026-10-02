//! Pyawa 的 C 层模块实现：从 Python 看到的 API 与语义。
//!
//! 逐模块合约归属 `docs/SPEC-c-modules.md`（`CM-`），见本 crate 的 `README.md`。
//!
//! **指令表的数据与纯函数归属 `pyawa-core`**（`BC-38` 的依赖边裁决：指令集是 VM 的一部分）。
//! 本 crate 只做 Python 层包装，见 [`opcode`]。其余模块仍是占位。

#![forbid(unsafe_code)]

pub mod builtins_module;
pub mod errno_map;
pub mod errno_module;
pub mod opcode;
/// `sys`（不依赖能力域的部分；契约 `docs/SPEC-c-modules.md` §5.2.3）
pub mod sys_module;

/// `itertools`（契约 `docs/SPEC-c-modules.md` §5.2.6；本层先落地 `count`）
pub mod itertools_module;

/// `_imp`（契约 `docs/SPEC-c-modules.md` §5.2.4；本层只落地 `pyc_magic_number_token` 与 `is_builtin`）
pub mod imp_module;
pub mod unicode_tables;
