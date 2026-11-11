//! **`arithmetic` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `arithmetic_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`inplace_arithmetic`、`numeric_payload`、`percent_format`、`raise`、`sequence_repeat`、`set_operation`、`unsupported_operand` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{numeric_payload, percent_format, sequence_repeat, set_operation, unsupported_operand};
use crate::HotId;
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
    // **`str % value` 优先于反射 `__rmod__`** ✓（`P3-42` round 44 修 ✗）：参照里 `'%s' % obj`
    // 由 **str 自己的 `__mod__`** 处理（它接受任意右操作数）⇒ **不会**再问 `obj.__rmod__` ✓。
    // 先前先跑 `binary_override` ⇒ `UserString.__rmod__` → `str(template) % self` → 再次反射 ⇒
    // **无限递归**（`test_userstring.test_rmod` 的 `fmt2 % str3` ✓）。只有**恰是 `str`** 走
    // 这条快路（`str` 子类覆写 `__mod__` 时仍按用户覆写 ✓）。
    if symbol == "%" && instance.type_of(left) == instance.singletons().str_type() {
        if let Some(template) = instance.text_value(left) {
            return percent_format(instance, &template, right, opcode);
        }
    }
    // **用户类覆写优先** ✓（第 411 轮，task-22 ✓）：`int` 子类（真 `enum.py` 的 `IntFlag`）在类体里
    // 定义 `__or__`／`__and__`／`__xor__` 一族 ⇒ 参照先查 `type(left).__or__` ✓、命中就以 Python
    // 语义调用 ✓；先前这里按**载荷**（int）直接走原生运算 ✗ ⇒ `Flags.X | Flags.Y` 得到**普通 int**
    // （缺 `.value`／`._name_`）✗ ⇒ `IntFlag` 组合值全错 ✓。
    let (overridden, had_override) = binary_override(instance, left, right, symbol, opcode)?;
    if let Some(result) = overridden {
        return Ok(result);
    }
    if had_override {
        // **左侧用户覆写返回 `NotImplemented` ⇒ 先问右侧的"原生反射"**（round 222 真 bug 修 ✗）：
        // 参照的 `binary_op1` 在左槽 NotImpl 后仍会调**右操作数的槽**（反序）✓ ——
        // `Counter.update` 的 `Evil + 1`（`Evil(int)` 的 `__add__` 返回 NotImplemented）⇒ 参照落
        // `int.__radd__` 得 `1` ✓；先前直接报"unsupported operand" ✗
        //（`test_collections.TestCounter.test_update_reentrant_add_clears_counter` ✓）。
        // 只在右侧**不是用户类**时试（两侧都用户类且都 NotImpl ⇒ 照旧报错 ✓，round 67 口径 ✓）。
        let right_type = instance.type_of(right);
        if !instance.is_user_type_object(right_type) {
            if let Ok(result) = arithmetic_public(instance, right, left, symbol, opcode) {
                return Ok(result);
            }
        }
        // **双方都返回 `NotImplemented`** ⇒ 参照报 `unsupported operand type(s) for <op>: 'A' and 'B'` ✓
        //（round 67：`DummyIntegral.__truediv__` 返回 `NotImplemented`、本层先前悄悄退回原生 int 除法 ✗
        // ⇒ `test_numeric_tower.test_fractions` 的 `assertRaises(TypeError)` 红 ✓）。
        let left_name = instance.type_name(instance.type_of(left)).to_owned();
        let right_name = instance.type_name(instance.type_of(right)).to_owned();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("unsupported operand type(s) for {symbol}: '{left_name}' and '{right_name}'"),
        ));
    }

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
            [HotId::Set, HotId::FrozenSet].into_iter().any(|id| {
                instance
                    .hot_type(id)
                    .is_some_and(|base| instance.is_subtype(ty, base))
            })
        };
        if is_set(instance.type_of(left)) && is_set(instance.type_of(right)) {
            return Ok(set_operation(instance, left, right, symbol));
        }
    }
    // **`dict | dict`**（`PEP 584` ✓，task-13）：两边都是 **dict 族**（子类也算 ✓）⇒ **新建** dict，
    // 先左后右、右侧**覆盖**同名键 ✓（参照 3.14.4 实测；语义见
    // [`crate::builtin::dict::dict_merge`] ✓）。动因：`Lib/sysconfig/__init__.py:427` 的
    // `_get_sysconfigdata() | vars` ✓ ⇒ `sysconfig.get_config_var()` 崩 ⇒ 官方 `test.support`
    // 导入链（`P3-32` 框架门槛）的最后一格之一 ✓。
    // 非 dict 的对侧**不在这里接** ✓（照旧落到文件末尾"一处真相"的 `unsupported_operand` ✓）；
    // `dict |= dict` 走 `protocol::inplace_dict_merge` ✓（那条行为不动 ✓）。
    let dict_type = instance.hot_type(HotId::Dict);
    let is_dict = |value: NonNull<Header>| {
        dict_type.is_some_and(|base| instance.is_subtype(instance.type_of(value), base))
    };
    if symbol == "|" && is_dict(left) && is_dict(right) {
        return Ok(crate::builtin::dict::dict_merge(instance, left, right));
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
    // **复数运算** ✓（task-30／P3-34）：任一侧是 `complex` ⇒ 整条走**复数塔** ✓
    //（`+ - * / **`；`int`／`float`／`bool` 混入按参照升到复数 ✓）。`//`／`%` 落进
    // `complex_arithmetic` 的兜底 ⇒ `unsupported_operand` 的既有文案 ✓（实测
    // `unsupported operand type(s) for //: 'complex' and 'int'` ✓）。
    if instance.complex_value(left).is_some() || instance.complex_value(right).is_some() {
        return complex_arithmetic(instance, left, right, symbol, opcode);
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
    let float_type = instance.hot_type(HotId::Float).expect("float 已登记");
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
    // **`bool & bool`／`|`／`^` ⇒ `bool`** ✓（round 47）：参照只有"两边都恰是 `bool`"时结果仍是
    // `bool`（`True & True` ⇒ `True`；`True & 1` ⇒ **int** `1`）✓（`test_bool.test_boolean` 钉住）。
    if matches!(symbol, "&" | "|" | "^")
        && instance.type_of(left) == instance.singletons().bool_type()
        && instance.type_of(right) == instance.singletons().bool_type()
    {
        let left_value = instance.bool_value(left).expect("类型刚确认是 bool");
        let right_value = instance.bool_value(right).expect("类型刚确认是 bool");
        let result = match symbol {
            "&" => left_value && right_value,
            "|" => left_value || right_value,
            _ => left_value != right_value,
        };
        return Ok(instance.retain(instance.singletons().boolean(result)));
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
                // **负指数**：参照给 `float`（`2 ** -1 == 0.5`、`2 ** -53` 同样是 float ✓）⇒ 走浮点 ✓
                //（第 722 轮；`Lib/random.py` 的 `RECIP_BPF = 2**-BPF` 在**导入期**就踩它 ✗）。
                if wide_right.signum() < 0 {
                    let base = wide_left.to_f64();
                    let exponent = wide_right.to_f64();
                    if base == 0.0 {
                        return Err(instance.raise_builtin_error(
                            "ZeroDivisionError",
                            "0.0 cannot be raised to a negative power",
                        ));
                    }
                    return Ok(instance.new_float(base.powf(exponent)));
                }
                let Some(exponent) = b.to_i64().and_then(|value| u32::try_from(value).ok()) else {
                    return Err(ExecError::Unsupported {
                        opcode,
                        what: "整数幂：指数超出 u32，尚未接线",
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

/// **`int.__add__` 的类型级描述符**（task-23 ✓）：`multiprocessing/reduction.py:220` 的
/// `type(int.__add__)` 是**导入期**取用 ✓ ⇒ 类型字典里要有这个名字 ✓（接收者＝第一个实参 ✓）。
///
/// 语义照参照实测（3.14.4）：接收者须是 `int`（**子类可以** ✓，`int.__add__(True, 2)` ⇒ `3` ✓）；
/// **右操作数不是 `int` ⇒ 返 `NotImplemented`** ✓（`int.__add__(1, 'x') is NotImplemented` ✓，
/// 与构造分派 `__add__` 槽同一条口径 ✓）；其余文案：无参 ⇒
/// `descriptor '__add__' of 'int' object needs an argument`、多参 ⇒ `expected 1 argument, got N`、
/// 关键字 ⇒ `wrapper __add__() takes no keyword arguments`。
pub fn int_add_descriptor(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(owner) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__add__' of 'int' object needs an argument",
        ));
    };
    let int_type = instance.hot_type(HotId::Int).expect("int 已登记");
    if !instance.is_subtype(instance.type_of(owner), int_type) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "descriptor '__add__' requires a 'int' object but received a '{}'",
                instance.type_name(instance.type_of(owner))
            ),
        ));
    }
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "wrapper __add__() takes no keyword arguments",
        ));
    }
    let given = args.len() - 1;
    if given != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("expected 1 argument, got {given}"),
        ));
    }
    let right = args[1];
    if !instance.is_subtype(instance.type_of(right), int_type) {
        return Ok(instance.retain(instance.not_implemented()));
    }
    arithmetic_public(instance, owner, right, "+", 0)
}

