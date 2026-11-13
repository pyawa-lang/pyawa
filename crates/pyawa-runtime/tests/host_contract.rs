//! **DESIGN §7.5「威胁模型与宿主责任」的机械验收**（round 267）：
//!
//! 只钉**可机检**的三条 VM 保证；宿主责任（壁钟超时、同实例串行、provider 根夹紧等）是
//! **宿主契约**，由 `SPEC-c-abi` 的编号与宿主侧实现负责（不在本测试里假装能测 ✗）。
//!
//! 1. **无原生扩展／FFI 出口**：`ctypes`／`cffi`／`_ctypes` 一律 `ModuleNotFoundError`
//!    （脚本不能把宿主符号拉进地址空间 ✓）；
//! 2. **未接线能力域如实报**：`_socket.getaddrinfo` 对域名报 `NotImplementedError`（`CM-8`），
//!    **绝不**退化成"直通宿主"（不静默成功 ✓）；
//! 3. **栈溢出是干净异常**：无限递归报 `RecursionError`，退出码 1（不是 SIGSEGV／abort ✓）。

use std::process::Command;

fn run(name: &str, script: &str) -> (Option<i32>, String, String) {
    let root = std::env::temp_dir().join(format!("pyawa-host-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// `DESIGN §7.5`：**无原生扩展／FFI 出口**——`ctypes`／`cffi`／`_ctypes` 必须导不进来。
#[test]
fn no_native_extension_or_ffi_escape_hatches() {
    for module in ["ctypes", "cffi", "_ctypes"] {
        let script = format!("import {module}\n");
        let (code, _stdout, stderr) = run(module, &script);
        assert_eq!(
            code,
            Some(1),
            "`import {module}` 应是普通导入失败（stderr={stderr}）"
        );
        assert!(
            stderr.contains("ModuleNotFoundError"),
            "`{module}` 应报 ModuleNotFoundError（stderr={stderr:?}）"
        );
        assert!(
            !stderr.contains("SIGSEGV") && !stderr.contains("SIGABRT"),
            "不许以信号结束（stderr={stderr:?}）"
        );
    }
}

/// `DESIGN §7.5`：**未接线能力域如实报**，不退化成直通宿主。
#[test]
fn unwired_network_domain_is_honest_not_passthrough() {
    let script = "import _socket\n_socket.getaddrinfo(\"example.com\", 80)\n";
    let (code, stdout, stderr) = run("dns", script);
    assert_eq!(code, Some(1), "应是脚本异常退出（stderr={stderr}）");
    assert!(
        stderr.contains("NotImplementedError"),
        "域名解析应报 NotImplementedError（`CM-8`；stderr={stderr:?}）"
    );
    assert!(
        stderr.contains("未接线"),
        "错误消息应说明未接线（stderr={stderr:?}）"
    );
    assert!(stdout.is_empty(), "不得静默成功（stdout={stdout:?}）");
}

/// `DESIGN §7.5`：**栈溢出是干净异常**（`RecursionError`），不是 abort／段错误。
#[test]
fn deep_recursion_is_a_clean_recursion_error() {
    let script = "def f():\n    return f()\nf()\n";
    let (code, _stdout, stderr) = run("recursion", script);
    assert_eq!(code, Some(1), "应是普通脚本异常（stderr={stderr}）");
    assert!(
        stderr.contains("RecursionError"),
        "应报 RecursionError（stderr={stderr:?}）"
    );
    assert!(
        !stderr.contains("SIGSEGV") && !stderr.contains("stack overflow") && !stderr.contains("abort"),
        "不许以 abort／段错误结束（stderr={stderr:?}）"
    );
}
