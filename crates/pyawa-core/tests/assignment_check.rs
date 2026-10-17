//! `TS-47` 的**运行期**赋值检查（`BC-65`）：`CHECK_ASSIGNMENT` 的语义与元数据。
//!
//! 验收照 `SPEC-type-system.md` 的 `TS-47`（运行期赋值也要检查、异常是 `TypeAssignmentError`、
//! 它是 `TypeError` 的子类）与 `TS-13`／`TS-28`…`TS-31`（标签相容判定沿用边界检查那一份）：
//! 标签形态与 `CHECK_BOUNDARY_*` 相同，但异常与消息是"赋值"语义，且**读栈顶不弹**
//! （后面那条存储指令还要用这个值）。

mod common;

use core::ptr::NonNull;

use pyawa_core::opcode;
use pyawa_core::opcode_metadata as meta;
use pyawa_core::{ExecError, Frame, Header, Value};

use common::{assemble, op, Item, Vm};

/// 一个标签条目的三种形态：类型对象、`"Any"`、`(外类型, 内标签)`（与边界检查同一套）。
fn label_type(vm: &Vm, name: &str) -> NonNull<Header> {
    let ty = vm.instance.type_named(name).expect("TS-41：类型应当已登记");
    vm.instance.type_value(ty)
}

fn label_any(vm: &Vm) -> NonNull<Header> {
    vm.instance.new_str("Any")
}

fn label_generic(vm: &Vm, outer: &str, inner: NonNull<Header>) -> NonNull<Header> {
    vm.instance.new_tuple(vec![label_type(vm, outer), inner])
}

/// 跑一段手写字节码：常量表 `[标签元组, 被检查的值]`，栈上先压值、再 `CHECK_ASSIGNMENT`、
/// 最后 `RETURN_VALUE`。返回值即"检查之后 `RETURN_VALUE` 拿到的那个值"——
/// 若检查把栈顶弹掉了，`RETURN_VALUE` 会失败（`BC-65`：读而不弹）。
fn run_assignment_at<'a>(
    vm: &'a Vm,
    label: NonNull<Header>,
    value: NonNull<Header>,
    filename: &'static str,
    line: u32,
) -> Result<Value<'a>, ExecError> {
    let signature = vm.instance.new_tuple(vec![label]);
    let body = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("CHECK_ASSIGNMENT"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    // `BC-18`：位置表与指令一一对应——每条都算在 `line` 这一行（失败消息要真行号）。
    let code = vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "<module>",
        "probe".to_owned(),
        filename.to_owned(),
        line as usize,
        8,
        0,
        0,
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        body,
        Vec::new(),
        vec![Some(signature), Some(value)],
        vec![(Some(line), Some(line), Some(0), Some(1)); 4],
    ));
    let namespace = vm
        .instance
        .alloc(pyawa_core::DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            core::cell::RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame)? {
        pyawa_core::ExecOutcome::Returned(value) => Ok(value),
        pyawa_core::ExecOutcome::Yielded(_) => panic!("顶层程序不该 yield"),
    }
}

/// 同 [`run_assignment_at`]，固定文件名与行号（消息断言用）。
fn run_assignment<'a>(
    vm: &'a Vm,
    label: NonNull<Header>,
    value: NonNull<Header>,
) -> Result<Value<'a>, ExecError> {
    run_assignment_at(vm, label, value, "probe.py", 7)
}

#[test]
fn assignment_metadata_is_right() {
    // `BC-65`：名字／带 oparg／栈效应 0／专有／低于 instrumented 区段
    let number = opcode::opcode("CHECK_ASSIGNMENT").expect("专有指令必须在表里");
    assert!(opcode::has_arg(number), "BC-65：CHECK_ASSIGNMENT 带 oparg");
    assert_eq!(opcode::opname(number), Some("CHECK_ASSIGNMENT"));
    assert_eq!(
        opcode::stack_effect(number, Some(0), None),
        Ok(0),
        "BC-65：不动值栈"
    );
    assert_eq!(
        opcode::stack_effect(number, Some(200), None),
        Ok(0),
        "BC-65：栈效应与 oparg 无关"
    );
    assert!(
        number < meta::MIN_INSTRUMENTED_OPCODE,
        "BC-31／BC-65：专有指令不得进入 instrumented 区段"
    );
    assert!(
        meta::PYAWA_SPECIFIC
            .iter()
            .any(|(candidate, _)| *candidate == "CHECK_ASSIGNMENT"),
        "BC-65：必须是 Pyawa 专有指令"
    );
    // `BC-40`：指令集**增了一条指令** ⇒ 版本必须已从 1 递增（取值由实现维护，不写死后续值）。
    assert!(
        meta::INSTRUCTION_SET_VERSION > 1,
        "BC-40：新增指令必须递增指令集版本（当前 {}）",
        meta::INSTRUCTION_SET_VERSION
    );
}