/// 运算符号 ⇒ （左操作数 dunder，右操作数反射 dunder）。
fn binary_dunder(symbol: &str) -> Option<(&'static str, &'static str)> {
    Some(match symbol {
        "+" => ("__add__", "__radd__"),
        "-" => ("__sub__", "__rsub__"),
        "*" => ("__mul__", "__rmul__"),
        "/" => ("__truediv__", "__rtruediv__"),
        "//" => ("__floordiv__", "__rfloordiv__"),
        "%" => ("__mod__", "__rmod__"),
        "**" => ("__pow__", "__rpow__"),
        "<<" => ("__lshift__", "__rlshift__"),
        ">>" => ("__rshift__", "__rrshift__"),
        "&" => ("__and__", "__rand__"),
        "|" => ("__or__", "__ror__"),
        "^" => ("__xor__", "__rxor__"),
        "@" => ("__matmul__", "__rmatmul__"),
        _ => return None,
    })
}

/// **用户类**的二元运算覆写（`NotImplemented` ⇒ 继续/退回原生 ✓）。
fn binary_override(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<(Option<NonNull<Header>>, bool), ExecError> {
    let Some((dunder, reflected)) = binary_dunder(symbol) else {
        return Ok((None, false));
    };
    let mut had_override = false;
    // **子类优先**（round 217 真 bug 修 ✗）：右侧类型是左侧的严格子类 ⇒ **先问右侧的反射 dunder**
    //（参照 `do_binary_op`／`binary_op1` 的次序 ✓）—— `ChainMap() | SubclassRor()` 里
    // `SubclassRor.__ror__` 必须先命中 ⇒ 结果类型是 `SubclassRor` ✓；先前固定先问左侧 `__or__`
    // ⇒ 结果成了 `ChainMap` ✗（`test_collections.TestChainMap.test_union_operators` ✓）。
    let left_type = instance.type_of(left);
    let right_type = instance.type_of(right);
    // **只在右侧"自己定义了反射 dunder"时先问右侧**（round 217）：参照里
    // `ChainMap() | Subclass()`（子类**没**覆写 `__ror__`）要得到 `ChainMap` ✓；
    // `ChainMap() | SubclassRor()`（子类**自己**写了 `__ror__`）要得到 `SubclassRor` ✓。
    // 判据：`type_lookup_owner(right_type, reflected)` 的 owner **就是 right_type** ✓。
    let right_owns_reflection = instance
        .type_lookup_owner(right_type, reflected)
        .is_some_and(|(owner, _)| owner == right_type);
    let right_first = right_type != left_type
        && instance.is_subtype(right_type, left_type)
        && right_owns_reflection;
    let order: [(&str, bool); 2] = if right_first {
        [(reflected, true), (dunder, false)]
    } else {
        [(dunder, false), (reflected, true)]
    };
    for (name, swapped) in order {
        let (receiver, other) = if swapped { (right, left) } else { (left, right) };
        match call_user_dunder(instance, receiver, other, name, opcode)? {
            // **没有该方法** ⇒ 继续找反射那一侧 ✓。
            None => continue,
            // **方法在、但返回 `NotImplemented`** ⇒ 记下"有覆写"✓（决定后面报不报 TypeError）。
            Some(None) => had_override = true,
            Some(Some(result)) => return Ok((Some(result), true)),
        }
    }
    Ok((None, had_override))
}

/// 若 `receiver` 的类型（沿 MRO）里有一个**用户类**定义的 `name` ⇒ 调用它并把结果交回 ✓；
/// 内建类型自己的实现（`int.__or__` 一类）**不算** —— 那些交给本文件的原生路径 ✓。
fn call_user_dunder(
    instance: &Instance,
    receiver: NonNull<Header>,
    other: NonNull<Header>,
    name: &str,
    opcode: u8,
) -> Result<Option<Option<NonNull<Header>>>, ExecError> {
    let receiver_type = instance.type_of(receiver);
    let Some((owner, _)) = instance.type_lookup_owner(receiver_type, name) else {
        return Ok(None);
    };
    // **用户类**的判据：定义者自己字典里有 `__module__`（内建类型没有挂它 ✓ ——
    // `attribute_lookup` 给内建类型兜底 `'builtins'` 是**读**层面的，不写进类型字典 ✓）。
    if instance.type_lookup(owner, "__module__").is_none() {
        return Ok(None);
    }
    let Some(method) = instance.type_lookup(receiver_type, name) else {
        return Ok(None);
    };
    // SAFETY: other 由调用方保证存活；`call_callable` 转移实参表。
    unsafe { instance.incref_object(other.as_ptr()) };
    let result = crate::executor::call::call_callable(
        instance,
        method,
        Some(receiver),
        vec![other],
        Vec::new(),
        opcode,
    )?;
    if result == instance.not_implemented() {
        unsafe { instance.release_object(result.as_ptr()) };
        return Ok(Some(None));
    }
    Ok(Some(Some(result)))
}

// ===================== **复数塔**（task-30／P3-34） =====================

/// **数值实参升到复数**（`(real, imag)`）：`complex` 原值；`float`／`int`／`bool` ⇒ 虚部 0 ✓。
///
/// 大整数折 `f64` 溢出 ⇒ 参照报 `OverflowError: int too large to convert to float` ✓
/// （与 `/` 那条同一口径 ✓）。
fn complex_operand(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<Option<(f64, f64)>, ExecError> {
    if let Some(parts) = instance.complex_value(value) {
        return Ok(Some(parts));
    }
    if let Some(number) = instance.float_value(value) {
        return Ok(Some((number, 0.0)));
    }
    if let Some(integer) = instance.int_of(value) {
        let number = integer.to_bigint().to_f64();
        if number.is_infinite() {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "int too large to convert to float",
            ));
        }
        return Ok(Some((number, 0.0)));
    }
    Ok(None)
}

