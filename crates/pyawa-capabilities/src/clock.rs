//! `clock` 域的形状 —— `SPEC-capabilities.md` §4 表里排第四（`CP-`）。
//!
//! 本文件**只含类型与函数指针**（`CP-12`），真实机器实现在 `pyawa-runtime`（`DESIGN.md` §7 的平台集中点）。
//! 对应 C 层模块：`time`（§4 表）。
//!
//! 遵循的条文与 `fs` 域同一套：**`CP-2`**（vtable 指针为 `null` ⇒ 整域未实现）、
//! **`CP-3`**（槽位为 `None` ⇒ 只有该操作未实现）、**`CP-5`**（每次调用必须返回三态之一：
//! 成功／机器错误／未实现 —— **禁止**把"未实现"编码成某个 `errno`）、**`CP-30`**（带版本／尺寸字段）。
//!
//! **本轮（第 317 轮）落地**：`now_ns`（挂钟）与 `monotonic_ns`（单调钟）两格 ——
//! 上限诊断里 `ModuleNotFoundError: No module named 'time'` × **54** 个模块的唯一卡点。
//! `sleep`／`perf` 一类随后补（补法：**追加槽位**，不改既有语义 ✓）。

use core::ffi::c_void;

pub use super::fs::CapStatus;

/// **`struct tm` 的 C 布局替身**（`CP-30` 的**追加槽**：`AB-41` 允许"表尾追加 ⇒ 次版本 +1、
/// 旧宿主仍可用"）。
///
/// 字段顺序与前 9 个字段的语义**照 C 的 `struct tm`**（`tm_mon` 是 **0-based**、`tm_year` 是
/// **自 1900 起**）——这是 `localtime`／`strftime`／`mktime` 三槽的公共载荷。后三个字段是
/// 时区信息（`tm_gmtoff` 秒、`tm_zone` 名字的上限 16 字节 ＋ 实长）：
/// glibc 把它们放在 `struct tm` 尾部，本层显式化（不把 C 指针带过 ABI，`CP-10`／`AB-14`）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpTm {
    /// 秒（0–61；参照接受 61 ＝ 闰秒口径）。
    pub tm_sec: i32,
    /// 分（0–59）。
    pub tm_min: i32,
    /// 时（0–23）。
    pub tm_hour: i32,
    /// 日（1–31；`strftime`／`asctime` 把 0 归一到 1，见 stdlib 的转换）。
    pub tm_mday: i32,
    /// 月（**0-based**：0 ＝ 一月）。
    pub tm_mon: i32,
    /// 年（**自 1900 起**）。
    pub tm_year: i32,
    /// 星期（0 ＝ 周一）。
    pub tm_wday: i32,
    /// 一年中的第几天（0-based 还是 1-based 由调用方给的 tm 决定；C 是 0-based）。
    pub tm_yday: i32,
    /// 夏令时标志（`>0` 生效、`0` 不生效、`<0` 未知）。
    pub tm_isdst: i32,
    /// 相对 UTC 的偏移**秒**（东八区 ⇒ `28800`）。
    pub tm_gmtoff: i64,
    /// 时区名的字节（NUL 不要求；实长见 `tm_zone_len`）。全 0 ＝ 没有名字。
    pub tm_zone: [u8; 16],
    /// `tm_zone` 的**有效字节数**。
    pub tm_zone_len: u32,
}

/// `clock` 域的 vtable（`CP-30`：带版本／尺寸字段）。
///
/// **`Copy`**：与 [`super::fs::CpFsVtable`] 同一手法 —— 读出来的是**一份快照** ✓
/// （`state` 仍指回 provider ✓，借用纪律见 `AB-16`／`AB-17` ✓）。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CpClockVtable {
    /// `CP-30`：版本／尺寸字段（编码与校验时机归 `SPEC-c-abi.md`）。
    pub version: u32,
    /// 本结构体的字节数。
    pub size: u32,
    /// provider 实例状态（不透明）。
    pub state: *mut c_void,

    /// **挂钟**：自 Unix 纪元起的**纳秒**数（`time.time_ns()` 那一格）。
    ///
    /// 返回三态：`Ok` ⇒ `out` 有效；`Machine` ⇒ 机器错误（本域通常没有）；`Unimplemented` ⇒ 该槽位没有。
    pub now_ns: Option<extern "C" fn(*mut c_void, *mut i64) -> CapStatus>,
    /// **单调钟**：不受系统时间调整影响的纳秒数（`time.monotonic_ns()` 那一格）。
    pub monotonic_ns: Option<extern "C" fn(*mut c_void, *mut i64) -> CapStatus>,
    /// **睡眠**：睡够给定纳秒（`time.sleep()` 那一格）。
    pub sleep_ns: Option<extern "C" fn(*mut c_void, i64, *mut i32) -> CapStatus>,
    /// **`localtime(secs)`**（`AB-41` 表尾追加 ①）：Unix 秒 ⇒ 本地时间 [`CpTm`]。
    pub localtime: Option<extern "C" fn(*mut c_void, i64, *mut CpTm, *mut i32) -> CapStatus>,
    /// **`strftime(format, tm)`**（表尾追加 ②）：按 C locale 格式化。
    ///
    /// 出参**两段式**（与 `fs.readlink` 同款约定）：provider 只写 `min(结果总长, cap)` 字节，
    /// `out_len` 回**结果总长** ⇒ 总长超容量时调用方按新容量再要一次。
    pub strftime: Option<
        extern "C" fn(
            *mut c_void,
            *const u8,
            usize,
            *const CpTm,
            *mut u8,
            usize,
            *mut usize,
            *mut i32,
        ) -> CapStatus,
    >,
    /// **`mktime(tm)`**（表尾追加 ③）：本地时间 [`CpTm`] ⇒ Unix 秒（C 语义：会归一化越界字段）。
    pub mktime: Option<extern "C" fn(*mut c_void, *const CpTm, *mut i64, *mut i32) -> CapStatus>,
}

impl CpClockVtable {
    /// **整域未实现**的形态（`CP-2`／`CP-3`：槽位全 `None` ✓）—— 与
    /// [`super::fs::CpFsVtable::UNIMPLEMENTED`] 同一手法，供测试与宿主临时占位 ✓。
    ///
    /// 旧宿主（1.2）的 vtable 没有尾部三槽 ⇒ 消费方按"该操作未实现"处理（`CP-3`），
    /// 与 `AB-41` 的"旧宿主仍可用"一致 ✓。
    pub const UNIMPLEMENTED: Self = Self {
        version: 1,
        size: core::mem::size_of::<CpClockVtable>() as u32,
        state: core::ptr::null_mut(),
        now_ns: None,
        monotonic_ns: None,
        sleep_ns: None,
        localtime: None,
        strftime: None,
        mktime: None,
    };
}
