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


// **每实例状态（`CX-3`／`T-CX-3`：禁止进程级对象表）** —— 见本文件"状态归属"一节（`ensure_class` 之前）。
use std::sync::Mutex;

// ==== 两档引擎（task-17 ✓）=========================================================================
//
// **为什么两档** ✓：Rust `regex` 是**线性时间、无回溯**的快路径 ✓，但**架构上不支持**
// look-around／反向引用 ✗ —— 而参照 `re` 支持 ✓（`Lib/textwrap.py:82` 的 `(?<=…)`／`(?=…)`
// 正卡在这一条上 ✗，`logging`（13 下游）／`concurrent.futures`／`traceback` 一族都压在它上面 ✓）
// ⇒ 用户 2026-10-16 拍板：快路径编译不过时退 `fancy-regex`（回溯引擎 ✓）。
// **一处真相**：编译只经 `build_engine` ✓、匹配产出只经 `Engine::captures*` ✓
// ⇒ span／group／findall／finditer／split／sub **两条引擎走同一条管线** ✓（口径天然一致 ✓）。

/// 编译好的引擎 ✓（`Fast` ＝ `regex` 快路径 ✓；`Fancy` ＝ `fancy-regex` 回溯路径 ✓）。
#[derive(Clone)]
enum Engine {
    Fast(regex::Regex),
    Fancy(fancy_regex::Regex),
}

/// 一次匹配的**全部组**跨度（**字节**下标 ✓；未匹配的组写 `None` ✓）—— 两档引擎共用的中间形状 ✓。
type GroupSpans = Vec<Option<(usize, usize)>>;

/// `regex` 一档的取跨度 ✓。
fn spans_fast(captures: regex::Captures<'_>) -> GroupSpans {
    captures.iter().map(|group| group.map(|found| (found.start(), found.end()))).collect()
}

/// `fancy-regex` 一档的取跨度 ✓（同序、同长 ✓ —— 第 0 项是整体 ✓）。
fn spans_fancy(captures: fancy_regex::Captures<'_, str>) -> GroupSpans {
    captures.iter().map(|group| group.map(|found| (found.start(), found.end()))).collect()
}

impl Engine {
    /// 从**字节**偏移 `start` 起找**首个**匹配的全部组跨度 ✓（`None` ⇒ 没匹配到 ✓；
    /// `Err` ⇒ 回溯引擎**运行期**报错 ✓ —— 快路径不产生这一档 ✓）。
    ///
    /// **为什么用引擎自己的 pos 接口** ✓（task-17 第 2 轮真 bug 修 ✗）：切片窗口会把 `pos` **之前**
    /// 的上下文一起切掉 ✗ ⇒ `^`／`\b`／look-behind 全错 ✓（实测 `(?<=a)b` 对 `"xabxa"` 的
    /// `search(…, 2)` ⇒ 参照 `(2,3)` ✓ 而切片法 `None` ✗）。两条边都**按整串上下文**解释起点 ✓
    /// （`regex::captures_at` ✓／`fancy_regex::captures_from_pos` ✓，`\A` 仍只在 `start == 0` 命中 ✓）。
    fn captures_from(&self, haystack: &str, start: usize) -> Result<Option<GroupSpans>, String> {
        match self {
            Engine::Fast(built) => Ok(built.captures_at(haystack, start).map(spans_fast)),
            Engine::Fancy(built) => built
                .captures_from_pos(haystack, start)
                .map(|found| found.map(spans_fancy))
                .map_err(|error| error.to_string()),
        }
    }

    /// 从 `start` 起**逐个**产出非重叠匹配 ✓（`findall`／`finditer`／`split`／`sub` 共用 ✓）。
    ///
    /// 空匹配**前进一个字符** ✓、到**句尾就停** ✓（否则句尾空匹配会无限重复 ✗）—— 与参照同口径 ✓
    /// （实测 `z*` 对 `"ab"` ⇒ `['', '', '']` ✓、`''` 对 `"abc"` 的 `search(pos=10)` ⇒ `(3,3)` ✓）。
    /// **两档同一条游标** ✓ ⇒ 不再依赖各自迭代器的空匹配推进口径 ✓。
    fn scan_from(&self, haystack: &str, start: usize) -> Result<Vec<GroupSpans>, String> {
        let mut all = Vec::new();
        let mut cursor = start;
        loop {
            let Some(spans) = self.captures_from(haystack, cursor)? else { break };
            let (whole_start, whole_end) = spans[0].expect("第 0 组一定有");
            all.push(spans);
            if whole_end > whole_start {
                cursor = whole_end;
            } else if whole_end >= haystack.len() {
                break;
            } else {
                cursor = next_char_boundary(haystack, whole_end);
            }
        }
        Ok(all)
    }

    /// 组数（**含第 0 组** ✓ —— `findall` 判"有无捕获组"用 ✓，两档同口径 ✓）。
    fn captures_len(&self) -> usize {
        match self {
            Engine::Fast(built) => built.captures_len(),
            Engine::Fancy(built) => built.captures_len(),
        }
    }
}

/// **字符**下标 ⇒ **字节**偏移 ✓（引擎一律收字节 ✓；下标必在字符边界上 ✓）。
fn char_to_byte(text: &str, index: i64) -> usize {
    let target = usize::try_from(index).unwrap_or(usize::MAX);
    text.char_indices().nth(target).map(|(byte, _)| byte).unwrap_or(text.len())
}

/// 下一个**字符边界**（空匹配前进用 ✓；已在末尾就回末尾 ✓）。
fn next_char_boundary(text: &str, byte: usize) -> usize {
    let mut next = byte + 1;
    while next < text.len() && !text.is_char_boundary(next) {
        next += 1;
    }
    next
}

/// **唯一的编译入口** ✓：`regex` 先试 ✓；失败（look-around／反向引用等它**不支持**的构造 ✗）
/// 才退 `fancy-regex` ✓。两档的 flags 口径一字不差 ✓（VERBOSE 已由调用方**自己剥** ✓）。
///
/// **报错口径** ✓：快路径失败**不**直接报 ✗ —— 因为 `\1`／`(?<=…)` 这些**参照合法**的构造
/// 正是要退回溯引擎的 ✓；两档都编译不过时才把**回溯引擎**的报错交出去 ✓。
fn build_engine(normalized: &str, flags: i64) -> Result<Engine, String> {
    if let Ok(built) = regex::RegexBuilder::new(normalized)
        .case_insensitive(flags & SRE_FLAG_IGNORECASE != 0)
        .multi_line(flags & SRE_FLAG_MULTILINE != 0)
        .dot_matches_new_line(flags & SRE_FLAG_DOTALL != 0)
        .ignore_whitespace(false)
        .build()
    {
        return Ok(Engine::Fast(built));
    }
    fancy_regex::RegexBuilder::new(normalized)
        .case_insensitive(flags & SRE_FLAG_IGNORECASE != 0)
        .multi_line(flags & SRE_FLAG_MULTILINE != 0)
        .dot_matches_new_line(flags & SRE_FLAG_DOTALL != 0)
        .ignore_whitespace(false)
        .build()
        .map(Engine::Fancy)
        .map_err(|error| error.to_string())
}

/// **编译引擎缓存的键**：`"<flags>\u{1f}<规范化模式>"` ✓（flags 与模式之间用 `U+001F`，
/// 正则源码里不可能出现的控制字符 ⇒ 拆分无歧义 ✓）。
const ENGINE_KEY_SEPARATOR: char = '\u{1f}';

/// **引擎缓存上限**（条）。取参照 `Lib/re/__init__.py` 的 `_MAXCACHE`（512 ✓）同量级：
/// 缓存无限长会把"编过的每个模式"永久钉在进程里；512 条覆盖 `Lib/` 的热模式 ✓。
const ENGINE_CACHE_LIMIT: usize = 512;

