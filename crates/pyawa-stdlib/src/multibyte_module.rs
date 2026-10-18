//! **`_multibytecodec`**：CJK 多字节编解码族的**共享引擎**（`_codecs_jp` 一族 ＋ `encodings/*` 的底座）。
//!
//! ## 为什么有这个模块
//!
//! CPython 的 `_codecs_jp`／`_codecs_cn`／`_codecs_kr`／`_codecs_tw` 是 **C 模块**
//! （`Modules/cjkcodecs/`），映射表在 C 里；`Lib/` 里拿不到。`Lib/encodings/cp932.py` 等
//! 逐字同步后，`import encodings.cp932` 会卡在 `No module named '_codecs_jp'`
//! ⇒ 表的**数据**从参照 `python3` dump、本模块实现**状态机**（同 `_sre` 的形态：数据与引擎分开）。
//!
//! ## 数据生成路径（**一处真相**）
//!
//! ```text
//! python3 tools/gen_codec_tables.py --family jp --emit
//!   ⇒ crates/pyawa-stdlib/src/codec_tables/jp.txt   （生成物，禁止手改）
//! ```
//!
//! 该文件由本模块 `include_str!` 读入，**首次用到时**解析成按码位／字节序列排序的数组。
//! 文本格式见生成脚本的模块 doc（`E`＝编码方向，`D`＝解码方向；两个方向**都 dump**，
//! 本模块不做反向推导 —— 反向会丢掉 cp932／big5 的重复映射与 JIS X 0213 的组合字符）。
//!
//! ## 本层真实现（照参照逐条对拍）
//!
//! - `MultibyteCodec.encode(input, errors='strict')` ⇒ `(bytes, 消耗码位数)`；
//!   `MultibyteCodec.decode(input, errors='strict')` ⇒ `(str, 消耗字节数)`
//!   （**返回形态照参照**：是二元组，不是裸 bytes／str ✓）。
//! - `errors=strict`：错误文案照参照（`'cp932' codec can't encode character '\u2603' in
//!   position 1: illegal multibyte sequence`；解码的 `incomplete`／`illegal` 分型与
//!   `bytes in position 0-1` 复数形态 ✓）。
//! - `errors=ignore`：按**错误单元**跳过（非法字节 1 个；输入末尾截断的多字节前缀整段跳）；
//!   `errors=replace`：非法字节每个替换成 `\uFFFD`，末尾截断前缀整段替换成一个 `\uFFFD`
//!   （均照参照实测口径 ✓）。
//! - `MultibyteIncrementalEncoder`／`MultibyteIncrementalDecoder`：真实现（缓冲尾部截断字节、
//!   `final` 语义、`reset`／`getstate`／`setstate`）。
//! - `MultibyteStreamReader`／`MultibyteStreamWriter`：**空基类** ✓ —— `Lib/encodings/*.py` 的
//!   `class StreamReader(Codec, mbc.MultibyteStreamReader, codecs.StreamReader)` 里，
//!   `read`／`write` 由 `codecs.StreamReader`／`StreamWriter`（纯 Python）按 `self.decode`／
//!   `self.encode` 的元组契约实现 ⇒ 这里不需要重复实现 ✓（照参照的 MRO 逐字）。
//!
//! ## 已落族
//!
//! - **`_codecs_jp`**（round 10）：7 个 `Lib/encodings/*` 模块 —— `cp932`／`euc_jp`／
//!   `euc_jis_2004`／`euc_jisx0213`／`shift_jis`／`shift_jis_2004`／`shift_jisx0213`；
//! - **`_codecs_cn`**（round 11）：2 个 —— `gb2312`／`gbk`（`cp936` 是 `gbk` 的别名，不另设模块 ✓）。
//!
//! 数据文件：`codec_tables/jp.txt`／`codec_tables/cn.txt`，同一条生成命令分别 `--family jp`／`--family cn`。
//!
//! ## 如实缺口（`CM-6`，不伪造）
//!
//! - `_codecs_tw`／`_codecs_kr`／`_codecs_hk` 的表**尚未生成** ⇒ 对应 `import encodings.big5`
//!   一族仍报 `No module named '_codecs_tw'`／`_codecs_kr`／`_codecs_hk`；`gb18030`（4 字节区间）
//!   与 `hz`（有状态 7-bit）需要各自的表示，生成器明确拒绝（理由在 `tools/gen_codec_tables.py`）。
//! - **自定义错误处理器**（`codecs.register_error` 注册的名字）调用时如实报
//!   `NotImplementedError`（本层的异常对象不带 `object`／`start`／`end` 结构化字段）；
//!   未注册的名字照参照报 `LookupError: unknown error handler name '...'` ✓。
//! - 编码入口对**任意对象**先 `str(obj)` 再编码（照参照 ✓）；解码入口要求 bytes-like
//!   （照参照报 `TypeError: a bytes-like object is required, not '...'` ✓）。

use core::ptr::NonNull;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`_multibytecodec`）。
pub const NAME: &str = "_multibytecodec";

