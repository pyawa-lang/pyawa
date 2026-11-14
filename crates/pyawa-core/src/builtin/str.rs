//! **`str` 的方法面**（45 个原生函数 ✓）—— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `str_method_native` **不搬** ✓（它同时管 `bytes` 的名字 ✓，属"族之间的粘合层" ✓）；
//! * 依赖的 `bound_text`／`text_argument`／`text_prefixes` 仍留在 `builtin_objects.rs`（已放宽到 `pub(crate)` ✓）；
//! * `NativeFn` 的定义也在 `builtin_objects.rs:49` ✓（`pub type` ✓）⇒ 这里 import ✓。

use crate::HotId;
use core::ptr::NonNull;

use crate::builtin_objects::{bound_text, text_argument, text_prefixes, MethodObject};
use crate::executor::{
    attribute_lookup_with_getattr, format_value_with_spec, subscript_read, Attribute,
};
use crate::header::Header;
use crate::instance::Instance;

/// **`find`／`rfind`／`index`／`rindex` 的 `start`／`end` 归一化** ✓（task-22 第 2 轮真 bug 修 ✗）。
///
/// 动因：先前这四口**只看子串**、把 `start`／`end` **整个忽略** ✗ ⇒ `logging.Manager._fixupParents` 的
/// `i = name.rfind(".", 0, i - 1)` 永不前进 ⇒ **死循环** ✗（实测 `logging.getLogger("a.b")` 与
/// `import concurrent.futures` 双双挂死 ✓，挂死比报错更难查 ✓）。
///
/// 口径（照参照逐例实测 ✓）：下标按**字符**数 ✓；负数先 `+len` 再夹到 `>=0` ✓；
/// `end` 缺省是 `len` ✓；`start > end` ⇒ 找不到 ✓；空子串那条另算（见调用处 ✓）。
/// **绑定 `str` 的借用视图**（`Cow`）：合法 UTF-8 ⇒ **借用**（不拷贝 ✓）；WTF-8 表示
/// （含孤立代理，`text_of` 给 `None`）⇒ 退化成有损 `String`。
///
/// 为什么要它：`bound_text` 每口都 `to_owned()` 克隆整个接收者 ✗ —— `test_userstring` 的
/// `test_find_periodic_pattern` 对 2–4 KB 串做上千次 `startswith(p, i)` ⇒ 光拷贝就几十 MB
/// （round 40 实测 33 s → 借用后回落）。热路径（`find`／`count`／`startswith` 一族）改用它 ✓。
fn bound_text_cow<'a>(
    instance: &'a Instance,
    bound: Option<NonNull<Header>>,
) -> Result<std::borrow::Cow<'a, str>, crate::ExecError> {
    let Some(bound) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor needs an argument"));
    };
    if let Some(text) = instance.text_of(bound) {
        return Ok(std::borrow::Cow::Borrowed(text));
    }
    Ok(std::borrow::Cow::Owned(instance.text_value(bound).unwrap_or_default()))
}

/// **`str`（含孤立代理）的 WTF-8 字节**（round 107）：`text_of` 对 WTF-8 表示给 `None` ✗
/// ⇒ 字符串方法（`split`／`rfind` 一族）按**字节**处理再还原 ✓（与 `concat` 同一手法 ✓）。
fn bound_wtf8_bytes(instance: &Instance, bound: Option<NonNull<Header>>) -> Result<Vec<u8>, crate::ExecError> {
    let Some(bound) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor needs an argument"));
    };
    instance
        .wtf8_of(bound)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs a str argument"))
}

/// 把 **WTF-8 字节偏移**换成**码点数**（`str.find` 一族的下标语义 ✓；越界按全文长度 ✓）。
fn wtf8_char_index(bytes: &[u8], byte_offset: usize) -> i64 {
    let mut count = 0i64;
    let mut index = 0usize;
    while index < byte_offset.min(bytes.len()) {
        // 跳过 UTF-8／WTF-8 的一个码元序列（首字节高 1 的个数决定长度；非法字节按 1 字节走 ✓）。
        let lead = bytes[index];
        let length = if lead < 0x80 {
            1
        } else if lead & 0xE0 == 0xC0 {
            2
        } else if lead & 0xF0 == 0xE0 {
            3
        } else if lead & 0xF8 == 0xF0 {
            4
        } else {
            1
        };
        index += length.min(bytes.len() - index);
        count += 1;
    }
    count
}

/// 在字节串里找子串（`reverse=true` ⇒ 从尾部找 ✓；空 needle 按参照规则处理 ✓）。
fn find_subsequence(haystack: &[u8], needle: &[u8], reverse: bool) -> Option<usize> {
    if needle.is_empty() {
        return Some(if reverse { haystack.len() } else { 0 });
    }
    if needle.len() > haystack.len() {
        return None;
    }
    if reverse {
        (0..=haystack.len() - needle.len())
            .rev()
            .find(|&start| &haystack[start..start + needle.len()] == needle)
    } else {
        (0..=haystack.len() - needle.len())
            .find(|&start| &haystack[start..start + needle.len()] == needle)
    }
}

/// `start`／`end` 区间（`str.find` 一族的参照口径 ✓）：负数加长度、越界夹到 `0..=len` ✓；
/// `start > end` 交给调用方给 `-1`／`ValueError` ✓。缺省 `start=0`、`end=len` ✓。
/// 参数非整数／超 3 个实参 ⇒ `TypeError` ✓（`None` 当缺省 ✓ —— 参照 `find(sub, None)` 合法 ✓）。
fn text_range_bounds(
    instance: &Instance,
    args: &[NonNull<Header>],
    length: usize,
    method: &str,
) -> Result<(usize, usize), crate::ExecError> {
    let length = length as i64;
    let mut values: [Option<i64>; 2] = [None, None];
    for (index, value) in args.iter().skip(1).take(2).enumerate() {
        if instance.type_of(*value) == instance.singletons().none_type() {
            continue;
        }
        // **超出 `i64` 的整数按饱和处理**（round 107）：参照 `"ab".rfind("xxx", sys.maxsize + 1, 0)`
        // 给 **-1** ✓（start > len ⇒ 空区间）—— `to_i64()` 对大整数给 `None` ✗ ⇒ 先前误报
        // `TypeError` ✗（官方 `test_userstring.test_rfind` ✓）。
        if let Some(number) = instance.int_of(*value) {
            let saturated = number.to_i64().unwrap_or_else(|| {
                if number.to_bigint().signum() < 0 {
                    i64::MIN
                } else {
                    i64::MAX
                }
            });
            values[index] = Some(saturated);
            continue;
        }
        let name = instance.type_name(instance.type_of(*value));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{method}() 的 start／end 要整数，不是 '{name}'"),
        ));
    }
    let normalize = |value: Option<i64>, default: i64| -> i64 {
        match value {
            Some(number) => {
                let shifted = if number < 0 { number + length } else { number };
                shifted.clamp(0, length)
            }
            None => default,
        }
    };
    let start = normalize(values[0], 0);
    let end = normalize(values[1], length);
    // **`start > len` 的显式形态**：参照 `"abc".rfind("", 4)` ⇒ -1 ✓（`end` 夹到 len 后 start>end ✓）。
    let raw_start = values[0].unwrap_or(0);
    let start = if raw_start > length { length + 1 } else { start };
    Ok((start as usize, end as usize))
}

