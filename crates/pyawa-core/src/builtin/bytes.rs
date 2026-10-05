//! **`bytes` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `bytes_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`starts_ends_with` ✓。

use core::ptr::NonNull;

use crate::builtin_objects::starts_ends_with;
use crate::header::Header;
use crate::instance::Instance;

/// 从绑定方法拿 `self` 的**字节载荷**（所有 `bytes` 方法的第一句）。
pub(crate) fn bytes_receiver(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<Vec<u8>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "bytes 的方法需要 self",
        });
    };
    Ok(instance
        .bytes_value(this)
        .map(<[u8]>::to_vec)
        .unwrap_or_default())
}
/// 取一个必须是 `bytes` 的实参；不是就按实测的 `TypeError` 报。
pub(crate) fn bytes_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    index: usize,
) -> Result<Vec<u8>, crate::ExecError> {
    let Some(argument) = args.get(index) else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "这个方法少给了实参（参数个数消息随后补）",
        });
    };
    let Some(value) = instance.bytes_value(*argument) else {
        let name = instance.type_name(instance.type_of(*argument));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("a bytes-like object is required, not '{name}'"),
        ));
    };
    Ok(value.to_vec())
}
/// `bytes.hex()`（实测 `b'abc'.hex() == '616263'`）。
pub(crate) fn bytes_hex_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let text: String = value.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(instance.new_str(&text))
}
/// `bytes.decode(encoding='utf-8')`：第一刀只认 UTF-8；其余编码按实测报 `LookupError`。
pub(crate) fn bytes_decode_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let encoding = match args.first() {
        None => "utf-8".to_owned(),
        Some(argument) => match instance.text_value(*argument) {
            Some(text) => text,
            None => {
                let name = instance.type_name(instance.type_of(*argument));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("decode() argument 'encoding' must be str, not {name}"),
                ));
            }
        },
    };
    match encoding.to_ascii_lowercase().replace('_', "-").as_str() {
        "utf-8" | "utf8" | "u8" => match String::from_utf8(value) {
            Ok(text) => Ok(instance.new_str(&text)),
            Err(_) => Err(instance.raise_builtin_error(
                "UnicodeDecodeError",
                "'utf-8' codec can't decode the given bytes",
            )),
        },
        other => Err(instance.raise_builtin_error(
            "LookupError",
            &format!("unknown encoding: {other}"),
        )),
    }
}
/// `bytes.maketrans(from, to)` ⇒ **256 字节**的查表 ✓（见上）。
pub fn bytes_maketrans_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "maketrans() takes exactly 2 arguments (1 given)",
        ));
    }
    let Some(from) = instance.bytes_value(args[0]).map(<[u8]>::to_vec) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "maketrans() argument 1 must be bytes",
        ));
    };
    let Some(to) = instance.bytes_value(args[1]).map(<[u8]>::to_vec) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "maketrans() argument 2 must be bytes",
        ));
    };
    if from.len() != to.len() {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "maketrans arguments must have same length",
        ));
    }
    if from.len() > 256 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "maketrans() arguments must be at most 256 bytes long",
        ));
    }
    let mut table = [0u8; 256];
    for (index, slot) in table.iter_mut().enumerate() {
        *slot = index as u8;
    }
    for (source, target) in from.iter().zip(to.iter()) {
        table[*source as usize] = *target;
    }
    Ok(instance.new_bytes(&table))
}
pub(crate) fn bytes_startswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    starts_ends_with(instance, bound, args, false)
}
pub(crate) fn bytes_endswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    starts_ends_with(instance, bound, args, true)
}
/// `bytes.find(sub)`：找到给下标、找不到给 `-1`（实测）。
pub(crate) fn bytes_find_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    let found = if needle.is_empty() {
        Some(0)
    } else {
        value
            .windows(needle.len())
            .position(|window| window == needle.as_slice())
    };
    Ok(instance.new_int(found.map_or(-1, |position| position as i64)))
}
/// `bytes.count(sub)`：**不重叠**计数（实测 `b'aaa'.count(b'aa') == 1`）。
pub(crate) fn bytes_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    if needle.is_empty() {
        return Ok(instance.new_int(value.len() as i64 + 1));
    }
    let mut count = 0i64;
    let mut at = 0usize;
    while at + needle.len() <= value.len() {
        if value[at..at + needle.len()] == needle[..] {
            count += 1;
            at += needle.len();
        } else {
            at += 1;
        }
    }
    Ok(instance.new_int(count))
}
/// `bytes.replace(old, new)`：全部替换。
pub(crate) fn bytes_replace_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let old = bytes_argument(instance, args, 0)?;
    let new = bytes_argument(instance, args, 1)?;
    if old.is_empty() {
        // 实测：`b'abc'.replace(b'', b'x') == b'xaxbxcx'`（每字节之间插一遍，两端也插）
        // **别让"被污染的长度"在这里炸成 `capacity overflow` panic** ✓（第 85 轮）：
        // 上限榜 `-6`（SIGABRT）族在隔离档下正是报在 `capacity overflow` 上 ✓，而全 `crates/` 里
        // 只有这一处是**乘积**形状 ✓ ⇒ 就是它 ✓。乘积能爆 ⇒ 说明**长度字段本身已被污染** ✓
        // （僵尸写，见 `docs/ROUNDS.md` 第 83／84 轮 ✓）—— 这里只做**有界**处理 ✓：
        // 算不出合理容量就**不预留** ✓（正确性不受影响 ✓，`push` 自己会增长 ✓）。
        let capacity = value
            .len()
            .checked_mul(new.len().saturating_add(1))
            .and_then(|size| size.checked_add(new.len()))
            .filter(|size| *size <= (1usize << 40))
            .unwrap_or(0);
        let mut out: Vec<u8> = Vec::with_capacity(capacity);
        out.extend_from_slice(&new);
        for byte in &value {
            out.push(*byte);
            out.extend_from_slice(&new);
        }
        return Ok(instance.new_bytes(&out));
    }
    let mut out: Vec<u8> = Vec::with_capacity(value.len());
    let mut at = 0usize;
    while at < value.len() {
        if at + old.len() <= value.len() && value[at..at + old.len()] == old[..] {
            out.extend_from_slice(&new);
            at += old.len();
        } else {
            out.push(value[at]);
            at += 1;
        }
    }
    Ok(instance.new_bytes(&out))
}
/// `bytes.upper()`／`lower()`：**只动 ASCII 字母**（实测）。
pub(crate) fn bytes_case_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    upper: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let mapped: Vec<u8> = value
        .into_iter()
        .map(|byte| if upper { byte.to_ascii_uppercase() } else { byte.to_ascii_lowercase() })
        .collect();
    Ok(instance.new_bytes(&mapped))
}
pub(crate) fn bytes_upper_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_case_native(instance, bound, true)
}
pub(crate) fn bytes_lower_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_case_native(instance, bound, false)
}
/// `bytes.strip()`：去掉两端的 **ASCII 空白**（实测 `b'  ab  '.strip() == b'ab'`）。
pub(crate) fn bytes_strip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    // 不带实参 ⇒ 去 ASCII 空白；带实参 ⇒ 那个**字节集合**（实测 `b'  ab  '.strip(b'a')`
    // 原样返回——空白不在集合里）
    let cut: Option<Vec<u8>> = match args.first() {
        None => None,
        Some(_) => Some(bytes_argument(instance, args, 0)?),
    };
    let is_cut = |byte: u8| match &cut {
        None => byte.is_ascii_whitespace(),
        Some(set) => set.contains(&byte),
    };
    let start = value.iter().position(|byte| !is_cut(*byte)).unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|byte| !is_cut(*byte))
        .map_or(start, |position| position + 1);
    Ok(instance.new_bytes(&value[start..end]))
}
/// `bytes.split(sep)`：按分隔符切开，给 `list[bytes]`。
pub(crate) fn bytes_split_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let separator = bytes_argument(instance, args, 0)?;
    if separator.is_empty() {
        // 实测：`b'abc'.split(b'')` ⇒ `ValueError: empty separator`
        return Err(instance.raise_builtin_error("ValueError", "empty separator"));
    }
    let mut parts: Vec<NonNull<Header>> = Vec::new();
    let mut start = 0usize;
    let mut at = 0usize;
    while at + separator.len() <= value.len() {
        if value[at..at + separator.len()] == separator[..] {
            parts.push(instance.new_bytes(&value[start..at]));
            at += separator.len();
            start = at;
        } else {
            at += 1;
        }
    }
    parts.push(instance.new_bytes(&value[start..]));
    Ok(instance.new_list(parts))
}
/// `bytes.join(iterable)`：把一串 `bytes` 用自己接起来。
pub(crate) fn bytes_join_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let separator = bytes_receiver(instance, bound)?;
    let Some(iterable) = args.first() else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "bytes.join 少给了实参",
        });
    };
    let items = instance.collect_iterable(*iterable)?;
    let mut out: Vec<u8> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.extend_from_slice(&separator);
        }
        let Some(value) = instance.bytes_value(*item) else {
            let name = instance.type_name(instance.type_of(*item));
            // 实测：`b','.join([1])` ⇒ `sequence item 0: expected a bytes-like object, int found`
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("sequence item {index}: expected a bytes-like object, {name} found"),
            ));
        };
        out.extend_from_slice(value);
    }
    Ok(instance.new_bytes(&out))
}
/// 找子串的**位置表**（`find`／`rfind` 共用；空针返回 `0`／`len`）。
pub(crate) fn bytes_occurrences(value: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() {
        return vec![0];
    }
    value
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(index, _)| index)
        .collect()
}
/// `bytes.rfind(sub)`：**最后一个**位置，找不到 `-1`。
pub(crate) fn bytes_rfind_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    let found = bytes_occurrences(&value, &needle).last().copied();
    Ok(instance.new_int(found.map_or(-1, |position| position as i64)))
}
/// `bytes.index(sub)`／`rindex(sub)`：与 `find`／`rfind` 同，但找不到报
/// 实测的 `ValueError: subsection not found`。
pub(crate) fn bytes_index_like(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    from_end: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    let occurrences = bytes_occurrences(&value, &needle);
    let found = if from_end { occurrences.last() } else { occurrences.first() };
    match found {
        Some(position) => Ok(instance.new_int(*position as i64)),
        None => Err(instance.raise_builtin_error("ValueError", "subsection not found")),
    }
}
pub(crate) fn bytes_index_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_index_like(instance, bound, args, false)
}
pub(crate) fn bytes_rindex_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_index_like(instance, bound, args, true)
}
/// `bytes.removeprefix(p)`／`removesuffix(s)`（实测：没有该前后缀时**原样返回**）。
pub(crate) fn bytes_remove_affix(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    suffix: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let affix = bytes_argument(instance, args, 0)?;
    let trimmed = if suffix {
        value
            .strip_suffix(affix.as_slice())
            .map(<[u8]>::to_vec)
            .unwrap_or(value)
    } else {
        value
            .strip_prefix(affix.as_slice())
            .map(<[u8]>::to_vec)
            .unwrap_or(value)
    };
    Ok(instance.new_bytes(&trimmed))
}
pub(crate) fn bytes_removeprefix_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_remove_affix(instance, bound, args, false)
}
pub(crate) fn bytes_removesuffix_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_remove_affix(instance, bound, args, true)
}
/// `bytes.lstrip()`／`rstrip()`（与 `strip` 同一套"无实参 ⇒ ASCII 空白，有实参 ⇒ 字节集合"）。
pub(crate) fn bytes_strip_side(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    left: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let cut: Option<Vec<u8>> = match args.first() {
        None => None,
        Some(_) => Some(bytes_argument(instance, args, 0)?),
    };
    let is_cut = |byte: u8| match &cut {
        None => byte.is_ascii_whitespace(),
        Some(set) => set.contains(&byte),
    };
    let kept = if left {
        let start = value.iter().position(|byte| !is_cut(*byte)).unwrap_or(value.len());
        &value[start..]
    } else {
        let end = value
            .iter()
            .rposition(|byte| !is_cut(*byte))
            .map_or(0, |position| position + 1);
        &value[..end]
    };
    Ok(instance.new_bytes(kept))
}
pub(crate) fn bytes_lstrip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_strip_side(instance, bound, args, true)
}
pub(crate) fn bytes_rstrip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_strip_side(instance, bound, args, false)
}
/// `bytes.zfill(width)`：左边补 `0`（有符号时符号在最前，实测 `b'-12'.zfill(5) == b'-0012'`）。
pub(crate) fn bytes_zfill_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let Some(width) = args.first().and_then(|arg| instance.int_of(*arg)) else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "bytes.zfill 少给了宽度的实参",
        });
    };
    let Some(width) = width.to_i64().filter(|width| *width > 0) else {
        return Ok(instance.new_bytes(&value));
    };
    let width = width as usize;
    if value.len() >= width {
        return Ok(instance.new_bytes(&value));
    }
    let missing = width - value.len();
    let (sign, digits) = match value.first() {
        Some(b'+') | Some(b'-') => (Some(value[0]), &value[1..]),
        _ => (None, &value[..]),
    };
    let mut out: Vec<u8> = Vec::with_capacity(width);
    if let Some(sign) = sign {
        out.push(sign);
    }
    out.extend(core::iter::repeat_n(b'0', missing));
    out.extend_from_slice(digits);
    Ok(instance.new_bytes(&out))
}
/// `bytes.splitlines()`：按 `\n`／`\r\n`／`\r` 切（不保留行尾）。
pub(crate) fn bytes_splitlines_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let mut parts: Vec<NonNull<Header>> = Vec::new();
    let mut start = 0usize;
    let mut at = 0usize;
    while at < value.len() {
        match value[at] {
            b'\n' => {
                parts.push(instance.new_bytes(&value[start..at]));
                at += 1;
                start = at;
            }
            b'\r' => {
                parts.push(instance.new_bytes(&value[start..at]));
                at += if value.get(at + 1) == Some(&b'\n') { 2 } else { 1 };
                start = at;
            }
            _ => at += 1,
        }
    }
    // 末尾没有换行符时还有一段
    if start < value.len() {
        parts.push(instance.new_bytes(&value[start..]));
    }
    Ok(instance.new_list(parts))
}
/// `bytes.isdigit()`／`isspace()`：**整串非空且全为**对应字符（实测）。
pub(crate) fn bytes_all_are(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    predicate: fn(u8) -> bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let matched = !value.is_empty() && value.iter().all(|byte| predicate(*byte));
    Ok(instance.retain(instance.singletons().boolean(matched)))
}
pub(crate) fn bytes_isdigit_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_all_are(instance, bound, |byte| byte.is_ascii_digit())
}
pub(crate) fn bytes_isspace_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_all_are(instance, bound, |byte| byte.is_ascii_whitespace())
}