/// **已编译引擎的缓存** —— 本文件**唯一**剩下的进程级结构，且它是**纯值缓存**（不是实例状态）：
///
/// - 键是**规范化模式串 ＋ flags**（都是 `str`／`int` 的纯值 ✓），值是**不可变**的 [`Engine`] ✓；
/// - **不含任何 Python 对象、类型指针或 `Instance` 指针** ⇒ 跨实例共享**在语义上不可观察**
///   （同源同 flags 编出来的引擎本来就该是同一个函数 ✓）⇒ 不可能再出现"第二个实例拿到第一个
///   实例的 `re.Pattern` 类型"那一类串对象 ✗；
/// - 有界（`ENGINE_CACHE_LIMIT`，FIFO 淘汰 ✓）⇒ 不会随编译次数无限增长 ✓（旧实现
///   `REGISTRY: Vec<Engine>` 是**无界**的 ✓）。
///
/// 为什么不能像另外三张表那样放进对象自身 ✗：`regex`／`fancy-regex` 的引擎是 **Rust 载荷**
/// （无对应 Python 对象），而 `pyawa-stdlib` 是 `#![forbid(unsafe_code)]`、core 也没有"按
/// 对象读写不透明载荷"的安全接口 ⇒ 本 crate 无法把引擎挂到 `re.Pattern` 实例上 ✓ —— 这是
/// **如实的技术边界**，不是把实例状态留在了全局 ✓（表里的东西与实例身份无关 ✓）。
static ENGINE_CACHE: Mutex<Option<EngineCache>> = Mutex::new(None);

/// 引擎缓存本体（`Option` 只是为了让 `static` 能 `const` 初始化 —— `HashMap::new` 不是 `const`）。
#[derive(Default)]
struct EngineCache {
    entries: std::collections::HashMap<String, Engine>,
    /// 插入顺序（FIFO 淘汰用 ✓；命中不改变顺序 —— 简单、可预期 ✓）。
    order: std::collections::VecDeque<String>,
}

impl EngineCache {
    /// 取引擎（命中 ✓）或编译并缓存（未命中／被淘汰 ✓）。
    ///
    /// **淘汰后仍可用** ✓：键里带着"规范化模式 ＋ flags"两个纯值 ⇒ 回退到**重新编译**即可
    /// （同源同 flags 编出来的引擎与缓存里那份等价 ✓）⇒ 淘汰只是性能事件，不是正确性事件 ✓。
    fn engine(&mut self, key: &str) -> Result<Engine, String> {
        if let Some(found) = self.entries.get(key) {
            return Ok(found.clone());
        }
        let (flags, normalized) = key
            .split_once(ENGINE_KEY_SEPARATOR)
            .ok_or_else(|| "坏掉的引擎键（缺 flags 分隔符）".to_owned())?;
        let flags: i64 = flags.parse().map_err(|_| "坏掉的引擎键（flags 不是整数）".to_owned())?;
        let built = build_engine(normalized, flags)?;
        while self.entries.len() >= ENGINE_CACHE_LIMIT {
            let Some(oldest) = self.order.pop_front() else { break };
            self.entries.remove(&oldest);
        }
        self.entries.insert(key.to_owned(), built.clone());
        self.order.push_back(key.to_owned());
        Ok(built)
    }
}

/// 规范化模式 ＋ flags ⇒ 引擎缓存的键 ✓。
fn engine_key(normalized: &str, flags: i64) -> String {
    let mut key = String::with_capacity(normalized.len() + 12);
    key.push_str(&flags.to_string());
    key.push(ENGINE_KEY_SEPARATOR);
    key.push_str(normalized);
    key
}

/// 按引擎键取引擎（未命中 ⇒ 编译并缓存 ✓）；编译错误报 `ValueError`（与旧 `build_engine`
/// 调用点同一档 ✓）。
fn engine_for_key(instance: &Instance, key: &str) -> Result<Engine, ExecError> {
    let built = {
        let mut table = ENGINE_CACHE
            .lock()
            .map_err(|_| instance.raise_builtin_error("RuntimeError", "引擎缓存被毒化"))?;
        let cache = table.get_or_insert_with(EngineCache::default);
        cache.engine(key)
    };
    built.map_err(|error| instance.raise_builtin_error("ValueError", &error))
}

