//! **`format` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `format_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * **`advance_iterator` 已搬到 [`super::iter_advance`]**（它属迭代器协议，不是 format 族）✓；
//! * 仍在原处的被引用项：`concat_public`、`contains`、`iter_value`、`iterable_item`、`iterable_length`、`normalize_exponent`、`strip_trailing_zeros`、`truthiness` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{normalize_exponent, strip_trailing_zeros};
use core::cell::Cell;
use crate::CodeObject;
use crate::ExceptionObject;
use crate::executor::ExecError;
use crate::header::Header;
use crate::instance::Instance;
use core::ptr::NonNull;
use core::cell::RefCell;
use crate::StrObject;
use crate::TypeObject;
use crate::executor::raise_builtin;


/// 释放一份引用（`OM-20`）。
pub(crate) fn release(instance: &Instance, raw: NonNull<Header>) {
    // SAFETY: 调用方交出的是一份新引用。
    unsafe { instance.release_object(raw.as_ptr()) };
}

/// 把一个值按**格式规格**文本渲染 ✓（`FORMAT_WITH_SPEC` 与 `str.format` 的**同一处** ✓）。
///
/// ① 属性通道里的 `__format__`（Python 级覆写优先 ✓，`TS-44` ✓）；② 原生 `format` 槽 ✓；
/// ③ 都不认 ⇒ 参照**实测**那句 `unsupported format string passed to X.__format__` ✓。
///
/// 返回的是**新引用**的 `str` ✓；`value` 本身**不在这里释放** ✗（调用方持有 ✓）。
pub(crate) fn format_value_with_spec(
    instance: &Instance,
    value: NonNull<Header>,
    spec_text: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: value 由调用方保证存活。
    let value_type = unsafe { value.as_ref() }.ty();
    // SAFETY: 类型名由注册表持有。
    let class_name = unsafe { value_type.as_ref() }.name().to_owned();
    match crate::executor::attribute_lookup(instance, value, "__format__") {
        Ok(crate::executor::Attribute::Method { function, this }) => {
            let args = vec![instance.new_str(spec_text)];
            crate::executor::call_callable(instance, function, Some(this), args, Vec::new(), opcode)
        }
        Ok(crate::executor::Attribute::Value(method))
        | Ok(crate::executor::Attribute::Owned(method)) => {
            let args = vec![instance.new_str(spec_text)];
            crate::executor::call_callable(instance, method, Some(value), args, Vec::new(), opcode)
        }
        Err(_) => {
            let message = format!("unsupported format string passed to {class_name}.__format__");
            Err(raise_builtin(instance, "TypeError", &message))
        }
    }
}

/// 按宽度／对齐／零填充拼一个**数值三段**（符号＋前缀＋数字）✓。
pub(crate) fn pad_number(
    out: &mut String,
    sign: &str,
    prefix: &str,
    digits: &str,
    width: Option<usize>,
    left_align: bool,
    zero: bool,
) {
    let total = sign.chars().count() + prefix.chars().count() + digits.chars().count();
    match width {
        Some(width) if width > total => {
            let fill = width - total;
            if left_align {
                out.push_str(sign);
                out.push_str(prefix);
                out.push_str(digits);
                for _ in 0..fill {
                    out.push(' ');
                }
            } else if zero {
                out.push_str(sign);
                out.push_str(prefix);
                for _ in 0..fill {
                    out.push('0');
                }
                out.push_str(digits);
            } else {
                for _ in 0..fill {
                    out.push(' ');
                }
                out.push_str(sign);
                out.push_str(prefix);
                out.push_str(digits);
            }
        }
        _ => {
            out.push_str(sign);
            out.push_str(prefix);
            out.push_str(digits);
        }
    }
}

