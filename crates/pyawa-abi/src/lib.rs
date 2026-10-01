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
//! **实例生命周期**（`AB-55`／`AB-56`／`AB-57`）也已落地：`pa_create` 经**出参**交回实例
//! （创建那一刻还没有栈，`AB-49` 的"经栈"在此不适用）、ABI 不匹配时**仍交出诊断实例**
//! （只有 `pa_errmsg`／`pa_destroy` 可用）、`pa_destroy` **释放实例本身**。
//!
//! **尚未落地**：`§15` 的其余函数（执行／栈／值转换／宿主注册／能力注册），它们都还缺
//! 各自的下一步（栈线格式、签名元数据 `AB-51`…`AB-54` 等）。

use core::ffi::{c_char, c_void};
use core::mem::size_of;
use core::ptr::NonNull;
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};

use pyawa_core::{DictObject, FloatObject, Header, Instance, IntObject, ListObject, StrObject};

pub mod stack;

pub use stack::tag;
use stack::{tag_of, truthy, VirtualStack};

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

// ---- 实例生命周期（`AB-55`／`AB-56`／`AB-57`）----

/// 实例句柄（`AB-14`：宿主只见**不透明**指针；头部与类型对象**禁止**出现在签名里）。
///
/// 名字照 C 侧（`AB-45` 的单一头文件里就写作 `pa_state`），故这里显式关掉命名检查。
#[allow(non_camel_case_types)]
///
/// 字段是本 crate 私有的；宿主拿到的只是 `*mut pa_state`。
pub struct pa_state {
    /// 被驱动的实例（`OM-15`：类型注册表与对象堆都按实例存放）。
    instance: Instance,
    /// **`AB-9`／`AB-13`**：每实例的虚拟栈。
    stack: VirtualStack,
    /// 上一次 `pa_tostring` 之类"借用视图"的落点（宿主禁止在后续调用之后继续使用，`AB-48`）。
    view: Option<Vec<u8>>,
    /// **`AB-56`**：诊断实例——ABI 不匹配时交出的那个，只有 `pa_errmsg`／`pa_destroy` 可用。
    diagnostic: bool,
    /// **`AB-48`**：错误信息**归属实例**，保留到下一次可能改写它的调用；`pa_errmsg` 返回借用。
    message: Option<CString>,
}

impl pa_state {
    /// 造一个新实例（`AB-55`：创建经出参交回，不接触栈）。
    fn new() -> Self {
        Self {
            instance: Instance::new(),
            stack: VirtualStack::new(),
            view: None,
            diagnostic: false,
            message: None,
        }
    }

    /// 造一个**诊断实例**（`AB-56`）：`reason` 是给宿主看的可诊断信息（`T-AB-4`）。
    fn diagnostic(reason: String) -> Self {
        Self {
            instance: Instance::new(),
            stack: VirtualStack::new(),
            view: None,
            diagnostic: true,
            // CString 只在内含 NUL 时失败；诊断串是自己拼的，不会含 NUL
            message: CString::new(reason).ok(),
        }
    }
}

