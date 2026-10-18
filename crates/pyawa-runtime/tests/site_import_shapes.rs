//! **`import site` 回归 ＋ `sys.copyright` 断言**（round 10 修复的守门测试；round 11 补上）。
//!
//! 背景（round 10，修复随 `4b17d1c` 之前的 M5 开局那笔提交进来）：`367da5f` 上**经真 CLI** 跑
//! `import site` 会红——`Lib/site.py:467-469` 的 `setcopyright()` 直接取 `sys.copyright`，而组合根
//! 装配的 `sys` 没装这个属性 ⇒ `AttributeError: 'module' object has no attribute 'copyright'`。
//! 修复＝`sys_module::build` 里补 `sys.copyright`（值＝参照 3.14.4 原文）。本文件把三点钉成回归：
//!
//! 1. **`import site` 本身**：经真 CLI（`CARGO_BIN_EXE_pyawa`）退出码 0，且**不得**再出现
//!    `copyright` 相关的 `AttributeError`；与 `python3` 跑**同一份脚本**对拍（stdout ＋ 退出码）。
//! 2. **`import site; print("site ok")`**：同上；顺带守 `print` 通道没被这条链影响。
//! 3. **`sys.copyright` 的直接断言**：`type(...) is str`、非空、**逐字节**等于参照 CPython 3.14.4
//!    的原文（值写进 [`REFERENCE_COPYRIGHT`]）；并与本机 `python3` 同脚本输出对拍。
//!
//! 跑法：`cargo test -p pyawa-runtime --test site_import_shapes`
//!
//! 口径：只经**真 CLI 子进程**（不绕 ABI、不 mock）——round 10 的 bug 就在组合根装配的 `sys` 上，
//! 进程外才真能看见它；参照侧用本机 `python3` 跑同一份脚本（本仓既有对拍惯例，见
//! `import_semantics_shapes.rs`）。脚本文件名避开标准库同名（`site_probe.py` 而不是 `site.py`：
//! `sys.path[0]` 是脚本目录，同名会抢先命中、量到的就不是标准库的 `site` 了）。

use std::path::PathBuf;
use std::process::{Command, Output};

/// 参照 CPython 3.14.4 的 `sys.copyright` 原文（逐字节）。
///
/// round 10 用同一脚本经 CLI 与 `python3` 的 stdout `diff` 验过两侧逐字节相同；这里把它固化成
/// 期望值，防"某次重构把版权文本顺手改了"（`sys.copyright` 是 `DESIGN.md` §9 的"实现观测面"，
/// 值本身没有语义，但既然照抄了参照就必须一直照抄）。
const REFERENCE_COPYRIGHT: &str = concat!(
    "Copyright (c) 2001 Python Software Foundation.\n",
    "All Rights Reserved.\n",
    "\n",
    "Copyright (c) 2000 BeOpen.com.\n",
    "All Rights Reserved.\n",
    "\n",
    "Copyright (c) 1995-2001 Corporation for National Research Initiatives.\n",
    "All Rights Reserved.\n",
    "\n",
    "Copyright (c) 1991-1995 Stichting Mathematisch Centrum, Amsterdam.\n",
    "All Rights Reserved.",
);

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_pyawa")
}

fn scratch(tag: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("pyawa-site-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("建临时目录");
    directory
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// 把 `source` 写成临时目录里的 `name`，用 `program` 在**同一目录**下跑它（cwd＝脚本目录）。
fn run_in(program: &str, tag: &str, name: &str, source: &str) -> Output {
    let directory = scratch(tag);
    let path = directory.join(name);
    std::fs::write(&path, source).expect("写脚本");
    let output = Command::new(program)
        .arg(&path)
        .current_dir(&directory)
        .output()
        .unwrap_or_else(|error| panic!("跑 `{program}` 失败：{error}"));
    let _ = std::fs::remove_dir_all(&directory);
    output
}

fn run_pyawa(tag: &str, name: &str, source: &str) -> Output {
    run_in(binary(), tag, name, source)
}

fn run_reference(tag: &str, name: &str, source: &str) -> Output {
    run_in("python3", tag, name, source)
}

fn assert_success(what: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{what} 必须成功（退出码 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn import_site_succeeds_and_agrees_with_the_reference() {
    const SOURCE: &str = "import site\n";
    const NAME: &str = "site_probe.py";
    let ours = run_pyawa("site", NAME, SOURCE);
    assert_success("`import site`", &ours);
    let stderr = text(&ours.stderr);
    assert!(
        !stderr.contains("copyright"),
        "`import site` 不得因 `sys.copyright` 缺失而报 AttributeError（stderr={stderr}）"
    );
    let reference = run_reference("site-ref", NAME, SOURCE);
    assert_eq!(ours.status.code(), reference.status.code(), "退出码与参照不一致");
    assert_eq!(text(&ours.stdout), text(&reference.stdout), "stdout 与参照不一致");
}

#[test]
fn import_site_then_print_agrees_with_the_reference() {
    const SOURCE: &str = "import site\nprint(\"site ok\")\n";
    const NAME: &str = "site_print_probe.py";
    let ours = run_pyawa("site-print", NAME, SOURCE);
    assert_success("`import site; print(\"site ok\")`", &ours);
    assert_eq!(text(&ours.stdout), "site ok\n", "site 之后 print 的输出");
    let reference = run_reference("site-print-ref", NAME, SOURCE);
    assert_eq!(ours.status.code(), reference.status.code(), "退出码与参照不一致");
    assert_eq!(text(&ours.stdout), text(&reference.stdout), "stdout 与参照不一致");
}

#[test]
fn sys_copyright_is_a_non_empty_string_equal_to_the_reference_value() {
    const SOURCE: &str = "import sys\nprint(type(sys.copyright) is str)\nprint(sys.copyright)\n";
    const NAME: &str = "copyright_probe.py";
    let ours = run_pyawa("copyright", NAME, SOURCE);
    assert_success("`print(sys.copyright)`", &ours);

    const HEADER: &str = "True\n";
    let stdout = text(&ours.stdout);
    assert!(
        stdout.starts_with(HEADER),
        "`type(sys.copyright) is str` 必须是 `True`：{stdout:?}"
    );
    let value = &stdout[HEADER.len()..];
    assert!(!value.is_empty(), "`sys.copyright` 不得为空");
    assert_eq!(
        value,
        format!("{REFERENCE_COPYRIGHT}\n"),
        "`sys.copyright` 必须逐字节等于参照 CPython 3.14.4 的原文"
    );

    let reference = run_reference("copyright-ref", NAME, SOURCE);
    assert_success("参照 `print(sys.copyright)`", &reference);
    assert_eq!(stdout, text(&reference.stdout), "`sys.copyright` 与参照 stdout 不一致");
}
