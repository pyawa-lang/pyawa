//! **`AB-52` 的 `type_expr` 字符串求值**（M4；`SPEC-type-system.md` §16 的 `TS-51`…`TS-56`，
//! `SPEC-c-abi.md` 的 `AB-52`）。
//!
//! 判据（`T-TS-14` 的 ABI 半）：
//! 1. 注册时字符串 `type_expr` 求值成与 `AB-69` **同一表示** ⇒ **边界运行期检查**生效；
//! 2. 同一签名同时进**编译期宿主调用点检查**（`HostFunctionSignature` 静态标签来自字符串通道）；
//! 3. 坏字符串/未知名 ⇒ `PA_ERR_INVALID`（不静默降级）；显式前向引用未解析 ⇒ 平台类型；
//! 4. `.pyi` 导出**仍用原字符串**（`AB-53`：导出是投影，不是求值结果）。

use core::mem::size_of;

use pyawa_abi::host::{pa_param, pa_sig, PaHostDealloc, PaHostTraverse};
use pyawa_abi::tag::*;
use pyawa_abi::status::*;
use pyawa_abi::*;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
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

/// 单实参宿主签名（`type_expr` 可为 `Some` 的字符串）；返回值可选注解。
fn signature_with_one_parameter(
    param: &'static [u8],
    type_expr: Option<&'static [u8]>,
    ret_expr: Option<&'static [u8]>,
) -> (pa_sig, [pa_param; 1]) {
    let params = [pa_param {
        size: size_of::<pa_param>(),
        name: param.as_ptr().cast(),
        type_expr: type_expr.map_or(core::ptr::null(), |text| text.as_ptr().cast()),
        flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
        default_handle: core::ptr::null_mut(),
    }];
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: ret_expr.map_or(core::ptr::null(), |text| text.as_ptr().cast()),
        nparams: params.len(),
        params: params.as_ptr(),
    };
    (signature, params)
}

/// 宿主函数：把第 1 个实参当整数返回（`int → int` 的探针）。
unsafe extern "C-unwind" fn host_echo_int(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回。
    unsafe {
        let mut value = 0i64;
        if pa_tointeger(state, -1, &mut value) != PA_OK {
            return PA_ERR_INVALID;
        }
        pa_pop(state, 1);
        pa_pushinteger(state, value)
    }
}

/// 宿主函数：忽略实参、返回常数 `0`（平台类型探针：不检查参数类型）。
unsafe extern "C-unwind" fn host_constant(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回。
    unsafe {
        pa_pop(state, 1);
        pa_pushinteger(state, 0)
    }
}

