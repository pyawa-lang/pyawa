//! **`AB-67`：宿主模块注入**（`pa_add_host_module` ＋ 原子 API）的验收。
//!
//! 判据（PLAN「钩子族实现需求」AB-67 九条）：
//! ① `import myapp`／`from myapp.api import f`／`myapp.api.f()` 三形态一致 ✓；
//! ② 宿主函数／常量／子模块都能装 ✓；
//! ③ 两实例拿到的模块互不相同（`AB-64`）✓；
//! ④ guest 拿不到任何 `pa_*` ✓；
//! ⑤ 保留名（`sys`／`builtins`／`io`…）注入 ⇒ `PA_ERR_INVALID` ✓；
//! ⑥ 宿主返回对象在脚本里只能是**不透明句柄**（`CP-10`；本轮钉"能往返、不能内省"）✓；
//! ⑦ 宿主函数抛错 ⇒ 脚本侧可捕获异常、panic 不穿 FFI（`AB-29`）✓；
//! ⑧ **回调内调 `pa_add_host_module` ⇒ 返回错误码而非 UB** ✓；
//! ⑨ 生命周期：setup 注入 ✓、两次调用间可加 ✓（模块对象是活的）。

use std::ffi::{c_void, CString};
use std::mem::size_of;

use pyawa_abi::host::{pa_param, pa_sig, param_flags, PaHostDealloc, PaHostFn, PaHostTraverse};
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

/// 一个 0 参宿主函数的签名（`AB-25`：注册必须带签名）。
fn zero_signature() -> pa_sig {
    pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    }
}

/// # Safety
///
/// `state` 存活；`module` 是本实例交回的句柄；`sig` 按 `AB-51` 契约可读。
unsafe fn add_function(
    state: *mut pa_state,
    module: *mut c_void,
    name: &str,
    function: PaHostFn,
    sig: pa_sig,
) {
    let name = c_string(name);
    // SAFETY: 调用方契约。
    assert_eq!(
        unsafe { pa_module_addfunction(state, module, name.as_ptr(), function, &sig) },
        PA_OK,
        "装宿主函数失败：{}",
        error_message(state)
    );
}

/// # Safety
///
/// `state` 存活。
unsafe fn add_module(state: *mut pa_state, name: &str) -> *mut c_void {
    let name = c_string(name);
    let mut module: *mut c_void = core::ptr::null_mut();
    // SAFETY: 调用方契约。
    assert_eq!(
        unsafe { pa_add_host_module(state, name.as_ptr(), &mut module) },
        PA_OK,
        "建宿主模块失败：{}",
        error_message(state)
    );
    assert!(!module.is_null());
    module
}

/// # Safety
///
/// `state` 存活。
unsafe fn add_submodule(state: *mut pa_state, parent: *mut c_void, name: &str) -> *mut c_void {
    let name = c_string(name);
    let mut module: *mut c_void = core::ptr::null_mut();
    // SAFETY: 调用方契约。
    assert_eq!(
        unsafe { pa_module_addsubmodule(state, parent, name.as_ptr(), &mut module) },
        PA_OK,
        "建子模块失败：{}",
        error_message(state)
    );
    assert!(!module.is_null());
    module
}

// ---- 演示用宿主函数 -------------------------------------------------------- #

/// `f() -> 42`
unsafe extern "C-unwind" fn host_answer(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, 42) }
}

/// `value() -> 99`（常量之外再验证一次函数）
unsafe extern "C-unwind" fn host_ninety_nine(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, 99) }
}

/// `touch(x) -> 1`（`x` 必须是**不透明句柄**；否则给 0）
unsafe extern "C-unwind" fn host_touch(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    let tag = unsafe { pa_type(state, -1) };
    let matched = tag == PA_THANDLE;
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, if matched { 1 } else { 0 }) }
}

/// `try_register() -> pa_add_host_module(...)` 的返回码（**回调内注册**必须得错误码）。
unsafe extern "C-unwind" fn host_try_register(state: *mut pa_state) -> i32 {
    let name = c_string("nested");
    let mut out: *mut c_void = core::ptr::null_mut();
    // SAFETY: 回调契约（`AB-67` ⑧：这里必须返回错误码，不是 UB）。
    let code = unsafe { pa_add_host_module(state, name.as_ptr(), &mut out) };
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, code as i64) }
}

/// `boom() -> 宿主主动抛错`（`AB-29`：脚本侧可捕获）。
unsafe extern "C-unwind" fn host_boom(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    unsafe { pa_error(state, c"宿主函数主动抛错".as_ptr()) }
}

