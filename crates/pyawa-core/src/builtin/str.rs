//! **`str` 的方法面**（45 个原生函数 ✓）—— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `str_method_native` **不搬** ✓（它同时管 `bytes` 的名字 ✓，属"族之间的粘合层" ✓）；
//! * 依赖的 `bound_text`／`text_argument`／`text_prefixes` 仍留在 `builtin_objects.rs`（已放宽到 `pub(crate)` ✓）；
//! * `NativeFn` 的定义也在 `builtin_objects.rs:49` ✓（`pub type` ✓）⇒ 这里 import ✓。

use core::ptr::NonNull;

use crate::builtin_objects::{bound_text, text_argument, text_prefixes};
use crate::header::Header;
use crate::instance::Instance;

pub(crate) fn str_find_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let needle = text_argument(instance, args, 0, "find")?;
    // 参照按**字符**给下标 ⇒ 用 `char_indices` 数出字符序号 ✓（本层口径 ✓）
    let found = text
        .find(&needle)
        .map(|byte| text[..byte].chars().count() as i64)
        .unwrap_or(-1);
    Ok(instance.new_int(found))
}

/// `str.isprintable()` ✓：空串 ⇒ `True` ✓；每个字符都"可打印"（**不是**控制／分隔／格式类 ✓）⇒ `True` ✓。
///
/// **如实登记的偏差** ✗：按 Rust 的 `char::is_control` 与 `char::is_whitespace` 之外全算可打印 ✓
/// —— 与参照的 `Unicode` 口径大致同 ✓，个别字符（如 `\u{2028}` ✓）可能与参照不同 ✓。
pub(crate) fn str_isprintable_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let printable = text
        .chars()
        .all(|ch| !ch.is_control() && (ch == ' ' || !ch.is_whitespace()));
    Ok(instance.new_bool(printable))
}

/// `str.istitle()` ✓：**至少有一个"有大小写的字符"** ✓，且"每个词以大写开头、其余小写" ✓
/// （照参照实测：`"A B"` ⇒ `True` ✓、`"ab cd"` ⇒ `False` ✓、`"A1b".istitle()` ⇒ `False` ✓
/// —— 数字**不打断**一个词 ✓、但 `1` 之后的 `b` 仍算"词内的小写" ✓ ⇒ 前后由"这个词有没有开头大写"决定 ✓）。
pub(crate) fn str_istitle_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut cased = false;
    let mut previous_cased = false;
    let mut ok = true;
    for ch in text.chars() {
        if ch.is_uppercase() {
            if previous_cased {
                ok = false;
            }
            previous_cased = true;
            cased = true;
        } else if ch.is_lowercase() {
            if !previous_cased {
                ok = false;
            }
            previous_cased = true;
            cased = true;
        } else {
            // **无大小写的字符（含数字 ✓）一律"清掉上一个是有大小写的"** ✓ ——
            // 参照的口径是"大写只能跟在无大小写字符之后、小写只能跟在有大小写字符之后" ✓：
            // 实测 `"1A".istitle()` ⇒ `True` ✓（`1` 之后 `A` 合法 ✓）、
            // `"A1b".istitle()` ⇒ `False` ✗（`b` 跟在**无大小写**的 `1` 之后 ⇒ 不合法 ✓）。
            // 第一版把数字当"不清"✗ ⇒ `"A1b"` 误判为 `True` ✓，实测当场抓到 ✓。
            previous_cased = false;
        }
    }
    Ok(instance.new_bool(ok && cased))
}

