//! **每实例内存配额 · CLI 宿主接缝**（`OM-3` 扩；`PYAWA_MEMORY_LIMIT` 环境变量）。
//!
//! 口径：CLI 只是**示例宿主**——它把环境变量翻译成 ABI `pa_set_memory_limit`（`AB-65`）；
//! 正式宿主应直接调 ABI。超限时脚本里是**可捕获的** `MemoryError`；未捕获则按普通脚本异常
//! 退出（退出码 1，不是 signal／abort ✓）。

use std::process::Command;

fn run(name: &str, script: &str, env: Option<(&str, &str)>) -> (Option<i32>, String, String) {
    let root = std::env::temp_dir().join(format!("pyawa-quota-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let mut command = Command::new(env!("CARGO_BIN_EXE_pyawa"));
    command.arg(&path);
    if let Some((key, value)) = env {
        command.env(key, value);
    }
    let output = command.output().expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn caught_quota_error_exits_cleanly() {
    let script = r#"
data = []
n = 0
try:
    while n < 200000:
        data.append("x" * 1000)
        n = n + 1
except MemoryError:
    print("caught")
"#;
    let (code, stdout, stderr) = run("caught", script, Some(("PYAWA_MEMORY_LIMIT", "4M")));
    assert_eq!(code, Some(0), "应有干净退出码\nstdout={stdout}\nstderr={stderr}");
    assert!(stdout.contains("caught"), "stdout={stdout:?}");
}

#[test]
fn uncaught_quota_error_is_a_script_error_not_a_signal() {
    let script = r#"
data = []
n = 0
while n < 200000:
    data.append("x" * 1000)
    n = n + 1
"#;
    let (code, _stdout, stderr) = run("uncaught", script, Some(("PYAWA_MEMORY_LIMIT", "4M")));
    assert_eq!(code, Some(1), "未捕获的 MemoryError 应走脚本异常退出码 1（stderr={stderr}）");
    assert!(stderr.contains("MemoryError"), "stderr={stderr:?}");
    assert!(!stderr.contains("SIGSEGV"), "不许崩（stderr={stderr:?}");
}

#[test]
fn huge_single_allocation_is_refused_before_touching_the_host() {
    // `1M` 后缀解析（宿主接缝）＋ 大块前置预算：2 MiB 的 `bytes`／`str` 直接 MemoryError。
    let script = r#"
try:
    s = "y" * (2 * 1024 * 1024)
    print("allocated", len(s))
except MemoryError:
    print("refused")
"#;
    let (code, stdout, stderr) = run("huge", script, Some(("PYAWA_MEMORY_LIMIT", "1m")));
    assert_eq!(code, Some(0), "stdout={stdout}\nstderr={stderr}");
    assert!(stdout.contains("refused"), "stdout={stdout:?}");
}

#[test]
fn no_limit_keeps_normal_programs_working() {
    let script = r#"
data = []
i = 0
while i < 1000:
    data.append(i)
    i = i + 1
print(len(data))
"#;
    let (code, stdout, stderr) = run("nolimit", script, None);
    assert_eq!(code, Some(0), "stdout={stdout}\nstderr={stderr}");
    assert!(stdout.trim() == "1000", "stdout={stdout:?}");
}

#[test]
fn invalid_limit_is_ignored_or_host_error() {
    // 非法值（非数字／空串）⇒ 宿主接缝**不设配额**（示例实现选择：忽略 ⇒ 不影响脚本 ✓）。
    // 正式宿主应自己校验（ABI 收 u64，校验在宿主侧 ✓）。
    let script = "print(\"ok\")\n";
    let (code, stdout, _stderr) = run("invalid", script, Some(("PYAWA_MEMORY_LIMIT", "not-a-number")));
    assert_eq!(code, Some(0));
    assert!(stdout.contains("ok"));
}
