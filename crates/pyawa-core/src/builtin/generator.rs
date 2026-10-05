//! **`generator` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `generator_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`async_generator_anext_native`、`async_generator_asend_native`、`exception_instance`、`resume_with_sent`、`stop_iteration`、`thrown_exception` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::cell::Cell;
use crate::builtin_objects::{BuiltinFunctionObject, ExceptionObject, GeneratorObject, MethodObject, NativeFn};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::{async_generator_anext_native, async_generator_asend_native, exception_instance, resume_with_sent, thrown_exception};


pub unsafe fn generator_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // **协程不是迭代器**（实测它没有 `__next__`，也没有 `__iter__`）；生成器两者都有。
    // SAFETY: ptr 是本类型的存活对象（槽位契约）。
    let is_generator = unsafe {
        instance
            .type_name(instance.type_of(NonNull::new_unchecked(ptr)))
            == "generator"
    };
    // SAFETY: ptr 是本类型的存活对象（槽位契约）。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let type_name = instance.type_name(instance.type_of(owner));
    if type_name == "async_generator" {
        match name {
            // `__aiter__()` 返回自己（实测异步生成器就是它自己的 async iterator）
            "__aiter__" => {
                // SAFETY: ptr 由槽位契约保证存活，这里新增一份交给调用方。
                unsafe { instance.incref_object(ptr) };
                return Some(owner);
            }
            // `__anext__()` 交出 awaitable（我们的 `AsendObject`）；`asend(v)` 同形、带值
            "__anext__" | "asend" => {
                let method_type = instance
                    .type_named("builtin_function_or_method")
                    .expect("引导期已登记");
                let handler: NativeFn = if name == "asend" {
                    async_generator_asend_native
                } else {
                    async_generator_anext_native
                };
                let native = instance.alloc(BuiltinFunctionObject::new(
                    method_type,
                    "async_generator",
                    Cell::new(handler),
                ));
                let native_raw = native.into_raw().cast::<Header>();
                // SAFETY: ptr 由槽位契约保证存活，方法对象要自己那份 self。
                unsafe { instance.incref_object(ptr) };
                let bound = instance.alloc(MethodObject::new(
                    instance.type_named("method").expect("method 已登记"),
                    native_raw,
                    owner,
                ));
                return Some(bound.into_raw().cast::<Header>());
            }
            _ => {}
        }
    }
    let handler: NativeFn = match name {
        "send" => generator_send_native,
        "__next__" if is_generator => generator_next_native,
        "throw" => generator_throw_native,
        "close" => generator_close_native,
        _ => return None,
    };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    // 方法对象要自己持有函数与 self 各一份引用
    let native = instance.alloc(BuiltinFunctionObject::new(method_type, "generator", Cell::new(handler)));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: ptr 是本类型的存活对象（槽位契约）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        // SAFETY: ptr 由槽位契约保证非空（是本类型的存活对象）。
        unsafe { NonNull::new_unchecked(ptr) },
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// `send(value)`：把值送进生成器，返回**下一个让出值**；跑完则抛 `StopIteration(返回值)`。
pub(crate) unsafe fn generator_send_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("send 是绑定方法，必须有 self");
    if !kwargs.is_empty() {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "生成器的 send 不接受关键字实参（参照实现同）",
        });
    }
    let sent = args.first().copied();
    // SAFETY: 本函数本身是槽位回调，调用方保证 generator 存活。
    unsafe { resume_with_sent(instance, generator, sent) }
}

/// `__next__()`：等价于 `send(None)`。
pub(crate) unsafe fn generator_next_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("__next__ 是绑定方法，必须有 self");
    if !args.is_empty() || !kwargs.is_empty() {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "生成器的 __next__ 不接受实参",
        });
    }
    // SAFETY: 同上。
    unsafe { resume_with_sent(instance, generator, None) }
}

/// `throw(类型[, 值[, traceback]])`：把异常**抛在挂起点**。
///
/// 实测口径：生成器**已经跑完**或**从未启动**时，异常抛在**调用处**（不进去）；类会自动实例化
/// （`throw(ValueError, 'msg')` ⇒ `ValueError: msg`）；实例再带值 ⇒
/// `TypeError: instance exception may not have a separate value`；实参超过 3 个 ⇒
/// `TypeError: throw expected at most 3 arguments, got N`；不是异常 ⇒ 见
/// [`thrown_exception`] 里那条实测消息。
pub(crate) unsafe fn generator_throw_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("throw 是绑定方法，必须有 self");
    if !kwargs.is_empty() {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "生成器的 throw 不接受关键字实参",
        });
    }
    if args.is_empty() {
        return Err(crate::executor::raise_builtin(
            instance,
            "TypeError",
            "throw expected at least 1 argument, got 0",
        ));
    }
    if args.len() > 3 {
        return Err(crate::executor::raise_builtin(
            instance,
            "TypeError",
            &format!("throw expected at most 3 arguments, got {}", args.len()),
        ));
    }
    let exception = thrown_exception(instance, args[0], args.get(1).copied())?;
    // SAFETY: 槽位契约保证这是本实例里存活的生成器。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    if object.finished() || !object.started() {
        // 还没进去（或已经结束）⇒ 抛在调用处
        return Err(ExecError::Raised { exception });
    }
    match crate::executor::resume_generator_with_raise(instance, generator, exception)? {
        crate::executor::GeneratorOutcome::Yielded(value) => Ok(value),
        crate::executor::GeneratorOutcome::Returned(value) => {
            Err(crate::builtin_objects::stop_iteration(instance, Some(value)))
        }
    }
}

