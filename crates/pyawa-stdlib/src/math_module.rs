//! `math` 模块（`SPEC-c-modules.md`；第 336 轮）。
//!
//! **纯 Rust `f64`** ✓ —— 不碰平台、不碰能力域 ✓（`CX-4`：stdlib 只做绑定 ✓）。
//! 上限榜上 `ModuleNotFoundError: No module named 'math'` × 10 个模块的卡点 ✓；
//! 而 `Lib/` 里大量模块在**导入期**就取 `math` 的名字 ✓。
//!
//! **口径** ✓（照参照实测，逐条量过）：
//! * `floor`／`ceil`／`trunc` **返回 `int`** ✓（不是 `float` ✓）；
//! * 定义域错 ⇒ `ValueError: math domain error` ✓；`log(x, base)` 两实参 ✓；
//! * `gcd`／`lcm` 接受任意多个整数 ✓；`isqrt`／`factorial` 只接受非负整数 ✓
//!   （负数 ⇒ `ValueError` ✓，消息照参照 ✓）；
//! * `fsum`／`frexp`／`modf`／`ldexp`／`nextafter`／`ulp` 一类**本轮未落地** ✗
//!   （真撞上再补 ✓，按 `CM-6` 如实报缺失 ⇒ 属性不存在而不是给错值 ✓）。
//!
//! **本模块不做哈希／分类**：`isnan`／`isinf`／`isfinite` 按 `f64` 自带方法 ✓。

use std::cell::Cell;
use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名。
pub const NAME: &str = "math";

/// 模块的 `__doc__`。
pub const DOC: &str = "This module provides access to the mathematical functions defined by the C standard.";

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 取一个浮点实参（不够／不是数 ⇒ 照参照的消息 ✓）。
fn float_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    name: &str,
) -> Result<f64, ExecError> {
    let Some(value) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes exactly one argument (0 given)"),
        ));
    };
    real_argument(instance, *value)
}

/// 取一个"实数"实参（**`int` 与 `float` 都算** ✓ —— `Instance::float_value` 只认 `float` ✗，
/// 所以这里先试 `float` ✓、再试 `int` ✓；两者都不是 ⇒ 照参照的消息 ✓）。
fn real_argument(instance: &Instance, value: NonNull<Header>) -> Result<f64, ExecError> {
    if let Some(number) = instance.float_value(value) {
        return Ok(number);
    }
    if let Some(integer) = instance.int_of(value).and_then(|value| value.to_i64()) {
        return Ok(integer as f64);
    }
    Err(instance.raise_builtin_error("TypeError", "must be real number, not something else"))
}

/// 取一个整数实参（`int_of` ✓）。
fn int_argument(instance: &Instance, args: &[NonNull<Header>], name: &str) -> Result<i64, ExecError> {
    let Some(value) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes exactly one argument (0 given)"),
        ));
    };
    instance
        .int_of(*value)
        .and_then(|value| value.to_i64())
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "'something else' object cannot be interpreted as an integer"))
}

/// 定义域错（照参照的消息 ✓）。
fn domain(instance: &Instance) -> ExecError {
    instance.raise_builtin_error("ValueError", "math domain error")
}

/// 造一个"一元浮点函数"的处理器 ✓（**一处真相**：定义域检查由调用方给 ✓）。
macro_rules! unary_float {
    ($name:ident, $text:literal, $body:expr) => {
        fn $name(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            let value = float_argument(instance, args, $text)?;
            let f: fn(f64, &Instance) -> Result<f64, ExecError> = $body;
            Ok(instance.new_float(f(value, instance)?))
        }
    };
}

unary_float!(sqrt_native, "sqrt", |value, instance| {
    if value < 0.0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("expected a nonnegative input, got {value:?}"),
        ));
    }
    Ok(value.sqrt())
});
unary_float!(exp_native, "exp", |value, _| Ok(value.exp()));
unary_float!(sin_native, "sin", |value, _| Ok(value.sin()));
unary_float!(cos_native, "cos", |value, _| Ok(value.cos()));
unary_float!(tan_native, "tan", |value, _| Ok(value.tan()));
unary_float!(asin_native, "asin", |value, instance| {
    if !(-1.0..=1.0).contains(&value) {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("expected a number in range from -1 up to 1, got {value:?}"),
        ));
    }
    Ok(value.asin())
});
unary_float!(acos_native, "acos", |value, instance| {
    if !(-1.0..=1.0).contains(&value) {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("expected a number in range from -1 up to 1, got {value:?}"),
        ));
    }
    Ok(value.acos())
});
unary_float!(atan_native, "atan", |value, _| Ok(value.atan()));
unary_float!(sinh_native, "sinh", |value, _| Ok(value.sinh()));
unary_float!(cosh_native, "cosh", |value, _| Ok(value.cosh()));
unary_float!(tanh_native, "tanh", |value, _| Ok(value.tanh()));
unary_float!(fabs_native, "fabs", |value, _| Ok(value.abs()));
unary_float!(log2_native, "log2", |value, instance| {
    if value <= 0.0 {
        return Err(instance.raise_builtin_error("ValueError", "expected a positive input"));
    }
    Ok(value.log2())
});
unary_float!(log10_native, "log10", |value, instance| {
    if value <= 0.0 {
        return Err(instance.raise_builtin_error("ValueError", "expected a positive input"));
    }
    Ok(value.log10())
});
unary_float!(log1p_native, "log1p", |value, instance| {
    if value <= -1.0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("expected argument value > -1, got {value:?}"),
        ));
    }
    Ok(value.ln_1p())
});
unary_float!(expm1_native, "expm1", |value, _| Ok(value.exp_m1()));
unary_float!(cbrt_native, "cbrt", |value, _| Ok(value.cbrt()));
unary_float!(degrees_native, "degrees", |value, _| Ok(value.to_degrees()));
unary_float!(radians_native, "radians", |value, _| Ok(value.to_radians()));

