//! `_sre` 模块（**`re` 的底座** ✓，`P3` 的"前 5 个 C 模块面"思路的直接应用 ✓）。
//!
//! 为什么先只落**常量** ✓：`re/_constants.py:18` 在**导入时**就 `from _sre import MAXREPEAT, MAXGROUPS` ✓，
//! `re/_compiler.py:18` 又 `assert _sre.MAGIC == MAGIC` ✓、`:397` 用 `_sre.CODESIZE` ✓
//! ⇒ 常量不对，`re` 连**编译期**都过不去 ✓；而 `compile`／`template`／四个 `*cased`/`*tolower` 函数随后接 ✓。
//!
//! 取值来源 ✓：**本机参照实现实测**（`python3 -c "import _sre; …"` ✓），逐值照抄 ✓：
//! `MAGIC=20230612`／`CODESIZE=4`／`MAXREPEAT=4294967295`／`MAXGROUPS=1073741823` ✓。
//!
//! 依赖口径 ✓（用户 2026-10-07：**允许依赖、不手写一切** ✓）：`compile` 将接到 `regex` crate ✓（已在
//! `pyawa-core` 的依赖边上记明理由 ✓），**不自研正则引擎** ✗。本文件目前不牵任何依赖 ✓。
//!
//! `CX-4`：stdlib 在静态扫描范围内 ⇒ 本文件不碰平台、不用 `unsafe` ✓。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`_sre`）。
pub const NAME: &str = "_sre";

/// 模块的 `__doc__`（与参照实现同源的一句话）。
pub const DOC: &str = "SRE regex support (Pyawa's built-in; the engine itself is a Rust crate).";

/// `re/_constants.py:16` 的 `MAGIC`（`re/_compiler.py:18` 会断言两者相等 ✓）。
pub const MAGIC: i64 = 20230612;

/// 码元宽度（`re/_compiler.py:397` 的 `_CODEBITS = _sre.CODESIZE * 8` ✓）。
pub const CODESIZE: i64 = 4;

/// 重复次数上限（`re/_constants.py:18` 直接 `from _sre import MAXREPEAT` ✓）。
pub const MAXREPEAT: i64 = 4294967295;

/// 分组数上限（同上 ✓）。
pub const MAXGROUPS: i64 = 1073741823;


fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// `_sre.unicode_iscased(cp)` ✓：**收整数码点**（不是字符串 ✗ —— 我先前误传 `str` ⇒ `TypeError` ✓）。
fn unicode_iscased_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code = args.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    // **"cased" 要连 titlecase 一起算** ✓（实测：`unicode_iscased(453)`（`ǅ` U+01C5，Lt 类）
    // 参照给 `True` ✗ 而我先前只判 `is_lowercase()||is_uppercase()` ⇒ 给 `False` ✗）。
    // 判据改成"**大小写映射会改变它**" ✓ —— 对 Lt 也成立 ✓，且与参照的 `cased` 口径一致 ✓。
    let cased = char::from_u32(code as u32)
        .map(|character| {
            character.to_lowercase().next() != Some(character)
                || character.to_uppercase().next() != Some(character)
        })
        .unwrap_or(false);
    Ok(instance.retain(instance.singletons().boolean(cased)))
}

/// `_sre.ascii_iscased(cp)` ✓：只认 ASCII 字母 ✓。
fn ascii_iscased_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code = args.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    let cased = u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .map(|character| character.is_ascii_alphabetic())
        .unwrap_or(false);
    Ok(instance.retain(instance.singletons().boolean(cased)))
}

/// `_sre.unicode_tolower(cp)` ✓（返回**整数码点** ✓）。
fn unicode_tolower_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code = args.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    let lowered = u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .and_then(|character| character.to_lowercase().next())
        .map(|character| character as i64)
        .unwrap_or(code);
    Ok(instance.new_int(lowered))
}

/// `_sre.ascii_tolower(cp)` ✓。
fn ascii_tolower_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code = args.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    let lowered = u32::try_from(code)
        .map(|value| {
            if value < 128 {
                ((value as u8).to_ascii_lowercase()) as i64
            } else {
                code
            }
        })
        .unwrap_or(code);
    Ok(instance.new_int(lowered))
}


// **已编译模式表**（第 565 轮 ✓）：Python 侧只拿**不透明 id** ✓ ⇒ **不新增载荷类型** ✗（`complex` 的教训 ✓）。
use std::sync::Mutex;

static REGISTRY: Mutex<Vec<regex::Regex>> = Mutex::new(Vec::new());

/// SRE 的几个位（取自 `re/_constants.py` ✓，与参照同值 ✓）。
const SRE_FLAG_IGNORECASE: i64 = 2;
const SRE_FLAG_MULTILINE: i64 = 8;
const SRE_FLAG_DOTALL: i64 = 16;
const SRE_FLAG_VERBOSE: i64 = 64;

