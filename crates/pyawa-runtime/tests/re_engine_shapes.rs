//! **两档引擎的边角口径**（task-17 第 2 轮）——`regex` 快路径与 `fancy-regex` 回退路径的
//! `pos`／`endpos`、`^`／`\b`／`$`、bytes 产出、空匹配前进语义，逐例与参照对 ✓。
//!
//! 参照值全部由本机 `python3`（3.14）的 `re` **逐例实测**取得 ✓（脚本见各 `const`）。
//!
//! **为什么单立一卷** ✓：第 2 轮在 `target/recon/re_engine_edges.py` 上抓到一处**真 bug** ✗ ——
//! `pos > 0` 时旧实现把 `pos` **之前的前缀一起切掉** ✗ ⇒ `^`／`\b`／look-behind 全看错上下文 ✓
//! （`(?<=a)b` 对 `"xabxa"` 的 `search(…, 2)` ⇒ 参照 `(2,3)` ✓ 而切片法 `None` ✗；
//! `^b` 的 `match(…, 1)` ⇒ 参照 `None` ✓ 而切片法**误报** `(1,2)` ✗）。
//! 现口径：**只截尾**（`endpos` ✓），起点交给引擎自己的 pos 接口 ✓
//! （`regex::captures_at` ✓／`fancy_regex::captures_from_pos` ✓）；空匹配由**共用游标**推进 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-re-engine-{}-{name}", std::process::id()));
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

const POS_SCRIPT: &str = r#"
import re


def span_of(m):
    if m is None:
        return None
    return m.span()


p = re.compile(r'(?<=a)b')
print(span_of(p.search("xabxa", 1)))
print(span_of(p.search("xabxa", 1, 4)))
print(span_of(p.search("xabxa", 2, 4)))
print(span_of(p.match("xabxa", 2)))
print(p.findall("abab", 1))
print(p.findall("abab", 0, 3))
print([span_of(m) for m in p.finditer("abab", 1)])
print(span_of(re.compile(r'^b').search("ab", 1)))
print(span_of(re.compile(r'a$').search("ab", 0, 1)))
print(span_of(re.compile(r'(?<=α)β').search("αβ", 1)))
print(re.compile(r'(?<=α)β').findall("αββ", 1))
print(span_of(re.compile(r'(?<=α)β').match("αβ", 1)))
print(span_of(re.compile(r'(.)\1').search("ααβ")))
"#;

/// 参照（`python3` 3.14 实测 ✓）：`pos`／`endpos` 窗口里，**前缀仍是上下文** ✓ ——
/// look-behind 能看 `pos` 之前的字符 ✓（`(2,3)` ✓）；`^` **不**在 `pos` 处命中 ✓（`None` ✓）；
/// `$` 认 `endpos` 为串尾 ✓（`(0,1)` ✓）；非 ASCII 的**字符**下标换算 ✓（`(?<=α)β` 的 `pos=1` ✓）。
const POS_EXPECTED: &str = "(2, 3)\n(2, 3)\n(2, 3)\n(2, 3)\n['b', 'b']\n['b']\n[(1, 2), (3, 4)]\nNone\n(0, 1)\n(1, 2)\n['β']\n(1, 2)\n(0, 2)\n";

#[test]
fn pos_and_endpos_keep_the_context_before_pos() {
    let stdout = run("pos", POS_SCRIPT);
    assert_eq!(stdout, POS_EXPECTED, "{stdout}");
}

const BYTES_SCRIPT: &str = r#"
import re


def span_of(m):
    if m is None:
        return None
    return m.span()


