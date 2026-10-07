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

const POS_SCRIPT: &str = r#"
import _sre
p = _sre.compile("a", 0, None, 0, {}, ())
print(p.search("banana", 2).span())
print(p.search("banana", 0, 2).span())
print(p.match("banana", 1).span())
print(p.match("banana", 1, 2) is None)
print(p.findall("banana", 2))
print(p.findall("banana", 0, 2))
print(p.fullmatch("banana", 1, 2) is not None)
print(p.fullmatch("banana", 1, 3) is None)
"#;

/// 参照（`python3` 3.14 实测 ✓）：窗口内匹配 ✓，下标仍相对**整串** ✓
/// （`findall("banana", 2)` ⇒ `['a', 'a']` ✓ —— 第 585 轮踩过"用窗口切片"的坑 ✓）。
const POS_EXPECTED: &str = "(3, 4)\n(1, 2)\n(1, 2)\nFalse\n['a', 'a']\n['a']\nTrue\nTrue\n";

#[test]
fn pos_and_endpos_agree_with_the_reference() {
    let stdout = run("pos", POS_SCRIPT);
    assert_eq!(stdout, POS_EXPECTED, "{stdout}");
}

const SPLIT_SCRIPT: &str = r#"
import _sre
def P(pat):
    return _sre.compile(pat, 0, None, 0, {}, ())
print(P("b").split("abcb"))
print(P("(b)").split("abcb"))
print(P("b").split("abcb", 1))
print(P("(,)|(;)").split("a,b;c"))
print(P("").split("abc"))
"#;

/// 参照（`python3` 3.14 实测 ✓）：无组 ⇒ 直接切 ✓；有组 ⇒ 组文本插进结果 ✓（未匹配 ⇒ `None` ✓）；
/// `maxsplit` 限制切几次 ✓；空模式也在每位切一刀 ✓。
const SPLIT_EXPECTED: &str = "['a', 'c', '']\n['a', 'b', 'c', 'b', '']\n['a', 'cb']\n['a', ',', None, 'b', None, ';', 'c']\n['', 'a', 'b', 'c', '']\n";

#[test]
fn split_agrees_with_the_reference() {
    let stdout = run("split", SPLIT_SCRIPT);
    assert_eq!(stdout, SPLIT_EXPECTED, "{stdout}");
}

const SUB_SCRIPT: &str = r#"
import _sre
def P(pat, groups=0, names=None):
    return _sre.compile(pat, 0, None, groups, names or {}, ())
print(P("a").sub("-", "banana"))
print(P("a").sub("-", "banana", 2))
print(P("(a)", 1).sub(r"[\1]", "banana"))
print(P("(?P<x>a)", 1, {"x": 1}).sub(r"<\g<x>>", "banana"))
print(P("a").subn("-", "banana"))
print(P("(a)|(b)", 2).sub(r"<\1|\2>", "ab"))
print(P("a").sub(r"\\n", "banana"))
print(P("a").sub(r"\t", "banana"))
print(P("(?P<x>a)", 1, {"x": 1}).subn(r"\g<0>!", "banana", 1))
"#;

/// 参照（`python3` 3.14 实测 ✓）：字符串模板 `\1`／`\g<名字>`／`\g<0>`／`\\`／`\t` ✓、
/// `count` 限制替换次数 ✓、`subn` 返回（新串, 次数）✓、未匹配的组展开成**空串** ✓。
/// 预期值由实测输出生成 ✓（避免手写转义出错 ✓）。
const SUB_EXPECTED: &str = "b-n-n-\nb-n-na\nb[a]n[a]n[a]\nb<a>n<a>n<a>\n('b-n-n-', 3)\n<a|><|b>\nb\\nn\\nn\\n\nb\tn\tn\t\n('ba!nana', 1)\n";

#[test]
fn sub_and_subn_agree_with_the_reference() {
    let stdout = run("sub", SUB_SCRIPT);
    assert_eq!(stdout, SUB_EXPECTED, "{stdout}");
}

const CALLABLE_SCRIPT: &str = r#"
import _sre
def P(pat, groups=0, names=None):
    return _sre.compile(pat, 0, None, groups, names or {}, ())
