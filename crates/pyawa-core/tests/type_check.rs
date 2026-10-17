//! **`TS-47` 编译期静态赋值检查** ＋ `TS-48`（平台类型）＋ `TS-50`（类型收束）。
//!
//! 口径的出处是 `docs/SPEC-type-system.md`：能**静态判定**的赋值不符在**编译期**报
//! [`CompileError::TypeAssignment`]；判不出的留给运行期 `CHECK_ASSIGNMENT`（`BC-65`）。
//! 纯 Python 模式下**一律不检查**（`TS-5`：`.py` 的标注不产生任何检查、产物逐字节不变）。
//!
//! 本文件的语料全部是**扩展模式**（`.pyawa`）；每组"应当报错"的用例都配一条"同形状但正确"的
//! 对照，避免把"检查器把一切都拒了"误判成通过。

use pyawa_core::compile::{compile, CheckTier, CompileError, Mode};

/// 扩展模式编译（浅层档位、优化级 0：两者不改变本轮检查）；文件名固定以便断言位置。
fn extension(source: &str) -> Result<(), CompileError> {
    compile(source, "probe.pyawa", Mode::Extension, CheckTier::Shallow, 0).map(|_| ())
}

/// 纯 Python 模式编译（同样固定档位）。
fn pure_python(source: &str) -> Result<(), CompileError> {
    compile(source, "probe.py", Mode::PurePython, CheckTier::Shallow, 0).map(|_| ())
}

/// 断言扩展模式编得过（`TS-48`／`TS-40`／`TS-2` 的"放行"面）。
fn expect_compiles(source: &str) {
    if let Err(error) = extension(source) {
        panic!("扩展模式应当编得过 {source:?}，却报了 {error:?}");
    }
}

/// 断言扩展模式报**编译期类型错**，返回消息。
fn expect_type_assignment(source: &str) -> String {
    match extension(source) {
        Err(CompileError::TypeAssignment(message)) => message,
        other => panic!(
            "`TS-47`：能静态判定的不符必须在编译期报 TypeAssignment，实际 {other:?}\n源码：{source:?}"
        ),
    }
}

/// ① **能静态判定的不符**在编译期报出（字面量／已标注名字两条推断路）＋ 消息四要素。
#[test]
fn statically_decidable_mismatches_are_reported_at_compile_time() {
    // 后续赋值：已经确知 `x: int`，字面量 `"s"` 一眼可判。
    let message = expect_type_assignment("x: int = 1\nx = \"s\"\n");
    assert!(
        message.contains("`x`"),
        "消息要带变量名（`TS-11` 的元素对静态报错同样可读）：{message}"
    );
    assert!(message.contains("expects int"), "消息要带期望标签：{message}");
    assert!(message.contains("got str"), "消息要带实际标签：{message}");
    assert!(message.contains("probe.pyawa: line 2"), "消息要带位置：{message}");
    assert!(message.contains("TS-47"), "消息要带规格引用：{message}");

    // 初始标注赋值同样是"赋给已标注变量"（`TS-47`：此后赋给它的值必须相容）。
    let message = expect_type_assignment("x: int = \"s\"\n");
    assert!(message.contains("probe.pyawa: line 1"), "位置指向赋值那一行：{message}");

    // `T?` ⇒ 期望标签渲染成 union（`TS-46` 的等价写法）。
    let message = expect_type_assignment("x: int? = \"s\"\n");
    assert!(
        message.contains("expects int | None"),
        "可空标注要渲染成 union：{message}"
    );

    // 复合标签（`list[int?]`）：期望渲染外层＋内层，实际 `None` ⇒ `NoneType`。
    let message = expect_type_assignment("x: list[int?] = None\n");
    assert!(
        message.contains("expects list[int | None]"),
        "复合标签要渲染内层：{message}"
    );
    assert!(message.contains("got None"), "实际标签：{message}");

    // RHS 是**已标注名字**时也按声明类型判：`src: str` 赋给 `dst: int`。
    let message = expect_type_assignment("src: str = \"s\"\ndst: int = src\n");
    assert!(
        message.contains("`dst`") && message.contains("got str"),
        "已标注名字的读取要按声明类型判：{message}"
    );
}

