//! **`_struct` 真实现**（第 725 轮 ✓；此前是"名字齐、调用报未接线"的占位 ✗）。
//!
//! 覆盖面：字节序／对齐前缀 `@ = < > !` ✓、格式字符 `x b B ? h H i I l L q Q n N e f d s p`
//! 与**重复计数**（`4s`／`3H` ✓）、模块级 `pack`／`pack_into`／`unpack`／`unpack_from`／
//! `iter_unpack`／`calcsize`／`_clearcache` ✓、`Struct` 类（`format`／`size` ＋ 同名方法 ✓）。
//!
//! **如实口径** ✗：
//! * `error` **就是 `ValueError`** ✓（不是独立子类 ✗ —— 本层还没有"按名字注册异常子类"的口子 ✓；
//!   `Lib/zipfile/__init__.py:212/597/612` 的 `except struct.error` 因此照样接得住 ✓）；
//! * 无符号 64 位超过 `i64` 上界的**取值**未接 ✗（`Q`／`L` 打包收 `i64` 范围 ✓）；
//! * `e`（半精度）未接 ✗ —— 按参照口径报 `bad char in struct format` ✓（不静默给错值 ✗）；
//! * `@` 的原生对齐按 **x86_64 Linux** 算 ✓（`l`／`L` ＝ 8 ✓、`n`／`N`／`P` ＝ 8 ✓）。

use core::ptr::NonNull;
use pyawa_core::Instance;

pub const NAME: &str = "_struct";

const DOC: &str = "Pyawa 的 _struct 面（与参照同口径的 pack／unpack／Struct）";

#[derive(Clone, Copy, PartialEq)]
enum Order {
    /// `@`：原生字节序（本机 x86_64 ⇒ 小端 ✓）＋**原生对齐** ✓。
    Native,
    /// `=`：原生字节序 ＋ **无对齐** ✓。
    NativeStandard,
    Little,
    Big,
}

struct Field {
    kind: char,
    count: usize,
    /// **单元素**宽度（标量 1／2／4／8 ✓；`s`／`p`／`x` 恒 1 ✓）。
    unit: usize,
    /// 该字段在缓冲区里的**起始**偏移（`@` 下含对齐填充 ✓）。
    offset: usize,
    /// 字段占的**总**字节数（标量 ＝ `unit * count` ✓；`s`／`p`／`x` ＝ count ✓）。
    size: usize,
}

struct Format {
    order: Order,
    fields: Vec<Field>,
    size: usize,
}

fn align_of(kind: char, native: bool) -> usize {
    match kind {
        'x' | 'b' | 'B' | '?' | 's' | 'p' => 1,
        'h' | 'H' | 'e' => 2,
        'i' | 'I' | 'f' => 4,
        'l' | 'L' => {
            if native {
                8
            } else {
                4
            }
        }
        'q' | 'Q' | 'd' | 'n' | 'N' | 'P' => 8,
        _ => 1,
    }
}

fn size_of(kind: char, count: usize, native: bool) -> usize {
    match kind {
        'x' => count,
        's' | 'p' => count,
        'b' | 'B' | '?' => 1,
        'h' | 'H' | 'e' => 2,
        'i' | 'I' | 'f' => 4,
        'l' | 'L' => {
            if native {
                8
            } else {
                4
            }
        }
        'q' | 'Q' | 'd' | 'n' | 'N' | 'P' => 8,
        _ => 1,
    }
}

fn align_up(value: usize, alignment: usize) -> usize {
    if alignment <= 1 {
        value
    } else {
        (value + alignment - 1) / alignment * alignment
    }
}

