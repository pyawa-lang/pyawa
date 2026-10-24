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
        crate::executor::values::values_equal_public(instance, item, *wanted)
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
        .filter(|item| crate::executor::values::values_equal_public(instance, *item, *wanted))
        .count() as i64;
    Ok(instance.new_int(count))
}
/// `reverse()`：就地反转 ✓（只动顺序，不碰引用 ✓）。
/// **`list.sort(*, key=None, reverse=False)`** ✓（task-23 ✓）：**原地**稳定排序 ✓，回 `None` ✓。
///
/// 关键字**只认** `key`／`reverse` ✓（参照里二者是**仅关键字** ✓ ⇒ 位置实参报
/// `sort() takes no positional arguments` ✓）；比较走 [`Instance::order_of`] ✓ —— 与 `sorted`
/// **同一处真相** ✓（连"比不了就报 `'<' not supported between instances of …`"那条口径也照搬 ✓）。
/// `Lib/heapq.py` 的 `nsmallest`／`nlargest` 就靠它 ✓（task-23 的立据 ✓）。
pub(crate) fn list_sort_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    if !args.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "sort() takes no positional arguments"));
    }
    let mut key: Option<NonNull<Header>> = None;
    let mut reverse = false;
    for (name, value) in kwargs {
        let Some(text) = instance.text_of(*name) else {
            return Err(instance.raise_builtin_error("TypeError", "keywords must be strings"));
        };
        match text {
            "key" => key = Some(*value),
            "reverse" => reverse = instance.truth_of(*value),
            _ => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("'{}' is an invalid keyword argument for sort()", text),
                ))
            }
        }
    }
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    let items = object.items().to_vec();
    // 装饰：key 只算**一次** ✓（与 `sorted` 同口径 ✓；`key=None` 时就用元素本身 ✓）。
    // **引用账**（`OM-20` ✓）：每项由本函数各持**两份** —— 一份当"将来交给列表的元素" ✓、
    // 一份当"排序用的键" ✓（`key=None` 时键＝`retain(元素)` ✓，所以最后**对称各还一份** ✓）。
    let mut decorated: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::with_capacity(items.len());
    for item in items {
        // SAFETY: item 由列表持有、本函数活着期间有效 ⇒ 留一份给自己用 ✓。
        unsafe { instance.incref_object(item.as_ptr()) };
        let item_key = match key {
            Some(callable) if callable != instance.singletons().none() => {
                crate::executor::call::call_value(instance, callable, &[item], &[])?
            }
            // SAFETY: 同一条：键那一份自己留 ✓。
            _ => {
                unsafe { instance.incref_object(item.as_ptr()) };
                item
            }
        };
        decorated.push((item, item_key));
    }
    let mut failure: Option<String> = None;
    decorated.sort_by(|left, right| match instance.order_of(left.1, right.1) {
        Some(ordering) => ordering,
        None => {
            failure.get_or_insert_with(|| {
                format!(
                    "'<' not supported between instances of '{}' and '{}'",
                    instance.type_name(instance.type_of(left.1)),
                    instance.type_name(instance.type_of(right.1))
                )
            });
            core::cmp::Ordering::Equal
        }
    });
    if reverse {
        decorated.reverse();
    }
    // 写回：**原地** ✓ —— 先逐项弹出旧引用并归还 ✓，再按新序 `append` ✓（元素那一份**转交**列表 ✓），
    // 键那一份是临时 ⇒ 还掉 ✓（两边各正好一份 ✓ ⇒ 不重不漏 ✓）。
    while let Some(old) = object.pop_last() {
        // SAFETY: 旧引用由列表持有 ⇒ 归还引擎 ✓。
        unsafe { instance.release_object(old.as_ptr()) };
    }
    for (item, item_key) in decorated {
        object.append(item);
        // SAFETY: 键那一份由本函数持有 ⇒ 归还 ✓。
        unsafe { instance.release_object(item_key.as_ptr()) };
    }
    if let Some(message) = failure {
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    Ok(instance.retain(instance.singletons().none()))
}

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
    let index = object.position_where(|item| crate::executor::values::values_equal_public(instance, item, *target));
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
        "__len__" => crate::builtin_objects::container_len_native,
        "extend" => list_extend_native,
        "pop" => list_pop_native,
        "insert" => list_insert_native,
        "index" => list_index_native,
        "count" => list_count_native,
        "reverse" => list_reverse_native,
        "sort" => list_sort_native,
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
/// **`list.append` 的类型级描述符**（task-23 ✓）：`multiprocessing/reduction.py:219` 的
/// `type(list.append)` 是**导入期**取用 ✓ ⇒ 类型字典里要有这个名字 ✓。
/// 类型字典取到的是**未绑定**的原生函数 ✓ ⇒ 接收者是**第一个实参** ✓（与 `int.bit_length` 同一条路 ✓）。
///
/// 文案照参照实测（3.14.4）：无参 ⇒ `unbound method list.append() needs an argument`；
/// 接收者不是 list（**子类可以** ✓）⇒ `descriptor 'append' for 'list' objects doesn't apply to a '<类型>' object`；
/// 关键字 ⇒ `list.append() takes no keyword arguments`；多余实参 ⇒ `list.append() takes exactly one argument (N given)`。
pub(crate) fn list_append_descriptor(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(owner) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "unbound method list.append() needs an argument",
        ));
    };
    let list_type = instance.type_named("list").expect("list 已登记");
    if !instance.is_subtype(instance.type_of(owner), list_type) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "descriptor 'append' for 'list' objects doesn't apply to a '{}' object",
                instance.type_name(instance.type_of(owner))
            ),
        ));
    }
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "list.append() takes no keyword arguments",
        ));
    }
    let given = args.len() - 1;
    if given != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("list.append() takes exactly one argument ({given} given)"),
        ));
    }
    list_append_native(instance, Some(owner), &args[1..], &[])
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
/// **`list.pop([index])`**（task-33）：无参弹**末项** ✓；带下标按下标弹 ✓（负数从尾部数 ✓）。
///
/// 文案逐条照参照 3.14.4 实测：
/// - 空表（两种形态）⇒ `IndexError: pop from empty list` ✓（`[].pop(0)` 也是这条 ✓）
/// - 非空越界（含负下标越过头部）⇒ `IndexError: pop index out of range` ✓
/// - 非整数（且无 `__index__`）⇒ `TypeError: '<类型>' object cannot be interpreted as an integer` ✓
/// - 关键字实参 ⇒ `TypeError: list.pop() takes no keyword arguments` ✓
/// - 位置实参多于 1 个 ⇒ `TypeError: pop expected at most 1 argument, got N` ✓
///
/// 下标的整数面走 `PyNumber_Index` 两步：`int` 快路径 ✓，其次 `__index__` 协议 ✓。
pub(crate) fn list_pop_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "list.pop() takes no keyword arguments",
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("pop expected at most 1 argument, got {}", args.len()),
        ));
    }
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    let Some(index_arg) = args.first() else {
        return match object.pop_last() {
            Some(item) => Ok(item),
            None => Err(instance.raise_builtin_error("IndexError", "pop from empty list")),
        };
    };
    let length = object.len();
    if length == 0 {
        return Err(instance.raise_builtin_error("IndexError", "pop from empty list"));
    }
    let index = match list_pop_index(instance, *index_arg)? {
        Some(index) => index,
        None => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "'{}' object cannot be interpreted as an integer",
                    instance.type_name(instance.type_of(*index_arg))
                ),
            ))
        }
    };
    let Some(index) = index else {
        // 大整数下标：一定在表外（表长装得下 `usize` ⇒ 不可能有这么大的合法下标 ✓）
        return Err(instance.raise_builtin_error("IndexError", "pop index out of range"));
    };
    let position = if index < 0 {
        index + length as i64
    } else {
        index
    };
    if position < 0 || position as usize >= length {
        return Err(instance.raise_builtin_error("IndexError", "pop index out of range"));
    }
    object
        .remove_at(position as usize)
        .ok_or_else(|| instance.raise_builtin_error("IndexError", "pop index out of range"))
}

