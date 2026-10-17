//! **import 语义两格**（第 408 轮，task-16 ✓）——两侧逐行一致。
//!
//! ① **循环相对导入回退**（CPython issue #17636）：`pkg/__init__ → a → b → from . import a`
//! 时，`a` 已在 `sys.modules` 但**还没挂成 `pkg` 的属性** ⇒ `IMPORT_FROM` 属性失败后必须回退查
//! `sys.modules[<module.__name__>.<name>]`（参照 `import_from` 同款 ✓）。
//! ② **失败模块不残留在 `sys.modules`**：`exec` 抛错 ⇒ 移掉半截模块（参照 `_load_unlocked`
//! 的 `if sys.modules.get(name) is module: del …` ✓）；顺带钉**嵌套失败**（`outer → mymod` 两格都清 ✓）
//! 与**成功回退换模块**的兼容（`decimal` → `_pydecimal`：`sys.modules['decimal'].__name__`
//! 仍是 `decimal` ✓，task-11 那条收尾不回归 ✓）。
//!
//! 期望输出由本机 `python3`（3.14.4）跑**同一棵临时目录树**取得（`-S`＋`PYTHONPATH=本仓 Lib`
//! 保证 `decimal` 同源 ✓；不是手写 ✓）。

fn run_python3(name: &str, decimal_only: bool) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-import-sem-ref-{}-{name}", std::process::id()));
    prepare_tree(&root, decimal_only);
    let output = std::process::Command::new("python3")
        .arg("-S")
        .arg(root.join("probe.py"))
        .current_dir(&root)
        .env("PYTHONPATH", concat!(env!("CARGO_MANIFEST_DIR"), "/../../Lib"))
        .output()
        .expect("跑 python3");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "python3 侧失败：{:?}", output);
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn run_pyawa(name: &str, decimal_only: bool) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-import-sem-{}-{name}", std::process::id()));
    prepare_tree(&root, decimal_only);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(root.join("probe.py"))
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

fn prepare_tree(root: &std::path::Path, decimal_only: bool) {
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root.join("pkg")).expect("建目录");
    std::fs::write(root.join("pkg/__init__.py"), "from . import a\n").expect("写");
    std::fs::write(root.join("pkg/a.py"), "from . import b\nVALUE_A = 1\n").expect("写");
    std::fs::write(root.join("pkg/b.py"), "from . import a\nVALUE_B = 2\nA_MODULE = a\n").expect("写");
    std::fs::write(root.join("mymod.py"), "import no_such_module_xyz\n").expect("写");
    std::fs::write(root.join("outer.py"), "import mymod\n").expect("写");
    let text = if decimal_only { DECIMAL_SCRIPT } else { PROBE_SCRIPT };
    std::fs::write(root.join("probe.py"), text).expect("写");
}

const PROBE_SCRIPT: &str = "import sys\n\nimport pkg\n\nprint(\"pkg_a\", pkg.a.VALUE_A, pkg.a is sys.modules[\"pkg.a\"])\nprint(\"pkg_b\", pkg.b.VALUE_B, pkg.b is sys.modules[\"pkg.b\"])\nprint(\"b_saw_a\", pkg.b.A_MODULE is pkg.a)\nprint(\"from_pkg\", __import__(\"pkg\", fromlist=[\"b\"]).b is pkg.b)\n\nprint(\"mymod_before\", \"mymod\" in sys.modules)\ntry:\n    import mymod\nexcept ImportError as exc:\n    print(\"mymod_error\", exc)\nprint(\"mymod_after\", \"mymod\" in sys.modules)\ntry:\n    import mymod\nexcept ImportError as exc:\n    print(\"mymod_retry_error\", exc)\nprint(\"mymod_after_retry\", \"mymod\" in sys.modules, \"no_such_module_xyz\" in sys.modules)\n\ntry:\n    import outer\nexcept ImportError as exc:\n    print(\"outer_error\", exc)\nprint(\"outer_after\", \"outer\" in sys.modules, \"mymod\" in sys.modules)\n\nimport decimal\nprint(\"decimal_name\", sys.modules[\"decimal\"].__name__)\nprint(\"decimal_face\", hasattr(decimal, \"Decimal\"), hasattr(decimal, \"getcontext\"))\n";

const DECIMAL_SCRIPT: &str = "import sys\nimport decimal\nprint(\"decimal_name\", sys.modules[\"decimal\"].__name__)\nprint(\"decimal_face\", hasattr(decimal, \"Decimal\"), hasattr(decimal, \"getcontext\"))\n";

#[test]
fn circular_relative_import_and_failed_module_cleanup_agree_with_the_reference() {
    let expected = run_python3("semantics", false);
    let actual = run_pyawa("semantics", false);
    assert_eq!(actual, expected, "{actual}");
}

#[test]
fn successful_module_replacement_fallback_still_agrees_with_the_reference() {
    let expected = run_python3("decimal", true);
    let actual = run_pyawa("decimal", true);
    assert_eq!(actual, expected, "{actual}");
}
