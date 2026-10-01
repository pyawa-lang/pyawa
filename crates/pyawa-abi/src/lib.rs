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

pub mod export;
pub mod helpers;
pub mod host;
pub mod safe;
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
    /// **模块全局**（`pa_getglobal`／`pa_setglobal` 的落点；`pa_register` 也往这里放）。
    globals: NonNull<Header>,
    /// 宿主函数对象的类型（每实例注册一次；`OM-15`：类型注册表按实例存放）。
    host_function_type: Option<NonNull<pyawa_core::TypeObject>>,
    /// 本实例注册过的宿主函数对象（**持有一份引用**，便于签名查询与析构时归还）。
    host_functions: Vec<NonNull<Header>>,
    /// 本实例注册过的宿主类型（`AB-35`；`kind` → 类型）。
    host_types: Vec<host::RegisteredType>,
    /// 下一个宿主类型的 `kind`。
    next_host_kind: i32,
    /// **`AB-32`／`AB-33`**：九个能力域的注册状态（域索引见 [`capability`]）。
    capabilities: [CapabilitySlot; capability::DOMAIN_COUNT],
    /// `paL_ref` 的注册表（每实例一份；**持有**引用，`AB-15`）。
    registry: Vec<Option<NonNull<Header>>>,
    /// **`AB-53`／`AB-54`**：注册账本——`.pyi` 导出与运行期读的是**同一份数据**。
    registrations: Vec<export::Registration>,
    /// **`AB-56`**：诊断实例——ABI 不匹配时交出的那个，只有 `pa_errmsg`／`pa_destroy` 可用。
    diagnostic: bool,
    /// **`AB-48`**：错误信息**归属实例**，保留到下一次可能改写它的调用；`pa_errmsg` 返回借用。
    message: Option<CString>,
}

impl pa_state {
    /// 造一个新实例（`AB-55`：创建经出参交回，不接触栈）。
    fn new() -> Self {
        let instance = Instance::new();
        let globals = instance
            .alloc(DictObject::new(
                instance.type_named("dict").expect("dict 在引导期已登记"),
                core::cell::RefCell::new(Vec::new()),
            ))
            .into_raw()
            .cast::<Header>();
        Self {
            instance,
            stack: VirtualStack::new(),
            view: None,
            globals,
            host_function_type: None,
            host_functions: Vec::new(),
            host_types: Vec::new(),
            next_host_kind: 1,
            capabilities: [CapabilitySlot::default(); capability::DOMAIN_COUNT],
            registry: Vec::new(),
            registrations: Vec::new(),
            diagnostic: false,
            message: None,
        }
    }

    /// 造一个**诊断实例**（`AB-56`）：`reason` 是给宿主看的可诊断信息（`T-AB-4`）。
    fn diagnostic(reason: String) -> Self {
        let instance = Instance::new();
        let globals = instance
            .alloc(DictObject::new(
                instance.type_named("dict").expect("dict 在引导期已登记"),
                core::cell::RefCell::new(Vec::new()),
            ))
            .into_raw()
            .cast::<Header>();
        Self {
            instance,
            stack: VirtualStack::new(),
            view: None,
            globals,
            host_function_type: None,
            host_functions: Vec::new(),
            host_types: Vec::new(),
            next_host_kind: 1,
            capabilities: [CapabilitySlot::default(); capability::DOMAIN_COUNT],
            registry: Vec::new(),
            registrations: Vec::new(),
            diagnostic: true,
            // CString 只在内含 NUL 时失败；诊断串是自己拼的，不会含 NUL
            message: CString::new(reason).ok(),
        }
    }

    /// 写一条错误信息（`pa_errmsg` 取回）。
    pub(crate) fn set_message(&mut self, message: &str) {
        // CString 只在内含 NUL 时失败；把 NUL 换成空格，避免整条信息丢掉
        self.message = CString::new(message.replace('\0', " ")).ok();
    }