/// 复数乘法（**复用给幂的重复平方** ✓）。
fn complex_multiply(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (
        a.0 * b.0 - a.1 * b.1,
        a.0 * b.1 + a.1 * b.0,
    )
}

/// 复数除法：CPython `_Py_c_quot` 的 **Smith 算法** ✓（除零由调用方先拦 ✓）。
fn complex_quotient(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (abs_real, abs_imag) = (b.0.abs(), b.1.abs());
    if abs_real >= abs_imag {
        let ratio = b.1 / b.0;
        let denominator = b.0 + b.1 * ratio;
        (
            (a.0 + a.1 * ratio) / denominator,
            (a.1 - a.0 * ratio) / denominator,
        )
    } else {
        let ratio = b.0 / b.1;
        let denominator = b.0 * ratio + b.1;
        (
            (a.0 * ratio + a.1) / denominator,
            (a.1 * ratio - a.0) / denominator,
        )
    }
}

/// 复数幂：CPython `_Py_c_pow` 的两条路 ✓ —— 指数是**小整数**（`|n| <= 100` 的整数值）时
/// 走**重复平方**（结果与参照逐位一致 ✓，`(2+3j)**100` 那种靠它 ✓）；否则走极坐标 ✓。
fn complex_power(
    instance: &Instance,
    base: (f64, f64),
    exponent: (f64, f64),
) -> Result<NonNull<Header>, ExecError> {
    let (real, imag) = base;
    let (exponent_real, exponent_imag) = exponent;
    // 零底的特殊口径（参照实测）：`0j**0` ⇒ `(1+0j)`；`0j**2` ⇒ `0j`；
    // `0j**-1`／`0j**-1j` ⇒ `ZeroDivisionError: zero to a negative or complex power` ✓。
    if real == 0.0 && imag == 0.0 {
        if exponent_real < 0.0 || exponent_imag != 0.0 {
            return Err(instance.raise_builtin_error(
                "ZeroDivisionError",
                "zero to a negative or complex power",
            ));
        }
        if exponent_real == 0.0 {
            return Ok(instance.new_complex(1.0, 0.0));
        }
        return Ok(instance.new_complex(0.0, 0.0));
    }
    // **小整数指数**：重复平方（C 实现在 `_Py_c_pow` 里同一分支 ✓）。
    if exponent_imag == 0.0
        && exponent_real == (exponent_real as i32) as f64
        && exponent_real >= -100.0
        && exponent_real <= 100.0
    {
        let mut remaining = (exponent_real as i32).unsigned_abs();
        let mut factor = base;
        let mut result = (1.0, 0.0);
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = complex_multiply(result, factor);
            }
            remaining >>= 1;
            if remaining > 0 {
                factor = complex_multiply(factor, factor);
            }
        }
        if exponent_real < 0.0 {
            result = complex_quotient((1.0, 0.0), result);
        }
        return Ok(instance.new_complex(result.0, result.1));
    }
    // **极坐标**（参照 `_Py_c_pow` 的通用分支 ✓）。
    let magnitude = real.hypot(imag);
    let mut length = magnitude.powf(exponent_real);
    let angle = imag.atan2(real);
    let mut phase = angle * exponent_real;
    if exponent_imag != 0.0 {
        length /= (angle * exponent_imag).exp();
        phase += exponent_imag * magnitude.ln();
    }
    Ok(instance.new_complex(length * phase.cos(), length * phase.sin()))
}

