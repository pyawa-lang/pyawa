//! **硬关键字／PEP 479／抽象类实例化**三条"只差一条"的回归（round 64）。
//!
//! * `keyword.kwlist` 里每个关键字当赋值名字都必须 `SyntaxError`，软关键字仍可当名字 ✓；
//! * 生成器漏出的 `StopIteration` 包装成 `RuntimeError` 时，`__cause__`／`__context__` 是原
//!   `StopIteration`、`__suppress_context__` 为真（`test_generator_stop` ✓）；
//! * 抽象类实例化 ⇒ `TypeError: Can't instantiate abstract class X without an implementation for
//!   abstract method(s) …`（`test_dynamicclassattribute` ✓）。
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

const SCRIPT: &str = r#"import abc
import keyword

bad = []
for key in keyword.kwlist:
    try:
        exec("%s = 42" % key)
        bad.append(key)
    except SyntaxError:
        pass
print("keywords", bad)

soft_ok = True
for key in keyword.softkwlist:
    ns = {}
    try:
        exec("%s = 42" % key, ns)
    except SyntaxError:
        soft_ok = False
print("soft", soft_ok)

def f():
    raise StopIteration

def g():
    yield f()

try:
    next(g())
except RuntimeError as error:
    print(
        "pep479",
        type(error.__cause__).__name__,
        type(error.__context__).__name__,
        error.__suppress_context__,
    )

class A(metaclass=abc.ABCMeta):
    @abc.abstractmethod
    def color(self):
        pass

try:
    A()
except TypeError as error:
    print("abstract1", error)

class B(metaclass=abc.ABCMeta):
    @abc.abstractmethod
    def alpha(self):
        pass

    @abc.abstractmethod
    def beta(self):
        pass

try:
    B()
except TypeError as error:
    print("abstract2", error)
"#;

const EXPECTED: &str = "keywords []\nsoft True\npep479 StopIteration StopIteration True\nabstract1 Can't instantiate abstract class A without an implementation for abstract method 'color'\nabstract2 Can't instantiate abstract class B without an implementation for abstract methods 'alpha', 'beta'\n";

#[test]
fn keywords_pep479_and_abstract_classes_match_the_reference() {
    assert_eq!(run("kw-abstract", SCRIPT), EXPECTED);
}
