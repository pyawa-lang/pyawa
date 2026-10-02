//! `builtins` 纯计算面的契约测试（`§5.2.2`）。
//!
//! 每条断言对着"参照实现的行为"（数值与消息），期望值都是从本机 CPython 3.14.4 探出来的。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};
use pyawa_stdlib::builtins_module;

/// 取一个内建函数，并按位置参数调用它（直接走原生调用通道）。
fn call(instance: &Instance, name: &str, args: &[NonNull<Header>]) -> Result<NonNull<Header>, ExecError> {
    let namespace = builtins_module::build(instance);
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("{name} 应当在 builtins 里"));
    // SAFETY: function 是本实例里存活的可调用对象；实参都是新引用，归调用方。
    unsafe {
        let raw = function.as_ptr().cast::<pyawa_core::BuiltinFunctionObject>();
        let handler = (*raw).function();
        handler(instance, None, args, &[])
    }
}

fn int_of(instance: &Instance, object: NonNull<Header>) -> i64 {
    instance.int_value(object).expect("应当是整数")
}

fn text_of(instance: &Instance, object: NonNull<Header>) -> String {
    instance.text_value(object).expect("应当是字符串")
}

fn bool_of(instance: &Instance, object: NonNull<Header>) -> bool {
    // `True`／`False` 的载荷
    instance.int_value(object).expect("布尔也是整数") != 0
}

/// 调用并把结果释放（返回错误消息）。
fn call_expecting_error(instance: &Instance, name: &str, args: &[NonNull<Header>]) -> String {
    match call(instance, name, args) {
        Ok(value) => panic!("{name} 应当报错，实际得到 {value:?}"),
        Err(ExecError::Raised { exception }) => {
            // SAFETY: exception 是存活对象。
            unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(instance)
                .unwrap_or_default()
        }
        Err(other) => panic!("{name} 报了非脚本异常：{other:?}"),
    }
}

#[test]
fn abs_follows_the_reference() {
    let instance = Instance::new();
    // abs(-3) ⇒ 3（int）
    let three = instance.new_int(-3);
    let result = call(&instance, "abs", &[three]).expect("abs(-3)");
    assert_eq!(int_of(&instance, result), 3);
    // abs(True) ⇒ 1，且**是 int 不是 bool**（实测）
    let flag = instance.new_bool(true);
    let result = call(&instance, "abs", &[flag]).expect("abs(True)");
    assert_eq!(int_of(&instance, result), 1);
    assert!(
        instance.type_of(result) == instance.singletons().int_type(),
        "abs(True) 的结果是 int（实测）"
    );
    // abs(-3.5) ⇒ 3.5
    let number = instance.new_float(-3.5);
    let result = call(&instance, "abs", &[number]).expect("abs(-3.5)");
    assert_eq!(instance.float_value(result), Some(3.5));
    // abs("x") ⇒ TypeError: bad operand type for abs(): 'str'
    let text = instance.new_str("x");
    assert_eq!(
        call_expecting_error(&instance, "abs", &[text]),
        "bad operand type for abs(): 'str'"
    );
}

#[test]
fn len_matches_the_reference() {
    let instance = Instance::new();
    let text = instance.new_str("abc");
    let result = call(&instance, "len", &[text]).expect("len(\"abc\")");
    assert_eq!(int_of(&instance, result), 3);
    let number = instance.new_int(5);
    assert_eq!(
        call_expecting_error(&instance, "len", &[number]),
        "object of type 'int' has no len()"
    );
}

#[test]
fn ord_and_chr_round_trip() {
    let instance = Instance::new();
    let upper = instance.new_str("A");
    let code = call(&instance, "ord", &[upper]).expect("ord(\"A\")");
    assert_eq!(int_of(&instance, code), 65);
    let back = call(&instance, "chr", &[instance.new_int(65)]).expect("chr(65)");
    assert_eq!(text_of(&instance, back), "A");
    // ord("ab") 的消息带**字符数**
    let two = instance.new_str("ab");
    assert_eq!(
        call_expecting_error(&instance, "ord", &[two]),
        "ord() expected a character, but string of length 2 found"
    );
    // chr(-1) ⇒ ValueError
    assert_eq!(
        call_expecting_error(&instance, "chr", &[instance.new_int(-1)]),
        "chr() arg not in range(0x110000)"
    );
    // chr(1.0) ⇒ TypeError
    assert_eq!(
        call_expecting_error(&instance, "chr", &[instance.new_float(1.0)]),
        "'float' object cannot be interpreted as an integer"
    );
}

#[test]
fn radix_helpers_match_the_reference() {
    let instance = Instance::new();
    let neg = instance.new_int(-5);
    let result = call(&instance, "bin", &[neg]).expect("bin(-5)");
    assert_eq!(text_of(&instance, result), "-0b101");
    let flag = instance.new_bool(true);
    let result = call(&instance, "bin", &[flag]).expect("bin(True)");
    assert_eq!(text_of(&instance, result), "0b1");
    let eight = instance.new_int(8);
    let result = call(&instance, "oct", &[eight]).expect("oct(8)");
    assert_eq!(text_of(&instance, result), "0o10");
    let number = instance.new_float(1.5);
    assert_eq!(
        call_expecting_error(&instance, "hex", &[number]),
        "'float' object cannot be interpreted as an integer"
    );
}