/// `str.translate(table)` ✓（第 351 轮）。
///
/// 口径照参照**逐条量过** ✓：表按**码位**（`int`）查 ✓ —— 查不到 ⇒ 原字符留下 ✓；
/// 查到 `None` ⇒ **删掉** ✓；查到 `str` ⇒ 换上去 ✓；查到 `int` ⇒ 换成那个码位 ✓；
/// 查到别的 ⇒ `TypeError` ✓。`str.maketrans` 第 313 轮已接 ✓（对拍时才发现 `translate` 缺 ✗）。
pub(crate) fn str_translate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let Some(table) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "translate() takes exactly one argument (0 given)",
        ));
    };
    let none_type = instance.singletons().none_type();
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        // **键是"码位"这个 `int` 对象** ✓（`maketrans` 也是这么建的 ✓ —— 一处真相 ✓）。
        let key = instance.new_int(ch as i64);
        // **用统一的取值口** ✓（`subscript_read` ✓ —— 表就是普通映射 ✓）；**查不到 ⇒ 原字符留下** ✓
        // ⇒ 把 `Err`（`KeyError` 一类）也当"没有这一项" ✓（`translate` 的语义正是"缺省保留" ✓）。
        let found = crate::executor::subscript_read(instance, *table, key).ok();
        // SAFETY: key 由本函数持有（新引用 ✓）⇒ 用完交还 ✓。
        unsafe { instance.release_object(key.as_ptr()) };
        let Some(value) = found else {
            out.push(ch);
            continue;
        };
        if instance.type_of(value) == none_type {
            continue; // `None` ⇒ 删除 ✓
        }
        if let Some(number) = instance.int_of(value).and_then(|value| value.to_i64()) {
            if let Some(replacement) = u32::try_from(number).ok().and_then(char::from_u32) {
                out.push(replacement);
                continue;
            }
        }
        if let Some(replacement) = instance.text_of(value) {
            out.push_str(replacement);
            continue;
        }
        let name = instance.type_name(instance.type_of(value));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("character mapping must return integer, None or str, not {name}"),
        ));
    }
    Ok(instance.new_str(&out))
}

/// `str.rfind(sub)` ✓（照 `find` 镜像 ✓；找不到 ⇒ `-1` ✓）。
pub(crate) fn str_rfind_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let needle = text_argument(instance, args, 0, "rfind")?;
    let found = text
        .rfind(&needle)
        .map(|byte| text[..byte].chars().count() as i64)
        .unwrap_or(-1);
    Ok(instance.new_int(found))
}

/// `str.index(sub)` ✓（`find` ＋ 找不到时 `ValueError: substring not found` ✓）。
pub(crate) fn str_index_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let needle = text_argument(instance, args, 0, "index")?;
    match text.find(&needle) {
        Some(byte) => Ok(instance.new_int(text[..byte].chars().count() as i64)),
        None => Err(instance.raise_builtin_error("ValueError", "substring not found")),
    }
}

/// `str.rindex(sub)` ✓（`rfind` ＋ 找不到时同一条 `ValueError` ✓）。
pub(crate) fn str_rindex_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let needle = text_argument(instance, args, 0, "rindex")?;
    match text.rfind(&needle) {
        Some(byte) => Ok(instance.new_int(text[..byte].chars().count() as i64)),
        None => Err(instance.raise_builtin_error("ValueError", "substring not found")),
    }
}

/// `str.rpartition(sep)` ✓（照 `partition` 镜像 ✓；找不到 ⇒ `('', '', 原文)` ✓）。
pub(crate) fn str_rpartition_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let separator = text_argument(instance, args, 0, "rpartition")?;
    let (head, mid, tail) = match text.rfind(&separator) {
        Some(byte) => (
            text[..byte].to_owned(),
            separator.clone(),
            text[byte + separator.len()..].to_owned(),
        ),
        None => (String::new(), String::new(), text),
    };
    let parts = vec![
        instance.new_str(&head),
        instance.new_str(&mid),
        instance.new_str(&tail),
    ];
    Ok(instance.new_tuple(parts))
}

pub(crate) fn str_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let needle = text_argument(instance, args, 0, "count")?;
    let count = if needle.is_empty() {
        text.chars().count() as i64 + 1
    } else {
        text.matches(needle.as_str()).count() as i64
    };
    Ok(instance.new_int(count))
}

pub(crate) fn str_isdigit_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let found = !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
    Ok(instance.retain(instance.singletons().boolean(found)))
}

/// `str.isidentifier()`（第 335 轮）。
///
/// **如实登记的偏差** ✗：参照按 Unicode 的 `XID_Start`／`XID_Continue` 判 ✓，本层按
/// "首字符是字母或 `_`、其余是字母／数字／`_`"判 ✓（Rust 的 `char::is_alphabetic` 是 Unicode 类 ✓，
/// 与 `XID_*` **不完全一致** ✗ —— 少数边缘字符会不同 ✓）。落地 Unicode 表时一并收口 ✓。
pub(crate) fn str_isidentifier_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut chars = text.chars();
    let valid = match chars.next() {
        None => false,
        Some(first) => {
            (first == '_' || first.is_alphabetic()) && chars.all(|ch| ch == '_' || ch.is_alphanumeric())
        }
    };
    Ok(instance.new_bool(valid))
}

