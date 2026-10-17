//! **`.pyac` 的容器侧**（`IM-18`…`IM-21`）：产物**路径规则**、**文件读写**、**两步陈旧判定** ✓。
//!
//! **归层**（第 404 轮 ✓）：纯编解码（头部字节、代码段序列化）已搬到 `pyawa-core::pyac` ✓ ——
//! 它是无 I/O 的纯函数，而 `pa_exec_bytecode`（ABI 只依赖 `pyawa-core` ✓）必须能解码产物 ✓。
//! 本模块把那些名字**原样再导出** ✓（`pub use`，一处真相 ✓），调用方路径不变 ✓。
use std::path::{Path, PathBuf};

pub use pyawa_core::pyac::{
    decode, decode_unit, encode, encode_unit, fingerprint, Product, PyacError, HEADER_LEN, MAGIC,
    MODE_EXTENDED, MODE_PURE,
};

/// 产物目录名（`IM-18`）。
pub const ARTIFACT_DIRECTORY: &str = "__pyawa__";

/// **产物路径**（`IM-18`）：`<pkgdir>/__pyawa__/<源文件名>.<指令集版本>.pyac`。
///
/// 模式体现在**源文件名**里（`foo.py` 与 `foo.pyawa` 名字不同）⇒ 改名或换模式必然换路径。
pub fn artifact_path(package_directory: &Path, source_file_name: &str, version: u32) -> PathBuf {
    package_directory
        .join(ARTIFACT_DIRECTORY)
        .join(format!("{source_file_name}.{version}.pyac"))
}

/// 陈旧判定的结果（`IM-20` 的第二步；第一步是**按名字精确查找**，见 [`find_artifact`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Staleness {
    /// ② 名字匹配、头部指纹与当前源码一致 ⇒ 可以用。
    Fresh,
    /// ② 名字匹配但指纹／长度／模式／优化级对不上 ⇒ 陈旧，要重编。
    Stale,
    /// 名字匹配但产物读不出来／坏了 ⇒ 当作陈旧。
    Unreadable(String),
}

/// **`IM-20` ①**：按名字**精确**找产物——名字里带着模式体现的源文件名与**指令集版本**，
/// 所以别的版本／别的模式留下的产物**根本不会被考虑**（连读都不读）。
pub fn find_artifact(
    package_directory: &Path,
    source_file_name: &str,
    version: u32,
) -> Option<PathBuf> {
    let path = artifact_path(package_directory, source_file_name, version);
    path.is_file().then_some(path)
}

/// **`IM-20` ②**：名字对得上时，读头部比对指纹（长度相同而哈希不同 ⇒ 陈旧）**与检查档位**
/// （档位不同 ⇒ 陈旧，`TS-31`）。
///
/// 调用方先用 [`find_artifact`] 走完第一步；本函数只管第二步。
/// **禁止**只看 mtime（`IM-20`），故这里一个 `metadata()` 都不碰。
pub fn staleness(
    path: &Path,
    mode: u8,
    optimization: u8,
    tier: u8,
    source: &[u8],
    version: u32,
) -> Staleness {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return Staleness::Unreadable(error.to_string()),
    };
    match decode(&bytes, version) {
        Ok(product) => {
            let same_length = product.source_length == source.len() as u64;
            let same_hash = product.source_fingerprint == fingerprint(source);
            if same_length
                && same_hash
                && product.mode == mode
                && product.optimization == optimization
                // `IM-20` ②：**档位不同即陈旧**（`TS-31`）
                && product.tier == tier
            {
                Staleness::Fresh
            } else {
                Staleness::Stale
            }
        }
        // 版本不符在第一步就该被挡掉；真走到这里也按"不能加载"处理
        Err(PyacError::VersionMismatch { .. }) => Staleness::Stale,
        Err(other) => Staleness::Unreadable(format!("{other:?}")),
    }
}

/// 产物文件名（不取目录）：`<源文件名>.<指令集版本>.pyac`。
pub fn file_name(source_file_name: &str, version: u32) -> String {
    format!("{source_file_name}.{version}.pyac")
}

/// 写产物（**只在需要写的地方调用**；`IM-16` 的只读根下不该走到这里）。
pub fn write(
    package_directory: &Path,
    source_file_name: &str,
    version: u32,
    mode: u8,
    optimization: u8,
    tier: u8,
    source: &[u8],
    code: &[u8],
) -> std::io::Result<PathBuf> {
    let directory = package_directory.join(ARTIFACT_DIRECTORY);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(file_name(source_file_name, version));
    let bytes = encode(mode, optimization, tier, source, code, version);
    // **原子落盘**（第 406 轮，并发自压当场抓到的真 bug ✗）：先前直写目标路径 ✗ ⇒ 4 路并发跑**同一个**
    // 脚本时，另一个进程可能读到**写了一半**的产物 ✗（实测 `MS-25` 并发自压 **2/4** ✗）。
    // 参照的 `.pyc` 也是"写临时文件 ＋ 原子改名" ✓；临时名带 pid ⇒ 并发写互不覆盖 ✓。
    let temporary = directory.join(format!(
        ".{}.{}.tmp",
        file_name(source_file_name, version),
        std::process::id()
    ));
    std::fs::write(&temporary, bytes)?;
    if let Err(error) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(path)
}

// ---- 代码段 = 编译产物的**确定性**序列化（`IM-31` 的 Rust 层这一半；`IM-21` 的纯函数性） ----
