//! **任意精度整数**（`TS-45`／`P1-11`）的验收：核心算法 ＋ **对象模型接线**。
//!
//! 期望值全部来自 `tests/fixture-int-3.14.json`（`tools/gen_int_fixture.py` 在参照实现上
//! **现场实测**导出，含 `TS-45` 点名的 `hash(2**100)` 与 4300 位上限那几条）。
//! 编译器的 `*`／`//`／`%`／`**` 尚未接线（`P1-10` 的缺口）⇒ 接线那一层直接调执行器的公开入口。

mod common;

use core::cmp::Ordering;
use core::ptr::NonNull;

use pyawa_core::bigint::{BigInt, IntValue};
use pyawa_core::executor::{arithmetic_public, compare_public, truthiness_public, unary_public, values_equal_public};
use pyawa_core::Header;

fn fixture() -> common::Json {
    common::parse(include_str!("fixture-int-3.14.json"))
}

/// 造一个 `int` 对象（任意精度载荷）。
fn object(vm: &common::Vm, text: &str) -> NonNull<Header> {
    vm.instance.new_int_value(IntValue::from_big(from(text)))
}

/// 读回十进制文本（**大小整数同一套**）。
fn decimal(vm: &common::Vm, value: NonNull<Header>) -> String {
    vm.instance.int_of(value).expect("应当是 int").to_decimal()
}

fn from(text: &str) -> BigInt {
    BigInt::from_decimal(text).unwrap_or_else(|| panic!("夹具里的整数必须能解析：{text}"))
}

#[test]
fn arithmetic_matches_the_reference() {
    let fixture = fixture();
    let rows = fixture.key("arithmetic").as_arr();
    let mut checked = 0;
    for row in rows {
        let op = row.key("op").as_str();
        let left = from(row.key("a").as_str());
        let right = from(row.key("b").as_str());
        let expected = row.key("result").as_str();
        let actual = match op {
            "add" => left.add(&right),
            "sub" => left.sub(&right),
            "mul" => left.mul(&right),
            "floordiv" => left.divmod_floor(&right).expect("夹具里没有零除数").0,
            "mod" => left.divmod_floor(&right).expect("夹具里没有零除数").1,
            "pow" => {
                let exponent = u32::try_from(right.to_i64().expect("指数是小的")).expect("指数非负");
                left.pow_u32(exponent)
            }
            other => panic!("夹具里有没见过的运算符 {other}"),
        };
        assert_eq!(
            actual.to_decimal(),
            expected,
            "{op}({}, {}) 与参照不一致",
            row.key("a").as_str(),
            row.key("b").as_str()
        );
        checked += 1;
    }
    assert!(checked > 100, "夹具条目太少（{checked}）");
}

#[test]
fn unary_matches_the_reference() {
    let fixture = fixture();
    for row in fixture.key("unary").as_arr() {
        let value = from(row.key("a").as_str());
        let expected = row.key("result").as_str();
        let actual = match row.key("op").as_str() {
            "neg" => value.neg(),
            "abs" => value.abs(),
            other => panic!("没见到的一元运算 {other}"),
        };
        assert_eq!(actual.to_decimal(), expected);
    }
}

#[test]
fn comparisons_match_the_reference() {
    let fixture = fixture();
    for row in fixture.key("compare").as_arr() {
        let left = from(row.key("a").as_str());
        let right = from(row.key("b").as_str());
        let order = left.cmp(&right);
        assert_eq!(order == Ordering::Less, row.key("lt").as_bool(), "{left:?} < {right:?}");
        assert_eq!(order != Ordering::Greater, row.key("le").as_bool());
        assert_eq!(order == Ordering::Equal, row.key("eq").as_bool());
        assert_eq!(order == Ordering::Greater, row.key("gt").as_bool());
        assert_eq!(order != Ordering::Less, row.key("ge").as_bool());
        assert_eq!(order != Ordering::Equal, row.key("ne").as_bool());
    }
}

#[test]
fn decimal_text_matches_the_reference() {
    let fixture = fixture();
    for row in fixture.key("text").as_arr() {
        let value = from(row.key("value").as_str());
        let text = value.to_decimal();
        assert_eq!(text, row.key("str").as_str(), "str() 与参照不一致");
        assert_eq!(text, row.key("repr").as_str(), "int 的 repr 与 str 相同（参照实测）");
        assert_eq!(from(&text), value, "往返必须回到同一个值");
    }
}

