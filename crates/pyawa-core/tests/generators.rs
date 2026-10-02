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
        Vec::new(),
        Vec::new(),
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
    
    Vec::new(),))
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
        Vec::new(),
        Vec::new(),
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
    
    Vec::new(),))
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

// ---- 生成器方法族：`send`／`__next__`（`§10` 生成器与协程族）----

/// 走**真路径**调用生成器的方法：`gen.send(值)` ＝ `LOAD 生成器; LOAD_ATTR send(+方法位); 实参; CALL`。
#[allow(clippy::type_complexity)]
fn call_method<'a>(
    vm: &'a Vm,
    generator: NonNull<Header>,
    method: &str,
    argument: Option<i64>,
) -> Result<(Option<Value<'a>>, Option<(String, String)>), ExecError> {
    let mut consts = vec![Some(generator)];
    let mut items = vec![
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        // `LOAD_ATTR` 的 oparg：名字下标 << 1 | 取方法位（`names` 只有一项 ⇒ 下标 0）
        Item::Instr(op("LOAD_ATTR"), 0 << 1 | 1),
    ];
    let argument_count = if let Some(value) = argument {
        consts.push(Some(vm.constant(value)));
        items.push(Item::Instr(op("LOAD_CONST"), 1));
        1
    } else {
        0
    };
    items.push(Item::Instr(op("CALL"), argument_count));
    items.push(Item::Instr(op("RETURN_VALUE"), 0));
    // 生成器的常量表要自己那份
    // SAFETY: generator 由调用方持有。
    unsafe { vm.instance.incref_object(generator.as_ptr()) };
    let code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec![method.to_owned()],
        assemble(&items),
        consts,
    );
    assert_eq!(code.get().name_at(0), Some(method));
    match vm.run(&code) {
        Ok(value) => Ok((Some(value), None)),
        Err(ExecError::Raised { exception }) => {
            // 生成器方法**直接返回** `Err(Raised)`（不像 `RAISE_VARARGS` 那样写进实例状态），
            // 所以这里从异常对象本身取类型名与 `str(e)`
            let type_name = vm.instance.type_name(vm.instance.type_of(exception));
            let text = vm.instance.object_str(exception);
            // SAFETY: exception 是本实例的存活对象，这里归还一份引用。
            unsafe { vm.instance.release_object(exception.as_ptr()) };
            Ok((None, Some((type_name, text))))
        }
        Err(other) => Err(other),
    }
}

/// 拿到一个生成器（`CALL` 见到 `CO_GENERATOR` 只包帧，不跑函数体）。
fn make_generator(vm: &Vm, returned: i64) -> core::ptr::NonNull<pyawa_core::Header> {
    let code = generator_code(vm, returned);
    let code_header = code.as_ptr().cast::<Header>();
    // SAFETY: code 由本测试持有。
    unsafe { vm.instance.incref_object(code_header.as_ptr()) };
    let function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        code_header,
        Vec::new(),
        None,
        core::cell::RefCell::new(None),
            core::cell::RefCell::new(None)));
    let function = function.into_raw().cast::<Header>();
    pyawa_core::call_value(&vm.instance, function, &[], &[]).expect("造生成器应当成功")
}

#[test]
fn generator_send_and_next_match_the_reference() {
    let vm = Vm::new();

    // ① `next()` 拿到第一个让出值
    let generator = make_generator(&vm, 3);
    let (first, raised) = call_method(&vm, generator, "__next__", None).expect("应当跑通");
    assert!(raised.is_none(), "第一次不该报错");
    let first = first.expect("第一次应当有值");
    assert_eq!(
        vm.instance
            .int_value(first.as_header(&vm.instance).expect("是具体对象"))
            .expect("是整数"),
        1,
        "`next()` 第一次让出 1"
    );

    // ② `send(42)` 返回下一个让出值
    let (second, raised) = call_method(&vm, generator, "send", Some(42)).expect("应当跑通");
    assert!(raised.is_none(), "送值不该报错");
    let second = second.expect("第二次应当有值");
    assert_eq!(
        vm.instance
            .int_value(second.as_header(&vm.instance).expect("是具体对象"))
            .expect("是整数"),
        2,
        "`send(42)` 让出 2"
    );

    // ③ 跑完抛 `StopIteration`，且**返回值在实参里**（`str(e)` ＝ "3"）
    let (exhausted, raised) = call_method(&vm, generator, "__next__", None).expect("应当跑通");
    assert!(exhausted.is_none(), "跑完该抛异常");
    let (type_name, text) = raised.expect("应当抛异常");
    assert_eq!(type_name, "StopIteration");
    assert_eq!(text, "3", "返回值进 `StopIteration` 的实参（`str(e)` 就是它）");
}

