//! **编译期栈预算守卫**（parser/emitter 递归）：深嵌套源码必须报**可捕获的 `MemoryError`**
//! （参照文案 `Parser stack overflowed - Python source too complex to parse`），**不再 abort 宿主**。
//!
//! 背景：脚本帧已堆化（`PLAN §12.22` S2/S3），但 parser/emitter 仍是 Rust 递归。实测（guard 前）
//! debug CLI 上 10 万层嵌套一元运算/括号会 `stack overflow, aborting`；CPython 在 parser stack
//! 超限时抛 `MemoryError`。本层的守卫用**实际栈用量预算**（`PYAWA_PARSE_STACK`，默认 512 KiB；
//! CLI 与对拍 harness 声明 64 MiB），超限即报同文案的 `MemoryError`。
//!
//! 测试放**子进程**：abort 只会让子进程失败，不会污染测试进程。

use std::process::Command;

fn run_with_budget(name: &str, source: &str, budget: Option<&str>) -> std::process::Output {
    let root = std::env::temp_dir().join(format!("pyawa-parser-depth-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, source).expect("写脚本");
    let mut command = Command::new(env!("CARGO_BIN_EXE_pyawa"));
    command.arg(&path).current_dir(&root);
    // 显式给预算：让 abort 用例在 debug/release 两档下都**确定**超限（不依赖 profile 的帧大小）。
    if let Some(budget) = budget {
        command.env("PYAWA_PARSE_STACK", budget);
    }
    let output = command.output().expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    output
}

fn run(name: &str, source: &str) -> std::process::Output {
    run_with_budget(name, source, None)
}

fn assert_parser_overflow_error(output: &std::process::Output, name: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.code().is_some(),
        "{name}：进程被信号终止（abort？）status={:?}",
        output.status
    );
    // 原生栈撞穿的既有症状：Rust 运行时 abort。
    assert!(
        !stderr.contains("aborting") && !stderr.contains("overflowed its stack"),
        "{name}：仍然撞穿原生栈：{stderr}"
    );
    // CLI 把编译期错误按"未捕获异常"收口：进程退出码 1，stderr 带消息（内部状态码 2 只出现在文案里）。
    // 消息与参照 `MemoryError: Parser stack overflowed - ...` 的文案一致。
    assert_eq!(output.status.code(), Some(1), "{name}：未捕获编译错误应退出码 1：{stderr}");
    assert!(
        stderr.contains("Parser stack overflowed - Python source too complex to parse"),
        "{name}：应报参照同文案的 parser 溢出消息，实际 stderr={stderr:?}"
    );
}

#[test]
fn deeply_nested_unary_does_not_abort() {
    let source = format!("x = {}1\nprint(x)\n", "-".repeat(100_000));
    // 预算 64 KiB ⇒ 两种 profile 都必然超限（debug 若不加守卫会在几十层处 abort；release 默认
    // 64 MiB 预算下 10 万层可能还能编译，所以显式收小，保证断言确定）。
    let output = run_with_budget("unary", &source, Some("65536"));
    assert_parser_overflow_error(&output, "unary");
}

#[test]
fn deeply_nested_brackets_does_not_abort() {
    let source = format!("x = {}{}\nprint(type(x))\n", "[".repeat(100_000), "]".repeat(100_000));
    let output = run_with_budget("brackets", &source, Some("65536"));
    assert_parser_overflow_error(&output, "brackets");
}

#[test]
fn modest_nesting_still_compiles_and_runs() {
    // 5000 层一元负号：CLI 声明 64 MiB 预算，应正常编译（参照在 ~6000 层以内也接受）。
    let source = format!("x = {}1\nprint(x)\n", "-".repeat(5_000));
    let output = run("modest", &source);
    assert!(
        output.status.success(),
        "正常嵌套不应失败：status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
}

#[test]
fn ordinary_code_is_unaffected() {
    let output = run("ordinary", "def f(a, b):\n    return (a + b) * (a - b)\nprint(f(3, 1))\n");
    assert!(output.status.success(), "普通代码不应受影响");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "8\n");
}
