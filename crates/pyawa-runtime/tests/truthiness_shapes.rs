//! **真值协议（`__bool__`／`__len__`）与类型对象恒真**（round 46，`P3-42` 语义缺口）。
//!
//! 症状：`test_bool.test_convert_to_bool` 处 **Rust 栈溢出 abort**（`fatal runtime error: stack
//! overflow`）—— `__bool__` 返回 `self` 时，旧实现把返回值**再取一次真值** ⇒ 无限递归 ✗。
//! 修法（照参照口径）：
//! * `__bool__` 的返回值**必须恰是 `bool`**，否则 `TypeError: __bool__ should return bool,
//!   returned <类型名>`；
//! * `__len__` 的返回值：整数且 `>= 0`（负数 ⇒ `ValueError: __len__() should return >= 0`；
//!   非整数 ⇒ `TypeError: '<类型>' object cannot be interpreted as an integer`；超 `i64` ⇒
//!   `OverflowError: cannot fit 'int' into an index-sized integer`）；
//! * **类型对象恒为真**（`bool(complex)`／`bool(float)` 先前按实例协议去取原生 `__bool__`、
//!   没有 self ⇒ `Unsupported` ✗）。
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

const TRUTH_SCRIPT: &str = r#"class SelfBool:
    def __bool__(self):
        return self

class StrBool:
    def __bool__(self):
        return "Yes"

class IntBool:
    def __bool__(self):
        return 1

class NegLen:
    def __len__(self):
        return -1

class BadLen:
    def __len__(self):
        return "x"

class HugeLen:
    def __len__(self):
        return 1 << 100

class ZeroLen:
    def __len__(self):
        return 0

class PosLen:
    def __len__(self):
        return 3

for label, value in [
    ("selfbool", SelfBool()),
    ("strbool", StrBool()),
    ("intbool", IntBool()),
    ("neglen", NegLen()),
    ("badlen", BadLen()),
    ("hugelen", HugeLen()),
    ("zerolen", ZeroLen()),
    ("poslen", PosLen()),
]:
    try:
        print(label, "->", bool(value))
    except Exception as error:
        print(label, "->", type(error).__name__, error)

print("types", [bool(t) for t in (bool, complex, dict, float, int, list, object, set, str, tuple, type)])
print("containers", bool(""), bool("x"), bool([]), bool([0]), bool({}), bool({1: 2}), bool(()), bool((1,)), bool(0), bool(1), bool(-0.0), bool(0.5), bool(complex(0, 0)))
print("not", not 0, not 1, not [], not [0])
print("is-true", bool(1) is True, bool(0) is False)"#;

const TRUTH_EXPECTED: &str = "selfbool -> TypeError __bool__ should return bool, returned SelfBool\nstrbool -> TypeError __bool__ should return bool, returned str\nintbool -> TypeError __bool__ should return bool, returned int\nneglen -> ValueError __len__() should return >= 0\nbadlen -> TypeError 'str' object cannot be interpreted as an integer\nhugelen -> OverflowError cannot fit 'int' into an index-sized integer\nzerolen -> False\nposlen -> True\ntypes [True, True, True, True, True, True, True, True, True, True, True]\ncontainers False True False True False True False True False True False True False\nnot True False True False\nis-true True True\n";

#[test]
fn truthiness_protocol_matches_the_reference() {
    assert_eq!(run("truth", TRUTH_SCRIPT), TRUTH_EXPECTED);
}
