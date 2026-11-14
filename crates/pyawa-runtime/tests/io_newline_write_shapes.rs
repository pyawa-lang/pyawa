//! **`_io` newline 写翻译／`.newlines`／`StringIO(newline=)`**（缺口3 第 2 轮 ✓，2026-11-14）。
//!
//! **动因**：上一轮只接了 `newline=` 的**读**口径（`crates/pyawa-stdlib/src/io_module.rs` 的
//! `NewlineMode`／`text_read_from`／`text_readline_from`）。本轮补：
//! ①**写**翻译（`open(..., 'w', newline=…)`／`TextIOWrapper.write`／`StringIO.write`）；
//! ②`.newlines` 属性（未读 ⇒ `None`；一种 ⇒ 该串；多种 ⇒ 元组，次序 CR、LF、CRLF）；
//! ③`StringIO(initial_value, newline=…)` 构造收档、读／写同口径。
//!
//! **口径来源**：本机 `python3` 3.14.4 实测（`target/recon/io_nl/` 的 `probe_write.py`／
//! `probe_newlines.py`／`probe_stringio_nl.py`，`run_all.sh` 逐字节 diff 无输出；期望输出就是同一段
//! 脚本在本机参照上的 stdout ✓）。
//!
//! **写翻译要点**（Linux 上 `os.linesep == '\n'`）：
//! - `open`／`TextIOWrapper`：`None` ⇒ 恒等（只译 `'\n'`）；`''`／`'\n'` ⇒ 恒等；
//!   `'\r'`／`'\r\n'` ⇒ **每个** `'\n'` 换成对应串（既有 `'\r'` 不配对 ⇒ `"\r\n"` 变 `"\r\r"`）；
//! - `StringIO`：`None` ⇒ **三档**换行（`\r\n`／`\r`／`\n`）都译成 `'\n'`（与 `TextIOWrapper` 不同 ✓）；
//!   `''`／`'\n'` ⇒ 恒等；`'\r'`／`'\r\n'` 同上；初始值也按写口径翻译。
//!
//! **`.newlines` 口径**：只有 `newline=None`／`''` 两档追踪；**首次真读**时扫整块
//! （`read(0)` 不触发、`readline(0)` 触发 ✓）；写不追踪；`StringIO` 在构造／写时从实参原文累计。
//!
//! **如实边界** ✗：
//! - `.newlines` 的追踪是**整块**口径（本层整个文件／缓冲都在内存；参照按 8192 字节块解码）；
//! - 文本流 `tell` 仍是**字符下标**（参照是字节 cookie）⇒ 纯 ASCII 一致，非 ASCII 写后偏小；
//! - `TextIOWrapper` 的 `seek`／`tell` 委派底层二进制层（文本层自己的 cookie 未建模）；
//! - `.newlines` 只读（赋值报 `AttributeError` ✓）与二进制档的 `AttributeError` **文案**受
//!   "本层 `open()` 一律返回 `FileIO`" 的既有边界影响（**异常类型一致**、消息不同 ✗）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-io-newline-write-{}-{name}", std::process::id()));
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

/// 写翻译：`TextIOWrapper`（BytesIO 底层，含 `writelines`／错误文案／非 ASCII）＋ `open()`
/// 真文件（`w`／`a`／`w+`）五档 `newline=`、`w+` 回读、写模式 `.newlines` 恒 `None`。
const WRITE_SCRIPT: &str = r#"import io

def show(label, value):
    print(label, repr(value))

# ==== 一、TextIOWrapper 写翻译（底层 BytesIO）====
payload = "a\nb\r\nc\rd\n"
for nl in (None, "", "\n", "\r", "\r\n"):
    b = io.BytesIO()
    w = io.TextIOWrapper(b, newline=nl)
    n = w.write(payload)
    t = w.tell()
    w.flush()
    show("tw-write-" + repr(nl), (n, t, b.getvalue(), w.newlines))

# 分多次 write
for nl in (None, "", "\r", "\r\n"):
    b = io.BytesIO()
    w = io.TextIOWrapper(b, newline=nl)
    a = w.write("x\n")
    c = w.write("\ny")
    w.flush()
    show("tw-split-" + repr(nl), (a, c, b.getvalue()))

