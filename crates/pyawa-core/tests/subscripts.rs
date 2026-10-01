//! 属性与下标族的**下标**部分（`docs/SPEC-bytecode.md` §10；3.14 无 `BINARY_SUBSCR`，
//! 下标读走 `BINARY_OP` ＋ `NB_SUBSCR`）。
//!
//! 压栈顺序是**实测**的：`BINARY_OP NB_SUBSCR` 是 `[容器, 键]`；`STORE_SUBSCR` 是
//! `[值, 容器, 键]`（键在 TOS）；`DELETE_SUBSCR` 是 `[容器, 键]`。

mod common;

use pyawa_core::opcode::get_nb_ops;
use pyawa_core::{DictObject, ExecError, Instance, ListObject, StrObject, Value};

use common::{emit, op, Vm};

fn nb(name: &str) -> u8 {
    get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

fn header_of(value: &Value<'_>, instance: &Instance) -> core::ptr::NonNull<pyawa_core::Header> {
    value.as_header(instance).expect("应当是具体对象")
}

/// # Safety
///
/// 调用方必须先确认类型（`T` 要与它一致）。
unsafe fn payload<'a, T>(value: &Value<'_>, instance: &'a Instance) -> &'a T {
    // SAFETY: 由调用方保证类型正确。
    unsafe { &*header_of(value, instance).as_ptr().cast::<T>() }
}

#[test]
fn list_index_reads_writes_and_deletes() {
    let vm = Vm::new();
    let make = |vm: &Vm| vec![Some(vm.constant(10)), Some(vm.constant(20)), Some(vm.constant(30))];

    // 读下标 1 → 20（下标要**单独**一个常量：常量表里放的是它的**值**）
    let mut consts = make(&vm);
    consts.push(Some(vm.constant(1)));
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("BUILD_LIST"), 3),
            (op("LOAD_CONST"), 3),
            (op("BINARY_OP"), nb("NB_SUBSCR")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(20), &vm.instance));

    // 负下标 −1 → 30
    let mut consts = make(&vm);
    consts.push(Some(vm.constant(-1)));
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("BUILD_LIST"), 3),
            (op("LOAD_CONST"), 3),
            (op("BINARY_OP"), nb("NB_SUBSCR")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(30), &vm.instance),
        "-1 取最后一个"
    );

    // 写下标 0 ＝ 99、删下标 1，然后返回列表本体
    let mut consts = make(&vm);
    consts.push(Some(vm.constant(99)));
    consts.push(Some(vm.constant(0))); // 下标 0
    consts.push(Some(vm.constant(1))); // 下标 1
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("BUILD_LIST"), 3),
            (op("STORE_FAST"), 0),
            // c[0] = 99：顺序是 [值, 容器, 键]（实测）
            (op("LOAD_CONST"), 3),
            (op("LOAD_FAST"), 0),
            (op("LOAD_CONST"), 4),
            (op("STORE_SUBSCR"), 0),
            // del c[1]：[容器, 键]
            (op("LOAD_FAST"), 0),
            (op("LOAD_CONST"), 5),
            (op("DELETE_SUBSCR"), 0),
            (op("LOAD_FAST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元造的是 list。
    let list = unsafe { payload::<ListObject>(&result, &vm.instance) };
    assert_eq!(list.len(), 2, "删掉一个之后剩两个");
    assert!(list
        .item(0)
        .map(|value| value == vm.constant_ref(&code, 3).unwrap())
        .unwrap_or(false), "下标 0 已被写进 99");
    assert_eq!(
        list.item(1),
        code.get().constant(2),
        "原来的 30 还在原位"
    );
}

#[test]
fn tuple_subscript_reads_but_refuses_assignment() {
    let vm = Vm::new();
    let consts = vec![
        Some(vm.constant(10)),
        Some(vm.constant(20)),
        Some(vm.constant(0)), // 下标 0
    ];
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_TUPLE"), 2),
            (op("LOAD_CONST"), 2),
            (op("BINARY_OP"), nb("NB_SUBSCR")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(10), &vm.instance));

    // t[0] = 1 必须被拒（tuple 不可变）
    let consts = vec![Some(vm.constant(10)), Some(vm.constant(1)), Some(vm.constant(0))];
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("BUILD_TUPLE"), 1),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("STORE_SUBSCR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    assert!(matches!(
        vm.run(&code),
        Err(ExecError::Unsupported { .. })
    ));
}

