//! 模式匹配族（`docs/SPEC-bytecode.md` §10）。骨架是**实测**的（本机 3.14.4 反汇编 `match`）：
//!
//! ```text
//! 字面量  case 1:        → LOAD_SMALL_INT 1; COMPARE_OP 88 (bool(==)); POP_JUMP_IF_FALSE; NOT_TAKEN
//! 序列    case [a, b]:   → MATCH_SEQUENCE; POP_JUMP_IF_FALSE; GET_LEN; …; UNPACK_SEQUENCE;
//!                          STORE_FAST_STORE_FAST 18 (a, b)
//! 映射    case {'k': v}: → MATCH_MAPPING; GET_LEN; COMPARE_OP 172 (>=); …; MATCH_KEYS; COPY 1;
//!                          POP_JUMP_IF_NONE; UNPACK_SEQUENCE 1; STORE_FAST v; POP_TOP; POP_TOP
//! 类      case P(x=0):   → MATCH_CLASS 0; COPY 1; POP_JUMP_IF_NONE; UNPACK_SEQUENCE …
//! ```
//!
//! 净栈效应一律以 `dis.stack_effect` 为准（`BC-60`）。实测到的两条形状差别：
//! **`MATCH_CLASS`（−2）连被测对象一起吃掉、只压结果**；**`MATCH_KEYS`（+1）保留被测对象与键 tuple**。
//! 另外 `str`／`dict` 都**不算**序列（实测），缺键 ⇒ 该 case 不匹配（实测）。
//!
//! `NOT_TAKEN` 属 §10 三分类②（参照实现会发、原表未列）⇒ VM **必须容受**，这里是无操作。

mod common;

use pyawa_core::opcode::get_nb_ops;
use core::cell::RefCell;

use pyawa_core::Value;

use common::{assemble, emit, op, Item, Vm};

fn nb(name: &str) -> u8 {
    get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

/// `COMPARE_OP` 的 oparg：cmp 下标 `<< 5`，bit 4 是 `bool(...)` 标志（`BC-58`）。
fn compare(cmp_index: u8, boolean: bool) -> u8 {
    (cmp_index << 5) | if boolean { 0x10 } else { 0x00 }
}

#[test]
fn literal_pattern_compares_and_jumps() {
    // `match x: case 1: return 1; case _: return 0`
    let vm = Vm::new();
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        // 把被测对象放进局部槽 0（真实代码里它是函数的实参）
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_FAST_BORROW"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("COMPARE_OP"), compare(2, true)),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "L1"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("L1"),
        // 实测：**字面量**模式的 `L1` 没有 `POP_TOP`——`COMPARE_OP` 已经把被测对象吃掉了
        // （`POP_TOP` 出现在序列／映射／类模式里，那里被测对象还留在栈上）
        Item::Instr(op("NOP"), 0),
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let consts = vec![
        Some(vm.constant(1)), // 被测对象（局部槽 0）
        Some(vm.constant(1)), // case 里的字面量
        Some(vm.constant(1)), // 命中分支返回
        Some(vm.constant(0)), // 兜底返回
    ];
    let code = vm.code(4, 1, bytes, consts);

    // 命中的那一支
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(1), &vm.instance), "case 1 命中");
}

#[test]
fn literal_pattern_falls_through_when_it_does_not_match() {
    let vm = Vm::new();
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        // 把被测对象放进局部槽 0（真实代码里它是函数的实参）
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_FAST_BORROW"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("COMPARE_OP"), compare(2, true)),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "L1"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("L1"),
        // 实测：**字面量**模式的 `L1` 没有 `POP_TOP`——`COMPARE_OP` 已经把被测对象吃掉了
        // （`POP_TOP` 出现在序列／映射／类模式里，那里被测对象还留在栈上）
        Item::Instr(op("NOP"), 0),
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let consts = vec![
        Some(vm.constant(5)), // 被测对象（不匹配 1）
        Some(vm.constant(1)), // case 里的字面量
        Some(vm.constant(1)),
        Some(vm.constant(0)),
    ];
    let code = vm.code(4, 1, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(0), &vm.instance), "case _ 兜底");
}

