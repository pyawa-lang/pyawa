//! **`_collections.defaultdict`（最小面）** ✓（task-24 尾件 ✓）。
//!
//! 参照里 `collections.defaultdict` 来自 C 的 `_collections` ✓ —— `Lib/collections/__init__.py`
//! 的 `from _collections import defaultdict` 有 try/except ✓，缺了它 `collections.defaultdict`
//! 就**静默不存在** ✗ ⇒ `Lib/typing.py` 第 23 行 `from collections import defaultdict` 直接
//! `ImportError` ✓（本轮实测的第一格 ✓）。
//!
//! **口径**：类型在 core 造（`alloc_type_raw` ✓ —— 与 `TypeVar`／`memoryview` 同款 ✓），
//! 载荷借 `AttributeObject`（`_factory` ＋ 内层 `_data` 字典 ✓）；方法面装进**类型命名空间**
//! （`__getitem__`／`__setitem__`／`__contains__`／`clear` ✓）—— 走既有协议回退：
//! `subscript_read` 的 `type_lookup` ✓、`subscript_del`／`subscript_write` 的属性通道 ✓、
//! `iter.rs` 的 `__contains__` ✓（都是**一处真相** ✓，本文件不另写查键 ✓）。
//!
//! **只接 `typing.py` 实际用到的那一面** ✗（如实登记）：`default_factory` 属性 ✓、
//! `defaultdict(factory)` ✓、`d[k]` 缺键调工厂并写回 ✓、`d[k] = v` ✓、`k in d` ✓、`clear` ✓；
//! 其余（`repr`／`copy`／`__eq__`／`__or__`／`__missing__` 覆写语义／`isinstance(d, dict)` ✓ 等）
//! 未接 ✓ —— 撞上再补 ✓。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use crate::builtin_objects::{AttributeObject, BuiltinFunctionObject, NativeFn};
use crate::executor::ExecError;
use crate::{Header, Instance, TypeObject};

/// 工厂（`default_factory`）在实例字典里的私有键 ✓。
const FACTORY: &str = "_factory";
/// 内层数据字典在实例字典里的私有键 ✓。
const DATA: &str = "_data";

/// 造一个原生可调用对象（**新引用** ✓；与 `collections_module`／`typing.rs` 同一做法 ✓）。
fn make_native(instance: &Instance, name: &'static str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(BuiltinFunctionObject::new(ty, name, Cell::new(handler)));
    object.into_raw().cast::<Header>()
}

/// 取实例字典里的**私有**属性（**借用** ✓；不是 `defaultdict` 实例或没建字典 ⇒ `None` ✓）。
fn private_attr(instance: &Instance, this: NonNull<Header>, key: &str) -> Option<NonNull<Header>> {
    let expected = instance.type_named("defaultdict")?;
    // SAFETY: 调用方保证 this 是存活对象；类型身份查注册表。
    if unsafe { this.as_ref() }.ty() != expected {
        return None;
    }
    // SAFETY: 类型身份刚确认 ⇒ 载荷就是 `AttributeObject` ✓。
    let object = unsafe { &*this.as_ptr().cast::<AttributeObject>() };
    let attributes = object.attributes()?;
    instance.dict_get(attributes, key)
}

/// 取本实例的数据字典（`_data` ✓；借用 ✓）。
fn data_of(instance: &Instance, this: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    private_attr(instance, this, DATA)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "defaultdict 的内部字典丢了"))
}

/// **`defaultdict(default_factory=None, /)`** ✓（`new` 槽 ✓）：实参借用 ⇒ 要留住的自己添引用 ✓。
///
/// # Safety
/// 由 `new` 槽契约保证 `class` 是类型对象、`args` 是帧值栈上的存活借用。
pub unsafe fn defaultdict_new(
    class: NonNull<TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    let none = instance.singletons().none();
    let factory = match args.first().copied() {
        Some(value) if value != none => instance.retain(value),
        _ => instance.retain(none),
    };
    let object = instance
        .alloc(AttributeObject::new(class, RefCell::new(Some(instance.new_dict()))))
        .into_raw()
        .cast::<Header>();
    // SAFETY: 刚造的就是 `AttributeObject` ✓。
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let Some(attributes) = view.attributes() else {
        instance.release(factory);
        return Err(instance.raise_builtin_error("RuntimeError", "defaultdict 没有实例字典"));
    };
    // `dict_set` 是**借用** ✓ ⇒ 放完自己那份要归还 ✓。
    instance.dict_set(attributes, FACTORY, factory);
    instance.release(factory);
    let data = instance.new_dict();
    instance.dict_set(attributes, DATA, data);
    instance.release(data);
    Ok(object)
}

/// **`defaultdict` 的属性面** ✓：`default_factory` 交回工厂 ✓（`dict_get` 是**借用** ⇒
/// 交出去前必须添一份新引用 ✓，与 `TypeVar.__name__` 那次真 bug 同一条契约 ✓）；其余 `None` ✓
/// —— 方法面在**类型命名空间**里，由属性通道的 ③ 段绑定 ✓。
///
/// # Safety
/// 由 `getattr` 槽契约保证 `ptr` 指向本类型的存活对象。
pub unsafe fn defaultdict_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if name != "default_factory" {
        return None;
    }
    // SAFETY: 契约同 `tuplegetter_get` ✓。
    let object = unsafe { &*ptr.cast::<AttributeObject>() };
    let attributes = object.attributes()?;
    let found = instance.dict_get(attributes, FACTORY)?;
    // SAFETY: found 由实例字典持有 ✓；这里交出新的一份 ✓。
    unsafe { instance.incref_object(found.as_ptr()) };
    Some(found)
}

