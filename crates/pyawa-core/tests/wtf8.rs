//! **WTF-8 编解码**（P3-38 S1）的**往返／边界／错误**测试。
//!
//! 规范口径见 `crates/pyawa-core/src/wtf8.rs` 的模块文档：WTF-8 ＝ UTF-8 的超集，**允许
//! 孤立代理**（3 字节 `ED A0..BF 80..BF`）；其余违规与 UTF-8 相同（截断／过长／非法首字节／
//! 非法续字节／超出 `U+10FFFF`）。这里**只走公开 API**（集成测试，不借内部实现 ✓）。

use pyawa_core::wtf8::{
    code_points, decode, encode_code_point, is_scalar_value, is_surrogate, is_valid_wtf8,
    to_string_lossy, validate, Wtf8Error, Wtf8ErrorKind, MAX_CODE_POINT, REPLACEMENT_CHARACTER,
    SURROGATE_END, SURROGATE_START,
};

/// 把一串码点编成 WTF-8（测试便捷；任一码点不可编就当场 panic —— 那本身是测试失败 ✓）。
fn encode(points: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    for &point in points {
        encode_code_point(point, &mut out).expect("测试里的码点都应可编码");
    }
    out
}

/// 一个边界码点表：ASCII 边界、2／3／4 字节分界、代理端点、上限 ✓。
const BOUNDARIES: [u32; 11] = [
    0x00,
    0x7F,
    0x80,
    0x7FF,
    0x800,
    0xFFFF,
    0xD7FF,
    0xD800,
    0xDFFF,
    0x10000,
    MAX_CODE_POINT,
];

#[test]
fn wtf8_fixed_boundary_bytes_match_the_specification() {
    // 与 UTF-8 相同的那部分（非代理）＋ WTF-8 独有的代理 3 字节形式。
    assert_eq!(encode(&[0x7F]), vec![0x7F]);
    assert_eq!(encode(&[0x80]), vec![0xC2, 0x80]);
    assert_eq!(encode(&[0x7FF]), vec![0xDF, 0xBF]);
    assert_eq!(encode(&[0x800]), vec![0xE0, 0xA0, 0x80]);
    assert_eq!(encode(&[0xFFFF]), vec![0xEF, 0xBF, 0xBF]);
    assert_eq!(encode(&[0x10000]), vec![0xF0, 0x90, 0x80, 0x80]);
    assert_eq!(
        encode(&[MAX_CODE_POINT]),
        vec![0xF4, 0x8F, 0xBF, 0xBF]
    );
    // 孤立代理（WTF-8 的核心形态 ✓）
    assert_eq!(encode(&[0xD800]), vec![0xED, 0xA0, 0x80]);
    assert_eq!(encode(&[0xDBFF]), vec![0xED, 0xAF, 0xBF]);
    assert_eq!(encode(&[0xDC00]), vec![0xED, 0xB0, 0x80]);
    assert_eq!(encode(&[0xDFFF]), vec![0xED, 0xBF, 0xBF]);
}

#[test]
fn every_boundary_code_point_round_trips() {
    for &point in &BOUNDARIES {
        let bytes = encode(&[point]);
        assert_eq!(
            decode(&bytes),
            Ok(vec![point]),
            "码点 {point:#06X} 的往返"
        );
        // 编码 → 解码 → 再编码：字节完全相同 ✓
        assert_eq!(encode(&decode(&bytes).expect("解得开")), bytes);
        assert!(validate(&bytes).is_ok(), "码点 {point:#06X} 应通过校验");
    }
}

#[test]
fn mixed_sequences_round_trip_byte_identically() {
    let cases: [&[u32]; 9] = [
        &[],
        &[0x61, 0x62, 0x63],
        &[0xE9],                          // é
        &[0x4F60, 0x597D],                // 你好
        &[0x1F980],                       // 🦀
        &[0xD800],                        // 孤立低位起点
        &[0xDFFF],                        // 孤立代理终点
        &[0x61, 0xD800, 0x62],            // a + 代理 + b
        &[0x1F980, 0xD800, 0xE9, 0x10FFFF], // 非 BMP + 代理 + BMP + 上限
    ];
    for points in cases {
        let bytes = encode(points);
        assert_eq!(decode(&bytes), Ok(points.to_vec()), "{points:?} 的往返");
        // 二次编码必须与首次编码**逐字节相同** ✓
        let again = encode(&decode(&bytes).expect("解得开"));
        assert_eq!(again, bytes, "{points:?} 的字节往返");
        assert!(validate(&bytes).is_ok());
    }
}

