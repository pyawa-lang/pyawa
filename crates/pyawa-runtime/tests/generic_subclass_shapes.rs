//! **`Generic.__init_subclass__` ＋ `__orig_bases__`**（task-24 步② 前置 ✓，`__parameters__` 落点 ✓）。
//!
//! 缺口 ✗：`class G(Generic[T])` 先前直接 `Cannot inherit from plain Generic` ✗（我们 `Generic` 是裸类 ✓，
//! 没有 `typing.py` 那条 `__init_subclass__` ✓）。修法：① 类创建凡有基类走了 `__mro_entries__`
//! （`Generic[T]` 那一族 ✓）就把**原始基类**记进 `__orig_bases__` ✓（照参照 ✓）；② core 给 `Generic`
//! 装一条最小 `__init_subclass__` ✓：`__type_params__`（PEP 695 ✓）或 `__orig_bases__` 里的 `__args__`
//! 收进 `__parameters__` ✓，两者都没有 ⇒ 照参照报 `Cannot inherit from plain Generic` ✓。
//!
//! **如实记** ✗：本层 `GenericAlias.__args__` 单参下标存的是**那个参数本身**（参照恒为元组 ✓）⇒ 收参数时
//! 元组就摊开、否则当单个 ✓；`type(G[int])` 本层叫 `GenericAlias`（参照 `typing._GenericAlias` ✓）、
//! `TypeVar` 的 `repr` 是默认对象 repr（参照 `~T` ✓）—— 两处留给轮 3 的 `Lib/typing.py` ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-generic-{}-{name}", std::process::id()));
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

/// `Generic[T]` 打底的类：`__parameters__` ✓／`__mro__` 含 Generic ✓／`G[int]` 可下标 ✓／
/// 裸 `Generic` 基类报 `Cannot inherit from plain Generic` ✓。
const GENERIC_SUBCLASS_SCRIPT: &str = r#"import _typing

T = _typing.TypeVar("T")


class G(_typing.Generic[T]):
    pass


print(len(G.__parameters__), type(G.__parameters__[0]).__name__, G.__parameters__[0].__name__)
print([b.__name__ for b in G.__mro__])
print(G[int].__origin__.__name__ if hasattr(G[int], "__origin__") else "no-origin")
try:
    class Plain(_typing.Generic):
        pass
except TypeError as exc:
    print("TypeError:", exc)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const GENERIC_SUBCLASS_EXPECTED: &str = "1 TypeVar T\n['G', 'Generic', 'object']\nG\nTypeError: Cannot inherit from plain Generic\n";

#[test]
fn generic_subclass_parameters_agree_with_the_reference() {
    let stdout = run("subclass", GENERIC_SUBCLASS_SCRIPT);
    assert_eq!(stdout, GENERIC_SUBCLASS_EXPECTED, "{stdout}");
}