/// `list.pop` 的下标转换（`PyNumber_Index` 的最小面 ✓）：
/// - `int`（含 `bool`）⇒ 快路径 ✓；
/// - 装不进 `i64` 的大整数 ⇒ `Some(None)`（调用方按"越界"报 ✓）；
/// - 有 `__index__` ⇒ 调它、结果必须是 `int` ✓；
/// - 其余 ⇒ `None`（调用方报 "cannot be interpreted as an integer" ✓）。
fn list_pop_index(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<Option<Option<i64>>, crate::ExecError> {
    if let Some(integer) = instance.int_of(value) {
        return Ok(Some(integer.to_i64().or(None)));
    }
    if let Some(method) = crate::executor::attribute_optional(instance, value, "__index__")? {
        let outcome = crate::executor::call_value(instance, method, &[], &[]);
        instance.release(method);
        let result = outcome?;
        let converted = instance.int_of(result);
        let type_name = instance.type_name(instance.type_of(result));
        instance.release(result);
        let Some(integer) = converted else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("__index__ returned non-int (type {type_name})"),
            ));
        };
        return Ok(Some(integer.to_i64().or(None)));
    }
    Ok(None)
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
        text.push_str(&crate::executor::protocol::element_repr(instance, *item)?);
    }
    text.push(']');
    instance.leave_repr(ptr as usize);
    Ok(text)
}