/// `pa_create(const pa_host *host, pa_state **out)`：创建实例（`AB-55`）。
///
/// 返回状态码；实例经 `*out` 交回。**ABI 不匹配时**（`AB-40`／`AB-41`）返回 `PA_ERR_ABI`，
/// 同时交出一个**诊断实例**（`AB-56`：只有 `pa_errmsg`／`pa_destroy` 可用）。
///
/// # Safety
///
/// `out` 必须是可写的 `pa_state *` 槽；`host` 要么是 `NULL`，要么指向宿主编译时的 `pa_host`
/// （`AB-43`：本函数只按 `min(宿主声明尺寸, 自身尺寸)` 读取）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_create(host: *const pa_host, out: *mut *mut pa_state) -> i32 {
    boundary(|| {
        if out.is_null() {
            return status::PA_ERR_INVALID;
        }
        // SAFETY: 由调用方保证 out 可写。
        unsafe { *out = core::ptr::null_mut() };
        if host.is_null() {
            // 宿主没传配置：这是宿主用法错误（不是 ABI 不兼容）
            return status::PA_ERR_INVALID;
        }
        // SAFETY: 由调用方保证 host 指向宿主编译时的 pa_host。
        let view = unsafe { view_host(host) };
        let compatible = matches!(view.abi_version, Some(version) if version_compatible(version));
        if !compatible {
            let host_version = view.abi_version.map(version_string).unwrap_or_else(|| {
                // 宿主声明的尺寸连版本字段都盖不住（`AB-43`：禁止越界读，故只能如实说）
                format!("未知（宿主只声明了 {} 字节）", view.abi_size.unwrap_or(0))
            });
            let reason = format!(
                "ABI 版本不兼容：宿主 {host_version}、运行时 {}（主 {}）",
                version_string(PA_ABI_VERSION),
                PA_ABI_MAJOR
            );
            let state = Box::new(pa_state::diagnostic(reason));
            // SAFETY: 由调用方保证 out 可写。
            unsafe { *out = Box::into_raw(state) };
            return status::PA_ERR_ABI;
        }
        // 能力接口实现（`AB-8`）：本层只记住它，域的注册/查询在 §15 的其余函数里
        let state = Box::new(pa_state::new());
        // SAFETY: 同上。
        unsafe { *out = Box::into_raw(state) };
        status::PA_OK
    })
}

/// `pa_destroy(pa_state *state)`：销毁实例（`AB-57`：**释放实例本身**）。
///
/// 此后 `pa_state *` **不可用**（禁止解引用或复用），实例内全部句柄同时失效（`AB-18`）。
/// 宿主**必须**在 `pa_create` 交出实例时（哪怕状态码是 `PA_ERR_ABI`）调用它（`AB-56`）。
///
/// # Safety
///
/// `state` 必须是 `pa_create` 交回、且**尚未**销毁的指针（或 `NULL`）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_destroy(state: *mut pa_state) -> i32 {
    boundary(|| {
        if state.is_null() {
            return status::PA_ERR_INVALID;
        }
        // SAFETY: 由调用方保证这是 pa_create 交回且尚未销毁的指针。
        drop(unsafe { Box::from_raw(state) });
        status::PA_OK
    })
}

/// `pa_interrupt(pa_state *state)`：请求中断；执行类函数随即返回 `PA_ERR_INTERRUPT`
/// （`AB-5`①；中断状态**按实例**存，`CX-3`）。
///
/// # Safety
///
/// `state` 必须是 `pa_create` 交回且尚未销毁的指针。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_interrupt(state: *mut pa_state) -> i32 {
    boundary(|| {
        let Some(state) = (unsafe { state.as_mut() }) else {
            return status::PA_ERR_INVALID;
        };
        if state.diagnostic {
            // `AB-56`：诊断实例只有 pa_errmsg／pa_destroy 可用
            return status::PA_ERR_ABI;
        }
        state.instance.request_interrupt();
        status::PA_OK
    })
}

