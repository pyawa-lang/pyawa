//! **`_io` universal newline 读路径**（缺口3 ✓，2026-11-14）。
//!
//! **动因**（`docs/SPEC-c-modules.md` §5.2.33 的如实归因）：`io.TextIOWrapper(io.BytesIO(b"a,b\rc,d\r"),
//! newline="")` 先前给 `['a,b\rc,d\r']`，参照给 `['a,b\r', 'c,d\r']` ⇒ 上游 `test_csv.py` 的
//! `test_roundtrip_quoteed_newlines`／`test_roundtrip_escaped_unquoted_newlines`（`lineterminator='\r'`）
//! 报 `_csv.Error: new-line character seen in unquoted field`。
//!
//! **修法**（`crates/pyawa-stdlib/src/io_module.rs` 的 `NewlineMode`／`text_read_from`／
//! `text_readline_from`）：文本 IO（`open()` 文本档与 `_io.TextIOWrapper`）的 `read`／`readline`／
//! `readlines`／迭代按 `newline=` 口径在内存里断／译，`_data`（原文）与写侧一字不动。口径照本机
//! `python3` 3.14.4 实测（`target/recon/io_nl/` 逐字节对拍）：
//! - `newline=None`（两处默认）：`\r`／`\n`／`\r\n` 三档断行，读出**翻译**成 `\n`；
//! - `newline=''`：三档断行、**不翻译**（`\r\n` 原样两字符）；
//! - `newline='\n'`／`'\r'`／`'\r\n'`：**只按给出的那一个**断行、不翻译；
//! - `read(n)`：`None` 的 `n` 按**翻译后**字符数算（`\r\n` 计 1 字符、两原字符一起消费）；其余档按
//!   原字符数；`readline(size)` 可切在 `\r\n` 中间（`newline=''` 的 `readline(1)` 对 `\r\n` 给 `'\r'`，
//!   下一行给 `'\n'`；`None` 的同一格给 `'\n'`、整对已消费）；
//! - 尾行无换行照常返回；EOF 给 `''`；`read(0)`／`readline(0)` 不吃字符；
//! - `newline=` 校验文案照参照（`illegal newline value: …`／`argument 'newline' must be str or None …`／
//!   二进制档 `binary mode doesn't take a newline argument`；次序：类型 ⇒ 二进制 ⇒ 取值）。
//!
//! **如实边界**（本轮范围外 ✗，不影响上游 `test_csv`）：
//! - `newline=` 的**写**翻译未接（既有行为不回退：`newline=''` 与 Linux 上的 `newline=None` 都是恒等 ✓；
//!   显式 `newline='\r\n'` 成对写 `'\n'` 这一档仍不翻译 ✗）；
//! - `.newlines` 属性、`StringIO(newline=…)` 的翻译仍未接（后者见 `stringio_shapes.rs` 头注）；
//! - 文本 `tell`／`seek` 的位置仍是**原文字符下标**（本层既有口径，不是参照的 cookie）。
//!
//! 期望输出由本机 `python3`（3.14.4）跑**同一段脚本** `face.py` 取得（`target/recon/io_nl/face_ref/out.txt`）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-io-newline-{}-{name}", std::process::id()));
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

/// `TextIOWrapper`（内存 buffer）＋ `builtins.open`／`io.open`（真文件）的 universal newline 读口径，
/// 覆盖 `read`／`readline`／`readlines`／迭代／`size`／`\r\n` 边界／尾行无换行／校验文案。
const SCRIPT: &str = r#"import io

def show(label, value):
    print(label, repr(value))

def tw(data, nl):
    return io.TextIOWrapper(io.BytesIO(data), newline=nl)

# ---- 缺口3 头号抽例：newline='' 的孤立 \r 断行（不翻译）----
for nl in (None, "", "\n", "\r", "\r\n"):
    show("lines-" + repr(nl), tw(b"a,b\rc,d\r", nl).readlines())

# ---- 三档断行：None 翻译、'' 不翻译、显式档只认那一个 ----
data = b"a\r\nb\rc\nd"
for nl in (None, "", "\n", "\r", "\r\n"):
    show("all-" + repr(nl), tw(data, nl).readlines())

# ---- read(n) 跨界 + readline(size) ----
for nl in (None, "", "\r"):
    w = tw(b"a\r\nb\rc", nl)
    show("read-" + repr(nl), [w.read(1), w.read(1), w.read(2), w.read()])

for nl in (None, "", "\r\n"):
    w = tw(b"abc\r\ndef\r\n", nl)
    show("rl-" + repr(nl), [w.readline(2), w.readline(2), w.readline(2), w.readline(), w.readline()])

for nl in (None, "", "\r\n"):
    w = tw(b"\r\nx", nl)
    show("crlf-" + repr(nl), [w.readline(1), w.readline(1), w.readline(1)])

# ---- 尾行无换行 / EOF / 空输入 ----
for nl in (None, "", "\r"):
    w = tw(b"abc\r", nl)
    show("tail-" + repr(nl), [w.readline(), w.readline()])
    w = tw(b"", nl)
    show("empty-" + repr(nl), [w.readline(), w.read()])

# ---- TextIOWrapper 默认 newline=None ----
w = io.TextIOWrapper(io.BytesIO(b"x\r\ny\r"))
show("default", [w.readline(), w.readline(), w.readline()])

# ---- builtins.open 真文件（读）----
with open("data.bin", "wb") as f:
    f.write(b"one\r\ntwo\rthree\nfour")

for nl in (None, "", "\n", "\r", "\r\n"):
    with open("data.bin", "r", newline=nl) as f:
        show("file-" + repr(nl), [f.readline() for _ in range(5)])

