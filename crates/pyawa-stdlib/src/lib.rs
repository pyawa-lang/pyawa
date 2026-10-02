//! Pyawa 的 C 层模块实现：从 Python 看到的 API 与语义。
//!
//! 逐模块合约归属 `docs/SPEC-c-modules.md`（`CM-`），见本 crate 的 `README.md`。
//!
//! **指令表的数据与纯函数归属 `pyawa-core`**（`BC-38` 的依赖边裁决：指令集是 VM 的一部分）。
//! 本 crate 只做 Python 层包装，见 [`opcode`]。

#![forbid(unsafe_code)]

pub mod builtins_module;
pub mod errno_map;
pub mod errno_module;
/// `operator`（契约 `docs/SPEC-c-modules.md` §5.2.7；本层第一刀：`eq`／`ne`／`is_`／`is_not`／`truth`／`not_`）
pub mod operator_module;
pub mod opcode;
/// `sys`（不依赖能力域的部分；契约 `docs/SPEC-c-modules.md` §5.2.3）
pub mod sys_module;

/// `itertools`（契约 `docs/SPEC-c-modules.md` §5.2.6；本层已落地 **18** 个：`count`／`repeat`／
/// `islice`／`chain`／`takewhile`／`dropwhile`／`filterfalse`／`accumulate`／`starmap`／`cycle`／
/// `pairwise`／`batched`／`zip_longest`／`compress`／`combinations`／`permutations`／
/// `combinations_with_replacement`／`product`；参照 20 个公开名，剩 `groupby`／`tee`／
/// `chain.from_iterable`）
pub mod itertools_module;

/// `_imp`（契约 `docs/SPEC-c-modules.md` §5.2.4；本层落地 `pyc_magic_number_token` 与 `is_builtin`，
/// 其余逐条记在 §5.2.4 的"未落地"）
pub mod imp_module;
pub mod unicode_tables;