#[test]
fn assignment_error_is_a_type_error_subclass() {
    // `TS-47`：`TypeAssignmentError` 必须是 `TypeError` 的子类（既有 `except TypeError` 能兜住）
    let vm = Vm::new();
    let assignment = vm
        .instance
        .type_named("TypeAssignmentError")
        .expect("TS-47：TypeAssignmentError 必须注册进实例异常表");
    let type_error = vm.instance.type_named("TypeError").expect("TypeError 在内建表里");
    assert!(vm.instance.is_subtype(assignment, type_error), "TS-47：必须是 TypeError 子类");
    // 与边界异常**同族、分工不同**：两者都是 TypeError 子类但不是同一个类
    let boundary = vm
        .instance
        .type_named("TypeBoundaryError")
        .expect("TypeBoundaryError 在实例异常表里");
    assert_ne!(assignment, boundary, "TS-47：赋值与边界是两个异常类");
    assert!(vm.instance.is_subtype(boundary, type_error), "TS-12：边界异常也是 TypeError 子类");
}

#[test]
fn an_int_label_accepts_int_and_bool() {
    // `TS-40`：`bool ⊂ int` ⇒ `int` 标注接受 `True`
    let vm = Vm::new();
    run_assignment(&vm, label_type(&vm, "int"), vm.instance.new_int(42))
        .expect("int 值应当通过 int 标注的赋值检查");
    run_assignment(&vm, label_type(&vm, "int"), vm.instance.new_bool(true))
        .expect("bool ⊂ int：True 应当通过 int 标注的赋值检查");
}

#[test]
fn a_rejected_assignment_raises_type_assignment_error() {
    // `TS-47`：失败抛 `TypeAssignmentError`，消息含 expected／got／文件名／行号
    let vm = Vm::new();
    let error = run_assignment(&vm, label_type(&vm, "int"), vm.instance.new_str("not an int"))
        .expect_err("str 不该通过 int 标注的赋值检查");
    assert!(matches!(error, ExecError::Raised { .. }), "失败必须抛异常：{error:?}");
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "TypeAssignmentError", "TS-47：赋值不符的异常类型");
    let message = message.expect("消息要带上期望／实际／位置");
    assert!(message.contains("expected int"), "期望类型：{message}");
    assert!(message.contains("got str"), "实际类型：{message}");
    assert!(message.contains("probe.py"), "文件名：{message}");
    assert!(message.contains("line 7"), "行号：{message}");
}

#[test]
fn an_any_label_accepts_everything() {
    // `TS-28`：`Any` 与一切类型双向相容 ⇒ 赋值检查放行
    let vm = Vm::new();
    run_assignment(&vm, label_any(&vm), vm.instance.new_str("anything"))
        .expect("Any 必须接受任意值");
    let mixed = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_str("x"),
    ]);
    run_assignment(&vm, label_any(&vm), mixed).expect("Any 必须接受容器");
}

#[test]
fn a_composite_label_judges_the_outer_type() {
    // `TS-13`／`TS-31`：标签形态带内层 `(list, int)` 时，首先判**外类型**——
    // 外层不对 ⇒ 拒；外层对 ⇒ 接受（深层档位的元素检查由编译器决定是否发内层标签）。
    let vm = Vm::new();
    let int_list = || label_generic(&vm, "list", label_type(&vm, "int"));
    let good = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_bool(true), // `bool ⊂ int`（TS-40）
    ]);
    run_assignment(&vm, int_list(), good).expect("list[int] 标签接受 [1, True]");

    // 外类型不是 list ⇒ 拒（消息里的期望渲染成 `list[int]`）
    run_assignment(&vm, int_list(), vm.instance.new_str("not a list"))
        .expect_err("外类型不对必须拒绝");
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "TypeAssignmentError");
    let message = message.expect("消息要带期望／实际／位置");
    assert!(message.contains("expected list[int]"), "期望标签的渲染：{message}");
    assert!(message.contains("got str"), "实际类型：{message}");

    // `TS-13` 的**浅层**默认＝编译器只发**裸**类型标签 ⇒ 元素不进检查（这不是缺口）
    let mixed = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_str("x"),
    ]);
    run_assignment(&vm, label_type(&vm, "list"), mixed)
        .expect("裸 `list` 标签是浅层：元素类型不进检查");
}

#[test]
fn the_checked_value_stays_on_the_stack() {
    // `BC-65`：读栈顶**不弹**——检查后的 `RETURN_VALUE` 必须拿到**原来那个值**
    let vm = Vm::new();
    let text = vm.instance.new_str("kept");
    let result = run_assignment(&vm, label_any(&vm), text).expect("检查后 RETURN_VALUE 应拿到原值");
    let returned = result.as_header(&vm.instance).expect("有返回值");
    assert_eq!(returned, text, "BC-65：被检查的值必须原样留在栈上（同一对象）");
    // SAFETY: returned 是存活的 str 对象。
    assert_eq!(
        unsafe { &*returned.as_ptr().cast::<pyawa_core::StrObject>() }.value(),
        "kept"
    );
}
