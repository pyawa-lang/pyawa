//! **`with` 语句**（上下文管理器协议）：`LOAD_SPECIAL` ＋ `WITH_EXCEPT_START`。
//!
//! 发射骨架是**照参照实测**抄的（`tools` 之外的探针，见提交说明）：
//!
//! ```text
//! LOAD_FAST_BORROW ctx; COPY 1; LOAD_SPECIAL __exit__; SWAP 2; SWAP 3;
//! LOAD_SPECIAL __enter__; CALL 0; STORE_FAST value; <body>;
//! LOAD_CONST None ×3; CALL 3; POP_TOP; LOAD_FAST value; RETURN_VALUE
//! 异常出口：PUSH_EXC_INFO; WITH_EXCEPT_START; TO_BOOL; POP_JUMP_IF_TRUE L1;
//!           NOT_TAKEN; RERAISE 2; L1: POP_TOP; POP_EXCEPT; POP_TOP ×3;
//!           LOAD_FAST_CHECK value; RETURN_VALUE
//! ```
//!
//! 两条指令的净栈效应都是 **+1**（`dis.stack_effect` 实测）：
//! `LOAD_SPECIAL` **弹出对象、压入 (self, 可调用)**；`WITH_EXCEPT_START` **只压结果**，
//! 栈上原有的那几项一个都不动（`RERAISE` 与抑制分支都要用）。

mod common;

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::ptr::NonNull;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance, NativeFn};

use common::{assemble_labeled, op, varint, Item, Vm};

/// `__enter__` 的调用次数与返回值由这两个静态量控制（测试探针；`CX-3` 只扫 `src/`）。
static ENTER_CALLS: AtomicUsize = AtomicUsize::new(0);
static EXIT_CALLS: AtomicUsize = AtomicUsize::new(0);
static EXIT_SAW_NONE: AtomicBool = AtomicBool::new(false);
static EXIT_SUPPRESSES: AtomicBool = AtomicBool::new(false);

unsafe fn enter(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    ENTER_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(instance.new_int(7))
}

unsafe fn exit(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    EXIT_CALLS.fetch_add(1, Ordering::SeqCst);
    // 正常出口时三个实参都应当是 `None`
    let all_none = args.iter().all(|argument| {
        // SAFETY: 实参由调用方保证存活。
        unsafe { argument.as_ref() }.ty() == instance.singletons().none_type()
    });
    if all_none && args.len() == 3 {
        EXIT_SAW_NONE.store(true, Ordering::SeqCst);
    }
    Ok(instance.new_bool(EXIT_SUPPRESSES.load(Ordering::SeqCst)))
}

/// 造一个带 `__enter__`／`__exit__` 的类型（原生实现）与一个实例。
fn make_manager(vm: &Vm, name: &'static str) -> NonNull<Header> {
    let ty = vm.instance.new_attribute_type(name);
    for (attribute, handler) in [
        ("__enter__", enter as NativeFn),
        ("__exit__", exit as NativeFn),
    ] {
        let native = vm.instance.alloc(BuiltinFunctionObject::new(
            vm.instance
                .type_named("builtin_function_or_method")
                .expect("引导期已登记"),
            attribute,
            core::cell::Cell::new(handler),
        ));
        vm.instance
            .set_type_attribute(ty, attribute, native.into_raw().cast::<Header>());
    }
    let object = vm
        .instance
        .alloc(pyawa_core::AttributeObject::new(ty, RefCell::new(None)));
    object.into_raw().cast::<Header>()
}

