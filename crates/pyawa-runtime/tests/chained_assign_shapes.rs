//! **链式赋值带元组目标 ＋ 形参 cell 留在 `varnames`**（task-22 第 3 轮 ✓）。
//!
//! ① **解析器新形态** ✗：`value, order, nxt = s = h[0]`（`Lib/heapq.py:373`）—— 先前元组目标之后
//! 把 `s = h[0]` 当"值表达式"解析 ✗ ⇒ "语句结尾多出了 Some(Assign)" ✓。参照语义＝**右值求一次、
//! 从右往左赋** ✓ ⇒ 先 `s = 值`、再 `UNPACK_SEQUENCE` 解元组目标 ✓。
//! ② **形参 cell** ✗：`parameter_count()` 只算 位置 ＋ 仅关键字 ✗ ⇒ 被内层闭包捕获的 `*args`／
//! `**kwargs` 被当"局部 cell"从 `varnames` 剔掉 ✗ ⇒ 运行期按槽位绑定**越界** ✓（实测
//! `import concurrent.futures.thread` 报**空消息**的 Rust panic ✓）。参照里形参 cell **留在**
//! `co_varnames` ✓（`def f(a, *args, **kwargs)` ⇒ `varnames=('a','args','kwargs','g')`、
//! `cellvars=('args','kwargs')` ✓）。
//! ③ 顺手把 `bind_arguments` 里那两处**裸下标**换成**带下标的 `TypeError`** ✓（不许 panic ✓）。
//! **如实登记** ✗：`heapq.nsmallest`／`nlargest` 还要**元组比较**（`'<' not supported between
//! instances of 'tuple' and 'tuple'` ✗）—— 那是另一格 ✓，本探针只用堆的三口 ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-chain-{}-{name}", std::process::id()));
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

/// `value, order, nxt = s = h[0]`（`Lib/heapq.py:373`）✓ ＋ `a, b = c = [1, 2]` ✓ ＋ `first, *rest = tail = items` ✓
const CHAIN_SCRIPT: &str = r#"def take(h, items):
    value, order, nxt = s = h[0]
    print(value, order, nxt, s)
    a, b = c = [1, 2]
    print(a, b, c)
    x = y = z = 7
    print(x, y, z)
    first, *rest = tail = items
    print(first, rest, tail)


take([[10, 20, 30]], [1, 2, 3, 4])
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const CHAIN_EXPECTED: &str = "10 20 30 [10, 20, 30]\n1 2 [1, 2]\n7 7 7\n1 [2, 3, 4] [1, 2, 3, 4]\n";

#[test]
fn chained_assignment_with_tuple_target_agrees_with_the_reference() {
    let stdout = run("chained_assignment_with_tuple_target_agrees", CHAIN_SCRIPT);
    assert_eq!(stdout, CHAIN_EXPECTED, "{stdout}");
}

/// 被内层闭包捕获的 `*args`／`**kwargs`（形参 cell 留在 `varnames` ✓）—— 先前 `parameter_count` 漏算 ⇒ 槽位越界 panic ✗
const VARARGS_SCRIPT: &str = r#"def simple(a, *args, **kwargs):
    return (a, args, kwargs)


def nested(a, *args, **kwargs):
    g = lambda: (args, kwargs)
    return g()


print(simple(1, 2, x=3))
print(nested(1, 2, x=3))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const VARARGS_EXPECTED: &str = "(1, (2,), {'x': 3})\n((2,), {'x': 3})\n";

#[test]
fn varargs_varkw_captured_by_closure_agree_with_the_reference() {
    let stdout = run("varargs_varkw_captured_by_closure_agree", VARARGS_SCRIPT);
    assert_eq!(stdout, VARARGS_EXPECTED, "{stdout}");
}

/// `import heapq` ＋ `heapify`／`heappush`／`heappop` ✓（链式赋值那格一收，`heapq` 就通 ✓）
const HEAPQ_SCRIPT: &str = r#"import heapq

data = [5, 1, 8, 3]
heapq.heapify(data)
print(data)
heapq.heappush(data, 0)
print(heapq.heappop(data), heapq.heappop(data))
print(data)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const HEAPQ_EXPECTED: &str = "[1, 3, 8, 5]\n0 1\n[3, 5, 8]\n";

#[test]
fn heapq_roundtrip_agrees_with_the_reference() {
    let stdout = run("heapq_roundtrip_agrees", HEAPQ_SCRIPT);
    assert_eq!(stdout, HEAPQ_EXPECTED, "{stdout}");
}

