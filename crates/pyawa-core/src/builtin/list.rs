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
    let Some(wanted) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "index() takes at least 1 argument",
        ));
    };
    // **区间**（round 110）：`list.index(x, start, end)` 的 start/end 照切片归一化 ✓
    //（负数加长度、夹到 `0..=len` ✓；`start > len` ⇒ 直接 ValueError ✓）。
    // **活体遍历**：`__eq__` 可能把表清空／改小（`seq_tests.test_index` 的 `EvilCmp` ✓）⇒
    // 每轮重读长度、比较期间自己持一份 ✓（先前的借用快照会悬垂 ✗）。
    // SAFETY: 绑定的是本实例的 list。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    let length = object.len() as i64;
    let (start, end) = list_range_bounds(instance, args, length, "index")?;
    let mut position = start;
    while position < end && position < object.len() as i64 {
        let item = object.item(position as usize).expect("刚查过长度，下标在范围内");
        // **富比较协议**：用户 `__eq__` 生效 ✓、异常原样传播 ✓；比较期间持一份引用 ✓。
        unsafe { instance.incref_object(item.as_ptr()) };
        let verdict = crate::executor::protocol::equality_public(instance, item, wanted);
        unsafe { instance.release_object(item.as_ptr()) };
        if verdict? {
            return Ok(instance.new_int(position));
        }
        position += 1;
    }
    Err(instance.raise_builtin_error("ValueError", " is not in list"))
}

/// `list.index(x, start, end)` 的区间归一化（round 110；与 `str.find` 家族同口径 ✓）。
/// `list.clear()`／`copy()`／`reverse()` 的**零实参**检查（round 110）：参照对多给的实参报
/// `TypeError: list.clear() takes no arguments (1 given)` ✓（官方 `list_tests.test_clear`／
/// `test_copy` 的 `assertRaises(TypeError, u.clear, None)` ✓）。
fn reject_extra_arguments(
    instance: &Instance,
    args: &[NonNull<Header>],
    method: &str,
) -> Result<(), crate::ExecError> {
    if args.is_empty() {
        return Ok(());
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!(
            "list.{method}() takes no arguments ({} given)",
            args.len()
        ),
    ))
}