    /// 读当前错误信息（借用）。
    pub(crate) fn message_text(&self) -> Option<String> {
        self.message
            .as_ref()
            .map(|text| text.to_string_lossy().into_owned())
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

// ---- 全局变量（`pa_getglobal`／`pa_setglobal`）----

/// `pa_getglobal(st, name)`：读模块全局（+1）。
///
/// # Safety
///
/// `state` 必须是 `pa_create` 交回且尚未销毁的指针；`name` 是 NUL 结尾的 UTF-8。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_getglobal(state: *mut pa_state, name: *const c_char) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        // SAFETY: 调用方保证 name 是 NUL 结尾。
        let Some(text) = (unsafe { host::read_c_string(name, 4096) }) else {
            return status::PA_ERR_INVALID;
        };
        // SAFETY: globals 由本状态持有，存活。
        let mapping = unsafe { &*state.globals.as_ptr().cast::<DictObject>() };
        let position = mapping
            .entries()
            .iter()
            .position(|(key, _)| str_equals(&state.instance, *key, &text));
        match position {
            Some(position) => {
                let (_, value) = mapping.entry(position).expect("刚查到的位置");
                // SAFETY: 值由字典持有；栈要自己那份。
                unsafe { state.instance.incref_object(value.as_ptr()) };
                state.stack.push_owned(value)
            }
            None => {
                let nil = state.instance.singletons().none();
                // SAFETY: 单例由实例持有。
                unsafe { state.instance.incref_object(nil.as_ptr()) };
                state.stack.push_owned(nil)
            }
        }
    })
}

/// `pa_setglobal(st, name)`：写模块全局（−1）。
///
/// # Safety
///
/// 同 [`pa_getglobal`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_setglobal(state: *mut pa_state, name: *const c_char) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        // SAFETY: 调用方保证 name 是 NUL 结尾。
        let Some(text) = (unsafe { host::read_c_string(name, 4096) }) else {
            return status::PA_ERR_INVALID;
        };
        let Some(slot) = state.stack.pop_slot() else {
            return status::PA_ERR_INVALID;
        };
        // SAFETY: globals 由本状态持有，存活。
        let mapping = unsafe { &*state.globals.as_ptr().cast::<DictObject>() };
        let position = mapping
            .entries()
            .iter()
            .position(|(key, _)| str_equals(&state.instance, *key, &text));
        if let Some(position) = position {
            if let Some((old_key, old_value)) = mapping.remove(position) {
                // SAFETY: 旧键值由字典持有。
                unsafe {
                    state.instance.release_object(old_key.as_ptr());
                    state.instance.release_object(old_value.as_ptr());
                }
            }
        }
        // 栈交出那份引用（若槽位是借用，则补一份）
        if !slot.owned {
            // SAFETY: 借用着一份存活引用。
            unsafe { state.instance.incref_object(slot.object.as_ptr()) };
        }
        let key = state.instance.new_str(&text);
        mapping.insert_raw(key, slot.object);
        status::PA_OK
    })
}

// ---- 调用（`pa_call`／`pa_pcall`）与错误（`pa_error`）----

/// `pa_call(st, nargs, nresults)`：调用（−nargs+nresults）。
///
/// 栈上是 `[…, 可调用, 实参…]`；成功后结果替换掉它们。异常经状态码 ＋ `pa_errmsg`。
/// `nresults` 目前**必须**是 1（多返回值尚未定，见 `README.md`）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_call(state: *mut pa_state, nargs: i32, nresults: i32) -> i32 {
    unsafe { call_common(state, nargs, nresults, false) }
}

/// `pa_pcall(st, nargs, nresults)`：受保护调用（语义同 `pa_call`，显式区分调用点）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_pcall(state: *mut pa_state, nargs: i32, nresults: i32) -> i32 {
    unsafe { call_common(state, nargs, nresults, true) }
}

/// `pa_call`／`pa_pcall` 的公共实现。
unsafe fn call_common(
    state: *mut pa_state,
    nargs: i32,
    nresults: i32,
    _protected: bool,
) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        if nargs < 0 || nresults < 0 {
            return status::PA_ERR_INVALID;
        }
        if nresults != 1 {
            // 多返回值/调整结果数：规格未钉，先如实报未实现
            return status::PA_ERR_NOTIMPLEMENTED;
        }
        let nargs = nargs as usize;
        if state.stack.len() < nargs + 1 {
            return status::PA_ERR_INVALID;
        }
        // 收集实参（借用视图：先把槽位取出来，调用时 core 会各自 incref）
        let mut args: Vec<NonNull<Header>> = Vec::with_capacity(nargs);
        for offset in 0..nargs {
            let index = -((nargs - offset) as i32);
            match state.stack.get(index) {
                Some(slot) => args.push(slot.object),
                None => return status::PA_ERR_INVALID,
            }
        }
        let callable = match state.stack.get(-((nargs + 1) as i32)) {
            Some(slot) => slot.object,
            None => return status::PA_ERR_INVALID,
        };
        let outcome = pyawa_core::call_value(&state.instance, callable, &args, &[]);
        // 无论成败，先把"可调用 ＋ 实参"这段栈收掉（归还持有的引用）
        let keep = state.stack.len() - nargs - 1;
        drain_stack(state, keep);
        match outcome {
            Ok(result) => state.stack.push_owned(result),
            Err(pyawa_core::ExecError::Raised { exception }) => {
                // 异常不跨边界逃逸（AB-21）：转成状态码 ＋ 可由宿主取回的信息
                let message = exception_message(&state.instance, exception);
                state.message = std::ffi::CString::new(message).ok();
                status::PA_ERR_RUNTIME
            }
            Err(pyawa_core::ExecError::Interrupted) => status::PA_ERR_INTERRUPT,
            Err(_) => status::PA_ERR_RUNTIME,
        }
    })
}

