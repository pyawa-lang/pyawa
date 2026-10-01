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

use common::{assemble, op, Item, Vm};

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
