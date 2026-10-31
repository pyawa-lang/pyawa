//! `time` 模块（`SPEC-c-modules.md`；`clock` 域的第一个消费者 ✓）。
//!
//! **本段落地**（第 317 轮）：`time()`／`time_ns()`／`monotonic()`／`monotonic_ns()` ——
//! 全部**经 `clock` 域**取（`CX-4`：本 crate 不碰平台 ✓）。
//!
//! **round 16 补上日历／格式化面**：`localtime`（走 `clock` 域**表尾追加的** `localtime` 槽 ⇒
//! **真实时区**；槽缺失时退回纯 UTC 旧实现）／`gmtime`（纯 UTC）／`strftime`（provider 的 libc
//! `strftime`，C locale）／`asctime`／`ctime`（纯 Rust 的 C `asctime` 口径）／`mktime`（provider
//! 的 libc `mktime`）；`timezone`／`altzone`／`daylight`／`tzname` 在能问到 provider 时**据实**
//! 填（`tm_gmtoff`／`tm_zone`）。
//!
//! **如实登记的未接面** ✗（`CM-6`：未提供 ⇒ `ImportError`／未实现 ⇒ `NotImplementedError` ✓）：
//! `perf_counter*`／`process_time*`／`thread_time*`（要另外的钟 ✓）、
//! `struct_time` 类型（返回值仍是 9 元组，`.tm_year` 等属性不可用 ✗）、`strptime`（要解析器）、
//! `tzset`（没有时区库；`timezone` 一族只能取**当前**分区）、`daylight` 恒 0／`altzone == timezone`
//! （无 DST 第二偏移通道）。
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

/// `time.sleep(seconds)`：经 `clock.sleep_ns` 分片可取消睡眠（`T-CP-10`，`CP-27`／`CP-28`）。
///
/// - 参数／错误形状照参照实测（`time.sleep() takes exactly one argument (N given)`、
///   `'str' object cannot be interpreted as an integer or float`、`sleep length must be non-negative`、
///   `Invalid value NaN (not a number)`、`OverflowError: timestamp out of range for platform time_t`）；
/// - **中断**：`pa_interrupt` ⇒ `cancellable_sleep_ns` 返回"已取消" ⇒ 这里转
///   `ExecError::Interrupted` ⇒ `pa_exec_*` 返回 `PA_ERR_INTERRUPT`（既有中断口径；参照是
///   Python 层 `KeyboardInterrupt`，此为如实差异）；
/// - `clock` 域/槽位未提供 ⇒ `NotImplementedError`（`CP-5` 的三态，与其它 `time` 函数同一口径）。
fn sleep_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "time.sleep() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("time.sleep() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let value = args[0];
    let seconds = if let Some(integer) = instance.int_value(value) {
        integer as f64
    } else if let Some(number) = instance.float_value(value) {
        number
    } else {
        let name = instance.type_name(instance.type_of(value));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{name}' object cannot be interpreted as an integer or float"),
        ));
    };
    if seconds.is_nan() {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "Invalid value NaN (not a number)",
        ));
    }
    if seconds < 0.0 {
        return Err(instance.raise_builtin_error("ValueError", "sleep length must be non-negative"));
    }
    if !seconds.is_finite() || seconds * 1e9 >= i64::MAX as f64 {
        return Err(instance.raise_builtin_error(
            "OverflowError",
            "timestamp out of range for platform time_t",
        ));
    }
    // 向上取整：不许比请求更早返回（与 `select` 的 timeout 换算同一口径）。
    let total_ns = (seconds * 1e9).ceil() as i64;
    match instance.cancellable_sleep_ns(total_ns) {
        // `CP-28`：已取消 ⇒ 转成既有中断口径（`PA_ERR_INTERRUPT`）；其后的脚本指令一条都不跑。
        Ok(false) => Err(ExecError::Interrupted),
        Ok(true) => Ok(instance.new_none()),
        // 三态错误（未注册／槽位未实现／机器错误）沿用 `clock_value` 的既有映射。
        // 三态错误（未注册／槽位未实现／机器错误）沿用 `clock_value` 的既有映射（一定返回 `Err`）。
        Err(error) => clock_value(instance, Err(error), "sleep").map(|_| instance.new_none()),
    }
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
        ("gmtime", gmtime_native as NativeFn),
        // **round 16**：日历／格式化面（`clock` 域表尾三槽的消费者）
        ("strftime", strftime_native as NativeFn),
        ("asctime", asctime_native as NativeFn),
        ("ctime", ctime_native as NativeFn),
        ("mktime", mktime_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    // **`struct_time` 类型**（`P3-42`）：`localtime`／`gmtime` 的返回值就是它 ✓。
    if let Some(class) = build_struct_time(instance) {
        let class_value = class.cast::<Header>();
        instance.dict_set(namespace, "struct_time", class_value);
    }
    instance.dict_set(namespace, "_STRUCT_TM_ITEMS", instance.new_int(11));
    // `timezone`／`altzone`／`daylight`／`tzname`：参照里是模块级变量 ✓ —— **能问到 provider 就据实填**
    // （`clock.localtime(0)` 的 `tm_gmtoff`／`tm_zone`）；问不到（旧宿主、或 conformance harness
    // 在 `install` 之后才注册 clock）就保留原来的 UTC 常量并登记偏差 ✓。
    // **如实偏差** ✗：`daylight` 恒 0、`altzone == timezone`（本层没有"夏令时第二偏移"的通道；
    // 无 DST 的时区——如本机 CST——与参照逐值一致 ✓）。
    let (timezone, zone_name) = match instance.clock_localtime(0) {
        Ok(tm) => {
            let zone_length = (tm.tm_zone_len as usize).min(tm.tm_zone.len());
            let zone = if zone_length == 0 {
                "UTC".to_owned()
            } else {
                String::from_utf8_lossy(&tm.tm_zone[..zone_length]).into_owned()
            };
            (-tm.tm_gmtoff, zone)
        }
        Err(_) => (0, "UTC".to_owned()),
    };
    instance.dict_set(namespace, "timezone", instance.new_int(timezone));
    instance.dict_set(namespace, "altzone", instance.new_int(timezone));
    instance.dict_set(namespace, "daylight", instance.new_int(0));
    let tzname = instance.new_tuple(vec![
        instance.new_str(&zone_name),
        instance.new_str(&zone_name),
    ]);
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
        instance.new_str("strftime"),
        instance.new_str("asctime"),
        instance.new_str("ctime"),
        instance.new_str("mktime"),
        instance.new_str("struct_time"),
    ]);
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}

