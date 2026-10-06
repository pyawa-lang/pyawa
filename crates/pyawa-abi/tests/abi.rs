//! `pyawa-abi` 已落地部分的性质（判据见 `docs/SPEC-c-abi.md` §13）：
//! 状态码取值（`AB-20`）、版本策略（`AB-39`…`AB-45`）、**有界读取**（`AB-43`）、
//! panic 边界（`AB-3`／`CX-11`，`T-AB-2` 的一半）。

use core::mem::size_of;

use pyawa_abi::status::*;
use pyawa_abi::helpers::{
    paL_checkinteger, paL_checkstring, paL_dofile, paL_dostring, paL_error, paL_execresult,
    paL_getsubtable, paL_len, paL_openlibs, paL_optinteger, paL_optstring, paL_ref,
    paL_requiref, paL_setfuncs, paL_traceback, paL_unref, paL_where,
};
use pyawa_abi::tag::*;
use pyawa_abi::*;

#[test]
fn status_codes_match_the_spec_table() {
    // AB-20：取值稳定，与 SPEC-c-abi.md §15.2 逐项对应
    assert_eq!(PA_OK, 0);
    assert_eq!(PA_ERR_RUNTIME, 1);
    assert_eq!(PA_ERR_SYNTAX, 2);
    assert_eq!(PA_ERR_MEMORY, 3);
    assert_eq!(PA_ERR_INTERRUPT, 4);
    assert_eq!(PA_ERR_NOTIMPLEMENTED, 5);
    assert_eq!(PA_ERR_INVALID, 6);
    assert_eq!(PA_ERR_ABI, 7);
    assert_eq!(PA_ERR_RESERVED_FIRST, 8);
    assert_eq!(PA_ERR_RESERVED_LAST, 31);
    assert!(PA_ERR_RESERVED_FIRST > PA_ERR_ABI, "AB-20：新增必须落在预留区");
}

#[test]
fn version_macros_agree_with_the_exported_functions() {
    assert_eq!(pa_abi_version(), PA_ABI_VERSION);
    assert_eq!(pa_abi_size(), PA_ABI_SIZE);
    assert_eq!(
        PA_ABI_SIZE,
        size_of::<pa_host>(),
        "AB-45：PA_ABI_SIZE 就是宿主结构体字节数"
    );
    assert_eq!(
        PA_ABI_VERSION,
        (PA_ABI_MAJOR << 16) | PA_ABI_MINOR,
        "版本编码：主版本在高 16 位"
    );
    // 版本字符串是 NUL 结尾的静态串
    // SAFETY: pa_version 返回静态内存的指针。
    let text = unsafe { core::ffi::CStr::from_ptr(pa_version()) };
    assert!(
        text.to_str()
            .unwrap()
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
    );
}

#[test]
fn version_compatibility_is_major_only() {
    // AB-41：次版本差异（表尾追加）旧宿主仍可用；主版本不同必须失败
    assert!(version_compatible(PA_ABI_VERSION));
    assert!(version_compatible((PA_ABI_MAJOR << 16) | 7), "次版本高一点仍兼容");
    assert!(!version_compatible(((PA_ABI_MAJOR + 1) << 16) | PA_ABI_MINOR));
    assert!(!version_compatible(0));
}

#[test]
fn mismatch_message_is_diagnosable() {
    // T-AB-4：信息必须可诊断（含两侧版本）
    let mismatch = VersionMismatch {
        host_version: (2 << 16) | 3,
        runtime_version: PA_ABI_VERSION,
    };
    let message = mismatch.message();
    assert!(message.contains("2.3"), "含宿主版本：{message}");
    assert!(message.contains("1.0"), "含运行时版本：{message}");
}

#[test]
fn host_view_never_reads_beyond_the_declared_size() {
    // AB-43：以 min(宿主 size, 自身 size) 为界读取，禁止越界读。
    #[repr(C)]
    struct TinyHost {
        abi_size: usize,
    }
    let tiny = TinyHost {
        abi_size: size_of::<usize>(),
    };
    // SAFETY: 指针指向 TinyHost，可读一个 usize（正是它声明的尺寸）。
    let view = unsafe { view_host((&tiny as *const TinyHost).cast::<pa_host>()) };
    assert_eq!(view.abi_size, Some(size_of::<usize>()));
    assert_eq!(view.abi_version, None, "尺寸不够覆盖版本字段 ⇒ 不许读");
    assert_eq!(view.capabilities, None, "同上");

    // 完整的宿主结构：字段都读得到
    let full = pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    };
    // SAFETY: 指针指向完整的 pa_host。
    let view = unsafe { view_host(&full) };
    assert_eq!(view.abi_version, Some(PA_ABI_VERSION));
    assert!(view.capabilities.is_some());

    // NULL 宿主：全是 None（不 UB）
    // SAFETY: 允许传 NULL。
    let view = unsafe { view_host(core::ptr::null()) };
    assert!(view.abi_size.is_none() && view.abi_version.is_none());
}

#[test]
fn boundary_catches_panics() {
    // AB-3／CX-11：panic 绝不跨边界（T-AB-2 的一半）
    assert_eq!(boundary(|| PA_OK), PA_OK);
    let caught = boundary(|| panic!("宿主函数炸了"));
    assert_eq!(caught, PA_ERR_RUNTIME, "被捕获并转成状态码，进程不崩");
}

// ---- 实例生命周期（`AB-55`／`AB-56`／`AB-57`）----

use core::ffi::{c_char, c_void, CStr};

/// 造一个版本兼容的宿主（`AB-43`：`abi_size` 是自己的尺寸）。
fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

#[test]
fn create_and_destroy_a_compatible_instance() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: host／state 都是本测试的局部变量，按 AB-55 的契约传。
    let status = unsafe { pa_create(&host, &mut state) };
    assert_eq!(status, PA_OK, "兼容的宿主应当成功");
    assert!(!state.is_null(), "AB-55：实例经出参交回");
    // 没有错误信息时 `pa_errmsg` 给 NULL
    // SAFETY: state 由 pa_create 交回且尚未销毁。
    assert!(unsafe { pa_errmsg(state) }.is_null());
    // 中断请求（AB-5①）：兼容实例受理
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_interrupt(state) }, PA_OK);
    // SAFETY: 同上（调用后 state 不可再用）。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn version_mismatch_hands_out_a_diagnostic_instance() {
    // AB-56：ABI 不匹配时**仍交出实例**，诊断信息存在里面（T-AB-4）
    let host = pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: ((PA_ABI_MAJOR + 1) << 16) | 7,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 同上。
    let status = unsafe { pa_create(&host, &mut state) };
    assert_eq!(status, PA_ERR_ABI);
    assert!(!state.is_null(), "AB-56：仍要交出诊断实例");
    // 诊断信息可读，且含两侧版本
    // SAFETY: state 由 pa_create 交回且尚未销毁。
    let message = unsafe { CStr::from_ptr(pa_errmsg(state)) }.to_str().unwrap();
    assert!(message.contains("2.7"), "含宿主版本：{message}");
    assert!(message.contains("1.0"), "含运行时版本：{message}");
    // AB-56：其余调用一律 PA_ERR_ABI
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_interrupt(state) }, PA_ERR_ABI);
    // SAFETY: 同上（宿主必须在 *out != NULL 时销毁）。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn a_host_too_small_is_an_abi_mismatch_without_over_reading() {
    // AB-43：宿主只声明 8 字节 ⇒ 版本字段读不到；如实说"未知"而不是越界读
    #[repr(C)]
    struct TinyHost {
        abi_size: usize,
    }
    let tiny = TinyHost {
        abi_size: size_of::<usize>(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 指针指向 TinyHost，可读它声明的那 8 字节。
    let status = unsafe { pa_create((&tiny as *const TinyHost).cast::<pa_host>(), &mut state) };
    assert_eq!(status, PA_ERR_ABI);
    assert!(!state.is_null());
    // SAFETY: state 由 pa_create 交回且尚未销毁。
    let message = unsafe { CStr::from_ptr(pa_errmsg(state)) }.to_str().unwrap();
    assert!(message.contains("未知"), "不许假装读到了版本：{message}");
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn invalid_uses_are_reported_not_crashed() {
    let host = compatible_host();
    // out 为 NULL ⇒ 宿主用法错误（AB-19）
    // SAFETY: host 合法，out 传 NULL 是宿主用法错误。
    assert_eq!(unsafe { pa_create(&host, core::ptr::null_mut()) }, PA_ERR_INVALID);
    // host 为 NULL ⇒ 同上（不是 ABI 不兼容）
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: out 合法，host 传 NULL。
    assert_eq!(unsafe { pa_create(core::ptr::null(), &mut state) }, PA_ERR_INVALID);
    assert!(state.is_null(), "失败时 *out 必须是 NULL");
    // 销毁 NULL 也是宿主用法错误
    // SAFETY: 传 NULL 是宿主用法错误。
    assert_eq!(unsafe { pa_destroy(core::ptr::null_mut()) }, PA_ERR_INVALID);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_interrupt(core::ptr::null_mut()) }, PA_ERR_INVALID);
}

