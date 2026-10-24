//! `unicodedata` 模块（round 19，task-40）：**最小诚实面**。
//!
//! 数据**全部复用**同目录的 [`crate::unicode_tables`]（`CM-13`／`CM-22`／`CM-24`：由
//! `tools/gen_unicode.py` 从参照实现导出，**禁止手写／禁止换 Unicode crate**）——本模块只把
//! 「数据 → Python 面」接起来，**不加任何新数据表**。
//!
//! ## 真实实现（表里有数据，语义照参照 3.14.4 逐项实测）
//! - `unidata_version`：`16.0.0`（`unicode_tables::UNIDATA_VERSION`，`CM-13` 要求与参照同串）。
//! - `category(chr)`：两字母通用类别（缺省 `Cn`；代理区照表给 `Cs`）。
//! - `combining(chr)`：组合类（缺省 `0`）。
//! - `east_asian_width(chr)`：东亚宽度（缺省 `N`）。
//! - `bidirectional(chr)`：双向类别（缺省空串）。
//! - `decimal(chr)`／`digit(chr)`：十进制/数字值（无值 ⇒ 参照的 `ValueError: not a decimal`／
//!   `not a digit` 文案）。
//! - `numeric(chr)`：数值，参照**恒返回 float**（`numeric('2')` ⇒ `2.0`）；无值 ⇒
//!   `ValueError: not a numeric character`。
//! - `decomposition(chr)`：分解串（原样保留 `<tag>`；无分解 ⇒ 空串）。
//!
//! ## 如实未实现（`NotImplementedError`，不编假数据）
//! `name`／`lookup`（没有 Unicode 名字表）、`normalize`／`is_normalized`（没有规范化所需的
//! 组合表；现有 `decomposition` 只够"分解"，不够"组合"）、`mirrored`（没有镜像表）。
//!
//! 参照的 `ucd_3_2_0`／`UCD`／`_ucnhash_CAPI` 也未接（属性缺失 ⇒ `AttributeError`；都不是
//! import 期或常用面，如实不冒充）。
//!
//! 动因：`_pyrepl/utils.py`（`pdb`／`doctest` 的依赖链）在导入期/类体里用
//! `unicodedata.category`／`combining`／`east_asian_width`。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

use crate::unicode_tables;

/// 模块名（`unicodedata`）。
pub const NAME: &str = "unicodedata";

/// 模块 `__doc__`（如实：最小诚实面）。
pub const DOC: &str = "Minimal honest Unicode database interface: category/combining/east_asian_width/\
bidirectional/decimal/digit/numeric/decomposition are backed by the generated tables; \
name/lookup/normalize/is_normalized/mirrored are not implemented.";

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
fn make_native(instance: &Instance, name: &'static str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        name,
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 把实参取成**单个字符的码点**（照参照 3.14.4 的实参口径，逐条实测）。
///
/// - 位置实参必须**恰好 1 个**（否则 `unicodedata.<名>() takes exactly one argument`）；
/// - 关键字一律拒（`unicodedata.<名>() takes no keyword arguments`）；
/// - 必须是 **str 且长度（码点）为 1**；长度不是 1 ⇒ 参照的
///   `<名>(): argument must be a unicode character, not a string of length N`；
/// - 非 str ⇒ `<名>() argument must be a unicode character, not <类型名>`（`None` 照参照特判成
///   `None`，不是 `NoneType`）。
fn character_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    function: &str,
) -> Result<u32, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("unicodedata.{function}() takes no keyword arguments"),
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "unicodedata.{function}() takes exactly one argument ({} given)",
                args.len()
            ),
        ));
    }
    let Some(text) = instance.text_of(args[0]) else {
        let label = if args[0] == instance.singletons().none() {
            "None".to_owned()
        } else {
            instance.type_name(instance.type_of(args[0]))
        };
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{function}() argument must be a unicode character, not {label}"),
        ));
    };
    let code_points: Vec<char> = text.chars().collect();
    if code_points.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "{function}(): argument must be a unicode character, not a string of length {}",
                code_points.len()
            ),
        ));
    }
    Ok(code_points[0] as u32)
}

/// `unicodedata.category(chr)` ⇒ 两字母类别（缺省 `Cn`）。
fn category_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code_point = character_argument(instance, args, kwargs, "category")?;
    let category = unicode_tables::general_category(code_point).unwrap_or("Cn");
    Ok(instance.new_str(category))
}

