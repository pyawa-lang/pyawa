//! `_signal` 模块（**记账式最小面** —— 2026-11-14 方案 A ✓）：`import signal`／`asyncio` 的运行闸。
//!
//! ## 为什么是它（`PLAN-milestones.md` §12.2）
//!
//! `asyncio` 的事件循环在**跑第一个协程之前**就依赖 `_signal`：`Lib/asyncio/runners.py:112` 读
//! `signal.getsignal(signal.SIGINT) is signal.default_int_handler`，`:116` `signal.signal(...)` 注册
//! SIGINT 处理器，`:136`／`:138` 收尾读回并恢复。先前两个函数**调用即 `NotImplementedError`**
//! ⇒ `asyncio.run()` 在起点即红 ✓。
//!
//! ## 方案 A：记账式（只记录、不投递）
//!
//! 依据 `docs/REQUIREMENTS.md`「信号面（2026-11-14）」行 ＋ `docs/PLAN-milestones.md` §12.2
//! （用户拍板选 A）：**不做真实信号投递**，只维护一张**记账表**；`signal()` 记账并返回**前值**，
//! `getsignal()` 读记账表。理由（原文）：POSIX 信号处置是**进程级**状态，与 `CP-13`／`OM-1` 的
//! 每实例隔离直接冲突，且会让脚本能改写宿主退出路径；而 asyncio 真正吃信号的场景（子进程
//! `SIGCHLD`／Ctrl-C）在本层本就非目标。代价（原文）：注册的 handler **永不触发** ⇒ 在
//! `tests/conformance/divergences.md` 记 `DIV-10`，合约（`SPEC-c-modules.md` §5.2.11）写明边界。
//!
//! **不调用宿主信号设施**（`CM-8` 禁区；`CX-4`：`pyawa-stdlib` 不碰平台，本文件只有对象操作）。
//!
//! ## 已落地面（照本机 `python3` 3.14.4 实测）
//!
//! - **45 个整型常量**（task-13 旧面，值逐个照参照）＋ 模块 `__doc__`（照参照原文）；
//! - `signal(signalnum, handler, /)`：参数校验（关键字／元数／非整数／越界／handler）逐字照参照；
//!   校验通过后**记账**并返回**前值**：未注册的普通信号 ⇒ `SIG_DFL`(0)、`SIGINT` ⇒
//!   `default_int_handler` 对象（同一对象，`is` 成立）、32／33 保留号 ⇒ 参照 `getsignal` 实测 `None`。
//! - `getsignal(signalnum, /)`：读记账表。初值：`SIGINT` ⇒ 本模块的 `default_int_handler`（**同一对象**
//!   ⇒ `Lib/asyncio/runners.py:112` 的身份判定成立）、32／33 ⇒ `None`、其余 ⇒ `SIG_DFL`(0)。
//! - `default_int_handler(signum, frame, /)`：存在、可调用、**调用必抛 `KeyboardInterrupt`**（照参照；
//!   本层永无自动触发）。元数／关键字文案照参照（`default_int_handler expected 2 arguments, got N`／
//!   `_signal.default_int_handler() takes no keyword arguments`）。
//! - `set_wakeup_fd(fd, /, *, warn_on_full_buffer=True)`：**记账式**——当前 wakeup fd 初值 `-1`；
//!   只接受 `-1`（≡ 解除），返回前值；元数／关键字／非整数文案照参照实测。
//! - **不可捕获号**（9 `SIGKILL`／19 `SIGSTOP`／32／33 保留号）：照参照实测折成
//!   `OSError: [Errno 22] Invalid argument` —— 这是**钉住的参照可观察值**，不是宿主调用。
//!
//! ## 每实例状态（`CX-3`／`T-CX-3`：禁止全局可变状态）
//!
//! 记账表存在**模块自身命名空间的隐藏键**里（`__pyawa_signal_handlers__`：dict，
//! 号码 → handler；`__pyawa_signal_wakeup_fd__`：int），随实例／模块走 ⇒ **每实例一份**。
//! 可观察偏差：`vars(_signal)` 比参照多这两个键（`__all__` 不含 ⇒ `from _signal import *` 不导出）。
//!
//! ## 如实未落地／已知偏差（不假装成功 ✗）
//!
//! - **handler 永不触发**（方案 A 的代价；`DIV-10`）；
//! - **初值表不反映宿主继承的处置**：参照进程里 `SIGPIPE`(13)／`SIGXFSZ`(25) 初始是 `SIG_IGN`
//!   （解释器／父进程装过），本层记账表一律 `SIG_DFL` —— "记账"与"读宿主"的定义差（`DIV-10` 归一）；
//! - `set_wakeup_fd(fd >= 0)` 需要校验宿主 fd（有效＋非阻塞）：**无法记账** ⇒ 如实报
//!   `NotImplementedError`（`CM-6`）⇒ 本层只能"解除"（`-1`）；
//! - **导出面刻意收窄**（沿用 task-13 口径）：`pthread_sigmask`／`sigpending`／`sigwait`／
//!   `valid_signals` 不导出 ⇒ `Lib/signal.py` 的四个包装不定义、`Sigmasks` 不建；其余 17 个函数与
//!   2 个类型不导出 ⇒ `Lib/asyncio/unix_events.py` 的 `add_signal_handler` 用的
//!   `siginterrupt`／`valid_signals` **缺名**（该路径如实不可用；`asyncio.run` 常规路径不需要）；
//! - 原生函数 `__doc__` 空串（core 口径）⇒ `signal.signal.__doc__` 读回空串（参照给 C 文档串）。
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

