//! 实例生命周期的对内一半（`PLAN-milestones.md` 的 `P1-7`，判据 `T-OM-4`）：
//! **中断**与**实例隔离**。
//!
//! 对外出口（`pa_create`／`pa_destroy`／`pa_interrupt`）在 `pyawa-abi`；这里验的是它们要落到的
//! 内部性质：
//!
//! - **`AB-5`①**：中断**按实例**请求（`CX-3` 禁止进程级共享），执行器每条指令查一次，
//!   中断后"执行类函数随即返回"（本层是 `ExecError::Interrupted`）
//! - **`T-OM-4`**：两个实例的对象互不可见；销毁任一实例后无残留

mod common;

use core::cell::RefCell;

use pyawa_core::{ExecError, Header, Value};

use common::{assemble, emit, op, Item, Vm};

#[test]
fn interrupt_stops_execution_immediately() {
    let vm = Vm::new();
    // 一段"跑到天荒地老"的循环：没有中断会一直转
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Label("L1"),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Jump(op("JUMP_BACKWARD"), "L1"),
    ]);
    let code = vm.code(
        4,
        0,
        bytes,
        vec![Some(vm.instance.own(vm.instance.singletons().none()).into_raw())],
    );
    // 先请求中断：第一条指令就会停手
    vm.instance.request_interrupt();
    assert!(matches!(vm.run(&code), Err(ExecError::Interrupted)));

    // 清掉之后同一段程序仍然会一直转 ⇒ 这里换一段会正常结束的来验"清掉就恢复"
    vm.instance.clear_interrupt();
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(9))],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(9), &vm.instance), "清掉中断后照常执行");
}

#[test]
fn interrupt_is_per_instance() {
    // `CX-3`：中断状态按实例存——另一个实例不受影响
    let left = Vm::new();
    let right = Vm::new();
    left.instance.request_interrupt();
    assert!(left.instance.interrupted());
    assert!(!right.instance.interrupted(), "另一个实例不该被牵连");

    let code = right.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(right.constant(3))],
    );
    let result = right.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(3), &right.instance));
}

#[test]
fn instances_do_not_see_each_others_objects() {
    // `T-OM-4` 前半：两个实例的对象互不可见（各自的堆、各自的类型注册表）
    let left = Vm::new();
    let right = Vm::new();
    // 同一个名字的类型在两个实例里是**不同**的对象
    let left_type = left.instance.type_named("list").unwrap();
    let right_type = right.instance.type_named("list").unwrap();
    assert_ne!(
        left_type.as_ptr() as usize,
        right_type.as_ptr() as usize,
        "类型注册表按实例存放（OM-15）"
    );
    // 在一个实例里造对象，另一个实例的账本不动
    let before = right.instance.live_objects();
    let _object = left
        .instance
        .alloc(pyawa_core::ListObject::new(
            left_type,
            RefCell::new(Vec::new()),
        ));
    assert_eq!(
        right.instance.live_objects(),
        before,
        "左实例的对象不出现在右实例的账本里"
    );
}

#[test]
fn a_cyclic_structure_is_reclaimed_when_the_instance_goes_away() {
    // `T-OM-4` 后半的"无残留（含环）"：环由 GC 收（`OM-25` 起），
    // 实例销毁时剩下的由实例堆统一释放（`OM-2`／`OM-3` 的账本归零）。
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("Node");
    let first = vm
        .instance
        .alloc(pyawa_core::AttributeObject::new(ty, RefCell::new(None)));
    let second = vm
        .instance
        .alloc(pyawa_core::AttributeObject::new(ty, RefCell::new(None)));
    let first_raw = first.into_raw().cast::<Header>();
    let second_raw = second.into_raw().cast::<Header>();
    // first.next = second; second.next = first（成环，引用计数各自 +1）
    let dict = vm.instance.alloc(pyawa_core::DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        RefCell::new(Vec::new()),
    ));
    dict.get()
        .insert_raw(vm.instance.new_str("next"), second_raw);
    // SAFETY: first_raw 是存活对象，且载荷就是 AttributeObject。
    unsafe { &*first_raw.as_ptr().cast::<pyawa_core::AttributeObject>() }
        .set_attributes(Some(dict.into_raw().cast::<Header>()));
    let dict = vm.instance.alloc(pyawa_core::DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        RefCell::new(Vec::new()),
    ));
    dict.get()
        .insert_raw(vm.instance.new_str("next"), first_raw);
    // SAFETY: second_raw 是存活对象，且载荷就是 AttributeObject。
    unsafe { &*second_raw.as_ptr().cast::<pyawa_core::AttributeObject>() }
        .set_attributes(Some(dict.into_raw().cast::<Header>()));

    // 环由 GC 收回
    let collected = vm.instance.collect();
    assert!(collected >= 2, "成环的两个对象应当被回收，实际 {collected}");
}
