//! **`print` 与 `str()` 必须走同一处**（第 597 轮真 bug 修 ✓）。
//!
//! 先前 `object_str_native` 只认类型的 `str` **槽** ✗ 并回落默认 `repr` ✗ —— **不查类字典的 `__str__`** ✗
//! ⇒ 凡覆写 `__str__` 的用户类，`print(a)` 都打 `<A object at 0x…>` ✗（参照打 `A-str` ✓），
//! 而 `str(a)` 却是对的 ✓ ⇒ 两处**口径分叉** ✓（面很宽 ✓）。
//! 修法：`object_str_native` 先走**属性通道**找 `__str__` ✓ —— 与 `object_repr` 对 `__repr__` 的口径一致 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-text-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
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

const SCRIPT: &str = r#"
class A:
    def __str__(self):
        return "A-str"
a = A()
print(a)
print(str(a))
print("prefix:", a)
"#;

/// 参照（`python3` 3.14 实测 ✓）：三条都是 `A-str` ✓（`print` 走 `str()` ✓）。
const EXPECTED: &str = "A-str\nA-str\nprefix: A-str\n";

#[test]
fn print_honours_dunder_str() {
    let stdout = run("dunder_str", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
