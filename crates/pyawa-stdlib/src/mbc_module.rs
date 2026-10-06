//! **`_multibytecodec`**（第 322 轮 ✓）：多字节编解码族所需的最小面 ✓。
//!
//! 依据 ✓：`Lib/encodings/cp949.py`（已同步 ✓）里 `import _multibytecodec as mbc` 之后只用到
//! **5 个类名**作基类 ✓ —— `MultibyteCodec`／`MultibyteIncrementalEncoder`／`MultibyteIncrementalDecoder`／
//! `MultibyteStreamReader`／`MultibyteStreamWriter` ✓。
//! **如实标注** ✗：本模块目前只提供这 5 个**可被 Python 继承**的类 ✓；编解码**语义**由 `_codecs_kr`
//! 那侧的实现承担（下一轮 ✓）—— 语义未做之前**不计入完成** ✗。
//!
//! 本模块**不碰**平台（`CX-4`）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};

/// 模块名（`_multibytecodec`）。
pub const NAME: &str = "_multibytecodec";

/// 类名（照参照 ✓）。
pub const CLASSES: &[&str] = &[
    "MultibyteCodec",
    "MultibyteIncrementalEncoder",
    "MultibyteIncrementalDecoder",
    "MultibyteStreamReader",
    "MultibyteStreamWriter",
];

/// 建 `_multibytecodec` 的**命名空间 dict** ✓（`lib.rs` 会包成模块 ✓）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    for name in CLASSES {
        if let Some(class) = instance.new_subclass_with_instance_dict(name, "object") {
            instance.dict_set(namespace, name, class);
        }
    }
    namespace
}
