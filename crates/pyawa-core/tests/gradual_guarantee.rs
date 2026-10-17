//! **`T-TS-1` 渐进保证**（`TS-7`／`TS-9`）的可执行验收，含 `T-TS-3`／`T-TS-7`。
//!
//! 判据原文（`SPEC-type-system.md` §5 与 §13 的测试表）：
//!
//! - **`TS-7`**（渐进保证）：对同一语料**增删标注**，**除类型错误外行为必须不变**
//!   （Siek／Vitousek／Cimini／Boyland, *Refined Criteria for Gradual Typing*, 2015）。
//! - **`TS-9`**：`TS-7`／`TS-8` 两条必达性质**必须各有可执行验收**（`T-TS-1`／`T-TS-2`），
//!   **禁止**只作声明 ⇒ 本文件是 `T-TS-1` 的可执行证据。
//! - **`T-TS-3`**：未标注代码**不产生**类型错误（`TS-1`／`TS-2`）。
//! - **`T-TS-7`**：两种模式下标注都保留且可读；**校验只在扩展模式发生**（`TS-5`）。
//!
//! ## `T-TS-1` 的可执行判定（本文件的口径）
//!
//! 语料是**成对程序**：`annotated` 版带标注，`plain` 版把标注**全删**（其余逐字符相同）。
//! 两版都用 [`Mode::Extension`] 编译（无标注源码在扩展模式下同样合法；`T?` 只出现在注解版里），
//! 同档位 [`CheckTier`] 各自 `compile → instantiate → execute`，取模块变量 `result` 的
//! `repr` 投影（内建值的 repr 是确定性的；语料不涉及时钟／地址／随机）。
//!
//! 判定规则（即 `TS-7` 的直接翻译）：
//!
//! 1. **去注解版必须编得过、且不得产生任何类型错误**（`T-TS-3`）。"类型错误"在本仓库的口径
//!    只有两类运行期异常（`TypeAssignmentError`／`TypeBoundaryError`，`TS-12`／`TS-47`）与
//!    编译期静态类型错误（`TS-36`／`TS-47` 的静态那一半，即
//!    `CompileError::TypeAssignment`）。**别的编译错误不算**类型错误、照常红。
//! 2. **注解版抛类型错误 ⇒ 允许**——这是 `TS-7` 里**唯一**允许的行为差异，本对不再比较返回值。
//!    类型错误可能出现在**编译期**（静态检查器"能静态判定的不符"）或**运行期**（检查指令），
//!    两种都算（`TS-47` 的两层检查）。
//! 3. **注解版正常返回 ⇒ 返回值的可观察投影必须与去注解版相等**。
//! 4. **其它异常两边必须一致**（异常类名 ＋ 消息）；注解版多抛／少抛非类型异常都算行为变化。
//!
//! ## `T-TS-2`／`T-TS-4` 的入口（本文件**不重复**，一处真相）
//!
//! 边界归责方向与四要素已由 `tests/boundary.rs` 覆盖（同轮不重写）：
//! `a_failed_incoming_check_blames_the_caller`（入参方向 ＋ 四要素）、
//! `a_failed_return_check_blames_the_callee`（返回值方向）、
//! `the_deep_tier_checks_container_elements`（带内层的标签）。本文件只做"增删标注 ⇒ 行为不变"
//! 的成对比较，不复述四要素的文本形态。
//!
//! ## 已知覆盖边界（如实）
//!
//! - 变量标注**不**进模块／函数的 `__annotations__`（本层如实登记的缺口）⇒ `T-TS-7` 的
//!   "可读"路径用**函数注解**（`__annotate__`／`__annotations__`）这条**真的能读**的通路。
//! - 语料只覆盖编译器已接线的语法面；未覆盖面见本轮报告（不在断言里假装覆盖）。

mod common;

use pyawa_core::compile::{compile, instantiate, CheckTier, CompileError, Mode};
use pyawa_core::{ExecError, Frame};

use common::Vm;

// --------------------------------------------------------------------------- #
// 端到端执行：一条语料 ⇒ 可观察结果
// --------------------------------------------------------------------------- #

