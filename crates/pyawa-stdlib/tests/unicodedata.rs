//! `unicodedata` 模块**最小诚实面**（round 19，task-40）的契约测试。
//!
//! 期望值全部来自本机 `python3` 3.14.4 实测；**数据面**复用 `unicode_tables.rs`（同一参照导出），
//! 测试只钉「表里有数据的真实现 ✓／没数据的如实 NotImplementedError ✓」这条边界。

use core::ptr::NonNull;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance};
use pyawa_stdlib::unicodedata_module;

fn native(instance: &Instance, namespace: NonNull<Header>, name: &str) -> pyawa_core::NativeFn {
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("`unicodedata.{name}` 必须存在"));
    // SAFETY: 模块里放进去的都是原生可调用对象。
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

fn int_of(instance: &Instance, object: NonNull<Header>) -> i64 {
    instance.int_value(object).expect("整数对象")
}

fn float_of(instance: &Instance, object: NonNull<Header>) -> f64 {
    instance.float_value(object).expect("浮点对象")
}

fn text_of(instance: &Instance, object: NonNull<Header>) -> String {
    instance.text_of(object).expect("字符串对象").to_owned()
}

#[test]
fn unidata_version_matches_the_reference() {
    let instance = Instance::new();
    let namespace = unicodedata_module::build(&instance);
    let version = instance
        .dict_get(namespace, "unidata_version")
        .expect("unidata_version 必须在命名空间里");
    assert_eq!(text_of(&instance, version), "16.0.0");
}

#[test]
fn table_backed_functions_are_real() {
    let instance = Instance::new();
    let namespace = unicodedata_module::build(&instance);

    // category：三档代表值（Lu/Lo/Cs）＋未分配默认 Cn。
    for (character, expected) in [("A", "Lu"), ("中", "Lo"), ("\u{10FFFF}", "Cn")] {
        let argument = instance.new_str(character);
        let handler = native(&instance, namespace, "category");
        let value = call(&instance, handler, &[argument], &[]).expect("category 不应失败");
        assert_eq!(text_of(&instance, value), expected, "category({character:?})");
    }

    // combining：组合符 230、普通字母 0。
    for (character, expected) in [("\u{0301}", 230), ("A", 0)] {
        let argument = instance.new_str(character);
        let handler = native(&instance, namespace, "combining");
        let value = call(&instance, handler, &[argument], &[]).expect("combining 不应失败");
        assert_eq!(int_of(&instance, value), expected, "combining({character:?})");
    }

    // east_asian_width：Na/W/N 三档。
    for (character, expected) in [("A", "Na"), ("中", "W"), ("\u{0000}", "N")] {
        let argument = instance.new_str(character);
        let handler = native(&instance, namespace, "east_asian_width");
        let value = call(&instance, handler, &[argument], &[]).expect("east_asian_width 不应失败");
        assert_eq!(text_of(&instance, value), expected, "east_asian_width({character:?})");
    }

    // bidirectional：L/BN/未分配空串。
    for (character, expected) in [("A", "L"), ("\u{0000}", "BN"), ("\u{10FFFF}", "")] {
        let argument = instance.new_str(character);
        let handler = native(&instance, namespace, "bidirectional");
        let value = call(&instance, handler, &[argument], &[]).expect("bidirectional 不应失败");
        assert_eq!(text_of(&instance, value), expected, "bidirectional({character:?})");
    }

    // decimal／digit／numeric：数字族的三档（含"有 digit 没 decimal"的上标）。
    let two = instance.new_str("2");
    let handler = native(&instance, namespace, "decimal");
    assert_eq!(int_of(&instance, call(&instance, handler, &[two], &[]).expect("decimal")), 2);
    let superscript_two = instance.new_str("\u{00B2}");
    let handler = native(&instance, namespace, "digit");
    assert_eq!(
        int_of(&instance, call(&instance, handler, &[superscript_two], &[]).expect("digit")),
        2
    );
    let half = instance.new_str("\u{00BD}");
    let handler = native(&instance, namespace, "numeric");
    let numeric = call(&instance, handler, &[half], &[]).expect("numeric");
    assert_eq!(float_of(&instance, numeric), 0.5);
    // 参照口径：numeric **恒返回 float**（`numeric('2')` ⇒ 2.0）。
    let handler = native(&instance, namespace, "numeric");
    let numeric_integral = call(&instance, handler, &[two], &[]).expect("numeric('2')");
    assert_eq!(float_of(&instance, numeric_integral), 2.0);
    assert_eq!(instance.type_name(instance.type_of(numeric_integral)), "float");

    // decomposition：有分解 ⇒ 十六进制序列；无分解 ⇒ 空串。
    let angstrom = instance.new_str("Å");
    let handler = native(&instance, namespace, "decomposition");
    assert_eq!(
        text_of(&instance, call(&instance, handler, &[angstrom], &[]).expect("decomposition")),
        "0041 030A"
    );
    let plain = instance.new_str("A");
    let handler = native(&instance, namespace, "decomposition");
    assert_eq!(
        text_of(&instance, call(&instance, handler, &[plain], &[]).expect("decomposition")),
        ""
    );
}

#[test]
fn value_errors_and_argument_messages_match_the_reference() {
    let instance = Instance::new();
    let namespace = unicodedata_module::build(&instance);

    // 无值的 ValueError 文案（照参照逐字）。
    let letter = instance.new_str("a");
    for (function, expected) in [
        ("decimal", "ValueError: not a decimal"),
        ("digit", "ValueError: not a digit"),
        ("numeric", "ValueError: not a numeric character"),
    ] {
        let handler = native(&instance, namespace, function);
        let error = call(&instance, handler, &[letter], &[]).expect_err("无值必须报错");
        assert_eq!(error_text(&instance, error), expected, "{function}");
    }

    // 实参口径（照参照 3.14.4 实测文案）。
    let two_characters = instance.new_str("ab");
    let handler = native(&instance, namespace, "category");
    let error = call(&instance, handler, &[two_characters], &[]).expect_err("长度 2 必须报错");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: category(): argument must be a unicode character, not a string of length 2"
    );

    let number = instance.new_int(65);
    let handler = native(&instance, namespace, "category");
    let error = call(&instance, handler, &[number], &[]).expect_err("整数必须报错（参照只收单字符 str）");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: category() argument must be a unicode character, not int"
    );

    let handler = native(&instance, namespace, "combining");
    let error = call(&instance, handler, &[], &[]).expect_err("缺实参必须报错");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: unicodedata.combining() takes exactly one argument (0 given)"
    );

    let argument = instance.new_str("A");
    let key = instance.new_str("chr");
    let handler = native(&instance, namespace, "east_asian_width");
    let error = call(&instance, handler, &[], &[(key, argument)]).expect_err("关键字必须报错");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: unicodedata.east_asian_width() takes no keyword arguments"
    );
}

#[test]
fn unimplemented_faces_are_honest() {
    let instance = Instance::new();
    let namespace = unicodedata_module::build(&instance);
    let argument = instance.new_str("A");
    for (function, marker) in [
        ("name", "unicodedata.name"),
        ("lookup", "unicodedata.lookup"),
        ("normalize", "unicodedata.normalize"),
        ("is_normalized", "unicodedata.is_normalized"),
        ("mirrored", "unicodedata.mirrored"),
    ] {
        let handler = native(&instance, namespace, function);
        let error = call(&instance, handler, &[argument], &[]).expect_err("未实现的面必须如实报");
        let text = error_text(&instance, error);
        assert!(text.starts_with("NotImplementedError: "), "{function}: {text}");
        assert!(text.contains(marker), "{function}: {text}");
    }
}
