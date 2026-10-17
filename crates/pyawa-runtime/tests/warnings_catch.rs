//! **挂账真 bug：`with` 体内再套 `try` ⇒ `StackUnderflow`**（`warnings.catch_warnings(record=True)` 一族）。
//!
//! 根因（`compile/emitter.rs` 的 `Statement::With` 臂）：`with` 体在值栈上留 **2 格／项**
//! （上下文 ＋ `__exit__`），但发体时没把这层格数记进异常表的 `depth` ⇒ 体内嵌套 `try` 的
//! 条目少 2 ⇒ 抛出时 `dispatch_raise` 把 `with` 的两格一起弹掉 ⇒ 退出调用 `CALL 3` 栈空。
//! 修法：体期间单记一项 `with_depth`（`2 × items.len()`），`record_exception` 记 `depth` 时加上；
//! 没并进 `handler_depth`，因为后者还给 `break`／`continue` 数 `POP_EXCEPT` 用。
//!
//! **选型（为什么在 runtime tests）**：验收要的是端到端可观察行为 —— 编译 → 执行 → `Lib/` import
//! → `print` 与退出码；`print` 一族是 runtime 层能力，core tests 要手工装内建与类体脚手架才能看同样
//! 的证据。期望值全部由本机 `python3`（3.14.4）跑**同一段探针**取得（`MS-17`：不改变 `.py` 既有
//! 可观察语义）。
//!
//! 如实边界（**不是本 bug**）：原始 `warnings.warn("boom")` 那条还要 `sys._getframe(1)`，而
//! `sys._getframe` 目前只接 `depth == 0`（长期挂账，见台账卷 04 第 616 轮前后）⇒ 本文件第 3 例改走
//! `warnings.warn_explicit`（同一套 `with`／`try` 发射面，`catch_warnings(record=True)` 的
//! `len(caught)`／消息语义不变），原始的 `warnings.warn` 版本留待 `_getframe` 帧链接通后再补。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-warnings-catch-{}-{name}", std::process::id()));
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

/// 最小复现 ＋ 对照（不抛异常）＋ **多项 `with`**（`depth` 要加 `2 × 2`）。
const WITH_TRY_SCRIPT: &str = r#"class C:
    def __init__(self, name):
        self.name = name
    def __enter__(self):
        print("enter", self.name)
        return self
    def __exit__(self, *args):
        print("exit", self.name)
        return False


def plain():
    with C("p"):
        try:
            pass
        except ValueError:
            print("unexpected")


def caught_min():
    with C("m"):
        try:
            raise ValueError
        except ValueError:
            print("caught")


def multi():
    with C("a") as left, C("b") as right:
        try:
            raise KeyError
        except KeyError:
            print("caught-multi")
    return left.name, right.name


plain()
caught_min()
print(multi())
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const WITH_TRY_EXPECTED: &str = "enter p\n\
exit p\n\
enter m\n\
caught\n\
exit m\n\
enter a\n\
enter b\n\
caught-multi\n\
exit b\n\
exit a\n\
('a', 'b')\n";

/// 体内 `return` 的**退出调用复制件**（`emitter.rs` 的 epilogue 路径）＋ `for` ＋ `try` 组合。
const WITH_EPILOGUE_SCRIPT: &str = r#"class C:
    def __init__(self, name):
        self.name = name
    def __enter__(self):
        print("enter", self.name)
        return self
    def __exit__(self, *args):
        print("exit", self.name)
        return False


def early():
    with C("r"):
        try:
            raise ValueError
        except ValueError:
            return "early"
    return "late"


def looped():
    out = []
    with C("l"):
        for x in [1, 2, 3]:
            try:
                if x == 2:
                    raise ValueError
                out.append(("ok", x))
            except ValueError:
                out.append(("caught", x))
    return out


print(early())
print(looped())
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const WITH_EPILOGUE_EXPECTED: &str = "enter r\n\
exit r\n\
early\n\
enter l\n\
exit l\n\
[('ok', 1), ('caught', 2), ('ok', 3)]\n";

/// 原始症状的 `warnings` 路径：`catch_warnings(record=True)` ＋ `simplefilter`（它自己的
/// `with _wm._lock: try: … except ValueError` 正是最初触发面）＋ 一条真被记录下来的告警。
const WARNINGS_SCRIPT: &str = r#"import warnings

warnings.simplefilter("always")

with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    warnings.warn_explicit("boom", UserWarning, "probe.py", 1)

print(len(caught))
print(str(caught[0].message))
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const WARNINGS_EXPECTED: &str = "1\nboom\n";

#[test]
fn with_body_try_dispatch_matches_the_reference() {
    let stdout = run("with_try", WITH_TRY_SCRIPT);
    assert_eq!(stdout, WITH_TRY_EXPECTED, "{stdout}");
}

#[test]
fn with_body_return_copy_and_for_loop_match_the_reference() {
    let stdout = run("with_epilogue", WITH_EPILOGUE_SCRIPT);
    assert_eq!(stdout, WITH_EPILOGUE_EXPECTED, "{stdout}");
}

#[test]
fn catch_warnings_record_matches_the_reference() {
    let stdout = run("warnings", WARNINGS_SCRIPT);
    assert_eq!(stdout, WARNINGS_EXPECTED, "{stdout}");
}