/// 一条语料的**可观察结果**。
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// 正常跑完：模块变量 `result` 的 `repr` 投影。
    Returned(String),
    /// 运行期**类型错误**（`TS-12` 的 `TypeBoundaryError` 或 `TS-47` 的 `TypeAssignmentError`）——
    /// `T-TS-1` **允许**注解版出现这个差异。
    TypeError { type_name: String, message: Option<String> },
    /// 运行期的**其它**异常：类名 ＋ 消息，两边必须一致。
    Raised { type_name: String, message: Option<String> },
    /// **编译期**类型错误：`CompileError::TypeAssignment`（`TS-36`／`TS-47` 的静态那一半，
    /// 静态检查器"能静态判定的不符"）——同样算类型错误，`T-TS-1` 允许注解版如此。
    TypeErrorCompile(String),
    /// 编译期的其它失败（`Syntax`／`Unsupported`）：这不是"类型错误"，算行为变化。
    BrokenCompile(String),
}

/// `CompileError` 是不是**编译期类型错误**（`TS-36`／`TS-47` 的静态那一半）。
///
/// **显式点名** `CompileError::TypeAssignment`：`T-TS-1` 只允许"类型错误"这一种差异，
/// `Syntax`／`Unsupported` 以及将来其它编译错误**一律不算**（照常红），免得静态检查器
/// 以后多出别的错误也被误放行。
fn is_compile_type_error(error: &CompileError) -> bool {
    matches!(error, CompileError::TypeAssignment(_))
}

/// 装一个最小的 `builtins` 映射（内建类型名）：注解表达式（`__annotate__` 里的 `int`）走
/// `LOAD_GLOBAL` ⇒ 先查全局、再查 `builtins`（`BC-57`）。不给的话读注解会报 `NameError`。
fn vm_with_builtins() -> Vm {
    let vm = Vm::new();
    let builtins = vm.instance.new_dict();
    for name in ["int", "str", "bool", "list", "dict", "float"] {
        if let Some(ty) = vm.instance.type_named(name) {
            let value = vm.instance.type_value(ty);
            vm.instance.dict_set(builtins, name, value);
        }
    }
    vm.instance.set_builtins(Some(builtins));
    vm
}

/// `compile → instantiate → execute` 一条语料，返回[可观察结果](Outcome)。
///
/// 执行成功后立刻在**帧还活着**时读命名空间里的 `result`（帧持有命名空间的那份引用，
/// 帧一释放映射就跟着释放——先取指针后读会是 use-after-free）。
fn run(vm: &Vm, source: &str, mode: Mode, tier: CheckTier) -> Outcome {
    let unit = match compile(source, "<gradual>", mode, tier, 0) {
        Ok(unit) => unit,
        Err(error) => {
            let detail = format!("{error:?}");
            return if is_compile_type_error(&error) {
                Outcome::TypeErrorCompile(detail)
            } else {
                Outcome::BrokenCompile(detail)
            };
        }
    };
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(pyawa_core::ExecOutcome::Returned(_)) => {
            let value = vm
                .instance
                .dict_get(namespace, "result")
                .unwrap_or_else(|| panic!("语料必须留下 `result` 变量：{source:?}"));
            let projection = vm
                .instance
                .object_repr(value)
                .unwrap_or_else(|error| panic!("`result` 的 repr 失败：{error:?}"));
            Outcome::Returned(projection)
        }
        Ok(pyawa_core::ExecOutcome::Yielded(_)) => panic!("顶层程序不该 yield：{source:?}"),
        Err(ExecError::Raised { .. }) => {
            let (type_name, message) = vm
                .pending_exception()
                .unwrap_or_else(|| panic!("脚本异常必须留下 pending_exception：{source:?}"));
            if is_runtime_type_error(&type_name) {
                Outcome::TypeError { type_name, message }
            } else {
                Outcome::Raised { type_name, message }
            }
        }
        // `Decode`／`Frame`／`Unsupported` 一类是"产物坏了"，不是语料语义 ⇒ 直接失败。
        Err(other) => panic!("编译产物的执行不该给出非脚本异常 {other:?}：{source:?}"),
    }
}

/// `TS-12`／`TS-47`：类型系统的两类运行期归责异常。
fn is_runtime_type_error(type_name: &str) -> bool {
    type_name == "TypeAssignmentError" || type_name == "TypeBoundaryError"
}

// --------------------------------------------------------------------------- #
// 成对语料 ＋ `T-TS-1` 判定
// --------------------------------------------------------------------------- #

