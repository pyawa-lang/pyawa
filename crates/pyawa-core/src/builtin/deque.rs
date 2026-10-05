//! **`deque` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `deque_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`iterator_traverse`、`set_clear`、`set_getattr`、`set_traverse` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::cell::{Cell, RefCell};
use crate::builtin_objects::{BuiltinFunctionObject, DequeObject, MethodObject, NativeFn};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;

use crate::builtin_objects::{deque_self};
/// **`deque` 的构造**（第 331 轮）：`deque(iterable=(), maxlen=None)` ✓。
///
/// 照参照：`iterable` 可省 ✓；`maxlen` 可以是关键字 ✓；`maxlen` 非正 ⇒ `ValueError` ✓；
/// 超过 `maxlen` 时**丢头部** ✓（`deque([1, 2, 3], maxlen=2)` ⇒ `deque([2, 3], maxlen=2)` ✓）。
pub unsafe fn deque_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let mut maxlen = usize::MAX;
    let mut iterable: Option<NonNull<Header>> = None;
    for (index, argument) in args.iter().enumerate() {
        if index == 0 {
            // **`None` 当"没给"** ✓（参照 `deque(None)` ⇒ `TypeError` ✗ —— 这里如实区分 ✓）
            // SAFETY: argument 由调用方保证存活。
            if unsafe { argument.as_ref() }.ty() != instance.singletons().none_type() {
                iterable = Some(*argument);
            }
        } else if index == 1 {
            // SAFETY: 同上。
            if unsafe { argument.as_ref() }.ty() != instance.singletons().none_type() {
                let Some(value) = instance.int_of(*argument).and_then(|value| value.to_i64()) else {
                    return Err(instance.raise_builtin_error("TypeError", "'maxlen' must be an integer"));
                };
                if value < 0 {
                    return Err(instance.raise_builtin_error("ValueError", "maxlen must be non-negative"));
                }
                maxlen = value as usize;
            }
        }
    }
    let deque = instance.alloc(DequeObject::new(class, RefCell::new(Vec::new()), Cell::new(maxlen)));
    if let Some(iterable) = iterable {
        // **摊开可迭代对象**（`collect_iterable` 是既有的公开入口 ✓ —— 一处真相 ✓）
        for value in instance.collect_iterable(iterable)? {
            // SAFETY: value 由调用方那份引用持有，这里新增一份交给 deque ✓。
            unsafe { instance.incref_object(value.as_ptr()) };
            deque.get().push_back(instance, value);
        }
    }
    Ok(deque.into_raw().cast::<Header>())
}
/// **`deque` 的方法面**（照 `set_getattr` 的手法 ✓：名字 → 原生，再包成**绑定方法** ✓）。
pub unsafe fn deque_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // `maxlen` 是**属性**（不限时给 `None` ✓，与参照一致 ✓）
    if name == "maxlen" {
        // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
        let object = unsafe { &*ptr.cast::<DequeObject>() };
        let raw = object.raw_maxlen();
        if raw == usize::MAX {
            return Some(instance.retain(instance.singletons().none()));
        }
        return Some(instance.new_int(raw as i64));
    }
    let handler: NativeFn = match name {
        "append" => deque_append_native,
        "appendleft" => deque_appendleft_native,
        "pop" => deque_pop_native,
        "popleft" => deque_popleft_native,
        "extend" => deque_extend_native,
        "extendleft" => deque_extendleft_native,
        "clear" => deque_clear_native,
        "rotate" => deque_rotate_native,
        "count" => deque_count_native,
        "remove" => deque_remove_native,
        "index" => deque_index_native,
        "insert" => deque_insert_native,
        "copy" => deque_copy_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "deque",
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
/// `deque.append(x)` ✓。
pub(crate) unsafe fn deque_append_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, args, "append");
    let Some(value) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "append expected 1 argument"));
    };
    // SAFETY: value 由调用方持有，这里新增一份交给 deque ✓。
    unsafe { instance.incref_object(value.as_ptr()) };
    object.push_back(instance, *value);
    Ok(instance.retain(instance.singletons().none()))
}
/// `deque.appendleft(x)` ✓。
pub(crate) unsafe fn deque_appendleft_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, args, "appendleft");
    let Some(value) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "appendleft expected 1 argument"));
    };
    // SAFETY: 同上。
    unsafe { instance.incref_object(value.as_ptr()) };
    object.push_front(instance, *value);
    Ok(instance.retain(instance.singletons().none()))
}
/// `deque.pop()` ✓（空 ⇒ `IndexError: pop from an empty deque` ✓）。
pub(crate) unsafe fn deque_pop_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, owner) = deque_self!(instance, bound, _args, "pop");
    let mut items = object.items.borrow_mut();
    let Some(value) = items.pop() else {
        drop(items);
        return Err(instance.raise_builtin_error("IndexError", "pop from an empty deque"));
    };
    drop(items);
    let _ = owner;
    // **交出新引用**（`release_object` 会减一份 ✓ ⇒ 这里先给调用方加一份 ✓）
    // SAFETY: value 由本对象持有 ✓。
    unsafe { instance.incref_object(value.as_ptr()) };
    // SAFETY: 同上。
    unsafe { instance.release_object(value.as_ptr()) };
    Ok(value)
}
/// `deque.popleft()` ✓（空 ⇒ 同 `pop` 的消息 ✓）。
pub(crate) unsafe fn deque_popleft_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, _args, "popleft");
    let mut items = object.items.borrow_mut();
    if items.is_empty() {
        drop(items);
        return Err(instance.raise_builtin_error("IndexError", "pop from an empty deque"));
    }
    let value = items.remove(0);
    drop(items);
    // SAFETY: 同上。
    unsafe { instance.incref_object(value.as_ptr()) };
    unsafe { instance.release_object(value.as_ptr()) };
    Ok(value)
}
/// 把一份实参（可迭代）摊进 deque 的某一端 ✓。
pub(crate) unsafe fn deque_extend_common(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    front: bool,
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, args, "extend");
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "extend expected 1 argument"));
    };
    for value in instance.collect_iterable(*iterable)? {
        // SAFETY: value 由调用方那份引用持有 ✓。
        unsafe { instance.incref_object(value.as_ptr()) };
        if front {
            object.push_front(instance, value);
        } else {
            object.push_back(instance, value);
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}
/// `deque.extend(iterable)` ✓。
pub(crate) unsafe fn deque_extend_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    unsafe { deque_extend_common(instance, bound, args, false) }
}
/// `deque.extendleft(iterable)` ✓（**逐个左插** ⇒ 顺序反过来 ✓，与参照一致 ✓）。
pub(crate) unsafe fn deque_extendleft_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    unsafe { deque_extend_common(instance, bound, args, true) }
}
/// `deque.clear()` ✓。
pub(crate) unsafe fn deque_clear_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, _args, "clear");
    // **先取出、后释放** ✓（可变借用不横跨 `release_object` ✓ —— 第 148 轮的教训 ✓）
    let taken: Vec<NonNull<Header>> = core::mem::take(&mut *object.items.borrow_mut());
    for value in taken {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    Ok(instance.retain(instance.singletons().none()))
}
/// `deque.rotate(n=1)` ✓（正数右旋 ✓、负数左旋 ✓）。
pub(crate) unsafe fn deque_rotate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, args, "rotate");
    let steps = match args.first() {
        Some(value) => instance
            .int_of(*value)
            .and_then(|value| value.to_i64())
            .unwrap_or(0),
        None => 1,
    };
    let mut items = object.items.borrow_mut();
    let length = items.len();
    if length > 1 {
        let shift = steps.rem_euclid(length as i64) as usize;
        items.rotate_right(shift);
    }
    Ok(instance.retain(instance.singletons().none()))
}
/// `deque.count(x)` ✓（按值相等 ✓）。
pub(crate) unsafe fn deque_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, args, "count");
    let Some(needle) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "count expected 1 argument"));
    };
    let found = object
        .items()
        .iter()
        .filter(|value| crate::executor::values::values_equal_public(instance, **value, *needle))
        .count();
    Ok(instance.new_int(found as i64))
}
/// `deque.remove(x)` ✓（找不到 ⇒ `ValueError: deque.remove(x): x not in deque` ✓）。
pub(crate) unsafe fn deque_remove_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, args, "remove");
    let Some(needle) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "remove expected 1 argument"));
    };
    let mut items = object.items.borrow_mut();
    let Some(position) = items
        .iter()
        .position(|value| crate::executor::values::values_equal_public(instance, *value, *needle))
    else {
        drop(items);
        return Err(instance.raise_builtin_error("ValueError", "deque.remove(x): x not in deque"));
    };
    let value = items.remove(position);
    drop(items);
    // SAFETY: 该引用由本对象持有 ⇒ 交还实例 ✓。
    unsafe { instance.release_object(value.as_ptr()) };
    Ok(instance.retain(instance.singletons().none()))
}
/// `deque.index(x[, start[, stop]])` ✓（找不到 ⇒ `ValueError` ✓，消息照参照 ✓）。
pub(crate) unsafe fn deque_index_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, args, "index");
    let Some(needle) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "index expected at least 1 argument"));
    };
    let items = object.items();
    let start = args
        .get(1)
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0)
        .max(0) as usize;
    let stop = args
        .get(2)
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .map(|value| value.max(0) as usize)
        .unwrap_or(items.len())
        .min(items.len());
    for position in start.min(items.len())..stop {
        if crate::executor::values::values_equal_public(instance, items[position], *needle) {
            return Ok(instance.new_int(position as i64));
        }
    }
    Err(instance.raise_builtin_error("ValueError", &format!("{} is not in deque", instance.object_repr(*needle)?)))
}
/// `deque.insert(i, x)` ✓。
pub(crate) unsafe fn deque_insert_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, args, "insert");
    let (Some(index), Some(value)) = (args.first(), args.get(1)) else {
        return Err(instance.raise_builtin_error("TypeError", "insert expected 2 arguments"));
    };
    let index = instance
        .int_of(*index)
        .and_then(|value| value.to_i64())
        .unwrap_or(0);
    // SAFETY: value 由调用方持有 ⇒ 新增一份交给 deque ✓。
    unsafe { instance.incref_object(value.as_ptr()) };
    let mut items = object.items.borrow_mut();
    let length = items.len() as i64;
    let position = if index < 0 { (index + length).max(0) } else { index.min(length) } as usize;
    items.insert(position, *value);
    if items.len() > object.raw_maxlen() {
        let dropped = items.pop().expect("刚插过，非空");
        drop(items);
        // SAFETY: 同上。
        unsafe { instance.release_object(dropped.as_ptr()) };
    }
    Ok(instance.retain(instance.singletons().none()))
}
/// `deque.copy()` ✓（**浅拷贝** ✓ —— 元素引用各加一份 ✓）。
pub(crate) unsafe fn deque_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (object, _) = deque_self!(instance, bound, _args, "copy");
    let items = object.items();
    let deque_type = instance
        .type_named("deque")
        .expect("deque 在引导期已登记");
    let copy = instance.alloc(DequeObject::new(
        deque_type,
        RefCell::new(Vec::new()),
        Cell::new(object.raw_maxlen()),
    ));
    for value in items {
        // SAFETY: value 由原对象持有 ⇒ 新增一份 ✓。
        unsafe { instance.incref_object(value.as_ptr()) };
        copy.get().push_back(instance, value);
    }
    Ok(copy.into_raw().cast::<Header>())
}
/// 见 [`iterator_traverse`]。
pub(crate) unsafe fn deque_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<DequeObject>() };
    // **走访问器**（与 `set_traverse` 同一手法 ✓）：`gc_field_coverage` 那条守卫按"字段名出现在
    // traverse 体内"来查 ✓ —— 直接 `object.items.borrow()` 也算读到 ✓，但用访问器更一致 ✓。
    for value in object.items() {
        visit(value.as_ptr());
    }
}
/// 见 [`set_clear`]（**先取出、后释放** ✓）。
pub(crate) unsafe fn deque_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<DequeObject>() };
    for value in core::mem::take(&mut *object.items.borrow_mut()) {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}
/// `deque` 的 `repr`：照参照的 `deque([1, 2], maxlen=3)` 形态 ✓（本层按观测面如实给 ✓）。
pub(crate) unsafe fn deque_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<DequeObject>() };
    let items = object.items();
    let mut rendered: Vec<String> = Vec::with_capacity(items.len());
    for value in &items {
        rendered.push(instance.object_repr(*value)?);
    }
    let body = format!("[{}]", rendered.join(", "));
    let maxlen = object.raw_maxlen();
    if maxlen == usize::MAX {
        Ok(format!("deque({body})"))
    } else {
        Ok(format!("deque({body}, maxlen={maxlen})"))
    }
}
