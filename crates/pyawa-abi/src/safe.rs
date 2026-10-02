//! 给**不允许写 `unsafe`** 的组合根用的安全门面（`pyawa-runtime` 就是这种：
//! 它自带 `#![forbid(unsafe_code)]`，而调 C ABI 必须 `unsafe`）。
//!
//! 这里**不新增任何语义**（`AB-4`／`AB-6`）：每个函数都是对已导出 `pa_*` 的一对一薄包装，
//! 只是把 `unsafe` 收在 `pyawa-abi` 内部。

use core::ffi::{c_char, CStr};

use crate::status;
use crate::{pa_create, pa_destroy, pa_errmsg, pa_host, pa_state};

/// 按已发布头文件的形状创建一个实例：返回 `(实例, 状态码)`。
///
/// 状态码非 `PA_OK` 时实例是**诊断实例**（`AB-56`），只有 [`message`] 与 [`destroy`] 可用。
pub fn create(host: &pa_host) -> (*mut pa_state, i32) {
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: host 由调用方按已发布头文件构造；state 是可写出参。
    let code = unsafe { pa_create(host, &mut state) };
    (state, code)
}

/// 读 `pa_errmsg`（放进 `String`，避免宿主拿到会失效的借用指针，`AB-48`）。
pub fn message(state: *mut pa_state) -> Option<String> {
    if state.is_null() {
        return None;
    }
    // SAFETY: 调用方保证 state 是 pa_create 交回且尚未销毁的指针。
    let raw: *const c_char = unsafe { pa_errmsg(state) };
    if raw.is_null() {
        return None;
    }
    // SAFETY: pa_errmsg 交回的借用指针在下次 API 调用前有效（这里立即复制）。
    Some(unsafe { CStr::from_ptr(raw) }.to_string_lossy().into_owned())
}

/// 销毁实例（幂等：`NULL` 直接返回）。
pub fn destroy(state: *mut pa_state) {
    if state.is_null() {
        return;
    }
    // SAFETY: 调用方保证这是 pa_create 交回、尚未销毁的指针，且此后不再使用。
    let _ = unsafe { pa_destroy(state) };
}

/// 状态码是否表示"可用实例"。
pub fn is_usable(code: i32) -> bool {
    code == status::PA_OK
}

/// **`DESIGN.md` §9 第 20 条**：把平台相关**只读常量**（`errno` 一类）注入实例。
///
/// 由组合根（`pyawa-runtime`）在启动时调用；**不新增能力域**（`REQUIREMENTS.md`），
/// 也**不是** C 导出（`AB-6` 只管 C ABI 的导出集合）。
pub fn set_platform_constants(state: *mut pa_state, constants: &[(&'static str, i64)]) {
    if state.is_null() {
        return;
    }
    // SAFETY: 调用方保证 state 是 pa_create 交回且尚未销毁的指针。
    let state_ref = unsafe { &*state };
    state_ref
        .instance
        .set_platform_constants(constants);
}

/// 按**名字**取实例里的平台常量（`CM-20`）。
pub fn platform_constant(state: *mut pa_state, name: &str) -> Option<i64> {
    if state.is_null() {
        return None;
    }
    // SAFETY: 同上。
    let state_ref = unsafe { &*state };
    state_ref.instance.platform_constant(name)
}