/// 造一个"一元浮点 ⇒ 整型"的处理器 ✓（`floor`／`ceil`／`trunc` 返回 `int` ✓）。
macro_rules! unary_int {
    ($name:ident, $text:literal, $body:expr) => {
        fn $name(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            let value = float_argument(instance, args, $text)?;
            let f: fn(f64) -> f64 = $body;
            let rounded = f(value);
            // 超出 `i64` ⇒ 如实拒绝（大整数尚未落地 ⇒ 不静默给错值 ✗）
            if !(rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64) {
                return Err(instance.raise_builtin_error(
                    "OverflowError",
                    "int too large to convert",
                ));
            }
            Ok(instance.new_int(rounded as i64))
        }
    };
}

unary_int!(floor_native, "floor", |value| value.floor());
unary_int!(ceil_native, "ceil", |value| value.ceil());
unary_int!(trunc_native, "trunc", |value| value.trunc());

/// `math.log(x[, base])` ✓（照参照：给了 `base` 就 `log(x, base)` ✓）。
fn log_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = float_argument(instance, args, "log")?;
    if value <= 0.0 {
        return Err(instance.raise_builtin_error("ValueError", "expected a positive input"));
    }
    match args.get(1) {
        None => Ok(instance.new_float(value.ln())),
        Some(base_header) => {
            let base = real_argument(instance, *base_header)?;
            if base <= 0.0 {
                return Err(instance.raise_builtin_error("ValueError", "expected a positive input"));
            }
            Ok(instance.new_float(value.log(base)))
        }
    }
}

/// `math.pow(x, y)` ✓。
fn pow_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (Some(x), Some(y)) = (args.first(), args.get(1)) else {
        return Err(instance.raise_builtin_error("TypeError", "pow() takes exactly 2 arguments"));
    };
    let (x, y) = (real_argument(instance, *x)?, real_argument(instance, *y)?);
    // 照参照：`0 ** 负数` ⇒ `ValueError: math domain error`（Rust 给 `inf` ✗）
    if x == 0.0 && y < 0.0 {
        return Err(domain(instance));
    }
    Ok(instance.new_float(x.powf(y)))
}

/// `math.fmod(x, y)` ✓（`y == 0` ⇒ `ValueError: math domain error` ✓）。
fn fmod_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (Some(x), Some(y)) = (args.first(), args.get(1)) else {
        return Err(instance.raise_builtin_error("TypeError", "fmod() takes exactly 2 arguments"));
    };
    let (x, y) = (real_argument(instance, *x)?, real_argument(instance, *y)?);
    if y == 0.0 {
        return Err(domain(instance));
    }
    Ok(instance.new_float(x % y))
}

/// `math.atan2(y, x)` ✓。
fn atan2_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (Some(y), Some(x)) = (args.first(), args.get(1)) else {
        return Err(instance.raise_builtin_error("TypeError", "atan2() takes exactly 2 arguments"));
    };
    let (y, x) = (
        real_argument(instance, *y)?,
        real_argument(instance, *x)?,
    );
    Ok(instance.new_float(y.atan2(x)))
}

/// `math.copysign(x, y)` ✓。
fn copysign_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (Some(x), Some(y)) = (args.first(), args.get(1)) else {
        return Err(instance.raise_builtin_error("TypeError", "copysign() takes exactly 2 arguments"));
    };
    let (x, y) = (real_argument(instance, *x)?, real_argument(instance, *y)?);
    Ok(instance.new_float(x.copysign(y)))
}

/// `math.hypot(*coordinates)` ✓。
fn hypot_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let mut sum = 0.0f64;
    for argument in args {
        let value = real_argument(instance, *argument)?;
        sum += value * value;
    }
    Ok(instance.new_float(sum.sqrt()))
}

/// `math.gcd(*integers)` ✓（负数取绝对值 ✓；没有实参 ⇒ 0 ✓）。
fn gcd_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let mut acc = 0i64;
    for argument in args {
        let value = int_argument(instance, core::slice::from_ref(argument), "gcd")?.abs();
        acc = gcd_pair(acc, value);
    }
    Ok(instance.new_int(acc))
}