/// 用 WTF-8 字节切分（`maxsplit < 0` ⇒ 不限 ✓）；返回**字节段**列表 ✓。
fn split_wtf8(text: &[u8], separator: &[u8], maxsplit: i64) -> Vec<Vec<u8>> {
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut rest = text;
    let mut splits = 0i64;
    if maxsplit < 0 {
        while let Some(position) = find_subsequence(rest, separator, false) {
            parts.push(rest[..position].to_vec());
            rest = &rest[position + separator.len()..];
        }
        parts.push(rest.to_vec());
        return parts;
    }
    while splits < maxsplit {
        let Some(position) = find_subsequence(rest, separator, false) else {
            break;
        };
        parts.push(rest[..position].to_vec());
        rest = &rest[position + separator.len()..];
        splits += 1;
    }
    parts.push(rest.to_vec());
    parts
}

/// 空白切分（`maxsplit` 语义照参照 ✓）：按**码点**判 `char::is_whitespace` ✓（孤立代理按非空白 ✓）。
fn split_whitespace_wtf8(text: &[u8], maxsplit: i64) -> Vec<Vec<u8>> {
    // 参照口径（3.14.4 实测 ✓）：`maxsplit == 0` ⇒ **去掉前导空白**后的整段（尾随空白保留 ✓）；
    // 达到 `maxsplit` 后 ⇒ 跳过空白、剩余整段原样收尾 ✓（" a  b c  ".split(None,1) ⇒ ['a','b c  '] ✓）。
    let points: Vec<u32> = crate::wtf8::code_points(text)
        .map(|point| point.expect("刚确认是 WTF-8"))
        .collect();
    let is_space = |point: u32| char::from_u32(point).is_some_and(char::is_whitespace);
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut index = 0usize;
    let emit = |from: usize, to: usize, parts: &mut Vec<Vec<u8>>| {
        let mut bytes: Vec<u8> = Vec::new();
        for &point in &points[from..to] {
            crate::wtf8::encode_code_point(point, &mut bytes).expect("码点可编码");
        }
        parts.push(bytes);
    };
    while index < points.len() && is_space(points[index]) {
        index += 1;
    }
    if maxsplit == 0 {
        if index < points.len() {
            emit(index, points.len(), &mut parts);
        }
        return parts;
    }
    while index < points.len() {
        let start = index;
        while index < points.len() && !is_space(points[index]) {
            index += 1;
        }
        emit(start, index, &mut parts);
        while index < points.len() && is_space(points[index]) {
            index += 1;
        }
        if maxsplit > 0 && parts.len() as i64 >= maxsplit {
            if index < points.len() {
                emit(index, points.len(), &mut parts);
            }
            break;
        }
    }
    parts
}

/// **字符数**（`str` 是 UTF-8：非 ASCII 时必须数 `chars()`；**ASCII 快路径**直接 `len()` ✓）。
///
/// 为什么要它：`test_userstring` 的 `test_find_periodic_pattern` 会对 2–4 KB 的 ASCII 串做
/// 上千次 `startswith(p, i)`；先前每口都 `text.chars().count()`（O(n)）＋ 逐字符找字节偏移
/// ⇒ 整体 O(n²)，54 s 的测试一下跑到 10 分钟量级 ✗（round 40 实测）。
fn text_char_len(text: &str) -> i64 {
    if text.is_ascii() {
        text.len() as i64
    } else {
        text.chars().count() as i64
    }
}

/// **方法实参个数上限检查**（`P3-42` round 44）：参照对多给实参报
/// `<name>() takes at most N arguments (M given)` ✓ —— 报文**必须以方法名开头**（有一条官方测试
/// 正是用 `^find\b` 这类正则钉报文前缀 ✓）。
fn check_max_arguments(
    instance: &Instance,
    args: &[NonNull<Header>],
    max: usize,
    method: &str,
) -> Result<(), crate::ExecError> {
    if args.len() > max {
        // 参照口径：`find expected at most 3 arguments, got 4` ✓（无括号、无 "takes"）。
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{method} expected at most {max} arguments, got {}", args.len()),
        ));
    }
    Ok(())
}

/// **切片下标实参**（`start`／`end`）：整数（含**超出 `i64` 的大整数**，按方向饱和 —— 切片语义随后
/// 会再夹到 `len` ✓）或 `__index__` 对象 ✓；否则参照同形的 `TypeError`。
fn bound_index(
    instance: &Instance,
    object: NonNull<Header>,
    method: &str,
) -> Result<i64, crate::ExecError> {
    if let Some(number) = instance.int_of(object) {
        return Ok(number.to_i64().unwrap_or_else(|| {
            // 超出 `i64` 的整数：按方向饱和（切片语义随后夹到 `len`）。
            if number.to_bigint().signum() < 0 {
                i64::MIN
            } else {
                i64::MAX
            }
        }));
    }
    match instance.index_value(object) {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{method}: 下标要是整数"),
        )),
        Err(error) => Err(error),
    }
}

fn search_bounds(
    instance: &Instance,
    text: &str,
    args: &[NonNull<Header>],
    method: &str,
) -> Result<(i64, i64, i64), crate::ExecError> {
    let length = text_char_len(text);
    let read = |index: usize, default: i64| -> Result<i64, crate::ExecError> {
        match args.get(index) {
            None => Ok(default),
            // **`None` ＝ 缺省**（本机 3.14.4 实测：`"abc".find("b", None)` ⇒ 1 ✓；
            // `None` 在切片口径里就是"用默认边界" ✓ —— 先前落到"下标要是整数" ✗）。
            Some(value) if *value == instance.singletons().none() => Ok(default),
            Some(value) => bound_index(instance, *value, method),
        }
    };
    let start = read(1, 0)?;
    let end = read(2, length)?;
    let start = if start < 0 { start.saturating_add(length).max(0) } else { start };
    let end = if end < 0 { end.saturating_add(length).max(0) } else { end };
    // **正的 `end` 也夹到 `len`** ✓（`P3-42` round 44）：空子串的 `rfind` 走
    // `end <= length` 那条判据，而 `UserString.rfind(sub, 0)` 传的是 `end=sys.maxsize`
    // ⇒ 不夹就误判成 -1 ✗（参照 `'abc'.rfind('', 0)` ⇒ 3 ✓）。
    let end = end.clamp(0, length);
    Ok((start, end, length))
}

/// 字符下标 ⇒ 字节偏移 ✓（越界按末尾 ✓）。
fn char_byte_offset(text: &str, char_index: i64) -> usize {
    if char_index <= 0 {
        return 0;
    }
    // **ASCII 快路径**：字节偏移＝字符下标 ✓（见 [`text_char_len`] 的动因）。
    if text.is_ascii() {
        return (char_index as usize).min(text.len());
    }
    match text.char_indices().nth(char_index as usize) {
        Some((byte, _)) => byte,
        None => text.len(),
    }
}