/// 把参照里合法、Rust `regex` 里**非法**的**字符类开头 `]`** 规范化 ✓（第 722 轮 ✓）。
///
/// 参照（以及 POSIX）把紧跟在 `[`（或 `[^`）后面的 `]` 当**字面量** ✓；Rust 的 `regex`
/// 直接报 `unclosed character class` ✗ —— `Lib/email/utils.py:34` 的
/// `special = re.compile(r'[][\\()<>@,:;".]')` 正是它 ✓（`urllib.parse` 落地后
/// `email.utils` 就卡在这一行 ✓）。
/// 做法：`[`／`[^` 之后紧跟 `]` 时补一个反斜杠 ✓（`\]` 在字符类里合法 ✓），
/// 并**让类继续开着** ✓（那个 `]` 不是收尾括号 ✓）。
fn normalize_character_classes(pattern: &str) -> String {
    let characters: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut index = 0usize;
    let mut in_class = false;
    while index < characters.len() {
        let character = characters[index];
        // **八进制转义**要翻译 ✓（第 722 轮）：参照认 `\041`（＝ `!` ✓，`Lib/email/header.py:48`
        // 的 `re.compile(r'[\041-\176]+:$')` 正是它 ✗），Rust 的 `regex` 把 `\0` 当**反向引用**
        // 报 `backreferences are not supported` ✗。规则照参照：`\0` 开头最多再两位八进制，
        // **或**恰好三位八进制（首位 1–7 ✓）；其余 `\1`–`\99` 是**组引用** ⇒ 原样留着 ✓（如实报错 ✓）。
        if character == '\\' {
            let mut digits = 0usize;
            while digits < 3
                && matches!(characters.get(index + 1 + digits), Some(c) if ('0'..='7').contains(c))
            {
                digits += 1;
            }
            let first_is_zero = characters.get(index + 1) == Some(&'0');
            if digits > 0 && (first_is_zero || digits == 3) {
                let text: String = characters[index + 1..index + 1 + digits].iter().collect();
                let value = u32::from_str_radix(&text, 8).unwrap_or(0);
                // 参照对 > 0o377 有专门报错 ✗ ⇒ 原样留着（让 `regex` 如实报错 ✓，不静默截断 ✓）。
                if value <= 0xFF {
                    out.push_str(&format!("\\x{value:02x}"));
                    index += 1 + digits;
                    continue;
                }
            }
            // **类内的 `\b`／`\a`／`\v`** ✓（第 722 轮）：参照在字符类里把它们当**控制字符**
            // （`\b` ＝ 退格 `0x08` ✓、`\a` ＝ 铃 `0x07` ✓、`\v` ＝ 垂直制表 `0x0B` ✓），
            // 而 Rust 的 `regex` 把 `\b` 当**词边界** ⇒ 类内直接报
            // `invalid escape sequence found in character class` ✗
            //（`Lib/json/encoder.py:18` 的 `re.compile(r'[\x00-\x1f\\"\b\f\n\r\t]')` 正是它 ✗）。
            // ⇒ 类内一律改写成 `\xNN` ✓。
            if in_class {
                if let Some(next) = characters.get(index + 1).copied() {
                    let replacement = match next {
                        'b' => Some(0x08u32),
                        'a' => Some(0x07u32),
                        'v' => Some(0x0Bu32),
                        _ => None,
                    };
                    if let Some(code) = replacement {
                        out.push_str(&format!("\\x{code:02x}"));
                        index += 2;
                        continue;
                    }
                }
            }
            // 其余转义序列**整段**照抄 ✓（`\]`／`\\` 一类不能被当成类边界 ✓）。
            out.push('\\');
            if let Some(next) = characters.get(index + 1) {
                out.push(*next);
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if !in_class && character == '[' {
            in_class = true;
            out.push('[');
            index += 1;
            if characters.get(index) == Some(&'^') {
                out.push('^');
                index += 1;
            }
            // 开头的 `]` 是**字面量** ✓：转义它、**不**收类 ✓。
            if characters.get(index) == Some(&']') {
                out.push('\\');
                out.push(']');
                index += 1;
            }
            continue;
        }
        if in_class && character == ']' {
            in_class = false;
        }
        // 类里的 `[` 参照当**字面量** ✓、Rust 的 `regex` 不接受未转义写法 ✗ ⇒ 转义 ✓
        // （`[][a]` 规范化后仍是 `[\]` ＋ `[a]`，内层那个 `[` 正是这种 ✓）。
        if in_class && character == '[' {
            out.push('\\');
            out.push('[');
            index += 1;
            continue;
        }
        out.push(character);
        index += 1;
    }
    out
}

/// SRE 的几个位（取自 `re/_constants.py` ✓，与参照同值 ✓）。
const SRE_FLAG_IGNORECASE: i64 = 2;
const SRE_FLAG_MULTILINE: i64 = 8;
const SRE_FLAG_DOTALL: i64 = 16;
const SRE_FLAG_VERBOSE: i64 = 64;

/// **`re.VERBOSE` 的空白剥离** ✓（第 722 轮真 bug 修 ✗）：参照的 `VERBOSE` **不**剥**字符类里**的空白 ✓
/// （`re.compile(r'[ \t\n\r]*', re.VERBOSE)` 里的空格仍是字面量 ✓），而 Rust 的
/// `ignore_whitespace(true)` 把类里的空白也吞掉 ✗ ⇒ `Lib/json/decoder.py:132` 的
/// `WHITESPACE = re.compile(r'[ \t\n\r]*', FLAGS)`（`FLAGS` 含 `re.VERBOSE` ✓）变成零宽匹配 ✓
/// ⇒ `JSONObject` 在多键对象上误报 `Expecting property name enclosed in double quotes` ✗
///（实测 `json.loads('{"a": 1, "b": 2}')` ✓）。做法：**自己剥** —— 类外剥空白与 `#` 注释、
/// 类内（含转义）原样 ✓ —— 再把 `ignore_whitespace` 关掉 ✓。
fn strip_verbose_whitespace(pattern: &str) -> String {
    let characters: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut index = 0usize;
    let mut in_class = false;
    while index < characters.len() {
        let character = characters[index];
        // 转义序列整段保留 ✓（`\ ` 那个空格是**字面量** ✓、`\#` 同理 ✓）。
        if character == '\\' {
            out.push('\\');
            if let Some(next) = characters.get(index + 1) {
                out.push(*next);
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if !in_class {
            if character.is_whitespace() {
                index += 1;
                continue;
            }
            if character == '#' {
                while index < characters.len() && characters[index] != '\n' {
                    index += 1;
                }
                continue;
            }
        }
        if character == '[' && !in_class {
            in_class = true;
        } else if character == ']' && in_class {
            in_class = false;
        }
        out.push(character);
        index += 1;
    }
    out
}

/// `compile_raw(pattern, flags) -> key` ✓：**直接编译 `pattern` 源串**（忽略 `_compiler` 给的 SRE 字节码 ✓）。
///
/// **返回纯值引擎键**（`str`：`"<flags>\u{1f}<规范化模式>"`）—— 不是进程级 `REGISTRY` 的下标 ✓
/// （task-10 删表：同进程两个实例不再共用一张模式表 ✓）。键自带还原信息 ⇒ `match_raw` 即使
/// 遇到缓存淘汰也能**重新编译**出等价引擎 ✓（见 [`EngineCache::engine`]）。
fn compile_raw_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(pattern_object) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "compile_raw: 第一个实参要是 str"));
    };
    let (pattern, _is_bytes) = pattern_source(instance, pattern_object)?;
    let flags = args.get(1).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    // **字符类规范化** ✓（第 722 轮）：`[]…]` 那种开头 `]` 参照当字面量 ✓、Rust 的 `regex` 不接受 ✗。
    let normalized = normalize_character_classes(&pattern);
    // **VERBOSE 自己剥** ✓（第 722 轮）：Rust 的 `ignore_whitespace` 会连**类内**空白一起吞 ✗。
    let normalized = if flags & SRE_FLAG_VERBOSE != 0 {
        strip_verbose_whitespace(&normalized)
    } else {
        normalized
    };
    let key = engine_key(&normalized, flags);
    // **立刻编译**：编译错误与旧口径同一时点抛出（`ValueError` ✓），不是等 `match_raw` 才报 ✓。
    let _ = engine_for_key(instance, &key)?;
    Ok(instance.new_str(&key))
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

/// `match_raw(key, string, kind) -> "s,e;g1s,g1e;…" | None` ✓（未匹配的分组写 `-1,-1` ✓）。
/// `key` 是 `compile_raw` 交回的**纯值引擎键** ✓；`kind`：`match`（锚头 ✓）／`fullmatch`（锚头尾 ✓）／
/// `search`（任意位置 ✓）。
fn match_raw_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(key) = args.first().and_then(|value| instance.text_of(*value)).map(str::to_owned) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "match_raw: 第一个实参要是 compile_raw 交回的引擎键（str）",
        ));
    };
    let Some(text) = args.get(1).and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "match_raw: 第二个实参要是 str"));
    };
    let kind = args
        .get(2)
        .and_then(|value| instance.text_of(*value))
        .unwrap_or("search");
    let built = engine_for_key(instance, &key)?;
    let caps = built
        .captures_from(text, 0)
        .map_err(|error| instance.raise_builtin_error("ValueError", &error))?;
    let matched = caps.filter(|spans| {
        let whole = spans[0].expect("第 0 组一定有");
        match kind {
            "match" => whole.0 == 0,
            "fullmatch" => whole.0 == 0 && whole.1 == text.len(),
            _ => true,
        }
    });
    let Some(caps) = matched else {
        return Ok(instance.retain(instance.singletons().none()));
    };
    // **字符偏移**（第 580 轮真 bug 修 ✓）：`regex` crate 给的是**字节**偏移 ✗，
    // 而参照 `re` 的 `span()`／`start()`／`end()` 全是**字符**偏移 ✓ ⇒ 非 ASCII 会整片错位 ✓
    // （实测：`\w+` 对 `"αβγ δ"` ⇒ 参照 `(0,3)`／我们旧码 `(0,6)` ✗）。
    // **两档引擎同口径** ✓（task-17）：中间形状都是**字节**跨度 ✓ ⇒ 换算只写这一处 ✓。
    let mut rendered = String::new();
    for (index, group) in caps.iter().enumerate() {
        if index > 0 {
            rendered.push(';');
        }
        match group {
            Some((start, end)) => rendered.push_str(&format!(
                "{},{}",
                char_offset(text, *start),
                char_offset(text, *end)
            )),
            None => rendered.push_str("-1,-1"),
        }
    }
    Ok(instance.new_str(&rendered))
}


// ==== `re.Pattern`／`re.Match`（第 582 轮 ✓）====================================================
//
// **为什么在 Rust 侧** ✓：`SPEC-c-modules.md:470` 写明「`_sre`（`re` 没有纯 Python 备份）必须用 Rust 重写」✓
// ⇒ 两个类与它们的方法都建在这里 ✓，**仍然不新增载荷类型** ✗：数据放在**对象自己的实例属性字典**里
// （每实例 ✓ —— task-10 起；先前是按对象地址索引的进程级静态表 ✗，见"状态归属"一节 ✓）。
//
// **如实范围** ✗（本轮的先落面）：Pattern 先给 `match`／`search`／`fullmatch` ✓、Match 先给
// `span`／`start`／`end` ✓；`group`／`groups`／`groupdict`／`findall`／`finditer`／`split`／`sub`／`subn`
// 与 `_sre.template` 随后补 ✓（`re` 要全了才算通 ✓）。`pos`／`endpos` 实参暂按"不切片"处理 ✗（随后补 ✓）。
// 状态随对象释放 ⇒ 不再有"表项不回收"的问题 ✓（task-10 ✓）。

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
    // **`m[g]`**（round 131，PEP 里 `Match.__getitem__` 是公开协议 ✓）：与 `m.group(g)` 同口径 ✓。
    ("__getitem__", match_getitem_native as NativeFn),
    ("__repr__", match_repr_native as NativeFn),
    // 参照里 `str(m) == repr(m)` ✓（`object.__str__` 回落到 `__repr__` ✓）——我们的回落没走这条路 ✗
    // ⇒ 两个名字挂**同一个原生** ✓（一处真相 ✓，不为 `print` 另写一份 ✓）。
    ("__str__", match_repr_native as NativeFn),
];