/// 按宽度／对齐拼一段文本（`%s` 一族；**零填充对它无效** ✓，照参照 ✓）。
pub(crate) fn pad_text(out: &mut String, text: &str, width: Option<usize>, left_align: bool) {
    let length = text.chars().count();
    match width {
        Some(width) if width > length => {
            let fill = width - length;
            if left_align {
                out.push_str(text);
                for _ in 0..fill {
                    out.push(' ');
                }
            } else {
                for _ in 0..fill {
                    out.push(' ');
                }
                out.push_str(text);
            }
        }
        _ => out.push_str(text),
    }
}

/// 取整数的（符号, 数字）——`i64` 直接排；**大整数**十进制借用它的 `str` ✓（其它进制如实报未接线）；
/// **浮点**按参照**截断**（`'%d' % 3.7` ⇒ `'3'`、`'%d' % -3.7` ⇒ `'-3'` ✓）。
pub(crate) fn integer_digits(
    instance: &Instance,
    value: NonNull<Header>,
    base: u32,
    upper: bool,
    conversion: char,
    integer_only: bool,
) -> Result<(bool, String), ExecError> {
    let digits = |magnitude: u64| match (base, upper) {
        (10, _) => magnitude.to_string(),
        (16, false) => format!("{magnitude:x}"),
        (16, true) => format!("{magnitude:X}"),
        (8, _) => format!("{magnitude:o}"),
        _ => magnitude.to_string(),
    };
    if let Some(number) = instance.int_value(value) {
        return Ok((number < 0, digits(number.unsigned_abs())));
    }
    if instance.int_of(value).is_some() {
        let text = instance.object_str(value)?;
        if base != 10 {
            return Err(instance.raise_builtin_error(
                "NotImplementedError",
                "大整数的 `%x`／`%X`／`%o` 尚未接线（十进制可以）",
            ));
        }
        return Ok((text.starts_with('-'), text.trim_start_matches('-').to_owned()));
    }
    if let Some(number) = instance.float_value(value) {
        if integer_only {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "%{conversion} format: an integer is required, not {}",
                    instance.type_name(instance.type_of(value))
                ),
            ));
        }
        if !number.is_finite() {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "cannot convert float infinity to integer",
            ));
        }
        let truncated = number.trunc();
        if truncated.abs() < 9.2e18 {
            let integer = truncated as i64;
            return Ok((integer < 0, digits(integer.unsigned_abs())));
        }
        // 大浮点：借 `str` 的整数部分（如实；精确路径随后补 ✓）
        let text = format!("{truncated:.0}");
        return Ok((text.starts_with('-'), text.trim_start_matches('-').to_owned()));
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!(
            "%{conversion} format: a real number is required, not {}",
            instance.type_name(instance.type_of(value))
        ),
    ))
}

/// 浮点转换（`f`／`F`／`e`／`E`／`g`／`G`）——口径照 C 的 printf（精度默认 **6** ✓）。
pub(crate) fn float_digits(
    instance: &Instance,
    value: NonNull<Header>,
    conversion: char,
    precision: usize,
) -> Result<String, ExecError> {
    let number = instance
        .float_value(value)
        .or_else(|| instance.int_value(value).map(|integer| integer as f64));
    let Some(number) = number else {
        // 参照实测：`'%f' % 'x'` ⇒ `TypeError: must be real number, not str` ✓
        // （与 `%d` 那条 `%d format: a real number is required, not str` **不同** ✓ —— 两条都量过 ✓）
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("must be real number, not {}", instance.type_name(instance.type_of(value))),
        ));
    };
    let upper = conversion.is_ascii_uppercase();
    if number.is_nan() {
        return Ok(if upper { "NAN" } else { "nan" }.to_owned());
    }
    if number.is_infinite() {
        let text = if upper { "INF" } else { "inf" };
        return Ok(if number.is_sign_negative() {
            format!("-{text}")
        } else {
            text.to_owned()
        });
    }
    let sign = if number.is_sign_negative() { "-" } else { "" };
    let magnitude = number.abs();
    let body = match conversion {
        'f' | 'F' => format!("{magnitude:.precision$}"),
        'e' | 'E' => normalize_exponent(&format!("{magnitude:.precision$e}"), upper),
        _ => {
            // `%g` 的口径：指数 < -4 或 ≥ 精度 ⇒ 走 `e`（精度 −1），否则走 `f`（精度 −1−指数）；
            // 末尾的零与孤立的小数点**去掉**（除非给了 `#` —— 本层 `#` 的这条支随后补 ✓）
            let significant = precision.max(1);
            let exponent = if magnitude == 0.0 {
                0
            } else {
                magnitude.log10().floor() as i32
            };
            if exponent < -4 || exponent >= significant as i32 {
                let text = normalize_exponent(
                    &format!("{magnitude:.prec$e}", prec = significant - 1),
                    upper,
                );
                strip_trailing_zeros(&text)
            } else {
                let decimals = (significant as i32 - 1 - exponent).max(0) as usize;
                let text = format!("{magnitude:.decimals$}");
                strip_trailing_zeros(&text)
            }
        }
    };
    Ok(format!("{sign}{body}"))
}