// --------------------------------------------------------------------------- #
// round 16：`localtime`／`gmtime`／`strftime`／`asctime`／`ctime`／`mktime`
//
// 口径（全部本机 `python3` 3.14.4 逐条实测，见 `tests/time_shapes.rs` 的期望值）：
// - `localtime` 走 `clock` 域的**表尾追加槽**（真实时区）；槽缺失（旧宿主／conformance harness）
//   ⇒ 退回**纯 UTC** 的旧实现（本层原来就是 UTC ⇒ 旧宿主行为不回归）；
// - `gmtime` 保持**纯 UTC**（不依赖 provider）；
// - `strftime` 走 provider 的 libc `strftime`（C locale，与参照启动态一致）；
// - `asctime`／`ctime` 用**纯 Rust** 按 C 的 `asctime` 口径格式化（24 字符、无换行）；
// - `mktime` 走 provider 的 libc `mktime`（越界字段由 C 归一化），返回 **float**（参照同）。
//
// **如实登记的偏差** ✗：
// - 没有 `struct_time` 类型 ⇒ `localtime`／`gmtime` 仍返回 **9 元组**（`.tm_year` 等属性不可用；
//   参照的 `time.struct_time(...)` repr 也没有）；
// - 9 元组里的 `tm_wday`／`tm_yday` 是**输入值**（C 口径），不是按年月日重算（参照同）；
// - 没有 `strptime`／`tzset`／`process_time*`／`thread_time*`（未接线，见模块头）；
// - `daylight` 恒 0、`altzone == timezone`（无 DST 第二偏移通道；无 DST 时区与参照一致）。
// --------------------------------------------------------------------------- #

/// 取**当前 Unix 秒**（`clock.now_ns` 折秒；`clock` 域未注册时退回宿主的 `SystemTime`）。
fn now_seconds(instance: &Instance) -> Result<i64, ExecError> {
    match instance.clock_now_ns() {
        Ok(nanoseconds) => Ok(nanoseconds.div_euclid(1_000_000_000)),
        Err(_) => Ok(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(0)),
    }
}

/// 机器错误 ⇒ `OSError`（带 `[Errno N] <strerror>` 文案；`strerror` 取宿主 std 的同一张表）。
///
/// 参照实测：`time.localtime(2**62)` ⇒ `OSError: [Errno 75] Value too large for defined data type`。
fn machine_os_error(instance: &Instance, errno: i32) -> ExecError {
    let text = std::io::Error::from_raw_os_error(errno).to_string();
    let text = text.split(" (os error").next().unwrap_or(&text).to_owned();
    instance.raise_builtin_error("OSError", &format!("[Errno {errno}] {text}"))
}

/// `clock` 槽的三态错误 ⇒ 脚本异常（照 [`clock_value`] 的既有映射，机器错误带 errno 文案）。
fn clock_slot_error(
    instance: &Instance,
    error: pyawa_core::CapabilityCallError,
    what: &str,
) -> ExecError {
    match error {
        pyawa_core::CapabilityCallError::NotImplemented => instance.raise_builtin_error(
            "NotImplementedError",
            &format!("`time.{what}`：`clock` 域的该槽位未提供"),
        ),
        pyawa_core::CapabilityCallError::NotRegistered => instance.raise_builtin_error(
            "NotImplementedError",
            &format!("`time.{what}`：`clock` 域未注册"),
        ),
        pyawa_core::CapabilityCallError::Machine(errno) => machine_os_error(instance, errno),
    }
}

