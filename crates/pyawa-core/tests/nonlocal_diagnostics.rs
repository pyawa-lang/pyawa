//! **`nonlocal` 无外层绑定 ⇒ 编译期 `SyntaxError`**（task-12，缺口③a）。
//!
//! 参照口径（本机 `python3` 3.14.4 实测，逐例对拍；探针脚本：
//! `target/recon/nonlocal/probe_both.py`）：
//!
//! - 函数里 `nonlocal x`、而**任何外层函数作用域都没有 `x` 的 local 绑定** ⇒
//!   `SyntaxError: no binding for nonlocal 'x' found`；
//! - **模块层**的 `nonlocal` ⇒ `SyntaxError: nonlocal declaration not allowed at module level`
//!   （类体不是"模块层"：模块层的类体里报的是"找不到绑定"）；
//! - `nonlocal` 与同作用域 `global` 同名 ⇒ `SyntaxError: name 'x' is nonlocal and global`；
//! - 形参被声明 `global`／`nonlocal` ⇒ `SyntaxError: name 'x' is parameter and global`／`... and nonlocal`；
//! - PEP 695 类型参数不能当 `nonlocal` 目标 ⇒
//!   `SyntaxError: nonlocal binding not allowed for type parameter 'T'`；
//! - **类作用域不是绑定来源**（`class C: x = 1` 里的 `x` 不算外层绑定）；**推导式的迭代目标**
//!   也不算（它是推导式自己的作用域）；注解声明（`x: int`）算 local、`del x` 也算、`import … as x`
//!   也算（与参照 `symtable` 的 `DEF_BOUND` 同口径）；
//! - 报错里的名字是**私有名改写后**的名字（`task-5` 的改写先执行）⇒ 类里的 `nonlocal __n`
//!   报 `_H__n`（实测参照：`class H: def bump(self): nonlocal __n` ⇒
//!   `no binding for nonlocal '_H__n' found`）；
//! - 时机：**编译期**（`compile()`／`exec()`／导入都走同一入口；`exec` 的异常文本格式另见报告）。
//!
//! **如实范围外**（`symtable` pass-1 的"先用后声明"一族；本层尚未实现，见报告）：
//! `is used prior to nonlocal declaration`、`is assigned to before nonlocal declaration`、
//! `annotated name 'x' can't be nonlocal`。
//!
//! 本文件断言的是 [`CompileError::Syntax`] 的**文案**（与参照 `SyntaxError.msg` 逐字一致）。

use pyawa_core::compile::{check_source, compile, CheckTier, CompileError, Mode};

/// 编译一段模块源码（纯 `.py` 模式；与执行路径同一入口）。
fn compile_module(source: &str) -> Result<(), CompileError> {
    compile(source, "<probe>", Mode::PurePython, CheckTier::Shallow, 0).map(|_| ())
}

/// `(源码, 参照的 SyntaxError.msg)`。
const SYNTAX_ERRORS: &[(&str, &str)] = &[
    // 函数里没有外层绑定。
    ("def f():\n    nonlocal x\n", "no binding for nonlocal 'x' found"),
    (
        "def outer():\n    def inner():\n        nonlocal x\n",
        "no binding for nonlocal 'x' found",
    ),
    // 外层只有 global 声明（global 不是绑定来源）。
    (
        "def outer():\n    global x\n    x = 1\n    def inner():\n        nonlocal x\n",
        "no binding for nonlocal 'x' found",
    ),
    // 先声明后赋值也一样（赋值不会创建外层绑定）。
    (
        "def f():\n    nonlocal x\n    x = 1\n",
        "no binding for nonlocal 'x' found",
    ),
    // 绑定只在**类作用域**里 ⇒ 不算。
    (
        "def f():\n    class C:\n        x = 1\n        def m():\n            nonlocal x\n",
        "no binding for nonlocal 'x' found",
    ),
    // 推导式的迭代目标不算外层 local（参照 symtable 口径）。
    (
        "def outer():\n    [x for x in range(2)]\n    def inner():\n        nonlocal x\n",
        "no binding for nonlocal 'x' found",
    ),
    // lambda 的形参不算外层 local。
    (
        "def outer():\n    g = lambda x: x\n    def inner():\n        nonlocal x\n",
        "no binding for nonlocal 'x' found",
    ),
    // 模块层的 nonlocal（含 if 体内）。
    ("nonlocal x\n", "nonlocal declaration not allowed at module level"),
    (
        "if True:\n    nonlocal x\n",
        "nonlocal declaration not allowed at module level",
    ),
    // 模块层的类体：不是"模块层"报错，而是"找不到绑定"。
    (
        "class C:\n    nonlocal x\n",
        "no binding for nonlocal 'x' found",
    ),
    // 类体自己不是外层函数，也不是绑定来源。
    (
        "class C:\n    def m(self):\n        nonlocal x\n",
        "no binding for nonlocal 'x' found",
    ),
    // `nonlocal` 与 `global` 同名（两种声明顺序）。
    (
        "def f():\n    global x\n    nonlocal x\n",
        "name 'x' is nonlocal and global",
    ),
    (
        "def f():\n    nonlocal x\n    global x\n",
        "name 'x' is nonlocal and global",
    ),
    (
        "class C:\n    global x\n    nonlocal x\n",
        "name 'x' is nonlocal and global",
    ),
    // 形参冲突（pass-1 在声明处当场报；与声明顺序有关，与参照同）。
    (
        "def f(x):\n    nonlocal x\n",
        "name 'x' is parameter and nonlocal",
    ),
    (
        "def f(x):\n    global x\n    nonlocal x\n",
        "name 'x' is parameter and global",
    ),
    (
        "def f(x):\n    nonlocal x\n    global x\n",
        "name 'x' is parameter and nonlocal",
    ),
    // 类型参数不能当 nonlocal 目标。
    (
        "def f[T]():\n    nonlocal T\n",
        "nonlocal binding not allowed for type parameter 'T'",
    ),
    (
        "class C[T]:\n    def m(self):\n        nonlocal T\n",
        "nonlocal binding not allowed for type parameter 'T'",
    ),
    // 多个缺失名：按声明（符号插入）序报第一个。
    ("def f():\n    nonlocal a, b\n", "no binding for nonlocal 'a' found"),
    ("def f():\n    nonlocal b, a\n", "no binding for nonlocal 'b' found"),
    // 父作用域的错误先于子作用域（参照 pass-2 自上而下）。
    (
        "def f():\n    nonlocal a\n    def g():\n        nonlocal b\n",
        "no binding for nonlocal 'a' found",
    ),
];