fn parse_format(instance: &Instance, text: &str) -> Result<Format, pyawa_core::ExecError> {
    let mut chars = text.chars().peekable();
    let mut order = Order::Native;
    if let Some(&first) = chars.peek() {
        match first {
            '@' => {
                order = Order::Native;
                chars.next();
            }
            '=' => {
                order = Order::NativeStandard;
                chars.next();
            }
            '<' => {
                order = Order::Little;
                chars.next();
            }
            '>' | '!' => {
                order = Order::Big;
                chars.next();
            }
            _ => {}
        }
    }
    let native = order == Order::Native;
    let mut fields: Vec<Field> = Vec::new();
    let mut offset = 0usize;
    let mut pending_count: Option<usize> = None;
    for kind in chars {
        if kind.is_ascii_whitespace() {
            continue;
        }
        if kind.is_ascii_digit() {
            let digit = kind.to_digit(10).expect("刚判过是数字") as usize;
            pending_count = Some(pending_count.unwrap_or(0) * 10 + digit);
            continue;
        }
        let count = pending_count.take().unwrap_or(1);
        if kind == 'x' {
            // `x` 只占位、不出现在参数表里 ✓。
            offset += size_of('x', count, native);
            continue;
        }
        if !matches!(
            kind,
            'b' | 'B' | '?' | 'h' | 'H' | 'i' | 'I' | 'l' | 'L' | 'q' | 'Q' | 'n' | 'N' | 'P'
                | 'e' | 'f' | 'd' | 's' | 'p'
        ) {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "bad char in struct format",
            ));
        }
        if kind == 'e' {
            // **未接** ⇒ 如实报参照的"坏字符"文案 ✓（不静默算错 ✗）。
            return Err(instance.raise_builtin_error(
                "ValueError",
                "bad char in struct format",
            ));
        }
        if native {
            offset = align_up(offset, align_of(kind, true));
        }
        // **真 bug 修**（第 726 轮）：标量带计数时**总宽 ＝ 单宽 × count** ✗（先前只加单宽 ⇒
        // `calcsize('<4s2B4HL2L2H')` 给 17 而参照是 30 ✗）⇒ `Lib/zipfile` 的头全部短写 ✗。
        let unit = match kind {
            's' | 'p' | 'x' => 1,
            _ => size_of(kind, 1, native),
        };
        let size = match kind {
            's' | 'p' | 'x' => count,
            _ => unit * count,
        };
        fields.push(Field {
            kind,
            count,
            unit,
            offset,
            size,
        });
        offset += size;
    }
    if let Some(_leftover) = pending_count {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "repeat count given without format specifier",
        ));
    }
    Ok(Format {
        order,
        fields,
        size: offset,
    })
}

fn put_uint(buffer: &mut [u8], offset: usize, size: usize, value: u64, big: bool) {
    for index in 0..size {
        let byte = ((value >> (index * 8)) & 0xff) as u8;
        if big {
            buffer[offset + size - 1 - index] = byte;
        } else {
            buffer[offset + index] = byte;
        }
    }
}

fn get_uint(buffer: &[u8], offset: usize, size: usize, big: bool) -> u64 {
    let mut value = 0u64;
    for index in 0..size {
        let byte = if big {
            buffer[offset + size - 1 - index]
        } else {
            buffer[offset + index]
        };
        value |= (byte as u64) << (index * 8);
    }
    value
}

fn as_int(instance: &Instance, object: NonNull<pyawa_core::Header>) -> Result<i64, pyawa_core::ExecError> {
    let value = instance.int_of(object).and_then(|value| value.to_i64());
    value.ok_or_else(|| {
        instance.raise_builtin_error("ValueError", "required argument is not an integer")
    })
}

fn as_float(instance: &Instance, object: NonNull<pyawa_core::Header>) -> Result<f64, pyawa_core::ExecError> {
    if let Some(value) = instance.float_value(object) {
        return Ok(value);
    }
    if let Some(value) = instance.int_of(object).and_then(|value| value.to_i64()) {
        return Ok(value as f64);
    }
    Err(instance.raise_builtin_error("ValueError", "required argument is not a float"))
}