/// ②③ **平台类型（`TS-48`）与合法赋值放行**：判不出的 RHS 一律不误报（`TS-47` ②）。
#[test]
fn platform_types_and_compatible_assignments_pass() {
    // 调用／运算／容器字面量 ⇒ 静态判不出 ⇒ 平台类型，放行（运行期 `CHECK_ASSIGNMENT` 兜住）。
    for source in [
        "x: int = later_unknown()\n",
        "def f(h):\n    y: int = h()\n    return y\n",
        "x: int = 1 + 2\n",
        "x: int = len([])\n",
        "x: int = []\n",
    ] {
        expect_compiles(source);
    }
    // ③ 同类型重新赋值。
    expect_compiles("x: int = 1\nx = 2\n");
    // ④ `TS-40`：`bool ⊂ int` ⇒ `bool` 字面量可以赋给 `int` 标注（反过来不行，见下）。
    expect_compiles("x: int = True\nx = False\n");
    // `T?` 接受 `None` 与 `T`（`TS-46`）。
    expect_compiles("x: int? = None\nx = 1\nx = None\n");
    // 显式 `Any` 与一切相容（`TS-28`／`TS-48`）。
    expect_compiles("from_平台: Any = 1\ny: int = from_平台\n");
    // 反向：`int` 字面量不能赋给 `bool` 标注（内建层次里 `bool` 在 `int` 之上）。
    let message = expect_type_assignment("x: bool = 1\n");
    assert!(
        message.contains("expects bool") && message.contains("got int"),
        "`int` 不是 `bool` 的子类型：{message}"
    );
    // 反向：`int` 也不能赋给 `float` 标注（参照的 `isinstance(1, float)` 为假 ⇒ 与运行期检查同口径）。
    let message = expect_type_assignment("x: float = 1\n");
    assert!(message.contains("expects float") && message.contains("got int"), "{message}");
}

/// ⑤ **纯 Python 模式一律不检查**（`TS-5`）：同一段源码在 `.py` 下编得过。
#[test]
fn pure_python_mode_never_checks() {
    for source in [
        "x: int = 1\nx = \"s\"\n",
        "x: int = \"s\"\n",
        "def g(h):\n    y: int = 2\n    y = h()\n    return y\n",
    ] {
        if let Err(error) = pure_python(source) {
            panic!("TS-5：纯 Python 模式同源码必须合法，实际 {error:?}\n源码：{source:?}");
        }
    }
    // 对照：同一段源码在扩展模式确实会报（证明上面的"通过"不是检查器没跑）。
    assert!(matches!(
        extension("x: int = \"s\"\n"),
        Err(CompileError::TypeAssignment(_))
    ));
}

/// ⑦ **未标注的名字／形参保持动态**（`TS-1`／`TS-2`）：赋任意值都不报。
#[test]
fn unannotated_names_are_never_checked() {
    for source in [
        "x = 1\nx = \"s\"\n",
        "def f(a):\n    a = \"s\"\n    return a\n",
        "def f(a):\n    b = a\n    b = 1\n    return b\n",
    ] {
        expect_compiles(source);
    }
}

