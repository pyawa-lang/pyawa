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


// ==== `re.Pattern`／`re.Match`（第 582 轮 ✓）====================================================
//
// **为什么在 Rust 侧** ✓：`SPEC-c-modules.md:470` 写明「`_sre`（`re` 没有纯 Python 备份）必须用 Rust 重写」✓
// ⇒ 两个类与它们的方法都建在这里 ✓，**仍然不新增载荷类型** ✗：数据放在**按对象地址索引**的静态表里 ✓
// （`re` 侧只看见普通实例 ✓）。
//
// **如实范围** ✗（本轮的先落面）：Pattern 先给 `match`／`search`／`fullmatch` ✓、Match 先给
// `span`／`start`／`end` ✓；`group`／`groups`／`groupdict`／`findall`／`finditer`／`split`／`sub`／`subn`
// 与 `_sre.template` 随后补 ✓（`re` 要全了才算通 ✓）。`pos`／`endpos` 实参暂按"不切片"处理 ✗（随后补 ✓）。
// 表项不回收 ✗（模式数量有限 ✓，随后随对象释放一起清 ✓）。

/// `re.Pattern` 的方法表 ✓（`re/__init__.py` 至少要 `match/search/fullmatch` ✓，其余随后补 ✓）。
const PATTERN_METHODS: &[(&str, NativeFn)] = &[
    ("match", pattern_match_native as NativeFn),
    ("search", pattern_search_native as NativeFn),
    ("fullmatch", pattern_fullmatch_native as NativeFn),
    ("findall", pattern_findall_native as NativeFn),
    ("finditer", pattern_finditer_native as NativeFn),
    ("split", pattern_split_native as NativeFn),
    ("sub", pattern_sub_native as NativeFn),
    ("subn", pattern_subn_native as NativeFn),
];

/// `re.Match` 的方法表 ✓（本轮先 `span/start/end` ✓，`group/groups` 随后补 ✓）。
const MATCH_METHODS: &[(&str, NativeFn)] = &[
    ("span", match_span_native as NativeFn),
    ("start", match_start_native as NativeFn),
    ("end", match_end_native as NativeFn),
    ("group", match_group_native as NativeFn),
    ("groups", match_groups_native as NativeFn),
    ("groupdict", match_groupdict_native as NativeFn),
    ("expand", match_expand_native as NativeFn),
];

/// 一个已编译模式的全部数据（`re.Pattern` 实例 ↔ 这张表 ✓）。
#[allow(dead_code)] // `groupindex`／`groups` 本轮先存着 ✓（`group`／`groups` 随后用 ✓）
struct PatternData {
    built: regex::Regex,
    groupindex: Vec<(String, i64)>,
    groups: i64,
}

/// 一次匹配的跨度（`re.Match` 实例 ↔ 这张表 ✓）：下标**从 0 起**（第 0 项是整体 ✓）。
#[allow(dead_code)] // `pattern` 本轮先存着 ✓（按名字取组随后用 ✓）
struct MatchData {
    spans: Vec<(i64, i64)>,
    text: String,
    /// 对应 `PatternData` 的键（对象地址 ✓）——给按名字取组用 ✓。
    pattern: usize,
}

static PATTERNS: Mutex<Option<std::collections::HashMap<usize, PatternData>>> = Mutex::new(None);
static MATCHES: Mutex<Option<std::collections::HashMap<usize, MatchData>>> = Mutex::new(None);

/// 两个类对象（按名字索引对象地址 ✓）。**本 crate 是 `#![forbid(unsafe_code)]`** ✓ ⇒
/// 不自己解 `TypeObject` 指针 ✗，而是走 core 的**安全**入口 `build_class_from_parts` ✓
/// —— 与 `class` 语句**同一条路** ✓（一处真相 ✓：类字典、方法绑定、名字登记都由它负责 ✓）。
static CLASSES: Mutex<Option<std::collections::HashMap<&'static str, usize>>> = Mutex::new(None);