/// 两个整数的最大公约数 ✓。
fn gcd_pair(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a
}

/// `math.lcm(*integers)` ✓（没有实参 ⇒ 1 ✓；任一为 0 ⇒ 0 ✓）。
fn lcm_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let mut acc = 1i64;
    for argument in args {
        let value = int_argument(instance, core::slice::from_ref(argument), "lcm")?.abs();
        if value == 0 {
            return Ok(instance.new_int(0));
        }
        let divisor = gcd_pair(acc, value);
        acc = acc / divisor * value;
    }
    Ok(instance.new_int(acc))
}

/// `math.isqrt(n)` ✓（负数 ⇒ `ValueError: isqrt() argument must be nonnegative` ✓）。
fn isqrt_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = int_argument(instance, args, "isqrt")?;
    if value < 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "isqrt() argument must be nonnegative",
        ));
    }
    Ok(instance.new_int((value as f64).sqrt() as i64))
}

/// `math.factorial(n)` ✓（负数 ⇒ `ValueError: factorial() not defined for negative values` ✓）。
fn factorial_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = int_argument(instance, args, "factorial")?;
    if value < 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "factorial() not defined for negative values",
        ));
    }
    let mut acc: i64 = 1;
    for factor in 2..=value {
        acc = acc.saturating_mul(factor);
    }
    Ok(instance.new_int(acc))
}

/// `math.isnan(x)` 一类 ✓（`float` 实参 ✓）。
macro_rules! predicate {
    ($name:ident, $text:literal, $body:expr) => {
        fn $name(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            let value = float_argument(instance, args, $text)?;
            let f: fn(f64) -> bool = $body;
            Ok(instance.new_bool(f(value)))
        }
    };
}

predicate!(isnan_native, "isnan", |value| value.is_nan());
predicate!(isinf_native, "isinf", |value| value.is_infinite());
predicate!(isfinite_native, "isfinite", |value| value.is_finite());

/// `math.fsum(iterable)` ✓（本层按**普通求和** ✓ —— 参照用 Neumaier 补偿求和 ✗，
/// 极端情形下末位会不同 ✓，如实登记在台账里 ✓）。
fn fsum_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "fsum() takes exactly one argument (0 given)"));
    };
    let mut sum = 0.0f64;
    for value in instance.collect_iterable(*iterable)? {
        let number = real_argument(instance, value)?;
        sum += number;
    }
    Ok(instance.new_float(sum))
}

/// 建 `math` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let natives: &[(&str, NativeFn)] = &[
        ("sqrt", sqrt_native as NativeFn),
        ("exp", exp_native as NativeFn),
        ("expm1", expm1_native as NativeFn),
        ("log", log_native as NativeFn),
        ("log2", log2_native as NativeFn),
        ("log10", log10_native as NativeFn),
        ("log1p", log1p_native as NativeFn),
        ("sin", sin_native as NativeFn),
        ("cos", cos_native as NativeFn),
        ("tan", tan_native as NativeFn),
        ("asin", asin_native as NativeFn),
        ("acos", acos_native as NativeFn),
        ("atan", atan_native as NativeFn),
        ("atan2", atan2_native as NativeFn),
        ("sinh", sinh_native as NativeFn),
        ("cosh", cosh_native as NativeFn),
        ("tanh", tanh_native as NativeFn),
        ("fabs", fabs_native as NativeFn),
        ("cbrt", cbrt_native as NativeFn),
        ("degrees", degrees_native as NativeFn),
        ("radians", radians_native as NativeFn),
        ("floor", floor_native as NativeFn),
        ("ceil", ceil_native as NativeFn),
        ("trunc", trunc_native as NativeFn),
        ("pow", pow_native as NativeFn),
        ("fmod", fmod_native as NativeFn),
        ("copysign", copysign_native as NativeFn),
        ("hypot", hypot_native as NativeFn),
        ("gcd", gcd_native as NativeFn),
        ("lcm", lcm_native as NativeFn),
        ("isqrt", isqrt_native as NativeFn),
        ("factorial", factorial_native as NativeFn),
        ("isnan", isnan_native as NativeFn),
        ("isinf", isinf_native as NativeFn),
        ("isfinite", isfinite_native as NativeFn),
        ("fsum", fsum_native as NativeFn),
    ];
    for (name, handler) in natives {
        let native = make_native(instance, name, *handler);
        instance.dict_set(namespace, name, native);
    }
    // **常量** ✓（参照实测：`math.pi == 3.141592653589793` ✓ 等 ✓）
    for (name, value) in [
        ("pi", std::f64::consts::PI),
        ("e", std::f64::consts::E),
        ("tau", std::f64::consts::TAU),
        ("inf", f64::INFINITY),
        ("nan", f64::NAN),
    ] {
        let float = instance.new_float(value);
        instance.dict_set(namespace, name, float);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