/// 一对语料的**预期分支**（决定比较走哪条规则）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expectation {
    /// 类型正确的语料：注解版**必须正常返回**，且投影与去注解版相等。
    /// 这条是**反空转**的：若静态检查器把合法程序一律判成类型错误，这里会红。
    SameValue,
    /// 语料故意含类型不符：注解版**必须报类型错误**（编译期或运行期皆可）——`T-TS-1` 的例外。
    TypeErrorRequired,
    /// 两边都抛**同一个非类型异常**：类名 ＋ 消息必须相等。
    SameNonTypeError,
    /// `T?`／收窄／容器深标签等"可能被静态检查器提前判出"的语料：注解版允许类型错误；
    /// 若正常返回则投影必须与去注解版相等。
    TypeErrorAllowed,
}

/// 一对语料：注解版／去注解版（除标注外逐字符相同）＋ 检查档位 ＋ 预期分支。
struct Pair {
    name: &'static str,
    annotated: &'static str,
    plain: &'static str,
    tier: CheckTier,
    expectation: Expectation,
}

/// `T-TS-1`／`T-TS-3` 的判定本体：跑一对语料，按 [`Expectation`] 比较。
fn check_pair(pair: &Pair) {
    let annotated_vm = vm_with_builtins();
    let annotated = run(&annotated_vm, pair.annotated, Mode::Extension, pair.tier);
    let plain_vm = vm_with_builtins();
    let plain = run(&plain_vm, pair.plain, Mode::Extension, pair.tier);

    // 规则 1（`T-TS-3`）：去注解版必须编得过、且**禁止**任何类型错误。
    match &plain {
        Outcome::BrokenCompile(detail) => {
            panic!("[{}] 去注解版必须编得过，却报了 {detail}", pair.name)
        }
        Outcome::TypeErrorCompile(detail) => panic!(
            "[{}] T-TS-3：去注解版不得被静态检查器判成类型错误（{detail}）",
            pair.name
        ),
        Outcome::TypeError { type_name, message } => panic!(
            "[{}] T-TS-3：去注解版不得抛类型错误（{type_name}：{message:?}）",
            pair.name
        ),
        _ => {}
    }

    match pair.expectation {
        // 规则 3：注解版正常返回 ⇒ 投影必须相等（且对"合法语料"强制要求走到这条路）。
        Expectation::SameValue => match &annotated {
            Outcome::Returned(value) => match &plain {
                Outcome::Returned(plain_value) => assert_eq!(
                    value, plain_value,
                    "[{}] T-TS-1：增删标注后返回值的可观察投影必须相等",
                    pair.name
                ),
                other => panic!("[{}] 去注解版应当正常返回，实际 {other:?}", pair.name),
            },
            Outcome::TypeErrorCompile(detail) => panic!(
                "[{}] T-TS-1：类型正确的语料不得被静态检查器判成类型错误（{detail}）",
                pair.name
            ),
            Outcome::BrokenCompile(detail) => panic!(
                "[{}] 类型正确的语料必须编得过，却报了 {detail}",
                pair.name
            ),
            Outcome::TypeError { type_name, message } => panic!(
                "[{}] T-TS-1：类型正确的语料注解版不得抛类型错误（{type_name}：{message:?}）",
                pair.name
            ),
            Outcome::Raised { type_name, message } => panic!(
                "[{}] T-TS-1：类型正确的语料注解版不得抛异常（{type_name}：{message:?}）",
                pair.name
            ),
        },
        // 规则 2：注解版**必须**报类型错误，且两种时机（编译期／运行期）都算。
        Expectation::TypeErrorRequired => match &annotated {
            Outcome::TypeError { .. } | Outcome::TypeErrorCompile(_) => {}
            Outcome::BrokenCompile(detail) => panic!(
                "[{}] 注解版必须是类型错误（不是语法／未接线）：{detail}",
                pair.name
            ),
            Outcome::Returned(value) => panic!(
                "[{}] T-TS-1：类型不符必须被检查拦下，注解版却返回了 {value}",
                pair.name
            ),
            Outcome::Raised { type_name, message } => panic!(
                "[{}] 注解版必须抛类型错误（`TypeAssignmentError`／`TypeBoundaryError`），实际 {type_name}：{message:?}",
                pair.name
            ),
        },
        // `T?`／收窄／深标签：允许类型错误；否则两边必须一致。
        Expectation::TypeErrorAllowed => match (&annotated, &plain) {
            (Outcome::TypeError { .. }, _) | (Outcome::TypeErrorCompile(_), _) => {}
            (Outcome::BrokenCompile(detail), _) => panic!(
                "[{}] 注解版必须是类型错误（不是语法／未接线）：{detail}",
                pair.name
            ),
            (Outcome::Returned(value), Outcome::Returned(plain_value)) => assert_eq!(
                value, plain_value,
                "[{}] T-TS-1：增删标注后返回值的可观察投影必须相等",
                pair.name
            ),
            (
                Outcome::Raised {
                    type_name,
                    message,
                },
                Outcome::Raised {
                    type_name: plain_type,
                    message: plain_message,
                },
            ) => {
                assert_eq!(
                    type_name, plain_type,
                    "[{}] T-TS-1：其它异常的类名必须一致",
                    pair.name
                );
                assert_eq!(
                    message, plain_message,
                    "[{}] T-TS-1：其它异常的消息必须一致",
                    pair.name
                );
            }
            (Outcome::Returned(value), other) => panic!(
                "[{}] T-TS-1：注解版返回了 {value}，去注解版却是 {other:?}",
                pair.name
            ),
            (Outcome::Raised { type_name, .. }, other) => panic!(
                "[{}] T-TS-1：注解版抛了非类型异常 {type_name}，去注解版却是 {other:?}",
                pair.name
            ),
        },
        // 规则 4：两边同一个非类型异常。
        Expectation::SameNonTypeError => match &annotated {
            Outcome::Raised {
                type_name,
                message,
            } => match &plain {
                Outcome::Raised {
                    type_name: plain_type,
                    message: plain_message,
                } => {
                    assert_eq!(
                        type_name, plain_type,
                        "[{}] T-TS-1：其它异常的类名必须一致",
                        pair.name
                    );
                    assert_eq!(
                        message, plain_message,
                        "[{}] T-TS-1：其它异常的消息必须一致",
                        pair.name
                    );
                }
                other => panic!(
                    "[{}] T-TS-1：注解版抛了 {type_name}，去注解版却是 {other:?}",
                    pair.name
                ),
            },
            Outcome::TypeError { type_name, message } => panic!(
                "[{}] 本对应抛非类型异常，注解版却抛了类型错误 {type_name}：{message:?}",
                pair.name
            ),
            Outcome::TypeErrorCompile(detail) => panic!(
                "[{}] 本对应抛的是运行期非类型异常，静态检查器却先报了类型错误：{detail}",
                pair.name
            ),
            Outcome::BrokenCompile(detail) => panic!(
                "[{}] 本对必须编得过，却报了 {detail}",
                pair.name
            ),
            Outcome::Returned(value) => panic!(
                "[{}] 本对应当抛非类型异常，注解版却返回了 {value}",
                pair.name
            ),
        },
    }
}