// ==== 状态归属（task-10：`_sre` 的五张进程级表 → 每实例）=========================================
//
// **真 bug（先于 task-10 存在）** ✗：先前这里有三张**进程级** `static Mutex` 对象表
// （`PATTERNS`／`MATCHES`／`TEMPLATES`）、一张**类型对象地址**表（`CLASSES`）和一个**无界**引擎
// `REGISTRY`。同进程**第二个** `Instance` 首次编译正则时，`CLASSES` 会命中**第一个实例**的
// `re.Pattern` 类型（它的 MRO 指回第一个实例的 `object`）⇒ `call_callable` 无限递归／栈溢出 ✗。
//
// **一处真相** ✓：实例状态一律**挂在 Python 对象自身**（`re.Pattern`／`re.Match`／`_sre.Template`
// 的实例属性字典；类对象则由**本实例的类型注册表**管） ⇒ 随实例生死，跨实例天然不串 ✓：
//
// - `CLASSES` → 删表；`ensure_class` 走 `instance.type_named(name)`（每个实例各有自己的类型注册表 ✓）。
//   这与 `posix_module.rs` 的 `stat_result_type`（round 727 的同款修复 ✓）是**同一手法** ✓。
// - `PATTERNS` → 删表；引擎键／bytes 标志／`groupindex`／`groups` 都在 `re.Pattern` 的实例属性字典里 ✓
//   （`Engine` 是 Rust 载荷、进不了 Python 对象 ⇒ 只把**纯值键**放进对象，引擎本身走下面的纯值缓存 ✓）。
// - `MATCHES` → 删表；跨度在 `re.Match` 的隐藏属性里（`__pyawa_sre_spans__`），主语／模式对象本来就在
//   可见属性 `string`／`re` 里 ✓。
// - `TEMPLATES` → 删表；`_sre.Template` 本来就有一份可见属性 `_parsed`（`template_native` 写的 ✓）。
// - `REGISTRY` → 删表；`compile_raw` 现在返回**纯值引擎键**（`str`），`match_raw` 按该键从纯值缓存取引擎 ✓。
//
// **唯一保留的进程级结构**是 [`ENGINE_CACHE`]：它是**纯值缓存**（键／值都不含 Python 对象或实例
// 指针，有界 FIFO）⇒ 共享在语义上不可观察、也不可能串对象 ✓（理由与边界见该 `static` 的注释 ✓）。

/// `re.Pattern` 实例属性字典里的**引擎键**隐藏属性（纯值：`"<flags>\u{1f}<规范化模式>"`）。
/// 不在 `dir()`／`__all__` 的语义面里；可观察偏差只到 `vars(pattern)`（与 `_signal` 的隐藏键同款口径 ✓）。
const ENGINE_KEY_ATTRIBUTE: &str = "__pyawa_sre_engine_key__";

/// `re.Match` 实例属性字典里的**跨度表**隐藏属性（`list`，`[s0, e0, s1, e1, …]`，字符下标 ✓）。
const SPANS_ATTRIBUTE: &str = "__pyawa_sre_spans__";

/// 一个 `re.Pattern` 实例的取数结果（**借用**；对象活着 ⇒ 数据活着 ✓）。
struct PatternState {
    /// 该模式对象本身 ✓（`Match.re` 要用 ✓）。
    object: NonNull<Header>,
    /// 从纯值缓存取到的引擎（clone ✓）。
    engine: Engine,
    /// **bytes 模式** ✓（第 722 轮 ✓）：pattern 是 `bytes` ⇒ 主语也必须是 `bytes` ✓，
    /// 且所有产出（组文本／`findall`／`split`／`sub`）都交回 `bytes` ✓。
    is_bytes: bool,
}

/// 一次匹配的数据（**借用** `re.Match` 对象自己的属性；不另存旁表 ✓）。
struct MatchData {
    spans: Vec<(i64, i64)>,
    /// 原始主语对象（与 `Match.string` 是同一个对象 ✓，借用）。
    subject: NonNull<Header>,
    /// 对应 `re.Pattern` 对象（与 `Match.re` 是同一个对象 ✓，借用）——按名字取组用 ✓。
    pattern: NonNull<Header>,
    /// 本匹配来自 **bytes 模式** ✓ ⇒ `group()` 一族交回 `bytes` ✓。
    is_bytes: bool,
}

/// 取对象的**实例属性字典**（我们的 `Pattern`／`Match` 都是带内联字典的普通实例 ✓）。
fn instance_attributes(instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
    pyawa_core::mounted_instance_dict(instance, object)
}

/// 读 `re.Pattern` 的**引擎键**（借用 ⇒ 复制成 `String`，出锁后用 ✓）。
fn pattern_engine_key(instance: &Instance, pattern: NonNull<Header>) -> Option<String> {
    let attributes = instance_attributes(instance, pattern)?;
    let key = instance.dict_get(attributes, ENGINE_KEY_ATTRIBUTE)?;
    instance.text_of(key).map(str::to_owned)
}

/// 读 `re.Pattern` 的**bytes 标志**（由可见属性 `pattern` 的类型判定 ✓ —— 与 `pattern_source`
/// 收进来的两种载荷一一对应 ✓：`str` ⇒ Unicode 模式；`bytes` ⇒ bytes 模式）。
fn pattern_is_bytes(instance: &Instance, pattern: NonNull<Header>) -> bool {
    let Some(attributes) = instance_attributes(instance, pattern) else { return false };
    let Some(pattern_value) = instance.dict_get(attributes, "pattern") else { return false };
    instance.text_of(pattern_value).is_none() && instance.bytes_value(pattern_value).is_some()
}

/// 读 `re.Pattern` 的 `groupindex`（可见属性 `{名字: 组号}` ✓ —— `re/_compiler.py` 给的那份 ✓）。
fn pattern_groupindex(instance: &Instance, pattern: NonNull<Header>) -> Vec<(String, i64)> {
    let Some(attributes) = instance_attributes(instance, pattern) else { return Vec::new() };
    let Some(mapping) = instance.dict_get(attributes, "groupindex") else { return Vec::new() };
    instance
        .dict_entries(mapping)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, value)| Some((instance.text_of(key)?.to_owned(), instance.int_value(value)?)))
        .collect()
}

/// 取一个 `re.Pattern` 实例的取数结果（`bound` 是本实例 ✓）。
fn pattern_state(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<PatternState, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "需要 re.Pattern 实例"));
    };
    let Some(key) = pattern_engine_key(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "不是已编译的 re.Pattern 实例"));
    };
    let engine = engine_for_key(instance, &key)?;
    Ok(PatternState { object: this, engine, is_bytes: pattern_is_bytes(instance, this) })
}

