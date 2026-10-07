//! **`int` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `int_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`bound_int`、`bytes_getattr`、`digit_limit_message`、`parse_decimal` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::bigint::IntValue;
use crate::builtin_objects::{BuiltinFunctionObject, Decimal, IntObject, MethodObject, NativeFn};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::{bound_int, digit_limit_message, parse_decimal};


/// `int.bit_count()` ✓（第 352 轮）：**绝对值里 1 的个数** ✓（负数按绝对值 ✓ —— 照参照：
/// `(-1).bit_count()` ⇒ `1` ✓）。
pub(crate) fn int_bit_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(owner) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "bit_count 缺少 self"));
    };
    let Some(value) = instance.int_of(owner).and_then(|value| value.to_i64()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "bit_count 只接线了 i64 范围内的整数",
        ));
    };
    Ok(instance.new_int(value.unsigned_abs().count_ones() as i64))
}

/// **`str` 的方法面**（第 143 轮）：照 `bytes_getattr` 的同一套路 ✓（返回**绑定**的
/// `builtin_function_or_method` ✓，`self` 就是那个字符串 ✓）。
/// **`int` 的方法面** ✓（第 195 轮新建 ✓）：先接 `to_bytes` ✓ 与 `bit_length` ✓ ——
/// `Lib/importlib/_bootstrap_external.py` 一带要 `to_bytes` ✓。
pub unsafe fn int_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "to_bytes" => int_to_bytes_native,
        "bit_length" => int_bit_length_native,
        // **第 352 轮补**：`int.bit_count` ✓（`Lib/` 与测试里常用 ✓）。
        "bit_count" => int_bit_count_native,
        _ => return None,
    };
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "int",
        core::cell::Cell::new(handler),
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

/// `int.to_bytes(length, byteorder, *, signed=False)` ✓（第 195 轮：`signed=True` **如实报未接线** ✗）。
pub(crate) fn int_to_bytes_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bound_int(instance, bound)?;
    let Some(length_object) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "to_bytes() missing length"));
    };
    let Some(length) = instance.int_value(*length_object) else {
        return Err(instance.raise_builtin_error("TypeError", "length must be an int"));
    };
    let Some(order_object) = args.get(1) else {
        return Err(instance.raise_builtin_error("TypeError", "to_bytes() missing byteorder"));
    };
    let Some(order) = instance.text_of(*order_object) else {
        return Err(instance.raise_builtin_error("TypeError", "byteorder must be a str"));
    };
    // **`signed=` 如实报未接线** ✗（随后补 ✓）—— 不静默按无符号算 ✗。
    for (key, value_object) in kwargs {
        if instance.text_of(*key).as_deref() == Some("signed") {
            let truthy = !matches!(instance.int_value(*value_object), Some(0))
                && instance.type_of(*value_object) != instance.singletons().none_type();
            if truthy {
                return Err(crate::ExecError::Unsupported {
                    opcode: 0,
                    what: "int.to_bytes(signed=True)：二进制补码形态随后补",
                });
            }
        }
    }
    let big_endian = match order {
        "big" => true,
        "little" => false,
        _ => {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "byteorder must be either 'little' or 'big'",
            ))
        }
    };
    if length < 0 || value < 0 {
        return Err(instance.raise_builtin_error(
            "OverflowError",
            "can't convert negative int to unsigned",
        ));
    }
    let mut bytes = vec![0u8; length as usize];
    let mut remaining = value as u64;
    for index in 0..length as usize {
        let byte = (remaining & 0xFF) as u8;
        let position = if big_endian { length as usize - 1 - index } else { index };
        bytes[position] = byte;
        remaining >>= 8;
    }
    if remaining != 0 {
        return Err(instance.raise_builtin_error(
            "OverflowError",
            "int too big to convert",
        ));
    }
    Ok(instance.new_bytes(&bytes))
}

/// `int.bit_length()` ✓（顺手 ✓）。
pub(crate) fn int_bit_length_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bound_int(instance, bound)?;
    let bits = if value == 0 { 0 } else { 64 - value.unsigned_abs().leading_zeros() as i64 };
    Ok(instance.new_int(bits))
}