#[test]
fn valid_utf8_is_valid_wtf8() {
    // WTF-8 是 UTF-8 的超集：任何合法 UTF-8 都必须通过（且解出同样码点 ✓）。
    for text in ["", "ascii", "héllo", "你好", "🦀 crab 🦀"] {
        let bytes = text.as_bytes();
        assert!(is_valid_wtf8(bytes), "{text:?} 的 UTF-8 应是合法 WTF-8");
        let points: Vec<u32> = text.chars().map(u32::from).collect();
        assert_eq!(decode(bytes), Ok(points));
    }
}

#[test]
fn encode_rejects_out_of_range_code_points() {
    for point in [MAX_CODE_POINT + 1, 0x11_0000, u32::MAX] {
        let mut out = Vec::new();
        let error = encode_code_point(point, &mut out).expect_err("应拒绝");
        assert_eq!(error.kind, Wtf8ErrorKind::EncodeCodePointOutOfRange);
        assert_eq!(error.offset, 0);
        assert!(out.is_empty(), "失败时不许写一半");
    }
    // 上限本身可编 ✓
    let mut out = Vec::new();
    assert_eq!(encode_code_point(MAX_CODE_POINT, &mut out), Ok(()));
    assert!(!out.is_empty());
}

#[test]
fn truncated_sequences_are_rejected_with_the_sequence_offset() {
    // ED A0 是代理 3 字节形式的前两字节 ⇒ 截断。
    let error = decode(&[0xED, 0xA0]).expect_err("截断应报错");
    assert_eq!(error.kind, Wtf8ErrorKind::UnexpectedEnd);
    assert_eq!(error.offset, 0);
    // 前缀合法时偏移取**序列起始**，不是串首 ✓
    let error = decode(&[0x41, 0x62, 0xED, 0xA0]).expect_err("截断应报错");
    assert_eq!(error.offset, 2);
    // 各长度分别截断
    for bytes in [
        vec![0xC2],
        vec![0xE0, 0xA0],
        vec![0xF0, 0x90, 0x80],
    ] {
        let error = decode(&bytes).expect_err("截断应报错");
        assert_eq!(error.kind, Wtf8ErrorKind::UnexpectedEnd, "{bytes:?}");
    }
}

#[test]
fn overlong_and_illegal_lead_bytes_are_rejected() {
    // 经典过长编码 C0 80：C0 永远非法（只会产生过长）⇒ 非法首字节 ✓
    let error = decode(&[0xC0, 0x80]).expect_err("过长编码应报错");
    assert_eq!(error.kind, Wtf8ErrorKind::InvalidLeadByte);
    assert_eq!(error.offset, 0);
    // 3 字节形式编码 < 0x800 ⇒ 真正的"过长"档 ✓
    let error = decode(&[0xE0, 0x80, 0x80]).expect_err("过长编码应报错");
    assert_eq!(error.kind, Wtf8ErrorKind::Overlong);
    // 4 字节形式编码 < 0x10000
    let error = decode(&[0xF0, 0x80, 0x80, 0x80]).expect_err("过长编码应报错");
    assert_eq!(error.kind, Wtf8ErrorKind::Overlong);
    // 5 字节首字节 F8（历史 UTF-8 的 5 字节形式）⇒ 非法首字节 ✓
    let error = decode(&[0xF8, 0x88, 0x80, 0x80, 0x80]).expect_err("5 字节应报错");
    assert_eq!(error.kind, Wtf8ErrorKind::InvalidLeadByte);
    // 其它非法首字节段
    for lead in [0x80u8, 0xBF, 0xC1, 0xF5, 0xFF] {
        let error = decode(&[lead]).expect_err("非法首字节应报错");
        assert_eq!(error.kind, Wtf8ErrorKind::InvalidLeadByte, "{lead:#04X}");
    }
}