# 空串 / 只有 \r 的写法
for nl in (None, "\r", "\r\n"):
    b = io.BytesIO()
    w = io.TextIOWrapper(b, newline=nl)
    n = w.write("")
    w.write("\r")
    w.flush()
    show("tw-empty-" + repr(nl), (n, b.getvalue()))

# writelines
b = io.BytesIO()
w = io.TextIOWrapper(b, newline="\r\n")
r = w.writelines(["a\n", "b\n"])
w.flush()
show("tw-writelines", (r, b.getvalue(), w.newlines))

# 写后 seek(0) 回读（写翻译已存档；读再按 newline 口径）
b = io.BytesIO()
w = io.TextIOWrapper(b, newline=None)
w.write("x\ry\n")
w.flush()
w.seek(0)
show("tw-write-read", (w.read(), w.newlines))

# 非 ASCII 写：返回字符数、tell 是底层字节位置
b = io.BytesIO()
w = io.TextIOWrapper(b, newline="\r\n")
n = w.write("\u4e2d\n")
w.flush()
show("tw-write-unicode", (n, w.tell(), b.getvalue()))

# ==== 二、TextIOWrapper.write 错误面 ====
t = io.TextIOWrapper(io.BytesIO())
try:
    t.write()
except Exception as e:
    show("tw-write-noarg", (type(e).__name__, str(e)))
try:
    t.write(b"x")
except Exception as e:
    show("tw-write-bytes", (type(e).__name__, str(e)))
try:
    t.write("a", "b")
except Exception as e:
    show("tw-write-2args", (type(e).__name__, str(e)))
try:
    t.write(x="a")
except Exception as e:
    show("tw-write-kw", (type(e).__name__, str(e)))
t.close()
try:
    t.write("x")
except Exception as e:
    show("tw-write-closed", (type(e).__name__, str(e)))

# ==== 三、open() 写翻译（真文件）====
for nl in (None, "", "\n", "\r", "\r\n"):
    with open("wout.txt", "w", newline=nl) as f:
        n = f.write(payload)
        t = f.tell()
    show("file-write-" + repr(nl), (n, t, open("wout.txt", "rb").read()))

for nl in (None, "\r", "\r\n"):
    with open("wout2.txt", "w", newline=nl) as f:
        f.write("x\n")
        f.write("\ny")
    show("file-split-" + repr(nl), open("wout2.txt", "rb").read())

# append 模式也按 newline 翻译
for nl in (None, "\r\n"):
    with open("app.txt", "wb") as f:
        f.write(b"head\n")
    with open("app.txt", "a", newline=nl) as f:
        f.write("tail\n")
    show("file-append-" + repr(nl), open("app.txt", "rb").read())

# w+ 写后回读：写侧已翻译落 _data，读侧按 newline 口径断行
for nl in (None, "", "\r", "\r\n"):
    with open("rw.txt", "w+", newline=nl) as f:
        f.write(payload)
        t = f.tell()
        f.seek(0)
        lines = [line for line in f]
    show("wplus-" + repr(nl), (t, lines, open("rw.txt", "rb").read()))

# 写模式的 .newlines 恒为 None（读才追踪）
for nl in (None, "", "\r", "\r\n"):
    with open("w.txt", "w", newline=nl) as f:
        before = f.newlines
        f.write("a\nb")
        show("file-write-newlines-" + repr(nl), (before, f.newlines))
"#;