#[test]
fn hash_matches_the_reference() {
    let fixture = fixture();
    let mut saw_the_named_one = false;
    for row in fixture.key("hash").as_arr() {
        let text = row.key("value").as_str();
        let value = from(text);
        assert_eq!(
            value.hash(),
            row.key("hash").as_i64(),
            "hash({text}) 与参照不一致（`TS-45` ②：算法定义、可观察）"
        );
        if text == "1267650600228229401496703205376" {
            saw_the_named_one = true;
            assert_eq!(value.hash(), 549_755_813_888, "`TS-45` 点名的 hash(2**100)");
        }
    }
    assert!(saw_the_named_one, "夹具里必须有 2**100 那条");
}

#[test]
fn division_by_zero_is_reported_by_the_core_as_none() {
    let fixture = fixture();
    for row in fixture.key("zero_divisor").as_arr() {
        let value = from(row.key("a").as_str());
        assert!(value.divmod_floor(&BigInt::zero()).is_none(), "除零必须给 None");
        // 消息是**留档**：接线时 `//`／`%` 抛 `ZeroDivisionError` 要照它拼（两条实测都是这一句）
        assert_eq!(row.key("floordiv_message").as_str(), "division by zero");
        assert_eq!(row.key("mod_message").as_str(), "division by zero");
    }
}

#[test]
fn float_conversion_matches_the_reference() {
    let fixture = fixture();
    for row in fixture.key("float").as_arr() {
        let value = from(row.key("value").as_str());
        let expected: f64 = row.key("float").as_str().parse().expect("参照的 repr 可解析");
        assert_eq!(
            value.to_f64(),
            expected,
            "float({}) 与参照不一致（正确舍入）",
            row.key("value").as_str()
        );
    }
    // 超出范围：核心给 ±inf，映射 `OverflowError` 是调用点的事（消息实测留档）
    let huge = from(&format!("1{}", "0".repeat(400)));
    assert!(huge.to_f64().is_infinite());
    assert_eq!(fixture.key("float_overflow").as_str(), "int too large to convert to float");
}

#[test]
fn the_string_digit_limit_is_recorded_and_the_core_does_not_enforce_it() {
    // `TS-45` ①：上限（默认 4300）是**调用点的策略**；核心只管数字，故 4301 位串照样往返。
    let fixture = fixture();
    let limits = fixture.key("limits");
    assert_eq!(limits.key("max_str_digits").as_i64(), 4300);
    let inside = limits.key("inside_value").as_str();
    assert_eq!(inside.len(), limits.key("inside_digits").as_i64() as usize);
    let outside = limits.key("outside_value").as_str();
    assert_eq!(outside.len(), limits.key("outside_digits").as_i64() as usize);
    assert_eq!(from(outside).to_decimal(), outside, "核心不设上限，4301 位也往返");
    assert_eq!(from(inside).to_decimal(), inside);
    // 两条消息的原文（接线时报 `ValueError` 要照它拼）
    let message = limits.key("to_str_message").as_str();
    assert!(message.starts_with("Exceeds the limit (4300 digits)"), "实际：{message}");
    let message = limits.key("from_str_message").as_str();
    assert!(message.starts_with("Exceeds the limit (4300 digits)"), "实际：{message}");
}

#[test]
fn the_small_integer_singleton_range_is_the_probed_one() {
    // `OM-23`：`-5..=256`（不是实现自由；`is` 可观测）。这里只钉住夹具里的参照值。
    let fixture = fixture();
    assert_eq!(fixture.key("small_int_range").key("min").as_i64(), -5);
    assert_eq!(fixture.key("small_int_range").key("max").as_i64(), 256);
}

// --------------------------------------------------------------------------- #
// 对象模型接线（`int` 的两种载荷 ＋ 执行器的公开入口）
// --------------------------------------------------------------------------- #

