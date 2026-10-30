//! `chr`／`ord` 的**孤立代理**面（P3-38 S2a-2 批 2，task-55）契约测试。
//!
//! 期望值来自本机 `python3` 3.14.4 实测：`chr(0xD800)` ⇒ `'\ud800'`（**1 个码点**）、
//! `ord` 回 55296；范围/类型/长度/实参个数文案照参照。构造走 `Instance::new_str_wtf8`
//! （单码点 WTF-8；合法标量仍是 UTF-8 表示 ⇒ 零行为改动）。

use core::ptr::NonNull;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance};
use pyawa_stdlib::builtins_module;

fn native(instance: &Instance, namespace: NonNull<Header>, name: &str) -> pyawa_core::NativeFn {
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("`{name}` 必须在 builtins 命名空间里"));
    // SAFETY: builtins 里这些名字放的是原生可调用对象。
    unsafe { (*function.as_ptr().cast::<BuiltinFunctionObject>()).function() }
}

fn call(
    instance: &Instance,
    handler: pyawa_core::NativeFn,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 实参由本测试持有（handler 只借用）。
    unsafe { handler(instance, None, args, kwargs) }
}

/// 异常对象的 `"类名: 消息"`。
fn error_text(instance: &Instance, error: ExecError) -> String {
    match error {
        ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            let type_name = unsafe { ty.as_ref() }.name().to_owned();
            let message = instance.object_str(exception).unwrap_or_default();
            format!("{type_name}: {message}")
        }
        other => panic!("期望 Python 异常，实际 {other:?}"),
    }
}

#[test]
fn chr_accepts_lone_surrogates_via_wtf8() {
    let instance = Instance::new();
    let namespace = builtins_module::build(&instance);
    let chr = native(&instance, namespace, "chr");
    for (code_point, expected_bytes) in [
        (0xD800u32, vec![0xED, 0xA0, 0x80]),
        (0xDFFF, vec![0xED, 0xBF, 0xBF]),
        // 合法标量：WTF-8 字节就是 UTF-8（零行为改动）。
        (0x10FFFF, vec![0xF4, 0x8F, 0xBF, 0xBF]),
        (0x41, vec![0x41]),
    ] {
        let argument = instance.new_int(i64::from(code_point));
        let text = call(&instance, chr, &[argument], &[]).expect("chr 不应失败");
        assert_eq!(
            instance.str_wtf8_bytes(text),
            Some(expected_bytes.as_slice()),
            "chr(0x{code_point:X}) 的 WTF-8 字节"
        );
        // **码点长度**：代理各算 1 个码点（`len()` 的语义口径；`builtin len` 的内核面见报告）。
        assert_eq!(instance.str_code_point_len(text), Some(1), "chr(0x{code_point:X}) 的码点长度");
        // repr 面（S2a-2 批 1）：孤立代理给 `'\ud800'` 这类转义形。
        let representation = instance.object_repr(text).expect("repr 不应失败");
        if code_point == 0xD800 {
            assert_eq!(representation, "'\\ud800'");
        }
        if code_point == 0xDFFF {
            assert_eq!(representation, "'\\udfff'");
        }
    }
    // `chr(True)` ⇒ `'\x01'`（bool 是 int 子类）；与参照同。
    let true_object = instance.retain(instance.singletons().boolean(true));
    let one = call(&instance, chr, &[true_object], &[]).expect("chr(True)");
    assert_eq!(instance.str_wtf8_bytes(one), Some(&b"\x01"[..]));
}

#[test]
fn ord_round_trips_lone_surrogates() {
    let instance = Instance::new();
    let namespace = builtins_module::build(&instance);
    let chr = native(&instance, namespace, "chr");
    let ord = native(&instance, namespace, "ord");
    for code_point in [0x41i64, 0x4E2D, 0xD800, 0xDFFF, 0x10FFFF] {
        let argument = instance.new_int(code_point);
        let text = call(&instance, chr, &[argument], &[]).expect("chr");
        let parsed = call(&instance, ord, &[text], &[]).expect("ord 不应失败");
        assert_eq!(instance.int_value(parsed), Some(code_point), "chr/ord 往返");
    }
    // 多码点/空串：文案按**码点数**（照参照）。
    for (text, expected) in [
        ("ab", "TypeError: ord() expected a character, but string of length 2 found"),
        ("", "TypeError: ord() expected a character, but string of length 0 found"),
    ] {
        let argument = instance.new_str(text);
        let error = call(&instance, ord, &[argument], &[]).expect_err("长度不为 1 必须报错");
        assert_eq!(error_text(&instance, error), expected);
    }
}

#[test]
fn error_messages_match_the_reference() {
    let instance = Instance::new();
    let namespace = builtins_module::build(&instance);
    let chr = native(&instance, namespace, "chr");
    let ord = native(&instance, namespace, "ord");

    // 范围错误（负数与 >= 0x110000 同一句文案）。
    for code in [-1i64, 0x11_0000] {
        let argument = instance.new_int(code);
        let error = call(&instance, chr, &[argument], &[]).expect_err("越界必须报错");
        assert_eq!(error_text(&instance, error), "ValueError: chr() arg not in range(0x110000)", "{code}");
    }
    // 非整数。
    let text = instance.new_str("a");
    let error = call(&instance, chr, &[text], &[]).expect_err("str 必须报 TypeError");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: 'str' object cannot be interpreted as an integer"
    );
    // 实参个数 / 关键字（参照文案）。
    let error = call(&instance, chr, &[], &[]).expect_err("缺实参");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: chr() takes exactly one argument (0 given)"
    );
    let error = call(&instance, ord, &[], &[]).expect_err("缺实参");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: ord() takes exactly one argument (0 given)"
    );
    let key = instance.new_str("i");
    let value = instance.new_int(65);
    let error = call(&instance, chr, &[], &[(key, value)]).expect_err("关键字必须报错");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: chr() takes no keyword arguments"
    );
    // 非 str 的 ord（`NoneType` 照参照）。
    let none = instance.retain(instance.singletons().none());
    let error = call(&instance, ord, &[none], &[]).expect_err("None 必须报 TypeError");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: ord() expected string of length 1, but NoneType found"
    );
}

#[test]
fn str_is_identity_on_str() {
    // 现状确认（task-55 ③）：`str(x)` 对 **`str` 精确类型**返回原对象。
    // 注意：`builtins.str` 在本层是**类型对象**（构造槽走 core），所以这里经由 CLI 探针确认；
    // 该测试只钉"本层的 `str` 名字面"——用 `str` 类型对象调用它。
    let instance = Instance::new();
    let namespace = builtins_module::build(&instance);
    let str_type = instance
        .dict_get(namespace, "str")
        .expect("builtins.str 必须存在");
    let text = instance.new_str("abc");
    let called = pyawa_core::executor::call::call_value(&instance, str_type, &[text], &[])
        .expect("str('abc') 不应失败");
    assert_eq!(called, text, "`str` 对 str 必须返回原对象（identity）");
}
