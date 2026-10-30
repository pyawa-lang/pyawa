//! **`complex` 对象类型**（`P3-34`，task-30）的验收：表示／属性／算术／比较／真值／哈希／
//! 类型面。期望值全部是 **参照 3.14.4 实测**（python3 同脚本逐行对拍；探针
//! `target/recon/task30_probe.py`），断言直接钉死文本与消息。
//!
//! 与 task-29／task-31 的分工：字面量（`1j`）归 emitter（super-fix），`abs()`／`hash()`／
//! `int()`／`float()` 四个**内置入口**归 stdlib（sys-exit）——本文件测的是**核心对象语义**
//! （`object_repr`／`arithmetic_public`／`values_equal_public`／`unary_public`／
//! `truthiness_public` 与 `hash_*` 算法）✓。

mod common;

use core::ptr::NonNull;

use pyawa_core::bigint::BigInt;
use pyawa_core::executor::{
    arithmetic_public, truthiness_public, unary_public, values_equal_public,
    hash_bigint_value, hash_complex_parts, hash_double_value, hash_integer_value,
};
use pyawa_core::{ExceptionObject, ExecError, Header, Instance};

fn complex(vm: &common::Vm, real: f64, imag: f64) -> NonNull<Header> {
    vm.instance.new_complex(real, imag)
}

fn repr_of(vm: &common::Vm, value: NonNull<Header>) -> String {
    vm.instance.object_repr(value).expect("repr 应当成功")
}

fn str_of(vm: &common::Vm, value: NonNull<Header>) -> String {
    vm.instance.object_str(value).expect("str 应当成功")
}

/// 一个异常对象的（类型名, 消息）。
fn describe(instance: &Instance, exception: NonNull<Header>) -> (String, String) {
    // SAFETY: exception 由实例保活。
    let ty = unsafe { exception.as_ref() }.ty();
    // SAFETY: 同上。
    let type_name = unsafe { ty.as_ref() }.name().to_owned();
    // SAFETY: 同上。
    let message = unsafe { &*exception.as_ptr().cast::<ExceptionObject>() }
        .message_with(instance)
        .unwrap_or_default();
    (type_name, message)
}

/// 一次调用/运算应当抛出的（类型名, 消息）。
fn raised(
    instance: &Instance,
    outcome: Result<NonNull<Header>, ExecError>,
) -> (String, String) {
    match outcome {
        Err(ExecError::Raised { exception }) => describe(instance, exception),
        other => panic!("应当抛异常，实际 {other:?}"),
    }
}

/// 一次比较应当抛出的（类型名, 消息）。
fn compare_raised(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
) -> (String, String) {
    match pyawa_core::executor::compare_public(instance, left, right, symbol, 0) {
        Err(ExecError::Raised { exception }) => describe(instance, exception),
        Ok(flag) => panic!("`{symbol}` 对复数应当抛 TypeError，实际给 {flag}"),
        Err(other) => panic!("`{symbol}` 应当抛 Raised，实际 {other:?}"),
    }
}

/// **属性/方法**：`attribute_read` 取出的东西（借一份新引用）。
fn attr(vm: &common::Vm, value: NonNull<Header>, name: &str) -> NonNull<Header> {
    pyawa_core::attribute_read(&vm.instance, value, name)
        .unwrap_or_else(|error| panic!("取 {name} 失败：{error:?}"))
}

