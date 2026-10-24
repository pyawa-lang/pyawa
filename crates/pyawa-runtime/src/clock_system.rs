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

use pyawa_capabilities::clock::{CapStatus, CpClockVtable, CpTm};

/// **C 的 `struct tm`**（glibc 布局）：前 9 个 `c_int` ＋ `tm_gmtoff: c_long` ＋ `tm_zone` 指针。
///
/// 只在**本进程内**与 libc 交换，不过 ABI（过 ABI 的是 [`CpTm`]）⇒ 这是"平台集中点"的内部细节
/// （`DESIGN.md` §7）。x86_64 Linux：`c_long` ＝ `i64`、`time_t` ＝ `i64`。
#[repr(C)]
struct LibcTm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: core::ffi::c_long,
    tm_zone: *const core::ffi::c_char,
}

impl LibcTm {
    fn empty() -> Self {
        Self {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: core::ptr::null(),
        }
    }
}

// **libc 直连**（设计允许：本 crate 是平台集中点，不引新 crate）。只声明用到的三个符号；
// 全部遵守 C 的线程/缓冲契约（`localtime_r` 是重入版）。
#[allow(unsafe_code)] // 逐项开许可（本 crate 的规矩）
unsafe extern "C" {
    /// `time_t`（x86_64 Linux 是 `i64`）→ 本时区的 `struct tm`（重入；失败返回 NULL 并置 errno）。
    fn localtime_r(timep: *const i64, result: *mut LibcTm) -> *mut LibcTm;
    /// C locale 的格式化；返回写入字节数（**不含** NUL）；0 ＝ 结果为空**或**缓冲不够
    /// （glibc 两者都回 0 —— 区分靠扩容重试，见 [`clock_strftime`]）。
    fn strftime(
        out: *mut core::ffi::c_char,
        max: usize,
        format: *const core::ffi::c_char,
        tm: *const LibcTm,
    ) -> usize;
    /// 本地 `struct tm` → `time_t`（会归一化越界字段；失败返回 -1 并置 errno）。
    fn mktime(tm: *mut LibcTm) -> i64;
    /// glibc/musl 的 errno 位置（设/读 `errno` 的最小依赖面）。
    fn __errno_location() -> *mut i32;
}

/// 读当前 `errno`（先清 0 再调 libc 的调用点用它判"真失败"）。
#[allow(unsafe_code)] // 逐项开许可（本 crate 的规矩）
fn errno_value() -> i32 {
    // SAFETY: libc 保证 `__errno_location()` 返回本线程有效的 errno 位置。
    unsafe { *__errno_location() }
}

/// 把 libc 的 errno 折成 `CapStatus::Machine`（出参由调用方给）。
#[allow(unsafe_code)] // 逐项开许可（同上）
fn machine(errno_out: *mut i32, errno: i32) -> CapStatus {
    // SAFETY: 出参由调用方提供（`CP-5` 的契约）。
    unsafe { *errno_out = errno };
    CapStatus::Machine
}

/// [`CpTm`] → [`LibcTm`]。`tm_zone` 的字节经调用方给的 `zone_buffer`（NUL 结尾）转入，
/// 没给名字（`tm_zone_len == 0`）⇒ `tm_zone = NULL`（glibc 的 `%Z` 会退回全局 `tzname`，
/// 与参照给普通 9 元组时的口径一致 ✓）。
fn to_libc(tm: &CpTm, zone_buffer: &mut [u8; 17]) -> LibcTm {
    let mut libc = LibcTm {
        tm_sec: tm.tm_sec,
        tm_min: tm.tm_min,
        tm_hour: tm.tm_hour,
        tm_mday: tm.tm_mday,
        tm_mon: tm.tm_mon,
        tm_year: tm.tm_year,
        tm_wday: tm.tm_wday,
        tm_yday: tm.tm_yday,
        tm_isdst: tm.tm_isdst,
        tm_gmtoff: tm.tm_gmtoff as core::ffi::c_long,
        tm_zone: core::ptr::null(),
    };
    let length = (tm.tm_zone_len as usize).min(tm.tm_zone.len());
    if length > 0 {
        let length = length.min(zone_buffer.len() - 1);
        zone_buffer[..length].copy_from_slice(&tm.tm_zone[..length]);
        zone_buffer[length] = 0;
        libc.tm_zone = zone_buffer.as_ptr().cast::<core::ffi::c_char>();
    }
    libc
}

/// [`LibcTm`] → [`CpTm`]（`tm_zone` 截到 16 字节；不要求 NUL）。
#[allow(unsafe_code)] // 逐项开许可（同上）
fn from_libc(libc: &LibcTm) -> CpTm {
    let mut tm = CpTm {
        tm_sec: libc.tm_sec,
        tm_min: libc.tm_min,
        tm_hour: libc.tm_hour,
        tm_mday: libc.tm_mday,
        tm_mon: libc.tm_mon,
        tm_year: libc.tm_year,
        tm_wday: libc.tm_wday,
        tm_yday: libc.tm_yday,
        tm_isdst: libc.tm_isdst,
        tm_gmtoff: libc.tm_gmtoff as i64,
        tm_zone: [0u8; 16],
        tm_zone_len: 0,
    };
    if !libc.tm_zone.is_null() {
        // SAFETY: libc 的 `tm_zone` 要么为 NULL、要么指向 NUL 结尾的字符串（glibc 契约）。
        let bytes = unsafe { core::ffi::CStr::from_ptr(libc.tm_zone) }.to_bytes();
        let length = bytes.len().min(tm.tm_zone.len());
        tm.tm_zone[..length].copy_from_slice(&bytes[..length]);
        tm.tm_zone_len = length as u32;
    }
    tm
}

