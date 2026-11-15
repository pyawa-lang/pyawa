//! **`str` 的 `start`／`end` 与 `split`／`rsplit` 语义**（`P3-41` round 40）。
//!
//! 落地：
//! * `startswith`／`endswith`／`count` 的 `start`／`end` **生效**（此前整段忽略 ⇒
//!   `test_userstring` 的 904 个假失败：`reference_find` 用 `s.startswith(p, i)`）——
//!   口径照本机 3.14.4：负数先 `+len`、`start > len` ⇒ **False**（连空串也一样）、`end` 超界夹到 `len`；
//! * `find`／`rfind`／`index`／`rindex`／`count`／`startswith`／`endswith` 接受 `None` 边界（3.14 切片口径）；
//! * `split`／`rsplit` 接 `None` 分隔符（按空白切）与 `maxsplit`（剩余段原样、只去段首空白）；
//! * `bytes.startswith`／`endswith` 同样接 `start`／`end` 与 `None`；
//! * ASCII 快路径（`text_char_len`／`char_byte_offset`／借用版 `bound_text_cow`）——
//!   周期串测试从 33 s 量级回落（先前每口都克隆并逐字符数）。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-{}-{}", std::process::id(), name));
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

const STR_BOUNDS_SCRIPT: &str = r#"import sys

def show(label, fn):
    try:
        print(label, "=>", fn())
    except TypeError as error:
        print(label, "=> TypeError:", error)
    except ValueError as error:
        print(label, "=> ValueError:", error)

cases = [
    ("abc", "b", 1), ("abc", "b", 2), ("abc", "b", -2), ("abc", "b", 1, 2),
    ("abc", "", 5), ("abc", "", 4, 4), ("abc", "", 3), ("abc", "abc", 0, 3),
    ("abc", "abc", 0, 2), ("abc", "c", 0, 2), ("abc", "c", 0, 3), ("abc", "a", -10),
    ("abc", "a", 10), ("abc", "a", 1, -10), ("abc", "b", 1, -1), ("abc", "b", None),
    ("abc", "abcabc", 0, sys.maxsize),
]
for text, pat, *rest in cases:
    show(f"startswith {pat!r} {rest}", lambda t=text, p=pat, r=rest: t.startswith(p, *r))
    show(f"endswith {pat!r} {rest}", lambda t=text, p=pat, r=rest: t.endswith(p, *r))
    show(f"find {pat!r} {rest}", lambda t=text, p=pat, r=rest: t.find(p, *r))
    show(f"count {pat!r} {rest}", lambda t=text, p=pat, r=rest: t.count(p, *r))

for text, sep, limit in [
    (" a  b c  ", None, 1), (" a  b c  ", None, 2), (" a  b c  ", None, -1), (" a  b c  ", None, 0),
    ("  ", None, 0), ("", None, 0), ("a,b,c", ",", 1), ("a,b,c", ",", 0), ("a,b,c", ",", -1),
    ("a,,b", ",", 1), ("", ",", 0), ("a b", None, None),
]:
    show(f"split {sep!r} {limit}", lambda t=text, s=sep, l=limit: t.split(s, l))
    show(f"rsplit {sep!r} {limit}", lambda t=text, s=sep, l=limit: t.rsplit(s, l))

show("split-bad", lambda: "a b".split(5))
show("split-bad-max", lambda: "a b".split(",", "x"))
show("startswith-tuple", lambda: "abc".startswith(("x", "b"), 1))
show("endswith-tuple", lambda: "abc".endswith(("x", "b"), 1, 2))
show("bytes-startswith", lambda: b"abc".startswith(b"b", 1))
show("bytes-endswith", lambda: b"abc".endswith(b"b", 1, 2))
show("bytes-startswith-empty", lambda: b"abc".startswith(b"", 5))
show("bytes-find-none", lambda: b"abc".find(b"b", None))
"#;