/// **`startswith`／`endswith` 的窗口**（`P3-41` round 40 修 ✗）：`start`／`end` 必须生效。
///
/// 参照口径（本机 3.14.4 逐例实测）：下标按**字符**数 ✓；负数先 `+len`、再夹到 `>=0` ✓；
/// **`start > len` ⇒ 窗口无效**（连空串也一样 —— `"abc".startswith("", 5)` ⇒ `False` ✓；
/// 这点与 `find` 的空子串口径不同）；`end` 缺省 `len`、超界夹到 `len` ✓；`start > end` ⇒ 无效 ✓；
/// `None` ＝ 缺省 ✓（3.14 的切片口径）。
fn prefix_suffix_window<'a>(
    instance: &Instance,
    text: &'a str,
    args: &[NonNull<Header>],
    method: &str,
) -> Result<Option<&'a str>, crate::ExecError> {
    let length = text_char_len(text);
    let read = |index: usize, default: i64| -> Result<i64, crate::ExecError> {
        match args.get(index) {
            None => Ok(default),
            Some(value) if *value == instance.singletons().none() => Ok(default),
            Some(value) => bound_index(instance, *value, method),
        }
    };
    let start = read(1, 0)?;
    let end = read(2, length)?;
    let start = if start < 0 { start + length } else { start };
    if start > length {
        return Ok(None);
    }
    let start = start.max(0);
    let end = if end < 0 { end + length } else { end };
    let end = end.clamp(0, length);
    if start > end {
        return Ok(None);
    }
    let byte_start = char_byte_offset(text, start);
    let byte_end = char_byte_offset(text, end);
    Ok(Some(&text[byte_start..byte_end]))
}

/// **四口共用的搜索** ✓：`reverse` 走 `rfind` 语义 ✓；回**字符**下标 ✓（找不到 `None` ✓）。
fn text_search(
    instance: &Instance,
    text: &str,
    needle: &str,
    args: &[NonNull<Header>],
    method: &str,
    reverse: bool,
) -> Result<Option<i64>, crate::ExecError> {
    let (start, end, length) = search_bounds(instance, text, args, method)?;
    if needle.is_empty() {
        // 空子串（照参照实测 ✓）：`find` 回 `start`（`start <= len` 且 `start <= end` ✓）；
        // `rfind` 回 `end`（`end <= len` 且 `start <= end` ✓）。
        return if reverse {
            if end <= length && start <= end {
                Ok(Some(end))
            } else {
                Ok(None)
            }
        } else if start <= length && start <= end {
            Ok(Some(start))
        } else {
            Ok(None)
        };
    }
    if start > end {
        return Ok(None);
    }
    let byte_start = char_byte_offset(text, start);
    let byte_end = char_byte_offset(text, end.min(length));
    if byte_start > byte_end {
        return Ok(None);
    }
    let window = &text[byte_start..byte_end];
    let found = if reverse { window.rfind(needle) } else { window.find(needle) };
    // **ASCII 快路径**：窗口是 ASCII 时字节下标＝字符下标 ✓。
    Ok(found.map(|byte| {
        if window.is_ascii() {
            start + byte as i64
        } else {
            start + window[..byte].chars().count() as i64
        }
    }))
}

pub(crate) fn str_find_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
        check_max_arguments(instance, args, 3, "find")?;
    let text = bound_text_cow(instance, bound)?;
    let needle = text_argument(instance, args, 0, "find")?;
    // 参照按**字符**给下标 ✓；`start`／`end` 照参照归一化 ✓（task-22 第 2 轮修 ✗）。
    let found = text_search(instance, &text, &needle, args, "find", false)?.unwrap_or(-1);
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
    // **WTF-8 码点层**（round 107）：含孤立代理的串 `text_of` 给 `None` ✗ ⇒ 先前
    // `bound_text` 退化成空串／丢代理 ✗；`re.escape` 的 `str.translate` 正走这里 ✓
    //（官方 `test_unicode_file` 的 `TESTFN_UNENCODABLE` ✓）。
    let source = bound_wtf8_bytes(instance, bound)?;
    let Some(table) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "translate() takes exactly one argument (0 given)",
        ));
    };
    let none_type = instance.singletons().none_type();
    let mut out: Vec<u8> = Vec::with_capacity(source.len());
    for point in crate::wtf8::code_points(&source) {
        let ch = point.expect("刚确认是 WTF-8");
        // **键是"码位"这个 `int` 对象** ✓（`maketrans` 也是这么建的 ✓ —— 一处真相 ✓）。
        let key = instance.new_int(ch as i64);
        // **用统一的取值口** ✓（`subscript_read` ✓ —— 表就是普通映射 ✓）；**查不到 ⇒ 原字符留下** ✓
        // ⇒ 把 `Err`（`KeyError` 一类）也当"没有这一项" ✓（`translate` 的语义正是"缺省保留" ✓）。
        // **`Err` 也要还**（task-18 `.ok()` 族）：`translate` 的语义是"查不到就原样保留"。
        // 先前 `.ok()` 把 `Err(Raised)` 的异常引用丢掉（用户映射的 `__getitem__` 抛错时漏一颗）。
        let found = match crate::executor::subscript::subscript_read(instance, *table, key) {
            Ok(value) => Some(value),
            Err(error) => {
                crate::executor::runtime::discard_error(instance, &error);
                None
            }
        };
        // SAFETY: key 由本函数持有（新引用 ✓）⇒ 用完交还 ✓。
        unsafe { instance.release_object(key.as_ptr()) };
        let Some(value) = found else {
            crate::wtf8::encode_code_point(ch, &mut out).expect("码点可编码");
            continue;
        };
        if instance.type_of(value) == none_type {
            continue; // `None` ⇒ 删除 ✓
        }
        if let Some(number) = instance.int_of(value).and_then(|value| value.to_i64()) {
            if let Ok(replacement) = u32::try_from(number) {
                crate::wtf8::encode_code_point(replacement, &mut out).expect("码点可编码");
                continue;
            }
        }
        if let Some(replacement) = instance.wtf8_of(value) {
            out.extend_from_slice(replacement);
            continue;
        }
        let name = instance.type_name(instance.type_of(value));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("character mapping must return integer, None or str, not {name}"),
        ));
    }
    Ok(instance
        .new_str_wtf8(&out)
        .expect("translate 的输出仍是合法 WTF-8"))
}