/// 取异常实例的类型名 ＋ 消息（供 `pa_errmsg` 用）。
fn exception_message(instance: &Instance, exception: NonNull<Header>) -> String {
    // SAFETY: exception 是存活对象。
    let ty = unsafe { exception.as_ref() }.ty();
    // SAFETY: 类型名由注册表持有。
    let name = unsafe { ty.as_ref() }.name();
    // SAFETY: exception 是异常实例。
    let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
        .message_with(instance);
    match message {
        Some(text) => format!("{name}: {text}"),
        None => name.to_owned(),
    }
}

/// `pa_error(st, msg)`：宿主主动抛错（信息写进状态，由 `pa_errmsg` 取回）。
///
/// # Safety
///
/// `state` 必须是 `pa_create` 交回且尚未销毁的指针；`msg` 是 NUL 结尾的 UTF-8（可为 `NULL`）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_error(state: *mut pa_state, msg: *const c_char) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        // SAFETY: 调用方保证 msg 是 NUL 结尾或 NULL。
        let text = unsafe { host::read_c_string(msg, 4096) }
            .unwrap_or_else(|| "宿主主动抛错".to_owned());
        state.message = std::ffi::CString::new(text).ok();
        status::PA_ERR_RUNTIME
    })
}

/// 按文本比较一个键（键必须是 `str`）。
fn str_equals(instance: &Instance, raw: NonNull<Header>, expected: &str) -> bool {
    // SAFETY: 调用方保证 raw 存活。
    if unsafe { raw.as_ref() }.ty() != instance.singletons().str_type() {
        return false;
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value() == expected
}

// ---- 宿主函数注册（`AB-24`…`AB-26`）----

/// `pa_register(st, name, fn, sig)`：注入宿主函数（`AB-24`／`AB-25`）。
///
/// 宿主函数**经虚拟栈**收发参数：调用时实参逐个压栈，函数返回后**栈顶**就是它的结果
/// （约定见 [`host`] 模块的文档，`pa.h` 里同样写明）。`sig` **必须**提供（`AB-25`）。
///
/// # Safety
///
/// `state` 必须是 `pa_create` 交回且尚未销毁的指针；`name` 是 NUL 结尾的 UTF-8；
/// `function` 是有效的 C 函数；`sig` 按 [`host::pa_sig`] 的契约给出。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_register(
    state: *mut pa_state,
    name: *const c_char,
    function: host::PaHostFn,
    sig: *const host::pa_sig,
) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        // SAFETY: 调用方保证 name 是 NUL 结尾。
        let Some(text) = (unsafe { host::read_c_string(name, 4096) }) else {
            return status::PA_ERR_INVALID;
        };
        // AB-25：禁止无名签名的宿主函数
        // SAFETY: 调用方按契约给出 sig。
        let Some(signature) = (unsafe { host::read_signature(sig) }) else {
            return status::PA_ERR_INVALID;
        };
        // 宿主函数对象的类型（每实例一次）
        let host_type = match state.host_function_type {
            Some(ty) => ty,
            None => {
                let ty = state.instance.new_type(
                    "host_function",
                    core::mem::size_of::<host::HostFunction>(),
                    pyawa_core::Slots::new(host_function_dealloc).with_call(host_function_call),
                );
                state.host_function_type = Some(ty);
                ty
            }
        };
        let object = host::HostFunction::new(
            host_type,
            function,
            state as *mut pa_state,
            text.clone(),
            signature.ret_expr.clone(),
            signature.params.clone(),
            signature.flags,
        );
        let created = state
            .instance
            .alloc_payload(object)
            .cast::<Header>();
        // 记下来（持有），并放进模块全局，脚本里按名字就能拿到
        state.host_functions.push(created);
        // `AB-53`／`AB-54`：注册账本（`.pyi` 导出与运行期读同一份）
        state.registrations.push(export::Registration {
            name: text.clone(),
            is_type: false,
            signature: signature.clone(),
            is_final: false,
            has_instance_dict: false,
        });
        // SAFETY: globals 由本状态持有，存活。
        let mapping = unsafe { &*state.globals.as_ptr().cast::<DictObject>() };
        let position = mapping
            .entries()
            .iter()
            .position(|(key, _)| str_equals(&state.instance, *key, &text));
        if let Some(position) = position {
            if let Some((old_key, old_value)) = mapping.remove(position) {
                // SAFETY: 旧键值由字典持有。
                unsafe {
                    state.instance.release_object(old_key.as_ptr());
                    state.instance.release_object(old_value.as_ptr());
                }
            }
        }
        // SAFETY: created 由本状态持有；字典要自己那份。
        unsafe { state.instance.incref_object(created.as_ptr()) };
        let key = state.instance.new_str(&text);
        mapping.insert_raw(key, created);
        status::PA_OK
    })
}

