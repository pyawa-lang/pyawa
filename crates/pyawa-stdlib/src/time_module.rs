//! `time` 模块（`SPEC-c-modules.md`；`clock` 域的第一个消费者 ✓）。
//!
//! **本段落地**（第 317 轮）：`time()`／`time_ns()`／`monotonic()`／`monotonic_ns()` ——
//! 全部**经 `clock` 域**取（`CX-4`：本 crate 不碰平台 ✓）。
//!
//! **如实登记的未接面** ✗（`CM-6`：未提供 ⇒ `ImportError`／未实现 ⇒ `NotImplementedError` ✓）：
//! `sleep()`（vtable 里的 `sleep_ns` 槽位本轮仍为 `None` ⇒ 调它报未实现 ✓）、
//! `perf_counter*`／`process_time*`／`thread_time*`（要另外的钟 ✓）、
//! `struct_time`／`gmtime`／`localtime`／`mktime`／`strftime`／`strptime`（要日历与时区表 ✓）、
//! `timezone`／`altzone`／`daylight`／`tzname`／`tzset`（要时区库 ✓）。
//!
//! 动因：上限诊断里 `ModuleNotFoundError: No module named 'time'` × **54** 个模块是本层最大一族 ✓。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`time`）。
pub const NAME: &str = "time";

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
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

/// 取一次 `clock` 域，失败时按三态分别报错（`CP-5`：未注册 ⇒ `NotImplementedError` 带上原因 ✓）。
fn clock_value(
    instance: &Instance,
    result: Result<i64, pyawa_core::CapabilityCallError>,
    what: &str,
) -> Result<i64, ExecError> {
    match result {
        Ok(value) => Ok(value),
        Err(pyawa_core::CapabilityCallError::NotImplemented) => Err(instance.raise_builtin_error(
            "NotImplementedError",
            &format!("`time.{what}`：`clock` 域的该槽位未提供"),
        )),
        Err(pyawa_core::CapabilityCallError::NotRegistered) => Err(instance.raise_builtin_error(
            "NotImplementedError",
            &format!("`time.{what}`：`clock` 域未注册"),
        )),
        Err(pyawa_core::CapabilityCallError::Machine(errno)) => Err(instance.raise_builtin_error(
            "OSError",
            &format!("`time.{what}`：机器错误（errno {errno}）"),
        )),
    }
}

/// `time.time()` ⇒ **浮点秒**（挂钟；参照给的也是浮点 ✓）。
fn time_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = clock_value(instance, instance.clock_now_ns(), "time")?;
    Ok(instance.new_float(value as f64 / 1_000_000_000.0))
}

/// `time.time_ns()` ⇒ **整数纳秒** ✓。
fn time_ns_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = clock_value(instance, instance.clock_now_ns(), "time_ns")?;
    Ok(instance.new_int(value))
}

/// `time.monotonic()` ⇒ 浮点秒（单调 ✓）。
fn monotonic_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = clock_value(instance, instance.clock_monotonic_ns(), "monotonic")?;
    Ok(instance.new_float(value as f64 / 1_000_000_000.0))
}

/// `time.monotonic_ns()` ⇒ 整数纳秒 ✓。
fn monotonic_ns_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = clock_value(instance, instance.clock_monotonic_ns(), "monotonic_ns")?;
    Ok(instance.new_int(value))
}

/// `time.perf_counter()`：本轮**照参照的"高分辨率单调钟"语义**，先用单调钟顶上 ✓
/// （**如实登记**：与 `monotonic` 同源，不是独立的计数器 ✓）。
fn perf_counter_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = clock_value(instance, instance.clock_monotonic_ns(), "perf_counter")?;
    Ok(instance.new_float(value as f64 / 1_000_000_000.0))
}

/// `time.perf_counter_ns()`（同上 ✓）。
fn perf_counter_ns_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = clock_value(instance, instance.clock_monotonic_ns(), "perf_counter_ns")?;
    Ok(instance.new_int(value))
}

/// `time.sleep(seconds)`：`sleep_ns` 槽位本轮仍为 `None` ⇒ **如实报未实现** ✓（不假装睡过 ✓）。
fn sleep_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`time.sleep`：`clock` 域的 `sleep_ns` 槽位未提供",
    ))
}

/// 建 `time` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, handler) in [
        ("time", time_native as NativeFn),
        ("time_ns", time_ns_native as NativeFn),
        ("monotonic", monotonic_native as NativeFn),
        ("monotonic_ns", monotonic_ns_native as NativeFn),
        ("perf_counter", perf_counter_native as NativeFn),
        ("perf_counter_ns", perf_counter_ns_native as NativeFn),
        ("sleep", sleep_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    // `timezone`／`altzone`／`daylight`／`tzname`：参照里是模块级变量 ✓ —— 本层给**如实**的值 ✓
    //（UTC 假定：偏移 0、无夏令时 ✓，**登记为偏差** ✗）。
    instance.dict_set(namespace, "timezone", instance.new_int(0));
    instance.dict_set(namespace, "altzone", instance.new_int(0));
    instance.dict_set(namespace, "daylight", instance.new_int(0));
    let tzname = instance.new_tuple(vec![instance.new_str("UTC"), instance.new_str("UTC")]);
    instance.dict_set(namespace, "tzname", tzname);
    let exports = instance.new_list(vec![
        instance.new_str("time"),
        instance.new_str("time_ns"),
        instance.new_str("monotonic"),
        instance.new_str("monotonic_ns"),
        instance.new_str("perf_counter"),
        instance.new_str("perf_counter_ns"),
        instance.new_str("sleep"),
    ]);
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