/// `str.rfind(sub)` ✓（照 `find` 镜像 ✓；找不到 ⇒ `-1` ✓）。
pub(crate) fn str_rfind_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
        check_max_arguments(instance, args, 3, "rfind")?;
    // **WTF-8 字节层搜索＋`start`／`end` 区间**（round 107）：含孤立代理的串 `text_of` 给 `None` ✗；
    // 且区间语义要照参照（`"abc".rfind("", 4)` ⇒ **-1** ✓ —— 先前一律给 3 ✗，官方夹具
    // `equality_and_str_semantics` 当场红 ✓）。
    let text = bound_wtf8_bytes(instance, bound)?;
    let Some(needle_value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "rfind() takes at least 1 argument (0 given)",
        ));
    };
    let Some(needle) = instance.wtf8_of(needle_value) else {
        return Err(instance.raise_builtin_error("TypeError", "must be str, not other"));
    };
    let (start, end) = text_range_bounds(instance, args, text.len(), "rfind")?;
    if start > end {
        return Ok(instance.new_int(-1));
    }
    let window = &text[start..end];
    let found = find_subsequence(window, needle, true)
        .map(|position| wtf8_char_index(&text, start + position))
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
        check_max_arguments(instance, args, 3, "index")?;
    let text = bound_text_cow(instance, bound)?;
    let needle = text_argument(instance, args, 0, "index")?;
    match text_search(instance, &text, &needle, args, "index", false)? {
        Some(found) => Ok(instance.new_int(found)),
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
        check_max_arguments(instance, args, 3, "rindex")?;
    // **WTF-8 字节层搜索＋`start`／`end` 区间**（round 107；与 `rfind` 同一口径 ✓）。
    let text = bound_wtf8_bytes(instance, bound)?;
    let Some(needle_value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "rindex() takes at least 1 argument (0 given)",
        ));
    };
    let Some(needle) = instance.wtf8_of(needle_value) else {
        return Err(instance.raise_builtin_error("TypeError", "must be str, not other"));
    };
    let (start, end) = text_range_bounds(instance, args, text.len(), "rindex")?;
    if start > end {
        return Err(instance.raise_builtin_error("ValueError", "substring not found"));
    }
    match find_subsequence(&text[start..end], needle, true) {
        Some(position) => Ok(instance.new_int(wtf8_char_index(&text, start + position))),
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
    // 空分隔符 ⇒ `ValueError: empty separator` ✓（与 `partition` 同口径）。
    if separator.is_empty() {
        return Err(instance.raise_builtin_error("ValueError", "empty separator"));
    }
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
        check_max_arguments(instance, args, 3, "count")?;
    let text = bound_text_cow(instance, bound)?;
    let needle = text_argument(instance, args, 0, "count")?;
    // **`start`／`end` 生效**（`P3-41` round 40 修 ✗）：与 `startswith` 同一条窗口口径
    // （`start > len` ⇒ 0 ✓，`"abc".count("", 5)` ⇒ 0 ✓、`"abc".count("", 3)` ⇒ 1 ✓）。
    let Some(window) = prefix_suffix_window(instance, &text, args, "count")? else {
        return Ok(instance.new_int(0));
    };
    let count = if needle.is_empty() {
        text_char_len(window) + 1
    } else {
        window.matches(needle.as_str()).count() as i64
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
        match ch {
            '\t' => {
                if tabsize == 0 {
                    continue;
                }
                let pad = tabsize - (column % tabsize);
                for _ in 0..pad {
                    out.push(' ');
                }
                column += pad;
            }
            // **`\n`／`\r` 都归零列** ✓（参照口径；先前按普通字符 +1 ✗ ⇒ 换行后的 tab 位置全错 ✗）。
            '\n' | '\r' => {
                out.push(ch);
                column = 0;
            }
            _ => {
                out.push(ch);
                column += 1;
            }
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

/// **`str.isspace()`**（task-24 尾件 ✓）：**非空**且每个字符都是空白 ⇒ `True` ✓
/// （与 `bytes.isspace` 同口径 ✓；`importlib.metadata` 的 `_adapters`／`_text` 一族撞在缺它上 ✓）。
///
/// **如实登记的偏差** ✗：Rust 的 `char::is_whitespace` 与 Python 的 `str.isspace` 都按 Unicode
/// `White_Space` ✓ —— 落地 Unicode 表时一并复核 ✓。
pub(crate) fn str_isspace_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let found = !text.is_empty() && text.chars().all(|c| c.is_whitespace());
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
    // **符号在零之前** ✓（`'+123'.zfill(5)` ⇒ `'+0123'`；先前给 `'0+123'` ✗）。
    let zeros: String = core::iter::repeat('0').take((width - current) as usize).collect();
    let padded = match text.chars().next() {
        Some(sign @ ('+' | '-')) => {
            let rest: String = text.chars().skip(1).collect();
            format!("{sign}{zeros}{rest}")
        }
        _ => format!("{zeros}{text}"),
    };
    Ok(instance.new_str(&padded))
}

pub(crate) fn str_splitlines_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    // **`keepends`**（task-19 路上钉死的真 bug ✗）：先前整条实参都**没读** ✗、还用 Rust 的
    // `lines()`（把行尾**丢掉** ✓）⇒ `'a\nb\n'.splitlines(True)` 给 `['a','b']` ✗（参照 `['a\n','b\n']` ✓）。
    // 参照口径 ✓：行界＝`\n \r \r\n \v \f \x1c \x1d \x1e \u{85} \u{2028} \u{2029}` ✓；
    // `\r\n` 算**一个**界 ✓；`keepends` 为真时**连界一起**切进去 ✓；末尾有界也**不**多出空项 ✓。
    let keepends = match args.first() {
        Some(value) => instance.truthiness_of(*value)?,
        None => match kwargs
            .iter()
            .find(|(key, _)| instance.text_of(*key) == Some("keepends"))
        {
            Some((_, value)) => instance.truthiness_of(*value)?,
            None => false,
        },
    };
    let characters: Vec<char> = text.chars().collect();
    let is_boundary = |character: char| {
        matches!(
            character,
            '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}'
                | '\u{2028}' | '\u{2029}'
        )
    };
    let mut parts: Vec<NonNull<Header>> = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    while index < characters.len() {
        let character = characters[index];
        if is_boundary(character) {
            let next = if character == '\r' && characters.get(index + 1) == Some(&'\n') {
                index + 2
            } else {
                index + 1
            };
            let stop = if keepends { next } else { index };
            let piece: String = characters[start..stop].iter().collect();
            parts.push(instance.new_str(&piece));
            start = next;
            index = next;
        } else {
            index += 1;
        }
    }
    if start < characters.len() {
        let piece: String = characters[start..].iter().collect();
        parts.push(instance.new_str(&piece));
    }
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
    // **带参形态**（第 527 轮接线 ✗）：`chars` 是**字符集合**（不是子串 ✓）——
    // 先前一律报 `NotImplementedError` ✗ ⇒ `Lib/site.py` 等在这一面上卡住 ✓。
    // 参照的报错口径：`strip arg must be None or str` ✓。
    let trimmed = if let Some(first) = args.first().filter(|value| **value != instance.singletons().none()) {
        // **`None` ＝ 缺省**（本机 3.14.4：`'  a  '.strip(None)` ⇒ `'a'` ✓；先前落到
        // "strip arg must be None or str" ✗ —— `UserString.strip(None)` 直接踩它 ✓）。
        let Some(chars) = instance.text_of(*first) else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "strip arg must be None or str",
            ));
        };
        if chars.is_empty() {
            text.clone()
        } else if from_left {
            text.trim_start_matches(|character| chars.contains(character))
                .to_owned()
        } else {
            text.trim_end_matches(|character| chars.contains(character))
                .to_owned()
        }
    } else if from_left {
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
                push_titlecase(&mut out, character);
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
/// **一个字符的 title 映射**（参照 `str.capitalize()`／`str.title()` 的词首口径）：
/// 先查生成表 `unicode_case::TITLE_CASE_OVERRIDES` ✓（title ≠ upper 的 135 个码点），否则回退
/// Rust 的 `to_uppercase`（与参照的 upper 映射一致 ✓）。
fn push_titlecase(out: &mut String, character: char) {
    let code_point = character as u32;
    if let Ok(index) = crate::unicode_case::TITLE_CASE_OVERRIDES
        .binary_search_by_key(&code_point, |(point, _)| *point)
    {
        for point in crate::unicode_case::TITLE_CASE_OVERRIDES[index].1 {
            if let Some(expanded) = char::from_u32(*point) {
                out.push(expanded);
            }
        }
        return;
    }
    out.extend(character.to_uppercase());
}

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
            // **title 映射**（不是 upper ✓）：`'ῼ'.capitalize()` ⇒ `'ῼ'` ✓（参照实测）。
            push_titlecase(&mut out, first);
            out.extend(characters.flat_map(|character| character.to_lowercase()));
            out
        }
        None => String::new(),
    };
    Ok(instance.new_str(&capitalized))
}

