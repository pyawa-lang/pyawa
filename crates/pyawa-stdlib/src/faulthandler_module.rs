//! `faulthandler` 模块（P3-38 S3e-2，task-65）：**最小诚实面**。
//!
//! 参照 3.14.4 的 `faulthandler` 是**信号/致命错误**钩子：`enable()` 装 SIGSEGV/SIGFPE/… 处理器，
//! 崩溃时把 Python 栈打到 `file`。本 VM 的情况（如实）：
//! - **没有信号/致命错误钩子**（`_signal` 只是最小面，且崩溃走的是 Rust panic/abort）⇒
//!   `enable`/`disable`/`is_enabled` 只维护**真实的开关状态**（`is_enabled()` 与参照一致），
//!   但**不会**在崩溃时打印 —— 这是本层的**能力边界**，如实写在 `__doc__` 与函数注释里，
//!   **不冒充**"已装处理器"；
//! - `dump_traceback` **真实现**：沿 `sys._getframe()` 的 `f_back` 链把当前调用栈写进 `file`
//!   （默认 `sys.stderr`），格式对照参照实测（`Current thread 0x… [name] (most recent call first):`
//!   ＋ 每条 `  File "…", line N in func`）—— 线程只有主线程 ⇒ 名字用 `[pyawa]`、id 用 `0x0` ✓；
//! - `register`/`unregister`：**登记表真实现**（`unregister` 的返回值就是"登记过没有"），但
//!   **信号到达时不会调用**（同上：没有信号/崩溃钩子）⇒ 函数注释如实说明；
//! - `dump_traceback_later`（要定时器线程）／`dump_c_stack`（要 C 栈遍历）／崩溃触发函数
//!   （`_sigsegv` 等）**如实 `NotImplementedError`**（不伪造崩溃/定时输出）。
//!
//! 动因（S3e-2）：survey 里 `ModuleNotFoundError: No module named 'faulthandler'` ×3
//! （`test_faulthandler`／`test_regrtest`／`test_xxtestfuzz`）；官方测试**导入期**就 `import
//! faulthandler` 并读名字 ⇒ 先把"名字与真实可做的那部分"接上。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};

/// 模块名（`faulthandler`）。
pub const NAME: &str = "faulthandler";

/// 模块的 `__doc__`（如实写能力边界）。
pub const DOC: &str = "faulthandler: minimal honest surface. State toggling and \
dump_traceback(file, all_threads) are real; signal/crash hooks, timers and C-stack dumps are \
not available in this VM (NotImplementedError).";

/// 状态键：是否 enabled（真值对象）。
const ENABLED_KEY: &str = "__pyawa_faulthandler_enabled__";
/// 状态键：已登记的 signum 表（dict：signum → None）。
const REGISTERED_KEY: &str = "__pyawa_faulthandler_registered__";

/// 读本模块的私有状态（依赖模块已登记进 `sys.modules`；本 VM 的正常路径成立 ✓）。
fn module_state(instance: &Instance, key: &str) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    let namespace = pyawa_core::mounted_instance_dict(instance, module)?;
    instance.dict_get(namespace, key)
}

/// 写回私有状态（`dict_set` 是"接管"语义 ⇒ 传进来的必须是新引用 ✓）。
fn set_module_state(instance: &Instance, key: &str, value: NonNull<Header>) {
    let Some(modules) = instance.modules() else { return };
    let Some(module) = instance.dict_get(modules, NAME) else {
        return;
    };
    let Some(namespace) = pyawa_core::mounted_instance_dict(instance, module) else {
        return;
    };
    instance.dict_set(namespace, key, value);
}

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
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

/// 当前开关状态（没有状态对象 ⇒ `False` ✓）。
fn is_enabled(instance: &Instance) -> bool {
    module_state(instance, ENABLED_KEY)
        .and_then(|value| instance.bool_value(value))
        .unwrap_or(false)
}

/// `None`/缺省 → `True`（参照 `file=None` 的默认语义）。
fn none_or(instance: &Instance, value: Option<NonNull<Header>>) -> bool {
    match value {
        None => true,
        Some(value) => value == instance.singletons().none(),
    }
}

