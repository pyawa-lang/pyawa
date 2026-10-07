//! **`runtime` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `runtime_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`advance_iterator`、`ascii_escape`、`float_digits`、`integer_digits`、`localsplus_name`、`new_exception`、`pad_number`、`pad_text`、`release` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{advance_iterator, ascii_escape, float_digits, integer_digits, localsplus_name, new_exception, pad_number, pad_text, release};
use crate::executor::ExecError;
use crate::frame::Frame;
use crate::header::Header;
use crate::instance::Instance;
use core::ptr::NonNull;
use crate::builtin_objects::StrObject;
use crate::type_object::TypeObject;
use crate::value::Value;
use crate::executor::ctrls::exception_type;
use crate::opcode;


pub(crate) fn opcode_of(name: &str) -> u8 {
    opcode::opcode(name).unwrap_or_else(|| panic!("opmap 缺 {name}")) as u8
}

/// 把一份**新引用**交给帧的值栈（`BC-43`）。
pub(crate) fn push(instance: &Instance, frame: &Frame, raw: NonNull<Header>) -> Result<(), ExecError> {
    frame.push(instance.own(raw).into_raw())?;
    Ok(())
}

/// 推进一个**按下标走**的迭代器（`FOR_ITER` 与"普通迭代器的 `SEND`"共用同一份逻辑）。
///
/// 返回 `Some(元素新引用)` 或 `None`（已耗尽）。**只认**本层接线的迭代器类型。
/// **迭代推进**的公开入口（`paL_next` 与测试用）：取下一个元素；耗尽给 `None`。
///
/// 与执行器内部那条是**同一处实现**（`opcode` 只用于错误消息，公开入口给 0）。
pub fn advance(
    instance: &Instance,
    iterator: NonNull<Header>,
) -> Result<Option<NonNull<Header>>, ExecError> {
    advance_iterator(instance, iterator, 0)
}

/// 按名字取一个已注册的内建类型（`TS-41` 的表是层次的出处）。
pub(crate) fn builtin_type(instance: &Instance, name: &str) -> NonNull<TypeObject> {
    instance
        .type_named(name)
        .unwrap_or_else(|| panic!("TS-41：{name} 应当已注册"))
}

