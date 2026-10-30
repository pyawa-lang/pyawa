//! **P3-38 S3a：`surrogateescape` / `surrogatepass` 编解码**（`Instance` 层契约测试）。
//!
//! 参照口径来自 python3 3.14.4 的实测矩阵（`target/recon/t60_ref_matrix.out`）：
//! - UTF-8 解码：`surrogateescape` **逐字节**映射 `U+DC00+byte`；`surrogatepass` 只认 3 字节代理序列；
//! - UTF-8 编码：`surrogateescape` 只还 `U+DC80..=U+DCFF`；`surrogatepass` 写 3 字节代理；
//!   含代理但处理器不匹配 ⇒ `UnicodeEncodeError: … surrogates not allowed`（连续多个用复数区间文案）；
//! - ascii／latin-1／cp437：`surrogateescape` 只救 `U+DC80..=U+DCFF`，其余照各编解码器的 strict 文案；
//! - 未知处理器 ⇒ `LookupError: unknown error handler name 'x'`。

use core::ptr::NonNull;

use pyawa_core::wtf8;
use pyawa_core::{ExecError, Header, Instance};

fn bytes(value: &[u8]) -> Vec<u8> {
    value.to_vec()
}

/// 用码点拼 WTF-8（测试期望值的来源）。
fn wtf8_of_codes(codes: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    for code in codes {
        wtf8::encode_code_point(*code, &mut out).expect("测试码点都在 WTF-8 范围内");
    }
    out
}

fn string_from_codes(instance: &Instance, codes: &[u32]) -> NonNull<Header> {
    instance
        .new_str_wtf8(&wtf8_of_codes(codes))
        .expect("WTF-8 合法")
}

fn decode(instance: &Instance, data: &[u8], errors: &str) -> Result<Vec<u8>, ExecError> {
    instance
        .decode_with_encoding_and_errors(data, "utf-8", errors)
        .map(|header| instance.wtf8_of(header).map(<[u8]>::to_vec).unwrap_or_default())
}

fn encode(instance: &Instance, codes: &[u32], errors: &str) -> Result<Vec<u8>, ExecError> {
    let text = string_from_codes(instance, codes);
    let bytes = instance.wtf8_of(text).expect("是 str");
    instance
        .encode_wtf8_with_encoding_and_errors(bytes, "utf-8", errors)
        .map(|header| instance.bytes_value(header).map(<[u8]>::to_vec).unwrap_or_default())
}

fn error_parts(instance: &Instance, error: ExecError) -> (String, Option<String>) {
    let ExecError::Raised { exception } = error else {
        panic!("期望抛异常，实际 {error:?}");
    };
    // SAFETY: 异常对象由本测试持有。
    let ty = unsafe { exception.as_ref() }.ty();
    // SAFETY: 类型由注册表持有。
    let name = unsafe { ty.as_ref() }.name().to_owned();
    // SAFETY: 异常对象是 ExceptionObject 布局。
    let message =
        unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }.message_with(instance);
    (name, message)
}