#[test]
fn invalid_continuation_bytes_are_rejected_with_their_own_offset() {
    // E1 41 A0：第二个字节不是续字节 ⇒ 偏移 1 ✓
    let error = decode(&[0xE1, 0x41, 0xA0]).expect_err("坏续字节应报错");
    assert_eq!(error.kind, Wtf8ErrorKind::InvalidContinuation);
    assert_eq!(error.offset, 1);
    // F0 90 80 41：第四个字节坏 ⇒ 偏移 3 ✓
    let error = decode(&[0xF0, 0x90, 0x80, 0x41]).expect_err("坏续字节应报错");
    assert_eq!(error.offset, 3);
}

#[test]
fn code_points_above_the_unicode_limit_are_rejected() {
    // F4 90 80 80 ⇒ 0x110000（超出 U+10FFFF）
    let error = decode(&[0xF4, 0x90, 0x80, 0x80]).expect_err("超出上限应报错");
    assert_eq!(error.kind, Wtf8ErrorKind::CodePointOutOfRange);
    assert_eq!(error.offset, 0);
}

#[test]
fn validate_matches_decode_on_both_good_and_bad_input() {
    for good in [
        b"".as_slice(),
        b"abc".as_slice(),
        &[0xED, 0xA0, 0x80],
        &[0xF4, 0x8F, 0xBF, 0xBF],
    ] {
        assert_eq!(validate(good), Ok(()), "{good:?}");
        assert!(decode(good).is_ok());
        assert!(is_valid_wtf8(good));
    }
    for bad in [
        &[0xED, 0xA0][..],
        &[0xC0, 0x80],
        &[0xF8],
        &[0xE1, 0x41, 0xA0],
    ] {
        assert!(validate(bad).is_err(), "{bad:?}");
        assert!(decode(bad).is_err());
        assert!(!is_valid_wtf8(bad));
    }
}

#[test]
fn code_points_exposes_lone_surrogates_as_code_points() {
    // WTF-8 的语义面：解码后**保留**代理码点（Rust char 装不下，但 u32 装得下 ✓）
    let points: Vec<Result<u32, Wtf8Error>> = code_points(&[0x61, 0xED, 0xA0, 0x80, 0x62]).collect();
    assert_eq!(points, vec![Ok(0x61), Ok(0xD800), Ok(0x62)]);
    // 出错项之后迭代终止 ✓
    let mut iterator = code_points(&[0x41, 0xED, 0xA0]);
    assert_eq!(iterator.next(), Some(Ok(0x41)));
    assert_eq!(
        iterator.next(),
        Some(Err(Wtf8Error::new(1, Wtf8ErrorKind::UnexpectedEnd)))
    );
    assert_eq!(iterator.next(), None);
}

#[test]
fn to_string_lossy_is_for_display_only_and_replaces_surrogates() {
    // 合法文本原样 ✓
    assert_eq!(to_string_lossy("héllo 🦀".as_bytes()), "héllo 🦀");
    // 孤立代理 ⇒ U+FFFD（**不可逆**：这正是"只用于显示"的原因 ✓）
    assert_eq!(
        to_string_lossy(&[0xED, 0xA0, 0x80]),
        REPLACEMENT_CHARACTER.to_string()
    );
    assert_eq!(
        to_string_lossy(&[0x61, 0xED, 0xA0, 0x80, 0x62]),
        format!("a{}b", REPLACEMENT_CHARACTER)
    );
    // 非法字节 ⇒ 替换；不与"真 U+FFFD"区分（如实注释里写明 ✓）
    assert_eq!(
        to_string_lossy(&[0xFF]),
        REPLACEMENT_CHARACTER.to_string()
    );
    assert_eq!(to_string_lossy(b""), "");
}

#[test]
fn surrogate_helpers_agree_with_encoding() {
    assert!(is_surrogate(SURROGATE_START));
    assert!(is_surrogate(SURROGATE_END));
    assert!(!is_surrogate(SURROGATE_START - 1));
    assert!(!is_surrogate(SURROGATE_END + 1));
    assert!(!is_scalar_value(SURROGATE_START));
    assert!(is_scalar_value(SURROGATE_END + 1));
    assert!(!is_scalar_value(MAX_CODE_POINT + 1));
}
