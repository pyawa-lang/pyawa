//! **`sys.meta_path` 里放什么会崩、什么不会**（第 224 轮的判别结果 ✓，钉成护栏）：
//! 纯 Python finder ✓、空表 ✓、内建函数 ✓、**真模块对象** ✓、嵌套列表 ✓ —— **都不崩** ✗。
//! ⇒ 第 217／218／223 轮那三条 SIGSEGV **不是**"表里有非 Python 对象"造成的 ✗，
//! 而是**我们那个 finder 的造法**（引导期用 `make_native` ＋ `mount` 造出来 ✓）✗。
//! 这四格就是后续排查的**基线** ✓：谁要是把它们弄红了，说明动到了元路径这条线 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-mp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    // **失败必须带走证据**（第 577 轮）：此前只报退出状态 ✗ ⇒ 子进程的 stderr（含
    // `PYAWA_SEGV_TRACE=1` 打出的栈 ✓）全被丢掉 ✓；第 577 轮就是靠这一格把 ① 的崩溃栈
    // 从"只在 harness 里出现 ✗"变成"可读 ✓"的。
    assert!(
        output.status.success(),
        "这一格**不许**崩（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

const TAIL: &str = "print(\"meta_path:\", len(sys.meta_path))\nimport os\nprint(\"os ok:\", os.sep)\n";

#[test]
fn a_python_level_finder_in_meta_path_is_safe() {
    let stdout = run(
        "python_finder",
        &format!(
            "import sys\nsys.meta_path = []\nclass F:\n    def find_spec(self, name, path=None, target=None):\n        return None\nsys.meta_path.append(F())\n{TAIL}"
        ),
    );
    assert!(stdout.contains("meta_path: 1"), "{stdout}");
    assert!(stdout.contains("os ok: /"), "{stdout}");
}

#[test]
fn an_empty_meta_path_is_safe() {
    let stdout = run("empty", &format!("import sys\nsys.meta_path = []\n{TAIL}"));
    assert!(stdout.contains("meta_path: 0"), "{stdout}");
}

#[test]
fn rust_side_objects_in_meta_path_are_safe() {
    // 内建函数 ✓、**真模块对象** ✓、嵌套列表 ✓ —— 三种都不许崩 ✗
    let stdout = run(
        "rust_objects",
        &format!("import sys\nsys.meta_path = [len]\n{TAIL}"),
    );
    assert!(stdout.contains("meta_path: 1"), "{stdout}");
    let stdout = run(
        "module_object",
        &format!("import sys\nsys.meta_path = [sys]\n{TAIL}"),
    );
    assert!(stdout.contains("meta_path: 1"), "{stdout}");
    let stdout = run(
        "nested_list",
        &format!("import sys\nsys.meta_path = [[1, 2]]\n{TAIL}"),
    );
    assert!(stdout.contains("meta_path: 1"), "{stdout}");
}