/// 一个 `time_t` 实参（`localtime`／`gmtime`／`ctime` 用）：整数或浮点 ⇒ **向下取整**的秒。
///
/// 参照实测：`localtime(1.9)` ⇒ 1 秒、`ctime(-0.9)` ⇒ 1969-12-31 23:59:59（**floor**）；
/// `localtime("x")` ⇒ `TypeError: 'str' object cannot be interpreted as an integer`；
/// `localtime(float('nan'))` ⇒ `ValueError: Invalid value NaN (not a number)`；
/// 超大（`2**63`／`inf`）⇒ `OverflowError: timestamp out of range for platform time_t`。
fn seconds_argument(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<i64, ExecError> {
    if let Some(number) = instance.int_of(value) {
        return number.to_i64().ok_or_else(|| {
            instance.raise_builtin_error("OverflowError", "timestamp out of range for platform time_t")
        });
    }
    if let Some(number) = instance.float_value(value) {
        if number.is_nan() {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "Invalid value NaN (not a number)",
            ));
        }
        if !number.is_finite() || number < i64::MIN as f64 || number > i64::MAX as f64 {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "timestamp out of range for platform time_t",
            ));
        }
        return Ok(number.floor() as i64);
    }
    let name = instance.type_name(instance.type_of(value));
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("'{name}' object cannot be interpreted as an integer"),
    ))
}