/// `faulthandler.enable(file=sys.stderr, all_threads=True)`（P3-38 S3e-2）。
///
/// **真**：开关状态（`is_enabled()` 跟着变）；**边界**：本 VM 没有信号/崩溃钩子 ⇒ 崩溃时不会
/// 打印栈（注释与 `__doc__` 都如实写明，不冒充）。多给的**位置**实参照实现（3.14.4）实测**接受** ✓；
/// 未知关键字照参照报 `enable() got an unexpected keyword argument '…'` ✓。
fn enable_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    for (key, _value) in kwargs {
        let Some(name) = instance.text_of(*key) else { continue };
        if name != "file" && name != "all_threads" {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("enable() got an unexpected keyword argument '{name}'"),
            ));
        }
    }
    // `file`／`all_threads` 收下但不产生"装处理器"的效果（能力边界见模块头）。
    let _ = args;
    let enabled = instance.singletons().boolean(true);
    instance.retain(enabled);
    set_module_state(instance, ENABLED_KEY, enabled);
    Ok(instance.retain(instance.singletons().none()))
}

/// `faulthandler.disable()` ⇒ **返回之前是否 enabled**（参照实测：未开 ⇒ `False`）✓。
fn disable_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("faulthandler.disable() takes no arguments ({} given)", args.len()),
        ));
    }
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "faulthandler.disable() takes no keyword arguments",
        ));
    }
    let previous = is_enabled(instance);
    let disabled = instance.singletons().boolean(false);
    instance.retain(disabled);
    set_module_state(instance, ENABLED_KEY, disabled);
    Ok(instance.new_bool(previous))
}

/// `faulthandler.is_enabled()` ⇒ 当前状态（无参；多给照参照报 `takes no arguments`）✓。
fn is_enabled_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("faulthandler.is_enabled() takes no arguments ({} given)", args.len()),
        ));
    }
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "faulthandler.is_enabled() takes no keyword arguments",
        ));
    }
    Ok(instance.new_bool(is_enabled(instance)))
}

/// 把**当前帧链**渲染成参照格式的文本（`dump_traceback` 的真实现）。
fn render_current_stack(instance: &Instance) -> Result<String, ExecError> {
    // **线程标签**：本 VM 只有主线程 ⇒ `0x0 [pyawa]`（参照给真实线程 id/名；如实不同，不伪造成
    // 十六进制随机数）。
    let mut text = String::from("Current thread 0x0 [pyawa] (most recent call first):\n");
    let mut frame = instance.current_frame();
    while let Some(current) = frame {
        let code = instance
            .attribute_optional_of(current, "f_code")
            .unwrap_or(None);
        let (filename, name) = match code {
            Some(code) => {
                let filename = instance
                    .attribute_optional_of(code, "co_filename")
                    .unwrap_or(None)
                    .and_then(|value| instance.text_of(value).map(str::to_owned))
                    .unwrap_or_else(|| "<unknown>".to_owned());
                let name = instance
                    .attribute_optional_of(code, "co_name")
                    .unwrap_or(None)
                    .and_then(|value| instance.text_of(value).map(str::to_owned))
                    .unwrap_or_else(|| "<unknown>".to_owned());
                (filename, name)
            }
            None => ("<unknown>".to_owned(), "<unknown>".to_owned()),
        };
        let line = instance
            .attribute_optional_of(current, "f_lineno")
            .unwrap_or(None)
            .and_then(|value| instance.int_of(value).and_then(|int| int.to_i64()))
            .unwrap_or(0);
        text.push_str(&format!("  File \"{filename}\", line {line} in {name}\n"));
        // `f_back` 属性在栈底给的是 `None` **单例**（不是 Rust `Option::None`）⇒ 必须过滤掉，
        // 否则会多渲染一个 `<unknown>` 帧 ✓。
        frame = instance
            .attribute_optional_of(current, "f_back")
            .unwrap_or(None)
            .filter(|parent| *parent != instance.singletons().none());
    }
    Ok(text)
}