// --------------------------------------------------------------------------- #
// 语料 1：变量标注（模块级与函数局部）
// --------------------------------------------------------------------------- #

/// **`T-TS-1` ＋ `TS-47`**：模块级变量增删标注 ⇒ 行为不变（`int` 与 `str` 两种）。
#[test]
fn tts1_module_level_assignment_is_gradual() {
    check_pair(&Pair {
        name: "module-int",
        annotated: "x: int = 1\nx = 2\nresult = x\n",
        plain: "x = 1\nx = 2\nresult = x\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::SameValue,
    });
    check_pair(&Pair {
        name: "module-str",
        annotated: "s: str = \"a\"\ns = \"b\"\nresult = s\n",
        plain: "s = \"a\"\ns = \"b\"\nresult = s\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::SameValue,
    });
}

/// **`T-TS-1` ＋ `TS-47`**：模块级 `int` 标注拒绝 `str` 赋值 ⇒ 注解版报类型错误；
/// 去注解版照常跑完（`T-TS-3`）。
#[test]
fn tts1_module_level_type_mismatch_is_a_type_error_only_when_annotated() {
    check_pair(&Pair {
        name: "module-int-rejects-str",
        annotated: "x: int = 1\nx = \"s\"\nresult = x\n",
        plain: "x = 1\nx = \"s\"\nresult = x\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::TypeErrorRequired,
    });
}