/// **复数算术**（`+ - * / **` ✓；其余符号 ⇒ 参照那条 `unsupported operand type(s)` ✓）。
fn complex_arithmetic(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    _opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    let Some(a) = complex_operand(instance, left)? else {
        return Err(unsupported_operand(instance, left, right, symbol));
    };
    let Some(b) = complex_operand(instance, right)? else {
        return Err(unsupported_operand(instance, left, right, symbol));
    };
    match symbol {
        "+" => Ok(instance.new_complex(a.0 + b.0, a.1 + b.1)),
        "-" => Ok(instance.new_complex(a.0 - b.0, a.1 - b.1)),
        "*" => {
            let (real, imag) = complex_multiply(a, b);
            Ok(instance.new_complex(real, imag))
        }
        "/" => {
            if b.0 == 0.0 && b.1 == 0.0 {
                return Err(instance.raise_builtin_error("ZeroDivisionError", "division by zero"));
            }
            let (real, imag) = complex_quotient(a, b);
            Ok(instance.new_complex(real, imag))
        }
        "**" => complex_power(instance, a, b),
        _ => Err(unsupported_operand(instance, left, right, symbol)),
    }
}

// ===================== **数值哈希**（task-30；stdlib 的 `hash()` 内置入口用 ✓） =====================

