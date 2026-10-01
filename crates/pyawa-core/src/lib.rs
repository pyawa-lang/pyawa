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
//! - 对象模型 §8 单例表（**OM-23**：`None`／`True`／`False`／小整数按实例创建；
//!   `IMMORTAL` 位按 **OM-24** 只预留并保持 0）
//! - 对象模型 §9 的**回收算法**：标记-清除（**OM-25**）、触发点与可配阈值（**OM-26**）、
//!   回收顺序的 ③④（**OM-27**：终结器 → 释放，含复活后的重判定）、遍历计数（**OM-29**）
//! - 对象模型 §12 的值表示（**OM-38**／**OM-39**：带标签枚举，只有单例覆盖到的值才内联）
//!   与类型擦除守卫 `PyRef`（**OM-40** 的访问器 `Instance::own`）
//! - 字节码 §8 的**指令表与元数据**（**BC-30**…**BC-42**：基线 154 名与编号、专有指令取空闲编号、
//!   19 个 cache 宽度、七类分类、栈效应规则、`nb_ops` 与 `cmp_op` 顺序、指令集版本常量）——
//!   数值由 `tools/gen_opcode_tables.py` 从运行时探测生成，`pyawa-stdlib` 的
//!   `_opcode`／`_opcode_metadata` 是它的 Python 层包装（**BC-38** 的依赖边裁决）
//! - 字节码 §8 的**码元解码**（**BC-32**…**BC-36**：2 字节码元、`EXTENDED_ARG` 大端折叠、
//!   cache 槽跳过，以及发射方体检 `validate`）
//! - 字节码 §9 的**帧布局**（**BC-42**…**BC-48**）：值栈上界、局部槽、独立的 cell 槽、
//!   指令指针与异常表游标、可挂起状态（挂起／恢复）；`BC-54` 的异常表**只保存字节串**
//! - 字节码 §9 的 **BC-45**：cell 是独立对象且 `GC_TRACKED`（经 `traverse`／`clear` 入链）
//! - 字节码 §10 起步指令集的前两批（**BC-49**）：常量与局部、整数运算与比较、`is`、
//!   `RETURN_VALUE`，以及**控制流**（跳转目标按 **BC-55** 算，`T-BC-17` 用参照实现产出的
//!   码元逐条对拍）——**调用、容器、属性与下标、异常、生成器尚未接线**
//! - 类型系统 **TS-40** 的内建层次第一层：`bool ⊂ int`（`True + 1` 算 2）＋ `Instance::is_subtype`
//!
//! **尚未接线**（占位，不要当成已就位）：
//!
//! - **空串单例**（`OM-23` 的"空串"）——需要 `str` 类型，随类型系统落地
//! - 字节码 §10 起步指令集的其余部分（控制流、调用、容器、属性与下标、异常、生成器、
//!   格式化、模式匹配、PEP 695）与 §11 的下降规则
//! - 字节码 §2.4 的完整 `co_*` 表面（`co_consts`／`co_names`／`co_positions()`…）与 **BC-54** 的
//!   异常表解析（现在只按字节串保存）
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
pub mod decode;
pub mod executor;
pub mod flags;
mod frame;
mod header;
mod instance;
mod macros;
pub mod opcode;
pub mod opcode_metadata;
mod refcount;
mod singleton;
mod type_object;
mod value;

pub use cell::CellObject;
pub use code::CodeObject;
pub use executor::{execute, ExecError};
pub use frame::{Frame, FrameError};
pub use header::{Header, PyObject};
pub use instance::Instance;
pub use refcount::{Borrowed, Owned, PyRef};
pub use singleton::{BoolObject, IntObject, NoneObject, Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
pub use type_object::{Slots, TypeObject};
pub use value::Value;
