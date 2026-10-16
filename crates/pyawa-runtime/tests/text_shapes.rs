//! **`print` 与 `str()` 必须走同一处**（第 597 轮真 bug 修 ✓）。
//!
//! 先前 `object_str_native` 只认类型的 `str` **槽** ✗ 并回落默认 `repr` ✗ —— **不查类字典的 `__str__`** ✗
//! ⇒ 凡覆写 `__str__` 的用户类，`print(a)` 都打 `<A object at 0x…>` ✗（参照打 `A-str` ✓），
//! 而 `str(a)` 却是对的 ✓ ⇒ 两处**口径分叉** ✓（面很宽 ✓）。
//! 修法：`object_str_native` 先走**属性通道**找 `__str__` ✓ —— 与 `object_repr` 对 `__repr__` 的口径一致 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-text-{}-{name}", std::process::id()));
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

const SCRIPT: &str = r#"
class A:
    def __str__(self):
        return "A-str"
a = A()
print(a)
print(str(a))
print("prefix:", a)
"#;

/// 参照（`python3` 3.14 实测 ✓）：三条都是 `A-str` ✓（`print` 走 `str()` ✓）。
const EXPECTED: &str = "A-str\nA-str\nprefix: A-str\n";


const FALLBACK_SCRIPT: &str = r#"
class B:
    def __repr__(self):
        return "B-repr"
b = B()
print(b)
print(str(b), repr(b))
class C:
    def __str__(self):
        return "C-str"
    def __repr__(self):
        return "C-repr"
c = C()
print(c, str(c), repr(c))
"#;

/// 参照（`python3` 3.14 实测 ✓）：只有 `__repr__` ⇒ `str()` **回落**到它 ✓（`b`／`str(b)`／`repr(b)` 都是 `B-repr` ✓）；
/// 两个都有 ⇒ `str` 用 `__str__` ✓、`repr` 用 `__repr__` ✓。
/// **如实** ✗：另一个**不相干**的缺口没混进来 —— 默认 repr 的**模块限定**：我们打 `<D object at 0x…>` ✓
/// 而参照打 `<__main__.D object at 0x…>` ✗（已记进 `NEXT.md` ✓）。
const FALLBACK_EXPECTED: &str = "B-repr\nB-repr B-repr\nC-str C-str C-repr\n";

#[test]
fn str_falls_back_to_repr() {
    let stdout = run("fallback", FALLBACK_SCRIPT);
    assert_eq!(stdout, FALLBACK_EXPECTED, "{stdout}");
}

#[test]
fn print_honours_dunder_str() {
    let stdout = run("dunder_str", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}

const MODULE_SCRIPT: &str = r#"
class D:
    pass
d = D()
print(str(d).startswith("<__main__.D object at 0x"))
print(repr(object()).startswith("<object object at 0x"))
class E:
    pass
E.__module__ = "mymod"
print(str(E()).startswith("<mymod.E object at 0x"))
"#;

/// 参照（`python3` 3.14 实测 ✓）：默认 repr 带**模块限定** ✓ —— 脚本里的类在 `__main__` ⇒ `<__main__.D …>` ✓；
/// **内建类型不带前缀** ✓（`object()` ⇒ `<object object at …>` ✓）；`__module__` 被改成别的名字 ⇒ 用那个名字 ✓
/// （`mymod.E` ✓）。三条断言都依赖这个形状 ✓（所以它们能钉住行为 ✓）。
const MODULE_EXPECTED: &str = "True\nTrue\nTrue\n";

#[test]
fn default_repr_is_module_qualified() {
    let stdout = run("module", MODULE_SCRIPT);
    assert_eq!(stdout, MODULE_EXPECTED, "{stdout}");
}

const DECODE_SCRIPT: &str = r#"
def bad(fn):
    try:
        fn()
    except TypeError:
        return "TypeError"
    return "no-error"

print(str(b'abc', 'latin1'))
print(str(b'\xc3\xa9', 'utf-8'))
print(str(b'abc', 'latin1', 'strict'))
print(str(b'abc'))
print(str('abc'))
print(bad(lambda: str('abc', 'utf-8')))
print(bad(lambda: str(123, 'utf-8')))
print(bad(lambda: str(b'abc', 1)))
"#;

/// 参照（`python3` 3.14 实测 ✓）：**两实参／三实参**的 `str(bytes, encoding[, errors])` ⇒ **解码** ✓
/// （`latin-1` ⇒ `abc` ✓、`utf-8` ⇒ `é` ✓）；单实参仍是**那一套 `str(x)`** ✓（`str(b'abc')` ⇒ `b'abc'` ✓）；
/// 三种坏形状都 `TypeError` ✓（`str` 对象／非 bytes 对象／非 str 的 encoding ✓）。
/// **真 bug 修** ✗（task-17 第 2 轮）：先前**只看第一个实参** ⇒ `str(b'abc', 'latin1')` 回 `"b'abc'"` ✗
/// —— 正是 `Lib/re/_parser.py:235` 的 bytes tokenizer ⇒ bytes 正则一直在读 `repr(bytes)` ✗。
/// **如实** ✗：`errors` 只认 `strict` 的报错面（同 `bytes.decode` ✓）；`str(bytearray, …)` 仍 `TypeError` ✗。
const DECODE_EXPECTED: &str = "abc\né\nabc\nb'abc'\nabc\nTypeError\nTypeError\nTypeError\n";

#[test]
fn str_two_argument_form_decodes_bytes() {
    let stdout = run("decode", DECODE_SCRIPT);
    assert_eq!(stdout, DECODE_EXPECTED, "{stdout}");
}