fn range_check(
    instance: &Instance,
    value: i64,
    bits: u32,
    signed: bool,
) -> Result<u64, pyawa_core::ExecError> {
    if signed {
        let min = -(1i64 << (bits - 1));
        let max = (1i64 << (bits - 1)) - 1;
        if value < min || value > max {
            return Err(instance.raise_builtin_error("ValueError", "argument out of range"));
        }
        Ok(value as u64)
    } else {
        if value < 0 || (bits < 64 && value >= (1i64 << bits)) {
            return Err(instance.raise_builtin_error("ValueError", "argument out of range"));
        }
        Ok(value as u64)
    }
}

fn pack_values(
    instance: &Instance,
    format: &Format,
    values: &[NonNull<pyawa_core::Header>],
) -> Result<Vec<u8>, pyawa_core::ExecError> {
    // **`s`／`p` 的 count 是"字节宽度"** ✓（不是实参个数 ✓）⇒ 每个字段只吃 **1** 个实参 ✓。
    let expected: usize = format.fields.iter().map(item_count).sum();
    if values.len() != expected {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!(
                "pack expected {expected} items for packing (got {})",
                values.len()
            ),
        ));
    }
    let big = format.order == Order::Big;
    let mut buffer = vec![0u8; format.size];
    let mut cursor = 0usize;
    for field in &format.fields {
        for index in 0..item_count(field) {
            let value = values[cursor];
            cursor += 1;
            // **每字段自己的下标** ✓（不是全局 cursor ✗ —— 先前用 `cursor - 1` ⇒ 第二个字段起偏移全错 ✓）。
            let field_offset = field_offset_within(field, index);
            match field.kind {
                'b' => {
                    let raw = range_check(instance, as_int(instance, value)?, 8, true)?;
                    put_uint(&mut buffer, field_offset, 1, raw, big);
                }
                'B' | '?' => {
                    let raw = if field.kind == '?' {
                        if instance.truth_of(value) {
                            1
                        } else {
                            0
                        }
                    } else {
                        range_check(instance, as_int(instance, value)?, 8, false)?
                    };
                    put_uint(&mut buffer, field_offset, 1, raw, big);
                }
                'h' | 'H' => {
                    let signed = field.kind == 'h';
                    let raw = range_check(instance, as_int(instance, value)?, 16, signed)?;
                    put_uint(&mut buffer, field_offset, 2, raw, big);
                }
                'i' | 'I' | 'l' | 'L' => {
                    let signed = field.kind == 'i' || field.kind == 'l';
                    let bits = if field.kind == 'i' || field.kind == 'I' { 32 } else if format.order == Order::Native { 64 } else { 32 };
                    let raw = range_check(instance, as_int(instance, value)?, bits, signed)?;
                    let width = if bits == 64 { 8 } else { 4 };
                    put_uint(&mut buffer, field_offset, width, raw, big);
                }
                'q' | 'Q' | 'n' | 'N' | 'P' => {
                    let signed = field.kind == 'q' || field.kind == 'n';
                    let raw = range_check(instance, as_int(instance, value)?, 64, signed)?;
                    put_uint(&mut buffer, field_offset, 8, raw, big);
                }
                'f' => {
                    let number = as_float(instance, value)? as f32;
                    put_uint(&mut buffer, field_offset, 4, number.to_bits() as u64, big);
                }
                'd' => {
                    let number = as_float(instance, value)?;
                    put_uint(&mut buffer, field_offset, 8, number.to_bits(), big);
                }
                's' | 'p' => {
                    let Some(raw) = instance.bytes_value(value) else {
                        return Err(instance.raise_builtin_error(
                            "ValueError",
                            &format!("argument for '{}' must be a bytes object", field.kind),
                        ));
                    };
                    if field.kind == 's' {
                        let copy = raw.len().min(field.size);
                        buffer[field_offset..field_offset + copy].copy_from_slice(&raw[..copy]);
                    } else {
                        let copy = raw.len().min(field.size.saturating_sub(1));
                        buffer[field_offset] = copy as u8;
                        buffer[field_offset + 1..field_offset + 1 + copy]
                            .copy_from_slice(&raw[..copy]);
                    }
                }
                _ => unreachable!("parse_format 已过滤"),
            }
        }
    }
    Ok(buffer)
}