/// `pa_errmsg(pa_state *state)`：取错误信息（**借用**；`AB-48`：宿主禁止在后续 API 调用之后
/// 继续使用它）。没有错误信息时返回 `NULL`。
///
/// # Safety
///
/// `state` 必须是 `pa_create` 交回且尚未销毁的指针。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_errmsg(state: *mut pa_state) -> *const c_char {
    // 这里不用 `boundary`：返回值是指针而不是状态码（`§15` 的栈契约是 `—`）
    let Some(state) = (unsafe { state.as_ref() }) else {
        return core::ptr::null();
    };
    match &state.message {
        Some(message) => message.as_ptr(),
        None => core::ptr::null(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `AB-5`①：中断请求要真的落到**那个实例**上（`CX-3`：按实例存，不用全局）。
    #[test]
    fn interrupt_reaches_the_instance() {
        let host = pa_host {
            abi_size: size_of::<pa_host>(),
            abi_version: PA_ABI_VERSION,
            capabilities: core::ptr::null(),
        };
        let mut state: *mut pa_state = core::ptr::null_mut();
        // SAFETY: 两个指针都是本测试的局部变量。
        assert_eq!(unsafe { pa_create(&host, &mut state) }, status::PA_OK);
        // SAFETY: state 由 pa_create 交回且尚未销毁；测试里直接看内部字段。
        let state_ref = unsafe { &*state };
        assert!(!state_ref.instance.interrupted(), "刚创建时不该是中断态");
        // SAFETY: 同上。
        assert_eq!(unsafe { pa_interrupt(state) }, status::PA_OK);
        // SAFETY: 同上。
        assert!(unsafe { &*state }.instance.interrupted(), "中断状态在该实例上");
        // 另一个实例不受影响（CX-3）
        let mut other: *mut pa_state = core::ptr::null_mut();
        // SAFETY: 同上。
        assert_eq!(unsafe { pa_create(&host, &mut other) }, status::PA_OK);
        // SAFETY: 同上。
        assert!(!unsafe { &*other }.instance.interrupted());
        // SAFETY: 两个都尚未销毁。
        unsafe {
            pa_destroy(state);
            pa_destroy(other);
        }
    }
}

// ---- 虚拟栈与值转换（§15.3 的一组；栈规则见 `stack.rs` 引的 `AB-9`…`AB-13`）----

/// 取状态；诊断实例（`AB-56`）除 `pa_errmsg`／`pa_destroy` 外一律 `PA_ERR_ABI`。
macro_rules! state_or {
    ($state:expr) => {{
        let Some(state) = (unsafe { $state.as_mut() }) else {
            return status::PA_ERR_INVALID;
        };
        if state.diagnostic {
            return status::PA_ERR_ABI;
        }
        state
    }};
}

/// `pa_gettop(st)`：当前栈深。
///
/// # Safety
///
/// `state` 必须是 `pa_create` 交回且尚未销毁的指针。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_gettop(state: *mut pa_state) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        state.stack.len() as i32
    })
}

/// `pa_settop(st, n)`：设置栈深（±；越界 `PA_ERR_INVALID`，`AB-12`）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_settop(state: *mut pa_state, count: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        if count < 0 {
            return status::PA_ERR_INVALID;
        }
        let count = count as usize;
        if count < state.stack.len() {
            for slot in state.stack.truncate(count) {
                if slot.owned {
                    // SAFETY: 该引用由栈持有。
                    unsafe { state.instance.release_object(slot.object.as_ptr()) };
                }
            }
            return status::PA_OK;
        }
        let nil = state.instance.singletons().none();
        state.stack.grow_to(count, nil)
    })
}

/// `pa_pushvalue(st, idx)`：压入栈上某项的副本（持有一个引用，`AB-10`）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pushvalue(state: *mut pa_state, index: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(slot) = state.stack.get(index) else {
            return status::PA_ERR_INVALID;
        };
        // SAFETY: 该槽位持有／借用一份存活引用。
        unsafe { state.instance.incref_object(slot.object.as_ptr()) };
        state.stack.push_owned(slot.object)
    })
}

/// `pa_pop(st, n)`：弹出并释放（`AB-10`／`OM-20`）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pop(state: *mut pa_state, count: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        if count < 0 || count as usize > state.stack.len() {
            return status::PA_ERR_INVALID;
        }
        for _ in 0..count {
            if let Some(slot) = state.stack.pop_slot() {
                if slot.owned {
                    // SAFETY: 该引用由栈持有。
                    unsafe { state.instance.release_object(slot.object.as_ptr()) };
                }
            }
        }
        status::PA_OK
    })
}

/// `pa_type(st, idx)`：类型标签（取值见 [`stack::tag`]）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_type(state: *mut pa_state, index: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        match state.stack.get(index) {
            Some(slot) => tag_of(&state.instance, slot.object),
            None => status::PA_ERR_INVALID,
        }
    })
}

