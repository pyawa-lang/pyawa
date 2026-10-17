//! **M4 判据① 归档**：`T-BC-8`（检查点可见）／`T-BC-9`（归责信息完整）的可执行证据。
//!
//! 判据唯一出处：`docs/PLAN-milestones.md` §6 的 M4 行——
//! ① `T-BC-8`／`T-BC-9` 可观测（检查点可见、归责信息完整）；② 渐进保证。
//! 本文件是**判据①**的归档测试；判据②（`T-TS-1`）的成对语料验收在
//! `tests/gradual_guarantee.rs`，两者配成 M4 收口的直接证据。
//!
//! 判据定义：`docs/SPEC-bytecode.md` §6 测试表（`T-BC-8`／`T-BC-9`）与 §4.1 的
//! `BC-25`／`BC-26`／`BC-27`／`BC-28`／`BC-65`。
//!
//! ## 一处真相：详细断言在既有测试，本文件是"判据口径"的归档复验
//!
//! 既有覆盖（**不重复**它们的断言细节，只把判据 → 测试指路）：
//!
//! - `tests/boundary.rs`：`annotations_become_boundary_checks_only_in_extension_mode_and_deep_tier`
//!   （`BC-25`② 的发射条件：扩展模式＋深层档位＋带标注）、
//!   `a_failed_incoming_check_blames_the_caller`／`a_failed_return_check_blames_the_callee`
//!   （`T-BC-9` 四要素用的手写字节码）、`boundary_metadata_is_right`（`BC-27` 边界指令元数据）。
//! - `tests/assignment_check.rs`：`assignment_metadata_is_right`（`BC-65` 元数据）、
//!   `a_rejected_assignment_raises_type_assignment_error`（赋值失败消息与异常类）。
//! - `tests/assignment_emission.rs`：`checks_are_emitted_only_for_annotated_names_in_extension_mode`
//!   （`CHECK_ASSIGNMENT` 的发射条件与计数）。
//!
//! 本文件只补**判据层**的三件事：①检查点在 `dis` 可见性口径下看得到；②从**编译器端到端**
//! （`compile → instantiate → execute`）看归责异常带齐四要素；③三条 Pyawa 专有检查指令的元数据
//! 聚合断言（`has_arg`／`stack_effect`／`opname`）。
//!
//! ## 已知缺口（如实注记，**不**作断言、不阻塞"可观测"）
//!
//! `BC-25`① 要求"**两侧皆标注**处禁止发射"——这需要**跨模块静态信息**（被调用方要知道调用方
//! 是否已标注）；**当前未实现**。工程现状是"**该函数带标注就保守发射**"（扩展模式＋深层档位），
//! 不检查调用方是否已标注 ⇒ `T-BC-8` 的"两侧皆标注处**没有**该指令"这半条**不作断言**，
//! 如实登记为 M4 的已知跨模块缺口（台账 `NEXT.md` 已记）。“可观测”这一判据本身由发射条件
//! （有标注／无标注、扩展／纯模式）成立。

mod common;

use pyawa_core::compile::{compile, instantiate, CheckTier, CompiledUnit, Constant, Mode};
use pyawa_core::{ExecError, Frame};

use common::Vm;

/// `compile → instantiate` **测试路径**的固定 `co_filename`（`instantiate` 的文档写明：这条路径
/// 供测试用，正式 loader 接上时会给真实路径）。`T-BC-9` 只要求"位置要素（文件名 ＋ 行号）"存在，
/// 所以本文件断言的是产物里**真实的** `co_filename` 与位置表行号，不把具体值当判据文本。
const TEST_FILENAME: &str = "<pyawa-test>";

// --------------------------------------------------------------------------- #
// 脚手架：产物反汇编（看检查点）＋ 端到端执行（看归责消息）
// --------------------------------------------------------------------------- #

/// 扩展模式编译（档位显式给：边界检查只在深层发，赋值检查浅层就发）。
fn extension(source: &str, filename: &str, tier: CheckTier) -> CompiledUnit {
    compile(source, filename, Mode::Extension, tier, 0)
        .unwrap_or_else(|error| panic!("扩展模式应当编得过 {source:?}，却报了 {error:?}"))
}

