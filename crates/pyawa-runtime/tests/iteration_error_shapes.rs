//! **迭代协议里的异常必须原样传播**（round 58）。
//!
//! 落地：`Instance::iterable_items` 改成 `Result<Option<Vec<…>>>`——先前迭代中途 `Err(_) => break`
//! 把生成器／`__next__` 里抛的错**吞成"提前结束"** ✗ ⇒ `list(g)`／`sum(g)`／`f(*g)` 静默少元素、
//! 甚至反手报"缺位置实参"✗（`test_extcall` 的 `TypeError: myerror` 用例）。
//! 另外区分「不是可迭代」（`Ok(None)` ⇒ 参照的 `'X' object is not iterable` /
//! `argument after * must be an iterable`）与「迭代协议自己抛错」（原样上抛 ✓）。
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

const SCRIPT: &str = r#"def broken():
    yield 1
    raise TypeError("myerror")

def gen():
    raise ValueError("boom")

def g(x):
    return x

class BrokenIterable:
    def __iter__(self):
        raise TypeError("iterboom")

class BrokenNext:
    def __iter__(self):
        return self
    def __next__(self):
        raise KeyError("nextboom")

cases = [
    ("list", lambda: list(broken())),
    ("tuple", lambda: tuple(broken())),
    ("sum", lambda: sum(broken())),
    ("star", lambda: g(*broken())),
    ("list-gen", lambda: list(gen())),
    ("iter-protocol", lambda: list(BrokenIterable())),
    ("next-protocol", lambda: list(BrokenNext())),
    ("not-iterable", lambda: list(7)),
]
for label, fn in cases:
    try:
        print(label, "OK", fn())
    except BaseException as error:
        print(label, type(error).__name__, error)
"#;

const EXPECTED: &str = "list TypeError myerror\ntuple TypeError myerror\nsum TypeError myerror\nstar TypeError myerror\nlist-gen ValueError boom\niter-protocol TypeError iterboom\nnext-protocol KeyError 'nextboom'\nnot-iterable TypeError 'int' object is not iterable\n";

#[test]
fn iteration_protocol_errors_propagate() {
    assert_eq!(run("iter-errors", SCRIPT), EXPECTED);
}
