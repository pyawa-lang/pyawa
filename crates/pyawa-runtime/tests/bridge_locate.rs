//! **只定位、不执行**（第 222 轮；`IM-31`：finder 的 `find_spec` 只能"找" ✗ 不许"装" ✓）：
//! `can_locate_through_bridge` 报告"找得到吗" ✓ —— 而**绝不能**顺手把模块装掉 ✗
//! （把"装"放进 `find_spec` 正是第 217／218 轮那条 SIGSEGV 的来源 ✓：嵌套导入会**重入**导入机制 ✗）。

use core::ffi::c_void;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_runtime::{fs_posix::PosixFs, PaState};

#[test]
fn locating_a_file_backed_module_does_not_execute_it() {
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

    // ① 文件型模块：**找得到** ✓（`os.py` 在 `sys.path` 上 ✓）
    assert!(
        pyawa_core::executor::import::can_locate_through_bridge(instance, "os"),
        "`os.py` 在 `sys.path` 上，应当定位得到 ✓"
    );
    // ② 但**没有**执行它：模块表里不许出现 `os` ✓（这是本格的要点 ✓）
    let modules = instance.modules().expect("模块表已装配");
    assert!(
        instance.dict_get(modules, "os").is_none(),
        "定位**不许**顺手把模块装掉 ✗（那正是 `find_spec` 重入的根因 ✓）"
    );
    // ③ 已在表里的（内建／stdlib 登记的）也算找得到 ✓
    assert!(pyawa_core::executor::import::can_locate_through_bridge(instance, "errno"));
    // ④ 不存在的名字 ⇒ 找不到 ✓
    assert!(!pyawa_core::executor::import::can_locate_through_bridge(instance, "pyawa_no_such_module"));
}
