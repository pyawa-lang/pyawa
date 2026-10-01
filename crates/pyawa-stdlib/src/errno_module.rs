//! `errno` 模块（**不依赖能力域**的第一个 stdlib 模块，`PLAN` §9.4 第 4 条）。
//!
//! 契约（`CM-4` 的"从 Python 看到的 API 与语义"）写在 `docs/SPEC-c-modules.md` §5.1。
//! 要点：
//!
//! - **`CM-20`**：常量取自**宿主平台**（由 `pyawa-runtime` 启动时注入到实例，按名字查），
//!   映射**按名字**匹配——本模块里没有任何硬编码的数字
//! - **`CM-19`**：`errno` 名字 → `OSError` 子类的映射由 [`crate::errno_map`] 提供（探测导出）
//! - 别名（`EAGAIN`／`EWOULDBLOCK` 这类）由注入表天然带上（值相同、各自一个名字）
//!
//! 本模块**不碰**平台（`CX-4`：stdlib 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};

use crate::errno_map;

/// 模块名（`errno`）。
pub const NAME: &str = "errno";

/// 模块的 `__doc__`（与参照实现同源的一句话）。
pub const DOC: &str = "This module makes available standard errno system symbols.";

/// 建 `errno` 模块的命名空间（**新引用** 的 `dict`）。
///
/// 内容：宿主平台的 `E*` 常量（整数）＋ `errorcode`（值 → 规范名）＋ `__name__`／`__doc__`。
/// 注入表为空时，模块**仍然存在**但没有任何常量——那是宿主没注入，不是"模块未提供"
/// （`CM-6`：模块未提供才抛 `ImportError`）。
pub fn build(instance: &Instance, constants: &[(&'static str, i64)]) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, value) in constants {
        let constant = instance.new_int(*value);
        instance.dict_set(namespace, name, constant);
    }
    // `errorcode`：值 → 规范名（别名只留探测给出的那一个）
    let errorcode = instance.new_dict();
    for (value, name) in errno_map::ERRNO_ERRORCODE {
        let text = instance.new_str(name);
        instance.dict_set_int(errorcode, *value, text);
    }
    instance.dict_set(namespace, "errorcode", errorcode);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}

/// **`CM-5`／`CM-19`**：errno **名字** → `OSError` 子类名（表外一律 `OSError`）。
pub fn oserror_class_for(name: &str) -> &'static str {
    errno_map::ERRNO_TO_CLASS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, class)| *class)
        .unwrap_or("OSError")
}

/// **`CM-5`／`CM-19`／`CM-20`**：errno **数字** → `OSError` 子类名。
///
/// 数字先经 `errorcode` 翻成**名字**，再按名字查映射——所以映射表里没有任何平台数字。
pub fn oserror_class_for_value(_instance: &Instance, value: i64) -> &'static str {
    let name = errno_map::ERRNO_ERRORCODE
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map(|(_, name)| *name);
    match name {
        Some(name) => oserror_class_for(name),
        None => "OSError",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mapping_is_by_name_only() {
        // CM-20：映射按名字——这里断言的是"没有平台数字被写进映射"这一形状
        assert!(errno_map::ERRNO_TO_CLASS.iter().all(|(name, _)| name.starts_with('E')));
        assert_eq!(oserror_class_for("EACCES"), "PermissionError");
        assert_eq!(oserror_class_for("ENOENT"), "FileNotFoundError");
        assert_eq!(oserror_class_for("EGAIN"), "OSError", "表外一律 OSError");
        // 别名各自一条，指向同一个数字（值相同）
        let again = errno_map::ERRNO_ERRORCODE
            .iter()
            .find(|(_, name)| *name == "EAGAIN")
            .map(|(value, _)| *value);
        assert!(again.is_some(), "EAGAIN 应当在 errorcode 里");
    }
}
