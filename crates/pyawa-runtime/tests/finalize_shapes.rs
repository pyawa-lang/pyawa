//! **终结器（`__del__`）的形状**（第 578 轮）—— 修掉两个真 bug 之后钉成护栏 ✓：
//!
//! 1. `incref` 的 `debug_assert!(refcount > 0)` 先前**挡住了整个终结器路径** ✗：
//!    `release_one` 按 `OM-20` ① 在**计数归零后**才调终结器 ✓，而终结器要把 `__del__`
//!    **绑到 `self`** ✓（`_PyObject_LookupSpecial` ✓）⇒ 从 0 incref 是**合法复活** ✓
//!    ⇒ 旧断言让**任何带 `__del__` 的脚本**在 debug 下当场中止 ✓（参照打 `bye/end` ✓）。
//! 2. `python_level_finalize` 先前走**完整 `getattr`** ✗（`call_object_method` ✓）：
//!    对 `super` 对象会绕进 `super_lookup` ✓ 读它自己**正在释放**的属性字典 ✓
//!    ⇒ 崩溃的读者就是它 ✓（第 578 轮实测 ✓，修后同一压力 **27/36 红 ⇒ 0/36** ✓）。
//!    参照取 `__del__` 用**特殊查找** ✓：只扫 `type(self)` 的 MRO ✓ ⇒ 本函数改成先扫 MRO ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-finalize-{}-{name}", std::process::id()));
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

#[test]
fn a_rebound_instance_calls_dunder_del() {
    // 参照：`bye` 在 `a = None` 时立刻打 ✓，随后 `end` ✓
    let stdout = run(
        "rebound",
        "class A:\n    def __del__(self):\n        print(\"bye\")\na = A()\na = None\nprint(\"end\")\n",
    );
    assert_eq!(stdout, "bye\nend\n", "{stdout}");
}

#[test]
fn dunder_del_may_resurrect_its_object() {
    // 参照（PEP 442）：终结器里把 `self` 存回全局 ⇒ 对象**复活** ✓（随后不再终结 ✓）
    let stdout = run(
        "resurrect",
        "saved = None\nclass A:\n    def __del__(self):\n        global saved\n        saved = self\na = A()\na = None\nprint(\"after:\", saved is not None)\n",
    );
    assert_eq!(stdout, "after: True\n", "{stdout}");
}