fn list_range_bounds(
    instance: &Instance,
    args: &[NonNull<Header>],
    length: i64,
    method: &str,
) -> Result<(i64, i64), crate::ExecError> {
    let mut values: [Option<i64>; 2] = [None, None];
    for (index, value) in args.iter().skip(1).take(2).enumerate() {
        if instance.type_of(*value) == instance.singletons().none_type() {
            continue;
        }
        if let Some(number) = instance.int_of(*value) {
            let saturated = number.to_i64().unwrap_or_else(|| {
                if number.to_bigint().signum() < 0 {
                    i64::MIN
                } else {
                    i64::MAX
                }
            });
            values[index] = Some(saturated);
            continue;
        }
        let name = instance.type_name(instance.type_of(*value));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{method}() 的 start／end 要整数，不是 '{name}'"),
        ));
    }
    let normalize = |value: Option<i64>, default: i64| match value {
        Some(number) => {
            let shifted = if number < 0 { number + length } else { number };
            shifted.clamp(0, length)
        }
        None => default,
    };
    let start = normalize(values[0], 0);
    let end = normalize(values[1], length);
    Ok((start, end))
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
    // **富比较协议**（round 110）：`count` 和 `in` 同一口径（用户 `__eq__` 生效、异常传播 ✓）；
    // 活体遍历 ＋ 每轮重读长度（`__eq__` 改表时不悬垂 ✓）。
    // SAFETY: 绑定的是本实例的 list。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    let mut count = 0i64;
    let mut index = 0usize;
    while index < object.len() {
        let item = object.item(index).expect("刚查过长度，下标在范围内");
        unsafe { instance.incref_object(item.as_ptr()) };
        let verdict = crate::executor::protocol::equality_public(instance, item, *wanted);
        unsafe { instance.release_object(item.as_ptr()) };
        if verdict? {
            count += 1;
        }
        index += 1;
    }
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
    // **排序期间被改表**要报 `ValueError`（参照 `list modified during sort` ✓）：比较键是用户代码，
    // 可能 `append`／`clear` 本表（`list_tests.test_sort` 的 `selfmodifyingComparison` ✓）。
    let original_length = object.len();
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
    if object.len() != original_length {
        // 清掉装饰期的两份引用，再报参照口径的 `ValueError` ✓（列表本身保持被改后的样子 ✓）。
        for (item, item_key) in decorated {
            unsafe {
                instance.release_object(item.as_ptr());
                instance.release_object(item_key.as_ptr());
            }
        }
        return Err(instance.raise_builtin_error(
            "ValueError",
            "list modified during sort",
        ));
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
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    reject_extra_arguments(instance, args, "reverse")?;
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
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    reject_extra_arguments(instance, args, "clear")?;
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
    // **富比较协议**（round 110）：`remove` 与 `in` 同口径（用户 `__eq__` 生效 ✓、顺序短路 ✓）。
    let mut found = None;
    let mut index = 0usize;
    while index < object.len() {
        let item = object.item(index).expect("刚查过长度，下标在范围内");
        unsafe { instance.incref_object(item.as_ptr()) };
        let verdict = crate::executor::protocol::equality_public(instance, item, *target);
        unsafe { instance.release_object(item.as_ptr()) };
        if verdict? {
            found = Some(index);
            break;
        }
        index += 1;
    }
    match found {
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
        // **`list` 的 dunder／`copy` 面**（round 106）：官方 `list_tests`／`seq_tests` 大量直接调用
        // `u.__getitem__`／`u.__setitem__`／`u.__delitem__`／`u.copy()`／`u.__iadd__` 一族 ✓ ——
        // 先前这些名字在实例上**取不到** ✗（`AttributeError: 'list' object has no attribute ...`）。
        "__init__" => list_init_native,
        "__getitem__" => list_dunder_getitem,
        "__setitem__" => list_dunder_setitem,
        "__delitem__" => list_dunder_delitem,
        "__iter__" => list_dunder_iter,
        "__reversed__" => list_dunder_reversed,
        "__add__" => list_dunder_add,
        "__iadd__" => list_dunder_iadd,
        "__imul__" => list_dunder_imul,
        "copy" => list_copy_native,
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
/// `list.__getitem__(self, key)`（round 106）：整数／切片都走**一处真相**的 `subscript_get` ✓。
fn list_dunder_getitem(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__getitem__() takes exactly one argument (0 given)",
        ));
    };
    crate::executor::subscript::subscript_get(instance, list, key, 0)
}

/// `list.__setitem__(self, key, value)`。
fn list_dunder_setitem(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let list = bound_list(instance, bound)?;
    let (Some(key), Some(value)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__setitem__() takes exactly 2 arguments",
        ));
    };
    // **值那份要按 `subscript_set` 的"接管"契约先 retain**（round 104 真 bug 修 ✗）：本函数是
    // **原生** `__setitem__`，`args` 是**借用视图**（`call_callable` 的 native 分支调用后会释放
    // 实参）✗ —— 先前把借用指针直接交给"接管值"的 `subscript_set` ⇒ 写入失败路径里值被释放一次、
    // native 分支返回后又释放一次 ⇒ **同一个值双放** ✓（round 102 的融合存取修复让对象更早被释放，
    // 于是在 `test_list.test_set_subscript` 暴露；`PYAWA_REF_TRACE=...BT_ALL` 现场：两次 -1 都
    // 落在 `subscript_set ← list_dunder_setitem` 与 `call_callable` ✓）。
    instance.retain(value);
    crate::executor::subscript::subscript_set(instance, list, key, value, 0)?;
    Ok(instance.retain(instance.singletons().none()))
}

/// `list.__delitem__(self, key)`。
fn list_dunder_delitem(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__delitem__() takes exactly one argument (0 given)",
        ));
    };
    crate::executor::subscript::subscript_del(instance, list, key, 0)?;
    Ok(instance.retain(instance.singletons().none()))
}

/// `list.__iter__(self)`：交现成的迭代器 ✓。
fn list_dunder_iter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let list = bound_list(instance, bound)?;
    instance.iter_object(list)
}