/// 纯 Python 模式编译（`BC-25`②：专有指令禁止出现在 `.py` 产物里）。
fn pure_python(source: &str, filename: &str, tier: CheckTier) -> CompiledUnit {
    compile(source, filename, Mode::PurePython, tier, 0)
        .unwrap_or_else(|error| panic!("纯 Python 模式应当编得过 {source:?}，却报了 {error:?}"))
}

/// 一段 code 的指令名序列（照 `boundary.rs` 的范式：`Decoder` 逐指令取 `opname`）。
///
/// `BC-28` 的收益就是"检查点在反汇编里可见"——`opname` 能读出 `CHECK_BOUNDARY_IN` 一类名字，
/// 而不是数字占位符。
fn names_of(code: &[u8]) -> Vec<String> {
    let mut decoder = pyawa_core::decode::Decoder::new(code);
    let mut names = Vec::new();
    while let Ok(Some(instruction)) = decoder.next_instruction() {
        names.push(
            pyawa_core::opcode::opname(u16::from(instruction.opcode))
                .unwrap_or("<未知>")
                .to_owned(),
        );
    }
    names
}

/// 取模块产物里某个嵌套 code 单元（`Constant::Code`）。
fn nested<'a>(unit: &'a CompiledUnit, name: &str) -> &'a CompiledUnit {
    unit.constants
        .iter()
        .find_map(|constant| match constant {
            Constant::Code(inner) if inner.name == name => Some(&**inner),
            _ => None,
        })
        .unwrap_or_else(|| panic!("产物里应当有 {name} 的 code"))
}

/// 某个嵌套 code 单元的指令名序列。
fn nested_names(unit: &CompiledUnit, name: &str) -> Vec<String> {
    names_of(&nested(unit, name).code)
}

fn has(names: &[String], target: &str) -> bool {
    names.iter().any(|name| name == target)
}

/// 端到端跑一条**应当抛脚本异常**的语料：`compile`（扩展模式）→ `instantiate` → `execute`，
/// 返回（异常类名，消息）。判据①的"归责信息完整"要在**真实编译产物**上也成立，
/// 不能只有手写字节码成立（那是 `boundary.rs` 的覆盖）。
fn run_expecting_raise(
    vm: &Vm,
    source: &str,
    filename: &str,
    tier: CheckTier,
) -> (String, Option<String>) {
    let unit = extension(source, filename, tier);
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        // 帧仍活着 ⇒ 命名空间与异常对象都还没释放。
        Err(ExecError::Raised { .. }) => {}
        Err(other) => panic!("应当是脚本异常（归责检查失败），实际执行器错误：{other:?}"),
        Ok(_) => panic!("这条语料必须抛类型错误，却正常跑完了：{source:?}"),
    }
    vm.pending_exception()
        .unwrap_or_else(|| panic!("脚本异常必须留下 pending_exception：{source:?}"))
}

/// `TS-12`／`TS-47`：两个归责异常都必须是 `TypeError` 的子类（既有 `except TypeError` 要能兜住）。
fn assert_type_error_subclass(vm: &Vm, type_name: &str) {
    let ty = vm
        .instance
        .type_named(type_name)
        .unwrap_or_else(|| panic!("{type_name} 必须注册进实例异常表"));
    let type_error = vm.instance.type_named("TypeError").expect("TypeError 在内建表里");
    assert!(
        vm.instance.is_subtype(ty, type_error),
        "{type_name} 必须是 TypeError 的子类（TS-12／TS-47）"
    );
}

/// `T-BC-9` 的"边界位置"要素：消息里有**文件名**，且 `line ` 后面跟的是**真的行号数字**
/// （不把具体行号写死：行号由位置表 `BC-18` 给，属实现可维护的值，不是判据文本）。
fn assert_position_in(message: &str, filename: &str) {
    assert!(message.contains(filename), "位置要素缺文件名 {filename}：{message}");
    let after_line = message
        .split("line ")
        .nth(1)
        .unwrap_or_else(|| panic!("位置要素缺 `line N`：{message}"));
    assert!(
        after_line.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "`line ` 后面必须是行号数字：{message}"
    );
}

