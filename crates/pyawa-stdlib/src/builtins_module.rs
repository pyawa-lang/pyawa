//! `builtins` 模块的**纯计算面**（`PLAN` §9.4 第 4 条）。
//!
//! 契约（`CM-4` 的"从 Python 看到的 API 与语义"）写在 `docs/SPEC-c-modules.md` §5.2.2。
//! 只做**不需要能力域、也不需要输出通道**的那些内建：需要平台（`print` 之类）的留给
//! `P3-14`／待裁口径。
//!
//! 本模块**只用安全函数**定义原生：核心的 `NativeFn` 是 `unsafe fn` 指针，而**安全 `fn`
//! 可以强转成它**（同一 ABI）⇒ stdlib 在 `#![forbid(unsafe_code)]` 下也能落地原生函数。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};

/// 模块名（`builtins`）。
pub const NAME: &str = "builtins";

/// 本模块落地的内建函数名（按名字排序；测试与合约核对用）。
pub const IMPLEMENTED: &[&str] = &[
    "abs", "bin", "callable", "chr", "hex", "isinstance", "issubclass", "len", "oct", "ord",
    "repr",
];

/// 建 `builtins` 模块的命名空间（**新引用** 的 `dict`）。
///
/// 内容：上表那些内建函数 ＋ **`__build_class__`**（由核心在引导期建好，`OM-14`）＋
/// `__name__`。**没有** `print`（要输出通道，口径待裁）、也没有需要能力域的那些。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let natives: &[(&str, pyawa_core::NativeFn)] = &[
        ("abs", abs_native as pyawa_core::NativeFn),
        ("bin", bin_native as pyawa_core::NativeFn),
        ("callable", callable_native as pyawa_core::NativeFn),
        ("chr", chr_native as pyawa_core::NativeFn),
        ("hex", hex_native as pyawa_core::NativeFn),
        ("isinstance", isinstance_native as pyawa_core::NativeFn),
        ("issubclass", issubclass_native as pyawa_core::NativeFn),
        ("len", len_native as pyawa_core::NativeFn),
        ("oct", oct_native as pyawa_core::NativeFn),
        ("ord", ord_native as pyawa_core::NativeFn),
        ("repr", repr_native as pyawa_core::NativeFn),
    ];
    for (name, handler) in natives {
        let function = make_native(instance, name, *handler);
        instance.dict_set(namespace, name, function);
    }
    // `__build_class__`：核心在引导期已经建好（`OM-14`），这里原样放进 `builtins`
    if let Some(build_class) = instance.build_class() {
        instance.dict_set(namespace, "__build_class__", build_class);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}

/// 造一个原生可调用对象（**新引用**）。
fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记（OM-13）");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        // `TypeObject::name` 要 `&'static str`：内建函数名是常量，泄漏一份即可
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 参数个数检查：不够就给统一的 `TypeError`（消息照参照实现的口径写）。
fn need_args(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    count: usize,
) -> Result<(), ExecError> {
    if args.len() < count {
        let message = format!(
            "{name}() takes at least {count} argument{} ({} given)",
            if count == 1 { "" } else { "s" },
            args.len()
        );
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    Ok(())
}

/// 类型名（诊断消息里用）。
fn type_name(instance: &Instance, object: NonNull<Header>) -> String {
    instance.type_name(instance.type_of(object))
}

/// `abs(x)`：整数给整数、浮点给浮点；别的 `TypeError`。
///
/// 实测：`abs(True)` ⇒ `1`（**int**，不是 bool）；`abs("x")` ⇒
/// `bad operand type for abs(): 'str'`。
fn abs_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "abs", args, 1)?;
    let value = args[0];
    if let Some(integer) = instance.int_value(value) {
        return Ok(instance.new_int(integer.abs()));
    }
    if let Some(number) = instance.float_value(value) {
        return Ok(instance.new_float(number.abs()));
    }
    let message = format!("bad operand type for abs(): '{}'", type_name(instance, value));
    Err(instance.raise_builtin_error("TypeError", &message))
}

/// `len(x)`：`str`／`list`／`tuple`／`dict`／`set`；别的 `TypeError`。
///
/// 实测：`len(5)` ⇒ `object of type 'int' has no len()`。
fn len_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "len", args, 1)?;
    let value = args[0];
    match instance.length_of(value) {
        Some(length) => Ok(instance.new_int(length as i64)),
        None => {
            let message = format!(
                "object of type '{}' has no len()",
                type_name(instance, value)
            );
            Err(instance.raise_builtin_error("TypeError", &message))
        }
    }
}

/// `ord(s)`：单字符字符串 → 码点。
///
/// 实测：`ord("ab")` ⇒ `ord() expected a character, but string of length 2 found`。
fn ord_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "ord", args, 1)?;
    let value = args[0];
    let text = match instance.text_value(value) {
        Some(text) => text,
        None => {
            let message = format!(
                "ord() expected string of length 1, but {} found",
                type_name(instance, value)
            );
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    // 长度按**码点**（`TS-22`）；本层 `str` 存 UTF-8 ⇒ 先按字符取
    let mut characters = text.chars();
    let first = characters.next();
    if first.is_none() || characters.next().is_some() {
        // 实测消息里的长度也是**字符数**
        let message = format!(
            "ord() expected a character, but string of length {} found",
            text.chars().count()
        );
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    Ok(instance.new_int(i64::from(first.expect("上面确认过有字符") as u32)))
}

/// `chr(i)`：码点 → 单字符字符串。
///
/// 实测：`chr(-1)`／`chr(0x110000)` ⇒ `chr() arg not in range(0x110000)`；
/// `chr(1.0)` ⇒ `'float' object cannot be interpreted as an integer`。
fn chr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "chr", args, 1)?;
    let value = args[0];
    let code = match instance.int_value(value) {
        Some(code) => code,
        None => {
            let message = format!(
                "'{}' object cannot be interpreted as an integer",
                type_name(instance, value)
            );
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    let character = match u32::try_from(code).ok().and_then(char::from_u32) {
        Some(character) => character,
        None => {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "chr() arg not in range(0x110000)",
            ))
        }
    };
    Ok(instance.new_str(&character.to_string()))
}

