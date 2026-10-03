//! `_io` 模块（**最小面**）：`sys.stdout`／`sys.stderr` 那类**文本流**的形状与 `write` ✓。
//!
//! 分工（用户钉死的边界）：`print` ⇒ `sys.stdout` ⇒ **本模块** ⇒ `fs` 域的 `write` 槽 ✓。
//! 本模块**不碰平台**（`CX-4`）：字节交给 `Instance::fs_vtable()`（`CP-12` 的形状 ✓），
//! 真正的系统调用在 `pyawa-runtime` 的平台集中点 ✓。

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{AttributeObject, ExecError, Header, Instance};

/// 模块名（`_io`）。
pub const NAME: &str = "_io";

/// `__doc__`（一句话；完整面待 `P3-` 立项）。
pub const DOC: &str = "The io module provides the Python interfaces to stream handling.";

/// **标准输出句柄**（提供者约定：`1` ⇒ `stdout`、`2` ⇒ `stderr`；见 `PosixFs` 的文档 ✓）。
pub const STDOUT_HANDLE: u64 = 1;
/// 标准错误句柄。
pub const STDERR_HANDLE: u64 = 2;

/// 文本流类型名（`_io.TextIOWrapper` 的**最小面**：类型归属在这里 ✓，实现只有 `write` ✓）。
pub const TEXT_WRAPPER: &str = "_io.TextIOWrapper";

/// 把**已经编码好**的字节经 `fs` 域写出去（平台在提供者那边 ✓，本层不碰 ✓ `CX-4`）。
pub fn write_bytes(
    instance: &Instance,
    handle: u64,
    bytes: &[u8],
) -> Result<usize, ExecError> {
    instance
        .fs_write(handle, bytes)
        .map_err(|error| ExecError::Unsupported {
            opcode: 0,
            what: match error {
                pyawa_core::CapabilityCallError::NotRegistered => {
                    "`fs` 域未注册：`sys.stdout` 没有输出落点（`CP-2`）"
                }
                pyawa_core::CapabilityCallError::NotImplemented => {
                    "`fs` 域的 `write` 槽未实现（`CP-5`）"
                }
                pyawa_core::CapabilityCallError::Machine(_) => {
                    "`fs` 域写标准流遇到机器错误（`CP-3`）"
                }
            },
        })
}

/// 造一个文本流对象（`sys.stdout` 的落点）：字典放 `__handle__`，类型上放 `write` ✓。
pub fn make_stream(instance: &Instance, handle: u64) -> NonNull<Header> {
    let stream_type = instance.new_attribute_type(TEXT_WRAPPER);
    let handle_object = instance.new_int(handle as i64);
    let write_object = instance.new_int(0);
    let namespace = instance.new_dict();
    instance.dict_set(namespace, "__handle__", handle_object);
    instance.set_type_attribute(stream_type, "write", write_object);
    let object = instance.alloc(AttributeObject::new(
        stream_type,
        RefCell::new(Some(namespace)),
    ));
    object.into_raw().cast::<Header>()
}
