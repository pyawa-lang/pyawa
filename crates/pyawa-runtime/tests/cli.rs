//! `pyawa` CLI 的端到端检查（`PLAN-milestones.md` 第 4 条第 ① 项的最小面）。
//!
//! 覆盖三件：跑得成（退出码 `0`）、脚本异常报 `1`、宿主侧错误报 `2`。
//! **不覆盖 `print`** —— 本版如实不打印（`CM-26`：`print` 须走 `sys.stdout → _io → fs`）。

use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_pyawa")
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pyawa-cli-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

#[test]
fn runs_a_script_through_the_fs_domain() {
    let dir = scratch("ok");
    let path = dir.join("ok.py");
    std::fs::write(&path, "x = 1\ny = x + 2\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reports_a_syntax_error_with_exit_one() {
    let dir = scratch("syntax");
    let path = dir.join("bad.py");
    std::fs::write(&path, "x = = 1\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("未捕获"), "stderr={stderr}");
}

#[test]
fn refuses_a_missing_file_with_exit_two() {
    let dir = scratch("missing");
    let output = Command::new(binary())
        .arg(dir.join("nope.py"))
        .output()
        .expect("跑 CLI");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn prints_through_the_fs_domain() {
    let dir = scratch("print");
    let path = dir.join("print.py");
    std::fs::write(&path, "print(\"hi\")\nprint(\"a\", \"b\")\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(output.status.code(), Some(0), "print 应正常退出");
    // `print` ⇒ `sys.stdout` ⇒ `_io` 文本层 ⇒ `fs` 域的 `write` ⇒ 标准流（`CM-26`：**不设临时 sink** ✓）
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hi\na b\n",
        "stdout 应是 `fs` 域写出去的字节"
    );
}

#[test]
fn exposes_the_script_path_as_argv_zero() {
    let dir = scratch("argv");
    let path = dir.join("argv.py");
    std::fs::write(&path, "from sys import argv\nprint(argv[0])\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(output.status.code(), Some(0), "stderr={}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        path.to_string_lossy(),
        "`sys.argv[0]` 应是脚本路径（与参照实现的 `argv[0]` 同义 ✓）"
    );
}

#[test]
fn imports_a_module_through_the_fs_domain() {
    let dir = scratch("import");
    std::fs::write(dir.join("helper.py"), "value = \"loaded\"\n").expect("写被导入模块");
    let path = dir.join("main.py");
    std::fs::write(&path, "import helper\nprint(helper.value)\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(output.status.code(), Some(0), "stderr={}", String::from_utf8_lossy(&output.stderr));
    // **I/O 走 `fs` 域**（`IM-15` ✓）：加载器按 `sys.path`（脚本所在目录 ✓）读 `helper.py` ✓
    assert_eq!(String::from_utf8_lossy(&output.stdout), "loaded\n");
}

// ---- `P3-32`：`sys.exit` ＋ `SystemExit` 退出码（参照 3.14.4 实测口径）----

fn run_script(tag: &str, source: &str) -> std::process::Output {
    let dir = scratch(tag);
    let path = dir.join(format!("{tag}.py"));
    std::fs::write(&path, source).expect("写脚本");
    Command::new(binary()).arg(&path).output().expect("跑 CLI")
}

#[test]
fn sys_exit_with_int_sets_the_process_exit_code() {
    let output = run_script("exit-int", "import sys\nsys.exit(3)\n");
    assert_eq!(
        output.status.code(),
        Some(3),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    // 未捕获的 `SystemExit` **不是错误** ⇒ 不打"未捕获"诊断
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("未捕获"), "stderr={stderr}");
}

#[test]
fn sys_exit_without_arguments_exits_zero() {
    let output = run_script("exit-none", "import sys\nsys.exit()\n");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sys_exit_with_a_string_prints_it_to_stderr_and_exits_one() {
    let output = run_script("exit-str", "import sys\nsys.exit('boom')\n");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("boom"), "stderr={stderr}");
    assert!(!stderr.contains("未捕获"), "stderr={stderr}");
}

#[test]
fn raise_system_exit_with_an_int_sets_the_process_exit_code() {
    let output = run_script("raise-exit", "raise SystemExit(7)\n");
    assert_eq!(
        output.status.code(),
        Some(7),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sys_exit_is_catchable_as_system_exit() {
    // `sys.exit` **本身不退出进程**：实现＝抛 `SystemExit`（参照同义）
    let output = run_script(
        "exit-caught",
        "import sys\ntry:\n    sys.exit(3)\nexcept SystemExit:\n    print('caught')\n",
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "caught\n");
}

#[test]
fn sys_exit_with_two_arguments_reports_the_measured_type_error() {
    let output = run_script("exit-two", "import sys\nsys.exit(3, 4)\n");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("TypeError: exit expected at most 1 argument, got 2"),
        "stderr={stderr}"
    );
}

// ---- `P3-33`：`PYTHONPATH` 进 `sys.path`（脚本目录之后、`Lib/` 之前）----

#[test]
fn pythonpath_entries_are_importable() {
    let dir = scratch("pythonpath");
    let library = dir.join("pplib");
    std::fs::create_dir_all(&library).expect("建 PYTHONPATH 目录");
    std::fs::write(library.join("ppmod.py"), "X = 1\n").expect("写 PYTHONPATH 模块");
    let path = dir.join("main.py");
    std::fs::write(&path, "import sys, ppmod\nprint(ppmod.X)\n").expect("写脚本");
    let output = Command::new(binary())
        .arg(&path)
        .env("PYTHONPATH", &library)
        .output()
        .expect("跑 CLI");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
}

// ---- `P3-32` 的冒烟：`unittest.main()` 正常退出（依赖任务 A/C：dict kwargs ＋ `Lib/test`）----

#[test]
fn unittest_main_smoke_exits_zero() {
    let output = run_script(
        "unittest-smoke",
        "import unittest\n\nclass T(unittest.TestCase):\n    def test_ok(self):\n        self.assertEqual(1, 1)\n\nunittest.main()\n",
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// **task-18 真 bug 回归**：`setattr(func, "dbval", (args, kwds))` 之后再**读回**该函数属性。
///
/// 先前 stdlib 的 `setattr` 把**借用视图**的实参直接交给"接手新引用"的属性协议入口 ✗ ⇒
/// 运行时新建的 dict 没有别处持有 ⇒ 当场释放 ⇒ `f1.dbval` 读出时 SIGSEGV（官方
/// `test_decorators.test_argforms` 的 `noteargs` 装饰器同型 ✓）。普通 tuple 常被 code
/// object 的常量表另持一份 ⇒ 症状被遮住，只有"含运行时 dict"的载荷才炸 ✓。
#[test]
fn setattr_keeps_a_runtime_dict_alive_in_function_attributes() {
    let dir = scratch("setattr-ownership");
    let path = dir.join("setattr.py");
    let source = r#"
def noteargs(*args, **kwds):
    def decorate(func):
        setattr(func, "dbval", (args, kwds))
        return func
    return decorate


@noteargs("a", "b", one=1)
def f1():
    return 42


print(f1())
print(f1.dbval)
"#;
    std::fs::write(&path, source).expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(
        output.status.code(),
        Some(0),
        "setattr 之后再读回函数属性不应当崩：stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "42\n(('a', 'b'), {'one': 1})\n", "stdout={stdout}");
}

/// **task-18 附带真 bug 回归**：`classmethod(绑定方法)` 的调用（`MethodObject` **套** `MethodObject`）。
///
/// 参照语义：`classmethod(A().foo)` 的 `B.bar()` 等价 `A().foo(B)` ✓。先前 `call_callable` 只剥
/// **一层**绑定方法 ✗ ⇒ 剩下的方法对象被按 `FunctionObject` 读载荷 ✗ ⇒ `function_defaults`
/// 读到垃圾长度（`capacity overflow`／misaligned panic ✗；官方
/// `test_decorators.TestDecorators.test_bound_function_inside_classmethod` 直接崩 ✓）。
#[test]
fn classmethod_wrapping_a_bound_method_calls_through_the_chain() {
    let dir = scratch("nested-method");
    let path = dir.join("nested_method.py");
    let source = r#"
class A:
    def foo(self, cls):
        return "spam"


class B:
    bar = classmethod(A().foo)


print(B.bar())
"#;
    std::fs::write(&path, source).expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(
        output.status.code(),
        Some(0),
        "嵌套绑定方法调用不应当崩：stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "spam\n",
        "stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// **task-22**：`classmethod`／`staticmethod` 包装体的**属性委派 ＋ repr ＋ staticmethod 可调用**。
///
/// 参照 3.14.4 语义：
/// - `__module__`／`__qualname__`／`__name__`／`__doc__`／`__annotations__` 从被包函数**委派** ✓
///   （且与 `func` 上取到的是**同一对象** —— 官方 `test_decorators` 用 `assertIs` ✓）；
/// - `repr` ＝ `<classmethod(<function func at 0x…>)>`／`<staticmethod(…)>` ✓；
/// - `staticmethod(f)(*a) == f(*a)` ✓（3.10+ 可调用）；`classmethod` **仍然不可调用** ✓。
#[test]
fn classmethod_and_staticmethod_expose_the_wrapped_function() {
    let dir = scratch("wrapper-face");
    let path = dir.join("wrapper_face.py");
    let source = r#"
def func(x):
    """文档串。"""
    return x


cm = classmethod(func)
sm = staticmethod(func)
print(cm.__module__, cm.__qualname__, cm.__name__, cm.__doc__)
print(cm.__annotations__, sm.__annotations__)
print(cm.__func__ is func, cm.__wrapped__ is func)
print(sm.__func__ is func, sm.__wrapped__ is func)
print(sm.__name__, sm.__qualname__, sm.__module__, sm.__doc__)
print(getattr(cm, "__name__") is getattr(func, "__name__"))
print(repr(cm).startswith("<classmethod(<function func at 0x"))
print(repr(sm).startswith("<staticmethod(<function func at 0x"))
print(sm(7))
try:
    cm(7)
except TypeError as error:
    print(type(error).__name__, str(error))
"#;
    std::fs::write(&path, source).expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = "__main__ func func 文档串。\n\
                    {} {}\n\
                    True True\n\
                    True True\n\
                    func func __main__ 文档串。\n\
                    True\n\
                    True\n\
                    True\n\
                    7\n\
                    TypeError 'classmethod' object is not callable\n";
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        expected,
        "stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}