/// **`T-TS-1` ＋ `TS-47`**：函数局部变量增删标注 ⇒ 行为不变；
/// 从**未标注函数**（平台类型，`TS-48`）拿到的 `str` 赋给 `int` 局部名 ⇒ 运行期类型错误。
#[test]
fn tts1_function_local_assignment_is_gradual() {
    check_pair(&Pair {
        name: "local-int",
        annotated: "def f():\n    y: int = 2\n    y = 3\n    return y\nresult = f()\n",
        plain: "def f():\n    y = 2\n    y = 3\n    return y\nresult = f()\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::SameValue,
    });
    check_pair(&Pair {
        name: "local-from-platform-type",
        annotated: "def h():\n    return \"s\"\ndef g(h):\n    y: int = 2\n    y = h()\n    return y\nresult = g(h)\n",
        plain: "def h():\n    return \"s\"\ndef g(h):\n    y = 2\n    y = h()\n    return y\nresult = g(h)\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::TypeErrorRequired,
    });
}

/// **`T-TS-2` 的旁证（不重复 `tests/boundary.rs`）**：未标注的局部名**不推断**（`TS-2`）——
/// 同一名字先 `int` 后 `str` 是**动态**的，不许被静态检查器判成类型错误。
#[test]
fn tts1_unannotated_names_stay_dynamic() {
    check_pair(&Pair {
        name: "dynamic-local",
        annotated: "def f(x: int) -> int:\n    t = x + 1\n    t = \"s\"\n    return x\nresult = f(1)\n",
        plain: "def f(x):\n    t = x + 1\n    t = \"s\"\n    return x\nresult = f(1)\n",
        tier: CheckTier::Deep,
        expectation: Expectation::SameValue,
    });
}

// --------------------------------------------------------------------------- #
// 语料 2：函数边界（入参／返回值）
// --------------------------------------------------------------------------- #

/// **`T-TS-1` ＋ `TS-10`**：函数**入参**边界——增删标注 ⇒ 行为不变（`int` 正常通过）。
#[test]
fn tts1_function_argument_boundary_is_gradual() {
    check_pair(&Pair {
        name: "argument-ok",
        annotated: "def f(x: int) -> int:\n    return x\nresult = f(41)\n",
        plain: "def f(x):\n    return x\nresult = f(41)\n",
        tier: CheckTier::Deep,
        expectation: Expectation::SameValue,
    });
}

/// **`T-TS-1` ＋ `TS-10`**：错误类型的实参 ⇒ 注解版（深层档位的入参检查）报类型错误；
/// 去注解版照常跑完（`T-TS-3`）。
#[test]
fn tts1_function_argument_mismatch_is_a_type_error_only_when_annotated() {
    check_pair(&Pair {
        name: "argument-rejects-str",
        annotated: "def f(x: int) -> int:\n    return 1\nresult = f(\"s\")\n",
        plain: "def f(x):\n    return 1\nresult = f(\"s\")\n",
        tier: CheckTier::Deep,
        expectation: Expectation::TypeErrorRequired,
    });
}

/// **`T-TS-1` ＋ `TS-10`**：错误类型的**返回值** ⇒ 注解版报类型错误；去注解版照常返回 `str`。
#[test]
fn tts1_function_return_mismatch_is_a_type_error_only_when_annotated() {
    check_pair(&Pair {
        name: "return-rejects-str",
        annotated: "def f() -> int:\n    return \"s\"\nresult = f()\n",
        plain: "def f():\n    return \"s\"\nresult = f()\n",
        tier: CheckTier::Deep,
        expectation: Expectation::TypeErrorRequired,
    });
}

/// **`T-TS-1` ＋ `TS-40`**：`bool ⊂ int` ⇒ `int` 标注**必须接受** `True`（增删标注行为一致；
/// 若被误拒，`SameValue` 会红 —— 这是"不许把 `bool` 当不相干类型"的护栏）。
#[test]
fn tts1_bool_is_an_int_at_the_boundary() {
    check_pair(&Pair {
        name: "bool-as-int",
        annotated: "def f(x: int) -> int:\n    return x\nresult = f(True)\n",
        plain: "def f(x):\n    return x\nresult = f(True)\n",
        tier: CheckTier::Deep,
        expectation: Expectation::SameValue,
    });
}

// --------------------------------------------------------------------------- #
// 语料 3：可空标注 `T?`（`TS-46`）
// --------------------------------------------------------------------------- #

/// **`T-TS-1` ＋ `TS-46`**：`str?` 接受 `None`（`T?` 的通过侧）⇒ 增删标注行为不变。
#[test]
fn tts1_nullable_annotation_accepts_none() {
    check_pair(&Pair {
        name: "nullable-none",
        annotated: "def greet(name: str?) -> str:\n    if name is None:\n        return \"anon\"\n    return \"hi \" + name\nresult = greet(None)\n",
        plain: "def greet(name):\n    if name is None:\n        return \"anon\"\n    return \"hi \" + name\nresult = greet(None)\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::TypeErrorAllowed,
    });
}