// ---- 虚拟栈与值转换（`AB-9`…`AB-13`）----

#[test]
fn stack_arithmetic_and_indexing() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按 AB-55 的契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);

    // SAFETY: state 由 pa_create 交回且尚未销毁。
    unsafe {
        assert_eq!(pa_gettop(state), 0, "新实例的栈是空的");
        // 压入：None、True、7、1.5、"文本"
        assert_eq!(pa_pushnil(state), PA_OK);
        assert_eq!(pa_pushboolean(state, 1), PA_OK);
        assert_eq!(pa_pushinteger(state, 7), PA_OK);
        assert_eq!(pa_pushnumber(state, 1.5), PA_OK);
        let text = b"hi\0";
        assert_eq!(pa_pushstring(state, text.as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_gettop(state), 5);

        // AB-9：正索引自底、负索引自顶
        assert_eq!(pa_type(state, 1), PA_TNIL);
        assert_eq!(pa_type(state, 3), PA_TINTEGER);
        assert_eq!(pa_type(state, -1), PA_TSTRING);
        assert_eq!(pa_type(state, 0), PA_ERR_INVALID, "索引 0 非法");
        assert_eq!(pa_type(state, 99), PA_ERR_INVALID, "越界");

        // 判定类
        assert_eq!(pa_isnil(state, 1), 1);
        assert_eq!(pa_isnil(state, 2), 0);
        assert_eq!(pa_isinteger(state, 3), 1);
        assert_eq!(pa_isnumber(state, 4), 1);
        assert_eq!(pa_isstring(state, -1), 1);
        assert_eq!(pa_isfunction(state, -1), 0);

        // 转换
        let mut integer = 0i64;
        assert_eq!(pa_tointeger(state, 3, &mut integer), PA_OK);
        assert_eq!(integer, 7);
        let mut number = 0.0f64;
        assert_eq!(pa_tonumber(state, 4, &mut number), PA_OK);
        assert!((number - 1.5).abs() < f64::EPSILON);
        let mut length = 0usize;
        let view = pa_tostring(state, -1, &mut length);
        assert!(!view.is_null(), "字符串给只读视图（借用）");
        assert_eq!(length, 2);
        assert_eq!(
            core::slice::from_raw_parts(view.cast::<u8>(), length),
            b"hi"
        );
        assert_eq!(pa_toboolean(state, 1), 0, "None 是假");
        assert_eq!(pa_toboolean(state, 2), 1, "True 是真");

        // 复制与弹出
        assert_eq!(pa_pushvalue(state, 3), PA_OK);
        assert_eq!(pa_gettop(state), 6);
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_settop(state, 2), PA_OK);
        assert_eq!(pa_gettop(state), 2);
        assert_eq!(pa_settop(state, 4), PA_OK, "AB-12：补 nil 到指定深度");
        assert_eq!(pa_gettop(state), 4);
        assert_eq!(pa_isnil(state, 3), 1);
        assert_eq!(pa_settop(state, -1), PA_ERR_INVALID, "负深度非法");
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn tables_lists_and_unimplemented_bits() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_newtable(state), PA_OK);
        assert_eq!(pa_type(state, -1), PA_TTABLE);
        assert_eq!(pa_istable(state, -1), 1);
        assert_eq!(pa_newlist(state, 3), PA_OK);
        assert_eq!(pa_type(state, -1), PA_THANDLE, "list 不是 table 标签");
        assert_eq!(pa_gettop(state), 2);

        // 未提供的能力如实报"未实现"（AB-22），不是"已实现但拒绝"
        // ——`AB-62` 之后 `pa_pushbytes` **已落地**（bytes 类型在 `P1-12` 就位）⇒ 这条改验
        //    `pa_exec_file`（仍如实报"未提供"），bytes 的往返由 `bytes_push_and_view_round_trip` 守
        assert_eq!(
            pa_exec_file(state, c"x.py".as_ptr(), c"python".as_ptr(), core::ptr::null()),
            PA_ERR_NOTIMPLEMENTED
        );
        // `AB-58` 之后 `pa_newhandle` 是真的：越界索引 ⇒ 用法错误
        assert_eq!(
            pa_newhandle(state, 99, core::ptr::null_mut()),
            PA_ERR_INVALID
        );
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn borrowed_to_owned_roundtrip() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_pushinteger(state, 1), PA_OK);
        assert_eq!(pa_retain(state, -1), PA_OK, "借用 → 持有");
        assert_eq!(pa_release(state, -1), PA_OK, "持有 → 释放");
        assert_eq!(pa_retain(state, 5), PA_ERR_INVALID);
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_gettop(state), 0);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn diagnostic_instances_refuse_everything_else() {
    // AB-56：诊断实例只有 pa_errmsg／pa_destroy 可用
    let host = pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: (3 << 16) | 1,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_ERR_ABI);
    // SAFETY: 诊断实例（尚未销毁）。
    unsafe {
        assert_eq!(pa_gettop(state), PA_ERR_ABI);
        assert_eq!(pa_pushinteger(state, 1), PA_ERR_ABI);
        assert_eq!(pa_settop(state, 1), PA_ERR_ABI);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// ---- 宿主函数注册与调用（`AB-24`…`AB-26`）----

use pyawa_abi::host::{pa_param, pa_sig, PaHostFn};

/// 宿主函数：把两个整数实参相加，结果留在栈顶（返回值约定见 `host` 模块文档）。
unsafe extern "C-unwind" fn host_add(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回。
    unsafe {
        let mut left = 0i64;
        let mut right = 0i64;
        if pa_tointeger(state, -2, &mut left) != PA_OK {
            return PA_ERR_INVALID;
        }
        if pa_tointeger(state, -1, &mut right) != PA_OK {
            return PA_ERR_INVALID;
        }
        pa_pop(state, 2);
        pa_pushinteger(state, left + right)
    }
}

/// 宿主函数：故意 panic，验证 `AB-26`（被捕获、转状态码、进程不崩）。
unsafe extern "C-unwind" fn host_panics(_state: *mut pa_state) -> i32 {
    panic!("宿主函数炸了");
}

#[test]
fn a_host_function_can_be_registered_and_called() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);

    // 签名：两个位置参数（`AB-25` 要求必须有签名）
    let name0 = b"left\0";
    let name1 = b"right\0";
    let params = [
        pa_param {
            size: size_of::<pa_param>(),
            name: name0.as_ptr().cast(),
            type_expr: core::ptr::null(),
            flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        },
        pa_param {
            size: size_of::<pa_param>(),
            name: name1.as_ptr().cast(),
            type_expr: core::ptr::null(),
            flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        },
    ];
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: params.len(),
        params: params.as_ptr(),
    };
    let registered = b"host_add\0";
    // SAFETY: 按契约传参（name／fn／sig 都有效）。
    assert_eq!(
        unsafe { pa_register(state, registered.as_ptr().cast(), host_add, &signature) },
        PA_OK
    );

    // SAFETY: state 存活。
    unsafe {
        // 取回注册进去的函数（`pa_getglobal`）⇒ 栈顶就是它
        assert_eq!(pa_getglobal(state, registered.as_ptr().cast()), PA_OK);
        assert_eq!(pa_isfunction(state, -1), 1, "注册进去的是可调用对象");
        // 压两个实参再调用：`[…, f, 20, 22]` ⇒ 结果 42
        assert_eq!(pa_pushinteger(state, 20), PA_OK);
        assert_eq!(pa_pushinteger(state, 22), PA_OK);
        assert_eq!(pa_call(state, 2, 1), PA_OK);
        assert_eq!(pa_gettop(state), 1, "−nargs+nresults ⇒ 只剩结果");
        let mut total = 0i64;
        assert_eq!(pa_tointeger(state, -1, &mut total), PA_OK);
        assert_eq!(total, 42);
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn a_panicking_host_function_becomes_a_status_code() {
    // AB-26：宿主函数内部的 panic 必须被捕获并转成状态码
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    let name = b"boom\0";
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe { pa_register(state, name.as_ptr().cast(), host_panics, &signature) },
        PA_OK
    );
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, name.as_ptr().cast()), PA_OK);
        assert_eq!(
            pa_pcall(state, 0, 1),
            PA_ERR_RUNTIME,
            "panic 被捕获 ⇒ 状态码，而不是崩掉"
        );
        assert_eq!(pa_pop(state, 0), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn register_without_a_signature_is_rejected() {
    // AB-25：禁止无名签名的宿主函数
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let name = b"nosig\0";
    let function: PaHostFn = host_add;
    // SAFETY: sig 故意给 NULL。
    assert_eq!(
        unsafe { pa_register(state, name.as_ptr().cast(), function, core::ptr::null()) },
        PA_ERR_INVALID
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// ---- 执行（`§15.3` 的 `pa_exec_*`：`AB-7`／`AB-60`；M1 判据的"执行一段脚本"）----

/// 一份"两位置参数"的签名（`AB-25`：注册必须带签名）。
fn signature_with_two_positional(left: &'static [u8], right: &'static [u8]) -> (pa_sig, [pa_param; 2]) {
    let params = [
        pa_param {
            size: size_of::<pa_param>(),
            name: left.as_ptr().cast(),
            type_expr: core::ptr::null(),
            flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        },
        pa_param {
            size: size_of::<pa_param>(),
            name: right.as_ptr().cast(),
            type_expr: core::ptr::null(),
            flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        },
    ];
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: params.len(),
        params: core::ptr::null(),
    };
    (signature, params)
}

/// 取当前错误信息（`pa_errmsg`，**借用**）成 owned 文本。
fn message_of(state: *mut pa_state) -> Option<String> {
    // SAFETY: state 由调用方保证存活。
    let raw = unsafe { pa_errmsg(state) };
    if raw.is_null() {
        return None;
    }
    // SAFETY: pa_errmsg 交回的是 NUL 结尾的借用串。
    Some(unsafe { core::ffi::CStr::from_ptr(raw) }.to_string_lossy().into_owned())
}

#[test]
fn exec_string_runs_a_script_that_calls_a_host_function() {
    // M1 判据的链路：执行脚本 → 脚本调到 `pa_register` 注入的宿主函数 → 取回值。
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let (mut signature, params) = signature_with_two_positional(b"left\0", b"right\0");
    signature.params = params.as_ptr();
    let registered = b"host_add\0";
    // SAFETY: name／fn／sig 都有效。
    assert_eq!(
        unsafe { pa_register(state, registered.as_ptr().cast(), host_add, &signature) },
        PA_OK
    );

    // 显式给长度（不走 NUL 结尾那条），源码里含一个全局名调用
    let source = b"result = host_add(2, 3)\n";
    let chunk = b"m1-probe\0";
    let mode = b"python\0";
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            chunk.as_ptr().cast(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_OK, "执行应当成功；诊断：{:?}", message_of(state));
    unsafe {
        assert_eq!(pa_gettop(state), 0, "§15.3：pa_exec_string 的栈契约是 `—`");
        assert_eq!(pa_getglobal(state, b"result\0".as_ptr().cast()), PA_OK);
        let mut value = 0i64;
        assert_eq!(pa_tointeger(state, -1, &mut value), PA_OK);
        assert_eq!(value, 5, "脚本调到的宿主函数把 2 与 3 加成 5");
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn exec_string_accepts_a_nul_terminated_source() {
    // `len < 0` ⇒ 按 NUL 结尾算（口径同 `pa_pushstring`）
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let source = b"x = 1 + 1\0";
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            -1,
            core::ptr::null(),
            b"pyawa\0".as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_OK, "诊断：{:?}", message_of(state));
    unsafe {
        assert_eq!(pa_getglobal(state, b"x\0".as_ptr().cast()), PA_OK);
        let mut value = 0i64;
        assert_eq!(pa_tointeger(state, -1, &mut value), PA_OK);
        assert_eq!(value, 2);
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn exec_string_maps_a_parse_failure_to_syntax() {
    // AB-60：源码自己有问题 ⇒ PA_ERR_SYNTAX(2)，与"传错参数"(6) 分开
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let status = unsafe {
        pa_exec_string(
            state,
            b"def (\0".as_ptr().cast(),
            -1,
            core::ptr::null(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_ERR_SYNTAX);
    let message = message_of(state).expect("语法错也要有诊断信息");
    assert!(!message.is_empty(), "诊断信息不能是空串");
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn exec_string_only_accepts_the_two_mode_strings() {
    // AB-60：`"python"`／`"pyawa"` 两个全串；空串／NULL／别名一律 6
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let source = b"ok = 1\0";
    unsafe {
        for bad in [
            core::ptr::null(),
            b"\0".as_ptr().cast::<core::ffi::c_char>(),
            b"py\0".as_ptr().cast(),
            b"Python\0".as_ptr().cast(),
            b".py\0".as_ptr().cast(),
            b".pyawa\0".as_ptr().cast(),
            b" python\0".as_ptr().cast(),
        ] {
            assert_eq!(
                pa_exec_string(state, source.as_ptr().cast(), -1, core::ptr::null(), bad, core::ptr::null()),
                PA_ERR_INVALID,
                "AB-60：只有两个全串合法"
            );
        }
        // 两个合法值都能跑
        for good in [b"python\0".as_ptr().cast::<core::ffi::c_char>(), b"pyawa\0".as_ptr().cast()] {
            assert_eq!(
                pa_exec_string(state, source.as_ptr().cast(), -1, core::ptr::null(), good, core::ptr::null()),
                PA_OK,
                "诊断：{:?}",
                message_of(state)
            );
        }
        // 源码指针不合法（NULL 配正长度）⇒ 宿主用法错误
        assert_eq!(
            pa_exec_string(state, core::ptr::null(), 4, core::ptr::null(), b"python\0".as_ptr().cast(), core::ptr::null()),
            PA_ERR_INVALID
        );
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn exec_string_reports_a_script_exception_as_runtime() {
    // AB-21：脚本异常转成状态码 ＋ 可诊断信息（不跨边界逃逸）
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let status = unsafe {
        pa_exec_string(
            state,
            b"x = never_defined_name\0".as_ptr().cast(),
            -1,
            core::ptr::null(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_ERR_RUNTIME);
    let message = message_of(state).expect("异常要有诊断信息");
    assert!(
        message.contains("NameError"),
        "诊断信息要带上异常类型，实际：{message}"
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// 造一份 `pa_options`（`AB-61` 的尺寸标记结构）。
fn options(check_tier: u32, optimization: u32) -> pa_options {
    pa_options {
        size: size_of::<pa_options>(),
        check_tier,
        optimization,
    }
}

#[test]
fn options_carry_the_check_tier_across_the_abi() {
    // AB-61：档位由宿主表态 ⇒ 深层要按 `BC-25`② 发边界检查，浅层（NULL 的默认）不发
    let host = compatible_host();
    let source = b"def f(x: int) -> int:\n    return x\n";

    // ① 深层：`f("hello")` 抛 `TypeBoundaryError`（`TS-12`）
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let deep = options(1, 0);
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            b"pyawa\0".as_ptr().cast(),
            &deep,
        )
    };
    assert_eq!(status, PA_OK, "诊断：{:?}", message_of(state));
    unsafe {
        assert_eq!(pa_getglobal(state, b"f\0".as_ptr().cast()), PA_OK);
        assert_eq!(pa_pushstring(state, b"hello\0".as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_call(state, 1, 1), PA_ERR_RUNTIME, "深层档位要归责");
        let message = message_of(state).expect("要有诊断信息");
        assert!(
            message.contains("TypeBoundaryError"),
            "归责异常的类型名要出现，实际：{message}"
        );
        // 失败调用已经把「可调用 ＋ 实参」那段收掉了（`pa_call` 的栈契约）⇒ 栈是空的
        assert_eq!(pa_gettop(state), 0, "失败的 pa_call 也要收干净");
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);

    // ② 浅层（`NULL` ＝ `AB-61` 的默认）：同一份源码、同一模式，检查不发、调用照常成功
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            b"pyawa\0".as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_OK, "诊断：{:?}", message_of(state));
    unsafe {
        assert_eq!(pa_getglobal(state, b"f\0".as_ptr().cast()), PA_OK);
        assert_eq!(pa_pushstring(state, b"hello\0".as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_call(state, 1, 1), PA_OK, "浅层不做边界检查");
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn options_reject_bad_sizes_tiers_and_levels() {
    // AB-61 ＋ AB-43 的惯例：尺寸标记有界读，非法值一律 6（**禁止**静默降级）
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let source = b"ok = 1\0";
    let mode = b"python\0";
    unsafe {
        // `size` 只盖到 `size` 字段本身 ⇒ 档位／优化级都没读到
        let too_small = pa_options {
            size: size_of::<usize>(),
            check_tier: 0,
            optimization: 0,
        };
        assert_eq!(
            pa_exec_string(
                state,
                source.as_ptr().cast(),
                -1,
                core::ptr::null(),
                mode.as_ptr().cast(),
                &too_small
            ),
            PA_ERR_INVALID,
            "尺寸盖不住字段 ⇒ 6"
        );
        // `check_tier` 只有 0／1
        let bad_tier = options(2, 0);
        assert_eq!(
            pa_exec_string(
                state,
                source.as_ptr().cast(),
                -1,
                core::ptr::null(),
                mode.as_ptr().cast(),
                &bad_tier
            ),
            PA_ERR_INVALID,
            "非法档位 ⇒ 6"
        );
        // 优化级超出 `u8`
        let bad_level = options(0, 300);
        assert_eq!(
            pa_exec_string(
                state,
                source.as_ptr().cast(),
                -1,
                core::ptr::null(),
                mode.as_ptr().cast(),
                &bad_level
            ),
            PA_ERR_INVALID,
            "优化级超出 u8 ⇒ 6"
        );
        // 合法组合（两种档位 × 几个优化级）都能过——优化级目前不改发射，但不拦
        for (tier, level) in [(0u32, 0u32), (1, 2), (0, 255)] {
            let good = options(tier, level);
            assert_eq!(
                pa_exec_string(
                    state,
                    source.as_ptr().cast(),
                    -1,
                    core::ptr::null(),
                    mode.as_ptr().cast(),
                    &good
                ),
                PA_OK,
                "档位 {tier}／优化级 {level} 应当收下；诊断：{:?}",
                message_of(state)
            );
        }
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn exec_string_binds_the_script_name_but_respects_a_host_binding() {
    // 脚本语义：`__name__` 未绑定时补 `"__main__"`（`python3 -c`／脚本同款）。类体序言要读它，
    // 缺了就 `NameError`——这是 M2 对拍 harness（`tests/conformance.rs`）抓到的第一处可观察缺口。
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let class = b"class C:\n    v = 5\n";
    let mode = b"python\0";
    let status = unsafe {
        pa_exec_string(
            state,
            class.as_ptr().cast(),
            class.len() as isize,
            core::ptr::null(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_OK, "类体要能跑；诊断：{:?}", message_of(state));
    let name_of = |state: *mut pa_state| -> String {
        let mut length = 0usize;
        // SAFETY: 本测试自己压栈、自己读。
        let pointer = unsafe { pa_tostring(state, -1, &mut length) };
        assert!(!pointer.is_null(), "栈顶应当是 str");
        // SAFETY: pa_tostring 交回 length 字节的借用视图。
        let bytes = unsafe { core::slice::from_raw_parts(pointer.cast::<u8>(), length) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, b"__name__\0".as_ptr().cast()), PA_OK);
        assert_eq!(name_of(state), "__main__", "没绑过就补脚本名");
        assert_eq!(pa_pop(state, 1), PA_OK);
        // 宿主自己绑过 ⇒ 不覆盖
        assert_eq!(pa_pushstring(state, b"mymod\0".as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_setglobal(state, b"__name__\0".as_ptr().cast()), PA_OK);
        let status = pa_exec_string(
            state,
            class.as_ptr().cast(),
            class.len() as isize,
            core::ptr::null(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        );
        assert_eq!(status, PA_OK, "诊断：{:?}", message_of(state));
        assert_eq!(pa_getglobal(state, b"__name__\0".as_ptr().cast()), PA_OK);
        assert_eq!(name_of(state), "mymod", "宿主绑过的名字不许被顶掉");
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn exec_file_reports_not_provided_while_bytecode_rejects_bad_input() {
    // AB-22："未提供"（5）与"已实现但拒绝"必须可区分
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    unsafe {
        assert_eq!(
            pa_exec_file(state, b"/tmp/x.py\0".as_ptr().cast(), b"python\0".as_ptr().cast(), core::ptr::null()),
            PA_ERR_NOTIMPLEMENTED
        );
        // **第 404 轮**：`pa_exec_bytecode` 已接线 ⇒ 这一格改为"已实现但拒绝"（`AB-22` 的区分 ✓）。
        // 正常产物 / 陈旧版本两格见 `exec_bytecode_runs_a_product_and_rejects_stale_or_malformed_input` ✓。
        assert_eq!(pa_exec_bytecode(state, core::ptr::null(), 0), PA_ERR_INVALID);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// ---- 宿主类型注册（`AB-35`…`AB-38`）----

use pyawa_abi::host::{PaHostDealloc, PaHostTraverse};

static HOST_DEALLOCS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static HOST_TRAVERSES: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
/// 宿主 `dealloc` 收到的载荷地址（`AB-58`：确认它指向 VM 分配的那块）
static HOST_DEALLOC_PAYLOAD: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// 宿主的 `dealloc`：把自己那份不透明载荷放掉（这里只是记一笔）。
unsafe extern "C" fn host_dealloc(payload: *mut c_void) {
    HOST_DEALLOCS.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    HOST_DEALLOC_PAYLOAD.store(payload as usize, core::sync::atomic::Ordering::SeqCst);
    // **`AB-58`**：载荷**存储**归 VM（随实例回收）⇒ 这里**禁止** `free`／`realloc`。
    // 宿主的 `dealloc` 只放掉它**塞在载荷里面**的东西（本例的载荷就是个 `u64`，没有内部资源）。
    let _ = payload;
}

/// **不共享计数**的一对回调：给"同时用 `host_dealloc` 会与计数断言互扰"的用例用。
///
/// 由来：`HOST_DEALLOCS`／`HOST_DEALLOC_PAYLOAD` 是**进程级静态**，多个用例并行跑时
/// 谁调用了 `host_dealloc` 都会改它 ⇒ 断言精确计数的用例会偶发变红（`MS-25` 抓到的正是这一类）。
unsafe extern "C" fn quiet_dealloc(_payload: *mut c_void) {}

/// 见 [`quiet_dealloc`]。
unsafe extern "C" fn quiet_traverse(
    _payload: *mut c_void,
    _context: *mut c_void,
    _visit: unsafe extern "C" fn(*mut c_void, *mut c_void),
) {
}

/// 宿主的 `traverse`：不持有脚本对象引用（本例没有）。
unsafe extern "C" fn host_traverse(
    _payload: *mut c_void,
    _context: *mut c_void,
    _visit: unsafe extern "C" fn(*mut c_void, *mut c_void),
) {
    HOST_TRAVERSES.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
}

#[test]
fn a_host_type_is_registered_as_a_real_type() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);

    let name = b"Widget\0";
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0, // 没设 PA_TYPE_FINAL ⇒ 默认可被继承（AB-37）
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    let dealloc: PaHostDealloc = host_dealloc;
    let traverse: PaHostTraverse = host_traverse;
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                name.as_ptr().cast(),
                size_of::<u64>(),
                dealloc,
                traverse,
                &signature,
            )
        },
        PA_OK
    );
    // `AB-58`：注册后类型进模块全局 ⇒ 宿主按名字取回它（`pa_newhandle` 的 `type` 参数）

    // 签名缺失即拒绝（AB-36）
    // SAFETY: sig 故意给 NULL。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"Widget2\0".as_ptr().cast(),
                size_of::<u64>(),
                dealloc,
                traverse,
                core::ptr::null(),
            )
        },
        PA_ERR_INVALID
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn host_type_accepts_the_final_flag() {
    // `AB-37`：`PA_TYPE_FINAL` 是"反向选择不可继承"的保留位；本层先接受它（执行随后补）
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: pyawa_abi::host::PA_TYPE_FINAL,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"Gadget\0".as_ptr().cast(),
                size_of::<u64>(),
                host_dealloc,
                host_traverse,
                &signature,
            )
        },
        PA_OK
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);

}

// ---- 属性与下标（`pa_getfield`…`pa_rawset`）----

#[test]
fn tables_support_field_and_subscript_access() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let key_text = b"answer\0";
    // SAFETY: state 存活。
    unsafe {
        // 表的下标读写：栈是 `[容器, 键, 值]`
        assert_eq!(pa_newtable(state), PA_OK);
        assert_eq!(pa_pushstring(state, key_text.as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_pushinteger(state, 42), PA_OK);
        assert_eq!(pa_settable(state, -3), PA_OK, "键与值都被消耗 ⇒ 净 −2");
        assert_eq!(pa_gettop(state), 1, "只剩表");

        // 读回来：键在 TOS、容器在 idx
        assert_eq!(pa_pushstring(state, key_text.as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_gettable(state, -2), PA_OK, "键被消耗、值就地放上");
        let mut value = 0i64;
        assert_eq!(pa_tointeger(state, -1, &mut value), PA_OK);
        assert_eq!(value, 42);

        // `pa_rawget`：同样的键，走不触发协议的直查
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_newtable(state), PA_OK);
        assert_eq!(pa_pushstring(state, key_text.as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_pushinteger(state, 9), PA_OK);
        assert_eq!(pa_settable(state, -3), PA_OK);
        assert_eq!(pa_pushstring(state, key_text.as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_rawget(state, -2), PA_OK);
        assert_eq!(pa_tointeger(state, -1, &mut value), PA_OK);
        assert_eq!(value, 9, "rawget 直查字典内部表");
        // 缺键 ⇒ nil
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_pushstring(state, b"missing\0".as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_rawget(state, -2), PA_OK);
        assert_eq!(pa_isnil(state, -1), 1);
        assert_eq!(pa_pop(state, 2), PA_OK);

        // `pa_setfield` 对**表**是错的用法：表没有实例字典 ⇒ 走属性通道报 AttributeError
        // （与参照实现一致：`{}.answer = 42` 也是 AttributeError）
        assert_eq!(pa_newtable(state), PA_OK);
        assert_eq!(pa_pushinteger(state, 1), PA_OK);
        assert_eq!(
            pa_setfield(state, -2, key_text.as_ptr().cast()),
            PA_ERR_RUNTIME,
            "属性写入走 OM-11 的 setattr；dict 没有实例字典 ⇒ AttributeError"
        );
        let message = CStr::from_ptr(pa_errmsg(state)).to_str().unwrap();
        assert!(
            message.contains("no __dict__ for setting new attributes"),
            "实测消息：{message}"
        );
        // 值被消耗（无论成败）：失败路径也弹掉 TOS，错误经状态码 ＋ `pa_errmsg` 报告
        assert_eq!(pa_gettop(state), 1, "只剩表");
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn rawget_is_reported_for_non_tables() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_pushinteger(state, 1), PA_OK);
        assert_eq!(pa_pushinteger(state, 0), PA_OK);
        assert_eq!(
            pa_rawget(state, -2),
            PA_ERR_INVALID,
            "rawget 只认 dict（本层），别的容器如实报用法错误"
        );
        assert_eq!(pa_pop(state, 2), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// ---- 能力接口注册（`AB-32`…`AB-34`、`T-AB-6`）----

use pyawa_abi::capability::*;

#[test]
fn a_capability_domain_needs_its_async_classification_first() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let fake_vtable = 0x1234usize as *const c_void;
    // SAFETY: state 存活。
    unsafe {
        // AB-34／CP-25／T-AB-6：没声明异步分类就注册 ⇒ 必须失败
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_FS, fake_vtable),
            PA_ERR_INVALID,
            "缺失异步分类即注册失败，禁止落默认值"
        );
        // 声明之后再注册才行
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK),
            PA_OK
        );
        assert_eq!(pa_setcapability(state, PA_DOMAIN_FS, fake_vtable), PA_OK);

        // 分类只有二值（CP-37）
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_NET, 42),
            PA_ERR_INVALID,
            "分类取值只有 PA_ASYNC_OK／PA_ASYNC_NO"
        );
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_IPC, PA_ASYNC_NO),
            PA_OK,
            "ipc 不可异步化"
        );

        // CP-2：null vtable ＝ 整域未实现（允许注册，调用时才报"未实现"）
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_IPC, core::ptr::null()),
            PA_OK
        );

        // 域编号越界
        assert_eq!(
            pa_setcapability_async(state, DOMAIN_COUNT as i32, PA_ASYNC_OK),
            PA_ERR_INVALID
        );
        assert_eq!(
            pa_setcapability(state, -1, fake_vtable),
            PA_ERR_INVALID
        );
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// ---- 辅助层 paL_（§15.4）----

use pyawa_abi::helpers::pa_reg;

#[test]
fn helpers_do_not_invent_new_semantics() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 存活。
    unsafe {
        // checkinteger / optinteger
        assert_eq!(pa_pushinteger(state, 5), PA_OK);
        let mut number = 0i64;
        assert_eq!(paL_checkinteger(state, -1, &mut number), PA_OK);
        assert_eq!(number, 5);
        assert_eq!(pa_pop(state, 1), PA_OK, "弹掉刚检查过的整数");
        assert_eq!(pa_pushstring(state, b"x\0".as_ptr().cast(), -1), PA_OK);
        assert_eq!(
            paL_checkinteger(state, -1, &mut number),
            PA_ERR_RUNTIME,
            "类型不符必须抛错（信息进 pa_errmsg）"
        );
        assert_eq!(pa_pop(state, 1), PA_OK, "先弹掉那个字符串");
        assert_eq!(pa_pushnil(state), PA_OK);
        assert_eq!(paL_optinteger(state, -1, 7, &mut number), PA_OK, "缺省给默认值");
        assert_eq!(number, 7);
        assert_eq!(pa_pop(state, 1), PA_OK);

        // checkstring / optstring
        assert_eq!(pa_pushstring(state, b"hello\0".as_ptr().cast(), -1), PA_OK);
        let mut view: *const c_char = core::ptr::null();
        let mut length = 0usize;
        assert_eq!(paL_checkstring(state, -1, &mut view, &mut length), PA_OK);
        assert_eq!(length, 5);
        assert_eq!(
            core::slice::from_raw_parts(view.cast::<u8>(), length),
            b"hello"
        );
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_pushnil(state), PA_OK);
        assert_eq!(
            paL_optstring(state, -1, b"fallback\0".as_ptr().cast(), &mut view, &mut length),
            PA_OK
        );
        assert_eq!(length, 8);
        assert_eq!(pa_pop(state, 1), PA_OK);

        // len：字符串按字节、表按条目
        assert_eq!(pa_pushstring(state, b"hello\0".as_ptr().cast(), -1), PA_OK);
        assert_eq!(paL_len(state, -1, &mut length), PA_OK);
        assert_eq!(length, 5);
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_newtable(state), PA_OK);
        assert_eq!(pa_pushstring(state, b"k\0".as_ptr().cast(), -1), PA_OK);
        assert_eq!(pa_pushinteger(state, 1), PA_OK);
        assert_eq!(pa_settable(state, -3), PA_OK);
        assert_eq!(paL_len(state, -1, &mut length), PA_OK);
        assert_eq!(length, 1);
        assert_eq!(pa_pop(state, 1), PA_OK);

        // getsubtable：取或建
        assert_eq!(pa_newtable(state), PA_OK);
        assert_eq!(
            paL_getsubtable(state, -1, b"sub\0".as_ptr().cast()),
            PA_OK
        );
        assert_eq!(pa_gettop(state), 2, "子表被压栈（+1）");
        assert_eq!(pa_istable(state, -1), 1);
        // 再取一次应当拿到同一张表（不新建）
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(
            paL_getsubtable(state, -1, b"sub\0".as_ptr().cast()),
            PA_OK
        );
        assert_eq!(pa_istable(state, -1), 1);
        assert_eq!(pa_pop(state, 2), PA_OK);

        // ref／unref
        assert_eq!(pa_pushinteger(state, 42), PA_OK);
        let mut reference = 0i32;
        assert_eq!(paL_ref(state, -1, &mut reference), PA_OK);
        assert!(reference > 0);
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(paL_unref(state, reference), PA_OK);
        assert_eq!(paL_unref(state, reference), PA_OK, "同键再释放是幂等的空槽");
        assert_eq!(paL_unref(state, 999), PA_ERR_INVALID);

        // traceback 只并入信息（不伪造 traceback 结构）；error 抛错
        assert_eq!(paL_error(state, b"boom\0".as_ptr().cast()), PA_ERR_RUNTIME);
        assert_eq!(
            paL_traceback(state, b"context\0".as_ptr().cast()),
            PA_OK
        );
        let text = CStr::from_ptr(pa_errmsg(state)).to_str().unwrap();
        assert!(text.contains("boom") && text.contains("context"), "实际：{text}");

        // execresult：PA_OK 压 True；别的原样返回
        assert_eq!(paL_execresult(state, PA_ERR_RUNTIME), PA_ERR_RUNTIME);
        assert_eq!(paL_execresult(state, PA_OK), PA_OK);
        assert_eq!(pa_isboolean(state, -1), 1);
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn helpers_report_unimplemented_parts_honestly() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let mut view: *const c_char = core::ptr::null();
    let mut length = 0usize;
    // SAFETY: state 存活。
    unsafe {
        // 这几条各自缺一块前置：标准库／编译器／traceback／模块系统
        assert_eq!(paL_openlibs(state), PA_ERR_NOTIMPLEMENTED);
        assert_eq!(
            paL_dostring(state, b"1+1\0".as_ptr().cast(), 0),
            PA_ERR_NOTIMPLEMENTED
        );
        assert_eq!(
            paL_dofile(state, b"/tmp/x.py\0".as_ptr().cast(), 0),
            PA_ERR_NOTIMPLEMENTED
        );
        assert_eq!(
            paL_where(state, 0, &mut view, &mut length),
            PA_ERR_NOTIMPLEMENTED
        );
        assert_eq!(
            paL_requiref(state, b"m\0".as_ptr().cast(), core::ptr::null(), 0),
            PA_ERR_NOTIMPLEMENTED
        );
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn setfuncs_registers_a_batch() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    let name_a = b"alpha\0";
    let name_b = b"beta\0";
    let regs = [
        pa_reg {
            name: name_a.as_ptr().cast(),
            function: host_add,
            sig: &signature,
        },
        pa_reg {
            name: name_b.as_ptr().cast(),
            function: host_add,
            sig: &signature,
        },
        // n < 0 时以 NULL 名字结尾
        pa_reg {
            name: core::ptr::null(),
            function: host_add,
            sig: core::ptr::null(),
        },
    ];
    // SAFETY: 按契约传参（regs 以 NULL 名字结尾）。
    assert_eq!(unsafe { paL_setfuncs(state, regs.as_ptr(), -1) }, PA_OK);
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, name_a.as_ptr().cast()), PA_OK);
        assert_eq!(pa_isfunction(state, -1), 1);
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_getglobal(state, name_b.as_ptr().cast()), PA_OK);
        assert_eq!(pa_isfunction(state, -1), 1);
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// ---- `.pyi` 导出（`AB-53`／`AB-54`）----

#[test]
fn the_pyi_export_is_a_projection_of_the_registrations() {
    use pyawa_abi::export;
    use pyawa_abi::host::param_flags;

    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);

    // 一个带注解、默认值、仅关键字参数与返回注解的宿主函数
    let left_name = b"left\0";
    let right_name = b"right\0";
    let params = [
        pa_param {
            size: size_of::<pa_param>(),
            name: left_name.as_ptr().cast(),
            type_expr: b"int\0".as_ptr().cast(),
            flags: param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        },
        pa_param {
            size: size_of::<pa_param>(),
            name: right_name.as_ptr().cast(),
            type_expr: core::ptr::null(),
            flags: param_flags::PA_PARAM_KEYWORD_ONLY | param_flags::PA_PARAM_HAS_DEFAULT,
            default_handle: core::ptr::null_mut(),
        },
    ];
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: b"int\0".as_ptr().cast(),
        nparams: params.len(),
        params: params.as_ptr(),
    };
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe {
            pa_register(
                state,
                b"combine\0".as_ptr().cast(),
                host_add,
                &signature,
            )
        },
        PA_OK
    );
    // 再注册一个不可继承的宿主类型
    let type_signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: pyawa_abi::host::PA_TYPE_FINAL,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    let dealloc: PaHostDealloc = host_dealloc;
    let traverse: PaHostTraverse = host_traverse;
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"Gadget\0".as_ptr().cast(),
                size_of::<u64>(),
                dealloc,
                traverse,
                &type_signature,
            )
        },
        PA_OK
    );

    // SAFETY: state 存活。
    let text = export::pyi(state);
    assert!(
        text.contains("def combine(left: int, *, right = ...) -> int: ..."),
        "函数的导出：{text}"
    );
    assert!(
        text.contains("class Gadget(object):  # PA_TYPE_FINAL"),
        "类型的导出：{text}"
    );
    assert!(text.contains("__dict__: dict[str, object]"), "实例字典声明：{text}");
    // `AB-54`：导出是**同一份数据**的投影 ⇒ 名字与运行期注册的一模一样
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, b"combine\0".as_ptr().cast()), PA_OK);
        assert_eq!(pa_isfunction(state, -1), 1);
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}