#[test]
fn representation_matches_the_reference() {
    // `repr`／`str` **同一口径**；纯虚、零的符号位、inf/nan、指数形态都在表里 ✓。
    let vm = common::Vm::new();
    let cases: &[(f64, f64, &str)] = &[
        (0.0, 1.0, "1j"),
        (0.0, -1.0, "-1j"),
        (0.0, 0.0, "0j"),
        (0.0, -0.0, "-0j"),
        (-0.0, 0.0, "(-0+0j)"),
        (-0.0, -0.0, "(-0-0j)"),
        (0.0, 1.5, "1.5j"),
        (0.0, f64::INFINITY, "infj"),
        (0.0, f64::NEG_INFINITY, "-infj"),
        (1.0, 1.0, "(1+1j)"),
        (1.0, -1.0, "(1-1j)"),
        (-1.0, 1.0, "(-1+1j)"),
        (1.5, -2.5, "(1.5-2.5j)"),
        (1.0, 0.0, "(1+0j)"),
        (1.0, -0.0, "(1-0j)"),
        (-1.0, 0.0, "(-1+0j)"),
        (-1.0, -0.0, "(-1-0j)"),
        (1e300, 0.0, "(1e+300+0j)"),
        (1e300, 1e-300, "(1e+300+1e-300j)"),
        (1e16, 1e16, "(1e+16+1e+16j)"),
        (1e-16, 1e-16, "(1e-16+1e-16j)"),
        (f64::INFINITY, 0.0, "(inf+0j)"),
        (f64::NEG_INFINITY, 0.0, "(-inf+0j)"),
        (f64::INFINITY, f64::NAN, "(inf+nanj)"),
        (f64::NAN, f64::INFINITY, "(nan+infj)"),
        (3.0, 4.0, "(3+4j)"),
        (0.1, 0.2, "(0.1+0.2j)"),
        (-0.0, 1.0, "(-0+1j)"),
    ];
    for (real, imag, expected) in cases {
        let value = complex(&vm, *real, *imag);
        assert_eq!(repr_of(&vm, value), *expected, "repr(complex({real:?}, {imag:?}))");
        assert_eq!(str_of(&vm, value), *expected, "str(complex({real:?}, {imag:?}))");
    }
    // `nan` 虚部的纯虚形态（参照实测 `nanj` ✓）
    let nan_imag = complex(&vm, 0.0, f64::NAN);
    assert_eq!(repr_of(&vm, nan_imag), "nanj");
}

#[test]
fn attributes_and_methods_match_the_reference() {
    let vm = common::Vm::new();
    let z = complex(&vm, 3.0, 4.0);
    // `.real`／`.imag` 是 **float** ✓
    let real = attr(&vm, z, "real");
    let imag = attr(&vm, z, "imag");
    assert_eq!(vm.instance.float_value(real), Some(3.0));
    assert_eq!(vm.instance.float_value(imag), Some(4.0));
    assert_eq!(vm.instance.type_name(vm.instance.type_of(real)), "float");
    // `conjugate()` ⇒ 新复数 ✓
    let conjugate = attr(&vm, z, "conjugate");
    let result = pyawa_core::call_value(&vm.instance, conjugate, &[], &[]).expect("conjugate()");
    assert_eq!(repr_of(&vm, result), "(3-4j)");
    // `__complex__()` ⇒ **自身** ✓（`is` 为真 ✓）
    let dunder = attr(&vm, z, "__complex__");
    let same = pyawa_core::call_value(&vm.instance, dunder, &[], &[]).expect("__complex__()");
    assert_eq!(same, z, "__complex__() 应当返回自身");
    // `__abs__()` ⇒ float 5.0 ✓
    let abs_method = attr(&vm, z, "__abs__");
    let magnitude = pyawa_core::call_value(&vm.instance, abs_method, &[], &[]).expect("__abs__()");
    assert_eq!(vm.instance.float_value(magnitude), Some(5.0));
    assert_eq!(vm.instance.type_name(vm.instance.type_of(magnitude)), "float");
    // `__bool__()` ✓
    let boolean = attr(&vm, z, "__bool__");
    let truth = pyawa_core::call_value(&vm.instance, boolean, &[], &[]).expect("__bool__()");
    assert!(vm.instance.truthiness_of(truth).expect("真值"));
    let zero = complex(&vm, 0.0, 0.0);
    let false_method = attr(&vm, zero, "__bool__");
    let falsy = pyawa_core::call_value(&vm.instance, false_method, &[], &[]).expect("__bool__()");
    assert!(!vm.instance.truthiness_of(falsy).expect("真值"));
    // `__hash__()` ⇒ int（算法见 hash 测试）✓
    let hash_method = attr(&vm, complex(&vm, 1.0, 0.0), "__hash__");
    let hashed = pyawa_core::call_value(&vm.instance, hash_method, &[], &[]).expect("__hash__()");
    assert_eq!(vm.instance.int_value(hashed), Some(1));
    // 类型级入口（`complex.conjugate(z)` 等）✓
    let complex_type = vm.instance.type_named("complex").expect("complex 已登记");
    assert_eq!(vm.instance.type_name(complex_type), "complex");
    let namespace = vm.instance.type_namespace(complex_type.cast()).expect("类型字典");
    for name in ["__complex__", "conjugate", "__hash__", "__abs__", "__bool__"] {
        assert!(
            vm.instance.dict_get(namespace, name).is_some(),
            "complex 的类型字典里应当有 {name}"
        );
    }
}

