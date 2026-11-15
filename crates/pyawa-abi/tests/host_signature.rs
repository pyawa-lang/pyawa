//! **`AB-69`：宿主函数签名（`pa_type_expr` 句柄版）的边界运行期验收**（round 272）。
//!
//! 本轮钉 PLAN 验收的 ②–⑦（①"检查期抓错"属编译期调用点检查，下一轮；见台账）：
//! ② 无标注插件在**边界运行期**抓错并归责插件 ✓；
//! ③ 不声明 ⇒ 与今天完全一致（不检查）✓；
//! ④ 签名含宿主不透明类型时仍不暴露内部 ✓；
//! ⑤ 可选／变参 flags 至少各一个用例 ✓；
//! ⑥ 宿主类型可进签名（`pa_type_expr_named` 同一性＝类型对象）✓；
//! ⑦ 泛型擦除口径（`list[int]` 边界只核 list 身份；元素类型不核）✓。
//!
//! 另钉 `pa_func_set_signature` 的**生命周期**：注册后、任何执行之前声明；晚声明 ⇒ `PA_ERR_INVALID`。

use std::ffi::{c_void, CString};
use std::mem::size_of;

use pyawa_abi::host::{
    pa_param, pa_sig, param_flags, PaHostDealloc, PaHostFn, PaHostTraverse,
};
use pyawa_abi::status::*;
use pyawa_abi::tag::*;
use pyawa_abi::*;

// --------------------------------------------------------------------------- #
// 基建
// --------------------------------------------------------------------------- #

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: PA_ABI_SIZE,
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

fn new_state() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: host 是局部量、出参可写。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    assert!(!state.is_null());
    state
}

fn c_string(text: &str) -> CString {
    CString::new(text).expect("名字无 NUL")
}

fn run(state: *mut pa_state, source: &str) -> i32 {
    let source = c_string(source);
    let mode = c_string("python");
    // SAFETY: state 存活；source／mode 是 NUL 结尾局部串。
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr(),
            -1,
            core::ptr::null(),
            mode.as_ptr(),
            core::ptr::null(),
        )
    }
}

fn error_message(state: *mut pa_state) -> String {
    // SAFETY: state 存活；借用只在本函数内用。
    let pointer = unsafe { pa_errmsg(state) };
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: `pa_errmsg` 返回 NUL 结尾借用（`AB-48`）。
    unsafe { std::ffi::CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

fn global_int(state: *mut pa_state, name: &str) -> i64 {
    let name = c_string(name);
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, name.as_ptr()), PA_OK);
        let mut value = 0i64;
        assert_eq!(pa_tointeger(state, -1, &mut value), PA_OK);
        assert_eq!(pa_settop(state, 0), PA_OK);
        value
    }
}

/// 注册一个宿主函数（带 `pa_sig`；`flags` 是每个形参的标志）。
///
/// # Safety
///
/// `state` 存活。
unsafe fn register(
    state: *mut pa_state,
    name: &str,
    function: PaHostFn,
    parameter_flags: &[u32],
) -> pa_sig {
    // 形参名要在注册期间存活。
    let parameter_names: Vec<CString> = (0..parameter_flags.len())
        .map(|index| c_string(&format!("arg{index}")))
        .collect();
    let mut params: Vec<pa_param> = parameter_names
        .iter()
        .zip(parameter_flags.iter())
        .map(|(parameter_name, flags)| pa_param {
            size: size_of::<pa_param>(),
            name: parameter_name.as_ptr(),
            // 类型表达式串留给 `AB-51/52` 的元数据；`AB-69` 的检查走 `pa_type_expr` ✓
            type_expr: core::ptr::null(),
            flags: *flags,
            default_handle: core::ptr::null_mut(),
        })
        .collect();
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: params.len(),
        params: if params.is_empty() {
            core::ptr::null()
        } else {
            params.as_mut_ptr()
        },
    };
    let name = c_string(name);
    // SAFETY: 调用方契约。
    assert_eq!(
        unsafe { pa_register(state, name.as_ptr(), function, &signature) },
        PA_OK,
        "注册宿主函数失败：{}",
        error_message(state)
    );
    signature
}

