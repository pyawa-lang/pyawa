//! **S3（`ADR-005`）**：`event` 能力域注册（`PA_DOMAIN_EVENT`）＋域计数（`PA_ABI_MINOR` 16）。
//!
//! 只验 ABI 注册面与三态错误；不启动宿主循环（S3 的参考宿主在 C 侧）。

use core::ffi::c_void;
use core::mem::size_of;

use pyawa_abi::capability::{PA_ASYNC_OK, DOMAIN_COUNT, PA_DOMAIN_EVENT};
use pyawa_abi::status::*;
use pyawa_abi::*;
use pyawa_capabilities::event::CpEventVtable;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

#[test]
fn event_domain_is_registrable_and_optional() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);

    // 未注册 ⇒ 仍然可创建实例（CP-40）。
    assert_eq!(unsafe { pa_setcapability_async(state, PA_DOMAIN_EVENT, PA_ASYNC_OK) }, PA_OK);
    let mut vtable = CpEventVtable::UNIMPLEMENTED;
    vtable.version = PA_ABI_VERSION;
    vtable.size = size_of::<CpEventVtable>() as u32;
    assert_eq!(
        unsafe { pa_setcapability(state, PA_DOMAIN_EVENT, (&mut vtable as *mut CpEventVtable).cast::<c_void>()) },
        PA_OK
    );
    // 超出域计数 ⇒ 宿主用法错误（AB-33）。
    assert_eq!(unsafe { pa_setcapability_async(state, DOMAIN_COUNT as i32, PA_ASYNC_OK) }, PA_ERR_INVALID);

    unsafe { pa_destroy(state) };
}
