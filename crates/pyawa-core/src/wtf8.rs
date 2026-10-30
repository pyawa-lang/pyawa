//! **WTF-8** 编解码（P3-38 S1）：UTF-8 的超集，**允许孤立代理**（`U+D800..=U+DFFF`）按
//! 3 字节形式 `ED A0..BF 80..BF` 编码。
//!
//! # 为什么需要它
//!
//! 参照 CPython 的 `str` 是「码点序列」，**合法持有孤立代理**（`'\ud800'` 是合法字符；
//! `surrogateescape` 把不可解码字节映射成 `U+DC80..=U+DCFF`）。本层此前用 Rust `String`
//! （只存合法 UTF-8）⇒ 这类字符串**存不进去**。S2 会把 `str` 的存储迁到「WTF-8 字节串」；
//! 本模块是那一层要复用的**自洽基础设施**（本轮 S1 **只加模块，不改任何既有存储/调用点**）。
//!
//! WTF-8 的规范性质（与本模块的判定一一对应）：
//! - 合法 UTF-8 的字节串**都是**合法 WTF-8（本模块永不拒它 ✓）；
//! - 唯一多出来的形态是**孤立代理的 3 字节编码**（代理对仍照 UTF-8 拼成 4 字节标量）；
//! - 其余违规与 UTF-8 相同：截断／过长编码／非法首字节／非法续字节／超出 `U+10FFFF`。
//!
//! # 与"改写为 `str`"的关系（如实说明）
//!
//! Rust 的 `String`/`char` 装不下孤立代理 ⇒ [`to_string_lossy`] 只能把代理序列**替换**成
//! `U+FFFD`（诊断／显示用途，**不能**用于语义转换）；真正的语义转换由 S2 的 `StrObject`
//! 直接吃 WTF-8 字节完成 ✓。
//!
//! # 错误口径
//!
//! [`Wtf8Error`] 带**字节偏移**：结构性错误（截断／非法首字节／过长／超界）取**序列起始**
//! 位置；续字节错误取**那个坏续字节**的位置。编码侧的错误（码点超界）偏移恒为 `0`（没有
//! 字节流可言）——调用方按 `kind` 判即可 ✓。

/// Unicode 的**最大码点**（`U+10FFFF`）；WTF-8 与 UTF-8 同界。
pub const MAX_CODE_POINT: u32 = 0x10_FFFF;

/// 代理区起点（`U+D800`）。
pub const SURROGATE_START: u32 = 0xD800;

/// 代理区终点（`U+DFFF`）。
pub const SURROGATE_END: u32 = 0xDFFF;

/// WTF-8 里的**替换字符**（`U+FFFD`；[`to_string_lossy`] 的占位）。
pub const REPLACEMENT_CHARACTER: char = '\u{FFFD}';

/// 是不是**代理码点**（`U+D800..=U+DFFF`）。
///
/// 代理码点在 UTF-8 里非法，在 WTF-8 里**合法**（作为孤立代理的 3 字节形式 ✓）。
pub const fn is_surrogate(code_point: u32) -> bool {
    code_point >= SURROGATE_START && code_point <= SURROGATE_END
}

/// 是不是**标量值**（合法码点且不是代理）——即能装进 Rust `char` 的码点。
pub const fn is_scalar_value(code_point: u32) -> bool {
    code_point <= MAX_CODE_POINT && !is_surrogate(code_point)
}

/// WTF-8 错误的**类别**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wtf8ErrorKind {
    /// 序列在字节串结束处**被截断**（首字节还在，续字节不够）。
    UnexpectedEnd,
    /// 首字节不可能出现在任何 WTF-8 序列里（`0x80..=0xC1`／`0xF5..=0xFF`；`0xC0`/`0xC1`
    /// 永远只会产生过长编码，也归这一档）。
    InvalidLeadByte,
    /// 续字节不是 `10xxxxxx`。
    InvalidContinuation,
    /// **过长编码**（用比需要更长的形式编码了小码点，如 `C0 80`）。
    Overlong,
    /// 解出的码点**超过 [`MAX_CODE_POINT`]**（`F4 90..` 往后）。
    CodePointOutOfRange,
    /// **编码**请求的码点超过 [`MAX_CODE_POINT`]（`encode_code_point`；偏移恒 `0`）。
    EncodeCodePointOutOfRange,
}

