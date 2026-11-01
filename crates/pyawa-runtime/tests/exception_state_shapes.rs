//! **异常状态是每帧保存／恢复的**（round 62）。
//!
//! 本层原先只有实例级一张 `exception_state` 栈：某个处理块压入的 `PUSH_EXC_INFO` 若没在帧退出时
//! 收掉，`sys.exc_info()` 就会在后续代码里看到**上一个已结束函数**的残留异常——
//! 实测 `import enum` 后 `sys.exc_info()` 给 `KeyError('keep')`、`import unittest` 后给
//! `KeyError(512)`（参照都是 `(None, None, None)` ✓）。修法：`execute` 的 RAII 守卫在**所有**
//! 退出路径上把异常状态截回入口深度 ✓。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    // `PYTHONPATH` 指向仓库 `Lib/`（与官方套件同一条标准库路径 ✓）。
    let library = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Lib");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .env("PYTHONPATH", library)
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
fn exception_state_does_not_leak_between_frames() {
    let script = r#"import sys

try:
    1 / 0
except ZeroDivisionError:
    inside = type(sys.exc_info()[1]).__name__
after = sys.exc_info() == (None, None, None)
import enum
after_enum = sys.exc_info() == (None, None, None)
import unittest
after_unittest = sys.exc_info() == (None, None, None)
print(inside, after, after_enum, after_unittest)
"#;
    assert_eq!(
        run("exception-state", script),
        "ZeroDivisionError True True True\n"
    );
}
