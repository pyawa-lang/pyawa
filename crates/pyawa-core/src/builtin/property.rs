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
    class: NonNull<crate::TypeObject>,
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
    // **用调用方给的那个类** ✓（`P3-17` 修 ✗）：`class abstractproperty(property)` 的子类实例
    // 必须带**自己的类型**（参照 `type(abstractproperty(f)) is abstractproperty` ✓）；先前硬取
    // `type_named("property")` ✗ ⇒ 子类实例报成基类 ✗（布局已由 `classes.rs` 的 AB-58 表沿用 ✓）。
    let ty = class;
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
    // **返回与接收者同类型的 property** ✓（参照实测：`abstractproperty(...).setter(f)` 仍是
    // `abstractproperty` ✓；`class abstractproperty(property)` 在 `Lib/abc.py` ✓）——
    // 先前硬取 `type_named("property")` ✗ ⇒ 子类经 `.getter/.setter/.deleter` 后**降级成基类** ✗。
    let ty = instance.type_of(owner);
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
    // **布局核对**：`class X(property)` 这类 Python 子类实例目前是**通用 `AttributeObject` 布局** ✗
    // ⇒ 不能当 `PropertyObject` 读载荷（否则读垃圾 ⇒ 未定义行为／中止 ✗）⇒ 如实报未接线 ✓。
    if !crate::builtin_objects::builtin_wrapper_layout_ok(
        instance,
        property,
        "property",
        core::mem::size_of::<PropertyObject>(),
    ) {
        return Err(crate::builtin_objects::wrapper_layout_unsupported(
            instance, property, "property",
        ));
    }
    // **实参个数照参照校验** ✓（`property.__get__()` ⇒ `__get__ expected at least 1 argument, got 0`
    // ✓；3 个 ⇒ `… at most 2 arguments, got 3` ✓）。
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__get__ expected at least 1 argument, got 0",
        ));
    }
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("__get__ expected at most 2 arguments, got {}", args.len()),
        ));
    }
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
    let target = target.expect("上面判过 `obj` 不是 `None`");
    if fget == instance.singletons().none() {
        return Err(instance.raise_builtin_error(
            "AttributeError",
            &format!("{} has no getter", property_display(instance, property, target)),
        ));
    }
    // **一律按"位置实参"交出去** ✓（task-14 真 bug 修 ✗）：先前用 `bound_self` 绑 ✗ ⇒ 对**普通函数**
    // `fget` 恰好对 ✓，但对**已绑定的可调用**（`MethodObject`／`builtin_function_or_method` ✗ ——
    // `namedtuple` 的字段回退实现 `property(_itemgetter(index))` 正是它 ✓）⇒ `call_callable` 会把
    // 传进来的 `bound_self` **丢掉**、换成那个方法自己的 `this` ✓ ⇒ 原生收到 `args=[]` ✓
    // ⇒ 实测 `Point(1,2).x` 报 `itemgetter expected 1 argument, got 0` ✗。
    // `call_value` 是公开入口 ✓（自己按契约拷贝实参 ✓），三种 `fget` 形态都合 ✓。
    crate::executor::call::call_value(instance, fget, &[target], &[])
}

/// **描述符的错误文案前缀**：`property '<名字>' of '<类型>' object` ✓。
///
/// 参照把属性名存在 property **对象自己**身上（`__set_name__` ✓）；本层的 `PropertyObject`
/// **不存名字** ✗ ⇒ 这里退一步：在**接收者类型的 MRO 类型字典**里按"值指针就是它"反查属性名 ✓
/// （正常访问路径 100% 命中 ✓；匿名 `property()` 反查不到 ⇒ 参照也是那句不带名字的文案 ✓）。
/// **如实登记**：把 `C.__dict__['x']` 这个 property 强行 `__set__` 到**别的类型**实例上时，
/// 参照仍报 `'x'`（名字存在对象上 ✓），本层反查不到 ⇒ 少个名字 ✓（不进差异清单，属未覆盖形态 ✓）。
fn property_display(
    instance: &Instance,
    property: NonNull<Header>,
    receiver: NonNull<Header>,
) -> String {
    // SAFETY: receiver 是本次调用借来的存活对象。
    let object_type = unsafe { receiver.as_ref() }.ty();
    let type_name = instance.type_name(object_type);
    // SAFETY: 类型对象由注册表持有，MRO 里的类型同样存活。
    for entry in unsafe { object_type.as_ref() }.mro() {
        // SAFETY: 同上。
        let Some(mapping) = (unsafe { entry.as_ref() }).dict() else {
            continue;
        };
        // SAFETY: mapping 由类型对象持有。
        let dict = unsafe { &*mapping.as_ptr().cast::<crate::builtin_objects::DictObject>() };
        let found = dict
            .entries()
            .into_iter()
            .find(|(_, value)| *value == property);
        if let Some((key, _)) = found {
            if let Some(name) = instance.text_of(key) {
                return format!("property '{name}' of '{type_name}' object");
            }
        }
    }
    format!("property of '{type_name}' object")
}

