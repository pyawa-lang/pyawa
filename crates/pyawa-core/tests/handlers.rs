//! 异常族后半：**处理块派发**（`BC-60` ①，判据 `T-BC-22`）。
//!
//! 栈形状是**实测**出来的（本机 3.14.4 的发射骨架）：
//!
//! - 异常表的 `target` 指向处理块入口，入口第一条就是 `PUSH_EXC_INFO`
//! - `depth` 是**回退到的目标栈深**（不是"弹几个"）；`lasti` 置位时先压"最后一条指令偏移"
//! - 处理块：`PUSH_EXC_INFO` → 取类 → `CHECK_EXC_MATCH` → `POP_JUMP_IF_FALSE` →
//!   命中则 `POP_TOP`（丢掉异常）＋ 处理体 ＋ `POP_EXCEPT`；没命中则 `RERAISE 0`
//! - `RERAISE n`：先弹 `n` 个额外值（通常是 `lasti`），再抛 TOS

mod common;

use std::ptr::NonNull;

use pyawa_core::{ExecError, Header, Value};

use common::{assemble_labeled, op, varint, Item, Vm};

fn type_header(vm: &Vm, name: &str) -> NonNull<Header> {
    let ty = vm.instance.type_named(name).unwrap();
    let header = ty.cast::<Header>();
    // SAFETY: ty 由注册表持有，存活。
    unsafe { vm.instance.incref_object(header.as_ptr()) };
    header
}

/// 造一段"`try: raise …` ＋ 处理块"的程序，返回（码元，异常表，常量表）。
fn try_program(
    vm: &Vm,
    handler_class: &str,
    on_match: i64,
) -> (Vec<u8>, Vec<u8>, Vec<Option<NonNull<Header>>>) {
    let raised = type_header(vm, "ValueError");
    let caught = type_header(vm, handler_class);
    let consts = vec![
        Some(raised),
        Some(caught),
        Some(vm.constant(on_match)),
        // depth 用例要用的额外值（下面按需 push）
        Some(vm.constant(7)),
    ];

    let items = vec![
        Item::Instr(op("RESUME"), 0),
        Item::Label("try_start"),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("RAISE_VARARGS"), 1),
        Item::Label("try_end"),
        Item::Label("handler"),
        Item::Instr(op("PUSH_EXC_INFO"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("CHECK_EXC_MATCH"), 0),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "reraise"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("POP_EXCEPT"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("reraise"),
        Item::Instr(op("RERAISE"), 0),
    ];
    let (bytes, labels) = assemble_labeled(&items);
    let offset_of = |name: &str| labels.iter().find(|(label, _)| *label == name).unwrap().1 / 2;
    let mut table = Vec::new();
    let start = offset_of("try_start");
    let entry = offset_of("handler");
    // 条目：起点、长度、目标（都是**码元**）、depth<<1|lasti
    varint(start, &mut table);
    varint(offset_of("try_end") - start, &mut table);
    varint(entry, &mut table);
    varint(0, &mut table);
    (bytes, table, consts)
}

#[test]
fn handler_catches_and_runs_the_body() {
    let vm = Vm::new();
    let (bytes, table, consts) = try_program(&vm, "ValueError", 42);
    let code = vm.try_code(8, 0, Vec::new(), bytes, consts, table);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(42), &vm.instance),
        "命中的处理体应当返回 42"
    );
    assert!(
        vm.instance.current_exception().is_none(),
        "POP_EXCEPT 之后当前异常应当还原为空"
    );
}

#[test]
fn non_matching_handler_reraises() {
    let vm = Vm::new();
    // 抛的是 ValueError，但处理块只接 TypeError ⇒ 走 RERAISE
    let (bytes, table, consts) = try_program(&vm, "TypeError", 42);
    let code = vm.try_code(8, 0, Vec::new(), bytes, consts, table);
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception().map(|(name, _)| name),
        Some("ValueError".to_owned()),
        "没命中就原样重抛"
    );
}

#[test]
fn depth_unwinds_the_stack_to_the_recorded_depth() {
    // 实测：`depth` 是**回退到的目标栈深**。这里在 try 体里多压一个值（栈深 1），
    // 异常表写 depth = 1 ⇒ 派发后处理块看到的栈里应当还留着那个值。
    let vm = Vm::new();
    let raised = type_header(&vm, "ValueError");
    let caught = type_header(&vm, "ValueError");
    let consts = vec![
        Some(raised),
        Some(caught),
        Some(vm.constant(7)),
    ];
    let items = vec![
        Item::Instr(op("RESUME"), 0),
        Item::Label("try_start"),
        Item::Instr(op("LOAD_CONST"), 2), // 额外压一个 7（栈深 1）
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("RAISE_VARARGS"), 1),
        Item::Label("try_end"),
        Item::Label("handler"),
        Item::Instr(op("PUSH_EXC_INFO"), 0), // 之后栈：[7, None, 异常]
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("CHECK_EXC_MATCH"), 0),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "reraise"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("POP_EXCEPT"), 0),
        // 现在栈：[7] ⇒ 直接返回它，证明 depth 把 7 留下来了
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("reraise"),
        Item::Instr(op("RERAISE"), 0),
    ];
    let (bytes, labels) = assemble_labeled(&items);
    let offset_of = |name: &str| labels.iter().find(|(label, _)| *label == name).unwrap().1 / 2;
    let mut table = Vec::new();
    let start = offset_of("try_start");
    varint(start, &mut table);
    varint(offset_of("try_end") - start, &mut table);
    varint(offset_of("handler"), &mut table);
    varint(1 << 1, &mut table); // depth = 1、lasti = 0
    let code = vm.try_code(8, 0, Vec::new(), bytes, consts, table);

    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(7), &vm.instance),
        "depth = 1 要求把多压的那个 7 留着"
    );
}

#[test]
fn lasti_bit_pushes_the_instruction_offset() {
    // lasti 置位时，派发会先压"最后一条指令偏移"（本层按**码元**压，见 executor 注释）
    let vm = Vm::new();
    let raised = type_header(&vm, "ValueError");
    let caught = type_header(&vm, "ValueError");
    let consts = vec![Some(raised), Some(caught), Some(vm.constant(9))];
    let items = vec![
        Item::Instr(op("RESUME"), 0),
        Item::Label("try_start"),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("RAISE_VARARGS"), 1),
        Item::Label("try_end"),
        Item::Label("handler"),
        Item::Instr(op("PUSH_EXC_INFO"), 0), // 之后栈：[lasti, None, 异常]
        Item::Instr(op("POP_TOP"), 0),       // 处理位：先丢掉异常
        Item::Instr(op("POP_EXCEPT"), 0),    // 再还原异常状态（它会吃掉那层 None）
        Item::Instr(op("RETURN_VALUE"), 0),  // 返回 lasti（一个整数）
    ];
    let (bytes, labels) = assemble_labeled(&items);
    let offset_of = |name: &str| labels.iter().find(|(label, _)| *label == name).unwrap().1 / 2;
    let mut table = Vec::new();
    let start = offset_of("try_start");
    varint(start, &mut table);
    varint(offset_of("try_end") - start, &mut table);
    varint(offset_of("handler"), &mut table);
    varint((0 << 1) | 1, &mut table); // depth = 0、lasti = 1
    let code = vm.try_code(8, 0, Vec::new(), bytes, consts, table);

    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当是整数对象");
    // SAFETY: raw 是存活对象。
    let text = unsafe { &*raw.as_ptr().cast::<pyawa_core::IntObject>() }.value;
    assert!(
        text > 0,
        "lasti 应当是一条指令的偏移（正数），实际 {text}"
    );
}