/// `rjust`／`ljust`／`center`：按宽度补**填充字符** ✓（第二实参；缺省空格 ✓）。
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
    // **填充字符**（`P3-42` round 44）：参照口径是"恰好一个字符的 str"；否则
    // `TypeError: The fill character must be exactly one character long` ✓。先前整条实参
    // 被**忽略** ✗ ⇒ `'abc'.center(10, '*')` 给空格 ✗。
    let fill: char = match args.get(1) {
        None => ' ',
        Some(value) => match instance.text_of(*value) {
            Some(fill) if fill.chars().count() == 1 => fill.chars().next().expect("刚数过 1 个"),
            _ => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "The fill character must be exactly one character long",
                ))
            }
        },
    };
    let current = text.chars().count() as i64;
    if width <= current {
        return Ok(instance.new_str(&text));
    }
    let pad = (width - current) as usize;
    let fill_text: String = core::iter::repeat(fill).take(pad).collect();
    let padded = match mode {
        0 => format!("{}{}", fill_text, text), // rjust
        1 => format!("{}{}", text, fill_text), // ljust
        _ => {
            // center 的**准确规则**照参照 ✓：`left = pad//2 + (pad & width & 1)`（CPython 的
            //   `str.center` 就是这么写的 ✓）—— 实测三个样例都对 ✓：`"a".center(2)` ⇒ `'a '` ✓、
            //   `"ab".center(5)` ⇒ `'  ab '` ✓、`"a".center(5)` ⇒ `'  a  '` ✓。
            //   （我先前先写成"多的一格在左"✗、又改成 ceil ✗，两次都不对 ⇒ 现在照公式 ✓。）
            let left = (pad / 2 + (pad & (width as usize) & 1)) as usize;
            let left_text: String = core::iter::repeat(fill).take(left).collect();
            let right_text: String = core::iter::repeat(fill).take(pad - left).collect();
            format!("{left_text}{text}{right_text}")
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
    let (separator, maxsplit) = split_arguments(instance, args, "rsplit")?;
    let parts: Vec<String> = match separator {
        Some(separator) => {
            if separator.is_empty() {
                return Err(instance.raise_builtin_error("ValueError", "empty separator"));
            }
            if maxsplit < 0 {
                let mut collected: Vec<String> =
                    text.rsplit(separator.as_str()).map(str::to_owned).collect();
                collected.reverse();
                collected
            } else {
                // `rsplitn` 从右往左产出（**逆序**）⇒ 收完再反转成参照顺序 ✓。
                let mut collected: Vec<String> = text
                    .rsplitn(maxsplit as usize + 1, separator.as_str())
                    .map(str::to_owned)
                    .collect();
                collected.reverse();
                collected
            }
        }
        None => rsplit_on_whitespace(&text, maxsplit),
    };
    let parts: Vec<NonNull<Header>> = parts
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
            crate::executor::subscript::subscript_write(instance, result, mapped, value)?;
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
        crate::executor::subscript::subscript_write(instance, result, key, value)?;
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
            crate::executor::subscript::subscript_write(instance, result, key, none)?;
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
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    // **带参形态**（第 527 轮接线 ✗）：`chars` 是**字符集合** ✓ —— 先前把实参**丢掉** ✗
    // ⇒ `"xxabcxx".strip("x")` 给的是 `"xxabcxx"` ✗（参照 `abc` ✓）。两侧都按集合剪 ✓。
    let trimmed = if let Some(first) = args.first().filter(|value| **value != instance.singletons().none()) {
        // **`None` ＝ 缺省** ✓（同上）。
        let Some(chars) = instance.text_of(*first) else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "strip arg must be None or str",
            ));
        };
        if chars.is_empty() {
            text.clone()
        } else {
            text.trim_matches(|character| chars.contains(character)).to_owned()
        }
    } else {
        text.trim().to_owned()
    };
    Ok(instance.new_str(&trimmed))
}

pub(crate) fn str_startswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
        check_max_arguments(instance, args, 3, "startswith")?;
    let text = bound_text_cow(instance, bound)?;
    // **也认元组** ✓（第 195 轮：`Lib/importlib/_bootstrap_external.py:61` 就是
    // `sys.platform.startswith(('win32', 'cygwin', 'darwin'))` ✓ —— 先前只认单个 `str` ✗）。
    let prefixes = text_prefixes(instance, args, "startswith")?;
    let found = match prefix_suffix_window(instance, &text, args, "startswith")? {
        Some(window) => prefixes.iter().any(|prefix| window.starts_with(prefix)),
        None => false,
    };
    Ok(instance.retain(instance.singletons().boolean(found)))
}

pub(crate) fn str_endswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
        check_max_arguments(instance, args, 3, "endswith")?;
    let text = bound_text_cow(instance, bound)?;
    // **也认元组** ✓（同 `startswith` ✓）。
    let suffixes = text_prefixes(instance, args, "endswith")?;
    let found = match prefix_suffix_window(instance, &text, args, "endswith")? {
        Some(window) => suffixes.iter().any(|suffix| window.ends_with(suffix)),
        None => false,
    };
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
    // **走迭代器协议**（`P3-42` round 44）：先前用 `iterable_items`（只认内建容器 ✓ 且是**借用快照** ✗）
    // ⇒ ① 生成器里的异常被"不是可迭代"顶掉（官方用例要求 `+` 原样透出 ✓）；
    // ② 元素不是 str 时报文没有下标与类型（参照 `sequence item 0: expected str instance, int found` ✓）。
    let iterator = match instance.iter_object(*iterable) {
        Ok(iterator) => iterator,
        Err(error) => {
            crate::executor::runtime::discard_error(instance, &error);
            return Err(instance.raise_builtin_error("TypeError", "can only join an iterable"));
        }
    };
    let mut result = String::new();
    let mut index = 0usize;
    let mut first = true;
    loop {
        let next = instance.advance_iterator(iterator);
        let item = match next {
            Ok(Some(item)) => item,
            Ok(None) => break,
            Err(error) => {
                // SAFETY: iterator 的这份引用由本函数持有。
                unsafe { instance.release_object(iterator.as_ptr()) };
                return Err(error);
            }
        };
        let Some(text) = instance.text_of(item) else {
            let kind = instance.type_name(instance.type_of(item));
            // SAFETY: item 是新引用。
            unsafe { instance.release_object(item.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(iterator.as_ptr()) };
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("sequence item {index}: expected str instance, {kind} found"),
            ));
        };
        if !first {
            result.push_str(&separator);
        }
        result.push_str(text);
        first = false;
        index += 1;
        // SAFETY: item 是新引用。
        unsafe { instance.release_object(item.as_ptr()) };
    }
    // SAFETY: iterator 的这份引用由本函数持有。
    unsafe { instance.release_object(iterator.as_ptr()) };
    Ok(instance.new_str(&result))
}

/// `split`／`rsplit` 的实参解码：`(separator: Option<String>, maxsplit: i64)`。
///
/// 口径照本机 3.14.4 实测：`separator` 为 `None` ＝ 按空白切 ✓；`maxsplit` 缺省 `-1`＝不限 ✓；
/// 非 str 分隔符 ⇒ `must be str or None, not <类型>` ✓；`maxsplit` 非整数 ⇒
/// `'<类型>' object cannot be interpreted as an integer` ✓。
fn split_arguments(
    instance: &Instance,
    args: &[NonNull<Header>],
    _method: &str,
) -> Result<(Option<String>, i64), crate::ExecError> {
    let separator = match args.first() {
        None => None,
        Some(value) if *value == instance.singletons().none() => None,
        Some(value) => match instance.text_of(*value) {
            Some(text) => Some(text.to_owned()),
            None => {
                let kind = instance.type_name(instance.type_of(*value));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("must be str or None, not {kind}"),
                ));
            }
        },
    };
    let maxsplit = match args.get(1) {
        None => -1,
        Some(value) => instance
            .index_value(*value)
            .ok()
            .flatten()
            .ok_or_else(|| {
                let kind = instance.type_name(instance.type_of(*value));
                instance.raise_builtin_error(
                    "TypeError",
                    &format!("'{kind}' object cannot be interpreted as an integer"),
                )
            })?,
    };
    Ok((separator, maxsplit))
}

