//! **`.pyac`**：Pyawa 的**自有**产物格式（`IM-18`…`IM-21`）。
//!
//! 与 CPython 的 `.pyc` **无关且不兼容**（`IM-18`）。本模块只负责**容器**：产物路径规则、
//! 头部编解码、两步陈旧判定；代码段那一半是**编译产物的确定性序列化**
//! （[`encode_unit`]／[`decode_unit`]，给 `P1-10` 的编译器用，`IM-28`／`IM-31`）。
//!
//! # 具体的字节编码（规格只固定**字段顺序**，`IM-19`；以下取值为实现自选，记在这里）
//!
//! | 偏移 | 宽度 | 字段 |
//! |---|---|---|
//! | 0 | 8 | `magic` ＝ `b"PYAWAC\0\0"` |
//! | 8 | 4 | 指令集版本（`BC-29`／`BC-40`，小端 `u32`） |
//! | 12 | 1 | 模式（`IM-1`：`0` ＝ 纯 Python，`1` ＝ 扩展） |
//! | 13 | 1 | 优化级 |
//! | 14 | 1 | **检查档位**（`TS-31`；`0` ＝ 浅层默认、`1` ＝ 深层，编码自选） |
//! | 15 | 8 | 源码长度（小端 `u64`） |
//! | 23 | 8 | 源码指纹（小端 `u64`，FNV-1a） |
//! | 31 | 4 | 代码段偏移（小端 `u32`） |
//! | 35 | 4 | 代码段长度（小端 `u32`） |
//!
//! 指纹用 64 位非密码学哈希：**参照实现自己也是 64 位**（`.pyc` 头部那个源码哈希），
//! 它的职责只是"**陈旧判定**"（`IM-20` ②），不是防篡改。
//!
//! # 纯函数性（`IM-21`）
//!
//! 产物**只**由「源码 ＋ 模式 ＋ 优化级 ＋ **检查档位**（`TS-31`）＋ 指令集版本」决定
//! （`IM-21` 的五要素）：本模块的 [`encode`] 只吃这五样，**不接受**路径／时间；
//! `IM-21` 里禁掉的那些东西因此**结构上就进不来**。档位不同即陈旧（`IM-20` ②）。

use std::path::{Path, PathBuf};

use pyawa_core::compile::{CompiledUnit, Constant};

/// `.pyac` 的 `magic`（8 字节；`IM-18` 的自有格式标记）。
pub const MAGIC: [u8; 8] = *b"PYAWAC\0\0";

/// 头部长度（字段见模块文档）。
pub const HEADER_LEN: usize = 39;

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
    /// **检查档位**（`TS-31`：`0` ＝ 浅层、`1` ＝ 深层）。
    pub tier: u8,
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

/// 编码一个产物（**纯函数**：同样的五样输入给同样的字节，`IM-21`）。
pub fn encode(
    mode: u8,
    optimization: u8,
    tier: u8,
    source: &[u8],
    code: &[u8],
    version: u32,
) -> Vec<u8> {
    let source_length = source.len() as u64;
    let source_fingerprint = fingerprint(source);
    let mut out = Vec::with_capacity(HEADER_LEN + code.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&version.to_le_bytes());
    out.push(mode);
    out.push(optimization);
    // `IM-19`：检查档位紧跟在优化级之后
    out.push(tier);
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
    let tier = bytes[14];
    let source_length = u64::from_le_bytes(bytes[15..23].try_into().expect("长度已查"));
    let source_fingerprint = u64::from_le_bytes(bytes[23..31].try_into().expect("长度已查"));
    let code_offset = u32::from_le_bytes(bytes[31..35].try_into().expect("长度已查")) as usize;
    let code_length = u32::from_le_bytes(bytes[35..39].try_into().expect("长度已查")) as usize;
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
        tier,
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
    std::fs::write(&path, bytes)?;
    Ok(path)
}

// ---- 代码段 = 编译产物的**确定性**序列化（`IM-31` 的 Rust 层这一半；`IM-21` 的纯函数性） ----

