//! **`property` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `property_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`bound_property`、`dict_getattr` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::builtin_objects::{BuiltinFunctionObject, MethodObject, NativeFn, PropertyObject, PropertySlot};
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::bound_property;

pub(crate) unsafe fn property_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<PropertyObject>() };
    // **三个字段都要走** ✓（第 186 轮加 `fset`／`fdel` ⇒ T/C **必须同步** ✓ —— 第 158 轮的教训 ✓）。
    visit(object.fget().as_ptr());
    visit(object.fset().as_ptr());
    visit(object.fdel().as_ptr());
}

pub(crate) unsafe fn property_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<PropertyObject>() };
    // SAFETY: 三份引用都由本对象持有。
    unsafe { instance.release_object(object.fget().as_ptr()) };
    unsafe { instance.release_object(object.fset().as_ptr()) };
    unsafe { instance.release_object(object.fdel().as_ptr()) };
}

pub unsafe fn property_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **`property(fget, fset, fdel, doc)`**（第 186 轮：无参也给 `fget=None` 的 property ✓；
    // `fset`／`fdel` 缺省用 `None` 单例 ✓；`doc` 本层**不收** ✗ —— 已登记的偏差 ✓）。
    let none = instance.singletons().none();
    let pick = |index: usize| -> NonNull<Header> {
        match args.get(index) {
            Some(given) => {
                instance.retain(*given);
                *given
            }
            None => instance.retain(none),
        }
    };
    let fget = pick(0);
    let fset = pick(1);
    let fdel = pick(2);
    let ty = instance
        .type_named("property")
        .expect("引导期已登记 property 类型");
    Ok(instance
        .alloc_payload(PropertyObject::new(ty, fget, fset, fdel))
        .cast::<Header>())
}

/// `p.getter(f)`／`p.setter(f)`／`p.deleter(f)` 的**共用实现** ✓（返回新 property ✓）。
pub(crate) unsafe fn property_bind_native(
    slot: PropertySlot,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let owner = bound_property(instance, bound)?;
    // **无参也合法** ✓（第 186 轮实测：参照的 `property.getter(fget=None)` 默认就是 `None` ✓）。
    let function = match args.first() {
        Some(given) => *given,
        None => instance.singletons().none(),
    };
    // SAFETY: owner 是这个类型的存活对象（绑定契约 ✓）。
    let existing = unsafe { &*owner.as_ptr().cast::<PropertyObject>() };
    let (fget, fset, fdel) = match slot {
        PropertySlot::Getter => (function, existing.fset(), existing.fdel()),
        PropertySlot::Setter => (existing.fget(), function, existing.fdel()),
        PropertySlot::Deleter => (existing.fget(), existing.fset(), function),
    };
    instance.retain(fget);
    instance.retain(fset);
    instance.retain(fdel);
    let ty = instance
        .type_named("property")
        .expect("引导期已登记 property 类型");
    Ok(instance
        .alloc_payload(PropertyObject::new(ty, fget, fset, fdel))
        .cast::<Header>())
}

pub(crate) unsafe fn property_getter_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    unsafe { property_bind_native(PropertySlot::Getter, bound, args, instance) }
}

pub(crate) unsafe fn property_setter_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    unsafe { property_bind_native(PropertySlot::Setter, bound, args, instance) }
}

pub(crate) unsafe fn property_deleter_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    unsafe { property_bind_native(PropertySlot::Deleter, bound, args, instance) }
}

/// **`property.__get__`** ✓（描述符协议；第 279 轮接线）—— `@property` 从"只能构造"变成**真的生效** ✓。
///
/// **为什么必须有它** ✗：`attribute_lookup` 的 ③ 段只认**类型字典里的 `__get__`**
/// （`instance.type_lookup(found_ty, "__get__")` ✓），而本层的内建描述符类型**从来没登记过**它 ✗
/// ⇒ `spec.has_location` 一类**属性**返回的是 **property 对象本身** ✗
/// （实测：`Lib/importlib/_bootstrap.py:793` 因此把 property 对象当真值判 ⇒ 撞"真假判定未接线" ✗）。
///
/// 调用形态由描述符分支给出 ✓：`this` ＝ **那个 property 对象**（借用 ✓）、
/// `args` ＝ `[obj_or_None, owner]`（借用 ✓）—— 与参照的 `property.__get__(self, obj, owner=None)` 同形 ✓。
/// 口径照参照：`obj is None`（类级访问）⇒ 交出 **property 自己** ✓（新引用 ✓）；
/// 否则调 `fget(obj)` ✓；没有 `fget` ⇒ `AttributeError` ✓。
pub unsafe fn property_descriptor_get(
    instance: &Instance,
    this: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(property) = this else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__get__' for 'property' objects doesn't apply to a 'NoneType' object",
        ));
    };
    let target = args.first().copied();
    let class_level = match target {
        Some(value) => value == instance.singletons().none(),
        None => true,
    };
    if class_level {
        // SAFETY: `this` 是本次调用借来的存活对象。
        unsafe { instance.incref_object(property.as_ptr()) };
        return Ok(property);
    }
    // SAFETY: `this` 是本次调用借来的存活对象。
    let fget = unsafe { &*property.as_ptr().cast::<PropertyObject>() }.fget();
    if fget == instance.singletons().none() {
        // 参照的消息带属性名与类名（本层的 property 对象**不存名字** ✗ ⇒ 如实给通用消息 ✓，
        // 该差异在"未接线"清单里，不进差异清单 ✓ —— `MS-19` 的适用范围 ✓）。
        return Err(instance.raise_builtin_error("AttributeError", "property has no getter"));
    }
    let target = target.expect("上面判过 `obj` 不是 `None`");
    // `fget` 是**普通函数** ⇒ 绑到 `obj` 上（`bound_self` 是**借用** ✓，见 `call_callable` 的契约）。
    crate::executor::call::call_callable(instance, fget, Some(target), Vec::new(), Vec::new(), 0)
}

/// `property` 的**方法面**（第 186 轮）：`fget`／`fset`／`fdel` 取值 ✓；
/// `getter`／`setter`／`deleter` 返回**绑定**的 native ✓（照 `dict_getattr` 那套 ✓）。
pub unsafe fn property_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<PropertyObject>() };
    match name {
        "fget" => {
            unsafe { instance.incref_object(object.fget().as_ptr()) };
            return Some(object.fget());
        }
        "fset" => {
            unsafe { instance.incref_object(object.fset().as_ptr()) };
            return Some(object.fset());
        }
        "fdel" => {
            unsafe { instance.incref_object(object.fdel().as_ptr()) };
            return Some(object.fdel());
        }
        "getter" | "setter" | "deleter" => {}
        _ => return None,
    }
    let handler: NativeFn = match name {
        "getter" => property_getter_native,
        "setter" => property_setter_native,
        _ => property_deleter_native,
    };
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "property",
        core::cell::Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}