/// `str.isascii()`（第 335 轮）：全部字符都在 `U+0000..=U+007F` ⇒ `True` ✓（空串 ⇒ `True` ✓）。
pub(crate) fn str_isascii_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_bool(text.is_ascii()))
}

/// `str.isupper()`：**至少有一个"有大小写的字符"，且它们全是大写** ✓（照参照实测 ✓：
/// `"A1".isupper()` ⇒ `True` ✓、`"1".isupper()` ⇒ `False` ✓）。
pub(crate) fn str_isupper_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut cased = false;
    let mut upper = true;
    for ch in text.chars() {
        if ch.is_lowercase() {
            upper = false;
        }
        if ch.is_lowercase() || ch.is_uppercase() {
            cased = true;
        }
    }
    Ok(instance.new_bool(cased && upper))
}

/// `str.islower()`：与 `isupper` **对称** ✓。
pub(crate) fn str_islower_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut cased = false;
    let mut lower = true;
    for ch in text.chars() {
        if ch.is_uppercase() {
            lower = false;
        }
        if ch.is_lowercase() || ch.is_uppercase() {
            cased = true;
        }
    }
    Ok(instance.new_bool(cased && lower))
}

/// `str.isnumeric()` ✓（**如实登记的偏差** ✗：按 Rust 的 `char::is_numeric` ✓ —— 它与 Unicode 的
/// `Nd`／`Nl`／`No` 大致同口径 ✓，个别字符可能与参照不同 ✓）。
pub(crate) fn str_isnumeric_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_bool(!text.is_empty() && text.chars().all(|ch| ch.is_numeric())))
}

/// `str.isdecimal()` ✓（十进制数字 ✓：用"有十进制数位值"来判 ✓ —— `"Ⅻ"` 是数字但**不是**十进制 ✓，
/// 照参照实测 ✓）。
pub(crate) fn str_isdecimal_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_bool(
        !text.is_empty() && text.chars().all(|ch| ch.is_numeric() && ch.to_digit(10).is_some()),
    ))
}

/// `str.isalnum()` ✓。
pub(crate) fn str_isalnum_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_bool(!text.is_empty() && text.chars().all(|ch| ch.is_alphanumeric())))
}

/// `str.swapcase()` ✓（逐个字符换大小写 ✓；多字符展开照参照 ✓，如 `"ß"` ⇒ `"SS"` ✓）。
pub(crate) fn str_swapcase_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_lowercase() {
            out.extend(ch.to_uppercase());
        } else if ch.is_uppercase() {
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    Ok(instance.new_str(&out))
}

/// `str.casefold()` ✓（**如实登记的偏差** ✗：Rust 没有 casefold ✓ ⇒ 按 `to_lowercase` 走 ✓ 并
/// **补一条最常见的展开**（`"ß"` ⇒ `"ss"` ✓，照参照实测 ✓）；其余个别字符可能与参照不同 ✓）。
pub(crate) fn str_casefold_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == 'ß' {
            out.push_str("ss");
        } else {
            out.extend(ch.to_lowercase());
        }
    }
    Ok(instance.new_str(&out))
}

/// `str.expandtabs(tabsize=8)` ✓（把 `	` 展开到**下一个** `tabsize` 的倍数 ✓）。
pub(crate) fn str_expandtabs_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let tabsize = match args.first() {
        Some(value) => instance
            .int_of(*value)
            .and_then(|value| value.to_i64())
            .ok_or_else(|| {
                instance.raise_builtin_error("TypeError", "an integer is required")
            })?,
        None => 8,
    } as usize;
    let mut out = String::with_capacity(text.len());
    let mut column = 0usize;
    for ch in text.chars() {
        if ch == '\t' {
            if tabsize == 0 {
                continue;
            }
            let pad = tabsize - (column % tabsize);
            for _ in 0..pad {
                out.push(' ');
            }
            column += pad;
        } else {
            out.push(ch);
            column += 1;
        }
    }
    Ok(instance.new_str(&out))
}