/// `SIG_DFL`（未注册／"系统默认"）。
const SIG_DFL: i64 = 0;

/// `SIGINT` 的号码。
const SIGINT: i64 = 2;

/// **记账表**的隐藏键（模块命名空间里；不在 `__all__` ⇒ `from _signal import *` 不导出）。
const HANDLERS_KEY: &str = "__pyawa_signal_handlers__";
/// **当前 wakeup fd** 的隐藏键（初值 `-1`）。
const WAKEUP_FD_KEY: &str = "__pyawa_signal_wakeup_fd__";

/// 参照实测：这两个**保留号**（glibc 内部 RT 信号）宿主不可查询 ⇒ `getsignal` 返回 `None`、
/// `signal` 报 `OSError EINVAL`。记账层照抄该可观察值（见模块头"不可捕获号"）。
const UNQUERYABLE: &[i64] = &[32, 33];

/// 不可捕获号码：`SIGKILL`(9)／`SIGSTOP`(19)（POSIX）＋ 两个保留号（参照实测 `OSError EINVAL`）。
fn is_uncatchable(number: i64) -> bool {
    matches!(number, 9 | 19) || UNQUERYABLE.contains(&number)
}

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
    ("SIGINT", SIGINT),
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
/// 导出面（`__all__`）＝ **45 个常量** ＋ `signal`／`getsignal`／`default_int_handler`／
/// `set_wakeup_fd`；四个可选包装与其余 17 个函数／2 个类型**不导出**（见模块头"导出面刻意收窄"）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let mut exports: Vec<&'static str> = Vec::with_capacity(INT_CONSTANTS.len() + 4);
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
    // `default_int_handler` **必须导出**：`Lib/signal.py` 走 `from _signal import *`，而
    // `Lib/asyncio/runners.py:112` 判 `signal.getsignal(signal.SIGINT) is signal.default_int_handler`
    // ⇒ `signal.default_int_handler` 必须与 `getsignal(2)` 返回的是**同一个对象**。
    let default_int_handler = make_native(instance, "default_int_handler", default_int_handler_native);
    instance.dict_set(namespace, "default_int_handler", default_int_handler);
    exports.push("default_int_handler");
    let set_wakeup_fd = make_native(instance, "set_wakeup_fd", set_wakeup_fd_native);
    instance.dict_set(namespace, "set_wakeup_fd", set_wakeup_fd);
    exports.push("set_wakeup_fd");
    // **记账表（每实例）**：号码 → handler；初值照参照实测：`SIGINT` ⇒ `default_int_handler`、
    // 32／33 ⇒ `None`、其余缺席 ⇒ `getsignal` 给 `SIG_DFL`(0)。
    let handlers = instance.new_dict();
    let none = instance.new_none();
    instance.dict_set_int(handlers, SIGINT, default_int_handler);
    instance.dict_set_int(handlers, UNQUERYABLE[0], none);
    instance.dict_set_int(handlers, UNQUERYABLE[1], none);
    instance.dict_set(namespace, HANDLERS_KEY, handlers);
    // **当前 wakeup fd（每实例）**：初值 `-1`（参照实测；`set_wakeup_fd` 只做记账）。
    let wakeup_fd = instance.new_int(-1);
    instance.dict_set(namespace, WAKEUP_FD_KEY, wakeup_fd);
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