/// 默认输出目标：`sys.stderr`（从 `sys.modules` 的 `sys` 命名空间取；取不到 ⇒ `NotImplementedError`）。
fn default_file(instance: &Instance) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let sys_module = instance.dict_get(modules, "sys")?;
    let namespace = pyawa_core::mounted_instance_dict(instance, sys_module)?;
    instance.dict_get(namespace, "stderr")
}

/// 往 `file.write(text)` 写一段（返回写入的字符数，参照 `TextIOWrapper.write` 口径）。
fn write_to_file(instance: &Instance, file: NonNull<Header>, text: &str) -> Result<(), ExecError> {
    let Some(write) = instance.attribute_optional_of(file, "write")? else {
        return Err(instance.raise_builtin_error(
            "RuntimeError",
            "faulthandler: the file has no write() method",
        ));
    };
    let value = instance.new_str(text);
    let result = pyawa_core::executor::call::call_value(instance, write, &[value], &[]);
    // 本函数的新引用；`call_value` 自己 retain 过一份 ⇒ 用完归还 ✓。
    instance.release(value);
    match result {
        Ok(written) => {
            instance.release(written);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// `faulthandler.dump_traceback(file=sys.stderr, all_threads=True)`（**真实现**）。
fn dump_traceback_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("dump_traceback() takes at most 2 arguments ({} given)", args.len()),
        ));
    }
    let mut file = args.first().copied();
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else { continue };
        match name {
            "file" => file = Some(*value),
            "all_threads" => {}
            other => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("dump_traceback() got an unexpected keyword argument '{other}'"),
                ))
            }
        }
    }
    let target = match file.filter(|value| none_or(instance, Some(*value)) == false) {
        Some(file) => file,
        None => default_file(instance).ok_or_else(|| {
            instance.raise_builtin_error(
                "RuntimeError",
                "faulthandler.dump_traceback(): sys.stderr is not available",
            )
        })?,
    };
    let text = render_current_stack(instance)?;
    write_to_file(instance, target, &text)?;
    Ok(instance.retain(instance.singletons().none()))
}

/// `dump_traceback_later(timeout, repeat=False, file=sys.stderr, exit=False)`（**定时器未接**，如实报）。
fn dump_traceback_later_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let timeout = args.first().copied().or_else(|| {
        kwargs
            .iter()
            .find(|(key, _)| instance.text_of(*key).as_deref() == Some("timeout"))
            .map(|(_, value)| *value)
    });
    let Some(timeout) = timeout else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "dump_traceback_later() missing required argument 'timeout' (pos 1)",
        ));
    };
    // 参照接受 **int 或 float**（`faulthandler.dump_traceback_later(1)` 合法 ⇒ 按 double 收 ✓）。
    let seconds = instance
        .float_value(timeout)
        .or_else(|| instance.int_of(timeout).and_then(|value| value.to_i64()).map(|value| value as f64));
    let Some(seconds) = seconds else {
        let name = instance.type_name(instance.type_of(timeout));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("mismatched type: expected a float, got {name}"),
        ));
    };
    if seconds <= 0.0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "timeout must be greater than 0",
        ));
    }
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`faulthandler.dump_traceback_later`：本 VM 没有定时器线程（能力边界，不伪造定时输出）",
    ))
}

/// `cancel_dump_traceback_later()`：没有定时器 ⇒ `None`（参照无活动定时器时也是 `None` ✓）。
fn cancel_dump_traceback_later_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}

/// `faulthandler._fatal_error_c_thread`／`dump_c_stack`／崩溃触发函数：如实未接。
fn not_implemented_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`faulthandler` 的这一项需要 C 栈 / 信号 / 致命错误钩子（本 VM 没有；如实未接）",
    ))
}

