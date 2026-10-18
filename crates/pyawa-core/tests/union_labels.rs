//! **`TS-46`** 的运行期语义：**union 标签**（`T?` ＝ `T | None`）。
//!
//! 接口契约（`BC-24`）：union 标签是**元组**，首项是字符串 `"Union"`，其后 ≥1 个成员标签
//! `("Union", 标签, 标签, …)`；成员可递归（类型对象／`"Any"`／嵌套 union／既有 `(外, 内)` 复合标签）。
//! 判定：**任一成员相容即相容**（`TS-46`：值为 `None` ⇒ 通过；否则按 `T` 检查）。
//!
//! 两条检查路都经由 `boundary_accepts` ⇒ union 语义对 `CHECK_BOUNDARY_*` 与 `CHECK_ASSIGNMENT`
//! **同时**生效（有意的：`TS-46` 的检查语义同时约束两处）。union 标签的**生成**在编译器侧，
//! 不在这里（本测试全部用手写字节码）。

mod common;

use core::ptr::NonNull;

use pyawa_core::{ExecError, Frame, Header, Value};

use common::{assemble, execute_value, op, Item, Vm};

/// 一个标签条目的第一种形态：**类型对象**（`TS-29` 按子类型判定）。
fn label_type(vm: &Vm, name: &str) -> NonNull<Header> {
    let ty = vm.instance.type_named(name).expect("TS-41：类型应当已登记");
    vm.instance.type_value(ty)
}

/// 第二种形态：字符串 `"Any"`（`TS-28`：与一切类型双向相容）。
fn label_any(vm: &Vm) -> NonNull<Header> {
    vm.instance.new_str("Any")
}

/// union 标签：`("Union", 成员…)`，**接手**各成员的新引用。
fn label_union(vm: &Vm, members: Vec<NonNull<Header>>) -> NonNull<Header> {
    let mut parts = Vec::with_capacity(members.len() + 1);
    parts.push(vm.instance.new_str("Union"));
    parts.extend(members);
    vm.instance.new_tuple(parts)
}

/// `None` 的值（`TS-46` 的通过侧）。
fn none(vm: &Vm) -> NonNull<Header> {
    vm.instance.new_none()
}

/// 造一个空的映射命名空间（模块级帧用；`for_code_with_namespace` **接手**一份新引用）。
fn new_namespace(vm: &Vm) -> NonNull<Header> {
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
    namespace
}

/// 跑一段带 `CHECK_BOUNDARY_IN` 的手写字节码：签名标签是 `(label,)`，局部槽 0 是待查值。
fn run_boundary_in<'a>(
    vm: &'a Vm,
    label: NonNull<Header>,
    value: NonNull<Header>,
) -> Result<Value<'a>, ExecError> {
    let signature = vm.instance.new_tuple(vec![label]);
    let body = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code_with_names(
        8,
        1,
        1,
        vec!["arg0".to_owned()],
        Vec::new(),
        body,
        vec![Some(signature)],
    );
    let namespace = new_namespace(vm);
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    // `set_local` 收**新引用** ⇒ 把 `value` 直接交出去。
    assert!(
        frame.set_local(0, Some(value)).expect("写局部槽").is_none(),
        "局部槽 0 原本应当是空的"
    );
    let frame = vm.instance.alloc(frame);
    execute_value(&vm.instance, &frame)
}

/// 跑一段带 `CHECK_BOUNDARY_OUT` 的手写字节码：签名标签是 `(label,)`，栈顶是待查返回值。
fn run_boundary_out<'a>(
    vm: &'a Vm,
    label: NonNull<Header>,
    value: NonNull<Header>,
) -> Result<Value<'a>, ExecError> {
    let signature = vm.instance.new_tuple(vec![label]);
    let body = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("CHECK_BOUNDARY_OUT"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code_with_names(
        8,
        1,
        0,
        Vec::new(),
        Vec::new(),
        body,
        vec![Some(signature)],
    );
    let namespace = new_namespace(vm);
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    assert!(
        frame.set_local(0, Some(value)).expect("写局部槽").is_none(),
        "局部槽 0 原本应当是空的"
    );
    let frame = vm.instance.alloc(frame);
    execute_value(&vm.instance, &frame)
}

