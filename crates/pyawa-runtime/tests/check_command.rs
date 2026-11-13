//! **`pyawa check` 子命令验收**（PLAN §10.1；LSP 线①）。
//!
//! 钉验收：① 407 语料（`.py`）0 诊断；② 故意写错的 `.pyawa` 一次 ≥2 条类型诊断；
//! ③ `--format=json` 字段齐全；④ 语料步骤在 CI fast（本文件只验命令本身）；⑤ 同一文件
//! 「`pyawa` 执行」与「`pyawa check`」的**语法**诊断一致；另钉退出码 0／1／2 与 `.py` 无类型诊断。

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|parent| parent.parent())
        .expect("crates/<crate> 的上一级是仓库根")
        .to_path_buf()
}

/// 临时目录（每个测试独占；结束删）。
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("pyawa-check-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("建临时目录");
        Self { path }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::write(&path, contents).expect("写文件");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run(arguments: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .args(arguments)
        .output()
        .expect("跑 pyawa");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_paths<P: AsRef<Path>>(arguments: &[P]) -> (Option<i32>, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pyawa"));
    command.arg("check");
    for argument in arguments {
        command.arg(argument.as_ref());
    }
    let output = command.output().expect("跑 pyawa check");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn batch_type_diagnostics_in_one_run() {
    let temp = TempDir::new("batch");
    let file = temp.write("bad.pyawa", "a: int = \"x\"\nb: str = 1\n");
    let (code, stdout, stderr) = run_paths(&[&file]);
    assert_eq!(code, Some(1), "有诊断 ⇒ 退出码 1（stderr={stderr}）");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "一次应报 ≥2 条诊断：{stdout}");
    assert!(lines.iter().all(|line| line.contains("[type]")), "{stdout}");
    assert!(lines[0].contains(":1:1:"), "行/列应从 1 起：{stdout}");
    assert!(lines[1].contains(":2:1:"), "{stdout}");
}

#[test]
fn json_format_has_all_required_fields() {
    let temp = TempDir::new("json");
    let file = temp.write("bad.pyawa", "a: int = \"x\"\nb: str = 1\n");
    let (code, stdout, stderr) = run(&[
        "check",
        "--format=json",
        file.to_str().expect("路径是 UTF-8"),
    ]);
    assert_eq!(code, Some(1), "stderr={stderr}");
    let json = stdout.trim();
    assert!(json.starts_with('[') && json.ends_with(']'), "{json}");
    for field in ["\"file\":", "\"line\":", "\"col\":", "\"code\":", "\"severity\":", "\"message\":"] {
        assert!(json.contains(field), "缺字段 {field}：{json}");
    }
    assert_eq!(json.matches("\"code\":\"type\"").count(), 2, "{json}");
    assert_eq!(json.matches("\"severity\":\"error\"").count(), 2, "{json}");
    assert!(json.contains("\"line\":1") && json.contains("\"line\":2"), "{json}");
    assert!(json.contains("\"col\":1"), "{json}");
}

#[test]
fn py_files_get_syntax_only_and_no_type_diagnostics() {
    let temp = TempDir::new("py-only");
    // `.py` 的同类标注**不产生**类型诊断（`MS-19`／`TS-26`）⇒ 退出码 0。
    let annotated = temp.write("annotated.py", "a: int = \"x\"\n");
    let (code, stdout, stderr) = run_paths(&[&annotated]);
    assert_eq!(code, Some(0), "`.py` 不该有类型诊断：{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
}

#[test]
fn syntax_diagnostics_match_execution() {
    let temp = TempDir::new("syntax");
    let source = "def broken(:\n    pass\n";
    let file = temp.write("broken.py", source);
    let path = file.to_str().expect("路径是 UTF-8");
    // `check` 的文本诊断消息。
    let (check_code, check_stdout, check_stderr) = run(&["check", path]);
    assert_eq!(check_code, Some(1), "stderr={check_stderr}");
    let diagnostic_line = check_stdout.lines().next().expect("应有一条诊断");
    let message = diagnostic_line
        .split(": error: ")
        .nth(1)
        .and_then(|rest| rest.rsplit_once(" [syntax]").map(|(message, _)| message))
        .expect("诊断行应含消息");
    assert!(!message.is_empty(), "{diagnostic_line}");
    // 同一文件**执行**：语法诊断消息必须一致（PLAN §10.1 ⑤）。
    let (exec_code, _exec_stdout, exec_stderr) = run(&[path]);
    assert_eq!(exec_code, Some(1), "执行语法错 ⇒ 1：{exec_stderr}");
    assert!(
        exec_stderr.contains(message),
        "执行与 check 的语法诊断必须一致：check={message:?} exec={exec_stderr:?}"
    );
}

#[test]
fn corpus_python_files_have_zero_diagnostics() {
    let root = workspace_root();
    let manifest = root.join("tests/conformance/corpus/manifest.tsv");
    let text = std::fs::read_to_string(&manifest).expect("读语料清单");
    let mut files: Vec<PathBuf> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(name) = line.split('\t').next() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        files.push(root.join("tests/conformance/corpus").join(name));
    }
    assert!(files.len() >= 400, "语料条数异常：{}", files.len());
    let (code, stdout, stderr) = run_paths(&files);
    assert_eq!(code, Some(0), "语料应 0 诊断：{stdout}\n{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
}

#[test]
fn usage_errors_exit_with_host_code() {
    let (code, _stdout, _stderr) = run(&["check"]);
    assert_eq!(code, Some(2), "缺文件 ⇒ 2");
    let (code, _stdout, _stderr) = run(&["check", "--format=xml", "x.py"]);
    assert_eq!(code, Some(2), "坏 --format ⇒ 2");
    let temp = TempDir::new("usage");
    let file = temp.write("note.txt", "");
    let (code, _stdout, _stderr) = run_paths(&[&file]);
    assert_eq!(code, Some(2), "非 .py/.pyawa ⇒ 2");
    let missing = temp.path.join("missing.py");
    let (code, _stdout, _stderr) = run_paths(&[&missing]);
    assert_eq!(code, Some(2), "读不到文件 ⇒ 2");
}
