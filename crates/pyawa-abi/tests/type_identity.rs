//! **`AB-76` 真实类型身份＋`AB-77` 真实继承**（`T-AB-11`／`T-AB-13`；`PLAN` §12.12 步 1／步 4）。
//!
//! `pa_type` 保持 `AB-63` 的值类别；身份查询走新增的 `pa_typeid`。`pa_newtype` 的 `base_index`
//! 建立真实 MRO（`issubclass`／`isinstance` 对宿主类型天然成立），负值 ⇒ `object`。

use core::mem::size_of;

use pyawa_abi::host::{pa_sig, PaHostDealloc, PaHostTraverse};
use pyawa_abi::status::*;
use pyawa_abi::*;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

unsafe extern "C" fn noop_dealloc(_payload: *mut core::ffi::c_void) {}
unsafe extern "C" fn noop_traverse(
    _payload: *mut core::ffi::c_void,
    _context: *mut core::ffi::c_void,
    _visit: unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void),
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

fn typeid(state: *mut pa_state, index: i32) -> *mut core::ffi::c_void {
    let mut out: *mut core::ffi::c_void = core::ptr::null_mut();
    // SAFETY: state 由调用方保证存活；out 可写。
    assert_eq!(
        unsafe { pa_typeid(state, index, &mut out) },
        PA_OK,
        "pa_typeid 失败：{}",
        message_of(state)
    );
    assert!(!out.is_null());
    out
}

fn typeid_of_named(state: *mut pa_state, name: &str) -> *mut core::ffi::c_void {
    // SAFETY: state 存活。
    let instance = unsafe { &*state }.instance();
    let ty = instance.type_named(name).unwrap_or_else(|| panic!("{name} 未登记"));
    ty.cast::<pyawa_core::Header>().as_ptr().cast()
}

