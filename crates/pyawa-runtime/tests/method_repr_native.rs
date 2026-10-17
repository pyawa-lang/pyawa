//! **绑定方法打印不应野读**（第 269 轮 ✓）：被绑的函数可能是 **native**
//! （`builtin_function_or_method` ✓，没有 `code()` ✗）—— 先前 `method_repr` 一律按
//! `FunctionObject` 取 `code()` ✗ ⇒ 把 native 载荷当 `CodeObject` 读 ⇒ 野读
//! （实测 `print(d.__setitem__)` ⇒ `memory allocation of 8386098843153034355 bytes failed` ✗）。

#[test]
fn printing_a_bound_method_of_a_builtin_subclass_does_not_abort() {
    let root = std::env::temp_dir().join(format!("pyawa-mrepr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(
        &path,
        "class D(dict):\n    pass\n\nd = D()\nprint('ok:', str(d.__setitem__)[:14])\n",
    )
    .expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "不应 aborted：{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ok: <bound meth"), "{stdout}");
}
