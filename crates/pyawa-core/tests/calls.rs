//! 调用与返回族（`docs/SPEC-bytecode.md` §10；绑定规则见 **BC-56**，验收 `T-BC-18`）。
//!
//! 压栈与 oparg 约定是**实测**的：
//!
//! - `MAKE_FUNCTION` **只**吃 code 对象；默认值由 `SET_FUNCTION_ATTRIBUTE` 挂
//!   （实测标志位：`1` ＝ defaults、`2` ＝ kwdefaults、`8` ＝ closure、`16` ＝ annotate）
//! - `SET_FUNCTION_ATTRIBUTE` 的栈是 `[属性值, 函数]`（**函数在 TOS**）
//! - `CALL n` 的栈是 `[可调用, NULL|self, 位置实参…]`
//! - `CALL_KW n` 另把**关键字名元组**放在 TOS，其下是关键字值，`n` ＝ 位置 ＋ 关键字个数
//!
//! `T-BC-18` 要求四类绑定错误报 `TypeError` 且**消息与参照实现一致**——消息必须探测，禁止手写。
//! 参照实现的原话（本机 3.14.4 实测，等异常对象接线后照抄）：
//!
//! ```text
//! strict() takes from 1 to 2 positional arguments but 3 were given
//! strict() missing 1 required positional argument: 'a'
//! strict() got multiple values for argument 'a'
//! strict() got an unexpected keyword argument 'zzz'
//! ```
//!
//! 现在只能报**类别**（四类各一个 [`ExecError`] 变体），异常对象未接线。

mod common;

use core::cell::RefCell;

use pyawa_core::opcode::get_nb_ops;
use pyawa_core::{ExecError, FunctionObject, Header, Instance, StrObject, TupleObject, Value};

use common::{emit, op, Vm};

