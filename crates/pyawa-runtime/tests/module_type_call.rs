//! **`X() takes no arguments` 的参照口径**（第 229 轮 ✓）：只有"`__new__` **没被覆盖** ＋ `__init__`
//! 也没被覆盖"时才报 ✓；**覆盖了 `__new__`** 的类型（如 `module` ✓）**允许**多余实参 ✓。
//! 先前一律按"通用分配"报错 ✗ ⇒ `types.ModuleType("x")` 报 `TypeError: module() takes no arguments` ✗。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-mtype-{}-{name}", std::process::id()));
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
fn module_type_accepts_a_name_argument() {
    let stdout = run(
        "module_type",
        "import types\nm = types.ModuleType('x')\nprint('ModuleType ok:', type(m).__name__)\n",
    );
    assert!(stdout.contains("ModuleType ok: module"), "{stdout}");
}

#[test]
fn a_plain_class_still_rejects_arguments() {
    let stdout = run(
        "plain_class",
        "class Empty:\n    pass\ntry:\n    Empty(1)\nexcept TypeError as error:\n    print('Empty(1):', str(error))\n",
    );
    assert!(stdout.contains("Empty() takes no arguments"), "{stdout}");
}