/// 纯 UTC 的民用历换算（Hinnant 口径）：Unix 秒 ⇒ `(年, 月, 日, 时, 分, 秒, 周几, 年内第几天)`。
///
/// 这是 `gmtime` 与 `localtime` 的**回退路径**（`clock` 域没提供 `localtime` 槽时）；
/// 1970-01-01 是周四 ⇒ 周一为 0 的 `tm_wday` 是 3 ✓。
fn utc_fields(seconds: i64) -> [i64; 8] {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
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
    let weekday = (days + 3).rem_euclid(7);
    let yday = days - days_from_civil(year, 1, 1) + 1;
    [
        year,
        month,
        day,
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60,
        weekday,
        yday,
    ]
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

/// 纯 UTC 回退：Unix 秒 ⇒ `CpTm` 形状（`tm_year` 自 1900、`tm_mon` 0-based）。
///
/// **`tm_wday` 用 C 的约定**（周日起 0）：CpTm 对齐 C 的 `struct tm`，所以 Python 的
/// "周一为 0"要转成 `(w + 1) % 7`（`utc_fields` 给的是 Python 约定）。
fn utc_tm(seconds: i64) -> pyawa_capabilities::clock::CpTm {
    let fields = utc_fields(seconds);
    pyawa_capabilities::clock::CpTm {
        tm_year: (fields[0] - 1900) as i32,
        tm_mon: (fields[1] - 1) as i32,
        tm_mday: fields[2] as i32,
        tm_hour: fields[3] as i32,
        tm_min: fields[4] as i32,
        tm_sec: fields[5] as i32,
        tm_wday: ((fields[6] + 1) % 7) as i32,
        tm_yday: (fields[7] - 1) as i32,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: [0u8; 16],
        tm_zone_len: 0,
    }
}

/// Unix 秒 ⇒ 本地时间（provider `localtime` 槽；槽缺失 ⇒ 纯 UTC 回退，见段首注释）。
fn local_tm(instance: &Instance, seconds: i64) -> Result<pyawa_capabilities::clock::CpTm, ExecError> {
    match instance.clock_localtime(seconds) {
        Ok(tm) => Ok(tm),
        Err(pyawa_core::CapabilityCallError::NotRegistered)
        | Err(pyawa_core::CapabilityCallError::NotImplemented) => Ok(utc_tm(seconds)),
        Err(error) => Err(clock_slot_error(instance, error, "localtime")),
    }
}

/// 9 元组 → [`pyawa_capabilities::clock::CpTm`]（`strftime`／`asctime`／`mktime` 的共用转换）。
///
/// `raw == true` 是 `mktime` 的**C 语义**（不校验、月份无条件减 1；越界由 libc 归一化）；
/// `raw == false` 是 `strftime`／`asctime` 的口径——**逐条照参照实测**：
/// - 非元组（含 `list`／`None`）⇒ `TypeError: Tuple or struct_time argument required`；
/// - 长度不是 9 ⇒ `TypeError: <func>(): illegal time tuple argument`；
/// - 元素不是整数 ⇒ `TypeError: '<类型>' object cannot be interpreted as an integer`；
///   元素超 `i32`／`i64` ⇒ `OverflowError: Python int too large to convert to C long`；
/// - `tm_mon`：输入 `>0` 才减 1（**0 视作一月**，照参照）；范围 0–11；
/// - `tm_mday`：0–31，且 **0 归一到 1**（照参照实测 `strftime("%d", …, 0)` ⇒ `01`）；
/// - 时 0–23／分 0–59／秒 0–61（闰秒口径）／一年第几天 0–366（输入 `>0` 才减 1 转 C 的
///   0-based）；`tm_wday` 不校验，按 `(w+1) % 7` 从 Python 口径（周一 0）转 C 口径（周日 0）。
fn tuple_to_tm(
    instance: &Instance,
    object: NonNull<Header>,
    func: &str,
    raw: bool,
) -> Result<pyawa_capabilities::clock::CpTm, ExecError> {
    // `P3-42`：`struct_time` 不是 tuple 子类 ⇒ 先认它的 `_items` 序列（`mktime(loctime_result)` ✓）。
    let Some(items) = instance.tuple_items(object).or_else(|| struct_time_items(instance, object)) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "Tuple or struct_time argument required",
        ));
    };
    if items.len() != 9 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{func}(): illegal time tuple argument"),
        ));
    }
    let mut fields = [0i64; 9];
    for (slot, item) in fields.iter_mut().zip(items.iter()) {
        let Some(value) = instance.int_of(*item) else {
            let name = instance.type_name(instance.type_of(*item));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("'{name}' object cannot be interpreted as an integer"),
            ));
        };
        let Some(value) = value.to_i64().and_then(|number| i32::try_from(number).ok()) else {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "Python int too large to convert to C long",
            ));
        };
        *slot = i64::from(value);
    }
    let mut tm = pyawa_capabilities::clock::CpTm {
        tm_year: i32::try_from(fields[0] - 1900).map_err(|_| {
            instance.raise_builtin_error("OverflowError", "Python int too large to convert to C long")
        })?,
        tm_mon: 0,
        tm_mday: fields[2] as i32,
        tm_hour: fields[3] as i32,
        tm_min: fields[4] as i32,
        tm_sec: fields[5] as i32,
        // C 的 `tm_wday` 以**周日**为 0；Python 的 9 元组以**周一**为 0 ⇒ 转回 C 口径。
        tm_wday: ((fields[6] + 1).rem_euclid(7)) as i32,
        tm_yday: fields[7] as i32,
        tm_isdst: fields[8] as i32,
        tm_gmtoff: 0,
        tm_zone: [0u8; 16],
        tm_zone_len: 0,
    };
    if raw {
        // `mktime`：月份**无条件**减 1（输入 0 ⇒ C 的 -1 ⇒ libc 归一化成上一年 12 月）；不校验。
        tm.tm_mon = i32::try_from(fields[1] - 1).map_err(|_| {
            instance.raise_builtin_error("OverflowError", "Python int too large to convert to C long")
        })?;
        return Ok(tm);
    }
    // `strftime`／`asctime`：月份 0 视作一月；其余逐项校验（文案照参照实测）。
    let mon = if fields[1] > 0 { fields[1] - 1 } else { fields[1] };
    if !(0..=11).contains(&mon) {
        return Err(instance.raise_builtin_error("ValueError", "month out of range"));
    }
    tm.tm_mon = mon as i32;
    if !(0..=31).contains(&fields[2]) {
        return Err(instance.raise_builtin_error("ValueError", "day of month out of range"));
    }
    // 0 归一到 1（参照实测：`asctime((2020,1,0,…))` 打印 `Jan  1`）。
    tm.tm_mday = if fields[2] == 0 { 1 } else { fields[2] as i32 };
    if !(0..=23).contains(&fields[3]) {
        return Err(instance.raise_builtin_error("ValueError", "hour out of range"));
    }
    if !(0..=59).contains(&fields[4]) {
        return Err(instance.raise_builtin_error("ValueError", "minute out of range"));
    }
    if !(0..=61).contains(&fields[5]) {
        return Err(instance.raise_builtin_error("ValueError", "seconds out of range"));
    }
    if !(0..=366).contains(&fields[7]) {
        return Err(instance.raise_builtin_error("ValueError", "day of year out of range"));
    }
    // C 的 `tm_yday` 是 **0-based**；Python 的 9 元组是 1-based ⇒ 输入 `>0` 才减 1
    //（输入 0 照参照留在 0：`strftime("%j", …, 0)` ⇒ `001`；输入 7 ⇒ `007`）。
    tm.tm_yday = if fields[7] > 0 { (fields[7] - 1) as i32 } else { fields[7] as i32 };
    Ok(tm)
}

/// [`pyawa_capabilities::clock::CpTm`] ⇒ 9 元组（`localtime`／`gmtime` 的返回形状）。
///
/// **如实偏差** ✗：参照返回 `struct_time`（tuple 子类、带 `.tm_year` 等属性）；本层只有 9 元组
/// （任务钉住的口径，见段首注释）。
fn tm_to_tuple(instance: &Instance, tm: &pyawa_capabilities::clock::CpTm) -> NonNull<Header> {
    let fields = [
        i64::from(tm.tm_year) + 1900,
        i64::from(tm.tm_mon) + 1,
        i64::from(tm.tm_mday),
        i64::from(tm.tm_hour),
        i64::from(tm.tm_min),
        i64::from(tm.tm_sec),
        // C 的 `tm_wday` 以周日为 0、Python 的 9 元组以周一为 0 ⇒ `(c + 6) % 7`。
        (i64::from(tm.tm_wday) + 6).rem_euclid(7),
        // C 的 `tm_yday` 是 0-based、Python 的 9 元组是 1-based。
        i64::from(tm.tm_yday) + 1,
        i64::from(tm.tm_isdst),
    ];
    let values: Vec<NonNull<Header>> = fields
        .iter()
        .map(|field| instance.new_int(*field))
        .collect();
    instance.new_tuple(values)
}