fn nb(name: &str) -> u8 {
    get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

fn header_of(value: &Value<'_>, instance: &Instance) -> std::ptr::NonNull<Header> {
    value.as_header(instance).expect("应当是具体对象")
}

/// # Safety
///
/// 调用方必须先确认类型（`T` 要与它一致）。
unsafe fn payload<'a, T>(value: &Value<'_>, instance: &'a Instance) -> &'a T {
    // SAFETY: 由调用方保证类型正确。
    unsafe { &*header_of(value, instance).as_ptr().cast::<T>() }
}

/// 造一个"两个位置参数相加"的 callee。
fn adder(vm: &Vm, stacksize: usize, argcount: usize, consts: Vec<Option<std::ptr::NonNull<Header>>>) -> pyawa_core::Owned<'_, pyawa_core::CodeObject> {
    vm.function_code(
        stacksize,
        argcount,
        argcount,
        0,
        0,
        0,
        (0..argcount).map(|index| format!("arg{index}")).collect(),
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_FAST"), 0),
            (op("LOAD_FAST"), 1),
            (op("BINARY_OP"), nb("NB_ADD")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    )
}

#[test]
fn calls_a_function_with_a_default() {
    // def add(a, b=10): return a + b   ——  然后调用 add(5)
    let vm = Vm::new();
    let callee = adder(&vm, 4, 2, Vec::new());
    let callee_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let consts = vec![Some(vm.constant(10)), Some(callee_header), Some(vm.constant(5))];
    let code = vm.code(
        8,
        1,
        emit(&[
            // 默认值（一个 tuple）先压栈，code 在后（实测顺序：[defaults, code]）
            (op("LOAD_CONST"), 0),
            (op("BUILD_TUPLE"), 1),
            (op("LOAD_CONST"), 1),
            (op("MAKE_FUNCTION"), 0),
            (op("SET_FUNCTION_ATTRIBUTE"), 1),
            (op("STORE_FAST"), 0),
            // add(5)：`[可调用, NULL, 实参]`
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 2),
            (op("CALL"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(15), &vm.instance),
        "5 + 默认值 10 ＝ 15"
    );
}

#[test]
fn calls_with_keywords() {
    // def f(a, *, c=2): return a + c   ——  调用 f(7, c=3)
    let vm = Vm::new();
    let str_type = vm.instance.singletons().str_type();
    let c_name = vm.instance.alloc(StrObject::new(str_type, "c".to_owned()));
    let c_name_header = c_name.into_raw().cast::<Header>();

    let callee = vm.function_code(
        4,
        2,
        1,
        0,
        1,
        0,
        vec!["a".to_owned(), "c".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_FAST"), 0),
            (op("LOAD_FAST"), 1),
            (op("BINARY_OP"), nb("NB_ADD")),
            (op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let callee_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let consts = vec![
        Some(c_name_header),
        Some(callee_header),
        Some(vm.constant(2)), // kwdefaults 的值
        Some(vm.constant(7)), // a
        Some(vm.constant(3)), // c
    ];
    let code = vm.code(
        8,
        1,
        emit(&[
            // kwdefaults：{'c': 2}
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 2),
            (op("BUILD_MAP"), 1),
            (op("LOAD_CONST"), 1),
            (op("MAKE_FUNCTION"), 0),
            (op("SET_FUNCTION_ATTRIBUTE"), 2),
            (op("STORE_FAST"), 0),
            // f(7, c=3)：`[可调用, NULL, 位置实参, 关键字值, 名表]`
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 3),
            (op("LOAD_CONST"), 4),
            (op("LOAD_CONST"), 0),
            (op("BUILD_TUPLE"), 1),
            (op("CALL_KW"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(10), &vm.instance),
        "7 + 关键字 c=3 ＝ 10"
    );
}

#[test]
fn varargs_collect_extra_positional() {
    // def f(a, *rest): return rest   ——  调用 f(1, 2, 3)
    let vm = Vm::new();
    let callee = vm.function_code(
        4,
        2,
        1,
        0,
        0,
        0x04, // CO_VARARGS（实测 4）
        vec!["a".to_owned(), "rest".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_FAST"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let callee_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let consts = vec![
        Some(callee_header),
        Some(vm.constant(1)),
        Some(vm.constant(2)),
        Some(vm.constant(3)),
    ];
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("MAKE_FUNCTION"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 3),
            (op("CALL"), 3),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: callee 返回的是 *args 那个 tuple。
    let rest = unsafe { payload::<TupleObject>(&result, &vm.instance) };
    assert_eq!(rest.len(), 2, "多出来的两个进了 *args");
    assert_eq!(rest.item(0), code.get().constant(2));
    assert_eq!(rest.item(1), code.get().constant(3));
}

#[test]
fn varkeywords_collect_unknown_names() {
    // def f(**kw): return kw   ——  调用 f(zzz=9)
    let vm = Vm::new();
    let str_type = vm.instance.singletons().str_type();
    let zzz = vm.instance.alloc(StrObject::new(str_type, "zzz".to_owned()));

    let callee = vm.function_code(
        4,
        1,
        0,
        0,
        0,
        0x08, // CO_VARKEYWORDS（实测 8）
        vec!["kw".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_FAST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let callee_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let consts = vec![
        Some(zzz.into_raw().cast::<Header>()),
        Some(callee_header),
        Some(vm.constant(9)),
    ];
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("LOAD_CONST"), 1),
            (op("MAKE_FUNCTION"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 0),
            (op("BUILD_TUPLE"), 1),
            (op("CALL_KW"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: callee 返回的是 **kwargs 那个 dict。
    let collected = unsafe { payload::<pyawa_core::DictObject>(&result, &vm.instance) };
    assert_eq!(collected.len(), 1);
    assert_eq!(collected.entry(0).map(|(_, value)| value), code.get().constant(2));
}

#[test]
fn binding_errors_raise_real_typeerror_with_the_probed_message() {
    // `T-BC-18`／`BC-56`：四类绑定错误都要报**真** `TypeError`，且消息与参照实现逐字一致。
    // 参照实现的原话见本文件头部（实测 `def demo(a, b=1)` 等形状）；这里的 callee 名字就是 "demo"。
    let vm = Vm::new();
    let make_callee = |vm: &Vm| {
        let callee = vm.function_code(
            4,
            2,
            2,
            0,
            0,
            0,
            vec!["a".to_owned(), "b".to_owned()],
            emit(&[
                (op("RESUME"), 0),
                (op("LOAD_FAST"), 0),
                (op("RETURN_VALUE"), 0),
            ]),
            Vec::new(),
        );
        let header = callee.as_ptr().cast::<Header>();
        // SAFETY: callee 由本测试持有。
        unsafe { vm.instance.incref_object(header.as_ptr()) };
        header
    };

    // ① 多余位置实参：`def demo(a, b)` 收到 3 个
    let callee = make_callee(&vm);
    let consts = vec![Some(callee), Some(vm.constant(1)), Some(vm.constant(2)), Some(vm.constant(3))];
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("MAKE_FUNCTION"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 3),
            (op("CALL"), 3),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "TypeError".to_owned(),
            Some("demo() takes 2 positional arguments but 3 were given".to_owned())
        ))
    );

    // ② 缺少必填实参：一个都没给
    let callee = make_callee(&vm);
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("MAKE_FUNCTION"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("CALL"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(callee)],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "TypeError".to_owned(),
            Some("demo() missing 2 required positional arguments: 'a' and 'b'".to_owned())
        ))
    );

    // ③ 位置与关键字重复
    let str_type = vm.instance.singletons().str_type();
    let a_name = vm.instance.alloc(StrObject::new(str_type, "a".to_owned()));
    let callee = make_callee(&vm);
    let consts = vec![
        Some(a_name.into_raw().cast::<Header>()),
        Some(callee),
        Some(vm.constant(1)),
        Some(vm.constant(2)),
    ];
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("LOAD_CONST"), 1),
            (op("MAKE_FUNCTION"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 3),
            (op("LOAD_CONST"), 0),
            (op("BUILD_TUPLE"), 1),
            (op("CALL_KW"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "TypeError".to_owned(),
            Some("demo() got multiple values for argument 'a'".to_owned())
        ))
    );

    // ④ 未知关键字（没有 **kwargs）
    let zzz = vm.instance.alloc(StrObject::new(str_type, "zzz".to_owned()));
    let callee = make_callee(&vm);
    let consts = vec![
        Some(zzz.into_raw().cast::<Header>()),
        Some(callee),
        Some(vm.constant(1)),
        Some(vm.constant(2)),
    ];
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("LOAD_CONST"), 1),
            (op("MAKE_FUNCTION"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 3),
            (op("LOAD_CONST"), 0),
            (op("BUILD_TUPLE"), 1),
            (op("CALL_KW"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "TypeError".to_owned(),
            Some("demo() got an unexpected keyword argument 'zzz'".to_owned())
        ))
    );
}

#[test]
fn functions_own_their_code() {
    // 函数持有 code object 的引用（OM-40）：释放函数即释放那一份
    let vm = Vm::new();
    let base_live = vm.instance.live_objects();
    let callee = vm.function_code(
        2,
        0,
        0,
        0,
        0,
        0,
        Vec::new(),
        emit(&[(op("RETURN_VALUE"), 0)]),
        Vec::new(),
    );
    let code = vm.code(
        4,
        1,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("MAKE_FUNCTION"), 0),
            (op("POP_TOP"), 0), // 建好就丢掉：函数应当随之释放（OM-20）
            (op("LOAD_CONST"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(callee.into_raw().cast::<Header>()),
            Some(vm.constant(42)),
        ],
    );
    let frame = vm
        .instance
        .alloc(pyawa_core::Frame::for_code(vm.frame_type, &code));
    // 顶层帧 ＋ code object（＋ 常量表里的 callee code object）
    let after_setup = vm.instance.live_objects();
    let result = common::execute_value(&vm.instance, &frame).unwrap();
    assert!(result.is_same(&Value::small_int(42), &vm.instance));
    assert_eq!(
        vm.instance.live_objects(),
        after_setup,
        "函数建好又丢掉之后，不该多留对象"
    );
    drop(frame);
    drop(code);
    assert_eq!(vm.instance.live_objects(), base_live);
}

#[test]
fn make_function_rejects_non_code() {
    let vm = Vm::new();
    let code = vm.code(
        2,
        0,
        emit(&[(op("LOAD_CONST"), 0), (op("MAKE_FUNCTION"), 0), (op("RETURN_VALUE"), 0)]),
        vec![Some(vm.constant(1))],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Unsupported { .. })));
}

#[test]
fn function_payload_is_visible_to_tests() {
    // TS-43：载荷布局由实现自选——这里确认它确实带 code 与默认值
    let vm = Vm::new();
    let callee = vm.function_code(
        2,
        0,
        0,
        0,
        0,
        0,
        Vec::new(),
        emit(&[(op("RETURN_VALUE"), 0)]),
        Vec::new(),
    );
    let code_header = callee.as_ptr().cast::<Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(code_header.as_ptr()) };
    let function = vm.instance.alloc(FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        code_header,
        Vec::new(),
        None,
    
    RefCell::new(None),
core::cell::RefCell::new(Vec::new()),
            core::cell::RefCell::new(None),
            core::cell::RefCell::new(None)));
    assert_eq!(function.get().code(), code_header);
    assert!(function.get().defaults().is_empty());
    assert!(function.get().kwdefaults().is_none());
}
