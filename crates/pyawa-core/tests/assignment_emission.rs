//! **`TS-47` 的运行期发射**：扩展模式（`.pyawa`）下已标注变量的赋值要发
//! `CHECK_ASSIGNMENT`（`BC-65`），不符 ⇒ 运行期抛 `TypeAssignmentError`。
//!
//! 验收口径（task-4 ①-④⑥⑦）：
//! - ① 函数局部：初始标注赋值 ＋ **后续赋值**都检查 —— `y: int = 2; y = h()`，`h()` 返回
//!   `str` ⇒ 运行期抛 `TypeAssignmentError`（`TypeError` 子类）；返回 `int` 正常；
//! - ② 初始标注赋值不符（`x: int = "s"`）同样抛；
//! - ③ **纯 Python 模式**同源码合法、**不产生任何检查**（`TS-5`）；
//! - ④ 产物里：纯模式**没有** `CHECK_ASSIGNMENT`、扩展模式浅层**有**、未标注名字**没有**（`TS-2`）；
//! - ⑥ 嵌套 `?` 的变量标注 `x: list[int?]` 扩展模式编得过；纯模式仍 `Syntax`（`BC-15`）；
//! - ⑦ 模块级标注变量同样有检查（不是只有函数局部）。
//!
//! ⑤（`x: int? = None` 通过／`= "s"` 抛）依赖执行器侧的 **union 标签**分支（`TS-46`，标签形态
//! `("Union", 左, 右)`）——那一半与另一写手同轮落地，测试写在文件末尾（等联调后启用）。

mod common;

use pyawa_core::compile::{compile, instantiate, CheckTier, CompileError, CompiledUnit, Mode};
use pyawa_core::opcode;
use pyawa_core::{ExecError, Frame, Value};

use common::Vm;

/// 扩展模式编译（浅层档位、优化级 0：档位不改变本轮发射，固定死以消掉无关变量）。
fn compile_extension(source: &str) -> CompiledUnit {
    compile(source, "<t>", Mode::Extension, CheckTier::Shallow, 0)
        .unwrap_or_else(|error| panic!("扩展模式应当编得过 {source:?}，却报了 {error:?}"))
}

/// 纯 Python 模式编译（同上两个入参）。
fn compile_pure(source: &str) -> Result<CompiledUnit, CompileError> {
    compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0)
}

/// 端到端：编译（给定模式）⇒ 实例化 ⇒ 跑模块，返回模块返回值。
fn run_module<'a>(vm: &'a Vm, source: &str, mode: Mode) -> Result<Value<'a>, ExecError> {
    let unit = compile(source, "<t>", mode, CheckTier::Shallow, 0)
        .unwrap_or_else(|error| panic!("应当编得过 {source:?}（模式 {mode:?}），却报了 {error:?}"));
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(pyawa_core::ExecOutcome::Returned(value)) => Ok(value),
        Ok(pyawa_core::ExecOutcome::Yielded(_)) => panic!("顶层程序不该 yield"),
        Err(error) => Err(error),
    }
}

/// 跑一个**应当**抛类型赋值错的扩展模式模块，断言异常类型与消息（返回消息）。
fn expect_assignment_error(vm: &Vm, source: &str) -> String {
    let error = run_module(vm, source, Mode::Extension).expect_err("赋值不符必须抛错");
    assert!(matches!(error, ExecError::Raised { .. }), "失败必须抛异常：{error:?}");
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(
        type_name, "TypeAssignmentError",
        "TS-47：赋值不符的异常类型（`TS-12` 的 `TypeBoundaryError` 是另一类）"
    );
    // `TS-47`：`TypeAssignmentError` 必须是 `TypeError` 的子类（既有 `except TypeError` 能兜住）
    let assignment = vm
        .instance
        .type_named("TypeAssignmentError")
        .expect("TS-47：TypeAssignmentError 必须注册进实例异常表");
    let type_error = vm.instance.type_named("TypeError").expect("TypeError 在内建表里");
    assert!(
        vm.instance.is_subtype(assignment, type_error),
        "TS-47：TypeAssignmentError 必须是 TypeError 子类"
    );
    message.expect("消息要带期望／实际／位置")
}