/// ⑥ **`TS-50` 收窄**：守卫过的可空名字按非 `None` 判；没守卫就按 union 报。
#[test]
fn ts50_narrowing_after_a_terminating_none_guard() {
    // `if x is None: return` ⇒ 其后 `x` 收窄为 `int`：`y: int = x` 合法。
    expect_compiles(
        "def f(x: int?) -> int:\n    if x is None:\n        return 0\n    y: int = x\n    return y\n",
    );
    // 没收窄 ⇒ union 里有 `NoneType` ⇒ 静态可判定不符 ⇒ 报错，消息的实际标签是 union。
    let message = expect_type_assignment(
        "def g(x: int?) -> int:\n    y: int = x\n    return y\n",
    );
    assert!(
        message.contains("expects int") && message.contains("got int | None"),
        "没守卫时必须按 union 判：{message}"
    );
    // `raise`／`continue`／`break` 也是"该支必然走掉"（`TS-50` 明列）。
    expect_compiles(
        "def f(x: int?) -> int:\n    if x is None:\n        raise ValueError\n    y: int = x\n    return y\n",
    );
    expect_compiles(
        "def f(x: int?, items):\n    for item in items:\n        if x is None:\n            continue\n        y: int = x\n    return 0\n",
    );
    // `while True` 落不下去的块同样算终止（`block_terminates` 的既有口径）。
    expect_compiles(
        "def f(x: int?) -> int:\n    if x is None:\n        while True:\n            pass\n    y: int = x\n    return y\n",
    );
}

/// ⑥ **`TS-50` 收窄**：`if x is not None:` 的**分支内**与 `else` 分支内都收窄。
#[test]
fn ts50_narrowing_inside_the_not_none_branch() {
    // 分支内 `x` 非 `None` ⇒ `y: int = x` 合法。
    expect_compiles(
        "def f(x: int?) -> int:\n    if x is not None:\n        y: int = x\n        return y\n    return 0\n",
    );
    // `else` 分支 = `x` 为 `None` 的那一支；反向守卫的 `else` 里 `x` 非 `None` ⇒ 合法。
    expect_compiles(
        "def f(x: int?) -> int:\n    if x is None:\n        pass\n    else:\n        y: int = x\n    return 0\n",
    );
    // 收窄**不外泄**：出 `if` 之后回到 `T?`；`then` 支 `return` 走掉 ⇒ 其后只剩
    // `x is None` 的那一支 ⇒ 按 `None` 收窄（静态可判定 `y: int = x` 不可能）。
    let message = expect_type_assignment(
        "def f(x: int?) -> int:\n    if x is not None:\n        return 1\n    y: int = x\n    return y\n",
    );
    assert!(
        message.contains("`y`") && message.contains("got None"),
        "`then` 支终止 ⇒ 其后按 else 的 NoneType 收窄：{message}"
    );
    // 没有守卫时的"回到 `T?`"：
    let message = expect_type_assignment(
        "def f(x: int?) -> int:\n    if x is not None:\n        pass\n    y: int = x\n    return y\n",
    );
    assert!(
        message.contains("got int | None"),
        "两支都落下来时回到 `T?`：{message}"
    );
    // 守卫之后再赋 `None` ⇒ 收窄失效（`TS-50`：出了控制流位置即回到原标签）。
    let message = expect_type_assignment(
        "def f(x: int?) -> int:\n    if x is None:\n        return 0\n    x = None\n    y: int = x\n    return y\n",
    );
    assert!(
        message.contains("`y`") && message.contains("got int | None"),
        "重新赋值 `None` 之后收窄必须失效：{message}"
    );
    // **未标注形参不窄化**（`TS-2`：不为平台类型造类型）⇒ 永远放行。
    expect_compiles(
        "def f(x):\n    if x is None:\n        return 0\n    y: int = x\n    return y\n",
    );
}

/// ⑥ **`assert` 不作收窄依据**（`TS-50`：优化下会被剥掉，据它收窄是空头承诺）。
#[test]
fn assert_does_not_narrow() {
    // `assert x is not None` 之后仍按 `int?` 判 ⇒ 静态可判定的不符要报。
    let message = expect_type_assignment(
        "def f(x: int?) -> int:\n    assert x is not None\n    y: int = x\n    return y\n",
    );
    assert!(
        message.contains("got int | None"),
        "assert 不能当收窄依据：{message}"
    );
}
