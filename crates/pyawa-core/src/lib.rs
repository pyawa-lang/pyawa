//! Pyawa VM 核心：实例、对象模型、帧、字节码解释器、编译管线。
//!
//! 归属规格与硬约束见本 crate 的 `README.md`。模块树按 `docs/SPEC-object-model.md`
//! 的编号组织，代码里的 `OM-n` 指回那条硬约束。
//!
//! **已落地**
//!
//! - §4 实例级内存（**OM-1**…**OM-4**）
//! - §5 对象头（**OM-5**…**OM-8**）
//! - §6 类型对象的最小骨架（**OM-9**…**OM-15**：字段与槽位就位、注册表按实例）
//! - §7 引用计数协议（**OM-16**…**OM-20**、**OM-22**）
//!
//! **尚未接线**（占位，不要当成已就位）：§8 单例表（**OM-23**；`IMMORTAL` 位已按 **OM-24** 预留）、
//! §9 循环回收（**OM-25**…**OM-30**）、§10 弱引用、§11 宿主对象、§12 值表示，
//! 以及 **OM-11** 槽位表里 `getattr`／`setattr`／`call`／`hash`／`richcompare`／`iter`／
//! `repr`／`str` 这八个槽位（它们的签名取决于值表示与类型系统）。
//!
//! **边界**
//!
//! - **OM-6**：本 crate 的内部表示（头部、句柄）**禁止**出现在 C ABI 签名里——那是 `pyawa-abi`。
//! - `DESIGN.md` §7 原则 5：本 crate 不依赖 `std::fs`／`std::net`／libc，不出现
//!   `#[cfg(target_os)]`（检查项见 `tests/ci/README.md` 第 3 项）。
//! - **OM-18**：**禁止**用 `Rc`／`Arc` 作对象引用。

#![deny(unsafe_op_in_unsafe_fn)]

pub mod flags;
mod header;
mod instance;
mod macros;
mod refcount;
mod type_object;

pub use header::{Header, PyObject};
pub use instance::Instance;
pub use refcount::{Borrowed, Owned};
pub use type_object::{Slots, TypeObject};
