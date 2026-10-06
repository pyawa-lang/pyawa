//! **推导式的回归护栏**（第 210 轮）：在修「条件里含调用」这一缺口之前，先把**已经对**的形态钉住 ✓。
//!
//! 已知缺口（本轮定位 ✓，**尚未修** ✗）：推导式的 `if` **条件里含函数调用**时，
//! 执行器在下一轮 `FOR_ITER` 看到错的栈内容 ✗（最小复现见台账第 208／209 轮：
//! `[n for n in xs if len(n) > 1]` ⇒ `指令 70：FOR_ITER 的对象不是本层接线的迭代器`）。
//! 我们发射的字节码与参照 `dis` **逐条一致** ✓ ⇒ 缺口在**执行器**侧 ✓。
//!
//! 本文件只钉**两种必须保持通过的形态**（修缺口时不许弄坏它们 ✓）。

fn run(name: &str, script: &str) -> String {
    // 每个测试**各自一个**临时目录 ✓（同一个 pid、并发跑 ⇒ 先前两个测试互踩 ✗）
    let root = std::env::temp_dir().join(format!("pyawa-comp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "应当成功：{output:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn comprehension_with_a_call_in_the_element_still_works() {
    // 调用在**元素**里 ✓（已对；修条件那一路时不许弄坏 ✓）
    let stdout = run("element_call", "print([len(n) for n in ['a', 'bb']])\n");
    assert!(stdout.contains("[1, 2]"), "元素里的调用：{stdout}");
}

#[test]
fn comprehension_without_a_call_in_the_condition_still_works() {
    // 条件里**没有**调用 ✓（已对；同上 ✓）
    let stdout = run("condition_plain", "print([n for n in ['a', 'bb'] if n != 'a'])\n");
    assert!(stdout.contains("['bb']"), "条件里没有调用：{stdout}");
}