/// 造一个整数实例，**类型用调用方给的类** ✓（第 610 轮 ✓）：`class N(int)` 的 `N(3)` 必须造出
/// **`N` 的实例** ✓ —— 先前一律造 `int` ✗ ⇒ `type(N(3)) is N` 为假 ✓、属性写到"没有字典的 `int`"上 ✗
///（实测报 `'int' object has no attribute 'tag' and no __dict__ …` ✓，`re/_constants.py:70` 同型 ✓）。
fn make_int(
    class: NonNull<crate::TypeObject>,
    instance: &Instance,
    value: IntValue,
) -> NonNull<Header> {
    if class == instance.singletons().int_type() {
        // 常规路径**原样** ✓（不动既有行为 ✓）
        return instance.new_int_value(value);
    }
    // 子类：布局与 `int` 相同 ✓ ⇒ 直接按**子类**类型分配 ✓（`int_of` 已放宽到认子类 ✓）
    let object = instance.alloc(crate::builtin_objects::IntObject::new(class, value));
    object.into_raw().cast::<Header>()
}

/// `int()`：0（零参形态）、整数、十进制串、**浮点**（向零截断）。
pub unsafe fn int_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **实测口径**（`tools/gen_constructors_fixture.py` 的 12 条里那两条 `int`）：
    //   `int('a')` ⇒ `ValueError: invalid literal for int() with base 10: 'a'`
    //   `int([])`  ⇒ `TypeError: int() argument must be a string, a bytes-like object or a real number, not 'list'`
    // 另实测：`' 12 '`／`'+12'`／`'-12'`／`'1_2'` 都接受；`'0x10'`（base 10）与 `'12.5'` 报 `ValueError`。
    // **未接线**：`base` 参数形态、非 ASCII 数字（`int('１２')` 参照**接受** ⇒ 我们不假装报 `ValueError`
    // ✗，而是如实报未实现）。
    // 任意精度本身**已落地**（`P1-11` 第一刀之后：不再有"超出 i64"这一说）。
    match args {
        [] => Ok(make_int(_class, instance, IntValue::Small(0))),
        [only] => {
            if let Some(value) = instance.int_of(*only) {
                // `int(5)` ⇒ 5；`int(True)` ⇒ 1（`bool` 的载荷就是整数）；大整数原样再交回
                return Ok(make_int(_class, instance, value));
            }
            if let Some(number) = instance.float_value(*only) {
                // `int(浮点)`：**向零截断**；`inf`／`nan` 各按参照实测的消息报错
                if number.is_nan() {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "cannot convert float NaN to integer",
                    ));
                }
                if number.is_infinite() {
                    return Err(instance.raise_builtin_error(
                        "OverflowError",
                        "cannot convert float infinity to integer",
                    ));
                }
                return Ok(make_int(
                    _class,
                    instance,
                    IntValue::from_big(crate::bigint::BigInt::from_f64_truncated(number)),
                ));
            }
            let Some(text) = instance.text_value(*only) else {
                let name = instance.type_name(instance.type_of(*only));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!(
                        "int() argument must be a string, a bytes-like object or a real number, not '{name}'"
                    ),
                ));
            };
            // **`TS-45` ①**：`str` → `int` 的位数上限（参照实测：**前导零也计入**，
            // 符号与下划线不计；`0` ＝ 不限）。消息带实际位数，照实测原文拼。
            let limit = instance.int_max_str_digits();
            if limit != 0 {
                let digits = text.chars().filter(|character| character.is_ascii_digit()).count();
                if digits > limit as usize {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        &format!(
                            "Exceeds the limit ({limit} digits) for integer string conversion: \
                             value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
                        ),
                    ));
                }
            }
            match parse_decimal(&text) {
                Decimal::Value(value) => {
                    Ok(make_int(_class, instance, IntValue::from_big(value)))
                }
                Decimal::NotALiteral => Err(instance.raise_builtin_error(
                    "ValueError",
                    &format!("invalid literal for int() with base 10: '{text}'"),
                )),
                Decimal::NotWired => Err(crate::ExecError::Unsupported {
                    opcode: 0,
                    what: "int_new：非 ASCII 数字／超出 i64 的写法还没接线（TS-45 的任意精度是 P1-11）",
                }),
            }
        }
        // **`int(x, base)`**（第 601 轮接线 ✓；`re/_compiler.py:402` 的 `int(二进制串, 2)` 要它 ✓）。
        // 口径照参照实测 ✓：`base` 只接整数 ✓（否则 `TypeError: '<名>' object cannot be interpreted as
        // an integer` ✓）；`base` 只能是 `0` 或 `2..=36` ✓；`base == 0` 走**前缀判定** ✓
        //（`0x`／`0o`／`0b` ✓；`"0"` ✓；前导零的十进制如 `"010"` 参照**报错** ✓）；
        // `base` 为 2／8／16 时**允许**对应前缀 ✓（`int("0x10", 16)` ⇒ 16 ✓）；下划线只允许"数字之间" ✓。
        [text_arg, base_arg] => {
            let Some(base) = instance.int_of(*base_arg).and_then(|value| value.to_i64()) else {
                let name = instance.type_name(instance.type_of(*base_arg));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("'{name}' object cannot be interpreted as an integer"),
                ));
            };
            if base != 0 && !(2..=36).contains(&base) {
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    "int() base must be >= 2 and <= 36, or 0",
                ));
            }
            // **`bytes`／`bytearray` 也收** ✓（第 690 轮 ✗ 修）：参照里 `int(b'10', 2)` 合法 ✓
            //（`textwrap`／`re` 一族都靠它 ✓）⇒ 先前只认 `str` ✗ ⇒ 报
            // `TypeError: int() can't convert non-string with explicit base` ✗。字节串按 **ASCII** 解 ✓。
            let text = if let Some(text) = instance.text_value(*text_arg) {
                text
            } else if let Some(bytes) = instance.bytes_value(*text_arg) {
                match String::from_utf8(bytes.to_vec()) {
                    Ok(text) => text,
                    Err(_) => {
                        return Err(instance.raise_builtin_error(
                            "ValueError",
                            &format!("invalid literal for int() with base {base}"),
                        ));
                    }
                }
            } else {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "int() can't convert non-string with explicit base",
                ));
            };
            match parse_radix(&text, base) {
                Some(value) => Ok(make_int(_class, instance, IntValue::from_big(value))),
                None => Err(instance.raise_builtin_error(
                    "ValueError",
                    &format!("invalid literal for int() with base {base}: '{text}'"),
                )),
            }
        }
        _ => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "int_new：实参多于两个的形态还没接线",
        }),
    }
}