/// 编码名存在实例的**私有**属性里（参照的 codec 对象**没有**公开 `name` 属性 ✓ —— 别造一个）。
const CODEC_NAME_ATTRIBUTE: &str = "__pyawa_codec_name__";

/// 增量编解码器的缓冲（`getstate`／`setstate` 的载荷；编码侧是 `str`、解码侧是 `bytes`）。
const BUFFER_ATTRIBUTE: &str = "buffer";

/// 「没有第二个码位」的哨兵（解码结果最多 2 个码位：JIS X 0213 的组合字符）。
const NO_SECOND: u32 = u32::MAX;

/// 各族的生成表（生成路径见模块头）：`jp`＝JIS 族、`cn`＝GB 族。
const JP_SOURCE: &str = include_str!("codec_tables/jp.txt");
const CN_SOURCE: &str = include_str!("codec_tables/cn.txt");

static JP_TABLES: OnceLock<Vec<CodecTable>> = OnceLock::new();
static CN_TABLES: OnceLock<Vec<CodecTable>> = OnceLock::new();

// ---- 表结构 -----------------------------------------------------------------------------------

/// 一个多字节编码的两张表（都从参照 dump；**不做反向推导**）。
struct CodecTable {
    /// 参照里的编码名（`cp932` 一族；错误文案与 `getcodec` 都用它）。
    name: &'static str,
    /// 编码方向：`(码位, 打包字节序列)`，按码位升序（二分查找）。
    encode: Vec<(u32, u64)>,
    /// 解码方向：`(打包字节序列, [码位1, 码位2])`，按键升序（二分查找）；`NO_SECOND` = 单码位。
    decode: Vec<(u128, [u32; 2])>,
    /// **两码位组合字符**的编码方向：键 `(码位1 << 32) | 码位2`，值同 `encode` 的打包（JIS X 0213）。
    encode_pairs: Vec<(u64, u64)>,
    /// 组合字符的**首码位**集合（增量编码器要把末尾这种字符留到下一段 ✓）。
    pair_starts: HashSet<u32>,
    /// **前缀状态 → 期望总字节数**（区分 `incomplete` 与 `illegal` 的结构信息；见生成脚本的 `X` 行）。
    expectations: HashMap<u128, usize>,
    /// 最长字节序列长度。
    max_sequence: usize,
}

/// 打包一条字节序列成**带长度标记**的 `u128`（长度在最高 8 位 ⇒ 字典序 = 先长度后字节序 ✓）。
fn pack_key(sequence: &[u8]) -> u128 {
    let mut bytes: u128 = 0;
    for byte in sequence {
        bytes = (bytes << 8) | u128::from(*byte);
    }
    ((sequence.len() as u128) << 120) | bytes
}

/// 打包编码方向的值（`(长度 << 56) | 字节`；本族最长 3 字节 ✓）。
fn pack_encoded(sequence: &[u8]) -> u64 {
    let mut bytes: u64 = 0;
    for byte in sequence {
        bytes = (bytes << 8) | u64::from(*byte);
    }
    ((sequence.len() as u64) << 56) | bytes
}

/// 解开编码方向的值（**新分配**；`Vec` 只为返回形态统一，长度 ≤ 7）。
fn unpack_encoded(value: u64) -> Vec<u8> {
    let length = (value >> 56) as usize;
    let bytes = (value & ((1u64 << 56) - 1)).to_be_bytes();
    bytes[8 - length..].to_vec()
}

fn parse_hex(text: &str) -> Option<u64> {
    u64::from_str_radix(text, 16).ok()
}

fn parse_hex_bytes(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    for index in (0..text.len()).step_by(2) {
        out.push(u8::from_str_radix(&text[index..index + 2], 16).ok()?);
    }
    Some(out)
}

/// 收尾一张表（算前缀集合与最大长度）。
fn finish_table(
    name: &'static str,
    encode: Vec<(u32, u64)>,
    decode: Vec<(u128, [u32; 2])>,
    encode_pairs: Vec<(u64, u64)>,
    expectations: HashMap<u128, usize>,
) -> CodecTable {
    let mut max_sequence = 0usize;
    for (key, _) in &decode {
        max_sequence = max_sequence.max((*key >> 120) as usize);
    }
    let mut pair_starts: HashSet<u32> = HashSet::new();
    for (key, _) in &encode_pairs {
        pair_starts.insert((*key >> 32) as u32);
    }
    CodecTable {
        name,
        encode,
        decode,
        encode_pairs,
        pair_starts,
        expectations,
        max_sequence,
    }
}

