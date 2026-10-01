//! **`.pyac`**：Pyawa 的**自有**产物格式（`IM-18`…`IM-21`）。
//!
//! 与 CPython 的 `.pyc` **无关且不兼容**（`IM-18`）。本模块只负责**容器**：产物路径规则、
//! 头部编解码、两步陈旧判定。代码段在这层是**不透明字节**——谁来填（Pyawa 自己的编译器，
//! `IM-28`）不归这里管。
//!
//! # 具体的字节编码（规格只固定**字段顺序**，`IM-19`；以下取值为实现自选，记在这里）
//!
//! | 偏移 | 宽度 | 字段 |
//! |---|---|---|
//! | 0 | 8 | `magic` ＝ `b"PYAWAC\0\0"` |
//! | 8 | 4 | 指令集版本（`BC-29`／`BC-40`，小端 `u32`） |
//! | 12 | 1 | 模式（`IM-1`：`0` ＝ 纯 Python，`1` ＝ 扩展） |
//! | 13 | 1 | 优化级 |
//! | 14 | 8 | 源码长度（小端 `u64`） |
//! | 22 | 8 | 源码指纹（小端 `u64`，FNV-1a） |
//! | 30 | 4 | 代码段偏移（小端 `u32`） |
//! | 34 | 4 | 代码段长度（小端 `u32`） |
//!
//! 指纹用 64 位非密码学哈希：**参照实现自己也是 64 位**（`.pyc` 头部那个源码哈希），
//! 它的职责只是"**陈旧判定**"（`IM-20` ②），不是防篡改。
//!
//! # 纯函数性（`IM-21`）
//!
//! 产物**只**由「源码 ＋ 模式 ＋ 优化级 ＋ 指令集版本」决定：本模块的 [`encode`] 只吃这四样，
//! **不接受**路径／时间；`IM-21` 里禁掉的那些东西因此**结构上就进不来**。

use std::path::{Path, PathBuf};

/// `.pyac` 的 `magic`（8 字节；`IM-18` 的自有格式标记）。
pub const MAGIC: [u8; 8] = *b"PYAWAC\0\0";

/// 头部长度（字段见模块文档）。
pub const HEADER_LEN: usize = 38;

/// 纯 Python 模式（`IM-1`）。
pub const MODE_PURE: u8 = 0;

/// 扩展模式（`IM-1`）。
pub const MODE_EXTENDED: u8 = 1;

/// 产物目录名（`IM-18`）。
pub const ARTIFACT_DIRECTORY: &str = "__pyawa__";

/// 一次 `.pyac` 解析出来的东西。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// 头部携带的指令集版本（`BC-29`）。
    pub instruction_set_version: u32,
    /// 模式（`IM-1`）。
    pub mode: u8,
    /// 优化级。
    pub optimization: u8,
    /// 编码时的源码长度。
    pub source_length: u64,
    /// 编码时的源码指纹。
    pub source_fingerprint: u64,
    /// 代码段（不透明字节）。
    pub code: Vec<u8>,
}

/// 解析／陈旧判定可能出的错。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyacError {
    /// 文件太短，连头部都不够。
    Truncated,
    /// `magic` 不对（不是 `.pyac`）。
    BadMagic,
    /// 指令集版本与运行时不一致（`BC-29`：判陈旧，**不**加载）。
    VersionMismatch {
        /// 文件里写的版本。
        found: u32,
        /// 运行时认的版本（`BC-40`）。
        expected: u32,
    },
    /// 头部声明的代码段越界。
    BadCodeSection,
    /// 读文件失败（`IO`；只在陈旧判定那一步可能出现）。
    Io(String),
}

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

/// 源码指纹（FNV-1a 64 位；见模块文档）。
pub fn fingerprint(source: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in source {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// 编码一个产物（**纯函数**：同样的四样输入给同样的字节，`IM-21`）。
pub fn encode(mode: u8, optimization: u8, source: &[u8], code: &[u8], version: u32) -> Vec<u8> {
    let source_length = source.len() as u64;
    let source_fingerprint = fingerprint(source);
    let mut out = Vec::with_capacity(HEADER_LEN + code.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&version.to_le_bytes());
    out.push(mode);
    out.push(optimization);
    out.extend_from_slice(&source_length.to_le_bytes());
    out.extend_from_slice(&source_fingerprint.to_le_bytes());
    out.extend_from_slice(&(HEADER_LEN as u32).to_le_bytes());
    out.extend_from_slice(&(code.len() as u32).to_le_bytes());
    out.extend_from_slice(code);
    out
}

/// 解码（校验 `magic` 与**指令集版本**：版本不符按 `BC-29` 判陈旧、**不**加载）。
pub fn decode(bytes: &[u8], expected_version: u32) -> Result<Product, PyacError> {
    if bytes.len() < HEADER_LEN {
        return Err(PyacError::Truncated);
    }
    if bytes[..8] != MAGIC {
        return Err(PyacError::BadMagic);
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().expect("长度已查"));
    if version != expected_version {
        return Err(PyacError::VersionMismatch {
            found: version,
            expected: expected_version,
        });
    }
    let mode = bytes[12];
    let optimization = bytes[13];
    let source_length = u64::from_le_bytes(bytes[14..22].try_into().expect("长度已查"));
    let source_fingerprint = u64::from_le_bytes(bytes[22..30].try_into().expect("长度已查"));
    let code_offset = u32::from_le_bytes(bytes[30..34].try_into().expect("长度已查")) as usize;
    let code_length = u32::from_le_bytes(bytes[34..38].try_into().expect("长度已查")) as usize;
    let end = code_offset
        .checked_add(code_length)
        .ok_or(PyacError::BadCodeSection)?;
    if end > bytes.len() || code_offset < HEADER_LEN {
        return Err(PyacError::BadCodeSection);
    }
    Ok(Product {
        instruction_set_version: version,
        mode,
        optimization,
        source_length,
        source_fingerprint,
        code: bytes[code_offset..end].to_vec(),
    })
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

/// **`IM-20` ②**：名字对得上时，读头部比对指纹（长度相同而哈希不同 ⇒ 陈旧）。
///
/// 调用方先用 [`find_artifact`] 走完第一步；本函数只管第二步。
/// **禁止**只看 mtime（`IM-20`），故这里一个 `metadata()` 都不碰。
pub fn staleness(
    path: &Path,
    mode: u8,
    optimization: u8,
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
    source: &[u8],
    code: &[u8],
) -> std::io::Result<PathBuf> {
    let directory = package_directory.join(ARTIFACT_DIRECTORY);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(file_name(source_file_name, version));
    let bytes = encode(mode, optimization, source, code, version);
    std::fs::write(&path, bytes)?;
    Ok(path)
}
