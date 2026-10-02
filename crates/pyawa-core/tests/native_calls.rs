//! **原生可调用对象**（`builtin_function_or_method`）。
//!
//! 实测形状：`repr(len)` = `<built-in function len>`；类型在探测表里（`TS-41`，阶梯 `later`）。
//!
//! 它是 `AB-24`／`AB-25` 的宿主函数与 `__build_class__` 一类内建函数的落点，故本层先把
//! "能调用一个 Rust 实现"这条打通：
//!
//! - 实参以**借用视图**递进去（要留住的自己 incref），返回值是**新引用**
//! - 绑定形态只是多带一个 `self`（`bound_self`）
//! - 出错经 [`pyawa_core::ExecError`] 冒泡（脚本异常照常）

mod common;

use core::cell::Cell;
use core::ptr::NonNull;

use pyawa_core::opcode::get_nb_ops;
use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance, Value};

use common::{emit, op, Vm};

fn nb(name: &str) -> u8 {
    get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

/// 一个把前两个实参相加的原生函数（`adder(a, b) = a + b`）。
unsafe fn add_two(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() != 2 {
        return Err(ExecError::Unsupported {
            opcode: 52,
            what: "add_two 只收两个实参",
        });
    }
    // SAFETY: 实参存活。
    let left = unsafe { &*args[0].as_ptr().cast::<pyawa_core::IntObject>() }.value.to_i64().expect("测试里是小整数");
    // SAFETY: 同上。
    let right = unsafe { &*args[1].as_ptr().cast::<pyawa_core::IntObject>() }.value.to_i64().expect("测试里是小整数");
    Ok(instance.new_int(left + right))
}

/// 一个用 `bound_self` 的原生函数（返回绑定对象本身的新引用）。
unsafe fn return_self(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(bound) = bound else {
        return Err(ExecError::Unsupported {
            opcode: 52,
            what: "return_self 需要绑定对象",
        });
    };
    // SAFETY: bound 由调用方保证存活。
    unsafe { instance.incref_object(bound.as_ptr()) };
    Ok(bound)
}

/// 把一个 Rust 函数包成原生可调用对象（**新引用**）。
fn native(vm: &Vm, name: &'static str, function: pyawa_core::NativeFn) -> NonNull<Header> {
    let object = vm.instance.alloc(BuiltinFunctionObject::new(
        vm.instance
            .type_named("builtin_function_or_method")
            .unwrap(),
        name,
        Cell::new(function),
    ));
    object.into_raw().cast::<Header>()
}

#[test]
fn a_native_callable_can_be_called_from_bytecode() {
    let vm = Vm::new();
    let function = native(&vm, "add_two", add_two);
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("CALL"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(function), Some(vm.constant(20)), Some(vm.constant(22))],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(42), &vm.instance), "20 + 22");
}

#[test]
fn a_native_callable_sees_the_bound_self() {
    // 绑定形态：`LOAD 值; LOAD 原生; ...` 走 CALL 的 `[可调用, self]` 约定
    let vm = Vm::new();
    let function = native(&vm, "return_self", return_self);
    let payload = vm.instance.new_str("绑定的对象");
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("CALL"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(function), Some(payload), Some(vm.instance.new_str("实参"))],
    );
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当是 str");
    // SAFETY: raw 是存活对象。
    let text = unsafe { &*raw.as_ptr().cast::<pyawa_core::StrObject>() }.value().to_owned();
    assert_eq!(text, "绑定的对象", "原生函数拿到的是 self 而不是实参");
}

#[test]
fn native_callable_repr_matches_the_reference() {
    let vm = Vm::new();
    let function = native(&vm, "len", add_two);
    assert_eq!(
        vm.instance.object_repr(function),
        "<built-in function len>",
        "实测形状"
    );
    let _ = nb("NB_ADD");
}

#[test]
fn native_callable_errors_propagate() {
    // 实参个数不对：原生函数自己报错 ⇒ 经 ExecError 冒泡（不是崩溃）
    let vm = Vm::new();
    let function = native(&vm, "add_two", add_two);
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("PUSH_NULL"), 0),
            (op("CALL"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(function)],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Unsupported { .. })));
}
