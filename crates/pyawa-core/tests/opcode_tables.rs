//! 与 `fixture-opcode-3.14.json` 逐项对拍。
//!
//! 覆盖：`BC-1`（27 个必含名）、`BC-27`（专有指令的栈效应）、`BC-30`（基线全等）、
//! `BC-31`（专有指令取空闲编号）、`BC-32`（两张特化表为空）、`BC-35`（cache 宽度）、
//! `BC-37`（7 类分类）、`BC-39`（`BINARY_OP` 的 oparg 顺序）、`BC-40`（版本常量）。
//!
//! 期望值**只**来自夹具（`CONSTRAINTS` 之外的红线：不许把数值硬编码进测试）。
//! 夹具是 JSON，而 `pyawa-core` 不带 JSON 依赖，所以这里自带一个够用的 JSON 解析器。

use std::collections::BTreeSet;

use pyawa_core::opcode;
use pyawa_core::opcode_metadata as meta;

mod common;

use common::Json;

fn fixture() -> Json {
    common::parse(include_str!("fixture-opcode-3.14.json"))
}

// --------------------------------------------------------------------------- #
// 对拍
// --------------------------------------------------------------------------- #

#[test]
fn baseline_is_a_subset_of_opmap() {
    let fixture = fixture();
    let baseline = fixture.key("opmap").as_obj();

    // 基线逐项全等（T-BC-11：**基线 ⊆ `opmap`**）
    for (name, number) in baseline {
        let expected = number.as_i64() as u16;
        assert_eq!(opcode::opcode(name), Some(expected), "opmap[{name}]");
        assert_eq!(opcode::opname(expected), Some(name.as_str()), "opname[{expected}]");
    }

    // 额外项**仅为** Pyawa 专有指令（T-BC-11），且取空闲编号、低于 instrumented 区段（BC-31）
    let baseline_names: BTreeSet<String> =
        baseline.iter().map(|(name, _)| name.clone()).collect();
    let extras: BTreeSet<String> = meta::OPMAP
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .filter(|name| !baseline_names.contains(name))
        .collect();
    let proprietary: BTreeSet<String> = meta::PYAWA_SPECIFIC
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    assert_eq!(extras, proprietary, "T-BC-11：额外项仅为专有指令");
    assert_eq!(
        meta::OPMAP.len(),
        baseline.len() + proprietary.len(),
        "T-BC-11：`opmap` ＝ 基线 ∪ 专有指令"
    );
    assert_eq!(baseline.len(), 154, "BC-30：实测基线 154 个名字");

    for (name, number) in meta::PYAWA_SPECIFIC {
        assert_eq!(opcode::opname(*number), Some(*name));
        assert!(
            !baseline_names.contains(*name),
            "BC-31：{name} 占用了基线编号"
        );
        assert!(*number < meta::MIN_INSTRUMENTED_OPCODE, "BC-31：{name}");
    }
}

#[test]
fn cmp_op_matches_the_oracle() {
    let fixture = fixture();
    let expected: Vec<String> = fixture
        .key("cmp_op")
        .as_arr()
        .iter()
        .map(|name| name.as_str().to_owned())
        .collect();
    let actual: Vec<String> = opcode::get_cmp_op()
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(actual, expected, "BC-39：COMPARE_OP 的 oparg 对应 opcode.cmp_op");
    assert_eq!(actual.len(), 6, "BC-39：六元组");
}

#[test]
fn constants_specializations_and_version() {
    let fixture = fixture();
    assert_eq!(
        i64::from(meta::HAVE_ARGUMENT),
        fixture.key("have_argument").as_i64()
    );
    assert_eq!(
        i64::from(meta::MIN_INSTRUMENTED_OPCODE),
        fixture.key("min_instrumented_opcode").as_i64()
    );

    // BC-32：Pyawa 不做特化，两张表必须为空——夹具里存的是"Pyawa 期望＝空"，
    // oracle 的实际项数另存备查（参照实现有特化，不照抄）。
    assert!(meta::SPECIALIZATIONS.is_empty(), "BC-32：SPECIALIZATIONS 必须为空");
    assert!(meta::SPECIALIZED_OPMAP.is_empty(), "BC-32：SPECIALIZED_OPMAP 必须为空");
    assert!(fixture.key("specializations_pyawa").as_obj().is_empty());
    assert!(fixture.key("specialized_opmap_pyawa").as_obj().is_empty());

    // BC-40：版本常量必须存在且可用（取值由实现维护，夹具不规定它）。
    assert!(meta::INSTRUCTION_SET_VERSION >= 1, "BC-40");
}

