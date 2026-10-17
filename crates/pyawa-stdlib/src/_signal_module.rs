//! `_signal` 模块（**task-13／`P3-29` 后续第一闸** ✓）：`import signal` 的导入面。
//!
//! 为什么先做它（侦察：`target/recon/cmodule_recon.md` §1 ＋ 本轮实测 ✓）：`Lib/signal.py` 在
//! **导入期**只要 `_signal` 两格 —— 模块存在、`signal`／`getsignal` 两个**可调用**属性
//! （`signal.py:56/62` 的 `@_wraps(_signal.X)` 在装饰期读 `wrapped.__doc__`）；
//! 前置的 `IntEnum._convert_` 已在 round 6 补进假货 `Lib/enum.py` ✓。解了这一格 ⇒
//! `asyncio` 一族 **118** 个模块（census 最大单族）与 `multiprocessing` 一族的共同第一道闸 ✓。
//!
//! ## 导出面（**刻意收窄** ✓）
//!
//! `__all__` ＝ **45 个整型常量** ＋ `signal`／`getsignal` —— **不导出** `pthread_sigmask`／
//! `sigpending`／`sigwait`／`valid_signals` 四个可选名 ⇒ `Lib/signal.py` 的四个包装**不定义**
//! （参照在**没有这些名字的平台**上同一条路 ✓），`Sigmasks` 类也不建（它只在那四个存在时转换）。
//! 其余 17 个函数（`alarm`／`pause`／`default_int_handler`／`setitimer`…）与 2 个类型
//! （`ItimerError`／`struct_siginfo`）本轮**不导出** ⇒ `signal` 命名空间里没有这些名字
//! （**如实记**：这是比参照少的语义面，不是"假装有" ✗）。
//!
//! ## 真面
//!
//! - **45 个常量**：值逐个照参照实测抄录（本机 `python3` 3.14.4／Linux ✓；`NSIG=65` 同时是
//!   合法号码上界 ⇒ 本文件只有**一处** `NSIG` ✓）；
//! - `signal(signalnum, handler, /)`／`getsignal(signalnum, /)` 的名字＋**参数校验语义**：
//!   关键字一律拒（`takes no keyword arguments`）、`signal` 的元数文案 `signal expected 2 arguments, got N`、
//!   号码非整数 ⇒ `'X' object cannot be interpreted as an integer`、越界 ⇒ `ValueError: signal number out of range`、
//!   handler 只收 `SIG_DFL`(0)／`SIG_IGN`(1)／可调用（**`bool` 除外**，照参照：`signal(2, True)` 也报
//!   `signal handler must be …`）、`getsignal` 的 `_signal.getsignal() takes exactly one argument (N given)`。
//!
//! ## 如实报错面（硬边界 ✓：**禁止假装注册成功**）
//!
//! 真信号注册／查询是**能力域**问题（`CM-8`）——本轮**不做** syscall ⇒ `signal()` 与 `getsignal()`
//! **参数校验全过后调用即** `NotImplementedError`（消息含"未接线"）。
//! **为什么不给 `getsignal` 返回 `None`**（实测决定 ✓）：参照对**有效号码**从不返回 `None` ——
//! 未注册的处理器返回 `SIG_DFL`(0)（`_signal.getsignal(10) ⇒ 0`），`SIGINT` 返回 `default_int_handler`；
//! 返回 `None` 会是**编出来的事实** ✗ ⇒ 统一如实报未接线 ✓。
//!
//! ## 未落地（如实记）
//!
//! - 两个函数的 `__doc__`：core 给**原生函数**统一空串（第 406 轮口径）⇒ `@_wraps(_signal.signal)`
//!   读得到属性、赋值不报错、`import signal` 不受影响 ✓；参照那一份 C 文档串要 core 扩展函数文档存储
//!   （不在本轮写域）。**模块** `_signal.__doc__` 是照参照抄的非空串 ✓。
//! - 真信号 handler 表（运行期；下一轮走能力域槽位）。
//!
//! 本模块**不碰平台**（`CX-4`：`pyawa-stdlib/src` 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, NativeFn};

/// 模块名（`_signal`）。
pub const NAME: &str = "_signal";