/// 参照（本机 `python3` 3.14.4 实跑，逐行）。
const WRITE_SCRIPT_EXPECTED: &str = r#"tw-write-None (9, 9, b'a\nb\r\nc\rd\n', None)
tw-write-'' (9, 9, b'a\nb\r\nc\rd\n', None)
tw-write-'\n' (9, 9, b'a\nb\r\nc\rd\n', None)
tw-write-'\r' (9, 9, b'a\rb\r\rc\rd\r', None)
tw-write-'\r\n' (9, 12, b'a\r\nb\r\r\nc\rd\r\n', None)
tw-split-None (2, 2, b'x\n\ny')
tw-split-'' (2, 2, b'x\n\ny')
tw-split-'\r' (2, 2, b'x\r\ry')
tw-split-'\r\n' (2, 2, b'x\r\n\r\ny')
tw-empty-None (0, b'\r')
tw-empty-'\r' (0, b'\r')
tw-empty-'\r\n' (0, b'\r')
tw-writelines (None, b'a\r\nb\r\n', None)
tw-write-read ('x\ny\n', ('\r', '\n'))
tw-write-unicode (2, 5, b'\xe4\xb8\xad\r\n')
tw-write-noarg ('TypeError', 'TextIOWrapper.write() takes exactly one argument (0 given)')
tw-write-bytes ('TypeError', 'write() argument must be str, not bytes')
tw-write-2args ('TypeError', 'TextIOWrapper.write() takes exactly one argument (2 given)')
tw-write-kw ('TypeError', 'TextIOWrapper.write() takes no keyword arguments')
tw-write-closed ('ValueError', 'I/O operation on closed file.')
file-write-None (9, 9, b'a\nb\r\nc\rd\n')
file-write-'' (9, 9, b'a\nb\r\nc\rd\n')
file-write-'\n' (9, 9, b'a\nb\r\nc\rd\n')
file-write-'\r' (9, 9, b'a\rb\r\rc\rd\r')
file-write-'\r\n' (9, 12, b'a\r\nb\r\r\nc\rd\r\n')
file-split-None b'x\n\ny'
file-split-'\r' b'x\r\ry'
file-split-'\r\n' b'x\r\n\r\ny'
file-append-None b'head\ntail\n'
file-append-'\r\n' b'head\ntail\r\n'
wplus-None (9, ['a\n', 'b\n', 'c\n', 'd\n'], b'a\nb\r\nc\rd\n')
wplus-'' (9, ['a\n', 'b\r\n', 'c\r', 'd\n'], b'a\nb\r\nc\rd\n')
wplus-'\r' (9, ['a\r', 'b\r', '\r', 'c\r', 'd\r'], b'a\rb\r\rc\rd\r')
wplus-'\r\n' (12, ['a\r\n', 'b\r\r\n', 'c\rd\r\n'], b'a\r\nb\r\r\nc\rd\r\n')
file-write-newlines-None (None, None)
file-write-newlines-'' (None, None)
file-write-newlines-'\r' (None, None)
file-write-newlines-'\r\n' (None, None)
"#;

#[test]
fn write_translation_matches_the_reference() {
    assert_eq!(run("write_translation", WRITE_SCRIPT), WRITE_SCRIPT_EXPECTED);
}

/// `.newlines`：`open()`／`TextIOWrapper` 五档、单一种类、无换行、`read(0)` 不触发 vs
/// `readline(0)` 触发、部分读的整块口径、写不追踪、`w+` 回读、关闭后仍可读、二进制档无此属性。
const NEWLINES_SCRIPT: &str = r#"import io

def show(label, value):
    print(label, repr(value))

DATA = b"a\r\nb\rc\nd"

# ==== 一、open() 文本读：.newlines 追踪 ====
for nl in (None, "", "\n", "\r", "\r\n"):
    with open("data.bin", "wb") as f:
        f.write(DATA)
    with open("data.bin", "r", newline=nl) as f:
        show("open-before-" + repr(nl), f.newlines)
        show("open-rl1-" + repr(nl), f.readline())
        show("open-after1-" + repr(nl), f.newlines)
        show("open-rest-" + repr(nl), f.read())
        show("open-after-" + repr(nl), f.newlines)
        f.seek(0)
        f.read()
        show("open-seek-" + repr(nl), f.newlines)

# 单一换行种类 × 读档（None／'' 追踪，显式档不追踪）
for kind, data in (("lf", b"a\nb\nc"), ("crlf", b"a\r\nb\r\nc"), ("cr", b"a\rb\rc")):
    for nl in (None, "", "\r"):
        with open("data.bin", "wb") as f:
            f.write(data)
        with open("data.bin", "r", newline=nl) as f:
            f.read()
            show("open-single-" + kind + "-" + repr(nl), f.newlines)

