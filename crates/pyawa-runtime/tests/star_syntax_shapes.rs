//! **赋值目标的星号语法错误 ＋ `*` 实参／解包的参照报文**（round 62）。
//!
//! 落地：
//! * 多个星号目标 ⇒ `SyntaxError: multiple starred expressions in assignment`（`for` 目标也同 ✓）；
//! * `*a = …` ⇒ `starred assignment target must be in a list or tuple`；
//! * 裸 `*a`（语句／右值）⇒ `can't use starred expression here`；
//! * `(*x),y = …` 这类括号里单个星号 ⇒ `cannot use starred expression here`；
//! * `*` 实参／显示的源不是可迭代 ⇒ `TypeError: Value after * must be an iterable, not X`
//!   （`LIST_EXTEND` 那条；`{*7}` 是另一句 `'int' object is not iterable` ✓）；
//! * 解包非可迭代 ⇒ `TypeError: cannot unpack non-iterable int object`；
//! * 重复关键字（`DICT_MERGE`）报文里的键用 `str(键)` 再套单引号 ✓（`'1'`／`'('x',)'`）。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`）。

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

const SCRIPT: &str = r#"a = None
[*[*a]] = [[7, 8, 9]]
print("nested-list", a)
for source in [
    "a, *b, c, *d, e = range(10)",
    "[*b, *c] = range(10)",
    "*a = range(10)",
    "*a",
    "x = *a",
    "for a, *b, *c in []: pass",
    "(*x),y = 1, 2",
]:
    try:
        compile(source, "<t>", "exec")
        print(source, "=> OK")
    except SyntaxError as error:
        print(source, "=>", type(error).__name__, error.args[0])

def f(*args, **kwargs):
    return args, kwargs

def show(label, fn):
    try:
        print(label, "OK", fn())
    except BaseException as error:
        print(label, type(error).__name__, error)

show("list-star", lambda: [*7])
show("call-star", lambda: f(1, *7))
show("set-star", lambda: {*7})
show("unpack", lambda: (lambda x: x)(*([1][:1])))
show("dup-int-key", lambda: f(**{1: 3}, **{1: 5}))
"#;

const EXPECTED: &str = "nested-list [[7, 8, 9]]\na, *b, c, *d, e = range(10) => SyntaxError multiple starred expressions in assignment\n[*b, *c] = range(10) => SyntaxError multiple starred expressions in assignment\n*a = range(10) => SyntaxError starred assignment target must be in a list or tuple\n*a => SyntaxError can't use starred expression here\nx = *a => SyntaxError can't use starred expression here\nfor a, *b, *c in []: pass => SyntaxError multiple starred expressions in assignment\n(*x),y = 1, 2 => SyntaxError cannot use starred expression here\nlist-star TypeError Value after * must be an iterable, not int\ncall-star TypeError Value after * must be an iterable, not int\nset-star TypeError 'int' object is not iterable\nunpack OK 1\ndup-int-key TypeError __main__.f() got multiple values for keyword argument '1'\n";

#[test]
fn starred_syntax_and_messages_match_the_reference() {
    assert_eq!(run("star-syntax", SCRIPT), EXPECTED);
}