/// 一个 `str` 对象的内容是否等于给定的 Rust 字符串。
pub(crate) fn str_matches_public(instance: &Instance, raw: NonNull<Header>, expected: &str) -> bool {
    // SAFETY: 调用方保证 raw 是存活对象。
    if unsafe { raw.as_ref() }.ty() != instance.singletons().str_type() {
        return false;
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value() == expected
}

/// **`str % value`**（printf 风格）✓（第 281 轮）：`Lib/` 里遍地都是 ✓ —— 实测先撞上的是
/// `codecs.py` 的 `raise SystemError('… %s' % e)` ✓（`encodings.*` 那一族因此全红 ✗）。
///
/// 口径照参照**实测**：
/// - 右操作数是**元组** ⇒ 位置实参；否则 ⇒ **单个**实参；格式里出现 `%(名字)` ⇒ 右操作数必须是
///   **映射**（否则 `TypeError: format requires a mapping` ✓）；
/// - 转换字符：`s`／`r`／`a`／`d`／`i`／`u`／`o`／`x`／`X`／`f`／`F`／`e`／`E`／`g`／`G`／`c`／`%%`；
/// - 修饰：`-`／`+`／空格／`#`／`0`／宽度／`.精度`；长度修饰符（`h`／`l`／`L`）**照参照忽略** ✓；
/// - 错误消息照实测：`not enough arguments for format string`／
///   `not all arguments converted during string formatting`／
///   `%d format: a real number is required, not str` ✓。
pub(crate) fn percent_format(
    instance: &Instance,
    template: &str,
    right: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    let _ = opcode;
    let characters: Vec<char> = template.chars().collect();
    // 右操作数的两种形态（**一处真相**：类型名取注册表 ✓）
    let items: Option<Vec<NonNull<Header>>> =
        if instance.type_name(instance.type_of(right)) == "tuple" {
            instance.tuple_items(right)
        } else {
            None
        };
    let mapping = if instance.type_name(instance.type_of(right)) == "dict" {
        Some(right)
    } else {
        None
    };
    let mut out = String::new();
    let mut cursor = 0usize;
    let mut argument_index = 0usize;
    let mut mapping_used = false;
    let mut positional_used = false;
    while cursor < characters.len() {
        if characters[cursor] != '%' {
            out.push(characters[cursor]);
            cursor += 1;
            continue;
        }
        cursor += 1;
        if cursor >= characters.len() {
            return Err(instance.raise_builtin_error("ValueError", "incomplete format"));
        }
        if characters[cursor] == '%' {
            out.push('%');
            cursor += 1;
            continue;
        }
        // `%(名字)`
        let mut key: Option<String> = None;
        if characters[cursor] == '(' {
            cursor += 1;
            let mut name = String::new();
            while cursor < characters.len() && characters[cursor] != ')' {
                name.push(characters[cursor]);
                cursor += 1;
            }
            if cursor >= characters.len() {
                return Err(instance.raise_builtin_error("ValueError", "incomplete format key"));
            }
            cursor += 1;
            key = Some(name);
        }
        // 修饰符
        let (mut left_align, mut plus, mut space, mut alternate, mut zero) =
            (false, false, false, false, false);
        while cursor < characters.len() {
            match characters[cursor] {
                '-' => left_align = true,
                '+' => plus = true,
                ' ' => space = true,
                '#' => alternate = true,
                '0' => zero = true,
                _ => break,
            }
            cursor += 1;
        }
        // 宽度
        if cursor < characters.len() && characters[cursor] == '*' {
            return Err(instance.raise_builtin_error(
                "NotImplementedError",
                "`%*` 的宽度取自实参尚未接线（宽度写死在格式串里可以）",
            ));
        }
        let mut width: Option<usize> = None;
        while cursor < characters.len() && characters[cursor].is_ascii_digit() {
            let digit = characters[cursor] as usize - '0' as usize;
            width = Some(width.unwrap_or(0) * 10 + digit);
            cursor += 1;
        }
        // 精度
        let mut precision: Option<usize> = None;
        if cursor < characters.len() && characters[cursor] == '.' {
            cursor += 1;
            let mut value = 0usize;
            while cursor < characters.len() && characters[cursor].is_ascii_digit() {
                let digit = characters[cursor] as usize - '0' as usize;
                value = value * 10 + digit;
                cursor += 1;
            }
            precision = Some(value);
        }
        // 长度修饰符（参照忽略）
        while cursor < characters.len() && matches!(characters[cursor], 'h' | 'l' | 'L') {
            cursor += 1;
        }
        if cursor >= characters.len() {
            return Err(instance.raise_builtin_error("ValueError", "incomplete format"));
        }
        let conversion = characters[cursor];
        cursor += 1;
        if conversion == '%' {
            out.push('%');
            continue;
        }
        // 取实参
        let value: NonNull<Header> = if let Some(key) = &key {
            if positional_used {
                return Err(instance.raise_builtin_error("TypeError", "format requires a mapping"));
            }
            mapping_used = true;
            let Some(mapping) = mapping else {
                return Err(instance.raise_builtin_error("TypeError", "format requires a mapping"));
            };
            // 消息**原样**给键 ✓（本层异常的 `str` 走 repr ⇒ 与参照的 `KeyError: 'a'` 同形 ✓）
            instance
                .dict_get(mapping, key)
                .ok_or_else(|| instance.raise_builtin_error("KeyError", key))?
        } else {
            if mapping_used {
                return Err(instance.raise_builtin_error("TypeError", "format requires a mapping"));
            }
            positional_used = true;
            match &items {
                Some(items) => *items.get(argument_index).ok_or_else(|| {
                    instance.raise_builtin_error("TypeError", "not enough arguments for format string")
                })?,
                None => {
                    if argument_index > 0 {
                        return Err(instance.raise_builtin_error(
                            "TypeError",
                            "not enough arguments for format string",
                        ));
                    }
                    right
                }
            }
        };
        argument_index += 1;
        // 转换
        let numeric = matches!(
            conversion,
            'd' | 'i' | 'u' | 'o' | 'x' | 'X' | 'f' | 'F' | 'e' | 'E' | 'g' | 'G'
        );
        let sign_and_body: (String, String) = match conversion {
            's' => (String::new(), instance.object_str(value)?),
            'r' => (String::new(), instance.object_repr(value)?),
            // `%a`：参照给 **ascii()**（非 ASCII 转义）—— 本层按 `repr` 的结果再转义非 ASCII ✓
            'a' => (String::new(), ascii_escape(&instance.object_repr(value)?)),
            'c' => {
                let body = if let Some(number) = instance.int_value(value) {
                    match u32::try_from(number).ok().and_then(char::from_u32) {
                        Some(character) => character.to_string(),
                        None => {
                            return Err(instance.raise_builtin_error(
                                "OverflowError",
                                "%c arg not in range(0x110000)",
                            ))
                        }
                    }
                } else if let Some(text) = instance.text_value(value) {
                    let length = text.chars().count();
                    if length == 1 {
                        text
                    } else {
                        return Err(instance.raise_builtin_error(
                            "TypeError",
                            &format!(
                                "%c requires an int or a unicode character, not a string of length {length}"
                            ),
                        ));
                    }
                } else {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        &format!(
                            "%c requires an int or a unicode character, not {}",
                            instance.type_name(instance.type_of(value))
                        ),
                    ));
                };
                (String::new(), body)
            }
            'd' | 'i' | 'u' | 'o' | 'x' | 'X' => {
                let base = match conversion {
                    'o' => 8,
                    'x' | 'X' => 16,
                    _ => 10,
                };
                let upper = conversion == 'X';
                let (negative, digits) =
                    integer_digits(instance, value, base, upper, conversion, base != 10)?;
                let sign = if negative {
                    "-".to_owned()
                } else if plus {
                    "+".to_owned()
                } else if space {
                    " ".to_owned()
                } else {
                    String::new()
                };
                let prefix = if alternate {
                    match conversion {
                        'x' => "0x".to_owned(),
                        'X' => "0X".to_owned(),
                        'o' => "0o".to_owned(),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                };
                pad_number(
                    &mut out, &sign, &prefix, &digits, width, left_align, zero && !left_align,
                );
                continue;
            }
            'f' | 'F' | 'e' | 'E' | 'g' | 'G' => {
                let body = float_digits(instance, value, conversion, precision.unwrap_or(6))?;
                (String::new(), body)
            }
            other => {
                // 参照实测：`ValueError: unsupported format character 'q' (0x71) at index 1`
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    &format!(
                        "unsupported format character '{other}' (0x{:x}) at index {}",
                        other as u32,
                        cursor - 1
                    ),
                ));
            }
        };
        let (sign, body) = sign_and_body;
        let sign = if numeric && sign.is_empty() && !body.starts_with('-') {
            if plus {
                "+".to_owned()
            } else if space {
                " ".to_owned()
            } else {
                sign
            }
        } else {
            sign
        };
        // **精度对 `%s` 一族是截断** ✓（参照 `'%.2s' % 'abcdef'` ⇒ `'ab'` ✓）
        let body = if !numeric {
            match precision {
                Some(limit) => body.chars().take(limit).collect(),
                None => body,
            }
        } else {
            body
        };
        if numeric {
            let (negative, digits) = if let Some(rest) = body.strip_prefix('-') {
                (true, rest.to_owned())
            } else {
                (false, body.clone())
            };
            let sign = if negative { "-".to_owned() } else { sign };
            pad_number(&mut out, &sign, "", &digits, width, left_align, zero && !left_align);
        } else {
            pad_text(&mut out, &body, width, left_align);
        }
    }
    // 位置实参没被用完 ⇒ 照参照报错 ✓（`'%s' % (1, 2)`）
    if positional_used {
        if let Some(items) = &items {
            if argument_index < items.len() {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "not all arguments converted during string formatting",
                ));
            }
        }
    }
    Ok(instance.new_str(&out))
}

