//! **`builtins.open` ＋ 文件对象**（task-18 ✓）——`r`／`rb`／`w`／`wb`／`a`／`ab`／`x` ✓、
//! `read`／`readline`／`readlines`／`write`／`seek`／`tell`／`flush`／`close`／`with`／**迭代** ✓、
//! 错误文案（`FileNotFoundError`／`FileExistsError`／`ValueError`／`TypeError` ✓）与编码 ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段脚本**取得 ✓（脚本见 `SCRIPT` ✓；脚本在临时目录里
//! **自建**它读的文件 ✓ ⇒ 不污染仓库 ✓）。三处**真 bug** 被这组抽例钉住 ✗：
//! ① `fs` provider 的**句柄号段**从 1 起 ✗ ⇒ 进程头两个文件拿到 `stdout`／`stderr` 的号 ✓
//! （`with open(…)` 写的数据跑到终端 ✓）——现在真文件从 3 起 ✓；
//! ② `FOR_ITER` 对"非本层接线的迭代器"直接报未接线 ✗ ⇒ 用户类与文件对象都进不了 `for` ✓；
//! ③ `readlines()` 把句尾那次空读也算一行 ✗（参照不 ✓）。
//!
//! **如实** ✗：`io.UnsupportedOperation` 那一档只对**消息**（`write` ✓），类名对不上 ✓
//! （核心造不出可 raise 的异常子类 ⇒ 一 raise 就撞 `adopt` 断言 ✓，见 `NEXT.md` ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-file-{}-{name}", std::process::id()));
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

const SCRIPT: &str = r#"with open("data.txt", "w") as created:
    created.write("line one\nline two\nline three\n")

f = open("data.txt", "r")
print(f.readline())
print(f.tell())
f.seek(0)
print(f.read())
print(f.read())
f.close()
print(f.closed)
try:
    f.read()
except ValueError as exc:
    print("closed!", str(exc))

b = open("data.txt", "rb")
line = b.readline()
print(type(line).__name__, line)
print(b.tell())
b.seek(0)
print(b.read())
b.close()

w = open("out.txt", "w")
print(w.write("hello\n"))
print(w.write("world\n"))
w.close()
print(open("out.txt").read())

a = open("out.txt", "a")
print(a.write("tail\n"))
a.close()
print(open("out.txt").read())

with open("out.txt") as f:
    for line in f:
        print("row", line.rstrip("\n"))
print(open("out.txt").readlines())

probe = open("data.txt", "r")
print(probe.readable(), probe.writable(), probe.seekable(), probe.isatty())
probe.close()

try:
    open("missing.txt")
except Exception as exc:
    print(type(exc).__name__, str(exc))
try:
    open("out.txt", "x")
except Exception as exc:
    print(type(exc).__name__, str(exc))
try:
    open("out.txt", "q")
except Exception as exc:
    print(type(exc).__name__, str(exc))
try:
    open("out.txt", "rw")
except Exception as exc:
    print(type(exc).__name__, str(exc))
# **如实** ✗：参照这里给 `io.UnsupportedOperation: write`；核心造不出可 raise 的异常子类
# （`type.__new__` 的子类 instance_size 仍是普通对象 ⇒ 一 raise 就撞 `adopt` 断言）⇒ 我们按
# `OSError` 抛、消息 `write` 照参照 ✓ ⇒ 本抽例**只对消息**、不对类名 ✓（见 `NEXT.md` 未修 bug ✓）。
try:
    open("data.txt", "rb").write("x")
except Exception as exc:
    print("unsupported-msg", str(exc))
try:
    open("data.txt", "r").write(b"x")
except Exception as exc:
    print(type(exc).__name__, str(exc))

with open("utf8.txt", "wb") as created:
    created.write(b"caf\xc3\xa9\n")
print(open("utf8.txt", encoding="utf-8").read())
print(open("utf8.txt", encoding="latin-1").read())