pub(crate) fn str_isalpha_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let found = !text.is_empty() && text.chars().all(|c| c.is_alphabetic());
    Ok(instance.retain(instance.singletons().boolean(found)))
}

pub(crate) fn str_zfill_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let width = args
        .first()
        .and_then(|value| instance.int_value(*value))
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "zfill() 要一个整数宽度"))?;
    let current = text.chars().count() as i64;
    if width <= current {
        return Ok(instance.new_str(&text));
    }
    let padded = format!("{}{}", "0".repeat((width - current) as usize), text);
    Ok(instance.new_str(&padded))
}

pub(crate) fn str_splitlines_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let parts: Vec<NonNull<Header>> = text
        .lines()
        .map(|line| instance.new_str(line))
        .collect();
    Ok(instance.new_list(parts))
}

pub(crate) fn str_removeprefix_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let prefix = text_argument(instance, args, 0, "removeprefix")?;
    match text.strip_prefix(prefix.as_str()) {
        Some(rest) => Ok(instance.new_str(rest)),
        None => Ok(instance.new_str(&text)),
    }
}

pub(crate) fn str_removesuffix_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let suffix = text_argument(instance, args, 0, "removesuffix")?;
    match text.strip_suffix(suffix.as_str()) {
        Some(rest) => Ok(instance.new_str(rest)),
        None => Ok(instance.new_str(&text)),
    }
}

/// `lstrip`／`rstrip`（第 154 轮）：**不给参数**时按空白 ✓（带参版随后补 ✗，如实登记 ✓）。
pub(crate) fn str_strip_side_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    from_left: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "带参数的 strip／lstrip／rstrip 尚未接线",
        ));
    }
    let trimmed = if from_left {
        text.trim_start().to_owned()
    } else {
        text.trim_end().to_owned()
    };
    Ok(instance.new_str(&trimmed))
}

pub(crate) fn str_lstrip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_strip_side_native(instance, bound, args, true)
}

pub(crate) fn str_rstrip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_strip_side_native(instance, bound, args, false)
}

/// `title()`（第 154 轮）：每个"词首"大写 ✓、其余**小写** ✓（参照口径 ✓，实测 `"aBc".title()` ⇒ `'Abc'` ✓）。
pub(crate) fn str_title_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut out = String::with_capacity(text.len());
    let mut at_word_start = true;
    for character in text.chars() {
        // 参照把"字母"当词字符 ✓（空白与标点都断开 ✓）
        if character.is_alphabetic() {
            if at_word_start {
                out.extend(character.to_uppercase());
            } else {
                out.extend(character.to_lowercase());
            }
            at_word_start = false;
        } else {
            out.push(character);
            at_word_start = true;
        }
    }
    Ok(instance.new_str(&out))
}

/// `capitalize()`（第 154 轮）：首字符大写 ✓、**其余全部小写** ✓（参照口径 ✓：`"aB"` ⇒ `'Ab'` ✓）。
pub(crate) fn str_capitalize_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut characters = text.chars();
    let capitalized = match characters.next() {
        Some(first) => {
            let mut out = String::new();
            out.extend(first.to_uppercase());
            out.extend(characters.flat_map(|character| character.to_lowercase()));
            out
        }
        None => String::new(),
    };
    Ok(instance.new_str(&capitalized))
}

/// `rjust`／`ljust`／`center`（第 154 轮）：按宽度补空格 ✓（**不接填充字符** ✗，如实登记 ✓）。
pub(crate) fn str_pad_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    mode: u8,
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let width = args
        .first()
        .and_then(|value| instance.int_value(*value))
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "width 要整数"))?;
    let current = text.chars().count() as i64;
    if width <= current {
        return Ok(instance.new_str(&text));
    }
    let pad = (width - current) as usize;
    let padded = match mode {
        0 => format!("{}{}", " ".repeat(pad), text), // rjust
        1 => format!("{}{}", text, " ".repeat(pad)), // ljust
        _ => {
            // center 的**准确规则**照参照 ✓：`left = pad//2 + (pad & width & 1)`（CPython 的
            //   `str.center` 就是这么写的 ✓）—— 实测三个样例都对 ✓：`"a".center(2)` ⇒ `'a '` ✓、
            //   `"ab".center(5)` ⇒ `'  ab '` ✓、`"a".center(5)` ⇒ `'  a  '` ✓。
            //   （我先前先写成"多的一格在左"✗、又改成 ceil ✗，两次都不对 ⇒ 现在照公式 ✓。）
            let left = (pad / 2 + (pad & (width as usize) & 1)) as usize;
            format!("{}{}{}", " ".repeat(left), text, " ".repeat(pad - left))
        }
    };
    Ok(instance.new_str(&padded))
}