/// 组装"`with ctx as value: <body>`"并跑，返回结果或错误。
fn run_with(
    vm: &Vm,
    ctx: NonNull<Header>,
    body: Vec<Item>,
    body_consts: Vec<Option<NonNull<Header>>>,
) -> Result<i64, ExecError> {
    // SAFETY: ctx 由调用方持有，常量表要自己那份。
    unsafe { vm.instance.incref_object(ctx.as_ptr()) };
    // 常量表：0 = ctx、1..=N = 体用到的常量、最后一个是 `None`（三处出口都用它）
    let mut consts = vec![Some(ctx)];
    consts.extend(body_consts);
    let none_index = consts.len();
    consts.push(Some(vm.instance.own(vm.instance.singletons().none()).into_raw()));

    let mut items = vec![
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("COPY"), 1),
        Item::Instr(op("LOAD_SPECIAL"), 1), // __exit__
        Item::Instr(op("SWAP"), 2),
        Item::Instr(op("SWAP"), 3),
        Item::Instr(op("LOAD_SPECIAL"), 0), // __enter__
        Item::Instr(op("CALL"), 0),
        Item::Instr(op("STORE_NAME"), 0), // value（模块级用 STORE_NAME；名字下标 0）
        Item::Label("body_start"),
    ];
    items.extend(body);
    items.push(Item::Instr(op("LOAD_CONST"), none_index as u8));
    items.push(Item::Instr(op("LOAD_CONST"), none_index as u8));
    items.push(Item::Instr(op("LOAD_CONST"), none_index as u8));
    items.push(Item::Instr(op("CALL"), 3)); // __exit__(None, None, None)
    items.push(Item::Instr(op("POP_TOP"), 0));
    items.push(Item::Instr(op("LOAD_NAME"), 0));
    items.push(Item::Instr(op("RETURN_VALUE"), 0));
    items.push(Item::Label("handler"));
    items.push(Item::Instr(op("PUSH_EXC_INFO"), 0));
    items.push(Item::Instr(op("WITH_EXCEPT_START"), 0));
    items.push(Item::Instr(op("TO_BOOL"), 0));
    items.push(Item::Jump(op("POP_JUMP_IF_TRUE"), "suppressed"));
    items.push(Item::Instr(op("NOT_TAKEN"), 0));
    items.push(Item::Instr(op("RERAISE"), 2));
    items.push(Item::Label("suppressed"));
    // 抑制分支的清理：本层的栈是 `[__exit__, self, lasti, prev, exc]`（**照参照的形状**：
    // `with` 的异常表条目**带 `lasti`**，派发时压了那个偏移）——`POP_TOP`（丢掉异常）、
    // `POP_EXCEPT`（还原上一个异常）、再三个 `POP_TOP`（`lasti`、`self`、`__exit__`）。
    items.push(Item::Instr(op("POP_TOP"), 0));
    items.push(Item::Instr(op("POP_EXCEPT"), 0));
    items.push(Item::Instr(op("POP_TOP"), 0));
    items.push(Item::Instr(op("POP_TOP"), 0));
    items.push(Item::Instr(op("POP_TOP"), 0));
    items.push(Item::Instr(op("LOAD_NAME"), 0));
    items.push(Item::Instr(op("RETURN_VALUE"), 0));

    let (bytes, labels) = assemble_labeled(&items);
    let offset_of = |name: &str| labels.iter().find(|(label, _)| *label == name).unwrap().1 / 2;
    let start = offset_of("body_start");
    let mut table = Vec::new();
    varint(start, &mut table);
    varint(offset_of("handler") - start, &mut table);
    varint(offset_of("handler"), &mut table);
    // `depth`：异常派发时把值栈回退到 `[__exit__, self]`（2 项）；`lasti` **置 1**
    // （编码是 `depth << 1 | lasti`；参照给 `with` 的条目就是带 `lasti` 的，`WITH_EXCEPT_START`
    // 的取项也因此要按"多一个 `lasti`"来数）。
    varint((2 << 1) | 1, &mut table);

    let code = vm.try_code(8, 0, vec!["value".to_owned()], bytes, consts, table);
    let namespace = vm
        .instance
        .alloc(pyawa_core::DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本函数持有，帧要自己那份；帧的全局层就取它。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    let value = common::execute_value(&vm.instance, &frame)?;
    match value {
        pyawa_core::Value::Int(number) => Ok(number),
        pyawa_core::Value::Object(raw) => Ok(vm
            .instance
            .int_value(raw.as_ptr())
            .expect("应当是整数")),
        other => panic!("期望整数，得到 {other:?}"),
    }
}