/// 模块 `__doc__`（**照参照逐字抄** ✓：`python3 -c "import _signal; print(_signal.__doc__)"`）。
pub const DOC: &str = r#"This module provides mechanisms to use signal handlers in Python.

Functions:

alarm() -- cause SIGALRM after a specified time [Unix only]
setitimer() -- cause a signal (described below) after a specified
               float time and the timer may restart then [Unix only]
getitimer() -- get current value of timer [Unix only]
signal() -- set the action for a given signal
getsignal() -- get the signal action for a given signal
pause() -- wait until a signal arrives [Unix only]
default_int_handler() -- default SIGINT handler

signal constants:
SIG_DFL -- used to refer to the system default handler
SIG_IGN -- used to ignore the signal
NSIG -- number of defined signals
SIGINT, SIGTERM, etc. -- signal numbers

itimer constants:
ITIMER_REAL -- decrements in real time, and delivers SIGALRM upon
               expiration
ITIMER_VIRTUAL -- decrements only when the process is executing,
               and delivers SIGVTALRM upon expiration
ITIMER_PROF -- decrements both when the process is executing and
               when the system is executing on behalf of the process.
               Coupled with ITIMER_VIRTUAL, this timer is usually
               used to profile the time spent by the application
               in user and kernel space. SIGPROF is delivered upon
               expiration.


*** IMPORTANT NOTICE ***
A signal handler function is called with two arguments:
the first is the signal number, the second is the interrupted stack frame."#;

/// **合法信号号码的上界**（参照实测 ✓：`1..=64` 有效、`0`／`65` ⇒ `ValueError`；
/// 与常量表里的 `NSIG` **同一处** ✓）。
const NSIG: i64 = 65;

/// 整型常量（**值照参照实测** ✓：本机 CPython 3.14.4／Linux）。
const INT_CONSTANTS: &[(&str, i64)] = &[
    ("ITIMER_PROF", 2),
    ("ITIMER_REAL", 0),
    ("ITIMER_VIRTUAL", 1),
    ("NSIG", NSIG),
    ("SIGABRT", 6),
    ("SIGALRM", 14),
    ("SIGBUS", 7),
    ("SIGCHLD", 17),
    ("SIGCLD", 17),
    ("SIGCONT", 18),
    ("SIGFPE", 8),
    ("SIGHUP", 1),
    ("SIGILL", 4),
    ("SIGINT", 2),
    ("SIGIO", 29),
    ("SIGIOT", 6),
    ("SIGKILL", 9),
    ("SIGPIPE", 13),
    ("SIGPOLL", 29),
    ("SIGPROF", 27),
    ("SIGPWR", 30),
    ("SIGQUIT", 3),
    ("SIGRTMAX", 64),
    ("SIGRTMIN", 34),
    ("SIGSEGV", 11),
    ("SIGSTKFLT", 16),
    ("SIGSTOP", 19),
    ("SIGSYS", 31),
    ("SIGTERM", 15),
    ("SIGTRAP", 5),
    ("SIGTSTP", 20),
    ("SIGTTIN", 21),
    ("SIGTTOU", 22),
    ("SIGURG", 23),
    ("SIGUSR1", 10),
    ("SIGUSR2", 12),
    ("SIGVTALRM", 26),
    ("SIGWINCH", 28),
    ("SIGXCPU", 24),
    ("SIGXFSZ", 25),
    ("SIG_BLOCK", 0),
    ("SIG_DFL", 0),
    ("SIG_IGN", 1),
    ("SIG_SETMASK", 2),
    ("SIG_UNBLOCK", 1),
];

/// 建 `_signal` 的命名空间（**新引用**的 `dict`；`CM-6`：模块**存在**即可 import）。
///
/// `__all__` **刻意收窄**（见模块头 ✓）：常量 ＋ `signal`／`getsignal`，四个可选包装因此不定义 ✓。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let mut exports: Vec<&'static str> = Vec::with_capacity(INT_CONSTANTS.len() + 2);
    for (name, value) in INT_CONSTANTS {
        instance.dict_set(namespace, name, instance.new_int(*value));
        exports.push(name);
    }
    let signal = make_native(instance, "signal", signal_native);
    instance.dict_set(namespace, "signal", signal);
    exports.push("signal");
    let getsignal = make_native(instance, "getsignal", getsignal_native);
    instance.dict_set(namespace, "getsignal", getsignal);
    exports.push("getsignal");
    let exports = instance.new_list(exports.iter().map(|name| instance.new_str(name)).collect());
    instance.dict_set(namespace, "__all__", exports);
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    namespace
}

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

