//! **`__call__`：实例可调用**（第 593 轮真缺口 ✓）。
//!
//! 先前 core 里**一处都没有** `__call__` 的调用分派 ✗ ⇒ 任何带 `__call__` 的用户类被调用都报
//! `TypeError: 'X' object is not callable` ✗（`_sre.template` 要返回的**模板可调用对象**也压在这上面 ✓）。
//! 现在：`call_callable` 在"不是那几种可调用类型"时，先走**属性通道**找 `__call__` ✓（同一处真相 ✓，
//! 不另开分派 ✗）；`Instance::is_callable`（`callable()` 的口径 ✓）同步对齐 ✓ —— 否则会出现
//! "`callable(x)` 为 `False` 但 `x()` 能调"的口径分叉 ✗（第一版就是 15 / False ✗）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-callable-{}-{name}", std::process::id()));
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
class Adder:
    def __init__(self, bias):
        self.bias = bias
    def __call__(self, value):
        return value + self.bias
add = Adder(10)
print(add(5), callable(add))
print(list(map(add, [1, 2, 3])))
class Doubler:
    def __call__(self, value, times=1):
        return value * (2 ** times)
print(Doubler()(3), Doubler()(3, 4))
"#;

/// 参照（`python3` 3.14 实测 ✓）：调用 ✓、`callable()` ✓、`map` ✓、默认参数 ✓。
const EXPECTED: &str = "15 True\n[11, 12, 13]\n6 48\n";

#[test]
fn instances_with_dunder_call_are_callable() {
    let stdout = run("dunder_call", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
