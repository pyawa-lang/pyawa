//! 内建类型的**载荷**（`docs/SPEC-type-system.md` 的 `TS-43`：布局由实现自选，不进 ABI）。
//!
//! **TS-41** 的表（`crate::builtin_types`）只记"类型存在、层次正确"；本文件才是它们的表示。
//! 只有 `OM-23` 点名的那几个才做单例（`None`／`True`／`False`／小整数／空串），
//! 其余类型"每次造一个新对象"——`is` 语义因此与参照实现一致（`OM-39`）。

mod prelude;

mod scalars;
mod range_slice;
mod containers;
mod contextvars;
mod thread;
mod iterators;
mod callables;
mod exceptions;
mod type_runtime;
mod constructors;
mod representations;
mod formatting;
mod memoryview;
mod complex;

pub use self::scalars::*;
pub use self::range_slice::*;
pub use self::containers::*;
pub use self::contextvars::*;
pub use self::thread::*;
pub use self::iterators::*;
pub use self::callables::*;
pub use self::exceptions::*;
pub use self::type_runtime::*;
pub use self::constructors::*;
pub use self::representations::*;
pub use self::formatting::*;
pub use self::complex::*;
pub use self::memoryview::*;
