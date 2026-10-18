//! 挂账 bug 批B 之一：`yield from <非内建可迭代对象>`（`GET_YIELD_FROM_ITER`）。
//!
//! **根因**：`executor.rs` 的 `GET_YIELD_FROM_ITER` 只走 `iterator_type_for`（只认内建容器），
//! 没回退 `iter_value` 的 `__iter__` 协议 ⇒ `ChainMap` 的 `Views.__iter__`（`yield from self._mapping`）
//! 报 `指令 16 …只接线了…的内建迭代器`。修法：回退改用 `iter_value`（与 `GET_ITER` 同一处实现）。
//!
//! **选型（为什么在 runtime tests）**：验收要的是端到端可观察行为——编译 → 执行 → `print`；
//! `ChainMap` 那条还要 `Lib/` 的 import 面。期望值全部由本机 `python3`（3.14.4）跑**同一段脚本**取得
//! （`MS-17`：不改变 `.py` 既有可观察语义）。

use std::process::Command;

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-chainmap-{}-{name}", std::process::id()));
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
fn yield_from_a_custom_iterable_follows_the_iter_protocol() {
    // 独立最小复现（不依赖 `collections`）：`yield from` 的对象**只有** `__iter__`。
    // 参照 python3：打印 `[1, 2]`。
    let stdout = run(
        "custom",
        "class I:\n    def __iter__(self):\n        return iter([1, 2])\n\n\ndef gen():\n    yield from I()\n\n\nprint(list(gen()))\n",
    );
    assert_eq!(stdout, "[1, 2]\n", "`yield from` 必须走 `__iter__` 协议");
}

#[test]
fn yield_from_a_generator_still_works() {
    // 既有路径回归：`yield from <生成器>` 仍原样驱动（新回退不能改坏它）。
    let stdout = run(
        "generator",
        "def sub():\n    yield 1\n    yield 2\n\n\ndef gen():\n    yield from sub()\n\n\nprint(list(gen()))\n",
    );
    assert_eq!(stdout, "[1, 2]\n");
}

#[test]
fn chainmap_keys_view_is_iterable() {
    // 真受害者：`collections.ChainMap.keys()` 的 `KeysView.__iter__` 是 `yield from self._mapping`
    // ⇒ `list(cm.keys())` 先要能构造/驱动那条生成器。
    // 参照 python3：`1` / `2` / `['x', 'y']`。
    let stdout = run(
        "chainmap",
        "from collections import ChainMap\n\ncm = ChainMap({\"x\": 1}, {\"y\": 2})\nprint(cm[\"x\"])\nprint(len(list(cm.keys())))\nprint(sorted(cm.keys()))\n",
    );
    assert_eq!(
        stdout, "1\n2\n['x', 'y']\n",
        "ChainMap 的键视图必须可迭代（与参照一致）"
    );
}