#[test]
fn utf8_decode_handlers_match_the_reference() {
    let instance = Instance::new();
    // strict 文案（与参照逐字一致）
    let (name, message) = error_parts(&instance, decode(&instance, b"\xff", "strict").unwrap_err());
    assert_eq!(name, "UnicodeDecodeError");
    assert_eq!(
        message.unwrap_or_default(),
        "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"
    );
    let (_, message) = error_parts(&instance, decode(&instance, b"\xc3", "strict").unwrap_err());
    assert_eq!(
        message.unwrap_or_default(),
        "'utf-8' codec can't decode byte 0xc3 in position 0: unexpected end of data"
    );
    // surrogateescape：**逐字节**映射
    assert_eq!(
        decode(&instance, b"\xff", "surrogateescape").unwrap(),
        wtf8_of_codes(&[0xDCFF])
    );
    assert_eq!(
        decode(&instance, b"\xed\xa0\x80", "surrogateescape").unwrap(),
        wtf8_of_codes(&[0xDCED, 0xDCA0, 0xDC80])
    );
    assert_eq!(
        decode(&instance, b"\xf0\x80\x80\x80", "surrogateescape").unwrap(),
        wtf8_of_codes(&[0xDCF0, 0xDC80, 0xDC80, 0xDC80])
    );
    assert_eq!(
        decode(&instance, b"\xc3(", "surrogateescape").unwrap(),
        wtf8_of_codes(&[0xDCC3, u32::from(b'(')])
    );
    // surrogatepass：只认 3 字节代理序列
    assert_eq!(
        decode(&instance, b"\xed\xa0\x80", "surrogatepass").unwrap(),
        wtf8_of_codes(&[0xD800])
    );
    assert_eq!(
        decode(&instance, b"\xed\xbf\xbf", "surrogatepass").unwrap(),
        wtf8_of_codes(&[0xDFFF])
    );
    let (name, _) = error_parts(&instance, decode(&instance, b"\xff", "surrogatepass").unwrap_err());
    assert_eq!(name, "UnicodeDecodeError");
    // ignore / replace（**按错误单元跨度**：结构错按 error_len、截断前缀整段算一个 ✓）
    assert_eq!(decode(&instance, b"\xff\xfe", "ignore").unwrap(), Vec::<u8>::new());
    assert_eq!(
        decode(&instance, b"\xff\xfe", "replace").unwrap(),
        wtf8_of_codes(&[0xFFFD, 0xFFFD])
    );
    assert_eq!(
        decode(&instance, b"\xed\xa0\x80", "replace").unwrap(),
        wtf8_of_codes(&[0xFFFD, 0xFFFD, 0xFFFD])
    );
    // **截断的多字节前缀算一个错误单元**（`b"ab\xe3\x81"` ⇒ `'ab�'`，sys-exit 实测的同一条 ✓）
    assert_eq!(
        decode(&instance, b"ab\xe3\x81", "replace").unwrap(),
        wtf8_of_codes(&[u32::from(b'a'), u32::from(b'b'), 0xFFFD])
    );
    assert_eq!(
        decode(&instance, b"\xe3\x81", "replace").unwrap(),
        wtf8_of_codes(&[0xFFFD])
    );
    assert_eq!(
        decode(&instance, b"\xf0\x9f\x98", "replace").unwrap(),
        wtf8_of_codes(&[0xFFFD])
    );
    // 结构错一次跨 error_len 字节：`b'\xe2\x82('` ⇒ `'�('` ✓
    assert_eq!(
        decode(&instance, b"\xe2\x82(", "replace").unwrap(),
        wtf8_of_codes(&[0xFFFD, u32::from(b'(')])
    );
    // surrogateescape 则在跨度内**逐字节**映射
    assert_eq!(
        decode(&instance, b"\xe2\x82(", "surrogateescape").unwrap(),
        wtf8_of_codes(&[0xDCE2, 0xDC82, u32::from(b'(')])
    );
    // 未知处理器
    let (name, message) = error_parts(&instance, decode(&instance, b"\xff", "bogus").unwrap_err());
    assert_eq!(name, "LookupError");
    assert_eq!(
        message.unwrap_or_default(),
        "unknown error handler name 'bogus'"
    );
}

#[test]
fn utf8_decode_final_round_keeps_truncated_tail() {
    let instance = Instance::new();
    // final=false：末尾截断序列留着（`_codecs.utf_8_decode(…, False)` 的口径）
    let (out, consumed) = instance
        .decode_utf8_with_errors(b"\xc3", "surrogateescape", false)
        .expect("不完整序列在非 final 轮应保留");
    assert_eq!(consumed, 0);
    assert!(out.is_empty());
    // final=true：逐字节映射
    let (out, consumed) = instance
        .decode_utf8_with_errors(b"\xc3", "surrogateescape", true)
        .expect("final 轮要处理");
    assert_eq!(consumed, 1);
    assert_eq!(out, wtf8_of_codes(&[0xDCC3]));
}

