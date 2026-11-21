//! **S1.1（`AB-88`/`AB-90`）**：脚本 `await` 宿主对象 ⇒ VM 让出 token，宿主 `pa_resume` 送值完成。
//!
//! 覆盖：`pa_type_ops.await_fn` 注册、GET_AWAITABLE 的 host await、SEND 两阶段、
//! `PA_YIELDED`/`PA_COMPLETED` 状态。

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

fn new_state() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    state
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
    // SAFETY: 契约保证 token_out 可写。
    unsafe { *token_out = 77 };
    PA_OK
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
            c"host-awaitable".as_ptr(),
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
fn await_host_object_yields_token_and_resumes() {
    let state = new_state();
    assert_eq!(exec(state, "async def f(x):\n    return (await x) + 1\n"), PA_OK);

    // 注册宿主类型，并装 await_fn。
    let signature = empty_signature();
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                c"HostAwait".as_ptr(),
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
        await_cancel_fn: None,
    };
    assert_eq!(unsafe { pa_set_type_ops(state, -1, &ops) }, PA_OK);

    let mut payload = core::ptr::null_mut();
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    // 栈 [type, instance]：把实例交给脚本全局 x，type 留在栈上。
    assert_eq!(unsafe { pa_setglobal(state, c"x".as_ptr()) }, PA_OK);

    // 取 f、把 x 作为实参调用 ⇒ coroutine。
    assert_eq!(get_global(state, "f"), PA_OK);
    assert_eq!(get_global(state, "x"), PA_OK);
    assert_eq!(unsafe { pa_call(state, 1, 1) }, PA_OK);
    // 协程存到全局 c，供多次 resume（pa_resume 会消费栈上的协程）。
    assert_eq!(unsafe { pa_setglobal(state, c"c".as_ptr()) }, PA_OK);

    // 首次 resume ⇒ 协程 `await x` 让出 token 77。
    assert_eq!(get_global(state, "c"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    let mut token = 0i64;
    assert_eq!(unsafe { pa_tointeger(state, -1, &mut token) }, PA_OK);
    assert_eq!(token, 77);

    // 送值 41 ⇒ await 结果为 41，f 返回 42。
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);
    assert_eq!(get_global(state, "c"), PA_OK);
    assert_eq!(unsafe { pa_pushinteger(state, 41) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 1, 1) }, PA_COMPLETED);
    let mut value = 0i64;
    assert_eq!(unsafe { pa_tointeger(state, -1, &mut value) }, PA_OK);
    assert_eq!(value, 42);

    unsafe { pa_destroy(state) };
}