/// 解析生成的数据文件（格式见 `tools/gen_codec_tables.py` 模块 doc）。
///
/// 生成物**不可能**有语法错 ⇒ 这里的 `expect` 是"生成物与解析器版本一致"的自检（改格式必须两边一起改）。
fn parse_tables(source: &'static str) -> Vec<CodecTable> {
    let mut tables: Vec<CodecTable> = Vec::new();
    let mut encode: Vec<(u32, u64)> = Vec::new();
    let mut decode: Vec<(u128, [u32; 2])> = Vec::new();
    let mut encode_pairs: Vec<(u64, u64)> = Vec::new();
    let mut expectations: HashMap<u128, usize> = HashMap::new();
    let mut current: Option<&'static str> = None;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("@ ") {
            if let Some(name) = current.take() {
                tables.push(finish_table(
                    name,
                    core::mem::take(&mut encode),
                    core::mem::take(&mut decode),
                    core::mem::take(&mut encode_pairs),
                    core::mem::take(&mut expectations),
                ));
            }
            current = Some(rest.trim());
            continue;
        }
        if let Some(rest) = line.strip_prefix("E ") {
            let mut fields = rest.split_whitespace();
            let codepoint = parse_hex(fields.next().expect("E 行缺码位"))
                .and_then(|value| u32::try_from(value).ok())
                .expect("E 行码位不是合法 u32");
            let sequence = parse_hex_bytes(fields.next().expect("E 行缺字节")).expect("E 行字节不是合法十六进制");
            encode.push((codepoint, pack_encoded(&sequence)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("X ") {
            let mut fields = rest.split_whitespace();
            let prefix = parse_hex_bytes(fields.next().expect("X 行缺前缀")).expect("X 行前缀不是合法十六进制");
            let total = fields
                .next()
                .and_then(parse_hex)
                .and_then(|value| usize::try_from(value).ok())
                .expect("X 行缺期望长度");
            expectations.insert(pack_key(&prefix), total);
            continue;
        }
        if let Some(rest) = line.strip_prefix("P ") {
            let mut fields = rest.split_whitespace();
            let first = fields
                .next()
                .and_then(parse_hex)
                .and_then(|value| u32::try_from(value).ok())
                .expect("P 行缺第一个码位");
            let second = fields
                .next()
                .and_then(parse_hex)
                .and_then(|value| u32::try_from(value).ok())
                .expect("P 行缺第二个码位");
            let sequence = parse_hex_bytes(fields.next().expect("P 行缺字节")).expect("P 行字节不是合法十六进制");
            encode_pairs.push(((u64::from(first) << 32) | u64::from(second), pack_encoded(&sequence)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("D ") {
            let mut fields = rest.split_whitespace();
            let sequence = parse_hex_bytes(fields.next().expect("D 行缺字节")).expect("D 行字节不是合法十六进制");
            let first = fields
                .next()
                .and_then(parse_hex)
                .and_then(|value| u32::try_from(value).ok())
                .expect("D 行缺第一个码位");
            let second = fields
                .next()
                .and_then(parse_hex)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(NO_SECOND);
            decode.push((pack_key(&sequence), [first, second]));
            continue;
        }
        panic!("生成表里有无法解析的行：{line}");
    }
    if let Some(name) = current.take() {
        tables.push(finish_table(name, encode, decode, encode_pairs, expectations));
    }
    for table in &tables {
        debug_assert!(table.encode.is_sorted_by_key(|entry| entry.0));
        debug_assert!(table.decode.is_sorted_by_key(|entry| entry.0));
        debug_assert!(table.encode_pairs.is_sorted_by_key(|entry| entry.0));
    }
    tables
}

/// `_codecs_jp` 族的表（首次用到时解析一次）。
fn jp_tables() -> &'static Vec<CodecTable> {
    JP_TABLES.get_or_init(|| parse_tables(JP_SOURCE))
}

/// `_codecs_cn` 族的表（同上）。
fn cn_tables() -> &'static Vec<CodecTable> {
    CN_TABLES.get_or_init(|| parse_tables(CN_SOURCE))
}

/// 按编码名取表：**跨族**查（各族的编码名不重名 ✓）；都没有 ⇒ `None`（`getcodec` 报 `LookupError`）。
fn table_for(name: &str) -> Option<&'static CodecTable> {
    jp_tables()
        .iter()
        .chain(cn_tables().iter())
        .find(|table| table.name == name)
}

// ---- 编解码循环（状态机） ---------------------------------------------------------------------

/// 一个解码单元的判定结果。
enum DecodeStep {
    /// 命中一条映射（1~2 个码位）。
    Text { cps: [u32; 2], used: usize },
    /// 合法前缀但输入不够：`incomplete multibyte sequence`。
    Incomplete { end: usize },
    /// 当前字节开不出任何合法序列：`illegal multibyte sequence`。
    Illegal,
}

/// 在 `input[position..]` 上判定一个解码单元（**照参照的 C 解码器分型** ✓）。
///
/// `expectations` 给"这个前缀状态总共还要几个字节"：字节不够 ⇒ `incomplete`（`end` = 输入末尾），
/// 够但配不上 ⇒ `illegal`。只靠前缀集合会把 `euc_jp` 的 `b'\x8f\xa1'`（结构上要第 3 字节）
/// 错报成 `illegal` ✗ —— 所以由生成脚本探出 `X` 行，这里按它分型。
fn decode_step(table: &CodecTable, input: &[u8], position: usize) -> DecodeStep {
    let mut bytes: u128 = 0;
    for depth in 1..=table.max_sequence {
        if position + depth > input.len() {
            return DecodeStep::Incomplete { end: input.len() };
        }
        bytes = (bytes << 8) | u128::from(input[position + depth - 1]);
        let key = ((depth as u128) << 120) | bytes;
        if let Ok(index) = table.decode.binary_search_by_key(&key, |entry| entry.0) {
            return DecodeStep::Text {
                cps: table.decode[index].1,
                used: depth,
            };
        }
        let Some(expected) = table.expectations.get(&key) else {
            return DecodeStep::Illegal;
        };
        if position + *expected > input.len() {
            return DecodeStep::Incomplete { end: input.len() };
        }
        if depth >= *expected {
            return DecodeStep::Illegal;
        }
    }
    DecodeStep::Illegal
}

/// 解码错误的对象（**文案照参照**：单字节用 `byte 0x.. in position N`，多字节用 `bytes in position N-M`）。
fn decode_error(instance: &Instance, name: &str, input: &[u8], start: usize, end: usize, reason: &str) -> ExecError {
    let byte = input.get(start).copied().unwrap_or(0);
    let message = if end.saturating_sub(start) == 1 {
        format!("'{name}' codec can't decode byte 0x{byte:02x} in position {start}: {reason}")
    } else {
        format!(
            "'{name}' codec can't decode bytes in position {start}-{}: {reason}",
            end.saturating_sub(1)
        )
    };
    instance.raise_builtin_error("UnicodeDecodeError", &message)
}

/// 编码错误的对象（字符字面量照参照：BMP 用 `\uXXXX`、增补面用 `\UXXXXXXXX`，小写十六进制 ✓）。
fn encode_error(instance: &Instance, name: &str, codepoint: u32, position: usize) -> ExecError {
    // 参照的字符字面量分型（实测 ✓）：≤ 0xFF 用 `\xNN`（所以 `'¥'` 报 `'\xa5'`，不是 `'\u00a5'`）、
    // BMP 用 `\uNNNN`、增补面用 `\UNNNNNNNN`，全小写十六进制。
    let literal = if codepoint <= 0xFF {
        format!("\\x{codepoint:02x}")
    } else if codepoint <= 0xFFFF {
        format!("\\u{codepoint:04x}")
    } else {
        format!("\\U{codepoint:08x}")
    };
    instance.raise_builtin_error(
        "UnicodeEncodeError",
        &format!(
            "'{name}' codec can't encode character '{literal}' in position {position}: illegal multibyte sequence"
        ),
    )
}

/// `errors` 不是内建三条时：已注册的处理器如实报未接线，未注册的照参照报 `LookupError`。
fn error_handler_failure(instance: &Instance, codec: &str, handler: &str) -> ExecError {
    if known_error_handler(instance, handler) {
        instance.raise_builtin_error(
            "NotImplementedError",
            &format!(
                "`{codec}` codec 的 `{handler}` 错误处理器尚未接线（只认 strict／ignore／replace；自定义处理器要 \
                 UnicodeEncodeError／UnicodeDecodeError 的 object／start／end 结构化字段）"
            ),
        )
    } else {
        instance.raise_builtin_error(
            "LookupError",
            &format!("unknown error handler name '{handler}'"),
        )
    }
}

/// `_codecs` 模块的注册表里有没有这个错误处理器名（`codecs.register_error` 注册的都在那）。
fn known_error_handler(instance: &Instance, handler: &str) -> bool {
    let Some(modules) = instance.modules() else {
        return false;
    };
    let Some(module) = instance.dict_get(modules, "_codecs") else {
        return false;
    };
    let Some(namespace) = pyawa_core::mounted_instance_dict(instance, module) else {
        return false;
    };
    let Some(handlers) = instance.dict_get(namespace, "__pyawa_error_handlers__") else {
        return false;
    };
    instance.dict_get(handlers, handler).is_some()
}

/// 解码整段输入（`final=true` 的静态路径；`errors` 的三条都接 ✓）。
fn decode_text(
    instance: &Instance,
    table: &CodecTable,
    input: &[u8],
    errors: &str,
    is_final: bool,
) -> Result<(String, usize, Option<usize>), ExecError> {
    let mut out = String::new();
    let mut position = 0usize;
    while position < input.len() {
        match decode_step(table, input, position) {
            DecodeStep::Text { cps, used } => {
                for codepoint in cps {
                    if codepoint == NO_SECOND {
                        continue;
                    }
                    out.push(char::from_u32(codepoint).unwrap_or('\u{fffd}'));
                }
                position += used;
            }
            DecodeStep::Illegal => match errors {
                "strict" => {
                    return Err(decode_error(
                        instance,
                        table.name,
                        input,
                        position,
                        position + 1,
                        "illegal multibyte sequence",
                    ))
                }
                "ignore" => position += 1,
                "replace" => {
                    out.push('\u{fffd}');
                    position += 1;
                }
                other => return Err(error_handler_failure(instance, table.name, other)),
            },
            DecodeStep::Incomplete { end } => {
                if !is_final {
                    // 增量解码：末尾的合法前缀**留着**，等下一段（与 `errors` 无关 —— 照参照 ✓）
                    return Ok((out, position, Some(position)));
                }
                match errors {
                    "strict" => {
                        return Err(decode_error(
                            instance,
                            table.name,
                            input,
                            position,
                            end,
                            "incomplete multibyte sequence",
                        ))
                    }
                    "ignore" => position = end,
                    "replace" => {
                        out.push('\u{fffd}');
                        position = end;
                    }
                    other => return Err(error_handler_failure(instance, table.name, other)),
                }
            }
        }
    }
    Ok((out, position, None))
}

/// 编码整段文本（返回**编码字节**与**消耗码位数** ✓）。
fn encode_text(
    instance: &Instance,
    table: &CodecTable,
    text: &str,
    errors: &str,
) -> Result<(Vec<u8>, usize), ExecError> {
    let characters: Vec<char> = text.chars().collect();
    let mut out: Vec<u8> = Vec::with_capacity(text.len());
    let mut position = 0usize;
    while position < characters.len() {
        // **两码位组合字符先试**（JIS X 0213；照参照：`'か' + '\u309a'` 合成一条序列 ✓）
        if let Some(second) = characters.get(position + 1) {
            let key = (u64::from(characters[position] as u32) << 32) | u64::from(*second as u32);
            if let Ok(index) = table
                .encode_pairs
                .binary_search_by_key(&key, |entry| entry.0)
            {
                out.extend_from_slice(&unpack_encoded(table.encode_pairs[index].1));
                position += 2;
                continue;
            }
        }
        let character = characters[position];
        match table
            .encode
            .binary_search_by_key(&(character as u32), |entry| entry.0)
        {
            Ok(index) => out.extend_from_slice(&unpack_encoded(table.encode[index].1)),
            Err(_) => match errors {
                "strict" => return Err(encode_error(instance, table.name, character as u32, position)),
                "ignore" => {}
                "replace" => out.push(b'?'),
                other => return Err(error_handler_failure(instance, table.name, other)),
            },
        }
        position += 1;
    }
    Ok((out, characters.len()))
}

// ---- 小工具（与其它模块同一做法） -------------------------------------------------------------

fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
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

/// `self` 可能在 `bound`（属性通道）或 `args[0]`（建实例的隐式调用）——两条都接。
fn this_of<'a>(
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> Option<(NonNull<Header>, &'a [NonNull<Header>])> {
    match bound {
        Some(this) => Some((this, args)),
        None => args.split_first().map(|(first, rest)| (*first, rest)),
    }
}

/// 取实例上的私有编码名（MultibyteCodec 实例）。
fn codec_name_of(instance: &Instance, this: NonNull<Header>) -> Option<String> {
    instance
        .attribute_optional_of(this, CODEC_NAME_ATTRIBUTE)
        .ok()
        .flatten()
        .and_then(|value| instance.text_of(value).map(str::to_owned))
}

/// 取实例上的表（MultibyteCodec 实例 ⇒ 按私有编码名查表）。
fn codec_table_of(instance: &Instance, this: NonNull<Header>) -> Option<&'static CodecTable> {
    codec_name_of(instance, this).and_then(|name| table_for(&name))
}