pb = re.compile(rb'(?<=a)b')
print(span_of(pb.search(b"xab")))
print(span_of(pb.search(b"xab", 1)))
print(pb.findall(b"abab"))
print(pb.split(b"abab"))
print(pb.sub(b"X", b"abab"))
print(re.compile(rb'(.)\1').findall(b"aabbc"))
print(re.compile(rb'(.)\1').sub(b"-", b"aabbc"))
print(span_of(re.compile(rb'(?P<x>.)(?P=x)').search(b"aabbc")))
print([span_of(m) for m in pb.finditer(b"abab")])
print(span_of(re.compile(rb'(?<=a)b', re.VERBOSE).search(b"ab")))
print(span_of(re.compile(rb'(?s)(?<=a).').search(b"a\nb")))
print(span_of(re.compile(rb'(?i)(?<=a)b').search(b"Ab")))
print(span_of(re.compile(rb'(?m)(?<=a)b').search(b"c\na b")))
"#;

/// 参照（`python3` 3.14 实测 ✓）：bytes 的 `findall`／`split`／`sub`／`finditer` 在回退引擎上
/// **全交回 bytes** ✓；`re.VERBOSE`（外置 flag ✓）与 `(?s)`／`(?i)`／`(?m)`（**内联 flag** ✓）
/// 都生效 ✓ —— 内联那三条钉的是 `re` 层解析器 ✓（`str(bytes, 'latin1')` 那条 core 修 ✗）。
const BYTES_EXPECTED: &str = "(2, 3)\n(2, 3)\n[b'b', b'b']\n[b'a', b'a', b'']\nb'aXaX'\n[b'a', b'b']\nb'--c'\n(0, 2)\n[(1, 2), (3, 4)]\n(1, 2)\n(1, 2)\n(1, 2)\nNone\n";

#[test]
fn bytes_outputs_on_the_fallback_engine_agree_with_the_reference() {
    let stdout = run("bytes", BYTES_SCRIPT);
    assert_eq!(stdout, BYTES_EXPECTED, "{stdout}");
}

const EMPTY_SCRIPT: &str = r#"
import re

print(re.findall(r'(?=a)', "aaa"))
print(re.compile(r'(?=a)').findall("aaa", 1))
print(re.compile(r'(?=a)').findall("aaa", 0, 1))
print(re.split(r'(?=a)', "baaa"))
print(re.sub(r'(?=a)', "-", "baaa"))
print(re.compile(r'(?=a)').sub("-", "baaa", 1))
print([m.span() for m in re.finditer(r'(?<!a)(?=b)', "abcbb")])
print(re.sub(r'(?<=a)(?=b)', "-", "abab"))
print(re.findall(r'z*', "ab"))
print(re.sub(r'z*', "-", "ab"))
print(re.findall(r'(?=)', "ab"))
print(re.sub(r'(?=)', "-", "ab"))
print(re.compile(r'(?=a)').findall("aaa", 10))
print(re.compile('').search('abc', 10).span())
print(re.compile('a').search('abc', -1).span())
print(re.compile('a').search('abc', 0, 0))
print(re.compile('').search('abc', 0, -1).span())
"#;

/// 参照（`python3` 3.14 实测 ✓）：空匹配**前进一步** ✓、**句尾只产出一次** ✓（`z*` 对 `"ab"` ⇒
/// `['', '', '']` ✓、`(?=)` 对 `"ab"` ⇒ `['', '', '']` ✓）；`sub` 的切段与前进同口径 ✓；
/// `pos`／`endpos` 的**夹取**照参照 ✓（`pos` 超长 ⇒ 夹到串尾 ✓、负 `pos` ⇒ 夹到 0 ✓、
/// `endpos=-1` ⇒ 夹到 0 且空模式仍在 0 命中 ✓）。
const EMPTY_EXPECTED: &str = "['', '', '']\n['', '']\n['']\n['b', 'a', 'a', 'a']\nb-a-a-a\nb-aaa\n[(3, 3), (4, 4)]\na-ba-b\n['', '', '']\n-a-b-\n['', '', '']\n-a-b-\n[]\n(3, 3)\n(0, 1)\nNone\n(0, 0)\n";

#[test]
fn empty_matches_advance_like_the_reference() {
    let stdout = run("empty", EMPTY_SCRIPT);
    assert_eq!(stdout, EMPTY_EXPECTED, "{stdout}");
}