for nl in (None, "", "\r"):
    with open("data.bin", "r", newline=nl) as f:
        show("fileiter-" + repr(nl), [line for line in f])
    with open("data.bin", "r", newline=nl) as f:
        show("fileread-" + repr(nl), [f.read(3), f.read(3), f.read()])
    with open("data.bin", "r", newline=nl) as f:
        show("filesize-" + repr(nl), [f.readline(4), f.readline(4), f.readline(4), f.readline()])

# ---- w+ 写完再读（写侧 identity；读侧按 newline 口径）----
with open("rw.txt", "w+", newline=None) as f:
    f.write("p\rq\r\nr\n")
    f.seek(0)
    show("wplus", [line for line in f])
with open("rw2.txt", "w+", newline="") as f:
    f.write("p\rq\r\nr\n")
    f.seek(0)
    show("wplus-empty", [line for line in f])
show("disk", open("rw2.txt", "rb").read())

# ---- 位置实参（第 6 位）／关键字 ----
print("posarg", [repr(x) for x in io.open("data.bin", "r", -1, None, None, "").readlines()])
print("kwarg", [repr(x) for x in io.open("data.bin", "r", newline="").readlines()])

# ---- 校验文案（照参照）----
for arg in ("x", 1, b"\n", "\n\r"):
    try:
        io.TextIOWrapper(io.BytesIO(b""), newline=arg)
        print("bad-tw", repr(arg), "ok")
    except Exception as e:
        print("bad-tw", repr(arg), type(e).__name__, "|" + repr(str(e)) + "|")
for arg in ("x", 1, "\n\r"):
    try:
        open("data.bin", newline=arg)
    except Exception as e:
        print("bad-open", repr(arg), type(e).__name__, "|" + repr(str(e)) + "|")
try:
    open("data.bin", "rb", newline="\n")
except Exception as e:
    print("bad-bin", type(e).__name__, "|" + repr(str(e)) + "|")
"#;

/// 参照（`python3` 3.14.4 跑 `SCRIPT` 实测 ✓，逐行）。
const EXPECTED: &str = r#"lines-None ['a,b\n', 'c,d\n']
lines-'' ['a,b\r', 'c,d\r']
lines-'\n' ['a,b\rc,d\r']
lines-'\r' ['a,b\r', 'c,d\r']
lines-'\r\n' ['a,b\rc,d\r']
all-None ['a\n', 'b\n', 'c\n', 'd']
all-'' ['a\r\n', 'b\r', 'c\n', 'd']
all-'\n' ['a\r\n', 'b\rc\n', 'd']
all-'\r' ['a\r', '\nb\r', 'c\nd']
all-'\r\n' ['a\r\n', 'b\rc\nd']
read-None ['a', '\n', 'b\n', 'c']
read-'' ['a', '\r', '\nb', '\rc']
read-'\r' ['a', '\r', '\nb', '\rc']
rl-None ['ab', 'c\n', 'de', 'f\n', '']
rl-'' ['ab', 'c\r', '\n', 'def\r\n', '']
rl-'\r\n' ['ab', 'c\r', '\nd', 'ef\r\n', '']
crlf-None ['\n', 'x', '']
crlf-'' ['\r', '\n', 'x']
crlf-'\r\n' ['\r', '\n', 'x']
tail-None ['abc\n', '']
empty-None ['', '']
tail-'' ['abc\r', '']
empty-'' ['', '']
tail-'\r' ['abc\r', '']
empty-'\r' ['', '']
default ['x\n', 'y\n', '']
file-None ['one\n', 'two\n', 'three\n', 'four', '']
file-'' ['one\r\n', 'two\r', 'three\n', 'four', '']
file-'\n' ['one\r\n', 'two\rthree\n', 'four', '', '']
file-'\r' ['one\r', '\ntwo\r', 'three\nfour', '', '']
file-'\r\n' ['one\r\n', 'two\rthree\nfour', '', '', '']
fileiter-None ['one\n', 'two\n', 'three\n', 'four']
fileread-None ['one', '\ntw', 'o\nthree\nfour']
filesize-None ['one\n', 'two\n', 'thre', 'e\n']
fileiter-'' ['one\r\n', 'two\r', 'three\n', 'four']
fileread-'' ['one', '\r\nt', 'wo\rthree\nfour']
filesize-'' ['one\r', '\n', 'two\r', 'three\n']
fileiter-'\r' ['one\r', '\ntwo\r', 'three\nfour']
fileread-'\r' ['one', '\r\nt', 'wo\rthree\nfour']
filesize-'\r' ['one\r', '\ntwo', '\r', 'three\nfour']
wplus ['p\n', 'q\n', 'r\n']
wplus-empty ['p\r', 'q\r\n', 'r\n']
disk b'p\rq\r\nr\n'
posarg ["'one\\r\\n'", "'two\\r'", "'three\\n'", "'four'"]
kwarg ["'one\\r\\n'", "'two\\r'", "'three\\n'", "'four'"]
bad-tw 'x' ValueError |'illegal newline value: x'|
bad-tw 1 TypeError |"TextIOWrapper() argument 'newline' must be str or None, not int"|
bad-tw b'\n' TypeError |"TextIOWrapper() argument 'newline' must be str or None, not bytes"|
bad-tw '\n\r' ValueError |'illegal newline value: \n\r'|
bad-open 'x' ValueError |'illegal newline value: x'|
bad-open 1 TypeError |"open() argument 'newline' must be str or None, not int"|
bad-open '\n\r' ValueError |'illegal newline value: \n\r'|
bad-bin ValueError |"binary mode doesn't take a newline argument"|
"#;

#[test]
fn universal_newline_reads_agree_with_the_reference() {
    let stdout = run("face", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
