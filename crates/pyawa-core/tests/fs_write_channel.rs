//! **经 `fs` 域写字节**（第 91／92 轮）：`Instance::fs_write` 的三态（`CP-2`／`CP-3`／`CP-5`）✓。
//!
//! 这是 `print ⇒ sys.stdout ⇒ _io ⇒ fs` 那条链的**最后一跳** ✓；平台在提供者那边 ✓（`CX-4`）。

use core::ffi::c_void;
use pyawa_capabilities::fs::{CapStatus, CpFsVtable, Handle};
use pyawa_core::{CapabilityCallError, Instance};
use std::sync::Mutex;

static SINK: Mutex<Vec<u8>> = Mutex::new(Vec::new());

extern "C" fn write_slot(
    _state: *mut c_void,
    handle: Handle,
    bytes: *const u8,
    len: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    // 只认标准输出句柄（提供者约定 `1` ✓）；别的句柄报机器错误（`CP-3`）
    if handle.0 != 1 {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = 9 };
        return CapStatus::Machine;
    }
    // SAFETY: 宿主侧约定 `bytes` 可读 `len` 字节。
    let slice = unsafe { core::slice::from_raw_parts(bytes, len) };
    SINK.lock().expect("sink 未中毒").extend_from_slice(slice);
    if !out_len.is_null() {
        // SAFETY: 同上。
        unsafe { *out_len = len };
    }
    CapStatus::Ok
}

extern "C" fn unimplemented_slot(
    _state: *mut c_void,
    _handle: Handle,
    _bytes: *const u8,
    _len: usize,
    _out_len: *mut usize,
    _errno_out: *mut i32,
) -> CapStatus {
    CapStatus::Unimplemented
}

#[test]
fn fs_write_reports_the_three_outcomes() {
    let instance = Instance::new();
    // **`CP-2`**：域未注册 ⇒ 不是"写失败"，而是"调用时报未实现" ✓
    assert_eq!(
        instance.fs_write(1, b"x"),
        Err(CapabilityCallError::NotRegistered)
    );

    let table = CpFsVtable {
        write: Some(write_slot),
        ..CpFsVtable::UNIMPLEMENTED
    };
    assert!(instance.set_capability(
        pyawa_capabilities::DOMAIN_FS,
        (&table as *const CpFsVtable).cast::<c_void>(),
        Some(pyawa_capabilities::fs::ASYNC_CLASSIFICATION)
    ));

    assert_eq!(instance.fs_write(1, b"hello "), Ok(6));
    assert_eq!(instance.fs_write(1, b"world"), Ok(5));
    assert_eq!(&*SINK.lock().expect("sink 未中毒"), b"hello world");

    // **`CP-3`**：机器错误带 `errno` 回来 ✓
    assert_eq!(instance.fs_write(7, b"nope"), Err(CapabilityCallError::Machine(9)));

    // **`CP-5`**：槽位未实现 ✓
    let unimplemented = CpFsVtable {
        write: Some(unimplemented_slot),
        ..CpFsVtable::UNIMPLEMENTED
    };
    assert!(instance.set_capability(
        pyawa_capabilities::DOMAIN_FS,
        (&unimplemented as *const CpFsVtable).cast::<c_void>(),
        Some(pyawa_capabilities::fs::ASYNC_CLASSIFICATION)
    ));
    assert_eq!(
        instance.fs_write(1, b"x"),
        Err(CapabilityCallError::NotImplemented)
    );
}