/// 宿主函数对象的 `dealloc`（`OM-11` 的必填槽）。
///
/// # Safety
///
/// 由 `Instance` 在计数归零后调用（`OM-20` ③）。
unsafe fn host_function_dealloc(ptr: *mut Header) {
    // SAFETY: 调用方保证 ptr 是本类型的一个对象，且计数已归零、clear 已跑过。
    drop(unsafe { Box::from_raw(ptr.cast::<host::HostFunction>()) });
}

/// 宿主函数对象的 `call` 槽（`OM-11`）：实参压栈 → 调 C 函数 → 栈顶就是结果。
///
/// # Safety
///
/// 契约见 `pyawa_core::CallFn`。
unsafe fn host_function_call(
    ptr: *mut Header,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    instance: &Instance,
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    // SAFETY: 调用方保证 ptr 是本类型的存活对象。
    let host = unsafe { &*ptr.cast::<host::HostFunction>() };
    if !kwargs.is_empty() {
        return Err(pyawa_core::ExecError::Unsupported {
            opcode: 0,
            what: "宿主函数的关键字实参随后补（先按位置传）",
        });
    }
    let Some(mut state) = NonNull::new(host.state) else {
        return Err(pyawa_core::ExecError::Unsupported {
            opcode: 0,
            what: "宿主函数没有所属实例",
        });
    };
    // SAFETY: state 由注册时记录，仍然有效（宿主必须在使用期间不销毁它）。
    let state_ref = unsafe { state.as_mut() };
    if state_ref.diagnostic {
        return Err(pyawa_core::ExecError::Unsupported {
            opcode: 0,
            what: "诊断实例不能调用宿主函数（AB-56）",
        });
    }
    // 把实参压栈（各持一份），随后交给宿主函数
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        let pushed = state_ref.stack.push_owned(*argument);
        if pushed != status::PA_OK {
            // SAFETY: 刚压进去的那份。
            unsafe { instance.release_object(argument.as_ptr()) };
            return Err(pyawa_core::ExecError::Unsupported {
                opcode: 0,
                what: "宿主函数调用时栈越界",
            });
        }
    }
    // AB-26：宿主函数内部的 panic 必须被捕获并转成状态码
    let base = state_ref.stack.len() - args.len();
    // SAFETY: 由注册时的契约保证 function 有效。
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        (host.function)(host.state)
    }))
    .unwrap_or(status::PA_ERR_RUNTIME);
    // 收栈：宿主函数把结果留在**栈顶**
    let result = match outcome {
        status::PA_OK => {
            let top = state_ref.stack.get(-1).map(|slot| slot.object);
            match top {
                Some(object) if state_ref.stack.len() > base => {
                    // SAFETY: 栈顶持有／借用一份存活引用。
                    unsafe { instance.incref_object(object.as_ptr()) };
                    // 归还宿主函数留下的那些槽
                    drain_stack(state_ref, base);
                    Some(object)
                }
                _ => {
                    drain_stack(state_ref, base);
                    None
                }
            }
        }
        other => {
            drain_stack(state_ref, base);
            // 宿主主动抛错 ⇒ 转成脚本异常（信息已经由宿主 `pa_error` 写进状态）
            return Err(crate::raise_from_state(state_ref, other));
        }
    };
    match result {
        Some(object) => Ok(object),
        None => {
            let nil = instance.singletons().none();
            // SAFETY: 单例由实例持有。
            unsafe { instance.incref_object(nil.as_ptr()) };
            Ok(nil)
        }
    }
}