/// 把 [`CompiledUnit`]（`P1-10` 的产物）序列化成代码段字节。
///
/// **确定性**（`IM-21`）：同一份产物永远给同一串字节——不用哈希表、不写路径／时间，
/// 所有长度都显式写成小端 `u32`／`u64`，字段顺序固定。规格只固定 `.pyac` **头部**的字段顺序
/// （`IM-19`），代码段自己的布局是实现自选，所以口径记在这里：
///
/// | 段 | 编码 |
/// |---|---|
/// | `name`／`qualname` | 各 `u32` 长度 ＋ UTF-8（`BC-4` 的 `co_qualname`） |
/// | `argcount`／`posonlyargcount`／`kwonlyargcount`／`nlocals`／`flags` | 各 `u32` |
/// | `names`／`varnames`／`cellvars`／`freevars` | `u32` 条数 ＋ 每项（`u32` 长度 ＋ UTF-8） |
/// | `constants` | `u32` 条数 ＋ 每项：`u8` 标签（`0` None／`1` Int／`2` Str／`3` Code／`4` Names）＋ 载荷 |
/// | `code` | `u32` 长度 ＋ 字节 |
/// | `positions` | `u32` 条数 ＋ 每条 4 个 `u32`（起始行／结束行／起始列／结束列） |
///
/// `Code` 递归（嵌套 code object 就是这么来的），`Names` 是 `CALL_KW` 的名元组。
pub fn encode_unit(unit: &CompiledUnit) -> Vec<u8> {
    let mut out = Vec::new();
    write_text(&mut out, &unit.name);
    // `BC-4` 的 `co_qualname`（与 `name` 一样是长度前缀文本）
    write_text(&mut out, &unit.qualname);
    for number in [
        unit.argcount,
        unit.posonlyargcount,
        unit.kwonlyargcount,
        unit.nlocals,
    ] {
        out.extend_from_slice(&(number as u32).to_le_bytes());
    }
    out.extend_from_slice(&unit.flags.to_le_bytes());
    // `BC-45` 的 `co_cellvars`／`co_freevars`（与 `names`／`varnames` 同一种文本表编码）
    for table in [&unit.names, &unit.varnames, &unit.cellvars, &unit.freevars] {
        out.extend_from_slice(&(table.len() as u32).to_le_bytes());
        for text in table {
            write_text(&mut out, text);
        }
    }
    out.extend_from_slice(&(unit.constants.len() as u32).to_le_bytes());
    for constant in &unit.constants {
        out.extend_from_slice(&encode_constant(constant));
    }
    out.extend_from_slice(&(unit.code.len() as u32).to_le_bytes());
    out.extend_from_slice(&unit.code);
    out.extend_from_slice(&(unit.positions.len() as u32).to_le_bytes());
    for (line_start, line_end, col_start, col_end) in &unit.positions {
        for number in [*line_start, *line_end, *col_start, *col_end] {
            out.extend_from_slice(&number.to_le_bytes());
        }
    }
    out
}