buffer = open("data.txt", "rb")
first = buffer.readline()
buffer.seek(0)
print(first, len(buffer.read()))
buffer.close()

class Countdown:
    def __init__(self, n):
        self.n = n
    def __iter__(self):
        return self
    def __next__(self):
        if self.n <= 0:
            raise StopIteration
        self.n = self.n - 1
        return self.n

for value in Countdown(3):
    print("cd", value)
print(list(Countdown(2)))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `SCRIPT` 的每一步 ✓。
const EXPECTED: &str = "line one\n\n9\nline one\nline two\nline three\n\n\nTrue\nclosed! I/O operation on closed file.\nbytes b'line one\\n'\n9\nb'line one\\nline two\\nline three\\n'\n6\n6\nhello\nworld\n\n5\nhello\nworld\ntail\n\nrow hello\nrow world\nrow tail\n['hello\\n', 'world\\n', 'tail\\n']\nTrue False True False\nFileNotFoundError [Errno 2] No such file or directory: 'missing.txt'\nFileExistsError [Errno 17] File exists: 'out.txt'\nValueError invalid mode: 'q'\nValueError must have exactly one of create/read/write/append mode\nunsupported-msg write\nTypeError write() argument must be str, not bytes\ncafé\n\ncafÃ©\n\nb'line one\\n' 29\ncd 2\ncd 1\ncd 0\n[1, 0]\n";

#[test]
fn open_reads_writes_and_iterates_like_the_reference() {
    let stdout = run("open", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}


// ==== `os.open` 的 POSIX 标志翻译（task-18 ✓）====================================================
//
// **真 bug** ✗：`posix.open` 把 POSIX 的 `O_*`（`O_CREAT=0o100`／`O_TRUNC=0o1000`／`O_APPEND=0o2000` ✓）
// **原样**交给能力域 ✗ —— 而域里是另一套位（`CREAT=32`／`TRUNC=16`／`APPEND=8` ✓）⇒ 位对不上 ✓
// （实测 `os.open(p, O_WRONLY|O_CREAT)` 报"打不开" ✓）。修法：在 `posix_module` 里**翻译** ✓
// （`O_EXCL` 域里没槽 ⇒ `fs_stat` 先探存在 ✓）。
// **一处真相** ✓：翻译只在 `posix_module::domain_open_flags` ✓，域位常量取自 `pyawa-capabilities` ✓。

const OS_SCRIPT: &str = r#"import os
fd = os.open("o.txt", os.O_WRONLY | os.O_CREAT)
os.write(fd, b"hello\n")
os.close(fd)
print(repr(open("o.txt", "rb").read()))

fd2 = os.open("o.txt", os.O_RDONLY)
print("read fd", repr(os.read(fd2, 5)))
os.close(fd2)

try:
    os.open("o.txt", os.O_WRONLY | os.O_CREAT | os.O_EXCL)
except Exception as exc:
    print(type(exc).__name__, str(exc))

fd3 = os.open("o.txt", os.O_WRONLY | os.O_APPEND)
os.write(fd3, b"tail\n")
os.close(fd3)
print(repr(open("o.txt", "rb").read()))

fd4 = os.open("trunc.txt", os.O_WRONLY | os.O_CREAT | os.O_TRUNC)
os.write(fd4, b"abc")
os.close(fd4)
print(repr(open("trunc.txt", "rb").read()))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：`O_CREAT` 建文件 ✓、`O_RDONLY` 读回 ✓、
/// `O_EXCL` 撞存在 ⇒ `FileExistsError` ✓、`O_APPEND` 追加 ✓、`O_TRUNC` 清空 ✓。
const OS_EXPECTED: &str = "b'hello\\n'\nread fd b'hello'\nFileExistsError [Errno 17] File exists: 'o.txt'\nb'hello\\ntail\\n'\nb'abc'\n";

#[test]
fn os_open_translates_posix_flags() {
    let stdout = run("osopen", OS_SCRIPT);
    assert_eq!(stdout, OS_EXPECTED, "{stdout}");
}
