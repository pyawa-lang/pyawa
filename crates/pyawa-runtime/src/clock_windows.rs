//! Windows `clock` 域的真实机器实现（`PLAN §12.20` W2）。
//!
//! - **`now_ns`**：`SystemTime`（Windows 上由 `GetSystemTimePreciseAsFileTime` 口径提供）。
//! - **`monotonic_ns`**：`Instant`（Windows 上由 **QPC** 提供；`PLAN` W2 的"QPC"落点）。
//! - **`sleep_ns`**：`std::thread::sleep`（VM 侧已把长等待切成 ≤10 ms 的有界片，`CP-27`）。
//!
//! **日历／时区三槽（`localtime`／`strftime`／`mktime`）真实现**（`PLAN §12.20` W2 补齐）：
//! 走 UCRT 的 `_localtime64_s`／`strftime`／`_mktime64`（与参照实现同源：Windows 本地时区由
//! 系统时区设置决定）；`tm_gmtoff`／`tm_zone` 由 `_get_timezone`／`_get_dstbias`／`_get_tzname`
//! 补（UCRT 的 `struct tm` 没有这两格 ⇒ 本 provider 自己填，`struct_time` 的 11 元组装载面要它们）。
//!
//! 本模块只在 `windows` 目标编译；宿主构建零影响。

use core::ffi::c_void;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pyawa_capabilities::clock::{CapStatus, CpClockVtable, CpTm};

/// UCRT 的 `struct tm`（与 [`CpTm`] 的**前九格**同序：本 provider 只借它做 C 调用）。
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WinTm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
}

#[allow(unsafe_code)]
mod ucrt {
    use super::WinTm;
    use core::ffi::{c_char, c_int, c_long};

    // MSVC 目标默认链接 ucrt（Rust std 的依赖链）⇒ 不写 `#[link]`。
    unsafe extern "C" {
        /// 本地时间（重入版；返回 errno_t，0 ＝ 成功）。
        pub fn _localtime64_s(result: *mut WinTm, time: *const i64) -> c_int;
        /// C locale 的格式化（返回写入字节数，0 ＝ 空结果或缓冲不够）。
        pub fn strftime(out: *mut c_char, max: usize, format: *const c_char, tm: *const WinTm) -> usize;
        /// 本地 `struct tm` ⇒ `time_t`（归一化越界字段；失败 -1 并置 errno）。
        pub fn _mktime64(tm: *mut WinTm) -> i64;
        /// 本线程 errno 位置。
        pub fn _errno() -> *mut c_int;
        /// 系统时区相对 UTC 的偏移（**秒，西为正**：UTC+8 ⇒ `-28800`）。
        pub fn _get_timezone(out: *mut c_long) -> c_int;
        /// DST 的附加偏移（秒，西为正）。
        pub fn _get_dstbias(out: *mut c_long) -> c_int;
        /// 时区名（index 0 ＝ 标准时、1 ＝ 夏令时；缓冲 NUL 结尾，返回值含 NUL 的长度）。
        pub fn _get_tzname(length: *mut usize, buffer: *mut c_char, size: usize, index: c_int) -> c_int;
    }
}

#[allow(unsafe_code)]
fn machine(errno_out: *mut i32, errno: i32) -> CapStatus {
    // SAFETY: 出参由调用方按 `CP-5` 提供。
    unsafe { *errno_out = errno };
    CapStatus::Machine
}

#[allow(unsafe_code)]
fn errno_value() -> i32 {
    // SAFETY: UCRT 保证 `_errno()` 返回本线程有效的 errno 位置。
    unsafe { *ucrt::_errno() }
}

fn win_tm_of(tm: &CpTm) -> WinTm {
    WinTm {
        tm_sec: tm.tm_sec,
        tm_min: tm.tm_min,
        tm_hour: tm.tm_hour,
        tm_mday: tm.tm_mday,
        tm_mon: tm.tm_mon,
        tm_year: tm.tm_year,
        tm_wday: tm.tm_wday,
        tm_yday: tm.tm_yday,
        tm_isdst: tm.tm_isdst,
    }
}

