//! Pyawa VM 核心：实例、对象模型、帧、字节码解释器、编译管线。
//!
//! 归属规格与硬约束见本 crate 的 `README.md`。模块树按规格编号组织，
//! 代码里的 `OM-n`／`BC-n` 指回那条硬约束。
//!
//! **已落地**
//!
//! - 对象模型 §4 实例级内存（**OM-1**…**OM-4**）
//! - 对象模型 §5 对象头（**OM-5**…**OM-8**）
//! - 对象模型 §6 类型对象的最小骨架（**OM-9**…**OM-15**：字段与槽位就位、注册表按实例）
//! - 对象模型 §7 引用计数协议（**OM-16**…**OM-22**；**OM-21** 的显式待处理栈已用于深链释放）
//! - 对象模型 §9 的**回收算法**：标记-清除（**OM-25**）、触发点与可配阈值（**OM-26**）、
//!   回收顺序的 ③④（**OM-27**：终结器 → 释放，含复活后的重判定）、遍历计数（**OM-29**）
//! - 字节码 §9 的**帧布局**（**BC-42**…**BC-48**）：值栈上界、局部槽、独立的 cell 槽、
//!   指令指针与异常表游标、可挂起状态（挂起／恢复）；`BC-54` 的异常表**只保存字节串**
//! - 字节码 §9 的 **BC-45**：cell 是独立对象且 `GC_TRACKED`（经 `traverse`／`clear` 入链）
//!
//! **尚未接线**（占位，不要当成已就位）：
//!
//! - 字节码 §10 的**起步指令集**与执行器、§11 的下降规则、§8 的码元解码
//! - 字节码 §2.4 的完整 `co_*` 表面（`co_consts`／`co_names`／`co_positions()`…）与 **BC-54** 的
//!   异常表解析（现在只按字节串保存）
//! - 对象模型 §8 单例表（**OM-23**；`IMMORTAL` 位已按 **OM-24** 预留）与 §12 值表示（**OM-38**／**OM-39**）
//! - 对象模型 §10 弱引用：**OM-27** 的 ② "先清弱引用"目前只是顺序上的占位点
//! - **OM-28** `gc` 模块的可见行为（需要模块系统，不属本层）
//! - 对象模型 §11 宿主对象（**OM-34**…**OM-37**）、**OM-13** C3 线性化、**OM-14** 宿主类型注册
//! - **OM-11** 槽位表里 `getattr`／`setattr`／`call`／`hash`／`richcompare`／`iter`／
//!   `repr`／`str` 这八个槽位（它们的签名取决于值表示与类型系统）
//! - **已知偏离 `OM-12`**：类型对象自身也会成环（`bases`／`mro`／`dict`），但**没有**标
//!   `GC_TRACKED`、也**没有** `traverse`／`clear`——`OM-12` 要求可成环的类型两者齐备。
//!   `alloc` 现在按"是否提供 `traverse`"判定入链，所以类型对象暂不入回收链表；
//!   补 `traverse`／`clear` 时**必须**同时补标记。
//!
//! **边界**
//!
//! - **OM-6**：本 crate 的内部表示（头部、句柄）**禁止**出现在 C ABI 签名里——那是 `pyawa-abi`。
//! - `DESIGN.md` §7 原则 5：本 crate 不依赖 `std::fs`／`std::net`／libc，不出现
//!   `#[cfg(target_os)]`（约束见 `CX-4`，检查实现见 `tests/ci/check.py`）。
//! - **OM-18**：**禁止**用 `Rc`／`Arc` 作对象引用。

#![deny(unsafe_op_in_unsafe_fn)]

mod cell;
mod code;
pub mod flags;
mod frame;
mod header;
mod instance;
mod macros;
mod refcount;
mod type_object;

pub use cell::CellObject;
pub use code::CodeObject;
pub use frame::{Frame, FrameError};
pub use header::{Header, PyObject};
pub use instance::Instance;
pub use refcount::{Borrowed, Owned};
pub use type_object::{Slots, TypeObject};