impl core::fmt::Display for Wtf8ErrorKind {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let text = match self {
            Wtf8ErrorKind::UnexpectedEnd => "WTF-8 序列在结尾处被截断",
            Wtf8ErrorKind::InvalidLeadByte => "非法的 WTF-8 首字节",
            Wtf8ErrorKind::InvalidContinuation => "非法的 WTF-8 续字节",
            Wtf8ErrorKind::Overlong => "WTF-8 过长编码",
            Wtf8ErrorKind::CodePointOutOfRange => "WTF-8 码点超出 U+10FFFF",
            Wtf8ErrorKind::EncodeCodePointOutOfRange => "要编码的码点超出 U+10FFFF",
        };
        formatter.write_str(text)
    }
}

/// 一处在解码／编码中发现的 WTF-8 错误：**类别 ＋ 字节偏移**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wtf8Error {
    /// 出错的字节偏移：结构性错误取**序列起始**；续字节错误取**那个续字节**；
    /// 编码侧（[`Wtf8ErrorKind::EncodeCodePointOutOfRange`]）恒为 `0`。
    pub offset: usize,
    /// 错误类别。
    pub kind: Wtf8ErrorKind,
}

impl Wtf8Error {
    /// 造一个错误（内部与测试都用；偏移语义见类型注释 ✓）。
    pub const fn new(offset: usize, kind: Wtf8ErrorKind) -> Self {
        Self { offset, kind }
    }
}

impl core::fmt::Display for Wtf8Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}（偏移 {}）", self.kind, self.offset)
    }
}

impl std::error::Error for Wtf8Error {}

/// 把**一个码点**按 WTF-8 追加到 `out`。
///
/// 接受合法标量值**与孤立代理**（`U+D800..=U+DFFF`）；超过 [`MAX_CODE_POINT`] 的码点返回
/// [`Wtf8ErrorKind::EncodeCodePointOutOfRange`]（`u32` 已排除负值）。
///
/// 字节形式与 UTF-8 相同：
/// - `<= 0x7F` ⇒ 1 字节；
/// - `<= 0x7FF` ⇒ 2 字节（`110xxxxx 10xxxxxx`）；
/// - `<= 0xFFFF` ⇒ 3 字节（含**代理**：`ED A0..BF 80..BF`）；
/// - `<= 0x10FFFF` ⇒ 4 字节（不编码代理对——那是两个代理码点，会各编各的 ✓）。
///
/// 失败时**不**往 `out` 里写任何字节（要么整条成功、要么不动 ✓）。
pub fn encode_code_point(code_point: u32, out: &mut Vec<u8>) -> Result<(), Wtf8Error> {
    if code_point > MAX_CODE_POINT {
        return Err(Wtf8Error::new(
            0,
            Wtf8ErrorKind::EncodeCodePointOutOfRange,
        ));
    }
    if code_point <= 0x7F {
        out.push(code_point as u8);
    } else if code_point <= 0x7FF {
        out.push(0xC0 | (code_point >> 6) as u8);
        out.push(0x80 | (code_point & 0x3F) as u8);
    } else if code_point <= 0xFFFF {
        out.push(0xE0 | (code_point >> 12) as u8);
        out.push(0x80 | ((code_point >> 6) & 0x3F) as u8);
        out.push(0x80 | (code_point & 0x3F) as u8);
    } else {
        out.push(0xF0 | (code_point >> 18) as u8);
        out.push(0x80 | ((code_point >> 12) & 0x3F) as u8);
        out.push(0x80 | ((code_point >> 6) & 0x3F) as u8);
        out.push(0x80 | (code_point & 0x3F) as u8);
    }
    Ok(())
}

/// 读一个续字节（`10xxxxxx`）；不是 ⇒ [`Wtf8ErrorKind::InvalidContinuation`]（偏移取它自己）。
fn continuation(byte: u8, offset: usize) -> Result<u32, Wtf8Error> {
    if byte & 0xC0 == 0x80 {
        Ok(u32::from(byte & 0x3F))
    } else {
        Err(Wtf8Error::new(offset, Wtf8ErrorKind::InvalidContinuation))
    }
}