/// 建 `re.Pattern`／`re.Match`／`_sre.Template` 类（幂等 ✓，**每实例一份** ✓），方法挂进类命名空间 ✓。
///
/// **一处真相** ✓：类对象的登记／查找由**本实例的类型注册表**负责（`new_type` 的 `types_by_name`
/// 先登记者优先 ✓）⇒ 先 `type_named` 查、没有才建 ✓ —— 不再有进程级 `CLASSES` 表 ✗。
/// 类型被孤儿回收时（`Instance::reclaim_orphan_user_types` 会同步摘掉名字索引 ✓）这里会重建一个 ✓；
/// 反过来，只要还有活实例／`re` 模块的 `Pattern` 引用，类就不会被回收 ✓。
fn ensure_class(
    instance: &Instance,
    name: &'static str,
    methods: &[(&str, NativeFn)],
) -> Option<NonNull<Header>> {
    if let Some(existing) = instance.type_named(name) {
        return Some(existing.cast::<Header>());
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
    Some(class)
}

/// 造一个类的实例（**不建**实例字典 —— 调用方给的那份 `attributes` 就是实例字典 ✓）。
fn new_instance_with(
    instance: &Instance,
    class_name: &str,
    attributes: NonNull<Header>,
) -> Option<NonNull<Header>> {
    let class = instance.type_named(class_name)?.cast::<Header>();
    let ty = class.cast::<pyawa_core::TypeObject>();
    let object = instance.alloc(pyawa_core::AttributeObject::new(
        ty,
        core::cell::RefCell::new(Some(attributes)),
    ));
    Some(object.into_raw().cast::<Header>())
}

/// **模式的源文本** ✓（第 722 轮 ✓）：`str` 直接用 ✓；`bytes` 按 **latin-1** 解码 ✓
/// （字节 ↔ `U+0000`–`U+00FF` 一一对应 ✓）并标记 bytes 模式 ✓ —— 产出交回 `bytes` 的判据就靠这个标记 ✓。
fn pattern_source(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<(String, bool), ExecError> {
    if let Some(text) = instance.text_of(value) {
        return Ok((text.to_owned(), false));
    }
    if let Some(bytes) = instance.bytes_value(value) {
        return Ok((decode_subject(bytes), true));
    }
    Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str 或 bytes"))
}

/// **bytes ↔ latin-1 直通** ✓（第 722 轮 ✓）：字节 `0xNN` ↔ 字符 `U+00NN` 一一对应 ✓
/// ⇒ 解码后的**字符偏移就是字节偏移** ✓（跨度不用换算 ✓），且往返无损 ✓。
fn decode_subject(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| *byte as char).collect()
}

/// latin-1 文本 ⇒ 原字节 ✓（只对 [`decode_subject`] 解出来的文本用 ✓）。
fn encode_text(text: &str) -> Vec<u8> {
    text.chars().map(|character| character as u8).collect()
}

/// **孤立代理的安全映射**（round 107）：正则引擎只认合法 UTF-8 的 `String` ✗，而 `str` 允许
/// 孤立代理（WTF-8 表示 ✓）。做法：把代理码点 `U+D800..=U+DFFF` 映射到**私用平面 15 的哨兵**
/// `U+F0000..=U+F07FF`（合法标量、与常规文本几乎不会冲突 ✓），匹配完再逆映射回 WTF-8 ✓。
/// 长度不变（一个码点 ↔ 一个 `char` ✓）⇒ 匹配下标语义不受影响 ✓。
fn wtf8_to_sentinel_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for point in pyawa_core::wtf8::code_points(bytes) {
        let code = point.unwrap_or(0xFFFD);
        if (0xD800..=0xDFFF).contains(&code) {
            // 0xF0000 + (code - 0xD800) <= 0xF07FF 是合法标量 ✓。
            out.push(char::from_u32(0xF0000 + (code - 0xD800)).expect("私用平面码点合法"));
        } else {
            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
        }
    }
    out
}

/// 哨兵串 → **WTF-8 字节**（逆映射 ✓；非法序列按替换字符处理 ✓）。
fn sentinel_string_to_wtf8(text: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(text.len());
    for character in text.chars() {
        let code = character as u32;
        if (0xF0000..=0xF07FF).contains(&code) {
            let surrogate = 0xD800 + (code - 0xF0000);
            let _ = pyawa_core::wtf8::encode_code_point(surrogate, &mut out);
        } else {
            let _ = pyawa_core::wtf8::encode_code_point(code, &mut out);
        }
    }
    out
}

/// 按模式的 bytes 口径**造产出对象** ✓（bytes 模式 ⇒ `bytes` ✓，否则 `str` ✓）。
fn text_object(instance: &Instance, text: &str, is_bytes: bool) -> NonNull<Header> {
    if is_bytes {
        instance.new_bytes(&encode_text(text))
    } else {
        let bytes = sentinel_string_to_wtf8(text);
        instance
            .new_str_wtf8(&bytes)
            .unwrap_or_else(|| instance.new_str(text))
    }
}

/// 取**主语** ✓（第 722 轮 ✓）：bytes 模式只收 bytes-like ✓、str 模式只收 str ✓
/// —— 与参照的 `cannot use a bytes pattern on a string-like object` 同规 ✓。
/// 返回（latin-1 解码后的文本 ✓，原始主语对象 ✓）。
fn subject_text(
    instance: &Instance,
    is_bytes: bool,
    value: NonNull<Header>,
) -> Result<(String, NonNull<Header>), ExecError> {
    if is_bytes {
        match instance.bytes_value(value) {
            Some(bytes) => Ok((decode_subject(bytes), value)),
            None => Err(instance.raise_builtin_error(
                "TypeError",
                "cannot use a bytes pattern on a string-like object",
            )),
        }
    } else {
        match instance.text_of(value) {
            Some(text) => Ok((text.to_owned(), value)),
            // **孤立代理的 str**（round 107）：按哨兵映射成合法 `String` 再匹配 ✓
            //（`glob.escape` 等对含 \udcff 的路径要用 ✓ —— 官方 `test_unicode_file` ✓）。
            None => match instance.wtf8_of(value) {
                Some(bytes) => Ok((wtf8_to_sentinel_string(bytes), value)),
                None => Err(instance.raise_builtin_error(
                    "TypeError",
                    "cannot use a string pattern on a bytes-like object",
                )),
            },
        }
    }
}

/// 跑一次匹配 ✓（`kind`: `match`／`fullmatch`／`search` ✓）⇒ 跨度（**字符**下标 ✓）。
///
/// 两档引擎共用 ✓（task-17）：`Engine::captures_from` 交回的字节跨度在这里唯一地换算成字符下标 ✓；
/// `start` 是**字节**起点 ✓、`haystack` 已按 `endpos` 截尾 ✓ ⇒ `^`／`\b`／look-behind 仍看**整串上下文** ✓。
fn run_match(
    built: &Engine,
    haystack: &str,
    start: usize,
    kind: &str,
) -> Result<Option<Vec<(i64, i64)>>, String> {
    let Some(caps) = built.captures_from(haystack, start)? else {
        return Ok(None);
    };
    let whole = caps[0].expect("第 0 组一定有");
    let ok = match kind {
        "match" => whole.0 == start,
        "fullmatch" => whole.0 == start && whole.1 == haystack.len(),
        _ => true,
    };
    if !ok {
        return Ok(None);
    }
    Ok(Some(
        caps.iter()
            .map(|group| match group {
                Some((start, end)) => (char_offset(haystack, *start), char_offset(haystack, *end)),
                None => (-1, -1),
            })
            .collect(),
    ))
}

/// 把一次匹配装成 `re.Match` 实例 ✓（**数据放进对象自己的属性字典**，不另存旁表 ✓）。
///
/// 隐藏属性 `__pyawa_sre_spans__` 是**纯值**（`list[int]`）；主语／模式对象分别就是可见属性
/// `string`／`re`（一处真相 ✓，`group()` 切片与按名字取组都从它们还原 ✓）。
fn make_match(
    instance: &Instance,
    pattern: NonNull<Header>,
    subject: NonNull<Header>,
    spans: Vec<(i64, i64)>,
    pos: i64,
    endpos: i64,
) -> Result<NonNull<Header>, ExecError> {
    ensure_class(instance, "re.Match", MATCH_METHODS)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "re.Match 未登记"))?;
    // **用户可见的属性**（第 592 轮）✓：`string`／`re`／`pos`／`endpos` ✓（挂实例字典 ✓ 同 `Pattern` ✓）
    let attributes = instance.new_dict();
    // `Match.string` 交回**原始主语对象** ✓（bytes 模式 ⇒ `bytes` ✓ —— 与参照一致 ✓）。
    let text_value = instance.retain(subject);
    instance.dict_set(attributes, "string", text_value);
    let pos_value = instance.new_int(pos);
    instance.dict_set(attributes, "pos", pos_value);
    let endpos_value = instance.new_int(endpos);
    instance.dict_set(attributes, "endpos", endpos_value);
    let kept = instance.retain(pattern);
    instance.dict_set(attributes, "re", kept);
    // **`lastindex`／`lastgroup`**（第 595 轮）✓：最后一个**匹配上的**捕获组 ✓（参照语义 ✓）
    let mut lastindex: i64 = -1;
    for (index, (start, _)) in spans.iter().enumerate().skip(1) {
        if *start >= 0 {
            lastindex = index as i64;
        }
    }
    let groupindex_of_pattern = pattern_groupindex(instance, pattern);
    let lastgroup = groupindex_of_pattern
        .iter()
        .find(|(_, index)| *index == lastindex)
        .map(|(name, _)| name.clone());
    match lastindex >= 0 {
        true => {
            let value = instance.new_int(lastindex);
            instance.dict_set(attributes, "lastindex", value);
        }
        false => {
            let none = instance.retain(instance.singletons().none());
            instance.dict_set(attributes, "lastindex", none);
        }
    }
    match lastgroup {
        Some(name) => {
            let value = instance.new_str(&name);
            instance.dict_set(attributes, "lastgroup", value);
        }
        None => {
            let none = instance.retain(instance.singletons().none());
            instance.dict_set(attributes, "lastgroup", none);
        }
    }
    // **跨度表（隐藏属性，纯值）**：`[s0, e0, s1, e1, …]`（字符下标；未匹配组写 `-1, -1` ✓）。
    let mut flat: Vec<NonNull<Header>> = Vec::with_capacity(spans.len() * 2);
    for (start, end) in &spans {
        flat.push(instance.new_int(*start));
        flat.push(instance.new_int(*end));
    }
    let spans_value = instance.new_list(flat);
    // **接管**那份新引用（`dict_set` 是"借用＋自己加一份" ⇒ 这里若用它就会多留一份 ⇒ 用 owned 入口 ✓）。
    instance.dict_set_owned_by_name(attributes, SPANS_ATTRIBUTE, spans_value);
    let object = new_instance_with(instance, "re.Match", attributes)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "re.Match 未登记"))?;
    Ok(object)
}

