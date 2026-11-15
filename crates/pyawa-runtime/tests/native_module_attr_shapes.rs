//! **原生可调用对象的 `__module__` 来源模块**（task-6／缺口2）的端到端验收。
//!
//! 口径（期望值全部由本机 `python3` 3.14.4 实跑取得）：
//! - **内建模块里的原生函数**如实报告**来源模块**：`_csv.reader.__module__ == "_csv"`、
//!   `math.sqrt == "math"`、`itertools.count == "itertools"`、`sys.exit == "sys"`、
//!   `_io.open == "_io"`；`len` 一类仍报 `"builtins"`；
//! - **ABI 宿主函数**：`pa_register` 装进全局命名空间的报 `"builtins"`（与内建 `len` 同档）；
//!   `pa_module_addfunction` 装的按**宿主模块完整点分名**报（顶层 `mymod`、子模块
//!   `mymod.sub`）——与 `sys.modules` 的键同一口径。
//!
//! 这个文件同时钉住上游 `test_csv.py::MiscTestCase::test__all__` 转 PASS 的直接原因：
//! `test.support.check__all__` 用 `obj.__module__ in ("csv", "_csv")` 认公共名。

use core::ffi::c_void;
use core::mem::size_of;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_abi::host::{pa_sig, PaHostFn};
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
    pyawa_stdlib::install(instance, "native-module-attr", &[]);
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

/// `f() -> 7`（只关心属性面，不关心行为）。
unsafe extern "C-unwind" fn host_seven(state: *mut pa_state) -> i32 {
    // SAFETY: 回调契约。
    unsafe { pa_pushinteger(state, 7) }
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
        "建宿主子模块失败：{}",
        error_message(state)
    );
    assert!(!module.is_null());
    module
}

/// **内建模块里的原生函数**：`__module__` 如实报告来源模块（参照 3.14.4 实测值）。
#[test]
fn stdlib_native_functions_report_source_module() {
    let provider = PosixFs::new();
    let vtable = provider.vtable();
    let state = make_state(&vtable);
    let raw = state.as_ptr();
    unsafe {
        exec(
            raw,
            "import _csv, math, itertools, sys, _io\n\
             values = ' '.join([\n\
                 len.__module__,\n\
                 _csv.reader.__module__,\n\
                 _csv.writer.__module__,\n\
                 _csv.field_size_limit.__module__,\n\
                 _csv.register_dialect.__module__,\n\
                 _csv.get_dialect.__module__,\n\
                 _csv.list_dialects.__module__,\n\
                 _csv.unregister_dialect.__module__,\n\
                 math.sqrt.__module__,\n\
                 itertools.count.__module__,\n\
                 sys.exit.__module__,\n\
                 _io.open.__module__,\n\
             ])\n",
        );
        assert_eq!(
            global_string(raw, "values"),
            // 本机 `python3` 3.14.4 跑同一段的输出（逐项一致）。
            "builtins _csv _csv _csv _csv _csv _csv _csv math itertools sys _io",
            "内建模块里的原生函数必须报告来源模块；len 一类仍报 builtins"
        );
    }
    // `PaState` 在作用域结束处销毁实例（不要在这里再 `pa_destroy`：那会双释放）。
}

/// **ABI 宿主函数**：`pa_register` ⇒ `"builtins"`；`pa_module_addfunction` ⇒ 宿主模块完整点分名。
#[test]
fn host_functions_report_registration_module() {
    let provider = PosixFs::new();
    let vtable = provider.vtable();
    let state = make_state(&vtable);
    let raw = state.as_ptr();
    // SAFETY: state 存活；模块句柄由本实例交回。
    unsafe {
        let sig = zero_signature();
        // ① `pa_register`：装进全局命名空间 ⇒ 与内建 `len` 同档，报 "builtins"。
        let registered = c_string("host_seven");
        assert_eq!(
            pa_register(raw, registered.as_ptr().cast(), host_seven, &sig),
            PA_OK,
            "注册宿主函数失败：{}",
            error_message(raw)
        );
        // ② `pa_module_addfunction`：顶层模块 `mymod` ＋ 子模块 `mymod.sub`。
        let module = add_module(raw, "mymod");
        add_function(raw, module, "touch", host_seven, zero_signature());
        let submodule = add_submodule(raw, module, "sub");
        add_function(raw, submodule, "deep", host_seven, zero_signature());
        exec(
            raw,
            "import mymod\n\
             values = ' '.join([\n\
                 host_seven.__module__,\n\
                 mymod.touch.__module__,\n\
                 mymod.sub.deep.__module__,\n\
             ])\n",
        );
        assert_eq!(
            global_string(raw, "values"),
            "builtins mymod mymod.sub",
            "pa_register ⇒ builtins；pa_module_addfunction ⇒ 宿主模块完整点分名"
        );
    }
    // `PaState` 在作用域结束处销毁实例（不要在这里再 `pa_destroy`：那会双释放）。
}
