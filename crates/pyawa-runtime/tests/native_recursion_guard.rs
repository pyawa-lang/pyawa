//! **A-0 验收（CLI 子进程）**：内部原生递归超限 ⇒ **可捕获的 `RecursionError`**，而不是原生栈
//! 溢出 abort。
//!
//! 背景（round 288）：`==` 的容器比较（`equality_public`／`values_equal`）、字典键哈希
//! （`key_hash_checked`／`hash_object`）、迭代器链推进（`advance_iterator`）都是**原生递归**，
//! 不经过 `execute` ⇒ `MAX_NATIVE_DEPTH` 管不到。环状容器（`a=[]; a.append(a); b=[]; b.append(b);
//! a==b`）实测在 debug/release CLI 上都会 `stack overflow, aborting`（参照 CPython 报
//! `RecursionError`）。本文件把验收钉在**真实 CLI 子进程**上（abort 不会污染测试进程），
//! 与 `T-CX-1`／`DIV-11` 口径一致。

use std::process::Command;

/// 跑一段脚本：要求 **exit 0 ＋ stderr 为空**（abort 会让状态码非 0、stderr 带 stack overflow），
/// 返回 stdout。
fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!(
        "pyawa-native-recursion-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
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
    assert!(
        output.stderr.is_empty(),
        "stderr 必须为空（abort 会在这里留下 stack overflow）：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 照参照口径断言"可捕获"：脚本自己 `try/except RecursionError` 打印标记。
fn expect_recursion_error(name: &str, script: &str) {
    let stdout = run(name, script);
    assert_eq!(stdout, "RECURSION_ERROR\n", "应当捕获 RecursionError：{stdout:?}");
}

#[test]
fn cyclic_list_equality_raises_recursion_error() {
    // CPython 3.14.4：`RecursionError`（不是 abort）。
    expect_recursion_error(
        "cyclic_list",
        r#"
a = []
a.append(a)
b = []
b.append(b)
try:
    print(a == b)
except RecursionError:
    print("RECURSION_ERROR")
"#,
    );
}

#[test]
fn cyclic_dict_equality_raises_recursion_error() {
    expect_recursion_error(
        "cyclic_dict",
        r#"
d = {}
d[1] = d
e = {}
e[1] = e
try:
    print(d == e)
except RecursionError:
    print("RECURSION_ERROR")
"#,
    );
}

#[test]
fn deep_nested_list_equality_raises_recursion_error() {
    expect_recursion_error(
        "deep_list_eq",
        r#"
a = []
b = []
for _ in range(5000):
    a = [a]
    b = [b]
try:
    print(a == b)
except RecursionError:
    print("RECURSION_ERROR")
"#,
    );
}

#[test]
fn deep_nested_tuple_equality_raises_recursion_error() {
    expect_recursion_error(
        "deep_tuple_eq",
        r#"
a = ()
b = ()
for _ in range(5000):
    a = (a,)
    b = (b,)
try:
    print(a == b)
except RecursionError:
    print("RECURSION_ERROR")
"#,
    );
}

#[test]
fn deep_nested_tuple_hash_raises_recursion_error() {
    // 参照：`hash()` 对 tuple 递归；超限同样报 `RecursionError`（不是崩溃）。
    expect_recursion_error(
        "deep_tuple_hash",
        r#"
t = ()
for _ in range(5000):
    t = (t,)
try:
    print(hash(t))
except RecursionError:
    print("RECURSION_ERROR")
"#,
    );
}

#[test]
fn nested_builtin_iterator_chain_raises_recursion_error() {
    // 内建迭代器链（`map(map(...))`）的推进是原生递归；超限 ⇒ 可捕获 `RecursionError`。
    expect_recursion_error(
        "nested_map",
        r#"
m = iter([0])
for _ in range(5000):
    m = map(lambda x: x, m)
try:
    print(next(m))
except RecursionError:
    print("RECURSION_ERROR")
"#,
    );
}

#[test]
fn equality_within_limit_still_works() {
    // 正常范围：100 层相等/不等、环状**同一对象**身份短路（`a == a` 必须为 True，不受递归守卫影响）。
    let stdout = run(
        "within_limit",
        r#"
a = []
b = []
for _ in range(100):
    a = [a]
    b = [b]
print(a == b, a != b)
c = []
c.append(c)
print(c == c)
print({"a": 1} == {"a": 1}, {1, 2} == {2, 1}, (1, [2]) == (1, [2]))
print(sorted([3, 1, 2]), min([3, 1, 2, 4]), max([3, 1, 2, 4]))
print(hash((1, 2)) == hash((1, 2)))
"#,
    );
    assert_eq!(
        stdout,
        "True False\nTrue\nTrue True True\n[1, 2, 3] 1 4\nTrue\n",
        "范围内比较/排序/哈希必须照旧"
    );
}

#[test]
fn recursion_limit_setting_applies_to_native_guard() {
    // 守卫用 `recursion_limit` 现成口径：`setrecursionlimit(50)` 后 200 层容器比较 ⇒ RecursionError。
    expect_recursion_error(
        "lowered_limit",
        r#"
import sys
sys.setrecursionlimit(50)
a = []
b = []
for _ in range(200):
    a = [a]
    b = [b]
try:
    print(a == b)
except RecursionError:
    print("RECURSION_ERROR")
"#,
    );
}
