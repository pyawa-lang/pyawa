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
