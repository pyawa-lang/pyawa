//! `FORMAT_WITH_SPEC`（`f'{x:>5}'`）与 `__format__` 的默认实现。
//!
//! 实测的形状与消息（本机 3.14.4）：
//!
//! ```text
//! format(42, '>5')       = '   42'          format(42, '05')   = '00042'
//! format(42, '<5')       = '42   '          format(42, '=+8')  = '+     42'
//! format(42, 'x')        = '2a'             format(42, '#x')   = '0x2a'
//! format(42, '_b')       = '10_1010'        format(42, 'c')    = '*'
//! format(42, '.2f')      = '42.00'          format(42, 'z')    ⇒ ValueError（负零强制的原话）
//! format(3.14159, '.2f') = '3.14'           format(3.14159, 'e') = '3.141590e+00'
//! format(3.14159, '%')   = '314.159000%'    format(3.0, 'g')   = '3'
//! format('ab', '>5')     = '   ab'          format('ab', '.1') = 'a'
//! format('ab', '5')      = 'ab   '          format('ab', 'd')  ⇒ ValueError: Unknown format code 'd' for object of type 'str'
//! format(True, 'd')      = '1'              format(True, '')   = 'True'
//! format(None, '')       = 'None'           format(None, 'd')  ⇒ TypeError: unsupported format string passed to NoneType.__format__
//! ```
//!
//! 栈契约（实测）：`LOAD 值; LOAD_CONST '规格'; FORMAT_WITH_SPEC`，净 **−1**。

mod common;

use core::cell::RefCell;

use pyawa_core::{StrObject, Value};

use common::{emit, op, Vm};

fn text_of(result: &Value<'_>, vm: &Vm) -> String {
    let raw = result.as_header(&vm.instance).expect("应当是 str");
    // SAFETY: 调用方保证这是 str。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned()
}

/// 跑一小段：把 `<值>` 按 `<规格>` 格式化后返回。
fn format_of(vm: &Vm, value: core::ptr::NonNull<pyawa_core::Header>, spec: &str) -> Result<String, pyawa_core::ExecError> {
    let spec_object = vm.instance.new_str(spec);
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("FORMAT_WITH_SPEC"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(value), Some(spec_object)],
    );
    vm.run(&code).map(|result| text_of(&result, vm))
}

#[test]
fn integer_specs_match_the_reference() {
    let vm = Vm::new();
    for (spec, expected) in [
        (">5", "   42"),
        ("<5", "42   "),
        ("^5", " 42  "),
        ("05", "00042"),
        ("+d", "+42"),
        (" d", " 42"),
        ("x", "2a"),
        ("X", "2A"),
        ("o", "52"),
        ("b", "101010"),
        ("#x", "0x2a"),
        ("=+8", "+     42"),
        ("_b", "10_1010"),
        ("c", "*"),
        (".2f", "42.00"),
        ("e", "4.200000e+01"),
        ("", "42"),
    ] {
        assert_eq!(
            format_of(&vm, vm.constant(42), spec).unwrap(),
            expected,
            "format(42, {spec:?})"
        );
    }
    // `z`（负零强制）：参照实现在整数上有专门报错，原话照抄
    let error = format_of(&vm, vm.constant(42), "z").unwrap_err();
    assert!(matches!(error, pyawa_core::ExecError::Raised { .. }));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "ValueError".to_owned(),
            Some("Negative zero coercion (z) not allowed in integer format specifier".to_owned())
        ))
    );
}

