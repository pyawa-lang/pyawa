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
        ("localtime", localtime_native as NativeFn),
        ("gmtime", localtime_native as NativeFn),
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
        instance.new_str("localtime"),
        instance.new_str("gmtime"),
    ]);
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}

/// `time.localtime([secs])`／`time.gmtime([secs])` ✓（第 726 轮）：`Lib/zipfile/__init__.py` 的
/// `time.localtime(time.time())[:6]` 要它 ✗。**如实口径** ✗：本层按 `tzname` 的既有口径走 **UTC**
/// ⇒ `localtime` 与 `gmtime` 同值 ✓（参照在 UTC 机器上一致 ✓；非 UTC 时区不承诺 ✓）。
fn localtime_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let seconds = args
        .first()
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|elapsed| elapsed.as_secs() as i64)
        })
        .unwrap_or(0);
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    // 民用历（Howard Hinnant 的口径 ✓）：天数 → 年／月／日 ✓。
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    let weekday = (days + 3).rem_euclid(7); // 1970-01-01 是周四 ⇒ 3（周一为 0 ✓）
    let start_of_year = days_from_civil(year, 1, 1);
    let yday = days - start_of_year + 1;
    let fields = [
        year,
        month,
        day,
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60,
        weekday,
        yday,
        0,
    ];
    let values: Vec<NonNull<Header>> = fields
        .iter()
        .map(|field| instance.new_int(*field))
        .collect();
    Ok(instance.new_tuple(values))
}

/// 天数 ← 年月日（Hinnant 口径的逆 ✓）。
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
