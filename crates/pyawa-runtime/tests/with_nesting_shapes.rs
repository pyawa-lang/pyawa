//! **嵌套 `with` 的退出路径**（round 45，`P3-42` 编译器真 bug）。
//!
//! 症状：`test_frame` 在 `test_proxy_key_unhashables` 处**状态 5**（`FOR_ITER` 拿到的不是迭代器，
//! 而是 `A` 实例）。最小复现不涉 generator：`for `…`: with A(): with B(): pass` 的第二轮
//! `FOR_ITER` 取到 `A` —— 外层 `with` 的 `(上下文, __exit__)` 两格**留在栈上** ✗。
//!
//! 两个根因（都在 `compile/emitter`）：
//! 1. **内层 `with` 的正常出口跳错**：`emit_rest_and_tail` 里"有外层循环 ⇒ 跳回循环续点"那条
//!    先于块尾兜底 ⇒ 内层 `with` 退出后**跳过外层 `with` 的退出调用** ✗。修法：`with_depth > 0`
//!    （正处在某层 `with` 体内）时**不跳循环续点**，改走块尾兜底（正好是外层 `with` 体出口 ✓）。
//! 2. **`break`／`continue` 不退出循环体内的 `with`**：`LoopFrame` 增 `with_levels`（进入循环时
//!    `with_exit_stack` 的层数）⇒ 出口臂从内向外发退出调用（循环体**外**的 `with` 不退 ✓）。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致；覆盖嵌套
//! `with`＋循环、多上下文项、`try/except`、`return`／`break`／`continue` 五条出口。

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

const WITH_SCRIPT: &str = r#"class A:
    def __init__(self, name):
        self.name = name
    def __enter__(self):
        print("enter", self.name)
        return self
    def __exit__(self, *info):
        print("exit", self.name, info[0].__name__ if info[0] else None)
        return False

def run_loop():
    for obj in (1, 2):
        with A("outer"):
            with A("inner"):
                print("body", obj)
            print("between", obj)
        print("after", obj)

print("=== nested loop")
run_loop()

print("=== multi-item")
for obj in (1, 2):
    with A("one"), A("two"):
        print("both", obj)
print("=== nested exception suppressed")
for obj in (1, 2):
    with A("outer"):
        with A("inner"):
            try:
                raise ValueError("boom")
            except ValueError:
                print("caught", obj)
        print("between", obj)
print("=== return from nested")
def drive(value):
    with A("outer"):
        with A("inner"):
            if value:
                return "early"
    return "late"
print(drive(True))
print(drive(False))
print("=== break in nested")
for obj in (1, 2, 3):
    with A("outer"):
        with A("inner"):
            if obj == 2:
                break
            print("alive", obj)
print("=== continue in nested")
for obj in (1, 2):
    with A("outer"):
        with A("inner"):
            if obj == 1:
                continue
            print("continue-alive", obj)
print("done")"#;

const WITH_EXPECTED: &str = "=== nested loop\nenter outer\nenter inner\nbody 1\nexit inner None\nbetween 1\nexit outer None\nafter 1\nenter outer\nenter inner\nbody 2\nexit inner None\nbetween 2\nexit outer None\nafter 2\n=== multi-item\nenter one\nenter two\nboth 1\nexit two None\nexit one None\nenter one\nenter two\nboth 2\nexit two None\nexit one None\n=== nested exception suppressed\nenter outer\nenter inner\ncaught 1\nexit inner None\nbetween 1\nexit outer None\nenter outer\nenter inner\ncaught 2\nexit inner None\nbetween 2\nexit outer None\n=== return from nested\nenter outer\nenter inner\nexit inner None\nexit outer None\nearly\nenter outer\nenter inner\nexit inner None\nexit outer None\nlate\n=== break in nested\nenter outer\nenter inner\nalive 1\nexit inner None\nexit outer None\nenter outer\nenter inner\nexit inner None\nexit outer None\n=== continue in nested\nenter outer\nenter inner\nexit inner None\nexit outer None\nenter outer\nenter inner\ncontinue-alive 2\nexit inner None\nexit outer None\ndone\n";

#[test]
fn nested_with_exits_match_the_reference() {
    assert_eq!(run("with", WITH_SCRIPT), WITH_EXPECTED);
}
