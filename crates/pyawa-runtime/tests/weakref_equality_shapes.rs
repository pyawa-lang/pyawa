//! **`weakref.ref` 的相等／哈希按"目标"算**（`P3-41` round 40 修 ✗）。
//!
//! 参照口径：`weakref.ref(a) == weakref.ref(b)` ⇔ 目标相等；`hash(ref(a)) == hash(a)`。
//! 先前落到"按身份"⇒ `Lib/_weakrefset.py` 的 `WeakSet` **不去重** ⇒ `abc` 的缓存
//! （`_py_abc` 路径、`_abc` 扩展缺位时用它）一次 `isinstance` 长一条 ⇒ `_abc_cache` 成员查询
//! 线性增长 ⇒ **O(n²)**：`test_userstring` 的 6.5 万次 `isinstance` 从 >900 s 降到 6.9 s ✓。
//!
//! 前 3 行与 `python3` 3.14.4 逐字节一致；`cache-*` 两行是**本层实现口径**（参照用 C 的 `_abc`，
//! 没有 `_abc_cache` 属性 ⇒ 不能对拍）：验证"同一类反复 `isinstance` 缓存**只有一条**"。

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

const WEAKREF_SCRIPT: &str = r#"import weakref

class A:
    pass

a = A()
print("same", weakref.ref(a) == weakref.ref(a))
print("hash-same", hash(weakref.ref(a)) == hash(weakref.ref(a)))
print("hash-target", hash(weakref.ref(a)) == hash(a))

u = __import__("collections").UserString("ab")
cls = type(u)
for _ in range(100):
    isinstance(u, cls)
print("cache-100", len(list(cls._abc_cache)))
for _ in range(1000):
    isinstance(u, cls)
print("cache-1100", len(list(cls._abc_cache)))
"#;

const WEAKREF_EXPECTED: &str = "same True
hash-same True
hash-target True
cache-100 1
cache-1100 1
";

#[test]
fn weakref_equality_hash_and_abc_cache_dedupe() {
    assert_eq!(run("probe", WEAKREF_SCRIPT), WEAKREF_EXPECTED);
}