/// **从 `offset` 处解一个码点**，返回 `(码点, 下一个偏移)`。
///
/// 严格按 WTF-8：拒绝截断／非法首字节／非法续字节／过长编码／超出 `U+10FFFF`；
/// **允许**代理码点（这是 WTF-8 与 UTF-8 的唯一差别 ✓）。
pub fn next_code_point(bytes: &[u8], offset: usize) -> Result<(u32, usize), Wtf8Error> {
    let Some(&first) = bytes.get(offset) else {
        return Err(Wtf8Error::new(offset, Wtf8ErrorKind::UnexpectedEnd));
    };
    // 单序列长度与"首字节载荷位宽"由首字节决定；非法首字节当场拒 ✓。
    let (length, initial) = match first {
        // `0x00..=0x7F`：ASCII
        0x00..=0x7F => return Ok((u32::from(first), offset + 1)),
        // `C2..=DF`：2 字节（C0/C1 只会产生过长编码 ⇒ 归非法首字节 ✓）
        0xC2..=0xDF => (2usize, u32::from(first & 0x1F)),
        // `E0..=EF`：3 字节（含代理区编码 ED A0..BF ✓）
        0xE0..=0xEF => (3, u32::from(first & 0x0F)),
        // `F0..=F4`：4 字节（F4 90 往后的"超界"由下面的总界检查兜 ✓）
        0xF0..=0xF4 => (4, u32::from(first & 0x07)),
        // 其余（80..=C1、F5..=FF）不可能是任何 WTF-8 序列的首字节 ✓。
        _ => {
            return Err(Wtf8Error::new(offset, Wtf8ErrorKind::InvalidLeadByte));
        }
    };
    if bytes.len() < offset + length {
        return Err(Wtf8Error::new(offset, Wtf8ErrorKind::UnexpectedEnd));
    }
    let mut code_point = initial;
    for index in 1..length {
        let byte = bytes[offset + index];
        code_point = (code_point << 6) | continuation(byte, offset + index)?;
    }
    // **过长编码**：每种长度都有下界（`C0 80` 解出 0 却用了 2 字节 ⇒ 过长 ✓）。
    let minimum = match length {
        2 => 0x80,
        3 => 0x800,
        _ => 0x1_0000,
    };
    if code_point < minimum {
        return Err(Wtf8Error::new(offset, Wtf8ErrorKind::Overlong));
    }
    if code_point > MAX_CODE_POINT {
        return Err(Wtf8Error::new(offset, Wtf8ErrorKind::CodePointOutOfRange));
    }
    Ok((code_point, offset + length))
}

/// **严格解码**整串 WTF-8 ⇒ 码点表。
///
/// 代理码点**保留**在结果里（WTF-8 的语义 ✓：调用方自己决定怎么用）；非法字节返回
/// [`Wtf8Error`]（带偏移）。
pub fn decode(bytes: &[u8]) -> Result<Vec<u32>, Wtf8Error> {
    code_points(bytes).collect()
}

/// **严格校验**整串是不是合法 WTF-8（不建码点表；错误同 [`decode`] ✓）。
pub fn validate(bytes: &[u8]) -> Result<(), Wtf8Error> {
    let mut offset = 0usize;
    while offset < bytes.len() {
        let (_, next) = next_code_point(bytes, offset)?;
        offset = next;
    }
    Ok(())
}

/// 便捷判定：整串是不是合法 WTF-8（[`validate`] 的 `bool` 形态）。
pub fn is_valid_wtf8(bytes: &[u8]) -> bool {
    validate(bytes).is_ok()
}

/// **WTF-8 码点迭代器**（[`code_points`] 的返回类型）。
///
/// 每一项是一个 `Result<u32, Wtf8Error>`；遇到错误时**产出那一项后终止**（不 panic ✓）。
#[derive(Debug, Clone)]
pub struct CodePoints<'a> {
    bytes: &'a [u8],
    offset: usize,
    finished: bool,
}

impl Iterator for CodePoints<'_> {
    type Item = Result<u32, Wtf8Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        if self.offset >= self.bytes.len() {
            self.finished = true;
            return None;
        }
        match next_code_point(self.bytes, self.offset) {
            Ok((code_point, next)) => {
                self.offset = next;
                Some(Ok(code_point))
            }
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

/// **逐码点遍历** WTF-8 字节（严格；错误处产出 `Err` 后终止 ✓）。
pub fn code_points(bytes: &[u8]) -> CodePoints<'_> {
    CodePoints {
        bytes,
        offset: 0,
        finished: false,
    }
}

