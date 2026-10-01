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