/// 一个字段吃几个实参 ✓（`s`／`p` 的 count 是字节宽度 ⇒ 恒 **1** 个 ✓）。
fn item_count(field: &Field) -> usize {
    if field.kind == 's' || field.kind == 'p' {
        1
    } else {
        field.count
    }
}

/// `s`／`p` 的每项偏移：`field.offset` ＋ 该项在字段内的字节偏移 ✓。
fn field_offset_within(field: &Field, index: usize) -> usize {
    match field.kind {
        // `s`／`p` 只吃 1 个实参（count 是字节宽度）⇒ 恒从字段起点开始 ✓。
        's' | 'p' => field.offset,
        // 标量：第 `index` 个元素从字段起点起**按单宽**排 ✓。
        _ => field.offset + index * field.unit,
    }
}

fn unpack_values(
    instance: &Instance,
    format: &Format,
    buffer: &[u8],
    start: usize,
) -> Result<Vec<NonNull<pyawa_core::Header>>, pyawa_core::ExecError> {
    let big = format.order == Order::Big;
    let mut values: Vec<NonNull<pyawa_core::Header>> = Vec::new();
    for field in &format.fields {
        for index in 0..item_count(field) {
            let offset = start + field_offset_within(field, index);
            let value: NonNull<pyawa_core::Header> = match field.kind {
                'b' => instance.new_int(get_uint(buffer, offset, 1, big) as i8 as i64),
                'B' => instance.new_int(get_uint(buffer, offset, 1, big) as i64),
                '?' => instance.new_bool(get_uint(buffer, offset, 1, big) != 0),
                'h' => instance.new_int(get_uint(buffer, offset, 2, big) as i16 as i64),
                'H' => instance.new_int(get_uint(buffer, offset, 2, big) as i64),
                'i' => instance.new_int(get_uint(buffer, offset, 4, big) as i32 as i64),
                'I' => instance.new_int(get_uint(buffer, offset, 4, big) as i64),
                'l' | 'L' => {
                    let width = if field.kind == 'l' && format.order == Order::Native { 8 } else if field.kind == 'L' && format.order == Order::Native { 8 } else { 4 };
                    let raw = get_uint(buffer, offset, width, big);
                    if field.kind == 'l' {
                        instance.new_int(if width == 8 { raw as i64 } else { raw as i32 as i64 })
                    } else {
                        instance.new_int(raw as i64)
                    }
                }
                'q' => instance.new_int(get_uint(buffer, offset, 8, big) as i64),
                'Q' | 'n' | 'N' | 'P' => instance.new_int(get_uint(buffer, offset, 8, big) as i64),
                'f' => instance.new_float(f32::from_bits(get_uint(buffer, offset, 4, big) as u32) as f64),
                'd' => instance.new_float(f64::from_bits(get_uint(buffer, offset, 8, big))),
                's' => instance.new_bytes(&buffer[offset..offset + field.size]),
                'p' => {
                    let length = (buffer[offset] as usize).min(field.size.saturating_sub(1));
                    instance.new_bytes(&buffer[offset + 1..offset + 1 + length])
                }
                _ => unreachable!("parse_format 已过滤"),
            };
            values.push(value);
        }
    }
    Ok(values)
}

fn parse_into_format(
    instance: &Instance,
    format: NonNull<pyawa_core::Header>,
) -> Result<Format, pyawa_core::ExecError> {
    // **`str` 与 `bytes` 都收** ✓（参照两者都收 ✓；`Lib/zipfile/__init__.py` 的
    // `structEndArchive = b"<4s4H2LH"` 一族正是 **`bytes`** 格式串 ✓）。
    if let Some(text) = instance.text_of(format) {
        return parse_format(instance, text);
    }
    if let Some(raw) = instance.bytes_value(format) {
        let text = String::from_utf8_lossy(raw).into_owned();
        return parse_format(instance, &text);
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        "struct: 第一个实参要是 str 或 bytes",
    ))
}