/// 跑一段带 `CHECK_ASSIGNMENT` 的手写字节码：常量表 `[标签元组, 待查值]`；
/// 检查**栈顶**后 `RETURN_VALUE` 返回它（`BC-65`：读而不弹）。
fn run_assignment<'a>(
    vm: &'a Vm,
    label: NonNull<Header>,
    value: NonNull<Header>,
) -> Result<Value<'a>, ExecError> {
    let signature = vm.instance.new_tuple(vec![label]);
    let body = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("CHECK_ASSIGNMENT"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code(8, 0, body, vec![Some(signature), Some(value)]);
    vm.run(&code)
}

/// 断言检查失败并取（异常类型名, 消息）——失败必须是 `ExecError::Raised`。
fn expect_rejected<T>(vm: &Vm, result: Result<T, ExecError>) -> (String, Option<String>) {
    match result {
        Ok(_) => panic!("这个值不该通过标签检查"),
        Err(ExecError::Raised { .. }) => vm.pending_exception().expect("应当有异常"),
        Err(other) => panic!("检查失败必须抛异常：{other:?}"),
    }
}

#[test]
fn a_union_of_int_and_none_accepts_none_and_int_but_rejects_str() {
    // `TS-46`＝`T?`：值为 `None` ⇒ 通过；否则按 `T` 检查 ⇒ 与单独的 `T` **不同**（`T` 拒 `None`）。
    let vm = Vm::new();
    let label = || label_union(&vm, vec![label_type(&vm, "int"), label_type(&vm, "NoneType")]);

    // `None` 通过（这半边正是 `T?` 与 `T` 的区别）
    run_boundary_in(&vm, label(), none(&vm)).expect("`None` 应当通过 Union[int, NoneType]");
    // 否则按 `T`＝`int` 检查：int 与 `bool ⊂ int`（`TS-40`）都通过
    run_boundary_in(&vm, label(), vm.instance.new_int(42)).expect("int 应当通过 Union[int, NoneType]");
    run_boundary_in(&vm, label(), vm.instance.new_bool(true))
        .expect("bool ⊂ int：True 应当通过 Union[int, NoneType]");
    // 不在任何成员里的类型 ⇒ 拒
    let (type_name, message) = expect_rejected(
        &vm,
        run_boundary_in(&vm, label(), vm.instance.new_str("not an int")),
    );
    assert_eq!(type_name, "TypeBoundaryError", "TS-12：边界检查的归责异常");
    let message = message.expect("消息要带期望／实际");
    // `TS-11` 的"期望"元素：union 标签要渲染成 `Union[...]`，不能退化成 `Any`
    assert!(
        message.contains("Union[int, NoneType]"),
        "expected 渲染 union 标签：{message}"
    );
    assert!(!message.contains("expected Any"), "union 不能渲染成 Any：{message}");
    assert!(message.contains("got str"), "实际类型：{message}");
}

#[test]
fn a_nested_union_accepts_each_member_and_rejects_outside() {
    // 成员可递归：`Union[Union[int, str], NoneType]`（成员的成员也逐个判）。
    let vm = Vm::new();
    let label = || {
        let inner = label_union(&vm, vec![label_type(&vm, "int"), label_type(&vm, "str")]);
        label_union(&vm, vec![inner, label_type(&vm, "NoneType")])
    };
    run_boundary_in(&vm, label(), vm.instance.new_int(1)).expect("int 在嵌套 union 里");
    run_boundary_in(&vm, label(), vm.instance.new_str("x")).expect("str 在嵌套 union 里");
    run_boundary_in(&vm, label(), none(&vm)).expect("None 在嵌套 union 里");
    // 三个成员之外 ⇒ 拒
    let (type_name, message) = expect_rejected(
        &vm,
        run_boundary_in(&vm, label(), vm.instance.new_float(1.5)),
    );
    assert_eq!(type_name, "TypeBoundaryError");
    let message = message.expect("消息要带期望／实际");
    assert!(
        message.contains("Union[Union[int, str], NoneType]"),
        "嵌套 union 的渲染：{message}"
    );
    assert!(message.contains("got float"), "实际类型：{message}");
}