print(P("a").sub(lambda m: m.group().upper(), "banana"))
print(P("(?P<x>a)", 1, {"x": 1}).sub(lambda m: "<" + m.group("x") + ">", "banana", 2))
print(P("(a)|(b)", 2).sub(lambda m: m.group(1) or "?", "ab"))
try:
    P("a").sub(lambda m: 1, "banana")
except TypeError:
    print("TypeError")
"#;

/// 参照（`python3` 3.14 实测 ✓）：替换可以是**函数** ✓（拿到 `re.Match` ✓）；
/// 返回非 `str` ⇒ `TypeError` ✓。预期值由实测输出生成 ✓。
const CALLABLE_EXPECTED: &str = "bAnAnA\nb<a>n<a>na\na?\nTypeError\n";

#[test]
fn callable_replacement_agrees_with_the_reference() {
    let stdout = run("callable", CALLABLE_SCRIPT);
    assert_eq!(stdout, CALLABLE_EXPECTED, "{stdout}");
}

const FINDITER_SCRIPT: &str = r#"
import _sre
def P(pat, groups=0, names=None):
    return _sre.compile(pat, 0, None, groups, names or {}, ())
print([m.span() for m in P("a").finditer("banana")])
print([m.span() for m in P("a").finditer("banana", 2)])
print([m.group() for m in P("(a)(n)?").finditer("banana")])
"#;

/// 参照（`python3` 3.14 实测 ✓）：`finditer` 的跨度、`pos` 窗口、组文本 ✓。
/// **如实** ✗：参照还满足 `hasattr(it, "__next__")` ✓ 而我们为 `False` ✗ —— 这是**迭代器类型**
/// 的属性面缺口 ✓（`next(it)` 已可用 ✓），属 core 侧 ✓ 不在 `_sre` ✓ ⇒ 本护栏只钉已对齐的 3 条 ✓。
const FINDITER_EXPECTED: &str = "[(1, 2), (3, 4), (5, 6)]\n[(3, 4), (5, 6)]\n['an', 'an', 'a']\n";

#[test]
fn finditer_agrees_with_the_reference() {
    let stdout = run("finditer", FINDITER_SCRIPT);
    assert_eq!(stdout, FINDITER_EXPECTED, "{stdout}");
}

const EXPAND_SCRIPT: &str = r#"
import _sre
def P(pat, groups=0, names=None):
    return _sre.compile(pat, 0, None, groups, names or {}, ())
m = P("(?P<x>a)(b)?", 1, {"x": 1}).search("xaby")
print(m.expand(r"[\1][\g<x>][\2]"))
print(m.expand("plain"))
print(m.expand(r"\g<0>!"))
"#;

/// 参照（`python3` 3.14 实测 ✓）：`Match.expand` 与 `sub` 的模板同一套语法 ✓，
/// 未匹配的组展开成**空串** ✓。预期值由实测输出生成 ✓。
const EXPAND_EXPECTED: &str = "[a][a][b]\nplain\nab!\n";

#[test]
fn match_expand_agrees_with_the_reference() {
    let stdout = run("expand", EXPAND_SCRIPT);
    assert_eq!(stdout, EXPAND_EXPECTED, "{stdout}");
}

const ATTR_SCRIPT: &str = r#"
import _sre
p = _sre.compile("(?P<x>a)(b)?", 32, None, 2, {"x": 1}, ())
print(p.pattern)
print(p.flags)
print(p.groups)
print(p.groupindex)
print(p.search("xaby").span())
"#;

/// 参照（`python3` 3.14 实测 ✓）：`Pattern.pattern`／`flags`／`groups`／`groupindex` ✓
/// （`re` 的 Python 层要读 `.pattern`／`.flags` ✓ ⇒ 它们挂进**实例字典** ✓ 走普通属性通道 ✓）。
/// 参照的 `re.compile` 默认带 `re.UNICODE`（32 ✓）⇒ 这里显式喂 32 ✓ 才是同一口径 ✓。
const ATTR_EXPECTED: &str = "(?P<x>a)(b)?\n32\n2\n{'x': 1}\n(1, 3)\n";

#[test]
fn pattern_attributes_agree_with_the_reference() {
    let stdout = run("attributes", ATTR_SCRIPT);
    assert_eq!(stdout, ATTR_EXPECTED, "{stdout}");
}

const MATCHATTR_SCRIPT: &str = r#"
import _sre
def P(pat, groups=0, names=None):
    return _sre.compile(pat, 0, None, groups, names or {}, ())
