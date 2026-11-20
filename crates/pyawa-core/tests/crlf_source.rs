//! **CRLF／lone-CR 源码的 universal newline**（`MS-19`：合法 Python 被拒必须修）。
//!
//! 背景（Windows W5 实测）：`Lib/os.py` 在 Windows checkout（`autocrlf=true`）是 CRLF，
//! 导入时报 `SyntaxError: 缩进对不齐：宽度 8 (os.py, line 411)`。根因是词法器的"块内空行"
//! 判定只认 `\n`：CRLF 空行先看到的是 `\r` ⇒ 被当成非空行 ⇒ 假 `Dedent`。
//! 修法＝`lex()` 入口做 CPython 同款 **universal newlines** 归一（`\r\n`／`\r` ⇒ `\n`）。
//!
//! 这里钉三件事：① 块内空行的 CRLF 源码可编译；② `Lib/os.py` 整体 CRLF 化后可编译；
//! ③ 单 `\r` 作为换行也可编译（CPython 的第三种行尾）。

use pyawa_core::compile::{compile, CheckTier, Mode};

fn parses(source: &str) -> bool {
    compile(source, "probe.py", Mode::PurePython, CheckTier::Shallow, 0).is_ok()
}

#[test]
fn crlf_blank_lines_inside_blocks_parse() {
    // 最小复现：缩进块内的空行（CRLF 下曾被误判成 Dedent）。
    assert!(parses("def f():\r\n    a = 1\r\n\r\n    b = 2\r\n"));
    // os.py:411 附近的形态：块尾 `except` 前面的空行。
    assert!(parses(
        "def f():\r\n    try:\r\n        a = 1\r\n\r\n    except OSError:\r\n        a = 2\r\n"
    ));
    // 空行带缩进空白也要当空行（不能因此判缩进块）。
    assert!(parses("def f():\r\n    a = 1\r\n    \r\n    b = 2\r\n"));
}

#[test]
fn lone_carriage_return_is_a_line_terminator() {
    assert!(parses("def f():\r    a = 1\r\r    b = 2\r"));
}

#[test]
fn crlf_converted_lib_os_py_parses() {
    // 端到端回归：真实 `Lib/os.py` 转 CRLF 后必须可编译（Windows checkout 的实际字节形态）。
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Lib/os.py");
    let lf = std::fs::read_to_string(&path).expect("读 Lib/os.py");
    let crlf = lf.replace('\n', "\r\n");
    assert!(parses(&crlf), "CRLF 化的 Lib/os.py 应当可编译");
}