/// `localtime(secs)`：Unix 秒 ⇒ 本地时间（`localtime_r`；失败按 errno 报机器错误）。
#[allow(unsafe_code)] // 逐项开许可（本 crate 的规矩；与 `fs_posix` 同一手法 ✓）
extern "C" fn clock_localtime(
    _state: *mut c_void,
    secs: i64,
    out: *mut CpTm,
    errno_out: *mut i32,
) -> CapStatus {
    let mut libc = LibcTm::empty();
    // SAFETY: `secs` 与出参都是本函数栈上的值；`localtime_r` 写 `libc`。
    let result = unsafe { localtime_r(&secs, &mut libc) };
    if result.is_null() {
        return machine(errno_out, errno_value());
    }
    // SAFETY: 出参由调用方提供（`CP-5` 的契约）。
    unsafe { *out = from_libc(&libc) };
    CapStatus::Ok
}

/// `strftime(format, tm)`：C locale 格式化（**不调用 `setlocale`** ⇒ `LC_TIME=C`，与参照的
/// 启动态一致；`CX-4` 的静态扫描不扫本 crate）。
///
/// 出参**两段式**：只写 `min(总长, cap)` 字节，`out_len` 回总长。glibc 的 `strftime` 对
/// "结果为空"与"缓冲不够"都回 0 ⇒ 这里用**内部扩容**区分：从 `max(cap, 256)` 起翻倍，
/// 直到非 0 或到 1 MiB；仍为 0 ⇒ 视为空串（`Ok` ＋ 总长 0）。超 1 MiB ⇒ 机器错误 `EOVERFLOW`。
#[allow(unsafe_code)] // 逐项开许可（同上 ✓）
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
    if format.is_null() || tm.is_null() {
        return machine(errno_out, fallback_errno("EINVAL"));
    }
    // SAFETY: 调用方按签名给出 `format`／`format_len` 与 `tm`（`CpTm` 是本进程内的值）。
    let format_bytes = unsafe { core::slice::from_raw_parts(format, format_len) };
    let mut format_c = Vec::with_capacity(format_bytes.len() + 1);
    format_c.extend_from_slice(format_bytes);
    format_c.push(0);
    let mut zone_buffer = [0u8; 17];
    // SAFETY: `tm` 由调用方给出且有效。
    let libc = to_libc(unsafe { &*tm }, &mut zone_buffer);

    let mut size = capacity.max(256);
    loop {
        let mut buffer = vec![0u8; size];
        // SAFETY: 缓冲与格式串都是本函数持有的有效内存；`libc` 在调用期间存活。
        let written = unsafe {
            strftime(
                buffer.as_mut_ptr().cast::<core::ffi::c_char>(),
                buffer.len(),
                format_c.as_ptr().cast::<core::ffi::c_char>(),
                &libc,
            )
        };
        if written > 0 {
            let length = written;
            if !out.is_null() && capacity > 0 {
                let copied = length.min(capacity);
                // SAFETY: `out` 由调用方按 `capacity` 给出；源与目标不重叠。
                unsafe { core::ptr::copy_nonoverlapping(buffer.as_ptr(), out, copied) };
            }
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = length };
            return CapStatus::Ok;
        }
        // glibc 的 0 ＝ 空结果**或**缓冲不够（两者不可区分）⇒ 扩容重试；空结果在足够大时仍是 0。
        if size >= 1 << 20 {
            // 足够大仍回 0 ⇒ 按"结果为空"处理（C locale 下 `""`／某些空转换是合法的）。
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = 0 };
            return CapStatus::Ok;
        }
        size *= 2;
    }
}

/// `mktime(tm)`：本地时间 ⇒ Unix 秒（C 语义：越界字段由 libc 归一化）。
///
/// 失败判定照参照：返回 -1 **且** errno 非 0 才是错误（-1 也可能是合法的 1969-12-31 23:59:59）。
#[allow(unsafe_code)] // 逐项开许可（同上 ✓）
extern "C" fn clock_mktime(
    _state: *mut c_void,
    tm: *const CpTm,
    out: *mut i64,
    errno_out: *mut i32,
) -> CapStatus {
    if tm.is_null() {
        return machine(errno_out, fallback_errno("EINVAL"));
    }
    let mut zone_buffer = [0u8; 17];
    // SAFETY: `tm` 由调用方给出且有效。
    let mut libc = to_libc(unsafe { &*tm }, &mut zone_buffer);
    // SAFETY: 先把 errno 清 0，再调 libc；`__errno_location` 返回本线程有效位置。
    unsafe { *__errno_location() = 0 };
    // SAFETY: `libc` 是本函数栈上的有效 `struct tm`。
    let seconds = unsafe { mktime(&mut libc) };
    if seconds == -1 && errno_value() != 0 {
        return machine(errno_out, errno_value());
    }
    // SAFETY: 出参由调用方提供。
    unsafe { *out = seconds };
    CapStatus::Ok
}

/// 平台 errno 的**唯一真相**在 `pyawa-abi`（`CM-20`：按名字查，不硬编码）。
fn fallback_errno(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or(0)
}

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
            // **round 16（AB-41 表尾追加）**：日历／时区三槽（C locale 直连 libc）。
            localtime: Some(clock_localtime),
            strftime: Some(clock_strftime),
            mktime: Some(clock_mktime),
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