#[test]
fn sequence_pattern_unpacks_into_two_locals() {
    // `match x: case [a, b]: return a + b; case _: return 0`（x ＝ (3, 4) ⇒ 7）
    let vm = Vm::new();
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("MATCH_SEQUENCE"), 0),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "L1"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("GET_LEN"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("COMPARE_OP"), compare(2, false)),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "L1"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("UNPACK_SEQUENCE"), 2),
        // 打包槽位：高 4 位收 TOS（第一个元素 ⇒ a＝1）、低 4 位收 TOS1（b＝2）
        Item::Instr(op("STORE_FAST_STORE_FAST"), (1 << 4) | 2),
        Item::Instr(op("LOAD_FAST"), 1),
        Item::Instr(op("LOAD_FAST"), 2),
        Item::Instr(op("BINARY_OP"), nb("NB_ADD")),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("L1"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let tuple = vm.instance.new_tuple(vec![vm.constant(3), vm.constant(4)]);
    let consts = vec![
        Some(tuple),
        Some(vm.constant(2)),
        Some(vm.constant(0)),
    ];
    let code = vm.code(8, 3, bytes.clone(), consts);
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(7), &vm.instance), "3 + 4");

    // 长度不对 ⇒ 走兜底
    let vm = Vm::new();
    let tuple = vm.instance.new_tuple(vec![vm.constant(3)]);
    let consts = vec![
        Some(tuple),
        Some(vm.constant(2)),
        Some(vm.constant(0)),
    ];
    let code = vm.code(8, 3, bytes.clone(), consts);
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(0), &vm.instance), "长度不对 ⇒ 兜底");
}

#[test]
fn mapping_pattern_reads_the_named_key() {
    // `match x: case {'k': v}: return v; case _: return 0`（x ＝ {'k': 9} ⇒ 9）
    let vm = Vm::new();
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("MATCH_MAPPING"), 0),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "L2"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("GET_LEN"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("COMPARE_OP"), compare(5, false)), // >=
        Item::Jump(op("POP_JUMP_IF_FALSE"), "L2"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("LOAD_CONST"), 2), // ('k',)
        Item::Instr(op("MATCH_KEYS"), 0),
        Item::Instr(op("COPY"), 1),
        Item::Jump(op("POP_JUMP_IF_NONE"), "L1"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("UNPACK_SEQUENCE"), 1),
        Item::Instr(op("STORE_FAST"), 1),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_FAST"), 1),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("L1"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("POP_TOP"), 0),
        Item::Label("L2"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let key = vm.instance.new_str("k");
    let keys = vm.instance.new_tuple(vec![key]);
    let dict = vm.instance.alloc(pyawa_core::DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        core::cell::RefCell::new(Vec::new()),
    ));
    let raw_key = vm.instance.new_str("k");
    dict.get().insert_raw(raw_key, vm.constant(9));
    let dict_raw = dict.into_raw().cast::<pyawa_core::Header>();
    let consts = vec![
        Some(dict_raw),
        Some(vm.constant(1)),
        Some(keys),
        Some(vm.constant(0)),
    ];
    let code = vm.code(8, 2, bytes.clone(), consts);
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(9), &vm.instance), "取到 'k' 的值");

    // 缺键 ⇒ 兜底（实测：缺键让这个 case 不匹配）
    let vm = Vm::new();
    let keys = vm.instance.new_tuple(vec![vm.instance.new_str("nope")]);
    let dict = vm.instance.alloc(pyawa_core::DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        core::cell::RefCell::new(Vec::new()),
    ));
    let raw_key = vm.instance.new_str("k");
    dict.get().insert_raw(raw_key, vm.constant(9));
    let dict_raw = dict.into_raw().cast::<pyawa_core::Header>();
    let consts = vec![
        Some(dict_raw),
        Some(vm.constant(1)),
        Some(keys),
        Some(vm.constant(0)),
    ];
    let code = vm.code(8, 2, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(0), &vm.instance), "缺键 ⇒ 兜底");
}

