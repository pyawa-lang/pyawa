//! **§12.13 #2/#3/#4**（`T-AB-17`／`T-AB-16`／`T-AB-15`；`SPEC-c-abi` 的 `AB-36`／`§15.3`／`AB-79`）。
//!
//! - #2：`pa_newtype` 的 NULL `dealloc`/`traverse` ⇒ **当场** `PA_ERR_INVALID`＋`pa_errmsg`；
//! - #3：`pa_getglobal` 缺失名 ⇒ `PA_ERR_INVALID` 且**不压栈**；
//! - #4：`pa_pushmodule` 把模块句柄上栈（+1），C 宿主据此给模块挂 `pa_object_ops`。

use core::ffi::{c_char, c_void};
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

fn empty_signature() -> pa_sig {
    pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    }
}

fn message_of(state: *mut pa_state) -> String {
    let raw = unsafe { pa_errmsg(state) };
    if raw.is_null() {
        return String::new();
    }
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
            b"usability\0".as_ptr().cast(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

fn new_state() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    state
}

/// 属性回调：`answer` ⇒ 42；其它 ⇒ 未处理。
unsafe extern "C" fn answer_getattr(
    state: *mut pa_state,
    _payload: *mut c_void,
    name: *const c_char,
    len: usize,
    _context: *mut c_void,
) -> i32 {
    // SAFETY: 契约保证 name 至少 len 字节可读。
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), len) };
    match name {
        b"answer" => unsafe { pa_pushinteger(state, 42) },
        _ => PA_ERR_NOTFOUND,
    }
}

#[test]
fn newtype_rejects_null_lifecycle_hooks_with_message() {
    let state = new_state();
    let signature = empty_signature();
    let before = unsafe { pa_gettop(state) };
    // SAFETY: 刻意把 NULL 塞进宿主回调形参（C 宿主的 `NULL` 形态），验证运行时**当场拒绝**。
    // C 宿主的 `NULL` 经 `Option<extern fn>` niche 映射为 `None`。
    let rejected = unsafe {
        pa_newtype(
            state,
            b"NullHooks\0".as_ptr().cast(),
            0,
            0,
            None,
            None,
            &signature,
        )
    };
    assert_eq!(rejected, PA_ERR_INVALID, "NULL hooks 必须当场拒绝");
    assert!(
        !message_of(state).is_empty(),
        "`AB-48`：拒绝必须带可诊断消息（§12.13 #2）"
    );
    assert_eq!(unsafe { pa_gettop(state) }, before, "拒绝不得动栈");
    // 合法注册仍照常（对照）。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"GoodHooks\0".as_ptr().cast(),
                0,
                0,
                Some(noop_dealloc as PaHostDealloc),
                Some(noop_traverse as PaHostTraverse),
                &signature,
            )
        },
        PA_OK,
        "合法注册不应被误伤：{}",
        message_of(state)
    );
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn getglobal_missing_name_is_an_error_and_does_not_push() {
    let state = new_state();
    let before = unsafe { pa_gettop(state) };
    // SAFETY: 名字 NUL 结尾。
    let status = unsafe { pa_getglobal(state, b"__does_not_exist__\0".as_ptr().cast()) };
    assert_eq!(status, PA_ERR_INVALID, "缺失名 ⇒ `PA_ERR_INVALID`（§12.13 #3）");
    assert_eq!(unsafe { pa_gettop(state) }, before, "缺失名不得压栈");
    assert!(!message_of(state).is_empty(), "缺失名消息必须可诊断（`AB-48`）");

    // 存在名照旧：`PA_OK` 且压栈一份新引用。
    // SAFETY: 栈操作按契约。
    assert_eq!(unsafe { pa_pushinteger(state, 7) }, PA_OK);
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(unsafe { pa_setglobal(state, b"present\0".as_ptr().cast()) }, PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_getglobal(state, b"present\0".as_ptr().cast()) }, PA_OK);
    assert_eq!(unsafe { pa_gettop(state) }, before + 1);
    // SAFETY: 收栈。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn pushmodule_exposes_module_object_to_c_hosts() {
    let state = new_state();
    let mut module: *mut c_void = core::ptr::null_mut();
    // SAFETY: 名字 NUL 结尾；out 可写。
    assert_eq!(
        unsafe { pa_add_host_module(state, b"pushmod\0".as_ptr().cast(), &mut module) },
        PA_OK,
        "{}",
        message_of(state)
    );
    let before = unsafe { pa_gettop(state) };
    // SAFETY: 模块句柄有效。
    assert_eq!(
        unsafe { pa_pushmodule(state, module) },
        PA_OK,
        "`pa_pushmodule` 失败：{}",
        message_of(state)
    );
    assert_eq!(unsafe { pa_gettop(state) }, before + 1, "栈契约 +1");
    // C 宿主路径：给模块对象挂协议钩子（`AB-75` 模块态）。
    let ops = pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: core::ptr::null_mut(),
        getattr_fn: Some(answer_getattr),
        setattr_fn: None,
    };
    // SAFETY: 模块对象在栈顶（-1，不消耗）。
    assert_eq!(
        unsafe { pa_set_object_ops(state, -1, &ops) },
        PA_OK,
        "{}",
        message_of(state)
    );
    // SAFETY: 收掉模块对象。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);

    assert_eq!(
        exec(state, b"import pushmod\nassert pushmod.answer == 42\n"),
        PA_OK,
        "模块态属性回退失败：{}",
        message_of(state)
    );

    // 非法句柄：不得 deref，直接 `PA_ERR_INVALID`。
    // SAFETY: 故意传非空但无主的指针。
    assert_eq!(
        unsafe { pa_pushmodule(state, 1 as *mut c_void) },
        PA_ERR_INVALID
    );
    assert!(!message_of(state).is_empty());
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
