//! 迭代族的可测性质（`docs/SPEC-bytecode.md` §10）。
//!
//! 栈纪律用**我们自己生成的 `stack_effect` 表**（数据来自 oracle）钉住，实测值是：
//! `GET_ITER` 净 0、`FOR_ITER` **两条分支都 +1**、`END_FOR` 与 `POP_ITER` 各 −1、`GET_LEN` +1。
//! 于是"耗尽"那条分支必须压一个占位（本层用内部 NULL 哨兵），由 `END_FOR`／`POP_ITER` 收尾——
//! 不然 `for` 循环的栈会一边多一边少。
//!
//! 迭代器类型名**照探测表取**：`str` 的迭代器在这台机器上叫 `str_ascii_iterator`（不是
//! `str_iterator`），`dict` 的叫 `dict_keyiterator`。

mod common;

use pyawa_core::opcode::get_nb_ops;
use pyawa_core::{builtin_types::builtin_type, ExecError, Value};

use common::{assemble, emit, op, Item, Vm};

fn nb(name: &str) -> u8 {
    get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

#[test]
fn for_loop_sums_a_list() {
    // total = 0; for x in [1, 2, 3]: total += x; return total
    let vm = Vm::new();
    let consts = vec![
        Some(vm.constant(0)),
        Some(vm.constant(1)),
        Some(vm.constant(2)),
        Some(vm.constant(3)),
    ];
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("BUILD_LIST"), 3),
        Item::Instr(op("GET_ITER"), 0),
        Item::Label("L1"),
        Item::Jump(op("FOR_ITER"), "L2"),
        Item::Instr(op("STORE_FAST"), 1),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("LOAD_FAST"), 1),
        Item::Instr(op("BINARY_OP"), nb("NB_ADD")),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Jump(op("JUMP_BACKWARD"), "L1"),
        Item::Label("L2"),
        Item::Instr(op("END_FOR"), 0),
        Item::Instr(op("POP_ITER"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    assert_eq!(pyawa_core::decode::validate(&bytes), Ok(()));

    let code = vm.code(8, 2, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(6), &vm.instance),
        "1 + 2 + 3 ＝ 6"
    );
}

#[test]
fn iterates_strings_and_dict_keys() {
    // 数一数字符串有几个字符
    let vm = Vm::new();
    let str_type = vm.instance.singletons().str_type();
    let text = vm.instance.alloc(pyawa_core::StrObject::new(str_type, "abc".to_owned()));
    let consts = vec![Some(text.into_raw().cast::<pyawa_core::Header>())];
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_FAST"), 0), // 留在局部槽里，稍后还要量长度
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("GET_ITER"), 0),
        Item::Label("L1"),
        Item::Jump(op("FOR_ITER"), "L2"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Jump(op("JUMP_BACKWARD"), "L1"),
        Item::Label("L2"),
        Item::Instr(op("END_FOR"), 0),
        Item::Instr(op("POP_ITER"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("GET_LEN"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code(4, 1, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(3), &vm.instance),
        "GET_LEN 对 str 给的是字符数"
    );
}

#[test]
fn get_len_on_a_list() {
    let vm = Vm::new();
    let consts = vec![
        Some(vm.constant(1)),
        Some(vm.constant(2)),
        Some(vm.constant(3)),
        Some(vm.constant(4)),
    ];
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 3),
            (op("BUILD_LIST"), 4),
            (op("GET_LEN"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(4), &vm.instance));
}

#[test]
fn dictionary_iteration_yields_keys() {
    // 数一数字典迭代出几个键：{7: 8} ⇒ 1
    let vm = Vm::new();
    let consts = vec![
        Some(vm.constant(0)),
        Some(vm.constant(7)),
        Some(vm.constant(8)),
        Some(vm.constant(1)),
    ];
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("BUILD_MAP"), 1),
        Item::Instr(op("GET_ITER"), 0),
        Item::Label("L1"),
        Item::Jump(op("FOR_ITER"), "L2"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("BINARY_OP"), nb("NB_ADD")),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Jump(op("JUMP_BACKWARD"), "L1"),
        Item::Label("L2"),
        Item::Instr(op("END_FOR"), 0),
        Item::Instr(op("POP_ITER"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code(8, 1, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(1), &vm.instance),
        "字典迭代一次（键），循环体加一"
    );
}

#[test]
fn iterator_types_match_the_probe_table() {
    let vm = Vm::new();
    for name in [
        "tuple_iterator",
        "list_iterator",
        "str_ascii_iterator",
        "dict_keyiterator",
        "set_iterator",
    ] {
        let entry = builtin_type(name).unwrap_or_else(|| panic!("探测表里应当有 {name}"));
        let ty = vm
            .instance
            .type_named(name)
            .unwrap_or_else(|| panic!("{name} 应当已注册"));
        // SAFETY: ty 由注册表持有。
        let bases: Vec<&str> = unsafe { ty.as_ref() }
            .bases()
            .iter()
            .map(|base| {
                // SAFETY: 同上。
                unsafe { base.as_ref() }.name()
            })
            .collect();
        assert_eq!(bases, entry.bases, "TS-41：{name} 的 __bases__");
    }
}

#[test]
fn iterating_a_non_iterable_is_reported() {
    // 迭代协议接线后，没有 `__iter__` 的对象报的是**脚本异常**（照参照实测的消息），
    // 不再是 VM 级的 `Unsupported`
    let vm = Vm::new();
    let code = vm.code(
        2,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("GET_ITER"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(1))],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "TypeError");
    assert_eq!(message.as_deref(), Some("'int' object is not iterable"));
}

#[test]
fn swap_and_copy_follow_the_reference() {
    // 真实 3.14 代码里推导式离不开这两条（§10 表外的"增量补齐"）
    let vm = Vm::new();
    let consts = vec![
        Some(vm.constant(1)),
        Some(vm.constant(2)),
        Some(vm.constant(3)),
    ];
    // 压 1、2、3 ⇒ SWAP 2 之后 TOS 是 2；COPY 2 复制 1；最后返回 TOS（应当是 1）
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("SWAP"), 2), // [1, 3, 2]
            (op("POP_TOP"), 0), // [1, 3]
            (op("COPY"), 2), // [1, 3, 1]
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(1), &vm.instance),
        "COPY(2) 复制的是 TOS[-2]（也就是最底下那个 1）"
    );
}
