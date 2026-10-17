//! **`array` 的面（`task-18` ✓）**：`multiprocessing` 族的下一道 C 模块闸。
//!
//! 口径（两侧逐行一致 ✓）：
//!
//! * **`FACE_SCRIPT`**：同一段探针在**本机 `python3` 3.14.4** 与 pyawa 上**逐行一致** ✓ ——
//!   模块 `__doc__`／公共名面（`ArrayType`／`array`／`typecodes`）／`typecodes == "bBuwhHiIlLqQfd"`
//!   ／`ArrayType is array`／类型名；**空数组**的 `repr`／`str`／`len`／`bool`／`typecode`／
//!   `itemsize`／`tobytes()`／`tolist()`；14 个 typecode 的 itemsize 真值（`u/w=4` 等）；
//!   构造校验文案（0 参／3 参／关键字／非法 typecode／非 str typecode 三形／`None` 的 `not None`
//!   口径／初始值非可迭代／空初始值三种形态）。
//!   期望值由**本机 `python3` 3.14.4 跑同一段脚本**取得 ✓（`FACE_EXPECTED` 逐行）。
//! * **`HONEST_SCRIPT`**：**如实边界**（pyawa 侧钉住；参照侧无对应物 —— 参照真存元素）：
//!   **只做空数组** —— 带非空初始值构造一律 `NotImplementedError`（消息含"未接线"）；
//!   元素级方法（`append`／`extend`／`frombytes`／`fromlist`／…共 16 个）**不登记**
//!   （缺名 = 语义少接线 ✓，如实记）；已登记的 5 个面（`__init__`／`__repr__`／`__len__`／
//!   `tobytes`／`tolist`）与模块两面（`ArrayType`／`typecodes`）在 ✓。
//!
//! **已知偏差（如实记，不钉参照那侧）**：
//! - `typecode`／`itemsize` 是**实例属性**（参照是只读 getset ⇒ 本层可写）；类型级
//!   `array.array.typecode` 描述符**不存在**（只登记实例面）。
//! - `'u'` 的 `DeprecationWarning` 不打（本层无 warnings 通道）。
//! - 元素存储未接线 ⇒ `tobytes()` 恒 `b''`、`tolist()` 恒 `[]`（**对象长度恒为 0**，因此这两个
//!   返回值就是参照对空数组的真值 ✓，不是编数据）。
//! - 原生 `__doc__`：core 统一空串（第 406 轮口径）⇒ `array.array.__doc__` 可读但为空（参照有 C 文档串）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-array-{}-{name}", std::process::id()));
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

/// 参照侧脚本（`python3` 3.14.4 跑同一段 ⇒ `FACE_EXPECTED` ✓）。
const FACE_SCRIPT: &str = r#"import array

print("array-doc", array.__doc__)
print("array-public", sorted(n for n in dir(array) if not n.startswith("_")))
print("typecodes", array.typecodes, type(array.typecodes).__name__)
print("ArrayType-is-array", array.ArrayType is array.array)
print("class-name", array.array.__name__)

a = array.array("i")
print("repr", repr(a))
print("str", str(a))
print("len", len(a))
print("bool", bool(a))
print("typecode", a.typecode)
print("itemsize", a.itemsize)
print("tobytes", a.tobytes())
print("tolist", a.tolist())

for typecode in array.typecodes:
    empty = array.array(typecode)
    print("itemsize", typecode, empty.itemsize, empty.typecode)

def show(label, fn):
    try:
        fn()
        print(label, "=> no error")
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, ":", exc)

show("no-args", lambda: array.array())
show("3-args", lambda: array.array("i", [], 3))
show("kwargs", lambda: array.array(typecode="i"))
show("bad-typecode", lambda: array.array("x"))
show("int-typecode", lambda: array.array(1))
show("bytes-typecode", lambda: array.array(b"i"))
show("long-str-typecode", lambda: array.array("ii"))
show("none-typecode", lambda: array.array(None))
show("bad-initializer", lambda: array.array("i", 5))
show("empty-initializer", lambda: array.array("i", []))
show("empty-tuple", lambda: array.array("i", ()))
show("empty-bytes", lambda: array.array("i", b""))
"#;