#[test]
fn utf8_encode_handlers_match_the_reference() {
    let instance = Instance::new();
    // surrogateescape：只还 U+DC80..=U+DCFF
    assert_eq!(encode(&instance, &[0xDC80], "surrogateescape").unwrap(), bytes(b"\x80"));
    assert_eq!(encode(&instance, &[0xDCFF], "surrogateescape").unwrap(), bytes(b"\xff"));
    assert_eq!(
        encode(&instance, &[0xDCED, 0xDCA0, 0xDC80], "surrogateescape").unwrap(),
        bytes(b"\xed\xa0\x80")
    );
    // surrogatepass：代理按 3 字节写
    assert_eq!(
        encode(&instance, &[0xD800], "surrogatepass").unwrap(),
        bytes(b"\xed\xa0\x80")
    );
    assert_eq!(
        encode(&instance, &[0xDFFF], "surrogatepass").unwrap(),
        bytes(b"\xed\xbf\xbf")
    );
    assert_eq!(
        encode(&instance, &[u32::from(b'a'), 0xD800, u32::from(b'b')], "surrogatepass").unwrap(),
        bytes(b"a\xed\xa0\x80b")
    );
    // strict：surrogates not allowed（单数 + 连续多个的复数区间文案）
    let (name, message) = error_parts(&instance, encode(&instance, &[0xD800], "strict").unwrap_err());
    assert_eq!(name, "UnicodeEncodeError");
    assert_eq!(
        message.unwrap_or_default(),
        "'utf-8' codec can't encode character '\\ud800' in position 0: surrogates not allowed"
    );
    let (_, message) = error_parts(
        &instance,
        encode(&instance, &[0xDCED, 0xDCA0, 0xDC80], "strict").unwrap_err(),
    );
    assert_eq!(
        message.unwrap_or_default(),
        "'utf-8' codec can't encode characters in position 0-2: surrogates not allowed"
    );
    // surrogateescape 对不可逃逸代理 ⇒ 同一句 surrogates not allowed
    let (name, _) = error_parts(&instance, encode(&instance, &[0xD800], "surrogateescape").unwrap_err());
    assert_eq!(name, "UnicodeEncodeError");
    // ignore / replace
    assert_eq!(encode(&instance, &[0xD800], "ignore").unwrap(), Vec::<u8>::new());
    assert_eq!(encode(&instance, &[0xD800], "replace").unwrap(), bytes(b"?"));
    // 未知处理器
    let (name, message) = error_parts(&instance, encode(&instance, &[0xD800], "bogus").unwrap_err());
    assert_eq!(name, "LookupError");
    assert_eq!(message.unwrap_or_default(), "unknown error handler name 'bogus'");
}

#[test]
fn single_byte_codecs_apply_surrogateescape_only_to_the_escape_range() {
    let instance = Instance::new();
    // ascii：U+DC80..DCFF ⇒ 单字节；其余照 ordinal not in range(128)
    let text = string_from_codes(&instance, &[0xDC80]);
    let bytes = instance.wtf8_of(text).expect("是 str");
    let encoded = instance
        .encode_wtf8_with_encoding_and_errors(bytes, "ascii", "surrogateescape")
        .expect("DC80 可逃逸");
    assert_eq!(instance.bytes_value(encoded), Some(b"\x80".as_slice()));
    let text = string_from_codes(&instance, &[0xD800]);
    let bytes = instance.wtf8_of(text).expect("是 str");
    let (name, message) = error_parts(
        &instance,
        instance
            .encode_wtf8_with_encoding_and_errors(bytes, "ascii", "surrogateescape")
            .unwrap_err(),
    );
    assert_eq!(name, "UnicodeEncodeError");
    assert_eq!(
        message.unwrap_or_default(),
        "'ascii' codec can't encode character '\\ud800' in position 0: ordinal not in range(128)"
    );
    // ascii 解码：surrogateescape 把高位字节映成 U+DCxx；surrogatepass 等同 strict
    let decoded = instance
        .decode_with_encoding_and_errors(b"\xff", "ascii", "surrogateescape")
        .expect("可逃逸");
    assert_eq!(instance.wtf8_of(decoded), Some(wtf8_of_codes(&[0xDCFF]).as_slice()));
    let (name, _) = error_parts(
        &instance,
        instance
            .decode_with_encoding_and_errors(b"\xff", "ascii", "surrogatepass")
            .unwrap_err(),
    );
    assert_eq!(name, "UnicodeDecodeError");
    // latin-1 编码的同族文案（ordinal not in range(256)）
    let text = string_from_codes(&instance, &[0xD800]);
    let bytes = instance.wtf8_of(text).expect("是 str");
    let (_, message) = error_parts(
        &instance,
        instance
            .encode_wtf8_with_encoding_and_errors(bytes, "latin-1", "surrogateescape")
            .unwrap_err(),
    );
    assert_eq!(
        message.unwrap_or_default(),
        "'latin-1' codec can't encode character '\\ud800' in position 0: ordinal not in range(256)"
    );
    // cp437 解码：0xff 有映射（NBSP），不做逃逸
    let decoded = instance
        .decode_with_encoding_and_errors(b"\xff", "cp437", "surrogateescape")
        .expect("cp437 的 0xff 有映射");
    assert_eq!(
        instance.wtf8_of(decoded),
        Some(wtf8_of_codes(&[0xA0]).as_slice())
    );
}