#[test]
fn float_and_string_specs_match_the_reference() {
    let vm = Vm::new();
    let float_type = vm.instance.type_named("float").unwrap();
    let value = |number: f64| {
        vm.instance
            .alloc(pyawa_core::FloatObject::new(float_type, number))
            .into_raw()
            .cast::<pyawa_core::Header>()
    };
    for (number, spec, expected) in [
        (3.14159, ".2f", "3.14"),
        (3.14159, "8.3f", "   3.142"),
        (3.14159, ">10.2f", "      3.14"),
        (3.14159, "e", "3.141590e+00"),
        (3.14159, "%", "314.159000%"),
        (3.0, "g", "3"),
    ] {
        assert_eq!(
            format_of(&vm, value(number), spec).unwrap(),
            expected,
            "format({number}, {spec:?})"
        );
    }

    // 字符串：默认左对齐、`.N` 截断
    for (text, spec, expected) in [
        ("ab", ">5", "   ab"),
        ("ab", "^6", "  ab  "),
        ("ab", "5", "ab   "),
        ("ab", ".1", "a"),
        ("ab", "", "ab"),
    ] {
        assert_eq!(
            format_of(&vm, vm.instance.new_str(text), spec).unwrap(),
            expected,
            "format({text:?}, {spec:?})"
        );
    }
}

#[test]
fn bool_and_none_follow_the_reference() {
    let vm = Vm::new();
    // 注意：常量表**持有**引用，所以每次调用都要各自取一份（同一份交给两张表会双重释放）
    let true_ref = |vm: &Vm| vm.instance.own(vm.instance.singletons().boolean(true)).into_raw();
    assert_eq!(
        format_of(&vm, true_ref(&vm), "d").unwrap(),
        "1",
        "bool 带类型码时按整数"
    );
    assert_eq!(
        format_of(&vm, true_ref(&vm), "").unwrap(),
        "True",
        "没有类型码时是 True"
    );
    let none_ref = vm.instance.own(vm.instance.singletons().none()).into_raw();
    assert_eq!(format_of(&vm, none_ref, "").unwrap(), "None");
    // `format(None, 'd')`：实测消息
    let error = format_of(
        &vm,
        vm.instance.own(vm.instance.singletons().none()).into_raw(),
        "d",
    )
    .unwrap_err();
    assert!(matches!(error, pyawa_core::ExecError::Raised { .. }));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "TypeError".to_owned(),
            Some("unsupported format string passed to NoneType.__format__".to_owned())
        ))
    );
}

#[test]
fn unknown_code_reports_the_reference_message() {
    let vm = Vm::new();
    let error = format_of(&vm, vm.instance.new_str("ab"), "d").unwrap_err();
    assert!(matches!(error, pyawa_core::ExecError::Raised { .. }));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "ValueError".to_owned(),
            Some("Unknown format code 'd' for object of type 'str'".to_owned())
        ))
    );
}

#[test]
fn a_python_level_format_override_wins() {
    // 路线 ①：类型字典里的 `__format__` 优先（用户类覆写走这条）
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    // def __format__(self, spec): return "覆盖了"
    let code = vm.code_with_names(
        4,
        2,
        2,
        vec!["self".to_owned(), "spec".to_owned()],
        Vec::new(),
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.instance.new_str("覆盖了"))],
    );
    let function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        code.into_raw().cast::<pyawa_core::Header>(),
        Vec::new(),
        None,
    
    RefCell::new(None),));
    vm.instance
        .set_type_attribute(ty, "__format__", function.into_raw().cast::<pyawa_core::Header>());
    let object = vm
        .instance
        .alloc(pyawa_core::AttributeObject::new(ty, RefCell::new(None)));

    let result = format_of(&vm, object.into_raw().cast::<pyawa_core::Header>(), ">5").unwrap();
    assert_eq!(result, "覆盖了", "Python 级的 __format__ 覆盖了默认实现");
}

#[test]
fn type_dict_natives_survive_a_collection() {
    // 风险点：`__format__` 的原生可调用对象挂在**类型字典**里，而类型对象由注册表持有
    // （不在 `live` 账本里）——若回收把类型字典当垃圾收掉，`format()` 会在一次 GC 之后失效。
    let vm = Vm::new();
    vm.instance.collect();
    assert_eq!(
        format_of(&vm, vm.constant(42), ">5").unwrap(),
        "   42",
        "回收之后 `int.__format__` 仍然在"
    );
    assert_eq!(
        format_of(&vm, vm.instance.new_str("ab"), "").unwrap(),
        "ab",
        "回收之后 `str.__format__` 仍然在"
    );
}