/// **`T-TS-1` ＋ `TS-46`**：`str?` 接受非 `None` 成员（按 `str` 检查）⇒ 增删标注行为不变。
#[test]
fn tts1_nullable_annotation_accepts_the_member_type() {
    check_pair(&Pair {
        name: "nullable-value",
        annotated: "def greet(name: str?) -> str:\n    if name is None:\n        return \"anon\"\n    return \"hi \" + name\nresult = greet(\"a\")\n",
        plain: "def greet(name):\n    if name is None:\n        return \"anon\"\n    return \"hi \" + name\nresult = greet(\"a\")\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::TypeErrorAllowed,
    });
}

/// **`T-TS-1` ＋ `TS-46`**：`int?` 不接受 `str`（`T?` 的拒绝侧）⇒ 注解版报类型错误；
/// 去注解版照常跑完。
#[test]
fn tts1_nullable_annotation_rejects_other_types() {
    check_pair(&Pair {
        name: "nullable-rejects-str",
        annotated: "x: int? = \"s\"\nresult = x\n",
        plain: "x = \"s\"\nresult = x\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::TypeErrorRequired,
    });
}

/// **`T-TS-1` ＋ `TS-50`**：`if x is None: return` 之后把 `x` 赋给 `int` 标注的局部名——
/// 收窄后合法；增删标注行为不变（静态检查器落地前，赋值检查在运行期兜住）。
#[test]
fn tts1_narrowing_after_none_guard_is_gradual() {
    check_pair(&Pair {
        name: "narrowing-after-none-guard",
        annotated: "def f(x: int?) -> int:\n    if x is None:\n        return 0\n    y: int = x\n    return y\nresult = f(None) + f(5)\n",
        plain: "def f(x):\n    if x is None:\n        return 0\n    y = x\n    return y\nresult = f(None) + f(5)\n",
        tier: CheckTier::Shallow,
        expectation: Expectation::TypeErrorAllowed,
    });
}

// --------------------------------------------------------------------------- #
// 两层检查的上层：能静态判定的不符必须在编译期报出（`TS-47` ③）
// --------------------------------------------------------------------------- #

/// **`T-TS-1`／`TS-47` ③**：**能静态判定**的赋值不符**必须**在编译期报出
/// （`CompileError::TypeAssignment`，`TS-36`）——这是 `T-TS-1` 允许注解版差异的**编译期入口**。
/// 这条同时钉住本文件的"编译期类型错"分类：它**真的被语料走到**，不是死分支。
#[test]
fn tts1_static_mismatch_is_reported_at_compile_time() {
    for source in [
        // 模块级：`int` 标注后赋 `str` 字面量（静态可判）
        "x: int = 1\nx = \"s\"\nresult = x\n",
        // 可空标注：`int?` 后赋 `str` 字面量（静态可判）
        "x: int? = \"s\"\nresult = x\n",
    ] {
        let error = compile(source, "<t>", Mode::Extension, CheckTier::Shallow, 0)
            .expect_err("TS-47 ③：静态可判定的赋值不符必须在编译期报出");
        assert!(
            matches!(error, CompileError::TypeAssignment(_)),
            "TS-36／TS-47：编译期类型错误必须是 `CompileError::TypeAssignment`，实际 {error:?}（源码 {source:?}）"
        );
    }
    // 对照：同形状的**正确**赋值编得过；去注解版也编得过（`T-TS-3`）。
    compile(
        "x: int = 1\nx = 2\nresult = x\n",
        "<t>",
        Mode::Extension,
        CheckTier::Shallow,
        0,
    )
    .expect("类型正确的标注必须编得过");
    compile(
        "x = 1\nx = \"s\"\nresult = x\n",
        "<t>",
        Mode::Extension,
        CheckTier::Shallow,
        0,
    )
    .expect("T-TS-3：去注解版必须编得过");
}

// --------------------------------------------------------------------------- #
// 语料 4：容器深标签（`TS-31` 的深层档位）
// --------------------------------------------------------------------------- #