#[test]
fn arithmetic_matches_the_reference() {
    let vm = common::Vm::new();
    let z = complex(&vm, 1.0, 2.0);
    let w = complex(&vm, 3.0, -4.0);
    let one = vm.instance.new_int(1);
    let two = vm.instance.new_int(2);
    let float_half = vm.instance.new_float(1.5);
    let truth = vm.instance.retain(vm.instance.singletons().boolean(true));

    let cases: &[(NonNull<Header>, NonNull<Header>, &str, &str)] = &[
        (z, w, "+", "(4-2j)"),
        (z, w, "-", "(-2+6j)"),
        (z, w, "*", "(11+2j)"),
        (z, w, "/", "(-0.2+0.4j)"),
        (z, one, "+", "(2+2j)"),
        (one, z, "+", "(2+2j)"),
        (z, float_half, "+", "(2.5+2j)"),
        (z, truth, "+", "(2+2j)"),
        (z, float_half, "*", "(1.5+3j)"),
        (z, one, "-", "2j"),
        (one, z, "-", "-2j"),
        (float_half, z, "/", "(0.3-0.6j)"),
        (z, two, "**", "(-3+4j)"),
        (z, complex(&vm, 0.5, 0.0), "**", "(1.272019649514069+0.7861513777574233j)"),
        (z, complex(&vm, 1.0, 1.0), "**", "(-0.24720004426291722+0.6964504870825432j)"),
        (two, z, "**", "(0.36691394948660344+1.9660554808224875j)"),
        (
            complex(&vm, 0.0, 0.0),
            vm.instance.new_int(0),
            "**",
            "(1+0j)",
        ),
    ];
    for (left, right, symbol, expected) in cases {
        let result = arithmetic_public(&vm.instance, *left, *right, symbol, 0)
            .unwrap_or_else(|error| panic!("{symbol} 应当成功：{error:?}"));
        assert_eq!(repr_of(&vm, result), *expected, "{symbol}");
    }

    // 小整数指数的重复平方法：`z**100` 与参照逐位一致 ✓
    let hundred = vm.instance.new_int(100);
    let power = arithmetic_public(&vm.instance, z, hundred, "**", 0).expect("z**100");
    assert_eq!(
        repr_of(&vm, power),
        "(-6.443164690985892e+34-6.113241307762508e+34j)"
    );
    // 负整数指数：`z**-1` ✓
    let minus_one = vm.instance.new_int(-1);
    let inverse = arithmetic_public(&vm.instance, z, minus_one, "**", 0).expect("z**-1");
    assert_eq!(repr_of(&vm, inverse), "(0.2-0.4j)");

    // `//`／`%` 的 TypeError 文案（参照逐字）✓
    for (symbol, expected) in [
        ("//", "unsupported operand type(s) for //: 'complex' and 'int'"),
        ("%", "unsupported operand type(s) for %: 'complex' and 'int'"),
    ] {
        let (type_name, message) = raised(
            &vm.instance,
            arithmetic_public(&vm.instance, z, two, symbol, 0),
        );
        assert_eq!(type_name, "TypeError");
        assert_eq!(message, expected, "{symbol}");
    }
    let (_, message) = raised(
        &vm.instance,
        arithmetic_public(&vm.instance, two, z, "//", 0),
    );
    assert_eq!(message, "unsupported operand type(s) for //: 'int' and 'complex'");

    // 除零口径 ✓（`z/0`／`z/0j`／`z/0.0` 同一条消息）
    for divisor in [vm.instance.new_int(0), complex(&vm, 0.0, 0.0), vm.instance.new_float(0.0)] {
        let (type_name, message) = raised(
            &vm.instance,
            arithmetic_public(&vm.instance, z, divisor, "/", 0),
        );
        assert_eq!(type_name, "ZeroDivisionError");
        assert_eq!(message, "division by zero");
    }
    // `0j ** 负数或复数` ⇒ 参照专门文案 ✓
    for exponent in [vm.instance.new_int(-1), complex(&vm, 0.0, -1.0)] {
        let (type_name, message) = raised(
            &vm.instance,
            arithmetic_public(&vm.instance, complex(&vm, 0.0, 0.0), exponent, "**", 0),
        );
        assert_eq!(type_name, "ZeroDivisionError");
        assert_eq!(message, "zero to a negative or complex power");
    }
}