#[test]
fn nonlocal_without_binding_is_a_compile_time_syntax_error() {
    for (source, expected) in SYNTAX_ERRORS {
        match compile_module(source) {
            Err(CompileError::Syntax(message)) => assert_eq!(
                message, *expected,
                "文案要与参照逐字一致\n源码：{source:?}"
            ),
            other => panic!(
                "应在**编译期**报 CompileError::Syntax({expected:?})，实际 {other:?}\n源码：{source:?}"
            ),
        }
    }
}

/// **私有名改写后**的报错名（`task-5` 的改写先于本校验）。
#[test]
fn nonlocal_error_uses_the_mangled_private_name() {
    let source = "class H:\n    def bump(self):\n        nonlocal __n\n";
    match compile_module(source) {
        Err(CompileError::Syntax(message)) => {
            assert_eq!(message, "no binding for nonlocal '_H__n' found")
        }
        other => panic!("应报改写后的名字 `_H__n`，实际 {other:?}"),
    }
}

/// **合法形态不许误报**（外层绑定来自：形参／赋值／`del`／`import`／注解声明／类体通道；
/// 类体与方法的特殊名字；类型参数作用域本身）。
const VALID_SOURCES: &[(&str, &str)] = &[
    (
        "fn-param",
        "def outer(x):\n    def inner():\n        nonlocal x\n",
    ),
    (
        "fn-assign",
        "def outer():\n    x = 1\n    def inner():\n        nonlocal x\n        x += 1\n",
    ),
    (
        "fn-del",
        "def outer():\n    del x\n    def inner():\n        nonlocal x\n",
    ),
    (
        "fn-import",
        "def outer():\n    import os as x\n    def inner():\n        nonlocal x\n",
    ),
    (
        "fn-annotation-only",
        "def outer():\n    x: int\n    def inner():\n        nonlocal x\n",
    ),
    (
        "class-channel",
        "def make():\n    count = 0\n    class C:\n        def m(self):\n            nonlocal count\n            count += 1\n    return C\n",
    ),
    (
        "class-body-nonlocal",
        "def f():\n    x = 1\n    class C:\n        nonlocal x\n        y = x\n",
    ),
    (
        "class-method-special-names",
        "class C:\n    def m(self):\n        nonlocal __class__\n",
    ),
    (
        "nested-class-inherits-specials",
        "class D:\n    class C:\n        nonlocal __class__\n",
    ),
    (
        "dup-nonlocal",
        "def outer():\n    x = 1\n    def inner():\n        nonlocal x\n        nonlocal x\n",
    ),
    (
        "two-level-chain",
        "def a():\n    x = 1\n    def b():\n        nonlocal x\n        def c():\n            nonlocal x\n",
    ),
    (
        "generic-class-method",
        "class C[T]:\n    def m(self):\n        return T\n",
    ),
    (
        "comprehension-walrus-binds",
        "def outer():\n    [y := i for i in range(2)]\n    def inner():\n        nonlocal y\n",
    ),
    (
        "no-nonlocal-at-all",
        "def f(x):\n    return [x for x in range(3)]\n",
    ),
];

#[test]
fn valid_nonlocal_forms_still_compile() {
    for (name, source) in VALID_SOURCES {
        if let Err(error) = compile_module(source) {
            panic!("{name}：合法形态被误报 {error:?}\n源码：{source:?}");
        }
    }
}

/// `check`（诊断路径）也报**同一条**语法诊断（`code == "syntax"`、文案逐字一致）。
#[test]
fn check_source_reports_the_same_nonlocal_syntax_diagnostic() {
    let diagnostics = check_source(
        "def f():\n    nonlocal x\n",
        "demo.py",
        Mode::PurePython,
    );
    assert_eq!(diagnostics.len(), 1, "诊断一条：{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "syntax");
    assert_eq!(diagnostics[0].file, "demo.py");
    assert_eq!(
        diagnostics[0].message,
        "no binding for nonlocal 'x' found",
        "诊断文案与执行路径同源"
    );
    // 合法源码给 0 条。
    assert!(
        check_source("def outer():\n    x = 1\n    def inner():\n        nonlocal x\n", "ok.py", Mode::PurePython)
            .is_empty()
    );
}
