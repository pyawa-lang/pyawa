//! **`_io.StringIO` 真实现**（`P3-39` 第 2 轮 ✓）。
//!
//! **动因**：`TypeError: cannot create 'StringIO' instances` 是官方套件分诊里的头号签名之一
//!（15 个官方测试阻塞；`unittest` 的失败消息缓冲、`pprint.pformat`、`test.support.captured_stderr`
//! 一族都压在它上面 —— 上一轮 `test_frame` 的 3 个 subTest ERROR 正是它 ✓）。
//!
//! **实现口径**（`crates/pyawa-stdlib/src/io_module.rs` 的 `build_string_io`）：状态放实例字典
//!（`_buf` 是 `str`、`_pos` 是**码点下标**、`_closed`）；`writelines`／`detach`／`readlines(sizehint)`／
//! 关闭态文案都照参照。**如实未接** ✗：`newline` 形参不做换行翻译（`test_memoryio` 的
//! `test_newline_*` 仍 FAIL）、`__getstate__`／`__setstate__`／pickle、`IntLike`（`__index__`）实参。
//!
//! 期望值全部由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`：不改变合法 Python 的可观察语义）。
//! `StringIO` 的位置是**码点**下标 ⇒ 与参照一致（非 ASCII 也一样 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-stringio-{}-{name}", std::process::id()));
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

/// `write`／`getvalue`／`tell`／`read`／`readline`／`seek`（含越过末尾的 `'\x00'` 补齐）／
/// `readlines`（EOF 不产生空串 ＋ `sizehint`）／`truncate`／`writelines`／关闭态文案。
const STRINGIO_SCRIPT: &str = r#"import io

s = io.StringIO()
print("write", s.write("abc"), "tell", s.tell(), "value", repr(s.getvalue()))
s.seek(0)
print("read2", repr(s.read(2)), "pos", s.tell())
print("readline", repr(s.readline()), "eof", repr(s.read()))

s2 = io.StringIO("abc")
print("init", s2.tell(), repr(s2.getvalue()), repr(s2.read()))

s3 = io.StringIO()
s3.seek(5)
print("pad", s3.write("x"), repr(s3.getvalue()))

s4 = io.StringIO("l1\nl2\n")
print("lines", [repr(line) for line in s4])
s5 = io.StringIO("1234567890\n" * 10)
print("readlines", s5.readlines() == ["1234567890\n"] * 10)
s5.seek(0)
print("readlines15", s5.readlines(15) == ["1234567890\n"] * 2)

s6 = io.StringIO("hello")
s6.seek(2)
print("truncate", s6.truncate(), repr(s6.getvalue()))
print("preds", s6.seekable(), s6.writable(), s6.readable(), s6.isatty())

s7 = io.StringIO()
s7.writelines(["ab", "cd"])
print("writelines", repr(s7.getvalue()))

s8 = io.StringIO("abc")
with s8 as f:
    f.write("d")
print("closed", s8.closed)
for what in ("getvalue", "write", "read", "tell", "readable", "seek", "truncate"):
    try:
        if what == "write":
            s8.write("x")
        elif what == "seek":
            s8.seek(0)
        else:
            getattr(s8, what)()
        print(what, "no-error")
    except Exception as e:
        print(what, "raised", type(e).__name__, "|" + str(e) + "|")
print("flush", s8.flush())
s8.close()
print("double-close ok")

try:
    io.StringIO().write(b"x")
except Exception as e:
    print("write-bytes", type(e).__name__, "|" + str(e) + "|")
print("props", repr(io.StringIO().encoding), repr(io.StringIO().errors), io.StringIO().line_buffering)
print("types", isinstance(io.StringIO(), io.TextIOBase), isinstance(io.StringIO(), io.IOBase))
"#;

/// 参照（本机 `python3` 3.14.4 实跑，逐行）。
const STRINGIO_EXPECTED: &str = "write 3 tell 3 value 'abc'\nread2 'ab' pos 2\nreadline 'c' eof ''\ninit 0 'abc' 'abc'\npad 1 '\\x00\\x00\\x00\\x00\\x00x'\nlines [\"'l1\\\\n'\", \"'l2\\\\n'\"]\nreadlines True\nreadlines15 True\ntruncate 2 'he'\npreds True True True False\nwritelines 'abcd'\nclosed True\ngetvalue raised ValueError |I/O operation on closed file|\nwrite raised ValueError |I/O operation on closed file|\nread raised ValueError |I/O operation on closed file|\ntell raised ValueError |I/O operation on closed file|\nreadable raised ValueError |I/O operation on closed file|\nseek raised ValueError |I/O operation on closed file|\ntruncate raised ValueError |I/O operation on closed file|\nflush None\ndouble-close ok\nwrite-bytes TypeError |string argument expected, got 'bytes'|\nprops None None False\ntypes True True\n";

#[test]
fn stringio_write_getvalue_close_match_the_reference() {
    assert_eq!(run("semantics", STRINGIO_SCRIPT), STRINGIO_EXPECTED);
}

/// `__index__` 形状的实参（`IntLike`）**未接** ⇒ 如实 FAIL/ERROR 的是 `test_memoryio` 其它方法；
/// 这里只钉"**码点**下标"这一条（非 ASCII 的 `tell`／`len` 与参照一致 ✓）。
const UNICODE_SCRIPT: &str = r#"import io

s = io.StringIO("\u00e9\u00e9")
print(s.tell(), len(s.getvalue()), repr(s.read(1)), s.tell())
s.seek(0)
s.write("\u00e9")
print(repr(s.getvalue()))
"#;

/// 参照（python3 3.14.4 实跑）：`0 2 'é' 1` ＋ `'éé'`。
const UNICODE_EXPECTED: &str = "0 2 'é' 1\n'éé'\n";

#[test]
fn stringio_positions_are_code_points_not_bytes() {
    assert_eq!(run("unicode", UNICODE_SCRIPT), UNICODE_EXPECTED);
}
