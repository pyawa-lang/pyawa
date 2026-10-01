//! `_opcode`／`_opcode_metadata` 的 **Python 层包装**（`BC-38`、`SPEC-c-modules.md` §12）。
//!
//! 指令表的数据与纯函数**归属 `pyawa-core`**——指令集是 VM 的一部分；
//! 依赖方向 `pyawa-stdlib → pyawa-core`（`REQUIREMENTS.md` 的 crate 依赖边行）。
//!
//! 本模块是这两个 Python 模块的挂载点：模块系统（`IM-`）落地前只做**转发**，
//! **不复制任何数值**——复制会造出第二个真相源（`BC-38`）。

pub use pyawa_core::opcode::*;
pub use pyawa_core::opcode_metadata::*;