/// C `asctime` 的口径：`Www Mmm dd hh:mm:ss yyyy`（24 字符、**无换行**；`dd` 是**空格填充**的两位）。
///
/// 逐条照参照实测：星期名按 C 的周日起 0 的 `tm_wday` 转回 Python 口径（-1 ⇒ Sun、8 ⇒ Tue）、
/// 月名按 0-based `tm_mon`、日 0 已在上游归一到 1、年份按**输入年**直接印
/// （10000 ⇒ `10000`、-1 ⇒ `-1`）。
fn asctime_text(tm: &pyawa_capabilities::clock::CpTm) -> String {
    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let weekday = WEEKDAYS[((i64::from(tm.tm_wday) + 6).rem_euclid(7)) as usize];
    let month_index = tm.tm_mon.rem_euclid(12) as usize;
    let month = MONTHS[month_index];
    format!(
        "{} {}{:3} {:02}:{:02}:{:02} {}",
        weekday,
        month,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        i64::from(tm.tm_year) + 1900
    )
}

/// `time.localtime([secs])`：**真实本地时间**（provider `localtime` 槽），返回 9 元组。
///
/// 参数校验照参照实测：关键字 ⇒ `localtime() takes no keyword arguments`；
/// 多于 1 个实参 ⇒ `localtime() takes at most 1 argument (N given)`；`None` 与缺省 ⇒ 当前时刻。
fn localtime_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "localtime() takes no keyword arguments",
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("localtime() takes at most 1 argument ({} given)", args.len()),
        ));
    }
    let seconds = match args.first() {
        None => now_seconds(instance)?,
        Some(value) if *value == instance.singletons().none() => now_seconds(instance)?,
        Some(value) => seconds_argument(instance, *value)?,
    };
    let tm = local_tm(instance, seconds)?;
    Ok(tm_to_struct_time(instance, &tm).unwrap_or_else(|| tm_to_tuple(instance, &tm)))
}

/// `time.gmtime([secs])`：**纯 UTC**（不依赖 provider），返回 9 元组（参数校验同 `localtime`）。
fn gmtime_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "gmtime() takes no keyword arguments",
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("gmtime() takes at most 1 argument ({} given)", args.len()),
        ));
    }
    let seconds = match args.first() {
        None => now_seconds(instance)?,
        Some(value) if *value == instance.singletons().none() => now_seconds(instance)?,
        Some(value) => seconds_argument(instance, *value)?,
    };
    let tm = utc_tm(seconds);
    Ok(tm_to_struct_time(instance, &tm).unwrap_or_else(|| tm_to_tuple(instance, &tm)))
}

/// `time.strftime(format[, t])`：`clock.strftime` 槽（C locale）的消费者。
///
/// 参数校验照参照实测：关键字 ⇒ `strftime() takes no keyword arguments`；0 实参 ⇒
/// `strftime() takes at least 1 argument (0 given)`；3 实参 ⇒ `takes at most 2 arguments (3 given)`；
/// `format` 不是 `str` ⇒ `strftime() argument 1 must be str, not <类型>`；`t` 缺省 ⇒ `localtime(now)`，
/// 给了就**必须是 9 元组**（`None` 也照 `Tuple or struct_time argument required` 报）。
fn strftime_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "strftime() takes no keyword arguments",
        ));
    }
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "strftime() takes at least 1 argument (0 given)",
        ));
    }
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("strftime() takes at most 2 arguments ({} given)", args.len()),
        ));
    }
    let Some(format) = instance.text_of(args[0]) else {
        let name = instance.type_name(instance.type_of(args[0]));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("strftime() argument 1 must be str, not {name}"),
        ));
    };
    let format = format.to_owned();
    let tm = match args.get(1) {
        Some(value) => tuple_to_tm(instance, *value, "strftime", false)?,
        None => local_tm(instance, now_seconds(instance)?)?,
    };
    match instance.clock_strftime(format.as_bytes(), &tm) {
        Ok(text) => Ok(instance.new_str(&text)),
        Err(error) => Err(clock_slot_error(instance, error, "strftime")),
    }
}

/// `time.asctime([t])`：纯 Rust 的 C `asctime` 口径（见 [`asctime_text`]）。
fn asctime_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "asctime() takes no keyword arguments",
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("asctime expected at most 1 argument, got {}", args.len()),
        ));
    }
    let tm = match args.first() {
        Some(value) => tuple_to_tm(instance, *value, "asctime", false)?,
        None => local_tm(instance, now_seconds(instance)?)?,
    };
    let text = asctime_text(&tm);
    Ok(instance.new_str(&text))
}