/// 数一个编译单元（含嵌套 code 常量）里某条指令出现几次（按指令表宽度走路）。
fn count_op(unit: &CompiledUnit, name: &str) -> usize {
    let target = opcode::opcode(name).expect("指令在表里") as u8;
    let mut count = 0usize;
    let mut cursor = 0usize;
    while cursor + 1 < unit.code.len() {
        let number = unit.code[cursor];
        if number == target {
            count += 1;
        }
        let width = 2 * (1 + opcode::inline_cache_entries(u16::from(number)) as usize);
        cursor += width;
    }
    for constant in &unit.constants {
        if let pyawa_core::compile::Constant::Code(inner) = constant {
            count += count_op(inner, name);
        }
    }
    count
}

/// ① 函数局部：**初始标注赋值**与**后续赋值**都要在运行期检查；`str` 被拒、`int` 通过。
#[test]
fn a_later_assignment_to_an_annotated_local_is_checked_at_runtime() {
    // `h()` 返回 `str`：能静态判定的不符由编译期报（下一轮），静态判不出的必须运行期兜住。
    let bad = "def h():\n    return \"s\"\n\ndef g(h):\n    y: int = 2\n    y = h()\n    return y\n\nresult = g(h)\n";
    let vm = Vm::new();
    let message = expect_assignment_error(&vm, bad);
    assert!(message.contains("expected int"), "期望类型：{message}");
    assert!(message.contains("got str"), "实际类型：{message}");

    // 返回 `int` 时正常跑完（且 `g` 真的被调用过：模块里把结果存下来）。
    let good = "def h():\n    return 1\n\ndef g(h):\n    y: int = 2\n    y = h()\n    return y\n\nresult = g(h)\n";
    let vm = Vm::new();
    run_module(&vm, good, Mode::Extension).expect("`int` 赋给 `int` 标注应当通过");
}

/// ② 初始标注赋值不符（RHS 静态判不出、运行期确知是 `str`）⇒ 运行期 `TypeAssignmentError`。
///
/// `TS-47` ③：**能静态判定**的不符已由编译期检查器报 `CompileError::TypeAssignment`
/// （见 `tests/type_check.rs`）；这里守的是"**静态判不出的**必须运行期抛错"那一半——
/// RHS 用**未标注函数**的返回值，静态只知平台类型（`TS-48`）。
#[test]
fn an_initial_annotated_assignment_is_checked() {
    let vm = Vm::new();
    let message =
        expect_assignment_error(&vm, "def hidden():\n    return \"s\"\n\nx: int = hidden()\n");
    assert!(message.contains("expected int"), "期望类型：{message}");
    assert!(message.contains("got str"), "实际类型：{message}");

    // 正值：`int` 与 `bool ⊂ int`（`TS-40`）都通过。
    for source in ["x: int = 1\nx = 2\n", "x: int = True\nx = False\n"] {
        let vm = Vm::new();
        run_module(&vm, source, Mode::Extension)
            .unwrap_or_else(|error| panic!("`{source}` 应当通过赋值检查，却报了 {error:?}"));
    }
}

/// ③ **纯 Python 模式**：同源码合法，且**不产生任何检查**（`TS-5`）——错值也不抛。
#[test]
fn pure_python_mode_never_checks() {
    let sources = [
        // 初始标注赋值不符
        "x: int = \"s\"\n",
        // 后续赋值不符（函数局部）
        "def h():\n    return \"s\"\n\ndef g(h):\n    y: int = 2\n    y = h()\n    return y\n\nresult = g(h)\n",
    ];
    for source in sources {
        let vm = Vm::new();
        run_module(&vm, source, Mode::PurePython).unwrap_or_else(|error| {
            panic!("纯模式同源码必须合法且不检查（TS-5）：{source:?} ⇒ {error:?}")
        });
    }
}

