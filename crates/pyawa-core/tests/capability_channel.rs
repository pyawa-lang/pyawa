//! **能力通道**（第 90 轮）：VM 侧把宿主注册的 `fs` 域 vtable 取回来并**按它调用** ✓。
//!
//! 这是 `print`（`sys.stdout → _io → fs`）、`site.py`（`IM-24`）与 import 的 I/O（`IM-15`）
//! 三处**共用**的那条通道 ✓；本轮只验**通道本身**（`print` 尚未接 ✗，如实 ✓）。

use core::ffi::c_void;
use pyawa_capabilities::fs::{CapStatus, CpFsVtable, Handle};
use pyawa_core::Instance;
use std::sync::Mutex;

/// 落点：测试线程之外的宿主状态（`CP-12`：状态经 vtable 的 `state` 指针走，这里用静态 ✓）。
static SINK: Mutex<Vec<u8>> = Mutex::new(Vec::new());

extern "C" fn write_slot(
    _state: *mut c_void,
    _handle: Handle,
    bytes: *const u8,
    len: usize,
    out_len: *mut usize,
    _errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 宿主侧约定 `bytes` 可读 `len` 字节（`SPEC-capabilities.md` §9.1）。
    let slice = unsafe { core::slice::from_raw_parts(bytes, len) };
    SINK.lock().expect("sink 未中毒").extend_from_slice(slice);
    if !out_len.is_null() {
        // SAFETY: 出参由调用方提供（同上）。
        unsafe { *out_len = len };
    }
    CapStatus::Ok
}

fn vtable() -> CpFsVtable {
    CpFsVtable {
        write: Some(write_slot),
        ..CpFsVtable::UNIMPLEMENTED
    }
}

#[test]
fn capability_channel_round_trips_the_fs_vtable() {
    SINK.lock().expect("sink 未中毒").clear();
    let instance = Instance::new();
    let table = vtable();
    let pointer = (&table as *const CpFsVtable).cast::<c_void>();

    // **`CP-25`**：没有异步分类 ⇒ 注册**必须失败** ✓（`AB-34`）
    assert!(
        !instance.set_capability(pyawa_capabilities::DOMAIN_FS, pointer, None),
        "缺分类的注册必须失败 ✓"
    );
    // **`CP-2`**：未注册 ⇒ 取不到 vtable（调用时报"未实现"，而不是创建时就拒绝 ✓）
    assert!(instance.fs_vtable().is_none());

    // 带上分类 ⇒ 注册成功 ✓
    assert!(instance.set_capability(
        pyawa_capabilities::DOMAIN_FS,
        pointer,
        Some(pyawa_capabilities::fs::ASYNC_CLASSIFICATION)
    ));
    let table_back = instance.fs_vtable().expect("注册后应能取回 vtable ✓");
    let write = table_back.write.expect("`write` 槽已接 ✓");

    let payload = b"pyawa-capability-channel\n";
    let mut written = 0usize;
    let mut errno = 0i32;
    let status = write(
        core::ptr::null_mut(),
        Handle(1),
        payload.as_ptr(),
        payload.len(),
        &mut written,
        &mut errno,
    );
    assert!(matches!(status, CapStatus::Ok), "通道调用应成功 ✓");
    assert_eq!(written, payload.len());
    assert_eq!(&*SINK.lock().expect("sink 未中毒"), payload);

    // 另一个域不受影响（`AB-33`：按域隔离 ✓）
    assert!(instance.capability(pyawa_capabilities::DOMAIN_FS).is_some());
    assert!(instance.capability(pyawa_capabilities::DOMAIN_FS + 1).is_none());
}
