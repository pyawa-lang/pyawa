//! **`event` 能力域**（`ADR-005`／`CP-40`…`CP-46`；S3 追加域）。
//!
//! 宿主事件源：`wait` 等待就绪/完成事件，`notify`/`cancel` 为可选槽。形状归属
//! `docs/SPEC-capabilities.md` §9.10；本文件只给 `repr(C)` 布局（`CP-41`）。

use crate::fs::CapStatus;
use core::ffi::c_void;

/// `event.wait` 的一项：`key` 是 provider 自有不透明键，`events`／`revents` 用 POSIX `poll` 位。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpEventEntry {
    /// provider 自有的不透明键（VM 不解释）。
    pub key: u64,
    /// 关心的事件位（POSIX `poll` 规范值）。
    pub events: i32,
    /// 本次就绪的事件位。
    pub revents: i32,
}

/// `event` 域的 vtable（`CP-41`：公共头 ＋ 表尾追加槽；未实现槽为 `None`）。
#[repr(C)]
pub struct CpEventVtable {
    /// `CP-30`／`AB-39`：版本字段（provider 填 `PA_ABI_VERSION`）。
    pub version: u32,
    /// 本结构体的字节数（`AB-43` 有界读取）。
    pub size: u32,
    /// provider 实例状态（不透明）。
    pub state: *mut c_void,
    /// `wait(entries, timeout_ns)` ⇒ 两段式就绪结果。
    pub wait: Option<
        extern "C" fn(
            *mut c_void,
            *const CpEventEntry,
            usize,
            i64,
            *mut CpEventEntry,
            usize,
            *mut usize,
            *mut i32,
        ) -> CapStatus,
    >,
    /// `notify(key, events)`（可选）。
    pub notify: Option<extern "C" fn(*mut c_void, u64, i32, *mut i32) -> CapStatus>,
    /// `cancel(key)`（可选）。
    pub cancel: Option<extern "C" fn(*mut c_void, u64, *mut i32) -> CapStatus>,
}

impl CpEventVtable {
    /// 整域未实现（所有槽为 `None`；`CP-2`／`CP-3`）。
    pub const UNIMPLEMENTED: Self = Self {
        version: 0,
        size: core::mem::size_of::<Self>() as u32,
        state: core::ptr::null_mut(),
        wait: None,
        notify: None,
        cancel: None,
    };
}