/// `Pattern.<kind>(string, pos=0, endpos=…) -> Match | None` ✓（`pos`／`endpos` 暂不切片 ✗，随后补 ✓）。
fn pattern_kind_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kind: &str,
) -> Result<NonNull<Header>, ExecError> {
    let state = pattern_state(instance, bound)?;
    let Some(subject) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str"));
    };
    let (text, subject) = subject_text(instance, state.is_bytes, subject)?;
    let (window, start, pos, endpos) = match_window(instance, &text, args);
    match run_match(&state.engine, window, start, kind) {
        // 跨度已是**整串**下标 ✓（窗口是整串前缀 ✓ ⇒ 字符下标不用平移 ✓）。
        Ok(Some(spans)) => make_match(instance, state.object, subject, spans, pos, endpos),
        Ok(None) => Ok(instance.retain(instance.singletons().none())),
        Err(error) => Err(instance.raise_builtin_error("ValueError", &error)),
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
    let data = match_data(instance, bound)?;
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



/// 扫出**全部**匹配的跨度 ✓（`Vec[0]` 是整体 ✓；空匹配由共用游标推进 ✓）。
///
/// 两档引擎共用 ✓（task-17）：`Engine::scan_from` 交回的字节跨度在这里唯一地换算成字符下标 ✓。
/// `start` 是**字节**起点 ✓（`sub`／`split` 一律传 0 ✓）。
fn scan_spans(built: &Engine, haystack: &str, start: usize) -> Result<Vec<Vec<(i64, i64)>>, String> {
    Ok(built
        .scan_from(haystack, start)?
        .into_iter()
        .map(|caps| {
            caps.into_iter()
                .map(|group| match group {
                    Some((start, end)) => (char_offset(haystack, start), char_offset(haystack, end)),
                    None => (-1, -1),
                })
                .collect()
        })
        .collect())
}

/// `Pattern.findall(string) -> list` ✓（无组 ⇒ 整体串 ✓；一组 ⇒ 该组串 ✓；多组 ⇒ 元组 ✓；
/// 未匹配的组按参照写**空串** ✗ 不是 `None` ✓）。
fn pattern_findall_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let state = pattern_state(instance, bound)?;
    let is_bytes = state.is_bytes;
    let Some(subject) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str"));
    };
    let (text, _subject) = subject_text(instance, is_bytes, subject)?;
    // **截尾前缀 ＋ 引擎 pos** ✓（task-17 第 2 轮）：跨度直接就是**整串**字符下标 ✓ ⇒ 不再平移 ✓；
    // 切片一律用**原文** ✓（第 585 轮踩过"把 text 覆盖成窗口"的坑 ✗）。
    let (window, start, _pos, _endpos) = match_window(instance, &text, args);
    let groups = state.engine.captures_len().saturating_sub(1);
    let mut items: Vec<NonNull<Header>> = Vec::new();
    for spans in scan_spans(&state.engine, window, start)
        .map_err(|error| instance.raise_builtin_error("ValueError", &error))?
    {
        if groups == 0 {
            let (start, end) = spans[0];
            items.push(text_object(instance, &slice_chars(&text, start, end), is_bytes));
        } else if groups == 1 {
            let (start, end) = spans[1];
            let piece = if start < 0 { String::new() } else { slice_chars(&text, start, end) };
            items.push(text_object(instance, &piece, is_bytes));
        } else {
            let mut row: Vec<NonNull<Header>> = Vec::with_capacity(groups);
            for (start, end) in spans.iter().skip(1) {
                let piece = if *start < 0 { String::new() } else { slice_chars(&text, *start, *end) };
                row.push(text_object(instance, &piece, is_bytes));
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
    let groupindex = pattern_groupindex(instance, data.pattern);
    let result = instance.new_dict();
    for (name, index) in groupindex {
        let value = match group_text(instance, &data, index) {
            Some(text) => text_object(instance, &text, data.is_bytes),
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
/// **相对整串**的 ✓ ⇒ 返回"截至 `endpos` 的**整串前缀** ＋ 起点的字节偏移" ✓。
///
/// **不能把 `pos` 之前的前缀切掉** ✗（task-17 第 2 轮真 bug 修 ✗）：`^`／`\b`／look-behind 要看
/// `pos` **之前**的上下文 ✓（实测 `(?<=a)b` 对 `"xabxa"` 的 `search(…, 2)` ⇒ 参照 `(2,3)` ✓
/// 而切片法 `None` ✗；`^b`／`\bb` 的 `match(…, 1)` 参照 `None` ✓ 而切片法**误报** ✗）。
/// ⇒ 只**截尾** ✓，起点交给引擎自己的 pos 接口 ✓（`\A`／`^` 仍只认真串首 ✓）。
///
/// 返回：`(截至 endpos 的前缀 ✓，pos 的字节偏移 ✓，pos 的字符下标 ✓，endpos 的字符下标 ✓)`。
fn match_window<'a>(
    instance: &Instance,
    text: &'a str,
    args: &[NonNull<Header>],
) -> (&'a str, usize, i64, i64) {
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
    (&text[..char_to_byte(text, endpos)], char_to_byte(text, pos), pos, endpos)
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
    pattern_groupindex(instance, data.pattern)
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, index)| *index)
        .ok_or_else(|| instance.raise_builtin_error("IndexError", "no such group"))
}

/// 取一组的**文本** ✓（未匹配 ⇒ `None` ✓）。
///
/// **一处真相** ✓：从 `re.Match` 自己的可见属性还原 —— 主语取 `Match.string`（`data.subject`），
/// bytes 模式按原字节切片（span 就是字节下标 ✓），str 模式按字符切片（孤立代理走哨兵映射 ✓）。
fn group_text(instance: &Instance, data: &MatchData, index: i64) -> Option<String> {
    let span = *data.spans.get(usize::try_from(index).ok()?)?;
    if span.0 < 0 {
        return None;
    }
    if data.is_bytes {
        let bytes = instance.bytes_value(data.subject)?;
        let start = usize::try_from(span.0).ok()?;
        let end = usize::try_from(span.1).ok()?;
        return Some(decode_subject(bytes.get(start..end)?));
    }
    // str 模式：合法 UTF-8 直接按字符切；孤立代理的 str 先映射成哨兵串再切（round 107 同口径 ✓）。
    match instance.text_of(data.subject) {
        Some(text) => Some(slice_chars(text, span.0, span.1)),
        None => {
            let text = wtf8_to_sentinel_string(instance.wtf8_of(data.subject)?);
            Some(slice_chars(&text, span.0, span.1))
        }
    }
}




/// `Pattern.finditer(string, pos=0, endpos=len) -> iterator` ✓（第 589 轮）。
///
/// 造一批 `re.Match` ✓ 再用 core 的公共入口 `iter_value` 包成**真迭代器** ✓
/// （`itertools` 也在用同一个入口 ✓ ⇒ 不是新通道 ✓）；窗口与 `findall` 同款 ✓：
/// **截尾前缀 ＋ 引擎 pos** ✓（task-17 第 2 轮），`Match.string` 存**原文** ✓ ⇒ `group()` 切片才对 ✓。
fn pattern_finditer_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let state = pattern_state(instance, bound)?;
    let is_bytes = state.is_bytes;
    let Some(subject) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str"));
    };
    let (text, subject) = subject_text(instance, is_bytes, subject)?;
    let (window, start, pos, endpos) = match_window(instance, &text, args);
    let mut items: Vec<NonNull<Header>> = Vec::new();
    for spans in scan_spans(&state.engine, window, start)
        .map_err(|error| instance.raise_builtin_error("ValueError", &error))?
    {
        items.push(make_match(instance, state.object, subject, spans, pos, endpos)?);
    }
    let list = instance.new_list(items);
    pyawa_core::executor::iter::iter_value(instance, list)
}


