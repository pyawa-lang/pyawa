//! `BC-4` 的 `co_*` 是**计算型属性**：走 `OM-11` 的 `getattr` 槽（不是类型字典里的常量），
//! 所以这一族同时验两件事——属性通道接对了，`co_*` 的值也对。
//!
//! **属性面现状**（对参照实现的 22 个 `co_*` 实测过一遍）：
//!
//! - **已接线 15 个**：`co_name`／`co_qualname`／`co_filename`／`co_firstlineno`／`co_argcount`／
//!   `co_posonlyargcount`／`co_kwonlyargcount`／`co_nlocals`／`co_stacksize`／`co_flags`／
//!   `co_varnames`／`co_names`／`co_consts`／`co_cellvars`／`co_freevars`
//! - **要 `bytes` 类型**（`TS-42` 排在 M3+）：`co_code`／`co_exceptiontable`／`co_linetable`／
//!   `co_lnotab`
//! - **要编译器产出的位置表**（`P3-12`）：`co_positions()`／`co_lines()`／`co_branches()`
//! - **本层多出来的两个**（参照实现没有）：`co_ncellvars`／`co_nfreevars`——是本层的便利属性，
//!   不是 `BC-4` 要求的

mod common;

use pyawa_core::{CodeObject, ExecError, Header, StrObject, TupleObject, Value};

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
    // `co_qualname`／`co_filename`／`co_firstlineno`（`BC-4`）走同一条槽
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


#[test]
fn cell_and_free_names_are_stored_separately() {
    // 参照实测：`co_varnames` **只含局部**（`('a', 'inner')`），而 `co_cellvars` 是 `('a', 'b')`
    // ——3.11+ 内部用 `co_localsplusnames`，三个属性是它的投影。所以名字必须**单独存**，
    // 不能从 `co_varnames` 推。
    let vm = Vm::new();
    let code = vm.instance.alloc(CodeObject::new(
        vm.code_type,
        "outer",
        "outer".to_owned(),
        "<pyawa-test>".to_owned(),
        0,
        4,
        2, // nlocals
        1, // argcount
        0,
        0,
        0b11,
        vec!["a".to_owned(), "inner".to_owned()],
        Vec::new(),
        vec!["a".to_owned(), "b".to_owned()],
        vec!["free".to_owned()],
        vec![],
        vec![],
        Vec::new(),
    
    Vec::new(),));
    let ptr = code.as_ptr().cast::<Header>();
    // SAFETY: code 由本测试持有，存活。
    let cellvars = unsafe { pyawa_core::code_getattr(ptr.as_ptr(), "co_cellvars", &vm.instance) }
        .expect("co_cellvars");
    // SAFETY: 返回的是元组。
    let cellvars = unsafe { &*cellvars.as_ptr().cast::<TupleObject>() };
    assert_eq!(cellvars.len(), 2);
    let names: Vec<String> = (0..cellvars.len())
        .map(|index| text_of(cellvars.item(index).expect("下标在范围内")))
        .collect();
    assert_eq!(names, vec!["a".to_owned(), "b".to_owned()], "cell 名字表");
    // SAFETY: 同上。
    let freevars = unsafe { pyawa_core::code_getattr(ptr.as_ptr(), "co_freevars", &vm.instance) }
        .expect("co_freevars");
    // SAFETY: 同上。
    let freevars = unsafe { &*freevars.as_ptr().cast::<TupleObject>() };
    assert_eq!(freevars.len(), 1);
    assert_eq!(text_of(freevars.item(0).expect("下标在范围内")), "free");
    // 条数属性与名字表一致
    // SAFETY: 同上。
    let ncell = unsafe { pyawa_core::code_getattr(ptr.as_ptr(), "co_ncellvars", &vm.instance) }
        .expect("co_ncellvars");
    // SAFETY: 是整数。
    assert_eq!(unsafe { &*ncell.as_ptr().cast::<pyawa_core::IntObject>() }.value, 2);
}
