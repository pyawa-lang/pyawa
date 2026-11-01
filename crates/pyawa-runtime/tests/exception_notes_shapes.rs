//! **PEP 678 `__notes__`**（round 65）：`BaseException.add_note` ＋ `__set_name__` 失败时自动 note。
//!
//! `test_subclassinit.test_set_name_error` 直接读 `cm.exception.__notes__` 的文本 ✓；
//! `add_note` 的非 `str` 实参报 `add_note() argument must be str, not int` ✓。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let library = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Lib");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .env("PYTHONPATH", library)
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

const SCRIPT: &str = r#"try:
    1 / 0
except ZeroDivisionError as error:
    error.add_note("first")
    error.add_note("second")
    print("notes", error.__notes__)
    try:
        error.add_note(3)
    except TypeError as failure:
        print("bad", failure)

class Descriptor:
    def __set_name__(self, owner, name):
        1 / 0

try:
    class NotGoingToWork:
        attr = Descriptor()
except ZeroDivisionError as error:
    print("setname", error.__notes__)
"#;

const EXPECTED: &str = "notes ['first', 'second']\nbad add_note() argument must be str, not int\nsetname [\"Error calling __set_name__ on 'Descriptor' instance 'attr' in 'NotGoingToWork'\"]\n";

#[test]
fn exception_notes_match_the_reference() {
    assert_eq!(run("exception-notes", SCRIPT), EXPECTED);
}
