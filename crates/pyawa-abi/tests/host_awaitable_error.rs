//! **P1**：host await 回调的失败映射——非 `PA_OK` ⇒ 脚本可捕获的 `RuntimeError`。

use core::ffi::c_void;
use core::mem::size_of;

use pyawa_abi::host::{pa_sig, pa_type_ops, PaHostDealloc, PaHostTraverse};
use pyawa_abi::status::*;
use pyawa_abi::*;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}
unsafe extern "C" fn noop_dealloc(_payload: *mut c_void) {}
unsafe extern "C" fn noop_traverse(
    _payload: *mut c_void,
    _context: *mut c_void,
    _visit: unsafe extern "C" fn(*mut c_void, *mut c_void),
) {
}
unsafe extern "C" fn failing_await(
    _state: *mut pa_state,
    _payload: *mut c_void,
    _context: *mut c_void,
    _token_out: *mut u64,
) -> i32 {
    PA_ERR_INVALID
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
fn exec(state: *mut pa_state, source: &str) -> i32 {
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            c"host-await-error".as_ptr(),
            c"python".as_ptr(),
            core::ptr::null(),
        )
    }
}
fn get_global(state: *mut pa_state, name: &str) -> i32 {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { pa_getglobal(state, name.as_ptr()) }
}

#[test]
fn await_callback_failure_maps_to_runtime_error() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    assert_eq!(exec(state, "async def f(x):\n    return await x\n"), PA_OK);
    let signature = empty_signature();
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                c"HostAwaitFail".as_ptr(),
                0,
                0,
                Some(noop_dealloc as PaHostDealloc),
                Some(noop_traverse as PaHostTraverse),
                &signature,
            )
        },
        PA_OK
    );
    let ops = pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: core::ptr::null_mut(),
        getattr_fn: None,
        setattr_fn: None,
        await_fn: Some(failing_await),
        await_cancel_fn: None,
    };
    assert_eq!(unsafe { pa_set_type_ops(state, -1, &ops) }, PA_OK);
    let mut payload = core::ptr::null_mut();
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    assert_eq!(unsafe { pa_setglobal(state, c"x".as_ptr()) }, PA_OK);
    assert_eq!(get_global(state, "f"), PA_OK);
    assert_eq!(get_global(state, "x"), PA_OK);
    assert_eq!(unsafe { pa_call(state, 1, 1) }, PA_OK);
    assert_eq!(unsafe { pa_setglobal(state, c"c".as_ptr()) }, PA_OK);
    assert_eq!(get_global(state, "c"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_ERR_RUNTIME);
    let raw = unsafe { pa_errmsg(state) };
    assert!(!raw.is_null());
    let msg = unsafe { core::ffi::CStr::from_ptr(raw) }.to_string_lossy();
    assert!(msg.contains("await 回调返回非 PA_OK"), "errmsg={msg}");
    unsafe { pa_destroy(state) };
}