p = P("a")
m = p.search("banana", 2)
print(m.string, m.pos, m.endpos)
print(m.re.pattern, m.re is p)
print(p.search("banana", 2, 4).pos, p.search("banana", 2, 4).endpos)
"#;

/// 参照（`python3` 3.14 实测 ✓）：`Match.string`／`pos`／`endpos`／`re` ✓
/// （`pos`／`endpos` 反映传入的窗口参数 ✓，`m.re is p` 为真 ✓）。
const MATCHATTR_EXPECTED: &str = "banana 2 6\na True\n2 4\n";

#[test]
fn match_attributes_agree_with_the_reference() {
    let stdout = run("matchattrs", MATCHATTR_SCRIPT);
    assert_eq!(stdout, MATCHATTR_EXPECTED, "{stdout}");
}

const TEMPLATE_SCRIPT: &str = r#"
import _sre
def P(pat, groups=0, names=None):
    return _sre.compile(pat, 0, None, groups, names or {}, ())
p = P("(?P<x>a)(b)?")
t = _sre.template(p, ['[', 1, ']'])
print(t(p.search("xaby")))
print(t(p.search("xazy")))
t2 = _sre.template(p, ['', 0, '!'])
print(t2(p.search("xaby")))
t3 = _sre.template(p, ['plain'])
print(t3(p.search("xaby")))
"#;

/// 参照（`python3` 3.14 实测 ✓，**等价口径** ✓）：CPython 的 `_sre.template` 返回的是
/// **`SRE_Template` 对象** ✓ —— 它**自己不可调用** ✗（是 C 层 `Pattern.sub` 特认的 ✓）；
/// 我们这边 `Pattern.sub` 收**任意可调用对象** ✓ ⇒ `_sre.template` 返回带 `__call__` 的对象 ✓
/// （第 593 轮刚接通的 `__call__` ✓），语义与参照的 `Match.expand(同一模板)` **逐条一致** ✓
/// —— 故对照脚本用 `expand` 作等价口径 ✓（直接调参照的 `_sre.template(...)(m)` 会抛
/// `TypeError: '_sre.SRE_Template' object is not callable` ✗，那是参照自己的形状 ✓）。
/// 解析结构由本机实测取得 ✓：`['[', 1, ']']`（字面量与组号交替 ✓ 转义已展开 ✓ 名字已换号 ✓）。
const TEMPLATE_EXPECTED: &str = "[a]\n[a]\nab!\nplain\n";

#[test]
fn template_expansion_agrees_with_the_reference() {
    let stdout = run("template", TEMPLATE_SCRIPT);
    assert_eq!(stdout, TEMPLATE_EXPECTED, "{stdout}");
}

const LAST_SCRIPT: &str = r#"
import _sre
def P(pat, groups=0, names=None):
    return _sre.compile(pat, 0, None, groups, names or {}, ())
print(P("(a)(b)?", 2).search("xaby").lastindex)
print(P("(a)(b)?", 2).search("xazy").lastindex)
print(P("(?P<x>a)(?P<y>b)", 2, {"x": 1, "y": 2}).search("xaby").lastgroup)
print(P("(a)(b)?", 2).search("xaby").lastgroup)
print(P("(?P<x>a)|(?P<y>b)", 2, {"x": 1, "y": 2}).search("b").lastindex,
      P("(?P<x>a)|(?P<y>b)", 2, {"x": 1, "y": 2}).search("b").lastgroup)
"#;

/// 参照（`python3` 3.14 实测 ✓）：`lastindex` ＝ 最后一个**匹配上的**捕获组 ✓（都没匹配 ⇒ `None` ✓）；
/// `lastgroup` ＝ 该组的名字 ✓（无名 ⇒ `None` ✓）。口径注意 ✓：本脚本显式传 `groupindex` ✓
/// —— `lastgroup` 只能从它推出来 ✓（第一版我漏传 ⇒ 报 `None` vs `y` ✗，是脚本的错 ✓ 不是实现的错 ✓）。
const LAST_EXPECTED: &str = "2\n1\ny\nNone\n2 y\n";

#[test]
fn lastindex_and_lastgroup_agree_with_the_reference() {
    let stdout = run("lastindex", LAST_SCRIPT);
    assert_eq!(stdout, LAST_EXPECTED, "{stdout}");
}
