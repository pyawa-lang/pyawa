//! **`map`／`filter` 的惰性 ＋ `dict.fromkeys` 的通用可迭代**（`P3-41` round 41）。
//!
//! 背景：`test_tempfile` 是 6 个 hang 里**唯一真 hang** —— `tempfile._RandomNameSequence`
//! 的 `map(str, itertools.count())` 在"急求值版 `map`"下会**消费无限序列**（无界内存）。
//! 本轮把 `map`／`filter` 改成**真惰性迭代器**（新 `ItStateKind::Map`／`Filter` ＋ 类型注册 ＋
//! 推进分支；`type(map(...)).__name__ == "map"` ✓、`iter(m) is m` ✓）；`dict.fromkeys` 补上
//! **通用可迭代协议**（原先只认 list/tuple/set/dict，`test_tempfile` 的 `fromkeys(range(...))` 卡它），
//! 并顺带修两个真 bug：重复键不合并、多键时 value 引用计数欠账。
//!
//! 两段脚本的期望值都由本机 `python3`（3.14.4）取（`MS-17`），逐字节一致。
//!
//! 效果：`test_tempfile` 从"无限挂死"到 **118 个测试 / 0.75 s 跑完**（FAILED failures=1, errors=80 ——
//! 剩余是 `TemporaryDirectory`／`os.replace` 一类普通缺口，不是 hang）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-p341-map-{}-{}", std::process::id(), name));
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

/// 惰性语义（含无限迭代器只取前 3 项）。
const MAP_FILTER_SCRIPT: &str = r#"import itertools

m = map(str, [1, 2, 3])
print("type", type(m).__name__, iter(m) is m)
print("list", list(m))
print("second", list(m))

f = filter(None, [0, 1, "", "x", None, 2])
print("filter-type", type(f).__name__)
print("filter-list", list(f))

# 无限迭代器：只取前 3 项（先前急求值会挂死）
inf = map(str, itertools.count())
print("infinite", [next(inf) for _ in range(3)])

# 多迭代器 + 最短收摊
m2 = map(lambda a, b: a + b, [1, 2], [10, 20, 30])
print("multi", list(m2))

# filter 谓词惰性
calls = []
def pred(x):
    calls.append(x)
    return x > 1
it = filter(pred, [1, 2, 3])
print("calls-before", calls)
print("first", next(it), "calls", calls)
print("rest", list(it), "calls", calls)
"#;

const MAP_FILTER_EXPECTED: &str = "type map True
list ['1', '2', '3']
second []
filter-type filter
filter-list [1, 'x', 2]
infinite ['0', '1', '2']
multi [11, 22]
calls-before []
first 2 calls [1, 2]
rest [3] calls [1, 2, 3]
";

/// `dict.fromkeys` 的通用可迭代（range／生成器／itertools／重复键／str）。
const FROMKEYS_SCRIPT: &str = r#"import itertools
print("range", dict.fromkeys(range(3)))
print("gen", dict.fromkeys((x * 2 for x in range(3))))
print("itertools", dict.fromkeys(itertools.islice(itertools.count(), 3), 0))
print("list-dup", dict.fromkeys([1, 1, 2], "x"))
print("tuple", dict.fromkeys((1, 2), "y"))
print("str", dict.fromkeys("ab", 0))
"#;

const FROMKEYS_EXPECTED: &str = "range {0: None, 1: None, 2: None}
gen {0: None, 2: None, 4: None}
itertools {0: 0, 1: 0, 2: 0}
list-dup {1: 'x', 2: 'x'}
tuple {1: 'y', 2: 'y'}
str {'a': 0, 'b': 0}
";

#[test]
fn lazy_map_and_filter_match_the_reference() {
    assert_eq!(run("map", MAP_FILTER_SCRIPT), MAP_FILTER_EXPECTED);
    assert_eq!(run("fromkeys", FROMKEYS_SCRIPT), FROMKEYS_EXPECTED);
}