/// **空白切分 ＋ `maxsplit`**（`str.split(None, n)` 的参照口径：剩余段**原样保留**，
/// 只去掉段首空白 ✓；`"  ".split(None, 0)` ⇒ `[]` ✓）。
/// **从右往左的空白切分 ＋ `maxsplit`**（`str.rsplit(None, n)` 的镜像口径 ✓）。
fn rsplit_on_whitespace(text: &str, maxsplit: i64) -> Vec<String> {
    if maxsplit < 0 {
        return text.split_whitespace().map(str::to_owned).collect();
    }
    let mut reversed: Vec<String> = Vec::new();
    let mut rest = text.trim_end();
    let mut remaining = maxsplit as usize;
    while remaining > 0 {
        if rest.is_empty() {
            break;
        }
        let start = rest
            .rfind(char::is_whitespace)
            .map(|index| index + rest[index..].chars().next().map(char::len_utf8).unwrap_or(0))
            .unwrap_or(0);
        reversed.push(rest[start..].to_owned());
        remaining -= 1;
        rest = rest[..start].trim_end();
    }
    if !rest.is_empty() {
        reversed.push(rest.to_owned());
    }
    reversed.reverse();
    reversed
}

pub(crate) fn str_split_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    check_max_arguments(instance, args, 2, "split")?;
    let text = bound_wtf8_bytes(instance, bound)?;
    let separator_value = args.first().copied();
    let separator_is_none = separator_value.is_none()
        || separator_value.is_some_and(|value| value == instance.singletons().none());
    let maxsplit = match args.get(1).copied() {
        None => -1,
        Some(value) => {
            // 参照：`maxsplit=None` 也 **TypeError**（不是"用缺省" ✓）；非整数同文案 ✓。
            match instance.index_value(value)? {
                Some(number) => number,
                None => {
                    let name = instance.type_name(instance.type_of(value));
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        &format!("'{name}' object cannot be interpreted as an integer"),
                    ));
                }
            }
        }
    };
    let parts: Vec<Vec<u8>> = if separator_is_none {
        split_whitespace_wtf8(&text, maxsplit)
    } else {
        let separator_value = separator_value.expect("刚判过不是 None");
        let Some(separator) = instance.wtf8_of(separator_value).map(<[u8]>::to_vec) else {
            let name = instance.type_name(instance.type_of(separator_value));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("must be str or None, not {name}"),
            ));
        };
        if separator.is_empty() {
            return Err(instance.raise_builtin_error("ValueError", "empty separator"));
        }
        split_wtf8(&text, &separator, maxsplit)
    };
    let parts: Vec<NonNull<Header>> = parts
        .iter()
        .map(|part| {
            instance
                .new_str_wtf8(part)
                .expect("切分后的 WTF-8 字节仍合法")
        })
        .collect();
    Ok(instance.new_list(parts))
}

pub(crate) fn str_replace_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let from = text_argument(instance, args, 0, "replace")?;
    let to = text_argument(instance, args, 1, "replace")?;
    // **`count` 真生效** ✓（`P3-42` round 44）：先前整条忽略 ✗ ⇒ `'abab'.replace('a','x',1)`
    // 给 `'xbxb'` ✗（参照 `'xbab'` ✓）。负数/缺省 ⇒ 不限 ✓。
    // `count` 也收**关键字**（`'aa'.replace('a', 'b', count=0)` ✓；官方 `test_count_keyword` ✓）。
    let count_argument = match args.get(2) {
        Some(value) => Some(*value),
        None => kwargs
            .iter()
            .find(|(key, _)| instance.text_of(*key) == Some("count"))
            .map(|(_, value)| *value),
    };
    let count = match count_argument {
        None => -1,
        Some(value) => bound_index(instance, value, "replace")?,
    };
    // `count == 0` ⇒ 原样返回（**不要**走"空 `from` 的空隙替换"那条 ✗）。
    if count == 0 {
        return Ok(instance.new_str(&text));
    }
    let replaced = if count < 0 {
        text.replace(&from, &to)
    } else if from.is_empty() {
        // 空 `from`：按参照在**每个空隙**替换，最多 `count` 次 ✓。
        let mut out = String::with_capacity(text.len());
        let mut done = 0i64;
        out.push_str(&to);
        done += 1;
        for character in text.chars() {
            out.push(character);
            if done < count {
                out.push_str(&to);
                done += 1;
            }
        }
        out
    } else {
        let mut out = String::with_capacity(text.len());
        let mut rest = text.as_str();
        let mut done = 0i64;
        while done < count {
            match rest.find(&from) {
                Some(at) => {
                    out.push_str(&rest[..at]);
                    out.push_str(&to);
                    rest = &rest[at + from.len()..];
                    done += 1;
                }
                None => break,
            }
        }
        out.push_str(rest);
        out
    };
    Ok(instance.new_str(&replaced))
}

/// `str.encode(encoding='utf-8', errors='strict')` —— **薄包装**：解析参数后交给
/// [`crate::Instance::encode_with_encoding_and_errors`]。
///
/// **一处委派**（task-9 ✅）：编码名白名单与实现都搬到了 `Instance`（`instance/convert.rs`）——
/// 内置 `utf-8`／`ascii`／`latin-1`／`cp437` 走快路径，**其余委派 `_codecs.lookup` 注册表**
/// （⇒ `'あ'.encode('cp932')` 与 `Lib/encodings/*` 全族可用 ✓）。先前这里另有一份 4 编码白名单 ✗
/// （与 `bytes.decode` 那份重复 ⇒ 两处都得改才通）。
/// 取绑定 `str` 的 **WTF-8 字节**（P3-38 S3a）：`str.encode` 的输入可含孤立代理 ⇒
/// `bound_text`（`.value()`）对 WTF-8 表示会 panic ✗，这里走字节视图 ✓。
pub(crate) fn str_encode_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_wtf8_bytes(instance, bound)?;
    let mut encoding = args.first().and_then(|value| instance.text_of(*value));
    let mut errors = args.get(1).and_then(|value| instance.text_of(*value));
    for (key, value) in kwargs {
        let Some(kwarg_name) = instance.text_of(*key) else {
            continue;
        };
        match kwarg_name {
            "encoding" => encoding = instance.text_of(*value),
            "errors" => errors = instance.text_of(*value),
            _ => {}
        }
    }
    let encoding = encoding.unwrap_or("utf-8");
    let errors = errors.unwrap_or("strict");
    instance.encode_wtf8_with_encoding_and_errors(&text, encoding, errors)
}