/// `compile_raw(pattern, flags) -> id` ✓：**直接编译 `pattern` 源串**（忽略 `_compiler` 给的 SRE 字节码 ✓）。
fn compile_raw_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(pattern) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "compile_raw: 第一个实参要是 str"));
    };
    let flags = args.get(1).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let built = regex::RegexBuilder::new(pattern)
        .case_insensitive(flags & SRE_FLAG_IGNORECASE != 0)
        .multi_line(flags & SRE_FLAG_MULTILINE != 0)
        .dot_matches_new_line(flags & SRE_FLAG_DOTALL != 0)
        .ignore_whitespace(flags & SRE_FLAG_VERBOSE != 0)
        .build()
        .map_err(|error| instance.raise_builtin_error("ValueError", &format!("{error}")))?;
    let mut registry = REGISTRY.lock().map_err(|_| {
        instance.raise_builtin_error("RuntimeError", "compile_raw: 模式表被毒化")
    })?;
    registry.push(built);
    Ok(instance.new_int(registry.len() as i64 - 1))
}


/// 字节偏移 ⇒ **字符**偏移 ✓（参照 `re` 的 `span()` 口径 ✓；第 580 轮）。
///
/// `regex` crate 报的是**字节**下标 ✓（UTF-8 下与非 ASCII 字符数不等 ✗），
/// 而 Python 的 `re` 一律用**字符**下标 ✓ ⇒ 必须换算 ✓，否则 `"αβγ"` 上的
/// `span()` 会给出 `6` 而参照给 `3` ✓。
fn char_offset(text: &str, byte: usize) -> i64 {
    match text.get(..byte) {
        Some(prefix) => prefix.chars().count() as i64,
        // 理论上到不了（regex 的边界必在字符边界 ✓）；真到了就取"不超过它的字符数" ✓
        None => text
            .char_indices()
            .take_while(|(index, _)| *index < byte)
            .count() as i64,
    }
}

/// `match_raw(id, string, kind) -> "s,e;g1s,g1e;…" | None` ✓（未匹配的分组写 `-1,-1` ✓）。
/// `kind`：`match`（锚头 ✓）／`fullmatch`（锚头尾 ✓）／`search`（任意位置 ✓）。
fn match_raw_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let id = args.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    let Some(text) = args.get(1).and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "match_raw: 第二个实参要是 str"));
    };
    let kind = args
        .get(2)
        .and_then(|value| instance.text_of(*value))
        .unwrap_or("search");
    let registry = REGISTRY
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "match_raw: 模式表被毒化"))?;
    let Some(built) = registry.get(usize::try_from(id).unwrap_or(usize::MAX)) else {
        return Err(instance.raise_builtin_error("ValueError", "match_raw: 模式 id 越界"));
    };
    let caps = built.captures(text);
    let matched = caps.as_ref().filter(|caps| {
        let whole = caps.get(0).expect("第 0 组一定有");
        match kind {
            "match" => whole.start() == 0,
            "fullmatch" => whole.start() == 0 && whole.end() == text.len(),
            _ => true,
        }
    });
    let Some(caps) = matched else {
        return Ok(instance.retain(instance.singletons().none()));
    };
    // **字符偏移**（第 580 轮真 bug 修 ✓）：`regex` crate 给的是**字节**偏移 ✗，
    // 而参照 `re` 的 `span()`／`start()`／`end()` 全是**字符**偏移 ✓ ⇒ 非 ASCII 会整片错位 ✓
    // （实测：`\w+` 对 `"αβγ δ"` ⇒ 参照 `(0,3)`／我们旧码 `(0,6)` ✗）。
    let mut rendered = String::new();
    for (index, group) in caps.iter().enumerate() {
        if index > 0 {
            rendered.push(';');
        }
        match group {
            Some(group) => rendered.push_str(&format!(
                "{},{}",
                char_offset(text, group.start()),
                char_offset(text, group.end())
            )),
            None => rendered.push_str("-1,-1"),
        }
    }
    Ok(instance.new_str(&rendered))
}

/// 建 `_sre` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, value) in [
        ("MAGIC", MAGIC),
        ("CODESIZE", CODESIZE),
        ("MAXREPEAT", MAXREPEAT),
        ("MAXGROUPS", MAXGROUPS),
    ] {
        let constant = instance.new_int(value);
        instance.dict_set(namespace, name, constant);
    }
    for (name, native) in [
        ("unicode_iscased", unicode_iscased_native as NativeFn),
        ("ascii_iscased", ascii_iscased_native as NativeFn),
        ("unicode_tolower", unicode_tolower_native as NativeFn),
        ("ascii_tolower", ascii_tolower_native as NativeFn),
    ] {
        let function = make_native(instance, name, native);
        instance.dict_set(namespace, name, function);
    }
    for (name, native) in [
        ("compile_raw", compile_raw_native as NativeFn),
        ("match_raw", match_raw_native as NativeFn),
    ] {
        let function = make_native(instance, name, native);
        instance.dict_set(namespace, name, function);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
