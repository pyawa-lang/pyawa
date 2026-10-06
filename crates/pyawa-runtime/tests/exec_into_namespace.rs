//! **`exec_module_into_namespace`**（第 235 轮；`IM-31` 的 loader 那半 ✓）：
//! 把模块源码装进**给定**的名字空间 ✓ —— 与 `can_locate_through_bridge`（只定位 ✓）配成一对，
//! 正是 finder 的 `find_spec`（找 ✓）＋ `exec_module`（装 ✓）两半 ✓。

use core::ffi::c_void;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_runtime::{fs_posix::PosixFs, PaState};

#[test]
fn loading_into_a_given_namespace_executes_the_module_there() {
    let state = PaState::new().expect("建实例");
    let raw = state.as_ptr();
    // SAFETY: `raw` 由 `PaState::new` 交回且活到本函数末尾。
    let instance = unsafe { &*raw }.instance();
    pyawa_stdlib::install(instance, "probe", &[]);
    let provider = PosixFs::new();
    let vtable = provider.vtable();
    let implementation = (&vtable as *const pyawa_capabilities::fs::CpFsVtable).cast::<c_void>();
    // SAFETY: `raw` 刚建成功；`vtable` 活到本函数末尾。
    unsafe {
        assert_eq!(
            pyawa_abi::pa_setcapability_async(raw, PA_DOMAIN_FS, PA_ASYNC_OK),
            0
        );
        assert_eq!(pyawa_abi::pa_setcapability(raw, PA_DOMAIN_FS, implementation), 0);
    }
    let lib = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/lib-full");
    pyawa_stdlib::set_module_search_path(instance, &[String::from(lib)]);

    // 目标名字空间由调用方给（`_bootstrap` 的 `create_module` 给的就是它 ✓）。
    let namespace = instance.new_dict();
    let loaded = pyawa_core::executor::import::exec_module_into_namespace(instance, "os", namespace)
        .expect("装载不该在这里报错");
    assert!(loaded, "`os.py` 在 `sys.path` 上 ⇒ 应当装进去 ✓");
    // 装进了**给定**的名字空间 ✓（`os.sep` 是 os.py 自己写的 ✓）
    assert!(
        instance.dict_get(namespace, "sep").is_some(),
        "`os.py` 的 `sep` 必须写进**我们给的那个**名字空间 ✓"
    );

    let missing_namespace = instance.new_dict();
    let missing = pyawa_core::executor::import::exec_module_into_namespace(
        instance,
        "pyawa_no_such_module",
        missing_namespace,
    )
    .expect("找不到不是异常 ✓");
    assert!(!missing, "找不到的模块必须如实给 `false` ✓");
}