# 无换行／空文件
for nl in (None, ""):
    with open("data.bin", "wb") as f:
        f.write(b"abc")
    with open("data.bin", "r", newline=nl) as f:
        f.read()
        show("open-nonewline-" + repr(nl), f.newlines)

# read(0) 不触发；readline(0) 触发；迭代／readlines 触发
with open("data.bin", "wb") as f:
    f.write(DATA)
for nl in (None, ""):
    with open("data.bin", "r", newline=nl) as f:
        show("trig-r0-" + repr(nl), (f.read(0), f.newlines))
    with open("data.bin", "r", newline=nl) as f:
        show("trig-rl0-" + repr(nl), (f.readline(0), f.newlines))
    with open("data.bin", "r", newline=nl) as f:
        show("trig-lines-" + repr(nl), (f.readlines(), f.newlines))
    with open("data.bin", "r", newline=nl) as f:
        show("trig-iter-" + repr(nl), ([line for line in f], f.newlines))

# readline 切在 \r\n 中间：追踪是整块（不是只到已消费位置）
for nl in (None, ""):
    with open("data.bin", "wb") as f:
        f.write(b"a\r\nb")
    with open("data.bin", "r", newline=nl) as f:
        show("cut-rl1-" + repr(nl), (f.readline(2), f.newlines))
        show("cut-rl2-" + repr(nl), (f.readline(1), f.newlines))
        show("cut-rl3-" + repr(nl), (f.readline(), f.newlines))

# 部分读后 .newlines 已是整块口径
for nl in (None, ""):
    with open("data.bin", "wb") as f:
        f.write(DATA)
    with open("data.bin", "r", newline=nl) as f:
        show("part-before-" + repr(nl), f.newlines)
        show("part-read1-" + repr(nl), (f.read(1), f.newlines))

# ==== 二、TextIOWrapper .newlines ====
for nl in (None, "", "\n", "\r", "\r\n"):
    t = io.TextIOWrapper(io.BytesIO(DATA), newline=nl)
    show("tw-before-" + repr(nl), t.newlines)
    show("tw-rl-" + repr(nl), t.readline())
    show("tw-after-" + repr(nl), t.newlines)
    show("tw-rest-" + repr(nl), t.read())
    show("tw-end-" + repr(nl), t.newlines)

for nl in (None, ""):
    t = io.TextIOWrapper(io.BytesIO(b"a\r\nb"), newline=nl)
    show("tw-rl0-" + repr(nl), (t.readline(0), t.newlines))
    t = io.TextIOWrapper(io.BytesIO(b"a\r\nb"), newline=nl)
    show("tw-r0-" + repr(nl), (t.read(0), t.newlines))

# 写不改变 .newlines（写模式 wrapper）
for nl in (None, "\r\n"):
    b = io.BytesIO()
    t = io.TextIOWrapper(b, newline=nl)
    t.write("a\nb")
    t.flush()
    show("tw-write-nl-" + repr(nl), (t.newlines, b.getvalue()))

# 写后 seek(0) 读：追踪由读触发
b = io.BytesIO()
t = io.TextIOWrapper(b, newline=None)
t.write("p\rq\n")
t.flush()
t.seek(0)
show("tw-write-read-" + repr(None), (t.read(), t.newlines))

# ==== 三、w+ 写后回读的 .newlines（写侧翻译、读侧整块追踪）====
for nl in (None, "", "\r"):
    with open("data.bin", "w+", newline=nl) as f:
        f.write("a\nb\r\nc")
        f.seek(0)
        show("wplus-nl-" + repr(nl), (f.read(), f.newlines))

# TextIOWrapper 关闭后 .newlines 仍可读（照参照 ✓）
t = io.TextIOWrapper(io.BytesIO(b"a\r\nb"))
t.read()
t.close()
show("tw-closed-newlines", t.newlines)

# 二进制档没有 .newlines（异常类型一致；文案受本层 open 一律 FileIO 的既有边界影响 ⇒ 只记类型）
with open("data.bin", "wb") as f:
    f.write(b"x")