/// **有损**转成 Rust `String`（诊断／显示用；**不是**语义转换 ✓）。
///
/// 规则（如实说明，与 `String::from_utf8_lossy` **不同**）：
/// - 合法**标量**码点 ⇒ 原字符 ✓；
/// - **孤立代理**（WTF-8 合法但 `char` 装不下）⇒ 一个 `U+FFFD`；
/// - **非法字节序列** ⇒ 那个起始字节替换成一个 `U+FFFD`，然后**前进一个字节**继续
///   （所以 `ED A0` 这种截断 `ED` 与 `A0` 各得一个替换符 ✓）。
///
/// # 为什么不能拿它做语义转换
///
/// `U+FFFD` 是**不可逆**的：`'\ud800'` 与字面量 `'\u{FFFD}'` 会给出同一结果。
/// 语义层必须直接用 WTF-8 字节（S2 的 `StrObject` 存储 ✓）；本函数只给 `Debug`／错误消息／
/// 终端显示这类地方用 ✓。
pub fn to_string_lossy(bytes: &[u8]) -> String {
    let mut text = String::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        match next_code_point(bytes, offset) {
            Ok((code_point, next)) => {
                offset = next;
                if let Some(character) = char::from_u32(code_point) {
                    text.push(character);
                } else {
                    // 只能是合法代理 ⇒ 有损替换 ✓
                    text.push(REPLACEMENT_CHARACTER);
                }
            }
            // 非法序列：一个起始字节 ⇒ 一个替换符，前进一字节继续 ✓
            Err(_) => {
                text.push(REPLACEMENT_CHARACTER);
                offset += 1;
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 编码一个码点（单测便捷：只关心成功路径 ✓）。
    fn encoded(code_point: u32) -> Vec<u8> {
        let mut out = Vec::new();
        encode_code_point(code_point, &mut out).expect("码点应可编码");
        out
    }

    #[test]
    fn surrogate_predicate_covers_exactly_the_surrogate_block() {
        assert!(!is_surrogate(0xD7FF));
        assert!(is_surrogate(0xD800));
        assert!(is_surrogate(0xDBFF));
        assert!(is_surrogate(0xDC00));
        assert!(is_surrogate(0xDFFF));
        assert!(!is_surrogate(0xE000));
        assert!(is_surrogate(SURROGATE_START));
        assert!(is_surrogate(SURROGATE_END));
    }

    #[test]
    fn scalar_value_predicate_excludes_surrogates_and_out_of_range() {
        assert!(is_scalar_value(0));
        assert!(is_scalar_value(0xD7FF));
        assert!(!is_scalar_value(0xD800));
        assert!(!is_scalar_value(0xDFFF));
        assert!(is_scalar_value(0xE000));
        assert!(is_scalar_value(MAX_CODE_POINT));
        assert!(!is_scalar_value(MAX_CODE_POINT + 1));
    }

    #[test]
    fn encode_forms_match_utf8_for_all_boundaries() {
        assert_eq!(encoded(0x00), vec![0x00]);
        assert_eq!(encoded(0x7F), vec![0x7F]);
        assert_eq!(encoded(0x80), vec![0xC2, 0x80]);
        assert_eq!(encoded(0x7FF), vec![0xDF, 0xBF]);
        assert_eq!(encoded(0x800), vec![0xE0, 0xA0, 0x80]);
        assert_eq!(encoded(0xD800), vec![0xED, 0xA0, 0x80]);
        assert_eq!(encoded(0xDFFF), vec![0xED, 0xBF, 0xBF]);
        assert_eq!(encoded(0xFFFF), vec![0xEF, 0xBF, 0xBF]);
        assert_eq!(encoded(0x10000), vec![0xF0, 0x90, 0x80, 0x80]);
        assert_eq!(encoded(MAX_CODE_POINT), vec![0xF4, 0x8F, 0xBF, 0xBF]);
    }

    #[test]
    fn encode_rejects_code_points_above_the_limit_without_touching_out() {
        let mut out = vec![0xAA];
        let error = encode_code_point(MAX_CODE_POINT + 1, &mut out).expect_err("应拒绝");
        assert_eq!(error.kind, Wtf8ErrorKind::EncodeCodePointOutOfRange);
        assert_eq!(error.offset, 0);
        assert_eq!(out, vec![0xAA], "失败时不许写一半");
    }

    #[test]
    fn next_code_point_accepts_ascii_and_surrogates() {
        assert_eq!(next_code_point(b"A", 0), Ok((0x41, 1)));
        assert_eq!(next_code_point(&[0xED, 0xA0, 0x80], 0), Ok((0xD800, 3)));
        assert_eq!(next_code_point(&[0xED, 0xBF, 0xBF], 0), Ok((0xDFFF, 3)));
        assert_eq!(
            next_code_point(&[0xF4, 0x8F, 0xBF, 0xBF], 0),
            Ok((0x10_FFFF, 4))
        );
    }

    #[test]
    fn next_code_point_error_kinds_and_offsets() {
        // 截断：ED A0（3 字节形式只给 2 字节）⇒ 序列起始偏移
        assert_eq!(
            next_code_point(&[0xED, 0xA0], 0),
            Err(Wtf8Error::new(0, Wtf8ErrorKind::UnexpectedEnd))
        );
        // 过长：C0 80（非法首字节 C0 ⇒ 这一档，不走到"过长"✓）
        assert_eq!(
            next_code_point(&[0xC0, 0x80], 0),
            Err(Wtf8Error::new(0, Wtf8ErrorKind::InvalidLeadByte))
        );
        // 真正的过长：E0 80 80（3 字节形式编码 < 0x800）
        assert_eq!(
            next_code_point(&[0xE0, 0x80, 0x80], 0),
            Err(Wtf8Error::new(0, Wtf8ErrorKind::Overlong))
        );
        // 5 字节首字节 F8 ⇒ 非法首字节
        assert_eq!(
            next_code_point(&[0xF8, 0x88, 0x80, 0x80, 0x80], 0),
            Err(Wtf8Error::new(0, Wtf8ErrorKind::InvalidLeadByte))
        );
        // 续字节坏：E1 41 A0 ⇒ 坏续字节的偏移 1
        assert_eq!(
            next_code_point(&[0xE1, 0x41, 0xA0], 0),
            Err(Wtf8Error::new(1, Wtf8ErrorKind::InvalidContinuation))
        );
        // 超界：F4 90 80 80 解出 0x110000
        assert_eq!(
            next_code_point(&[0xF4, 0x90, 0x80, 0x80], 0),
            Err(Wtf8Error::new(0, Wtf8ErrorKind::CodePointOutOfRange))
        );
        // 空输入取码点 ⇒ 截断
        assert_eq!(
            next_code_point(b"", 0),
            Err(Wtf8Error::new(0, Wtf8ErrorKind::UnexpectedEnd))
        );
    }

    #[test]
    fn code_points_iterator_yields_error_then_stops() {
        let mut iterator = code_points(&[0x41, 0xED, 0xA0]);
        assert_eq!(iterator.next(), Some(Ok(0x41)));
        assert_eq!(
            iterator.next(),
            Some(Err(Wtf8Error::new(1, Wtf8ErrorKind::UnexpectedEnd)))
        );
        assert_eq!(iterator.next(), None, "出错后终止（不 panic、不无限循环）");
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn code_points_iterator_keeps_surrogates_as_code_points() {
        let points: Result<Vec<u32>, Wtf8Error> = code_points(&[0x61, 0xED, 0xA0, 0x80]).collect();
        assert_eq!(points, Ok(vec![0x61, 0xD800]));
    }

    #[test]
    fn to_string_lossy_replaces_surrogates_and_bad_bytes() {
        // 合法 UTF-8 原样 ✓
        assert_eq!(to_string_lossy("héllo 🦀".as_bytes()), "héllo 🦀");
        // 孤立代理 ⇒ U+FFFD ✓
        assert_eq!(to_string_lossy(&[0xED, 0xA0, 0x80]), "\u{FFFD}");
        // 混合：a + 代理 + b
        assert_eq!(
            to_string_lossy(&[0x61, 0xED, 0xA0, 0x80, 0x62]),
            "a\u{FFFD}b"
        );
        // 截断 ED A0 ⇒ 两个字节各一个替换符（本函数的既定口径 ✓）
        assert_eq!(to_string_lossy(&[0xED, 0xA0]), "\u{FFFD}\u{FFFD}");
        // 非法首字节同样替换 ✓
        assert_eq!(to_string_lossy(&[0xFF]), "\u{FFFD}");
        assert_eq!(to_string_lossy(b""), "");
    }

    #[test]
    fn validate_and_is_valid_agree() {
        assert!(validate(b"").is_ok());
        assert!(validate(b"abc").is_ok());
        assert!(validate(&[0xED, 0xA0, 0x80]).is_ok(), "孤立代理在 WTF-8 里合法");
        assert!(is_valid_wtf8(&[0xED, 0xA0, 0x80]));
        assert!(!is_valid_wtf8(&[0xED, 0xA0]));
        assert!(!is_valid_wtf8(&[0xC0, 0x80]));
        assert!(!is_valid_wtf8(&[0xF8]));
    }
}