/// 建 `re.Pattern`／`re.Match` 两个类（幂等 ✓），方法挂进**类命名空间** ✓。
fn ensure_class(
    instance: &Instance,
    name: &'static str,
    methods: &[(&str, NativeFn)],
) -> Option<NonNull<Header>> {
    let mut table = CLASSES.lock().ok()?;
    let map = table.get_or_insert_with(std::collections::HashMap::new);
    if let Some(existing) = map.get(name) {
        return NonNull::new(*existing as *mut Header);
    }
    let base = instance.type_named("object")?.cast::<Header>();
    let namespace = instance.new_dict();
    for (method_name, handler) in methods {
        let function = make_native(instance, method_name, *handler);
        instance.dict_set(namespace, method_name, function);
    }
    // 契约（`classes.rs:248-251`）：`build_class_from_parts` **吃掉**调用方那份 namespace ✓
    // ⇒ 刚建的 dict（rc=1 ✓）正是"自有的一份" ✓。
    let class = pyawa_core::build_class_from_parts(
        instance,
        name.to_owned(),
        vec![base],
        namespace,
        None,
    )
    .ok()?;
    map.insert(name, class.as_ptr() as usize);
    Some(class)
}

/// 造一个类的实例（**不建**实例字典 ✓ —— 数据在静态表里 ✓）。
fn new_instance(instance: &Instance, class_name: &str) -> Option<NonNull<Header>> {
    let pointer = {
        let table = CLASSES.lock().ok()?;
        *table.as_ref()?.get(class_name)?
    };
    let class = NonNull::new(pointer as *mut Header)?;
    let ty = class.cast::<pyawa_core::TypeObject>();
    let object = instance.alloc(pyawa_core::AttributeObject::new(
        ty,
        core::cell::RefCell::new(None),
    ));
    Some(object.into_raw().cast::<Header>())
}

/// 跑一次匹配 ✓（`kind`: `match`／`fullmatch`／`search` ✓）⇒ 跨度（**字符**下标 ✓）。
fn run_match(built: &regex::Regex, text: &str, kind: &str) -> Option<Vec<(i64, i64)>> {
    let caps = built.captures(text)?;
    let whole = caps.get(0).expect("第 0 组一定有");
    let ok = match kind {
        "match" => whole.start() == 0,
        "fullmatch" => whole.start() == 0 && whole.end() == text.len(),
        _ => true,
    };
    if !ok {
        return None;
    }
    Some(
        caps.iter()
            .map(|group| match group {
                Some(group) => (
                    char_offset(text, group.start()),
                    char_offset(text, group.end()),
                ),
                None => (-1, -1),
            })
            .collect(),
    )
}

/// 把 `MatchData` 装成 `re.Match` 实例 ✓。
fn make_match(instance: &Instance, pattern_key: usize, text: &str, spans: Vec<(i64, i64)>) -> Result<NonNull<Header>, ExecError> {
    ensure_class(instance, "re.Match", MATCH_METHODS)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "re.Match 未登记"))?;
    let object = new_instance(instance, "re.Match")
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "re.Match 未登记"))?;
    let mut table = MATCHES
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "match 表被毒化"))?;
    table.get_or_insert_with(std::collections::HashMap::new).insert(
        object.as_ptr() as usize,
        MatchData { spans, text: text.to_owned(), pattern: pattern_key },
    );
    Ok(object)
}

/// Pattern 方法共用的取数（`bound` 是本实例 ✓）。
fn pattern_of(instance: &Instance, bound: Option<NonNull<Header>>) -> Result<(usize, regex::Regex), ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "需要 re.Pattern 实例"));
    };
    let key = this.as_ptr() as usize;
    let table = PATTERNS
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "pattern 表被毒化"))?;
    let Some(data) = table.as_ref().and_then(|map| map.get(&key)) else {
        return Err(instance.raise_builtin_error("TypeError", "不是已编译的 re.Pattern 实例"));
    };
    Ok((key, data.built.clone()))
}

/// `Pattern.<kind>(string, pos=0, endpos=…) -> Match | None` ✓（`pos`／`endpos` 暂不切片 ✗，随后补 ✓）。
fn pattern_kind_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kind: &str,
) -> Result<NonNull<Header>, ExecError> {
    let (key, built) = pattern_of(instance, bound)?;
    let Some(text) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str"));
    };
    let (window, offset) = match_window(instance, text, args);
    match run_match(&built, &window, kind) {
        // 跨度是**窗口内**的 ✓ ⇒ 平移回整串下标 ✓（`MatchData.text` 存**原文** ✓ ⇒ `group()` 切片才对 ✓）
        Some(spans) => {
            let shifted = spans
                .into_iter()
                .map(|(start, end)| {
                    if start < 0 {
                        (start, end)
                    } else {
                        (start + offset, end + offset)
                    }
                })
                .collect();
            make_match(instance, key, text, shifted)
        }
        None => Ok(instance.retain(instance.singletons().none())),
    }
}