fn calcsize_native(
    instance: &Instance,
    _bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let Some(format) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "calcsize() takes exactly 1 argument (0 given)"));
    };
    let parsed = parse_into_format(instance, format)?;
    Ok(instance.new_int(parsed.size as i64))
}

fn pack_native(
    instance: &Instance,
    _bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let Some(format) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "pack() 至少要 1 个实参"));
    };
    let parsed = parse_into_format(instance, format)?;
    let bytes = pack_values(instance, &parsed, &args[1..])?;
    Ok(instance.new_bytes(&bytes))
}

fn unpack_native(
    instance: &Instance,
    _bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let (Some(format), Some(buffer)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error("TypeError", "unpack() 要 2 个实参"));
    };
    let parsed = parse_into_format(instance, format)?;
    let Some(raw) = instance.bytes_value(buffer) else {
        return Err(instance.raise_builtin_error("TypeError", "unpack() 的第二个实参要是 bytes"));
    };
    if raw.len() != parsed.size {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("unpack requires a buffer of {} bytes", parsed.size),
        ));
    }
    let values = unpack_values(instance, &parsed, raw, 0)?;
    Ok(instance.new_tuple(values))
}

fn unpack_from_native(
    instance: &Instance,
    _bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let (Some(format), Some(buffer)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error("TypeError", "unpack_from() 至少要 2 个实参"));
    };
    let offset = args
        .get(2)
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0);
    let parsed = parse_into_format(instance, format)?;
    let Some(raw) = instance.bytes_value(buffer) else {
        return Err(instance.raise_builtin_error("TypeError", "unpack_from() 的第二个实参要是 bytes"));
    };
    let start = offset.max(0) as usize;
    if start + parsed.size > raw.len() {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!(
                "unpack_from requires a buffer of at least {} bytes for unpacking {} bytes at offset {} (actual buffer size is {})",
                start + parsed.size,
                parsed.size,
                offset,
                raw.len()
            ),
        ));
    }
    let values = unpack_values(instance, &parsed, raw, start)?;
    Ok(instance.new_tuple(values))
}

fn pack_into_native(
    instance: &Instance,
    _bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let (Some(format), Some(buffer), Some(offset)) =
        (args.first().copied(), args.get(1).copied(), args.get(2).copied())
    else {
        return Err(instance.raise_builtin_error("TypeError", "pack_into() 至少要 3 个实参"));
    };
    let parsed = parse_into_format(instance, format)?;
    let start = instance
        .int_of(offset)
        .and_then(|value| value.to_i64())
        .unwrap_or(0)
        .max(0) as usize;
    let bytes = pack_values(instance, &parsed, &args[3..])?;
    // **只认 `bytearray`** ✓（`bytes` 不可写 ⇒ 参照也报错 ✓）；越界在这里一并拦下 ✓。
    if !instance.write_bytearray_at(buffer, start, &bytes) {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!(
                "pack_into requires a writable bytearray of at least {} bytes at offset {}",
                start + bytes.len(),
                start
            ),
        ));
    }
    Ok(instance.retain(instance.singletons().none()))
}

fn iter_unpack_native(
    instance: &Instance,
    _bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let (Some(format), Some(buffer)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error("TypeError", "iter_unpack() 要 2 个实参"));
    };
    let parsed = parse_into_format(instance, format)?;
    let Some(raw) = instance.bytes_value(buffer) else {
        return Err(instance.raise_builtin_error("TypeError", "iter_unpack() 的第二个实参要是 bytes"));
    };
    if parsed.size == 0 {
        return Ok(instance.new_list(Vec::new()));
    }
    if raw.len() % parsed.size != 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "iter_unpack requires a buffer size that is a multiple of the size of the format",
        ));
    }
    // **如实口径** ✗：返回**列表**（参照给迭代器 ✓；列表可迭代 ✓，但 `next()` 不成立 ✗）。
    let mut items: Vec<NonNull<pyawa_core::Header>> = Vec::new();
    let mut start = 0usize;
    while start + parsed.size <= raw.len() {
        let values = unpack_values(instance, &parsed, raw, start)?;
        items.push(instance.new_tuple(values));
        start += parsed.size;
    }
    Ok(instance.new_list(items))
}

