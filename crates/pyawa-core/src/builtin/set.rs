//! **`set` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `set_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`bound_set`、`container_contains_native`、`tuple_clear`、`tuple_traverse` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::cell::Cell;
use crate::builtin_objects::{BuiltinFunctionObject, DictObject, ListObject, MethodObject, NativeFn, SetObject, TupleObject};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::{bound_set, container_contains_native};















/// **`set` 的方法面**（第 146 轮）：`add`／`discard`／`update`／`copy` ✓ —— 与 `str`／`list`／`dict`
/// 同一套路 ✓（返回绑定的 `MethodObject` ✓）。相等性按 `values_equal`（引擎统一口径 ✓）。
pub unsafe fn set_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "add" => set_add_native,
        "__contains__" => container_contains_native,
        "discard" => set_discard_native,
        "update" => set_update_native,
        "copy" => set_copy_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "set",
        Cell::new(handler),
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

/// 集合里有没有与 `item` 相等的元素 ✓（引擎统一比较口径 ✓）。
pub(crate) fn set_contains(
    instance: &Instance,
    set: NonNull<Header>,
    item: NonNull<Header>,
) -> Option<usize> {
    // SAFETY: 调用方保证 set 是本实例的 `set`。
    let object = unsafe { &*set.as_ptr().cast::<SetObject>() };
    object.position_of(|candidate| crate::executor::values_equal_public(instance, candidate, item))
}

pub(crate) fn set_add_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let set = bound_set(instance, bound)?;
    let Some(item) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "add() takes exactly one argument (0 given)",
        ));
    };
    if set_contains(instance, set, *item).is_none() {
        instance.retain(*item);
        instance.set_insert_raw(set, *item);
    }
    Ok(instance.retain(instance.singletons().none()))
}

pub(crate) fn set_discard_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let set = bound_set(instance, bound)?;
    let Some(item) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "discard() takes exactly one argument (0 given)",
        ));
    };
    if let Some(index) = set_contains(instance, set, *item) {
        // SAFETY: 上面刚确认是本实例的 set。
        let object = unsafe { &*set.as_ptr().cast::<SetObject>() };
        if let Some(removed) = object.remove_at(index) {
            // 被移除的那份引用由本对象持有 ⇒ 归还引擎 ✓
            unsafe { instance.release_object(removed.as_ptr()) };
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}

pub(crate) fn set_update_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let set = bound_set(instance, bound)?;
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "update() takes exactly one argument (0 given)",
        ));
    };
    let items = match instance.iterable_items(*iterable) {
        Some(items) => items,
        None => {
            return Err(instance.raise_builtin_error("TypeError", "object is not iterable"))
        }
    };
    for item in items {
        if set_contains(instance, set, item).is_none() {
            instance.retain(item);
            instance.set_insert_raw(set, item);
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}

pub(crate) fn set_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let set = bound_set(instance, bound)?;
    // `set_items` 是**借用** ✓ ⇒ 每项先还一份交给新集合 ✓
    let items: Vec<NonNull<Header>> = instance
        .set_items(set)
        .unwrap_or_default()
        .into_iter()
        .map(|item| instance.retain(item))
        .collect();
    Ok(instance.new_set(items))
}



/// 见 [`tuple_traverse`]。
pub(crate) unsafe fn set_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}

/// 见 [`tuple_clear`]。
pub(crate) unsafe fn set_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    // **同 `list_clear`** ✗（第 206 轮修复 ✓）。
    for value in core::mem::take(&mut *object.items.borrow_mut()) {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

/// `set()`：空集合。
pub unsafe fn set_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let empty = instance
        .alloc(SetObject::new(class, core::cell::RefCell::new(Vec::new())))
        .into_raw()
        .cast::<Header>();
    let Some(source) = args.first() else {
        return Ok(empty);
    };
    // **`set(可迭代)`** ✓（第 197 轮，补上已登记的缺口 ✓）：容器走快路 ✓，其余走
    // `iter_object` ＋ `advance_iterator`（**一处真相** ✓）。
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let mut borrowed = true;
    match instance.type_name(instance.type_of(*source)).as_str() {
        "list" => items = unsafe { &*source.as_ptr().cast::<ListObject>() }.items().to_vec(),
        "tuple" => items = unsafe { &*source.as_ptr().cast::<TupleObject>() }.items().to_vec(),
        "set" | "frozenset" => {
            items = unsafe { &*source.as_ptr().cast::<SetObject>() }.items().to_vec()
        }
        "dict" => {
            items = unsafe { &*source.as_ptr().cast::<DictObject>() }
                .entries()
                .into_iter()
                .map(|(key, _)| key)
                .collect()
        }
        _ => {
            borrowed = false;
            let iterator = instance.iter_object(*source)?;
            loop {
                match instance.advance_iterator(iterator)? {
                    Some(item) => items.push(item),
                    None => break,
                }
            }
            unsafe { instance.release_object(iterator.as_ptr()) };
        }
    }
    // **去重靠 `set_contains` ＋ `set_insert_raw`** ✓（与 `set.add` **同一处** ✓）。
    for item in items {
        if set_contains(instance, empty, item).is_some() {
            if !borrowed {
                unsafe { instance.release_object(item.as_ptr()) };
            }
            continue;
        }
        if borrowed {
            instance.retain(item);
        }
        instance.set_insert_raw(empty, item);
    }
    Ok(empty)
}

/// `set` 的 `repr`：空是 `set()`、否则 `{a, b}`（实测）。
pub unsafe fn set_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    let items = object.items();
    if items.is_empty() {
        return Ok("set()".to_owned());
    }
    let mut text = String::from("{");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(instance, *item)?);
    }
    text.push('}');
    Ok(text)
}
