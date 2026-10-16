//! **类体注解声明 ⇒ `__annotate__`（PEP 649 惰性注解）**（task-21 第 1 轮 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。
//!
//! 落地的口径（照 `dis` 实测 ✓）：
//! - 类体里 `x: int`／`x: int = 1` **只发赋值那半边** ✓（注解那半边不发指令 ✓）；
//! - 类体收尾先发 `LOAD_CONST <__annotate__ 单元>; MAKE_FUNCTION; STORE_NAME __annotate_func__` ✓
//!   （排在 `__static_attributes__` **之前** ✓），`__static_attributes__` 与 `__classdictcell__` 次序不动 ✓；
//! - 类对象上 `C.__annotate__` 由运行期从 `__annotate_func__` 给出 ✓（`C.__dict__` 里留的名字**不变** ✓，
//!   与参照一致 ✓）；`C.__annotations__` 走 `__annotate__(1)`（`Format.VALUE` ✓）**不缓存** ✓；
//! - **嵌套块里的注解也收** ✓（`if True: w: float` ✓ —— 参照同 ✓）；
//! - 没有注解的类 ⇒ `__annotations__` 给新的空 `dict` ✓、**不缓存** ✓（3.14 实测 ✓）；
//!   模块对象仍是**建并缓存** ✓（3.14 实测模块缓存 ✓）。
//!
//! **如实范围** ✗：注解表达式按**函数作用域**求值（名字走 `LOAD_GLOBAL` ✓）—— 参照另有
//! `LOAD_DEREF __classdict__; LOAD_FROM_DICT_OR_GLOBALS`（类字典优先 ✓）；**模块级**注解声明这一轮
//! **还没发** `__annotate__`（只发赋值 ✓）；`@dataclass` **带字段**那条路仍卡在下一格 ✗
//! （`_process_class` 里 `'bool' object has no attribute '_field_type'` ✓）。

const ANNOTATIONS_SCRIPT: &str = r#"class C:
    x: int
    y: 'str'
    z: int = 5

    if True:
        w: float


print(sorted(C.__annotations__))
print(C.__annotations__ == {"x": int, "y": "str", "z": int, "w": float})
print(C.__annotate__(1) == {"x": int, "y": "str", "z": int, "w": float})
print(callable(C.__annotate__))
print(C.z)


class D:
    pass


print(D.__annotations__, D.__dict__.get("__annotate_func__"))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const ANNOTATIONS_EXPECTED: &str = "['w', 'x', 'y', 'z']\nTrue\nTrue\nTrue\n5\n{} None\n";

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-classann-{}-{name}", std::process::id()));
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

#[test]
fn class_body_annotations_reach_annotations_with_the_reference() {
    let stdout = run("class_body", ANNOTATIONS_SCRIPT);
    assert_eq!(stdout, ANNOTATIONS_EXPECTED, "{stdout}");
}