// ---- 参数校验（照参照实测的文案分型 ✓） ---------------------------------------------

fn type_error(instance: &Instance, text: &str) -> pyawa_core::ExecError {
    instance.raise_builtin_error("TypeError", text)
}

fn range_error(instance: &Instance) -> pyawa_core::ExecError {
    instance.raise_builtin_error("ValueError", "signal number out of range")
}

/// 关键字一律拒（参照实测：`_signal.getsignal(signalnum=2)` ⇒ `takes no keyword arguments`，
/// 且**先于**元数检查 ⇒ 两个 native 都先调它 ✓）。
fn reject_keywords(
    instance: &Instance,
    name: &str,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<(), pyawa_core::ExecError> {
    if kwargs.is_empty() {
        return Ok(());
    }
    Err(type_error(
        instance,
        &format!("_signal.{name}() takes no keyword arguments"),
    ))
}

/// 信号号码：先当整数（非整数 ⇒ `'X' object cannot be interpreted as an integer`），
/// 再查范围（`1..NSIG`，越界 ⇒ `ValueError: signal number out of range`）—— 照参照的检查顺序 ✓。
fn signal_number(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<i64, pyawa_core::ExecError> {
    let Some(number) = instance.int_value(object) else {
        let type_name = instance.type_name(instance.type_of(object));
        return Err(type_error(
            instance,
            &format!("'{type_name}' object cannot be interpreted as an integer"),
        ));
    };
    if number < 1 || number >= NSIG {
        return Err(range_error(instance));
    }
    Ok(number)
}

/// handler 只收 `SIG_DFL`(0)／`SIG_IGN`(1)／可调用对象；**`bool` 不算 int**（照参照：
/// `signal(2, True)` ⇒ `signal handler must be …` ✓ —— 参照走 `PyLong_CheckExact` ✓）。
fn handler_is_valid(instance: &Instance, object: NonNull<Header>) -> bool {
    if instance.is_bool(object) {
        return false;
    }
    if let Some(number) = instance.int_value(object) {
        return number == 0 || number == 1;
    }
    instance.is_callable(object)
}

/// 未接线的如实报错（**不注册、不返回假事实** ✓）。
fn not_wired(instance: &Instance, name: &str) -> pyawa_core::ExecError {
    instance.raise_builtin_error(
        "NotImplementedError",
        &format!("`_signal.{name}`：真实信号注册／查询是能力域（CM-8）问题，本轮未接线"),
    )
}

/// `_signal.signal(signalnum, handler, /)`。
///
/// 参照实测的错误面：关键字 ⇒ `takes no keyword arguments`；元数 ⇒ `signal expected 2 arguments, got N`
/// （**这是参照自己的文案**，不是通用格式 ✓）；号码／handler 校验见上面两个 helper。
fn signal_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    reject_keywords(instance, "signal", kwargs)?;
    if args.len() != 2 {
        return Err(type_error(
            instance,
            &format!("signal expected 2 arguments, got {}", args.len()),
        ));
    }
    signal_number(instance, args[0])?;
    if !handler_is_valid(instance, args[1]) {
        return Err(type_error(
            instance,
            "signal handler must be signal.SIG_IGN, signal.SIG_DFL, or a callable object",
        ));
    }
    Err(not_wired(instance, "signal"))
}

/// `_signal.getsignal(signalnum, /)`。
fn getsignal_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    reject_keywords(instance, "getsignal", kwargs)?;
    if args.len() != 1 {
        return Err(type_error(
            instance,
            &format!(
                "_signal.getsignal() takes exactly one argument ({} given)",
                args.len()
            ),
        ));
    }
    signal_number(instance, args[0])?;
    Err(not_wired(instance, "getsignal"))
}