/// `sys.hash_info.modulus`：`2**61 - 1` ✓（CPython 3.14 口径，`sys` 模块同值 ✓）。
const HASH_MODULUS: u64 = (1u64 << 61) - 1;
/// `sys.hash_info.width`：61 位 ✓。
const HASH_BITS: i32 = 61;
/// `sys.hash_info.inf` ✓。
const HASH_INFINITY: i64 = 314159;
/// `sys.hash_info.nan` ✓。
const HASH_NAN: i64 = 0;
/// `sys.hash_info.imag`：复数哈希的虚部乘数 ✓。
const HASH_IMAG: i64 = 1_000_003;

/// `f64` 的 `frexp`（`v = m * 2^e`，`0.5 <= |m| < 1`）——`_Py_HashDouble` 的起点 ✓。
fn frexp(value: f64) -> (f64, i32) {
    if value == 0.0 || !value.is_finite() {
        return (value, 0);
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7FF) as i32;
    if exponent == 0 {
        // 次正规数：先乘 2^54 归一，再把指数补回去 ✓。
        let (mantissa, extra) = frexp(value * 18_014_398_509_481_984.0);
        return (mantissa, extra - 54);
    }
    let mantissa_bits = (bits & 0x800F_FFFF_FFFF_FFFF) | 0x3FE0_0000_0000_0000;
    (f64::from_bits(mantissa_bits), exponent - 1022)
}