/// `%a` 用：把非 ASCII 字符转义成 `\xNN`／`\uNNNN`／`\UNNNNNNNN` ✓（照 `ascii()` 的口径 ✓）。
pub(crate) fn ascii_escape(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        if character.is_ascii() {
            escaped.push(character);
        } else if (character as u32) <= 0xFF {
            escaped.push_str(&format!("\\x{:02x}", character as u32));
        } else if (character as u32) <= 0xFFFF {
            escaped.push_str(&format!("\\u{:04x}", character as u32));
        } else {
            escaped.push_str(&format!("\\U{:08x}", character as u32));
        }
    }
    escaped
}

/// 造一个异常实例（`args` 只有一个 `str` 消息）——**新引用**。
pub(crate) fn new_exception(instance: &Instance, ty: NonNull<TypeObject>, message: &str) -> NonNull<Header> {
    let text = instance
        .alloc(StrObject::new(
            instance.singletons().str_type(),
            message.to_owned(),
        ))
        .into_raw()
        .cast::<Header>();
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(vec![text]),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
        RefCell::new(None),
    ));
    object.into_raw().cast::<Header>()
}

/// 造一个异常实例，`args` 用给定的那批**新引用**（异常对象接手）。
pub(crate) fn new_exception_with_args(
    instance: &Instance,
    ty: NonNull<TypeObject>,
    args: Vec<NonNull<Header>>,
) -> NonNull<Header> {
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(args),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
        RefCell::new(None),
    ));
    object.into_raw().cast::<Header>()
}

/// 按**指令序号**取位置表里的行（`BC-18`）：`offset` 是**码元**偏移。
/// **槽号 → 名字** ✓（第 267 轮诊断用）：按 `localsplus` 那条规矩 ✓（局部在前 ✓、追加的 cell 其次 ✓、free 最后 ✓）。
pub(crate) fn localsplus_name(code: &CodeObject, slot: usize) -> String {
    if slot < code.nlocals() {
        if let Some(name) = code.varname(slot) {
            return name.to_owned();
        }
    }
    // 形参 cell 的判断走 `varname(slot)` ✓（`CodeObject` 没有整表的访问器 ✓）。
    let is_parameter = |name: &String| {
        (0..code.nlocals()).any(|slot| code.varname(slot) == Some(name.as_str()))
    };
    let appended: Vec<&String> = code.cellvars().iter().filter(|name| !is_parameter(name)).collect();
    if let Some(name) = appended.get(slot.saturating_sub(code.nlocals())) {
        return (*name).clone();
    }
    let base = code.nlocals() + appended.len();
    if let Some(name) = code.freevars().get(slot.saturating_sub(base)) {
        return name.clone();
    }
    "<未知>".to_owned()
}