/// 判定类函数（`pa_is*`）共用：命中给 1，否则 0；索引非法给 `PA_ERR_INVALID`。
unsafe fn is_tag(state: *mut pa_state, index: i32, wanted: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        match state.stack.get(index) {
            Some(slot) => i32::from(tag_of(&state.instance, slot.object) == wanted),
            None => status::PA_ERR_INVALID,
        }
    })
}

/// `pa_isnil(st, idx)`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_isnil(state: *mut pa_state, index: i32) -> i32 {
    unsafe { is_tag(state, index, tag::PA_TNIL) }
}

/// `pa_isboolean(st, idx)`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_isboolean(state: *mut pa_state, index: i32) -> i32 {
    unsafe { is_tag(state, index, tag::PA_TBOOLEAN) }
}

/// `pa_isinteger(st, idx)`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_isinteger(state: *mut pa_state, index: i32) -> i32 {
    unsafe { is_tag(state, index, tag::PA_TINTEGER) }
}

/// `pa_isnumber(st, idx)`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_isnumber(state: *mut pa_state, index: i32) -> i32 {
    unsafe { is_tag(state, index, tag::PA_TNUMBER) }
}

/// `pa_isstring(st, idx)`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_isstring(state: *mut pa_state, index: i32) -> i32 {
    unsafe { is_tag(state, index, tag::PA_TSTRING) }
}

/// `pa_istable(st, idx)`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_istable(state: *mut pa_state, index: i32) -> i32 {
    unsafe { is_tag(state, index, tag::PA_TTABLE) }
}

/// `pa_isfunction(st, idx)`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_isfunction(state: *mut pa_state, index: i32) -> i32 {
    unsafe { is_tag(state, index, tag::PA_TFUNCTION) }
}

/// `pa_pushnil(st)`：压入 `None`（+1）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pushnil(state: *mut pa_state) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let nil = state.instance.singletons().none();
        // SAFETY: 单例由实例持有；栈要自己那份。
        unsafe { state.instance.incref_object(nil.as_ptr()) };
        state.stack.push_owned(nil)
    })
}

/// `pa_pushboolean(st, b)`：压入布尔（+1）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pushboolean(state: *mut pa_state, value: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let flag = state.instance.singletons().boolean(value != 0);
        // SAFETY: 单例由实例持有。
        unsafe { state.instance.incref_object(flag.as_ptr()) };
        state.stack.push_owned(flag)
    })
}

/// `pa_pushinteger(st, i)`：压入整数（+1）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pushinteger(state: *mut pa_state, value: i64) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let object = state.instance.new_int(value);
        state.stack.push_owned(object)
    })
}

/// `pa_pushnumber(st, d)`：压入浮点（+1）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pushnumber(state: *mut pa_state, value: f64) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let float_type = match state.instance.type_named("float") {
            Some(ty) => ty,
            None => return status::PA_ERR_RUNTIME,
        };
        let object = state
            .instance
            .alloc(FloatObject::new(float_type, value))
            .into_raw()
            .cast::<Header>();
        state.stack.push_owned(object)
    })
}

/// `pa_pushstring(st, s, len)`：压入字符串（**复制**语义；`len < 0` 时按 NUL 结尾算）。
///
/// # Safety
///
/// `s` 要么是 `NULL`（且 `len <= 0`），要么指向 `len` 字节可读（`len < 0` 时须 NUL 结尾）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pushstring(
    state: *mut pa_state,
    text: *const c_char,
    len: isize,
) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let bytes: &[u8] = if text.is_null() {
            if len > 0 {
                return status::PA_ERR_INVALID;
            }
            &[]
        } else if len < 0 {
            // SAFETY: 调用方保证 NUL 结尾。
            unsafe { core::ffi::CStr::from_ptr(text) }.to_bytes()
        } else {
            // SAFETY: 调用方保证 len 字节可读。
            unsafe { core::slice::from_raw_parts(text.cast::<u8>(), len as usize) }
        };
        let owned = match String::from_utf8(bytes.to_vec()) {
            Ok(text) => text,
            Err(_) => return status::PA_ERR_INVALID,
        };
        let object = state.instance.new_str(&owned);
        state.stack.push_owned(object)
    })
}

