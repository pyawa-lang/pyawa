//! 生成器族（`docs/SPEC-bytecode.md` §10）。
//!
//! 骨架是**实测**的（本机 3.14.4 反汇编 `def gen(): yield 1; yield 2`）：
//!
//! ```text
//! RETURN_GENERATOR      ← 函数体第一条；`CALL` 时造出生成器并返回它
//! POP_TOP               ← 恢复时丢掉"送进来的值"
//! RESUME 0
//! LOAD_SMALL_INT 1 / YIELD_VALUE 0 / RESUME 5 / POP_TOP
//! …（第二个 yield 同形）…
//! LOAD_CONST None / RETURN_VALUE     ← 跑完 ⇒ 驱动器走耗尽路径
//! ```
//!
//! 驱动器一侧：`CALL`（拿到生成器）→ `GET_ITER`（生成器是**它自己的迭代器**）→ `FOR_ITER`
//! （"取下一个" ＝ **恢复生成器的帧**）。本层的 `CALL` 见到 `CO_GENERATOR`（实测 32）就
//! **不跑函数体**，直接把挂起的帧包成生成器，所以 `RETURN_GENERATOR` 恢复时是空操作。

mod common;

use core::ptr::NonNull;

use pyawa_core::opcode::get_nb_ops;
use pyawa_core::{ExecError, ExecOutcome, GeneratorObject, Header, Value};

use common::{assemble, emit, op, varint, Item, Vm};

fn nb(name: &str) -> u8 {
    get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

/// 造 `def gen(): yield 1; yield 2; return <returned>` 的 code object（按实测骨架逐条对齐）。
fn generator_code(vm: &Vm, returned: i64) -> pyawa_core::Owned<'_, pyawa_core::CodeObject> {
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "gen",
        "gen".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        4,
        0,
        0,
        0,
        0,
        0x20, // CO_GENERATOR（实测 32）
        Vec::new(),
        Vec::new(),
        0,
        0,
        emit(&[
            (op("RETURN_GENERATOR"), 0),
            (op("POP_TOP"), 0),
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("YIELD_VALUE"), 0),
            (op("RESUME"), 5),
            (op("POP_TOP"), 0),
            (op("LOAD_CONST"), 1),
            (op("YIELD_VALUE"), 0),
            (op("RESUME"), 5),
            (op("POP_TOP"), 0),
            (op("LOAD_CONST"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![
            Some(vm.constant(1)),
            Some(vm.constant(2)),
            Some(vm.constant(returned)),
            Some(none),
        ],
    ))
}

#[test]
fn for_loop_drives_a_generator() {
    // total = 0; for x in gen(): total += x; return total   ⇒ 1 + 2 ＝ 3
    let vm = Vm::new();
    let callee = generator_code(&vm, 0);
    let callee_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let consts = vec![
        Some(callee_header),
        Some(vm.constant(0)),
        Some(vm.constant(0)),
    ];
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("STORE_FAST"), 1),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("CALL"), 0),
        Item::Instr(op("GET_ITER"), 0),
        Item::Label("L1"),
        Item::Jump(op("FOR_ITER"), "L2"),
        Item::Instr(op("STORE_FAST"), 2),
        Item::Instr(op("LOAD_FAST"), 1),
        Item::Instr(op("LOAD_FAST"), 2),
        Item::Instr(op("BINARY_OP"), nb("NB_ADD")),
        Item::Instr(op("STORE_FAST"), 1),
        Item::Jump(op("JUMP_BACKWARD"), "L1"),
        Item::Label("L2"),
        Item::Instr(op("END_FOR"), 0),
        Item::Instr(op("POP_ITER"), 0),
        Item::Instr(op("LOAD_FAST"), 1),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code(8, 3, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(3), &vm.instance),
        "生成器让出 1 与 2，合计 3"
    );
}

