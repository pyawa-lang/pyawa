//! **`AB-58`**：定长宿主布局（载荷由 VM 分配）的最小往返。

mod common;

use core::ffi::c_void;
use pyawa_core::{free_fixed_layout, HostVisit, Slots};
use common::Vm;

unsafe extern "C" fn host_dealloc(_payload: *mut c_void) {}

unsafe extern "C" fn host_traverse(
    _payload: *mut c_void,
    _context: *mut c_void,
    _visit: HostVisit,
) {
}

#[test]
fn a_zero_payload_host_object_round_trips() {
    // `AB-58`：`payload_size == 0` ⇒ 载荷指针为 `NULL`，对象只有头部那么大
    let vm = Vm::new();
    let instance = &vm.instance;
    let ty = instance.new_type(
        "EmptyHost",
        pyawa_core::HEADER_SIZE_BYTES,
        Slots::new(free_fixed_layout).with_traverse(|_ptr, _visit| {}),
    );
    // SAFETY: ty 由注册表持有。
    unsafe { ty.as_ref() }.mark_external_instance_dict();
    let (object, payload) = instance.alloc_host_object(ty);
    assert!(payload.is_none(), "AB-58：payload_size == 0 ⇒ NULL");
    // SAFETY: object 是本实例的存活对象。
    unsafe {
        assert!(
            (&*object.as_ptr()).instance_dict().is_none(),
            "OM-14：还没写属性时不该有字典（读到脏值说明头的布局/分配对不上）"
        );
        instance.release_object(object.as_ptr());
    }
}

#[test]
fn a_host_layout_object_round_trips() {
    let vm = Vm::new();
    let instance = &vm.instance;
    // 槽位用一个"什么都不报"的 traverse（本用例只验分配与释放这条路）
    let ty = instance.new_type(
        "Widget",
        40 + 8,
        Slots::new(free_fixed_layout)
            .with_traverse(|_ptr, _visit| {})
            // ABI 侧还给宿主类型挂了 Python 级终结器（`__del__`），这里一并验
            .with_finalize(pyawa_core::python_level_finalize),
    );
    // SAFETY: ty 由注册表持有。
    unsafe { ty.as_ref() }.set_host_hooks(host_dealloc, host_traverse);
    // SAFETY: 同上。
    unsafe { ty.as_ref() }.mark_external_instance_dict();

    let (object, payload) = instance.alloc_host_object(ty);
    let payload = payload.expect("载荷 8 字节");
    // SAFETY: 载荷是 VM 分配的 8 字节。
    unsafe { payload.cast::<u64>().write(0x4242) };
    // SAFETY: 同上。
    assert_eq!(unsafe { payload.cast::<u64>().read() }, 0x4242);

    // SAFETY: object 是本实例的存活对象，本测试持有它那份引用。
    unsafe { instance.release_object(object.as_ptr()) };
}