/// `pa_pushbytes(st, p, len)`：压入字节串——**字节串类型尚未落地**（`TS-42` 排在 M3+）⇒
/// 如实返回 `PA_ERR_NOTIMPLEMENTED`（`AB-22`："未提供"与"已实现但拒绝"必须区分）。
///
/// # Safety
///
/// 同 [`pa_pushstring`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pushbytes(
    _state: *mut pa_state,
    _bytes: *const c_char,
    _len: isize,
) -> i32 {
    status::PA_ERR_NOTIMPLEMENTED
}

/// `pa_pushhandle(st, h)`：压入已有对象句柄（不透明，`AB-14`）——**新增一份引用**。
///
/// # Safety
///
/// `handle` 必须是本实例此前通过 ABI 取得的、仍然有效的句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pushhandle(state: *mut pa_state, handle: *mut c_void) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(object) = NonNull::new(handle.cast::<Header>()) else {
            return status::PA_ERR_INVALID;
        };
        // SAFETY: 调用方保证句柄有效。
        unsafe { state.instance.incref_object(object.as_ptr()) };
        state.stack.push_owned(object)
    })
}

/// `pa_newhandle(st, kind)`：新建宿主对象句柄——**宿主对象尚未接线**（`OM-34`…`OM-37`）⇒
/// 如实返回 `PA_ERR_NOTIMPLEMENTED`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_newhandle(_state: *mut pa_state, _kind: i32) -> i32 {
    status::PA_ERR_NOTIMPLEMENTED
}

/// `pa_toboolean(st, idx)`：真值转换（给 0／1）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_toboolean(state: *mut pa_state, index: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        match state.stack.get(index) {
            Some(slot) => i32::from(truthy(&state.instance, slot.object)),
            None => status::PA_ERR_INVALID,
        }
    })
}

/// `pa_tointeger(st, idx)`：整数转换；失败 `PA_ERR_INVALID`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_tointeger(state: *mut pa_state, index: i32, out: *mut i64) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        if out.is_null() {
            return status::PA_ERR_INVALID;
        }
        let Some(slot) = state.stack.get(index) else {
            return status::PA_ERR_INVALID;
        };
        let value = match tag_of(&state.instance, slot.object) {
            tag::PA_TINTEGER => {
                // SAFETY: 类型身份已确认。
                unsafe { &*slot.object.as_ptr().cast::<IntObject>() }.value
            }
            tag::PA_TBOOLEAN => {
                // SAFETY: 同上。
                i64::from(unsafe { &*slot.object.as_ptr().cast::<pyawa_core::BoolObject>() }.value)
            }
            tag::PA_TNUMBER => {
                // SAFETY: 同上。
                unsafe { &*slot.object.as_ptr().cast::<FloatObject>() }.value as i64
            }
            _ => return status::PA_ERR_INVALID,
        };
        // SAFETY: 调用方保证 out 可写。
        unsafe { *out = value };
        status::PA_OK
    })
}

/// `pa_tonumber(st, idx)`：浮点转换；失败 `PA_ERR_INVALID`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_tonumber(state: *mut pa_state, index: i32, out: *mut f64) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        if out.is_null() {
            return status::PA_ERR_INVALID;
        }
        let Some(slot) = state.stack.get(index) else {
            return status::PA_ERR_INVALID;
        };
        let value = match tag_of(&state.instance, slot.object) {
            tag::PA_TNUMBER => {
                // SAFETY: 类型身份已确认。
                unsafe { &*slot.object.as_ptr().cast::<FloatObject>() }.value
            }
            tag::PA_TINTEGER => {
                // SAFETY: 同上。
                unsafe { &*slot.object.as_ptr().cast::<IntObject>() }.value as f64
            }
            _ => return status::PA_ERR_INVALID,
        };
        // SAFETY: 调用方保证 out 可写。
        unsafe { *out = value };
        status::PA_OK
    })
}