#[test]
fn callable_isinstance_and_issubclass() {
    let instance = Instance::new();
    let namespace = builtins_module::build(&instance);
    let len_fn = instance.dict_get(namespace, "len").expect("len 在 builtins 里");
    let result = call(&instance, "callable", &[len_fn]).expect("callable(len)");
    assert!(bool_of(&instance, result));
    let one = instance.new_int(1);
    let result = call(&instance, "callable", &[one]).expect("callable(1)");
    assert!(!bool_of(&instance, result));

    // isinstance(1, int) ⇒ True；isinstance(True, int) ⇒ True（bool 是 int 子类）
    let int_type = instance.type_value(instance.type_named("int").expect("int 已登记"));
    let result = call(&instance, "isinstance", &[one, int_type]).expect("isinstance(1, int)");
    assert!(bool_of(&instance, result));
    let truthy = instance.new_bool(true);
    let result = call(&instance, "isinstance", &[truthy, int_type]).expect("isinstance(True, int)");
    assert!(bool_of(&instance, result), "bool 是 int 的子类（实测）");

    // isinstance(1, (int, str)) ⇒ True
    let str_type = instance.type_value(instance.type_named("str").expect("str 已登记"));
    let pair = instance.new_tuple(vec![int_type, str_type]);
    let result = call(&instance, "isinstance", &[one, pair]).expect("isinstance(1, (int, str))");
    assert!(bool_of(&instance, result));

    // isinstance(1, 5) ⇒ TypeError（实测消息）
    let five = instance.new_int(5);
    assert_eq!(
        call_expecting_error(&instance, "isinstance", &[one, five]),
        "isinstance() arg 2 must be a type, a tuple of types, or a union"
    );

    // issubclass(bool, int) ⇒ True；issubclass(int, bool) ⇒ False
    let bool_type = instance.type_value(instance.type_named("bool").expect("bool 已登记"));
    let result = call(&instance, "issubclass", &[bool_type, int_type]).expect("issubclass(bool, int)");
    assert!(bool_of(&instance, result));
    let result = call(&instance, "issubclass", &[int_type, bool_type]).expect("issubclass(int, bool)");
    assert!(!bool_of(&instance, result));

    // issubclass(1, int) ⇒ TypeError
    assert_eq!(
        call_expecting_error(&instance, "issubclass", &[one, int_type]),
        "issubclass() arg 1 must be a class"
    );
}

#[test]
fn repr_uses_the_repr_channel() {
    let instance = Instance::new();
    let text = instance.new_str("a");
    let result = call(&instance, "repr", &[text]).expect("repr('a')");
    assert_eq!(text_of(&instance, result), "'a'");
}


// ---- §6 的对拍夹具：`tools/gen_builtins_fixture.py` 导出的 19 个用例（Rust 源码，免解析）----

#[path = "fixtures/builtins.rs"]
mod fixture;

/// 按关键字实参调用（本夹具要 `key=`／`default=`／`reverse=`）。
fn call_with(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let namespace = builtins_module::build(instance);
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("{name} 应当在 builtins 里"));
    // SAFETY: function 是本实例里存活的可调用对象；实参都是新引用，归调用方。
    unsafe {
        let raw = function.as_ptr().cast::<pyawa_core::BuiltinFunctionObject>();
        (*raw).function()(instance, None, args, kwargs)
    }
}

/// 照夹具里的值造对象（新引用）。
fn build_fixture_value(instance: &Instance, value: fixture::Value) -> NonNull<Header> {
    match value {
        fixture::Value::Int(number) => instance.new_int(number),
        fixture::Value::Str(text) => instance.new_str(text),
        fixture::Value::List(items) => instance.new_list(
            items
                .iter()
                .map(|item| build_fixture_value(instance, *item))
                .collect(),
        ),
        fixture::Value::Tuple(items) => instance.new_tuple(
            items
                .iter()
                .map(|item| build_fixture_value(instance, *item))
                .collect(),
        ),
    }
}

#[test]
fn min_max_sorted_match_the_reference_fixture() {
    let instance = Instance::new();
    let mut checked = 0usize;
    for case in fixture::CASES {
        let args: Vec<NonNull<Header>> = case
            .args
            .iter()
            .map(|value| build_fixture_value(&instance, *value))
            .collect();
        let mut kwargs: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::new();
        if let Some(key) = case.key {
            // `key=` 只允许写成内建函数名 ⇒ 从 `builtins` 里取同名原生
            kwargs.push((instance.new_str("key"), call_lookup(&instance, key)));
        }
        if let Some(reverse) = case.reverse {
            kwargs.push((instance.new_str("reverse"), instance.new_bool(reverse)));
        }
        if let Some(default) = case.default {
            kwargs.push((
                instance.new_str("default"),
                build_fixture_value(&instance, default),
            ));
        }
        let observed = call_with(&instance, case.call, &args, &kwargs);
        match case.repr {
            Some(expected) => {
                let raw = observed.unwrap_or_else(|error| {
                    panic!("{} 应当成功，却报了 {error:?}", case.name)
                });
                assert_eq!(
                    instance.object_repr(raw),
                    expected,
                    "{} 的结果（参照夹具）",
                    case.name
                );
            }
            None => {
                assert!(observed.is_err(), "{} 应当报错", case.name);
                let raw = instance.pending_exception().expect("应当有异常");
                assert_eq!(
                    instance.type_name(instance.type_of(raw)),
                    case.error.expect("夹具里应当有错误类型"),
                    "{} 的异常类型",
                    case.name
                );
                assert_eq!(
                    Some(instance.object_str(raw)),
                    case.message.map(|text| text.to_owned()),
                    "{} 的异常消息",
                    case.name
                );
            }
        }
        checked += 1;
    }
    assert!(checked >= 15, "对拍的用例要够多，实际 {checked} 条");
}

/// 从 `builtins` 里取一个同名原生（夹具里的 `key=` 用）。
fn call_lookup(instance: &Instance, name: &str) -> NonNull<Header> {
    let namespace = builtins_module::build(instance);
    instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("{name} 应当在 builtins 里"))
}
