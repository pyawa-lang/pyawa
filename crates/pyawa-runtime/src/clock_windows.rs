//! Windows `clock` 域的真实机器实现（`PLAN §12.20` W2）。
//!
//! - **`now_ns`**：`SystemTime`（Windows 上由 `GetSystemTimePreciseAsFileTime` 口径提供）。
//! - **`monotonic_ns`**：`Instant`（Windows 上由 **QPC** 提供；`PLAN` W2 的"QPC"落点）。
//! - **`sleep_ns`**：`std::thread::sleep`（VM 侧已把长等待切成 ≤10 ms 的有界片，`CP-27`）。
//!
//! **日历／时区三槽（`localtime`／`strftime`／`mktime`）给 `None`**（`CP-3`：只有该槽位未实现）——
//! stdlib 的 `time` 模块对此有**纯 UTC 回退**（`crates/pyawa-stdlib/src/time_module.rs` 的既有口径），
//! 在 `TZ=UTC` 下与参照实现一致（`time_shapes` 的探针就是 `TZ=UTC`）。**如实边界**：非 UTC 时区的
//! 本地时间面尚未接（真正的时区 provider 留 W5／W6，届时按 `AB-41` 填这三槽、不改既有语义）。
//!
//! 本模块只在 `windows` 目标编译；宿主构建零影响。

use core::ffi::c_void;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pyawa_capabilities::clock::{CapStatus, CpClockVtable};

/// Windows 时钟 provider：`Instant` 作单调钟起点（QPC 口径）。
pub struct WindowsClock {
    origin: Instant,
}

impl WindowsClock {
    /// 新建 provider：单调钟从此刻起算（与 POSIX `SystemClock` 同口径）。
    pub fn new() -> Self {
        Self { origin: Instant::now() }
    }

    /// 本 provider 的 vtable（`CP-30`：带版本／尺寸；`state` 指回自己）。
    pub fn vtable(&self) -> CpClockVtable {
        CpClockVtable {
            version: pyawa_abi::PA_ABI_VERSION,
            size: core::mem::size_of::<CpClockVtable>() as u32,
            state: (self as *const Self).cast_mut().cast::<c_void>(),
            now_ns: Some(clock_now_ns),
            monotonic_ns: Some(clock_monotonic_ns),
            sleep_ns: Some(clock_sleep_ns),
            // `CP-3`：Windows 的日历／时区三槽未接（stdlib 走纯 UTC 回退；见模块文档）。
            localtime: None,
            strftime: None,
            mktime: None,
        }
    }
}

impl Default for WindowsClock {
    fn default() -> Self {
        Self::new()
    }
}

/// `now_ns`：自 Unix 纪元起的纳秒；系统时钟早于纪元 ⇒ 机器错误（`CP-5`）。
#[allow(unsafe_code)] // 逐项开许可（写调用方给的有效出参）
extern "C" fn clock_now_ns(_state: *mut c_void, out: *mut i64) -> CapStatus {
    if out.is_null() {
        return CapStatus::Machine;
    }
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => {
            // SAFETY: 上面已判空；出参由调用方按 `CP-5` 给出。
            unsafe { *out = elapsed.as_nanos() as i64 };
            CapStatus::Ok
        }
        Err(_) => CapStatus::Machine,
    }
}

/// `monotonic_ns`：自 provider 建立起的纳秒（`Instant` ＝ Windows QPC）。
#[allow(unsafe_code)]
extern "C" fn clock_monotonic_ns(state: *mut c_void, out: *mut i64) -> CapStatus {
    if state.is_null() || out.is_null() {
        return CapStatus::Machine;
    }
    // SAFETY: state 由本 vtable 的 `vtable()` 给出（指回活着的 provider）；out 已判空。
    let provider = unsafe { &*(state as *const WindowsClock) };
    let nanos = provider.origin.elapsed().as_nanos() as i64;
    // SAFETY: out 已判空。
    unsafe { *out = nanos };
    CapStatus::Ok
}

/// `sleep_ns`：有界睡眠（VM 保证 `ns <= 10 ms`；`ns <= 0` ⇒ 立即成功）。
#[allow(unsafe_code)]
extern "C" fn clock_sleep_ns(_state: *mut c_void, ns: i64, _errno_out: *mut i32) -> CapStatus {
    if ns <= 0 {
        return CapStatus::Ok;
    }
    std::thread::sleep(Duration::from_nanos(ns as u64));
    CapStatus::Ok
}