/// `panic_here() -> 宿主 panic`（`AB-26`／`AB-29`：panic 不穿 FFI，转状态码）。
unsafe extern "C-unwind" fn host_panic(_state: *mut pa_state) -> i32 {
    panic!("宿主函数的 panic 必须被边界接住");
}

/// `make() -> 不透明句柄`（`AB-58`；`⑥` 用）。
unsafe extern "C-unwind" fn host_make_handle(state: *mut pa_state) -> i32 {
    let type_name = c_string("OpaqueProbe");
    // SAFETY: 回调契约；类型在 setup 阶段由 `pa_newtype` 注册进模块全局。
    if unsafe { pa_getglobal(state, type_name.as_ptr()) } != PA_OK {
        return PA_ERR_RUNTIME;
    }
    let mut payload: *mut c_void = core::ptr::null_mut();
    // SAFETY: 栈顶是刚取回的类型对象（`AB-59`）。
    let status = unsafe { pa_newhandle(state, -1, &mut payload) };
    if status != PA_OK {
        return status;
    }
    if !payload.is_null() {
        // SAFETY: `pa_newtype` 声明的 payload_size=8；由 VM 分配、宿主填值 ✓。
        unsafe { *payload.cast::<i64>() = 42 };
    }
    // 栈顶现在是句柄（VM 取它当返回值；下面的类型对象由调用侧收栈）。
    PA_OK
}

// `pa_newtype` 的必填槽（`AB-36`）。
unsafe extern "C" fn noop_dealloc(_payload: *mut c_void) {}
unsafe extern "C" fn noop_traverse(
    _payload: *mut c_void,
    _context: *mut c_void,
    _visit: unsafe extern "C" fn(*mut c_void, *mut c_void),
) {
}

// --------------------------------------------------------------------------- #
// ①②③⑤⑨：基本注入面
// --------------------------------------------------------------------------- #

#[test]
fn host_module_supports_three_import_forms() {
    let state = new_state();
    // SAFETY: state 存活。
    let module = unsafe { add_module(state, "myapp") };
    // 常量：VALUE = 7（`pa_pushinteger` → `pa_module_setconst`，值取自栈顶）。
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_pushinteger(state, 7), PA_OK);
        let name = c_string("VALUE");
        assert_eq!(pa_module_setconst(state, module, name.as_ptr()), PA_OK);
    }
    // 子模块 `myapp.api`（树形命名空间）。
    // SAFETY: state 存活。
    let api = unsafe { add_submodule(state, module, "api") };
    // SAFETY: state 存活。
    unsafe {
        add_function(state, api, "f", host_answer, zero_signature());
        add_function(state, module, "value", host_ninety_nine, zero_signature());
    }
    let script = r#"
