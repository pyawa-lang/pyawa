//! **`pyawa check` 的 core 部分**（PLAN §10.1）：`check_source` 只编译不执行、**批量**类型诊断、
//! `.py` 只出语法类（`TS-26`／`MS-19`）、`.pyawa` 出语法＋类型。

use pyawa_core::compile::{check_source, Mode};

#[test]
fn pyawa_files_report_multiple_type_diagnostics_in_one_pass() {
    let source = "a: int = \"x\"\nb: str = 1\n";
    let diagnostics = check_source(source, "demo.pyawa", Mode::Extension);
    assert_eq!(
        diagnostics.len(),
        2,
        "批量诊断应一次报出 ≥2 条：{diagnostics:?}"
    );
    assert_eq!(diagnostics[0].code, "type");
    assert_eq!(diagnostics[1].code, "type");
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[1].line, 2);
    assert!(diagnostics[0].col >= 1, "列号应从 1 起：{diagnostics:?}");
    assert_eq!(diagnostics[0].severity, "error");
    assert_eq!(diagnostics[0].file, "demo.pyawa");
    for diagnostic in &diagnostics {
        assert!(
            diagnostic.message.contains("TS-47"),
            "消息应与执行路径同源（TS-47）：{}",
            diagnostic.message
        );
    }
}

#[test]
fn py_files_never_get_type_diagnostics() {
    // `MS-19`／`TS-26`：`.py` 的标注不产生任何检查、也没有类型诊断。
    let source = "a: int = \"x\"\nb: str = 1\n";
    let diagnostics = check_source(source, "demo.py", Mode::PurePython);
    assert!(
        diagnostics.is_empty(),
        "`.py` 不得出类型诊断（MS-19）：{diagnostics:?}"
    );
}

#[test]
fn py_syntax_errors_are_reported_with_a_location_when_available() {
    // 既有 parser 的**部分**错误消息带「（第 L 行，列 C-D）」标记 ⇒ 提取成结构位点；
    // 不带的（这里这类"形参表"错误）位置字段给 0（**如实记** ✗：统一 span 是 LSP 梯②的事）。
    let located = check_source("x = 1 + )\n", "broken.py", Mode::PurePython);
    assert_eq!(located.len(), 1, "语法错误只报一条：{located:?}");
    assert_eq!(located[0].code, "syntax");
    assert_eq!(located[0].file, "broken.py");
    assert_eq!(located[0].severity, "error");
    assert!(
        !located[0].message.is_empty(),
        "消息应与执行路径同源：{located:?}"
    );
    if located[0].line > 0 {
        assert!(located[0].col >= 1, "有行号就应有列号：{located:?}");
    }
    let no_location = check_source("def broken(:\n    pass\n", "broken2.py", Mode::PurePython);
    assert_eq!(no_location.len(), 1);
    assert_eq!(no_location[0].code, "syntax");
    assert!(!no_location[0].message.is_empty());
}

#[test]
fn valid_sources_have_no_diagnostics() {
    let python = "def add(a, b):\n    return a + b\n\nprint(add(1, 2))\n";
    assert!(check_source(python, "ok.py", Mode::PurePython).is_empty());
    let pyawa = "x: int = 1\ny: str = \"ok\"\n";
    assert!(check_source(pyawa, "ok.pyawa", Mode::Extension).is_empty());
    // 判不出的标注 ⇒ 按 `TS-48` 平台类型放行（不误报）✓。
    let dynamic = "u = \"x\"\na: int = u\n";
    assert!(check_source(dynamic, "dynamic.pyawa", Mode::Extension).is_empty());
}

#[test]
fn user_function_argument_and_return_types_are_checked() {
    // 用户报告的 bug：`f` 的签名没参与检查 ⇒ `a`／`b` 不报、`c` 报。
    let source = "def f(x: int) -> int:\n    return x\na: str = f(1)\nb: int = f(\"nope\")\nc: list[int] = \"x\"\n";
    let diagnostics = check_source(source, "user.pyawa", Mode::Extension);
    assert_eq!(
        diagnostics.len(),
        3,
        "应报 a/b/c 三条：{diagnostics:?}"
    );
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("expects str, got int"));
    assert_eq!(diagnostics[1].line, 4);
    assert!(
        diagnostics[1].message.contains("call type check failed")
            && diagnostics[1].message.contains("argument 0"),
        "{diagnostics:?}"
    );
    assert_eq!(diagnostics[2].line, 5);
    assert!(diagnostics[2].message.contains("expects list[int], got str"));
}

#[test]
fn valid_user_function_calls_pass() {
    let source = "def f(x: int) -> int:\n    return x\na: int = f(1)\nb: int = f(True)\n";
    assert!(
        check_source(source, "valid.pyawa", Mode::Extension).is_empty(),
        "合法调用不得误报（bool ⊂ int 相容）"
    );
}

#[test]
fn decorated_functions_are_not_statically_checked() {
    // 装饰器可能改签名 ⇒ 静态不猜（避免误报）。
    let source = "def deco(g):\n    return g\n@deco\ndef f(x: int) -> int:\n    return x\na: str = f(1)\n";
    assert!(
        check_source(source, "decorated.pyawa", Mode::Extension).is_empty(),
        "带装饰器的函数不做调用点检查"
    );
}

#[test]
fn calls_with_unpacking_skip_argument_checks() {
    // `*args`／关键字解包时静态个数不可判 ⇒ 参数检查跳过（不误报），返回类型仍可查。
    let source = "def f(x: int) -> int:\n    return x\na: int = f(*[1])\n";
    assert!(
        check_source(source, "unpack.pyawa", Mode::Extension).is_empty(),
        "解包调用的参数检查应跳过"
    );
}
