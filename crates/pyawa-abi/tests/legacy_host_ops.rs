//! **S3 零回归**：1.14 宿主（旧 `pa_type_ops` 尺寸、旧 ABI 次版本）仍可用；新字段按"未提供"。

use core::ffi::c_void;
use core::mem::size_of;

use pyawa_abi::host::{pa_sig, pa_type_ops, PaHostDealloc, PaHostTraverse, PaTypeGetAttr, PaTypeSetAttr};
use pyawa_abi::status::*;
use pyawa_abi::*;

#[repr(C)]
struct LegacyTypeOps {
    size: usize,
    context: *mut c_void,
    getattr_fn: Option<PaTypeGetAttr>,
    setattr_fn: Option<PaTypeSetAttr>,
}

unsafe extern "C" fn noop_dealloc(_payload: *mut c_void) {}
unsafe extern "C" fn noop_traverse(
    _payload: *mut c_void,
    _context: *mut c_void,
    _visit: unsafe extern "C" fn(*mut c_void, *mut c_void),
) {
}

fn empty_signature() -> pa_sig {
    pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    }
}

#[test]
fn legacy_type_ops_size_and_old_abi_host_still_work() {
    let host = pa_host {
        abi_size: size_of::<pa_host>(),
        // 1.14 宿主：低于当前 1.17 ⇒ 兼容（AB-41 追加式）。
        abi_version: (1u32 << 16) | 14,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };

    let signature = empty_signature();
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                c"LegacyHostType".as_ptr(),
                0,
                0,
                Some(noop_dealloc as PaHostDealloc),
                Some(noop_traverse as PaHostTraverse),
                &signature,
            )
        },
        PA_OK
    );
    // 旧宿主只声明到 setattr 字段；新字段（await_fn/await_cancel_fn）不在 `size` 内 ⇒ 未提供。
    let legacy = LegacyTypeOps {
        size: size_of::<LegacyTypeOps>(),
        context: core::ptr::null_mut(),
        getattr_fn: None,
        setattr_fn: None,
    };
    assert_eq!(
        unsafe { pa_set_type_ops(state, -1, (&legacy as *const LegacyTypeOps).cast::<pa_type_ops>()) },
        PA_OK
    );
    // 新尺寸仍可注册（同一实例两种记录并存）。
    let fresh = pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: core::ptr::null_mut(),
        getattr_fn: None,
        setattr_fn: None,
        await_fn: None,
        await_cancel_fn: None,
    };
    assert_eq!(unsafe { pa_set_type_ops(state, -1, &fresh) }, PA_OK);

    unsafe { pa_destroy(state) };
}