fn clear_cache_native(
    instance: &Instance,
    _bound: Option<NonNull<pyawa_core::Header>>,
    _args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    // 本层**不做格式缓存** ✓ ⇒ 无可清 ✓（如实 ✓，不是假装 ✗）。
    Ok(instance.retain(instance.singletons().none()))
}

/// 类方法原生：`self` 可能在 **`bound`**（属性通道 ✓）或 **`args[0]`**（建实例时的隐式调用 ✗）——
/// 两条都接 ✓（照参照的 `__init__` 调用口径 ✓）。
fn this_of<'a>(
    bound: Option<NonNull<pyawa_core::Header>>,
    args: &'a [NonNull<pyawa_core::Header>],
) -> Option<(NonNull<pyawa_core::Header>, &'a [NonNull<pyawa_core::Header>])> {
    match bound {
        Some(this) => Some((this, args)),
        None => args.split_first().map(|(first, rest)| (*first, rest)),
    }
}

/// `Struct.__init__(self, format)` ✓。
fn struct_init_native(
    instance: &Instance,
    bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.__init__ 缺少 self"));
    };
    let Some(format) = rest.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "Struct() 要 1 个实参"));
    };
    let parsed = parse_into_format(instance, format)?;
    let _ = instance.set_attribute_value(this, "format", format);
    let size = instance.new_int(parsed.size as i64);
    let _ = instance.set_attribute_value(this, "size", size);
    Ok(instance.retain(instance.singletons().none()))
}

fn struct_format(instance: &Instance, this: NonNull<pyawa_core::Header>) -> Result<Format, pyawa_core::ExecError> {
    let Some(format) = instance.attribute_optional_of(this, "format")? else {
        return Err(instance.raise_builtin_error("TypeError", "Struct 实例缺少 format"));
    };
    parse_into_format(instance, format)
}

fn struct_pack_native(
    instance: &Instance,
    bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.pack 缺少 self"));
    };
    let parsed = struct_format(instance, this)?;
    let bytes = pack_values(instance, &parsed, rest)?;
    Ok(instance.new_bytes(&bytes))
}

fn struct_unpack_native(
    instance: &Instance,
    bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.unpack 缺少 self"));
    };
    let parsed = struct_format(instance, this)?;
    let Some(buffer) = rest.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.unpack 要 1 个实参"));
    };
    let Some(raw) = instance.bytes_value(buffer) else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.unpack 的实参要是 bytes"));
    };
    if raw.len() != parsed.size {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("unpack requires a buffer of {} bytes", parsed.size),
        ));
    }
    let values = unpack_values(instance, &parsed, raw, 0)?;
    Ok(instance.new_tuple(values))
}

fn struct_unpack_from_native(
    instance: &Instance,
    bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.unpack_from 缺少 self"));
    };
    let parsed = struct_format(instance, this)?;
    let Some(buffer) = rest.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.unpack_from 要 1 个实参"));
    };
    let offset = rest
        .get(1)
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0);
    let Some(raw) = instance.bytes_value(buffer) else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.unpack_from 的实参要是 bytes"));
    };
    let start = offset.max(0) as usize;
    if start + parsed.size > raw.len() {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!(
                "unpack_from requires a buffer of at least {} bytes for unpacking {} bytes at offset {} (actual buffer size is {})",
                start + parsed.size,
                parsed.size,
                offset,
                raw.len()
            ),
        ));
    }
    let values = unpack_values(instance, &parsed, raw, start)?;
    Ok(instance.new_tuple(values))
}

