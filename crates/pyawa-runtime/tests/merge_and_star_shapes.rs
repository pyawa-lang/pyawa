//! **`**映射`／`*可迭代` 与关键字实参的协议面**（round 50，`DICT_MERGE`／`CALL_FUNCTION_EX`）。
//!
//! 落地：
//! * `DICT_MERGE`／`DICT_UPDATE` 收**任意映射**（`keys()` ＋ `__getitem__`），不再只认 `dict`；
//! * `DICT_MERGE` 的同名键 ⇒ 参照原文（带被调对象限定名：
//!   `__main__.f() got multiple values for keyword argument 'a'`；可调用实例用其 `repr`）；
//! * `DICT_MERGE` 的源不是映射 ⇒ `... argument after ** must be a mapping, not int`；
//! * `CALL_FUNCTION_EX` 的 `*实参` 收**任意可迭代**（自定义 `__iter__`），不可迭代 ⇒
//!   `... argument after * must be an iterable, not int`（"显式位置实参在前 ⇒ `Value after *`"
//!   那一条还没对齐，见台账 ✓）；
//! * 关键字名不是 `str` ⇒ `TypeError: keywords must be strings`（`str` 子类照收 ✓）。
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

const SCRIPT: &str = r#"class Mapping:
    def __init__(self, data):
        self.data = data
    def keys(self):
        return list(self.data.keys())
    def __getitem__(self, key):
        return self.data[key]

class BadMapping:
    def keys(self):
        return [1]
    def __getitem__(self, key):
        return 2

class Iterable:
    def __init__(self, items):
        self.items = items
    def __iter__(self):
        return iter(self.items)

class S(str):
    pass

def f(a=None, *args, **kwargs):
    return a, args, kwargs

def g(*args, **kwargs):
    return args, kwargs

def outer():
    def inner(a=None, **kwargs):
        return a, kwargs
    return inner

class C:
    def m(self, a=None, **kwargs):
        return a, kwargs

def show(label, fn):
    try:
        print(label, "OK", fn())
    except Exception as error:
        print(label, type(error).__name__, error)

show("merge-dict", lambda: f(**{"x": 1}, **{"y": 2}))
show("merge-mapping", lambda: f(b=3, **Mapping({"x": 1})))
show("merge-dup", lambda: f(a=1, **{"a": 2}))
show("merge-dup-inner", lambda: outer()(a=1, **{"a": 2}))
show("merge-dup-method", lambda: C().m(a=1, **{"a": 2}))
show("not-mapping", lambda: f(**5))
show("list-not-mapping", lambda: f(**[]))
show("bad-key-mapping", lambda: f(**BadMapping()))
show("star-iterable", lambda: f(0, *Iterable([1, 2])))
show("star-str", lambda: f(0, *"ab"))
show("star-not-iterable", lambda: g(*7))
show("str-subclass-keyword", lambda: g(**{S("a"): 1}))
show("dict-display-mapping", lambda: {**Mapping({"x": 1}), "y": 2})
show("update-dup", lambda: {**{"a": 1}, "a": 2})
show("method-call-mapping", lambda: print(*[1, 2], **Mapping({"sep": "-"})))"#;

const EXPECTED: &str = "merge-dict OK (None, (), {'x': 1, 'y': 2})\nmerge-mapping OK (None, (), {'b': 3, 'x': 1})\nmerge-dup TypeError __main__.f() got multiple values for keyword argument 'a'\nmerge-dup-inner TypeError __main__.outer.<locals>.inner() got multiple values for keyword argument 'a'\nmerge-dup-method TypeError __main__.C.m() got multiple values for keyword argument 'a'\nnot-mapping TypeError __main__.f() argument after ** must be a mapping, not int\nlist-not-mapping TypeError __main__.f() argument after ** must be a mapping, not list\nbad-key-mapping TypeError keywords must be strings\nstar-iterable OK (0, (1, 2), {})\nstar-str OK (0, ('a', 'b'), {})\nstar-not-iterable TypeError __main__.g() argument after * must be an iterable, not int\nstr-subclass-keyword OK ((), {'a': 1})\ndict-display-mapping OK {'x': 1, 'y': 2}\nupdate-dup OK {'a': 2}\n1-2\nmethod-call-mapping OK None\n";

#[test]
fn merge_and_star_argument_protocols_match_the_reference() {
    assert_eq!(run("merge", SCRIPT), EXPECTED);
}
