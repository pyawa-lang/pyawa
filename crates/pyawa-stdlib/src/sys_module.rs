//! `sys` 模块（**不依赖能力域**的那部分；契约 `docs/SPEC-c-modules.md` §5.2.3）。
//!
//! 要点：
//!
//! - **`CX-13`**：`implementation.name` **必须**报 `pyawa`——谎报 `cpython` 会让库去加载
//!   **不存在**的 C 扩展，而库自带的纯 Python 回退路径才是"生态可用"能成立的原因
//! - **语言版本 vs 实现版本**（`DESIGN.md` §9 的"实现观测面"）：`version_info`／`hexversion`
//!   报**语言级别**（对拍参照 3.14.4 ⇒ `(3, 14, 4, 'final', 0)`），供库做特性检测；
//!   身份由 `implementation` 承载（`name`／`cache_tag`／`version`）
//! - 本段只做**不依赖能力域、输出通道与 import 机制**的属性；`stdout` 一族（`_io` ⇒ `fs` 域）、
//!   路径一族（`prefix`／`executable`）、importlib 一族（`meta_path`）与绑定未定实现参数的
//!   `*_info`（哈希／整数表示）都不在其中
//!
//! 本模块**不碰**平台（`CX-4`：stdlib 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）：
//! 字节序用 `cfg!(target_endian)`，不调任何平台接口。

use core::ptr::NonNull;

use pyawa_core::{AttributeObject, Header, Instance};

/// 模块名（`sys`）。
pub const NAME: &str = "sys";

/// 模块的 `__doc__`（与参照实现同源的一句话）。
pub const DOC: &str = "This module provides access to some objects used or maintained by the\ninterpreter and to functions that interact strongly with the interpreter.";

/// **本实现所实现的语言级别**：`(主, 次, 微, 发布级, 序号)`。
///
/// 对拍参照是 3.14.4（`REQUIREMENTS.md`）⇒ `version_info`／`hexversion` 报它，
/// **不是**报本实现自己的版本号——库用 `sys.version_info >= (3, 11)` 一类做特性检测。
pub const LANGUAGE_VERSION: (u32, u32, u32, &str, u32) = (3, 14, 4, "final", 0);

/// **本实现自己的**版本：工作区版本（`Cargo.toml`），现为 `0.0.0`（未发布）。
pub const IMPLEMENTATION_VERSION: (u32, u32, u32, &str, u32) = (0, 0, 0, "alpha", 0);

/// **`CX-13`**：`implementation.name`——**必须**报这个名字。
pub const IMPLEMENTATION_NAME: &str = "pyawa";

/// `implementation.cache_tag`：**自己的值**（`CX-13`）＝ `pyawa-<指令集版本>`（`BC-29`）。
pub fn cache_tag() -> String {
    format!(
        "{IMPLEMENTATION_NAME}-{}",
        pyawa_core::opcode_metadata::INSTRUCTION_SET_VERSION
    )
}

/// `sys.version`：**构建串**，含 `pyawa` 与本实现自己的版本（**禁止**伪装成 CPython 的构建串）。
pub fn version_string() -> String {
    let (major, minor, micro, _, _) = LANGUAGE_VERSION;
    let (impl_major, impl_minor, impl_micro, _, _) = IMPLEMENTATION_VERSION;
    format!("{major}.{minor}.{micro} ({IMPLEMENTATION_NAME} {impl_major}.{impl_minor}.{impl_micro})")
}

/// 发布级 → `hexversion` 里的那一段（照参照实现的编码：`alpha` `0xA`、`beta` `0xB`、
/// `candidate` `0xC`、`final` `0xF`）。
fn release_level_code(release_level: &str) -> Option<i64> {
    match release_level {
        "alpha" => Some(0xA),
        "beta" => Some(0xB),
        "candidate" => Some(0xC),
        "final" => Some(0xF),
        _ => None,
    }
}