/// 给已注册函数声明签名；断言成功。
///
/// # Safety
///
/// `state` 存活；`params`／`ret` 是本实例建的 `pa_type_expr`。
unsafe fn set_signature(
    state: *mut pa_state,
    function: PaHostFn,
    params: &mut [*mut c_void],
    ret: *mut c_void,
) {
    // SAFETY: 调用方契约。
    assert_eq!(
        unsafe {
            pa_func_set_signature(
                state,
                function,
                params.as_mut_ptr(),
                params.len(),
                ret,
                core::ptr::null(),
                0,
                0,
            )
        },
        PA_OK,
        "声明签名失败：{}",
        error_message(state)
    );
}

// ---- 演示用宿主函数 -------------------------------------------------------- #

/// 0 参：返回 1（走边界后到达）。
unsafe extern "C-unwind" fn host_ok(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, 1) }
}

/// 1 参：故意不看实参，返回 1（边界检查在调用前）。
unsafe extern "C-unwind" fn host_one(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, 1) }
}

/// 1 参：返回**字符串**（用于返回类型不符的用例）。
unsafe extern "C-unwind" fn host_returns_str(state: *mut pa_state) -> i32 {
    let text = c_string("不是 int");
    // SAFETY: 回调契约。
    unsafe { pa_pushstring(state, text.as_ptr(), -1) }
}

/// `tparams` 用例：返回 0 参占位。
unsafe extern "C-unwind" fn host_two(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, 2) }
}

unsafe extern "C" fn noop_dealloc(_payload: *mut c_void) {}
unsafe extern "C" fn noop_traverse(
    _payload: *mut c_void,
    _context: *mut c_void,
    _visit: unsafe extern "C" fn(*mut c_void, *mut c_void),
) {
}

/// 建一个不透明句柄类型的实例并返回（`AB-58`）。
unsafe extern "C-unwind" fn host_make_handle(state: *mut pa_state) -> i32 {
    let type_name = c_string("OpaqueProbe");
    // SAFETY: type 在 setup 阶段由 `pa_newtype` 注册进全局。
    if unsafe { pa_getglobal(state, type_name.as_ptr()) } != PA_OK {
        return PA_ERR_RUNTIME;
    }
    let mut payload: *mut c_void = core::ptr::null_mut();
    // SAFETY: 栈顶是类型对象。
    let status = unsafe { pa_newhandle(state, -1, &mut payload) };
    status
}

// --------------------------------------------------------------------------- #
// ①③④⑤⑥⑦：边界运行期的核心用例
// --------------------------------------------------------------------------- #

#[test]
fn boundary_checks_arguments_and_blames_the_plugin() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        register(state, "expect_int", host_one, &[param_flags::PA_PARAM_POSITIONAL]);
        // ②：声明 `int` 参数。
        let int_ty = pa_type_expr_runtime_tag(state, PA_TINTEGER);
        assert!(!int_ty.is_null());
        let mut params = [int_ty];
        set_signature(state, host_one, &mut params, core::ptr::null_mut());
    }
    let script = r#"
ok = expect_int(1)
caught = 0
try:
    expect_int("不是 int")
except:
    caught = 1