// ---- `AB-58`：载荷由 VM 分配、归 VM 所有，宿主只填 ----

#[test]
fn newhandle_hands_out_a_vm_allocated_payload() {
    use core::sync::atomic::Ordering;

    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    let type_name = b"Widget\0";
    let dealloc: PaHostDealloc = host_dealloc;
    let traverse: PaHostTraverse = host_traverse;
    HOST_DEALLOCS.store(0, Ordering::SeqCst);
    HOST_DEALLOC_PAYLOAD.store(0, Ordering::SeqCst);

    // 注册：载荷 8 字节（AB-58）
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                type_name.as_ptr().cast(),
                size_of::<u64>(),
                dealloc,
                traverse,
                &signature,
            )
        },
        PA_OK
    );

    // SAFETY: state 存活。
    unsafe {
        // **`AB-59`**：注册成功后类型**已经在栈顶**（不透明句柄）——不必再自己 `pa_getglobal`
        assert_eq!(pa_gettop(state), 1, "AB-59：`pa_newtype` 的栈契约是 `+1`");
        let mut payload: *mut c_void = core::ptr::null_mut();
        // 该槽**不消耗**（`AB-11`）：按栈索引取类型，函数只再压入新实例
        assert_eq!(pa_newhandle(state, -1, &mut payload), PA_OK);
        assert!(!payload.is_null(), "AB-58：载荷由 VM 分配并经出参交回");
        assert_eq!(pa_gettop(state), 2, "`+1`：类型仍在栈上，新对象在它上面");
        // 宿主**填**载荷（对象对脚本可见之前）；这里写一个可辨认的值
        *payload.cast::<u64>() = 0x5152_5354_5556_5758;
        assert_eq!(*payload.cast::<u64>(), 0x5152_5354_5556_5758);
        let payload_address = payload as usize;

        // 弹出并释放对象 ⇒ 宿主的 `dealloc` 收到**同一个**载荷地址（存储仍由 VM 释放）
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_pop(state, 1), PA_OK);
        assert_eq!(pa_gettop(state), 0);
        assert_eq!(
            HOST_DEALLOCS.load(Ordering::SeqCst),
            1,
            "宿主 dealloc 应当在对象释放时被调用"
        );
        assert_eq!(
            HOST_DEALLOC_PAYLOAD.load(Ordering::SeqCst),
            payload_address,
            "宿主 dealloc 收到的正是那块 VM 分配的载荷"
        );
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn one_registration_builds_many_instances() {
    // `AB-59` 的收益：**压一次类型、建多个实例**——`pa_newhandle` 的类型槽**不消耗**。
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    let name = b"Twice\0";
    // 用 **quiet** 的那对回调：本用例不关心 dealloc 次数，不该去碰进程级计数（避免并行互扰）
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                name.as_ptr().cast(),
                size_of::<u64>(),
                quiet_dealloc,
                quiet_traverse,
                &signature,
            )
        },
        PA_OK
    );
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_gettop(state), 1, "AB-59：类型在栈顶");
        let mut first: *mut c_void = core::ptr::null_mut();
        let mut second: *mut c_void = core::ptr::null_mut();
        // 第一次：类型在 `-1`
        assert_eq!(pa_newhandle(state, -1, &mut first), PA_OK);
        // 第二次：类型**还在**，现在位于 `-2`（实例压在它上面）
        assert_eq!(pa_gettop(state), 2, "类型没被消耗");
        assert_eq!(pa_newhandle(state, -2, &mut second), PA_OK);
        assert_eq!(pa_gettop(state), 3, "类型仍在，两个实例都在");
        assert!(!first.is_null() && !second.is_null(), "两份载荷都由 VM 分配");
        assert_ne!(first, second, "两个实例的载荷不是同一块");
        // 收尾：三个槽全弹掉（栈要干净）
        assert_eq!(pa_pop(state, 3), PA_OK);
        assert_eq!(pa_gettop(state), 0);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn a_zero_size_payload_gives_null() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    let dealloc: PaHostDealloc = host_dealloc;
    let traverse: PaHostTraverse = host_traverse;
    let name = b"Empty\0";
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe { pa_newtype(state, name.as_ptr().cast(), 0, dealloc, traverse, &signature) },
        PA_OK
    );
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, name.as_ptr().cast()), PA_OK);
        let mut payload: *mut c_void = 1usize as *mut c_void;
        assert_eq!(pa_newhandle(state, -1, &mut payload), PA_OK);
        assert!(payload.is_null(), "AB-58：payload_size == 0 ⇒ 出参为 NULL");
        assert_eq!(pa_pop(state, 2), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn newhandle_rejects_a_non_host_type() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 存活。
    unsafe {
        // 压一个**不是宿主类型**的值（整数）⇒ 用法错误
        assert_eq!(pa_pushinteger(state, 1), PA_OK);
        let mut payload: *mut c_void = core::ptr::null_mut();
        assert_eq!(pa_newhandle(state, -1, &mut payload), PA_ERR_INVALID);
        // 越界索引同样是用法错误
        assert_eq!(pa_newhandle(state, 99, &mut payload), PA_ERR_INVALID);
        assert_eq!(pa_newhandle(state, 0, &mut payload), PA_ERR_INVALID);
        assert_eq!(pa_pop(state, 1), PA_OK);
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// --------------------------------------------------------------------------- #
// `AB-62`：整数的十进制桥（`pa_tointstring`／`pa_pushintstring`）＋ `bytes` 两条（已有函数落地）
// --------------------------------------------------------------------------- #

/// 造一个实例；测试用完就销毁。
fn fresh_state() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: host／state 都是局部变量，按 AB-55 的契约传。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    state
}

/// 读 `pa_errmsg`（借用；立即转成 owned 文本）。
fn error_message(state: *mut pa_state) -> String {
    // SAFETY: state 有效。
    let pointer = unsafe { pa_errmsg(state) };
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: pa_errmsg 给 NUL 结尾的借用视图。
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

/// 把一个十进制串压成整数，再走桥取回十进制文本。
fn round_trip_int_string(state: *mut pa_state, text: &[u8]) -> Result<String, i32> {
    let push = unsafe { pa_pushintstring(state, text.as_ptr().cast(), text.len() as isize) };
    if push != PA_OK {
        return Err(push);
    }
    // 压进来了：用 `pa_tointstring` 取回（**覆盖全部整数**，故 i64 内也走它）
    let mut length = 0usize;
    let view = unsafe { pa_tointstring(state, -1, &mut length) };
    if view.is_null() {
        return Err(PA_ERR_INVALID);
    }
    // SAFETY: 桥给 len 字节的借用视图。
    let observed = unsafe { core::slice::from_raw_parts(view.cast::<u8>(), length) }.to_vec();
    // 取完就弹掉（栈契约 +1）
    let _ = unsafe { pa_pop(state, 1) };
    Ok(String::from_utf8(observed).expect("十进制是 ASCII"))
}

#[test]
fn the_integer_string_bridge_covers_all_integers() {
    let state = fresh_state();
    for (input, expected) in [
        (&b"0"[..], "0"),
        (b"7", "7"),
        (b"-7", "-7"),
        (b"+42", "42"),
        (b"  42  ", "42"),
        (b"1_000", "1000"),
        (b"9223372036854775807", "9223372036854775807"),
        // 越 `i64`：`pa_tointeger` 处理不了，桥**必须**覆盖
        (b"1267650600228229401496703205376", "1267650600228229401496703205376"),
        (b"-1267650600228229401496703205376", "-1267650600228229401496703205376"),
    ] {
        let observed = round_trip_int_string(state, input)
            .unwrap_or_else(|status| panic!("`{}` 应当成功，得到状态 {status}", String::from_utf8_lossy(input)));
        assert_eq!(observed, expected, "`{}` 的十进制往返", String::from_utf8_lossy(input));
    }
    // 大整数走 `pa_tointeger` **必须如实失败**（禁止截断）
    // `len == -1` 要求 **NUL 结尾**（这里必须写 `\0`，否则会读到字面量之外——第一版测试就是这么错的）
    assert_eq!(unsafe { pa_pushintstring(state, b"1267650600228229401496703205376\0".as_ptr().cast(), -1) }, PA_OK);
    let mut out = 0i64;
    assert_eq!(
        unsafe { pa_tointeger(state, -1, &mut out) },
        PA_ERR_NOTIMPLEMENTED,
        "越 i64 ⇒ 如实失败（AB-62 的分工）"
    );
    assert!(error_message(state).contains("i64"), "诊断要说清是 i64 的边界：{}", error_message(state));
    // 桥仍然给得出
    let mut length = 0usize;
    let view = unsafe { pa_tointstring(state, -1, &mut length) };
    assert!(!view.is_null(), "桥覆盖全部整数");
    assert_eq!(length, 31);
    // `len < 0` ⇒ NUL 结尾（口径同 `pa_pushstring`）
    assert_eq!(unsafe { pa_pushintstring(state, b"123\0".as_ptr().cast(), -1) }, PA_OK);
    assert_eq!(unsafe { pa_pop(state, 2) }, PA_OK);
    // SAFETY: state 由 pa_create 交回且尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn the_integer_string_bridge_reports_failures_like_the_spec() {
    let state = fresh_state();
    // 解析失败 ⇒ `PA_ERR_INVALID`
    for bad in [&b"abc"[..], b"12.5", b"", b"0x10", b"12 34"] {
        assert_eq!(
            unsafe { pa_pushintstring(state, bad.as_ptr().cast(), bad.len() as isize) },
            PA_ERR_INVALID,
            "`{}` 应当报 PA_ERR_INVALID",
            String::from_utf8_lossy(bad)
        );
    }
    // 位数超上限（默认 4300）⇒ `AB-62` 的"⇒ ValueError"⇒ 本 ABI 走 `PA_ERR_RUNTIME`，消息照实测
    let huge = "1".repeat(4301);
    assert_eq!(
        unsafe { pa_pushintstring(state, huge.as_ptr().cast(), huge.len() as isize) },
        PA_ERR_RUNTIME
    );
    let message = error_message(state);
    assert!(
        message.contains("Exceeds the limit (4300 digits)"),
        "超限消息照 `TS-45` 实测：{message}"
    );
    // `NULL`＋正长度 ⇒ INVALID（与 `pa_pushstring` 同口径）
    assert_eq!(unsafe { pa_pushintstring(state, core::ptr::null(), 3) }, PA_ERR_INVALID);
    // 非整数取不出：`str` 与 `bool` 都给 NULL 并把原因写进消息（借用型返回没有状态码通道）
    assert_eq!(unsafe { pa_pushstring(state, b"12\0".as_ptr().cast(), -1) }, PA_OK);
    assert!(unsafe { pa_tointstring(state, -1, core::ptr::null_mut()) }.is_null());
    assert!(error_message(state).contains("不是 `int`"), "{}", error_message(state));
    assert_eq!(unsafe { pa_pushboolean(state, 1) }, PA_OK);
    assert!(
        unsafe { pa_tointstring(state, -1, core::ptr::null_mut()) }.is_null(),
        "`bool` 不走整数桥（它的 i64 视图走 `pa_tointeger`）"
    );
    // SAFETY: state 由 pa_create 交回且尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn bytes_push_and_view_round_trip() {
    let state = fresh_state();
    // 空字节串、含 NUL 的字节串、任意二进制
    for value in [&b""[..], b"\x00\x01\xff", b"abc"] {
        assert_eq!(
            unsafe { pa_pushbytes(state, value.as_ptr().cast(), value.len() as isize) },
            PA_OK
        );
        let mut length = 0usize;
        let view = unsafe { pa_tobytes(state, -1, &mut length) };
        assert!(!view.is_null(), "bytes 给只读字节视图");
        assert_eq!(length, value.len());
        // SAFETY: 借用视图有 length 字节。
        let observed = unsafe { core::slice::from_raw_parts(view.cast::<u8>(), length) };
        assert_eq!(observed, value, "bytes 往返");
        assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    }
    // `len < 0` ⇒ 按 NUL 结尾算；`NULL`＋正长度 ⇒ INVALID
    assert_eq!(unsafe { pa_pushbytes(state, b"hi\0".as_ptr().cast(), -1) }, PA_OK);
    let mut length = 0usize;
    assert!(!unsafe { pa_tobytes(state, -1, &mut length) }.is_null());
    assert_eq!(length, 2);
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(unsafe { pa_pushbytes(state, core::ptr::null(), 3) }, PA_ERR_INVALID);
    assert_eq!(unsafe { pa_pushbytes(state, core::ptr::null(), 0) }, PA_OK, "NULL＋0 ⇒ 空字节串");
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // `bytes` 有**自己的标签**（`AB-63` 的 `PA_TBYTES`）：判类型看 `pa_type`，
    // `pa_tobytes` 只负责取值（不再拿"非 NULL"间接判类型——那是第二个真相）
    assert_eq!(unsafe { pa_pushbytes(state, b"by".as_ptr().cast(), 2) }, PA_OK);
    assert_eq!(unsafe { pa_type(state, -1) }, PA_TBYTES);
    assert_eq!(
        unsafe { pa_isstring(state, -1) },
        0,
        "bytes 不是 str（`PA_TSTRING`）"
    );
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // 非 bytes ⇒ NULL（取值通道照旧）
    assert_eq!(unsafe { pa_pushstring(state, b"x\0".as_ptr().cast(), -1) }, PA_OK);
    assert_eq!(unsafe { pa_type(state, -1) }, PA_TSTRING);
    assert!(unsafe { pa_tobytes(state, -1, &mut length) }.is_null());
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // SAFETY: state 由 pa_create 交回且尚未销毁。
    unsafe { pa_destroy(state) };
}

// --------------------------------------------------------------------------- #
// `AB-63`：标签取值域（`pa.h` ↔ Rust 的 `tag` 模块必须一致，且只能末尾追加）
// --------------------------------------------------------------------------- #

#[test]
fn the_tag_domain_matches_the_header() {
    // 头文件的取值域是**唯一**对外口径 ⇒ 从 `pa.h` 真读一遍，和 Rust 常量逐项比：
    // 一头一尾各写一份、谁都不比，就会漂（`AB-63` 的"末尾追加"也靠它兜住）
    let header = include_str!("../include/pa.h");
    let mut from_header: Vec<(String, i32)> = Vec::new();
    for line in header.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("PA_T") else {
            continue;
        };
        let Some((name, value)) = rest.split_once('=') else {
            continue;
        };
        let name = name.trim().trim_end_matches([' ', ',']).to_owned();
        let digits: String = value
            .trim()
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            continue;
        }
        from_header.push((name, digits.parse().expect("标签值是十进制整数")));
    }
    let rust: &[(&str, i32)] = &[
        ("NIL", PA_TNIL),
        ("BOOLEAN", PA_TBOOLEAN),
        ("INTEGER", PA_TINTEGER),
        ("NUMBER", PA_TNUMBER),
        ("STRING", PA_TSTRING),
        ("TABLE", PA_TTABLE),
        ("FUNCTION", PA_TFUNCTION),
        ("HANDLE", PA_THANDLE),
        ("BYTES", PA_TBYTES),
    ];
    assert_eq!(
        from_header.len(),
        rust.len(),
        "`pa.h` 的标签个数与 Rust 的 `tag` 模块不一致：头文件 {from_header:?}"
    );
    for (index, (name, value)) in rust.iter().enumerate() {
        assert_eq!(
            from_header[index],
            ((*name).to_owned(), *value),
            "第 {index} 个标签（{name}）在 `pa.h` 与 Rust 里不一致"
        );
    }
    // **末尾追加**：编号必须是 0..=n 连续的密排（`AB-63` 禁止改动既有编号）
    for (index, (_, value)) in from_header.iter().enumerate() {
        assert_eq!(*value, index as i32, "标签编号必须从 0 连续排到 {index}");
    }
}

