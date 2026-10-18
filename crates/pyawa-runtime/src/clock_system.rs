//! `clock` 域的**真实机器实现**（`DESIGN.md` §7 的平台集中点）。
//!
//! 形状在 `pyawa-capabilities`（`CP-12`：那边只有类型与函数指针 ✓），本文件才是碰平台的一侧 ✓。
//! 对应 C 层模块 `time` ✓。
//!
//! **第 317 轮落地**：`now_ns`（`SystemTime` ⇒ 自 Unix 纪元的纳秒 ✓）与
//! `monotonic_ns`（`Instant` ⇒ 同进程内单调 ✓，起点是本 provider 建立那一刻 ✓）。
//!
//! **round 12（`T-CP-10`）落地 `sleep_ns`**：本槽只做**有界**睡眠（一次调用最长 10 ms 量级）——
//! `CP-27` 的"可取消等待"在 **VM 侧**做：`crates/pyawa-core/src/instance/wait.rs` 把长等待切成
//! 时间片、逐片调本槽、片间查实例中断标志（`AB-44`／`CP-9`：不改 vtable 签名、调用仍同步）。
//! 取消 → 呈现沿用既有中断口径（`ExecError::Interrupted` ⇒ `PA_ERR_INTERRUPT`）。

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
    ///
    /// `version` 填**三份契约共用的** `pyawa_abi::PA_ABI_VERSION`（`AB-39`）——`pa_setcapability`
    /// 在注册时按 `CP-30` 校验这个公共头（主版本不符／次版本高于运行时 ⇒ 注册失败）。
    pub fn vtable(&self) -> CpClockVtable {
        CpClockVtable {
            version: pyawa_abi::PA_ABI_VERSION,
            size: core::mem::size_of::<CpClockVtable>() as u32,
            state: (self as *const Self).cast_mut().cast::<c_void>(),
            now_ns: Some(clock_now_ns),
            monotonic_ns: Some(clock_monotonic_ns),
            // `T-CP-10`：有界睡眠；可取消在 VM 侧（`instance/wait.rs` 的分片轮询）。
            sleep_ns: Some(clock_sleep_ns),
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

/// `sleep_ns`：**有界**睡眠（`T-CP-10`；`CP-27` 的取消在 VM 侧分片循环里做）。
///
/// 契约：`ns > 0` 睡够再回；`ns == 0` 立即成功；`ns < 0` 是调用方错误 ⇒ 机器错误 `EINVAL`
/// （`CP-5` 的机器错误那一路，不伪装"未实现"）。本槽**不**自己无限等——VM 每次只给一片
/// （`instance/wait.rs` 的 `CANCEL_SLICE_NS` = 10 ms），所以中断延迟 ≤ 片长。
#[allow(unsafe_code)] // 逐项开许可（同上 ✓）
extern "C" fn clock_sleep_ns(_state: *mut c_void, ns: i64, errno_out: *mut i32) -> CapStatus {
    if ns < 0 {
        // 平台 errno 的**唯一真相**在 `pyawa-abi` 的注入表（`CM-20`：按名字查，不硬编码）。
        let errno = pyawa_abi::platform_errno::HOST_ERRNO
            .iter()
            .find(|(name, _)| *name == "EINVAL")
            .map(|(_, value)| *value as i32)
            .unwrap_or(0);
        // SAFETY: 出参由调用方提供（`CP-5` 的契约）。
        unsafe { *errno_out = errno };
        return CapStatus::Machine;
    }
    if ns > 0 {
        std::thread::sleep(std::time::Duration::from_nanos(ns as u64));
    }
    CapStatus::Ok
}
