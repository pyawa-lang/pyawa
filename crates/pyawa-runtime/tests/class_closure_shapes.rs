//! **类体也是闭包体**（task-24 轮 2 步① ✓）。
//!
//! 真缺口 ✗：`def outer(x): class C: y = x` —— 类体引用**外层函数的局部**时，参照把该局部变成
//! **cell**（实测 `outer.__code__.co_cellvars == ('x',)` ✓）、类体走 `COPY_FREE_VARS`／`LOAD_DEREF` ✓；
//! 本层先前：① 符号表只收嵌套 `def` ✗ ⇒ `x` 不成 cell ✓；② 类体编译器不吃 freevars ✗ ⇒ 类体里发
//! `LOAD_NAME` ✓；③ `__build_class__` 造类体帧时**不装闭包** ✗ ⇒ 即便前两步修好也读到**空 cell**
//! （`NameError: cannot access free variable 'x' where it is not associated with a value yet` ✓）。
//! 三步一起补齐 ✓（与函数调用那条 `install_closure` 同一口径 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-classclo-{}-{name}", std::process::id()));
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

/// 类体引用外层局部（两种形态：直接读 ✓／经 `sum(...)` 读 ✓）。
const CLASS_CLOSURE_SCRIPT: &str = r#"def f(x):
    class C:
        y = x

    return C


print(f(7).y)
print(f(9).y)


def g(items):
    class D:
        total = sum(items)

    return D.total


print(g([1, 2, 3]))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const CLASS_CLOSURE_EXPECTED: &str = "7\n9\n6\n";

#[test]
fn class_body_closes_over_enclosing_locals_with_the_reference() {
    let stdout = run("class_closure", CLASS_CLOSURE_SCRIPT);
    assert_eq!(stdout, CLASS_CLOSURE_EXPECTED, "{stdout}");
}