fn pattern_match_native(instance: &Instance, bound: Option<NonNull<Header>>, args: &[NonNull<Header>], _kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<NonNull<Header>, ExecError> {
    pattern_kind_native(instance, bound, args, "match")
}

fn pattern_search_native(instance: &Instance, bound: Option<NonNull<Header>>, args: &[NonNull<Header>], _kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<NonNull<Header>, ExecError> {
    pattern_kind_native(instance, bound, args, "search")
}

fn pattern_fullmatch_native(instance: &Instance, bound: Option<NonNull<Header>>, args: &[NonNull<Header>], _kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<NonNull<Header>, ExecError> {
    pattern_kind_native(instance, bound, args, "fullmatch")
}

/// 取 `MatchData` 里第 `group` 组的跨度 ✓（-1,-1 ⇒ 未匹配 ✓）。
fn match_span_at(instance: &Instance, bound: Option<NonNull<Header>>, group: i64) -> Result<Option<(i64, i64)>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "需要 re.Match 实例"));
    };
    let key = this.as_ptr() as usize;
    let table = MATCHES
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "match 表被毒化"))?;
    let Some(data) = table.as_ref().and_then(|map| map.get(&key)) else {
        return Err(instance.raise_builtin_error("TypeError", "不是 re.Match 实例"));
    };
    let index = usize::try_from(group).unwrap_or(usize::MAX);
    let Some(span) = data.spans.get(index) else {
        return Err(instance.raise_builtin_error("IndexError", "no such group"));
    };
    if span.0 < 0 {
        Ok(None)
    } else {
        Ok(Some(*span))
    }
}

/// 组号实参（缺省 0 ✓）。
fn group_argument(instance: &Instance, args: &[NonNull<Header>]) -> Result<i64, ExecError> {
    match args.first() {
        Some(value) => instance
            .int_value(*value)
            .filter(|index| *index >= 0)
            .ok_or_else(|| instance.raise_builtin_error("IndexError", "no such group")),
        None => Ok(0),
    }
}

fn match_span_native(instance: &Instance, bound: Option<NonNull<Header>>, args: &[NonNull<Header>], _kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<NonNull<Header>, ExecError> {
    let group = group_argument(instance, args)?;
    match match_span_at(instance, bound, group)? {
        Some((start, end)) => {
            let tuple = instance.new_tuple(vec![instance.new_int(start), instance.new_int(end)]);
            Ok(tuple)
        }
        None => Ok(instance.retain(instance.singletons().none())),
    }
}

fn match_start_native(instance: &Instance, bound: Option<NonNull<Header>>, args: &[NonNull<Header>], _kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<NonNull<Header>, ExecError> {
    let group = group_argument(instance, args)?;
    match match_span_at(instance, bound, group)? {
        Some((start, _)) => Ok(instance.new_int(start)),
        None => Ok(instance.retain(instance.singletons().none())),
    }
}

fn match_end_native(instance: &Instance, bound: Option<NonNull<Header>>, args: &[NonNull<Header>], _kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<NonNull<Header>, ExecError> {
    let group = group_argument(instance, args)?;
    match match_span_at(instance, bound, group)? {
        Some((_, end)) => Ok(instance.new_int(end)),
        None => Ok(instance.retain(instance.singletons().none())),
    }
}



/// 取模式表里的一份（clone ✓，不在锁里调进解释器 ✓）。
fn pattern_data(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<(usize, regex::Regex, Vec<(String, i64)>), ExecError> {
    let (key, built) = pattern_of(instance, bound)?;
    let table = PATTERNS
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "pattern 表被毒化"))?;
    let groupindex = table
        .as_ref()
        .and_then(|map| map.get(&key))
        .map(|data| data.groupindex.clone())
        .unwrap_or_default();
    Ok((key, built, groupindex))
}

/// 扫出**全部**匹配的跨度 ✓（`Vec[0]` 是整体 ✓；空匹配由 regex 自己的迭代器推进 ✓）。
fn scan_spans(built: &regex::Regex, text: &str) -> Vec<Vec<(i64, i64)>> {
    built
        .captures_iter(text)
        .map(|caps| {
            caps.iter()
                .map(|group| match group {
                    Some(group) => (
                        char_offset(text, group.start()),
                        char_offset(text, group.end()),
                    ),
                    None => (-1, -1),
                })
                .collect()
        })
        .collect()
}

/// `Pattern.findall(string) -> list` ✓（无组 ⇒ 整体串 ✓；一组 ⇒ 该组串 ✓；多组 ⇒ 元组 ✓；
/// 未匹配的组按参照写**空串** ✗ 不是 `None` ✓）。
fn pattern_findall_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (_key, built, _groupindex) = pattern_data(instance, bound)?;
    let Some(text) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str"));
    };
    // **窗口只用于扫描** ✓；切片必须用**原文** ✗（平移后的下标是整串下标 ✓ —— 第 585 轮踩过：
    // 先前把 `text` 覆盖成窗口 ⇒ 第二次匹配切片越界 ⇒ 返回空串 ✗，与参照 `['a','a']` 不符 ✓）。
    let (window, offset) = match_window(instance, text, args);
    let groups = built.captures_len().saturating_sub(1);
    let mut items: Vec<NonNull<Header>> = Vec::new();
    for spans in scan_spans(&built, &window) {
        let spans: Vec<(i64, i64)> = spans
            .into_iter()
            .map(|(start, end)| {
                if start < 0 {
                    (start, end)
                } else {
                    (start + offset, end + offset)
                }
            })
            .collect();
        if groups == 0 {
            let (start, end) = spans[0];
            items.push(instance.new_str(&slice_chars(text, start, end)));
        } else if groups == 1 {
            let (start, end) = spans[1];
            let piece = if start < 0 { String::new() } else { slice_chars(text, start, end) };
            items.push(instance.new_str(&piece));
        } else {
            let mut row: Vec<NonNull<Header>> = Vec::with_capacity(groups);
            for (start, end) in spans.iter().skip(1) {
                let piece = if *start < 0 { String::new() } else { slice_chars(text, *start, *end) };
                row.push(instance.new_str(&piece));
            }
            items.push(instance.new_tuple(row));
        }
    }
    Ok(instance.new_list(items))
}