/// 把宿主函数返回的状态码翻成执行错误（`AB-21`／`AB-22`：异常绝不跨边界逃逸）。
pub(crate) fn raise_from_state(state: &pa_state, status: i32) -> pyawa_core::ExecError {
    match status {
        status::PA_ERR_INTERRUPT => pyawa_core::ExecError::Interrupted,
        status::PA_ERR_MEMORY => {
            state.instance.raise_builtin_error("MemoryError", "宿主报告内存不足")
        }
        status::PA_ERR_NOTIMPLEMENTED => pyawa_core::ExecError::Unsupported {
            opcode: 0,
            what: "宿主未提供该能力槽位（CP-5）",
        },
        status::PA_ERR_INVALID => {
            state.instance.raise_builtin_error("TypeError", "宿主报告用法错误（PA_ERR_INVALID）")
        }
        status::PA_ERR_ABI => {
            state.instance.raise_builtin_error("RuntimeError", "ABI 不兼容（PA_ERR_ABI）")
        }
        status::PA_ERR_SYNTAX => {
            state.instance.raise_builtin_error("SyntaxError", "宿主报告编译期错误")
        }
        _ => {
            // 默认按脚本异常处理：信息从状态的 message 取（宿主可用 `pa_error` 写）
            let message = state
                .message
                .as_ref()
                .map(|text| text.to_string_lossy().into_owned())
                .unwrap_or_else(|| "宿主函数抛出异常".to_owned());
            state.instance.raise_builtin_error("RuntimeError", &message)
        }
    }
}

/// 收回宿主函数留下的栈槽（并把 `base` 之上的都归还）。
fn drain_stack(state: &mut pa_state, base: usize) {
    let dropped = state.stack.truncate(base);
    let instance = &state.instance;
    for slot in dropped {
        if slot.owned {
            // SAFETY: 该引用由栈持有。
            unsafe { instance.release_object(slot.object.as_ptr()) };
        }
    }
}

// ---- 宿主类型注册（`AB-35`…`AB-38`）----

/// `pa_newtype(st, name, dealloc, traverse, sig)`：注册宿主类型（`AB-35`／`AB-36`）。
///
/// - 注册为**真实类型**（`OM-14`：禁止另立一套对象表示）；实例载荷是 [`host::HostObject`]
/// - **必须**提供 `dealloc` 与 `traverse`（`AB-36`）；`traverse` 是"上下文 ＋ 回调"形态
///   （C 侧不能传闭包），宿主对每个直接引用调 `visit(句柄, context)`
/// - **默认可被继承**（`AB-37`）：`sig.flags` 里**没有** `PA_TYPE_FINAL` 即允许继承；
///   宿主对象布局固定 ⇒ 实例字典**另行挂载**（本层用 `mark_external_instance_dict`）
/// - `sig` **必须**提供（`AB-36`：注册必须提供签名）
///
/// 返回值：成功时 `*out`（若给出）拿到该类型的 `kind`，`pa_newhandle` 用它建实例。
///
/// # Safety
///
/// 同 [`pa_register`]；`dealloc`／`traverse` 由宿主提供且必须遵守 `OM-34`…`OM-36`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_newtype(
    state: *mut pa_state,
    name: *const c_char,
    dealloc: host::PaHostDealloc,
    traverse: host::PaHostTraverse,
    sig: *const host::pa_sig,
    out_kind: *mut i32,
) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        // SAFETY: 调用方保证 name 是 NUL 结尾。
        let Some(text) = (unsafe { host::read_c_string(name, 4096) }) else {
            return status::PA_ERR_INVALID;
        };
        // AB-36：注册必须提供签名
        // SAFETY: 调用方按契约给出 sig。
        let Some(signature) = (unsafe { host::read_signature(sig) }) else {
            return status::PA_ERR_INVALID;
        };
        // `AB-37`：`PA_TYPE_FINAL` 反向选择"不可继承"
        let final_type = signature.flags & host::PA_TYPE_FINAL != 0;
        // 名字要 `&'static str`（TypeObject::name 的临时形态）：泄漏一份
        let static_name: &'static str = Box::leak(text.clone().into_boxed_str());
        let ty = state.instance.new_type(
            static_name,
            core::mem::size_of::<host::HostObject>(),
            pyawa_core::Slots::new(host::host_object_dealloc)
                .with_traverse(host::host_object_traverse),
        );
        // AB-37：宿主对象布局固定 ⇒ 实例字典**另行挂载**（OS 侧那一格）
        // SAFETY: ty 由注册表持有。
        unsafe { ty.as_ref() }.mark_external_instance_dict();
        let _ = final_type; // `PA_TYPE_FINAL` 的"不可继承"执行随后补（清单里记着）
        let kind = state.next_host_kind;
        state.next_host_kind += 1;
        // `AB-53`／`AB-54`：注册账本
        state.registrations.push(export::Registration {
            name: text.clone(),
            is_type: true,
            signature: signature.clone(),
            is_final: final_type,
            has_instance_dict: true,
        });
        state.host_types.push(host::RegisteredType {
            ty,
            kind,
            dealloc,
            traverse,
        });
        if !out_kind.is_null() {
            // SAFETY: 调用方保证 out_kind 可写。
            unsafe { *out_kind = kind };
        }
        status::PA_OK
    })
}