"#;
    assert_eq!(run(state, script), PA_OK, "{}", error_message(state));
    assert_eq!(global_int(state, "ok"), 1);
    assert_eq!(global_int(state, "caught"), 1, "边界应抛可捕获的 TypeError");
    // 错误信息应带宿主函数名（归责插件）——用未捕获的一次看 `pa_errmsg`。
    let bad = run(state, "expect_int([1])\n");
    assert_eq!(bad, PA_ERR_RUNTIME);
    let message = error_message(state);
    assert!(
        message.contains("expect_int") && message.contains("AB-69"),
        "错误消息应归责插件并带声明来源：{message}"
    );
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn undeclared_functions_are_not_checked() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        register(
            state,
            "undeclared_ok",
            host_one,
            &[param_flags::PA_PARAM_POSITIONAL],
        );
    }
    // ③：不声明签名 ⇒ 与今天完全一致（传什么类型都不检查）。
    assert_eq!(
        run(state, "ok = undeclared_ok(\"任意类型\")\n"),
        PA_OK,
        "{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "ok"), 1);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn host_opaque_types_can_appear_in_signatures() {
    let state = new_state();
    let type_name = c_string("OpaqueProbe");
    let empty_sig = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    // SAFETY: state 存活；dealloc／traverse 合法。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                type_name.as_ptr(),
                0,
                8,
                    Some(noop_dealloc as PaHostDealloc),
                    Some(noop_traverse as PaHostTraverse),
                &empty_sig,)
        },
        PA_OK
    );
    // SAFETY: `pa_newtype` 压了一份类型对象在栈顶。
    unsafe { assert_eq!(pa_settop(state, 0), PA_OK) };
    // SAFETY: state 存活。
    unsafe {
        register(state, "make_handle", host_make_handle, &[]);
        register(state, "expect_handle", host_one, &[param_flags::PA_PARAM_POSITIONAL]);
        // ⑥：宿主类型进签名（同一性＝类型对象）。
        let handle_ty = pa_type_expr_named(state, c_string("OpaqueProbe").as_ptr());
        assert!(!handle_ty.is_null(), "{}", error_message(state));
        let mut params = [handle_ty];
        set_signature(state, host_one, &mut params, core::ptr::null_mut());
    }
    let script = r#"
handle = make_handle()
ok = expect_handle(handle)
caught = 0
try:
    expect_handle(1)
except:
    caught = 1
"#;
    assert_eq!(run(state, script), PA_OK, "{}", error_message(state));
    assert_eq!(global_int(state, "ok"), 1, "不透明句柄应被接受（不暴露内部 ✓）");
    assert_eq!(global_int(state, "caught"), 1, "非宿主类型应被边界拒绝");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn optional_and_variadic_flags_are_honored() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        register(
            state,
            "optional_int",
            host_one,
            &[param_flags::PA_PARAM_POSITIONAL | param_flags::PA_PARAM_HAS_DEFAULT],
        );
        let int_ty = pa_type_expr_runtime_tag(state, PA_TINTEGER);
        let mut params = [int_ty];
        set_signature(state, host_one, &mut params, core::ptr::null_mut());

        register(
            state,
            "varargs_int",
            host_two,
            &[param_flags::PA_PARAM_VARARGS],
        );
        let int_ty = pa_type_expr_runtime_tag(state, PA_TINTEGER);
        let mut params = [int_ty];
        set_signature(state, host_two, &mut params, core::ptr::null_mut());
    }
    let script = r#"
# ⑤ 可选：0 个实参合法；给了就必须是 int
optional_zero = optional_int()
optional_one = optional_int(7)
optional_bad = 0
try:
    optional_int("x")
except:
    optional_bad = 1
# ⑤ 变参：都按元素类型（int）核
varargs_ok = varargs_int(1, 2, 3)
varargs_bad = 0
try:
    varargs_int(1, "x")
except:
    varargs_bad = 1
"#;
    assert_eq!(run(state, script), PA_OK, "{}", error_message(state));
    assert_eq!(global_int(state, "optional_zero"), 1);
    assert_eq!(global_int(state, "optional_one"), 1);
    assert_eq!(global_int(state, "optional_bad"), 1);
    assert_eq!(global_int(state, "varargs_ok"), 2);
    assert_eq!(global_int(state, "varargs_bad"), 1);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn generic_element_types_are_erased_at_the_boundary() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        register(state, "expect_list", host_one, &[param_flags::PA_PARAM_POSITIONAL]);
        // ⑦：`list[int]` 在边界只核 list 身份（TS-35 擦除）⇒ 元素类型不核。
        let int_ty = pa_type_expr_runtime_tag(state, PA_TINTEGER);
        let list_ty = pa_type_expr_list_of(state, int_ty);
        let mut params = [list_ty];
        set_signature(state, host_one, &mut params, core::ptr::null_mut());
    }
    let script = r#"
