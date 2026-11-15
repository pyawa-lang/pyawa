//! **`io.open`／`pathlib` 文件面**（PLAN 收口后下一阶段 ③）的参照对拍（round 268）。
//!
//! 口径：同一段脚本在本机 **CPython 3.14.4** 与 pyawa 上**逐行一致**（`EXPECTED` 由参照在
//! **干净工作目录**跑出 ✓ —— 不能把仓库 `target/` 放进 `sys.path`：那里有 `nt.py` 一类探针脚本
//! 会 shadow 标准库模块并往 stdout 打东西 ✗）。
//!
//! 覆盖：`Path.write_text/read_text/write_bytes/read_bytes/open/stat/iterdir/glob/touch/rename/
//! unlink/rmdir/symlink/readlink/with_suffix/is_relative_to/exists/chmod`、`io.open`（读写/追加/
//! 编码/`with`/逐行迭代/缺失文件）、`io.DEFAULT_BUFFER_SIZE`、`io.text_encoding`、`os.fspath`，
//! 以及**前导括号表达式 + `.open()`** 的 `with` 形态（解析器 round 268 修 ✓）。
//!
//! 已知偏差（如实记 ✗）：`Path.resolve()/absolute()/home()/cwd()` 的输出含**绝对路径** ⇒ 不进
//! 对拍（本文件只钉相对路径确定面）；`open_code` 仍是占位（`_io` stub ✓）。

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/<crate> 的上一级是仓库根")
        .to_path_buf()
}

fn run_reference_pyawa(script: &str) -> (i32, String) {
    let root = workspace_root();
    let dir = root.join("target/io_pathlib_face_script");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .current_dir(&dir)
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&dir);
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

const SCRIPT: &str = r##"import io
import os
import pathlib
import shutil

root = pathlib.Path("target/io_pathlib_face_root")

def with_write(path):
    with io.open(path, "w", encoding="utf-8") as f:
        return f.write("one\ntwo\n")

def with_read(path):
    with io.open(path, encoding="utf-8") as f:
        return f.read()

def with_paren(path):
    with (path).open("w", encoding="utf-8") as f:
        return f.write("paren")

def iter_lines(path):
    out = []
    with io.open(path, encoding="utf-8") as f:
        for line in f:
            out.append(line)
    return out

def readlink_result():
    target = root / "b.bin"
    (root / "link2").symlink_to(target)
    return os.readlink(root / "link2")

def rmdir_error():
    d = root / "dd"
    d.mkdir()
    (d / "x").touch()
    try:
        d.rmdir()
    except OSError as exc:
        return type(exc).__name__, exc.errno, exc.strerror is not None
    return "no-error"

def io_open_missing():
    try:
        io.open(root / "nope", "r").read()
    except OSError as exc:
        return type(exc).__name__
    return "no-error"

try:
    shutil.rmtree(root)
except OSError:
    pass
root.mkdir(parents=True)

p = root / "a.txt"
print("mkdir", root.exists(), root.is_dir())
print("write_text", p.write_text("alpha\nbeta\n"))
print("read_text", repr(p.read_text()))
print("encoding", repr(p.write_text("héllo", encoding="utf-8")), repr(p.read_text(encoding="utf-8")))
print("bytes", (root / "b.bin").write_bytes(b"\x00\x01"), repr((root / "b.bin").read_bytes()))
p.chmod(0o640)
print("stat", p.stat().st_size, oct(p.stat().st_mode & 0o777))
print("iterdir", sorted(x.name for x in root.iterdir()))
print("glob", sorted(x.name for x in root.glob("*.txt")))
print("open_r", repr(p.open("r").read()))
print("open_a", (p.open("a")).write("tail\n"), repr(p.read_text()))
print("with_write", with_write(root / "c.txt"))
print("with_read", repr(with_read(root / "c.txt")))
print("with_paren", with_paren(root / "d.txt"))
print("iter_lines", iter_lines(p))
print("touch", (root / "t").touch() is None, (root / "t").is_file())
print("rename", (root / "t").rename(root / "u").name)
print("readlink", readlink_result())
(root / "link").symlink_to(root / "b.bin")
print("symlink", (root / "link").is_symlink(), os.readlink(root / "link"))
print("unlink", (root / "u").unlink())
print("rmdir_error", rmdir_error())
print("suffix", (root / "a.txt").with_suffix(".md").name)
print("relative", (root / "a.txt").is_relative_to(root))
print("missing", (root / "nope").exists())
print("io_open_missing", io_open_missing())
print("fspath", os.fspath(p))
print("buffer", io.DEFAULT_BUFFER_SIZE)
print("text_encoding", io.text_encoding(None), io.text_encoding("utf-8"))
"##;

/// 参照（CPython 3.14.4）在同一脚本、同一干净相对路径下的逐行输出。
const EXPECTED: &str = r##"mkdir True True
write_text 11
read_text 'alpha\nbeta\n'
encoding 5 'héllo'
bytes 2 b'\x00\x01'
stat 6 0o640
iterdir ['a.txt', 'b.bin']
glob ['a.txt']
open_r 'héllo'
open_a 5 'héllotail\n'
with_write 8
with_read 'one\ntwo\n'
with_paren 5
iter_lines ['héllotail\n']
touch True True
rename u
readlink target/io_pathlib_face_root/b.bin
symlink True target/io_pathlib_face_root/b.bin
unlink None
rmdir_error ('OSError', 39, True)
suffix a.md
relative True
missing False
io_open_missing FileNotFoundError
fspath target/io_pathlib_face_root/a.txt
buffer 131072
text_encoding locale utf-8
"##;

#[test]
fn io_and_pathlib_daily_face_matches_the_reference() {
    let (code, stdout) = run_reference_pyawa(SCRIPT);
    assert_eq!(code, 0, "脚本应干净结束\nstdout=\n{stdout}");
    assert_eq!(stdout, EXPECTED, "与 CPython 3.14.4 的逐行输出必须一致");
}

/// **`with (` 两种形态的运行期行为**（round 268）：项列表 ⇒ 依次进入／`as` 绑定；
/// 括号表达式 + `.open()` ⇒ 先求值再进入（与 CPython 3.14.4 逐行一致）。
const WITH_SCRIPT: &str = r##"class CM:
    def __init__(self, name):
        self.name = name
    def __enter__(self):
        print("enter", self.name)
        return self
    def __exit__(self, *exc):
        return None
    def open(self, mode):
        print("open", self.name)
        return self

a = CM("a")
b = CM("b")
with (a, b):
    print("body1")
with (a as x, b as y):
    print("body2", x.name, y.name)
with (a).open("wb") as f:
    print("body3", f.name)
"##;

const WITH_EXPECTED: &str = "enter a\nenter b\nbody1\nenter a\nenter b\nbody2 a b\nopen a\nenter a\nbody3 a\n";

#[test]
fn parenthesized_with_forms_behave_like_the_reference() {
    let root = workspace_root();
    let dir = root.join("target/io_pathlib_face_with");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("probe.py");
    std::fs::write(&path, WITH_SCRIPT).expect("写脚本");
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .current_dir(&dir)
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "脚本应干净结束\nstderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        WITH_EXPECTED,
        "with 两种形态的进入顺序／绑定必须与参照一致"
    );
}