/// 参照（本机 `python3` 3.14.4 跑 `FACE_SCRIPT` 实测 ✓，逐行）。
const FACE_EXPECTED: &str = r#"array-doc This module defines an object type which can efficiently represent
an array of basic values: characters, integers, floating-point
numbers.  Arrays are sequence types and behave very much like lists,
except that the type of objects stored in them is constrained.

array-public ['ArrayType', 'array', 'typecodes']
typecodes bBuwhHiIlLqQfd str
ArrayType-is-array True
class-name array
repr array('i')
str array('i')
len 0
bool False
typecode i
itemsize 4
tobytes b''
tolist []
itemsize b 1 b
itemsize B 1 B
itemsize u 4 u
itemsize w 4 w
itemsize h 2 h
itemsize H 2 H
itemsize i 4 i
itemsize I 4 I
itemsize l 8 l
itemsize L 8 L
itemsize q 8 q
itemsize Q 8 Q
itemsize f 4 f
itemsize d 8 d
no-args !! TypeError : array() takes at least 1 argument (0 given)
3-args !! TypeError : array() takes at most 2 arguments (3 given)
kwargs !! TypeError : array.array() takes no keyword arguments
bad-typecode !! ValueError : bad typecode (must be b, B, u, w, h, H, i, I, l, L, q, Q, f or d)
int-typecode !! TypeError : array() argument 1 must be a unicode character, not int
bytes-typecode !! TypeError : array() argument 1 must be a unicode character, not bytes
long-str-typecode !! TypeError : array() argument 1 must be a unicode character, not a string of length 2
none-typecode !! TypeError : array() argument 1 must be a unicode character, not None
bad-initializer !! TypeError : 'int' object is not iterable
empty-initializer => no error
empty-tuple => no error
empty-bytes => no error
"#;

/// pyawa 侧脚本：如实边界（见文件头 ✓）。
const HONEST_SCRIPT: &str = r#"import array

def verdict(label, fn):
    try:
        value = fn()
        print(label, "=> returned", type(value).__name__, repr(value))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, "未接线" in str(exc))

verdict("initializer", lambda: array.array("i", [1, 2, 3]))
verdict("initializer-bytes", lambda: array.array("B", b"abc"))
verdict("empty-ok", lambda: array.array("i", []))
missing = ["append", "extend", "frombytes", "fromlist", "byteswap", "buffer_info", "count", "index", "insert", "pop", "remove", "reverse", "tofile", "fromfile", "tounicode", "fromunicode"]
print("missing-methods", [name for name in missing if hasattr(array.array, name)])
print("present-methods", [name for name in ("__len__", "__repr__", "tobytes", "tolist", "typecode", "itemsize") if hasattr(array.array, name)])
print("module-attrs", hasattr(array, "ArrayType"), hasattr(array, "typecodes"))
"#;

/// pyawa 侧期望（非空初始值 ⇒ `NotImplementedError`＋"未接线" ✓；元素级方法缺名 ✓）。
const HONEST_EXPECTED: &str = r#"initializer !! NotImplementedError True
initializer-bytes !! NotImplementedError True
empty-ok => returned array array('i')
missing-methods []
present-methods ['__len__', '__repr__', 'tobytes', 'tolist']
module-attrs True True
"#;

#[test]
fn array_face_matches_reference() {
    assert_eq!(run("face", FACE_SCRIPT), FACE_EXPECTED);
}

#[test]
fn array_element_storage_is_honestly_not_wired() {
    assert_eq!(run("honest", HONEST_SCRIPT), HONEST_EXPECTED);
}
