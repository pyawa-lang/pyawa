//! `_warnings` 模块（第 194 轮）：**最小面** = 名字齐 ✓、真实现随后接 ✗。
//!
//! `Lib/importlib/_bootstrap_external.py` 与 `Lib/warnings.py` 一带会先导入它 ✓。
//!
//! `CX-4`：本 crate 不碰平台 ⇒ 这里不 import 任何平台接口 ✓。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};

/// 模块名（`_warnings`）。
pub const NAME: &str = "_warnings";

const NAMES: [&str; 5] = [
    "warn",
    "warn_explicit",
    "filters_mutated",
    "_filters_mutated",
    "get_filters",
];

/// 建 `_warnings` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    crate::build_stub_module(instance, NAME, &NAMES, "警告真实现随后接")
}
