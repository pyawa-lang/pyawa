//! **P3-38 S3a：`_codecs` 模块的 `surrogateescape`／`surrogatepass`**（stdlib 面契约测试）。
//!
//! 四套实现里的"`_codecs` 这一套"：`utf_8_decode/encode`、`ascii_*`、`charmap_*`、以及
//! `_codecs.encode/decode` 直调；语义与 `Instance` 的核心编解码器**同一处真相**（本测试钉
//! 模块面的形状：返回二元组、`consumed`、WTF-8 结果、`lookup_error` 可取到内建处理器）。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};
use pyawa_stdlib::codecs_module;

fn call(
    instance: &Instance,
    namespace: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("`_codecs` 缺 {name}"));
    pyawa_core::executor::call::call_value(instance, function, args, &[])
}

fn pair_parts(instance: &Instance, value: NonNull<Header>) -> (Vec<u8>, i64) {
    let items = instance.tuple_items(value).expect("二元组");
    assert_eq!(items.len(), 2, "二元组");
    let wtf8 = instance
        .wtf8_of(items[0])
        .map(<[u8]>::to_vec)
        .expect("第一项是 str");
    let consumed = instance
        .int_of(items[1])
        .and_then(|value| value.to_i64())
        .expect("第二项是 int");
    (wtf8, consumed)
}

fn wtf8_of_codes(codes: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    for code in codes {
        pyawa_core::wtf8::encode_code_point(*code, &mut out).expect("测试码点合法");
    }
    out
}

#[test]
fn utf8_decode_and_encode_surrogate_handlers() {
    let instance = Instance::new();
    let namespace = codecs_module::build(&instance);
    // `utf_8_decode(b'\xff', 'surrogateescape')` ⇒ ('\udcff', 1)
    let input = instance.new_bytes(b"\xff");
    let errors = instance.new_str("surrogateescape");
    let result = call(&instance, namespace, "utf_8_decode", &[input, errors]).expect("可解码");
    let (wtf8, consumed) = pair_parts(&instance, result);
    assert_eq!(wtf8, wtf8_of_codes(&[0xDCFF]));
    assert_eq!(consumed, 1);
    // surrogatepass：`ED A0 80` ⇒ U+D800，消耗 3
    let input = instance.new_bytes(b"\xed\xa0\x80");
    let errors = instance.new_str("surrogatepass");
    let result = call(&instance, namespace, "utf_8_decode", &[input, errors]).expect("可解码");
    let (wtf8, consumed) = pair_parts(&instance, result);
    assert_eq!(wtf8, wtf8_of_codes(&[0xD800]));
    assert_eq!(consumed, 3);
    // `_codecs.utf_8_encode('\udcff', 'surrogateescape')` ⇒ (b'\xff', 1)
    let text = instance
        .new_str_wtf8(&wtf8_of_codes(&[0xDCFF]))
        .expect("WTF-8 合法");
    let errors = instance.new_str("surrogateescape");
    let result = call(&instance, namespace, "utf_8_encode", &[text, errors]).expect("可编码");
    let items = instance.tuple_items(result).expect("二元组");
    assert_eq!(instance.bytes_value(items[0]), Some(b"\xff".as_slice()));
    assert_eq!(instance.int_of(items[1]).and_then(|v| v.to_i64()), Some(1));
    // surrogatepass：'\ud800' ⇒ ED A0 80
    let text = instance
        .new_str_wtf8(&wtf8_of_codes(&[0xD800]))
        .expect("WTF-8 合法");
    let errors = instance.new_str("surrogatepass");
    let result = call(&instance, namespace, "utf_8_encode", &[text, errors]).expect("可编码");
    let items = instance.tuple_items(result).expect("二元组");
    assert_eq!(
        instance.bytes_value(items[0]),
        Some(b"\xed\xa0\x80".as_slice())
    );
}

#[test]
fn utf8_decode_final_argument_keeps_truncated_tail() {
    let instance = Instance::new();
    let namespace = codecs_module::build(&instance);
    // final=False（显式传 False）：截断序列留着 ⇒ ('', 0)
    let input = instance.new_bytes(b"\xc3");
    let errors = instance.new_str("surrogateescape");
    let final_flag = instance.singletons().boolean(false);
    let result = call(
        &instance,
        namespace,
        "utf_8_decode",
        &[input, errors, final_flag],
    )
    .expect("final=False 可解");
    let (wtf8, consumed) = pair_parts(&instance, result);
    assert!(wtf8.is_empty());
    assert_eq!(consumed, 0);
    // final=True：逐字节映射 ⇒ ('\udcc3', 1)
    let input = instance.new_bytes(b"\xc3");
    let errors = instance.new_str("surrogateescape");
    let final_flag = instance.singletons().boolean(true);
    let result = call(
        &instance,
        namespace,
        "utf_8_decode",
        &[input, errors, final_flag],
    )
    .expect("final=True 可解");
    let (wtf8, consumed) = pair_parts(&instance, result);
    assert_eq!(wtf8, wtf8_of_codes(&[0xDCC3]));
    assert_eq!(consumed, 1);
}