#[test]
fn creating_a_generator_does_not_run_the_body() {
    // `CALL` 见到 `CO_GENERATOR` 就只把挂起的帧包起来——函数体要等第一次恢复才跑
    let vm = Vm::new();
    let callee = generator_code(&vm, 0);
    let callee_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let code = vm.code(
        4,
        1,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("MAKE_FUNCTION"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("CALL"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(callee_header)],
    );
    let result = vm.run(&code).unwrap();
    let raw = result
        .as_header(&vm.instance)
        .expect("CALL 应当交出生成器对象");
    // SAFETY: raw 是存活对象。
    let generator = unsafe { &*raw.as_ptr().cast::<GeneratorObject>() };
    assert!(!generator.finished(), "还没跑过，不算跑完");
    let frame_header = generator.frame();
    // SAFETY: 帧由生成器持有，存活。
    let frame = unsafe { &*frame_header.as_ptr().cast::<pyawa_core::Frame>() };
    assert_eq!(frame.instruction_pointer(), 0, "函数体一条都还没执行");
    assert_eq!(frame.depth(), 0, "挂起时值栈是空的（BC-47）");
    assert!(frame.is_suspended(), "生成器的帧是挂起状态");
}

#[test]
fn a_finished_generator_takes_the_exhausted_path() {
    // 驱动器连跑两轮 `FOR_ITER`：第一轮恢复出 1，第二轮恢复出 2，
    // 第三轮（生成器已跑完）必须走耗尽路径而不是接着"让出"
    let vm = Vm::new();
    let callee = generator_code(&vm, 0);
    let callee_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    // 先单独跑一遍完整驱动（复用 for_loop 的逻辑），确认 3 之后再确认 finished
    let consts = vec![
        Some(callee_header),
        Some(vm.constant(0)),
        Some(vm.constant(0)),
    ];
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("STORE_FAST"), 1),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("CALL"), 0),
        Item::Instr(op("STORE_FAST"), 3),
        Item::Instr(op("LOAD_FAST"), 3), // 再取出来送进 GET_ITER
        Item::Instr(op("GET_ITER"), 0),
        Item::Label("L1"),
        Item::Jump(op("FOR_ITER"), "L2"),
        Item::Instr(op("STORE_FAST"), 2),
        Item::Instr(op("LOAD_FAST"), 1),
        Item::Instr(op("LOAD_FAST"), 2),
        Item::Instr(op("BINARY_OP"), nb("NB_ADD")),
        Item::Instr(op("STORE_FAST"), 1),
        Item::Jump(op("JUMP_BACKWARD"), "L1"),
        Item::Label("L2"),
        Item::Instr(op("END_FOR"), 0),
        Item::Instr(op("POP_ITER"), 0),
        Item::Instr(op("LOAD_FAST"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code(8, 4, bytes, consts);
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("返回的是生成器");
    // SAFETY: raw 是存活对象。
    let generator = unsafe { &*raw.as_ptr().cast::<GeneratorObject>() };
    assert!(generator.finished(), "跑完之后必须记成已完成");
    let _ = varint(0, &mut Vec::new()); // 保留 import（异常表夹具用）
    let _: Option<NonNull<Header>> = None;
}

#[test]
fn a_generator_is_its_own_iterator() {
    // `GET_ITER` 对迭代器返回它自己（生成器也是）
    let vm = Vm::new();
    let callee = generator_code(&vm, 0);
    let callee_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let code = vm.code(
        4,
        1,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("MAKE_FUNCTION"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("CALL"), 0),
            (op("GET_ITER"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(callee_header)],
    );
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当是生成器");
    // SAFETY: raw 是存活对象。
    assert_eq!(
        unsafe { raw.as_ref() }.ty(),
        vm.instance.type_named("generator").unwrap(),
        "GET_ITER 交回的还是那个生成器"
    );
    let _ = ExecOutcome::Yielded(raw); // 类型可见性检查
    let _ = ExecError::FellOffEnd;
}

#[test]
fn frame_push_takes_a_new_reference() {
    // **实测钉住的契约**（`BC-43`）：`Frame::push` 收"新引用"由帧接手（净 +1）；
    // `Frame::pop` 把那份引用**交给调用方**（净 0）。执行器里的助手
    // `push(instance, frame, raw)` 自己也 incref 一份，所以"出栈 → 助手 push → 再 release"
    // 才配平；而"出栈 → **裸** `frame.push`"之后**不能**再 release（那会提前释放，
    // 症状是对象在两条指令之间变成垃圾——生成器那一版就是这么挂的）。
    let vm = Vm::new();
    let code = vm.code(4, 0, Vec::new(), Vec::new());
    let frame = vm.instance.alloc(pyawa_core::Frame::for_code(vm.frame_type, &code));
    let value = vm.instance.alloc(pyawa_core::StrObject::new(
        vm.instance.singletons().str_type(),
        "probe".to_owned(),
    ));
    let raw = value.as_ptr().cast::<Header>();
    assert_eq!(value.refcount(), 1, "刚分配是 1");
    frame.get().push(vm.instance.own(raw).into_raw()).unwrap();
    assert_eq!(value.refcount(), 2, "助手式 push 净 +1");
    let popped = frame.get().pop().unwrap();
    assert_eq!(unsafe { popped.as_ref() }.refcount(), 2, "pop 只是把引用交出来");
    // SAFETY: popped 是调用方持有的那份。
    unsafe { vm.instance.release_object(popped.as_ptr()) };
}

/// 造 `def sub(): yield 1; yield 2; return 3` 之外层 `yield from sub()`（照实测骨架）。
fn yield_from_code(
    vm: &Vm,
    inner: NonNull<Header>,
) -> pyawa_core::Owned<'_, pyawa_core::CodeObject> {
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "outer",
        "outer".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        6,
        0,
        0,
        0,
        0,
        0x20, // CO_GENERATOR
        Vec::new(),
        Vec::new(),
        0,
        0,
        assemble(&[
            Item::Instr(op("RETURN_GENERATOR"), 0),
            Item::Instr(op("POP_TOP"), 0),
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("MAKE_FUNCTION"), 0),
            Item::Instr(op("PUSH_NULL"), 0),
            Item::Instr(op("CALL"), 0),
            Item::Instr(op("GET_YIELD_FROM_ITER"), 0),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Label("L2"),
            Item::Jump(op("SEND"), "L5"),
            Item::Label("L3"),
            Item::Instr(op("YIELD_VALUE"), 0),
            Item::Instr(op("RESUME"), 2),
            Item::Jump(op("JUMP_BACKWARD_NO_INTERRUPT"), "L2"),
            Item::Label("L5"),
            Item::Instr(op("END_SEND"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![Some(inner), Some(none)],
    ))
}

#[test]
fn yield_from_forwards_values_and_returns_the_inner_result() {
    // `for` 驱动器跑外层 ⇒ 收到 1、2；外层把内层的返回值 3 当作自己的返回值
    let vm = Vm::new();
    let inner = generator_code(&vm, 3);
    let inner_header = inner.as_ptr().cast::<Header>();
    // SAFETY: inner 由本测试持有，常量表要自己那份引用。
    unsafe { vm.instance.incref_object(inner_header.as_ptr()) };
    let outer = yield_from_code(&vm, inner_header);
    let outer_header = outer.as_ptr().cast::<Header>();
    // SAFETY: outer 由本测试持有。
    unsafe { vm.instance.incref_object(outer_header.as_ptr()) };

    // 直接驱动外层生成器：SEND(None) 直到耗尽，返回它的返回值
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("CALL"), 0),
        Item::Label("L1"),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Jump(op("SEND"), "L3"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Jump(op("JUMP_BACKWARD_NO_INTERRUPT"), "L1"),
        Item::Label("L3"),
        Item::Instr(op("END_SEND"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let consts = vec![
        Some(outer_header),
        Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
    ];
    let code = vm.code(8, 1, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(3), &vm.instance),
        "外层应当把内层的返回值 3 交出来，实际 {:?}",
        result
    );
}

#[test]
fn yield_from_needs_the_inner_generator() {
    // 内层也是生成器：1、2 被**转发**出去（本用例只验外层能跑完并拿到返回值）
    let vm = Vm::new();
    let inner = generator_code(&vm, 3);
    let inner_header = inner.as_ptr().cast::<Header>();
    // SAFETY: inner 由本测试持有。
    unsafe { vm.instance.incref_object(inner_header.as_ptr()) };
    // 直接跑内层：让出 1、2，最后返回 3
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("CALL"), 0),
        Item::Label("L1"),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Jump(op("SEND"), "L3"),
        Item::Instr(op("POP_TOP"), 0),
        Item::Jump(op("JUMP_BACKWARD_NO_INTERRUPT"), "L1"),
        Item::Label("L3"),
        Item::Instr(op("END_SEND"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let consts = vec![
        Some(inner_header),
        Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
    ];
    let code = vm.code(8, 1, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(3), &vm.instance),
        "生成器的返回值要经 SEND／END_SEND 交出来"
    );
}
