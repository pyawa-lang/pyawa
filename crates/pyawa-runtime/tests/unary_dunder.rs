//! **一元运算符的 dunder 面**（第 219／220 轮）✓：`-obj`／`~obj`／`+obj` 走对象的 dunder ✓；
//! **没有** dunder 时，错误面与参照**逐字一致** ✓（`TypeError: bad operand type for unary +: 'C'` ✓
//! —— 第 219 轮之前这里报的是 `Unsupported`，连文案都对不上 ✗）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-unary-{}-{name}", std::process::id()));
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
fn unary_operators_use_object_dunders() {
    let stdout = run(
        "dunders",
        "class C:\n    def __neg__(self):\n        return 'neg'\n    def __invert__(self):\n        return 'inv'\nclass D:\n    def __pos__(self):\n        return 'pos'\nprint(-C(), ~C(), +D())\nprint(-3, -1.5, abs(-2), ~5, +1.5)\n",
    );
    assert!(stdout.contains("neg inv pos"), "dunder 面：{stdout}");
    assert!(stdout.contains("-3 -1.5 2 -6 1.5"), "数字面不许回归：{stdout}");
}

#[test]
fn unary_error_face_matches_the_reference() {
    let stdout = run(
        "error_face",
        "class C:\n    def __neg__(self):\n        return 'neg'\ntry:\n    print(+C())\nexcept TypeError as error:\n    print('TypeError:', str(error))\n",
    );
    assert!(
        stdout.contains("bad operand type for unary +: 'C'"),
        "错误面要与参照一致（不是 Unsupported）：{stdout}"
    );
}