/// `Match.groupdict(default=None)` ✓：`{名字: 文本}` ✓（未匹配 ⇒ `default` ✓）。
fn match_groupdict_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let data = match_data(instance, bound)?;
    let default = args.first().copied();
    let table = PATTERNS
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "pattern 表被毒化"))?;
    let groupindex = table
        .as_ref()
        .and_then(|map| map.get(&data.pattern))
        .map(|pattern| pattern.groupindex.clone())
        .unwrap_or_default();
    drop(table);
    let result = instance.new_dict();
    for (name, index) in groupindex {
        let value = match group_text(&data, index) {
            Some(text) => instance.new_str(&text),
            None => match default {
                Some(value) => instance.retain(value),
                None => instance.retain(instance.singletons().none()),
            },
        };
        instance.dict_set(result, &name, value);
    }
    Ok(result)
}


/// `pos`／`endpos`（第 585 轮 ✓）：在 `[pos, endpos)` 这段**字符窗口**上匹配 ✓。
///
/// 参照语义 ✓：`Pattern.match(string, pos, endpos)` 只在窗口内找 ✓，而报出的下标仍是
/// **相对整串**的 ✓ ⇒ 这里返回"窗口文本 ＋ 平移量" ✓，匹配后把跨度加回去 ✓。
fn match_window(
    instance: &Instance,
    text: &str,
    args: &[NonNull<Header>],
) -> (String, i64) {
    let length = text.chars().count() as i64;
    let pos = args
        .get(1)
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(0)
        .clamp(0, length);
    let endpos = args
        .get(2)
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(length)
        .clamp(pos, length);
    (slice_chars(text, pos, endpos), pos)
}

/// 按**字符**下标切片 ✓（`span()` 的口径是字符 ✓ ⇒ 与参照一致 ✓）。
fn slice_chars(text: &str, start: i64, end: i64) -> String {
    let from = usize::try_from(start).unwrap_or(usize::MAX);
    let to = usize::try_from(end).unwrap_or(usize::MAX);
    text.chars().skip(from).take(to.saturating_sub(from)).collect()
}