#[test]
fn arithmetic_through_the_object_model_matches_the_reference() {
    let vm = common::Vm::new();
    let mut checked = 0;
    for row in fixture().key("arithmetic").as_arr() {
        let symbol = match row.key("op").as_str() {
            "add" => "+",
            "sub" => "-",
            "mul" => "*",
            "floordiv" => "//",
            "mod" => "%",
            "pow" => "**",
            other => panic!("夹具里有没见过的运算符 {other}"),
        };
        let left = object(&vm, row.key("a").as_str());
        let right = object(&vm, row.key("b").as_str());
        let result = arithmetic_public(&vm.instance, left, right, symbol, 0).unwrap_or_else(|error| {
            panic!(
                "{symbol}({}, {}) 出错：{error:?}",
                row.key("a").as_str(),
                row.key("b").as_str()
            )
        });
        assert_eq!(
            decimal(&vm, result),
            row.key("result").as_str(),
            "{symbol}({}, {}) 与参照不一致",
            row.key("a").as_str(),
            row.key("b").as_str()
        );
        checked += 1;
    }
    assert!(checked > 100, "夹具条目太少（{checked}）");
}

#[test]
fn an_overflowing_result_is_still_the_same_type() {
    // `TS-45`：`int` **只有一个类型对象**（`type(2**100) is int`）——载荷换了大整数不换类型
    let vm = common::Vm::new();
    let big = object(&vm, "1267650600228229401496703205376");
    assert_eq!(
        vm.instance.type_of(big),
        vm.instance.type_named("int").expect("int 在注册表里"),
        "大整数与 `type(2**100)` 必须仍是 `int`"
    );
    // 装不下 `i64`：`int_value`（快路径）给 `None`，`int_of`（按类型分派用）给得出
    assert_eq!(vm.instance.int_value(big), None, "`i64` 快路径装不下");
    assert_eq!(decimal(&vm, big), "1267650600228229401496703205376");
}

#[test]
fn comparison_and_equality_through_the_object_model_match_the_reference() {
    let vm = common::Vm::new();
    for row in fixture().key("compare").as_arr() {
        let left = object(&vm, row.key("a").as_str());
        let right = object(&vm, row.key("b").as_str());
        for (symbol, field) in [("<", "lt"), ("<=", "le"), (">", "gt"), (">=", "ge")] {
            let expected = row.key(field).as_bool();
            let result = compare_public(&vm.instance, left, right, symbol, 0)
                .unwrap_or_else(|error| panic!("{symbol} 应当可比：{error:?}"));
            assert_eq!(result, expected, "{symbol}");
        }
        let equal = values_equal_public(&vm.instance, left, right);
        assert_eq!(equal, row.key("eq").as_bool(), "== 与参照不一致");
        assert_eq!(!equal, row.key("ne").as_bool(), "!= 与参照不一致");
    }
}

#[test]
fn unary_and_truthiness_through_the_object_model_match_the_reference() {
    let vm = common::Vm::new();
    for row in fixture().key("unary").as_arr() {
        let symbol = match row.key("op").as_str() {
            "neg" => "-",
            "abs" => "abs",
            other => panic!("夹具里有没见过的一元运算 {other}"),
        };
        let value = object(&vm, row.key("a").as_str());
        let result = unary_public(&vm.instance, value, symbol, 0)
            .unwrap_or_else(|error| panic!("{symbol} 应当成功：{error:?}"));
        assert_eq!(decimal(&vm, result), row.key("result").as_str(), "{symbol}");
    }
    // 真值：大整数非零为真（`int_value` 的快路径对它给 `None` ⇒ 曾会被误判成假）
    let big = object(&vm, "1267650600228229401496703205376");
    assert!(truthiness_public(&vm.instance, big, 0).expect("真值判定"));
    assert!(!truthiness_public(&vm.instance, object(&vm, "0"), 0).expect("真值判定"));
}

#[test]
fn dividing_a_big_integer_by_zero_raises_the_reference_message() {
    let vm = common::Vm::new();
    let left = object(&vm, "1267650600228229401496703205376");
    let zero = object(&vm, "0");
    for symbol in ["//", "%"] {
        let error = arithmetic_public(&vm.instance, left, zero, symbol, 0).expect_err("除零要报错");
        match error {
            pyawa_core::ExecError::Raised { exception } => {
                // SAFETY: 异常对象由实例保活。
                let ty = unsafe { exception.as_ref() }.ty();
                assert_eq!(unsafe { ty.as_ref() }.name(), "ZeroDivisionError", "{symbol}");
            }
            other => panic!("{symbol} 应当抛 ZeroDivisionError，得到 {other:?}"),
        }
    }
}

// --------------------------------------------------------------------------- #
// `TS-45` ①：`str` → `int` 的位数上限（`sys.set_int_max_str_digits` 的落点）
// --------------------------------------------------------------------------- #