const STR_BOUNDS_EXPECTED: &str = "startswith 'b' [1] => True
endswith 'b' [1] => False
find 'b' [1] => 1
count 'b' [1] => 1
startswith 'b' [2] => False
endswith 'b' [2] => False
find 'b' [2] => -1
count 'b' [2] => 0
startswith 'b' [-2] => True
endswith 'b' [-2] => False
find 'b' [-2] => 1
count 'b' [-2] => 1
startswith 'b' [1, 2] => True
endswith 'b' [1, 2] => True
find 'b' [1, 2] => 1
count 'b' [1, 2] => 1
startswith '' [5] => False
endswith '' [5] => False
find '' [5] => -1
count '' [5] => 0
startswith '' [4, 4] => False
endswith '' [4, 4] => False
find '' [4, 4] => -1
count '' [4, 4] => 0
startswith '' [3] => True
endswith '' [3] => True
find '' [3] => 3
count '' [3] => 1
startswith 'abc' [0, 3] => True
endswith 'abc' [0, 3] => True
find 'abc' [0, 3] => 0
count 'abc' [0, 3] => 1
startswith 'abc' [0, 2] => False
endswith 'abc' [0, 2] => False
find 'abc' [0, 2] => -1
count 'abc' [0, 2] => 0
startswith 'c' [0, 2] => False
endswith 'c' [0, 2] => False
find 'c' [0, 2] => -1
count 'c' [0, 2] => 0
startswith 'c' [0, 3] => False
endswith 'c' [0, 3] => True
find 'c' [0, 3] => 2
count 'c' [0, 3] => 1
startswith 'a' [-10] => True
endswith 'a' [-10] => False
find 'a' [-10] => 0
count 'a' [-10] => 1
startswith 'a' [10] => False
endswith 'a' [10] => False
find 'a' [10] => -1
count 'a' [10] => 0
startswith 'a' [1, -10] => False
endswith 'a' [1, -10] => False
find 'a' [1, -10] => -1
count 'a' [1, -10] => 0
startswith 'b' [1, -1] => True
endswith 'b' [1, -1] => True
find 'b' [1, -1] => 1
count 'b' [1, -1] => 1
startswith 'b' [None] => False
endswith 'b' [None] => False
find 'b' [None] => 1
count 'b' [None] => 1
startswith 'abcabc' [0, 9223372036854775807] => False
endswith 'abcabc' [0, 9223372036854775807] => False
find 'abcabc' [0, 9223372036854775807] => -1
count 'abcabc' [0, 9223372036854775807] => 0
split None 1 => ['a', 'b c  ']
rsplit None 1 => [' a  b', 'c']
split None 2 => ['a', 'b', 'c  ']
rsplit None 2 => [' a', 'b', 'c']
split None -1 => ['a', 'b', 'c']
rsplit None -1 => ['a', 'b', 'c']
split None 0 => ['a  b c  ']
rsplit None 0 => [' a  b c']
split None 0 => []
rsplit None 0 => []
split None 0 => []
rsplit None 0 => []
split ',' 1 => ['a', 'b,c']
rsplit ',' 1 => ['a,b', 'c']
split ',' 0 => ['a,b,c']
rsplit ',' 0 => ['a,b,c']
split ',' -1 => ['a', 'b', 'c']
rsplit ',' -1 => ['a', 'b', 'c']
split ',' 1 => ['a', ',b']
rsplit ',' 1 => ['a,', 'b']
split ',' 0 => ['']
rsplit ',' 0 => ['']
split None None => TypeError: 'NoneType' object cannot be interpreted as an integer
rsplit None None => TypeError: 'NoneType' object cannot be interpreted as an integer
split-bad => TypeError: must be str or None, not int
split-bad-max => TypeError: 'str' object cannot be interpreted as an integer
startswith-tuple => True
endswith-tuple => True
bytes-startswith => True
bytes-endswith => True
bytes-startswith-empty => False
bytes-find-none => 1
";

#[test]
fn str_bounds_and_split_match_the_reference() {
    assert_eq!(run("probe", STR_BOUNDS_SCRIPT), STR_BOUNDS_EXPECTED);
}
