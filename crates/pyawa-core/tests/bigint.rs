//! **任意精度整数**（`TS-45`／`P1-11`）的纯算术核心 · 参照对拍。
//!
//! 期望值全部来自 `tests/fixture-int-3.14.json`（`tools/gen_int_fixture.py` 在参照实现上
//! **现场实测**导出，含 `TS-45` 点名的 `hash(2**100)` 与 4300 位上限那几条）。
//! 本文件只验**算法**；`int` 载荷／`BINARY_OP`／`repr`／上限策略的接线随后逐笔做。

mod common;

use core::cmp::Ordering;

use pyawa_core::bigint::BigInt;

fn fixture() -> common::Json {
    common::parse(include_str!("fixture-int-3.14.json"))
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