fn exec(state: *mut pa_state, source: &[u8]) -> i32 {
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            b"typeid\0".as_ptr().cast(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

#[test]
fn typeid_reports_real_type_identity() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };

    // 内建标量：`int`／`str` 身份不同，各自等于注册表里的类型。
    // SAFETY: 栈操作按契约。
    assert_eq!(unsafe { pa_pushinteger(state, 1) }, PA_OK);
    let int_id = typeid(state, -1);
    assert_eq!(int_id, typeid_of_named(state, "int"));
    // SAFETY: 收掉栈项。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(unsafe { pa_pushstring(state, b"x".as_ptr().cast(), 1) }, PA_OK);
    let str_id = typeid(state, -1);
    assert_eq!(str_id, typeid_of_named(state, "str"));
    assert_ne!(int_id, str_id, "不同内建类型必须给出不同身份");
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);

    // 普通实例（list）：两个 list 值共享同一类型身份。
    assert_eq!(unsafe { pa_newlist(state, 0) }, PA_OK);
    assert_eq!(unsafe { pa_newlist(state, 0) }, PA_OK);
    assert_eq!(typeid(state, -1), typeid_of_named(state, "list"));
    assert_eq!(typeid(state, -1), typeid(state, -2));
    assert_eq!(unsafe { pa_pop(state, 2) }, PA_OK);

    // 宿主对象：实例身份＝其注册类型身份。
    let signature = empty_signature();
    // SAFETY: 名字/函数/签名都有效；负值 base ⇒ object。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"IdentityProbe\0".as_ptr().cast(),
                0,
                0,
                    Some(noop_dealloc as PaHostDealloc),
                    Some(noop_traverse as PaHostTraverse),
                &signature,)
        },
        PA_OK,
        "{}",
        message_of(state)
    );
    let mut payload: *mut core::ffi::c_void = core::ptr::null_mut();
    // SAFETY: 类型在栈顶（-1）。
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    let registered = typeid_of_named(state, "IdentityProbe");
    assert_eq!(typeid(state, -1), registered, "宿主对象身份＝其注册类型");
    // `pa_type` 仍报值类别（`PA_THANDLE`），两者分工不同。
    assert_eq!(unsafe { pa_pop(state, 2) }, PA_OK);

    // 非法：越界／NULL 出参。
    let mut out: *mut core::ffi::c_void = core::ptr::null_mut();
    assert_eq!(
        unsafe { pa_typeid(state, 999, &mut out) },
        PA_ERR_INVALID
    );
    assert_eq!(
        unsafe { pa_typeid(state, -1, core::ptr::null_mut()) },
        PA_ERR_INVALID
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn newtype_base_index_creates_real_inheritance() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };

    // 宿主异常类继承 `ValueError`（真实 MRO）。基类经 guest 全局别名取得
    // （`ValueError` 在内建命名空间，`pa_getglobal` 只查模块全局）。
    assert_eq!(
        exec(state, b"HostBaseError = ValueError\n"),
        PA_OK,
        "取 ValueError 别名失败：{}",
        message_of(state)
    );
    assert_eq!(
        unsafe { pa_getglobal(state, b"HostBaseError\0".as_ptr().cast()) },
        PA_OK
    );
    let signature = empty_signature();
    // SAFETY: 基类在栈顶（-1，不消耗）；名字/函数/签名都有效。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"HostValueError\0".as_ptr().cast(),
                -1,
                0,
                    Some(noop_dealloc as PaHostDealloc),
                    Some(noop_traverse as PaHostTraverse),
                &signature,)
        },
        PA_OK,
        "{}",
        message_of(state)
    );
    // 直接核 Rust 侧 MRO：宿主类型必须是 `ValueError` 的子类型。
    {
        let instance = unsafe { &*state }.instance();
        let host_ty = instance.type_named("HostValueError").expect("HostValueError 已登记");
        let value_ty = instance.type_named("ValueError").expect("ValueError 已登记");
        assert!(
            instance.is_subtype(host_ty, value_ty),
            "MRO 没有包含 ValueError（is_subtype 假）"
        );
    }

    // 类型在栈顶 ⇒ 绑定到全局名（消费栈顶）。
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(
        unsafe { pa_setglobal(state, b"HostValueError\0".as_ptr().cast()) },
        PA_OK
    );
    // SAFETY: 收掉 `ValueError` 类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);

    // 建一个宿主实例并绑定到全局名。
    // SAFETY: 名字/句柄按契约。
    assert_eq!(
        unsafe { pa_getglobal(state, b"HostValueError\0".as_ptr().cast()) },
        PA_OK
    );
    let mut payload: *mut core::ffi::c_void = core::ptr::null_mut();
    // SAFETY: 类型在栈顶（-1）。
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    // SAFETY: 实例在栈顶 ⇒ 写全局。
    assert_eq!(
        unsafe { pa_setglobal(state, b"host_error_obj\0".as_ptr().cast()) },
        PA_OK
    );
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);

    for (probe, what) in [
        (&b"assert issubclass(HostValueError, ValueError)\n"[..], "issubclass"),
        (b"assert isinstance(host_error_obj, HostValueError)\n", "isinstance-own"),
        (b"assert isinstance(host_error_obj, ValueError)\n", "isinstance-base"),
    ] {
        assert_eq!(exec(state, probe), PA_OK, "{what} 失败：{}", message_of(state));
    }

    // 负值 ⇒ `object`；没有基类时 `issubclass(HostValueError, ValueError)` 为 False 的另一类型。
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"PlainHostType\0".as_ptr().cast(),
                0,
                0,
                    Some(noop_dealloc as PaHostDealloc),
                    Some(noop_traverse as PaHostTraverse),
                &signature,)
        },
        PA_OK
    );
    // SAFETY: 类型在栈顶。
    assert_eq!(
        unsafe { pa_setglobal(state, b"PlainHostType\0".as_ptr().cast()) },
        PA_OK
    );
    let script = b"assert not issubclass(PlainHostType, ValueError)\nassert issubclass(PlainHostType, object)\n";
    assert_eq!(exec(state, script), PA_OK, "负值 base 应当落到 object：{}", message_of(state));

    // 终态类型不可作基类：先注册一个 `PA_TYPE_FINAL` 类型，再用它作基类 ⇒ PA_ERR_INVALID。
    let mut final_signature = empty_signature();
    final_signature.flags = pyawa_abi::host::PA_TYPE_FINAL;
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"FinalHostType\0".as_ptr().cast(),
                0,
                0,
                    Some(noop_dealloc as PaHostDealloc),
                    Some(noop_traverse as PaHostTraverse),
                &final_signature,)
        },
        PA_OK
    );
    // 终态类型在栈顶 ⇒ 用它作基类（`base_index = -1`，不消耗）。
    // SAFETY: 同上。
    let rejected = unsafe {
        pa_newtype(
            state,
            b"DerivedFromFinal\0".as_ptr().cast(),
            -1,
            0,
                    Some(noop_dealloc as PaHostDealloc),
                    Some(noop_traverse as PaHostTraverse),
            &signature,)
    };
    assert_eq!(rejected, PA_ERR_INVALID, "终态基类必须拒绝：{}", message_of(state));
    // SAFETY: 收掉终态类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