// --------------------------------------------------------------------------- #
// `T-BC-8`：检查点可见（`BC-25`／`BC-28`／`BC-65`）
// --------------------------------------------------------------------------- #

/// **`T-BC-8`**：标注／未标注交界处**能观察到检查指令**；未标注处与纯 Python 模式观察不到。
///
/// 观测口径就是 `BC-28` 说的"`dis` 里可见"：用 `Decoder` ＋ `opname` 逐指令读，检查点必须
/// 显示成 `CHECK_BOUNDARY_IN`／`CHECK_BOUNDARY_OUT`／`CHECK_ASSIGNMENT` 而不是数字占位符。
#[test]
fn tbc8_checkpoints_are_visible_at_annotated_boundaries() {
    // ① 边界检查：扩展模式＋深层档位＋带标注函数 ⇒ 两条检查指令都在**嵌套 code**里。
    let deep = extension("def f(x: int) -> int:\n    return x\n", "<m4>", CheckTier::Deep);
    let f_ops = nested_names(&deep, "f");
    assert!(
        has(&f_ops, "CHECK_BOUNDARY_IN"),
        "T-BC-8：带标注函数的入参检查必须在反汇编里可见：{f_ops:?}"
    );
    assert!(
        has(&f_ops, "CHECK_BOUNDARY_OUT"),
        "T-BC-8：带标注函数的返回值检查必须在反汇编里可见：{f_ops:?}"
    );

    // ② 未标注函数 ⇒ 观察不到（`TS-1`／`TS-2`：没标注就没有边界检查）。
    let plain = extension(
        "def h(x):\n    return x\n",
        "<m4>",
        CheckTier::Deep,
    );
    let h_ops = nested_names(&plain, "h");
    assert!(
        !has(&h_ops, "CHECK_BOUNDARY_IN") && !has(&h_ops, "CHECK_BOUNDARY_OUT"),
        "T-BC-8：未标注函数不得有边界检查指令：{h_ops:?}"
    );

    // ③ 纯 Python 模式 ⇒ 观察不到（`BC-25`②：专有指令禁止出现在纯模式产物里）。
    let pure = pure_python("def f(x: int) -> int:\n    return x\n", "<m4>", CheckTier::Deep);
    let pure_ops = nested_names(&pure, "f");
    assert!(
        !has(&pure_ops, "CHECK_BOUNDARY_IN") && !has(&pure_ops, "CHECK_BOUNDARY_OUT"),
        "T-BC-8／BC-25②：纯 Python 模式不得产出边界检查指令：{pure_ops:?}"
    );

    // ④ "交界处"的口径：检查在**被调用方**的序言里（`BC-25`② 第二段）——
    //    已标注 `f` 被未标注 `g` 调用时，`f` 里仍有检查，`g` 自己（未标注）没有。
    let mixed = extension(
        "def f(x: int) -> int:\n    return x\ndef g(f, x):\n    return f(x)\n",
        "<m4>",
        CheckTier::Deep,
    );
    assert!(
        has(&nested_names(&mixed, "f"), "CHECK_BOUNDARY_IN"),
        "T-BC-8：已标注被调用方的序言里必须有入参检查"
    );
    let g_ops = nested_names(&mixed, "g");
    assert!(
        !has(&g_ops, "CHECK_BOUNDARY_IN") && !has(&g_ops, "CHECK_BOUNDARY_OUT"),
        "T-BC-8：未标注调用方自己没有边界检查指令：{g_ops:?}"
    );

    // ⑤ 赋值检查（`BC-65`）：扩展模式下**已标注变量**的赋值 ⇒ 可见（模块级）。
    let module_assignment = extension("x: int = 1\nx = 2\n", "<m4>", CheckTier::Shallow);
    assert!(
        has(&names_of(&module_assignment.code), "CHECK_ASSIGNMENT"),
        "T-BC-8／BC-65：已标注模块级变量的赋值检查必须可见"
    );
    // 函数局部：检查在嵌套 code 里。
    let local_assignment = extension(
        "def g(h):\n    y: int = 2\n    y = h()\n    return y\n",
        "<m4>",
        CheckTier::Shallow,
    );
    assert!(
        has(&nested_names(&local_assignment, "g"), "CHECK_ASSIGNMENT"),
        "T-BC-8／BC-65：已标注局部变量的赋值检查必须可见"
    );

    // ⑥ 未标注名字 ⇒ 观察不到（`TS-2`：不为未标注的名字推断 ⇒ 不检查）。
    let unannotated_module = extension("x = 1\nx = 2\n", "<m4>", CheckTier::Shallow);
    assert!(
        !has(&names_of(&unannotated_module.code), "CHECK_ASSIGNMENT"),
        "T-BC-8／BC-65：未标注名字不得有赋值检查指令"
    );
    let unannotated_local = extension(
        "def g(h):\n    y = 2\n    y = h()\n    return y\n",
        "<m4>",
        CheckTier::Shallow,
    );
    assert!(
        !has(&nested_names(&unannotated_local, "g"), "CHECK_ASSIGNMENT"),
        "T-BC-8／BC-65：未标注局部名不得有赋值检查指令"
    );

    // ⑦ 纯 Python 模式 ⇒ 赋值检查也观察不到（`BC-25`②）。
    let pure_assignment = pure_python("x: int = 1\nx = 2\n", "<m4>", CheckTier::Shallow);
    assert!(
        !has(&names_of(&pure_assignment.code), "CHECK_ASSIGNMENT"),
        "T-BC-8／BC-65：纯 Python 模式不得产出赋值检查指令"
    );

    // 如实注记：`BC-25`①"两侧皆标注处禁止发射"需要跨模块静态信息，当前未实现 ⇒ 不作断言
    // （见文件头"已知缺口"）：上面 ④ 只验"检查在被调用方序言里"，不验"调用方已标注就不发"。
}