/// `Match.expand(template) -> str` ✓（第 590 轮）：按本匹配展开模板 ✓（语义与 `sub` 的模板同一处 ✓）。
/// **`re.Match.__getitem__(key)`**（round 131）：参照口径与 `group(key)` 一致 ✓
/// （int／str 分组名都收；其它类型由 `group` 自己报错 ✓）。
fn match_getitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // **快路径（round 137 热点）**：`m[整数]` 直接取组 —— `Lib/ntpath.expandvars` 的
    // replacement 对每个匹配都做 `m[lastindex]`（100k 次 ⇒ 每微秒都算数 ✓）。走通用
    // `match_group_native` 要过"参数解析/可变参准备/组解析"三道，这里直取 `group_text` ✓。
    if args.len() == 1 && kwargs.is_empty() {
        if let Some(index) = instance.int_of(args[0]).and_then(|value| value.to_i64()) {
            let data = match_data(instance, bound)?;
            return match group_text(instance, &data, index) {
                Some(text) => Ok(text_object(instance, &text, data.is_bytes)),
                None => Ok(instance.retain(instance.singletons().none())),
            };
        }
    }
    match_group_native(instance, bound, args, kwargs)
}

fn match_expand_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let data = match_data(instance, bound)?;
    let Some(template_object) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "expand: 实参要是 str"));
    };
    let (template, _) = subject_text(instance, data.is_bytes, template_object)?;
    let groupindex = pattern_groupindex(instance, data.pattern);
    let rendered = expand_template(instance, &data, &groupindex, &template)?;
    Ok(text_object(instance, &rendered, data.is_bytes))
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
            out.push_str(&group_text(instance, data, index).unwrap_or_default());
        } else if escape.is_ascii_digit() {
            let mut digits = escape.to_string();
            if let Some(next) = chars.peek().copied() {
                if next.is_ascii_digit() {
                    digits.push(next);
                    chars.next();
                }
            }
            let index = digits.parse::<i64>().unwrap_or(0);
            out.push_str(&group_text(instance, data, index).unwrap_or_default());
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

/// `sub`／`subn` 共用 ✓：返回（新串 ✓，替换次数 ✓，是否 bytes 模式 ✓）。
fn substitute(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
) -> Result<(String, i64, bool), ExecError> {
    let state = pattern_state(instance, bound)?;
    let is_bytes = state.is_bytes;
    let groupindex = pattern_groupindex(instance, state.object);
    // 替换可以是**字符串模板** ✓ 也可以是**可调用对象** ✓（`re.sub` 两种都收 ✓，第 588 轮）；
    // **bytes 模式**下模板必须同口径（bytes ✓）—— 不同口径的 str／bytes 一律当**可调用**试 ✗（不是可调用就由调用期报错 ✓）。
    let repl = args.first().copied();
    let mut template: Option<String> = None;
    let mut callable: Option<NonNull<Header>> = None;
    match repl {
        Some(value) => {
            if is_bytes {
                match instance.bytes_value(value) {
                    Some(bytes) => template = Some(decode_subject(bytes)),
                    None => callable = Some(value),
                }
            } else {
                match instance.text_of(value) {
                    Some(text) => template = Some(text.to_owned()),
                    None => callable = Some(value),
                }
            }
        }
        None => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "sub() missing required argument 'repl' (pos 1)",
            ))
        }
    }
    let Some(subject_object) = args.get(1).copied() else {
        return Err(instance.raise_builtin_error("TypeError", "第二个实参要是 str"));
    };
    let (text, _subject) = subject_text(instance, is_bytes, subject_object)?;
    let count = args.get(2).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let mut out = String::new();
    let mut last = 0_i64;
    let mut replaced = 0_i64;
    for spans in scan_spans(&state.engine, &text, 0)
        .map_err(|error| instance.raise_builtin_error("ValueError", &error))?
    {
        if count > 0 && replaced >= count {
            break;
        }
        let (start, end) = spans[0];
        out.push_str(&slice_chars(&text, last, start));
        let data = MatchData {
            spans: spans.clone(),
            subject: subject_object,
            pattern: state.object,
            is_bytes,
        };
        let piece = match &template {
            Some(template) => expand_template(instance, &data, &groupindex, template)?,
            None => {
                let Some(callable) = callable else {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "替换要是字符串或可调用对象",
                    ));
                };
                // 可调用替换 ✓：造一个 `re.Match` 交给它 ✓（与参照一致 ✓）
                let matched = make_match(
                    instance,
                    state.object,
                    subject_object,
                    spans.clone(),
                    0,
                    text.chars().count() as i64,
                )?;
                let outcome = pyawa_core::executor::call::call_value(
                    instance,
                    callable,
                    &[matched],
                    &[],
                );
                // 我们持有 `matched` 那一份（`call_value` 若需要会自己 incref ✓）
                instance.release(matched);
                let result = outcome?;
                let rendered = if is_bytes {
                    match instance.bytes_value(result) {
                        Some(bytes) => decode_subject(bytes),
                        None => {
                            instance.release(result);
                            return Err(instance.raise_builtin_error(
                                "TypeError",
                                "替换函数必须返回 bytes",
                            ));
                        }
                    }
                } else {
                    match instance.text_of(result) {
                        Some(rendered) => rendered.to_owned(),
                        None => {
                            instance.release(result);
                            return Err(instance.raise_builtin_error(
                                "TypeError",
                                "替换函数必须返回 str",
                            ));
                        }
                    }
                };
                instance.release(result);
                rendered
            }
        };
        out.push_str(&piece);
        last = end;
        replaced += 1;
    }
    out.push_str(&slice_chars(&text, last, text.chars().count() as i64));
    Ok((out, replaced, is_bytes))
}

/// `Pattern.sub(repl, string, count=0) -> str | bytes` ✓。
fn pattern_sub_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (text, _count, is_bytes) = substitute(instance, bound, args)?;
    Ok(text_object(instance, &text, is_bytes))
}

/// `Pattern.subn(repl, string, count=0) -> (str | bytes, int)` ✓。
fn pattern_subn_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (text, count, is_bytes) = substitute(instance, bound, args)?;
    let value = text_object(instance, &text, is_bytes);
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
    let state = pattern_state(instance, bound)?;
    let is_bytes = state.is_bytes;
    let Some(subject) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "第一个实参要是 str"));
    };
    let (text, _subject) = subject_text(instance, is_bytes, subject)?;
    let maxsplit = args.get(1).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let mut last = 0_i64;
    let mut cuts = 0_i64;
    for spans in scan_spans(&state.engine, &text, 0)
        .map_err(|error| instance.raise_builtin_error("ValueError", &error))?
    {
        if maxsplit > 0 && cuts >= maxsplit {
            break;
        }
        let (start, end) = spans[0];
        items.push(text_object(instance, &slice_chars(&text, last, start), is_bytes));
        for (group_start, group_end) in spans.iter().skip(1) {
            if *group_start < 0 {
                items.push(instance.retain(instance.singletons().none()));
            } else {
                items.push(text_object(
                    instance,
                    &slice_chars(&text, *group_start, *group_end),
                    is_bytes,
                ));
            }
        }
        last = end;
        cuts += 1;
    }
    items.push(text_object(
        instance,
        &slice_chars(&text, last, text.chars().count() as i64),
        is_bytes,
    ));
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
        match group_text(instance, &data, index) {
            Some(text) => values.push(text_object(instance, &text, data.is_bytes)),
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
        match group_text(instance, &data, index as i64) {
            Some(text) => values.push(text_object(instance, &text, data.is_bytes)),
            None => match default {
                Some(value) => values.push(instance.retain(value)),
                None => values.push(instance.retain(instance.singletons().none())),
            },
        }
    }
    Ok(instance.new_tuple(values))
}

