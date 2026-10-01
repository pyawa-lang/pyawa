//! Pyawa 独立运行时：真实机器能力实现与入口（REPL/CLI）。
//!
//! 工作区内平台依赖的**集中点**，见本 crate 的 `README.md`。

// `CX-4` 的静态扫描**排除**本 crate（`docs/CONSTRAINTS.md` §3.1："它**就是**平台依赖的集中点"）
// ⇒ 这里允许定点使用平台相关代码（FFI／libc），但**逐项**开许可、不许整文件放开。
#![deny(unsafe_code)]

use core::ffi::{c_void, c_char};
use pyawa_abi::{pa_host, pa_state, safe, status};

/// **`§15.4`**：`paL_newstate()`——`pa_create` ＋ 真实机器 provider 的便捷入口（C 形态）。
///
/// 本实现先给出**没有能力实现**的实例（九域全部未注册 ⇒ 调用时报"未实现"，`CP-2`）；
/// 真实机器 provider（`DESIGN.md` §12 的 `M5`）接上之后，逐域注册的动作加在这里。
/// 创建失败时返回 `NULL`（诊断信息在诊断实例里，`AB-56`）。
///
/// # Safety
///
/// 交回的指针必须用 `pa_destroy` 释放，且不得跨线程共享（`OM-1` 的实例隔离）。
// 这是 C ABI 导出符号：`#[no_mangle]` 属于 unsafe 面，按 `CX-4` 的例外定点放开。
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn paL_newstate() -> *mut pa_state {
    let host = pa_host {
        abi_size: core::mem::size_of::<pa_host>(),
        abi_version: pyawa_abi::PA_ABI_VERSION,
        capabilities: core::ptr::null::<c_void>(),
    };
    let (state, code) = safe::create(&host);
    if state.is_null() || !safe::is_usable(code) {
        // 诊断实例也交回（宿主可用 `pa_errmsg` 看原因，`AB-56`）；完全没建起来才给 NULL
        if state.is_null() {
            return core::ptr::null_mut();
        }
    }
    state
}

/// **`§15.4`** 的 Rust 侧持有者：方便在 Rust 里用（`Drop` 时销毁）。
pub struct PaState {
    raw: *mut pa_state,
    /// 创建时的状态码（`PA_ERR_ABI` ⇒ 诊断实例，`AB-56`）。
    pub code: i32,
}

impl PaState {
    /// 创建一个实例（Rust 侧入口，等价于 [`paL_newstate`]）。
    pub fn new() -> Option<Self> {
        let host = pa_host {
            abi_size: core::mem::size_of::<pa_host>(),
            abi_version: pyawa_abi::PA_ABI_VERSION,
            capabilities: core::ptr::null::<c_void>(),
        };
        let (raw, code) = safe::create(&host);
        if raw.is_null() {
            return None;
        }
        Some(Self { raw, code })
    }

    /// 创建时的状态码。
    pub fn code(&self) -> i32 {
        self.code
    }

    /// 是不是诊断实例（`AB-56`）。
    pub fn is_diagnostic(&self) -> bool {
        self.code == status::PA_ERR_ABI
    }

    /// 当前诊断信息（复制成 `String`，避免借用指针失效，`AB-48`）。
    pub fn message(&self) -> Option<String> {
        safe::message(self.raw)
    }

    /// 裸指针。
    pub fn as_ptr(&self) -> *mut pa_state {
        self.raw
    }
}

impl Drop for PaState {
    fn drop(&mut self) {
        safe::destroy(self.raw);
        self.raw = core::ptr::null_mut();
    }
}

// 让 `c_char` 的导入在未使用时有明确去处（C 形态函数签名里随后会用到）
#[allow(dead_code)]
type CChar = c_char;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newstate_hands_out_a_usable_instance() {
        let state = PaState::new().expect("应当能建实例");
        assert_eq!(state.code(), status::PA_OK);
        assert!(!state.is_diagnostic());
        assert!(state.as_ptr().is_null() == false);
    }

    #[test]
    fn the_c_entry_point_matches_the_rust_one() {
        // SAFETY: 本测试自己用、自己销毁。
        let raw = paL_newstate();
        assert!(!raw.is_null());
        assert_eq!(safe::message(raw), None, "可用实例没有诊断信息");
        safe::destroy(raw);
    }
}