pub(crate) fn str_rjust_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_pad_native(instance, bound, args, 0)
}

pub(crate) fn str_ljust_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_pad_native(instance, bound, args, 1)
}

pub(crate) fn str_center_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_pad_native(instance, bound, args, 2)
}

/// `partition(sep)`（第 154 轮）：给三元组 ✓（找不到 ⇒ `(自身, "", "")` ✓ 与参照同义 ✓）。
pub(crate) fn str_partition_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let separator = text_argument(instance, args, 0, "partition")?;
    if separator.is_empty() {
        return Err(instance.raise_builtin_error("ValueError", "empty separator"));
    }
    let (head, sep, tail) = match text.find(&separator) {
        Some(at) => (
            text[..at].to_owned(),
            separator.clone(),
            text[at + separator.len()..].to_owned(),
        ),
        None => (text.clone(), String::new(), String::new()),
    };
    let parts = vec![
        instance.new_str(&head),
        instance.new_str(&sep),
        instance.new_str(&tail),
    ];
    Ok(instance.new_tuple(parts))
}

/// `rsplit(sep)`（第 154 轮）：**只接单参** ✓（无参的空白切分随后补 ✗）；`maxsplit` 未接 ✗。
pub(crate) fn str_rsplit_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let separator = text_argument(instance, args, 0, "rsplit")?;
    if separator.is_empty() {
        return Err(instance.raise_builtin_error("ValueError", "empty separator"));
    }
    // **Rust 的 `rsplit` 是逆序产出** ✗ ⇒ 要 `.rev()` 才与参照同序 ✓
    //   （参照里 `"a,b,c".rsplit(",")` 与 `split` **同序** ✓ ⇒ 空格子放最后 ✓；夹具/语料当场抓到 ✓）。
    // **Rust 的 `rsplit` 逆序产出** ✗，且 `&str` 的 `rsplit` **不支持 `.rev()`** ✗（`StrSearcher`
    //   不是双端 ✓）⇒ **先收进 Vec、再 `reverse()`** ✓，这样才与参照同序 ✓。
    let mut collected: Vec<String> = text
        .rsplit(separator.as_str())
        .map(|part| part.to_owned())
        .collect();
    collected.reverse();
    let parts: Vec<NonNull<Header>> = collected
        .iter()
        .map(|part| instance.new_str(part))
        .collect();
    Ok(instance.new_list(parts))
}