#[test]
fn comparisons_match_the_reference() {
    let vm = common::Vm::new();
    let one_zero = complex(&vm, 1.0, 0.0);
    let one_two = complex(&vm, 1.0, 2.0);
    let one = vm.instance.new_int(1);
    let one_float = vm.instance.new_float(1.0);
    let truth = vm.instance.retain(vm.instance.singletons().boolean(true));
    let text = vm.instance.new_str("x");

    // 相等：复数升到复数塔比 ✓
    assert!(values_equal_public(&vm.instance, one_zero, one));
    assert!(values_equal_public(&vm.instance, one_zero, one_float));
    assert!(values_equal_public(&vm.instance, one_zero, truth));
    assert!(values_equal_public(&vm.instance, one_two, complex(&vm, 1.0, 2.0)));
    assert!(!values_equal_public(&vm.instance, one_two, one));
    assert!(!values_equal_public(&vm.instance, one_zero, one_two));
    assert!(!values_equal_public(&vm.instance, one_two, text));

    // 排序：参照的 TypeError 文案（左右类型名都照实）✓
    for (symbol, expected) in [
        ("<", "'<' not supported between instances of 'complex' and 'int'"),
        ("<=", "'<=' not supported between instances of 'complex' and 'int'"),
        (">", "'>' not supported between instances of 'complex' and 'int'"),
        (">=", "'>=' not supported between instances of 'complex' and 'int'"),
    ] {
        let (type_name, message) = compare_raised(&vm.instance, one_two, one, symbol);
        assert_eq!(type_name, "TypeError", "{symbol}");
        assert_eq!(message, expected, "{symbol}");
    }
    let (_, message) = compare_raised(&vm.instance, one, one_two, "<");
    assert_eq!(message, "'<' not supported between instances of 'int' and 'complex'");
    let (_, message) = compare_raised(&vm.instance, one_two, complex(&vm, 1.0, 2.0), "<");
    assert_eq!(
        message,
        "'<' not supported between instances of 'complex' and 'complex'"
    );
}

#[test]
fn truthiness_and_unary_match_the_reference() {
    let vm = common::Vm::new();
    assert!(!truthiness_public(&vm.instance, complex(&vm, 0.0, 0.0), 0).expect("真值"));
    assert!(truthiness_public(&vm.instance, complex(&vm, 0.0, 1.0), 0).expect("真值"));
    assert!(!truthiness_public(&vm.instance, complex(&vm, -0.0, -0.0), 0).expect("真值"));
    // `nan` 为真 ✓（`nan != 0.0`）
    assert!(truthiness_public(&vm.instance, complex(&vm, 0.0, f64::NAN), 0).expect("真值"));

    let z = complex(&vm, 1.0, 2.0);
    let negated = unary_public(&vm.instance, z, "-", 0).expect("-z");
    assert_eq!(repr_of(&vm, negated), "(-1-2j)");
    let positive = unary_public(&vm.instance, z, "+", 0).expect("+z");
    assert_eq!(repr_of(&vm, positive), "(1+2j)");
    // `abs(z)` 的**核心面** ⇒ float（stdlib 的 `abs()` 内置入口调的就是这条语义 ✓）
    let magnitude = unary_public(&vm.instance, complex(&vm, 3.0, 4.0), "abs", 0).expect("abs(z)");
    assert_eq!(vm.instance.float_value(magnitude), Some(5.0));
}