/// `hexversion`：`主 << 24 ｜ 次 << 16 ｜ 微 << 8 ｜ 发布级 << 4 ｜ 序号`（与参照同式）。
pub fn hexversion(version: (u32, u32, u32, &str, u32)) -> Option<i64> {
    let (major, minor, micro, release_level, serial) = version;
    let level = release_level_code(release_level)?;
    Some(
        ((major as i64) << 24)
            | ((minor as i64) << 16)
            | ((micro as i64) << 8)
            | (level << 4)
            | (serial as i64),
    )
}

/// 本机字节序（`byteorder` 的值）：与参照同源——都取宿主端序，不硬编码 `'little'`。
pub const fn byteorder() -> &'static str {
    if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    }
}

/// 建 `sys` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();

    // `argv`：启动参数列表。REPL／嵌入式默认 `[""]`（`-c` 入口由 CLI 改写，§5.2.3）
    let empty = instance.new_str("");
    let argv = instance.new_list(vec![empty]);
    instance.dict_set(namespace, "argv", argv);
    // `path`：由 `site.py` 构建（`IM-24`）——本层只暴露这个列表，不另立路径逻辑
    let path = instance.new_list(Vec::new());
    instance.dict_set(namespace, "path", path);
    // `modules`：import 系统的模块表；import 未接之前只保证这个键存在
    let modules = instance.new_dict();
    instance.dict_set(namespace, "modules", modules);

    // 语言版本（供特性检测）
    let (major, minor, micro, release_level, serial) = LANGUAGE_VERSION;
    let version_info = instance.new_tuple(vec![
        instance.new_int(i64::from(major)),
        instance.new_int(i64::from(minor)),
        instance.new_int(i64::from(micro)),
        instance.new_str(release_level),
        instance.new_int(i64::from(serial)),
    ]);
    instance.dict_set(namespace, "version_info", version_info);
    if let Some(hexversion) = hexversion(LANGUAGE_VERSION) {
        let encoded = instance.new_int(hexversion);
        instance.dict_set(namespace, "hexversion", encoded);
    }

    // 构建串：必须含 `pyawa`
    let version = instance.new_str(&version_string());
    instance.dict_set(namespace, "version", version);

    // 与实现无关的常量
    let maxunicode = instance.new_int(0x10FFFF);
    instance.dict_set(namespace, "maxunicode", maxunicode);
    let maxsize = instance.new_int(isize::MAX as i64);
    instance.dict_set(namespace, "maxsize", maxsize);
    let byteorder = instance.new_str(byteorder());
    instance.dict_set(namespace, "byteorder", byteorder);

    // `implementation`：点号可访问的命名空间（`CX-13` 的载体）。
    // 用核心**安全**的公开面搭：`new_attribute_type` ＋ `set_type_attribute`（不碰 unsafe）。
    let namespace_type = instance.new_attribute_type("sys.implementation");
    let name = instance.new_str(IMPLEMENTATION_NAME);
    instance.set_type_attribute(namespace_type, "name", name);
    let tag = instance.new_str(&cache_tag());
    instance.set_type_attribute(namespace_type, "cache_tag", tag);
    let (impl_major, impl_minor, impl_micro, impl_level, impl_serial) = IMPLEMENTATION_VERSION;
    let impl_version = instance.new_tuple(vec![
        instance.new_int(i64::from(impl_major)),
        instance.new_int(i64::from(impl_minor)),
        instance.new_int(i64::from(impl_micro)),
        instance.new_str(impl_level),
        instance.new_int(i64::from(impl_serial)),
    ]);
    instance.set_type_attribute(namespace_type, "version", impl_version);
    let implementation =
        instance.alloc(AttributeObject::new(namespace_type, core::cell::RefCell::new(None)));
    instance.dict_set(
        namespace,
        "implementation",
        implementation.into_raw().cast::<Header>(),
    );

    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
