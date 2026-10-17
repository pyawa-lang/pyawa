//! **`type.__annotations__` 描述符 ＋ `annotationlib`**（task-20 第 1 轮 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓（脚本见各 `const` ✓）。
//!
//! 打通的东西：
//! ① **`type.__dict__["__annotations__"]`** ✗ 先前 `KeyError`（`type` 类型字典里没有这个 getset
//!    描述符 ✓）⇒ `annotationlib:1141` 的 `type.__dict__["__annotations__"].__get__` 直接炸 ✓；
//! ② **3.14 口径** ✓：自有 `__annotations__` 就给 ✓、**没有就建空 `dict` 并缓存** ✓（类对象写命名空间 ✓、
//!    模块对象写实例字典 ✓ —— 两条路都实测 ✓）；
//! ③ **`operator.attrgetter`** ✗ 先前只有 `itemgetter` ⇒ `inspect` 的
//!    `from operator import attrgetter` 当场 `ImportError` ✓（点分路径 `a.b` 也认 ✓）。
//!
//! **如实范围** ✗：类体的**注解声明**（`x: int`）我方编译器还不落 `__annotations__`／`__annotate__` ✓
//! —— 故本组抽例只钉**描述符与建空缓存**那半边 ✓；`import inspect` 仍差另行立支的两格
//! （`%` 映射取键 `KeyError: __mro__` ✓ 与一处 `str + tuple` ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-annotations-{}-{name}", std::process::id()));
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

/// `__annotations__` 两条路 ＋ `__get__` ＋ `get_annotations` ＋ `attrgetter` ✓
const ANNOTATIONS_SCRIPT: &str = r#"import annotationlib
from operator import attrgetter


class C:
    pass


print("own", C.__annotations__)
print("same", C.__annotations__ is C.__annotations__)
C.__annotations__["x"] = int
print("filled", C.__annotations__)

d = type.__dict__["__annotations__"]
print("desc", type(d).__name__)
print("get", d.__get__(C))

m = annotationlib.get_annotations(C)
print("annlib", type(m).__name__, m)

import sys
print("module", type(sys.modules["sys"].__annotations__).__name__)
print("modget", annotationlib.get_annotations(annotationlib))


class P:
    def __init__(self):
        self.a = 1
        self.b = "x"

    @property
    def c(self):
        return self.a + 1


print("g1", attrgetter("a")(P()))
print("g2", attrgetter("a", "b")(P()))
print("g3", attrgetter("c")(P()))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const ANNOTATIONS_EXPECTED: &str = "own {}\nsame True\nfilled {'x': <class 'int'>}\ndesc getset_descriptor\nget {'x': <class 'int'>}\nannlib dict {'x': <class 'int'>}\nmodule dict\nmodget {}\ng1 1\ng2 (1, 'x')\ng3 2\n";

#[test]
fn annotations_descriptor_and_attrgetter_agree_with_the_reference() {
    let stdout = run("annotations_descriptor_and_attrgetter", ANNOTATIONS_SCRIPT);
    assert_eq!(stdout, ANNOTATIONS_EXPECTED, "{stdout}");
}

/// `import annotationlib` ＋ `operator.attrgetter` 名字到位 ✓
const IMPORT_SCRIPT: &str = r#"import annotationlib
import operator


class P:
    a = 5


print(annotationlib.Format.__name__)
print(operator.attrgetter("a")(P()))
print(annotationlib.get_annotations(P))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const IMPORT_EXPECTED: &str = "Format\n5\n{}\n";

#[test]
fn annotationlib_and_operator_import_agree_with_the_reference() {
    let stdout = run("annotationlib_and_operator_import", IMPORT_SCRIPT);
    assert_eq!(stdout, IMPORT_EXPECTED, "{stdout}");
}