/// **`T-TS-1` ＋ `TS-31`**：`list[int]` 深标签接受元素全为 `int` 的列表 ⇒ 行为不变。
#[test]
fn tts1_container_boundary_accepts_matching_elements() {
    check_pair(&Pair {
        name: "container-ok",
        annotated: "def first(xs: list[int]) -> int:\n    return xs[0]\nresult = first([7, 8])\n",
        plain: "def first(xs):\n    return xs[0]\nresult = first([7, 8])\n",
        tier: CheckTier::Deep,
        expectation: Expectation::TypeErrorAllowed,
    });
}

/// **`T-TS-1` ＋ `TS-31`**：`list[int]` 深标签拒掉含 `str` 元素的列表 ⇒ 注解版报类型错误；
/// 去注解版照常返回第一个元素。
#[test]
fn tts1_container_boundary_rejects_mismatched_elements() {
    check_pair(&Pair {
        name: "container-rejects-str-element",
        annotated: "def first(xs: list[int]) -> int:\n    return xs[0]\nresult = first([\"s\"])\n",
        plain: "def first(xs):\n    return xs[0]\nresult = first([\"s\"])\n",
        tier: CheckTier::Deep,
        expectation: Expectation::TypeErrorRequired,
    });
}

// --------------------------------------------------------------------------- #
// 语料 5：其它异常不变（`T-TS-1` 的"除类型错误外"）
// --------------------------------------------------------------------------- #

/// **`T-TS-1`**：两条都抛同一个非类型异常（`ZeroDivisionError`）⇒ 类名与消息必须一致。
#[test]
fn tts1_other_exceptions_are_unchanged() {
    check_pair(&Pair {
        name: "non-type-exception-parity",
        annotated:
            "def f(x: int) -> int:\n    return x\nresult = f(1)\nresult = result // 0\n",
        plain: "def f(x):\n    return x\nresult = f(1)\nresult = result // 0\n",
        tier: CheckTier::Deep,
        expectation: Expectation::SameNonTypeError,
    });
}

// --------------------------------------------------------------------------- #
// `T-TS-3`：未标注代码不产生类型错误
// --------------------------------------------------------------------------- #

/// **`T-TS-3`**：完全没有标注的程序照常动态执行（`TS-1`／`TS-2`）——同一名字随意换类型、
/// 运算符跨类型，两档位与纯 Python 模式都**不得**出现类型错误。
#[test]
fn tts3_unannotated_code_has_no_type_errors() {
    let source = "def mix(flag):\n    value = 1\n    if flag:\n        value = \"s\"\n    return value\n\na = mix(True) + \"!\"\nb = mix(False) + 1\nresult = [a, b]\n";
    for (mode, tier) in [
        (Mode::Extension, CheckTier::Shallow),
        (Mode::Extension, CheckTier::Deep),
        (Mode::PurePython, CheckTier::Shallow),
    ] {
        let vm = vm_with_builtins();
        match run(&vm, source, mode, tier) {
            Outcome::Returned(projection) => assert_eq!(
                projection, "['s!', 2]",
                "T-TS-3：未标注程序在 {mode:?}／{tier:?} 下照常执行"
            ),
            other => panic!(
                "T-TS-3：未标注代码不得产生类型错误（{mode:?}／{tier:?}），实际 {other:?}"
            ),
        }
    }
}

// --------------------------------------------------------------------------- #
// `T-TS-7`：标注保留可读；校验只在扩展模式发生
// --------------------------------------------------------------------------- #

