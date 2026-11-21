//! **S3 修复验收**：同一宿主对象被多协程 await（不串态）、按协程取消、u64 token 精度。

use core::ffi::c_void;
use core::mem::size_of;
use core::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use pyawa_abi::host::{pa_sig, pa_type_ops, PaHostDealloc, PaHostTraverse};
use pyawa_abi::status::*;
use pyawa_abi::*;

static TOKEN: AtomicU64 = AtomicU64::new(77);
static CANCEL_COUNT: AtomicI32 = AtomicI32::new(0);
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
    unsafe { *token_out = TOKEN.load(Ordering::SeqCst) };
    PA_OK
}
unsafe extern "C" fn host_await_cancel(
    _state: *mut pa_state,
    _payload: *mut c_void,
    _context: *mut c_void,
    reason: i32,
) {
    CANCEL_COUNT.fetch_add(1, Ordering::SeqCst);
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
            c"host-awaitable-multi".as_ptr(),
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

fn setup() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    assert_eq!(
        exec(
            state,
            "async def f(x):\n    return (await x) + 1\nasync def g(x):\n    return (await x) + 2\nexc = ValueError('boom')\n",
        ),
        PA_OK
    );
    let signature = empty_signature();
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                c"HostAwaitMulti".as_ptr(),
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
    state
}

fn start(state: *mut pa_state, fname: &str, cname: &str) {
    assert_eq!(get_global(state, fname), PA_OK);
    assert_eq!(get_global(state, "x"), PA_OK);
    assert_eq!(unsafe { pa_call(state, 1, 1) }, PA_OK);
    let cname = std::ffi::CString::new(cname).unwrap();
    assert_eq!(unsafe { pa_setglobal(state, cname.as_ptr()) }, PA_OK);
}

#[test]
fn two_coroutines_can_await_same_host_object() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = setup();
    TOKEN.store(77, Ordering::SeqCst);
    start(state, "f", "c1");
    start(state, "g", "c2");

    // 两个协程依次让出同一个 token（状态按帧隔离，不串）。
    assert_eq!(get_global(state, "c1"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(top_int(state), 77);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);
    assert_eq!(get_global(state, "c2"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(top_int(state), 77);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    assert_eq!(get_global(state, "c1"), PA_OK);
    assert_eq!(unsafe { pa_pushinteger(state, 10) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 1, 1) }, PA_COMPLETED);
    assert_eq!(top_int(state), 11);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);
    assert_eq!(get_global(state, "c2"), PA_OK);
    assert_eq!(unsafe { pa_pushinteger(state, 20) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 1, 1) }, PA_COMPLETED);
    assert_eq!(top_int(state), 22);
    unsafe { pa_destroy(state) };
}

#[test]
fn throw_cancel_only_affects_the_target_coroutine() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = setup();
    TOKEN.store(77, Ordering::SeqCst);
    CANCEL_COUNT.store(0, Ordering::SeqCst);
    LAST_REASON.store(-1, Ordering::SeqCst);
    start(state, "f", "c1");
    start(state, "g", "c2");
    assert_eq!(get_global(state, "c1"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);
    assert_eq!(get_global(state, "c2"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    // throw 只取消 c1 的 host await；c2 仍可完成。
    assert_eq!(get_global(state, "c1"), PA_OK);
    assert_eq!(get_global(state, "exc"), PA_OK);
    assert_eq!(unsafe { pa_throw(state, 1, 1) }, PA_ERR_RUNTIME);
    assert_eq!(CANCEL_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(LAST_REASON.load(Ordering::SeqCst), 2);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);
    assert_eq!(get_global(state, "c2"), PA_OK);
    assert_eq!(unsafe { pa_pushinteger(state, 20) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 1, 1) }, PA_COMPLETED);
    assert_eq!(top_int(state), 22);
    unsafe { pa_destroy(state) };
}

#[test]
fn u64_token_survives_as_arbitrary_precision_int() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = setup();
    TOKEN.store(u64::MAX, Ordering::SeqCst);
    start(state, "f", "c1");
    assert_eq!(get_global(state, "c1"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    let mut len = 0usize;
    let text = unsafe { pa_tointstring(state, -1, &mut len) };
    assert!(!text.is_null());
    let actual = unsafe { core::slice::from_raw_parts(text.cast::<u8>(), len) };
    assert_eq!(actual, u64::MAX.to_string().as_bytes());
    unsafe { pa_destroy(state) };
}
