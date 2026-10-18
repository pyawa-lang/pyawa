//! 挂账 bug 批B 之二：**未捕获异常消息**（`pa_errmsg`）少一层 `repr`。
//!
//! **根因**：ABI 的 `exception_message` 取"首实参的裸 `str` 内容" ⇒ `KeyError("missing")` 报
//! `KeyError: missing`；参照的未捕获报错尾行是 `str(exception)` 口径 ⇒ `KeyError: 'missing'`
//! （`KeyError` 单实参走实参 `repr`，已在 `builtin_objects::exception_str` 实现）。
//! 修法：core 的 `executor::exception_uncaught_text` 复用异常 `str` 槽；ABI 只调用它。
//!
//! **catch 后的 `repr(e)`／`str(e)` 不属于本 bug**（本来就对）⇒ 同一文件守回归，防止修坏。
//! **选型（为什么在 runtime tests）**：CLI 的 stderr 是"未捕获消息"的唯一可观察面。
//! 期望值均由本机 `python3`（3.14.4）跑同一段脚本取得。

use std::process::Command;

/// 跑脚本，返回（退出码, stdout, stderr）。
fn run(name: &str, script: &str) -> (Option<i32>, String, String) {
    let root = std::env::temp_dir().join(format!("pyawa-excmsg-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn uncaught_key_error_message_has_the_repr_layer() {
    // 参照 python3 尾行：`KeyError: 'missing'`（本层先前少一层 repr）。
    let (code, _stdout, stderr) = run("keyerror-uncaught", "raise KeyError(\"missing\")\n");
    assert_eq!(code, Some(1), "脚本异常必须退 1");
    assert!(
        stderr.contains("KeyError: 'missing'"),
        "未捕获消息要带 repr 层（参照 `KeyError: 'missing'`）：{stderr}"
    );
    assert!(
        !stderr.contains("KeyError: missing"),
        "不许再少一层 repr：{stderr}"
    );
}

#[test]
fn uncaught_key_error_with_non_string_key_uses_repr_too() {
    // `KeyError(5)` ⇒ 参照 `KeyError: 5`（KeyError 的单实参一律是实参 repr）。
    let (code, _stdout, stderr) = run("keyerror-int", "raise KeyError(5)\n");
    assert_eq!(code, Some(1));
    assert!(stderr.contains("KeyError: 5"), "stderr={stderr}");
}

#[test]
fn uncaught_no_argument_exception_is_just_the_type_name() {
    // `raise KeyError()` ⇒ 参照尾行只有 `KeyError`（空 `str` 不补冒号）。
    let (code, _stdout, stderr) = run("keyerror-empty", "raise KeyError()\n");
    assert_eq!(code, Some(1));
    assert!(stderr.contains("KeyError"), "stderr={stderr}");
    assert!(!stderr.contains("KeyError:"), "空消息不能补冒号：{stderr}");
}

#[test]
fn uncaught_value_error_message_is_the_str_of_its_argument() {
    // 非 KeyError 的单实参异常：`str(e)`＝实参的 `str` ⇒ `ValueError: boom`（行为不变）。
    let (code, _stdout, stderr) = run("valueerror", "raise ValueError(\"boom\")\n");
    assert_eq!(code, Some(1));
    assert!(stderr.contains("ValueError: boom"), "stderr={stderr}");
}

#[test]
fn uncaught_multi_argument_exception_uses_tuple_rendering() {
    // 多实参：参照 `str(Exception('a', 'b'))` ⇒ `('a', 'b')` ⇒ 尾行 `Exception: ('a', 'b')`。
    let (code, _stdout, stderr) = run("multi", "raise Exception(\"a\", \"b\")\n");
    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("Exception: ('a', 'b')"),
        "多实参按 `str(exception)` 渲染：{stderr}"
    );
}

#[test]
fn caught_key_error_str_and_repr_are_unchanged() {
    // 回归：catch 之后 `repr(e)`／`str(e)`／`e.args[0]` 本来就与参照一致 ⇒ 修"未捕获消息"
    // 不能把它们改坏（参照：`KeyError('missing')` / `'missing'` / `missing`）。
    let (code, stdout, stderr) = run(
        "keyerror-caught",
        "try:\n    raise KeyError(\"missing\")\nexcept KeyError as e:\n    print(repr(e))\n    print(str(e))\n    print(e.args[0])\n",
    );
    assert_eq!(code, Some(0), "stderr={stderr}");
    assert_eq!(stdout, "KeyError('missing')\n'missing'\nmissing\n");
}
