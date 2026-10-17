//! **模块／脚本帧的 `__file__`**（第 407 轮，task-14 ✓）——两侧逐行一致。
//!
//! 口径（参照 3.14.4 实测）：
//! * **脚本本体**：`__file__` 是**绝对路径**（`python3 t.py` ⇒ `/…/t.py`），但**不解析符号链接**；
//! * **从文件加载的模块／包**：`__file__` ＝ **加载时用的路径原样**（`sys.path` 项 ＋ 相对路径，
//!   本层的 `Lib/` 项是绝对路径 ⇒ 参照同样给绝对路径 ✓，**不** canonicalize）；
//! * **Rust 侧内建模块**（`sys` 等）**没有** `__file__` ✓（与参照的 builtin 模块一致 ✓）。
//!
//! 抽例用 `os.path.basename`／`isabs` 级比较（临时目录绝对路径不同 ⇒ **不钉绝对路径** ✓）。
//! 期望输出由本机 `python3`（3.14.4）跑**同一段脚本**取得（不是手写 ✓）。

fn run_python3(name: &str, script: &str, helper: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-file-attr-ref-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建目录");
    std::fs::write(root.join("helper.py"), helper).expect("写 helper");
    std::fs::write(root.join("probe.py"), script).expect("写脚本");
    // **参照侧也从本仓 `Lib/` 导入**（否则 `difflib.__file__` 的目录会是 CPython 的
    // `python3.14/`，与本仓 `Lib/` 不可比 ✗）⇒ 用 `PYTHONPATH` 指向同一个 `Lib/`、
    // `-S` 跳过 `site`（与本层不跑 site.py 同口径 ✓）。
    let output = std::process::Command::new("python3")
        .arg("-S")
        .arg(root.join("probe.py"))
        .current_dir(&root)
        .env("PYTHONPATH", concat!(env!("CARGO_MANIFEST_DIR"), "/../../Lib"))
        .output()
        .expect("跑 python3");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "python3 侧失败：{:?}", output);
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn run_pyawa(name: &str, script: &str, helper: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-file-attr-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建目录");
    std::fs::write(root.join("helper.py"), helper).expect("写 helper");
    std::fs::write(root.join("probe.py"), script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(root.join("probe.py"))
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

const HELPER: &str = "VALUE = 7\n";

const SCRIPT: &str = "import os\nimport sys\nimport difflib\nimport functools\nimport xml.etree\nimport helper\n\nprint(\"script_basename\", os.path.basename(__file__))\nprint(\"script_isabs\", os.path.isabs(__file__))\nprint(\"script_dirname\", os.path.basename(os.path.dirname(__file__)) == os.path.basename(os.getcwd()))\n\n\ndef read_file_global():\n    return __file__\n\n\nprint(\"function_same\", read_file_global() == __file__)\n\nprint(\"module_basename\", os.path.basename(difflib.__file__))\nprint(\"module_dirname\", os.path.basename(os.path.dirname(difflib.__file__)))\nprint(\"functools_basename\", os.path.basename(functools.__file__))\nprint(\"package_basename\", os.path.basename(xml.etree.__file__))\nprint(\"package_dirname\", os.path.basename(os.path.dirname(xml.etree.__file__)))\nprint(\"sibling_basename\", os.path.basename(helper.__file__))\nprint(\"sibling_dirname\", os.path.basename(os.path.dirname(helper.__file__)))\nprint(\"rust_module_has_file\", hasattr(sys, \"__file__\"))\nprint(\"os_has_file\", hasattr(os, \"__file__\"))\n";

/// 两侧的临时目录名不同（`pyawa-file-attr-…` 与 `…-ref-…`）⇒ 把临时根目录名归一成 `<TMP>` ✓
/// （抽例本来就不钉绝对路径 ✓ —— 见模块头的口径 ✓）。
fn sanitize(text: &str) -> String {
    text.lines()
        .map(|line| match line.strip_prefix("sibling_dirname ") {
            Some(_) => "sibling_dirname <TMP>".to_owned(),
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn script_and_module_file_attributes_agree_with_the_reference() {
    let expected = sanitize(&run_python3("script", SCRIPT, HELPER));
    let actual = sanitize(&run_pyawa("script", SCRIPT, HELPER));
    assert_eq!(actual, expected, "{actual}");
}