fn exec(state: *mut pa_state, source: &'static [u8], mode: &'static [u8]) -> i32 {
    // SAFETY: 按 `pa_exec_string` 契约传参（源码显式给长度、NUL 结尾的块名/模式）。
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            b"ab52\0".as_ptr().cast(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

#[test]
fn string_type_expr_enables_boundary_checks() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let (mut signature, params) =
        signature_with_one_parameter(b"value\0", Some(b"int\0"), Some(b"int\0"));
    signature.params = params.as_ptr();
    // SAFETY: 名字/函数/签名都有效。
    assert_eq!(
        unsafe { pa_register(state, b"echo_int\0".as_ptr().cast(), host_echo_int, &signature) },
        PA_OK
    );
    // 边界运行期检查：`"bad"` 不是 `int` ⇒ 归责插件的 `TypeError`。
    let status = exec(state, b"x = echo_int(\"bad\")\n", b"python\0");
    assert_eq!(status, PA_ERR_RUNTIME, "应当被边界检查拦下：{status}");
    let message = message_of(state);
    assert!(
        message.contains("echo_int") && message.contains("int"),
        "错误消息应含宿主函数名与期望类型：{message}"
    );
    // 合法实参照常执行。
    assert_eq!(exec(state, b"x = echo_int(5)\n", b"python\0"), PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn string_type_expr_feeds_compile_time_host_call_checks() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let (mut signature, params) =
        signature_with_one_parameter(b"value\0", Some(b"int\0"), Some(b"int\0"));
    signature.params = params.as_ptr();
    // SAFETY: 名字/函数/签名都有效。
    assert_eq!(
        unsafe { pa_register(state, b"echo_int\0".as_ptr().cast(), host_echo_int, &signature) },
        PA_OK
    );
    // 编译期：扩展模式 + 已声明的宿主签名 ⇒ 调用点静态检查（`PA_ERR_SYNTAX` 是编译期错误档）。
    let status = exec(state, b"x: int = echo_int(\"bad\")\n", b"pyawa\0");
    assert_eq!(
        status, PA_ERR_SYNTAX,
        "编译期应当抓住字符串实参：{status} / {}",
        message_of(state)
    );
    // 编译期放行的调用能跑。
    assert_eq!(exec(state, b"x: int = echo_int(5)\n", b"pyawa\0"), PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn bad_type_expr_is_rejected_at_registration() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    for (name, type_expr) in [
        (b"bad_syntax\0".as_slice(), b"list[\0".as_slice()),
        (b"bad_name\0".as_slice(), b"NoSuchType\0".as_slice()),
        (b"bad_shape\0".as_slice(), b"Optional\0".as_slice()),
    ] {
        let (mut signature, params) =
            signature_with_one_parameter(b"value\0", Some(type_expr), None);
        signature.params = params.as_ptr();
        // SAFETY: 名字/函数/签名都有效（字符串是宿主给的不合法类型）。
        let status = unsafe {
            pa_register(state, name.as_ptr().cast(), host_constant, &signature)
        };
        assert_eq!(status, PA_ERR_INVALID, "{name:?} 应当被拒绝");
        assert!(!message_of(state).is_empty(), "拒绝消息必须可诊断");
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn deferred_forward_reference_is_a_platform_type() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // 显式前向引用 `"Later"` 注册时查不到 ⇒ 平台类型（不检查；不是"把坏注解吞掉"）。
    let (mut signature, params) =
        signature_with_one_parameter(b"value\0", Some(b"\"Later\"\0"), None);
    signature.params = params.as_ptr();
    // SAFETY: 同上。
    assert_eq!(
        unsafe { pa_register(state, b"later\0".as_ptr().cast(), host_constant, &signature) },
        PA_OK
    );
    // 平台类型不做检查：字符串实参也能调用。
    assert_eq!(exec(state, b"x = later(\"anything\")\n", b"python\0"), PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn pyi_export_keeps_the_original_type_expr_text() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    let (mut signature, params) =
        signature_with_one_parameter(b"value\0", Some(b"list[int]\0"), Some(b"list[int]\0"));
    signature.params = params.as_ptr();
    // SAFETY: 同上。
    assert_eq!(
        unsafe { pa_register(state, b"takes_list\0".as_ptr().cast(), host_constant, &signature) },
        PA_OK
    );
    let pyi = pyawa_abi::export::pyi(state);
    assert!(
        pyi.contains("list[int]"),
        "`.pyi` 必须保留宿主给的原始 type_expr 文本：{pyi}"
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// **`TS-56` 的同一性探针**：同一签名两种声明通道（`AB-52` 字符串 vs `AB-69` 句柄）在
/// **边界运行期**与**编译期**行为一致。
#[test]
fn string_and_handle_channels_behave_identically() {
    fn run(declare: fn(*mut pa_state)) -> (i32, i32) {
        let host = compatible_host();
        let mut state: *mut pa_state = core::ptr::null_mut();
        // SAFETY: 按契约传参。
        assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
        declare(state);
        let boundary = exec(state, b"x = echo_int(\"bad\")\n", b"python\0");
        let compile = exec(state, b"x: int = echo_int(\"bad\")\n", b"pyawa\0");
        // SAFETY: 同上。
        assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
        (boundary, compile)
    }

    // 通道一：`AB-52` 字符串 `type_expr`。
    fn by_strings(state: *mut pa_state) {
        let (mut signature, params) =
            signature_with_one_parameter(b"value\0", Some(b"int\0"), Some(b"int\0"));
        signature.params = params.as_ptr();
        // SAFETY: 名字/函数/签名都有效。
        assert_eq!(
            unsafe { pa_register(state, b"echo_int\0".as_ptr().cast(), host_echo_int, &signature) },
            PA_OK
        );
    }

    // 通道二：`AB-69` 的 `pa_type_expr` 句柄（注册时 `type_expr` 留空，随后声明签名）。
    fn by_handles(state: *mut pa_state) {
        let (mut signature, params) = signature_with_one_parameter(b"value\0", None, None);
        signature.params = params.as_ptr();
        // SAFETY: 名字/函数/签名都有效。
        assert_eq!(
            unsafe { pa_register(state, b"echo_int\0".as_ptr().cast(), host_echo_int, &signature) },
            PA_OK
        );
        // SAFETY: AB-69 契约：注册后、执行前声明。
        unsafe {
            let int_ty = pa_type_expr_runtime_tag(state, PA_TINTEGER);
            let mut types = [int_ty];
            assert_eq!(
                pa_func_set_signature(
                    state,
                    host_echo_int,
                    types.as_mut_ptr(),
                    types.len(),
                    int_ty,
                    core::ptr::null(),
                    0,
                    0,
                ),
                PA_OK
            );
        }
    }

    let (string_boundary, string_compile) = run(by_strings);
    let (handle_boundary, handle_compile) = run(by_handles);
    // 两条通道都：边界运行期 `PA_ERR_RUNTIME`、编译期 `PA_ERR_SYNTAX`。
    assert_eq!(string_boundary, handle_boundary, "边界行为不一致");
    assert_eq!(string_compile, handle_compile, "编译期行为不一致");
    assert_eq!(string_compile, PA_ERR_SYNTAX);
}

unsafe extern "C" fn noop_dealloc(_payload: *mut core::ffi::c_void) {}

unsafe extern "C" fn noop_traverse(
    _payload: *mut core::ffi::c_void,
    _context: *mut core::ffi::c_void,
    _visit: unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void),
) {
}

#[test]
fn newtype_sig_type_expr_is_validated() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // 好字符串：接受。
    let (mut signature, params) =
        signature_with_one_parameter(b"value\0", Some(b"int\0"), None);
    signature.params = params.as_ptr();
    // SAFETY: state 存活；dealloc/traverse 是合法函数指针。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"GoodPayload\0".as_ptr().cast(),
                8,
                noop_dealloc as PaHostDealloc,
                noop_traverse as PaHostTraverse,
                &signature,
            )
        },
        PA_OK,
        "合法 type_expr 应当接受：{}",
        message_of(state)
    );
    // 坏字符串：注册处拒绝（`TS-54`，与 `pa_register` 同口径）。
    let (mut bad_signature, bad_params) =
        signature_with_one_parameter(b"value\0", Some(b"list[\0"), None);
    bad_signature.params = bad_params.as_ptr();
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"BadPayload\0".as_ptr().cast(),
                8,
                noop_dealloc as PaHostDealloc,
                noop_traverse as PaHostTraverse,
                &bad_signature,
            )
        },
        PA_ERR_INVALID
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
