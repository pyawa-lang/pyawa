//! **`list` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `list_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`bound_list`、`container_contains_native`、`str_getattr`、`tuple_clear`、`tuple_traverse` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::cell::Cell;
use crate::builtin_objects::{BuiltinFunctionObject, DictObject, ListObject, MethodObject, NativeFn, SetObject, TupleObject};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::{bound_list, container_contains_native};

/// **按下标插入**：参照里 `insert(i, x)` 的 `i` 会被**夹到 `[0, len]`** ✓（负数表从尾部数 ✓）。
pub(crate) fn list_insert_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "insert() takes exactly 2 arguments",
        ));
    }
    let length = instance.list_items(list).map(|items| items.len()).unwrap_or(0) as i64;
    let mut index = instance.int_value(args[0]).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "insert() 的下标要整数")
    })?;
    if index < 0 {
        index += length;
        if index < 0 {
            index = 0;
        }
    }
    let index = index.min(length) as usize;
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    instance.retain(args[1]);
    object.insert_at(index, args[1]);
    Ok(instance.retain(instance.singletons().none()))
}
/// `index(x)`：找不到 ⇒ `ValueError`（与参照同文 ✓）。
pub(crate) fn list_index_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(wanted) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "index() takes at least 1 argument",
        ));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    match object.position_where(|item| {
        crate::executor::values_equal_public(instance, item, *wanted)
    }) {
        Some(index) => Ok(instance.new_int(index as i64)),
        None => Err(instance.raise_builtin_error("ValueError", " is not in list")),
    }
}
/// `count(x)`：相等元素个数 ✓。
pub(crate) fn list_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(wanted) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "count() takes exactly one argument",
        ));
    };
    let items = instance.list_items(list).unwrap_or_default();
    let count = items
        .into_iter()
        .filter(|item| crate::executor::values_equal_public(instance, *item, *wanted))
        .count() as i64;
    Ok(instance.new_int(count))
}
/// `reverse()`：就地反转 ✓（只动顺序，不碰引用 ✓）。
pub(crate) fn list_reverse_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    object.reverse_items();
    Ok(instance.retain(instance.singletons().none()))
}
/// `clear()`（第 154 轮）：逐项弹出并把那份引用**归还引擎** ✓。
pub(crate) fn list_clear_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    // SAFETY: 绑定的是本实例的 list。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    while let Some(item) = object.pop_last() {
        unsafe { instance.release_object(item.as_ptr()) };
    }
    Ok(instance.retain(instance.singletons().none()))
}
/// `remove(x)`（第 154 轮）：按**值**找到第一项并摘掉 ✓（那份引用**归还引擎** ✓）；找不到报
/// `ValueError: list.remove(x): x not in list` ✓ 同文 ✓。
pub(crate) fn list_remove_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(target) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "remove() takes exactly one argument"));
    };
    // SAFETY: 绑定的是本实例的 list。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    let index = object.position_where(|item| crate::executor::values_equal_public(instance, item, *target));
    match index {
        Some(at) => {
            if let Some(removed) = object.remove_at(at) {
                unsafe { instance.release_object(removed.as_ptr()) };
            }
            Ok(instance.retain(instance.singletons().none()))
        }
        None => Err(instance.raise_builtin_error("ValueError", "list.remove(x): x not in list")),
    }
}
/// **`list` 的方法面**（第 143 轮）：照 `str_getattr` 同一套路 ✓（返回绑定的 `MethodObject` ✓）。
pub unsafe fn list_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "append" => list_append_native,
        "__contains__" => container_contains_native,
        "extend" => list_extend_native,
        "pop" => list_pop_native,
        "insert" => list_insert_native,
        "index" => list_index_native,
        "count" => list_count_native,
        "reverse" => list_reverse_native,
        "clear" => list_clear_native,
        "remove" => list_remove_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "list",
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
pub(crate) fn list_append_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(item) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "append() takes exactly one argument (0 given)",
        ));
    };
    // `append` **接管**一份引用 ⇒ 这里先还一份 ✓（实参是借来的 ✓）
    instance.retain(*item);
    instance.list_append(list, *item);
    Ok(instance.retain(instance.singletons().none()))
}
pub(crate) fn list_extend_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "extend() takes exactly one argument (0 given)",
        ));
    };
    let items = match instance.iterable_items(*iterable) {
        Some(items) => items,
        None => {
            return Err(instance.raise_builtin_error("TypeError", "object is not iterable"))
        }
    };
    for item in items {
        instance.retain(item);
        instance.list_append(list, item);
    }
    Ok(instance.retain(instance.singletons().none()))
}
pub(crate) fn list_pop_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "pop() 目前只接无参（带下标随后补）",
        ));
    }
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    match object.pop_last() {
        Some(item) => Ok(item),
        None => Err(instance.raise_builtin_error("IndexError", "pop from empty list")),
    }
}
/// 见 [`tuple_traverse`]。
pub(crate) unsafe fn list_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}
/// 见 [`tuple_clear`]。
pub(crate) unsafe fn list_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    // **必须把元素也取走** ✗（第 206 轮修复 ✓）：只释放不腾空 ⇒ 容器里留着**已释放的指针** ✗
    // ⇒ 容器**若还活着**（复活／二次 `clear` ✓）再用一次就会**再释放一次** ✗。
    for value in core::mem::take(&mut *object.items.borrow_mut()) {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}
/// `list()`：空列表。
pub unsafe fn list_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **`list(...)`**（第 184 轮：替掉"只接无参"的形态 ✗ —— `list` 这个名字改成**类型对象**之后，
    // `list(可迭代)` 就走到这里了 ✓）。
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let mut borrowed = true;
    if let Some(source) = args.first() {
        let source_ty = instance.type_name(instance.type_of(*source));
        match source_ty.as_str() {
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
                // **任何可迭代对象** ✓：走**一处真相**的 `iter_object` ＋ `advance_iterator` ✓
                // （`list(迭代器)`／`list(range(…))` 等全靠它 ✓）；这条路给的是**新引用** ✓。
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
    }
    if borrowed {
        // 容器那几条支路给的是**借用** ⇒ 逐个取一份新引用交给新列表 ✓。
        for item in &items {
            unsafe { instance.incref_object(item.as_ptr()) };
        }
    }
    Ok(
        instance
            .alloc(ListObject::new(class, core::cell::RefCell::new(items)))
            .into_raw()
            .cast::<Header>(),
    )
}
/// `list` 的 `repr`：`[a, b]`；自引用给 `[...]`（实测）。
pub unsafe fn list_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Ok("[...]".to_owned());
    }
    let items = object.items();
    let mut text = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(instance, *item)?);
    }
    text.push(']');
    instance.leave_repr(ptr as usize);
    Ok(text)
}
