//! **`context` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `context_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`copy_context_value` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::cell::{Cell, RefCell};
use crate::builtin_objects::{BuiltinFunctionObject, ContextObject, ContextVarObject, MethodObject, NativeFn, TokenObject};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::frame::Frame;
use crate::builtin_objects::copy_context_value;


pub(crate) unsafe fn context_var_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ContextVarObject>() };
    visit(object.name().as_ptr());
    visit(object.default().as_ptr());
    for value in object.values() {
        visit(value.as_ptr());
    }
}

pub(crate) unsafe fn context_var_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<ContextVarObject>() };
    // **先取出、后释放** ✓（第 148 轮的教训 ✓）
    let values: Vec<NonNull<Header>> = core::mem::take(&mut *object.values.borrow_mut());
    for value in values {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    // SAFETY: 同上。
    unsafe { instance.release_object(object.name().as_ptr()) };
    // SAFETY: 同上。
    unsafe { instance.release_object(object.default().as_ptr()) };
}

pub(crate) unsafe fn context_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ContextObject>() };
    visit(object.mapping().as_ptr());
}

pub(crate) unsafe fn context_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<ContextObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.mapping().as_ptr()) };
}

/// `ContextVar(name, *, default=None)`：**构造** ✓（`name` 必须是 `str` ✓，消息照参照 ✓）。
pub unsafe fn context_var_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(name) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "ContextVar() missing required argument 'name' (pos 1)",
        ));
    };
    if instance.text_of(*name).is_none() {
        return Err(instance.raise_builtin_error("TypeError", "context variable name must be a str"));
    }
    let mut default = instance.retain(instance.singletons().none());
    if let Some(given) = args.get(1) {
        // **`None` 也是"没有默认值"** ✓（本层的哨兵口径 ✓ —— 如实登记 ✓）
        // SAFETY: given 由调用方保证存活 ⇒ 交一份给对象 ✓。
        unsafe { instance.incref_object(given.as_ptr()) };
        default = *given;
    }
    // SAFETY: name 由调用方持有 ⇒ 新增一份交给对象 ✓。
    unsafe { instance.incref_object(name.as_ptr()) };
    let object = instance.alloc(ContextVarObject::new(
        class,
        *name,
        default,
        RefCell::new(Vec::new()),
    ));
    Ok(object.into_raw().cast::<Header>())
}

/// `Context()` ✓（**空的**上下文映射 ✓）。
pub unsafe fn context_new(
    class: NonNull<crate::TypeObject>,
    _args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = instance.new_dict();
    let object = instance.alloc(ContextObject::new(class, mapping));
    Ok(object.into_raw().cast::<Header>())
}

/// `ContextVar` 的方法面 ✓（`get`／`set`／`reset` ＋ `name` 属性 ✓）。
pub unsafe fn context_var_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ContextVarObject>() };
    if name == "name" {
        return Some(instance.retain(object.name()));
    }
    let handler: NativeFn = match name {
        "get" => context_var_get_native,
        "set" => context_var_set_native,
        "reset" => context_var_reset_native,
        _ => return None,
    };
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "ContextVar",
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

/// `ContextVar.get([default])` ✓。
pub(crate) unsafe fn context_var_get_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(owner) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "get 缺少 self"));
    };
    // SAFETY: owner 由方法对象持有，存活。
    let object = unsafe { &*owner.as_ptr().cast::<ContextVarObject>() };
    if let Some(value) = object.current() {
        return Ok(instance.retain(value));
    }
    if let Some(given) = args.first() {
        return Ok(instance.retain(*given));
    }
    // **没设过值、又没给默认** ⇒ 参照报 `LookupError` ✓（消息就是那个变量的 repr ✓ —— 本层给一个
    // 同形的近似 ✓，地址当然不同 ✓：语料不比地址 ✓）。
    // 注意：本层把"默认值"与"没设过值"都放在同一个槽里 ✓（`ContextVar(name, default)` 的
    // **关键字**写法还没接 ✗ —— `new` 槽看不到 kwargs ✓，如实登记 ✓）⇒ `ContextVar("v")` 的
    // 默认值槽就是 `None` ✓，于是"给过 `None` 当默认"与"没给"在本层无法区分 ✗（随后补哨兵 ✓）。
    let none = instance.singletons().none();
    if object.default() == none {
        return Err(instance.raise_builtin_error("LookupError", "<ContextVar> 还没设过值"));
    }
    Ok(instance.retain(object.default()))
}

