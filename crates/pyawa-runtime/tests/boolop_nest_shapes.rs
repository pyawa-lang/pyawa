//! **嵌套 boolop 的求值次序与短路保真**（task-6 语义抽例 ✓）——两侧逐行一致。
//!
//! 与 `crates/pyawa-core/tests/boolop_nest.rs`（编译／跳转极性）配对：这一条走**运行期**，
//! 用**带副作用的 `t(name, value)`** 钉住"哪些操作数被求值、按什么次序" ⇒ 短路保真 ✗ 藏不住
//!（先前 4 个形态不 panic 但**值错** ✗：`(a and b) and c` 在 `a` 假时错返回 `c`、
//! `a and (b and c) and d`／`a or (b and c) and d`／`(a or b) or c` 同理 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段脚本**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-boolop-nest-{}-{name}", std::process::id()));
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

/// 每个 `show(表达式)` 打印 `结果 ＋ 已求值操作数序列` ⇒ 值与短路次序**一起**比 ✓。
const SIDE_EFFECT_SCRIPT: &str = r#"log = []


def t(name, value):
    log.append(name)
    return value


def show(result):
    print(result, log)
    del log[:]


show(t("a", 0) and (t("b", 2) and t("c", 3)) or t("d", 4))
show(t("a", 1) and (t("b", 0) and t("c", 3)) or t("d", 4))
show(t("a", 1) and (t("b", 2) and t("c", 0)) or t("d", 4))
show(t("a", 1) and (t("b", 2) and t("c", 3)) or t("d", 4))
show(t("a", 1) and (t("b", 2) or t("c", 3)) or t("d", 4))
show(t("a", 0) and (t("b", 2) or t("c", 3)) or t("d", 4))
show(t("a", 1) and (t("b", 0) or t("c", 3)) or t("d", 4))
show((t("a", 1) and t("b", 2)) and t("c", 3))
show((t("a", 0) and t("b", 2)) and t("c", 3))
show(t("a", 1) and (t("b", 0) and t("c", 3)) and t("d", 4))
show(t("a", 0) or (t("b", 2) and t("c", 3)) and t("d", 4))
show((t("a", 0) or t("b", 0)) or t("c", 3))
show((t("a", 2) or t("b", 0)) or t("c", 3))
show(t("a", 1) and (t("b", 2) and t("c", 0)) or t("d", 5) and t("e", 6))
show(t("a", 1) and (t("b", 2) and t("c", 0)) or t("d", 0) and t("e", 6))
show(t("a", 7) and (t("b", 8) and t("c", 9)) or t("d", 10))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const SIDE_EFFECT_EXPECTED: &str = "4 ['a', 'd']\n\
4 ['a', 'b', 'd']\n\
4 ['a', 'b', 'c', 'd']\n\
3 ['a', 'b', 'c']\n\
2 ['a', 'b']\n\
4 ['a', 'd']\n\
3 ['a', 'b', 'c']\n\
3 ['a', 'b', 'c']\n\
0 ['a']\n\
0 ['a', 'b']\n\
4 ['a', 'b', 'c', 'd']\n\
3 ['a', 'b', 'c']\n\
2 ['a']\n\
6 ['a', 'b', 'c', 'd', 'e']\n\
0 ['a', 'b', 'c', 'd']\n\
9 ['a', 'b', 'c']\n";

#[test]
fn nested_boolop_evaluation_order_agrees_with_the_reference() {
    let stdout = run("nested_boolop_evaluation_order", SIDE_EFFECT_SCRIPT);
    assert_eq!(stdout, SIDE_EFFECT_EXPECTED, "{stdout}");
}
