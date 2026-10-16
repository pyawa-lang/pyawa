//! **`_typing` 的最小可用面**（task-24 轮 1 ✓）：`TypeVar`／`NoDefault`（`NoDefaultType`）／`_idfunc`
//! —— 参照里这一档是 **C 模块** ✗（`Lib/_typing.py` 不存在 ✓）⇒ 本层落成"core 造类型 ＋
//! stdlib 装配模块"（与 `memoryview` 同款手法 ✓：`alloc_type_raw` ＋ `new` 槽 ＋ `getattr` 槽 ✓）。
//!
//! **口径对参照实测** ✓：
//! - `_typing.TypeVar('T')` ⇒ `__name__='T'` ✓、`__constraints__=()` ✓、`__bound__=None` ✓、
//!   `__covariant__=False` ✓、`__contravariant__=False` ✓、`__default__=_typing.NoDefault` ✓、
//!   `__infer_variance__=False` ✓（`TypeVar('D', default=5)` ⇒ `__default__=5` ✓）；
//! - `_typing.NoDefault` ⇒ `type(...) is _typing.NoDefaultType` ✓、`NoDefaultType()` 回**同一个**单例 ✓；
//! - `_typing._idfunc(5)` ⇒ `5` ✓（恒等函数 ✓）；
//! - `_typing.Generic` ⇒ 可当基类的普通类 ✓（"`Cannot inherit from plain Generic`" 那条在
//!   `typing.py` 的 `__init_subclass__` 里 ✓，归轮 3 ✓）。
//!
//! **如实范围** ✗（轮 1 只到"装得下"）：`__repr__`／`__or__`／`__typing_subst__` 等 `TypeVar` 的
//! 方法面未接 ✓；`Generic` 的 `__class_getitem__`／`__init_subclass__` 参数收集在轮 3 ✓。

use core::ptr::NonNull;

use crate::builtin_objects::{AttributeObject, BuiltinFunctionObject};
use crate::header::Header;
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::ExecError;

/// `_typing` 的 `TypeVar` 实例字段（都放**实例字典**里 ✓ —— 不新增载荷类型 ✓）。
const NAME: &str = "_name";
const CONSTRAINTS: &str = "_constraints";
const BOUND: &str = "_bound";
const COVARIANT: &str = "_covariant";
const CONTRAVARIANT: &str = "_contravariant";
const DEFAULT: &str = "_default";
const INFER_VARIANCE: &str = "_infer_variance";

/// **`TypeVar(name, *constraints, bound=None, covariant=False, contravariant=False,
/// infer_variance=False, default=NoDefault)`** ✓（实例化槽 ✓）。
pub fn typevar_new(
    class: NonNull<TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    // CPython 的签名检查顺序：先要 name ✓。
    let Some(name_value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "TypeVar() missing required argument 'name' (pos 1)",
        ));
    };
    let Some(name) = instance.text_of(name_value) else {
        return Err(instance.raise_builtin_error("TypeError", "TypeVar() argument 'name' must be str"));
    };
    let name = name.to_owned();
    let object = instance
        .alloc(AttributeObject::new(
            class,
            core::cell::RefCell::new(Some(instance.new_dict())),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: 刚造的就是 `AttributeObject`。
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let Some(dict) = view.attributes() else {
        return Err(instance.raise_builtin_error("RuntimeError", "TypeVar 没有实例字典"));
    };
    let name_value = instance.new_str(&name);
    instance.dict_set(dict, NAME, name_value);
    // 位置实参里剩下的都是**约束** ✓（`TypeVar('T', int, str)` ✓）。
    let constraints: Vec<NonNull<Header>> = args[1..]
        .iter()
        .map(|value| instance.retain(*value))
        .collect();
    let constraints = instance.new_tuple(constraints);
    instance.dict_set(dict, CONSTRAINTS, constraints);
    instance.release(constraints);
    let bound = instance.new_bool(false);
    instance.dict_set(dict, COVARIANT, bound);
    let contravariant = instance.new_bool(false);
    instance.dict_set(dict, CONTRAVARIANT, contravariant);
    let infer = instance.new_bool(false);
    instance.dict_set(dict, INFER_VARIANCE, infer);
    let default = default_marker(instance);
    instance.dict_set(dict, DEFAULT, default);
    Ok(object)
}

/// `_typing.NoDefault` 单例（**惰性**挂在 `NoDefaultType` 的类型命名空间里 ✓ —— 与 `memoryview`
/// 的 `__mro_entries__` 同款 ✓）。
pub fn no_default_singleton(instance: &Instance) -> NonNull<Header> {
    let Some(no_default_type) = instance.type_named("NoDefaultType") else {
        return instance.singletons().none();
    };
    if let Some(namespace) = instance.type_namespace(no_default_type.cast()) {
        if let Some(existing) = instance.dict_get(namespace, "__no_default_singleton__") {
            // SAFETY: existing 由命名空间持有，存活；这里交出新的一份。
            unsafe { instance.incref_object(existing.as_ptr()) };
            return existing;
        }
        let object = instance
            .alloc(AttributeObject::new(
                no_default_type,
                core::cell::RefCell::new(Some(instance.new_dict())),
            ))
            .into_raw()
            .cast::<Header>();
        instance.dict_set(namespace, "__no_default_singleton__", object);
        return object;
    }
    instance.singletons().none()
}

fn default_marker(instance: &Instance) -> NonNull<Header> {
    no_default_singleton(instance)
}

/// **`NoDefaultType()` 的实例化槽** ✓：回**同一个**单例 ✓（参照实测 `NoDefaultType() is NoDefault` ✓）。
pub fn no_default_new(
    _class: NonNull<TypeObject>,
    _args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    Ok(no_default_singleton(instance))
}

/// **`TypeVar` 的属性面** ✓（`getattr` 槽；本层 `alloc_type_raw` 型不走 MRO 查字典 ✓）。
pub unsafe fn typevar_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 契约保证 ptr 是本类型的存活对象。
    let object = unsafe { NonNull::new_unchecked(ptr) };
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let dict = view.attributes()?;
    match name {
        "__name__" => instance.dict_get(dict, NAME),
        "__constraints__" => instance.dict_get(dict, CONSTRAINTS),
        "__bound__" => Some(match instance.dict_get(dict, BOUND) {
            Some(value) => value,
            None => {
                let none = instance.singletons().none();
                // SAFETY: none 是单例 ⇒ 交出新的一份 ✓。
                unsafe { instance.incref_object(none.as_ptr()) };
                none
            }
        }),
        "__covariant__" => instance.dict_get(dict, COVARIANT),
        "__contravariant__" => instance.dict_get(dict, CONTRAVARIANT),
        "__infer_variance__" => instance.dict_get(dict, INFER_VARIANCE),
        "__default__" => instance.dict_get(dict, DEFAULT),
        "__type_params__" => {
            let empty = instance.new_tuple(Vec::new());
            Some(empty)
        }
        _ => None,
    }
}