/// `errors` 实参（位置或关键字；缺省／`None` ⇒ `"strict"`；照参照的类型文案 ✓）。
fn codec_errors_argument(
    instance: &Instance,
    positional: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    function: &str,
) -> Result<String, ExecError> {
    let mut errors: Option<NonNull<Header>> = positional.first().copied();
    if positional.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "{function}() takes at most 2 arguments ({} given)",
                positional.len() + 1
            ),
        ));
    }
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            return Err(instance.raise_builtin_error("TypeError", "keywords must be strings"));
        };
        if name == "errors" {
            if errors.is_some() {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("{function}() got multiple values for argument 'errors'"),
                ));
            }
            errors = Some(*value);
        } else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("{function}() got an unexpected keyword argument '{name}'"),
            ));
        }
    }
    match errors {
        None => Ok("strict".to_owned()),
        Some(value) if value == instance.singletons().none() => Ok("strict".to_owned()),
        Some(value) => match instance.text_value(value) {
            Some(text) => Ok(text),
            None => Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "{function}() argument 'errors' must be str, not {}",
                    instance.type_name(instance.type_of(value))
                ),
            )),
        },
    }
}

/// 编码实参：参照对**任意对象**先 `str(obj)` 再编码（不解对象类型 ⇒ `b'x'` 编成 `"b'x'"` ✓）。
fn encode_input_text(instance: &Instance, value: NonNull<Header>) -> Result<String, ExecError> {
    if let Some(text) = instance.text_value(value) {
        return Ok(text);
    }
    instance.object_str(value)
}

