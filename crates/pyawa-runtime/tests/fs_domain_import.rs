//! **`IM-15` 的可执行判据**（第 212 轮）：模块 I/O **经能力层** ✓。
//!
//! 两边的**装配完全相同** ✓（都装 stdlib），只在**注册不注册 `fs` 域**上不同：
//! 注册了 ⇒ 文件型模块（`os`）导得进来 ✓；不注册 ⇒ 导不进来 ✗。
//! 差别只可能来自"读模块文件要过 `fs` 域" ⇒ 这就是 `IM-15` 落到可执行面的判据 ✓
//! （而不是只看 `read_file_through_fs` 这个名字 ✗）。

use core::ffi::c_void;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_runtime::{fs_posix::PosixFs, PaState};

/// 装配一个实例（装 stdlib），按需注册 `fs` 域，然后跑 `import os`，回状态码。
fn import_os(register_fs: bool) -> i32 {
    let state = PaState::new().expect("建实例");
    let raw = state.as_ptr();
    // SAFETY: `raw` 由 `PaState::new` 交回且活到本函数末尾。
    let instance = unsafe { &*raw }.instance();
    pyawa_stdlib::install(instance, "probe", &[]);
    if register_fs {
        let provider = PosixFs::new();
        let vtable = provider.vtable();
        let implementation = (&vtable as *const pyawa_capabilities::fs::CpFsVtable).cast::<c_void>();
        // SAFETY: `raw` 刚建成功；`vtable` 活到本函数末尾。
        let async_status = unsafe {
            pyawa_abi::pa_setcapability_async(raw, PA_DOMAIN_FS, PA_ASYNC_OK)
        };
        let registered = unsafe { pyawa_abi::pa_setcapability(raw, PA_DOMAIN_FS, implementation) };
        assert_eq!(async_status, 0, "`fs` 域分类注册应当成功");
        assert_eq!(registered, 0, "`fs` 域注册应当成功");
    }
    let source = b"import os\n";
    let mode = b"python\0";
    // SAFETY: 源码与模式名都是 NUL 结尾的常量；`opts` 传 NULL（`AB-60`／`AB-61`）。
    unsafe {
        pyawa_abi::pa_exec_string(
            raw,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

#[test]
fn file_backed_import_needs_the_fs_domain() {
    let with_fs = import_os(true);
    assert_eq!(with_fs, 0, "注册了 `fs` 域，`import os` 应当成功（PA_OK）");
    let without_fs = import_os(false);
    assert_ne!(
        without_fs, 0,
        "不注册 `fs` 域却把文件型模块导进来了 ⇒ 模块 I/O 没走能力层（`IM-15`）"
    );
}