/// 取 `MatchData`：**从 `re.Match` 对象自己的属性字典还原** ✓（不查进程级表 ✓）。
///
/// 隐藏属性 `__pyawa_sre_spans__` 是 `list[int]`（纯值）；主语／模式对象借可见属性 `string`／`re` ✓
/// （`this` 是调用期间的 `self` ⇒ 这两个对象由它保活 ✓，返回的 `MatchData` 只借用、不持引用 ✓）。
fn match_data(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<MatchData, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "需要 re.Match 实例"));
    };
    let Some(attributes) = instance_attributes(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "不是 re.Match 实例"));
    };
    let (Some(subject), Some(pattern), Some(spans_value)) = (
        instance.dict_get(attributes, "string"),
        instance.dict_get(attributes, "re"),
        instance.dict_get(attributes, SPANS_ATTRIBUTE),
    ) else {
        return Err(instance.raise_builtin_error("TypeError", "不是 re.Match 实例"));
    };
    let Some(items) = instance.list_items(spans_value) else {
        return Err(instance.raise_builtin_error("TypeError", "不是 re.Match 实例"));
    };
    let mut spans = Vec::with_capacity(items.len() / 2);
    for pair in items.chunks_exact(2) {
        let (Some(start), Some(end)) =
            (instance.int_value(pair[0]), instance.int_value(pair[1]))
        else {
            return Err(instance.raise_builtin_error("TypeError", "不是 re.Match 实例"));
        };
        spans.push((start, end));
    }
    Ok(MatchData {
        spans,
        subject,
        pattern,
        is_bytes: instance.bytes_value(subject).is_some(),
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
    let Some(pattern_object) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "compile: 第一个实参要是 str"));
    };
    // `_is_bytes`：bytes 模式由可见属性 `pattern` 的类型**当场判定**（`pattern_is_bytes` ✓）
    // ⇒ 不再另存一份标志 ✓（一处真相 ✓）。
    let (pattern, _is_bytes) = pattern_source(instance, pattern_object)?;
    let flags = args.get(1).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let groups = args.get(3).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    // **字符类规范化** ✓（第 722 轮）：同 `compile_raw` ✓（`re.compile` 走的是这一支 ✓）。
    let normalized = normalize_character_classes(&pattern);
    // **VERBOSE 自己剥** ✓（第 722 轮）：Rust 的 `ignore_whitespace` 会连**类内**空白一起吞 ✗。
    let normalized = if flags & SRE_FLAG_VERBOSE != 0 {
        strip_verbose_whitespace(&normalized)
    } else {
        normalized
    };
    let engine_key = engine_key(&normalized, flags);
    // **立刻编译**：编译失败与旧口径同一时点抛 `ValueError` ✓（引擎进纯值缓存 ✓）。
    let _ = engine_for_key(instance, &engine_key)?;
    // `groups`／`groupindex`（名字 ⇒ 组号 ✓）都留在**可见属性**里 ⇒ 按名字取组从对象自己读 ✓
    // （不再存进程级表 ✓）。
    ensure_class(instance, "re.Pattern", PATTERN_METHODS)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "re.Pattern 建类失败"))?;
    // **用户可见的属性**（第 591 轮）✓：`re/__init__.py` 会读 `Pattern.pattern`／`.flags` ✓
    // ⇒ 挂进**实例字典** ✓（本类的实例本来就有字典槽 ✓，属性通道自然读得到 ✓ 不需要新通道 ✓）。
    let attributes = instance.new_dict();
    // `.pattern` 交回**原始** pattern 对象 ✓（bytes 模式 ⇒ `bytes` ✓ —— 与参照一致 ✓）。
    let pattern_value = instance.retain(pattern_object);
    instance.dict_set(attributes, "pattern", pattern_value);
    let flags_value = instance.new_int(flags);
    instance.dict_set(attributes, "flags", flags_value);
    let groups_value = instance.new_int(groups);
    instance.dict_set(attributes, "groups", groups_value);
    match args.get(4) {
        Some(mapping) => instance.dict_set(attributes, "groupindex", *mapping),
        None => {
            let empty = instance.new_dict();
            instance.dict_set(attributes, "groupindex", empty);
        }
    }
    // **引擎键（隐藏属性，纯值）**：`pattern_state` 从它还原引擎 ✓（对象自带状态 ⇒ 跨实例不串 ✓）。
    let key_value = instance.new_str(&engine_key);
    instance.dict_set_owned_by_name(attributes, ENGINE_KEY_ATTRIBUTE, key_value);
    let object = new_instance_with(instance, "re.Pattern", attributes)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "re.Pattern 未登记"))?;
    Ok(object)
}


// ==== `_sre.template`（第 594 轮 ✓）==============================================================
//
// `re/__init__.py:375 _compile_template` ⇒ `_sre.template(pattern, _parser.parse_template(repl, pattern))` ✓
// —— 3.13+ 的 `re.sub` **必经**这一步 ✓：它返回一个**可调用对象** ✓，随后交给 `Pattern.sub` ✓。
// `parse_template` 的产物形状**已实测** ✓（本机 3.14 ✓）：`['[', 1, ']']` —— **字面量与组号交替的列表** ✓
// （转义已展开 ✓、名字已换成号 ✓ ⇒ 我们不必再解析字符串 ✓）。

/// **模板对象的解析列表**就存在它自己的实例字典里（`template_native` 写的可见属性 `_parsed` ✓）
/// —— task-10 删掉了原先按对象地址索引的进程级 `TEMPLATES` 表 ✓。

/// `Match.__repr__` ✓（第 596 轮）：参照形状 `<re.Match object; span=(s, e), match='文本'>` ✓
/// （未匹配到的那次调用到不了这里 ✓；文本用参照的**单引号**形状 ✓）。
fn match_repr_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let data = match_data(instance, bound)?;
    let (start, end) = data.spans.first().copied().unwrap_or((-1, -1));
    let text = if start < 0 {
        String::new()
    } else {
        group_text(instance, &data, 0).unwrap_or_default()
    };
    // 参照的 `repr`：能用单引号就用单引号 ✓；含单引号而**不含**双引号时改用双引号 ✓（`OM` 的 str repr 口径 ✓）
    let quoted = if text.contains('\'') && !text.contains('"') {
        format!("\"{text}\"")
    } else {
        format!("'{text}'")
    };
    Ok(instance.new_str(&format!(
        "<re.Match object; span=({start}, {end}), match={quoted}>"
    )))
}

/// `__call__(match) -> str` ✓：按解析列表拼串 ✓（字符串 ⇒ 字面量 ✓；整数 ⇒ 该组文本 ✓，未匹配 ⇒ 空串 ✓）。
fn template_call_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "模板对象要有 self"));
    };
    let parsed = instance_attributes(instance, this)
        .and_then(|attributes| instance.dict_get(attributes, "_parsed"));
    let Some(parsed) = parsed else {
        return Err(instance.raise_builtin_error("TypeError", "不是 _sre 模板对象"));
    };
    let Some(matched) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "模板要一个 re.Match"));
    };
    let data = match_data(instance, Some(matched))?;
    let items = instance
        .list_items(parsed)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "模板的解析结果要是 list"))?;
    let mut out = String::new();
    for item in items {
        if let Some(text) = instance.text_of(item) {
            out.push_str(text);
        } else if let Some(index) = instance.int_value(item) {
            out.push_str(&group_text(instance, &data, index).unwrap_or_default());
        } else {
            return Err(instance.raise_builtin_error("TypeError", "模板项要是 str 或 int"));
        }
    }
    Ok(instance.new_str(&out))
}

/// `template(pattern, parsed) -> Template` ✓（返回**可调用对象** ✓ —— `__call__` 已由第 593 轮接通 ✓）。
fn template_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(parsed) = args.get(1).copied() else {
        return Err(instance.raise_builtin_error("TypeError", "template: 缺 parsed"));
    };
    let attributes = instance.new_dict();
    instance.dict_set(attributes, "_parsed", parsed);
    ensure_class(instance, "_sre.Template", TEMPLATE_METHODS)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "_sre.Template 建类失败"))?;
    let object = new_instance_with(instance, "_sre.Template", attributes)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "_sre.Template 未登记"))?;
    Ok(object)
}

/// `_sre.Template` 的方法表 ✓。
const TEMPLATE_METHODS: &[(&str, NativeFn)] = &[("__call__", template_call_native as NativeFn)];

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
        ("template", template_native as NativeFn),
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