#[test]
fn send_refuses_a_non_none_value_on_a_just_started_generator() {
    let vm = Vm::new();
    let generator = make_generator(&vm, 3);
    let (outcome, raised) = call_method(&vm, generator, "send", Some(42)).expect("应当跑通");
    assert!(outcome.is_none(), "刚创建的生成器只接受 `send(None)`");
    let (type_name, text) = raised.expect("应当抛异常");
    assert_eq!(type_name, "TypeError");
    assert_eq!(text, "can't send non-None value to a just-started generator");
}

// ---- `throw`／`close`（恢复时先抛）----

/// 造一个"让出一次、`try/except` 接住异常、再让出"的生成器。
///
/// 骨架照参照实测：
/// ```text
/// RETURN_GENERATOR; POP_TOP; RESUME; LOAD_CONST 1; YIELD_VALUE; RESUME; POP_TOP;
/// LOAD_CONST 2; YIELD_VALUE; RESUME; POP_TOP; LOAD_CONST 3; RETURN_VALUE
/// 处理块：PUSH_EXC_INFO; …; POP_EXCEPT; POP_TOP; LOAD_CONST 4; YIELD_VALUE; …
/// ```
fn catching_generator_code(vm: &Vm) -> pyawa_core::Owned<'_, pyawa_core::CodeObject> {
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    let mut items = vec![
        Item::Instr(op("RETURN_GENERATOR"), 0),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("RESUME"), 0),
        Item::Label("body_start"),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("YIELD_VALUE"), 0),
        Item::Instr(op("RESUME"), 5),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("YIELD_VALUE"), 0),
        Item::Instr(op("RESUME"), 5),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
        Item::Label("handler"),
        Item::Instr(op("PUSH_EXC_INFO"), 0),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("POP_EXCEPT"), 0),
        Item::Instr(op("LOAD_CONST"), 4),
        Item::Instr(op("YIELD_VALUE"), 0),
        Item::Instr(op("RESUME"), 5),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
    ];
    let (bytes, labels) = common::assemble_labeled(&items);
    let offset_of = |name: &str| labels.iter().find(|(label, _)| *label == name).unwrap().1 / 2;
    let start = offset_of("body_start");
    let mut table = Vec::new();
    varint(start, &mut table);
    varint(offset_of("handler") - start, &mut table);
    varint(offset_of("handler"), &mut table);
    // `depth` 0、`lasti` 1（编码是 `depth << 1 | lasti`）
    varint(1, &mut table);
    let _ = &mut items;
    vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "catcher",
        "catcher".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        8,
        0,
        0,
        0,
        0,
        0x20, // CO_GENERATOR
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        bytes,
        table,
        vec![
            Some(vm.constant(1)),
            Some(vm.constant(2)),
            Some(vm.constant(3)),
            Some(none),
            Some(vm.constant(5)), // 处理块里让出的标记值（下标 4）
        ],
    
    Vec::new(),))
}

fn call_with_code(
    vm: &Vm,
    code: pyawa_core::Owned<'_, pyawa_core::CodeObject>,
) -> NonNull<Header> {
    let code_header = code.as_ptr().cast::<Header>();
    // SAFETY: code 由调用方持有。
    unsafe { vm.instance.incref_object(code_header.as_ptr()) };
    let function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        code_header,
        Vec::new(),
        None,
        core::cell::RefCell::new(None),
            core::cell::RefCell::new(None)));
    let function = function.into_raw().cast::<Header>();
    pyawa_core::call_value(&vm.instance, function, &[], &[]).expect("造生成器应当成功")
}

fn call_with_args<'a>(
    vm: &'a Vm,
    generator: NonNull<Header>,
    method: &str,
    args: &[Option<NonNull<Header>>],
) -> Result<(Option<Value<'a>>, Option<(String, String)>), ExecError> {
    let mut consts = vec![Some(generator)];
    let mut items = vec![
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("LOAD_ATTR"), 0 << 1 | 1),
    ];
    for (index, value) in args.iter().enumerate() {
        let value = value.expect("本用例只送具体对象");
        // SAFETY: value 由调用方持有，常量表要自己那份。
        unsafe { vm.instance.incref_object(value.as_ptr()) };
        consts.push(Some(value));
        items.push(Item::Instr(op("LOAD_CONST"), (index + 1) as u8));
    }
    items.push(Item::Instr(op("CALL"), args.len() as u8));
    items.push(Item::Instr(op("RETURN_VALUE"), 0));
    // SAFETY: generator 由调用方持有。
    unsafe { vm.instance.incref_object(generator.as_ptr()) };
    let code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec![method.to_owned()],
        assemble(&items),
        consts,
    );
    match vm.run(&code) {
        Ok(value) => Ok((Some(value), None)),
        Err(ExecError::Raised { exception }) => {
            let type_name = vm.instance.type_name(vm.instance.type_of(exception));
            let text = vm.instance.object_str(exception);
            // SAFETY: exception 是存活对象，归还本函数这一份。
            unsafe { vm.instance.release_object(exception.as_ptr()) };
            Ok((None, Some((type_name, text))))
        }
        Err(other) => Err(other),
    }
}