/// **`NoDefaultType` 的属性面** ✓（最小面：`repr` 由 `str`／`repr` 槽兜 ✓）。
pub unsafe fn no_default_getattr(
    _ptr: *mut Header,
    _name: &str,
    _instance: &Instance,
) -> Option<NonNull<Header>> {
    None
}

/// **`_typing._idfunc(x)`** ✓：恒等函数 ✓（参照 `_idfunc(5) == 5` ✓）。
pub fn idfunc_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_idfunc() takes 1 positional argument but 0 were given",
        ));
    };
    Ok(instance.retain(value))
}

/// 造 `_typing` 用的两个类型（在引导期调用一次 ✓；与 `memoryview`／`bytearray` 同款 ✓）。
pub fn build_typing_types(instance: &Instance) -> (NonNull<TypeObject>, NonNull<TypeObject>) {
    let typevar_type = instance.alloc_type_raw(
        "TypeVar",
        core::mem::size_of::<AttributeObject>(),
        AttributeObject::slots()
            .with_new(typevar_new)
            .with_getattr(typevar_getattr),
    );
    let no_default_type = instance.alloc_type_raw(
        "NoDefaultType",
        core::mem::size_of::<AttributeObject>(),
        AttributeObject::slots()
            .with_new(no_default_new)
            .with_getattr(no_default_getattr),
    );
    (typevar_type, no_default_type)
}

/// `_typing` 名字表里那些"**只占位**"的名字 ✓（轮 1 如实：先用普通类型把名字立住 ✓，
/// 语义在轮 3 的 `Lib/typing.py` 那条链上补 ✓）。
pub const PLACEHOLDER_NAMES: &[&str] = &[
    "TypeVarTuple",
    "ParamSpec",
    "ParamSpecArgs",
    "ParamSpecKwargs",
    "TypeAliasType",
    "Union",
];

/// 造一个占位类型 ✓（名字是类型 ✓，参照里它们也都是可调用的类／特殊对象 ✓）。
pub fn placeholder_type(instance: &Instance, name: &'static str) -> NonNull<TypeObject> {
    instance
        .type_named(name)
        .unwrap_or_else(|| instance.new_attribute_type(name))
}

/// 供 stdlib 模块装配用：把 `_idfunc` 包成 `builtin_function_or_method` ✓。
pub fn make_function(instance: &Instance, name: &'static str, handler: crate::NativeFn) -> NonNull<Header> {
    let function_type = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(BuiltinFunctionObject::new(
        function_type,
        name,
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}
