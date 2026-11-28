//! **`D1`（round 289）：原生回调预算（`PYAWA_NATIVE_DEPTH`）的 CLI 行为**。
//!
//! 背景：`PLAN §12.22` S2 后脚本→脚本调用走显式帧栈，但「内建/协议 → 脚本回调」仍靠 Rust
//! 递归，历史上固定 `MAX_NATIVE_DEPTH=64`。`D1` 把上限改成**宿主声明的栈预算**：CLI 在 512 MiB
//! 线程里声明 1000（见 `crates/pyawa-runtime/src/bin/pyawa.rs`），ABI／嵌入／wasm 不声明仍是 64。
//!
//! 本测试钉 CLI 侧（子进程隔离，abort 不会污染测试进程）：
//! * 200 层用户 `__eq__`／`__repr__` 链 ⇒ 与参照一样**成功**；
//! * 1500 层 ⇒ 按 `recursion_limit`／预算报**可捕获** `RecursionError`，进程正常退出；
//! * 之后实例仍可用（守卫触发不破坏 VM 状态）。
//!
//! ABI／嵌入默认 64 的行为由 `recursion_guard_shapes.rs` 钉住（不变）。

use std::process::Command;

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-native-depth-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
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

const SCRIPT: &str = r#"class Node:
    def __init__(self, child):
        self.child = child

    def __eq__(self, other):
        if not isinstance(other, Node):
            return NotImplemented
        return self.child == other.child

    def __repr__(self):
        return "N(" + repr(self.child) + ")"


def chain(n):
    value = None
    for _ in range(n):
        value = Node(value)
    return value


a = chain(200)
b = chain(200)
print(a == b)
repr(a)
print("repr-ok")

deep_a = chain(1500)
deep_b = chain(1500)
try:
    deep_a == deep_b
except RecursionError:
    print("deep-eq: caught")
else:
    print("deep-eq: no-error")

try:
    repr(deep_a)
except RecursionError:
    print("deep-repr: caught")
else:
    print("deep-repr: no-error")

# 守卫触发之后：VM 仍可用（同一实例继续算）
print(chain(30) == chain(30))
"#;

const EXPECTED: &str = "True\nrepr-ok\ndeep-eq: caught\ndeep-repr: caught\nTrue\n";

#[test]
fn cli_native_callback_depth_follows_declared_budget() {
    assert_eq!(run("depth", SCRIPT), EXPECTED);
}
