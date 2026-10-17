//! **`dict.update` 键值对可迭代 ＋ `itertools.chain.from_iterable` ＋ `_imp` 锁**（task-25 ✓）：
//! 两侧逐行一致。
//!
//! 三格都是 round 12 链侦察钉出的**下一格**：
//! ① `dict.update((k, v) 可迭代)`（`multiprocessing/__init__.py:23` 的
//!    `globals().update((name, getattr(...)) for …)`）——**序列路径与 `dict |=` 同一处真相**
//!    （`builtin::dict::dict_update_sequence` ✓）；
//! ② `itertools.chain.from_iterable`（`asyncio/base_events.py:175/1590`）——`chain` 已**升格成类型**
//!    （`type(itertools.chain)` 与参照同类＝`type` ✓）；**惰性**照参照 ✓；
//! ③ `_imp.acquire_lock`／`release_lock`（`test/support` 链：`check_sanitizer` → `sysconfig` →
//!    `importlib._bootstrap._setup(sys, _imp)`）——无参、回 `None`（**如实偏差**：本层无线程 ⇒ 无真锁语义）。
//!
//! 参照值由本机 `python3`（3.14.4）跑**同一段脚本**取得 ✓（逐行一致）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-update-chain-imp-{}-{name}", std::process::id()));
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

const PROBE_SCRIPT: &str = r#"import itertools
import _imp


def show(label, fn):
    try:
        print(label, "=>", repr(fn()))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__ + ":", exc)


print("=== dict.update ===")
d = {}
show("update pairs", lambda: d.update((("a", 1), ("b", 2))))
print("d =", d)
show("update generator", lambda: (lambda e: (e.update((chr(97 + i), i) for i in range(2)), e)[1])({}))
show("update list", lambda: (lambda e: (e.update([("x", 9)]), e)[1])({}))
show("update iter", lambda: (lambda e: (e.update(iter([("i", 3)])), e)[1])({}))
show("update kwargs only", lambda: (lambda e: (e.update(a=1), e)[1])({}))
show("update dict+kwargs", lambda: (lambda e: (e.update({"k": 1}, j=2), e)[1])({}))
show("update()", lambda: (lambda e: (e.update(), e)[1])({}))
show("update([1])", lambda: {}.update([1]))
show("update([(1,2,3)])", lambda: {}.update([(1, 2, 3)]))
show("update('ab')", lambda: {}.update("ab"))
show("update(5)", lambda: {}.update(5))
show("update 2 args", lambda: {}.update({}, {}))
show("update partial keeps", lambda: (lambda e: (show if False else None, e.update([("z", 1), (1, 2, 3)]), e)[2])({}))


class M:
    def keys(self):
        return ["m"]

    def __getitem__(self, k):
        return 42


show("update mapping", lambda: (lambda e: (e.update(M()), e)[1])({}))


class BadKeys:
    def keys(self):
        return 5


show("update bad keys", lambda: {}.update(BadKeys()))


def ior_merge(pairs):
    e = {}
    e |= pairs
    return e


show("|= pairs", lambda: ior_merge((("p", 1),)))
show("|= bad elem", lambda: ior_merge([1]))
show("|= bad len", lambda: ior_merge([(1, 2, 3)]))
print()
print("=== chain ===")
print("type(chain) =", type(itertools.chain).__name__)
show("chain call", lambda: list(itertools.chain([1, 2], (3,), [4, 5])))
show("chain lazy", lambda: list(itertools.islice(itertools.chain(itertools.count(5), [9]), 3)))
show("chain()", lambda: list(itertools.chain()))
show("chain kw", lambda: itertools.chain(x=1))
show("chain(1) then list", lambda: list(itertools.chain(1)))
show("from_iterable", lambda: list(itertools.chain.from_iterable([[1, 2], [3], (4, 5)])))
show("from_iterable gen", lambda: list(itertools.chain.from_iterable((range(i) for i in (1, 2, 3)))))
show("from_iterable lazy", lambda: list(itertools.islice(itertools.chain.from_iterable([itertools.count(7)]), 3)))
show("from_iterable()", lambda: itertools.chain.from_iterable())
show("from_iterable 2", lambda: itertools.chain.from_iterable([1], [2]))
show("from_iterable kw", lambda: list(itertools.chain.from_iterable(iterable=[[1]])))
print()
print("=== _imp locks ===")
print("has:", hasattr(_imp, "acquire_lock"), hasattr(_imp, "release_lock"))
show("acquire", lambda: _imp.acquire_lock())
show("release", lambda: _imp.release_lock())
show("acquire(1)", lambda: _imp.acquire_lock(1))
show("acquire kw", lambda: _imp.acquire_lock(x=1))
show("release(1,2)", lambda: _imp.release_lock(1, 2))
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const PROBE_EXPECTED: &str = "=== dict.update ===\n\
update pairs => None\n\
d = {'a': 1, 'b': 2}\n\
update generator => {'a': 0, 'b': 1}\n\
update list => {'x': 9}\n\
update iter => {'i': 3}\n\
update kwargs only => {'a': 1}\n\
update dict+kwargs => {'k': 1, 'j': 2}\n\
update() => {}\n\
update([1]) !! TypeError: object is not iterable\n\
update([(1,2,3)]) !! ValueError: dictionary update sequence element #0 has length 3; 2 is required\n\
update('ab') !! ValueError: dictionary update sequence element #0 has length 1; 2 is required\n\
update(5) !! TypeError: 'int' object is not iterable\n\
update 2 args !! TypeError: update expected at most 1 argument, got 2\n\
update partial keeps !! ValueError: dictionary update sequence element #1 has length 3; 2 is required\n\
update mapping => {'m': 42}\n\
update bad keys !! TypeError: BadKeys.keys() returned a non-iterable (type int)\n\
|= pairs => {'p': 1}\n\
|= bad elem !! TypeError: object is not iterable\n\
|= bad len !! ValueError: dictionary update sequence element #0 has length 3; 2 is required\n\
\n\
=== chain ===\n\
type(chain) = type\n\
chain call => [1, 2, 3, 4, 5]\n\
chain lazy => [5, 6, 7]\n\
chain() => []\n\
chain kw !! TypeError: chain() takes no keyword arguments\n\
chain(1) then list !! TypeError: 'int' object is not iterable\n\
from_iterable => [1, 2, 3, 4, 5]\n\
from_iterable gen => [0, 0, 1, 0, 1, 2]\n\
from_iterable lazy => [7, 8, 9]\n\
from_iterable() !! TypeError: chain.from_iterable() takes exactly one argument (0 given)\n\
from_iterable 2 !! TypeError: chain.from_iterable() takes exactly one argument (2 given)\n\
from_iterable kw !! TypeError: chain.from_iterable() takes no keyword arguments\n\
\n\
=== _imp locks ===\n\
has: True True\n\
acquire => None\n\
release => None\n\
acquire(1) !! TypeError: _imp.acquire_lock() takes no arguments (1 given)\n\
acquire kw !! TypeError: _imp.acquire_lock() takes no keyword arguments\n\
release(1,2) !! TypeError: _imp.release_lock() takes no arguments (2 given)\n";

#[test]
fn update_chain_and_imp_locks_agree_with_the_reference() {
    let stdout = run("update_chain_imp", PROBE_SCRIPT);
    assert_eq!(stdout, PROBE_EXPECTED, "{stdout}");
}
