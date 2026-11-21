//! **S3.1（`AB-89`）**：host await 的取消/销毁/throw 派发（INTERRUPT／DESTROY／THROW）。

use core::ffi::c_void;
use core::mem::size_of;
use core::sync::atomic::{AtomicI32, Ordering};

use pyawa_abi::host::{pa_sig, pa_type_ops, PaHostDealloc, PaHostTraverse};
use pyawa_abi::status::*;
use pyawa_abi::*;

static LAST_REASON: AtomicI32 = AtomicI32::new(-1);
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    unsafe { *token_out = 77 };
    PA_OK
}
unsafe extern "C" fn host_await_cancel(
    _state: *mut pa_state,
    _payload: *mut c_void,
    _context: *mut c_void,
    reason: i32,
) {
    LAST_REASON.store(reason, Ordering::SeqCst);
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
            c"host-await-cancel".as_ptr(),
            c"python".as_ptr(),
            core::ptr::null(),
        )
    }
}

fn get_global(state: *mut pa_state, name: &str) -> i32 {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { pa_getglobal(state, name.as_ptr()) }
}

/// 建一个挂起在宿主 awaitable 上的协程，协程存全局 `c`。
fn setup() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    assert_eq!(
        exec(
            state,
            "async def f(x):\n    return (await x) + 1\nexc = ValueError('boom')\n",
        ),
        PA_OK
    );
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
        await_cancel_fn: Some(host_await_cancel),
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
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);
    LAST_REASON.store(-1, Ordering::SeqCst);
    state
}

#[test]
fn interrupt_calls_await_cancel_with_interrupt() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = setup();
    assert_eq!(unsafe { pa_interrupt(state) }, PA_OK);
    assert_eq!(get_global(state, "c"), PA_OK);
    assert_eq!(unsafe { pa_pushinteger(state, 1) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 1, 1) }, PA_ERR_INTERRUPT);
    assert_eq!(LAST_REASON.load(Ordering::SeqCst), 0);
    unsafe { pa_destroy(state) };
}

#[test]
fn throw_calls_await_cancel_with_throw() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = setup();
    assert_eq!(get_global(state, "c"), PA_OK);
    assert_eq!(get_global(state, "exc"), PA_OK);
    assert_eq!(unsafe { pa_throw(state, 1, 1) }, PA_ERR_RUNTIME);
    assert_eq!(LAST_REASON.load(Ordering::SeqCst), 2);
    unsafe { pa_destroy(state) };
}

#[test]
fn destroy_calls_await_cancel_with_destroy() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = setup();
    unsafe { pa_destroy(state) };
    assert_eq!(LAST_REASON.load(Ordering::SeqCst), 1);
}