list_int = expect_list([1, 2])
list_str = expect_list(["a", "b"])
caught = 0
try:
    expect_list(3)
except:
    caught = 1
"#;
    assert_eq!(run(state, script), PA_OK, "{}", error_message(state));
    assert_eq!(global_int(state, "list_int"), 1);
    assert_eq!(
        global_int(state, "list_str"),
        1,
        "泛型元素类型按 TS-35 擦除 ⇒ list[str] 也过边界"
    );
    assert_eq!(global_int(state, "caught"), 1, "非 list 应被边界拒绝");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn return_types_are_checked_too() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        register(state, "returns_str", host_returns_str, &[]);
        let int_ty = pa_type_expr_runtime_tag(state, PA_TINTEGER);
        // 返回类型声明 int，但宿主返回 str ⇒ 边界报 TypeError（脚本可捕获）。
        assert_eq!(
            pa_func_set_signature(
                state,
                host_returns_str,
                core::ptr::null(),
                0,
                int_ty,
                core::ptr::null(),
                0,
                0,
            ),
            PA_OK,
            "{}",
            error_message(state)
        );
    }
    assert_eq!(
        run(
            state,
            "caught = 0\ntry:\n    returns_str()\nexcept:\n    caught = 1\n"
        ),
        PA_OK,
        "{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "caught"), 1, "返回类型不符应是可捕获异常");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

// --------------------------------------------------------------------------- #
// 签名生命周期与参数校验（"同一时刻给"）
// --------------------------------------------------------------------------- #

#[test]
fn signature_must_be_declared_before_any_execution() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        register(state, "late", host_ok, &[]);
    }
    // 先跑一次脚本（注册后已执行 ⇒ 窗口关闭）。
    assert_eq!(run(state, "x = 1\n"), PA_OK);
    // SAFETY: state 存活。
    let status = unsafe {
        pa_func_set_signature(
            state,
            host_ok,
            core::ptr::null(),
            0,
            core::ptr::null_mut(),
            core::ptr::null(),
            0,
            0,
        )
    };
    assert_eq!(status, PA_ERR_INVALID, "晚声明签名应被拒");
    let message = error_message(state);
    assert!(message.contains("AB-69"), "{message}");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn signature_argument_validation() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        register(state, "one_arg", host_one, &[param_flags::PA_PARAM_POSITIONAL]);
    }
    // 未注册的函数指针 ⇒ 拒绝。
    // SAFETY: host_ok 没有注册。
    assert_eq!(
        unsafe {
            pa_func_set_signature(
                state,
                host_ok,
                core::ptr::null(),
                0,
                core::ptr::null_mut(),
                core::ptr::null(),
                0,
                0,
            )
        },
        PA_ERR_INVALID
    );
    // flags 非 0 ⇒ 拒绝。
    // SAFETY: state 存活。
    assert_eq!(
        unsafe {
            pa_func_set_signature(
                state,
                host_one,
                core::ptr::null(),
                1,
                core::ptr::null_mut(),
                core::ptr::null(),
                0,
                1,
            )
        },
        PA_ERR_INVALID
    );
    // nparams 与注册时不一致 ⇒ 拒绝。
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            pa_func_set_signature(
                state,
                host_one,
                core::ptr::null(),
                0,
                core::ptr::null_mut(),
                core::ptr::null(),
                0,
                0,
            )
        },
        PA_ERR_INVALID
    );
    // 坏句柄（NULL 元素配 n>0）⇒ 拒绝。
    let null_params = [core::ptr::null_mut()];
    // SAFETY: 指针数组可读、元素显式 NULL。
    assert_eq!(
        unsafe {
            pa_func_set_signature(
                state,
                host_one,
                null_params.as_ptr(),
                1,
                core::ptr::null_mut(),
                core::ptr::null(),
                0,
                0,
            )
        },
        PA_ERR_INVALID
    );
    // 类型变量未在 `tparams` 声明 ⇒ 边界归责插件（脚本侧 TypeError）。
    // SAFETY: state 存活。
    unsafe {
        let variable = pa_type_expr_var(state, c_string("T").as_ptr());
        assert!(!variable.is_null());
        let mut params = [variable];
        assert_eq!(
            pa_func_set_signature(
                state,
                host_one,
                params.as_mut_ptr(),
                1,
                core::ptr::null_mut(),
                core::ptr::null(),
                0,
                0,
            ),
            PA_OK
        );
    }
    assert_eq!(
        run(state, "caught = 0\ntry:\n    one_arg(1)\nexcept:\n    caught = 1\n"),
        PA_OK,
        "{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "caught"), 1, "未声明的类型变量应被边界拒绝");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn pa_type_expr_constructors_validate_handles() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        // 不存在的类型名 ⇒ NULL ＋ 消息。
        assert!(pa_type_expr_named(state, c_string("不存在的类型").as_ptr()).is_null());
        assert!(!error_message(state).is_empty());
        // 非法 tag ⇒ NULL。
        assert!(pa_type_expr_runtime_tag(state, 99).is_null());
        // 空句柄 ⇒ NULL。
        assert!(pa_type_expr_list_of(state, core::ptr::null_mut()).is_null());
        // union 至少要 1 个成员。
        assert!(pa_type_expr_union(state, core::ptr::null(), 0).is_null());
    }
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