with open("data.bin", "rb") as f:
    try:
        show("binary-newlines", f.newlines)
    except Exception as e:
        show("binary-newlines", type(e).__name__)
"#;

/// 参照（本机 `python3` 3.14.4 实跑，逐行）。
const NEWLINES_SCRIPT_EXPECTED: &str = r#"open-before-None None
open-rl1-None 'a\n'
open-after1-None ('\r', '\n', '\r\n')
open-rest-None 'b\nc\nd'
open-after-None ('\r', '\n', '\r\n')
open-seek-None ('\r', '\n', '\r\n')
open-before-'' None
open-rl1-'' 'a\r\n'
open-after1-'' ('\r', '\n', '\r\n')
open-rest-'' 'b\rc\nd'
open-after-'' ('\r', '\n', '\r\n')
open-seek-'' ('\r', '\n', '\r\n')
open-before-'\n' None
open-rl1-'\n' 'a\r\n'
open-after1-'\n' None
open-rest-'\n' 'b\rc\nd'
open-after-'\n' None
open-seek-'\n' None
open-before-'\r' None
open-rl1-'\r' 'a\r'
open-after1-'\r' None
open-rest-'\r' '\nb\rc\nd'
open-after-'\r' None
open-seek-'\r' None
open-before-'\r\n' None
open-rl1-'\r\n' 'a\r\n'
open-after1-'\r\n' None
open-rest-'\r\n' 'b\rc\nd'
open-after-'\r\n' None
open-seek-'\r\n' None
open-single-lf-None '\n'
open-single-lf-'' '\n'
open-single-lf-'\r' None
open-single-crlf-None '\r\n'
open-single-crlf-'' '\r\n'
open-single-crlf-'\r' None
open-single-cr-None '\r'
open-single-cr-'' '\r'
open-single-cr-'\r' None
open-nonewline-None None
open-nonewline-'' None
trig-r0-None ('', None)
trig-rl0-None ('', ('\r', '\n', '\r\n'))
trig-lines-None (['a\n', 'b\n', 'c\n', 'd'], ('\r', '\n', '\r\n'))
trig-iter-None (['a\n', 'b\n', 'c\n', 'd'], ('\r', '\n', '\r\n'))
trig-r0-'' ('', None)
trig-rl0-'' ('', ('\r', '\n', '\r\n'))
trig-lines-'' (['a\r\n', 'b\r', 'c\n', 'd'], ('\r', '\n', '\r\n'))
trig-iter-'' (['a\r\n', 'b\r', 'c\n', 'd'], ('\r', '\n', '\r\n'))
cut-rl1-None ('a\n', '\r\n')
cut-rl2-None ('b', '\r\n')
cut-rl3-None ('', '\r\n')
cut-rl1-'' ('a\r', '\r\n')
cut-rl2-'' ('\n', '\r\n')
cut-rl3-'' ('b', '\r\n')
part-before-None None
part-read1-None ('a', ('\r', '\n', '\r\n'))
part-before-'' None
part-read1-'' ('a', ('\r', '\n', '\r\n'))
tw-before-None None
tw-rl-None 'a\n'
tw-after-None ('\r', '\n', '\r\n')
tw-rest-None 'b\nc\nd'
tw-end-None ('\r', '\n', '\r\n')
tw-before-'' None
tw-rl-'' 'a\r\n'
tw-after-'' ('\r', '\n', '\r\n')
tw-rest-'' 'b\rc\nd'
tw-end-'' ('\r', '\n', '\r\n')
tw-before-'\n' None
tw-rl-'\n' 'a\r\n'
tw-after-'\n' None
tw-rest-'\n' 'b\rc\nd'
tw-end-'\n' None
tw-before-'\r' None
tw-rl-'\r' 'a\r'
tw-after-'\r' None
tw-rest-'\r' '\nb\rc\nd'
tw-end-'\r' None
tw-before-'\r\n' None
tw-rl-'\r\n' 'a\r\n'
tw-after-'\r\n' None
tw-rest-'\r\n' 'b\rc\nd'
tw-end-'\r\n' None
tw-rl0-None ('', '\r\n')
tw-r0-None ('', None)
tw-rl0-'' ('', '\r\n')
tw-r0-'' ('', None)
tw-write-nl-None (None, b'a\nb')
tw-write-nl-'\r\n' (None, b'a\r\nb')
tw-write-read-None ('p\nq\n', ('\r', '\n'))
wplus-nl-None ('a\nb\nc', ('\n', '\r\n'))
wplus-nl-'' ('a\nb\r\nc', ('\n', '\r\n'))
wplus-nl-'\r' ('a\rb\r\rc', None)
tw-closed-newlines '\r\n'
binary-newlines 'AttributeError'
"#;