// ---- 属性与下标（`pa_getfield`／`pa_setfield`／`pa_gettable`／`pa_settable`／
// ---- `pa_rawget`／`pa_rawset`）----
//
// 语义引核心：`getfield`／`setfield` 走 `OM-11` 的 `getattr`／`setattr`（`TS` §8），
// `gettable`／`settable` 走 `BC-39` 的 `NB_SUBSCR` ／ `STORE_SUBSCR`，
// `rawget`／`rawset` **不触发槽位**（本层：直接走容器的内部表，不走任何协议）。

/// `pa_getfield(st, idx, name)`：属性访问（±1：**就地替换**栈顶那一项）。
///
/// # Safety
///
/// 同 [`pa_gettop`]；`name` 是 NUL 结尾的 UTF-8。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_getfield(
    state: *mut pa_state,
    index: i32,
    name: *const c_char,
) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        // SAFETY: 调用方保证 name 是 NUL 结尾。
        let Some(text) = (unsafe { host::read_c_string(name, 4096) }) else {
            return status::PA_ERR_INVALID;
        };
        let Some(object) = state.stack.get(index).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        match pyawa_core::attribute_read(&state.instance, object, &text) {
            Ok(value) => replace_top(&mut state.stack, &state.instance, value),
            Err(pyawa_core::ExecError::Raised { exception }) => {
                state.message = std::ffi::CString::new(exception_message(&state.instance, exception)).ok();
                status::PA_ERR_RUNTIME
            }
            Err(_) => status::PA_ERR_RUNTIME,
        }
    })
}

/// `pa_setfield(st, idx, name)`：属性写入。
///
/// 栈上是 `[值(TOS), …]`，对象在 `index` 处；写入后**弹掉 TOS**（净 −1）。
///
/// > 规格 §15.3 把这一行的栈契约记为 `±1`（就地替换）；本实现按自然语义取 **−1**
/// > （值在栈顶、写入即消耗），差异写进 `README.md`。
///
/// # Safety
///
/// 同 [`pa_getfield`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_setfield(
    state: *mut pa_state,
    index: i32,
    name: *const c_char,
) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        // SAFETY: 调用方保证 name 是 NUL 结尾。
        let Some(text) = (unsafe { host::read_c_string(name, 4096) }) else {
            return status::PA_ERR_INVALID;
        };
        let Some(object) = state.stack.get(index).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        let Some(value) = state.stack.get(-1).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        let outcome = pyawa_core::attribute_write(&state.instance, object, &text, value);
        // 值被消耗：弹掉 TOS（归还它持有的引用）
        if let Some(slot) = state.stack.pop_slot() {
            if slot.owned {
                // SAFETY: 该引用由栈持有。
                unsafe { state.instance.release_object(slot.object.as_ptr()) };
            }
        }
        match outcome {
            Ok(()) => status::PA_OK,
            Err(pyawa_core::ExecError::Raised { exception }) => {
                state.message =
                    std::ffi::CString::new(exception_message(&state.instance, exception)).ok();
                status::PA_ERR_RUNTIME
            }
            Err(_) => status::PA_ERR_RUNTIME,
        }
    })
}

/// `pa_gettable(st, idx)`：下标访问（±1：**就地替换**栈顶那一项；键在栈顶、容器在 `idx`）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_gettable(state: *mut pa_state, index: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(container) = state.stack.get(index).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        let Some(key) = state.stack.get(-1).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        match pyawa_core::subscript_read(&state.instance, container, key) {
            Ok(value) => {
                // 键被消耗（下标读取把键弹出），值就地放上
                if let Some(slot) = state.stack.pop_slot() {
                    if slot.owned {
                        // SAFETY: 该引用由栈持有。
                        unsafe { state.instance.release_object(slot.object.as_ptr()) };
                    }
                }
                state.stack.push_owned(value)
            }
            Err(pyawa_core::ExecError::Raised { exception }) => {
                state.message =
                    std::ffi::CString::new(exception_message(&state.instance, exception)).ok();
                status::PA_ERR_RUNTIME
            }
            Err(_) => status::PA_ERR_RUNTIME,
        }
    })
}