/// ④ 产物里的指令：纯模式**没有** `CHECK_ASSIGNMENT`；扩展模式浅层**有**；未标注名字**没有**。
#[test]
fn checks_are_emitted_only_for_annotated_names_in_extension_mode() {
    // 初始标注赋值 ＋ 后续赋值 ⇒ 各一条检查（模块级名字目标）。
    let annotated = "x: int = 1\nx = 2\n";
    assert_eq!(
        count_op(&compile_extension(annotated), "CHECK_ASSIGNMENT"),
        2,
        "扩展模式：初始标注赋值与后续赋值各发一条 CHECK_ASSIGNMENT"
    );
    assert_eq!(
        count_op(&compile_pure(annotated).expect("纯模式编得过"), "CHECK_ASSIGNMENT"),
        0,
        "TS-5：纯 Python 模式不产生任何检查"
    );

    // `TS-2`：未标注的名字**不推断** ⇒ 不检查。
    assert_eq!(
        count_op(&compile_extension("x = 1\nx = 2\n"), "CHECK_ASSIGNMENT"),
        0,
        "TS-2：未标注名字不产生检查"
    );

    // 函数局部的检查在**嵌套 code 常量**里（不是模块顶层）。
    let local = "def g(h):\n    y: int = 2\n    y = h()\n    return y\n";
    assert_eq!(
        count_op(&compile_extension(local), "CHECK_ASSIGNMENT"),
        2,
        "函数局部的初始标注赋值与后续赋值各发一条"
    );
}

/// ⑥ 嵌套 `?` 的变量标注：扩展模式编得过（`TS-46` 收口）；纯模式仍 `Syntax`（`BC-15`）。
#[test]
fn nested_nullable_variable_annotation_compiles_in_extension_mode() {
    // 扩展模式：`x: list[int?]` ＝ `list[int | None]`（嵌套后缀由类型语法自己吃掉）。
    let _ = compile_extension("x: list[int?] = [1, None]\n");
    let _ = compile_extension("def g():\n    x: list[int?] = [1, None]\n    return x\n");
    // 纯 Python 模式：`?` 不是合法词素（`BC-15`）——嵌套位置也一样。
    let error = compile_pure("x: list[int?] = [1, None]\n").expect_err("纯模式必须报 `Syntax`");
    assert!(
        matches!(error, CompileError::Syntax(_)),
        "BC-15：纯 Python 模式下 `T?` 必须是 Syntax，实际 {error:?}"
    );
}

/// ⑦ 模块级标注变量同样有检查（不是只有函数局部）。
#[test]
fn module_level_annotated_variables_are_checked() {
    let vm = Vm::new();
    let message = expect_assignment_error(&vm, "def hidden():\n    return \"s\"\n\nx: int = 1\nx = hidden()\n");
    assert!(message.contains("expected int"), "期望类型：{message}");
    // 正值：同一形状的合法赋值通过。
    let vm = Vm::new();
    run_module(&vm, "x: int = 1\nx = 2\n", Mode::Extension).expect("模块级 int 赋值应当通过");
}

/// ⑤ `x: int? = None` 通过、`x: int? = "s"` 抛（`TS-46` 的检查语义：值为 `None` 通过，
/// 否则按 `T` 检查）。
///
/// 依赖执行器侧的 **union 标签**分支（标签形态 `("Union", 左, 右)`，与 `instantiate_constant`
/// 的编译侧契约一致）——那一半与另一写手同轮落地（`ds-union-labels`／task-5 已完成并全绿）。
#[test]
fn nullable_annotation_accepts_none_and_rejects_other_types() {
    // `None` 通过（T? 与 T 的区别就在这一条）。
    let vm = Vm::new();
    run_module(&vm, "x: int? = None\n", Mode::Extension).expect("`int?` 必须接受 `None`");
    // `int` 也通过。
    run_module(&vm, "x: int? = 1\n", Mode::Extension).expect("`int?` 必须接受 `int`");
    // 不是 `int` 也不是 `None` ⇒ 抛。
    let vm = Vm::new();
    let message =
        expect_assignment_error(&vm, "def hidden():\n    return \"s\"\n\nx: int? = hidden()\n");
    assert!(message.contains("expected"), "消息要带期望标签：{message}");
}