#[test]
fn an_any_member_accepts_everything() {
    // `TS-28`：成员是 `"Any"` ⇒ 该成员接受一切（union 里有一个全相容成员就整体放行）。
    let vm = Vm::new();
    let label = || label_union(&vm, vec![label_any(&vm), label_type(&vm, "int")]);
    run_boundary_in(&vm, label(), vm.instance.new_str("anything")).expect("Any 成员接受 str");
    run_boundary_in(&vm, label(), vm.instance.new_float(2.5)).expect("Any 成员接受 float");
    run_boundary_in(&vm, label(), none(&vm)).expect("Any 成员接受 None");
    let mixed = vm
        .instance
        .new_list(vec![vm.instance.new_int(1), vm.instance.new_str("x")]);
    run_boundary_in(&vm, label(), mixed).expect("Any 成员接受容器");
}

#[test]
fn non_union_label_shapes_are_not_regressed() {
    // 既有的三种非 union 标签形态（`TS-28`／`TS-29`／`TS-13`／`TS-31`）行为不变：边界 IN／OUT 各一条。
    let vm = Vm::new();

    // ① 类型对象：int 接受 int／bool，拒 str
    run_boundary_in(&vm, label_type(&vm, "int"), vm.instance.new_int(7)).expect("IN：int 接受 int");
    run_boundary_out(&vm, label_type(&vm, "int"), vm.instance.new_bool(true))
        .expect("OUT：bool ⊂ int 接受");
    let (type_name, _) = expect_rejected(
        &vm,
        run_boundary_in(&vm, label_type(&vm, "int"), vm.instance.new_str("x")),
    );
    assert_eq!(type_name, "TypeBoundaryError", "IN：str 该被 int 拒");
    let (type_name, _) = expect_rejected(
        &vm,
        run_boundary_out(&vm, label_type(&vm, "int"), vm.instance.new_str("x")),
    );
    assert_eq!(type_name, "TypeBoundaryError", "OUT：str 该被 int 拒");

    // ② `"Any"`：双向相容
    run_boundary_in(&vm, label_any(&vm), vm.instance.new_str("x")).expect("IN：Any 接受 str");
    let mixed = vm
        .instance
        .new_list(vec![vm.instance.new_int(1), vm.instance.new_str("x")]);
    run_boundary_out(&vm, label_any(&vm), mixed).expect("OUT：Any 接受容器");

    // ③ 复合标签 `(list, int)`：外类型先判、`TS-31` 深层递归元素
    let int_list = || vm.instance.new_tuple(vec![label_type(&vm, "list"), label_type(&vm, "int")]);
    let good = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_bool(true), // `bool ⊂ int`
    ]);
    run_boundary_in(&vm, int_list(), good).expect("IN：list[int] 接受 [1, True]");
    run_boundary_out(&vm, int_list(), vm.instance.new_list(vec![vm.instance.new_int(2)]))
        .expect("OUT：list[int] 接受 [2]");
    let (type_name, _) = expect_rejected(
        &vm,
        run_boundary_in(&vm, int_list(), vm.instance.new_str("not a list")),
    );
    assert_eq!(type_name, "TypeBoundaryError", "IN：外层不对该拒");
}