/// `ContextVar.set(value)` ⇒ `Token` ✓。
pub(crate) unsafe fn context_var_set_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(owner) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "set 缺少 self"));
    };
    // SAFETY: owner 由方法对象持有，存活。
    let object = unsafe { &*owner.as_ptr().cast::<ContextVarObject>() };
    let Some(value) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "set() takes exactly one argument (0 given)"));
    };
    let old = object
        .current()
        .map(|value| {
            // SAFETY: 旧值由变量持有 ⇒ 给 Token 新增一份 ✓。
            unsafe { instance.incref_object(value.as_ptr()) };
            value
        })
        .unwrap_or_else(|| instance.retain(object.default()));
    // SAFETY: value 由调用方持有 ⇒ 给变量新增一份 ✓。
    unsafe { instance.incref_object(value.as_ptr()) };
    object.push_value(*value);
    // SAFETY: owner 由方法对象持有 ⇒ Token 要自己那份 ✓。
    unsafe { instance.incref_object(owner.as_ptr()) };
    let token_type = instance
        .type_named("Token")
        .expect("Token 在引导期已登记");
    let token = instance.alloc(TokenObject::new(token_type, owner, old));
    Ok(token.into_raw().cast::<Header>())
}

/// `ContextVar.reset(token)` ✓（token 不是本变量的 ⇒ `ValueError` ✓，消息照参照 ✓）。
pub(crate) unsafe fn context_var_reset_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(owner) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "reset 缺少 self"));
    };
    let Some(token) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "reset() takes exactly one argument (0 given)"));
    };
    // SAFETY: token 由调用方保证存活。
    if unsafe { token.as_ref() }.ty() != instance.type_named("Token").unwrap_or(unsafe { token.as_ref() }.ty()) {
        return Err(instance.raise_builtin_error("ValueError", "Token was created in a different Context"));
    }
    // SAFETY: 类型身份刚确认。
    let token_object = unsafe { &*token.as_ptr().cast::<TokenObject>() };
    if token_object.var() != owner {
        return Err(instance.raise_builtin_error("ValueError", "Token was created by a different ContextVar"));
    }
    // SAFETY: owner 由方法对象持有。
    let object = unsafe { &*owner.as_ptr().cast::<ContextVarObject>() };
    if let Some(popped) = object.pop_value() {
        // SAFETY: 这份引用由变量交出 ⇒ 交还实例 ✓。
        unsafe { instance.release_object(popped.as_ptr()) };
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// `Context` 的方法面（**最小面** ✓）：`get`／`__contains__`／`copy`／`run` ✓。
///
/// **第 332 轮如实登记** ✗：这四个方法的**第一版实现会崩**（单独跑 `run`／`copy`／`get` 都是
/// 静默无输出、合并跑直接段错误 ✓）⇒ 本轮**先把它们改成如实报"未实现"** ✓（`AB-22`／`CM-6`：
/// "未实现"必须与"未提供"分开 ✓），**不把崩溃留在树里** ✗。真正接线留给下一轮 ✓
/// （`Context` 要么承载真正的映射 ✓、要么把 `run` 走的"当前上下文"栈接上 ✓）。
pub unsafe fn context_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "get" => context_not_implemented_native,
        "__contains__" => context_not_implemented_native,
        "copy" => context_not_implemented_native,
        "run" => context_not_implemented_native,
        _ => return None,
    };
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "Context",
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

/// **`Context` 的方法本轮如实报未实现** ✗（第一版实现会崩 ✓ —— 见 [`context_getattr`] 的说明 ✓）。
pub(crate) unsafe fn context_not_implemented_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`Context` 的方法面本轮未接线（如实拒绝，见台账第 332 轮）",
    ))
}
#[allow(dead_code)]