/// **`property.__set__(obj, value)`** ✓（`P3-17` 的**数据描述符写**）。
///
/// 参照语义：有 `fset` ⇒ `fset(obj, value)` ✓、返回 `None` ✓；没有 ⇒
/// `AttributeError: property '<名>' of '<类型>' object has no setter` ✓
/// （本层只能靠[`property_display`]反查名字 ✓）。**关键**：`instance_attribute_set` 的数据描述符
/// 分支会在**写实例字典之前**调这里 ✓ ⇒ `@x.setter` 不再把值写进实例字典 ✓。
pub unsafe fn property_descriptor_set(
    instance: &Instance,
    this: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(property) = this else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__set__' for 'property' objects doesn't apply to a 'NoneType' object",
        ));
    };
    // **布局核对**（同 `__get__` ✓）。
    if !crate::builtin_objects::builtin_wrapper_layout_ok(
        instance,
        property,
        "property",
        core::mem::size_of::<PropertyObject>(),
    ) {
        return Err(crate::builtin_objects::wrapper_layout_unsupported(
            instance, property, "property",
        ));
    }
    let Some(target) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__set__' needs an argument",
        ));
    };
    let Some(value) = args.get(1).copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__set__' needs a value",
        ));
    };
    // SAFETY: `this` 是本次调用借来的存活对象。
    let fset = unsafe { &*property.as_ptr().cast::<PropertyObject>() }.fset();
    if fset == instance.singletons().none() {
        return Err(instance.raise_builtin_error(
            "AttributeError",
            &format!("{} has no setter", property_display(instance, property, target)),
        ));
    }
    // 同 `__get__`：**按位置实参**交出去 ✓（覆盖普通函数／已绑定可调用／原生三种形态 ✓）。
    let returned = crate::executor::call::call_value(instance, fset, &[target, value], &[])?;
    // SAFETY: returned 是本次调用的新引用，`__set__` 返回的是 `None`（丢掉 setter 的返回值 ✓）。
    unsafe { instance.release_object(returned.as_ptr()) };
    Ok(instance.retain(instance.singletons().none()))
}

/// **`property.__delete__(obj)`** ✓（`P3-17` 的数据描述符删）。
///
/// 参照语义：有 `fdel` ⇒ `fdel(obj)` ✓；没有 ⇒ `AttributeError: property '<名>' of '<类型>' object has no deleter` ✓。
pub unsafe fn property_descriptor_delete(
    instance: &Instance,
    this: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(property) = this else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__delete__' for 'property' objects doesn't apply to a 'NoneType' object",
        ));
    };
    // **布局核对**（同 `__get__` ✓）。
    if !crate::builtin_objects::builtin_wrapper_layout_ok(
        instance,
        property,
        "property",
        core::mem::size_of::<PropertyObject>(),
    ) {
        return Err(crate::builtin_objects::wrapper_layout_unsupported(
            instance, property, "property",
        ));
    }
    let Some(target) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__delete__' needs an argument",
        ));
    };
    // SAFETY: `this` 是本次调用借来的存活对象。
    let fdel = unsafe { &*property.as_ptr().cast::<PropertyObject>() }.fdel();
    if fdel == instance.singletons().none() {
        return Err(instance.raise_builtin_error(
            "AttributeError",
            &format!("{} has no deleter", property_display(instance, property, target)),
        ));
    }
    let returned = crate::executor::call::call_value(instance, fdel, &[target], &[])?;
    // SAFETY: returned 是本次调用的新引用（`__delete__` 返回 `None` ✓）。
    unsafe { instance.release_object(returned.as_ptr()) };
    Ok(instance.retain(instance.singletons().none()))
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
