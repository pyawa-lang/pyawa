//! **内建类型的严格子类：实例访问要绑到子类自己的函数**（第 271 轮 ✓）：`class D(dict)` 的
//! `D.__setitem__` 记在**子类类型字典** ✓，而 `dict` 的 `getattr` 槽被**继承** ✗ ⇒ 先前实例拿到
//! 基类 native ✗（第 269 轮实测 `<bound method builtin_function_or_method of {}>` ✗，
//! 参照 `<bound method D.__setitem__ of {}>` ✓）。同一根卡住 `Lib/enum.py` 的 `EnumDict.__setitem__` ✓。
//! **条件已收窄**（第 270 轮那版放太宽 ✗ ⇒ 被判据③拦下 ✗；本轮只对内建严格子类生效 ✓）。

#[test]
fn a_builtin_subclass_instance_binds_its_own_method() {
    let root = std::env::temp_dir().join(format!("pyawa-submeth-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(
        &path,
        "class D(dict):\n    def __setitem__(self, k, v):\n        super().__setitem__(k, v)\n\nd = D()\nprint('bound:', str(d.__setitem__))\n",
    )
    .expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "不应失败：{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bound: <bound method D.__setitem__"), "{stdout}");
}
