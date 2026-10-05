//! `_thread` 模块（第 280 轮；**VM 侧最小面** —— 用户裁定 A ✓）。
//!
//! 契约见 `docs/SPEC-c-modules.md` §5.2.8。只落 **`importlib` 引导路径要的那几个**：
//! `RLock`（`_ModuleLock` 的锁 ✓）、`allocate_lock`、`get_ident`；其余入口按 `CM-6`
//! **如实报未实现** ✓，逐条列在 §5.2.8 的"未落地"里，**不伪造** ✓。
//!
//! 归属：`SPEC-capabilities.md` §9.9 的 `ipc` 行自己写着"`thread_*` …**线程语义归 VM 侧**"
//! （且不可异步化）；本层**每实例单线程**（`DESIGN.md` §5 的挂起是协作式的）⇒ 锁就是 VM 内的记账，
//! **不碰外部世界权威** ✓ ⇒ 载荷与语义都在 `pyawa-core`（`ThreadLockObject`）✓。
//!
//! `CX-4`：本 crate 不碰平台 ✓。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`_thread`）。
pub const NAME: &str = "_thread";

/// 模块的 `__doc__`。
pub const DOC: &str = "This module provides primitive operations to write multi-threaded programs.";

/// **未落地**的入口（线程创建一类）：按 `CM-6` **如实报未实现** ✓（**不**静默给别的结果 ✗）。
fn not_implemented_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_thread` 的线程创建未实现（本层每实例单线程；`DESIGN.md` §5）",
    ))
}

/// `_thread._make_thread_handle(ident, ...)`：参照是"给**已存在**的线程造一个句柄" ✓。
///
/// 本层没有真线程 ✓ ⇒ 返回一个**占位句柄** ✓（`_ThreadHandle` 类型 ✓、"未启动" ✓）——
/// 这比"报未实现"更接近参照的用法 ✓（`threading` 拿它当"这个线程的把手" ✓，本层没有可把的东西 ✓），
/// 而且**不伪造**任何线程状态 ✓（句柄里没有真状态 ✓，如实登记在台账 ✓）。
fn make_thread_handle_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    pyawa_core::thread_handle_new(instance)
}

/// `_thread._is_main_interpreter()`：本层恒 `True` ✓（只有一个解释器 ✓，如实 ✓）。
fn is_main_interpreter_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.new_bool(true))
}

/// `_thread._shutdown()`：本层**没有后台线程** ⇒ 如实实现为"无事可做" ✓（返回 `None` ✓）。
fn shutdown_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}

/// 造一个原生可调用对象（**新引用**；与 `weakref_module`／`builtins_module` 同一做法）。
fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 建 `_thread` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // **两个锁类型**：`LockType` 是 `lock` 类型（参照实测 `LockType.__name__ == 'lock'` ✓），
    // `RLock` 就是它自己 ✓。类型在**引导期**由 core 登记 ✓（与 `weakref.ref` 同一手法 ✓）。
    // **先 `retain` 再交给字典** ✓（`dict_set` 走"接管一份引用"的规矩 ✓）。
    for (export, type_name) in [("LockType", "lock"), ("RLock", "RLock")] {
        if let Some(ty) = instance.type_named(type_name) {
            instance.retain(ty.cast());
            instance.dict_set(namespace, export, ty.cast());
        }
    }
    // `allocate_lock()`：参照里它建的就是 `lock` 类型 ✓。
    let allocate = make_native(
        instance,
        "allocate_lock",
        pyawa_core::thread_allocate_lock_native,
    );
    instance.dict_set(namespace, "allocate_lock", allocate);
    // **`get_ident` 一族**（本层恒为同一个 ident；取值属 `MS-17` 的实现观测面 ✓）。
    for name in ["get_ident", "get_native_id", "_get_main_thread_ident"] {
        let native = make_native(instance, name, pyawa_core::thread_get_ident_native);
        instance.dict_set(namespace, name, native);
    }
    // `TIMEOUT_MAX`：**实测量过**的参照取值（本机 `9223372036.0` ✓）。
    let timeout = instance.new_float(9223372036.0);
    instance.dict_set(namespace, "TIMEOUT_MAX", timeout);
    // `error`：参照里就是 `RuntimeError` 本身 ✓（实测 `_thread.error is RuntimeError` ✓）。
    if let Some(error_type) = instance.type_named("RuntimeError") {
        instance.retain(error_type.cast());
        instance.dict_set(namespace, "error", error_type.cast());
    }
    // **未落地**的入口（名字齐 ✓、调用时如实报未实现 ✓）——`threading.py` 一类还没进 `Lib/`。
    for name in [
        "start_new_thread",
        "exit",
        "exit_thread",
        "interrupt_main",
        "stack_size",
        "daemon_threads_allowed",
        // **第 333 轮补的三条** ✓：`Lib/threading.py` 在**模块级**就取它们 ✓
        // （`_start_joinable_thread = _thread.start_joinable_thread` 等 ✓）⇒ 名字必须在 ✓，
        // 调用时按 `CM-6` **如实报未实现** ✓（本层没有真线程 ✓）。
        "start_joinable_thread",
        "set_name",
    ] {
        let native = make_native(instance, name, not_implemented_native);
        instance.dict_set(namespace, name, native);
    }
    // **`_ThreadHandle`**：类型占位 ✓（`threading.py` 模块级取它 ✓）。
    if let Some(handle) = instance.type_named("_ThreadHandle") {
        instance.retain(handle.cast());
        instance.dict_set(namespace, "_ThreadHandle", handle.cast());
    }
    // **`_is_main_interpreter()`** ✓：本层恒为"主解释器" ✓（如实 ✓ —— 只有这一个 ✓）。
    let is_main = make_native(
        instance,
        "_is_main_interpreter",
        is_main_interpreter_native,
    );
    instance.dict_set(namespace, "_is_main_interpreter", is_main);
    // **`_make_thread_handle`** 单独接 ✓（它造占位句柄 ✓，见上面的说明 ✓）。
    let make_handle = make_native(
        instance,
        "_make_thread_handle",
        make_thread_handle_native,
    );
    instance.dict_set(namespace, "_make_thread_handle", make_handle);
    // **`_shutdown()`** ✓：本层没有后台线程 ⇒ **无事可做** ✓（返回 `None` ✓，不是"未实现" ✗ ——
    // 单线程下"关掉所有线程"这件事**已经**成立 ✓，这是**如实实现** ✓）。
    let shutdown = make_native(instance, "_shutdown", shutdown_native);
    instance.dict_set(namespace, "_shutdown", shutdown);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
