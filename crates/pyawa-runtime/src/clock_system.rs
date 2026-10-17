//! `clock` 域的**真实机器实现**（`DESIGN.md` §7 的平台集中点）。
//!
//! 形状在 `pyawa-capabilities`（`CP-12`：那边只有类型与函数指针 ✓），本文件才是碰平台的一侧 ✓。
//! 对应 C 层模块 `time` ✓。
//!
//! **本轮（第 317 轮）落地**：`now_ns`（`SystemTime` ⇒ 自 Unix 纪元的纳秒 ✓）与
//! `monotonic_ns`（`Instant` ⇒ 同进程内单调 ✓，起点是本 provider 建立那一刻 ✓）。

use core::ffi::c_void;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use pyawa_capabilities::clock::{CapStatus, CpClockVtable};

/// `clock` 域的 provider：单调钟的**零点**（`Instant`）。
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// 新建（单调钟的零点就是此刻 ✓）。
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    /// 本 provider 的 vtable（`CP-30`：带版本／尺寸字段；`state` 指回自己）。
    pub fn vtable(&self) -> CpClockVtable {
        CpClockVtable {
            version: 1,
            size: core::mem::size_of::<CpClockVtable>() as u32,
            state: (self as *const Self).cast_mut().cast::<c_void>(),
            now_ns: Some(clock_now_ns),
            monotonic_ns: Some(clock_monotonic_ns),
            // 睡眠那一格随后补（`CP-3`：槽位为 `None` ⇒ 只有该操作未实现 ✓，不影响其余 ✓）
            sleep_ns: None,
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

/// `now_ns`：自 Unix 纪元起的纳秒；系统时钟早于纪元 ⇒ 机器错误（`CP-5` 的 `Machine` 那一路）。
#[allow(unsafe_code)] // 逐项开许可（本 crate 的规矩；与 `fs_posix` 同一手法 ✓）
extern "C" fn clock_now_ns(state: *mut c_void, out: *mut i64) -> CapStatus {
    let _ = state;
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => {
            // SAFETY: 调用方（核心）给了有效的出参指针（`CP-5` 的契约）。
            unsafe { *out = elapsed.as_nanos() as i64 };
            CapStatus::Ok
        }
        Err(_) => CapStatus::Machine,
    }
}

/// `monotonic_ns`：相对 provider 零点的纳秒（恒非负 ⇒ 不会走 `Machine` ✓）。
#[allow(unsafe_code)] // 逐项开许可（同上 ✓）
extern "C" fn clock_monotonic_ns(state: *mut c_void, out: *mut i64) -> CapStatus {
    // SAFETY: `state` 由 `vtable()` 给出，且 provider 比 vtable 活得久（见 `CP-30` 的纪律）。
    let provider = unsafe { &*(state as *const SystemClock) };
    // SAFETY: 同上；`out` 由调用方给。
    unsafe { *out = provider.origin.elapsed().as_nanos() as i64 };
    CapStatus::Ok
}
