//! **doctest 的导入前置面**（round 49）：`sys.gettrace`／`displayhook`／`stdin`、隐式异常的
//! `__traceback__`、编码器的 `backslashreplace`。
//!
//! 为什么这些一起做：`test_extcall`／`test_genexps`／`test_unpack_ex`／`test_pep646_syntax`
//! 四个文件的**唯一**错误曾是 doctest 的导入前置（`sys.gettrace` → `sys.stdin` → `sys.displayhook`
//! → `exc.__traceback__.tb_next` → `ascii() ... backslashreplace`）——逐层暴露、逐一补掉后它们
//! 才真正跑进 doctest 用例（剩余缺口见台账：`DICT_MERGE` 的映射协议与同名键报文）。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
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

const SCRIPT: &str = r#"import sys

print("trace", sys.gettrace(), sys.getprofile())
sys.settrace(None)
sys.setprofile(None)
print("stdin", sys.stdin.name, sys.stdin.mode, sys.stdin.isatty(), repr(sys.stdin.read()))
print("stdout", sys.stdout.name, sys.stdout.encoding)
sys.displayhook(42)
sys.displayhook(None)
print("esc", "é".encode("ascii", "backslashreplace"), repr("\u20ac".encode("ascii", "backslashreplace")), repr("\U0001f600".encode("utf-8", "backslashreplace")))

def divide():
    1 / 0

try:
    divide()
except ZeroDivisionError as error:
    print("divide-tb", error.__traceback__ is not None)

try:
    compile("1+", "<x>", "single")
except SyntaxError as error:
    print("compile-tb", error.__traceback__ is not None)

try:
    raise ValueError("v")
except ValueError as error:
    print("raise-tb", error.__traceback__ is not None)"#;

const EXPECTED: &str = "trace None None\nstdin <stdin> r False ''\nstdout <stdout> utf-8\n42\nesc b'\\\\xe9' b'\\\\u20ac' b'\\xf0\\x9f\\x98\\x80'\ndivide-tb True\ncompile-tb True\nraise-tb True\n";

#[test]
fn doctest_support_surface_matches_the_reference() {
    assert_eq!(run("doctest", SCRIPT), EXPECTED);
}