#[test]
fn inline_cache_widths_match() {
    let fixture = fixture();
    let table = fixture.key("inline_cache_entries").as_obj();
    let mut non_zero = 0;

    for (name, entries) in table {
        let expected = entries.as_i64() as u32;
        let op = opcode::opcode(name).unwrap_or_else(|| panic!("夹具里的 {name} 不在 opmap"));
        assert_eq!(
            opcode::inline_cache_entries(op),
            expected,
            "BC-35：{name} 的 cache 宽度"
        );
        if expected > 0 {
            non_zero += 1;
        }
    }
    assert_eq!(non_zero, 19, "BC-35：实测 19 个带 cache 的指令");
    assert_eq!(meta::INLINE_CACHE_ENTRIES.len(), 19);
}

#[test]
fn has_predicates_match_every_baseline_opcode() {
    let fixture = fixture();
    let table = fixture.key("has");
    let families: [(&str, fn(u16) -> bool); 7] = [
        ("arg", opcode::has_arg),
        ("const", opcode::has_const),
        ("name", opcode::has_name),
        ("jump", opcode::has_jump),
        ("free", opcode::has_free),
        ("local", opcode::has_local),
        ("exc", opcode::has_exc),
    ];

    // 基线逐指令一致（BC-37）
    for (family, predicate) in families {
        let expected: BTreeSet<String> = table
            .key(family)
            .as_arr()
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        let actual: BTreeSet<String> = fixture
            .key("opmap")
            .as_obj()
            .iter()
            .filter(|(_, number)| predicate(number.as_i64() as u16))
            .map(|(name, _)| name.clone())
            .collect();
        assert_eq!(actual, expected, "BC-37：has_{family} 的分类必须逐指令一致");
    }

    // 额外项（专有指令）：只有 `has_arg` 为真（BC-24），其余六类为假
    for (name, op) in meta::PYAWA_SPECIFIC {
        assert!(opcode::has_arg(*op), "BC-24：{name} 带 oparg");
        for (family, predicate) in families[1..].iter() {
            assert!(!predicate(*op), "BC-37：{name} 不应属于 has_{family}");
        }
    }
}

#[test]
fn stack_effect_matches_every_sample() {
    let fixture = fixture();
    let table = fixture.key("stack_effect").as_obj();
    let mut checked = 0;

    for (name, samples) in table {
        let op = opcode::opcode(name).unwrap_or_else(|| panic!("夹具里的 {name} 不在 opmap"));
        for (key, expected) in samples.as_obj() {
            let (oparg_text, jump_text) = key.split_once('|').expect("采样键形如 `oparg|jump`");
            let oparg: i64 = oparg_text.parse().expect("oparg 是整数");
            let jump = match jump_text {
                "null" => None,
                "true" => Some(true),
                "false" => Some(false),
                other => panic!("未知 jump 形态 {other}"),
            };
            let actual = opcode::stack_effect(op, Some(oparg), jump);
            match expected {
                Json::Null => assert!(actual.is_err(), "BC-38：{name} {key} 期望报错"),
                Json::Num(value) => {
                    assert_eq!(actual.ok(), Some(*value as i32), "BC-38：{name} {key}")
                }
                other => panic!("意外的期望值 {other:?}"),
            }
            checked += 1;
        }
    }
    assert!(checked > 2000, "采样面太小（{checked}），不足以说明问题");
}