/// 二元运算的类型不匹配错误（**一处真相**）：`unsupported operand type(s) for <op>: 'A' and 'B'`
/// （参照实测）。
pub(crate) fn unsupported_operand(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
) -> ExecError {
    let left_name = instance.type_name(instance.type_of(left));
    let right_name = instance.type_name(instance.type_of(right));
    instance.raise_builtin_error(
        "TypeError",
        &{
            // **句首站点** ✓（第 663 轮，门控 `PYAWA_BINOP_DEBUG=1`）：两个类型名放句首 ✓
            //（尾巴会被长消息挤掉 ✗ —— 这是这条链上反复吃过亏的地方 ✓）。
            if crate::diag::flag("PYAWA_BINOP_DEBUG") {
                eprintln!(
                    "[binop] {symbol} 左='{left_name}' 右='{right_name}' 站点={}",
                    instance.current_site()
                );
            }
            format!("unsupported operand type(s) for {symbol}: '{left_name}' and '{right_name}'")
        },
    )
}

/// 抛一个异常：记在实例上（借它保活）并交出错误（`BC-60` ②）。
pub(crate) fn raise(instance: &Instance, exception: NonNull<Header>) -> ExecError {
    // **两份所有权**：实例状态一份、`Err(Raised)` 一份——所以这里必须为状态**新增**一份。
    // 少了这一步就是双重所有权：状态与错误各自以为"我持有它"，先释放的一方让另一方悬垂
    // （症状：调用方拿到 `Err(Raised)` 里的异常对象时读到已释放内存）。
    // `Err(Raised)` 那一份由派发器接手（`push` 进值栈）或由调用方消费。
    // SAFETY: exception 由调用方保证存活。
    unsafe { instance.incref_object(exception.as_ptr()) };
    if let Some(previous) = instance.set_pending_exception(Some(exception)) {
        release(instance, previous);
    }
    ExecError::Raised { exception }
}