#[test]
fn match_sequence_rejects_str_and_dict() {
    // 实测：`str` 与 `dict` 都**不算**序列
    // 注意：值必须与跑它的 `Vm` **同一个实例**（跨实例混用对象会双重释放）
    for label in ["str", "dict"] {
        let vm = Vm::new();
        let value = if label == "str" {
            vm.instance.new_str("ab")
        } else {
            vm.instance
                .alloc(pyawa_core::DictObject::new(
                    vm.instance.type_named("dict").unwrap(),
                    core::cell::RefCell::new(Vec::new()),
                ))
                .into_raw()
                .cast::<pyawa_core::Header>()
        };
        let code = vm.code(
            4,
            0,
            emit(&[
                (op("RESUME"), 0),
                (op("LOAD_CONST"), 0),
                (op("MATCH_SEQUENCE"), 0),
                (op("RETURN_VALUE"), 0),
            ]),
            vec![Some(value)],
        );
        let result = vm.run(&code).unwrap();
        assert!(
            result.is_same(&Value::Bool(false), &vm.instance),
            "{label} 不该算序列"
        );
    }
}

// ---- §6 的"夹具对拍参照真产物"：`tools/gen_match_fixture.py` 导出的 10 个用例 ----

/// 照夹具里的**被测值描述**造对象。
fn build_subject(vm: &Vm, description: &common::Json) -> core::ptr::NonNull<pyawa_core::Header> {
    match description.key("kind").as_str() {
        "int" => vm.instance.new_int(description.key("value").as_i64()),
        "str" => vm.instance.new_str(description.key("text").as_str()),
        "list" => {
            let items: Vec<_> = description
                .key("items")
                .as_arr()
                .iter()
                .map(|item| build_subject(vm, item))
                .collect();
            vm.instance
                .alloc(pyawa_core::ListObject::new(
                    vm.instance.type_named("list").unwrap(),
                    RefCell::new(items),
                ))
                .into_raw()
                .cast::<pyawa_core::Header>()
        }
        "tuple" => {
            let items: Vec<_> = description
                .key("items")
                .as_arr()
                .iter()
                .map(|item| build_subject(vm, item))
                .collect();
            vm.instance.new_tuple(items)
        }
        "dict" => {
            let mapping = vm.instance.new_dict();
            for entry in description.key("entries").as_arr() {
                let key = entry.as_arr()[0].as_str();
                let value = build_subject(vm, &entry.as_arr()[1]);
                vm.instance.dict_set(mapping, key, value);
            }
            mapping
        }
        other => panic!("夹具里出现了没见过的值种类：{other}"),
    }
}

