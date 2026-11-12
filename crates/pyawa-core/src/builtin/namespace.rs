//! **`types.SimpleNamespace`**（round 113）：`Lib/types.py:20` 的
//! `SimpleNamespace = type(sys.implementation)` ✓ —— 参照里 `sys.implementation` 是
//! `types.SimpleNamespace` 的**实例**（`repr` 为 `namespace(name=…)` ✓）。
//!
//! 本层先前把 `sys.implementation` 造成**一个类型对象**（`new_attribute_type("sys.implementation")`）
//! ✗ ⇒ `types.SimpleNamespace` 变成那个类型 ✗、`types.SimpleNamespace(a=1)` 报
//! `sys.implementation() takes no arguments` ✗（`test_pprint` 的 SimpleNamespace 三格 ✓）。
//!
//! 口径：载荷 `AttributeObject`（实例属性字典 ✓）；`new` 无位置实参、`__init__` 收关键字并写进
//! 属性字典 ✓；`repr` 照参照 `namespace(k=v, …)`（子类用子类名 ✓）；空实例 ⇒ `namespace()` ✓。

use crate::HotId;
use core::cell::RefCell;
use core::ptr::NonNull;

use crate::builtin_objects::{AttributeObject, BuiltinFunctionObject, NativeFn};
use crate::executor::ExecError;
use crate::{Header, Instance, TypeObject};

/// 造一个原生可调用对象（**新引用** ✓；与 `defaultdict.rs` 同一做法 ✓）。
fn make_native(instance: &Instance, name: &'static str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance.hot_type(HotId::BuiltinFunctionOrMethod)
        .expect("builtin_function_or_method 在引导期已登记");
    instance
        .alloc(BuiltinFunctionObject::new(ty, name, core::cell::Cell::new(handler)))
        .into_raw()
        .cast::<Header>()
}

/// 取实例属性字典（**借用** ✓）。
fn attributes_of(instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
    // **实例字典走 `OM-14` 那格**（与普通用户类一致 ✓）：`AttributeObject` 载荷的 attributes 留空 ✓，
    // 字典挂在头部（`mark_external_instance_dict` ✓）⇒ 子类（`class AdvancedNamespace(...)`）
    // 也自动同构 ✓（内联/外挂两套混用会分叉 ✗）。
    crate::mounted_instance_dict(instance, object)
}

/// **`types.SimpleNamespace` 的类型对象**（每实例一个 ✓）。
pub fn build_simple_namespace_type(instance: &Instance) -> NonNull<TypeObject> {
    if let Some(existing) = instance.hot_type(HotId::SimpleNamespace) {
        return existing;
    }
    let ty = instance.alloc_type_raw(
        "SimpleNamespace",
        core::mem::size_of::<AttributeObject>(),
        AttributeObject::slots()
            .with_new(namespace_new)
            .with_repr(namespace_repr),
    );
    if let Some(object_type) = instance.hot_type(HotId::Object) {
        let _ = instance.register_bases(ty, vec![object_type]);
    }
    // **实例字典外挂**（`OM-14`）：与用户类一致 ✓（见 `attributes_of` 的注释 ✓）。
    // SAFETY: ty 刚建、由注册表持有。
    unsafe { ty.as_ref() }.mark_external_instance_dict();
    // 命名空间里会写 `__module__` ⇒ 二元运算的用户-dunder 预筛位置上（round 248 性能，A2-3 ✓）。
    instance.mark_dunder_capable(ty);
    if let Some(namespace) = instance.type_namespace(ty.cast()) {
        let module = instance.new_str("types");
        instance.dict_set(namespace, "__module__", module);
        instance.release(module);
        let init = make_native(instance, "__init__", namespace_init_native);
        instance.dict_set(namespace, "__init__", init);
        instance.release(init);
    }
    ty
}

/// `SimpleNamespace` 的 `new`：**不收位置实参** ✓（关键字归 `__init__` ✓）。
///
/// # Safety
/// 由 `new` 槽契约保证。
pub unsafe fn namespace_new(
    class: NonNull<TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "SimpleNamespace() takes no positional arguments",
        ));
    }
    Ok(instance
        .alloc(AttributeObject::new(class, RefCell::new(None)))
        .into_raw()
        .cast::<Header>())
}

/// `SimpleNamespace.__init__(**kwargs)`：把关键字写进属性字典 ✓（参照的 `__dict__` ✓）。
fn namespace_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // **两种调用形态**：显式 `ns.__init__(**kw)`（`bound` 是 self ✓）与类型实例化后的
    // `type_call`（`bound=None`、self 在 `args[0]` ✓）—— 与 `container_receiver` 同一口径 ✓。
    let (receiver, rest) = crate::builtin_objects::container_receiver(bound, args);
    if !rest.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "SimpleNamespace() takes no positional arguments",
        ));
    }
    let Some(this) = receiver else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__init__' needs an argument",
        ));
    };
    for (key, value) in kwargs {
        let Some(text) = instance.text_of(*key) else {
            return Err(instance.raise_builtin_error("TypeError", "keywords must be strings"));
        };
        let text = text.to_owned();
        // **走统一的属性写通道** ✓（`instance_attribute_set` 会按需建外挂字典 ✓）。
        // 它**接管**一份引用 ✓ ⇒ 关键字是借用视图 ⇒ 先添一份再交 ✓（多放会悬垂 ✗）。
        // SAFETY: value 由调用方保证存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        crate::executor::protocol::instance_attribute_set(instance, this, &text, *value, 0)?;
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// `repr(ns)`：`namespace(k=v, …)` ✓（子类用子类名；空实例 ⇒ `namespace()` ✓）。
///
/// # Safety
/// 由 `repr` 槽契约保证。
pub unsafe fn namespace_repr(
    ptr: *mut Header,
    instance: &Instance,
) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 是本类型族（AttributeObject 载荷）的存活对象。
    let header = unsafe { &*ptr };
    // SAFETY: 类型名由注册表持有。
    let type_name = unsafe { header.ty().as_ref() }.name().to_owned();
    // 参照：`types.SimpleNamespace` 自己的实例 repr 用 **`namespace(...)`** ✓（子类用子类名 ✓）。
    let type_name = if type_name == "SimpleNamespace" {
        "namespace".to_owned()
    } else {
        type_name
    };
    let object = NonNull::new(ptr).expect("调用方保证非空");
    let entries = attributes_of(instance, object)
        .and_then(|attributes| instance.dict_entries(attributes))
        .unwrap_or_default();
    let mut rendered: Vec<String> = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let key_text = instance.text_of(key).unwrap_or("?").to_owned();
        rendered.push(format!("{key_text}={}", instance.object_repr(value)?));
    }
    Ok(format!("{type_name}({})", rendered.join(", ")))
}

/// **建一个填好属性的 `SimpleNamespace` 实例**（`sys.implementation` 用 ✓；新引用 ✓）。
pub fn make_simple_namespace(
    instance: &Instance,
    items: &[(&str, NonNull<Header>)],
) -> Option<NonNull<Header>> {
    let ty = build_simple_namespace_type(instance);
    let object = instance
        .alloc(AttributeObject::new(ty, RefCell::new(None)))
        .into_raw()
        .cast::<Header>();
    for (name, value) in items {
        // **统一属性写通道**（外挂字典按需建 ✓）；它接管一份引用 ⇒ 先添一份（调用方那份仍归它 ✓）。
        // SAFETY: value 由调用方保证存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        crate::executor::protocol::instance_attribute_set(instance, object, name, *value, 0).ok()?;
    }
    Some(object)
}