/// `time.ctime([secs])`：本地时间（provider）→ `asctime` 口径字符串；`None`／缺省 ⇒ 当前时刻。
fn ctime_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "ctime() takes no keyword arguments",
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("ctime() takes at most 1 argument ({} given)", args.len()),
        ));
    }
    let seconds = match args.first() {
        None => now_seconds(instance)?,
        Some(value) if *value == instance.singletons().none() => now_seconds(instance)?,
        Some(value) => seconds_argument(instance, *value)?,
    };
    let tm = local_tm(instance, seconds)?;
    let text = asctime_text(&tm);
    Ok(instance.new_str(&text))
}

/// `time.mktime(t)`：本地 9 元组 ⇒ provider 的 libc `mktime`（C 语义：越界字段归一化）⇒ **float**。
fn mktime_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "time.mktime() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("time.mktime() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let tm = tuple_to_tm(instance, args[0], "mktime", true)?;
    match instance.clock_mktime(&tm) {
        Ok(seconds) => Ok(instance.new_float(seconds as f64)),
        Err(error) => Err(clock_slot_error(instance, error, "mktime")),
    }
}

// ==== `time.struct_time`（`P3-42` 第 4 轮）=================================================
//
// **动因** ✓：`'module' object has no attribute 'struct_time'` 是官方分诊里的两条签名
//（`test_strftime`／`test_structseq`）；且 `localtime`／`gmtime` 的返回值在参照里**就是**
// struct_time（`t.tm_year` 一类属性可用 ✓），本层先前只给 9 元组 ✗。
//
// **口径（如实 ✗）**：本层的 `struct_time` 是**可序列类型**（`__getitem__`／`__len__`／
// `__iter__`／`__eq__`／`__hash__`／`__add__`／`__mul__`／`__contains__` ＋ `tm_*` 属性），
// 但**不是 `tuple` 的子类** ✗（`isinstance(t, tuple)` 为假 ⇒ `test_structseq.test_tuple` 仍 FAIL，
// 如实记）。构造语义照本机 3.14.4 实测：`t(seq)` 要 9 项以上、只取前 9 项进序列；额外的第 10 项
// 当 `tm_zone`（字符串）；`dict=` 只认 `tm_zone`／`tm_gmtoff`，与九个字段名冲突或未知键 ⇒
// `TypeError: got duplicate or unexpected field name(s)` ✓。

/// `struct_time` 的九个字段名（顺序即序列下标 ✓）。
const STRUCT_TIME_FIELDS: [&str; 9] = [
    "tm_year", "tm_mon", "tm_mday", "tm_hour", "tm_min", "tm_sec", "tm_wday", "tm_yday",
    "tm_isdst",
];

/// 取 `struct_time` 的序列（`_items` 元组；借用视图）。
fn struct_time_items(instance: &Instance, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
    let stored = instance.attribute_optional_of(object, "_items").ok().flatten()?;
    instance.tuple_items(stored)
}

fn struct_time_this(
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
) -> Option<(NonNull<Header>, &[NonNull<Header>])> {
    match bound {
        Some(this) => Some((this, args)),
        None => args.split_first().map(|(first, rest)| (*first, rest)),
    }
}

/// 把任意序列收成元素表（`str` 按码点逐个造单字符；`bytes` 同理；tuple／list 借用后再 retain ✓）。
fn struct_time_sequence(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<Vec<NonNull<Header>>, ExecError> {
    if let Some(items) = instance.tuple_items(value) {
        return Ok(items.iter().map(|item| instance.retain(*item)).collect());
    }
    if let Some(items) = instance.list_items(value) {
        return Ok(items.iter().map(|item| instance.retain(*item)).collect());
    }
    if let Some(text) = instance.str_wtf8_bytes(value) {
        let mut items = Vec::new();
        if let Ok(points) = pyawa_core::wtf8::decode(text) {
            for point in points {
                items.push(instance.new_str_code_point(point));
            }
        }
        return Ok(items);
    }
    if let Some(raw) = instance.bytes_value(value) {
        return Ok(raw.iter().map(|byte| instance.new_int(*byte as i64)).collect());
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        "struct_time() argument must be a sequence",
    ))
}

/// 键的文本（非文本键 ⇒ 报参照的 duplicate/unexpected 文案 ✓）。
fn struct_time_key(instance: &Instance, key: NonNull<Header>) -> Option<String> {
    instance.text_of(key).map(str::to_owned)
}