// ---- 每实例记账状态（模块命名空间里的隐藏键；`CX-3`：本文件无任何全局可变状态） --------------

/// 读本模块的**命名空间**（依赖模块已登记进 `sys.modules`；本 VM 的正常路径成立 ✓）。
fn module_namespace(instance: &Instance) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    pyawa_core::mounted_instance_dict(instance, module)
}

/// 读一个隐藏状态键（**借用**）。
fn module_state(instance: &Instance, key: &str) -> Option<NonNull<Header>> {
    instance.dict_get(module_namespace(instance)?, key)
}

/// 记账表缺失 ⇒ 内部状态坏了（**如实报**，不能静默给 `SIG_DFL` 假事实）。
fn handlers_table(instance: &Instance) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    module_state(instance, HANDLERS_KEY).ok_or_else(|| {
        instance.raise_builtin_error(
            "RuntimeError",
            "`_signal`：每实例记账表缺失（模块状态未初始化）",
        )
    })
}

/// 读记账表里该号码的 handler（**借用**；没有条目 ⇒ `None`）。
fn accounted_handler(instance: &Instance, table: NonNull<Header>, number: i64) -> Option<NonNull<Header>> {
    instance
        .dict_entries(table)?
        .into_iter()
        .find(|(key, _)| instance.int_value(*key) == Some(number))
        .map(|(_, value)| value)
}

// ---- 参数校验（照参照实测的文案分型 ✓） ---------------------------------------------

fn type_error(instance: &Instance, text: &str) -> pyawa_core::ExecError {
    instance.raise_builtin_error("TypeError", text)
}

fn range_error(instance: &Instance) -> pyawa_core::ExecError {
    instance.raise_builtin_error("ValueError", "signal number out of range")
}

/// 关键字一律拒（参照实测：`_signal.getsignal(signalnum=2)` ⇒ `takes no keyword arguments`，
/// 且**先于**元数检查 ⇒ `getsignal`／`signal`／`default_int_handler` 都先调它 ✓）。
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

/// 实参 ⇒ `i64`：非整数 ⇒ `'X' object cannot be interpreted as an integer`；**装不下 `i64` 的
/// 大整数** ⇒ 参照的 `OverflowError: Python int too large to convert to C int`（本机实测 ✓）。
fn integer_argument(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<i64, pyawa_core::ExecError> {
    let Some(value) = instance.int_of(object) else {
        let type_name = instance.type_name(instance.type_of(object));
        return Err(type_error(
            instance,
            &format!("'{type_name}' object cannot be interpreted as an integer"),
        ));
    };
    value.to_i64().ok_or_else(|| {
        instance.raise_builtin_error(
            "OverflowError",
            "Python int too large to convert to C int",
        )
    })
}

/// 信号号码：先当整数（含大整数溢出文案），再查范围（`1..NSIG`，越界 ⇒
/// `ValueError: signal number out of range`）—— 照参照的检查顺序 ✓。
fn signal_number(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<i64, pyawa_core::ExecError> {
    let number = integer_argument(instance, object)?;
    if !(1..NSIG).contains(&number) {
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
        return number == SIG_DFL || number == 1;
    }
    instance.is_callable(object)
}

// ---- 记账式 `_signal` 面 ------------------------------------------------------------------

/// `_signal.signal(signalnum, handler, /)`：**记账**并返回**前值**（方案 A；不注册、不投递）。
///
/// 参照实测的错误面：关键字 ⇒ `takes no keyword arguments`；元数 ⇒ `signal expected 2 arguments, got N`
/// （**这是参照自己的文案**，不是通用格式 ✓）；号码／handler 校验见上面两个 helper；校验全过后
/// 不可捕获号 ⇒ 参照的 `OSError: [Errno 22] Invalid argument`。
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
    let number = signal_number(instance, args[0])?;
    if !handler_is_valid(instance, args[1]) {
        return Err(type_error(
            instance,
            "signal handler must be signal.SIG_IGN, signal.SIG_DFL, or a callable object",
        ));
    }
    // 不可捕获号：照参照实测（handler 校验**之后**才由宿主拒绝 ⇒ 文案顺序照抄）。
    if is_uncatchable(number) {
        return Err(instance.raise_builtin_error(
            "OSError",
            "[Errno 22] Invalid argument",
        ));
    }
    let table = handlers_table(instance)?;
    let previous = match accounted_handler(instance, table, number) {
        Some(handler) => instance.retain(handler),
        None => instance.new_int(SIG_DFL),
    };
    // `dict_set_int` 是**借用＋自留一份**（`OM-` 口径）⇒ 直接把调用方的借来 handler 记进去 ✓。
    instance.dict_set_int(table, number, args[1]);
    Ok(previous)
}

/// `_signal.getsignal(signalnum, /)`：读记账表。
///
/// 初值（模块 `build` 时写死）：`SIGINT` ⇒ `default_int_handler`（**同一对象**）、32／33 ⇒ `None`、
/// 其余 ⇒ `SIG_DFL`(0)（记账表缺席即 `SIG_DFL`）。
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
    let number = signal_number(instance, args[0])?;
    let table = handlers_table(instance)?;
    Ok(match accounted_handler(instance, table, number) {
        Some(handler) => instance.retain(handler),
        None => instance.new_int(SIG_DFL),
    })
}