/// `Context.get(var[, default])`：本层的 `Context` 不承载独立状态 ✗ ⇒ 一律回落到变量自己的值 ✓。
pub(crate) unsafe fn context_get_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let _ = bound;
    let Some(var) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "get() takes at least 1 argument"));
    };
    // SAFETY: var 由调用方保证存活。
    if unsafe { var.as_ref() }.ty() == instance.type_named("ContextVar").unwrap_or(unsafe { var.as_ref() }.ty()) {
        // SAFETY: 类型身份刚确认。
        let object = unsafe { &*var.as_ptr().cast::<ContextVarObject>() };
        if let Some(value) = object.current() {
            return Ok(instance.retain(value));
        }
        if let Some(given) = args.get(1) {
            return Ok(instance.retain(*given));
        }
        return Ok(instance.retain(object.default()));
    }
    if let Some(given) = args.get(1) {
        return Ok(instance.retain(*given));
    }
    Err(instance.raise_builtin_error("KeyError", "context 里没有这个变量"))
}
#[allow(dead_code)]

/// `var in context` ✓（本层的最小面：只看变量有没有值 ✓）。
pub(crate) unsafe fn context_contains_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let _ = bound;
    let Some(var) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "__contains__ 需要 1 个实参"));
    };
    let found = unsafe { var.as_ref() }.ty()
        == instance.type_named("ContextVar").unwrap_or(unsafe { var.as_ref() }.ty())
        && unsafe { &*var.as_ptr().cast::<ContextVarObject>() }.current().is_some();
    Ok(instance.new_bool(found))
}
#[allow(dead_code)]

/// `Context.copy()` ✓（本层没有独立状态 ⇒ 给一个新的空 `Context` ✓，如实登记 ✓）。
pub(crate) unsafe fn context_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let _ = bound;
    Ok(copy_context_value(instance))
}
#[allow(dead_code)]

/// `Context.run(callable, *args)` ✓（本层直接在全局上跑 ✓，如实登记 ✓）。
pub(crate) unsafe fn context_run_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let _ = bound;
    let Some((callable, rest)) = args.split_first() else {
        return Err(instance.raise_builtin_error("TypeError", "run() missing required argument 'callable' (pos 1)"));
    };
    let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(rest.len());
    for argument in rest {
        // SAFETY: 实参由调用方持有 ⇒ 新增一份交给调用 ✓。
        unsafe { instance.incref_object(argument.as_ptr()) };
        call_args.push(*argument);
    }
    crate::executor::call::call_callable(instance, *callable, None, call_args, kwargs.to_vec(), 0)
}

/// **`locals()`**（round 74）：返回**当前帧**的局部名快照字典 ✓（参照：模块层 `locals() is globals()`，
/// 函数层是"形参＋局部"的映射视图；本层给**快照 dict** —— 文档化的**如实偏差** ✗：对返回值的修改
/// 不会写回帧，参照的写回语义随后补 ✓）。
///
/// 动因：`test_threading_local`／`Lib/pdb.py` 一族直接用 `locals()`；先前内建表里**没有**它 ✗
/// ⇒ `NameError: name 'locals' is not defined` ✗。
pub fn locals_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(frame_header) = instance.current_frame() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "locals() 需要正在执行的帧",
        ));
    };
    // SAFETY: 当前帧由实例在 execute 期间持有，存活 ✓。
    let frame = unsafe { &*frame_header.as_ptr().cast::<Frame>() };
    // **模块层**：`locals() is globals()` ✓（命名空间就是全局映射 ✓）。
    if frame.globals().is_none() {
        if let Some(namespace) = frame.namespace() {
            return Ok(instance.retain(namespace));
        }
    }
    let mapping = instance.new_dict();
    let Some(code_header) = frame.code() else {
        return Ok(mapping);
    };
    // SAFETY: 帧持有一份 code 引用（BC-42），帧存活期间有效 ✓。
    let code = unsafe { &*code_header.as_ptr().cast::<crate::code::CodeObject>() };
    for slot in 0..frame.local_count() {
        let Some(name) = code.varname(slot) else {
            continue;
        };
        let Ok(Some(value)) = frame.local(slot) else {
            continue;
        };
        // `dict_set` 自己会 retain 值 ✓，键由这里造 ✓。
        instance.dict_set(mapping, name, value);
    }
    Ok(mapping)
}