/// 解码实参：必须是 bytes-like（照参照 `TypeError: a bytes-like object is required, not '...'`）。
fn decode_input_bytes(instance: &Instance, value: NonNull<Header>) -> Result<Vec<u8>, ExecError> {
    instance.bytes_value(value).map(<[u8]>::to_vec).ok_or_else(|| {
        instance.raise_builtin_error(
            "TypeError",
            &format!(
                "a bytes-like object is required, not '{}'",
                instance.type_name(instance.type_of(value))
            ),
        )
    })
}

// ---- `MultibyteCodec`（`_codecs_jp.getcodec(...)` 交回的类型） ---------------------------------

/// `codec.encode(input, errors='strict')` ⇒ `(bytes, 消耗码位数)` ✓。
fn multibyte_encode_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "MultibyteCodec.encode 缺少 self"));
    };
    let Some(table) = codec_table_of(instance, this) else {
        return Err(instance.raise_builtin_error("LookupError", "no such codec is supported."));
    };
    let Some((input, rest)) = args.split_first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "encode() takes at least 1 argument (0 given)",
        ));
    };
    let errors = codec_errors_argument(instance, rest, kwargs, "encode")?;
    let text = encode_input_text(instance, *input)?;
    let (bytes, consumed) = encode_text(instance, table, &text, &errors)?;
    let value = instance.new_bytes(&bytes);
    Ok(instance.new_tuple(vec![value, instance.new_int(consumed as i64)]))
}

