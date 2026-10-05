//! Pyawa VM 核心：实例、对象模型、帧、字节码解释器、编译管线。
//!
//! **模块与落地状态**（原先挂在本文件头部的 2133 行 `//!` ✗）见
//! [`docs/CORE-MODULES.md`](../../docs/CORE-MODULES.md) ✓ —— 一处真相 ✓。
//! 归属规格与硬约束见 `docs/REQUIREMENTS.md`／`docs/DESIGN.md`／`docs/CONSTRAINTS.md`；
//! 编号（`OM-n`／`BC-n`／`TS-n`…）的出处见 `docs/SPEC-INDEX.md` ✓。

#![deny(unsafe_op_in_unsafe_fn)]

// **`dict.fromkeys`** ✓（第 184 轮）：实现在 core（要看容器内部 ✓）⇒ 从根**再导出** ✓ ——
// `builtin_objects` 是**私有模块** ✗，stdlib 直接调不到 ✓（与 `weakref_new` 同一个教训 ✓）。
pub use builtin_objects::dict_fromkeys_native;
pub use builtin_objects::object_init_native;
pub use builtin_objects::dict_init_native;
pub use builtin_objects::object_text_native;
pub use builtin_objects::str_method_native;
pub use builtin_objects::reversed_new;
pub use builtin_objects::zip_new;
pub use builtin_objects::enumerate_new;
pub use builtin_objects::getframe_native;
pub use builtin_objects::super_new;
pub use builtin_objects::type_new_native;
pub use builtin_objects::object_new_native;
pub use builtin_objects::property_descriptor_get;
pub use executor::raise_object_public;
pub use builtin_objects::{thread_allocate_lock_native, thread_get_ident_native};
pub use builtin_objects::function_code_native;
pub use builtin_objects::function_globals_native;
pub mod argdecode;
pub mod bigint;
mod builtin_objects;
mod cell;
mod classes;
mod code;
pub mod compile;
pub mod builtin_types;
pub mod decode;
pub mod executor;
mod format;
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

pub use builtin_objects::{
    free_fixed_layout, python_level_finalize, AttributeObject, BoolObject, DictObject,
    ExceptionObject, FloatObject, FunctionObject, BuiltinFunctionObject, GeneratorObject,
    IntObject, IteratorObject, ListObject, MethodObject, NativeFn, NoneObject, NullObject,
    PlainObject, SetObject, StrObject, TupleObject,
};
pub use cell::CellObject;
pub use code::{code_getattr, CodeObject};
pub use executor::{mounted_instance_dict, 
    attribute_read, attribute_write, call_value, execute, subscript_read, subscript_write,
    values_equal_public, ExecError, ExecOutcome,
};
pub use format::repr_float;
pub use format::SpecError;
pub use frame::{Frame, FrameError};
pub use header::{Header, PyObject, HEADER_SIZE_BYTES};
pub use instance::{Instance, INT_MAX_STR_DIGITS_DEFAULT, INT_MAX_STR_DIGITS_THRESHOLD, CapabilityCallError};
pub use refcount::{Borrowed, Owned, PyRef};
pub use singleton::{Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
pub use type_object::{
    HostDealloc, HostTraverse, HostVisit, Slots, TypeObject, GENERIC_ALLOCATION,
    HAS_INSTANCE_DICT,
};
pub use value::Value;

/// `_contextvars.copy_context()` 的实现入口（第 332 轮；stdlib 的模块要用 ✓）。
pub use builtin_objects::copy_context_value;
pub use builtin_objects::thread_handle_new;
pub use builtin_objects::filter_new;
pub use builtin_objects::map_new;