#[test]
fn type_face_and_constructor_match_the_reference() {
    let vm = common::Vm::new();
    let complex_type = vm.instance.type_named("complex").expect("complex 已登记");
    let object_type = vm.instance.type_named("object").expect("object 已登记");
    assert!(
        vm.instance.is_subtype(complex_type, object_type),
        "issubclass(complex, object) 应当为真"
    );
    // `type(<复数>)` 就是 `complex` ✓
    let value = vm.instance.new_complex(0.0, 1.0);
    assert_eq!(vm.instance.type_of(value), complex_type);
    assert_eq!(vm.instance.complex_value(value), Some((0.0, 1.0)));
    assert_eq!(vm.instance.complex_value(vm.instance.new_float(1.0)), None);

    // 构造槽：0/1/2 参、identity、错误文案（字符串解析归 stdlib 的 `__new__` ✓）
    let constructor = vm.instance.type_value(complex_type);
    // SAFETY: 参数由本测试持有；`call_value` 借用实参、自取引用。
    let call = |args: &[NonNull<Header>]| {
        pyawa_core::call_value(&vm.instance, constructor, args, &[])
    };
    assert_eq!(repr_of(&vm, call(&[]).expect("complex()")), "0j");
    let one = vm.instance.new_int(1);
    assert_eq!(repr_of(&vm, call(&[one]).expect("complex(1)")), "(1+0j)");
    let two = vm.instance.new_int(2);
    assert_eq!(repr_of(&vm, call(&[one, two]).expect("complex(1, 2)")), "(1+2j)");
    let existing = vm.instance.new_complex(1.0, 2.0);
    let same = call(&[existing]).expect("complex(z)");
    assert_eq!(same, existing, "complex(z) 应当返回原对象（`is` 为真 ✓）");
    // 3 参错误文案 ✓
    let three = vm.instance.new_int(3);
    let (type_name, message) = raised(&vm.instance, call(&[one, two, three]));
    assert_eq!(type_name, "TypeError");
    assert_eq!(message, "complex() takes at most 2 arguments (3 given)");
    // 非数字 ⇒ 参照那条（**类型名不带引号** ✓）
    let none = vm.instance.retain(vm.instance.singletons().none());
    let (_, message) = raised(&vm.instance, call(&[none]));
    assert_eq!(
        message,
        "complex() argument must be a string or a number, not NoneType"
    );
    // 大整数溢出 ✓（2**2000 超出 double）
    let huge_text = {
        let mut digits = String::from("1");
        for _ in 0..2000 {
            digits.push('0');
        }
        digits
    };
    let huge = vm
        .instance
        .new_int_value(pyawa_core::bigint::IntValue::from_big(
            BigInt::from_decimal(&huge_text).expect("十进制解析"),
        ));
    let (type_name, message) = raised(&vm.instance, call(&[huge]));
    assert_eq!(type_name, "OverflowError");
    assert_eq!(message, "int too large to convert to float");
    // 2 参里带复数 ⇒ 参照的**历史口径** `real + imag*1j` ✓
    let complex_arg = vm.instance.new_complex(1.0, 2.0);
    let five = vm.instance.new_int(5);
    assert_eq!(
        repr_of(&vm, call(&[complex_arg, five]).expect("complex(1+2j, 5)")),
        "(1+7j)"
    );
}

#[test]
fn hash_algorithm_matches_the_reference() {
    // `_Py_HashDouble` 的实测锚点（python3 3.14.4）✓
    for (value, expected) in [
        (1.0, 1),
        (0.5, 1_152_921_504_606_846_976),
        (-1.0, -2),
        (1.5, 1_152_921_504_606_846_977),
        (0.1, 230_584_300_921_369_408),
        (-2.5, -1_152_921_504_606_846_978),
        (1e300, 1_224_995_262_755_759_164),
        (1e-300, 482_449_582_752_280_463),
        (f64::INFINITY, 314_159),
        (f64::NEG_INFINITY, -314_159),
    ] {
        assert_eq!(hash_double_value(value), expected, "hash({value:?})");
    }
    // 整数口径（`hash(n) == n`，`-1` ⇒ `-2`）✓
    for (value, expected) in [(0, 0), (1, 1), (-1, -2), (2, 2)] {
        assert_eq!(hash_integer_value(value), expected, "hash({value})");
    }
    // 大整数：`|n| mod (2**61-1)` ✓（实测：2**61 ⇒ 1、2**62 ⇒ 2、2**61-1 ⇒ 0、2**200 ⇒ 131072）
    for (text, expected) in [
        ("2305843009213693952", 1),
        ("4611686018427387904", 2),
        ("2305843009213693951", 0),
        ("1606938044258990275541962092341162602522202993782792835301376", 131_072),
    ] {
        let value = BigInt::from_decimal(text).expect("十进制解析");
        assert_eq!(hash_bigint_value(&value), expected, "hash({text})");
    }
    // 复数：`hash(real) + 1000003 * hash(imag)` ✓
    for ((real, imag), expected) in [
        ((0.0, 0.0), 0),
        ((1.0, 0.0), 1),
        ((-1.0, 0.0), -2),
        ((1.5, 0.0), 1_152_921_504_606_846_977),
        ((0.0, 1.0), 1_000_003),
        ((1.0, 2.0), 2_000_007),
        ((f64::INFINITY, 0.0), 314_159),
        ((1e300, 1e-300), -3_889_407_508_137_364_311),
        ((0.1, 0.2), 1_614_090_106_475_185_856),
    ] {
        assert_eq!(
            hash_complex_parts(real, imag),
            expected,
            "hash(complex({real:?}, {imag:?}))"
        );
    }
}