/// `codec.decode(input, errors='strict')` ⇒ `(str, 消耗字节数)` ✓。
fn multibyte_decode_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "MultibyteCodec.decode 缺少 self"));
    };
    let Some(table) = codec_table_of(instance, this) else {
        return Err(instance.raise_builtin_error("LookupError", "no such codec is supported."));
    };
    let Some((input, rest)) = args.split_first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "decode() takes at least 1 argument (0 given)",
        ));
    };
    let errors = codec_errors_argument(instance, rest, kwargs, "decode")?;
    let bytes = decode_input_bytes(instance, *input)?;
    let (text, consumed, _) = decode_text(instance, table, &bytes, &errors, true)?;
    let value = instance.new_str(&text);
    Ok(instance.new_tuple(vec![value, instance.new_int(consumed as i64)]))
}

/// 建 `MultibyteCodec` 类（照 `_sre`／`_random` 的 `build_class_from_parts` 手法 ✓）。
fn build_multibyte_codec_class(instance: &Instance) -> Result<NonNull<Header>, ExecError> {
    let base = instance
        .type_named("object")
        .expect("object 在引导期已登记")
        .cast::<Header>();
    let class_namespace = instance.new_dict();
    for (name, handler) in [
        ("encode", multibyte_encode_native as NativeFn),
        ("decode", multibyte_decode_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    pyawa_core::build_class_from_parts(
        instance,
        "MultibyteCodec".to_owned(),
        vec![base],
        class_namespace,
        None,
    )
}

/// **`getcodec(name)`**（`_codecs_jp` 等各族的共同入口）⇒ 一个 `MultibyteCodec` 实例 ✓。
pub(crate) fn getcodec_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "getcodec() takes exactly one argument (0 given)",
        ));
    };
    let Some(name) = instance.text_value(value) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "getcodec() argument must be str, not {}",
                instance.type_name(instance.type_of(value))
            ),
        ));
    };
    let Some(table) = table_for(&name) else {
        return Err(instance.raise_builtin_error("LookupError", "no such codec is supported."));
    };
    let class = build_multibyte_codec_class(instance)?;
    let codec = pyawa_core::executor::call::call_value(instance, class, &[], &[])?;
    let name_value = instance.new_str(table.name);
    let _ = instance.set_attribute_value(codec, CODEC_NAME_ATTRIBUTE, name_value);
    Ok(codec)
}

// ---- 增量编解码器 -----------------------------------------------------------------------------

/// 取实例属性里的文本（缺省 `fallback`）。
fn instance_text(instance: &Instance, this: NonNull<Header>, name: &str, fallback: &str) -> String {
    instance
        .attribute_optional_of(this, name)
        .ok()
        .flatten()
        .and_then(|value| instance.text_of(value).map(str::to_owned))
        .unwrap_or_else(|| fallback.to_owned())
}

/// 原样设置实例属性（`instance.set_attribute_value` 自己 retain ⇒ 这里传借用值即可）。
fn set_instance_text(instance: &Instance, this: NonNull<Header>, name: &str, text: &str) -> Result<(), ExecError> {
    let value = instance.new_str(text);
    instance.set_attribute_value(this, name, value)
}

/// 增量类的 `__init__(self, errors='strict')`：只记 `errors` 并给缓冲一个空初值。
fn incremental_init(instance: &Instance, bound: Option<NonNull<Header>>, args: &[NonNull<Header>], kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<(NonNull<Header>, String), ExecError> {
    let Some((this, positional)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "增量编解码器缺少 self"));
    };
    // 位置／关键字两种写法都收（参照签名 `__init__(self, errors='strict')` ✓）
    let errors = codec_errors_argument(instance, positional, kwargs, "__init__")?;
    set_instance_text(instance, this, "errors", &errors)?;
    Ok((this, errors))
}