/// `list.__reversed__(self)`：**新列表**（反向快照 ✓；本层没有 lazy reverseiterator）。
fn list_dunder_reversed(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let list = bound_list(instance, bound)?;
    // **真正的 `list_reverseiterator`**（round 111 真 bug 修 ✗）：参照的反向迭代器**引用原列表** ✓、
    // 游标是"下一个要取的下标 ＋ 1" ✓ —— 先前把列表**快照**成新 list 再正向迭代 ✗ ⇒
    // ① 类型名给 `list_iterator` ✗；② 快照看不到之后对原列表的修改 ✗
    //（`test_reversed_pickle`：反序列化后 `a[:] = data` 期望反向迭代反映新数据 ✓）。
    let length = instance.list_items(list).map(|items| items.len()).unwrap_or(0);
    let iterator_type = instance
        .type_named("list_reverseiterator")
        .expect("list_reverseiterator 已登记");
    // 迭代器要自己那份原列表引用 ✓。
    unsafe { instance.incref_object(list.as_ptr()) };
    let iterator = instance.alloc(crate::builtin_objects::IteratorObject::new(
        iterator_type,
        list,
        core::cell::Cell::new(length),
    ));
    Ok(iterator.into_raw().cast::<Header>())
}

/// `list.__add__(self, other)`：与 `+` 同一处（`concat_public` ✓）。
fn list_dunder_add(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(other) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__add__() takes exactly one argument (0 given)",
        ));
    };
    crate::executor::iter::concat_public(instance, list, other, 0)
}

/// `list.__iadd__(self, other)`：**就地**扩展 ✓（保持对象身份；官方 `list_tests.test_iadd` ✓）。
fn list_dunder_iadd(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(other) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__iadd__() takes exactly one argument (0 given)",
        ));
    };
    let Some(items) = instance.list_items(other) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "can only concatenate list (not \"other\") to list",
        ));
    };
    for item in items {
        // SAFETY: 元素由另一列表持有，存活；本列表要自己那份。
        unsafe { instance.incref_object(item.as_ptr()) };
        instance.list_append(list, item);
    }
    Ok(instance.retain(list))
}

/// `list.__imul__(self, count)`：走 `*=` 的就地重复（一处真相 ✓）。
fn list_dunder_imul(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(count_value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__imul__() takes exactly one argument (0 given)",
        ));
    };
    let Some(count) = instance.index_value(count_value)? else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "can't multiply sequence by non-int of type",
        ));
    };
    // **就地重复、返回同一个对象**（round 110 真 bug 修 ✗）：先前走通用 `inplace_arithmetic`
    // ⇒ 返回新列表 ✗ ⇒ `s *= 10` 后 `id(s) != oldid` ✗（`list_tests.test_imul`／`test_repeat` ✓）。
    // **上限守卫生效**：`sys.maxsize` 一类 ⇒ `MemoryError`（`test_list_resize_overflow` ✓）。
    // SAFETY: 绑定的是本实例的 list。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    const MAX_REPEAT_ITEMS: i64 = 1 << 33;
    if count > 0 && object.len() as i64 >= MAX_REPEAT_ITEMS / count.max(1) {
        return Err(instance.raise_builtin_error(
            "MemoryError",
            "list 重复后长度过大",
        ));
    }
    if count <= 0 {
        while let Some(item) = object.pop_last() {
            unsafe { instance.release_object(item.as_ptr()) };
        }
        return Ok(instance.retain(list));
    }
    let original: Vec<NonNull<Header>> = object
        .items()
        .iter()
        .map(|item| instance.retain(*item))
        .collect();
    for _ in 1..count {
        for item in &original {
            unsafe { instance.incref_object(item.as_ptr()) };
            object.append(*item);
        }
    }
    // 自己那份临时引用归还 ✓（元素由列表各持一份 ✓）。
    for item in original {
        unsafe { instance.release_object(item.as_ptr()) };
    }
    Ok(instance.retain(list))
}