/// `int(text, base)` 的解析 ✓（第 601 轮）：空白／正负号／前缀／下划线都按参照实测处理 ✓。
fn parse_radix(text: &str, base: i64) -> Option<crate::bigint::BigInt> {
    let trimmed = text.trim_matches(|character: char| character.is_ascii_whitespace());
    let (negative, rest) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let lowered = rest.to_ascii_lowercase();
    let (digits, radix) = if base == 0 {
        if let Some(rest) = lowered.strip_prefix("0x") {
            (rest.to_owned(), 16)
        } else if let Some(rest) = lowered.strip_prefix("0o") {
            (rest.to_owned(), 8)
        } else if let Some(rest) = lowered.strip_prefix("0b") {
            (rest.to_owned(), 2)
        } else if lowered == "0" {
            (lowered, 10)
        } else {
            // 参照：`base == 0` 时**只认前缀写法** ✓ ⇒ 前导零的十进制（`"010"`）**报错** ✓
            return None;
        }
    } else {
        let prefix = match base {
            2 => Some("0b"),
            8 => Some("0o"),
            16 => Some("0x"),
            _ => None,
        };
        match prefix.and_then(|prefix| lowered.strip_prefix(prefix)) {
            Some(stripped) => (stripped.to_owned(), base),
            None => (lowered, base),
        }
    };
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
    {
        return None;
    }
    let digits: String = digits.chars().filter(|character| *character != '_').collect();
    if digits.is_empty() {
        return None;
    }
    let value = crate::bigint::BigInt::from_str_radix(&digits, radix as u32)?;
    Some(if negative { value.neg() } else { value })
}

/// `int` 的 `repr`：十进制（大整数走 `BigInt::to_decimal`）。
///
/// **`TS-45` ①的输出方向**：位数超过 `sys.get_int_max_str_digits()`（`0` ＝ 不限）⇒
/// `ValueError`（消息照参照**实测**，与输入方向那句不同：这句不带 `value has N digits`）。
pub unsafe fn int_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IntObject>() };
    let text = object.value.to_decimal();
    let limit = instance.int_max_str_digits();
    if limit != 0 && text.trim_start_matches('-').len() > limit as usize {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &digit_limit_message(limit),
        ));
    }
    Ok(text)
}