#[test]
fn dict_keys_use_value_equality() {
    // TS-40 的可观察后果：`d[True]` 命中的是键 `1`
    let vm = Vm::new();
    let str_type = vm.instance.singletons().str_type();
    let text = vm.instance.alloc(StrObject::new(str_type, "a".to_owned()));
    let truth = vm.instance.own(vm.instance.singletons().boolean(true)).into_raw();
    let consts = vec![
        Some(vm.constant(1)),
        Some(text.into_raw().cast::<pyawa_core::Header>()),
        Some(truth),
    ];

    let code = vm.code(
        6,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_MAP"), 1),
            (op("LOAD_CONST"), 2),
            (op("BINARY_OP"), nb("NB_SUBSCR")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert_eq!(
        header_of(&result, &vm.instance),
        code.get().constant(1).unwrap(),
        "取回的是原来那个 str 对象"
    );

    // 写：d[True] = 'b'，键仍然是 1 那一个；再删掉它
    let text_b = vm.instance.alloc(StrObject::new(str_type, "b".to_owned()));
    let truth = vm.instance.own(vm.instance.singletons().boolean(true)).into_raw();
    let consts = vec![
        Some(vm.constant(1)),
        Some(text_b.into_raw().cast::<pyawa_core::Header>()),
        Some(truth),
    ];
    let code = vm.code(
        6,
        1,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_MAP"), 1),
            (op("STORE_FAST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_FAST"), 0),
            (op("LOAD_CONST"), 2),
            (op("STORE_SUBSCR"), 0),
            (op("LOAD_FAST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元造的是 dict。
    let dict = unsafe { payload::<DictObject>(&result, &vm.instance) };
    assert_eq!(dict.len(), 1, "True 与 1 是同一个键，不该多出一条");
    assert_eq!(
        dict.entry(0).map(|(key, _)| key),
        code.get().constant(0),
        "键保留先出现的那个对象"
    );
}

#[test]
fn dict_delete_and_missing_key() {
    let vm = Vm::new();
    let consts = vec![Some(vm.constant(1)), Some(vm.constant(2))];

    // 建 {1: 2} → del d[1] → 返回字典（空）
    let code = vm.code(
        4,
        1,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_MAP"), 1),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("LOAD_CONST"), 0),
            (op("DELETE_SUBSCR"), 0),
            (op("LOAD_FAST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元造的是 dict。
    let dict = unsafe { payload::<DictObject>(&result, &vm.instance) };
    assert!(dict.is_empty(), "删掉唯一一条之后应当为空");

    // 取不存在的键 ⇒ **真 `KeyError`**（其实参照实现的 `args` 就是那个键）
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_MAP"), 1),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb("NB_SUBSCR")),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(1)), Some(vm.constant(2))],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception().map(|(name, _)| name),
        Some("KeyError".to_owned())
    );
}

#[test]
fn string_index_returns_one_character() {
    let vm = Vm::new();
    let str_type = vm.instance.singletons().str_type();
    let text = vm.instance.alloc(StrObject::new(str_type, "abc".to_owned()));
    let consts = vec![
        Some(text.into_raw().cast::<pyawa_core::Header>()),
        Some(vm.constant(1)),
    ];
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb("NB_SUBSCR")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: str 下标返回 str。
    let character = unsafe { payload::<StrObject>(&result, &vm.instance) };
    assert_eq!(character.value(), "b");
}

#[test]
fn index_out_of_range_is_reported() {
    let vm = Vm::new();
    let consts = vec![Some(vm.constant(1)), Some(vm.constant(2))];
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_LIST"), 2),
            (op("LOAD_CONST"), 0), // 下标 1（值 1）——长度 2，但下面换成越界的 5
            (op("BINARY_OP"), nb("NB_SUBSCR")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    // 先用合法的下标确认程序本身没问题
    assert!(vm.run(&code).is_ok());

    let consts = vec![Some(vm.constant(1)), Some(vm.constant(2)), Some(vm.constant(5))];
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_LIST"), 2),
            (op("LOAD_CONST"), 2),
            (op("BINARY_OP"), nb("NB_SUBSCR")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "IndexError".to_owned(),
            Some("list index out of range".to_owned())
        ))
    );
}
