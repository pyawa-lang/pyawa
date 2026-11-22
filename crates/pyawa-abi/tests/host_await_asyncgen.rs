//! **P0-2**：async generator / `async for` 内部的 host awaitable 驱动。

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
unsafe extern "C" fn host_await(
    _state: *mut pa_state,
    _payload: *mut c_void,
    _context: *mut c_void,
    token_out: *mut u64,
) -> i32 {
    unsafe { *token_out = 99 };
    PA_OK
}
unsafe extern "C" fn host_await_cancel(
    _state: *mut pa_state,
    _payload: *mut c_void,
    _context: *mut c_void,
    _reason: i32,
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
fn exec(state: *mut pa_state, source: &str) -> i32 {
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            c"asyncgen-host-await".as_ptr(),
            c"python".as_ptr(),
            core::ptr::null(),
        )
    }
}
fn get_global(state: *mut pa_state, name: &str) -> i32 {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { pa_getglobal(state, name.as_ptr()) }
}
fn top_int(state: *mut pa_state) -> i64 {
    let mut value = 0i64;
    assert_eq!(unsafe { pa_tointeger(state, -1, &mut value) }, PA_OK);
    value
}

#[test]
fn async_generator_can_await_host_object() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    let script = r#"
async def agen(x):
    yield (await x) + 1

async def main(x):
    out = []
    async for v in agen(x):
        out.append(v)
    return out[0]
"#;
    assert_eq!(exec(state, script), PA_OK);
    let signature = empty_signature();
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                c"HostAwaitAsyncGen".as_ptr(),
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
        await_fn: Some(host_await),
        await_cancel_fn: Some(host_await_cancel),
    };
    assert_eq!(unsafe { pa_set_type_ops(state, -1, &ops) }, PA_OK);
    let mut payload = core::ptr::null_mut();
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    assert_eq!(unsafe { pa_setglobal(state, c"x".as_ptr()) }, PA_OK);
    assert_eq!(get_global(state, "main"), PA_OK);
    assert_eq!(get_global(state, "x"), PA_OK);
    assert_eq!(unsafe { pa_call(state, 1, 1) }, PA_OK);
    assert_eq!(unsafe { pa_setglobal(state, c"c".as_ptr()) }, PA_OK);
    assert_eq!(get_global(state, "c"), PA_OK);
    let first = unsafe { pa_resume(state, 0, 1) };
    if first != PA_YIELDED {
        let raw = unsafe { pa_errmsg(state) };
        let msg = if raw.is_null() { String::new() } else { unsafe { core::ffi::CStr::from_ptr(raw) }.to_string_lossy().into_owned() };
        panic!("first resume = {first}, errmsg = {msg}");
    }
    assert_eq!(top_int(state), 99);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);
    assert_eq!(get_global(state, "c"), PA_OK);
    assert_eq!(unsafe { pa_pushinteger(state, 41) }, PA_OK);
    let second = unsafe { pa_resume(state, 1, 1) };
    if second != PA_COMPLETED {
        let raw = unsafe { pa_errmsg(state) };
        let msg = if raw.is_null() { String::new() } else { unsafe { core::ffi::CStr::from_ptr(raw) }.to_string_lossy().into_owned() };
        panic!("second resume = {second}, errmsg = {msg}");
    }
    // 结果应为 42。
    assert_eq!(top_int(state), 42);
    unsafe { pa_destroy(state) };
}