/// `list.copy()`：**浅拷贝** ✓（官方 `list_tests.test_copy` ✓）。
fn list_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_extra_arguments(instance, args, "copy")?;
    let list = bound_list(instance, bound)?;
    let items: Vec<NonNull<Header>> = instance
        .list_items(list)
        .unwrap_or_default()
        .into_iter()
        // **借用 → 新引用**（`new_list` 吃所有权 ✓；round 106 修 ✗）。
        .map(|item| instance.retain(item))
        .collect();
    Ok(instance.new_list(items))
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
    let items = match instance.iterable_items(*iterable)? {
        Some(items) => items,
        None => {
            return Err(instance.raise_builtin_error("TypeError", "object is not iterable"))
        }
    };
    // `iterable_items` 交的是**新引用** ⇒ `list_append` 直接**接手**（先前多 retain 一次 ✗
    // ⇒ 每项漏一份、`list.extend` 循环把计数顶穿 ✓；round 58 统一口径 ✓）。
    for item in items {
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
/// **`list.__init__(self, iterable=(), /)`**（round 110）：**清空后重填** ✓ ——
/// `list_tests.test_init` 钉住「Init clears previous values」「Init overwrites previous values」✓；
/// 先前 `list` 的 `__init__` 取不到（实例上 AttributeError ✗，且 `a.__init__([4,5,6])` 不生效 ✗）。
pub(crate) fn list_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // `list.__init__` 只收一个位置实参（可迭代）；关键字一律不收（参照 `list() takes no keyword
    // arguments` ✓）。
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "list() takes no keyword arguments",
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "list expected at most 1 argument, got {}",
                args.len()
            ),
        ));
    }
    let list = bound_list(instance, bound)?;
    // SAFETY: 绑定的是本实例的 list。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    while let Some(item) = object.pop_last() {
        unsafe { instance.release_object(item.as_ptr()) };
    }
    if let Some(source) = args.first().copied() {
        // 复用 `list_new` 的"任意可迭代"口径：先收集（各自持一份新引用），再交给列表 ✓。
        let iterator = instance.iter_object(source)?;
        loop {
            match instance.advance_iterator(iterator)? {
                Some(item) => object.append(item),
                None => break,
            }
        }
        unsafe { instance.release_object(iterator.as_ptr()) };
    }
    Ok(instance.retain(instance.singletons().none()))
}

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
        let source_type = instance.type_of(*source);
        let source_ty = instance.type_name(source_type);
        // **子类覆写了 `__iter__` ⇒ 必须走迭代协议**（round 111）：先前按**类型名**走载荷快路 ✗
        // ⇒ `LyingTuple((2,)).__iter__()` 明明只吐 `1`，`list(...)` 却按底层元组给 `[2]` ✗
        //（`seq_tests.test_constructors` 的 `LyingTuple`／`LyingList` ✓）。
        let default_iter = |base: &str| {
            instance
                .type_lookup_owner(source_type, "__iter__")
                .map(|(owner, _)| {
                    Some(owner) == instance.type_named(base)
                        || Some(owner) == instance.type_named("object")
                })
                .unwrap_or(true)
        };
        match source_ty.as_str() {
            "list" if default_iter("list") => {
                items = unsafe { &*source.as_ptr().cast::<ListObject>() }.items().to_vec()
            }
            "tuple" if default_iter("tuple") => {
                items = unsafe { &*source.as_ptr().cast::<TupleObject>() }.items().to_vec()
            }
            "set" | "frozenset" if default_iter(&source_ty) => {
                items = unsafe { &*source.as_ptr().cast::<SetObject>() }.items().to_vec()
            }
            "dict" if default_iter("dict") => {
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
    // `P3-41`：自引用 ⇒ `[...]`；嵌套太深 ⇒ `RecursionError`（**不得**顶穿原生栈 ✗）；
    // 守卫用 RAII 摘除（任何提前 `?` 返回都不留脏条目）。
    let _guard = match instance.enter_repr(ptr as usize) {
        crate::instance::ReprEntry::Cycle => return Ok("[...]".to_owned()),
        crate::instance::ReprEntry::TooDeep => return Err(instance.repr_recursion_error()),
        crate::instance::ReprEntry::Entered(scope) => scope,
    };
    // **活体遍历**（round 110 真 bug 修 ✗）：元素的 `__repr__` 可能**改表**（`test_list.test_repr_mutate`
    // 的元素会 `mylist.pop()` ✓）—— 参照 `list_repr` 每轮重读 `Py_SIZE` ✓、且比较期间元素要自己持一份 ✓；
    // 先前借 `object.items()` 快照（无引用）✗ ⇒ 元素被 pop 释放后 `element_repr` 读悬垂指针 ✗
    //（实测 68 GB 分配失败 abort ✗）。
    let mut text = String::from("[");
    let mut index = 0usize;
    while index < object.len() {
        let item = object.item(index).expect("刚查过长度，下标在范围内");
        // SAFETY: 元素由列表持有；`__repr__` 期间本函数自己持一份 ✓。
        unsafe { instance.incref_object(item.as_ptr()) };
        if index > 0 {
            text.push_str(", ");
        }
        let rendered = crate::executor::protocol::element_repr(instance, item);
        // SAFETY: 归还本函数那一份。
        unsafe { instance.release_object(item.as_ptr()) };
        text.push_str(&rendered?);
        index += 1;
    }
    text.push(']');
    Ok(text)
}
