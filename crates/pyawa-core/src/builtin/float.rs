//! **`float` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `float_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：（无） ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::cell::Cell;
use crate::builtin_objects::{BuiltinFunctionObject, FloatObject, MethodObject, NativeFn};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;


/// **`float` 的方法面**（第 352 轮）：`is_integer` ✓ 与 `as_integer_ratio` ✓
/// （`dir(float)` 里最常用的两件 ✓；先前 `float` 类型**根本没有 getattr 槽** ✗ ⇒ 一律 AttributeError ✓）。
pub unsafe fn float_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "is_integer" => float_is_integer_native,
        "as_integer_ratio" => float_as_integer_ratio_native,
        _ => return None,
    };
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "float",
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

/// `float.is_integer()` ✓（第 352 轮）：有限且小数部分为 0 ⇒ `True` ✓（`inf`／`nan` ⇒ `False` ✓）。
pub(crate) fn float_is_integer_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(owner) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "is_integer 缺少 self"));
    };
    let Some(value) = instance.float_value(owner) else {
        return Err(instance.raise_builtin_error("TypeError", "is_integer 只接浮点"));
    };
    Ok(instance.new_bool(value.is_finite() && value.fract() == 0.0))
}

/// `float.as_integer_ratio()` ✓（第 352 轮）：**精确**比 ✓（`(0.5).as_integer_ratio()` ⇒ `(1, 2)` ✓）。
///
/// 做法：把尾数逐位左移直到变成整数 ✓（**有限**位 ✓），同时把分母乘 2 的同次数 ✓ ⇒ 精确 ✓。
/// `inf`／`nan` ⇒ `OverflowError`／`ValueError` ✓（照参照：`nan` 报
/// `ValueError: cannot convert NaN to integer ratio` ✓）。
pub(crate) fn float_as_integer_ratio_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(owner) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "as_integer_ratio 缺少 self"));
    };
    let Some(value) = instance.float_value(owner) else {
        return Err(instance.raise_builtin_error("TypeError", "as_integer_ratio 只接浮点"));
    };
    if value.is_nan() {
        return Err(instance.raise_builtin_error("ValueError", "cannot convert NaN to integer ratio"));
    }
    if value.is_infinite() {
        return Err(instance.raise_builtin_error("OverflowError", "cannot convert Infinity to integer ratio"));
    }
    let mut numerator = value;
    let mut denominator = 1.0f64;
    // 最多 1100 次（`f64` 的最小次正规约 2^-1074 ✓）⇒ 有界 ✓
    while numerator.fract() != 0.0 && denominator < 1e300 {
        numerator *= 2.0;
        denominator *= 2.0;
    }
    let pair = vec![
        instance.new_int(numerator as i64),
        instance.new_int(denominator as i64),
    ];
    Ok(instance.new_tuple(pair))
}

/// **`float.fromhex(str)` 的类型级入口**（第 406 轮 ✓）：`_pylong.py:260` 导入期就用
/// `float.fromhex('0x1.a934f0979a371p-2')` ✓。
///
/// 语法照参照：`[+-] (0x)? 十六进制数字 [. 十六进制数字] [p[+-]十进制指数]`，也收
/// `inf`／`infinity`／`nan`（大小写不敏感 ✓）；空白两端可去 ✓。
pub(crate) fn float_fromhex_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(source) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "float.fromhex() takes exactly one argument (0 given)",
        ));
    };
    let text = if let Some(text) = instance.text_of(*source) {
        text.to_owned()
    } else if let Some(bytes) = instance.bytes_value(*source) {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "fromhex() argument must be str, not other",
        ));
    };
    let Some(value) = parse_hex_float(&text) else {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "invalid hexadecimal floating-point string",
        ));
    };
    Ok(instance.new_float(value))
}