// --------------------------------------------------------------------------- #
// `T-BC-9`：归责信息完整（`BC-26`／`TS-11`／`TS-12`）
// --------------------------------------------------------------------------- #

/// **`T-BC-9`**：检查失败时异常携带**方向、期望类型、实际类型、文件名与行号**。
///
/// 观测点是**编译器端到端**（真实产物上的检查指令），手写字节码那一层由 `boundary.rs`／
/// `assignment_check.rs` 钉住（一处真相）。坏赋值特意用**静态检查器判不出**的来源
/// （未标注函数的返回值 ＝ 平台类型，`TS-48`），让运行期 `CHECK_ASSIGNMENT` 真的兜住。
#[test]
fn tbc9_type_errors_carry_direction_expected_actual_and_position() {
    // ① 坏实参 ⇒ `TypeBoundaryError`，方向 argument，归责调用方（`TS-10`）。
    let vm = Vm::new();
    let (type_name, message) = run_expecting_raise(
        &vm,
        "def f(x: int) -> int:\n    return x\nresult = f(\"s\")\n",
        TEST_FILENAME,
        CheckTier::Deep,
    );
    let message = message.expect("失败消息必须带四要素");
    assert_eq!(type_name, "TypeBoundaryError", "TS-12：入参检查失败的异常类");
    assert!(message.contains("argument"), "方向要素（入参）：{message}");
    assert!(message.contains("expected int"), "期望类型要素：{message}");
    assert!(message.contains("got str"), "实际类型要素：{message}");
    assert_position_in(&message, TEST_FILENAME);
    assert_type_error_subclass(&vm, "TypeBoundaryError");

    // ② 坏返回值 ⇒ `TypeBoundaryError`，方向 return，归责被调用方（`TS-10`）。
    let vm = Vm::new();
    let (type_name, message) = run_expecting_raise(
        &vm,
        "def f() -> int:\n    return \"s\"\nresult = f()\n",
        TEST_FILENAME,
        CheckTier::Deep,
    );
    let message = message.expect("失败消息必须带四要素");
    assert_eq!(type_name, "TypeBoundaryError", "TS-12：返回值检查失败的异常类");
    assert!(message.contains("return"), "方向要素（返回值）：{message}");
    assert!(message.contains("expected int"), "期望类型要素：{message}");
    assert!(message.contains("got str"), "实际类型要素：{message}");
    assert_position_in(&message, TEST_FILENAME);
    assert_type_error_subclass(&vm, "TypeBoundaryError");

    // ③ 坏赋值 ⇒ `TypeAssignmentError`（`TS-47`；与边界异常同族、分工不同）。
    //    `y: int = 2; y = h()`，`h` 未标注 ⇒ RHS 是平台类型，静态判不出 ⇒ 运行期兜住。
    let vm = Vm::new();
    let (type_name, message) = run_expecting_raise(
        &vm,
        "def h():\n    return \"s\"\ndef g(h):\n    y: int = 2\n    y = h()\n    return y\nresult = g(h)\n",
        TEST_FILENAME,
        CheckTier::Shallow,
    );
    let message = message.expect("失败消息必须带期望／实际／位置");
    assert_eq!(type_name, "TypeAssignmentError", "TS-47：赋值检查失败的异常类");
    // `TS-47` 没有"方向"要素（那是边界的 `TS-11`）；赋值语义由消息里的 `assignment` 标明。
    assert!(message.contains("assignment"), "赋值检查的语义标识：{message}");
    assert!(message.contains("expected int"), "期望类型要素：{message}");
    assert!(message.contains("got str"), "实际类型要素：{message}");
    assert_position_in(&message, TEST_FILENAME);
    assert_type_error_subclass(&vm, "TypeAssignmentError");

    // 两个归责异常**同族、分工不同**：都是 TypeError 子类，但不是同一个类。
    let boundary = vm
        .instance
        .type_named("TypeBoundaryError")
        .expect("TypeBoundaryError 在实例异常表里");
    let assignment = vm
        .instance
        .type_named("TypeAssignmentError")
        .expect("TypeAssignmentError 在实例异常表里");
    assert_ne!(boundary, assignment, "TS-12／TS-47：边界与赋值是两个异常类");
}