/// 走**类型调用**造 `int`（`int('<十进制串>')` ⇒ `int_new`）：集成测试戳不到私有模块，
/// 从"用户能看到的行为"打进去。
fn int_from_text(vm: &common::Vm, text: &str) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let callable = vm
        .instance
        .type_value(vm.instance.type_named("int").expect("int 在注册表里"));
    let argument = vm.instance.new_str(text);
    pyawa_core::call_value(&vm.instance, callable, &[argument], &[])
}

/// 取脚本异常的 `"类名: 消息"`。
fn error_message(vm: &common::Vm, error: pyawa_core::ExecError) -> String {
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: 异常对象由实例保活。
            let ty = unsafe { exception.as_ref() }.ty();
            let name = unsafe { ty.as_ref() }.name().to_owned();
            let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(&vm.instance);
            match message {
                Some(text) => format!("{name}: {text}"),
                None => name,
            }
        }
        other => panic!("应当是脚本异常，得到 {other:?}"),
    }
}

#[test]
fn the_string_digit_limit_is_enforced_when_parsing() {
    let vm = common::Vm::new();
    let fixture = fixture();
    let limits = fixture.key("limits");
    let edges = fixture.key("sys_limits");
    let default = limits.key("max_str_digits").as_i64() as usize;

    // 上限内：能解析、十进制往返（顺带证明结果确实是大整数）
    let inside = limits.key("inside_value").as_str();
    let parsed = int_from_text(&vm, inside).expect("上限内的位数应当能解析");
    assert_eq!(decimal(&vm, parsed), inside);

    // 超出：消息与参照**逐字**一致（含实际位数；夹具存的是 `str(error)`，这里补类名前缀）
    let outside = limits.key("outside_value").as_str();
    let error = int_from_text(&vm, outside).expect_err("超限必须报 ValueError");
    assert_eq!(
        error_message(&vm, error),
        format!("ValueError: {}", limits.key("from_str_message").as_str())
    );

    // **前导零也计入**（参照实测）
    let zeros = "0".repeat(default + 1);
    let error = int_from_text(&vm, &zeros).expect_err("前导零也计入上限");
    assert_eq!(
        error_message(&vm, error),
        // 这条是 `error_of` 记的 ⇒ 已带 `ValueError: ` 前缀
        edges.key("leading_zeros_over_limit_message").as_str()
    );

    // 带符号的**正好**上限位 ⇒ 可以（符号不计入位数）
    assert!(edges.key("sign_plus_exactly_limit_is_ok").as_bool());
    let signed = format!("-{}", "1".repeat(default));
    assert!(int_from_text(&vm, &signed).is_ok(), "符号不计入位数");

    // 宿主调低上限（`sys.set_int_max_str_digits` 的落点）⇒ 640 位也不行；调 0 ⇒ 不限
    let threshold = edges.key("threshold").as_i64() as u32;
    vm.instance.set_int_max_str_digits(threshold);
    assert!(int_from_text(&vm, &"1".repeat(threshold as usize + 1)).is_err(), "调低后要拦住");
    assert!(int_from_text(&vm, &"1".repeat(threshold as usize)).is_ok(), "正好阈值可以");
    assert!(edges.key("setting_zero_succeeds").as_bool());
    assert!(edges.key("zero_means_unlimited").as_bool());
    vm.instance.set_int_max_str_digits(0);
    assert!(int_from_text(&vm, outside).is_ok(), "0 ＝ 不限");
}

#[test]
fn the_string_digit_limit_is_enforced_when_rendering() {
    // `TS-45` ①的**输出方向**：`repr(huge)`／`str(huge)` 超限 ⇒ `ValueError`
    // （消息与输入方向那句**不同**：这条不带 `value has N digits`）
    let vm = common::Vm::new();
    let fixture = fixture();
    let limits = fixture.key("limits");
    let default = limits.key("max_str_digits").as_i64() as usize;

    // 上限内：正好 4300 位能渲染
    let inside = object(&vm, limits.key("inside_value").as_str());
    assert_eq!(vm.instance.object_repr(inside).expect("repr").len(), default);

    // 超出：4301 位 ⇒ `ValueError`；`repr` 与 `str`（缺省回退到 `repr`）都拦
    let outside = object(&vm, limits.key("outside_value").as_str());
    let expected = format!("ValueError: {}", limits.key("to_str_message").as_str());
    let error = vm.instance.object_repr(outside).expect_err("超限必须报 ValueError");
    assert_eq!(error_message(&vm, error), expected);
    let error = vm.instance.object_str(outside).expect_err("str 也要拦");
    assert_eq!(error_message(&vm, error), expected);

    // 宿主调 `0`（不限）⇒ 4301 位照样渲染
    vm.instance.set_int_max_str_digits(0);
    assert_eq!(vm.instance.object_repr(outside).expect("repr").len(), default + 1);
}