// --------------------------------------------------------------------------- #
// ①：检查期（编译期调用点检查；`TS-47` 第二插入点）
// --------------------------------------------------------------------------- #

/// 扩展模式（`pyawa`）下的编译期检查：**有标注**的调用点不匹配 ⇒ `PA_ERR_SYNTAX`（脚本没起来）。
#[test]
fn annotated_call_sites_are_checked_at_compile_time() {
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        register(state, "expect_int", host_one, &[param_flags::PA_PARAM_POSITIONAL]);
        let int_ty = pa_type_expr_runtime_tag(state, PA_TINTEGER);
        let mut params = [int_ty];
        set_signature(state, host_one, &mut params, core::ptr::null_mut());
    }
    // 静态可判的不符 ⇒ **编译期**拒绝（`PA_ERR_SYNTAX`；`TS-47` 的静态档）。
    let source = c_string("s: str = \"x\"\ny = expect_int(s)\n");
    let mode = c_string("pyawa");
    // SAFETY: state 存活；source／mode 是 NUL 结尾局部串。
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr(),
            -1,
            core::ptr::null(),
            mode.as_ptr(),
            core::ptr::null(),
        )
    };
    assert_eq!(
        status, PA_ERR_SYNTAX,
        "有标注调用点的不符应在**检查期**报错；实际状态 {status}，消息 {}",
        error_message(state)
    );
    let message = error_message(state);
    assert!(
        message.contains("expect_int") && message.contains("AB-69"),
        "编译期错误应归责插件：{message}"
    );
    // 静态可判的**相符** ⇒ 正常编译并运行（宿主函数被调到）。
    let good = c_string("x: int = 1\nok = expect_int(x)\n");
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            pa_exec_string(
                state,
                good.as_ptr(),
                -1,
                core::ptr::null(),
                mode.as_ptr(),
                core::ptr::null(),
            )
        },
        PA_OK,
        "{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "ok"), 1);
    // 判不出的实参（无标注的名字）⇒ 编译期放行，**边界运行期**才报（两档分工 ✓）。
    let dynamic = c_string("u = \"x\"\ncaught = 0\ntry:\n    expect_int(u)\nexcept:\n    caught = 1\n");
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            pa_exec_string(
                state,
                dynamic.as_ptr(),
                -1,
                core::ptr::null(),
                mode.as_ptr(),
                core::ptr::null(),
            )
        },
        PA_OK,
        "{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "caught"), 1, "判不出时由边界运行期兜住");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}
