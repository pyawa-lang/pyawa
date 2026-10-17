//! **f-string 相邻链／`ascii()`／`TextIOWrapper` 读路径**（task-19 第 3 轮 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓（脚本见各 `const` ✓）。
//!
//! 被这组抽例钉住的真 bug：
//! ① **普通串 ＋ 两枚以上 f-string** ✗：`parse_atom` 的 `Str` 臂只吃**一枚**后随 f-string ✗
//!    ⇒ `('Field(' f'name={…!r},' f'type={…!r},' ')')` 的第二枚留在原地 ⇒ 括号那一组报
//!    「括号没有闭合，实际 Some(FStr …)」✗（`Lib/dataclasses.py:314` 正是这个形状 ✓）；
//! ② **词法器减法溢出 panic** ✗：单字符前缀（`f'…'` ⇒ `index＝2` ✓）走到 `index - 3` 求 `r` 前缀 ⇒
//!    调试构建下直接 panic ✓（`f"{'a'}"` 的子词法当场炸 ✓）⇒ 改 `checked_sub` ✓；
//! ③ **`ascii()` 未接线** ✗（`NameError` ✓）⇒ `repr` ＋ 非 ASCII 转义 ✓；
//! ④ **`_io.TextIOWrapper` 构造不出** ✗（占位类型 `new` 槽拒实参 ✓）⇒ 真类 ＋ 读路径 ✓
//!    （`read`／`readline`／`readlines`／迭代／`with`／`close`／`encoding` ✓；解码走
//!    `decode_with_encoding` **一处真相** ✓）⇒ `tokenize.open` 端到端 ✓。
//!
//! **如实范围** ✗：`TextIOWrapper` 只接**读**（写路径／`newline=`／`buffering` 立支 ✓）；
//! `type(sys.stdout) is _io.TextIOWrapper` 不再为真 ✓（`sys.stdout` 仍是那个占位文本流类型 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-fstring-io-{}-{name}", std::process::id()));
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

/// `ascii()` ＝ `repr` ＋ 非 ASCII 转义 ✓（`\xNN`／`\uNNNN`／`\U…` ✓）
const ASCII_SCRIPT: &str = r#"print(ascii("é"), ascii("中"), ascii(1234), ascii("a\x00"), ascii("😀"), ascii(["é", 1]))
print(ascii("\u200b"), ascii("plain"), ascii({"中": 1}))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `ASCII_SCRIPT` ✓。
const ASCII_EXPECTED: &str = "'\\xe9' '\\u4e2d' 1234 'a\\x00' '\\U0001f600' ['\\xe9', 1]\n'\\u200b' 'plain' {'\\u4e2d': 1}\n";

/// 相邻字面量链（普通串 ＋ **两枚以上** f-string ✓）与「表达式以字符串开头」的子词法 ✓
const FSTRING_CHAIN_SCRIPT: &str = r#"x = 1


def f():
    return ('Field('
            f'name={x!r},'
            f'type={x!r},'
            ')')


print(f())
print(f"{'a'}")
print(f"{'a'!r}")
print(f"x{'b'}y")
print('p' f'a{x}' f'b{x}')
print(f'a{x}' 'p' f'b{x}')
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `FSTRING_CHAIN_SCRIPT` ✓。
const FSTRING_CHAIN_EXPECTED: &str = "Field(name=1,type=1,)\na\n'a'\nxby\npa1b1\na1pb1\n";

/// `_io.TextIOWrapper` 读路径 ＋ `tokenize.open` 端到端 ✓
const TEXT_IO_SCRIPT: &str = r#"import io
import tokenize

PATH = "probe_data.py"
SRC = "def f(a, b):\n    # comment\n    text = \"全角：αβ\"\n    return a + b\n"
with open(PATH, "w", encoding="utf-8") as fp:
    fp.write(SRC)

f = open(PATH, "rb")
t = io.TextIOWrapper(f, "utf-8")
print("enc", t.encoding, "has-mode", hasattr(t, "mode"))
print("L1", repr(t.readline()))
print("L2", repr(t.readline()))
rest = t.read()
print("rest-lines", len(rest.splitlines()), "has-return", "return" in rest)
t.close()
print("closed", t.closed)

with tokenize.open(PATH) as fp:
    data = fp.read()
print("open-enc", fp.encoding, "open-mode", fp.mode)
print("open-len", len(data))
lines = data.splitlines()
print("open-first", repr(lines[0]))
print("open-last", repr(lines[-1]))

f2 = open(PATH, "rb")
w = io.TextIOWrapper(f2, encoding="utf-8")
count = 0
for line in w:
    count += 1
print("iter-lines", count)
w.close()
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `TEXT_IO_SCRIPT` ✓。
const TEXT_IO_EXPECTED: &str = "enc utf-8 has-mode False\nL1 'def f(a, b):\\n'\nL2 '    # comment\\n'\nrest-lines 2 has-return True\nclosed True\nopen-enc utf-8 open-mode r\nopen-len 63\nopen-first 'def f(a, b):'\nopen-last '    return a + b'\niter-lines 4\n";

#[test]
fn ascii_builtin_agrees_with_the_reference_agree_with_the_reference() {
    let stdout = run("ascii_builtin_agrees_with_the_reference", ASCII_SCRIPT);
    assert_eq!(stdout, ASCII_EXPECTED, "{stdout}");
}
#[test]
fn adjacent_fstrings_and_leading_string_agree_with_the_reference() {
    let stdout = run("adjacent_fstrings_and_leading_string", FSTRING_CHAIN_SCRIPT);
    assert_eq!(stdout, FSTRING_CHAIN_EXPECTED, "{stdout}");
}
#[test]
fn text_io_wrapper_and_tokenize_open_agree_with_the_reference() {
    let stdout = run("text_io_wrapper_and_tokenize_open", TEXT_IO_SCRIPT);
    assert_eq!(stdout, TEXT_IO_EXPECTED, "{stdout}");
}
