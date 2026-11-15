//! **`dict` 的哈希索引 ＋ 迭代中改键的 `RuntimeError`**（`P3-41` round 42）。
//!
//! 背景：`test_dict` 的 `test_copy_fuzz`（10 万项建表 ＋ `d == d2`）与 `test_weakref` 都卡在
//! **O(n²)** —— `DictObject` 只有"条目 Vec ＋ 线性扫"。本轮给它加**惰性查找索引**
//! （键哈希 → 条目下标；`insert_hashed` 增量维护，`insert_raw`／`remove` 作废，改值不动），
//! 键哈希 `values::key_hash` 与 `values_equal` **同源**（`values_equal(a,b) ⇒ 哈希相同`）：
//! `None`／数值（`1 == True == 1.0` 同哈希）／`str`／`bytes`／`tuple`（递归）／`weakref`（按目标）／
//! 其余按身份；`list`／`dict`／`set`／`bytearray` 不可哈希 ⇒ `None`（线性兜底）。
//! 接进 `dict_set`／`dict_get`／`dict_set_int`／下标读·写·删／`dict` 相等比较。
//!
//! 另：`dict_keyiterator` 加**键集版本守卫** —— 迭代中改键 ⇒ `RuntimeError`，文案两档照参照
//! （长度变了 ⇒ `changed size`；只变键集 ⇒ `changed keys`）；先前**不检测** ⇒
//! `for i in d: d[i+1] = 1` 永不结束（`test_dict.test_mutating_iteration`）。
//!
//! 效果：10 万项建表 **>180 s ⇒ 0.275 s**、`d == d2` **⇒ 0.023 s**、`test_dict` 整文件
//! **超时 ⇒ 3.59 s**（剩 57 failures／120 errors 是普通语义缺口）；官方套件**超时 2⇒1**。
//!
//! 两段脚本的期望值都由本机 `python3`（3.14.4）取（`MS-17`），逐字节一致。
//!
//! **如实未接** ✗：`d.values()`／`d.items()` 在本层返回**列表快照**（不是视图对象）⇒ 在它们
//! 上面"迭代中改字典"不会报 `RuntimeError`（`test_weakref` 的 len_race 慢另有原因：
//! 见 round 42 台账的 GC 诊断 —— GC 在 weakref/线程上下文里 freed≈0、live 涨到 66 万）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-dict-{}-{}", std::process::id(), name));
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

/// 建表／查找／删除／数值键合并／元组键（10 万项那档也要跑得动）。
const DICT_HASH_SCRIPT: &str = r#"import sys

d = {}
for i in range(10000):
    d[i] = i
print("len", len(d), d[9999], 12345 in d)

d2 = d.copy()
print("eq", d == d2, d == {**d})
d2["key"] = "value"
print("neq", d != d2, len(d2))

print("str-keys", {"a": 1, "b": 2}, d.get("x", "none"))

# 混合数值键：True/1、1/1.0 合并
m = {}
m[1] = "int"
m[True] = "bool"
m[1.0] = "float"
print("numeric", m)

# 元组键
t = {(1, 2): "a", (1, 3): "b"}
print("tuple", t[(1, 2)], t[(1, 3)])

# 删除与再插入
del d[5000]
print("after-del", 5000 in d, len(d))
d[5000] = 5000
print("reinsert", d[5000])
"#;

const DICT_HASH_EXPECTED: &str = "len 10000 9999 False
eq True True
neq True 10001
str-keys {'a': 1, 'b': 2} none
numeric {1: 'float'}
tuple a b
after-del False 9999
reinsert 5000
";

/// 迭代中改键的两档 `RuntimeError` ＋ 只改值不报错 ＋ 10 万项建表。
const DICT_GUARD_SCRIPT: &str = r#"d = {}
d[1] = 1
try:
    for i in d:
        d[i + 1] = 1
except RuntimeError as error:
    print("add", error)

d = {0: 0}
try:
    for i in d:
        del d[0]
        d[0] = 0
except RuntimeError as error:
    print("del", error)

d = {1: 1, 2: 2}
for i in d:
    d[i] = d[i] * 10
print("update-values", d)

big = {}
for i in range(100000):
    big[i] = i
print("big", len(big), big[99999])

# 哈希一致性：数值键合并 / 元组键 / 字符串键
m = {}
m[1] = "int"
m[True] = "bool"
m[1.0] = "float"
print("numeric", m)
t = {(1, 2): "a", (1, 3): "b"}
print("tuple", t[(1, 2)], t[(1, 3)])
print("str", {"a": 1}.get("a"), {"a": 1}.get("b", "none"))
"#;

const DICT_GUARD_EXPECTED: &str = "add dictionary changed size during iteration
del dictionary keys changed during iteration
update-values {1: 10, 2: 20}
big 100000 99999
numeric {1: 'float'}
tuple a b
str 1 none
";

#[test]
fn dict_hashing_and_iteration_guard_match_the_reference() {
    assert_eq!(run("hash", DICT_HASH_SCRIPT), DICT_HASH_EXPECTED);
    assert_eq!(run("guard", DICT_GUARD_SCRIPT), DICT_GUARD_EXPECTED);
}