/// `unicodedata.combining(chr)` ⇒ 组合类（缺省 `0`）。
fn combining_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code_point = character_argument(instance, args, kwargs, "combining")?;
    let combining = unicode_tables::combining(code_point).unwrap_or(0);
    Ok(instance.new_int(i64::from(combining)))
}

/// `unicodedata.east_asian_width(chr)` ⇒ 东亚宽度（缺省 `N`）。
fn east_asian_width_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code_point = character_argument(instance, args, kwargs, "east_asian_width")?;
    let width = unicode_tables::east_asian_width(code_point).unwrap_or("N");
    Ok(instance.new_str(width))
}

/// `unicodedata.bidirectional(chr)` ⇒ 双向类别（缺省空串）。
fn bidirectional_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code_point = character_argument(instance, args, kwargs, "bidirectional")?;
    let direction = unicode_tables::bidirectional(code_point).unwrap_or("");
    Ok(instance.new_str(direction))
}

/// `unicodedata.decimal(chr)` ⇒ 十进制值；无值 ⇒ `ValueError: not a decimal`。
fn decimal_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code_point = character_argument(instance, args, kwargs, "decimal")?;
    match unicode_tables::decimal(code_point) {
        Some(value) => Ok(instance.new_int(i64::from(value))),
        None => Err(instance.raise_builtin_error("ValueError", "not a decimal")),
    }
}

/// `unicodedata.digit(chr)` ⇒ 数字值；无值 ⇒ `ValueError: not a digit`。
fn digit_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code_point = character_argument(instance, args, kwargs, "digit")?;
    match unicode_tables::digit(code_point) {
        Some(value) => Ok(instance.new_int(i64::from(value))),
        None => Err(instance.raise_builtin_error("ValueError", "not a digit")),
    }
}

/// `unicodedata.numeric(chr)` ⇒ 数值（参照**恒返回 float**）；无值 ⇒
/// `ValueError: not a numeric character`。
fn numeric_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code_point = character_argument(instance, args, kwargs, "numeric")?;
    match unicode_tables::numeric(code_point) {
        Some(value) => Ok(instance.new_float(value)),
        None => Err(instance.raise_builtin_error("ValueError", "not a numeric character")),
    }
}

/// `unicodedata.decomposition(chr)` ⇒ 分解串（原样保留 `<tag>`；无分解 ⇒ 空串）。
fn decomposition_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code_point = character_argument(instance, args, kwargs, "decomposition")?;
    let decomposition = unicode_tables::decomposition(code_point).unwrap_or("");
    Ok(instance.new_str(decomposition))
}

/// 生成"如实未实现"的入口：真调用 ⇒ `NotImplementedError`（带名字与原因，不假返回）。
macro_rules! unimplemented_native {
    ($function:ident => $name:literal, $reason:literal $(,)?) => {
        fn $function(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            _args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            Err(instance.raise_builtin_error(
                "NotImplementedError",
                concat!(
                    "`unicodedata.",
                    $name,
                    "`：",
                    $reason,
                    "（P3-37 挂账；数据面随后补，不编假数据）"
                ),
            ))
        }
    };
}

unimplemented_native!(name_native => "name", "本层没有 Unicode 名字表");
unimplemented_native!(lookup_native => "lookup", "本层没有 Unicode 名字表");
unimplemented_native!(
    normalize_native => "normalize",
    "本层没有规范化所需的数据（组合表；现有表只有分解）"
);
unimplemented_native!(
    is_normalized_native => "is_normalized",
    "本层没有规范化所需的数据（组合表；现有表只有分解）"
);
unimplemented_native!(mirrored_native => "mirrored", "本层没有镜像（mirrored）表");

/// 建 `unicodedata` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, handler) in [
        ("category", category_native as NativeFn),
        ("combining", combining_native as NativeFn),
        ("east_asian_width", east_asian_width_native as NativeFn),
        ("bidirectional", bidirectional_native as NativeFn),
        ("decimal", decimal_native as NativeFn),
        ("digit", digit_native as NativeFn),
        ("numeric", numeric_native as NativeFn),
        ("decomposition", decomposition_native as NativeFn),
        // 如实未实现（见各入口与模块头）
        ("name", name_native as NativeFn),
        ("lookup", lookup_native as NativeFn),
        ("normalize", normalize_native as NativeFn),
        ("is_normalized", is_normalized_native as NativeFn),
        ("mirrored", mirrored_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    // `unidata_version`：与 `unicode_tables.rs` 的导出同源（`CM-13`）。
    let version = instance.new_str(unicode_tables::UNIDATA_VERSION);
    instance.dict_set(namespace, "unidata_version", version);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