/// `close()`：往生成器里抛 `GeneratorExit`。实测口径——
/// 已经跑完或从未启动 ⇒ `None`（**不跑函数体**）；被关闭时又让出 ⇒
/// `RuntimeError: generator ignored GeneratorExit`；正常收尾/捕获后返回 ⇒ 交回**返回值**
/// （所以 `return 99` 的生成器 `close()` 得 `99`，普通情况是 `None`）；
/// 抛出别的异常 ⇒ 照原样往外抛。
pub(crate) unsafe fn generator_close_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("close 是绑定方法，必须有 self");
    if !args.is_empty() || !kwargs.is_empty() {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "生成器的 close 不接受实参",
        });
    }
    let none = || instance.new_none();
    // SAFETY: 同上。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    if object.finished() || !object.started() {
        object.mark_finished();
        return Ok(none());
    }
    object.mark_finished();
    let exit = exception_instance(instance, "GeneratorExit", Vec::new());
    let outcome = crate::executor::resume_generator_with_raise(instance, generator, exit);
    match outcome {
        Ok(crate::executor::GeneratorOutcome::Yielded(value)) => {
            // 让出的值交回一份引用（异常要抛出去，值用不上了）
            // SAFETY: value 是新引用。
            unsafe { instance.release_object(value.as_ptr()) };
            Err(crate::executor::raise_builtin(
                instance,
                "RuntimeError",
                "generator ignored GeneratorExit",
            ))
        }
        Ok(crate::executor::GeneratorOutcome::Returned(value)) => Ok(value),
        Err(ExecError::Raised { exception }) => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            let generator_exit = instance
                .type_named("GeneratorExit")
                .expect("异常层次在引导期已登记");
            if instance.is_subtype(ty, generator_exit) {
                // SAFETY: 这一份由本函数持有。
                unsafe { instance.release_object(exception.as_ptr()) };
                Ok(none())
            } else if instance.type_name(ty) == "StopIteration" {
                // 生成器内部抛 `StopIteration` ⇒ `close` 交出它的返回值（有就取第一个实参）
                // SAFETY: 类型身份已确认。
                let exception_object = unsafe { &*exception.as_ptr().cast::<ExceptionObject>() };
                let value = exception_object.args().first().copied();
                match value {
                    Some(value) => {
                        // SAFETY: 值由异常对象持有，这里新增一份交给调用方。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        // SAFETY: 异常那份由本函数持有。
                        unsafe { instance.release_object(exception.as_ptr()) };
                        Ok(value)
                    }
                    None => {
                        // SAFETY: 同上。
                        unsafe { instance.release_object(exception.as_ptr()) };
                        Ok(none())
                    }
                }
            } else {
                Err(ExecError::Raised { exception })
            }
        }
        Err(other) => Err(other),
    }
}

/// `OM-40`：列出生成器持有的引用。
pub(crate) unsafe fn generator_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    visit(object.frame().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出帧。
pub(crate) unsafe fn generator_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.frame().as_ptr()) };
}

/// 生成器的 `repr`：`<generator object gen at 0x…>`（实测）。
pub unsafe fn generator_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    unsafe { generator_repr_named(ptr, instance, "generator") }
}

/// 生成器／协程共用的 `repr`：词不同（实测 `<generator object f at 0x…>`／`<coroutine object f at 0x…>`）。
pub(crate) unsafe fn generator_repr_named(
    ptr: *mut Header,
    _instance: &Instance,
    word: &str,
) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    let frame = object.frame();
    // SAFETY: 帧由生成器持有，存活。
    let frame_ref = unsafe { &*frame.as_ptr().cast::<crate::Frame>() };
    let name = match frame_ref.code() {
        // SAFETY: code 由帧持有，存活。
        Some(code) => unsafe { code.cast::<crate::CodeObject>().as_ref() }.name(),
        None => "?",
    };
    Ok(format!("<{word} object {name} at {ptr:p}>"))
}