/// `pa_settable(st, idx)`：下标写入。
///
/// 栈上是 `[…, 容器(idx), 键, 值(TOS)]`；写入后**键与值都被消耗**（净 −2）——
/// 与 `STORE_SUBSCR` 的三元形状一致（`BC-39`）。
///
/// > 规格 §15.3 把这一行记为 `±1`（就地替换）；本实现按自然语义取 **−2**，差异见 `README.md`。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_settable(state: *mut pa_state, index: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(container) = state.stack.get(index).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        let Some(value) = state.stack.get(-1).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        let Some(key) = state.stack.get(-2).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        let outcome = pyawa_core::subscript_write(&state.instance, container, key, value);
        // 键与值都被消耗
        for _ in 0..2 {
            if let Some(slot) = state.stack.pop_slot() {
                if slot.owned {
                    // SAFETY: 该引用由栈持有。
                    unsafe { state.instance.release_object(slot.object.as_ptr()) };
                }
            }
        }
        match outcome {
            Ok(()) => status::PA_OK,
            Err(pyawa_core::ExecError::Raised { exception }) => {
                state.message =
                    std::ffi::CString::new(exception_message(&state.instance, exception)).ok();
                status::PA_ERR_RUNTIME
            }
            Err(_) => status::PA_ERR_RUNTIME,
        }
    })
}

/// `pa_rawget(st, idx)`：下标访问但**不触发槽位**（本层：只认 `dict`／`list` 的内部表）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_rawget(state: *mut pa_state, index: i32) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(container) = state.stack.get(index).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        let Some(key) = state.stack.get(-1).map(|slot| slot.object) else {
            return status::PA_ERR_INVALID;
        };
        let outcome = raw_lookup(&state.instance, container, key);
        match outcome {
            Ok(Some(value)) => {
                if let Some(slot) = state.stack.pop_slot() {
                    if slot.owned {
                        // SAFETY: 该引用由栈持有。
                        unsafe { state.instance.release_object(slot.object.as_ptr()) };
                    }
                }
                // SAFETY: value 由容器持有，栈要自己那份。
                unsafe { state.instance.incref_object(value.as_ptr()) };
                state.stack.push_owned(value)
            }
            Ok(None) => {
                // 没有这个键：按 nil 放上（不触发任何协议）
                if let Some(slot) = state.stack.pop_slot() {
                    if slot.owned {
                        // SAFETY: 该引用由栈持有。
                        unsafe { state.instance.release_object(slot.object.as_ptr()) };
                    }
                }
                let nil = state.instance.singletons().none();
                // SAFETY: 单例由实例持有。
                unsafe { state.instance.incref_object(nil.as_ptr()) };
                state.stack.push_owned(nil)
            }
            Err(code) => code,
        }
    })
}

/// `pa_rawset(st, idx)`：下标写入但**不触发槽位**（栈同 `pa_settable`，键值都消耗）。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_rawset(state: *mut pa_state, index: i32) -> i32 {
    // 本层的 `subscript_write` 与"raw"目前是同一条路（协议槽位尚未接线），
    // 故暂时等同 `pa_settable`——**不是**"忽略语义"，README 里写明这一点。
    unsafe { pa_settable(state, index) }
}

