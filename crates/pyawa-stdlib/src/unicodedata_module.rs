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
/// 字面量 `<tag>` 前缀（兼容分解的标记 ✓）。
fn is_compat_decomposition(text: &str) -> bool {
    text.starts_with('<')
}

/// **一级分解**（`compat=true` 也收带 `<tag>` 的兼容分解 ✓）；`None` ＝ 无分解 ✓。
fn raw_decomposition(code_point: u32, compat: bool) -> Option<Vec<u32>> {
    let text = unicode_tables::decomposition(code_point)?;
    if text.is_empty() {
        return None;
    }
    if is_compat_decomposition(text) {
        if !compat {
            return None;
        }
        // 跳过 `<tag>` 那一段（空格分隔 ✓）。
        let rest = text.split_once(' ').map(|(_, rest)| rest).unwrap_or("");
        return Some(parse_hex_sequence(rest));
    }
    Some(parse_hex_sequence(text))
}

fn parse_hex_sequence(text: &str) -> Vec<u32> {
    text.split_whitespace()
        .filter_map(|token| u32::from_str_radix(token, 16).ok())
        .collect()
}

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

/// **递归分解**（Hangul 按算法 ✓；其余查表递归 ✓；深度守卫防坏数据）。
fn decompose_into(code_point: u32, compat: bool, out: &mut Vec<u32>, depth: usize) {
    if depth > 32 {
        out.push(code_point);
        return;
    }
    // Hangul 音节（算法分解 ✓）。
    if (S_BASE..S_BASE + S_COUNT).contains(&code_point) {
        let index = code_point - S_BASE;
        let lead = L_BASE + index / N_COUNT;
        let vowel = V_BASE + (index % N_COUNT) / T_COUNT;
        let trail_index = index % T_COUNT;
        out.push(lead);
        out.push(vowel);
        if trail_index != 0 {
            out.push(T_BASE + trail_index);
        }
        return;
    }
    match raw_decomposition(code_point, compat) {
        Some(parts) => {
            for part in parts {
                decompose_into(part, compat, out, depth + 1);
            }
        }
        None => out.push(code_point),
    }
}

/// **规范排序**（按组合类稳定排序 ✓；标准实现：只交换 `cc(prev) > cc(curr) > 0` 的相邻对）。
fn canonical_order(points: &mut [u32]) {
    let combining = |code_point: u32| -> u32 { unicode_tables::combining(code_point).unwrap_or(0) };
    let mut index = 1usize;
    while index < points.len() {
        let current = combining(points[index]);
        if current != 0 {
            let mut previous = index;
            while previous > 0 {
                let before = combining(points[previous - 1]);
                if before == 0 || before <= current {
                    break;
                }
                points.swap(previous - 1, previous);
                previous -= 1;
            }
        }
        index += 1;
    }
}

/// Hangul 的算法组合（表里没有 ✓）：`L+V`、`LV+T` ✓。
fn compose_hangul(first: u32, second: u32) -> Option<u32> {
    if (L_BASE..L_BASE + L_COUNT).contains(&first)
        && (V_BASE..V_BASE + V_COUNT).contains(&second)
    {
        return Some(S_BASE + ((first - L_BASE) * V_COUNT + (second - V_BASE)) * T_COUNT);
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&first)
        && (first - S_BASE) % T_COUNT == 0
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&second)
    {
        return Some(first + (second - T_BASE));
    }
    None
}

/// **规范组合**（D→C 之后那一步 ✓）。
fn compose(points: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(points.len());
    let mut starter_position: Option<usize> = None;
    let mut last_combining: u32 = 0;
    for &code_point in points {
        let combining = unicode_tables::combining(code_point).unwrap_or(0);
        if let Some(position) = starter_position {
            let starter = out[position];
            // **阻塞**（标准定义 ✓）：若中间存在组合类 ≥ 当前字符的非首字符 ⇒ 不能组合。
            let blocked = combining != 0 && last_combining != 0 && last_combining >= combining;
            if !blocked {
                if let Some(composed) =
                    compose_hangul(starter, code_point).or_else(|| crate::unicode_composition::compose_pair(starter, code_point))
                {
                    out[position] = composed;
                    continue;
                }
            }
        }
        if combining == 0 {
            starter_position = Some(out.len());
            last_combining = 0;
        } else {
            last_combining = combining;
        }
        out.push(code_point);
    }
    out
}

/// **`normalize(form, unistr)`** ✓（round 107）：NFC／NFD／NFKC／NFKD。
fn normalize_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(form) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "normalize() 的第一个实参要是 str",
        ));
    };
    let composed_form = match form {
        "NFC" => true,
        "NFD" => false,
        "NFKC" => true,
        "NFKD" => false,
        other => {
            return Err(instance.raise_builtin_error(
                "ValueError",
                &format!("invalid normalization form: '{other}'"),
            ))
        }
    };
    let compat = matches!(form, "NFKC" | "NFKD");
    let Some(input) = args.get(1).copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "normalize() missing required argument 'unistr' (pos 2)",
        ));
    };
    let Some(text) = instance.str_code_point_vec(input) else {
        let name = instance.type_name(instance.type_of(input));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("normalize() argument 2 must be str, not {name}"),
        ));
    };
    let mut points: Vec<u32> = Vec::with_capacity(text.len());
    for code_point in text {
        decompose_into(code_point, compat, &mut points, 0);
    }
    canonical_order(&mut points);
    if composed_form {
        points = compose(&points);
    }
    Ok(instance.new_str_code_points(&points))
}

/// **`is_normalized(form, unistr)`** ✓：比较 `normalize(form, s) == s`（一处真相 ✓）。
fn is_normalized_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let normalized = normalize_native(instance, bound, args, &[])?;
    let original = args.get(1).copied().unwrap_or_else(|| normalized);
    let equal = pyawa_core::executor::values::values_equal_public(instance, normalized, original);
    instance.release(normalized);
    Ok(instance.new_bool(equal))
}
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
