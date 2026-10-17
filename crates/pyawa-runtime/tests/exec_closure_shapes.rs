//! **`exec` ＋ `nonlocal` 增强赋值 ＋ 推导式清理块顺序**（task-20 第 3 轮 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓（脚本见各 `const` ✓）。
//!
//! 被这组抽例钉住的真 bug：
//! ① **函数收尾与推导式清理块的顺序** ✗：先前 `else` 支先 `flush_pending_cleanups()`、再发隐式
//!    `LOAD_CONST None; RETURN_VALUE` ✗ ⇒ **正常路径直接落进处理块** ✓（`SWAP 2` 撞空栈 ⇒
//!    `帧操作失败：StackUnderflow` ✓ —— `Lib/dataclasses.py::_init_fn` 的
//!    `func_builder.add_fn(…)` 之后就是这样 ✓）⇒ 改成"**先返回、后冲刷处理块**" ✓（参照同序 ✓，
//!    编译器逐字节夹具不动 ✓）；
//! ② **`nonlocal x; x += 1`** ✗：增强赋值的名字目标只认**局部 varname** ✗ ⇒ 落到
//!    `LOAD_NAME`／`STORE_NAME` ✓ ⇒ 函数帧没有命名空间 ⇒ `LOAD_NAME 需要命名空间帧` ✓
//!    （`Lib/functools.py::_lru_cache_wrapper` 一族就卡在这 ✓）⇒ cell／自由变量优先
//!    （`LOAD_DEREF`／`STORE_DEREF` ✓）；
//! ③ **`exec` 未接线** ✗：`Lib/dataclasses.py::_create_fn` 一族靠它跑**生成的源码** ✓ ⇒
//!    `Instance::exec_source`（与 `eval_source` **同一处真相** ✓：`compile` ⇒ `instantiate` ⇒
//!    命名空间帧 ⇒ `execute` ✓）＋ `sys.get/setrecursionlimit` ✓（后者**只降不升** ✓，如实 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-exec-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 推导式清理块排在**隐式返回之后** ✓（`tail()` 那种"函数体以无返回的调用收尾" ✓）
const CLEANUP_SCRIPT: &str = r#"def run(xs):
    items = [x * 2 for x in xs if x]
    return items


def tail(xs):
    total = sum([x for x in xs])
    print("total", total)


print(run([1, 0, 2]))
tail([1, 2, 3])
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const CLEANUP_EXPECTED: &str = "[2, 4]\ntotal 6\n";

#[test]
fn comprehension_cleanup_after_epilogue_agrees_with_the_reference() {
    let stdout = run("comprehension_cleanup_after_epilogue_agrees", CLEANUP_SCRIPT);
    assert_eq!(stdout, CLEANUP_EXPECTED, "{stdout}");
}

/// `nonlocal x; x += 1` 走 `LOAD_DEREF`／`STORE_DEREF` ✓（先前 `LOAD_NAME` ⇒ 函数帧没有命名空间 ✗）
const NONLOCAL_SCRIPT: &str = r#"def a():
    misses = 0

    def wrapper():
        nonlocal misses
        misses += 1
        return misses

    return wrapper


w = a()
print("A", w(), w())
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const NONLOCAL_EXPECTED: &str = "A 1 2\n";

#[test]
fn nonlocal_augassign_uses_deref_agrees_with_the_reference() {
    let stdout = run("nonlocal_augassign_uses_deref_agrees", NONLOCAL_SCRIPT);
    assert_eq!(stdout, NONLOCAL_EXPECTED, "{stdout}");
}

/// `exec(源码, globals, locals)` ✓＋`sys.set/getrecursionlimit` ✓＋`@dataclass`（无字段 ✓）
const EXEC_SCRIPT: &str = r#"import sys

ns = {}
exec("x = 5\ny = x + 1\n", ns, ns)
print(ns["y"], ns["x"])

ns2 = {}
exec("def f(a, b):\n    return a + b\n", ns2, ns2)
print(ns2["f"](2, 3))

sys.setrecursionlimit(50)
print(sys.getrecursionlimit())


from dataclasses import dataclass


@dataclass
class Empty:
    pass


print(Empty())
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const EXEC_EXPECTED: &str = "6 5\n5\n50\nEmpty()\n";

#[test]
fn exec_recursion_and_dataclass_agree_with_the_reference() {
    let stdout = run("exec_recursion_and_dataclass_agree", EXEC_SCRIPT);
    assert_eq!(stdout, EXEC_EXPECTED, "{stdout}");
}