/// 组号实参 ⇒ 组下标 ✓（`int` 直接用 ✓；`str` 查模式的 `groupindex` ✓）。
fn resolve_group(
    instance: &Instance,
    data: &MatchData,
    value: NonNull<Header>,
) -> Result<i64, ExecError> {
    if let Some(index) = instance.int_value(value) {
        return Ok(index);
    }
    let Some(name) = instance.text_of(value) else {
        return Err(instance.raise_builtin_error("IndexError", "no such group"));
    };
    let table = PATTERNS
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "pattern 表被毒化"))?;
    let found = table
        .as_ref()
        .and_then(|map| map.get(&data.pattern))
        .and_then(|pattern| {
            pattern
                .groupindex
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, index)| *index)
        });
    found.ok_or_else(|| instance.raise_builtin_error("IndexError", "no such group"))
}

/// 取一组的**文本** ✓（未匹配 ⇒ `None` ✓）。
fn group_text(data: &MatchData, index: i64) -> Option<String> {
    let span = *data.spans.get(usize::try_from(index).ok()?)?;
    if span.0 < 0 {
        return None;
    }
    Some(slice_chars(&data.text, span.0, span.1))
}




/// `Pattern.finditer(string, pos=0, endpos=len) -> iterator` ✓（第 589 轮）。
///
/// 造一批 `re.Match` ✓ 再用 core 的公共入口 `iter_value` 包成**真迭代器** ✓
/// （`itertools` 也在用同一个入口 ✓ ⇒ 不是新通道 ✓）；窗口与 `findall` 同款 ✓：
/// **窗口只用于扫** ✓，跨度平移回整串 ✓，`MatchData.text` 存**原文** ✓ ⇒ `group()` 切片才对 ✓。
fn pattern_finditer_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (key, built, _groupindex) = pattern_data(instance, bound)?;
    let Some(text) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str"));
    };
    let (window, offset) = match_window(instance, text, args);
    let mut items: Vec<NonNull<Header>> = Vec::new();
    for spans in scan_spans(&built, &window) {
        let shifted: Vec<(i64, i64)> = spans
            .into_iter()
            .map(|(start, end)| {
                if start < 0 {
                    (start, end)
                } else {
                    (start + offset, end + offset)
                }
            })
            .collect();
        items.push(make_match(instance, key, text, shifted)?);
    }
    let list = instance.new_list(items);
    pyawa_core::executor::iter::iter_value(instance, list)
}


/// `Match.expand(template) -> str` ✓（第 590 轮）：按本匹配展开模板 ✓（语义与 `sub` 的模板同一处 ✓）。
fn match_expand_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let data = match_data(instance, bound)?;
    let Some(template) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "expand: 实参要是 str"));
    };
    let groupindex = {
        let table = PATTERNS
            .lock()
            .map_err(|_| instance.raise_builtin_error("RuntimeError", "pattern 表被毒化"))?;
        table
            .as_ref()
            .and_then(|map| map.get(&data.pattern))
            .map(|pattern| pattern.groupindex.clone())
            .unwrap_or_default()
    };
    let rendered = expand_template(instance, &data, &groupindex, template)?;
    Ok(instance.new_str(&rendered))
}

/// 展开**替换模板** ✓（第 587 轮）：`\g<名字>`／`\g<0>`／`\1`…`\99`／`\\`／`\n`／`\t`／`\r` ✓。
///
/// **如实范围** ✗：未识别转义按参照**原样保留**（`\q` ⇒ `\q` ✓）；CPython 3.12+ 对模板里的
/// 未知转义报错 ✗ —— 这条差异随后补 ✓（先不静默改语义 ✓）。
fn expand_template(
    instance: &Instance,
    data: &MatchData,
    groupindex: &[(String, i64)],
    template: &str,
) -> Result<String, ExecError> {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(escape) = chars.next() else {
            return Err(instance.raise_builtin_error("ValueError", "bad escape (end of pattern)"));
        };
        if escape == 'g' {
            if chars.next() != Some('<') {
                return Err(instance.raise_builtin_error("ValueError", "missing < in group name"));
            }
            let mut name = String::new();
            loop {
                match chars.next() {
                    Some('>') => break,
                    Some(ch) => name.push(ch),
                    None => {
                        return Err(instance.raise_builtin_error(
                            "ValueError",
                            "missing >, unterminated name",
                        ))
                    }
                }
            }
            let index = match name.parse::<i64>() {
                Ok(number) => number,
                Err(_) => groupindex
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, index)| *index)
                    .ok_or_else(|| {
                        instance.raise_builtin_error("IndexError", "unknown group name")
                    })?,
            };
            out.push_str(&group_text(data, index).unwrap_or_default());
        } else if escape.is_ascii_digit() {
            let mut digits = escape.to_string();
            if let Some(next) = chars.peek().copied() {
                if next.is_ascii_digit() {
                    digits.push(next);
                    chars.next();
                }
            }
            let index = digits.parse::<i64>().unwrap_or(0);
            out.push_str(&group_text(data, index).unwrap_or_default());
        } else {
            match escape {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '\\' => out.push('\\'),
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
        }
    }
    Ok(out)
}

