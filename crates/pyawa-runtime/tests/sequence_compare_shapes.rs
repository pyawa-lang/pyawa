//! **序列字典序比较 ＋ `list.sort`**（task-23 ✓；立据＝`heapq.nsmallest`／`nlargest`）。
//!
//! ① `tuple`／`list` 的 `<`／`<=`／`>`／`>=`：逐元素字典序 ✓、一方是**前缀** ⇒ 短的小 ✓、
//! **首个不等元素**定序（递归 ✓ ⇒ 元素不可比时报 `'<' not supported between instances of 'str' and 'int'` ✓）、
//! **混合**（tuple vs list）照参照**报 `TypeError`** ✓（实测 `(1,2) < [1,3]` ✓）。
//! ② `list.sort(*, key=None, reverse=False)`：**原地**稳定排序 ✓、回 `None` ✓、位置实参报
//! `sort() takes no positional arguments` ✓；比较与 `sorted` **同一处**（`Instance::order_of` ✓）——
//! 于是那边也顺带补上了序列序 ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-seqcmp-{}-{name}", std::process::id()));
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

/// `tuple`／`list` 字典序（前缀规则 ✓、嵌套 ✓、混合与元素不可比 ⇒ `TypeError` 同文 ✓）＋ `heapq.nsmallest`／`nlargest` ✓
const SEQUENCE_CMP_SCRIPT: &str = r#"print((1, 2) < (1, 3), (1, 2) < (1, 2, 0), [1, 2] < [1, 3], () < (1,), (1, 2) <= (1, 2), (1, 3) > (1, 2))
print((1, 3) >= (1, 2), (1, 2) > (1, 3), [2] >= [1, 2], [1, 2] <= [1, 2])


def mixed():
    return (1, 2) < [1, 3]


def bad_element():
    return (1, "a") < (1, 2)


for probe in (mixed, bad_element):
    try:
        print(probe())
    except TypeError as exc:
        print("TypeError:", exc)


print((1, 2) == [1, 2], (1, [2]) < (1, [3]))
import heapq

print(heapq.nsmallest(2, [9, 4, 7, 1]), heapq.nlargest(2, [9, 4, 7, 1]))
print(sorted([(2, "b"), (1, "a")]))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const SEQUENCE_CMP_EXPECTED: &str = "True True True True True True\nTrue False True True\nTypeError: '<' not supported between instances of 'tuple' and 'list'\nTypeError: '<' not supported between instances of 'str' and 'int'\nFalse True\n[1, 4] [9, 7]\n[(1, 'a'), (2, 'b')]\n";

#[test]
fn sequence_lexicographic_compare_agrees_with_the_reference() {
    let stdout = run("sequence_lexicographic_compare_agrees", SEQUENCE_CMP_SCRIPT);
    assert_eq!(stdout, SEQUENCE_CMP_EXPECTED, "{stdout}");
}

/// `list.sort(*, key, reverse)` ✓（原地、稳定 ✓）＋ `sorted` 走**同一处**序 ✓
const LIST_SORT_SCRIPT: &str = r#"data = [3, 1, 2]
data.sort()
print(data)
data.sort(reverse=True)
print(data)
words = ["bb", "a", "ccc"]
words.sort(key=len)
print(words)
words.sort(key=len, reverse=True)
print(words)
print(sorted([(2, "b"), (1, "a")]))
print(sorted([[2], [1, 3], [1, 2]]))
print(sorted((3, 1, 2), reverse=True))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const LIST_SORT_EXPECTED: &str = "[1, 2, 3]\n[3, 2, 1]\n['a', 'bb', 'ccc']\n['ccc', 'bb', 'a']\n[(1, 'a'), (2, 'b')]\n[[1, 2], [1, 3], [2]]\n[3, 2, 1]\n";

#[test]
fn list_sort_and_sorted_agree_with_the_reference() {
    let stdout = run("list_sort_and_sorted_agree", LIST_SORT_SCRIPT);
    assert_eq!(stdout, LIST_SORT_EXPECTED, "{stdout}");
}

