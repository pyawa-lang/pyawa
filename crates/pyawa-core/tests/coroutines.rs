//! **协程**（`§10` 生成器与协程族）：`CO_COROUTINE`（实测 `0x80`）＋ `GET_AWAITABLE` ＋ `SEND`。
//!
//! `await` 的发射骨架照参照实测：
//!
//! ```text
//! RETURN_GENERATOR; POP_TOP; RESUME 0;
//! LOAD_FAST_BORROW inner; GET_AWAITABLE 0; LOAD_CONST None; SEND →END_SEND;
//! YIELD_VALUE; RESUME 3; JUMP_BACKWARD_NO_INTERRUPT →SEND; END_SEND; …
//! ```
//!
//! 协程与生成器**载荷同形**（一个挂起的帧 ＋ 标志），差别只有：`repr` 的词、以及
//! **协程不是迭代器**（没有 `__next__`／`__iter__`）；`SEND`／`send`／`throw`／`close` 走同一条路。

mod common;

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{CodeObject, Header, Value};

use common::{assemble, op, varint, Item, Vm};

/// `CO_COROUTINE`（实测 `0x80`）＋ `CO_OPTIMIZED|CO_NEWLOCALS`（`0x03`）。
const COROUTINE_FLAGS: u32 = 0x80 | 0x03;

/// 造一个"直接返回 7 的协程函数"。
fn inner_code(vm: &Vm) -> pyawa_core::Owned<'_, CodeObject> {
    vm.instance.alloc(CodeObject::new(
        vm.code_type,
        "inner",
        "inner".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        4,
        0,
        0,
        0,
        0,
        COROUTINE_FLAGS,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RETURN_GENERATOR"), 0),
            Item::Instr(op("POP_TOP"), 0),
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![Some(vm.constant(7))],
    ))
}