#[test]
fn the_with_protocol_matches_the_reference_observably() {
    // 三个场景**顺序**跑：计数器是共享的静态量，拆成三个并行用例会互相干扰
    normal_exit_calls_enter_then_exit_with_nones();
    an_exception_can_be_suppressed_by_exit();
    an_exception_propagates_when_exit_returns_false();
}

fn normal_exit_calls_enter_then_exit_with_nones() {
    ENTER_CALLS.store(0, Ordering::SeqCst);
    EXIT_CALLS.store(0, Ordering::SeqCst);
    EXIT_SAW_NONE.store(false, Ordering::SeqCst);
    let vm = Vm::new();
    let ctx = make_manager(&vm, "Manager");
    let body = vec![Item::Instr(op("NOP"), 0)];
    let result = run_with(&vm, ctx, body, Vec::new()).expect("`with` 应当正常跑完");
    assert_eq!(ENTER_CALLS.load(Ordering::SeqCst), 1, "`__enter__` 调一次");
    assert_eq!(EXIT_CALLS.load(Ordering::SeqCst), 1, "`__exit__` 调一次");
    assert!(
        EXIT_SAW_NONE.load(Ordering::SeqCst),
        "正常出口时 `__exit__(None, None, None)`"
    );
    assert_eq!(result, 7, "`as value` 拿到 `__enter__` 的返回值");
}

fn an_exception_can_be_suppressed_by_exit() {
    ENTER_CALLS.store(0, Ordering::SeqCst);
    EXIT_CALLS.store(0, Ordering::SeqCst);
    EXIT_SUPPRESSES.store(true, Ordering::SeqCst);
    let vm = Vm::new();
    let ctx = make_manager(&vm, "Suppressing");
    // 体里抛一个异常：`__exit__` 返回 True ⇒ 抑制、继续走到 `LOAD_NAME value`
    let error = make_exception(&vm, "ValueError", "boom");
    let body = vec![
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("RAISE_VARARGS"), 1),
    ];
    let result = run_with(&vm, ctx, body, vec![Some(error)]).expect("异常被抑制 ⇒ 不该往外抛");
    assert_eq!(result, 7, "抑制后继续用 `__enter__` 的返回值");
    assert_eq!(EXIT_CALLS.load(Ordering::SeqCst), 1);
    EXIT_SUPPRESSES.store(false, Ordering::SeqCst);
}

fn an_exception_propagates_when_exit_returns_false() {
    ENTER_CALLS.store(0, Ordering::SeqCst);
    EXIT_CALLS.store(0, Ordering::SeqCst);
    EXIT_SUPPRESSES.store(false, Ordering::SeqCst);
    let vm = Vm::new();
    let ctx = make_manager(&vm, "Passing");
    let error = make_exception(&vm, "ValueError", "boom");
    let body = vec![
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("RAISE_VARARGS"), 1),
    ];
    let outcome = run_with(&vm, ctx, body, vec![Some(error)]);
    assert!(outcome.is_err(), "`__exit__` 返回假 ⇒ 原异常继续往外");
    assert_eq!(EXIT_CALLS.load(Ordering::SeqCst), 1, "`__exit__` 仍被调用");
}

/// 造一个异常实例（与 `tests/exceptions.rs` 里同一套接口）。
fn make_exception(vm: &Vm, class: &str, message: &str) -> NonNull<Header> {
    let ty = vm.instance.type_named(class).expect("异常类已登记");
    let text = vm.instance.new_str(message);
    let object = vm.instance.alloc(pyawa_core::ExceptionObject::new(
        ty,
        RefCell::new(vec![text]),
        RefCell::new(None),
        RefCell::new(None),
        core::cell::Cell::new(false),
        RefCell::new(None),
    ));
    object.into_raw().cast::<Header>()
}
