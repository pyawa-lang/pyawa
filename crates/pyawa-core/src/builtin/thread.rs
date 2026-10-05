//! **`thread` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `thread_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`MAIN_THREAD_IDENT`、`bound_lock`、`copy_context_value`、`lock_is_recursive` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::cell::Cell;
use crate::builtin_objects::{BuiltinFunctionObject, MethodObject, NativeFn, ThreadHandleObject, ThreadLockObject, TupleObject};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::{MAIN_THREAD_IDENT, bound_lock, lock_is_recursive};


/// `lock` ／ `RLock` 的**构造槽**（两者共用；可重入性由**类型名**决定 ✓）。
pub unsafe fn thread_lock_new(
    class: NonNull<crate::TypeObject>,
    _args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let object = instance.alloc(ThreadLockObject::new(class, Cell::new(0), Cell::new(0)));
    Ok(object.into_raw().cast::<Header>())
}

/// `_thread` 锁的**方法面**（`OM-11` 的 `getattr` 槽 ✓ —— 与 [`property_getattr`] 同一手法：
/// 交**绑定**的原生方法 ✓，而不是往类型字典里塞原生 ✓）。
pub unsafe fn thread_lock_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "acquire" => thread_lock_acquire,
        "release" => thread_lock_release,
        "locked" => thread_lock_locked,
        "_is_owned" => thread_lock_is_owned,
        "_recursion_count" => thread_lock_recursion_count,
        "_acquire_restore" => thread_lock_acquire_restore,
        "_release_save" => thread_lock_release_save,
        "__enter__" => thread_lock_enter,
        "__exit__" => thread_lock_exit,
        _ => return None,
    };
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "lock",
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

/// **`_thread.allocate_lock()`**（模块级函数 ✓；参照里它建的就是 `lock` 类型 ✓）。
pub unsafe fn thread_allocate_lock_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let ty = instance
        .type_named("lock")
        .expect("引导期已登记 `lock` 类型");
    let object = instance.alloc(ThreadLockObject::new(ty, Cell::new(0), Cell::new(0)));
    Ok(object.into_raw().cast::<Header>())
}

/// **`_thread.get_ident()`**（一并给 `get_native_id`／`_get_main_thread_ident` 用 ✓）。
pub unsafe fn thread_get_ident_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    Ok(instance.new_int(MAIN_THREAD_IDENT))
}

/// `acquire(blocking=True, timeout=-1)` —— 口径见 [`ThreadLockObject`] 的文档 ✓。
pub(crate) unsafe fn thread_lock_acquire(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let lock = bound_lock(instance, bound)?;
    // 第一个实参是 `blocking`（默认 `True`）；本层只区分"显式 `False`"这一种（`timeout` 随后接 ✓）
    let blocking = match args.first() {
        Some(value) => instance.bool_value(*value).unwrap_or(true),
        None => true,
    };
    // SAFETY: lock 由本次调用借来，存活 ✓。
    let object = unsafe { &*lock.as_ptr().cast::<ThreadLockObject>() };
    if object.depth() == 0 {
        object.owner.set(MAIN_THREAD_IDENT);
        object.depth.set(1);
        return Ok(instance.new_bool(true));
    }
    if lock_is_recursive(instance, lock) {
        object.depth.set(object.depth() + 1);
        return Ok(instance.new_bool(true));
    }
    if !blocking {
        return Ok(instance.new_bool(false));
    }
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "普通锁在已持有时取用会阻塞；本层每实例单线程、挂起是协作式的 ⇒ 持有者跑不到 `release`（死锁）",
    ))
}

/// `release()` —— 未持时报参照实测的那句话 ✓（`release unlocked lock`）。
pub(crate) unsafe fn thread_lock_release(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let lock = bound_lock(instance, bound)?;
    // SAFETY: lock 由本次调用借来，存活 ✓。
    let object = unsafe { &*lock.as_ptr().cast::<ThreadLockObject>() };
    if object.depth() == 0 {
        return Err(instance.raise_builtin_error("RuntimeError", "release unlocked lock"));
    }
    if object.depth() == 1 {
        object.depth.set(0);
        object.owner.set(0);
    } else {
        object.depth.set(object.depth() - 1);
    }
    Ok(instance.new_none())
}