/// `pa_tostring(st, idx, len*)`：取只读视图（**借用**，`AB-15`／`AB-48`）。
///
/// 返回的指针在**下一次可能改写它的调用**之前有效（本实现把它放在状态的 `view` 里）。
///
/// # Safety
///
/// `len` 可为 `NULL`；否则须可写。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_tostring(
    state: *mut pa_state,
    index: i32,
    len: *mut usize,
) -> *const c_char {
    let Some(state) = (unsafe { state.as_mut() }) else {
        return core::ptr::null();
    };
    if state.diagnostic {
        return core::ptr::null();
    }
    let Some(slot) = state.stack.get(index) else {
        return core::ptr::null();
    };
    if tag_of(&state.instance, slot.object) != tag::PA_TSTRING {
        return core::ptr::null();
    }
    // SAFETY: 类型身份已确认。
    let text = unsafe { &*slot.object.as_ptr().cast::<StrObject>() }.value().to_owned();
    if !len.is_null() {
        // SAFETY: 调用方保证 len 可写。
        unsafe { *len = text.len() };
    }
    state.view = Some(text.into_bytes());
    match &state.view {
        Some(bytes) => bytes.as_ptr().cast::<c_char>(),
        None => core::ptr::null(),
    }
}

/// `pa_tobytes(st, idx, len*)`：字节串类型尚未落地 ⇒ `NULL`（配合 `pa_pushbytes` 的
/// `PA_ERR_NOTIMPLEMENTED`）。
///
/// # Safety
///
/// 同 [`pa_tostring`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_tobytes(
    _state: *mut pa_state,
    _index: i32,
    _len: *mut usize,
) -> *const c_char {
    core::ptr::null()
}

/// `pa_newtable(st)`：新建表（本层就是 `dict`）（+1）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_newtable(state: *mut pa_state) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let dict_type = match state.instance.type_named("dict") {
            Some(ty) => ty,
            None => return status::PA_ERR_RUNTIME,
        };
        let object = state
            .instance
            .alloc(DictObject::new(dict_type, core::cell::RefCell::new(Vec::new())))
            .into_raw()
            .cast::<Header>();
        state.stack.push_owned(object)
    })
}

/// `pa_newlist(st, n)`：新建长度 `n` 的列表（元素为 `None`）（+1）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_newlist(state: *mut pa_state, length: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        if length < 0 {
            return status::PA_ERR_INVALID;
        }
        let list_type = match state.instance.type_named("list") {
            Some(ty) => ty,
            None => return status::PA_ERR_RUNTIME,
        };
        let nil = state.instance.singletons().none();
        let mut items: Vec<NonNull<Header>> = Vec::with_capacity(length as usize);
        for _ in 0..length {
            // SAFETY: 单例由实例持有；列表要自己那份。
            unsafe { state.instance.incref_object(nil.as_ptr()) };
            items.push(nil);
        }
        let object = state
            .instance
            .alloc(ListObject::new(list_type, core::cell::RefCell::new(items)))
            .into_raw()
            .cast::<Header>();
        state.stack.push_owned(object)
    })
}

/// `pa_retain(st, idx)`：借用 → 持有（`AB-15`）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_retain(state: *mut pa_state, index: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(slot) = state.stack.get(index) else {
            return status::PA_ERR_INVALID;
        };
        if slot.owned {
            return status::PA_OK;
        }
        // SAFETY: 该槽位借用着一份存活引用。
        unsafe { state.instance.incref_object(slot.object.as_ptr()) };
        let _ = state.stack.mark_owned(index);
        status::PA_OK
    })
}

/// `pa_release(st, idx)`：持有 → 释放（`AB-15`）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_release(state: *mut pa_state, index: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(was_owned) = state.stack.mark_borrowed(index) else {
            return status::PA_ERR_INVALID;
        };
        if was_owned {
            let Some(slot) = state.stack.get(index) else {
                return status::PA_ERR_INVALID;
            };
            // SAFETY: 该引用由栈持有，刚转成借用 ⇒ 这里归还。
            unsafe { state.instance.release_object(slot.object.as_ptr()) };
        }
        status::PA_OK
    })
}
