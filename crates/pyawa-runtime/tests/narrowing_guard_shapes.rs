//! **4 位融合槽位的上界守卫（横扫补齐）**（task-21 第 3 轮 ✓）。
//!
//! 同族真 bug 共四例 ✗（都是"把超出字段宽度的值 `as u8`/塞半字节"）：
//! ① `EXTENDED_ARG` 名字下标（第 725 轮 ✓）；② `LOAD_CONST` 延迟入池（task-20 第 2 轮 ✓）；
//! ③ `SWAP` 窥孔长度（task-14 ✓）；④ `STORE_FAST_LOAD_FAST` 槽位（task-21 第 2 轮 ✓）。
//! 本笔又补 **字典推导式** `LOAD_FAST_BORROW_LOAD_FAST_BORROW` 那处的 `(键槽 << 4) | 值槽` ✗
//! （槽 ≥ 16 时截断 ⇒ 读别的槽 ✓），并把**槽／下标／计数**那类 `as u8` 写码点**统一改走
//! `emit_indexed`**（下标 > 255 自动补 `EXTENDED_ARG` ✓；≤ 255 逐字节不变 ✓）。
//!
//! 探针：字典推导式放在**十六个局部**的函数里 ⇒ 键／值槽位 ≥ 16 ✓（先前会融合成错的半字节 ✓）。

const DICT_COMP_SCRIPT: &str = r#"def wide(items):
    a0 = a1 = a2 = a3 = a4 = a5 = a6 = a7 = 0
    a8 = a9 = a10 = a11 = a12 = a13 = a14 = a15 = 1
    return {k: k + 1 for k in items}


print(list(wide([1, 2, 3]).items()))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const DICT_COMP_EXPECTED: &str = "[(1, 2), (2, 3), (3, 4)]\n";

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-sweep-{}-{name}", std::process::id()));
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

#[test]
fn wide_dict_comprehension_agrees_with_the_reference() {
    let stdout = run("dict_comp", DICT_COMP_SCRIPT);
    assert_eq!(stdout, DICT_COMP_EXPECTED, "{stdout}");
}

/// **名字表 > 255 项**的 `STORE_NAME`／`LOAD_NAME`（`emit_indexed` 补 `EXTENDED_ARG` ✓）：
/// 脚本**按需生成** 300 个名字 ✓，断言结果 **自证**（对不上就是下标被截断 ✓）。
#[test]
fn name_index_above_255_still_stores_and_loads() {
    let mut source = String::new();
    for index in 0..300 {
        source.push_str(&format!("name_{index} = {index}\n"));
    }
    source.push_str("total = 0\n");
    for index in 0..300 {
        source.push_str(&format!("total += name_{index}\n"));
    }
    source.push_str("print(total)\n");
    let stdout = run("wide_names", &source);
    assert_eq!(stdout.trim(), (0..300i64).sum::<i64>().to_string());
}
