//! **`find`／`rfind` 的 `start`／`end`（str ＋ bytes）** —— 挂死真 bug 的回归（task-22 第 2 轮 ✓）。
//!
//! 真 bug ✗（**挂死比报错难查** ✓）：`str.find`／`rfind`／`index`／`rindex` **整个忽略** `start`／`end` ✗
//! ⇒ `logging.Manager._fixupParents` 的
//! `while (i > 0) and not rv: … i = name.rfind(".", 0, i - 1)` **永不前进** ✗ ⇒ 无限循环 ✓
//! （实测 `logging.getLogger("a.b")` 与 `import concurrent.futures` 双双挂死 ✓；
//! `PYAWA_TRACE_IMPORT` 只显示到 `logging` 执行完 ✓，再用 **AST 前缀二分**夹到
//! `LOGGER = logging.getLogger("concurrent.futures")` ✓ —— 线索是"`sys.setrecursionlimit` 只降不升、
//! 64 层该炸 `RecursionError` 却**不炸**" ✓ ⇒ 不是递归而是循环 ✓）。
//! `bytes.find`／`rfind` 同族 ✗（`rfind` 忽略起止、负数被 `max(0)` 吞 ✓）。
//!
//! 口径（照参照逐例实测 ✓）：下标按**字符**（`str`）／字节（`bytes`）✓；负数先 `+len` 再夹 `>=0` ✓；
//! `end` 缺省 `len` ✓；`start > end` ⇒ 找不到 ✓；空子串：`find` 回 `start`、`rfind` 回 `end` ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-search-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
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

/// `find`／`rfind`／`index`／`rindex` 的 `start`／`end`（含负数与空子串 ✓）—— 先前**整个忽略** ✗ ⇒ `logging._fixupParents` 死循环 ✗
const SEARCH_BOUNDS_SCRIPT: &str = r#"n = "abcabc"
print(n.find("b"), n.find("b", 2), n.find("b", 2, 3), n.find("b", -2), n.find("b", 0, -1))
print(n.rfind("b"), n.rfind("b", 0, 3), n.rfind("b", 0, 1), n.rfind("b", -3, -1))
print(n.find(""), n.find("", 3), n.find("", 4), n.find("", 5), n.rfind("", 3), n.rfind("", 0, 0))
print(n.find("b", 5, 1), n.find("b", 3, 1), n.rfind("b", 5, 1))
s = "abc"
print("find", s.find("", 4), s.find("", 3), s.find("", 2, 1), s.find("", 5, 7), s.find("b", 4), s.find("b", 0, 9))
print("rfind", s.rfind("", 4), s.rfind("", 3), s.rfind("", 2, 1), s.rfind("", -1), s.rfind("b", 0, 9), s.rfind("", 0, -9))
try:
    print(s.index("", 4))
except ValueError as exc:
    print("ValueError", exc)
print(n.index("b", 2), n.rindex("b", 0, 3))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const SEARCH_BOUNDS_EXPECTED: &str = "1 4 -1 4 1\n4 1 -1 4\n0 3 4 5 6 0\n-1 -1 -1\nfind -1 3 -1 -1 -1 1\nrfind -1 3 -1 3 1 0\nValueError substring not found\n4 1\n";

#[test]
fn find_and_rfind_start_end_agree_with_the_reference() {
    let stdout = run("find_and_rfind_start_end_agree", SEARCH_BOUNDS_SCRIPT);
    assert_eq!(stdout, SEARCH_BOUNDS_EXPECTED, "{stdout}");
}

/// `bytes.find`／`rfind` 同一条规矩 ✓（先前 `rfind` 忽略起止 ✗、负数直接 `max(0)` ✗）
const BYTES_SEARCH_SCRIPT: &str = r#"data = b"abcabc"
print(data.find(b"b"), data.find(b"b", 2), data.find(b"b", 2, 3), data.rfind(b"b", 0, 3))
print(data.find(b"", 3), data.rfind(b"", 3))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const BYTES_SEARCH_EXPECTED: &str = "1 4 -1 1\n3 6\n";

#[test]
fn bytes_find_and_rfind_start_end_agree_with_the_reference() {
    let stdout = run("bytes_find_and_rfind_start_end_agree", BYTES_SEARCH_SCRIPT);
    assert_eq!(stdout, BYTES_SEARCH_EXPECTED, "{stdout}");
}

/// `logging.getLogger("concurrent.futures")`（点号名 ✓）与四口起止一起 ✓ —— 挂死回归抽例 ✓
const NO_HANG_SCRIPT: &str = r#"import logging

logger = logging.getLogger("concurrent.futures")
print(logger.name)
print(logging.getLogger("a.b.c").name)
n = "abcabc"
print(n.find("b", 2, 3), n.rfind("b", 0, 3), n.find("", 3), n.rfind("", 3), n.rfind("b", -3, -1))
data = b"abcabc"
print(data.find(b"b", 2, 3), data.rfind(b"b", 0, 3), data.find(b"", 3), data.rfind(b"", 3))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const NO_HANG_EXPECTED: &str = "concurrent.futures\na.b.c\n-1 1 3 6 4\n-1 1 3 6\n";

#[test]
fn getlogger_and_futures_do_not_hang_with_the_reference() {
    let stdout = run("getlogger_and_futures_do_not_hang", NO_HANG_SCRIPT);
    assert_eq!(stdout, NO_HANG_EXPECTED, "{stdout}");
}

