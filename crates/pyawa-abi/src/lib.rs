//! Pyawa 稳定 C ABI：栈式线格式、不透明句柄、`catch_unwind` 边界。
//!
//! 函数清单归属 `docs/SPEC-c-abi.md`（`AB-`），见本 crate 的 `README.md`。
//!
//! 本 crate 是工作区内唯一预期需要 `unsafe` 的地方（FFI 边界）。
//!
//! # 本文件已落地的范围
//!
//! 只有**不依赖任何待裁口径**的那几件（`AB-19`／`AB-20` 的状态码、`AB-39`…`AB-45` 的版本策略、
//! `AB-43` 的**有界读取**、`AB-3`／`CX-11` 的 panic 边界，以及版本三件套
//! `pa_version`／`pa_abi_version`／`pa_abi_size`）。
//!
//! **尚未落地**（各有原因，逐条写明）：
//!
//! - `pa_create`：§15 只写 `pa_create(const pa_host *)`、栈契约 `—`，而 `AB-49` 要求
//!   "返回值不经状态码传递、经栈传递"、`AB-13` 又把栈绑在实例上 ⇒ **实例经哪条路交回宿主**
//!   这一处口径待裁（连同 `T-AB-4` 的诊断信息落到哪、`pa_destroy` 之后 state 指针本身
//!   是释放还是仅失效）
//! - `pa_state`／`pa_destroy`／`pa_interrupt`：要实例句柄，落在上面那条口径之后
//! - 其余函数：`§15` 的清单已齐，但都建立在 `pa_create` 之上

use core::ffi::{c_char, c_void};
use core::mem::size_of;
use std::panic::{catch_unwind, AssertUnwindSafe};

// ---- 状态码（`AB-19`／`AB-20`）----

/// `AB-19`：每个函数**必须**返回状态码；详细信息经实例查询，**禁止**用全局错误变量。
///
/// `AB-20`：既有取值**禁止**改变含义，新增**必须**追加到预留区（8…31）。
/// 这份表与 `docs/SPEC-c-abi.md` §15.2 逐项对应，改动前先动规格。
pub mod status {
    /// 成功。
    pub const PA_OK: i32 = 0;
    /// 脚本异常；信息经 `pa_errmsg` 取回（`AB-21`）。
    pub const PA_ERR_RUNTIME: i32 = 1;
    /// 编译期错误。
    pub const PA_ERR_SYNTAX: i32 = 2;
    /// 分配失败。
    pub const PA_ERR_MEMORY: i32 = 3;
    /// 被 `pa_interrupt` 中断（`AB-5`①）。
    pub const PA_ERR_INTERRUPT: i32 = 4;
    /// 该嵌入**未提供**所要求的能力槽位（`CP-5`）——**必须**与"已实现但拒绝"区分（`AB-22`）。
    pub const PA_ERR_NOTIMPLEMENTED: i32 = 5;
    /// 宿主用法错误（栈越界、类型不符、句柄失效等）。
    pub const PA_ERR_INVALID: i32 = 6;
    /// ABI 版本或尺寸不兼容（`AB-40`）。
    pub const PA_ERR_ABI: i32 = 7;
    /// 预留区起点（`AB-20`：新增状态码**必须**落在 `8..=31`）。
    pub const PA_ERR_RESERVED_FIRST: i32 = 8;
    /// 预留区终点。
    pub const PA_ERR_RESERVED_LAST: i32 = 31;
}

// ---- 版本策略（`AB-39`…`AB-45`）----

/// ABI 主版本（`AB-41`：改既有槽位语义／删函数／改结构体字段形状 ⇒ 主版本 +1，旧宿主 `create` 必须失败）。
pub const PA_ABI_MAJOR: u32 = 1;
/// ABI 次版本（`AB-41`：**末尾追加**函数 ⇒ 次版本 +1，旧宿主仍可用）。
pub const PA_ABI_MINOR: u32 = 0;
/// `AB-45` 的宏 `PA_ABI_VERSION` 的取值：**主版本在高 16 位**（编码由本实现定，写进头文件）。
pub const PA_ABI_VERSION: u32 = (PA_ABI_MAJOR << 16) | PA_ABI_MINOR;

/// `AB-39`：嵌入 API ＋ 能力接口 ＋ 宿主对象契约**共用**这一个版本号（它也是 `CP-30` 的版本字段）。
pub fn version_string(version: u32) -> String {
    format!("{}.{}", version >> 16, version & 0xFFFF)
}

/// 版本兼容判定（`AB-41` 的"允许的改动"矩阵）：**主版本相同**即可用；
/// 次版本差异只意味着函数表在表尾多了几项（`AB-44`）。
pub fn version_compatible(host_version: u32) -> bool {
    (host_version >> 16) == PA_ABI_MAJOR
}

/// 版本不兼容时的**可诊断**信息（`AB-40`：**禁止**静默降级）。
///
/// 这里只负责"造出这份信息"；它**经由哪条路**交回宿主（`pa_errmsg` 还是别处）
/// 属于上面那条待裁口径。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VersionMismatch {
    /// 宿主编译时的版本号。
    pub host_version: u32,
    /// 本运行时自己的版本号。
    pub runtime_version: u32,
}

