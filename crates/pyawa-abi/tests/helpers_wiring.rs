//! **§12.18 C 组接线**：`paL_dostring`／`paL_dofile`／`paL_where`／`paL_requiref`。
//!
//! 纯 ABI、无 numpy；`paL_errorf` 是 C 头 `static inline`（由 `examples/reference_host.c` 真编译覆盖）。

use core::ffi::{c_char, c_void};
use core::mem::size_of;

use pyawa_abi::helpers::{paL_dofile, paL_dostring, paL_requiref, paL_where};
use pyawa_abi::host::pa_sig;
use pyawa_abi::status::*;
use pyawa_abi::*;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

fn new_state(with_stdlib: bool) -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    if with_stdlib {
        // SAFETY: state 由 pa_create 交回、尚未销毁。
        unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    }
    state
}

fn message_of(state: *mut pa_state) -> String {
    // SAFETY: state 存活。
    let raw = unsafe { pa_errmsg(state) };
    if raw.is_null() {
        return String::new();
    }
    // SAFETY: NUL 结尾借用串。
    unsafe { core::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

fn exec(state: *mut pa_state, source: &[u8]) -> i32 {
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            b"helpers-wiring\0".as_ptr().cast(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

#[test]
fn dostring_executes_and_pushes_true() {
    let state = new_state(false);
    // SAFETY: state/source 按契约。
    assert_eq!(
        unsafe { paL_dostring(state, b"x = 1 + 1\0".as_ptr().cast(), 0) },
        PA_OK,
        "{}",
        message_of(state)
    );
    // SAFETY: paL_execresult 在 PA_OK 时压 True。
    assert_eq!(unsafe { pa_isboolean(state, -1) }, 1);
    // SAFETY: 收栈。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // invalid mode。
    // SAFETY: 同上。
    assert_eq!(
        unsafe { paL_dostring(state, b"x = 1\0".as_ptr().cast(), 7) },
        PA_ERR_INVALID
    );
    assert!(!message_of(state).is_empty());
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn dofile_without_fs_is_not_implemented() {
    let state = new_state(false);
    // SAFETY: path 按契约；本实例无 fs 域。
    assert_eq!(
        unsafe { paL_dofile(state, b"/tmp/no-such-file.py\0".as_ptr().cast(), 0) },
        PA_ERR_NOTIMPLEMENTED
    );
    assert!(!message_of(state).is_empty());
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn requiref_reuses_sys_modules_and_openf() {
    let state = new_state(true);
    let before = unsafe { pa_gettop(state) };
    // `sys` 已在 `sys.modules`（stdlib 装配）⇒ 直接取。
    // SAFETY: name 按契约。
    assert_eq!(
        unsafe { paL_requiref(state, b"sys\0".as_ptr().cast(), core::ptr::null(), 0) },
        PA_OK,
        "{}",
        message_of(state)
    );
    assert_eq!(unsafe { pa_gettop(state) }, before + 1);
    // SAFETY: 收栈。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);

    // 未登记名字 + 宿主 openf：openf 压一个值，requiref 登记并压栈。
    unsafe extern "C" fn open_module(state: *mut pa_state) -> i32 {
        // SAFETY: state 由调用方交回。
        unsafe { pa_newtable(state) }
    }
    // SAFETY: 回调指针按契约交。
    assert_eq!(
        unsafe {
            paL_requiref(
                state,
                b"host_mod\0".as_ptr().cast(),
                open_module as *const c_void,
                0,
            )
        },
        PA_OK,
        "{}",
        message_of(state)
    );
    assert_eq!(unsafe { pa_gettop(state) }, before + 1);
    // SAFETY: 收栈。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // 同一名字第二次 ⇒ 从 sys.modules 命中（openf 不再被调）。
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            paL_requiref(
                state,
                b"host_mod\0".as_ptr().cast(),
                core::ptr::null(),
                0,
            )
        },
        PA_OK
    );
    assert_eq!(unsafe { pa_gettop(state) }, before + 1);
    // SAFETY: 收栈。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn where_reports_current_frame_code_line_inside_host_callback() {
    let state = new_state(false);
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    unsafe extern "C-unwind" fn host_where(state: *mut pa_state) -> i32 {
        let mut out: *const c_char = core::ptr::null();
        let mut len = 0usize;
        // SAFETY: 由 VM 回调，当前帧存在。
        if unsafe { paL_where(state, 0, &mut out, &mut len) } != PA_OK {
            return PA_ERR_RUNTIME;
        }
        // SAFETY: out/len 是借用视图，复制后压栈。
        unsafe { pa_pushstring(state, out, len as isize) }
    }
    // SAFETY: 注册 0 形参宿主函数。
    assert_eq!(
        unsafe { pa_register(state, b"host_where\0".as_ptr().cast(), host_where, &signature) },
        PA_OK
    );
    // SAFETY: 执行 guest。
    assert_eq!(
        exec(state, b"where_text = host_where()\nassert ':' in where_text, where_text\n"),
        PA_OK,
        "{}",
        message_of(state)
    );
    // 无当前帧时 ⇒ 如实未实现。
    let mut out: *const c_char = core::ptr::null();
    let mut len = 0usize;
    // SAFETY: 状态已不在执行期。
    assert_eq!(
        unsafe { paL_where(state, 0, &mut out, &mut len) },
        PA_ERR_NOTIMPLEMENTED
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