/// `locked()`。
pub(crate) unsafe fn thread_lock_locked(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let lock = bound_lock(instance, bound)?;
    // SAFETY: lock 由本次调用借来，存活 ✓。
    let object = unsafe { &*lock.as_ptr().cast::<ThreadLockObject>() };
    Ok(instance.new_bool(object.depth() > 0))
}

/// `_is_owned()`。
pub(crate) unsafe fn thread_lock_is_owned(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let lock = bound_lock(instance, bound)?;
    // SAFETY: lock 由本次调用借来，存活 ✓。
    let object = unsafe { &*lock.as_ptr().cast::<ThreadLockObject>() };
    Ok(instance.new_bool(object.depth() > 0 && object.owner() == MAIN_THREAD_IDENT))
}

/// `_recursion_count()`。
pub(crate) unsafe fn thread_lock_recursion_count(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let lock = bound_lock(instance, bound)?;
    // SAFETY: lock 由本次调用借来，存活 ✓。
    let object = unsafe { &*lock.as_ptr().cast::<ThreadLockObject>() };
    Ok(instance.new_int(object.depth() as i64))
}

/// `_release_save()` —— 参照实测给**二元组** `(重入深度, 持有者 ident)` ✓，并把锁整个释放 ✓。
pub(crate) unsafe fn thread_lock_release_save(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let lock = bound_lock(instance, bound)?;
    // SAFETY: lock 由本次调用借来，存活 ✓。
    let object = unsafe { &*lock.as_ptr().cast::<ThreadLockObject>() };
    let depth = object.depth() as i64;
    let owner = object.owner();
    object.depth.set(0);
    object.owner.set(0);
    let state = instance.new_tuple(vec![instance.new_int(depth), instance.new_int(owner)]);
    Ok(state)
}

/// `_acquire_restore(state)` —— 参照实测收 `_release_save()` 交出的那个**二元组** ✓。
pub(crate) unsafe fn thread_lock_acquire_restore(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let lock = bound_lock(instance, bound)?;
    let Some(state) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_acquire_restore() takes exactly one argument (0 given)",
        ));
    };
    if instance.type_name(instance.type_of(state)) != "tuple" {
        // 参照实测：`TypeError: _acquire_restore() argument 1 must be 2-item tuple, not int`
        let kind = instance.type_name(instance.type_of(state));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("_acquire_restore() argument 1 must be 2-item tuple, not {kind}"),
        ));
    }
    // SAFETY: 类型身份刚确认。
    let tuple = unsafe { &*state.as_ptr().cast::<TupleObject>() };
    if tuple.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_acquire_restore() argument 1 must be 2-item tuple",
        ));
    }
    let depth = tuple.item(0).and_then(|item| instance.int_value(item)).unwrap_or(0);
    // SAFETY: lock 由本次调用借来，存活 ✓。
    let object = unsafe { &*lock.as_ptr().cast::<ThreadLockObject>() };
    object.owner.set(MAIN_THREAD_IDENT);
    object.depth.set(depth.max(0) as usize);
    Ok(instance.new_none())
}

/// `__enter__` —— 参照实测交的是 `acquire()` 的结果（**布尔** ✓，不是 `self` ✗）。
pub(crate) unsafe fn thread_lock_enter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    unsafe { thread_lock_acquire(instance, bound, args, kwargs) }
}

/// `__exit__(exc_type, exc, tb)` —— 释放；返回 `None`（照参照实测 ✓）。
pub(crate) unsafe fn thread_lock_exit(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    unsafe { thread_lock_release(instance, bound, &[], &[]) }
}

/// **造一个 `_ThreadHandle` 占位句柄**（第 333 轮）：本层没有真线程 ✓ ⇒ 句柄里没有真状态 ✓。
///
/// 给 stdlib 的 `_thread._make_thread_handle` 用 ✓（**类型本身不必公开** ✓ —— 与
/// [`copy_context_value`] 同一手法：公开的是"入口"而不是内部类型 ✓）。
pub fn thread_handle_new(instance: &Instance) -> Result<NonNull<Header>, ExecError> {
    let ty = instance.type_named("_ThreadHandle").ok_or(ExecError::Unsupported {
        opcode: 0,
        what: "`_ThreadHandle` 尚未登记（引导期）",
    })?;
    let handle = instance.alloc(ThreadHandleObject::new(ty, Cell::new(false)));
    Ok(handle.into_raw().cast::<Header>())
}
