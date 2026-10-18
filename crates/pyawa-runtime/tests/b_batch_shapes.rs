//! **扩展特性 B 批的端到端抽例**（真实 CLI：`.pyawa` 扩展模式 vs `.py` 纯 Python 模式）。
//!
//! 与 `crates/pyawa-core/tests/b_batch.rs`（编译期／进程内）配对：这一条走**真实 CLI**，
//! 把 `BC-62`／`BC-63` 的**可观察语义**（stdout 逐字节）钉住 ——
//!
//! - `BC-62`：同一 body 的扩展模式 `lambda` 与 `.py` 下的 `def` 版**逐行对拍**；单行表达式形态
//!   两模式产物行为一致；
//! - `BC-63`：四种操作符的输出与规格里"等价纯 Python 写法"的输出**逐行对拍**；
//! - 纯 Python 模式（`.py`）下四种写法**必须**在 CLI 上失败（`BC-15`：`SyntaxError`）。
//!
//! 期望输出由本机 `python3`（3.14.4）跑**同段纯 Python 等价脚本**取得（不是手写值）。

/// 跑一个脚本文件；返回（是否成功，stdout，stderr）。临时目录按测试名隔离。
fn run(name: &str, file_name: &str, script: &str) -> (bool, String, String) {
    let root = std::env::temp_dir().join(format!("pyawa-bbatch-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join(file_name);
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// 期望输出（本机 `python3` 3.14.4 跑纯 Python 等价脚本实测，逐行）。
const BC62_EXPECTED: &str = "['Alice ', '  Bob', 'bob']\n21\n11\nTrue\nHI\n";

/// 扩展模式（`.pyawa`）：规格示例里的多语句 `lambda` body 形态。
const BC62_EXTENSION: &str = r#"names = ["  Bob", "Alice ", "bob"]
print(sorted(names, key=lambda s: (
    t = s.strip()
    t.lower()
)))
print((lambda x: (y = x * 2; y + 1))(10))
handlers = {"inc": lambda v: (
    n = v + 1
    n
)}
print(handlers["inc"](10))
print((lambda x: (y = x + 1))(1) is None)
print((lambda s: (
    t = s.strip()
    t.upper()
))("  hi "))
"#;

/// 同 body 的 `def` 版（`.py` 下合法；`BC-62` 的对拍物）。
const BC62_DEF: &str = r#"names = ["  Bob", "Alice ", "bob"]
def key(s):
    t = s.strip()
    return t.lower()
print(sorted(names, key=key))
def inline(x):
    y = x * 2
    return y + 1
print(inline(10))
def inc(v):
    n = v + 1
    return n
print({"inc": inc}["inc"](10))
def no_tail(x):
    y = x + 1
print(no_tail(1) is None)
def strip_upper(s):
    t = s.strip()
    return t.upper()
print(strip_upper("  hi "))
"#;

/// **`BC-62`**：扩展模式 lambda 与 `def` 版**逐行对拍**（两边都等于参照输出）。
#[test]
fn bc62_multi_statement_lambda_agrees_with_the_def_version() {
    let (ok, stdout, stderr) = run("bc62-ext", "probe.pyawa", BC62_EXTENSION);
    assert!(ok, "扩展模式脚本应当跑成功\n--- stderr ---\n{stderr}");
    assert_eq!(stdout, BC62_EXPECTED, "扩展模式输出：" );
    let (ok, def_stdout, stderr) = run("bc62-def", "probe.py", BC62_DEF);
    assert!(ok, "def 版脚本应当跑成功\n--- stderr ---\n{stderr}");
    assert_eq!(def_stdout, stdout, "`BC-62`：同一 body 的 lambda 与 def 版必须逐行一致");
}

/// **`BC-62`**：单行表达式形态两模式行为一致（`BC-62` 的"增量仅为 body 扩充"）。
#[test]
fn bc62_single_line_expression_form_is_unchanged() {
    let source = "print((lambda x: x + 1)(41))\nprint((lambda s: s.strip().upper())(\"  ok \"))\n";
    let (ok, stdout, stderr) = run("bc62-single-ext", "probe.pyawa", source);
    assert!(ok, "扩展模式单行 lambda 应当跑成功\n{stderr}");
    let (ok, pure_stdout, stderr) = run("bc62-single-pure", "probe.py", source);
    assert!(ok, "纯 Python 模式单行 lambda 应当跑成功\n{stderr}");
    assert_eq!(stdout, pure_stdout);
}

/// 期望输出（本机 `python3` 3.14.4 跑等价纯 Python 脚本实测，逐行）。
const BC63_EXPECTED: &str = "3\n缺省\nNone\n20\n-1\n5\n5\n0\n\n[]\nTrue\n[]\nTrue\n";

/// 扩展模式（`.pyawa`）：`BC-63` 四种操作符的示例。
const BC63_EXTENSION: &str = r#"def coalesce(value, default):
    return value if value is not None else default
class Box:
    def __init__(self, value):
        self.value = value
table = Box(Box(3))
print(table?.value?.value ?? "缺省")
none = None
print(none?.value ?? "缺省")
print(none?.value.value)
xs = [10, 20]
print(xs?[1] ?? -1)
print(none?[0] ?? -1)
acc = None
acc ??= 5
print(acc)
acc ??= 9
print(acc)
print(0 ?? 42)
print("" ?? "x")
print([] ?? [1])
log = []
def side(v):
    log.append(v)
    return v
call = none?.value(side(1))
print(call is None)
print(log)
print(table?.value is not None)
"#;

/// 规格"等价纯 Python 写法"表组合出来的同语义脚本（`BC-63` 的对拍物）。
const BC63_EQUIVALENT_PURE: &str = r#"def coalesce(value, default):
    return value if value is not None else default
class Box:
    def __init__(self, value):
        self.value = value
table = Box(Box(3))
t = (lambda _t: _t.value if _t is not None else None)(table)
r = (lambda _u: _u.value if _u is not None else None)(t)
print(coalesce(r, "缺省"))
none = None
n1 = (lambda _t: _t.value if _t is not None else None)(none)
print(coalesce(n1, "缺省"))
print((lambda _t: _t.value.value if _t is not None else None)(none))
xs = [10, 20]
u = (lambda _t: _t[1] if _t is not None else None)(xs)
print(coalesce(u, -1))
v = (lambda _t: _t[0] if _t is not None else None)(none)
print(coalesce(v, -1))
acc = None
if acc is None:
    acc = 5
print(acc)
if acc is None:
    acc = 9
print(acc)
print(coalesce(0, 42))
print(coalesce("", "x"))
print(coalesce([], [1]))
log = []
def side(v):
    log.append(v)
    return v
call = (lambda _t: _t.value(side(1)) if _t is not None else None)(none)
print(call is None)
print(log)
print(t is not None)
"#;

/// **`BC-63`**：空安全操作符的输出与"等价纯 Python 写法"**逐行对拍**（含只求值一次／惰性）。
#[test]
fn bc63_null_safe_agrees_with_the_equivalent_pure_python() {
    let (ok, stdout, stderr) = run("bc63-ext", "probe.pyawa", BC63_EXTENSION);
    assert!(ok, "扩展模式脚本应当跑成功\n--- stderr ---\n{stderr}");
    assert_eq!(stdout, BC63_EXPECTED, "扩展模式输出不符");
    let (ok, pure_stdout, stderr) = run("bc63-pure", "probe.py", BC63_EQUIVALENT_PURE);
    assert!(ok, "等价纯 Python 脚本应当跑成功\n--- stderr ---\n{stderr}");
    assert_eq!(
        pure_stdout, stdout,
        "`BC-63`：扩展模式与该表的等价纯 Python 写法必须逐行一致"
    );
}

/// **`BC-15`**：纯 Python 模式（`.py`）下 `BC-63` 的四种写法都必须**语法失败**（CLI 退出码 1）。
#[test]
fn bc63_forms_are_syntax_error_in_pure_python() {
    for source in [
        "a = obj?.b\n",
        "a = obj?.b(1)\n",
        "a = xs?[0]\n",
        "a = x ?? y\n",
        "x ??= 1\n",
    ] {
        let (ok, _stdout, stderr) = run("bc63-syntax", "probe.py", source);
        assert!(!ok, "纯 Python 模式下这段必须失败：{source:?}");
        assert!(
            stderr.contains("未捕获"),
            "CLI 应报未捕获的语法错：{stderr}"
        );
    }
}

/// **`BC-62`／`BC-15`**：纯 Python 模式（`.py`）下多语句 lambda 新形态必须语法失败。
#[test]
fn bc62_new_form_is_syntax_error_in_pure_python() {
    for source in [
        "f = lambda x: (y = x; y + 1)\n",
        "f = lambda x: (y = x)\n",
        "f = lambda x: (\n    y = x\n    y\n)\n",
    ] {
        let (ok, _stdout, stderr) = run("bc62-syntax", "probe.py", source);
        assert!(!ok, "纯 Python 模式下这段必须失败：{source:?}");
        assert!(
            stderr.contains("未捕获"),
            "CLI 应报未捕获的语法错：{stderr}"
        );
    }
}

/// **`BC-63`**：`?.` 只吞 `None` —— 属性不存在仍是 `AttributeError`（端到端）。
#[test]
fn bc63_missing_attribute_still_raises_attribute_error() {
    let (ok, _stdout, stderr) = run(
        "bc63-attrerror",
        "probe.pyawa",
        "class C:\n    pass\nc = C()\nr = c?.missing\n",
    );
    assert!(!ok, "属性不存在必须抛 AttributeError");
    assert!(
        stderr.contains("AttributeError"),
        "未捕获异常应是 AttributeError：{stderr}"
    );
}
