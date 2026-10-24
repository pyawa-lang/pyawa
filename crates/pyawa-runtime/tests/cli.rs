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

// ---- `P3-32`：`sys.exit` ＋ `SystemExit` 退出码（参照 3.14.4 实测口径）----

fn run_script(tag: &str, source: &str) -> std::process::Output {
    let dir = scratch(tag);
    let path = dir.join(format!("{tag}.py"));
    std::fs::write(&path, source).expect("写脚本");
    Command::new(binary()).arg(&path).output().expect("跑 CLI")
}

#[test]
fn sys_exit_with_int_sets_the_process_exit_code() {
    let output = run_script("exit-int", "import sys\nsys.exit(3)\n");
    assert_eq!(
        output.status.code(),
        Some(3),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    // 未捕获的 `SystemExit` **不是错误** ⇒ 不打"未捕获"诊断
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("未捕获"), "stderr={stderr}");
}

#[test]
fn sys_exit_without_arguments_exits_zero() {
    let output = run_script("exit-none", "import sys\nsys.exit()\n");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sys_exit_with_a_string_prints_it_to_stderr_and_exits_one() {
    let output = run_script("exit-str", "import sys\nsys.exit('boom')\n");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("boom"), "stderr={stderr}");
    assert!(!stderr.contains("未捕获"), "stderr={stderr}");
}

#[test]
fn raise_system_exit_with_an_int_sets_the_process_exit_code() {
    let output = run_script("raise-exit", "raise SystemExit(7)\n");
    assert_eq!(
        output.status.code(),
        Some(7),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sys_exit_is_catchable_as_system_exit() {
    // `sys.exit` **本身不退出进程**：实现＝抛 `SystemExit`（参照同义）
    let output = run_script(
        "exit-caught",
        "import sys\ntry:\n    sys.exit(3)\nexcept SystemExit:\n    print('caught')\n",
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "caught\n");
}

#[test]
fn sys_exit_with_two_arguments_reports_the_measured_type_error() {
    let output = run_script("exit-two", "import sys\nsys.exit(3, 4)\n");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("TypeError: exit expected at most 1 argument, got 2"),
        "stderr={stderr}"
    );
}

// ---- `P3-33`：`PYTHONPATH` 进 `sys.path`（脚本目录之后、`Lib/` 之前）----

#[test]
fn pythonpath_entries_are_importable() {
    let dir = scratch("pythonpath");
    let library = dir.join("pplib");
    std::fs::create_dir_all(&library).expect("建 PYTHONPATH 目录");
    std::fs::write(library.join("ppmod.py"), "X = 1\n").expect("写 PYTHONPATH 模块");
    let path = dir.join("main.py");
    std::fs::write(&path, "import sys, ppmod\nprint(ppmod.X)\n").expect("写脚本");
    let output = Command::new(binary())
        .arg(&path)
        .env("PYTHONPATH", &library)
        .output()
        .expect("跑 CLI");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
}

// ---- `P3-32` 的冒烟：`unittest.main()` 正常退出（依赖任务 A/C：dict kwargs ＋ `Lib/test`）----

#[test]
fn unittest_main_smoke_exits_zero() {
    let output = run_script(
        "unittest-smoke",
        "import unittest\n\nclass T(unittest.TestCase):\n    def test_ok(self):\n        self.assertEqual(1, 1)\n\nunittest.main()\n",
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