#[test]
fn newlines_attribute_matches_the_reference() {
    assert_eq!(run("newlines_attribute", NEWLINES_SCRIPT), NEWLINES_SCRIPT_EXPECTED);
}

/// `StringIO(initial_value, newline=…)`：构造档／初始值翻译／写翻译／读口径／`tell`／
/// `.newlines`（含关闭后 `ValueError`）／校验文案。
const STRINGIO_SCRIPT: &str = r#"import io

def show(label, value):
    print(label, repr(value))

# ==== 一、构造签名与默认档 ====
s = io.StringIO()
show("default", (s.getvalue(), s.newlines, s.tell()))
s = io.StringIO(newline=None)
show("none-kw", (s.getvalue(), s.newlines))
s = io.StringIO(None, newline=None)
show("none-arg", (s.getvalue(), s.newlines))

# 第二个位置参数就是 newline
s = io.StringIO("a\nb", "\r")
show("pos-newline", s.getvalue())
s = io.StringIO(initial_value="a\nb", newline="\r\n")
show("kw-newline", (s.getvalue(), s.read()))

# ==== 二、写翻译（五档）====
payload = "a\nb\r\nc\rd\n"
for nl in (None, "", "\n", "\r", "\r\n"):
    s = io.StringIO(newline=nl)
    n = s.write(payload)
    show("write-" + repr(nl), (n, s.getvalue(), s.tell(), s.newlines))

# 分次写
for nl in (None, "\r", "\r\n"):
    s = io.StringIO(newline=nl)
    a = s.write("x\n")
    c = s.write("\ny")
    show("split-" + repr(nl), (a, c, s.getvalue()))

# 越界写：'\x00' 补齐后按翻译长度推进位置
for nl in (None, "\r\n"):
    s = io.StringIO(newline=nl)
    s.seek(3)
    n = s.write("a\r\nb")
    show("pad-" + repr(nl), (n, s.getvalue(), s.tell(), s.newlines))

# ==== 三、初始值翻译（五档，含追踪）====
for nl in (None, "", "\n", "\r", "\r\n"):
    s = io.StringIO("a\nb\r\nc\rd", newline=nl)
    show("init-" + repr(nl), (s.getvalue(), s.newlines, s.tell(), s.readlines(), s.newlines))

# 单一换行种类 × 构造档
for kind, value in (("lf", "a\nb\nc"), ("crlf", "a\r\nb\r\nc"), ("cr", "a\rb\rc")):
    for nl in (None, "", "\r"):
        s = io.StringIO(value, newline=nl)
        show("single-" + kind + "-" + repr(nl), (s.getvalue(), s.read(), s.newlines))

# ==== 四、读口径 ====
for nl in (None, "", "\n", "\r", "\r\n"):
    s = io.StringIO("a\r\nb\rc\nd", newline=nl)
    show("read-" + repr(nl), (s.readline(), s.readline(), s.readline(), s.readline(), s.newlines))

# read(n) 与翻译后计数
for nl in (None, "", "\r"):
    s = io.StringIO("a\r\nb", newline=nl)
    show("readn-" + repr(nl), (s.read(1), s.read(1), s.read(1), s.read(1), s.tell()))

# readlines sizehint
for nl in (None, ""):
    s = io.StringIO("a\rb\r\nc\nd", newline=nl)
    show("readlines-" + repr(nl), (s.readlines(3), s.read(), s.newlines))

