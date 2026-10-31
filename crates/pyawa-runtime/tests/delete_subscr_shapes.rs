//! **`DELETE_SUBSCR` 的下标删除面**（`P3-35` 第 5 轮 / round 36）：`del` 协议补齐。
//!
//! 落地：
//! * `__delitem__` 协议（用户类／容器子类）——参照里 `del obj[k]` 一律先问**类型上**的 `__delitem__`；
//! * `bytearray` 的整数下标／切片删除（含带步长；从后往前删，不会串位）；
//! * 内建不可删类型（`str`／`tuple`／`bytes`…）的文案照参照：`'X' object doesn't support item deletion`。
//!
//! **如实未接** ✗：`deque`（与它的下标面一起，属后续轮）；`\N{…}`／`except*`／`import *`／
//! async 推导式／PEP 695 仍未接线（见 round 36 台账）。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-p335-del-{}-{name}", std::process::id()));
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

/// 下标删除的四条路（dict 子类／自定义 `__delitem__`／bytearray 三形态／嵌套 list／str 的 TypeError）。
const DELETE_SUBSCR_SCRIPT: &str = r#"class MyDict(dict):
    pass

d = MyDict(a=1, b=2)
del d["a"]
print("dict-subclass", d)

class MyDel:
    def __init__(self):
        self.store = {"x": 1}
    def __delitem__(self, key):
        del self.store[key]

m = MyDel()
del m["x"]
print("dunder", m.store)

ba = bytearray(b"abcdef")
del ba[0]
print("ba-item", ba)
del ba[1:3]
print("ba-slice", ba)
del ba[::2]
print("ba-step", ba)

lst = [[1, 2, 3], [4, 5]]
del lst[0][1]
print("nested", lst)
try:
    del "abc"[0]
except TypeError as e:
    print("str", "|" + str(e) + "|")"#;

/// 参照输出（逐行）。
const DELETE_SUBSCR_EXPECTED: &str = "dict-subclass {'b': 2}\ndunder {}\nba-item bytearray(b'bcdef')\nba-slice bytearray(b'bef')\nba-step bytearray(b'e')\nnested [[1, 3], [4, 5]]\nstr |'str' object doesn't support item deletion|\n";

#[test]
fn delete_subscript_matches_the_reference() {
    assert_eq!(run("del", DELETE_SUBSCR_SCRIPT), DELETE_SUBSCR_EXPECTED);
}
