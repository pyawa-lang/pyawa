//! `pyawa` CLI 的端到端检查（`PLAN-milestones.md` 第 4 条第 ① 项的最小面）。
//!
//! 覆盖三件：跑得成（退出码 `0`）、脚本异常报 `1`、宿主侧错误报 `2`。
//! **不覆盖 `print`** —— 本版如实不打印（`CM-26`：`print` 须走 `sys.stdout → _io → fs`）。

use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_pyawa")
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pyawa-cli-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

#[test]
fn runs_a_script_through_the_fs_domain() {
    let dir = scratch("ok");
    let path = dir.join("ok.py");
    std::fs::write(&path, "x = 1\ny = x + 2\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reports_a_syntax_error_with_exit_one() {
    let dir = scratch("syntax");
    let path = dir.join("bad.py");
    std::fs::write(&path, "x = = 1\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("未捕获"), "stderr={stderr}");
}

#[test]
fn refuses_a_missing_file_with_exit_two() {
    let dir = scratch("missing");
    let output = Command::new(binary())
        .arg(dir.join("nope.py"))
        .output()
        .expect("跑 CLI");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn prints_through_the_fs_domain() {
    let dir = scratch("print");
    let path = dir.join("print.py");
    std::fs::write(&path, "print(\"hi\")\nprint(\"a\", \"b\")\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(output.status.code(), Some(0), "print 应正常退出");
    // `print` ⇒ `sys.stdout` ⇒ `_io` 文本层 ⇒ `fs` 域的 `write` ⇒ 标准流（`CM-26`：**不设临时 sink** ✓）
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hi\na b\n",
        "stdout 应是 `fs` 域写出去的字节"
    );
}

#[test]
fn exposes_the_script_path_as_argv_zero() {
    let dir = scratch("argv");
    let path = dir.join("argv.py");
    std::fs::write(&path, "from sys import argv\nprint(argv[0])\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(output.status.code(), Some(0), "stderr={}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        path.to_string_lossy(),
        "`sys.argv[0]` 应是脚本路径（与参照实现的 `argv[0]` 同义 ✓）"
    );
}

#[test]
fn imports_a_module_through_the_fs_domain() {
    let dir = scratch("import");
    std::fs::write(dir.join("helper.py"), "value = \"loaded\"\n").expect("写被导入模块");
    let path = dir.join("main.py");
    std::fs::write(&path, "import helper\nprint(helper.value)\n").expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    assert_eq!(output.status.code(), Some(0), "stderr={}", String::from_utf8_lossy(&output.stderr));
    // **I/O 走 `fs` 域**（`IM-15` ✓）：加载器按 `sys.path`（脚本所在目录 ✓）读 `helper.py` ✓
    assert_eq!(String::from_utf8_lossy(&output.stdout), "loaded\n");
}