// --------------------------------------------------------------------------- #
// 位运算与位移（`TS-45`：负数走补码语义／位移是 floor）
// --------------------------------------------------------------------------- #

#[test]
fn bitwise_matches_the_reference() {
    let fixture = fixture();
    let mut checked = 0;
    for row in fixture.key("bitwise").as_arr() {
        let left = from(row.key("a").as_str());
        let right = from(row.key("b").as_str());
        let (text_a, text_b) = (row.key("a").as_str(), row.key("b").as_str());
        assert_eq!(left.bit_and(&right).to_decimal(), row.key("and").as_str(), "{text_a} & {text_b}");
        assert_eq!(left.bit_or(&right).to_decimal(), row.key("or").as_str(), "{text_a} | {text_b}");
        assert_eq!(left.bit_xor(&right).to_decimal(), row.key("xor").as_str(), "{text_a} ^ {text_b}");
        checked += 1;
    }
    assert!(checked > 20, "夹具条目太少（{checked}）");
}

#[test]
fn shifts_match_the_reference() {
    let fixture = fixture();
    let mut checked = 0;
    for row in fixture.key("shifts").as_arr() {
        let value = from(row.key("a").as_str());
        let count = row.key("n").as_i64() as u64;
        let text = row.key("a").as_str();
        assert_eq!(
            value.shl(count).expect("位移在实现上限内").to_decimal(),
            row.key("lshift").as_str(),
            "{text} << {count}"
        );
        assert_eq!(value.shr(count).to_decimal(), row.key("rshift").as_str(), "{text} >> {count}");
        checked += 1;
    }
    assert!(checked > 100, "夹具条目太少（{checked}）");
    // 位移量任意大：`1 >> 2**62 == 0`（参照实测），不报错、不炸内存
    assert_eq!(BigInt::from_i64(1).shr(1 << 62).to_decimal(), "0");
    assert_eq!(BigInt::from_i64(-1).shr(1 << 62).to_decimal(), "-1");
    // `1 << 2**62`：参照实测 `MemoryError` ⇒ 核心给 `None`（不是假装算出来）
    assert!(BigInt::from_i64(1).shl(1 << 62).is_none());
}

#[test]
fn invert_matches_the_reference() {
    for row in fixture().key("invert").as_arr() {
        let value = from(row.key("a").as_str());
        assert_eq!(value.invert().to_decimal(), row.key("result").as_str(), "~{}", row.key("a").as_str());
    }
}

// --------------------------------------------------------------------------- #
// 位运算／位移与 int↔float 的**调用点**（走执行器公开入口与类型调用）
// --------------------------------------------------------------------------- #

#[test]
fn bitwise_and_shifts_through_the_object_model_match_the_reference() {
    let vm = common::Vm::new();
    let fixture = fixture();
    let mut checked = 0;
    for row in fixture.key("bitwise").as_arr() {
        for (symbol, field) in [("&", "and"), ("|", "or"), ("^", "xor")] {
            let left = object(&vm, row.key("a").as_str());
            let right = object(&vm, row.key("b").as_str());
            let result = arithmetic_public(&vm.instance, left, right, symbol, 0)
                .unwrap_or_else(|error| panic!("{symbol} 应当成功：{error:?}"));
            assert_eq!(
                decimal(&vm, result),
                row.key(field).as_str(),
                "{}{symbol}{} 与参照不一致",
                row.key("a").as_str(),
                row.key("b").as_str()
            );
        }
        checked += 1;
    }
    assert!(checked > 20, "夹具条目太少（{checked}）");

    for row in fixture.key("shifts").as_arr() {
        for (symbol, field) in [("<<", "lshift"), (">>", "rshift")] {
            let value = object(&vm, row.key("a").as_str());
            let count = object(&vm, &row.key("n").as_i64().to_string());
            let result = arithmetic_public(&vm.instance, value, count, symbol, 0)
                .unwrap_or_else(|error| panic!("{symbol} 应当成功：{error:?}"));
            assert_eq!(
                decimal(&vm, result),
                row.key(field).as_str(),
                "{}{symbol}{} 与参照不一致",
                row.key("a").as_str(),
                row.key("n").as_i64()
            );
        }
    }

    // 错误路径：负位移量；巨大位移量（实测 `MemoryError`，消息为空）
    let one = object(&vm, "1");
    let negative = object(&vm, "-1");
    let errors = fixture.key("shift_errors");
    for symbol in ["<<", ">>"] {
        let error = arithmetic_public(&vm.instance, one, negative, symbol, 0).expect_err("负位移量");
        assert_eq!(error_message(&vm, error), errors.key("negative_left").as_str(), "{symbol}");
    }
    let huge = object(&vm, &(1i64 << 62).to_string());
    let error = arithmetic_public(&vm.instance, one, huge, "<<", 0).expect_err("巨大左移量");
    assert_eq!(error_message(&vm, error), errors.key("huge_left").as_str());
    // `1 >> 2**62 == 0`（参照实测：不报错）
    let zero = arithmetic_public(&vm.instance, one, huge, ">>", 0).expect("巨大右移量应当给 0");
    assert_eq!(decimal(&vm, zero), "0");
}