impl VersionMismatch {
    /// 人类可读的诊断串（含两侧版本，`T-AB-4`）。
    pub fn message(&self) -> String {
        format!(
            "ABI 版本不兼容：宿主 {}（主 {}）、运行时 {}（主 {}）",
            version_string(self.host_version),
            self.host_version >> 16,
            version_string(self.runtime_version),
            self.runtime_version >> 16
        )
    }
}

// ---- 宿主结构（`AB-8`／`AB-43`）----

/// 宿主在 `create` 时传来的结构（`AB-8`：含**能力接口实现**；`AB-43`：含 `(abi_size, abi_version)`）。
///
/// `abi_size` 放在**偏移 0**：`AB-43` 要求运行时以 `min(宿主 size, 自身 size)` 为界读取、
/// **禁止**越界读——所以"宿主声明的尺寸"必须能在读到任何其它字段之前先读到。
///
/// 能力接口 vtable 的**形状**归属 `docs/SPEC-capabilities.md`（`CP-`）；本层只存一个不透明指针
/// （`AB-32`：本 ABI 只提供注册入口）。
#[repr(C)]
pub struct pa_host {
    /// 宿主编译时**本结构体**的字节数（`AB-43` 的 `abi_size`）。
    pub abi_size: usize,
    /// `AB-39` 的共用版本号。
    pub abi_version: u32,
    /// 能力接口实现（形状引 `CP-`）；`NULL` ＝ 一个域都没提供（`CP-5` 的"未提供"）。
    pub capabilities: *const c_void,
}

/// `AB-45` 的宏 `PA_ABI_SIZE` 的取值：本结构体的字节数。
pub const PA_ABI_SIZE: usize = size_of::<pa_host>();

/// **有界读取**（`AB-43`）：从宿主编译时的结构里只读 `min(宿主声明尺寸, 自身尺寸)` 范围内的东西。
///
/// 读不到的字段是 `None`——**不是**默认值（`AB-40`：禁止静默降级）。
#[derive(Clone, Copy, Debug)]
pub struct HostView {
    /// 宿主声明的尺寸（读到 `abi_size` 字段本身才能拿到；太小则是 `None`）。
    pub abi_size: Option<usize>,
    /// 宿主的版本号（尺寸不够覆盖它则是 `None`）。
    pub abi_version: Option<u32>,
    /// 能力接口指针（尺寸不够覆盖它则是 `None`）。
    pub capabilities: Option<*const c_void>,
}

/// 读宿主结构（`AB-43`）。`host` 为 `NULL` 时全是 `None`。
///
/// # Safety
///
/// `host` 要么是 `NULL`，要么指向一块至少 `size_of::<usize>()` 字节可读的内存
/// （宿主编译时的 `pa_host`）。
pub unsafe fn view_host(host: *const pa_host) -> HostView {
    if host.is_null() {
        return HostView {
            abi_size: None,
            abi_version: None,
            capabilities: None,
        };
    }
    // SAFETY: 由调用方保证至少能读一个 usize（见 Safety 段）。
    let declared = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*host).abi_size)) };
    let limit = declared.min(PA_ABI_SIZE);
    let version_offset = core::mem::offset_of!(pa_host, abi_version);
    let capabilities_offset = core::mem::offset_of!(pa_host, capabilities);
    let abi_version = if limit >= version_offset + size_of::<u32>() {
        // SAFETY: 偏移落在宿主声明的尺寸内。
        Some(unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*host).abi_version)) })
    } else {
        None
    };
    let capabilities = if limit >= capabilities_offset + size_of::<*const c_void>() {
        // SAFETY: 同上。
        Some(unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*host).capabilities)) })
    } else {
        None
    };
    HostView {
        abi_size: Some(declared),
        abi_version,
        capabilities,
    }
}

// ---- panic 边界（`AB-3`／`CX-11`／`DESIGN.md` §3 不变量 3）----

/// 每个导出入口都要走的边界：**panic 绝不允许跨 FFI 边界**。
///
/// 被捕获时返回 `PA_ERR_RUNTIME`（`T-AB-2` 只要求"被捕获、返回状态码、进程不崩"）。
/// 更精确的映射（脚本异常 vs 内部缺陷）等错误信息归属定下来后再细分。
pub fn boundary<F>(body: F) -> i32
where
    F: FnOnce() -> i32,
{
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(status::PA_ERR_RUNTIME)
}

// ---- 版本三件套（§15.3 里唯一不依赖 `pa_create` 的三条）----

/// 版本字符串（静态、NUL 结尾，`§15` 的 `pa_version`）。
const VERSION_C: &[u8] = b"0.0.0\0";

/// `pa_version()`：Pyawa 版本字符串（静态）。
///
/// # Safety
///
/// 返回的指针指向**静态**内存，进程存活期内一直有效；返回类型是 `*const c_char`。
#[unsafe(no_mangle)]
pub extern "C" fn pa_version() -> *const c_char {
    VERSION_C.as_ptr().cast::<c_char>()
}

/// `pa_abi_version()`：ABI 版本号（`AB-45` 的 `PA_ABI_VERSION`）。
#[unsafe(no_mangle)]
pub extern "C" fn pa_abi_version() -> u32 {
    PA_ABI_VERSION
}

/// `pa_abi_size()`：函数表字节数（`AB-45` 的 `PA_ABI_SIZE`）。
#[unsafe(no_mangle)]
pub extern "C" fn pa_abi_size() -> usize {
    PA_ABI_SIZE
}
