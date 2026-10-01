//! `BC-4` 的 `co_*` 是**计算型属性**：走 `OM-11` 的 `getattr` 槽（不是类型字典里的常量），
//! 所以这一族同时验两件事——属性通道接对了，`co_*` 的值也对。
//!
//! **未接线**（`code.rs` 的清单为准）：`co_code`／`co_exceptiontable`（要 `bytes` 类型）、
//! `co_positions()`／`co_lines()`（要方法调用）、`co_filename`／`co_qualname`／`co_firstlineno`
//! （字段还没存）。

mod common;

use pyawa_core::{ExecError, StrObject, TupleObject, Value};

use common::{emit, op, Vm};

fn header(value: &Value<'_>, vm: &Vm) -> core::ptr::NonNull<pyawa_core::Header> {
    value.as_header(&vm.instance).expect("应当是具体对象")
}

fn int_of(value: &Value<'_>, vm: &Vm) -> i64 {
    let raw = header(value, vm);
    // SAFETY: 调用方保证这是个整数。
    unsafe { &*raw.as_ptr().cast::<pyawa_core::IntObject>() }.value
}

fn text_of(raw: core::ptr::NonNull<pyawa_core::Header>) -> String {
    // SAFETY: 调用方保证这是个 str。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned()
}

#[test]
fn code_attributes_are_computed_through_the_getattr_slot() {
    let vm = Vm::new();
    // def demo(alpha, beta): return alpha  —— 名字表里放 co_argcount
    let callee = vm.code_with_names(
        4,
        2,
        2,
        vec!["alpha".to_owned(), "beta".to_owned()],
        vec!["co_argcount".to_owned(), "co_varnames".to_owned(), "missing".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_FAST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(11)), Some(vm.constant(22))],
    );
    let callee_header = callee.as_ptr().cast::<pyawa_core::Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    // `callee.co_argcount`
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["co_argcount".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0), // 名字下标 0、不带方法位
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(callee_header)],
    );
    let result = vm.run(&code).unwrap();
    assert_eq!(int_of(&result, &vm), 2, "co_argcount");

    // `callee.co_varnames` ⇒ 两个名字的 tuple
    // 常量表**持有**引用（OM-40）：第二段程序要自己那份引用
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["co_varnames".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(callee_header)],
    );
    let result = vm.run(&code).unwrap();
    let raw = header(&result, &vm);
    // SAFETY: 属性槽给出的是 tuple。
    let names = unsafe { &*raw.as_ptr().cast::<TupleObject>() };
    assert_eq!(names.len(), 2, "co_varnames 的长度 ＝ co_nlocals");
    assert_eq!(text_of(names.item(0).unwrap()), "alpha");
    assert_eq!(text_of(names.item(1).unwrap()), "beta");
}

#[test]
fn code_consts_are_a_tuple_of_the_constants() {
    let vm = Vm::new();
    let callee = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["co_consts".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(7)), Some(vm.constant(8))],
    );
    let callee_header = callee.as_ptr().cast::<pyawa_core::Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["co_consts".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(callee_header)],
    );
    let result = vm.run(&code).unwrap();
    let raw = header(&result, &vm);
    // SAFETY: 属性槽给出的是 tuple。
    let consts = unsafe { &*raw.as_ptr().cast::<TupleObject>() };
    assert_eq!(consts.len(), 2);
    // SAFETY: 常量都是本测试造的整数。
    assert_eq!(unsafe { &*consts.item(0).unwrap().as_ptr().cast::<pyawa_core::IntObject>() }.value, 7);
}

#[test]
fn unimplemented_code_attributes_fall_through_to_attribute_error() {
    // `co_code` 要 `bytes` 类型（TS-42 排在 M3+）⇒ 槽位返回 None，落到 AttributeError
    let vm = Vm::new();
    let callee = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["co_code".to_owned()],
        emit(&[(op("RESUME"), 0), (op("RETURN_VALUE"), 0)]),
        Vec::new(),
    );
    let callee_header = callee.as_ptr().cast::<pyawa_core::Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["co_code".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(callee_header)],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception().map(|(name, _)| name),
        Some("AttributeError".to_owned())
    );
}

#[test]
fn code_identity_attributes_are_exposed() {
    // `co_qualname`（`BC-4`）；`co_filename`／`co_firstlineno` 走同一条槽
    let vm = Vm::new();
    let callee = vm.function_code(
        4,
        0,
        0,
        0,
        0,
        0,
        Vec::new(),
        emit(&[(op("RESUME"), 0), (op("RETURN_VALUE"), 0)]),
        Vec::new(),
    );
    let callee_header = callee.as_ptr().cast::<pyawa_core::Header>();
    // SAFETY: callee 由本测试持有。
    unsafe { vm.instance.incref_object(callee_header.as_ptr()) };

    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["co_qualname".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(callee_header)],
    );
    let result = vm.run(&code).unwrap();
    assert_eq!(text_of(header(&result, &vm)), "demo", "co_qualname");
}