/// `_signal.default_int_handler(signum, frame, /)`：存在、可调用、**调用必抛 `KeyboardInterrupt`**
/// （照参照；本层记账式、永不自动触发）。两种实参都收下但不用（参照的 C 函数也只抛异常）。
fn default_int_handler_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    reject_keywords(instance, "default_int_handler", kwargs)?;
    if args.len() != 2 {
        return Err(type_error(
            instance,
            &format!(
                "default_int_handler expected 2 arguments, got {}",
                args.len()
            ),
        ));
    }
    Err(instance.raise_builtin_error("KeyboardInterrupt", ""))
}

/// `_signal.set_wakeup_fd(fd, /, *, warn_on_full_buffer=True)`：**记账式**（不真正写 fd）。
///
/// - 当前 wakeup fd 初值 `-1`，返回**前值**（参照实测：`_signal.set_wakeup_fd(-1) ⇒ -1`）；
/// - 只接受 `-1`（≡ 解除）：其它整数要校验**宿主 fd**（有效＋非阻塞）⇒ **无法记账** ⇒ 如实报
///   `NotImplementedError`（`CM-6`，**禁止假成功**）；
/// - 元数／关键字／非整数文案照参照实测：`set_wakeup_fd() takes exactly 1 positional argument (N given)`
///   （`N=0`／`2`）、`N>2` ⇒ `takes at most 2 arguments (N given)`、
///   `got an unexpected keyword argument 'X'`、`'X' object cannot be interpreted as an integer`。
fn set_wakeup_fd_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if args.len() != 1 {
        let text = if args.len() > 2 {
            format!("set_wakeup_fd() takes at most 2 arguments ({} given)", args.len())
        } else {
            format!(
                "set_wakeup_fd() takes exactly 1 positional argument ({} given)",
                args.len()
            )
        };
        return Err(type_error(instance, &text));
    }
    for (key, _value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            continue;
        };
        if name != "warn_on_full_buffer" {
            return Err(type_error(
                instance,
                &format!("set_wakeup_fd() got an unexpected keyword argument '{name}'"),
            ));
        }
    }
    let fd = integer_argument(instance, args[0])?;
    if fd != -1 {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "`_signal.set_wakeup_fd`：非 -1 的 fd 要校验宿主 fd（有效／非阻塞），记账层无法判定（CM-6）",
        ));
    }
    let Some(state) = module_state(instance, WAKEUP_FD_KEY) else {
        return Err(instance.raise_builtin_error(
            "RuntimeError",
            "`_signal`：每实例 wakeup fd 状态缺失（模块状态未初始化）",
        ));
    };
    // 读前值（**借用** ⇒ `retain` 交出新引用）；再记账回 `-1`（`dict_set` 借用＋自留一份）。
    let previous = match instance.int_value(state) {
        Some(value) => instance.new_int(value),
        None => instance.new_int(-1),
    };
    instance.dict_set(module_namespace(instance).expect("上面刚取过"), WAKEUP_FD_KEY, instance.new_int(-1));
    Ok(previous)
}