/// `struct_time(sequence, dict)`：构造并写状态。
fn struct_time_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "struct_time() takes at least 1 argument (0 given)",
        ));
    };
    let mut sequence_arg = rest.first().copied();
    let mut dict_arg = rest.get(1).copied();
    for (key, value) in kwargs {
        match instance.text_of(*key).as_deref() {
            Some("sequence") => sequence_arg = Some(*value),
            Some("dict") => dict_arg = Some(*value),
            Some(other) => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("struct_time() got an unexpected keyword argument '{other}'"),
                ))
            }
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "struct_time() keywords must be strings",
                ))
            }
        }
    }
    let Some(sequence_arg) = sequence_arg else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "struct_time() takes at least 1 argument (0 given)",
        ));
    };
    let items = struct_time_sequence(instance, sequence_arg)?;
    if items.len() < 9 {
        for item in &items {
            instance.release(*item);
        }
        return Err(instance.raise_builtin_error(
            "TypeError",
            "time.struct_time() takes a sequence of length 9",
        ));
    }
    // **字典**（可选）：只认 `tm_zone`／`tm_gmtoff`；与字段名重复／未知键 ⇒ 参照文案。
    let mut zone_override: Option<NonNull<Header>> = None;
    let mut gmtoff_override: Option<NonNull<Header>> = None;
    if let Some(dict_arg) = dict_arg.filter(|value| *value != instance.singletons().none()) {
        let Some(entries) = instance.dict_entries(dict_arg) else {
            for item in &items {
                instance.release(*item);
            }
            return Err(instance.raise_builtin_error(
                "TypeError",
                "struct_time() argument 2 must be a dict",
            ));
        };
        for (key, value) in entries {
            let name = struct_time_key(instance, key);
            let known = matches!(
                name.as_deref(),
                Some("tm_zone") | Some("tm_gmtoff")
            );
            if !known || STRUCT_TIME_FIELDS.contains(&name.as_deref().unwrap_or("")) {
                for item in &items {
                    instance.release(*item);
                }
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "got duplicate or unexpected field name(s)",
                ));
            }
            if name.as_deref() == Some("tm_zone") {
                zone_override = Some(instance.retain(value));
            } else {
                gmtoff_override = Some(instance.retain(value));
            }
        }
    }
    // 序列前 9 项 ⇒ 字段值；第 10 项 ⇒ 额外的 `tm_zone`（参照实测）。
    let extra_zone = items.get(9).map(|value| instance.retain(*value));
    let sequence: Vec<NonNull<Header>> = items[..9]
        .iter()
        .map(|item| instance.retain(*item))
        .collect();
    let stored = instance.new_tuple(sequence);
    for (index, name) in STRUCT_TIME_FIELDS.iter().enumerate() {
        instance.set_attribute_value(this, name, items[index])?;
    }
    let zone = zone_override.or(extra_zone);
    match zone {
        Some(zone) => {
            instance.set_attribute_value(this, "tm_zone", zone)?;
            instance.release(zone);
        }
        None => {
            let none = instance.new_none();
            instance.set_attribute_value(this, "tm_zone", none)?;
            instance.release(none);
        }
    }
    match gmtoff_override {
        Some(value) => {
            instance.set_attribute_value(this, "tm_gmtoff", value)?;
            instance.release(value);
        }
        None => {
            // **如实**：`tm_gmtoff` 未给 ⇒ 不挂属性（读取时按普通缺名处理；本层没有删除接口 ✗）。
        }
    }
    instance.set_attribute_value(this, "_items", stored)?;
    for item in &items {
        instance.release(*item);
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// `struct_time.__repr__`：`time.struct_time(tm_year=…, …, tm_isdst=…)` ✓（照参照逐字）。
fn struct_time_repr(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__repr__' needs an argument"));
    };
    let items = struct_time_items(instance, this).unwrap_or_default();
    let mut text = String::from("time.struct_time(");
    for (index, name) in STRUCT_TIME_FIELDS.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        let rendered = match items.get(index) {
            Some(item) => instance.object_repr(*item)?,
            None => "None".to_owned(),
        };
        text.push_str(name);
        text.push('=');
        text.push_str(&rendered);
    }
    text.push(')');
    Ok(instance.new_str(&text))
}

fn struct_time_len(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__len__' needs an argument"));
    };
    let length = struct_time_items(instance, this).map(|items| items.len()).unwrap_or(0);
    Ok(instance.new_int(length as i64))
}

/// `__getitem__`：整数（含负）⇒ 一项；切片 ⇒ **普通 `tuple`** ✓（参照口径）。
fn struct_time_getitem(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__getitem__' needs an argument"));
    };
    let Some(key) = rest.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "struct_time.__getitem__() takes exactly one argument",
        ));
    };
    let items = struct_time_items(instance, this).unwrap_or_default();
    if let Some(index) = instance.int_of(key).and_then(|value| value.to_i64()) {
        let length = items.len() as i64;
        let resolved = if index < 0 { index + length } else { index };
        if resolved < 0 || resolved >= length {
            return Err(instance.raise_builtin_error("IndexError", "tuple index out of range"));
        }
        return Ok(instance.retain(items[resolved as usize]));
    }
    // **切片** ⇒ 普通 `tuple` ✓（照参照）。边界规则走执行器的**同一处实现**（`slice_pick_public` ✓，
    // 一处真相；stdlib 禁 `unsafe` ⇒ 不直接读 `SliceObject` 载荷 ✗）。
    if Some(instance.type_of(key)) == instance.type_named("slice") {
        let picked: Vec<NonNull<Header>> =
            pyawa_core::executor::ctrls::slice_pick_public(instance, key, items.len())?
                .into_iter()
                .map(|index| instance.retain(items[index]))
                .collect();
        return Ok(instance.new_tuple(picked));
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        "struct_time indices must be integers or slices",
    ))
}

fn struct_time_iter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__iter__' needs an argument"));
    };
    let items = struct_time_items(instance, this).unwrap_or_default();
    let tuple = instance.new_tuple(items);
    instance.iter_object(tuple)
}