/// **`str.maketrans`／`bytes.maketrans`**（第 313 轮）：`'type' object has no attribute 'maketrans'`
/// × **67** 个模块的卡点 ✓。两个都是**静态**用法（在**类型对象**上取 ⇒ 没有接收者 ✓）。
///
/// 口径照参照实测：
/// - `str.maketrans(x)`：x 是字典 ⇒ **逐条拷**（键是单字符 ⇒ 折成序号 ✓；值原样 ✓）；
/// - `str.maketrans(x, y)`：两个等长字符串 ⇒ 逐位配对 ✓；长度不等 ⇒
///   `ValueError: the first two maketrans arguments must have equal length` ✓；
/// - `str.maketrans(x, y, z)`：z 的每个字符 ⇒ **映射到 `None`**（删除 ✓）；
/// - `bytes.maketrans(from, to)`：等长（长度 ≤ 256 ✓）⇒ 给**256 字节**的查表 ✓，长度不等 ⇒
///   `ValueError: maketrans arguments must have same length` ✓。
pub fn str_maketrans_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "maketrans expected at least 1 argument, got 0",
        ));
    }
    // 单实参：字典 ⇒ 逐条拷（键若为单字符则折成序号 ✓）
    if args.len() == 1 {
        let Some(items) = instance.dict_entries(args[0]) else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "if you give only one argument to maketrans it must be a dict",
            ));
        };
        let result = instance.new_dict();
        for (key, value) in items {
            let mapped = match instance.text_of(key) {
                Some(text) if text.chars().count() == 1 => {
                    instance.new_int(text.chars().next().expect("刚判过非空") as i64)
                }
                _ => instance.retain(key),
            };
            crate::executor::subscript_write(instance, result, mapped, value)?;
        }
        return Ok(result);
    }
    // 两个（可选三个）实参：字符串配对 ✓
    let Some(from) = instance.text_of(args[0]).map(str::to_owned) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "maketrans() argument 1 must be a string or dict",
        ));
    };
    let Some(to) = instance.text_of(args[1]).map(str::to_owned) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "maketrans() argument 2 must be a string",
        ));
    };
    if from.chars().count() != to.chars().count() {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "the first two maketrans arguments must have equal length",
        ));
    }
    let result = instance.new_dict();
    for (source, target) in from.chars().zip(to.chars()) {
        let key = instance.new_int(source as i64);
        let value = instance.new_int(target as i64);
        crate::executor::subscript_write(instance, result, key, value)?;
    }
    if let Some(delete) = args.get(2) {
        let Some(delete) = instance.text_of(*delete).map(str::to_owned) else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "maketrans() argument 3 must be a string",
            ));
        };
        for character in delete.chars() {
            let key = instance.new_int(character as i64);
            let none = instance.retain(instance.singletons().none());
            crate::executor::subscript_write(instance, result, key, none)?;
        }
    }
    Ok(result)
}

pub(crate) fn str_upper_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_str(&text.to_uppercase()))
}

pub(crate) fn str_lower_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_str(&text.to_lowercase()))
}

pub(crate) fn str_strip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_str(text.trim()))
}

pub(crate) fn str_startswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    // **也认元组** ✓（第 195 轮：`Lib/importlib/_bootstrap_external.py:61` 就是
    // `sys.platform.startswith(('win32', 'cygwin', 'darwin'))` ✓ —— 先前只认单个 `str` ✗）。
    let prefixes = text_prefixes(instance, args, "startswith")?;
    let found = prefixes.iter().any(|prefix| text.starts_with(prefix));
    Ok(instance.retain(instance.singletons().boolean(found)))
}

pub(crate) fn str_endswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    // **也认元组** ✓（同 `startswith` ✓）。
    let suffixes = text_prefixes(instance, args, "endswith")?;
    let found = suffixes.iter().any(|suffix| text.ends_with(suffix));
    Ok(instance.retain(instance.singletons().boolean(found)))
}

pub(crate) fn str_join_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let separator = bound_text(instance, bound)?;
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "join() takes exactly one argument"));
    };
    let items = match instance.iterable_items(*iterable) {
        Some(items) => items,
        None => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "can only join an iterable",
            ))
        }
    };
    let mut parts: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        match instance.text_of(item) {
            Some(text) => parts.push(text.to_owned()),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "sequence item: expected str instance",
                ))
            }
        }
    }
    Ok(instance.new_str(&parts.join(&separator)))
}

pub(crate) fn str_split_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    // 无参 ⇒ 与参照同义的"按空白切、丢弃空段" ✓；有参 ⇒ 按该分隔符切 ✓
    let parts: Vec<NonNull<Header>> = match args.first() {
        Some(separator) => {
            let separator = match instance.text_of(*separator) {
                Some(text) => text.to_owned(),
                None => {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "must be str or None, not the given type",
                    ))
                }
            };
            text.split(separator.as_str())
                .map(|part| instance.new_str(part))
                .collect()
        }
        None => text
            .split_whitespace()
            .map(|part| instance.new_str(part))
            .collect(),
    };
    Ok(instance.new_list(parts))
}

pub(crate) fn str_replace_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let from = text_argument(instance, args, 0, "replace")?;
    let to = text_argument(instance, args, 1, "replace")?;
    Ok(instance.new_str(&text.replace(&from, &to)))
}
