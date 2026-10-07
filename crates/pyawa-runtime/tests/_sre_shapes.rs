//! **`_sre` 原始面的形状**（第 580 轮）——`re`（≈77 个模块）压在它上面 ✓，先把它钉住 ✓。
//!
//! 参照值全部由本机 `python3`（3.14）的 `re` **逐例实测**取得 ✓（`span()` 拼成同一格式 ✓）：
//! ```
//! ("(a)(b)?", "xaby", search) => 1,3;1,2;2,3      ("(a)(b)?", "xaby", match) => None
//! ("ab", "xaby", search)      => 1,3              ("a.*y", "xaby", fullmatch) => None
//! ("(?i)AB", "xaby", search)  => 1,3              ("z", "xaby", search)        => None
//! ```
//! **非 ASCII 那四条是本轮的真 bug 修** ✓：`regex` crate 报**字节**偏移 ✗，而参照 `re`
//! 一律用**字符**偏移 ✓ ⇒ 换算前 `\w+` 对 `"αβγ δ"` 给 `0,6`（错 ✗），换算后 `0,3` ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-sre-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
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

const SCRIPT: &str = r#"
import _sre
cases = [
    ("(a)(b)?", 0, "xaby", "search"),
    ("(a)(b)?", 0, "xaby", "match"),
    ("ab", 0, "xaby", "search"),
    ("a.*y", 0, "xaby", "fullmatch"),
    ("(?i)AB", 2, "xaby", "search"),
    ("z", 0, "xaby", "search"),
    ("\w+", 0, "\u03b1\u03b2\u03b3 \u03b4", "search"),
    ("\w+", 0, "\u03b1\u03b2\u03b3 \u03b4", "fullmatch"),
    ("(\w)(\w+)", 0, "\u03b1\u03b2\u03b3\u03b4", "match"),
    ("\u03b2", 0, "\u03b1\u03b2\u03b3", "search"),
    ("(?i)\u03a9", 0, "\u03c9", "search"),
]
for pattern, flags, text, kind in cases:
    ident = _sre.compile_raw(pattern, flags)
    print(_sre.match_raw(ident, text, kind))
"#;

const EXPECTED: &str = "1,3;1,2;2,3\n\
None\n\
1,3\n\
None\n\
1,3\n\
None\n\
0,3\n\
None\n\
0,4;0,1;1,4\n\
1,2\n\
0,1\n";

#[test]
fn raw_compile_and_match_agree_with_the_reference() {
    let stdout = run("raw", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}

const PATTERN_SCRIPT: &str = r#"
import _sre
p = _sre.compile("(a)(b)?", 0, None, 1, {}, ())
m = p.search("xaby")
print(m.span(), m.start(), m.end())
print(m.span(1), m.span(2))
print(p.match("xaby"))
print(p.fullmatch("ab") is not None)
"#;

/// 参照（`python3` 3.14 实测 ✓）：`re.compile("(a)(b)?").search("xaby")` ⇒ span `(1, 3)` ✓。
const PATTERN_EXPECTED: &str = "(1, 3) 1 3\n(1, 2) (2, 3)\nNone\nTrue\n";

#[test]
fn compile_returns_a_pattern_with_match_search_fullmatch() {
    let stdout = run("pattern", PATTERN_SCRIPT);
    assert_eq!(stdout, PATTERN_EXPECTED, "{stdout}");
}

const GROUP_SCRIPT: &str = r#"
import _sre
p = _sre.compile("(?P<w>a)(b)?", 0, None, 2, {"w": 1}, ())
m = p.search("xaby")
print(m.group(), m.group(1), m.group(2), m.group("w"))
print(m.groups())
n = p.search("xazy")
print(n.group(2), n.groups())
"#;

/// 参照（`python3` 3.14 实测 ✓）：命名组、未匹配组 ⇒ `None` ✓。
const GROUP_EXPECTED: &str = "ab a b a\n('a', 'b')\nNone ('a', None)\n";

#[test]
fn match_group_and_groups_agree_with_the_reference() {
    let stdout = run("group", GROUP_SCRIPT);
    assert_eq!(stdout, GROUP_EXPECTED, "{stdout}");
}

const FINDALL_SCRIPT: &str = r#"
import _sre
p0 = _sre.compile("a.", 0, None, 0, {}, ())
print(p0.findall("ab ac ad"))
p1 = _sre.compile("a(.)", 0, None, 1, {}, ())
print(p1.findall("ab ac"))
p2 = _sre.compile("(?P<x>a)(b)?", 0, None, 2, {"x": 1}, ())
print(p2.findall("ab ac"))
m = p2.search("ac")
print(m.groupdict(), m.groupdict("-"))
"#;

/// 参照（`python3` 3.14 实测 ✓）：无组 ⇒ 串列表 ✓、一组 ⇒ 串列表 ✓、多组 ⇒ 元组列表 ✓、
/// 未匹配的组在 `findall` 里是**空串** ✓（不是 `None` ✓）；`groupdict` 给 `default` ✓。
const FINDALL_EXPECTED: &str = "['ab', 'ac', 'ad']\n['b', 'c']\n[('a', 'b'), ('a', '')]\n{'x': 'a'} {'x': 'a'}\n";

#[test]
fn findall_and_groupdict_agree_with_the_reference() {
    let stdout = run("findall", FINDALL_SCRIPT);
    assert_eq!(stdout, FINDALL_EXPECTED, "{stdout}");
}