/// **`pa_exec_bytecode` 的三格**（第 404 轮接线 ✓）：
/// ① 产物能跑（`IM-18`…`IM-21` 的容器 ＋ 代码段）；② **指令集版本不符 ⇒ `PA_ERR_INVALID`**（`BC-29`）；
/// ③ `NULL`／非正长度 ⇒ `PA_ERR_INVALID`（`AB-22`：已实现 ⇒ 与"未提供"必须区分 ✓）。
#[test]
fn exec_bytecode_runs_a_product_and_rejects_stale_or_malformed_input() {
    use pyawa_core::compile::{compile, CheckTier, Mode};

    let source = "answer = 42\n";
    let unit = compile(source, "<bytecode-test>", Mode::PurePython, CheckTier::Shallow, 0)
        .expect("样例源码应当能编译");
    let code = pyawa_core::pyac::encode_unit(&unit);
    let version = pyawa_core::opcode_metadata::INSTRUCTION_SET_VERSION;
    let product = pyawa_core::pyac::encode(
        pyawa_core::pyac::MODE_PURE,
        0,
        0,
        source.as_bytes(),
        &code,
        version,
    );

    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);

    // ① 正常产物 ⇒ 执行成功
    assert_eq!(
        unsafe { pa_exec_bytecode(state, product.as_ptr().cast(), product.len() as isize) },
        PA_OK
    );

    // ② 陈旧版本 ⇒ 判陈旧、不加载（`BC-29`）
    let stale = pyawa_core::pyac::encode(
        pyawa_core::pyac::MODE_PURE,
        0,
        0,
        source.as_bytes(),
        &code,
        version + 1,
    );
    assert_eq!(
        unsafe { pa_exec_bytecode(state, stale.as_ptr().cast(), stale.len() as isize) },
        PA_ERR_INVALID
    );

    // ③ 非法参数 ⇒ 与"未提供"区分（`AB-22`）
    assert_eq!(
        unsafe { pa_exec_bytecode(state, core::ptr::null(), 0) },
        PA_ERR_INVALID
    );

    unsafe { pa_destroy(state) };
}

/// **裸 ABI 实例的导入面**（第 211 轮实测 ✓）：`pa_create` 交回的实例**没有装 stdlib** ⇒
/// 连 `import sys` 都是 `PA_ERR_NOTIMPLEMENTED`（5）✗ —— 于是**不能**用"裸实例"来当
/// `IM-15`（模块 I/O 经能力层）的判据 ✗（那条判据要有**运行时装配**的实例：装了 stdlib、
/// 再比"注册／不注册 `fs` 域"两种情形 ✓）。
///
/// 本格钉住的是**这条边界本身** ✓：谁要是以后让裸实例能导入，就会在这里看到变化 ✓，
/// 从而必须回头确认 `IM-15` 的判据面 ✓。
#[test]
fn a_bare_instance_cannot_import_even_builtin_modules() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let mode = b"python\0";
    let source = b"import sys\n";
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(
        status, PA_ERR_NOTIMPLEMENTED,
        "裸实例（没装 stdlib）的导入面变了 ⇒ `IM-15` 的判据面要重核"
    );
    unsafe { pa_destroy(state) };
}
