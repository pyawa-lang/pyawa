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
        assert_eq!(pa_pushbytes(state, core::ptr::null(), 0), PA_ERR_NOTIMPLEMENTED);
        assert_eq!(pa_newhandle(state, 0), PA_ERR_NOTIMPLEMENTED);
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

// ---- 宿主类型注册（`AB-35`…`AB-38`）----

use pyawa_abi::host::{PaHostDealloc, PaHostTraverse};

static HOST_DEALLOCS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static HOST_TRAVERSES: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// 宿主的 `dealloc`：把自己那份不透明载荷放掉（这里只是记一笔）。
unsafe extern "C" fn host_dealloc(payload: *mut c_void) {
    HOST_DEALLOCS.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    if !payload.is_null() {
        // SAFETY: 载荷是宿主自己用 Box 交出来的一个 u64。
        drop(unsafe { Box::from_raw(payload.cast::<u64>()) });
    }
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
    let mut kind = 0i32;
    let dealloc: PaHostDealloc = host_dealloc;
    let traverse: PaHostTraverse = host_traverse;
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                name.as_ptr().cast(),
                dealloc,
                traverse,
                &signature,
                &mut kind,
            )
        },
        PA_OK
    );
    assert!(kind > 0, "注册成功应当给出 kind");

    // 签名缺失即拒绝（AB-36）
    let mut second = 0i32;
    // SAFETY: sig 故意给 NULL。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"Widget2\0".as_ptr().cast(),
                dealloc,
                traverse,
                core::ptr::null(),
                &mut second,
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
    let mut kind = 0i32;
    // SAFETY: 按契约传参。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"Gadget\0".as_ptr().cast(),
                host_dealloc,
                host_traverse,
                &signature,
                &mut kind,
            )
        },
        PA_OK
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);

    // **未接线**：`pa_newhandle`（宿主数据的挂载约定规格未钉，见 README 与报告）
    let mut state2: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_create(&host, &mut state2) }, PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_newhandle(state2, 1) }, PA_ERR_NOTIMPLEMENTED);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state2) }, PA_OK);
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
