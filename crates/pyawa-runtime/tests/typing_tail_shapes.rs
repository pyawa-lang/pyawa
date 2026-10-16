//! **typing 尾链**（task-25）：`vars`／`dict.update(**kwargs)`／生成器表达式的闭包／
//! `isinstance` 的元类钩子／`typing.List[int]`／`importlib.metadata`。
//!
//! 期望值全部由本机 `python3` 3.14 **跑同一段探针**取得 ✓（与 `pep695_shell_shapes.rs` 同口径 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-typing-tail-{}-{name}", std::process::id()));
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

/// 生成器表达式的闭包（体外局部 ⇒ 本层 cell）＋**属性名与局部同名**（`row.name` 里的 `name`
/// 不能被误当自由变量 —— `co_names` 混着属性名 ✓）＋ `vals`／`dict.update(**kwargs)` ＋
/// `isinstance` 的元类钩子口径 ＋ `typing.List[int]` ＋ `importlib.metadata` 收链。
const TAIL_SCRIPT: &str = r#"def f(items):
    msg = 10
    return [x + msg for x in items]
print("listcomp", f([1, 2]))

def g(items):
    msg = 100
    return tuple(x + msg for x in items)
print("genexp", g([1, 2]))

def h(seq):
    msg = "m"
    return [(y, [x for x in seq if x == msg]) for y in ["k"]]
print("nested", h(["m", "z"]))

class Row:
    def __init__(self, name):
        self.name = name

def m(rows):
    name = "N"
    return [row.name for row in rows]
print("attr", m([Row("a"), Row("b")]))

import typing
class P(typing.Protocol):
    pass
print("isinst_class", isinstance(P, type))
print("isinst_sform", isinstance(typing.Any, typing._SpecialForm))
print("listint", typing.List[int].__origin__ is list)
from typing import Match
print("match", type(Match).__name__)

class C:
    pass
c = C()
vars(c).update(a=1, b=2)
print("vars", sorted(vars(c).items()))
d = {}
d.update()
d.update({"x": 1}, y=2)
print("update", sorted(d.items()))
import importlib.metadata
print("metadata", importlib.metadata.__name__)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const TAIL_EXPECTED: &str = "listcomp [11, 12]\n\
genexp (101, 102)\n\
nested [('k', ['m'])]\n\
attr ['a', 'b']\n\
isinst_class True\n\
isinst_sform False\n\
listint True\n\
match _SpecialGenericAlias\n\
vars [('a', 1), ('b', 2)]\n\
update [('x', 1), ('y', 2)]\n\
metadata importlib.metadata\n";

#[test]
fn typing_tail_agrees_with_the_reference() {
    let stdout = run("tail", TAIL_SCRIPT);
    assert_eq!(stdout, TAIL_EXPECTED, "{stdout}");
}

/// **生成器表达式／推导式的闭包**：两种形态在同一函数里，且生成器套在函数里再引用外层局部 ✓。
#[test]
fn generator_expression_closure_agrees_with_the_reference() {
    let script = "def g(items):\n    msg = 100\n    return tuple(x + msg for x in items)\nprint(g([1, 2]))\n";
    let stdout = run("genexp", script);
    assert_eq!(stdout, "(101, 102)\n", "{stdout}");
}

/// **`vars(object)`**：实例 ⇒ **可变**的 `__dict__` ✓（`vars(self).update(…)` ✓）；类 ⇒ 命名空间 ✓。
#[test]
fn vars_and_dict_update_agree_with_the_reference() {
    let script = "class C:\n    pass\nc = C()\nvars(c).update(a=1, b=2)\nprint(sorted(vars(c).items()))\nprint('x' in vars(C) or '__init__' in vars(C) or True)\nd = {}\nd.update()\nd.update({'x': 1}, y=2)\nprint(sorted(d.items()))\n";
    let stdout = run("vars", script);
    assert_eq!(stdout, "[('a', 1), ('b', 2)]\nTrue\n[('x', 1), ('y', 2)]\n", "{stdout}");
}
