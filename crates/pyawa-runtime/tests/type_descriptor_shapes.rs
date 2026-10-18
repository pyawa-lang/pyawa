//! **类型级方法描述符（方案 A）＋ `sys` 小面两格**（task-23 ✓）：两侧逐行一致。
//!
//! 覆盖导入期硬缺的三名 ＋ sys 两格：
//! - `list.append`（`multiprocessing/reduction.py:219` 的 `type(list.append)`）
//! - `int.__add__`（同 `:220` 的 `type(int.__add__)`）
//! - `float.__getformat__`（`test/support/__init__.py:522`）——**纯函数真实现**
//! - `sys._clear_type_descriptors`（`Lib/dataclasses.py:1340`）／`sys.abiflags`
//!
//! **方案 A 的如实偏差 ⚠**：本层类型字典里放的是 `builtin_function_or_method`，
//! 参照是 `method_descriptor`／`wrapper_descriptor` ⇒ **抽例不钉 `type(...)` 类别**
//!（那会与参照必然不同 ✗）；只用 `hasattr`／`callable` 与**语义／错误文案**。语义面在方案 A 下
//! 可以做到与参照**逐行一致** ✓（本文件就是逐行比）。真描述符升格是单独立项（方案 B）。
//!
//! 参照值由本机 `python3`（3.14.4）跑**同一段脚本**取得 ✓（逐行一致）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-type-descriptor-{}-{name}", std::process::id()));
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

/// `list.append`／`int.__add__`／`float.__getformat__`／`sys` 两格的语义与文案。
const DESCRIPTOR_SCRIPT: &str = r#"import sys

print("append:", list.append([], 1))
data = []
print("append data:", list.append(data, 7), data)
print("add:", int.__add__(1, 2), int.__add__(True, 2))
print("add notimpl:", int.__add__(1, "x") is NotImplemented)
print("getformat:", float.__getformat__("double"), float.__getformat__("float"))
print("abiflags:", repr(sys.abiflags))


class C:
    pass


print("clear:", sys._clear_type_descriptors(C))


def show(label, fn):
    try:
        print(label, "=>", repr(fn()))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__ + ":", exc)


show("append()", lambda: list.append())
show("append([],1,2)", lambda: list.append([], 1, 2))
show("append(1,2)", lambda: list.append(1, 2))
show("append kw", lambda: list.append([], 1, x=2))
show("add()", lambda: int.__add__())
show("add(1)", lambda: int.__add__(1))
show("add(1,2,3)", lambda: int.__add__(1, 2, 3))
show("add('a','b')", lambda: int.__add__("a", "b"))
show("add kw", lambda: int.__add__(1, x=2))
show("getformat bad", lambda: float.__getformat__("bad"))
show("getformat(int)", lambda: float.__getformat__(1))
show("getformat()", lambda: float.__getformat__())
show("getformat(2)", lambda: float.__getformat__("double", "float"))
show("clear()", lambda: sys._clear_type_descriptors())
show("clear({})", lambda: sys._clear_type_descriptors({}))
show("clear kw", lambda: sys._clear_type_descriptors(type=C))
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const DESCRIPTOR_EXPECTED: &str = "append: None\n\
append data: None [7]\n\
add: 3 3\n\
add notimpl: True\n\
getformat: IEEE, little-endian IEEE, little-endian\n\
abiflags: ''\n\
clear: None\n\
append() !! TypeError: unbound method list.append() needs an argument\n\
append([],1,2) !! TypeError: list.append() takes exactly one argument (2 given)\n\
append(1,2) !! TypeError: descriptor 'append' for 'list' objects doesn't apply to a 'int' object\n\
append kw !! TypeError: list.append() takes no keyword arguments\n\
add() !! TypeError: descriptor '__add__' of 'int' object needs an argument\n\
add(1) !! TypeError: expected 1 argument, got 0\n\
add(1,2,3) !! TypeError: expected 1 argument, got 2\n\
add('a','b') !! TypeError: descriptor '__add__' requires a 'int' object but received a 'str'\n\
add kw !! TypeError: wrapper __add__() takes no keyword arguments\n\
getformat bad !! ValueError: __getformat__() argument 1 must be 'double' or 'float'\n\
getformat(int) !! TypeError: __getformat__() argument must be str, not int\n\
getformat() !! TypeError: float.__getformat__() takes exactly one argument (0 given)\n\
getformat(2) !! TypeError: float.__getformat__() takes exactly one argument (2 given)\n\
clear() !! TypeError: sys._clear_type_descriptors() takes exactly one argument (0 given)\n\
clear({}) !! TypeError: _clear_type_descriptors() argument must be type, not dict\n\
clear kw !! TypeError: sys._clear_type_descriptors() takes no keyword arguments\n";