fn incremental_encoder_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _errors) = incremental_init(instance, bound, args, kwargs)?;
    set_instance_text(instance, this, BUFFER_ATTRIBUTE, "")?;
    Ok(instance.new_none())
}

fn incremental_decoder_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _errors) = incremental_init(instance, bound, args, kwargs)?;
    let empty = instance.new_bytes(b"");
    instance.set_attribute_value(this, BUFFER_ATTRIBUTE, empty)?;
    Ok(instance.new_none())
}

/// 增量编码器实例的 `codec`／`errors`／`buffer` 三格（都从实例或类属性上取）。
fn incremental_parts(
    instance: &Instance,
    this: NonNull<Header>,
) -> Result<(&'static CodecTable, String, String), ExecError> {
    let Some(codec_object) = instance.attribute_optional_of(this, "codec").ok().flatten() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "增量编解码器缺少 `codec` 类属性（`encodings/*` 的子类会设它）",
        ));
    };
    let Some(table) = codec_table_of(instance, codec_object) else {
        return Err(instance.raise_builtin_error("LookupError", "no such codec is supported."));
    };
    let errors = instance_text(instance, this, "errors", "strict");
    let buffer = instance_text(instance, this, BUFFER_ATTRIBUTE, "");
    Ok((table, errors, buffer))
}

/// `IncrementalEncoder.encode(input, final=False)` ⇒ `bytes`（**不带元组** ✓）。
fn incremental_encoder_encode(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "IncrementalEncoder.encode 缺少 self"));
    };
    let (table, errors, buffer) = incremental_parts(instance, this)?;
    let Some(input) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "encode() takes at least 1 argument (0 given)",
        ));
    };
    let final_round = args
        .get(1)
        .map(|value| *value != instance.singletons().none() && *value != instance.singletons().boolean(false))
        .unwrap_or(false);
    let text = encode_input_text(instance, input)?;
    let mut combined = if buffer.is_empty() { text } else { buffer + &text };
    // **末尾的组合字符首码位先留着** ✓（照参照：`ie.encode('か', False)` 交 `b''`、
    // 下一段接上 `'\u309a'` 才输出组合编码 —— 见模块头的增量口径）
    let mut held = String::new();
    if !final_round {
        if let Some(last) = combined.chars().last() {
            if table.pair_starts.contains(&(last as u32)) {
                held.push(last);
                combined.truncate(combined.len() - last.len_utf8());
            }
        }
    }
    let (bytes, _) = encode_text(instance, table, &combined, &errors)?;
    set_instance_text(instance, this, BUFFER_ATTRIBUTE, &held)?;
    Ok(instance.new_bytes(&bytes))
}

/// `IncrementalEncoder.reset()`。
fn incremental_encoder_reset(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "reset() 缺少 self"));
    };
    set_instance_text(instance, this, BUFFER_ATTRIBUTE, "")?;
    Ok(instance.new_none())
}

/// `IncrementalEncoder.getstate()` ⇒ `int`（照参照：`_multibytecodec` 的 C 实现**返回 int 0** ——
/// 它**不暴露**内部留着的组合字符首码位；本层保持同一形态 ✓）。
fn incremental_encoder_getstate(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if bound.is_none() {
        return Err(instance.raise_builtin_error("TypeError", "getstate() 缺少 self"));
    }
    Ok(instance.new_int(0))
}

/// `IncrementalEncoder.setstate(state)`。
fn incremental_encoder_setstate(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "setstate() 缺少 self"));
    };
    let Some(state) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "setstate() takes exactly 1 argument (0 given)",
        ));
    };
    // 照参照：编码器的 state 是 **int**（不是 buffer 的载荷 —— 缓冲不外露 ✓）
    if instance.int_value(state).is_none() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "setstate() argument must be int, not {}",
                instance.type_name(instance.type_of(state))
            ),
        ));
    }
    set_instance_text(instance, this, BUFFER_ATTRIBUTE, "")?;
    Ok(instance.new_none())
}

/// 增量解码器实例的 `codec`／`errors`／缓冲（缓冲是 `bytes`）。
fn incremental_decoder_parts(
    instance: &Instance,
    this: NonNull<Header>,
) -> Result<(&'static CodecTable, String, Vec<u8>), ExecError> {
    let (table, errors, _) = incremental_parts(instance, this)?;
    let buffer = instance
        .attribute_optional_of(this, BUFFER_ATTRIBUTE)
        .ok()
        .flatten()
        .and_then(|value| instance.bytes_value(value).map(<[u8]>::to_vec))
        .unwrap_or_default();
    Ok((table, errors, buffer))
}

fn set_instance_bytes(instance: &Instance, this: NonNull<Header>, name: &str, bytes: &[u8]) -> Result<(), ExecError> {
    let value = instance.new_bytes(bytes);
    instance.set_attribute_value(this, name, value)
}

