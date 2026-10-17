//! **`set`／`dict` 的就地位运算族**（task-4 ✓；立据＝`sysconfig/__init__.py:165` 的
//! `_INSTALL_SCHEMES |= {…}` 一族，上游 overlay 口径下压着 42 个模块 ✓）。
//!
//! ① `set` 的 `|=`／`&=`／`-=`／`^=`：**就地**改并回**同一对象**（`is` 为真 ✓）；右侧只认
//! `set`／`frozenset`（参照实测：`s |= [4]` ⇒ `unsupported operand type(s) for |=: 'set' and 'list'` ✓，
//! **不是** `set.update` 的"任意可迭代"路 ✗）；自身别名（`s ^= s`）安全 ✓。
//! ② `dict` 的 `|=`：`dict`／`[(键, 值)]` 序列／有 `keys()` 的映射三种右侧形态 ✓；错误臂照参照
//! 逐字（`'int' object is not iterable`／`object is not iterable`／
//! `dictionary update sequence element #0 has length 1; 2 is required` ✓），已并入的部分**保留** ✓。
//! ③ **不许多接线**：`bytearray |=`／`list |=` 参照就是 `TypeError` ⇒ 本层照报，不冒充 ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓（与 `sequence_compare_shapes.rs` 同款）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-inplace-{}-{name}", std::process::id()));
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

/// `set` 四则（含 `frozenset` 右侧 ✓、别名可见 ✓、`is` 身份 ✓）＋ 越界形态的报错文案 ✓
/// ＋ `bytearray |=`／`list |=` 照参照报 `TypeError` ✓。
const SET_INPLACE_SCRIPT: &str = r#"s = {1, 2, 3}
alias = s
s |= {4}
print(s, s is alias)
s |= frozenset({5})
print(s, s is alias)
s &= {2, 3, 4, 5}
print(s, s is alias)
s -= {3}
print(s, s is alias)
s ^= {4, 6}
print(s, s is alias)
empty = {1, 2}
same = empty
empty ^= empty
print(empty, empty is same)
try:
    s |= [9]
except TypeError as exc:
    print("TypeError:", exc)
try:
    s &= [9]
except TypeError as exc:
    print("TypeError:", exc)
try:
    s -= {9: 1}
except TypeError as exc:
    print("TypeError:", exc)
print(s)
data = bytearray(b"ab")
try:
    data |= bytearray(b"cd")
except TypeError as exc:
    print("TypeError:", exc)
print(data)
items = [1]
try:
    items |= [2]
except TypeError as exc:
    print("TypeError:", exc)
print(items)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const SET_INPLACE_EXPECTED: &str = "{1, 2, 3, 4} True\n{1, 2, 3, 4, 5} True\n{2, 3, 4, 5} True\n{2, 4, 5} True\n{2, 5, 6} True\nset() True\nTypeError: unsupported operand type(s) for |=: 'set' and 'list'\nTypeError: unsupported operand type(s) for &=: 'set' and 'list'\nTypeError: unsupported operand type(s) for -=: 'set' and 'dict'\n{2, 5, 6}\nTypeError: unsupported operand type(s) for |=: 'bytearray' and 'bytearray'\nbytearray(b'ab')\nTypeError: unsupported operand type(s) for |=: 'list' and 'list'\n[1]\n";

#[test]
fn set_inplace_operations_agree_with_the_reference() {
    let stdout = run("set_inplace_operations_agree", SET_INPLACE_SCRIPT);
    assert_eq!(stdout, SET_INPLACE_EXPECTED, "{stdout}");
}

/// `dict |= ` 三种右侧形态（`dict` ✓／键值对序列 ✓／有 `keys()` 的映射 ✓）＋ 覆盖已有键 ✓
/// ＋ 报错文案与**部分并入保留** ✓。
const DICT_INPLACE_SCRIPT: &str = r#"d = {"a": 1}
alias = d
d |= {"b": 2}
print(d, d is alias)
d |= [("c", 3)]
print(d, d is alias)
d |= [("a", 9)]
print(d, d is alias)


class Mapping:
    def keys(self):
        return ["x"]

    def __getitem__(self, key):
        return 42


d |= Mapping()
print(d, d is alias)
try:
    d |= 5
except TypeError as exc:
    print("TypeError:", exc)
try:
    d |= [("y",)]
except ValueError as exc:
    print("ValueError:", exc)
try:
    d |= [1]
except TypeError as exc:
    print("TypeError:", exc)
print(d)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const DICT_INPLACE_EXPECTED: &str = "{'a': 1, 'b': 2} True\n{'a': 1, 'b': 2, 'c': 3} True\n{'a': 9, 'b': 2, 'c': 3} True\n{'a': 9, 'b': 2, 'c': 3, 'x': 42} True\nTypeError: 'int' object is not iterable\nValueError: dictionary update sequence element #0 has length 1; 2 is required\nTypeError: object is not iterable\n{'a': 9, 'b': 2, 'c': 3, 'x': 42}\n";

#[test]
fn dict_inplace_or_agrees_with_the_reference() {
    let stdout = run("dict_inplace_or_agrees", DICT_INPLACE_SCRIPT);
    assert_eq!(stdout, DICT_INPLACE_EXPECTED, "{stdout}");
}