/// **`d[key]`** ✓：`type_lookup("__getitem__")` 走这条路（`Some(container)` 就是 `bound` ✓）；
/// 有键 ⇒ 原样读 ✓；缺键 ⇒ 调工厂、**写回**、返回 ✓（`defaultdict.__missing__` 的语义 ✓）；
/// 没工厂 ⇒ 交回内层字典的 `KeyError` ✓（消息与参照同形：`KeyError` 的 `args` 是那个键 ✓）。
///
/// # Safety
/// 由 `NativeFn` 契约保证实参是存活借用。
pub unsafe fn defaultdict_getitem(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__getitem__ expected 2 arguments"));
    };
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "__getitem__ expected 2 arguments"));
    };
    let data = data_of(instance, this)?;
    if crate::executor::iter::contains(instance, data, key, 0)? {
        return crate::executor::subscript::subscript_read(instance, data, key);
    }
    let none = instance.singletons().none();
    let Some(factory) = private_attr(instance, this, FACTORY) else {
        return crate::executor::subscript::subscript_read(instance, data, key);
    };
    if factory == none {
        return crate::executor::subscript::subscript_read(instance, data, key);
    }
    // 工厂是**借用** ✓ ⇒ 调用时自己添引用 ✓（`call_callable` 的绑定位是 `None` ✓）。
    instance.retain(factory);
    let produced =
        crate::executor::call::call_callable(instance, factory, None, Vec::new(), Vec::new(), 0)?;
    // **写回**：`subscript_set` 借用键、**接管值** ✓ ⇒ 先添一份给字典、我们这份交给调用方 ✓。
    instance.retain(produced);
    crate::executor::subscript::subscript_set(instance, data, key, produced, 0)?;
    Ok(produced)
}

/// **`d[key] = value`** ✓（属性通道的 `__setitem__` ✓）。`subscript_set` 接管值 ⇒ 先添引用 ✓。
///
/// # Safety
/// 契约同 [`defaultdict_getitem`] ✓。
pub unsafe fn defaultdict_setitem(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__setitem__ expected 3 arguments"));
    };
    let (Some(key), Some(value)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error("TypeError", "__setitem__ expected 3 arguments"));
    };
    let data = data_of(instance, this)?;
    instance.retain(value);
    crate::executor::subscript::subscript_set(instance, data, key, value, 0)?;
    Ok(instance.retain(instance.singletons().none()))
}

/// **`k in d`** ✓（内层字典的 `__contains__` ✓；`iter.rs` 的 `type_lookup` 走这条路 ✓）。
///
/// # Safety
/// 契约同 [`defaultdict_getitem`] ✓。
pub unsafe fn defaultdict_contains(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__contains__ expected 2 arguments"));
    };
    let Some(item) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "__contains__ expected 2 arguments"));
    };
    let data = data_of(instance, this)?;
    let outcome = crate::executor::iter::contains(instance, data, item, 0)?;
    Ok(instance.new_bool(outcome))
}

/// **`d.clear()`** ✓（内层字典的 `clear` ✓ —— 一处真相 ✓）。
///
/// # Safety
/// 契约同 [`defaultdict_getitem`] ✓。
pub unsafe fn defaultdict_clear(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "clear expected 1 argument"));
    };
    let data = data_of(instance, this)?;
    crate::builtin::dict::dict_clear_native(instance, Some(data), &[], &[])?;
    Ok(instance.retain(instance.singletons().none()))
}

/// 取／建 `defaultdict` 类型（**幂等** ✓；`_collections` 装配时调用 ✓）。
pub fn build_defaultdict_type(instance: &Instance) -> NonNull<TypeObject> {
    if let Some(existing) = instance.type_named("defaultdict") {
        return existing;
    }
    let ty = instance.alloc_type_raw(
        "defaultdict",
        core::mem::size_of::<AttributeObject>(),
        AttributeObject::slots()
            .with_new(defaultdict_new)
            .with_getattr(defaultdict_getattr),
    );
    if let Some(object_type) = instance.type_named("object") {
        assert!(
            instance.register_bases(ty, vec![object_type]).is_some(),
            "OM-13：defaultdict 的 MRO 应当可线性化"
        );
    }
    if let Some(namespace) = instance.type_namespace(ty.cast()) {
        let methods: [(&'static str, NativeFn); 4] = [
            ("__getitem__", defaultdict_getitem),
            ("__setitem__", defaultdict_setitem),
            ("__contains__", defaultdict_contains),
            ("clear", defaultdict_clear),
        ];
        for (name, handler) in methods {
            let function = make_native(instance, name, handler);
            instance.dict_set(namespace, name, function);
            instance.release(function);
        }
        // 参照的 `collections.defaultdict.__module__` 就是 `'collections'` ✓（`typing._alias` 会读它 ✓）。
        let module = instance.new_str("collections");
        instance.dict_set(namespace, "__module__", module);
        instance.release(module);
    }
    ty
}
