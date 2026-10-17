//! **`int(x, base)`**（第 601 轮接线 ✓）——`re/_compiler.py:402` 的 `int(二进制串, 2)` 要它 ✓。
//!
//! 口径照参照实测 ✓：`base` 只接整数（否则 `TypeError: '<名>' object cannot be interpreted as an integer` ✓）；
//! `base` 只能是 `0` 或 `2..=36` ✓；`base == 0` 走**前缀判定** ✓（`0x`／`0o`／`0b` ✓、`"0"` ✓、
//! 前导零的十进制 `"010"` **报错** ✓）；`base` 为 2／8／16 时**允许**对应前缀 ✓；下划线只允许数字之间 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-int-{}-{name}", std::process::id()));
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
cases = ["ff", "0x10", "0b101", "0o17", "101", "-5", " 10 ", "1_0", "z", "0", "0x1f", "12.5", "010", "ff"]
bases = [16, 16, 2, 8, 2, 10, 2, 2, 36, 0, 0, 10, 0, 0]
for text, base in zip(cases, bases):
    try:
        print(text, base, "=>", int(text, base))
    except Exception as error:
        print(text, base, "=>", type(error).__name__, error)
try:
    print("int(5, 2) =>", int(5, 2))
except Exception as error:
    print("int(5, 2) =>", type(error).__name__, error)
try:
    print("base 37 =>", int("1", 37))
except Exception as error:
    print("base 37 =>", type(error).__name__, error)
"#;

/// 参照（`python3` 3.14 实测 ✓，15 例含异常类型与原文消息 ✓）。
const EXPECTED: &str = "ff 16 => 255\n0x10 16 => 16\n0b101 2 => 5\n0o17 8 => 15\n101 2 => 5\n-5 10 => -5\n 10  2 => 2\n1_0 2 => 2\nz 36 => 35\n0 0 => 0\n0x1f 0 => 31\n12.5 10 => ValueError invalid literal for int() with base 10: '12.5'\n010 0 => ValueError invalid literal for int() with base 0: '010'\nff 0 => ValueError invalid literal for int() with base 0: 'ff'\nint(5, 2) => TypeError int() can't convert non-string with explicit base\nbase 37 => ValueError int() base must be >= 2 and <= 36, or 0\n";

#[test]
fn int_with_base_agrees_with_the_reference() {
    let stdout = run("with_base", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