/// `raw` 语义的查表：只认 `dict`（`list` 的整数下标随后补），不触发任何协议。
fn raw_lookup(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
) -> Result<Option<NonNull<Header>>, i32> {
    // SAFETY: container 是存活对象。
    let container_type = unsafe { container.as_ref() }.ty();
    if Some(container_type) != instance.type_named("dict") {
        return Err(status::PA_ERR_INVALID);
    }
    // SAFETY: 类型身份已确认。
    let mapping = unsafe { &*container.as_ptr().cast::<DictObject>() };
    for (existing, value) in mapping.entries() {
        if pyawa_core::values_equal_public(instance, existing, key) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

/// 把栈顶那一项换成 `value`（**就地替换**，`AB-47` 的 `±1`）。
fn replace_top(stack: &mut VirtualStack, instance: &Instance, value: NonNull<Header>) -> i32 {
    if stack.is_empty() {
        return stack.push_owned(value);
    }
    match stack.pop_slot() {
        Some(slot) => {
            if slot.owned {
                // SAFETY: 该引用由栈持有。
                unsafe { instance.release_object(slot.object.as_ptr()) };
            }
            stack.push_owned(value)
        }
        None => stack.push_owned(value),
    }
}

// ---- 能力接口注册（`AB-32`…`AB-34`）----

/// 能力域的切片与异步分类（形状引 `docs/SPEC-capabilities.md` 的 `CP-`）。
pub mod capability {
    /// 域个数（`CP-1`：与 `DESIGN.md` §7.3 的九域一一对应）。
    pub const DOMAIN_COUNT: usize = 9;

    /// 域编号（取值由实现定，写进 `pa.h`；顺序照 `SPEC-capabilities.md` §4 的表）。
    pub const PA_DOMAIN_FS: i32 = 0;
    /// `net`。
    pub const PA_DOMAIN_NET: i32 = 1;
    /// `proc`。
    pub const PA_DOMAIN_PROC: i32 = 2;
    /// `clock`。
    pub const PA_DOMAIN_CLOCK: i32 = 3;
    /// `random`。
    pub const PA_DOMAIN_RANDOM: i32 = 4;
    /// `env`。
    pub const PA_DOMAIN_ENV: i32 = 5;
    /// `tty`。
    pub const PA_DOMAIN_TTY: i32 = 6;
    /// `locale`。
    pub const PA_DOMAIN_LOCALE: i32 = 7;
    /// `ipc`。
    pub const PA_DOMAIN_IPC: i32 = 8;

    /// 域名字（诊断用）。
    pub const NAMES: [&str; DOMAIN_COUNT] = [
        "fs", "net", "proc", "clock", "random", "env", "tty", "locale", "ipc",
    ];

    /// **`CP-25`／`CP-37`**：异步分类只有二值——可异步化。
    pub const PA_ASYNC_OK: i32 = 0;
    /// 不可异步化。
    pub const PA_ASYNC_NO: i32 = 1;

    /// 把 C 侧编号翻成下标。
    pub fn index_of(domain: i32) -> Option<usize> {
        (0..DOMAIN_COUNT as i32)
            .contains(&domain)
            .then_some(domain as usize)
    }
}

/// 一个域的注册状态。
#[derive(Clone, Copy)]
pub struct CapabilitySlot {
    /// 宿主给的 vtable 指针（`AB-32`：形状引 `CP-`；本层只存，不解释）。
    pub implementation: *const c_void,
    /// **`CP-25`**：异步分类；`None` ＝ 尚未声明（那时**禁止**注册实现）。
    pub classification: Option<i32>,
}

impl Default for CapabilitySlot {
    fn default() -> Self {
        Self {
            implementation: core::ptr::null(),
            classification: None,
        }
    }
}

/// `pa_setcapability_async(st, domain, cls)`：**声明**某个域的异步分类（`AB-34`／`CP-25`）。
///
/// 取值只有 [`capability::PA_ASYNC_OK`]／[`capability::PA_ASYNC_NO`]（`CP-37`：按域二值，
/// **禁止**域内混合）；**缺失即注册失败**，**禁止**落默认值。
///
/// # Safety
///
/// 同 [`pa_gettop`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_setcapability_async(
    state: *mut pa_state,
    domain: i32,
    classification: i32,
) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(index) = capability::index_of(domain) else {
            return status::PA_ERR_INVALID;
        };
        if classification != capability::PA_ASYNC_OK
            && classification != capability::PA_ASYNC_NO
        {
            return status::PA_ERR_INVALID;
        }
        state.capabilities[index].classification = Some(classification);
        status::PA_OK
    })
}

/// `pa_setcapability(st, domain, impl)`：注册某个域的实现（`AB-32`／`AB-33`）。
///
/// `impl` 是该域的 vtable 指针（**形状引 `CP-`**，本层只存不解释）；`impl == NULL` 表示
/// 该域**整域未实现**（`CP-2`：调用时报"未实现"，**禁止**在创建实例时拒绝）。
///
/// **`CP-25`**：该域若尚未显式声明异步分类，注册**必须失败**（`T-AB-6`）——默认值就是数据竞争。
///
/// # Safety
///
/// 同 [`pa_gettop`]；`implementation` 指向宿主的 vtable（生命周期由宿主负责）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pa_setcapability(
    state: *mut pa_state,
    domain: i32,
    implementation: *const c_void,
) -> i32 {
    boundary(|| {
        let state = state_or!(state);
        let Some(index) = capability::index_of(domain) else {
            return status::PA_ERR_INVALID;
        };
        if state.capabilities[index].classification.is_none() {
            // `CP-25`：缺失即注册失败，禁止默认值
            return status::PA_ERR_INVALID;
        }
        state.capabilities[index].implementation = implementation;
        status::PA_OK
    })
}