/// `sub`／`subn` 共用 ✓：返回（新串 ✓，替换次数 ✓）。
fn substitute(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
) -> Result<(String, i64), ExecError> {
    let (key, built, groupindex) = pattern_data(instance, bound)?;
    // 替换可以是**字符串模板** ✓ 也可以是**可调用对象** ✓（`re.sub` 两种都收 ✓，第 588 轮）。
    let repl = args.first().copied();
    let template = repl.and_then(|value| instance.text_of(value));
    let Some(text) = args.get(1).and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "第二个实参要是 str"));
    };
    let count = args.get(2).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let mut out = String::new();
    let mut last = 0_i64;
    let mut replaced = 0_i64;
    for spans in scan_spans(&built, text) {
        if count > 0 && replaced >= count {
            break;
        }
        let (start, end) = spans[0];
        out.push_str(&slice_chars(text, last, start));
        let data = MatchData { spans: spans.clone(), text: text.to_owned(), pattern: key };
        let piece = match template {
            Some(template) => expand_template(instance, &data, &groupindex, template)?,
            None => {
                let Some(callable) = repl else {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "替换要是字符串或可调用对象",
                    ));
                };
                // 可调用替换 ✓：造一个 `re.Match` 交给它 ✓（与参照一致 ✓）
                let matched = make_match(instance, key, text, spans.clone())?;
                let outcome = pyawa_core::executor::call::call_value(
                    instance,
                    callable,
                    &[matched],
                    &[],
                );
                // 我们持有 `matched` 那一份（`call_value` 若需要会自己 incref ✓）
                instance.release(matched);
                let result = outcome?;
                let Some(rendered) = instance.text_of(result) else {
                    instance.release(result);
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "替换函数必须返回 str",
                    ));
                };
                let owned = rendered.to_owned();
                instance.release(result);
                owned
            }
        };
        out.push_str(&piece);
        last = end;
        replaced += 1;
    }
    out.push_str(&slice_chars(text, last, text.chars().count() as i64));
    Ok((out, replaced))
}

/// `Pattern.sub(repl, string, count=0) -> str` ✓。
fn pattern_sub_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (text, _count) = substitute(instance, bound, args)?;
    Ok(instance.new_str(&text))
}

/// `Pattern.subn(repl, string, count=0) -> (str, int)` ✓。
fn pattern_subn_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (text, count) = substitute(instance, bound, args)?;
    let value = instance.new_str(&text);
    let number = instance.new_int(count);
    Ok(instance.new_tuple(vec![value, number]))
}

/// `Pattern.split(string, maxsplit=0) -> list` ✓（第 586 轮）。
///
/// 参照语义 ✓：按每处匹配切开 ✓；模式**有捕获组**时把各组文本**插进**结果 ✓
/// （未匹配的组插 `None` ✓ 不是空串 ✗）；`maxsplit` 限制**切几次** ✓（0 ⇒ 不限 ✓）。
fn pattern_split_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (_key, built, _groupindex) = pattern_data(instance, bound)?;
    let Some(text) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str"));
    };
    let maxsplit = args.get(1).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let mut last = 0_i64;
    let mut cuts = 0_i64;
    for spans in scan_spans(&built, text) {
        if maxsplit > 0 && cuts >= maxsplit {
            break;
        }
        let (start, end) = spans[0];
        items.push(instance.new_str(&slice_chars(text, last, start)));
        for (group_start, group_end) in spans.iter().skip(1) {
            if *group_start < 0 {
                items.push(instance.retain(instance.singletons().none()));
            } else {
                items.push(instance.new_str(&slice_chars(text, *group_start, *group_end)));
            }
        }
        last = end;
        cuts += 1;
    }
    items.push(instance.new_str(&slice_chars(text, last, text.chars().count() as i64)));
    Ok(instance.new_list(items))
}