/// 单项常量的编码（`Constant::Tuple` 要递归；`encode_unit` 与它互递归）。
fn encode_constant(constant: &Constant) -> Vec<u8> {
    let mut out = Vec::new();
    match constant {
        Constant::None => out.push(0),
        Constant::Int(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        Constant::Str(text) => {
            out.push(2);
            write_text(&mut out, text);
        }
        Constant::Code(inner) => {
            out.push(3);
            out.extend_from_slice(&encode_unit(inner));
        }
        Constant::Bool(value) => {
            out.push(7);
            out.push(u8::from(*value));
        }
        Constant::Names(names) => {
            out.push(4);
            out.extend_from_slice(&(names.len() as u32).to_le_bytes());
            for name in names {
                write_text(&mut out, name);
            }
        }
        Constant::Type(name) => {
            // `TS-31` 的边界标签：类型按**名字**引用（编译器不认识运行期类型对象）
            out.push(5);
            write_text(&mut out, name);
        }
        Constant::Bytes(value) => {
            // `P1-12`：`bytes` 字面量（长度 ＋ 原始字节）
            out.push(8);
            out.extend_from_slice(&(value.len() as u32).to_le_bytes());
            out.extend_from_slice(value);
        }
        Constant::Tuple(parts) => {
            out.push(6);
            out.extend_from_slice(&(parts.len() as u32).to_le_bytes());
            for part in parts {
                out.extend_from_slice(&encode_constant(part));
            }
        }
    }
    out
}

/// [`encode_unit`] 的逆；字节不合规时给 [`PyacError::BadCodeSection`]（**不 panic**）。
pub fn decode_unit(bytes: &[u8]) -> Result<CompiledUnit, PyacError> {
    let mut reader = UnitReader { bytes, at: 0 };
    let unit = reader.unit()?;
    if reader.at != bytes.len() {
        return Err(PyacError::BadCodeSection);
    }
    Ok(unit)
}

fn write_text(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}

struct UnitReader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl UnitReader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], PyacError> {
        let end = self.at.checked_add(count).ok_or(PyacError::BadCodeSection)?;
        if end > self.bytes.len() {
            return Err(PyacError::BadCodeSection);
        }
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, PyacError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, PyacError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn i64(&mut self) -> Result<i64, PyacError> {
        let bytes = self.take(8)?;
        Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn usize(&mut self) -> Result<usize, PyacError> {
        Ok(self.u32()? as usize)
    }

    fn text(&mut self) -> Result<String, PyacError> {
        let length = self.usize()?;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| PyacError::BadCodeSection)
    }

    /// 原始字节段（`bytes` 常量用）。
    fn bytes(&mut self) -> Result<Vec<u8>, PyacError> {
        let length = self.usize()?;
        Ok(self.take(length)?.to_vec())
    }

    fn unit(&mut self) -> Result<CompiledUnit, PyacError> {
        let name = self.text()?;
        let qualname = self.text()?;
        let argcount = self.usize()?;
        let posonlyargcount = self.usize()?;
        let kwonlyargcount = self.usize()?;
        let nlocals = self.usize()?;
        let flags = self.u32()?;
        let names = self.text_table()?;
        let varnames = self.text_table()?;
        let cellvars = self.text_table()?;
        let freevars = self.text_table()?;
        let count = self.usize()?;
        let mut constants = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            constants.push(self.constant()?);
        }
        let code_length = self.usize()?;
        let code = self.take(code_length)?.to_vec();
        let position_count = self.usize()?;
        let mut positions = Vec::with_capacity(position_count.min(4096));
        for _ in 0..position_count {
            positions.push((self.u32()?, self.u32()?, self.u32()?, self.u32()?));
        }
        Ok(CompiledUnit {
            name,
            qualname,
            argcount,
            posonlyargcount,
            kwonlyargcount,
            nlocals,
            flags,
            names,
            varnames,
            cellvars,
            freevars,
            constants,
            code,
            positions,
        })
    }

    /// 单项常量（`Tag 6` 的标签元组要递归）。
    fn constant(&mut self) -> Result<Constant, PyacError> {
        Ok(match self.u8()? {
            0 => Constant::None,
            1 => Constant::Int(self.i64()?),
            2 => Constant::Str(self.text()?),
            3 => Constant::Code(Box::new(self.unit()?)),
            7 => Constant::Bool(self.u8()? != 0),
            4 => Constant::Names(self.text_table()?),
            5 => Constant::Type(self.text()?),
            8 => Constant::Bytes(self.bytes()?),
            6 => {
                let count = self.usize()?;
                let mut parts = Vec::with_capacity(count.min(1024));
                for _ in 0..count {
                    parts.push(self.constant()?);
                }
                Constant::Tuple(parts)
            }
            _ => return Err(PyacError::BadCodeSection),
        })
    }

    fn text_table(&mut self) -> Result<Vec<String>, PyacError> {
        let count = self.usize()?;
        let mut table = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            table.push(self.text()?);
        }
        Ok(table)
    }
}
