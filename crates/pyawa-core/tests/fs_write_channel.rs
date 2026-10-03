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

// --------------------------------------------------------------------------- #
// 读的一半（第 97 轮）：`open` / `read` / `close` 也走同一条通道 ✓
// --------------------------------------------------------------------------- #

extern "C" fn open_slot(
    _state: *mut c_void,
    path: *const u8,
    len: usize,
    _flags: i32,
    _mode: u32,
    handle_out: *mut Handle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 契约同 `SPEC-capabilities.md` §9.1。
    let name = unsafe { core::slice::from_raw_parts(path, len) };
    if name != b"/hello.txt" {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = 2 };
        return CapStatus::Machine;
    }
    if !handle_out.is_null() {
        // SAFETY: 同上。
        unsafe { *handle_out = Handle(41) };
    }
    CapStatus::Ok
}

extern "C" fn read_slot(
    _state: *mut c_void,
    handle: Handle,
    buffer: *mut u8,
    len: usize,
    _capacity: usize,
    out_len: *mut usize,
    _errno_out: *mut i32,
) -> CapStatus {
    if handle.0 != 41 {
        return CapStatus::Machine;
    }
    let payload = b"hello world";
    let count = len.min(payload.len());
    // SAFETY: 缓冲区由调用方按 `len` 给出。
    unsafe {
        core::ptr::copy_nonoverlapping(payload.as_ptr(), buffer, count);
        *out_len = count;
    }
    CapStatus::Ok
}

extern "C" fn close_slot(
    _state: *mut c_void,
    handle: Handle,
    _errno_out: *mut i32,
) -> CapStatus {
    if handle.0 == 41 {
        CapStatus::Ok
    } else {
        CapStatus::Machine
    }
}

#[test]
fn fs_read_channel_reports_the_three_outcomes() {
    let instance = Instance::new();
    let mut buffer = [0u8; 32];
    // **`CP-2`**：域未注册 ⇒ 调用时报未实现 ✓
    assert_eq!(
        instance.fs_open(b"/hello.txt", pyawa_capabilities::fs::open_flag::RDONLY, 0),
        Err(CapabilityCallError::NotRegistered)
    );
    assert_eq!(
        instance.fs_read(41, &mut buffer),
        Err(CapabilityCallError::NotRegistered)
    );

    let table = CpFsVtable {
        open: Some(open_slot),
        read: Some(read_slot),
        close: Some(close_slot),
        ..CpFsVtable::UNIMPLEMENTED
    };
    assert!(instance.set_capability(
        pyawa_capabilities::DOMAIN_FS,
        (&table as *const CpFsVtable).cast::<c_void>(),
        Some(pyawa_capabilities::fs::ASYNC_CLASSIFICATION)
    ));

    let handle = instance
        .fs_open(b"/hello.txt", pyawa_capabilities::fs::open_flag::RDONLY, 0)
        .expect("打开成功 ✓");
    assert_eq!(handle, 41);
    // 机器错误带 `errno`（`CP-3` ✓）
    assert_eq!(
        instance.fs_open(b"/nope", pyawa_capabilities::fs::open_flag::RDONLY, 0),
        Err(CapabilityCallError::Machine(2))
    );
    let read = instance.fs_read(handle, &mut buffer).expect("读成功 ✓");
    assert_eq!(read, 11);
    assert_eq!(&buffer[..read], b"hello world");
    assert_eq!(instance.fs_close(handle), Ok(()));
}