/// `Match.group([组…])` ✓：无实参 ⇒ 整体 ✓；一个 ⇒ 该组 ✓（未匹配 ⇒ `None` ✓）；多个 ⇒ 元组 ✓。
fn match_group_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let data = match_data(instance, bound)?;
    let mut values: Vec<NonNull<Header>> = Vec::new();
    let indices: Vec<i64> = match args.len() {
        0 => vec![0],
        _ => {
            let mut resolved = Vec::with_capacity(args.len());
            for argument in args {
                resolved.push(resolve_group(instance, &data, *argument)?);
            }
            resolved
        }
    };
    for index in indices {
        match group_text(&data, index) {
            Some(text) => values.push(instance.new_str(&text)),
            None => values.push(instance.retain(instance.singletons().none())),
        }
    }
    if values.len() == 1 {
        return Ok(values[0]);
    }
    Ok(instance.new_tuple(values))
}

/// `Match.groups(default=None)` ✓：第 1…n 组 ✓（未匹配 ⇒ `default` ✓）。
fn match_groups_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let data = match_data(instance, bound)?;
    let default = args.first().copied();
    let mut values: Vec<NonNull<Header>> = Vec::new();
    for index in 1..data.spans.len() {
        match group_text(&data, index as i64) {
            Some(text) => values.push(instance.new_str(&text)),
            None => match default {
                Some(value) => values.push(instance.retain(value)),
                None => values.push(instance.retain(instance.singletons().none())),
            },
        }
    }
    Ok(instance.new_tuple(values))
}

/// 取 `MatchData`（借用本实例的表项 ✓，调用方clone 出需要的字段 ✓）。
fn match_data(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<MatchData, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "需要 re.Match 实例"));
    };
    let key = this.as_ptr() as usize;
    let table = MATCHES
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "match 表被毒化"))?;
    let Some(data) = table.as_ref().and_then(|map| map.get(&key)) else {
        return Err(instance.raise_builtin_error("TypeError", "不是 re.Match 实例"));
    };
    Ok(MatchData {
        spans: data.spans.clone(),
        text: data.text.clone(),
        pattern: data.pattern,
    })
}

/// `compile(pattern, flags, code, groups, groupindex, indexgroup) -> Pattern` ✓。
///
/// 忽略 `code`（`_compiler` 生成的 SRE 字节码 ✗）与 `indexgroup` ✓，用 `pattern` 源串直编 ✓；
/// `groupindex`（名字 ⇒ 组号 ✓）留着给按名字取组用 ✓。
fn compile_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(pattern) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "compile: 第一个实参要是 str"));
    };
    let flags = args.get(1).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let groups = args.get(3).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let mut groupindex = Vec::new();
    if let Some(mapping) = args.get(4) {
        // `groupindex` 是 `{名字: 组号}` ✓（`re/_compiler.py` 的 `p.state.groupdict` ✓）
        for (key, value) in instance.dict_entries(*mapping).unwrap_or_default() {
            if let (Some(name), Some(index)) = (instance.text_of(key), instance.int_value(value)) {
                groupindex.push((name.to_owned(), index));
            }
        }
    }
    let built = regex::RegexBuilder::new(pattern)
        .case_insensitive(flags & SRE_FLAG_IGNORECASE != 0)
        .multi_line(flags & SRE_FLAG_MULTILINE != 0)
        .dot_matches_new_line(flags & SRE_FLAG_DOTALL != 0)
        .ignore_whitespace(flags & SRE_FLAG_VERBOSE != 0)
        .build()
        .map_err(|error| instance.raise_builtin_error("ValueError", &format!("{error}")))?;
    ensure_class(instance, "re.Pattern", PATTERN_METHODS)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "re.Pattern 建类失败"))?;
    let object = new_instance(instance, "re.Pattern")
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "re.Pattern 未登记"))?;
    let mut table = PATTERNS
        .lock()
        .map_err(|_| instance.raise_builtin_error("RuntimeError", "pattern 表被毒化"))?;
    table.get_or_insert_with(std::collections::HashMap::new).insert(
        object.as_ptr() as usize,
        PatternData { built, groupindex, groups },
    );
    Ok(object)
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
        ("compile", compile_native as NativeFn),
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