import myapp
a = myapp.VALUE
b = myapp.value()
from myapp.api import f
c = f()
import myapp.api
d = myapp.api.f()
"#;
    assert_eq!(run(state, script), PA_OK, "导入失败：{}", error_message(state));
    assert_eq!(global_int(state, "a"), 7, "模块常量应可见");
    assert_eq!(global_int(state, "b"), 99, "模块宿主函数应可调");
    assert_eq!(global_int(state, "c"), 42, "`from myapp.api import f` 应可调");
    assert_eq!(global_int(state, "d"), 42, "`myapp.api.f()` 应可调");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn reserved_invalid_and_duplicate_names_are_refused() {
    let state = new_state();
    let mut out: *mut c_void = core::ptr::null_mut();
    for name in ["sys", "builtins", "io", "_io", "importlib", "_imp"] {
        let name = c_string(name);
        // SAFETY: state 存活；out 可写。
        assert_eq!(
            unsafe { pa_add_host_module(state, name.as_ptr(), &mut out) },
            PA_ERR_INVALID,
            "保留名应被拒"
        );
    }
    for name in ["a.b", "1module", "", "with space"] {
        let display = name.to_owned();
        let name = c_string(name);
        // SAFETY: 同上。
        assert_eq!(
            unsafe { pa_add_host_module(state, name.as_ptr(), &mut out) },
            PA_ERR_INVALID,
            "非法模块名 `{display}` 应被拒"
        );
    }
    // SAFETY: state 存活。
    let module = unsafe { add_module(state, "myapp") };
    // 重复顶层名 ⇒ 拒绝。
    let duplicate = c_string("myapp");
    // SAFETY: state 存活。
    assert_eq!(
        unsafe { pa_add_host_module(state, duplicate.as_ptr(), &mut out) },
        PA_ERR_INVALID,
        "重复模块名应被拒"
    );
    // 重复子模块 ⇒ 拒绝。
    // SAFETY: state 存活。
    let _ = unsafe { add_submodule(state, module, "api") };
    let api_name = c_string("api");
    // SAFETY: state 存活。
    assert_eq!(
        unsafe { pa_module_addsubmodule(state, module, api_name.as_ptr(), &mut out) },
        PA_ERR_INVALID,
        "重复子模块名应被拒"
    );
    // 坏句柄／空出参 ⇒ 用法错误（不崩）。
    // SAFETY: 显式 NULL 是契约允许的用法错误。
    assert_eq!(
        unsafe { pa_module_setconst(state, core::ptr::null_mut(), duplicate.as_ptr()) },
        PA_ERR_INVALID
    );
    // SAFETY: state 存活。
    assert_eq!(
        unsafe { pa_add_host_module(state, duplicate.as_ptr(), core::ptr::null_mut()) },
        PA_ERR_INVALID
    );
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn two_instances_get_independent_modules() {
    let state_a = new_state();
    let state_b = new_state();
    // SAFETY: 两个 state 都存活。
    let module_a = unsafe { add_module(state_a, "myapp") };
    // SAFETY: 同上。
    let module_b = unsafe { add_module(state_b, "myapp") };
    assert_ne!(module_a, module_b, "两实例的模块句柄必须不同");
    // SAFETY: state_a 存活。
    unsafe {
        assert_eq!(pa_pushinteger(state_a, 1), PA_OK);
        let name = c_string("VALUE");
        assert_eq!(pa_module_setconst(state_a, module_a, name.as_ptr()), PA_OK);
    }
    // SAFETY: state_b 存活。
    unsafe {
        assert_eq!(pa_pushinteger(state_b, 2), PA_OK);
        let name = c_string("VALUE");
        assert_eq!(pa_module_setconst(state_b, module_b, name.as_ptr()), PA_OK);
    }
    assert_eq!(run(state_a, "import myapp\na = myapp.VALUE\n"), PA_OK);
    assert_eq!(run(state_b, "import myapp\nb = myapp.VALUE\n"), PA_OK);
    assert_eq!(global_int(state_a, "a"), 1);
    assert_eq!(global_int(state_b, "b"), 2);
    // 没有注入的实例看不到这个模块（隔离）。
    let state_c = new_state();
    assert_eq!(
        run(
            state_c,
            "blocked = 0\ntry:\n    import myapp\nexcept:\n    blocked = 1\n"
        ),
        PA_OK,
        "{}",
        error_message(state_c)
    );
    assert_eq!(global_int(state_c, "blocked"), 1, "未注入的实例不该能导入");
    // SAFETY: 三个 state 都由 pa_create 交回、尚未销毁。
    unsafe {
        pa_destroy(state_c);
        pa_destroy(state_a);
        pa_destroy(state_b);
    }
}

#[test]
fn guest_cannot_reach_abi_entry_points() {
    let state = new_state();
    // SAFETY: state 存活。
    let module = unsafe { add_module(state, "myapp") };
    let sig = zero_signature();
    // SAFETY: state 存活。
    unsafe { add_function(state, module, "touch", host_touch, sig) };
    // 脚本能看到的只有注入面（`__name__`／函数），没有任何 `pa_*`（`AB-67` ④）。
    let script = r#"
import myapp
leaked = 0
for key in myapp.__dict__:
    if key == "pa_add_host_module":
        leaked = 1
    if key == "pa_module_addfunction":
        leaked = 1
    if key == "pa_setpath":
        leaked = 1
name = myapp.__name__
"#;
    assert_eq!(run(state, script), PA_OK, "{}", error_message(state));
    assert_eq!(global_int(state, "leaked"), 0, "guest 不该拿到任何 pa_*");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn module_stays_live_between_host_calls() {
    let state = new_state();
    // SAFETY: state 存活。
    let module = unsafe { add_module(state, "myapp") };
    // SAFETY: state 存活。
    unsafe { add_function(state, module, "value", host_ninety_nine, zero_signature()) };
    assert_eq!(
        run(state, "import myapp\nfirst = myapp.value()\n"),
        PA_OK,
        "{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "first"), 99);
    // 两次调用之间：往**同一个**模块加常量＋函数（模块对象是活的 ✓）。
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_pushinteger(state, 123), PA_OK);
        let late = c_string("LATE");
        assert_eq!(pa_module_setconst(state, module, late.as_ptr()), PA_OK);
        add_function(state, module, "late_answer", host_answer, zero_signature());
    }
    assert_eq!(
        run(state, "import myapp\nsecond = myapp.LATE + myapp.late_answer()\n"),
        PA_OK,
        "两次调用之间加的成员应立即可见：{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "second"), 165);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

// --------------------------------------------------------------------------- #
// ⑦⑧：宿主函数失败面
// --------------------------------------------------------------------------- #

#[test]
fn host_function_errors_stay_script_exceptions() {
    let state = new_state();
    // SAFETY: state 存活。
    let module = unsafe { add_module(state, "myapp") };
    // SAFETY: state 存活。
    unsafe {
        add_function(state, module, "boom", host_boom, zero_signature());
        add_function(state, module, "panic_here", host_panic, zero_signature());
    }
    let script = r#"
import myapp
caught = 0
try:
    myapp.boom()
except:
    caught = caught + 1
try:
    myapp.panic_here()
except:
    caught = caught + 1
"#;
    assert_eq!(
        run(state, script),
        PA_OK,
        "宿主错误/panic 不应穿 FFI：{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "caught"), 2, "两种失败都应是脚本侧可捕获异常");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn registering_from_inside_a_callback_returns_an_error_code() {
    let state = new_state();
    // SAFETY: state 存活。
    let module = unsafe { add_module(state, "myapp") };
    // SAFETY: state 存活。
    unsafe {
        let sig = zero_signature();
        add_function(state, module, "try_register", host_try_register, sig);
    }
    assert_eq!(
        run(state, "import myapp\ncode = myapp.try_register()\n"),
        PA_OK,
        "{}",
        error_message(state)
    );
    // `⑧`：回调内调 `pa_add_host_module` **必须**拿到错误码（不是 UB／崩溃）。
    assert_eq!(
        global_int(state, "code"),
        PA_ERR_INVALID as i64,
        "回调内的模块注册应返回 PA_ERR_INVALID"
    );
    // 失败后实例仍可正常用（没崩、没污染）。
    assert_eq!(run(state, "import myapp\nstill = 1\n"), PA_OK);
    assert_eq!(global_int(state, "still"), 1);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

// --------------------------------------------------------------------------- #
// ⑥：宿主返回对象只能是不透明句柄（能往返、不能内省）
// --------------------------------------------------------------------------- #

#[test]
fn host_returned_objects_are_opaque_handles() {
    let state = new_state();
    // `AB-35`／`AB-58`：先注册宿主类型（`pa_newtype` 会把类型对象压栈 ⇒ 收栈）。
    let type_name = c_string("OpaqueProbe");
    let sig = zero_signature();
    // SAFETY: state 存活；dealloc／traverse 是合法回调。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                type_name.as_ptr(),
                0,
                8,
                noop_dealloc as PaHostDealloc,
                noop_traverse as PaHostTraverse,
                &sig,
            )
        },
        PA_OK
    );
    // SAFETY: 注册压了一份类型对象在栈顶。
    unsafe { assert_eq!(pa_settop(state, 0), PA_OK) };
    // SAFETY: state 存活。
    let module = unsafe { add_module(state, "myapp") };
    // SAFETY: state 存活。
    unsafe {
        add_function(state, module, "make", host_make_handle, zero_signature());
        // `touch(x)` 需要一个参数位（签名只影响检查；先给个位置参数名）。
        let parameter_name = c_string("handle");
        let parameters = [pa_param {
            size: size_of::<pa_param>(),
            name: parameter_name.as_ptr(),
            type_expr: core::ptr::null(),
            flags: param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        }];
        let touch_sig = pa_sig {
            size: size_of::<pa_sig>(),
            flags: 0,
            ret_expr: core::ptr::null(),
            nparams: 1,
            params: parameters.as_ptr(),
        };
        add_function(state, module, "touch", host_touch, touch_sig);
    }
    let script = r#"
import myapp
handle = myapp.make()
round_trip = myapp.touch(handle)
introspected = 0
try:
    handle.payload
except:
    introspected = 1
"#;
    assert_eq!(run(state, script), PA_OK, "{}", error_message(state));
    assert_eq!(global_int(state, "round_trip"), 1, "句柄应能原样交回宿主");
    assert_eq!(
        global_int(state, "introspected"),
        1,
        "宿主对象内部载荷对脚本不可见（CP-10）"
    );
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}