/// 造一个"`return await inner`"的协程函数（`inner` 是形参，与参照骨架同形）。
fn outer_code(vm: &Vm) -> pyawa_core::Owned<'_, CodeObject> {
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    vm.instance.alloc(CodeObject::new(
        vm.code_type,
        "outer",
        "outer".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        6,
        1, // nlocals
        1, // argcount：inner
        0,
        0,
        COROUTINE_FLAGS,
        vec!["inner".to_owned()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RETURN_GENERATOR"), 0),
            Item::Instr(op("POP_TOP"), 0),
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_FAST_BORROW"), 0),
            Item::Instr(op("GET_AWAITABLE"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Label("resend"),
            Item::Jump(op("SEND"), "done"),
            Item::Instr(op("YIELD_VALUE"), 0),
            Item::Instr(op("RESUME"), 3),
            Item::Jump(op("JUMP_BACKWARD_NO_INTERRUPT"), "resend"),
            Item::Label("done"),
            Item::Instr(op("END_SEND"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![Some(none)],
    ))
}

/// 把 code object 包成函数并调用（协程函数的 `CALL` **不跑函数体**，只交出协程对象）。
fn make_coroutine(vm: &Vm, code: &pyawa_core::Owned<'_, CodeObject>, args: &[NonNull<Header>]) -> NonNull<Header> {
    let code_header = code.as_ptr().cast::<Header>();
    // SAFETY: code 由调用方持有。
    unsafe { vm.instance.incref_object(code_header.as_ptr()) };
    let function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        code_header,
        Vec::new(),
        None,
        RefCell::new(None),
    ));
    let function = function.into_raw().cast::<Header>();
    // `call_value` **接手**实参表（调用方的那份引用会被消费）⇒ 这里各新增一份，
    // 免得调用方随后还要用同一个对象（第一版就是这样双重释放的）。
    for argument in args {
        // SAFETY: 实参由调用方保证存活。
        unsafe { vm.instance.incref_object(argument.as_ptr()) };
    }
    pyawa_core::call_value(&vm.instance, function, args, &[]).expect("造协程应当成功")
}

/// 在**真路径**上调协程的方法：`coro.send(值)` ＝ `LOAD 协程; LOAD_ATTR send(+方法位); 值; CALL`。
#[allow(clippy::type_complexity)]
fn call_method<'a>(
    vm: &'a Vm,
    coroutine: NonNull<Header>,
    method: &str,
    argument: Option<NonNull<Header>>,
) -> Result<(Option<Value<'a>>, Option<(String, String)>), pyawa_core::ExecError> {
    let mut consts = vec![Some(coroutine)];
    let mut items = vec![
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("LOAD_ATTR"), 0 << 1 | 1),
    ];
    let argument_count = if let Some(value) = argument {
        // SAFETY: value 由调用方持有，常量表要自己那份。
        unsafe { vm.instance.incref_object(value.as_ptr()) };
        consts.push(Some(value));
        items.push(Item::Instr(op("LOAD_CONST"), 1));
        1
    } else {
        0
    };
    items.push(Item::Instr(op("CALL"), argument_count));
    items.push(Item::Instr(op("RETURN_VALUE"), 0));
    // SAFETY: coroutine 由调用方持有。
    unsafe { vm.instance.incref_object(coroutine.as_ptr()) };
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
        Err(pyawa_core::ExecError::Raised { exception }) => {
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
fn awaiting_a_coroutine_runs_it_to_completion() {
    let vm = Vm::new();
    let inner = make_coroutine(&vm, &inner_code(&vm), &[]);
    let outer = make_coroutine(&vm, &outer_code(&vm), &[inner]);

    // `repr` 的词是 `coroutine`（实测 `<coroutine object outer at 0x…>`）
    let rendered = vm.instance.object_repr(outer);
    assert!(
        rendered.starts_with("<coroutine object outer at 0x"),
        "协程的 repr 形状：{rendered}"
    );

    // `outer.send(None)`：`await inner` ⇒ 内层直接返回 7 ⇒ 外层也跑完，`StopIteration(7)`
    let (value, raised) = call_method(&vm, outer, "send", None).expect("应当跑通");
    assert!(value.is_none(), "跑完应当是异常");
    let (type_name, text) = raised.expect("应当抛 StopIteration");
    assert_eq!(type_name, "StopIteration");
    assert_eq!(text, "7", "`await` 的结果成为协程的返回值");
}

#[test]
fn coroutine_specific_messages_match_the_reference() {
    let vm = Vm::new();

    // 刚创建的协程 `send(非 None)` ⇒ 词是 `coroutine`（实测）
    let coroutine = make_coroutine(&vm, &inner_code(&vm), &[]);
    let number = vm.instance.new_int(5);
    let (value, raised) = call_method(&vm, coroutine, "send", Some(number)).expect("应当跑通");
    assert!(value.is_none());
    let (type_name, text) = raised.expect("应当抛");
    assert_eq!(type_name, "TypeError");
    assert_eq!(
        text,
        "can't send non-None value to a just-started coroutine"
    );

    // `await 42` ⇒ 实测 `TypeError: 'int' object can't be awaited`
    let number = vm.instance.new_int(42);
    let outer = make_coroutine(&vm, &outer_code(&vm), &[number]);
    let (value, raised) = call_method(&vm, outer, "send", None).expect("应当跑通");
    assert!(value.is_none());
    let (type_name, text) = raised.expect("应当抛");
    assert_eq!(type_name, "TypeError");
    assert_eq!(text, "'int' object can't be awaited");
}

#[test]
fn a_coroutine_is_not_an_iterator() {
    let vm = Vm::new();
    let coroutine = make_coroutine(&vm, &inner_code(&vm), &[]);
    // 协程没有 `__next__`：走属性通道应当落到 `AttributeError`（参照实现同）
    let mut consts = vec![Some(coroutine)];
    // SAFETY: coroutine 由本测试持有。
    unsafe { vm.instance.incref_object(coroutine.as_ptr()) };
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["__next__".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("LOAD_ATTR"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        std::mem::take(&mut consts),
    );
    let _ = vm.run(&code);
    let (type_name, text) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "AttributeError");
    assert_eq!(
        text.as_deref(),
        Some("'coroutine' object has no attribute '__next__'")
    );
}

// ---- 异步生成器（`CO_ASYNC_GENERATOR`，实测 `0x200`）＋ `async for` 的指令 ----

/// 造一个"让出 1、让出 2、返回"的**异步**生成器函数。
fn async_generator_code(vm: &Vm) -> pyawa_core::Owned<'_, CodeObject> {
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    vm.instance.alloc(CodeObject::new(
        vm.code_type,
        "agen",
        "agen".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        4,
        0,
        0,
        0,
        0,
        0x200 | 0x03, // CO_ASYNC_GENERATOR | CO_OPTIMIZED | CO_NEWLOCALS
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RETURN_GENERATOR"), 0),
            Item::Instr(op("POP_TOP"), 0),
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("YIELD_VALUE"), 0),
            Item::Instr(op("RESUME"), 5),
            Item::Instr(op("POP_TOP"), 0),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("YIELD_VALUE"), 0),
            Item::Instr(op("RESUME"), 5),
            Item::Instr(op("POP_TOP"), 0),
            Item::Instr(op("LOAD_CONST"), 2),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![
            Some(vm.constant(1)),
            Some(vm.constant(2)),
            Some(vm.constant(3)),
            Some(none),
        ],
    ))
}