# 迭代
for nl in (None, ""):
    s = io.StringIO("a\rb\r\nc\nd", newline=nl)
    show("iter-" + repr(nl), (list(s), s.newlines))

# ==== 五、写后 seek(0) 回读 ====
for nl in (None, "", "\r", "\r\n"):
    s = io.StringIO(newline=nl)
    s.write(payload)
    s.seek(0)
    show("rt-" + repr(nl), (s.readline(), s.readline(), s.readline(), s.readline(), s.newlines))

# 读后写：位置与覆盖
s = io.StringIO("abc", newline=None)
show("read-write", (s.read(1), s.tell(), s.write("X"), s.getvalue()))

# 初始值 + 追加写：追踪累积
s = io.StringIO("a\nb", newline=None)
s.seek(0, 2)
s.write("c\r\nd")
show("accum", (s.getvalue(), s.newlines))

# truncate 不清追踪
s = io.StringIO("a\r\nb", newline="")
s.truncate(1)
show("truncate", (s.getvalue(), s.newlines))

# ==== 六、校验与错误文案 ====
for arg in ("x", 1, b"\n", "\n\r", "\r\r"):
    try:
        io.StringIO("", newline=arg)
        show("nl-ok-" + repr(arg), "ok")
    except Exception as e:
        show("nl-bad-" + repr(arg), (type(e).__name__, str(e)))

for value in (b"x", 1, [], None):
    try:
        s = io.StringIO(value)
        show("init-ok-" + repr(value), s.getvalue())
    except Exception as e:
        show("init-bad-" + repr(value), (type(e).__name__, str(e)))

# 初始值类型错 + newline 非法：先判 newline
try:
    io.StringIO(b"x", newline="x")
except Exception as e:
    show("both-bad", (type(e).__name__, str(e)))

try:
    io.StringIO("", "\n", "x")
except Exception as e:
    show("3args", (type(e).__name__, str(e)))
try:
    io.StringIO("a", bogus=1)
except Exception as e:
    show("bogus-kw", (type(e).__name__, str(e)))

# 写非 str
s = io.StringIO()
try:
    s.write(b"x")
except Exception as e:
    show("write-bytes", (type(e).__name__, str(e)))

# 关闭后 newlines（参照：ValueError，无句点）
s = io.StringIO("a\nb")
s.close()
try:
    show("closed-newlines", s.newlines)
except Exception as e:
    show("closed-newlines", (type(e).__name__, str(e)))

# 属性面
s = io.StringIO("abc")
show("props", (s.encoding, s.errors, s.line_buffering, s.newlines))
"#;