#[test]
fn ascii_and_charmap_apply_surrogateescape() {
    let instance = Instance::new();
    let namespace = codecs_module::build(&instance);
    // ascii_decode(b'\xff', 'surrogateescape') ⇒ '\udcff'
    let input = instance.new_bytes(b"\xff");
    let errors = instance.new_str("surrogateescape");
    let result = call(&instance, namespace, "ascii_decode", &[input, errors]).expect("可解码");
    let (wtf8, consumed) = pair_parts(&instance, result);
    assert_eq!(wtf8, wtf8_of_codes(&[0xDCFF]));
    assert_eq!(consumed, 1);
    // ascii_encode('\udc80', 'surrogateescape') ⇒ b'\x80'
    let text = instance
        .new_str_wtf8(&wtf8_of_codes(&[0xDC80]))
        .expect("WTF-8 合法");
    let errors = instance.new_str("surrogateescape");
    let result = call(&instance, namespace, "ascii_encode", &[text, errors]).expect("可编码");
    let items = instance.tuple_items(result).expect("二元组");
    assert_eq!(instance.bytes_value(items[0]), Some(b"\x80".as_slice()));
    // charmap_decode（映射里没有 0xff 的定义）⇒ 缺定义的字节按 surrogateescape 映成 '\udcff'
    let empty_mapping = instance.new_dict();
    let input = instance.new_bytes(b"\xff");
    let errors = instance.new_str("surrogateescape");
    let result = call(
        &instance,
        namespace,
        "charmap_decode",
        &[input, errors, empty_mapping],
    )
    .expect("可解码");
    let (wtf8, consumed) = pair_parts(&instance, result);
    assert_eq!(wtf8, wtf8_of_codes(&[0xDCFF]));
    assert_eq!(consumed, 1);
    // charmap_encode('\udcff', 'surrogateescape', {}) ⇒ b'\xff'
    let text = instance
        .new_str_wtf8(&wtf8_of_codes(&[0xDCFF]))
        .expect("WTF-8 合法");
    let errors = instance.new_str("surrogateescape");
    let target_mapping = instance.new_dict();
    let result = call(
        &instance,
        namespace,
        "charmap_encode",
        &[text, errors, target_mapping],
    )
    .expect("可编码");
    let items = instance.tuple_items(result).expect("二元组");
    assert_eq!(instance.bytes_value(items[0]), Some(b"\xff".as_slice()));
}

#[test]
fn codecs_encode_decode_entries_and_lookup_error() {
    let instance = Instance::new();
    let namespace = codecs_module::build(&instance);
    // `_codecs.encode('\udcff', 'utf-8', 'surrogateescape')` ⇒ b'\xff'
    let text = instance
        .new_str_wtf8(&wtf8_of_codes(&[0xDCFF]))
        .expect("WTF-8 合法");
    let encoding = instance.new_str("utf-8");
    let errors = instance.new_str("surrogateescape");
    let result = call(
        &instance,
        namespace,
        "encode",
        &[text, encoding, errors],
    )
    .expect("可编码");
    assert_eq!(instance.bytes_value(result), Some(b"\xff".as_slice()));
    // `_codecs.decode(b'\xff', 'utf-8', 'surrogateescape')` ⇒ '\udcff'
    let input = instance.new_bytes(b"\xff");
    let encoding = instance.new_str("utf-8");
    let errors = instance.new_str("surrogateescape");
    let result = call(
        &instance,
        namespace,
        "decode",
        &[input, encoding, errors],
    )
    .expect("可解码");
    assert_eq!(instance.wtf8_of(result), Some(wtf8_of_codes(&[0xDCFF]).as_slice()));
    // 内建处理器的 `lookup_error` 正查由 CLI 探针覆盖（本测试只手工建模块命名空间，
    // 没把模块登记进 `sys.modules` ⇒ `lookup_error` 的注册表路径取不到；未知名字那条不依赖注册表 ✓）。
    // 未知处理器仍按参照报 LookupError
    let name = instance.new_str("bogus");
    match call(&instance, namespace, "lookup_error", &[name]) {
        Err(ExecError::Raised { .. }) => {
            let (kind, message) = instance
                .pending_exception()
                .map(|exception| {
                    // SAFETY: 异常对象由实例持有且存活。
                    let ty = unsafe { exception.as_ref() }.ty();
                    // SAFETY: 类型由注册表持有。
                    let name = unsafe { ty.as_ref() }.name().to_owned();
                    // SAFETY: 异常为 ExceptionObject 布局。
                    let message = unsafe {
                        &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>()
                    }
                    .message_with(&instance);
                    (name, message)
                })
                .expect("应有未决异常");
            assert_eq!(kind, "LookupError");
            assert_eq!(
                message.unwrap_or_default(),
                "unknown error handler name 'bogus'"
            );
        }
        other => panic!("应当抛 LookupError，实际 {other:?}"),
    }
}