/// "`await` 一个 async iterator 一次"的协程（本层口径：await 异步生成器 ＝ 推进它一次）。
fn take_one_code(vm: &Vm) -> pyawa_core::Owned<'_, CodeObject> {
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    vm.instance.alloc(CodeObject::new(
        vm.code_type,
        "take_one",
        "take_one".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        6,
        1,
        1,
        0,
        0,
        COROUTINE_FLAGS,
        vec!["aiter".to_owned()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RETURN_GENERATOR"), 0),
            Item::Instr(op("POP_TOP"), 0),
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_FAST_BORROW"), 0),
            Item::Instr(op("GET_AWAITABLE"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Label("resend"),
            Item::Jump(op("SEND"), "done"),
            Item::Instr(op("YIELD_VALUE"), 0),
            Item::Instr(op("RESUME"), 3),
            Item::Jump(op("JUMP_BACKWARD_NO_INTERRUPT"), "resend"),
            Item::Label("done"),
            Item::Instr(op("END_SEND"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![Some(none)],
    ))
}

#[test]
fn an_async_generator_is_not_directly_awaitable() {
    // **实测**：`await agen` ⇒ `TypeError: 'async_generator' object can't be awaited`
    // （要经 `__anext__()` 交出的 awaitable）。第一版让"await 异步生成器 ＝ 推进一次"，
    // 是被这条实测打回的——捷径会让 `await`／`async for` 的语义走样。
    let vm = Vm::new();
    let agen = make_coroutine(&vm, &async_generator_code(&vm), &[]);
    let rendered = vm.instance.object_repr(agen);
    assert!(
        rendered.starts_with("<async_generator object agen at 0x"),
        "异步生成器的 repr 形状：{rendered}"
    );

    let coroutine = make_coroutine(&vm, &take_one_code(&vm), &[agen]);
    let (value, raised) = call_method(&vm, coroutine, "send", None).expect("应当跑通");
    assert!(value.is_none(), "应当抛");
    let (type_name, text) = raised.expect("应当抛 TypeError");
    assert_eq!(type_name, "TypeError");
    assert_eq!(text, "'async_generator' object can't be awaited");
}

#[test]
fn get_aiter_and_end_async_for_behave_as_measured() {
    let vm = Vm::new();
    let agen = make_coroutine(&vm, &async_generator_code(&vm), &[]);

    // `GET_AITER`：异步生成器原样就是 async iterator（净 0）
    // SAFETY: agen 由本测试持有，常量表要自己那份。
    unsafe { vm.instance.incref_object(agen.as_ptr()) };
    let code = vm.code(
        4,
        0,
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("GET_AITER"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![Some(agen)],
    );
    let value = vm.run(&code).expect("应当跑通");
    let raw = value.as_header(&vm.instance).expect("应当是具体对象");
    assert_eq!(vm.instance.type_name(vm.instance.type_of(raw)), "async_generator");

    // `END_ASYNC_FOR`（净 −2）：`StopAsyncIteration` ⇒ 丢掉异常与迭代器并**跳到目标**
    let stop_async = vm.instance.alloc(pyawa_core::ExceptionObject::new(
        vm.instance.type_named("StopAsyncIteration").unwrap(),
        RefCell::new(Vec::new()),
        RefCell::new(None),
        RefCell::new(None),
        core::cell::Cell::new(false),
        RefCell::new(None),
    ));
    let stop_async = stop_async.into_raw().cast::<Header>();
    // SAFETY: agen 与 stop_async 都由本测试持有，常量表各要一份。
    unsafe {
        vm.instance.incref_object(agen.as_ptr());
        vm.instance.incref_object(stop_async.as_ptr());
    }
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0), // async iterator
            Item::Instr(op("LOAD_CONST"), 1), // StopAsyncIteration（TOS）
            Item::Jump(op("END_ASYNC_FOR"), "after"),
            Item::Label("after"),
            Item::Instr(op("LOAD_CONST"), 2),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![Some(agen), Some(stop_async), Some(vm.constant(7))],
    );
    let value = vm.run(&code).expect("应当跑通");
    let raw = value.as_header(&vm.instance).expect("应当是具体对象");
    assert_eq!(
        vm.instance.type_name(vm.instance.type_of(raw)),
        "int",
        "END_ASYNC_FOR 之后应当落在目标上的 `LOAD_CONST 7`"
    );
    assert_eq!(vm.instance.int_value(raw), Some(7), "跳到了 L3 之后的常量");
}

#[test]
fn stop_iteration_escaping_a_coroutine_becomes_a_runtime_error() {
    // 实测：协程里逃出来的 `StopIteration` 变成 `RuntimeError: coroutine raised StopIteration`
    // （`@types.coroutine` 生成器那条则是 `generator raised StopIteration`——词由**被驱动的
    // 对象种类**决定，所以转换做在知道种类的地方，而不是只靠栈上那条 intrinsic）。
    let vm = Vm::new();
    let stop = vm.instance.alloc(pyawa_core::ExceptionObject::new(
        vm.instance.type_named("StopIteration").unwrap(),
        RefCell::new(vec![vm.instance.new_int(5)]),
        RefCell::new(None),
        RefCell::new(None),
        core::cell::Cell::new(false),
        RefCell::new(None),
    ));
    let stop = stop.into_raw().cast::<Header>();
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    let code = vm.instance.alloc(CodeObject::new(
        vm.code_type,
        "raises",
        "raises".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        4,
        0,
        0,
        0,
        0,
        COROUTINE_FLAGS,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RETURN_GENERATOR"), 0),
            Item::Instr(op("POP_TOP"), 0),
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("RAISE_VARARGS"), 1),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![Some(stop), Some(none)],
    ));
    let coroutine = make_coroutine(&vm, &code, &[]);
    let (value, raised) = call_method(&vm, coroutine, "send", None).expect("应当跑通");
    assert!(value.is_none());
    let (type_name, text) = raised.expect("应当抛");
    assert_eq!(type_name, "RuntimeError");
    assert_eq!(text, "coroutine raised StopIteration");
}

/// `BINARY_OP` 的 `+` 在 `nb_ops` 表里的下标（从表里取，别写死）。
fn plus_index() -> u8 {
    pyawa_core::opcode::get_nb_ops()
        .iter()
        .position(|entry| entry.1 == "+")
        .expect("nb_ops 里应当有 +") as u8
}

/// 造一个"`async for` 求和"的协程（骨架照参照实测，含循环的异常表）。
fn async_for_sum_code(vm: &Vm, source: NonNull<Header>) -> pyawa_core::Owned<'_, CodeObject> {
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    // SAFETY: source 由调用方持有，常量表要自己那份。
    unsafe { vm.instance.incref_object(source.as_ptr()) };
    let items = vec![
        Item::Instr(op("RETURN_GENERATOR"), 0),
        Item::Instr(op("POP_TOP"), 0),
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 1), // total = 0
        Item::Instr(op("STORE_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 0), // async iterator（异步生成器自己）
        Item::Instr(op("GET_AITER"), 0),
        Item::Label("loop"),
        Item::Instr(op("GET_ANEXT"), 0),
        Item::Instr(op("LOAD_CONST"), 2), // None
        Item::Label("resend"),
        Item::Jump(op("SEND"), "done"),
        Item::Instr(op("YIELD_VALUE"), 0),
        Item::Instr(op("RESUME"), 3),
        Item::Jump(op("JUMP_BACKWARD_NO_INTERRUPT"), "resend"),
        Item::Label("done"),
        Item::Instr(op("END_SEND"), 0),
        Item::Instr(op("NOT_TAKEN"), 0),
        Item::Instr(op("STORE_FAST"), 1), // value
        // `total = total + value`（就地 `+=` 的 `NB_` 还没接线，这里用已接线的 `+`）
        Item::Instr(op("LOAD_FAST_BORROW"), 0),
        Item::Instr(op("LOAD_FAST_BORROW"), 1),
        Item::Instr(op("BINARY_OP"), plus_index()),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Jump(op("JUMP_BACKWARD"), "loop"), // 回边：交给汇编器按标签算
        Item::Label("handler"),
        // 异常从 `SEND` 冒出来（异步生成器耗尽 ⇒ `StopAsyncIteration`）：先 `CLEANUP_THROW`，
        // 再由 `END_ASYNC_FOR` 收掉异常与 async iterator、跳到循环之后。
        // （参照实现在这中间还有一条回 `SEND` 的边，那是给 `throw`／`close` 穿过帧的情形用的，
        // 本用例不涉及，故按"直接落 `END_ASYNC_FOR`"搭。）
        Item::Instr(op("CLEANUP_THROW"), 0),
        Item::Jump(op("END_ASYNC_FOR"), "after"),
        Item::Label("after"),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ];
    let (patched, labels) = common::assemble_labeled(&items);
    let offset_of = |name: &str| labels.iter().find(|(label, _)| *label == name).unwrap().1 / 2;

    let mut table = Vec::new();
    varint(offset_of("loop") - 1, &mut table); // 起点：GET_ANEXT 之前的 GET_AITER 之后
    varint(offset_of("after") - offset_of("loop"), &mut table);
    varint(offset_of("handler"), &mut table);
    varint(1 << 1 | 0, &mut table); // depth 1、lasti 0
    vm.instance.alloc(CodeObject::new(
        vm.code_type,
        "sum_async",
        "sum_async".to_owned(),
        "<pyawa-test>".to_owned(),
        1,
        8,
        2,
        0,
        0,
        0,
        COROUTINE_FLAGS,
        vec!["total".to_owned(), "value".to_owned()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        patched,
        table,
        vec![Some(source), Some(vm.constant(0)), Some(none)],
    ))
}

#[test]
fn async_for_can_drive_an_async_generator() {
    let vm = Vm::new();
    let source = make_coroutine(&vm, &async_generator_code(&vm), &[]);
    let driver = make_coroutine(&vm, &async_for_sum_code(&vm, source), &[]);
    // `async for` 里每一次 `await __anext__()` 都要驱动器接着发 `None`
    let mut last: Option<(String, String)> = None;
    for _ in 0..8 {
        let (value, raised) = call_method(&vm, driver, "send", None).expect("应当跑通");
        if let Some((type_name, text)) = raised {
            last = Some((type_name, text));
            break;
        }
        assert!(value.is_some(), "驱动器应当让出（await 的中间态）");
    }
    let (type_name, text) = last.expect("驱动器最终要结束");
    assert_eq!(type_name, "StopIteration");
    assert_eq!(text, "3", "`async for` 把 1 与 2 加起来");
}