fn struct_time_eq(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__eq__' needs an argument"));
    };
    let Some(other) = rest.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "struct_time.__eq__() takes exactly one argument",
        ));
    };
    let left = struct_time_items(instance, this).unwrap_or_default();
    let right = match struct_time_items(instance, other) {
        Some(items) => items,
        None => match instance.tuple_items(other) {
            Some(items) => items,
            None => return Ok(instance.new_bool(false)),
        },
    };
    if left.len() != right.len() {
        return Ok(instance.new_bool(false));
    }
    for (a, b) in left.iter().zip(right.iter()) {
        if !pyawa_core::executor::values::values_equal_public(instance, *a, *b) {
            return Ok(instance.new_bool(false));
        }
    }
    Ok(instance.new_bool(true))
}

fn struct_time_hash(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__hash__' needs an argument"));
    };
    let items = struct_time_items(instance, this).unwrap_or_default();
    let tuple = instance.new_tuple(items);
    let outcome = crate::builtins_module::hash_object(instance, tuple)?;
    instance.release(tuple);
    Ok(instance.new_int(outcome))
}

fn struct_time_contains(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__contains__' needs an argument"));
    };
    let Some(wanted) = rest.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "struct_time.__contains__() takes exactly one argument",
        ));
    };
    let items = struct_time_items(instance, this).unwrap_or_default();
    for item in items {
        if pyawa_core::executor::values::values_equal_public(instance, item, wanted) {
            return Ok(instance.new_bool(true));
        }
    }
    Ok(instance.new_bool(false))
}

/// `t + other`：返回**普通 tuple**（`tuple(t) + other`，照参照 ✓）。
fn struct_time_add(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__add__' needs an argument"));
    };
    let Some(other) = rest.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "struct_time.__add__() takes exactly one argument",
        ));
    };
    let items = struct_time_items(instance, this).unwrap_or_default();
    let tuple = instance.new_tuple(items);
    let outcome = pyawa_core::executor::iter::concat_public(instance, tuple, other, 0);
    instance.release(tuple);
    outcome
}

/// `t * n`（以及 `n * t` 的 `__rmul__`）：返回**普通 tuple** ✓。
fn struct_time_repeat(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = struct_time_this(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__mul__' needs an argument"));
    };
    let Some(count) = rest.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "struct_time.__mul__() takes exactly one argument",
        ));
    };
    let count = instance
        .int_of(count)
        .and_then(|value| value.to_i64())
        .unwrap_or(0)
        .max(0) as usize;
    let items = struct_time_items(instance, this).unwrap_or_default();
    let mut repeated: Vec<NonNull<Header>> = Vec::with_capacity(items.len() * count);
    for _ in 0..count {
        for item in &items {
            repeated.push(instance.retain(*item));
        }
    }
    Ok(instance.new_tuple(repeated))
}

/// 建 `time.struct_time`（`build_class_from_parts`；`P3-42`）。
fn build_struct_time(instance: &Instance) -> Option<NonNull<Header>> {
    let base = instance
        .type_named("object")
        .expect("`object` 已登记");
    let namespace = instance.new_dict();
    let handlers: [(&str, NativeFn); 12] = [
        ("__init__", struct_time_init as NativeFn),
        ("__repr__", struct_time_repr as NativeFn),
        ("__len__", struct_time_len as NativeFn),
        ("__getitem__", struct_time_getitem as NativeFn),
        ("__iter__", struct_time_iter as NativeFn),
        ("__eq__", struct_time_eq as NativeFn),
        ("__hash__", struct_time_hash as NativeFn),
        ("__contains__", struct_time_contains as NativeFn),
        ("__add__", struct_time_add as NativeFn),
        ("__mul__", struct_time_repeat as NativeFn),
        ("__rmul__", struct_time_repeat as NativeFn),
        ("__ne__", struct_time_eq as NativeFn),
    ];
    for (name, handler) in handlers {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    // 字段数常量（`test_structseq.test_fields`）：9 个可见字段 ＋ `tm_zone` ＋ `tm_gmtoff` = 11。
    for (name, value) in [
        ("n_fields", 11i64),
        ("n_sequence_fields", 9),
        ("n_unnamed_fields", 0),
    ] {
        let number = instance.new_int(value);
        instance.dict_set(namespace, name, number);
    }
    let class = pyawa_core::build_class_from_parts(
        instance,
        "struct_time".to_owned(),
        vec![base.cast()],
        namespace,
        None,
    )
    .ok()?;
    Some(class)
}

/// `tm` ⇒ `struct_time` 实例（`localtime`／`gmtime` 的返回值 ✓）。
fn tm_to_struct_time(instance: &Instance, tm: &pyawa_capabilities::clock::CpTm) -> Option<NonNull<Header>> {
    let tuple = tm_to_tuple(instance, tm);
    let class = instance.type_named("struct_time")?;
    let outcome = pyawa_core::call_value(instance, class.cast::<Header>(), &[tuple], &[]);
    instance.release(tuple);
    outcome.ok()
}