/// `register(signum, file=sys.stderr, all_threads=True, chain=False)`：**登记表真实现**，
/// 但信号到达不会回调（本 VM 没有信号/崩溃钩子；注释与 `__doc__` 如实写明）。
fn register_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let signum = args.first().copied().or_else(|| {
        kwargs
            .iter()
            .find(|(key, _)| instance.text_of(*key).as_deref() == Some("signum"))
            .map(|(_, value)| *value)
    });
    let Some(signum) = signum else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "register() missing required argument 'signum' (pos 1)",
        ));
    };
    let Some(signal) = instance.index_value(signum)? else {
        let name = instance.type_name(instance.type_of(signum));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{name}' object cannot be interpreted as an integer"),
        ));
    };
    let mapping = module_state(instance, REGISTERED_KEY).unwrap_or_else(|| instance.new_dict());
    instance.dict_set_int(mapping, signal, instance.retain(instance.singletons().none()));
    set_module_state(instance, REGISTERED_KEY, mapping);
    Ok(instance.retain(instance.singletons().none()))
}

/// `unregister(signum)` ⇒ 这个 signum **登记过没有**（真值；参照同一语义）✓。
fn unregister_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let signum = args.first().copied().or_else(|| {
        kwargs
            .iter()
            .find(|(key, _)| instance.text_of(*key).as_deref() == Some("signum"))
            .map(|(_, value)| *value)
    });
    let Some(signum) = signum else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "unregister() missing required argument 'signum' (pos 1)",
        ));
    };
    let Some(signal) = instance.index_value(signum)? else {
        let name = instance.type_name(instance.type_of(signum));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{name}' object cannot be interpreted as an integer"),
        ));
    };
    let Some(mapping) = module_state(instance, REGISTERED_KEY) else {
        return Ok(instance.new_bool(false));
    };
    let entries = instance.dict_entries(mapping).unwrap_or_default();
    let position = entries
        .iter()
        .position(|(key, _)| instance.int_of(*key).and_then(|int| int.to_i64()) == Some(signal));
    let Some(position) = position else {
        return Ok(instance.new_bool(false));
    };
    // 重建一张去掉该键的表（`dict` 没有公开的 del；key/value 借用 ⇒ 自己 retain）。
    let rebuilt = instance.new_dict();
    for (index, (key, value)) in entries.into_iter().enumerate() {
        if index == position {
            continue;
        }
        instance.retain(key);
        instance.retain(value);
        instance.dict_insert_raw(rebuilt, key, value);
    }
    set_module_state(instance, REGISTERED_KEY, rebuilt);
    Ok(instance.new_bool(true))
}

/// 建 `faulthandler` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let false_value = instance.singletons().boolean(false);
    instance.retain(false_value);
    instance.dict_set(namespace, ENABLED_KEY, false_value);
    let registered = instance.new_dict();
    instance.dict_set(namespace, REGISTERED_KEY, registered);
    for (name, handler) in [
        ("enable", enable_native as pyawa_core::NativeFn),
        ("disable", disable_native as pyawa_core::NativeFn),
        ("is_enabled", is_enabled_native as pyawa_core::NativeFn),
        ("dump_traceback", dump_traceback_native as pyawa_core::NativeFn),
        (
            "dump_traceback_later",
            dump_traceback_later_native as pyawa_core::NativeFn,
        ),
        (
            "cancel_dump_traceback_later",
            cancel_dump_traceback_later_native as pyawa_core::NativeFn,
        ),
        ("register", register_native as pyawa_core::NativeFn),
        ("unregister", unregister_native as pyawa_core::NativeFn),
        ("dump_c_stack", not_implemented_native as pyawa_core::NativeFn),
        (
            "_fatal_error_c_thread",
            not_implemented_native as pyawa_core::NativeFn,
        ),
        ("_read_null", not_implemented_native as pyawa_core::NativeFn),
        ("_sigabrt", not_implemented_native as pyawa_core::NativeFn),
        ("_sigfpe", not_implemented_native as pyawa_core::NativeFn),
        ("_sigsegv", not_implemented_native as pyawa_core::NativeFn),
        ("_stack_overflow", not_implemented_native as pyawa_core::NativeFn),
    ] {
        let native = make_native(instance, name, handler);
        instance.dict_set(namespace, name, native);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
