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
}

impl CpClockVtable {
    /// **整域未实现**的形态（`CP-2`／`CP-3`：槽位全 `None` ✓）—— 与
    /// [`super::fs::CpFsVtable::UNIMPLEMENTED`] 同一手法，供测试与宿主临时占位 ✓。
    pub const UNIMPLEMENTED: Self = Self {
        version: 1,
        size: core::mem::size_of::<CpClockVtable>() as u32,
        state: core::ptr::null_mut(),
        now_ns: None,
        monotonic_ns: None,
        sleep_ns: None,
    };
}
