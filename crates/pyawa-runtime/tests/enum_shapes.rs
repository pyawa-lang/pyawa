//! **`enum`（上游真本）在 `Lib/` 里的行为面** —— 本护栏只钉 `re`／`http` 依赖的那几件事 ✓。
//!
//! 历史：第 601 轮起 `Lib/enum.py` 是**假货**（只为把 `re` 顶起来 ✓）；第 411 轮（task-22 B6）
//! **整文件换回上游逐字节真本**（CX-8 偏差归零 ✓）⇒ 本护栏按**参照（python3 3.14）实测**重钉 ✓：
//! `_simple_enum` 造类 ✓、成员是 `IntFlag` **实例** ✓（`isinstance(F.A, F)` **True** ✓）、
//! `KEEP`／`IntFlag` 名字在 ✓、MRO 含 `ReprEnum` ✓、`object.__str__` 经 `__repr__` 覆写给成员 repr ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-enum-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
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

const SCRIPT: &str = r#"
import enum

@enum._simple_enum(enum.IntFlag, boundary=enum.KEEP)
class F:
    NOFLAG = 0
    A = 2
    B = 4
    __str__ = object.__str__
    _numeric_repr_ = hex

print(F.__name__, [base.__name__ for base in F.__mro__])
print(F.A, F.B, F.A | F.B, F.B & F.A, isinstance(F.A, int), isinstance(F.A, F))
print(enum.KEEP, enum.IntFlag.__name__, enum.Enum.__name__)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：真本下 MRO 含 `ReprEnum` ✓、成员是 **IntFlag 实例**
/// （`isinstance(F.A, F)` **True** ✓）、成员 `str()` 走类体里那句 `__str__ = object.__str__`
/// ⇒ 给**成员 repr** `<F.A: 2>` ✓；`enum.KEEP` 是 `StrEnum` 子类成员 ⇒ 打印值 `keep` ✓。
const EXPECTED: &str = "F ['F', 'IntFlag', 'int', 'ReprEnum', 'Flag', 'Enum', 'object']\n<F.A: 2> <F.B: 4> <F.A|B: 6> <F.NOFLAG: 0> True True\nkeep IntFlag Enum\n";

#[test]
fn enum_surface_used_by_re() {
    let stdout = run("surface", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}

/// **`StrEnum` 导入面 ＋ 继承关系** ✓（第 700 轮起；第 411 轮换真本后按参照重钉 ✓）：
/// 整包判定里 `http` 曾卡在 `ImportError: cannot import name 'StrEnum' from 'enum'` ✗ ⇒ 这条把
/// **导入面 ＋ 继承关系**钉住 ✓（只钉参照下成立的部分 ✓；类的 repr 形不钉 ✓）。
const STRENUM_SCRIPT: &str = r#"
from enum import StrEnum, IntEnum, Enum, Flag, IntFlag, auto

print(StrEnum.__name__, IntEnum.__name__, Enum.__name__, Flag.__name__, IntFlag.__name__, auto.__name__)
print(issubclass(StrEnum, str), issubclass(IntEnum, int), issubclass(IntFlag, int))
"#;

/// 参照（`python3` 3.14 实测 ✓）：与假货在这两条上一致 ✓。
const STRENUM_EXPECTED: &str =
    "StrEnum IntEnum Enum Flag IntFlag auto\nTrue True True\n";

#[test]
fn enum_has_strenum_for_http() {
    let stdout = run("strenum", STRENUM_SCRIPT);
    assert_eq!(stdout, STRENUM_EXPECTED, "{stdout}");
}