/// **`T-TS-7`**：两种模式下**函数注解都保留且可读**（`TS-5` 的第一条）——本层真正能读的入口是
/// `f.__annotate__`／`f.__annotations__`（`tests/annotations.rs` 钉过执行器那一半）；
/// `__annotations__` 里的 `int` 是**同一个类型对象**（身份相等）。
///
/// 变量标注**不**进 `__annotations__`（本层如实登记的缺口，见文件头"已知覆盖边界"），
/// 所以这里只钉函数注解这条真通路。
#[test]
fn tts7_annotations_are_readable_in_both_modes() {
    let source = "def f(x: int) -> int:\n    return x\n";
    for mode in [Mode::PurePython, Mode::Extension] {
        let vm = vm_with_builtins();
        let unit = compile(source, "<t>", mode, CheckTier::Shallow, 0).unwrap_or_else(|error| {
            panic!("T-TS-7：{mode:?} 下带注解源码必须编得过，却报了 {error:?}")
        });
        let code = instantiate(&vm.instance, &unit);
        let namespace = vm.instance.new_dict();
        let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
        let frame = vm.instance.alloc(frame);
        pyawa_core::execute(&vm.instance, &frame)
            .unwrap_or_else(|error| panic!("T-TS-7：{mode:?} 下定义 f 失败：{error:?}"));
        let f = vm.instance.dict_get(namespace, "f").expect("命名空间里应当有 f");
        let int_object = vm
            .instance
            .type_value(vm.instance.type_named("int").expect("int 在内建表里"));
        let annotations = pyawa_core::executor::attribute::attribute_read(
            &vm.instance,
            f,
            "__annotations__",
        )
        .unwrap_or_else(|error| {
            panic!("T-TS-7：{mode:?} 下 `__annotations__` 必须可读：{error:?}")
        });
        assert_eq!(
            vm.instance.dict_get(annotations, "x"),
            Some(int_object),
            "T-TS-7：{mode:?} 下形参注解必须保留为 int"
        );
        assert_eq!(
            vm.instance.dict_get(annotations, "return"),
            Some(int_object),
            "T-TS-7：{mode:?} 下返回值注解必须保留为 int"
        );
        let annotate =
            pyawa_core::executor::attribute::attribute_read(&vm.instance, f, "__annotate__")
                .unwrap_or_else(|error| {
                    panic!("T-TS-7：{mode:?} 下 `__annotate__` 必须可读：{error:?}")
                });
        assert_ne!(
            annotate,
            vm.instance.singletons().none(),
            "T-TS-7：{mode:?} 下带注解的函数 `__annotate__` 必须非 None"
        );
    }
}

/// **`T-TS-7`**：同源码**纯 Python 模式不产生任何检查**（照常执行、返回动态结果），
/// **扩展模式才校验**（运行期 `TypeAssignmentError`，或静态检查器落地后的编译期类型错误）。
#[test]
fn tts7_checks_only_happen_in_extension_mode() {
    let source = "x: int = 1\nx = \"s\"\nresult = x\n";

    // `TS-5`：纯 Python 模式下同一源码合法且不产生任何检查 ⇒ 正常返回 `str`。
    let pure_vm = vm_with_builtins();
    match run(&pure_vm, source, Mode::PurePython, CheckTier::Shallow) {
        Outcome::Returned(projection) => assert_eq!(
            projection, "'s'",
            "TS-5：纯 Python 模式照常执行，标注是惰性的"
        ),
        other => panic!("T-TS-7：纯 Python 模式不得产生任何检查，实际 {other:?}"),
    }

    // 扩展模式：赋值检查生效（`TS-47`）。
    let extension_vm = vm_with_builtins();
    match run(&extension_vm, source, Mode::Extension, CheckTier::Shallow) {
        Outcome::TypeError { .. } | Outcome::TypeErrorCompile(_) => {}
        other => panic!("T-TS-7：校验只在扩展模式发生，扩展模式实际 {other:?}"),
    }
}

// --------------------------------------------------------------------------- #
// 反空转：类型正确的注解版语料必须真的跑起来
// --------------------------------------------------------------------------- #

/// **`T-TS-1` 的反空转护栏**：类型**明确正确**的注解版语料必须真的编译并跑出预期值——
/// 否则"注解版允许类型错误"那条例外会把"静态检查器把一切注解都判错"也放行。
///
/// 只放不可能有类型错误的语料（字面量与标注完全匹配、`bool ⊂ int`），不依赖收窄／静态推断。
#[test]
fn tts1_positive_annotated_programs_still_run() {
    for (source, tier, expected) in [
        ("x: int = 1\nx = 2\nresult = x\n", CheckTier::Shallow, "2"),
        ("s: str = \"a\"\nresult = s\n", CheckTier::Shallow, "'a'"),
        (
            "def f(x: int) -> int:\n    return x\nresult = f(41)\n",
            CheckTier::Deep,
            "41",
        ),
        (
            "def f(x: int) -> int:\n    return x\nresult = f(True)\n",
            CheckTier::Deep,
            "True",
        ),
        (
            "def f():\n    y: int = 2\n    return y\nresult = f()\n",
            CheckTier::Shallow,
            "2",
        ),
    ] {
        let vm = vm_with_builtins();
        match run(&vm, source, Mode::Extension, tier) {
            Outcome::Returned(projection) => assert_eq!(
                projection, expected,
                "T-TS-1：类型正确的注解版必须照常执行：{source:?}"
            ),
            other => panic!(
                "T-TS-1：类型正确的注解版不得被判类型错误或抛异常：{source:?} ⇒ {other:?}"
            ),
        }
    }
}
