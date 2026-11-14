//! **`AB-73` 宿主类型属性派发**（`PLAN` §12.11 的 B 阶段第一步；`SPEC-c-abi.md` 的 `AB-73`）。
//!
//! 已覆盖：属性回退读（`getattr_fn` 命中／`PA_ERR_NOTFOUND` 回退到 `AttributeError`／宿主
//! 错误 ⇒ 脚本可捕获的 `RuntimeError`）、`pa_set_type_ops(NULL)` 与"未装 ops"的**零回归**。
//! 未覆盖（下一步 B2）：`setattr_fn`、方法绑定、`AB-74` 异常映射、`len`/`for`/`+` 协议链路。

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

/// 宿主类型的属性回调：`answer` ⇒ 42；`boom` ⇒ 错误；其它 ⇒ 未处理（回退）。
unsafe extern "C" fn type_getattr(
    state: *mut pa_state,
    _payload: *mut c_void,
    name: *const c_char,
    len: usize,
    _context: *mut c_void,
) -> i32 {
    // SAFETY: 契约保证 name 至少 len 字节可读。
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), len) };
    match name {
        b"answer" => {
            // SAFETY: state 是当前实例。
            unsafe { pa_pushinteger(state, 42) }
        }
        b"boom" => PA_ERR_INVALID,
        _ => PA_ERR_NOTFOUND,
    }
}

fn ops_with(getattr_fn: Option<pyawa_abi::host::PaTypeGetAttr>) -> pa_type_ops {
    pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: core::ptr::null_mut(),
        getattr_fn,
        setattr_fn: None,
    }
}

fn message_of(state: *mut pa_state) -> String {
    // SAFETY: state 由调用方保证存活。
    let raw = unsafe { pa_errmsg(state) };
    if raw.is_null() {
        return String::new();
    }
    // SAFETY: pa_errmsg 交回 NUL 结尾的借用串。
    unsafe { core::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

fn exec(state: *mut pa_state, source: &[u8]) -> i32 {
    // SAFETY: 按 `pa_exec_string` 契约传参（显式长度、NUL 结尾块名/模式）。
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            b"ab73\0".as_ptr().cast(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

/// 建一个宿主类型实例并绑定到全局名；返回类型在栈上的位置由调用方先 `pa_newtype`。
fn install_instance(state: *mut pa_state, name: &[u8]) {
    let mut payload: *mut c_void = core::ptr::null_mut();
    // 类型在栈顶（pa_newtype 的 +1）；`pa_newhandle` 的 type_index 指它，且不消耗。
    // SAFETY: 契约见 `pa_newhandle`。
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    // 实例在栈顶 ⇒ 写成全局（消费栈顶）。
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(unsafe { pa_setglobal(state, name.as_ptr().cast()) }, PA_OK);
    // 类型还在栈上 ⇒ 收掉（不消耗属性的检查用）。
    // SAFETY: 上面确认栈上有类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
}

fn setup_type_with_ops(state: *mut pa_state, type_name: &[u8], global: &[u8], callback: pyawa_abi::host::PaTypeGetAttr) {
    let signature = empty_signature();
    // SAFETY: 名字/函数/签名都有效；`pa_newtype` 压入类型对象（+1）。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                type_name.as_ptr().cast(),
                0,
                noop_dealloc as PaHostDealloc,
                noop_traverse as PaHostTraverse,
                &signature,
            )
        },
        PA_OK,
        "注册宿主类型失败：{}",
        message_of(state)
    );
    let ops = ops_with(Some(callback));
    // SAFETY: 类型在栈顶（-1），ops 有效。
    assert_eq!(
        unsafe { pa_set_type_ops(state, -1, &ops) },
        PA_OK,
        "装 pa_type_ops 失败：{}",
        message_of(state)
    );
    install_instance(state, global);
}

#[test]
fn host_attribute_dispatch_and_fallback() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    setup_type_with_ops(state, b"Thing\0", b"obj\0", type_getattr);

    // ① 命中：`obj.answer` ⇒ 42。
    assert_eq!(exec(state, b"value = obj.answer\ntaken = value == 42\n"), PA_OK);
    // ② 未处理 ⇒ VM 回退到默认 `AttributeError`（裸宿主没有内建名字可 `except` ⇒ 直接看未捕获消息）。
    assert_eq!(exec(state, b"obj.missing\n"), PA_ERR_RUNTIME);
    assert!(
        message_of(state).contains("AttributeError"),
        "回退应当是 AttributeError：{}",
        message_of(state)
    );
    // ③ 宿主错误 ⇒ 不得静默回退，必须转成脚本异常（`RuntimeError`）。
    assert_eq!(exec(state, b"obj.boom\n"), PA_ERR_RUNTIME);
    assert!(
        message_of(state).contains("RuntimeError"),
        "宿主错误应当是 RuntimeError：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn type_without_ops_is_unchanged() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // 未装 ops（`pa_set_type_ops` 的 `NULL` 路径）⇒ 行为与旧类型完全一致。
    let signature = empty_signature();
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"Plain\0".as_ptr().cast(),
                0,
                noop_dealloc as PaHostDealloc,
                noop_traverse as PaHostTraverse,
                &signature,
            )
        },
        PA_OK
    );
    // SAFETY: 类型在栈顶；NULL ⇒ 清空/不派发。
    assert_eq!(unsafe { pa_set_type_ops(state, -1, core::ptr::null()) }, PA_OK);
    install_instance(state, b"plain\0");

    // 未装 ops ⇒ 行为与旧类型一致：属性缺失走默认 `AttributeError`（未捕获 ⇒ 状态码＋消息）。
    assert_eq!(exec(state, b"plain.answer\n"), PA_ERR_RUNTIME);
    assert!(
        message_of(state).contains("AttributeError"),
        "未装 ops 的类型不应有属性：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
