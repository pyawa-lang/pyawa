//! 内建指令（`CALL_INTRINSIC_1`）与普通迭代器的 `SEND`（`docs/SPEC-bytecode.md` §10）。
//!
//! 实测：
//!
//! ```text
//! +a          → CALL_INTRINSIC_1 5 (INTRINSIC_UNARY_POSITIVE)     净 0
//! (*[1,2],)   → CALL_INTRINSIC_1 6 (INTRINSIC_LIST_TO_TUPLE)      净 0
//! yield from [1,2] 的 SEND 那条：接收者不是生成器 ⇒ 走"取下一个"
//! 生成器里漏出 StopIteration ⇒ RuntimeError: generator raised StopIteration（实测原话）
//! ```
//!
//! 编号到名字的对应来自 `opcode_metadata::INTRINSIC1_DESCS`（探测产物，`BC-38`），
//! 分派**按名字**（`BC-50`）——不认识的编号如实报未接线，不猜语义。

mod common;

use core::cell::RefCell;

use pyawa_core::{Header, Value};

use common::{assemble, emit, op, Item, Vm};

fn intrinsic(name: &str) -> u8 {
    pyawa_core::opcode_metadata::INTRINSIC1_DESCS
        .iter()
        .position(|candidate| *candidate == name)
        .unwrap_or_else(|| panic!("内建表里没有 {name}")) as u8
}

#[test]
fn unary_positive_leaves_the_operand() {
    let vm = Vm::new();
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("CALL_INTRINSIC_1"), intrinsic("INTRINSIC_UNARY_POSITIVE")),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(7))],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(7), &vm.instance), "+7 就是 7");
}

#[test]
fn list_to_tuple_converts_in_place() {
    let vm = Vm::new();
    let list_type = vm.instance.type_named("list").unwrap();
    let list = vm.instance.alloc(pyawa_core::ListObject::new(
        list_type,
        RefCell::new(Vec::new()),
    ));
    list.get().append(vm.constant(1));
    list.get().append(vm.constant(2));
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("CALL_INTRINSIC_1"), intrinsic("INTRINSIC_LIST_TO_TUPLE")),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(list.into_raw().cast::<Header>())],
    );
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当是元组");
    // SAFETY: raw 是存活对象。
    assert_eq!(
        unsafe { raw.as_ref() }.ty(),
        vm.instance.type_named("tuple").unwrap(),
        "列表换成了元组"
    );
    // SAFETY: 类型身份已确认。
    let tuple = unsafe { &*raw.as_ptr().cast::<pyawa_core::TupleObject>() };
    assert_eq!(tuple.len(), 2);
}

#[test]
fn stopiteration_error_becomes_a_runtime_error() {
    // 生成器里漏出的 StopIteration ⇒ RuntimeError（实测消息）
    let vm = Vm::new();
    let stop = vm.instance.type_named("StopIteration").unwrap();
    let exception = vm.instance.alloc(pyawa_core::ExceptionObject::new(
        stop,
        RefCell::new(Vec::new()),
        RefCell::new(None),
        RefCell::new(None),
        core::cell::Cell::new(false),
    ));
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("CALL_INTRINSIC_1"), intrinsic("INTRINSIC_STOPITERATION_ERROR")),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(exception.into_raw().cast::<Header>())],
    );
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当是异常实例");
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    // SAFETY: 同上。
    assert_eq!(unsafe { ty.as_ref() }.name(), "RuntimeError");
    // SAFETY: 类型身份已确认。
    let object = unsafe { &*raw.as_ptr().cast::<pyawa_core::ExceptionObject>() };
    assert_eq!(
        object.message_with(&vm.instance).as_deref(),
        Some("generator raised StopIteration")
    );
}

#[test]
fn send_drives_a_plain_iterator() {
    // `yield from [1, 2]` 的 SEND 循环：接收者是**普通迭代器**时走"取下一个"
    let vm = Vm::new();
    let list_type = vm.instance.type_named("list").unwrap();
    let list = vm.instance.alloc(pyawa_core::ListObject::new(
        list_type,
        RefCell::new(Vec::new()),
    ));
    list.get().append(vm.constant(1));
    list.get().append(vm.constant(2));
    let list_raw = list.into_raw().cast::<Header>();
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();

    // 把可迭代对象变成迭代器，然后 SEND(None) 三次：1、2，第三次耗尽走返回值那条
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("GET_YIELD_FROM_ITER"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        // 第一轮
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Jump(op("SEND"), "L_after1"),
        Item::Label("L1"),
        Item::Instr(op("STORE_FAST"), 1), // 存第一个值
        // 第二轮
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Jump(op("SEND"), "L_after2"),
        Item::Label("L2"),
        Item::Instr(op("STORE_FAST"), 2), // 存第二个值
        // 第三轮：耗尽
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Jump(op("SEND"), "L_after3"),
        Item::Label("L3"),
        Item::Instr(op("RETURN_VALUE"), 0), // 第三个让出值（耗尽时压的 None）
        Item::Label("L_after1"),
        Item::Instr(op("END_SEND"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("L_after2"),
        Item::Instr(op("END_SEND"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("L_after3"),
        Item::Instr(op("END_SEND"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code(
        8,
        3,
        bytes,
        vec![Some(list_raw), Some(none)],
    );
    let result = vm.run(&code).unwrap();
    // 第三条 SEND 已经耗尽 ⇒ 交出 None
    assert!(
        result.is_same(&Value::None, &vm.instance),
        "耗尽那条压的是 None，实际 {result:?}"
    );
}