/// 造一个 `float` 对象。
fn float_object(vm: &common::Vm, value: f64) -> NonNull<Header> {
    vm.instance
        .alloc(pyawa_core::FloatObject::new(
            vm.instance.type_named("float").expect("float 在注册表里"),
            value,
        ))
        .into_raw()
        .cast::<Header>()
}

/// 走**类型调用**造 `int`／`float`（`int(x)`／`float(x)` ⇒ 各自的 `new` 槽）。
fn call_type(
    vm: &common::Vm,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let callable = vm
        .instance
        .type_value(vm.instance.type_named(name).expect("类型在注册表里"));
    pyawa_core::call_value(&vm.instance, callable, args, &[])
}

#[test]
fn int_and_float_conversions_through_the_object_model_match_the_reference() {
    let vm = common::Vm::new();
    let fixture = fixture();

    // `float(<整数>)`：正确舍入（夹具那几行的整数含 2**100 与 10**30）
    for row in fixture.key("float").as_arr() {
        let integer = object(&vm, row.key("value").as_str());
        let converted = call_type(&vm, "float", &[integer]).expect("float(整数) 应当成功");
        assert_eq!(
            vm.instance.float_value(converted),
            Some(row.key("float").as_str().parse::<f64>().expect("参照 repr 可解析")),
            "float({}) 与参照不一致",
            row.key("value").as_str()
        );
    }
    // 溢出：实测消息
    let huge = object(&vm, &format!("1{}", "0".repeat(400)));
    let error = call_type(&vm, "float", &[huge]).expect_err("超出 double 应当报错");
    assert_eq!(
        error_message(&vm, error),
        format!("OverflowError: {}", fixture.key("float_overflow").as_str())
    );

    // `int(<浮点>)`：向零截断；`inf`／`nan` 各按实测消息
    let conversions = fixture.key("float_to_int");
    let positive = float_object(&vm, 2.5);
    let converted = call_type(&vm, "int", &[positive]).expect("int(2.5) 应当成功");
    assert_eq!(decimal(&vm, converted), conversions.key("truncate_positive").as_str());
    let negative = float_object(&vm, -2.5);
    let converted = call_type(&vm, "int", &[negative]).expect("int(-2.5) 应当成功");
    assert_eq!(decimal(&vm, converted), conversions.key("truncate_negative").as_str());
    let negative_zero = float_object(&vm, -0.0);
    let converted = call_type(&vm, "int", &[negative_zero]).expect("int(-0.0) 应当成功");
    assert_eq!(decimal(&vm, converted), conversions.key("from_negative_zero").as_str());
    for (value, field) in [
        (f64::INFINITY, "from_infinity"),
        (f64::NAN, "from_nan"),
    ] {
        let number = float_object(&vm, value);
        let error = call_type(&vm, "int", &[number]).expect_err("inf／nan 应当报错");
        assert_eq!(error_message(&vm, error), conversions.key(field).as_str(), "{field}");
    }

    // `int(<大 double>)`：double 的**精确值**（不能借道 `i64` 静默饱和）
    for (value, field) in [
        (1e300_f64, "exact_from_large_double"),
        (-1e300_f64, "exact_from_negative_large_double"),
        (5e-324_f64, "exact_from_denormal"),
        (0.5_f64, "exact_from_half"),
    ] {
        let number = float_object(&vm, value);
        let converted = call_type(&vm, "int", &[number]).expect("int(double) 应当成功");
        assert_eq!(
            decimal(&vm, converted),
            conversions.key(field).as_str(),
            "int({value:e}) 的精确值与参照不一致"
        );
    }
}

