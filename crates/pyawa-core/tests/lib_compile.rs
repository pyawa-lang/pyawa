//! **`Lib/` 全量编译扫描** ✓（第 207 轮）：**只编译、不执行** ✓ —— 配上编译器里的**不变量检查**
//! （`compile/verify.rs` ✓），覆盖面就从"手写夹具"扩到**整个标准库** ✓。`Lib/os.py` 那个
//! "局部槽越界"（`def walk(...)` ✓）正是被它当场抓住的 ✓。
//!
//! **`KNOWN` 的用法**（与夹具 `covered: false` 同一条纪律 ✓）：列在这里的是**已知**编不过的文件 ✓，
//! 每条写明原由 ✓；测试**两个方向**都查 —— 其它文件必须全过 ✓，`KNOWN` 里的必须**仍然**不过 ✓
//! （哪天修好了 ⇒ 测试**当场**提醒把它删掉 ✓）。

use pyawa_core::compile::{CheckTier, Mode, compile};
use std::path::{Path, PathBuf};

/// 已知编不过的（相对 `Lib/` 的路径 ⇒ 原由）。
const KNOWN: &[(&str, &str)] = &[
    // **第 261 轮新增的"落格"检查抓到的三个** ✗（`MS-19` 纪律：先登记、附原由、再修 ✓）：
    // 检查内容是「`*_DEREF`／`MAKE_CELL` 的槽号必须落在 **Cell／Free** 格上」✓ —— 槽号**在范围内**
    // 但指着 `Local` 格 ✗ ⇒ 运行期会走 `Frame::set_cell()` ⇒ 报 `SlotOutOfRange` ✗
    //（这正是 `DIV-9` 的根 ✓）。
    // **已定位到相位／口径** ✓：序言里的 `MAKE_CELL` 槽号与**最终** `localsplus` 布局对不上 ✗
    //（`os.py` 要槽 4、`posixpath.py` 要槽 4、`site.py` 要槽 10 ✓）⇒ 下一轮修 ✓。
    ("os.py", "`MAKE_CELL` 要槽 4、而最终第 4 格是 Local ✗（`_create_environ_mapping` ✓）⇒ 待修 ✓"),
    ("posixpath.py", "同型：`MAKE_CELL` 要槽 4 ✗ ⇒ 待修 ✓"),
    ("site.py", "同型：`MAKE_CELL` 要槽 10 ✗ ⇒ 待修 ✓"),
];

fn python_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            python_files(&path, out);
        } else if path.extension().map(|kind| kind == "py").unwrap_or(false) {
            out.push(path);
        }
    }
}

#[test]
fn every_lib_file_compiles_under_the_invariants() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Lib");
    assert!(root.is_dir(), "找不到 Lib/：{}", root.display());
    let mut files = Vec::new();
    python_files(&root, &mut files);
    files.sort();
    assert!(!files.is_empty(), "Lib/ 里没找到 .py");

    let mut failed: Vec<(String, String)> = Vec::new();
    for path in &files {
        let relative = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(source) = std::fs::read_to_string(path) else {
            failed.push((relative, "读不了".to_owned()));
            continue;
        };
        if let Err(error) = compile(&source, &relative, Mode::PurePython, CheckTier::Shallow, 0) {
            failed.push((relative, format!("{error:?}")));
        }
    }

    // 方向一：除 `KNOWN` 之外都必须过 ✓。
    let unexpected: Vec<&(String, String)> = failed
        .iter()
        .filter(|(name, _)| !KNOWN.iter().any(|(known, _)| known == name))
        .collect();
    // 方向二：`KNOWN` 里的必须**仍然**不过 ✓（修好了就该删 ✓）。
    let stale: Vec<&(&str, &str)> = KNOWN
        .iter()
        .filter(|(known, _)| !failed.iter().any(|(name, _)| name == known))
        .collect();
    assert!(
        unexpected.is_empty(),
        "有文件没过编译期不变量（要修，或按纪律登记进 KNOWN）：{unexpected:#?}"
    );
    assert!(stale.is_empty(), "KNOWN 里这些已经能编过了 ⇒ 请删掉：{stale:#?}");
}