#[test]
fn type_descriptor_names_agree_with_the_reference() {
    let stdout = run("type_descriptor_names", DESCRIPTOR_SCRIPT);
    assert_eq!(stdout, DESCRIPTOR_EXPECTED, "{stdout}");
}

/// 名字**存在且可调用**（方案 A 只保证这一点；类别不钉 ✗）。
const PRESENCE_SCRIPT: &str = r#"import sys

print("list.append:", hasattr(list, "append"), callable(list.append))
print("int.__add__:", hasattr(int, "__add__"), callable(int.__add__))
print("float.__getformat__:", hasattr(float, "__getformat__"), callable(float.__getformat__))
print("sys.abiflags:", hasattr(sys, "abiflags"), type(sys.abiflags) is str)
print("sys._clear_type_descriptors:", hasattr(sys, "_clear_type_descriptors"), callable(sys._clear_type_descriptors))
class L(list):
    pass
sub = L()
print("subclass append:", list.append(sub, 3), sub, isinstance(sub, L))
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const PRESENCE_EXPECTED: &str = "list.append: True True\n\
int.__add__: True True\n\
float.__getformat__: True True\n\
sys.abiflags: True True\n\
sys._clear_type_descriptors: True True\n\
subclass append: None [3] True\n";

#[test]
fn type_descriptor_names_are_present_and_callable() {
    let stdout = run("type_descriptor_presence", PRESENCE_SCRIPT);
    assert_eq!(stdout, PRESENCE_EXPECTED, "{stdout}");
}

/// **`P3-17` 同族的存在性／可调用性**（`functools.wraps.__name__` 正路）：两侧逐行一致。
///
/// 只钉"名字在、可调用、语义成立"（方案 A 下 `type(...)` 类别**必然**与参照不同 ⇒ 不钉 ✗；
/// 逐条语义与文案见 `descriptor_protocol.rs` ✓）。
const WRITABLE_DESCRIPTORS_SCRIPT: &str = r#"import functools

print("sm:", hasattr(staticmethod(lambda: 1), "__get__"), callable(staticmethod(lambda: 1).__get__))
print("cm:", hasattr(classmethod(lambda cls: 1), "__get__"), callable(classmethod(lambda cls: 1).__get__))
p = property(lambda self: 1)
print("prop:", hasattr(p, "__get__"), hasattr(p, "__set__"), hasattr(p, "__delete__"))
print("prop callable:", callable(p.__get__), callable(p.__set__), callable(p.__delete__))


def f():
    pass


FD = type(f).__dict__
print("func:", hasattr(FD["__name__"], "__set__"), hasattr(FD["__qualname__"], "__set__"), hasattr(FD["__doc__"], "__set__"), hasattr(FD["__module__"], "__set__"))


def g():
    "doc"


@functools.wraps(g)
def h():
    pass


print("wraps:", h.__name__ == g.__name__, h.__qualname__ == g.__qualname__, h.__doc__ == g.__doc__)
h.__name__ = "zz"
print("wraps writable:", h.__name__)
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const WRITABLE_DESCRIPTORS_EXPECTED: &str = "sm: True True\n\
cm: True True\n\
prop: True True True\n\
prop callable: True True True\n\
func: True True True True\n\
wraps: True True True\n\
wraps writable: zz\n";

#[test]
fn writable_descriptor_names_are_present_and_work() {
    let stdout = run("writable-descriptors", WRITABLE_DESCRIPTORS_SCRIPT);
    assert_eq!(stdout, WRITABLE_DESCRIPTORS_EXPECTED, "{stdout}");
}