#[test]
fn a_union_label_is_enforced_on_assignment_and_on_boundary_in() {
    // 两条路都经由 `boundary_accepts` ⇒ union 语义两处同时生效（`CHECK_ASSIGNMENT`／`CHECK_BOUNDARY_IN`）。
    let vm = Vm::new();
    let label = || label_union(&vm, vec![label_type(&vm, "int"), label_type(&vm, "NoneType")]);

    // `CHECK_ASSIGNMENT`：`None` 通过；str 拒 ⇒ `TypeAssignmentError`（`TS-47`），消息的 expected 是 union
    run_assignment(&vm, label(), none(&vm)).expect("赋值检查：None 应当通过");
    run_assignment(&vm, label(), vm.instance.new_int(3)).expect("赋值检查：int 应当通过");
    let (type_name, message) = expect_rejected(
        &vm,
        run_assignment(&vm, label(), vm.instance.new_str("s")),
    );
    assert_eq!(type_name, "TypeAssignmentError", "TS-47：赋值不符的异常类型");
    let message = message.expect("消息要带期望／实际");
    assert!(
        message.contains("expected Union[int, NoneType]"),
        "赋值检查的 expected 要渲染 union（TS-11 同款期望元素）：{message}"
    );
    assert!(!message.contains("expected Any"), "union 不能渲染成 Any：{message}");
    assert!(message.contains("got str"), "实际类型：{message}");

    // `CHECK_BOUNDARY_IN`：同一标签、同一语义（`None` 通过、str 拒）
    run_boundary_in(&vm, label(), none(&vm)).expect("边界检查：None 应当通过");
    let (type_name, message) = expect_rejected(
        &vm,
        run_boundary_in(&vm, label(), vm.instance.new_str("s")),
    );
    assert_eq!(type_name, "TypeBoundaryError", "边界检查仍用边界异常（分工不同）");
    let message = message.expect("消息要带期望／实际");
    assert!(message.contains("Union[int, NoneType]"), "边界侧 expected：{message}");
}

#[test]
fn a_composite_label_with_a_type_object_head_is_not_a_union() {
    // 元组首项是**类型对象** ⇒ 既有复合标签 `(外, 内)`，**不是** union。
    // 反证：若被误判成 union（成员＝[int]），下面这些 list 值会被 int 成员拒掉。
    let vm = Vm::new();
    let int_list = || vm.instance.new_tuple(vec![label_type(&vm, "list"), label_type(&vm, "int")]);
    let good = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_bool(true), // `bool ⊂ int`（TS-40）
    ]);
    run_boundary_in(&vm, int_list(), good).expect("`(list, int)` 是复合标签：list 值按外层接受");
    run_boundary_out(&vm, int_list(), vm.instance.new_list(vec![vm.instance.new_int(2)]))
        .expect("OUT：`(list, int)` 仍按复合标签判");

    // `TS-31` 深层：元素不符 ⇒ 拒（复合标签的既有语义没被 union 分支改掉）
    let bad = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_str("x"),
    ]);
    let (type_name, message) = expect_rejected(&vm, run_boundary_in(&vm, int_list(), bad));
    assert_eq!(type_name, "TypeBoundaryError");
    let message = message.expect("消息要带期望／实际");
    assert!(message.contains("expected list[int]"), "复合标签的渲染：{message}");
    assert!(message.contains("got list"), "实际类型：{message}");

    // 外层不对 ⇒ 拒
    let (type_name, message) = expect_rejected(
        &vm,
        run_boundary_in(&vm, int_list(), vm.instance.new_str("nope")),
    );
    assert_eq!(type_name, "TypeBoundaryError");
    assert!(
        message.expect("消息").contains("expected list[int]"),
        "复合标签的渲染不能变"
    );

    // 裸 `list` 类型对象标签是**浅层**（`TS-13` 默认）：`list[str]` 值只看外类型 ⇒ 通过
    let shallow = vm
        .instance
        .new_list(vec![vm.instance.new_str("x"), vm.instance.new_str("y")]);
    run_boundary_in(&vm, label_type(&vm, "list"), shallow)
        .expect("裸 `list` 标签是浅层：list[str] 值按外层类型接受");
}
