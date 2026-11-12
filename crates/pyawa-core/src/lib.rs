//! Pyawa VM 核心：实例、对象模型、帧、字节码解释器、编译管线。
//!
//! **模块与落地状态**（原先挂在本文件头部的 2133 行 `//!` ✗）见
//! [`docs/CORE-MODULES.md`](../../docs/CORE-MODULES.md) ✓ —— 一处真相 ✓。
//! 归属规格与硬约束见 `docs/REQUIREMENTS.md`／`docs/DESIGN.md`／`docs/CONSTRAINTS.md`；
//! 编号（`OM-n`／`BC-n`／`TS-n`…）的出处见 `docs/SPEC-INDEX.md` ✓。

#![deny(unsafe_op_in_unsafe_fn)]

// **`dict.fromkeys`** ✓（第 184 轮）：实现在 core（要看容器内部 ✓）⇒ 从根**再导出** ✓ ——
// `builtin_objects` 是**私有模块** ✗，stdlib 直接调不到 ✓（与 `weakref_new` 同一个教训 ✓）。
pub use builtin::context::locals_native;
pub use builtin::dict::dict_fromkeys_native;
pub use builtin::object::object_dir_native;
pub use builtin::object::object_comparison_not_implemented_native;
pub use builtin::object::object_getstate_native;
pub use builtin::object::object_reduce_native;
pub use builtin::object::object_sizeof_native;
pub use builtin::object::object_init_native;
// **`builtins.compile()`**（task-43）：stdlib 的 `compile` 注册与 `exec`/`eval` 的 code 对象分支用它。
pub use builtin::compile::{
    compile_native, eval_code_public, eval_source_public, exec_code_public, implicit_namespaces,
    is_code_object,
};
pub use builtin::dict::dict_init_native;
pub use builtin::object::object_text_native;
pub use builtin_objects::str_method_native;
pub use builtin::typing::{
    build_typing_types, generic_init_subclass_native, idfunc_native, make_function,
    no_default_singleton, placeholder_type, typevar_new_kwargs, PLACEHOLDER_NAMES,
};
pub use builtin::defaultdict::build_defaultdict_type;
// **数据类型自己的 `__new__`**（task-21 G2 ✓）：stdlib 的 `builtins` 安装处要按类型挂上 ✓。
pub use builtin_objects::{
    bytes_dunder_new, data_dunder_repr_native, data_dunder_str_native, dict_dunder_new,
    float_dunder_new, int_dunder_new, is_default_new, list_dunder_new, set_dunder_new,
    str_dunder_new, tuple_new_native,
};
pub use builtin::function::function_descriptor_get;
pub use builtin_objects::weakref_dunder_new;
/// **Unicode 大小写映射**（`tools/gen_unicode.py` 生成；`P3-42` round 44）。
pub mod unicode_case;
pub use builtin_objects::{
    weakref_hash_state, weakref_key_hash_state, weakref_state, weakref_store_hash,
    weakref_store_key_hash, weakref_target,
};
pub use builtin_objects::reversed_new;
/// **内建 `iter`／`reversed` 函数对象的引导期缓存**（round 232；`it.__reduce__()` 用 ✓）。
pub use builtin_objects::cache_builtin_iterator_function;
pub use builtin_objects::zip_new;
pub use builtin_objects::dir_new;
pub use builtin_objects::enumerate_new;
pub use builtin_objects::getframe_native;
pub use builtin_objects::getframemodulename_native;
pub use builtin_objects::super_new;
pub use builtin_objects::type_new_native;
pub use classes::build_class_from_parts;
pub use builtin::object::object_new_native;
pub use builtin::property::property_descriptor_get;
pub use executor::raise_object_public;
pub use executor::protocol::instance_attribute_delete;
pub use builtin::thread::thread_allocate_lock_native;
pub use builtin::thread::thread_get_ident_native;
pub use builtin::function::function_code_native;
pub use builtin::function::function_globals_native;
mod diag;
/// **内部集合的快速哈希**（round 248 性能，A2-3）：见模块文档。
pub(crate) mod fast_hash;
pub mod argdecode;
pub mod bigint;
pub mod host_env;
/// **WTF-8 编解码**（P3-38 S1）：允许孤立代理的 UTF-8 超集；供 `str` 存储迁移（S2）复用。
pub mod wtf8;
mod builtin;
mod builtin_objects;
mod call_edge;
mod cell;
mod classes;
mod code;
pub mod compile;
/// **`_tokenize` 的词法底层** ✓（task-19 ✓，一处真相：就是编译器的词法器 ✓）。
pub use compile::lexer::{python_tokens, PythonToken, PythonTokenKind};
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
/// **`.pyac` 的纯编解码**（`IM-18`…`IM-21`；路径/IO/陈旧判定在 `pyawa-runtime`）
pub mod pyac;
pub mod opcode_metadata;
mod refcount;
mod singleton;
mod type_object;
mod unicode_names;
mod value;

pub use builtin_objects::{
    free_fixed_layout, python_level_finalize, AttributeObject, BoolObject, DictObject,
    ExceptionObject, FloatObject, FunctionObject, BuiltinFunctionObject, GeneratorObject,
    IntObject, IteratorObject, ListObject, MethodObject, NativeFn, NoneObject, NullObject,
    PlainObject, SetObject, StrObject, TupleObject,
};
pub use cell::CellObject;
pub use code::{code_getattr, CodeObject};
pub use executor::{discard_raised, mounted_instance_dict, 
    attribute_read, attribute_write, call_value, execute, subscript_read, subscript_write,
    values_equal_public, ExecError, ExecOutcome,
};
pub use format::repr_float;
pub use format::SpecError;
pub use frame::{Frame, FrameError};
pub use header::{Header, PyObject, HEADER_SIZE_BYTES};
pub use instance::{Instance, INT_MAX_STR_DIGITS_DEFAULT, INT_MAX_STR_DIGITS_THRESHOLD, CapabilityCallError};
pub(crate) use instance::HotId;
pub use refcount::{Borrowed, Owned, PyRef};
pub use singleton::{Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
pub use type_object::{
    HostDealloc, HostTraverse, HostVisit, Slots, TypeObject, GENERIC_ALLOCATION,
    HAS_INSTANCE_DICT,
};
pub use value::Value;

/// `_contextvars.copy_context()` 的实现入口（第 332 轮；stdlib 的模块要用 ✓）。
pub use builtin_objects::new_classmethod;

/// **AST 解析回调**（round 86）：`compile(..., PyCF_ONLY_AST)` 交给 `_ast` 模块登记的实现 ✓
/// （core 不依赖 stdlib：由 stdlib 在引导期把函数指针登记进实例 ✓）。
pub type AstParserFn =
    fn(&crate::Instance, &str, &str)
        -> Result<core::ptr::NonNull<crate::Header>, crate::ExecError>;
pub use builtin_objects::copy_context_value;
pub use builtin::thread::thread_handle_new;
pub use builtin::thread::{thread_handle_mark_done, thread_handle_getattr};
pub use builtin_objects::filter_new;
pub use builtin_objects::map_new;

pub use builtin::namespace::{build_simple_namespace_type, make_simple_namespace};

pub use builtin::deque::install_deque_init;
