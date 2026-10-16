//! **槽 ≥ 16 的推导式不融合 `STORE_FAST_LOAD_FAST`**（task-21 第 2 轮真 bug 修 ✗）。
//!
//! 真 bug ✗：推导式把「存目标」与「紧随其后的那次读」打成超指令
//! `STORE_FAST_LOAD_FAST` —— oparg ＝ `(存槽 << 4) | 取槽` ✓ ⇒ **槽 ≥ 16 时装不下** ✗
//! 而代码里是 `((slot << 4) | next_slot) as u8` ✓ ⇒ **截断** ✓。实测：`dataclasses._process_class`
//! 那种**十九个局部**的函数里，推导式目标落在槽 19 ⇒ oparg 51 ＝ 槽 (3,3) ✗ ⇒ 存错槽、取到
//! `repr`（一个 bool ✓）⇒ `'bool' object has no attribute '_field_type'` ✓（`@dataclass` 整条链卡这 ✗）。
//! 参照在槽 ≥ 16 时**不融合**（发 `STORE_FAST` ＋ `LOAD_FAST_BORROW` 两条 ✓）⇒ 本笔加同款上界 ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-highslot-{}-{name}", std::process::id()));
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

/// 槽 ≥ 16 时**不融合** `STORE_FAST_LOAD_FAST` ✓（4 位装不下 ⇒ 截断 ⇒ 存错槽／取错值 ✗）
const HIGH_SLOT_SCRIPT: &str = r#"def many(items):
    a0 = a1 = a2 = a3 = a4 = a5 = a6 = a7 = 0
    a8 = a9 = a10 = a11 = a12 = a13 = a14 = a15 = False
    a16 = a17 = 1
    return [x for x in items if x]


def many2(items):
    a0 = a1 = a2 = a3 = a4 = a5 = a6 = a7 = 0
    a8 = a9 = a10 = a11 = a12 = a13 = a14 = a15 = False
    a16 = a17 = 1
    return list(x for x in items if x)


print(many([1, 0, 2]))
print(many2([1, 0, 2]))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const HIGH_SLOT_EXPECTED: &str = "[1, 2]\n[1, 2]\n";

#[test]
fn high_slot_comprehensions_agree_with_the_reference() {
    let stdout = run("high_slot_comprehensions_agree", HIGH_SLOT_SCRIPT);
    assert_eq!(stdout, HIGH_SLOT_EXPECTED, "{stdout}");
}

/// `@dataclass(frozen=True, kw_only=True)` 带注解字段 ⇒ 造实例／`__repr__` 对参照 ✓
const DATACLASS_SCRIPT: &str = r#"from dataclasses import dataclass


@dataclass(frozen=True, kw_only=True)
class A:
    x: int = 1
    y: str = "z"


a = A()
print(a)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const DATACLASS_EXPECTED: &str = "A(x=1, y='z')\n";

#[test]
fn dataclass_fields_agree_with_the_reference() {
    let stdout = run("dataclass_fields_agree", DATACLASS_SCRIPT);
    assert_eq!(stdout, DATACLASS_EXPECTED, "{stdout}");
}

