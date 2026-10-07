//! **假货 `enum`** 的行为面（第 601 轮 ✓）——只为把 `re` 顶起来 ✓，**将来整文件换回上游** ✓。
//!
//! 换回流程写在 `Lib/enum.py` 文件头 ✓（三步）✓。本护栏**只钉 `re` 依赖的那几件事** ✓：
//! `_simple_enum` 造类 ✓、成员**就是普通 int** ✓、`KEEP`／`IntFlag` 名字在 ✓、类的 MRO 尾巴是 `IntFlag` ✓。
//! **不钉**假货的偏离（没有 `.name`／`.value` ✗、成员不是实例 ✗、`Flag` 边界差异 ✗）——那些是**已知偏离** ✓，
//! 换回真货后本护栏**照样该绿** ✓（参照下这几条也成立 ✓）。

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

/// 参照（`python3` 3.14 实测 ✓）：**假货与真货在这三条上应当一致** ✓ —— 唯一的差别是
/// 真货的 `isinstance(F.A, F)` 为 **True** ✓ 而假货为 **False** ✗（成员是普通 int ✓，见 `Lib/enum.py` 文件头 ✓）。
/// 故本护栏**按假货钉** ✓，并在换回真货时**同笔改这一条** ✓（这正是"可换回"要付的那点对账 ✓）。
const EXPECTED: &str = "F ['F', 'IntFlag', 'int', 'Flag', 'Enum', 'object']\n2 4 6 0 True False\nKEEP IntFlag Enum\n";

#[test]
fn fake_enum_surface_used_by_re() {
    let stdout = run("surface", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}

/// **`StrEnum` 一跳** ✓（第 700 轮 ✓）：整包判定里 `http` 卡在
/// `ImportError: cannot import name 'StrEnum' from 'enum'` ✗ ⇒ 这条把**导入面 ＋ 继承关系**钉住 ✓
/// （只钉**真假两侧一致**的部分 ✓ —— 假货的 repr 是 `<class 'StrEnum'>` ✗、参照是 `<enum 'StrEnum'>` ✓，
/// 这条偏离**不钉** ✓，换回上游后本护栏照样该绿 ✓）。
const STRENUM_SCRIPT: &str = r#"
from enum import StrEnum, IntEnum, Enum, Flag, IntFlag, auto

print(StrEnum.__name__, IntEnum.__name__, Enum.__name__, Flag.__name__, IntFlag.__name__, auto.__name__)
print(issubclass(StrEnum, str), issubclass(IntEnum, int), issubclass(IntFlag, int))
"#;

/// 参照（`python3` 3.14 实测 ✓）：与假货在这两条上一致 ✓。
const STRENUM_EXPECTED: &str =
    "StrEnum IntEnum Enum Flag IntFlag auto\nTrue True True\n";

#[test]
fn fake_enum_has_strenum_for_http() {
    let stdout = run("strenum", STRENUM_SCRIPT);
    assert_eq!(stdout, STRENUM_EXPECTED, "{stdout}");
}
