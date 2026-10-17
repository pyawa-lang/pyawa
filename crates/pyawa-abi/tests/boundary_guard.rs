//! **`CX-11`（panic 不跨 FFI 边界）的机械化守卫**：枚举 `crates/pyawa-abi/src/` 里**每一个**
//! `#[unsafe(no_mangle)]` 导出，断言它要么在**自己的函数体**里走 `boundary(`／`boundary_ptr(`，
//! 要么在**显式白名单**里且带理由。
//!
//! 理由的**一处真相**在 `src/lib.rs` 的源码注释里（版本三件套上的 ``CX-11-EXEMPT`` 行）；
//! 本文件另存一份显式的**期望名单** `EXEMPT_EXPECTED`——两份必须一致（改一处不改另一处即红）。
//!
//! 依据：`docs/CONSTRAINTS.md` 的 `CX-11`、`docs/SPEC-c-abi.md` 的 `AB-3`。
//! 本守卫**不靠人肉记忆**：新增一个未包边界的入口 ⇒ 红。

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// 源码注释里的白名单标记（`src/lib.rs` 的版本三件套上各出现一次）。
const EXEMPT_MARKER: &str = "CX-11-EXEMPT";
/// 允许不包边界的入口（**显式**第二道闸：改这里必须同时给出源码理由）。
const EXEMPT_EXPECTED: &[&str] = &["pa_abi_size", "pa_abi_version", "pa_version"];

/// 一个 `#[unsafe(no_mangle)]` 导出及其解析出的源码片段。
#[derive(Debug)]
struct Export {
    file: String,
    name: String,
    /// 从属性到函数体 `{` 之前的源码（含 `extern "C"` 等签名信息）。
    signature: String,
    /// 函数体：从 `{` 到配平的 `}`（含两端）。
    body: String,
    /// 紧邻属性上方的 `///` 文档块。
    doc: String,
}

/// 递归收集 `dir` 下的 `.rs`。
fn collect_rust_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|error| panic!("读不到 {}：{error}", dir.display()))
    {
        let path = entry.expect("目录项").path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

/// `src/` 下的全部 `.rs`（递归；按路径排序，保证判定确定）。
fn source_files() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    collect_rust_files(&root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("读不到 {}：{error}", path.display()));
            (name, text)
        })
        .collect()
}

/// 从 `open`（一个 `{`）出发，返回与之配平的 `}` 的下标。
///
/// 跳过字符串／字符字面量与注释里的花括号（`format!("{}")` 一类不能干扰）。
fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = open;
    let mut depth = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'/' if index + 1 < bytes.len() && bytes[index + 1] == b'/' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if index + 1 < bytes.len() && bytes[index + 1] == b'*' => {
                index += 2;
                while index + 1 < bytes.len()
                    && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index += 2;
            }
            b'"' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index += 2;
                        continue;
                    }
                    if bytes[index] == b'"' {
                        index += 1;
                        break;
                    }
                    index += 1;
                }
            }
            b'\'' => {
                // 字符字面量（`'\0'`／`'a'`）与生命周期（`'a`）要分开
                if index + 1 < bytes.len() && bytes[index + 1] == b'\\' {
                    index += 2;
                    while index < bytes.len() && bytes[index] != b'\'' {
                        index += 1;
                    }
                    index += 1;
                } else if index + 2 < bytes.len() && bytes[index + 2] == b'\'' {
                    index += 3;
                } else {
                    index += 1;
                }
            }
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth -= 1;
                index += 1;
                if depth == 0 {
                    return Some(index - 1);
                }
            }
            _ => index += 1,
        }
    }
    None
}

/// 属性上方紧邻的 `///` 文档块（不含属性行本身）。
fn doc_block_before(source: &str, attr: usize) -> String {
    let bytes = source.as_bytes();
    let mut line_start = attr;
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }
    let mut region_start = line_start;
    let mut cursor = line_start;
    while cursor > 0 {
        let previous_end = cursor - 1; // 指向上一个换行
        let mut previous_start = previous_end;
        while previous_start > 0 && bytes[previous_start - 1] != b'\n' {
            previous_start -= 1;
        }
        let line = &source[previous_start..previous_end];
        if line.trim_start().starts_with("///") {
            region_start = previous_start;
            cursor = previous_start;
        } else {
            break;
        }
    }
    source[region_start..line_start].to_owned()
}

