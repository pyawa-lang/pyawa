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
    // **第 277 轮再次清空** ✓：第 263 轮登记的那一条（`site.py` 的 `register_readline`：
    // 推导式目标 `p` 晚到 ⇒ 序言 `MAKE_CELL` 要槽 10、最终布局是 11 ✗）已修 ✓ ——
    // 根因与 `DIV-9` 同族：**预扫没把推导式目标收全** ✗。第 264 轮的修法只认
    // “右值恰好就是推导式” ✗，而 `sys.path = [p for p in …]` 走的是 **`AssignAttr`**（属性目标 ✓）、
    // 那两条臂在 `collect_locals` 里**根本不存在** ✗ ⇒ 现在：预扫改成**递归表达式遍历**（含 `:=` ✓），
    // 并把 `AssignAttr`／`AssignSubscript`／`AssignChained`／`AssignTuple`／`AugAssign`／`Return`／
    // `Expression`／`If`／`While`／`For`／`With` 各臂的表达式一并交进去 ✓
    // ⇒ **`Lib/` 全量过编译期不变量（`KNOWN` 为空 ✓）**。
    //
    // **round 167／168**：`Lib/test/tokenizedata/` 是**从上游复制来的测试数据包**（`test.*` 由本层
    // `Lib/test` 提供 ⇒ 数据只能放在这里）；其中三个是**故意编不过**的样本，供 tokenizer 相关测试
    // （`test_unicode_identifiers`／`test_source_encoding` 等）断言 `SyntaxError` ✓ ——
    // 它们是**数据**，不是可编译模块 ⇒ 按本文件的纪律登记 ✓。
    (
        "test/tokenizedata/bad_coding2.py",
        "故意坏样本：源文件带 U+FEFF（BOM）但声明非 UTF-8 编码 ⇒ SyntaxError（tokenizer 测试数据）",
    ),
    (
        "test/tokenizedata/badsyntax_3131.py",
        "故意坏样本：含 `€`（U+20AC）非法标识符 ⇒ SyntaxError（test_unicode_identifiers 断言用）",
    ),
    (
        "test/tokenizedata/badsyntax_pep3120.py",
        "故意坏样本：PEP 3120 非法源编码样本 ⇒ SyntaxError（编码测试数据）",
    ),
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
        // **临时诊断**（第 292 轮，随后还原）：panic 时也要知道是哪个文件 ✓
        if std::env::var_os("PYAWA_TRACE_COMPILE_FILE").is_some() {
            eprintln!("[编译扫描] {relative}");
        }
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