// --------------------------------------------------------------------------- #
// `TS-45`：大整数的 `__format__`（走类型字典里那个原生 `__format__`）
// --------------------------------------------------------------------------- #

/// 调 `int.__format__(self, spec)`（`FORMAT_WITH_SPEC` 走的就是这条）。
fn format_object(
    vm: &common::Vm,
    value: NonNull<Header>,
    spec: &str,
) -> Result<String, pyawa_core::ExecError> {
    let ty = vm.instance.type_named("int").expect("int 在注册表里");
    let function = vm
        .instance
        .type_lookup(ty, "__format__")
        .expect("int 的类型字典里有 __format__");
    // SAFETY: 类型字典里放的是原生可调用对象。
    let handler =
        unsafe { (*function.as_ptr().cast::<pyawa_core::BuiltinFunctionObject>()).function() };
    let spec_object = vm.instance.new_str(spec);
    // SAFETY: 按原生函数契约调用：bound ＝ self，一个实参 ＝ 规格。
    let result = unsafe { handler(&vm.instance, Some(value), &[spec_object], &[])? };
    Ok(vm.instance.text_value(result).expect("结果是 str"))
}

#[test]
fn formatting_big_integers_matches_the_reference() {
    let vm = common::Vm::new();
    let fixture = fixture();
    let mut checked = 0;
    for row in fixture.key("format").as_arr() {
        let value = object(&vm, row.key("value").as_str());
        let spec = row.key("spec").as_str();
        let label = format!("format({}, {spec:?})", row.key("value").as_str());
        let outcome = format_object(&vm, value, spec);
        match (outcome, row.get("result"), row.get("error")) {
            (Ok(text), Some(common::Json::Str(expected)), _) => {
                assert_eq!(&text, expected, "{label} 与参照不一致")
            }
            (Err(error), _, Some(common::Json::Str(expected))) => {
                assert_eq!(&error_message(&vm, error), expected, "{label} 的报错与参照不一致")
            }
            (other, _, _) => panic!("{label} 与夹具对不上：{other:?}"),
        }
        checked += 1;
    }
    assert!(checked > 20, "夹具条目太少（{checked}）");

    // 几个**单列**的实测事实（`format_errors`）
    let errors = fixture.key("format_errors");
    let two_100 = object(&vm, "1267650600228229401496703205376");
    let error = format_object(&vm, two_100, "c").expect_err("`c` 超 C long");
    assert_eq!(error_message(&vm, error), errors.key("char_too_large").as_str());
    let minus_one = object(&vm, "-1");
    let error = format_object(&vm, minus_one, "c").expect_err("`c` 负数超 Unicode 范围");
    assert_eq!(error_message(&vm, error), errors.key("char_negative").as_str());
    assert_eq!(
        format_object(&vm, object(&vm, "42"), "c").expect("`c` 正常值"),
        errors.key("char_ok").as_str()
    );

    // 位数上限**管**十进制码、**不管**十六进制码（两条都是实测定下来的）
    let over = object(&vm, &format!("1{}", "0".repeat(5000)));
    let error = format_object(&vm, over, "").expect_err("十进制超上限");
    assert_eq!(error_message(&vm, error), errors.key("decimal_over_limit").as_str());
    assert!(
        matches!(errors.get("hex_over_limit"), Some(common::Json::Null)),
        "参照实测：十六进制不受限（夹具记的是 null）"
    );
    let hex = format_object(&vm, over, "x").expect("十六进制不受位数上限约束");
    assert_eq!(hex, errors.key("hex_over_limit_text").as_str(), "十六进制逐字对拍");

    // 超大整数上的**浮点码**：先撞 `float()` 的溢出（实测两条消息相同）
    for spec in ["e", ".2f"] {
        let error = format_object(&vm, over, spec).expect_err("超出 double 范围");
        assert_eq!(
            error_message(&vm, error),
            errors.key("float_code_over_limit").as_str(),
            "{spec}"
        );
    }
}