fn struct_pack_into_native(
    instance: &Instance,
    bound: Option<NonNull<pyawa_core::Header>>,
    args: &[NonNull<pyawa_core::Header>],
    _kwargs: &[(NonNull<pyawa_core::Header>, NonNull<pyawa_core::Header>)],
) -> Result<NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.pack_into 缺少 self"));
    };
    let parsed = struct_format(instance, this)?;
    let (Some(buffer), Some(offset)) = (rest.first().copied(), rest.get(1).copied()) else {
        return Err(instance.raise_builtin_error("TypeError", "Struct.pack_into 要 2 个实参"));
    };
    let start = instance
        .int_of(offset)
        .and_then(|value| value.to_i64())
        .unwrap_or(0)
        .max(0) as usize;
    let bytes = pack_values(instance, &parsed, &rest[2..])?;
    if !instance.write_bytearray_at(buffer, start, &bytes) {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!(
                "pack_into requires a writable bytearray of at least {} bytes at offset {}",
                start + bytes.len(),
                start
            ),
        ));
    }
    Ok(instance.retain(instance.singletons().none()))
}

fn register(
    instance: &Instance,
    namespace: NonNull<pyawa_core::Header>,
    name: &str,
    handler: pyawa_core::NativeFn,
) {
    let function = crate::builtins_module::make_native(instance, name, handler);
    instance.dict_set(namespace, name, function);
}

/// **建 `_struct` 模块的命名空间** ✓（新引用 ✓）。
pub fn build(instance: &Instance) -> NonNull<pyawa_core::Header> {
    let namespace = instance.new_dict();
    register(instance, namespace, "calcsize", calcsize_native as pyawa_core::NativeFn);
    register(instance, namespace, "pack", pack_native as pyawa_core::NativeFn);
    register(instance, namespace, "pack_into", pack_into_native as pyawa_core::NativeFn);
    register(instance, namespace, "unpack", unpack_native as pyawa_core::NativeFn);
    register(instance, namespace, "unpack_from", unpack_from_native as pyawa_core::NativeFn);
    register(instance, namespace, "iter_unpack", iter_unpack_native as pyawa_core::NativeFn);
    register(instance, namespace, "_clearcache", clear_cache_native as pyawa_core::NativeFn);

    // **`Struct` 类** ✓（与 `_random.Random` 同一手法 ✓）。
    let object_type = instance.type_named("object").expect("object 已登记");
    let class_namespace = instance.new_dict();
    for (name, handler) in [
        ("__init__", struct_init_native as pyawa_core::NativeFn),
        ("pack", struct_pack_native as pyawa_core::NativeFn),
        ("unpack", struct_unpack_native as pyawa_core::NativeFn),
        ("pack_into", struct_pack_into_native as pyawa_core::NativeFn),
        ("unpack_from", struct_unpack_from_native as pyawa_core::NativeFn),
    ] {
        let function = crate::builtins_module::make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    // `iter_unpack` 复用模块级实现 ✓（返回列表，见上面的如实口径 ✓）。
    let iter_unpack = crate::builtins_module::make_native(
        instance,
        "iter_unpack",
        iter_unpack_native as pyawa_core::NativeFn,
    );
    instance.dict_set(class_namespace, "iter_unpack", iter_unpack);
    if let Ok(struct_type) = pyawa_core::build_class_from_parts(
        instance,
        "Struct".to_owned(),
        vec![object_type.cast()],
        class_namespace,
        None,
    ) {
        instance.dict_set(namespace, "Struct", struct_type);
    }

    // **`error`** ✓：见文件头的如实口径 ✗ —— 就是 `ValueError` ✓（`except struct.error` 照接 ✓）。
    if let Some(error_type) = instance.type_named("ValueError") {
        instance.retain(error_type.cast());
        instance.dict_set(namespace, "error", error_type.cast());
    }
    let exports = instance.new_list(
        [
            "calcsize", "pack", "pack_into", "unpack", "unpack_from", "iter_unpack",
            "Struct", "error",
        ]
        .iter()
        .map(|name| instance.new_str(name))
        .collect(),
    );
    instance.dict_set(namespace, "__all__", exports);
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    namespace
}