/// `str.format(*args, **kwargs)` ✓（第 722 轮 ✓）：`Lib/base64.py:246` 的
/// `_B32_ENCODE_DOCSTRING.format(encoding='base32')` 在**导入期**就调它 ✗ ⇒
/// 先前 `'str' object has no attribute 'format'` 把 `base64` ⇒ email 一族整片压在下面 ✓。
///
/// 已接：`{}`／`{N}`／`{名字}`／`.属性`／`[下标]`（纯数字键折成 `int` ✓）、`!s`／`!r`／`!a`、
/// `:` 规格段（走 [`format_value_with_spec`] ⇒ 与 `FORMAT_WITH_SPEC` **同一处** ✓）、
/// 规格段里的**嵌套一层** `{…}` ✓、`{{`／`}}` 转义 ✓。
/// 未接的形态**如实报错** ✗（不静默给假值 ✓）。
/// **`str.__mod__`**（`P3-42` round 44）：`'abc'.__mod__(x)` ≡ `'abc' % x` ✓。
///
/// 为什么必须显式登记：`string_tests.test_formatting` 里大量 `self.checkraises(TypeError, 'abc',
/// '__mod__', …)` 是**按名字取方法再调** ✓ —— 先前 `str` 类型上没有 `__mod__` ⇒ 取属性先报
/// `AttributeError` ✗ ⇒ 那些"该报 TypeError"的断言全红 ✓（`%` 运算本身走 `arithmetic_public`，
/// 从不经过属性查表 ⇒ 之前一直没暴露 ✗）。
pub(crate) fn str_mod_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let template = bound_text(instance, bound)?;
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("expected 1 argument, got {}", args.len()),
        ));
    }
    crate::executor::percent_format(instance, &template, args[0], 0)
}

pub(crate) fn str_format_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let template = bound_text(instance, bound)?;
    // P3-38 S2a-2 批 2 最小直通：模板就是**单个自动／手工编号字段**（`{}`／`{0}`）且实参恰为
    // **WTF-8 表示的 `str`**（含孤立代理）⇒ 参照 `"{}".format(s) is s`（实测 ✓）⇒ 直接回原对象，
    // 避免 `render_format` 的 `String` 装不下代理。**带规格／前后缀／多字段的完整语义留下批** ✓。
    if kwargs.is_empty() && args.len() == 1 && (template == "{}" || template == "{0}") {
        if instance.is_str_object(args[0]) && instance.text_of(args[0]).is_none() {
            return Ok(instance.retain(args[0]));
        }
    }
    // **单字段模板的直通**（P3-38 S2c）：`"{:>5}".format(s)` 的参照结果仍是**含代理的 str**；
    // 而 `render_format` 的输出通道是 `String`（装不下代理）⇒ 这里对"整条模板就是一个简单字段"
    // 的形态直接走 `format_value_with_spec`，返回 **str 对象** ✓（合法 UTF-8 结果逐字节同旧路径 ✓）。
    // 只认：`{字段[:规格]}`（无 `!` 转换、字段名无 `.`/`[`、规格里无花括号）——其余走原路径 ✓。
    if template.starts_with('{') && template.ends_with('}') && !template[1..template.len() - 1].contains('{') {
        let inner = &template[1..template.len() - 1];
        // `!` 转换／嵌套花括号 ⇒ 交回原路径
        if !inner.contains('!') && !inner.contains('{') && !inner.contains('}') {
            let (name, spec) = match inner.split_once(':') {
                Some((name, spec)) => (name, spec),
                None => (inner, ""),
            };
            if !name.contains('.') && !name.contains('[') {
                let value = if name.is_empty() || name.chars().all(|c| c.is_ascii_digit()) {
                    let index = if name.is_empty() { 0usize } else { name.parse::<usize>().unwrap_or(usize::MAX) };
                    args.get(index).copied()
                } else {
                    // 关键字字段：名字是合法 UTF-8（代理名不可能等于 `name` ✓）
                    kwargs
                        .iter()
                        .find(|(key, _)| instance.text_of(*key) == Some(name))
                        .map(|(_, value)| *value)
                };
                if let Some(value) = value {
                    // **只给含代理的 `str`（WTF-8 表示）走这条**：合法 UTF-8 与其它类型仍走原
                    // render 路径 ⇒ 旧行为逐字节不变 ✓（WTF-8 的 `String` 通道装不下代理，才需直通）。
                    if instance.is_str_object(value) && instance.text_of(value).is_none() {
                        return format_value_with_spec(instance, value, spec, 0);
                    }
                }
            }
        }
    }
    let mut named: Vec<(String, NonNull<Header>)> = Vec::with_capacity(kwargs.len());
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            let kind = instance.type_name(instance.type_of(*key));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("keywords must be strings (got {kind})"),
            ));
        };
        named.push((name.to_owned(), *value));
    }
    let mut state = FormatState::default();
    let text = render_format(instance, &template, args, &named, &mut state, 0)?;
    Ok(instance.new_str(&text))
}

/// `str.format` 的**自动／手动编号**状态 ✓（`{}` 与 `{0}` 不许混用 ✓ —— 与参照同规 ✓）。
#[derive(Default)]
struct FormatState {
    auto: usize,
    auto_seen: bool,
    manual_seen: bool,
}

/// 渲染一段模板 ✓（`{{`／`}}` 转义 ✓；`{字段}` 走 [`render_field`] ✓）。
fn render_format(
    instance: &Instance,
    template: &str,
    args: &[NonNull<Header>],
    named: &[(String, NonNull<Header>)],
    state: &mut FormatState,
    depth: u32,
) -> Result<String, crate::ExecError> {
    // 参照只允许**一层**嵌套规格 ✓ ⇒ 超了照参照那句报 ✓。
    if depth > 1 {
        return Err(instance.raise_builtin_error("ValueError", "Max string recursion exceeded"));
    }
    let characters: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut index = 0usize;
    while index < characters.len() {
        match characters[index] {
            '{' if characters.get(index + 1) == Some(&'{') => {
                out.push('{');
                index += 2;
            }
            '}' if characters.get(index + 1) == Some(&'}') => {
                out.push('}');
                index += 2;
            }
            '{' => {
                // 找配对的 `}` ✓（规格段里的 `{}` 计入层数 ✓）。
                let start = index + 1;
                let mut level = 1i32;
                let mut end = start;
                while end < characters.len() {
                    match characters[end] {
                        '{' => level += 1,
                        '}' => {
                            level -= 1;
                            if level == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    end += 1;
                }
                if level != 0 {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "Single '{' encountered in format string",
                    ));
                }
                let field: String = characters[start..end].iter().collect();
                out.push_str(&render_field(instance, &field, args, named, state, depth)?);
                index = end + 1;
            }
            '}' => {
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    "Single '}' encountered in format string",
                ));
            }
            character => {
                out.push(character);
                index += 1;
            }
        }
    }
    Ok(out)
}

