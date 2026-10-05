//! **`arithmetic` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `arithmetic_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`builtin_type`、`inplace_arithmetic`、`numeric_payload`、`percent_format`、`raise`、`sequence_repeat`、`set_operation`、`unsupported_operand` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{builtin_type, numeric_payload, percent_format, sequence_repeat, set_operation, unsupported_operand};
use crate::executor::ExecError;
use crate::header::Header;
use crate::instance::Instance;
use crate::bigint::IntValue;
use core::ptr::NonNull;
use crate::type_object::TypeObject;


/// **整数算术的公开入口**（`TS-40` 的数值面；**任意精度**见 `TS-45`）。
///
/// `symbol` 取 `"+"`／`"-"`／`"*"`／`"//"`／`"%"`／`"**"`／位运算与移位
/// （`operator.*` 与 `BINARY_OP` 共用）。非整数（浮点还没落地、或字符串这类）按**参照实测**
/// 的消息报 `TypeError: unsupported operand type(s) for +: 'int' and 'str'`。
///
/// **一条真相**：四则／整除／取模／幂／位运算／移位一律走 [`crate::bigint`] 的任意精度核心；
/// 补码语义（负数）与 floor 位移由核心负责，这里只管类型检查与参照实测的消息。
pub fn arithmetic_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // **`str % value`**（printf 风格）✓（第 281 轮）：`Lib/` 里遍地都是 ✓ —— 实测第一个撞上的是
    // `codecs.py` 的 `raise SystemError('… %s' % e)` ✓（`encodings.*` 那一族因此全红 ✗）。
    if symbol == "%" {
        if let Some(template) = instance.text_value(left) {
            return percent_format(instance, &template, right, opcode);
        }
    }
    // **集合运算**（第 102 轮）：`&`／`|`／`-`／`^` —— 先前一律落到"一处真相"的
    // `unsupported_operand` ✗ ⇒ 上限榜那一整族（**119** 个模块 ✓）的**第一句错**就是 `set & set` ✓
    // （`Lib/enum.py` 里的集合运算 ✓，`argparse`／`asyncio` 一族都压在它上面 ✓）。
    // 认的是**子类型** ✓（`frozenset` 也算 ✓）；结果一律**新的 `set`** ✓（照参照 ✓）。
    if matches!(symbol, "&" | "|" | "-" | "^") {
        let is_set = |ty: NonNull<TypeObject>| {
            ["set", "frozenset"].iter().any(|name| {
                instance
                    .type_named(name)
                    .is_some_and(|base| instance.is_subtype(ty, base))
            })
        };
        if is_set(instance.type_of(left)) && is_set(instance.type_of(right)) {
            return Ok(set_operation(instance, left, right, symbol));
        }
    }
    // **序列重复 `*`**（第 305 轮）：`"-" * 40`、`[0] * 3`、`b"ab" * 2` —— `Lib/` 里遍地都是。
    // 实测第一处撞上的是 `Lib/traceback.py` 的 `f"{'a' * 3}"`：先前直接落到整数那条路 ⇒
    // `TypeError: unsupported operand type(s) for *: 'str' and 'int'`。
    if symbol == "*" {
        if let Some(repeated) = sequence_repeat(instance, left, right)? {
            return Ok(repeated);
        }
    }
    // **真除法 `/`**：结果为 **float**（实测 `7/2 == 3.5`、`0/5 == 0.0`），
    // 除零报 `ZeroDivisionError: division by zero`（与 `//`／`%` 同一条消息）；
    // 大整数超出 double ⇒ 参照报 `OverflowError: int too large to convert to float`
    // `@`（矩阵乘）：本层没有矩阵类型 ⇒ **如实报参照实测的 `TypeError`**
    // （`1 @ 2` ⇒ `unsupported operand type(s) for @: 'int' and 'int'`）；
    // `@=` 一族由 `inplace_arithmetic` 走到这里，消息里的符号随之是 `@`
    if symbol == "@" {
        return Err(unsupported_operand(instance, left, right, "@"));
    }
    if symbol == "/" {
        let left_number = numeric_payload(instance, left).ok_or_else(|| {
            unsupported_operand(instance, left, right, "/")
        })?;
        let right_number = numeric_payload(instance, right).ok_or_else(|| {
            unsupported_operand(instance, left, right, "/")
        })?;
        if let Some(value) = instance.int_of(left).map(|value| value.to_bigint()) {
            if value.to_f64().is_infinite() {
                return Err(instance.raise_builtin_error(
                    "OverflowError",
                    "int too large to convert to float",
                ));
            }
        }
        if let Some(value) = instance.int_of(right).map(|value| value.to_bigint()) {
            if value.to_f64().is_infinite() {
                return Err(instance.raise_builtin_error(
                    "OverflowError",
                    "int too large to convert to float",
                ));
            }
        }
        if right_number == 0.0 {
            return Err(instance.raise_builtin_error("ZeroDivisionError", "division by zero"));
        }
        return Ok(instance.new_float(left_number / right_number));
    }
    // **浮点四则**（第 318 轮，参照口径）：**任一侧是 float ⇒ 结果就是 float** ✓
    //（`1 + 2.0 == 3.0` ✓）。先前这条整段缺失 ⇒ `1.5 + 0.5` 报
    // `unsupported operand type(s) for +: 'float' and 'float'` ✗（`Lib/` 里浮点遍地都是 ✓）。
    let float_type = builtin_type(instance, "float");
    // SAFETY: 两个指针都由调用方保证存活。
    let float_involved = unsafe { left.as_ref() }.ty() == float_type
        || unsafe { right.as_ref() }.ty() == float_type;
    if float_involved {
        let a = numeric_payload(instance, left)
            .ok_or_else(|| unsupported_operand(instance, left, right, symbol))?;
        let b = numeric_payload(instance, right)
            .ok_or_else(|| unsupported_operand(instance, left, right, symbol))?;
        // 超大整数折成 `f64` 会到无穷 ⇒ 参照报 `OverflowError`（与 `/` 那条同一口径 ✓）
        if b.is_infinite() || a.is_infinite() {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "int too large to convert to float",
            ));
        }
        let result = match symbol {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" => {
                if b == 0.0 {
                    return Err(instance.raise_builtin_error("ZeroDivisionError", "division by zero"));
                }
                a / b
            }
            "//" => {
                if b == 0.0 {
                    return Err(instance.raise_builtin_error("ZeroDivisionError", "division by zero"));
                }
                (a / b).floor()
            }
            "%" => {
                if b == 0.0 {
                    return Err(instance.raise_builtin_error("ZeroDivisionError", "division by zero"));
                }
                // 参照的 `%` 取**除数**的符号（`math.fmod` 取被除数 ⇒ 不能直接用 ✓）
                a - (a / b).floor() * b
            }
            "**" => {
                // 负底数 ＋ 非整数指数在参照里给**复数** ⇒ 本层如实报未实现（不静默给 NaN ✗）
                if a < 0.0 && b.fract() != 0.0 {
                    return Err(ExecError::Unsupported {
                        opcode,
                        what: "浮点幂：负底数配非整数指数（参照给复数）尚未接线",
                    });
                }
                a.powf(b)
            }
            _ => return Err(unsupported_operand(instance, left, right, symbol)),
        };
        return Ok(instance.new_float(result));
    }
    if let (Some(a), Some(b)) = (instance.int_of(left), instance.int_of(right)) {
        // 除零在参照里是 `ZeroDivisionError: division by zero`（实测）——`//` 与 `%` 都一样
        if b.is_zero() && matches!(symbol, "//" | "%") {
            return Err(instance.raise_builtin_error("ZeroDivisionError", "division by zero"));
        }
        let (wide_left, wide_right) = (a.to_bigint(), b.to_bigint());
        let result = match symbol {
            "+" => wide_left.add(&wide_right),
            "-" => wide_left.sub(&wide_right),
            "*" => wide_left.mul(&wide_right),
            "//" => wide_left.divmod_floor(&wide_right).expect("除零已在上面拦下").0,
            "%" => wide_left.divmod_floor(&wide_right).expect("除零已在上面拦下").1,
            "**" => {
                // 负指数在参照里给 `float`（`2 ** -1 == 0.5`）⇒ 与 `float` 互转接线前如实报未实现
                let Some(exponent) = b.to_i64().and_then(|value| u32::try_from(value).ok()) else {
                    return Err(ExecError::Unsupported {
                        opcode,
                        what: "整数幂：指数为负（参照给 float）或超出 u32，尚未接线",
                    });
                };
                wide_left.pow_u32(exponent)
            }
            "&" | "|" | "^" => match symbol {
                // 补码语义（负数无限符号扩展），核心已按参照夹具对拍
                "&" => wide_left.bit_and(&wide_right),
                "|" => wide_left.bit_or(&wide_right),
                _ => wide_left.bit_xor(&wide_right),
            },
            "<<" | ">>" => {
                // 位移量：负数 ⇒ `ValueError`（实测 `negative shift count`）
                let Some(count) = b.to_i64() else {
                    // 装不下 `i64` 的位移量：`<<` 一律超出实现上限 ⇒ `MemoryError`（实测同款）；
                    // `>>` 一定超过位宽 ⇒ 正数 0、负数 -1（`shr` 自己处理）
                    if symbol == "<<" {
                        return Err(instance.raise_builtin_error("MemoryError", ""));
                    }
                    return Ok(instance.new_int_value(IntValue::from_big(wide_left.shr(u64::MAX))));
                };
                if count < 0 {
                    return Err(instance.raise_builtin_error("ValueError", "negative shift count"));
                }
                let count = count as u64;
                if symbol == "<<" {
                    let Some(shifted) = wide_left.shl(count) else {
                        // 实测：`1 << 2**62` ⇒ `MemoryError`（消息为空）
                        return Err(instance.raise_builtin_error("MemoryError", ""));
                    };
                    shifted
                } else {
                    wide_left.shr(count)
                }
            }
            _ => {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "arithmetic_public 收到了没见过的运算符",
                })
            }
        };
        return Ok(instance.new_int_value(IntValue::from_big(result)));
    }
    Err(unsupported_operand(instance, left, right, symbol))
}
