//! **内建容器子类的布局／`__new__`／迭代／描述符**（task-14 ✓）—— 与参照逐例对 ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段脚本**取得 ✓（脚本见各 `const` ✓）。
//!
//! 被这组抽例钉住的**真 bug**（都在 `tuple` 子类这条路上 ✗）：
//! ① `tuple.__dict__["__new__"]` 被 `builtins` 里那份 **`object.__new__` 占位**盖掉 ✗
//!    （`enum.py` 的 `'__new__' in base.__dict__` 兼容做法 ✓）⇒ `tuple.__new__(cls, it)` 落到
//!    `object.__new__` ✓（`namedtuple` 的 `_tuple_new(cls, …)` 正压在这格 ✓）；
//! ② `type(...)`／类体造类时 `requalified_method` **换 qualname** 会**丢闭包 cell** ✗
//!    ⇒ `namedtuple` 生成的 `__repr__`／`_asdict` 报 `cannot access free variable` ✓；
//! ③ `%` 格式化的右操作数只认**恰好**是 tuple／dict ✗ ⇒ tuple **子类**不拆包 ✓
//!    ⇒ `'%r' % self` 里 `%r` 去 `repr(self)` ✓ ⇒ `__repr__` **自递归** ✓；
//! ④ 迭代（`iterator_type_for`／`iterable_length`／`iterable_item`）与**相等**（`values_equal`）
//!    都按**恰好**是内建类型判 ✗ ⇒ 子类 `list(Point(1,2))`／`Point(1,2) == (1,2)` 全废 ✓；
//! ⑤ 内建族实例上的**数据描述符**（`property` ✓）被"自有字典"那条快路原样交回 ✗
//!    ⇒ `Point(1,2).x` 给 `<property object>` ✓；`property.__get__` 又把 `fget` 当 `bound_self` ✗
//!    ⇒ 已绑定的 `fget`（`namedtuple` 回退用 `property(_itemgetter(index))` ✓）收不到实参 ✓。
//!
//! **如实** ✗：`type(namedtuple 字段).__name__` 参照是 `_tuplegetter` ✓，我们是 `property` ✓
//! （没有 `_collections._tuplegetter` ✓，走 `collections` 的**回退实现** ✓）—— 值语义一致 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-tuple-{}-{name}", std::process::id()));
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

const LAYOUT_SCRIPT: &str = r#"class P(tuple):
    pass

p = P((1, 2))
print(type(p).__name__, isinstance(p, P), isinstance(p, tuple), p)
q = tuple.__new__(P, (3, 4))
print(type(q).__name__, q)
print(tuple.__new__ is object.__new__)
print(tuple.__new__(tuple, (5, 6)))

class R(tuple):
    y = property(lambda self: 99)

print(R((1,)).y)

from collections import namedtuple
Point = namedtuple("Point", "x y")
pt = Point(1, 2)
print(pt.x, pt.y, pt[0], pt[1], len(pt))
print(repr(pt))
print(list(pt))
print(pt == (1, 2))

def outer():
    fmt = "%r"
    def inner(self):
        return fmt
    ns = {"inner": inner}
    return type("K", (), ns)

K = outer()
print(K().inner())

class L(list):
    pass

print(type(L([1, 2])).__name__, list(L([1, 2])), len(L([1, 2])))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `LAYOUT_SCRIPT` ✓。
const LAYOUT_EXPECTED: &str = "P True True (1, 2)\nP (3, 4)\nFalse\n(5, 6)\n99\n1 2 1 2 2\nPoint(x=1, y=2)\n[1, 2]\nTrue\n%r\nL [1, 2] 2\n";

#[test]
fn tuple_subclass_layout_new_iteration_and_descriptors_agree_with_the_reference() {
    let stdout = run("layout", LAYOUT_SCRIPT);
    assert_eq!(stdout, LAYOUT_EXPECTED, "{stdout}");
}

// ==== `namedtuple` 造的 `CodecInfo`（task-14 的收益面 ✓）============================================

const CODEC_SCRIPT: &str = r#"import codecs
info = codecs.lookup("cp437")
print(type(info).__name__, info.name)
print(bytes([0x82]).decode("cp437"))
import encodings.cp437
print("cp437 module ok")
"#;

/// 参照（`python3` 3.14 实测 ✓）：`codecs.lookup('cp437')` 回**真 `CodecInfo`** ✓（`namedtuple` 产物 ✓）。
const CODEC_EXPECTED: &str = "CodecInfo cp437\né\ncp437 module ok\n";

#[test]
fn codec_info_built_by_namedtuple_agrees_with_the_reference() {
    let stdout = run("codec", CODEC_SCRIPT);
    assert_eq!(stdout, CODEC_EXPECTED, "{stdout}");
}