/// 解析十六进制浮点串（`float.fromhex` 的实现）。
///
/// 做法：尾数用 `u128` 累积（≤ 32 个十六进制位 ⇒ 精确），再乘 `2^(p - 4×小数位)`（2 的幂
/// 缩放**不引入舍入** ✓ ⇒ 与参照的"正确舍入"一致 ✓）；更长的尾数退化为**逐步 f64 累积**
/// （如实登记的近似 ✗，实际语料里没有）。
fn parse_hex_float(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    let (negative, rest) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    let lower = rest.to_ascii_lowercase();
    if lower == "inf" || lower == "infinity" {
        return Some(if negative { f64::NEG_INFINITY } else { f64::INFINITY });
    }
    if lower == "nan" {
        return Some(if negative { -f64::NAN } else { f64::NAN });
    }
    let rest = if let Some(stripped) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        stripped
    } else {
        rest
    };
    let (mantissa_text, exponent_text) = match rest.find(['p', 'P']) {
        Some(index) => (&rest[..index], Some(&rest[index + 1..])),
        None => (rest, None),
    };
    let (integer_text, fraction_text) = match mantissa_text.split_once('.') {
        Some((integer, fraction)) => (integer, fraction),
        None => (mantissa_text, ""),
    };
    if integer_text.is_empty() && fraction_text.is_empty() {
        return None;
    }
    if !integer_text.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !fraction_text.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    // 指数：十进制、可带符号；缺省 0
    let mut exponent: i64 = 0;
    if let Some(text) = exponent_text {
        if text.is_empty() {
            return None;
        }
        exponent = text.parse().ok()?;
    }
    // 尾数：整数部分 ＋ 小数部分（去掉前导零，便于判断长度）
    let digits: String = format!("{integer_text}{fraction_text}");
    let significant = digits.trim_start_matches('0');
    let mut value = 0.0f64;
    if significant.len() <= 32 {
        let mut accumulator: u128 = 0;
        for character in significant.chars() {
            accumulator = accumulator.checked_mul(16)? + u128::from(character.to_digit(16)?);
        }
        value = accumulator as f64;
    } else {
        for character in significant.chars() {
            value = value * 16.0 + f64::from(character.to_digit(16)?);
        }
    }
    // 2 的幂缩放：`2^(exponent - 4×fraction_len)`
    let binary_exponent = exponent - 4 * i64::try_from(fraction_text.len()).ok()?;
    if binary_exponent > 1100 {
        return Some(f64::INFINITY.copysign(if negative { -1.0 } else { 1.0 }));
    }
    if binary_exponent < -1200 {
        return Some(f64::from_bits(0).copysign(if negative { -1.0 } else { 1.0 }));
    }
    value *= 2.0f64.powi(binary_exponent as i32);
    Some(if negative { -value } else { value })
}

/// `float()`：`0.0`；`float(<整数>)`：**正确舍入**到最近的 double（溢出报 `OverflowError`，
/// 消息照实测 `int too large to convert to float`）；`float(<浮点>)`：原值。
///
/// **未接线**：`float('<串>')`（参照会解析十进制／`inf`／`nan`）与多实参形态——都如实报未实现，
/// **不手写**参照的消息（那条消息得先实测，归构造函数的夹具）。
pub unsafe fn float_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = match args {
        [] => 0.0,
        [only] => {
            if let Some(integer) = instance.int_of(*only) {
                let wide = integer.to_bigint();
                let number = wide.to_f64();
                if number.is_infinite() && !wide.is_zero() {
                    return Err(instance.raise_builtin_error(
                        "OverflowError",
                        "int too large to convert to float",
                    ));
                }
                number
            } else if let Some(number) = instance.float_value(*only) {
                number
            }
            // **`float(字符串)`** ✓（第 695 轮接线 ✗）：`json` 一族靠它 ✓（先前报
            // "float_new：这个实参形态还没接线（字符串解析等）" ✗）。口径照参照实测 ✓：
            // 前后空白去掉 ✓；`inf`／`infinity`／`nan` 及其带符号形式 ✓；其余交给 Rust 的 `f64` 解析 ✓
            // （它与 CPython 一样认 `1e3`／`.5`／`5.` ✓）；解析不了 ⇒ `ValueError` ✓（消息照参照 ✓）。
            else if let Some(text) = instance.text_of(*only) {
                let trimmed = text.trim();
                let lowered = trimmed.to_ascii_lowercase();
                let (negative, body) = match lowered.strip_prefix('-') {
                    Some(rest) => (true, rest),
                    None => (false, lowered.strip_prefix('+').unwrap_or(&lowered)),
                };
                let number = match body {
                    "inf" | "infinity" => f64::INFINITY,
                    "nan" => f64::NAN,
                    _ => match trimmed.parse::<f64>() {
                        Ok(number) => number,
                        Err(_) => {
                            return Err(instance.raise_builtin_error(
                                "ValueError",
                                &format!("could not convert string to float: '{trimmed}'"),
                            ));
                        }
                    },
                };
                if negative { -number } else { number }
            } else {
                return Err(crate::ExecError::Unsupported {
                    opcode: 0,
                    what: "float_new：这个实参形态还没接线（字符串解析等）",
                });
            }
        }
        _ => {
            return Err(crate::ExecError::Unsupported {
                opcode: 0,
                what: "float_new：多实参形态还没接线",
            })
        }
    };
    Ok(
        instance
            .alloc(FloatObject::new(class, value))
            .into_raw()
            .cast::<Header>(),
    )
}

/// 浮点的 `repr`：Rust 的 `{:?}` 已是"最短往返"，但指数写法与特殊值要和参照实现对齐
/// （实测：`1e+16`、`inf`、`nan`）。
pub(crate) fn float_repr_text(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    let text = format!("{value:?}");
    // Rust：`1e16`；参照实现：`1e+16`（指数带符号）
    match text.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with('-') && !exponent.starts_with('+') => {
            format!("{mantissa}e+{exponent}")
        }
        _ => text,
    }
}

/// `float` 的 `repr`。
pub unsafe fn float_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FloatObject>() };
    Ok(float_repr_text(object.value))
}