/// 搭"`case [a, b]` 然后 `case {'k': v}` 然后兜底"的骨架（形状照参照实测；把编译器的
/// 清理路径按同一语义写简）。返回值是 `('seq', a, b)`／`('map', v)`／`None`。
fn match_program(vm: &Vm, subject: core::ptr::NonNull<pyawa_core::Header>) -> pyawa_core::Owned<'_, pyawa_core::CodeObject> {
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    let keys = vm.instance.new_tuple(vec![vm.instance.new_str("k")]);
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0), // 被测值
        Item::Instr(op("COPY"), 1),
        Item::Instr(op("MATCH_SEQUENCE"), 0),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "map"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("GET_LEN"), 0),
        Item::Instr(op("LOAD_CONST"), 1), // 2
        Item::Instr(op("COMPARE_OP"), compare(2, false)), // ==
        Item::Jump(op("POP_JUMP_IF_FALSE"), "map"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("UNPACK_SEQUENCE"), 2),
        Item::Instr(op("STORE_FAST_STORE_FAST"), (0 << 4) | 1), // a, b
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 2), // 'seq'
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("LOAD_FAST"), 1),
        Item::Instr(op("BUILD_TUPLE"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("map"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("MATCH_MAPPING"), 0),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "no_match"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("GET_LEN"), 0),
        Item::Instr(op("LOAD_CONST"), 4), // 1
        Item::Instr(op("COMPARE_OP"), compare(5, false)), // >=
        Item::Jump(op("POP_JUMP_IF_FALSE"), "no_match"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("LOAD_CONST"), 3), // ('k',)
        Item::Instr(op("MATCH_KEYS"), 0),
        Item::Instr(op("COPY"), 1),
        Item::Jump(op("POP_JUMP_IF_NONE"), "keys_missing"),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("UNPACK_SEQUENCE"), 1),
        Item::Instr(op("STORE_FAST"), 2), // v
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 5), // 'map'
        Item::Instr(op("LOAD_FAST"), 2),
        Item::Instr(op("BUILD_TUPLE"), 2),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("keys_missing"),
        // 这条路栈上是 `[被测值, 键, 值]` ⇒ 三个 `POP_TOP`；不能跳进 `no_match`
        // （那条路只剩 `[被测值]`，会多 pop 一次——第一版就是这么写的）
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 6),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("no_match"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 6), // None
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "match_demo",
        "match_demo".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        8,
        3,
        0,
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        bytes,
        Vec::new(),
        vec![
            Some(subject),
            Some(vm.constant(2)),
            Some(vm.instance.new_str("seq")),
            Some(keys),
            Some(vm.constant(1)),
            Some(vm.instance.new_str("map")),
            Some(none),
        ],
    ))
}

/// 把返回的对象读成"命中哪一支 + 绑定值"（与夹具同形）。
fn read_outcome(vm: &Vm, raw: core::ptr::NonNull<pyawa_core::Header>) -> Option<(String, Vec<i64>)> {
    if vm.instance.type_of(raw) == vm.instance.singletons().none_type() {
        return None;
    }
    // SAFETY: 调用方保证这是 tuple。
    let tuple = unsafe { &*raw.as_ptr().cast::<pyawa_core::TupleObject>() };
    let tag = tuple.item(0).expect("至少有标签");
    // SAFETY: 标签是 str。
    let tag = unsafe { &*tag.as_ptr().cast::<pyawa_core::StrObject>() }
        .value()
        .to_owned();
    let mut bound = Vec::new();
    for index in 1..tuple.len() {
        let item = tuple.item(index).expect("下标在范围内");
        bound.push(vm.instance.int_value(item).expect("绑定值都是整数"));
    }
    Some((tag, bound))
}

#[test]
fn match_judgements_match_the_reference_fixture() {
    let fixture = common::parse(include_str!("fixture-match-3.14.json"));
    let vm = Vm::new();
    let mut checked = 0usize;
    for (name, entry) in fixture.key("cases").as_obj() {
        let subject = build_subject(&vm, entry.key("subject"));
        let code = match_program(&vm, subject);
        let result = vm.run(&code).expect("匹配程序应当跑通");
        let raw = result.as_header(&vm.instance).expect("应当是具体对象");
        let observed = read_outcome(&vm, raw);
        let expected = match entry.get("outcome") {
            Some(common::Json::Obj(_)) => {
                let outcome = entry.key("outcome");
                let tag = outcome.key("kind").as_str().to_owned();
                let bound: Vec<i64> = match outcome.key("bound") {
                    common::Json::Num(number) => vec![*number],
                    common::Json::Arr(items) => items.iter().map(|item| item.as_i64()).collect(),
                    other => panic!("夹具里 bound 的形状不对：{other:?}"),
                };
                Some((tag, bound))
            }
            _ => None,
        };
        assert_eq!(observed, expected, "被测值 {name} 的匹配结果");
        checked += 1;
    }
    assert!(checked >= 8, "对拍的用例要够多，实际 {checked} 条");
}