/// 参照（本机 `python3` 3.14.4 实跑，逐行）。
const STRINGIO_SCRIPT_EXPECTED: &str = r#"default ('', None, 0)
none-kw ('', None)
none-arg ('', None)
pos-newline 'a\rb'
kw-newline ('a\r\nb', 'a\r\nb')
write-None (9, 'a\nb\nc\nd\n', 8, ('\r', '\n', '\r\n'))
write-'' (9, 'a\nb\r\nc\rd\n', 9, ('\r', '\n', '\r\n'))
write-'\n' (9, 'a\nb\r\nc\rd\n', 9, None)
write-'\r' (9, 'a\rb\r\rc\rd\r', 9, None)
write-'\r\n' (9, 'a\r\nb\r\r\nc\rd\r\n', 12, None)
split-None (2, 2, 'x\n\ny')
split-'\r' (2, 2, 'x\r\ry')
split-'\r\n' (2, 2, 'x\r\n\r\ny')
pad-None (4, '\x00\x00\x00a\nb', 6, '\r\n')
pad-'\r\n' (4, '\x00\x00\x00a\r\r\nb', 8, None)
init-None ('a\nb\nc\nd', ('\r', '\n', '\r\n'), 0, ['a\n', 'b\n', 'c\n', 'd'], ('\r', '\n', '\r\n'))
init-'' ('a\nb\r\nc\rd', ('\r', '\n', '\r\n'), 0, ['a\n', 'b\r\n', 'c\r', 'd'], ('\r', '\n', '\r\n'))
init-'\n' ('a\nb\r\nc\rd', None, 0, ['a\n', 'b\r\n', 'c\rd'], None)
init-'\r' ('a\rb\r\rc\rd', None, 0, ['a\r', 'b\r', '\r', 'c\r', 'd'], None)
init-'\r\n' ('a\r\nb\r\r\nc\rd', None, 0, ['a\r\n', 'b\r\r\n', 'c\rd'], None)
single-lf-None ('a\nb\nc', 'a\nb\nc', '\n')
single-lf-'' ('a\nb\nc', 'a\nb\nc', '\n')
single-lf-'\r' ('a\rb\rc', 'a\rb\rc', None)
single-crlf-None ('a\nb\nc', 'a\nb\nc', '\r\n')
single-crlf-'' ('a\r\nb\r\nc', 'a\r\nb\r\nc', '\r\n')
single-crlf-'\r' ('a\r\rb\r\rc', 'a\r\rb\r\rc', None)
single-cr-None ('a\nb\nc', 'a\nb\nc', '\r')
single-cr-'' ('a\rb\rc', 'a\rb\rc', '\r')
single-cr-'\r' ('a\rb\rc', 'a\rb\rc', None)
read-None ('a\n', 'b\n', 'c\n', 'd', ('\r', '\n', '\r\n'))
read-'' ('a\r\n', 'b\r', 'c\n', 'd', ('\r', '\n', '\r\n'))
read-'\n' ('a\r\n', 'b\rc\n', 'd', '', None)
read-'\r' ('a\r', '\r', 'b\r', 'c\r', None)
read-'\r\n' ('a\r\r\n', 'b\rc\r\n', 'd', '', None)
readn-None ('a', '\n', 'b', '', 3)
readn-'' ('a', '\r', '\n', 'b', 4)
readn-'\r' ('a', '\r', '\r', 'b', 4)
readlines-None (['a\n', 'b\n'], 'c\nd', ('\r', '\n', '\r\n'))
readlines-'' (['a\r', 'b\r\n'], 'c\nd', ('\r', '\n', '\r\n'))
iter-None (['a\n', 'b\n', 'c\n', 'd'], ('\r', '\n', '\r\n'))
iter-'' (['a\r', 'b\r\n', 'c\n', 'd'], ('\r', '\n', '\r\n'))
rt-None ('a\n', 'b\n', 'c\n', 'd\n', ('\r', '\n', '\r\n'))
rt-'' ('a\n', 'b\r\n', 'c\r', 'd\n', ('\r', '\n', '\r\n'))
rt-'\r' ('a\r', 'b\r', '\r', 'c\r', None)
rt-'\r\n' ('a\r\n', 'b\r\r\n', 'c\rd\r\n', '', None)
read-write ('a', 1, 1, 'aXc')
accum ('a\nbc\nd', ('\n', '\r\n'))
truncate ('a', '\r\n')
nl-bad-'x' ('ValueError', "illegal newline value: 'x'")
nl-bad-1 ('TypeError', 'newline must be str or None, not int')
nl-bad-b'\n' ('TypeError', 'newline must be str or None, not bytes')
nl-bad-'\n\r' ('ValueError', "illegal newline value: '\\n\\r'")
nl-bad-'\r\r' ('ValueError', "illegal newline value: '\\r\\r'")
init-bad-b'x' ('TypeError', 'initial_value must be str or None, not bytes')
init-bad-1 ('TypeError', 'initial_value must be str or None, not int')
init-bad-[] ('TypeError', 'initial_value must be str or None, not list')
init-ok-None ''
both-bad ('ValueError', "illegal newline value: 'x'")
3args ('TypeError', 'StringIO() takes at most 2 arguments (3 given)')
bogus-kw ('TypeError', "StringIO() got an unexpected keyword argument 'bogus'")
write-bytes ('TypeError', "string argument expected, got 'bytes'")
closed-newlines ('ValueError', 'I/O operation on closed file')
props (None, None, False, None)
"#;

#[test]
fn stringio_newline_parameter_matches_the_reference() {
    assert_eq!(run("stringio_newline_parameter", STRINGIO_SCRIPT), STRINGIO_SCRIPT_EXPECTED);
}
