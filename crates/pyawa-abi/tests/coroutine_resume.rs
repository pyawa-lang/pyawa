//! **S3（`PLAN §12.27`）**：`pa_resume`／`pa_throw` 的 ABI 验收（`T-AB-21`…`T-AB-23`）。
//!
//! 纯 ABI、无 numpy：`pa_create` + `pyawa_stdlib::install`（生成器/协程依赖 builtins）。

use core::mem::size_of;

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

fn exec(state: *mut pa_state, source: &str) -> i32 {
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            c"coroutine-resume".as_ptr(),
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


fn push_int(state: *mut pa_state, value: i64) {
    assert_eq!(unsafe { pa_pushinteger(state, value) }, PA_OK);
}

const SRC: &str = r#"
def gen():
    a = yield 1
    b = yield a + 2
    return b + 3

def cat():
    try:
        yield 7
    except ValueError:
        yield 99

gg = gen()
cc = cat()
exc = ValueError('boom')
"#;

#[test]
fn resume_yields_completes_and_rejects_completed() {
    let state = new_state();
    assert_eq!(exec(state, SRC), PA_OK);

    assert_eq!(get_global(state, "gg"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(top_int(state), 1);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    assert_eq!(get_global(state, "gg"), PA_OK);
    push_int(state, 10);
    assert_eq!(unsafe { pa_resume(state, 1, 1) }, PA_YIELDED);
    assert_eq!(top_int(state), 12);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    assert_eq!(get_global(state, "gg"), PA_OK);
    push_int(state, 5);
    assert_eq!(unsafe { pa_resume(state, 1, 1) }, PA_COMPLETED);
    assert_eq!(top_int(state), 8);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    // 已完成 ⇒ INVALID（T-AB-22）；栈不变。
    assert_eq!(get_global(state, "gg"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_ERR_INVALID);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    unsafe { pa_destroy(state) };
}

#[test]
fn throw_injects_exception_and_first_value_resume_is_invalid() {
    let state = new_state();
    assert_eq!(exec(state, SRC), PA_OK);

    // 首次 resume 带值 ⇒ INVALID（AB-84：首次必须 0）。
    assert_eq!(get_global(state, "cc"), PA_OK);
    push_int(state, 1);
    assert_eq!(unsafe { pa_resume(state, 1, 1) }, PA_ERR_INVALID);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    assert_eq!(get_global(state, "cc"), PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(top_int(state), 7);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    // 取出脚本预置的 ValueError("boom") 并 throw 回协程。
    assert_eq!(get_global(state, "cc"), PA_OK);
    assert_eq!(get_global(state, "exc"), PA_OK);
    assert_eq!(unsafe { pa_throw(state, 1, 1) }, PA_YIELDED);
    assert_eq!(top_int(state), 99);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    unsafe { pa_destroy(state) };
}