#[test]
fn descriptors_match() {
    let fixture = fixture();

    let nb_ops: Vec<(&str, &str)> = fixture
        .key("nb_ops")
        .as_arr()
        .iter()
        .map(|pair| {
            let items = pair.as_arr();
            (items[0].as_str(), items[1].as_str())
        })
        .collect();
    assert_eq!(opcode::get_nb_ops(), nb_ops.as_slice(), "BC-39：nb_ops 顺序");

    for (key, actual) in [
        ("intrinsic1", opcode::get_intrinsic1_descs()),
        ("intrinsic2", opcode::get_intrinsic2_descs()),
        ("special_methods", opcode::get_special_method_names()),
    ] {
        let expected: Vec<&str> = fixture
            .key(key)
            .as_arr()
            .iter()
            .map(|item| item.as_str())
            .collect();
        assert_eq!(actual, expected.as_slice(), "{key}");
    }
}

#[test]
fn boundary_instructions_take_free_numbers() {
    let fixture = fixture();
    assert_eq!(meta::PYAWA_SPECIFIC.len(), 2, "BC-23／BC-31：两条专有指令");

    let baseline = fixture.key("opmap").as_obj();
    let baseline_names: BTreeSet<String> =
        baseline.iter().map(|(name, _)| name.clone()).collect();
    let baseline_numbers: BTreeSet<u16> =
        baseline.iter().map(|(_, number)| number.as_i64() as u16).collect();

    for (name, op) in meta::PYAWA_SPECIFIC {
        assert!(
            !baseline_names.contains(*name),
            "BC-31：{name} 占用了基线的名字"
        );
        assert!(
            !baseline_numbers.contains(op),
            "BC-31：{name} 占用了基线编号 {op}"
        );
        assert!(
            *op < meta::MIN_INSTRUMENTED_OPCODE,
            "BC-31：{name} 不得进入 instrumented 区段"
        );
        assert_eq!(
            opcode::stack_effect(*op, Some(0), None),
            Ok(0),
            "BC-27：{name} 不动值栈"
        );
        assert_eq!(
            opcode::stack_effect(*op, Some(7), None),
            Ok(0),
            "BC-27：{name} 的栈效应与 oparg 无关"
        );
        assert_eq!(opcode::pyawa_specific(name), Some(*op));
        assert!(
            opcode::has_arg(*op),
            "BC-24：{name} 带 oparg（签名条目索引），必须进 has_arg"
        );
    }
}

#[test]
fn required_names_exist_and_executors_are_never_returned() {
    // BC-1／SPEC-bytecode §2.3 的 27 个必含名（清单来自规格，不来自夹具）
    const REQUIRED: [&str; 27] = [
        "EXTENDED_ARG", "COMPARE_OP", "BINARY_OP", "CALL_INTRINSIC_1", "CALL_INTRINSIC_2",
        "CONTAINS_OP", "CONVERT_VALUE", "END_ASYNC_FOR", "ENTER_EXECUTOR", "FOR_ITER",
        "IMPORT_NAME", "IS_OP", "JUMP_BACKWARD", "LOAD_ATTR", "LOAD_COMMON_CONSTANT",
        "LOAD_FAST_BORROW_LOAD_FAST_BORROW", "LOAD_FAST_LOAD_FAST", "LOAD_GLOBAL",
        "LOAD_SMALL_INT", "LOAD_SPECIAL", "LOAD_SUPER_ATTR", "SEND", "SET_FUNCTION_ATTRIBUTE",
        "STORE_FAST_LOAD_FAST", "STORE_FAST_STORE_FAST", "STORE_GLOBAL", "STORE_NAME",
    ];
    for name in REQUIRED {
        assert!(opcode::opcode(name).is_some(), "BC-1：缺 {name}");
    }

    // BC-32：无 JIT、不发射 executor ⇒ 恒 None
    assert_eq!(opcode::get_executor(&"code", 0), None);
    assert_eq!(opcode::get_executor(&(), 12345), None);

    // 未定义的编号必须报错，而不是给一个看起来像样的值
    assert_eq!(
        opcode::stack_effect(9999, None, None),
        Err(opcode::StackEffectError::UnknownOpcode(9999))
    );
}