/// 渲染**一个字段** ✓：`字段名[!转换][:规格]` ⇒ 已格式化的文本 ✓。
fn render_field(
    instance: &Instance,
    field: &str,
    args: &[NonNull<Header>],
    named: &[(String, NonNull<Header>)],
    state: &mut FormatState,
    depth: u32,
) -> Result<String, crate::ExecError> {
    // 拆 `字段名[!转换][:规格]` ✓：`[` 里的 `!`／`:` **不算分隔符** ✓（下标字符串可以带它们 ✓）。
    let characters: Vec<char> = field.chars().collect();
    let mut level = 0i32;
    let mut name_end = characters.len();
    let mut conversion: Option<char> = None;
    let mut spec_start: Option<usize> = None;
    let mut index = 0usize;
    while index < characters.len() {
        match characters[index] {
            '[' => level += 1,
            ']' => level -= 1,
            '!' if level == 0 => {
                name_end = index;
                conversion = characters.get(index + 1).copied();
                if conversion.is_none() {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "end of format while looking for conversion specifier",
                    ));
                }
                if characters.get(index + 2) == Some(&':') {
                    spec_start = Some(index + 3);
                }
                break;
            }
            ':' if level == 0 => {
                name_end = index;
                spec_start = Some(index + 1);
                break;
            }
            _ => {}
        }
        index += 1;
    }
    let name: String = characters[..name_end].iter().collect();
    let spec: String = match spec_start {
        Some(start) if start < characters.len() => characters[start..].iter().collect(),
        _ => String::new(),
    };
    // 字段名 → 值（**新引用** ✓，后面每一步都把它换掉 ✓）。
    let mut value = resolve_field_base(instance, &name, args, named, state)?;
    let identifier_length: usize = name
        .chars()
        .take_while(|character| *character != '.' && *character != '[')
        .map(|character| character.len_utf8())
        .sum();
    let mut rest = &name[identifier_length..];
    while !rest.is_empty() {
        if let Some(after_dot) = rest.strip_prefix('.') {
            let attribute_length: usize = after_dot
                .chars()
                .take_while(|character| *character != '.' && *character != '[')
                .map(|character| character.len_utf8())
                .sum();
            if attribute_length == 0 {
                unsafe { instance.release_object(value.as_ptr()) };
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    "Empty attribute in format string",
                ));
            }
            let attribute = &after_dot[..attribute_length];
            let next = format_getattr(instance, value, attribute);
            unsafe { instance.release_object(value.as_ptr()) };
            value = next?;
            rest = &after_dot[attribute_length..];
        } else if let Some(after_open) = rest.strip_prefix('[') {
            let Some(close) = after_open.find(']') else {
                unsafe { instance.release_object(value.as_ptr()) };
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    "Missing ']' in format string",
                ));
            };
            let key_text = &after_open[..close];
            let key = if !key_text.is_empty()
                && key_text.chars().all(|character| character.is_ascii_digit())
            {
                let Ok(number) = key_text.parse::<i64>() else {
                    unsafe { instance.release_object(value.as_ptr()) };
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "Too many decimal digits in format string",
                    ));
                };
                instance.new_int(number)
            } else {
                instance.new_str(key_text)
            };
            let next = subscript_read(instance, value, key);
            unsafe { instance.release_object(key.as_ptr()) };
            unsafe { instance.release_object(value.as_ptr()) };
            value = next?;
            rest = &after_open[close + 1..];
        } else {
            unsafe { instance.release_object(value.as_ptr()) };
            return Err(instance.raise_builtin_error(
                "ValueError",
                "Only '.' or '[' may follow ']' in format field specifier",
            ));
        }
    }
    // **转换**（`!s`／`!r`／`!a`）✓：转换后值就是那个 `str` ✓、规格段作用在**它**上面 ✓；
    // 没有转换时规格直接作用在**原值**上 ✓（`'{:>5}'.format(42)` 走整数格式化 ✓）。
    if let Some(kind) = conversion {
        let text = match kind {
            's' => instance.object_str(value)?,
            'r' => instance.object_repr(value)?,
            'a' => crate::executor::escape_non_ascii(&instance.object_repr(value)?),
            other => {
                unsafe { instance.release_object(value.as_ptr()) };
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    &format!("Unknown conversion specifier {other}"),
                ));
            }
        };
        unsafe { instance.release_object(value.as_ptr()) };
        value = instance.new_str(&text);
    }
    // 规格段里的嵌套字段：**同一个**编号状态 ✓（参照同序 ✓）。
    let spec_text = if spec.contains('{') {
        render_format(instance, &spec, args, named, state, depth + 1)?
    } else {
        spec
    };
    let formatted = format_value_with_spec(instance, value, &spec_text, 0);
    unsafe { instance.release_object(value.as_ptr()) };
    let formatted = formatted?;
    // P3-38 S2c：`str.format` 的输出通道仍是 `String` ⇒ 含代理字段走**安全口径**（有损 U+FFFD；
    // `format_value_with_spec` 已在码点层把宽度/填充做好 ✓）。合法 UTF-8 路径逐字节不变 ✓。
    let text = instance.text_value(formatted);
    unsafe { instance.release_object(formatted.as_ptr()) };
    match text {
        Some(text) => Ok(text),
        None => Err(instance.raise_builtin_error("TypeError", "__format__ must return a str")),
    }
}

/// 取**字段名**对应的值 ✓（`{}`／`{N}` 走位置实参 ✓，名字走关键字实参 ✓ ⇒ **新引用** ✓）。
fn resolve_field_base(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    named: &[(String, NonNull<Header>)],
    state: &mut FormatState,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **标识符**＝名字里第一段（到 `.`／`[` 为止 ✓）：`0.real` 的那个 `0` 是**位置下标** ✓
    //（`name.chars().all(数字)` 判整串会把 `0.real` 误判成关键字名 ✗ —— 实测踩到 ✓）。
    let identifier: String = name
        .chars()
        .take_while(|character| *character != '.' && *character != '[')
        .collect();
    if identifier.is_empty() {
        if state.manual_seen {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "cannot switch from manual field specification to automatic field numbering",
            ));
        }
        state.auto_seen = true;
        let position = state.auto;
        state.auto += 1;
        match args.get(position) {
            Some(value) => Ok(instance.retain(*value)),
            None => Err(instance.raise_builtin_error(
                "IndexError",
                &format!("Replacement index {position} out of range for positional args tuple"),
            )),
        }
    } else if identifier.chars().all(|character| character.is_ascii_digit()) {
        if state.auto_seen {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "cannot switch from automatic field numbering to manual field specification",
            ));
        }
        state.manual_seen = true;
        let position: usize = match identifier.parse() {
            Ok(position) => position,
            Err(_) => {
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    "Too many decimal digits in format string",
                ))
            }
        };
        match args.get(position) {
            Some(value) => Ok(instance.retain(*value)),
            None => Err(instance.raise_builtin_error(
                "IndexError",
                &format!("Replacement index {position} out of range for positional args tuple"),
            )),
        }
    } else {
        match named.iter().find(|(key, _)| key == &identifier) {
            Some((_, value)) => Ok(instance.retain(*value)),
            None => Err(instance.raise_builtin_error("KeyError", &format!("'{identifier}'"))),
        }
    }
}

/// `str.format` 字段名里的 `.属性` ✓：与 `getattr(obj, 名)` **同一口径** ✓
///（含 `__getattr__` 回退 ✓）；取到方法就照 `LOAD_ATTR` 的非取方法位造**绑定方法** ✓。
fn format_getattr(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<NonNull<Header>, crate::ExecError> {
    match attribute_lookup_with_getattr(instance, object, name)? {
        Attribute::Owned(value) => Ok(value),
        Attribute::Value(value) => Ok(instance.retain(value)),
        Attribute::Method { function, this } => {
            // SAFETY: 函数与 `this` 都还活着（由类型字典与调用方持有）⇒ 各自 incref 给绑定方法 ✓。
            unsafe {
                instance.incref_object(function.as_ptr());
                instance.incref_object(this.as_ptr());
            }
            let bound = instance.alloc(MethodObject::new(
                instance.hot_type(HotId::Method).expect("method 已登记"),
                function,
                this,
            ));
            Ok(bound.into_raw().cast::<Header>())
        }
    }
}
