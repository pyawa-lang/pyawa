//! **绑定方法的函数面代理**（第 276 轮 ✓）：参照里 `d.__setitem__.__qualname__` 转发给被绑函数 ✓；
//! 先前没有这几格 ✗ ⇒ `AttributeError: 'method' object has no attribute '__qualname__'` ✗。

#[test]
fn a_bound_method_proxies_the_function_face() {
    let root = std::env::temp_dir().join(format!("pyawa-mface-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(
        &path,
        "class D(dict):\n    def __setitem__(self, k, v):\n        super().__setitem__(k, v)\n\nd = D()\nbound = d.__setitem__\nprint('qualname:', bound.__qualname__)\nprint('name:', bound.__name__)\n",
    )
    .expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "不应失败：{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("qualname: D.__setitem__"), "{stdout}");
    assert!(stdout.contains("name: __setitem__"), "{stdout}");
}
