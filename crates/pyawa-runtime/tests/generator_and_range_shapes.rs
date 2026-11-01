//! **生成器重入守卫 ＋ `range` 的成员判定**（round 56）。
//!
//! 落地：
//! * 生成器（协程／异步生成器同族）**正在执行**时再被驱动 ⇒ 参照原文
//!   `ValueError: generator already executing`（种类词随对象：`coroutine already executing`）。
//!   先前**不查重入** ✗ ⇒ 递归恢复同一帧 ⇒ `帧操作失败：StackUnderflow`（`test_genexps` 的
//!   `g = (next(me) for i in range(10))` 一头撞死 ✓）。
//! * `in range(...)` 是**算术判定**（不是容器迭代）：`1.0 in range(3)` ⇒ True、
//!   `'a' in range(3)` ⇒ False、`range(3) in range(3)` ⇒ False、负步长同理 ✓。
//!   先前没有这一档 ✗ ⇒ TypeError "argument of type 'range' is not a container or iterable"
//!   （`test_genexps` 的 `... if j in range(n)` ✓）。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-{}-{}", std::process::id(), name));
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

#[test]
fn generator_reentrancy_raises_value_error() {
    let script = r#"def gen():
    yield next(me)

me = gen()
try:
    next(me)
except ValueError as error:
    print(type(error).__name__, error)
"#;
    assert_eq!(run("reentrancy", script), "ValueError generator already executing\n");
}

#[test]
fn range_membership_is_arithmetic() {
    let script = r#"print(1 in range(3), 1.0 in range(3), 'a' in range(3))
print(4 in range(0, 10, 2), 5 in range(0, 10, 2), 1 in range(5, 0, -1))
print(range(3) in range(3))
"#;
    assert_eq!(
        run("range-contains", script),
        "True True False\nTrue False True\nFalse\n"
    );
}

#[test]
fn nested_generator_for_clauses_with_range_condition() {
    let script = r#"print(list((i, j) for i in range(3) for j in range(2) if j in range(2)))
"#;
    assert_eq!(
        run("nested-genexp", script),
        "[(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1)]\n"
    );
}