/// `bin`／`oct`／`hex` 的共同实现（`base` 是 2／8／16，前缀照参照实现）。
fn radix_native(
    instance: &Instance,
    name: &str,
    base: u32,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, name, args, 1)?;
    let value = args[0];
    let integer = match instance.int_value(value) {
        Some(integer) => integer,
        None => {
            let message = format!(
                "'{}' object cannot be interpreted as an integer",
                type_name(instance, value)
            );
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    let sign = if integer < 0 { "-" } else { "" };
    let magnitude = integer.unsigned_abs();
    let digits = match base {
        2 => format!("{magnitude:b}"),
        8 => format!("{magnitude:o}"),
        _ => format!("{magnitude:x}"),
    };
    let prefix = match base {
        2 => "0b",
        8 => "0o",
        _ => "0x",
    };
    Ok(instance.new_str(&format!("{sign}{prefix}{digits}")))
}

/// `bin(x)`：`bin(-5)` ⇒ `'-0b101'`；`bin(True)` ⇒ `'0b1'`。
fn bin_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "bin", 2, args)
}

/// `oct(x)`：`oct(8)` ⇒ `'0o10'`。
fn oct_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "oct", 8, args)
}

/// `hex(x)`：`hex(1.5)` ⇒ `'float' object cannot be interpreted as an integer`。
fn hex_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "hex", 16, args)
}

/// `callable(x)`：**有 `call` 槽就返回 `True`**（`OM-11`），否则 `False`。
fn callable_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "callable", args, 1)?;
    let value = args[0];
    Ok(instance.new_bool(instance.is_callable(value)))
}

/// `isinstance(x, T)`：`T` 可以是类型，也可以是类型的元组。
///
/// 实测：`isinstance(1, 5)` ⇒
/// `isinstance() arg 2 must be a type, a tuple of types, or a union`。
fn isinstance_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "isinstance", args, 2)?;
    let subject = args[0];
    let matches = type_matches(instance, subject, args[1], "isinstance")?;
    Ok(instance.new_bool(matches))
}

/// `issubclass(T, U)`：`bool` 是 `int` 的子类，反向不是。
///
/// 实测：`issubclass(1, int)` ⇒ `issubclass() arg 1 must be a class`。
fn issubclass_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "issubclass", args, 2)?;
    let subject = args[0];
    // 注意：`issubclass(T, U)` 的**主题类型是 `T` 自己**（不是 `type_of(T)`，那是 `type`）
    let subject_type = match instance.as_type(subject) {
        Some(ty) => ty,
        None => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "issubclass() arg 1 must be a class",
            ))
        }
    };
    let matches = type_matches_against(instance, subject_type, args[1], "issubclass")?;
    Ok(instance.new_bool(matches))
}

/// `isinstance` 的判定：`T` 是类型 ⇒ 直接比；是元组 ⇒ 逐个比。
fn type_matches(
    instance: &Instance,
    subject: NonNull<Header>,
    expected: NonNull<Header>,
    caller: &str,
) -> Result<bool, ExecError> {
    let subject_type = instance.type_of(subject);
    type_matches_against(instance, subject_type, expected, caller)
}

/// 判定"某个类型是不是 `expected` 描述的类型（或之一）"。
fn type_matches_against(
    instance: &Instance,
    subject_type: NonNull<pyawa_core::TypeObject>,
    expected: NonNull<Header>,
    caller: &str,
) -> Result<bool, ExecError> {
    if let Some(target) = instance.as_type(expected) {
        return Ok(instance.is_subtype(subject_type, target));
    }
    if let Some(items) = instance.tuple_items(expected) {
        for item in items {
            if type_matches_against(instance, subject_type, item, caller)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let message = format!("{caller}() arg 2 must be a type, a tuple of types, or a union");
    Err(instance.raise_builtin_error("TypeError", &message))
}

/// `repr(x)`：走 `OM-11` 的 `repr` 槽（`TS-44` 的口径）。
fn repr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "repr", args, 1)?;
    let text = instance.object_repr(args[0]);
    Ok(instance.new_str(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_module_lists_what_it_implements() {
        let instance = Instance::new();
        let namespace = build(&instance);
        for name in IMPLEMENTED {
            assert!(
                instance.dict_get(namespace, name).is_some(),
                "{name} 应当在 builtins 命名空间里"
            );
        }
        // `__build_class__` 来自核心（OM-14）
        assert!(instance.dict_get(namespace, "__build_class__").is_some());
        // 需要输出通道的 `print` 不在这里（口径待裁，见 §5.2.2）
        assert!(instance.dict_get(namespace, "print").is_none());
    }
}