/// UCRT 的 `struct tm` ⇒ [`CpTm`]（补 `tm_gmtoff`／`tm_zone`）。
#[allow(unsafe_code)]
fn cp_tm_from(win: &WinTm) -> CpTm {
    let mut tm = CpTm {
        tm_sec: win.tm_sec,
        tm_min: win.tm_min,
        tm_hour: win.tm_hour,
        tm_mday: win.tm_mday,
        tm_mon: win.tm_mon,
        tm_year: win.tm_year,
        tm_wday: win.tm_wday,
        tm_yday: win.tm_yday,
        tm_isdst: win.tm_isdst,
        tm_gmtoff: 0,
        tm_zone: [0u8; 16],
        tm_zone_len: 0,
    };
    let mut timezone: core::ffi::c_long = 0;
    let mut dstbias: core::ffi::c_long = 0;
    // SAFETY: 出参是活局部量；UCRT 契约（`_get_timezone`／`_get_dstbias`）。
    unsafe {
        ucrt::_get_timezone(&mut timezone);
        ucrt::_get_dstbias(&mut dstbias);
    }
    let isdst = if win.tm_isdst > 0 { 1 } else { 0 };
    tm.tm_gmtoff = -(timezone as i64 + if isdst != 0 { dstbias as i64 } else { 0 });
    let mut buffer = [0u8; 64];
    let mut length: usize = 0;
    // SAFETY: 缓冲是活局部量、长度如实给出（`_get_tzname` 的契约）。
    let status = unsafe {
        ucrt::_get_tzname(
            &mut length,
            buffer.as_mut_ptr().cast::<core::ffi::c_char>(),
            buffer.len(),
            isdst,
        )
    };
    if status == 0 && length > 1 {
        let bytes = &buffer[..length - 1];
        let copied = bytes.len().min(tm.tm_zone.len());
        tm.tm_zone[..copied].copy_from_slice(&bytes[..copied]);
        tm.tm_zone_len = copied as u32;
    }
    tm
}

/// `localtime(secs)`：Unix 秒 ⇒ 本地时间（UCRT `_localtime64_s`；失败按 errno 报机器错误）。
#[allow(unsafe_code)]
extern "C" fn clock_localtime(
    _state: *mut c_void,
    secs: i64,
    out: *mut CpTm,
    errno_out: *mut i32,
) -> CapStatus {
    if out.is_null() {
        return machine(errno_out, 22);
    }
    let mut win = WinTm::default();
    // SAFETY: 出参是活局部量、`secs` 是活局部量。
    let status = unsafe { ucrt::_localtime64_s(&mut win, &secs) };
    if status != 0 {
        return machine(errno_out, errno_value());
    }
    // SAFETY: 出参由调用方提供。
    unsafe { *out = cp_tm_from(&win) };
    CapStatus::Ok
}

/// `strftime(format, tm)`：C locale 格式化（UCRT `strftime`；两段式出参，与 POSIX 同口径）。
#[allow(unsafe_code)]
extern "C" fn clock_strftime(
    _state: *mut c_void,
    format: *const u8,
    format_len: usize,
    tm: *const CpTm,
    out: *mut u8,
    capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if format.is_null() || tm.is_null() || out_len.is_null() {
        return machine(errno_out, 22);
    }
    // SAFETY: 调用方按签名给出 `format`／`format_len` 与 `tm`（本进程内的值）。
    let format_bytes = unsafe { core::slice::from_raw_parts(format, format_len) };
    let mut format_c = Vec::with_capacity(format_bytes.len() + 1);
    format_c.extend_from_slice(format_bytes);
    format_c.push(0);
    // SAFETY: `tm` 由调用方按 `CpTm` 给出。
    let win = win_tm_of(unsafe { &*tm });
    let mut size = capacity.max(256);
    loop {
        let mut buffer = vec![0u8; size];
        // SAFETY: buffer 至少 size 字节；format_c NUL 结尾；win 是活局部量。
        let written = unsafe {
            ucrt::strftime(
                buffer.as_mut_ptr().cast::<core::ffi::c_char>(),
                size,
                format_c.as_ptr().cast::<core::ffi::c_char>(),
                &win,
            )
        };
        if written > 0 {
            let copied = written.min(capacity);
            if copied > 0 {
                // SAFETY: `out` 有 `capacity` 字节，调用方保证（`CP-5`）。
                unsafe { core::ptr::copy_nonoverlapping(buffer.as_ptr(), out, copied) };
            }
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = written };
            return CapStatus::Ok;
        }
        if size >= 1024 * 1024 {
            return machine(errno_out, 75);
        }
        size *= 2;
    }
}

/// `mktime(tm)`：本地 `struct tm` ⇒ Unix 秒（UCRT `_mktime64`；会归一化越界字段）。
#[allow(unsafe_code)]
extern "C" fn clock_mktime(
    _state: *mut c_void,
    tm: *const CpTm,
    out: *mut i64,
    errno_out: *mut i32,
) -> CapStatus {
    if tm.is_null() || out.is_null() {
        return machine(errno_out, 22);
    }
    // SAFETY: `tm` 由调用方按 `CpTm` 给出。
    let mut win = win_tm_of(unsafe { &*tm });
    // SAFETY: `_mktime64` 只读写本局部量 `win`。
    let seconds = unsafe { ucrt::_mktime64(&mut win) };
    if seconds == -1 {
        let errno = errno_value();
        if errno != 0 {
            return machine(errno_out, errno);
        }
    }
    // SAFETY: 出参由调用方提供。
    unsafe { *out = seconds };
    CapStatus::Ok
}

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
            localtime: Some(clock_localtime),
            strftime: Some(clock_strftime),
            mktime: Some(clock_mktime),
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