/// `IncrementalDecoder.decode(input, final=False)` ⇒ `str`（尾部截断前缀留到下一段 ✓）。
fn incremental_decoder_decode(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "IncrementalDecoder.decode 缺少 self"));
    };
    let (table, errors, buffer) = incremental_decoder_parts(instance, this)?;
    let Some(input) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "decode() takes at least 1 argument (0 given)",
        ));
    };
    let bytes = decode_input_bytes(instance, input)?;
    let final_round = args
        .get(1)
        .map(|value| *value != instance.singletons().none() && *value != instance.singletons().boolean(false))
        .unwrap_or(false);
    let mut combined = buffer;
    combined.extend_from_slice(&bytes);
    let (text, _consumed, tail) = decode_text(instance, table, &combined, &errors, final_round)?;
    match tail {
        Some(start) => set_instance_bytes(instance, this, BUFFER_ATTRIBUTE, &combined[start..])?,
        None => set_instance_bytes(instance, this, BUFFER_ATTRIBUTE, b"")?,
    }
    Ok(instance.new_str(&text))
}

/// `IncrementalDecoder.reset()`。
fn incremental_decoder_reset(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "reset() 缺少 self"));
    };
    set_instance_bytes(instance, this, BUFFER_ATTRIBUTE, b"")?;
    Ok(instance.new_none())
}

/// `IncrementalDecoder.getstate()` ⇒ `(缓冲字节, 0)`（照参照的二元组形态 ✓）。
fn incremental_decoder_getstate(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "getstate() 缺少 self"));
    };
    let (_, _, buffer) = incremental_decoder_parts(instance, this)?;
    let bytes = instance.new_bytes(&buffer);
    let zero = instance.new_int(0);
    Ok(instance.new_tuple(vec![bytes, zero]))
}

/// `IncrementalDecoder.setstate(state)`。
fn incremental_decoder_setstate(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "setstate() 缺少 self"));
    };
    let Some(state) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "setstate() takes exactly 1 argument (0 given)",
        ));
    };
    // 照参照：解码器的 state 是**二元组** `(缓冲字节, 0)` ✓
    let Some(items) = instance.tuple_items(state) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "setstate() argument must be tuple, not {}",
                instance.type_name(instance.type_of(state))
            ),
        ));
    };
    if items.len() != 2 {
        return Err(instance.raise_builtin_error("TypeError", "setstate(): illegal state argument"));
    }
    let Some(bytes) = instance.bytes_value(items[0]).map(<[u8]>::to_vec) else {
        return Err(instance.raise_builtin_error("TypeError", "setstate(): illegal state argument"));
    };
    set_instance_bytes(instance, this, BUFFER_ATTRIBUTE, &bytes)?;
    Ok(instance.new_none())
}

// ---- 模块装配 ---------------------------------------------------------------------------------

/// 建一个"只有方法面"的类（Rust 造类，照 `_socket`／`_random` ✓）。
fn build_method_class(
    instance: &Instance,
    name: &str,
    methods: &[(&str, NativeFn)],
) -> Result<NonNull<Header>, ExecError> {
    let base = instance
        .type_named("object")
        .expect("object 在引导期已登记")
        .cast::<Header>();
    let class_namespace = instance.new_dict();
    for (method_name, handler) in methods {
        let function = make_native(instance, method_name, *handler);
        instance.dict_set(class_namespace, method_name, function);
    }
    pyawa_core::build_class_from_parts(instance, name.to_owned(), vec![base], class_namespace, None)
}

/// 建 `_multibytecodec` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let classes: [(&str, &[(&str, NativeFn)]); 4] = [
        (
            "MultibyteIncrementalEncoder",
            &[
                ("__init__", incremental_encoder_init as NativeFn),
                ("encode", incremental_encoder_encode as NativeFn),
                ("reset", incremental_encoder_reset as NativeFn),
                ("getstate", incremental_encoder_getstate as NativeFn),
                ("setstate", incremental_encoder_setstate as NativeFn),
            ],
        ),
        (
            "MultibyteIncrementalDecoder",
            &[
                ("__init__", incremental_decoder_init as NativeFn),
                ("decode", incremental_decoder_decode as NativeFn),
                ("reset", incremental_decoder_reset as NativeFn),
                ("getstate", incremental_decoder_getstate as NativeFn),
                ("setstate", incremental_decoder_setstate as NativeFn),
            ],
        ),
        // 流包装：**空基类**（`read`／`write` 由 `codecs.StreamReader`／`StreamWriter` 用
        // `self.decode`／`self.encode` 的元组契约实现 ✓ —— 见模块头）
        ("MultibyteStreamReader", &[] as &[(&str, NativeFn)]),
        ("MultibyteStreamWriter", &[] as &[(&str, NativeFn)]),
    ];
    for (name, methods) in classes {
        if let Ok(class) = build_method_class(instance, name, methods) {
            instance.dict_set(namespace, name, class);
        }
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(
        "Multibyte codec shared engine (Pyawa's built-in implementation of _codecs_jp's base classes).",
    );
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}

/// 已落地族的**全部编码名**（测试／诊断用；`getcodec` 的可用集合）。
pub fn supported_codecs() -> Vec<&'static str> {
    jp_tables()
        .iter()
        .chain(cn_tables().iter())
        .map(|table| table.name)
        .collect()
}