// --------------------------------------------------------------------------- #
// `BC-27`：三条检查指令的元数据（聚合断言；**不发明新测试编号**——元数据细节的既有测试
// 在 `tests/boundary.rs`／`tests/assignment_check.rs`）
// --------------------------------------------------------------------------- #

/// **`BC-27`**（`BC-65` 同款要求）：`CHECK_BOUNDARY_IN`／`CHECK_BOUNDARY_OUT`／
/// `CHECK_ASSIGNMENT` 都 `has_arg`＝真、`stack_effect`＝0、`opname` 可读（`dis` 可见性，`BC-28`）。
///
/// 单条指令的细节元数据已由 `boundary.rs`／`assignment_check.rs` 分别钉住；这里是判据①的
/// **聚合**复验（三条检查点一起看）。
#[test]
fn tbc27_check_instruction_metadata_is_dis_visible() {
    for name in [
        "CHECK_BOUNDARY_IN",
        "CHECK_BOUNDARY_OUT",
        "CHECK_ASSIGNMENT",
    ] {
        let number = pyawa_core::opcode::opcode(name)
            .unwrap_or_else(|| panic!("BC-23／BC-65：{name} 必须在指令表里"));
        assert!(
            pyawa_core::opcode::has_arg(number),
            "BC-27：{name} 必须带 oparg"
        );
        assert_eq!(
            pyawa_core::opcode::opname(number),
            Some(name),
            "BC-28：`dis` 必须能按 opname 读出 {name}（检查点可见）"
        );
        // 栈效应与 oparg 无关（BC-27：stack_effect 必须正确处理）。
        for oparg in [Some(0), Some(200)] {
            assert_eq!(
                pyawa_core::opcode::stack_effect(number, oparg, None).expect("算得出栈效应"),
                0,
                "BC-27：{name} 不动值栈"
            );
        }
        assert!(
            number < pyawa_core::opcode_metadata::MIN_INSTRUMENTED_OPCODE,
            "BC-31：{name} 不得占用 instrumented 区段"
        );
        assert!(
            pyawa_core::opcode_metadata::PYAWA_SPECIFIC
                .iter()
                .any(|(candidate, _)| *candidate == name),
            "BC-28／BC-65：{name} 必须是 Pyawa 专有指令"
        );
    }
}