/// 枚举一个源文件里的全部 `#[unsafe(no_mangle)]` 导出。
fn exports_in(file: &str, source: &str) -> Vec<Export> {
    const ATTR: &str = "#[unsafe(no_mangle)]";
    let mut exports = Vec::new();
    let mut cursor = 0usize;
    while let Some(offset) = source[cursor..].find(ATTR) {
        let attr = cursor + offset;
        let fn_pos = source[attr..]
            .find("fn ")
            .map(|position| attr + position)
            .unwrap_or_else(|| panic!("{file}: {ATTR} 之后没有 `fn`"));
        let name_start = fn_pos + "fn ".len();
        let name: String = source[name_start..]
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        assert!(!name.is_empty(), "{file}: 解析不出导出函数名");
        let open = source[fn_pos..]
            .find('{')
            .map(|position| fn_pos + position)
            .unwrap_or_else(|| panic!("{file}: {name} 没有函数体"));
        let signature = source[attr..open].to_owned();
        let close = matching_brace(source, open)
            .unwrap_or_else(|| panic!("{file}: {name} 的函数体花括号不配平"));
        exports.push(Export {
            file: file.to_owned(),
            name,
            signature,
            body: source[open..=close].to_owned(),
            doc: doc_block_before(source, attr),
        });
        cursor = close + 1;
    }
    exports
}

/// 守卫的**自检**：证明解析器真的分得开"包了边界"与"裸的"（否则上面那条断言可能空转）。
#[test]
fn the_extractor_separates_bounded_from_naked_exports() {
    let sample = r#"
        #[unsafe(no_mangle)]
        pub extern "C" fn wrapped() -> i32 { boundary(|| 0) }

        /// **CX-11-EXEMPT 白名单**：无 panic 面——纯常量，不分配。
        #[unsafe(no_mangle)]
        pub extern "C" fn constant() -> i32 { 7 }

        #[unsafe(no_mangle)]
        pub extern "C" fn naked() -> i32 { naive_helper() }
    "#;
    let exports = exports_in("<sample>", sample);
    let names: Vec<&str> = exports.iter().map(|export| export.name.as_str()).collect();
    assert_eq!(names, ["wrapped", "constant", "naked"]);
    assert!(exports[0].body.contains("boundary("), "包了边界的要认出来");
    assert!(!exports[1].body.contains("boundary("), "常量入口没有 boundary");
    assert!(exports[1].doc.contains(EXEMPT_MARKER), "常量入口带源码理由");
    assert!(!exports[2].body.contains("boundary("), "裸入口没有 boundary");
    assert!(
        !exports[2].doc.contains(EXEMPT_MARKER),
        "裸入口不得靠标记蒙混（自检要能戳破）"
    );
}

/// `CX-11` 的主断言：每个导出要么自带边界，要么在显式白名单且源码里写明理由。
#[test]
fn every_export_is_bounded_or_explicitly_exempt() {
    let mut exports = Vec::new();
    for (file, source) in source_files() {
        exports.extend(exports_in(&file, &source));
    }
    assert!(
        exports.len() >= 40,
        "守卫应当枚举到全部导出，只找到 {} 个（解析器坏了？）",
        exports.len()
    );

    let mut seen = BTreeSet::new();
    let mut unbounded = Vec::new();
    for export in &exports {
        assert!(
            seen.insert(export.name.clone()),
            "导出重名：{}（{}）",
            export.name,
            export.file
        );
        assert!(
            export.signature.contains("extern \"C\""),
            "{}: {} 不是 extern \"C\"（AB-3）",
            export.file,
            export.name
        );
        let bounded =
            export.body.contains("boundary(") || export.body.contains("boundary_ptr(");
        if !bounded {
            unbounded.push(export);
        }
    }
    assert!(
        !seen.is_empty(),
        "一个导出都没枚举到 ⇒ 守卫失效"
    );

    let mut exempt = Vec::new();
    for export in unbounded {
        let marker_line = export
            .doc
            .lines()
            .find(|line| line.contains(EXEMPT_MARKER))
            .unwrap_or_else(|| {
                panic!(
                    "{}: {} 既没有 boundary/ boundary_ptr，也没有 `{EXEMPT_MARKER}` 白名单理由",
                    export.file, export.name
                )
            });
        let reason = marker_line
            .split_once('：')
            .map(|(_, reason)| reason.trim())
            .unwrap_or("");
        assert!(
            reason.chars().count() >= 8,
            "{}: {} 的白名单理由缺失或过短（`{EXEMPT_MARKER}：<理由>`）：{marker_line:?}",
            export.file,
            export.name
        );
        exempt.push(export.name.clone());
    }
    exempt.sort();

    assert_eq!(
        exempt, EXEMPT_EXPECTED,
        "无边界入口的实际清单与显式白名单不一致：\n  实际 {exempt:?}\n  期望 {EXEMPT_EXPECTED:?}\n\
         （新增入口请包 `boundary`／`boundary_ptr`；确属无 panic 面才写源码理由并同步本名单）"
    );
}
