//! `os.stat_result` 的端到端形状（task-19）：**时间戳族** ＋ **逐结果独立**（不是共享类型上的字段）。
//!
//! 为什么在 runtime tests：验收要的是端到端可观察行为 —— 编译 → 执行 → `Lib/` import → `os.stat`
//! → `print`；而且 `fs` 域的真实现（`PosixFs`）只有 CLI 有。期望值都来自本机 `python3` 3.14.4
//! 跑**同一段探针**（`MS-17`：不改变 `.py` 的既有可观察语义）。
//!
//! 覆盖三条：
//! 1. `st_mtime`／`st_mtime_ns` 存在，且与参照同口径（`st_mtime == st_mtime_ns / 1e9`，浮点秒／整数纳秒）；
//! 2. `st_mode` 是宿主原始 `st_mode`（文件类型位可判：目录 `S_IFDIR`、普通文件 `S_IFREG`）；
//! 3. **逐结果独立**：先 stat 小文件、再 stat 大文件，回头再 stat 小文件尺寸不串
//!    （既有 bug：字段写在共享类型上 ⇒ 会被后一次 stat 覆盖）。

fn run_in_temp_dir(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-stat-result-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    // 两个**大小不同**的文件（后者覆盖前者是既有 bug 的症状）
    std::fs::write(root.join("a.txt"), "12345").expect("写小文件");
    std::fs::write(root.join("b.txt"), "1234567890").expect("写大文件");
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

/// 探针（`python3` 3.14.4 实测逐行一致）。
const STAT_PROBE: &str = r#"import os

s = os.stat(".")
print("dir-mode:", s.st_mode & 0o170000 == 0o040000)
print("dir-mtime:", s.st_mtime > 0, s.st_mtime == s.st_mtime_ns / 1e9)

a = os.stat("a.txt")
b = os.stat("b.txt")
print("file-sizes:", a.st_size, b.st_size)
print("file-again:", os.stat("a.txt").st_size)
print("types:", type(a.st_mtime).__name__, type(a.st_mtime_ns).__name__)
print("file-mode:", a.st_mode & 0o170000 == 0o100000)
print("times:", hasattr(a, "st_atime"), hasattr(a, "st_ctime"),
      hasattr(a, "st_atime_ns"), hasattr(a, "st_ctime_ns"))
"#;

/// 参照（本机 `python3` 3.14.4 实测 ✓，逐行）。
const STAT_PROBE_EXPECTED: &str = "dir-mode: True\n\
dir-mtime: True True\n\
file-sizes: 5 10\n\
file-again: 5\n\
types: float int\n\
file-mode: True\n\
times: True True True True\n";

#[test]
fn stat_result_has_times_and_fields_are_per_object() {
    let stdout = run_in_temp_dir("stat", STAT_PROBE);
    assert_eq!(stdout, STAT_PROBE_EXPECTED, "{stdout}");
}