/// **`_Py_HashDouble`**（CPython 3.14 口径）：`hash(<float>)` 的算法 ✓。
///
/// 实测锚点（3.14.4，`sys.hash_info` 同上）：`hash(1.0) == 1`、`hash(0.5) == 1152921504606846976`、
/// `hash(-1.0) == -2`、`hash(1.5) == 1152921504606846977`、`hash(inf) == 314159`、
/// `hash(-inf) == -314159`、`hash(nan) == 0` ✓。
pub fn hash_double_value(value: f64) -> i64 {
    if !value.is_finite() {
        if value.is_infinite() {
            return if value > 0.0 { HASH_INFINITY } else { -HASH_INFINITY };
        }
        return HASH_NAN;
    }
    let (mut mantissa, mut exponent) = frexp(value);
    let sign = if mantissa < 0.0 {
        mantissa = -mantissa;
        -1i64
    } else {
        1i64
    };
    let mut hash: u64 = 0;
    while mantissa != 0.0 {
        hash = (hash.wrapping_shl(28) & HASH_MODULUS) | (hash >> (HASH_BITS as u32 - 28));
        mantissa *= 268_435_456.0; // 2^28
        exponent -= 28;
        let digit = mantissa as u64;
        mantissa -= digit as f64;
        hash = hash.wrapping_add(digit);
        if hash >= HASH_MODULUS {
            hash -= HASH_MODULUS;
        }
    }
    exponent = if exponent >= 0 {
        exponent % HASH_BITS
    } else {
        HASH_BITS - 1 - ((-1 - exponent) % HASH_BITS)
    };
    hash = (hash.wrapping_shl(exponent as u32) & HASH_MODULUS)
        | (hash >> (HASH_BITS as u32 - exponent as u32));
    let mut result = (hash as i64) * sign;
    if result == -1 {
        result = -2;
    }
    result
}

/// **小整数 `hash()`**：CPython 里 `hash(n) == n`（`-1` 例外 ⇒ `-2` ✓）。
pub fn hash_integer_value(value: i64) -> i64 {
    if value == -1 { -2 } else { value }
}

/// **大整数 `hash()`**（CPython `long_hash` ✓）：`|n| mod (2**61-1)`，负数取反，`-1` ⇒ `-2` ✓。
pub fn hash_bigint_value(value: &crate::bigint::BigInt) -> i64 {
    let modulus = crate::bigint::BigInt::from_i64((1i64 << 61) - 1);
    let Some((_, remainder)) = value.divmod_floor(&modulus) else {
        return HASH_NAN;
    };
    let mut folded = remainder.to_i64().unwrap_or(0);
    if folded == (1i64 << 61) - 1 {
        folded = 0;
    }
    if value.signum() < 0 && folded != 0 {
        folded = -folded;
    }
    if folded == -1 {
        folded = -2;
    }
    folded
}

/// **复数的 `hash()`**：`hash(real) + 1000003 * hash(imag)` ✓（CPython `complex_hash` ✓；
/// 实测 `hash(1+0j) == hash(1) == 1`、`hash(complex(0,0)) == 0` ✓）。
pub fn hash_complex_parts(real: f64, imag: f64) -> i64 {
    let real_hash = hash_double_value(real);
    let imag_hash = hash_double_value(imag);
    let combined = real_hash.wrapping_add(HASH_IMAG.wrapping_mul(imag_hash));
    if combined == -1 { -2 } else { combined }
}
