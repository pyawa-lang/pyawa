//! `_io` 模块（第 194 轮）：**最小面** = 名字齐 ✓、真 I/O 尚未接线 ✗。
//!
//! `Lib/importlib/_bootstrap_external.py`（`import _io` ✓）与 `Lib/site.py`（`import _io as io` ✓）都要**先导入成功** ✓；
//! 文件读写按 `CM-8` **必须**走能力域（`fs` ✓）⇒ 这里只放名字 ✓、调用时**如实报未接线** ✗。
//!
//! `CX-4`：本 crate 不碰平台 ⇒ 这里不 import 任何平台接口 ✓。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};

/// 模块名（`_io`）。
pub const NAME: &str = "_io";

const NAMES: [&str; 5] = [
    "open",
    "open_code",
    "FileIO",
    "BytesIO",
    "IncrementalNewlineDecoder",
];

/// 建 `_io` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    crate::build_stub_module(instance, NAME, &NAMES, "真 I/O 走 fs 能力域")
}
