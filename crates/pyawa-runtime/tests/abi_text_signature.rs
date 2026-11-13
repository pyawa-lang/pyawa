//! **`AB-54`**：宿主函数的 `__text_signature__` 是注册信息（`pa_sig`）的**运行期投影**。
//!
//! 口径：
//! - 权威＝注册时的 `pa_sig`；文本签名是**投影**（不是 `.pyi`、不读 fs）；
//! - 文本签名**不含注解**（`Lib/inspect.py` 的 `_signature_fromstr` 对带注解形参直接报错）；
//! - 默认值只有在能还原成 `literal_eval` 常数（`None`／`bool`／`int`）时才写进文本签名；
//!   否则**如实不提供**（`__text_signature__` 为 `None`，不编值）。
//!
//! 本文件同时钉住 `inspect.signature()` 的端到端可用性（真装 stdlib ＋ 真注册宿主函数）。

use core::ffi::c_void;
use core::mem::size_of;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_abi::host::{pa_param, pa_sig, param_flags, PaHostFn};
use pyawa_abi::status::*;
use pyawa_abi::*;
use pyawa_runtime::fs_posix::PosixFs;
use pyawa_runtime::PaState;

fn c_string(text: &str) -> std::ffi::CString {
    std::ffi::CString::new(text).expect("测试用名字不含 NUL")
}

/// 装实例：stdlib ＋ 模块搜索路径（`Lib/`）＋ `fs` 域（`import` 要读模块文件）。
fn make_state(vtable: &pyawa_capabilities::fs::CpFsVtable) -> PaState {
    let state = PaState::new().expect("建实例");
    let raw = state.as_ptr();
    // SAFETY: `raw` 由 `PaState::new` 交回、活到本函数返回值被 drop。
    let instance = unsafe { &*raw }.instance();
    pyawa_stdlib::install(instance, "abi-text-signature", &[]);
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|parent| parent.parent())
        .expect("crates/<crate> 的上一级是仓库根")
        .join("Lib");
    pyawa_stdlib::set_module_search_path(instance, &[lib.to_string_lossy().into_owned()]);
    let implementation = (vtable as *const pyawa_capabilities::fs::CpFsVtable).cast::<c_void>();
    // SAFETY: `raw` 刚建成功；表由调用方持有（活到 state 之后）。
    unsafe {
        assert_eq!(
            pa_setcapability_async(raw, PA_DOMAIN_FS, PA_ASYNC_OK),
            PA_OK,
            "`fs` 域分类注册应当成功"
        );
        assert_eq!(
            pa_setcapability(raw, PA_DOMAIN_FS, implementation),
            PA_OK,
            "`fs` 域注册应当成功"
        );
    }
    state
}

/// 注册一个宿主函数（名字／flags 由调用方给；默认值一律 NULL）。
///
/// # Safety
///
/// `state` 存活；`names` 活到调用结束。
unsafe fn register(
    state: *mut pa_state,
    name: &str,
    function: PaHostFn,
    names: &[&str],
    flags: &[u32],
) {
    assert_eq!(names.len(), flags.len(), "测试自身：名字与 flags 数量一致");
    let name_strings: Vec<std::ffi::CString> = names.iter().map(|item| c_string(item)).collect();
    let params: Vec<pa_param> = name_strings
        .iter()
        .zip(flags.iter())
        .map(|(parameter_name, flags)| pa_param {
            size: size_of::<pa_param>(),
            name: parameter_name.as_ptr(),
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
            params.as_ptr()
        },
    };
    let function_name = c_string(name);
    // SAFETY: 调用方契约（指针都活到本次调用内）。
    assert_eq!(
        unsafe { pa_register(state, function_name.as_ptr().cast(), function, &signature) },
        PA_OK,
        "注册宿主函数失败：{}",
        error_message(state)
    );
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

/// 跑一段脚本；`python` 模式。
///
/// # Safety
///
/// `state` 存活。
unsafe fn exec(state: *mut pa_state, source: &str) {
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_OK, "脚本执行失败：{}", error_message(state));
}

/// 取脚本写进全局的名字（字符串）。
///
/// # Safety
///
/// `state` 存活。
unsafe fn global_string(state: *mut pa_state, name: &str) -> String {
    let name = c_string(name);
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, name.as_ptr()), PA_OK);
        let mut length = 0usize;
        let pointer = pa_tostring(state, -1, &mut length);
        assert!(!pointer.is_null(), "全局 {name:?} 不是字符串");
        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), length);
        let text = String::from_utf8(bytes.to_vec()).expect("宿主签名文本是 UTF-8");
        assert_eq!(pa_settop(state, 0), PA_OK);
        text
    }
}

/// 取脚本写进全局的名字（整数）。
///
/// # Safety
///
/// `state` 存活。
unsafe fn global_int(state: *mut pa_state, name: &str) -> i64 {
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

/// 造一个"总是返回 0"的宿主函数（本测试只关心签名，不关心行为）。
unsafe extern "C-unwind" fn host_zero(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, 0) }
}

#[test]
fn host_signature_projects_to_text_signature_and_inspect() {
    let provider = PosixFs::new();
    let vtable = provider.vtable();
    let state = make_state(&vtable);
    let raw = state.as_ptr();

    // ① 位置参数：`(a, b)`；`inspect.signature` 读的就是 `__text_signature__`
    unsafe {
        register(
            raw,
            "host_add",
            host_zero,
            &["a", "b"],
            &[param_flags::PA_PARAM_POSITIONAL; 2],
        );
        exec(
            raw,
            "import inspect\n\
             first = str(inspect.signature(host_add))\n\
             second = host_add.__text_signature__\n\
             result = first + '|' + second\n",
        );
        assert_eq!(
            global_string(raw, "result"),
            "(a, b)|(a, b)",
            "`pa_sig` → `__text_signature__` → `inspect.signature` 应当逐字一致"
        );
    }

    // ② 位置 ＋ `*args` ＋ 仅关键字 ＋ `**kw`（flags 的组合形态）
    unsafe {
        register(
            raw,
            "host_kw",
            host_zero,
            &["a", "args", "c", "kw"],
            &[
                param_flags::PA_PARAM_POSITIONAL,
                param_flags::PA_PARAM_VARARGS,
                param_flags::PA_PARAM_KEYWORD_ONLY,
                param_flags::PA_PARAM_VARKW,
            ],
        );
        exec(raw, "result = host_kw.__text_signature__\n");
        assert_eq!(global_string(raw, "result"), "(a, *args, c, **kw)");
    }

    // ③ 诚实边界：`HAS_DEFAULT` 但默认值句柄为 NULL（不可渲染）⇒ **不提供**文本签名
    unsafe {
        register(
            raw,
            "host_default",
            host_zero,
            &["x"],
            &[param_flags::PA_PARAM_POSITIONAL | param_flags::PA_PARAM_HAS_DEFAULT],
        );
        exec(
            raw,
            "result = 1 if host_default.__text_signature__ is None else 0\n",
        );
        assert_eq!(
            global_int(raw, "result"),
            1,
            "不可渲染的默认值必须如实不给文本签名，禁止编一个假值"
        );
    }
}
