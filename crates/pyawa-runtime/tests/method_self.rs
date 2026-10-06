//! **绑定方法的 `__self__`／`__func__`**（第 259 轮 ✓）：`classmethod`／`staticmethod` 那对钩子
//! （第 346 轮，39 个模块的卡点 ✓）是模板 ✓；**绑定 `method` 此前完全没有属性钩子** ✗ ⇒
//! `m.__self__` 报 `AttributeError: 'method' object has no attribute '__self__'` ✗。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-mself-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "应当成功：{output:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn a_bound_method_exposes_self_and_func() {
    let stdout = run(
        "self_func",
        "class C:\n    def m(self):\n        return 1\nc = C()\nbound = c.m\nprint('func:', bound.__func__.__name__)\nprint('self is c:', bound.__self__ is c)\n",
    );
    assert!(stdout.contains("func: m"), "{stdout}");
    assert!(stdout.contains("self is c: True"), "{stdout}");
}