#[test]
fn throw_delivers_the_exception_at_the_suspension_point() {
    let vm = Vm::new();
    let generator = call_with_code(&vm, catching_generator_code(&vm));
    // 先启动：拿第一个让出值
    let (first, raised) = call_with_args(&vm, generator, "__next__", &[]).expect("跑通");
    assert!(raised.is_none());
    assert!(first.is_some());
    // `throw(ValueError('x'))`：体里接住 ⇒ 走到处理块，让出 4
    let error = vm.instance.new_str("x");
    let class_ty = vm.instance.type_named("ValueError").unwrap();
    let class_object = class_ty.cast::<Header>();
    // SAFETY: 类型对象由注册表持有。
    unsafe { vm.instance.incref_object(class_object.as_ptr()) };
    let (value, raised) = call_with_args(&vm, generator, "throw", &[Some(class_object), Some(error)])
        .expect("跑通");
    assert!(raised.is_none(), "体内接住了异常，不该往外抛：{raised:?}");
    assert_eq!(
        vm.instance
            .int_value(value.expect("应当有值").as_header(&vm.instance).unwrap())
            .unwrap(),
        5,
        "处理块里让出 5"
    );
}

#[test]
fn close_and_throw_edge_cases_match_the_reference() {
    let vm = Vm::new();

    // `close()`：挂起中的生成器 ⇒ `None`，之后取下一个是 `StopIteration`
    let generator = make_generator(&vm, 3);
    let (_, _) = call_with_args(&vm, generator, "__next__", &[]).expect("跑通");
    let (value, raised) = call_with_args(&vm, generator, "close", &[]).expect("跑通");
    assert!(raised.is_none(), "close 不该抛");
    assert!(value.is_some(), "close 交出返回值（此处是 None 对象）");
    let (_, raised) = call_with_args(&vm, generator, "__next__", &[]).expect("跑通");
    assert_eq!(raised.expect("应当抛").0, "StopIteration", "关闭之后再取就是耗尽");

    // 被关闭时**又让出** ⇒ `RuntimeError`（同一个"接住任何异常再让出"的生成器）
    let generator = call_with_code(&vm, catching_generator_code(&vm));
    let (_, _) = call_with_args(&vm, generator, "__next__", &[]).expect("跑通");
    let (value, raised) = call_with_args(&vm, generator, "close", &[]).expect("跑通");
    assert!(value.is_none(), "忽略 GeneratorExit ⇒ 不交回值");
    let (type_name, text) = raised.expect("应当抛");
    assert_eq!(type_name, "RuntimeError");
    assert_eq!(text, "generator ignored GeneratorExit");

    // 刚创建的生成器 `throw` ⇒ **抛在调用处**（不进去）
    let generator = make_generator(&vm, 3);
    let class_object = vm.instance.type_named("ValueError").unwrap().cast::<Header>();
    // SAFETY: 类型对象由注册表持有。
    unsafe { vm.instance.incref_object(class_object.as_ptr()) };
    let (value, raised) = call_with_args(&vm, generator, "throw", &[Some(class_object)])
        .expect("跑通");
    assert!(value.is_none());
    assert_eq!(raised.expect("应当抛").0, "ValueError", "没启动 ⇒ 抛在调用处");

    // `throw(非异常)` ⇒ 实测消息
    let generator = make_generator(&vm, 3);
    let (_, _) = call_with_args(&vm, generator, "__next__", &[]).expect("跑通");
    let number = vm.instance.new_int(42);
    let (value, raised) = call_with_args(&vm, generator, "throw", &[Some(number)]).expect("跑通");
    assert!(value.is_none());
    let (type_name, text) = raised.expect("应当抛");
    assert_eq!(type_name, "TypeError");
    assert_eq!(
        text,
        "exceptions must be classes or instances deriving from BaseException, not int"
    );
}
