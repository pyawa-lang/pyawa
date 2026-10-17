//! **仪器护栏：`pa_create` 必须注入宿主 `errno` 表**（task-20 ✓，P0 修复的回归）
//!
//! 背景（`target/recon/type_method_descriptors.md` §8）：判据①／上限 census 的被测入口是
//! conformance runner（`execute_pyawa` → **`pa_create`**）。先前平台常量只在 CLI 手动补
//! （`bin/pyawa.rs`）⇒ runner 走的 ABI 路径上 `errno` 是**空表** ⇒ census 里
//! `'module' object has no attribute 'ECHILD'`（48 格）＋`cannot import name 'ENOENT' from 'errno'`
//! （2 格）＝ ~50 格**仪器假红**。修法（用户拍板）＝表搬到 `pyawa-abi`、**在 `pa_create` 内注入一次**
//! （一处真相：CLI／语料 harness／C 宿主同源）。
//!
//! 本抽例两件：
//! 1. **CLI 面**：`import errno` 的常量/`errorcode` 与参照（本机 `python3` 3.14.4）**逐行一致** ✓；
//! 2. **ABI 面**：`safe::create`（＝`pa_create` 的薄门面）交回的实例**按名字查得到**生成表里的
//!    errno 值（用**表本身**当期望，不手写平台数字 ✓ `CM-20`）。

use core::ffi::c_void;
use std::path::PathBuf;

use pyawa_abi::{pa_host, safe, status, PA_ABI_VERSION};

fn run_cli(script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-errno-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path: PathBuf = root.join("probe.py");
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

/// `errno` 的脚本面（两侧逐行一致；期望由本机 `python3` 3.14.4 实测）。
const ERRNO_SCRIPT: &str = r#"import errno

print("echild", errno.ECHILD)
print("enoent", errno.ENOENT)
print("eagain-alias", errno.EAGAIN == errno.EWOULDBLOCK)
print("errorcode-2", errno.errorcode[2])
print("errorcode-10", errno.errorcode[10])
print("errorcode-type", type(errno.errorcode).__name__)
print("module-name", errno.__name__)
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const ERRNO_EXPECTED: &str = "echild 10\nenoent 2\neagain-alias True\nerrorcode-2 ENOENT\nerrorcode-10 ECHILD\nerrorcode-type dict\nmodule-name errno\n";

#[test]
fn cli_errno_face_matches_reference() {
    assert_eq!(run_cli(ERRNO_SCRIPT), ERRNO_EXPECTED);
}

/// 从生成表里取一个名字的值（**不手写平台数字** ✓ `CM-20`：数字随平台、映射按名字）。
fn table_value(name: &str) -> i64 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, value)| *value)
        .unwrap_or_else(|| panic!("生成表里应当有 {name}"))
}

#[test]
fn pa_create_injects_the_platform_errno_table() {
    let host = pa_host {
        abi_size: core::mem::size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null::<c_void>(),
    };
    // `safe::create` 是 `pa_create` 的一对一薄门面（`AB-4`／`AB-6`）⇒ 测的就是 ABI 创建路径。
    let (state, code) = safe::create(&host);
    assert_eq!(code, status::PA_OK, "建实例应当成功");
    assert!(!state.is_null(), "可用实例不该是 NULL");
    // ① 按名字查得到生成表里的值（`ECHILD`／`ENOENT` 就是先前假红的两族）
    assert_eq!(safe::platform_constant(state, "ECHILD"), Some(table_value("ECHILD")));
    assert_eq!(safe::platform_constant(state, "ENOENT"), Some(table_value("ENOENT")));
    // ② 表是**整表**注入（不是只补两个名字）
    assert!(
        pyawa_abi::platform_errno::HOST_ERRNO.len() > 100,
        "生成表本身应当有一百多项"
    );
    assert_eq!(safe::platform_constant(state, "绝不会存在的 errno 名字"), None);
    safe::destroy(state);
}
