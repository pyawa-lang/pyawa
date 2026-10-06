//! **内置类型子类的基础面**（护栏 ✓，守第 262／263 轮那两笔修复）：
//! - 第 262 轮：内置基类的 `repr`／`str` **槽**要随布局一起继承 ✓（先前子类退化成 `<D object at …>` ✗）；
//! - 第 263 轮：**内建布局继承表**要含数值／文本类型 ✓（先前 `class I(int)` 落到通用布局 ✗ ⇒ `I(5)` 报
//!   `I() takes no arguments` ✗）。
//! 两笔此前只有临场复现脚本（`target/recon/subs.py`）✓，没有入库的回归保护 ✗ —— 本文件补上 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-subcls-{}-{name}", std::process::id()));
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
fn a_dict_subclass_keeps_the_builtin_repr_and_len() {
    // 参照：`{'a': 1} 1 1`（第 262 轮修好 ✓）。
    let stdout = run(
        "dict",
        "class D(dict):\n    pass\n\nd = D()\nd['a'] = 1\nprint(repr(d), len(d), d['a'])\n",
    );
    assert!(stdout.contains("{'a': 1} 1 1"), "{stdout}");
}

#[test]
fn a_list_subclass_keeps_the_builtin_repr_and_len() {
    // 参照：`[1] 1 1`（同上 ✓）。
    let stdout = run(
        "list",
        "class L(list):\n    pass\n\nl = L()\nl.append(1)\nprint(repr(l), len(l), l[0])\n",
    );
    assert!(stdout.contains("[1] 1 1"), "{stdout}");
}

#[test]
fn an_int_subclass_can_be_constructed_with_an_argument() {
    // 参照：`5 6`（第 263 轮修好 ✓ —— 那之前报 `I() takes no arguments` ✗）。
    let stdout = run(
        "int",
        "class I(int):\n    pass\n\ni = I(5)\nprint(repr(i), i + 1)\n",
    );
    assert!(stdout.contains("5 6"), "{stdout}");
}