/// 抛一个内建异常（带消息）。
pub(crate) fn raise_builtin(instance: &Instance, name: &str, message: &str) -> ExecError {
    let exception = new_exception(instance, exception_type(instance, name), message);
    raise(instance, exception)
}

/// **未绑定局部槽** ✓（第 277 轮诊断升级）：参照在同样情形给
/// `UnboundLocalError: cannot access local variable '<名>' where it is not associated with a value` ✓
/// ⇒ 照它报，并带上**变量名**（先前只报"槽 N 未绑定（未接线）" ✗ ⇒ 无从下手 ✓）。
pub(crate) fn unbound_local_error(
    instance: &Instance,
    frame: &Frame,
    slot: usize,
) -> ExecError {
    let name = match frame.code() {
        Some(header) => {
            let code = unsafe { &*header.as_ptr().cast::<crate::CodeObject>() };
            localsplus_name(code, slot)
        }
        None => "<未知>".to_owned(),
    };
    instance.raise_builtin_error(
        "UnboundLocalError",
        &format!("cannot access local variable '{name}' where it is not associated with a value"),
    )
}

/// 把 [`Value`] 变成帧值栈要的**新引用**（内联的那几种换算成它们对应的单例）。
pub(crate) fn value_into_raw(instance: &Instance, value: Value<'_>) -> NonNull<Header> {
    let (raw, needs_reference) = match value {
        Value::None => (instance.singletons().none(), true),
        Value::Bool(flag) => (instance.singletons().boolean(flag), true),
        Value::Int(number) => (
            instance
                .singletons()
                .small_int(number)
                .expect("内联整数一定落在单例区间（OM-39）"),
            true,
        ),
        Value::Object(reference) => (reference.into_raw(), false),
    };
    if needs_reference {
        // SAFETY: 单例由实例持有，存活。
        unsafe { instance.incref_object(raw.as_ptr()) };
    }
    raw
}
