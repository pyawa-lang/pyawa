//! Pyawa 独立运行时：真实机器能力实现与入口（REPL/CLI）。
//!
//! 工作区内平台依赖的**集中点**，见本 crate 的 `README.md`。

// `CX-4` 的静态扫描**排除**本 crate（`docs/CONSTRAINTS.md` §3.1："它**就是**平台依赖的集中点"）
// ⇒ 这里允许定点使用平台相关代码（FFI／libc），但**逐项**开许可、不许整文件放开。
#![deny(unsafe_code)]

use core::ffi::{c_void, c_char};
#[cfg(any(unix, windows))]
use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_CLOCK, PA_DOMAIN_FS, PA_DOMAIN_NET};
#[cfg(any(unix, windows))]
use pyawa_abi::{pa_setcapability, pa_setcapability_async};
use pyawa_abi::{pa_error, pa_host, pa_state, safe, status};

// **平台门控（`PLAN §12.21` E1）**：三个真实机器 provider 只用 POSIX／libc；
// wasm32（浏览器）没有 OS ⇒ 整域如实"未提供"（`CP-2`；由 JS 宿主在 E4 提供 `clock`），
// 不在此处伪造成功。非 unix 目标（wasm32-unknown-unknown）不编译这三个模块。
#[cfg(unix)]
pub mod clock_system;
#[cfg(unix)]
pub mod fs_posix;
#[cfg(unix)]
pub mod net_posix;
// `.pyac` 的落盘路径依赖 `std::fs` 与进程号 ⇒ 宿主侧专用，wasm 目标不编译（产物由宿主决定）。
#[cfg(not(target_family = "wasm"))]
pub mod pyac;
// **Windows 平台 provider（`PLAN §12.20` W2／W3）**：真实机器实现，只在 windows 目标编译。
#[cfg(windows)]
pub mod clock_windows;
#[cfg(windows)]
pub mod fs_windows;
#[cfg(windows)]
pub mod net_windows;
#[cfg(windows)]
pub mod win32_reg;
#[cfg(windows)]
pub mod win32_overlapped;
// **Web（WASM）E2**：固定槽位 trampoline ＋ JS 填充协议（`PLAN §12.21` E2）。
// 只在 wasm32 目标编译；宿主构建零影响。模块内放开 `unsafe_code`（FFI 入口与分配器，逐项有 SAFETY）。
#[cfg(target_family = "wasm")]
#[allow(unsafe_code)]
pub mod wasm_bridge;

// **平台 provider 的类型别名**（`PLAN §12.23` 选项 A）：CLI 只写一份源码——
// Unix 用 POSIX provider，Windows 用 Win32 provider；三者的 `new()`／`vtable()` 形状相同
// ⇒ CLI 不需要 `cfg` 分支，也不需要 `std::fs` 直读（`IM-15`）。
#[cfg(unix)]
pub type PlatformFs = fs_posix::PosixFs;
#[cfg(windows)]
pub type PlatformFs = fs_windows::WindowsFs;
#[cfg(unix)]
pub type PlatformClock = clock_system::SystemClock;
#[cfg(windows)]
pub type PlatformClock = clock_windows::WindowsClock;
#[cfg(unix)]
pub type PlatformNet = net_posix::PosixNet;
#[cfg(windows)]
pub type PlatformNet = net_windows::WindowsNet;

/// **组合根电池**（`§12.13 #1`／路线 A）：`paL_newstate` 用的真实机器 provider ＋ 它们的 vtable。
///
/// **生命周期（如实）**：C 便捷入口没有"实例销毁回调"可挂（`pa_destroy` 是 ABI 的），
/// provider／vtable 的指针由 `pa_setcapability` **原样保存**、必须在实例存活期内有效 ⇒
/// 这里**按实例泄漏**一份（进程生命周期；宿主需要精确回收时改用 `pa_create` ＋ 自己注册能力）。
/// 这是组合根便捷入口的边界，不是静默泄漏：见 `PLAN §12.13` 的"如实边界"。
#[cfg(unix)]
#[allow(unsafe_code)]
struct MachineProviders {
    fs: fs_posix::PosixFs,
    clock: clock_system::SystemClock,
    net: net_posix::PosixNet,
}

/// 泄漏到进程末的 vtable（`state` 指回 [`MachineProviders`] 的稳定地址）。
#[cfg(unix)]
struct MachineVtables {
    fs: pyawa_capabilities::fs::CpFsVtable,
    clock: pyawa_capabilities::clock::CpClockVtable,
    net: pyawa_capabilities::net::CpNetVtable,
}

/// 建电池并注册 `fs`／`clock`／`net` 三个真实机器能力域（每域先声明异步分类，`AB-34`）。
///
/// **生命周期**：provider 与 vtable 都**先放进稳定的 Box、再泄漏到进程末**；vtable 的 `state`
/// 必须指向**不会移动**的 provider（先前把 provider 在栈上建好再 move 进 Box ⇒ `state` 悬垂，
/// 实测 `pa_exec_file` 的 `fs_open` 段错误）。
#[cfg(unix)]
#[allow(unsafe_code)]
fn install_machine_capabilities(state: *mut pa_state) -> i32 {
    let providers = Box::into_raw(Box::new(MachineProviders {
        fs: fs_posix::PosixFs::new(),
        clock: clock_system::SystemClock::new(),
        net: net_posix::PosixNet::new(),
    }));
    // SAFETY: providers 已固定在堆上（泄漏到进程末）。
    let (fs_vtable, clock_vtable, net_vtable) = unsafe {
        ((*providers).fs.vtable(), (*providers).clock.vtable(), (*providers).net.vtable())
    };
    let vtables = Box::into_raw(Box::new(MachineVtables {
        fs: fs_vtable,
        clock: clock_vtable,
        net: net_vtable,
    }));
    // SAFETY: vtables／providers 都泄漏到进程末 ⇒ 指针在实例存活期内一直有效。
    let (tables, state_ptr) = unsafe { (&*vtables, state) };
    let fs_impl = core::ptr::addr_of!(tables.fs).cast::<c_void>();
    let clock_impl = core::ptr::addr_of!(tables.clock).cast::<c_void>();
    let net_impl = core::ptr::addr_of!(tables.net).cast::<c_void>();
    // SAFETY: 按 `AB-33`/`AB-34` 的契约传参。
    let checks = unsafe {
        [
            pa_setcapability_async(state_ptr, PA_DOMAIN_FS, PA_ASYNC_OK),
            pa_setcapability(state_ptr, PA_DOMAIN_FS, fs_impl),
            pa_setcapability_async(state_ptr, PA_DOMAIN_CLOCK, PA_ASYNC_OK),
            pa_setcapability(state_ptr, PA_DOMAIN_CLOCK, clock_impl),
            pa_setcapability_async(state_ptr, PA_DOMAIN_NET, PA_ASYNC_OK),
            pa_setcapability(state_ptr, PA_DOMAIN_NET, net_impl),
        ]
    };
    for code in checks {
        if code != status::PA_OK {
            return code;
        }
    }
    status::PA_OK
}

/// **非 unix**：没有 OS。wasm32 上由 **JS 宿主提供 `clock`**（`PLAN §12.21` E4）——
/// `fs`／`net` **不注册**（浏览器没有 ⇒ 如实"未提供"，`CP-2`）；其它非 unix（Windows）留 W3／W4。
#[cfg(not(unix))]
#[allow(unsafe_code)]
fn install_machine_capabilities(state: *mut pa_state) -> i32 {
    #[cfg(target_family = "wasm")]
    {
        return wasm_bridge::install_js_capabilities(state);
    }
    #[cfg(windows)]
    {
        return install_windows_capabilities(state);
    }
    #[cfg(all(not(target_family = "wasm"), not(windows)))]
    {
        let _ = state;
        status::PA_OK
    }
}


// --------------------------------------------------------------------------- //
// Windows 组合根（`PLAN §12.20` W2／W3）
// --------------------------------------------------------------------------- //

#[cfg(windows)]
struct WindowsProviders {
    fs: fs_windows::WindowsFs,
    clock: clock_windows::WindowsClock,
    net: net_windows::WindowsNet,
}

#[cfg(windows)]
struct WindowsVtables {
    fs: pyawa_capabilities::fs::CpFsVtable,
    clock: pyawa_capabilities::clock::CpClockVtable,
    net: pyawa_capabilities::net::CpNetVtable,
}

/// 注册 Windows 的 `fs`／`clock`／`net` 三个真实机器能力域（`PLAN §12.20` W2–W4）。
///
/// 生命周期与 unix 组合根同口径：provider／vtable 泄漏到进程末（实例存续期内指针有效）。
#[cfg(windows)]
#[allow(unsafe_code)]
fn install_windows_capabilities(state: *mut pa_state) -> i32 {
    let providers = Box::into_raw(Box::new(WindowsProviders {
        fs: fs_windows::WindowsFs::new(),
        clock: clock_windows::WindowsClock::new(),
        net: net_windows::WindowsNet::new(),
    }));
    // SAFETY: providers 已固定在堆上（泄漏到进程末）。
    let (fs_vtable, clock_vtable, net_vtable) = unsafe {
        ((*providers).fs.vtable(), (*providers).clock.vtable(), (*providers).net.vtable())
    };
    let vtables = Box::into_raw(Box::new(WindowsVtables {
        fs: fs_vtable,
        clock: clock_vtable,
        net: net_vtable,
    }));
    // SAFETY: vtables／providers 都泄漏到进程末 ⇒ 指针在实例存活期内一直有效。
    let (tables, state_ptr) = unsafe { (&*vtables, state) };
    let fs_impl = core::ptr::addr_of!(tables.fs).cast::<c_void>();
    let clock_impl = core::ptr::addr_of!(tables.clock).cast::<c_void>();
    let net_impl = core::ptr::addr_of!(tables.net).cast::<c_void>();
    // SAFETY: 按 `AB-33`/`AB-34` 的契约传参。
    let checks = unsafe {
        [
            pa_setcapability_async(state_ptr, PA_DOMAIN_FS, PA_ASYNC_OK),
            pa_setcapability(state_ptr, PA_DOMAIN_FS, fs_impl),
            pa_setcapability_async(state_ptr, PA_DOMAIN_CLOCK, PA_ASYNC_OK),
            pa_setcapability(state_ptr, PA_DOMAIN_CLOCK, clock_impl),
            pa_setcapability_async(state_ptr, PA_DOMAIN_NET, PA_ASYNC_OK),
            pa_setcapability(state_ptr, PA_DOMAIN_NET, net_impl),
        ]
    };
    for code in checks {
        if code != status::PA_OK {
            return code;
        }
    }
    status::PA_OK
}

/// 组合根装配：内建名字空间＋`sys`＋`Lib/` 搜索路径（`CM-14`／`CM-26`；`P3-14` 的临时代劳）。
#[allow(unsafe_code)]
fn install_library(state: *mut pa_state, program: &str) -> i32 {
    // SAFETY: state 由调用方保证存活。
    let instance = unsafe { &*state }.instance();
    pyawa_stdlib::install(instance, program, &[]);
    // **`PLAN §12.24` 的真实语义首批**：平台 FFI 留在本 crate（`CX-4`）⇒ 便利装配路径把
    // stdlib 的 `winreg` 诚实桩替换成真实 Win32 注册表实现（裸 ABI 宿主不注入，仍如实 NotImplemented）。
    #[cfg(windows)]
    win32_reg::install(instance);
    // **§12.24 真实语义第二批**：`_winapi`／`_overlapped` 的最小真实语义（事件／IOCP 生命周期；
    // 重叠 I/O 数据面如实 NotImplemented）。
    #[cfg(windows)]
    win32_overlapped::install(instance);
    status::PA_OK
}

/// 便捷入口的错误消息（不崩溃；`AB-48`）。
#[allow(unsafe_code)]
fn report(state: *mut pa_state, message: &str) {
    let Ok(text) = std::ffi::CString::new(message) else {
        return;
    };
    // SAFETY: state 由调用方保证存活；`pa_error` 只复制消息。
    unsafe {
        pa_error(state, text.as_ptr());
    }
}

/// **`§15.4`**：`paL_newstate()`——`pa_create` ＋ 真实机器 provider 的便捷入口（C 形态）。
///
/// 本实现先给出**没有能力实现**的实例（九域全部未注册 ⇒ 调用时报"未实现"，`CP-2`）；
/// 真实机器 provider（`DESIGN.md` §12 的 `M5`）接上之后，逐域注册的动作加在这里。
/// 创建失败时返回 `NULL`（诊断信息在诊断实例里，`AB-56`）。
///
/// # Safety
///
/// 交回的指针必须用 `pa_destroy` 释放，且不得跨线程共享（`OM-1` 的实例隔离）。
// 这是 C ABI 导出符号：`#[no_mangle]` 属于 unsafe 面，按 `CX-4` 的例外定点放开。
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn paL_newstate() -> *mut pa_state {
    let host = pa_host {
        abi_size: core::mem::size_of::<pa_host>(),
        abi_version: pyawa_abi::PA_ABI_VERSION,
        capabilities: core::ptr::null::<c_void>(),
    };
    let (state, code) = safe::create(&host);
    // 平台相关常量（`errno` 一族）**已在 `pa_create` 内注入**（task-20 ✓，一处真相：
    // 所有 ABI 宿主同源）⇒ 这里**不再**手动补。`DESIGN.md` §9 第 20 条／`CM-20`。
    if state.is_null() || !safe::is_usable(code) {
        // 诊断实例也交回（宿主可用 `pa_errmsg` 看原因，`AB-56`）；完全没建起来才给 NULL
        if state.is_null() {
            return core::ptr::null_mut();
        }
        return state;
    }
    // **`§12.13 #1`（路线 A）**：电池版 = 真实机器能力（fs／clock／net）＋ stdlib 装配。
    let capabilities = install_machine_capabilities(state);
    if capabilities != status::PA_OK {
        report(
            state,
            &format!("paL_newstate：真实机器能力注册失败（状态 {capabilities}）；实例仍可 pa_create 裸用"),
        );
        return state;
    }
    if install_library(state, "<embedded>") != status::PA_OK {
        report(state, "paL_newstate：stdlib 装配失败");
    }
    state
}

/// **`§15.4`**：`paL_openlibs(st)`——对**已有**实例装 builtins／`sys`（组合根实现；`§12.13 #1`）。
///
/// `pyawa-abi` 的最小产物**不**导出本符号（`SPEC-c-abi` §15.4：提供方＝组合根）。
/// 本函数**不**注册能力域（那由宿主自己按 `AB-33`／`AB-34` 决定）；`print` 之类的输出链
/// 需要在宿主注册 `fs` 域后才可用。失败（`state == NULL`）⇒ `PA_ERR_INVALID`。
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn paL_openlibs(state: *mut pa_state) -> i32 {
    if state.is_null() {
        return status::PA_ERR_INVALID;
    }
    // SAFETY: 调用方保证 state 是活着的实例（AB-57）。
    if unsafe { (*state).instance() }.builtins().is_none() {
        // 没有内建名空间 ⇒ 装一次；已装过（重复调用）⇒ 幂等成功（不重装第二份 builtins）。
        let code = install_library(state, "<embedded>");
        if code != status::PA_OK {
            report(state, "paL_openlibs：stdlib 装配失败");
        }
        return code;
    }
    status::PA_OK
}

/// **`§15.4`** 的 Rust 侧持有者：方便在 Rust 里用（`Drop` 时销毁）。
pub struct PaState {
    raw: *mut pa_state,
    /// 创建时的状态码（`PA_ERR_ABI` ⇒ 诊断实例，`AB-56`）。
    pub code: i32,
}

impl PaState {
    /// 创建一个实例（Rust 侧入口，等价于 [`paL_newstate`]）。
    pub fn new() -> Option<Self> {
        let host = pa_host {
            abi_size: core::mem::size_of::<pa_host>(),
            abi_version: pyawa_abi::PA_ABI_VERSION,
            capabilities: core::ptr::null::<c_void>(),
        };
        let (raw, code) = safe::create(&host);
        if raw.is_null() {
            return None;
        }
        // 平台常量由 `pa_create` 注入（task-20：一处真相）⇒ 这里不再补。
        Some(Self { raw, code })
    }

    /// 创建时的状态码。
    pub fn code(&self) -> i32 {
        self.code
    }

    /// 是不是诊断实例（`AB-56`）。
    pub fn is_diagnostic(&self) -> bool {
        self.code == status::PA_ERR_ABI
    }

    /// 当前诊断信息（复制成 `String`，避免借用指针失效，`AB-48`）。
    pub fn message(&self) -> Option<String> {
        safe::message(self.raw)
    }

    /// 裸指针。
    pub fn as_ptr(&self) -> *mut pa_state {
        self.raw
    }

    /// 按**名字**取平台常量（`CM-20`：数字随平台、映射按名字）。
    pub fn platform_constant(&self, name: &str) -> Option<i64> {
        safe::platform_constant(self.raw, name)
    }
}

impl Drop for PaState {
    fn drop(&mut self) {
        safe::destroy(self.raw);
        self.raw = core::ptr::null_mut();
    }
}

// 让 `c_char` 的导入在未使用时有明确去处（C 形态函数签名里随后会用到）
#[allow(dead_code)]
type CChar = c_char;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newstate_hands_out_a_usable_instance() {
        let state = PaState::new().expect("应当能建实例");
        assert_eq!(state.code(), status::PA_OK);
        assert!(!state.is_diagnostic());
        assert!(state.as_ptr().is_null() == false);
    }

    #[test]
    fn the_c_entry_point_matches_the_rust_one() {
        // SAFETY: 本测试自己用、自己销毁。
        let raw = paL_newstate();
        assert!(!raw.is_null());
        assert_eq!(safe::message(raw), None, "可用实例没有诊断信息");
        safe::destroy(raw);
    }
}

#[cfg(test)]
mod platform_tests {
    use super::*;

    /// 表的**唯一真相**现在在 `pyawa-abi`（task-20：`pa_create` 注入；表随生成脚本走）。
    use pyawa_abi::platform_errno;

    #[test]
    fn startup_injects_the_host_errno_table() {
        let state = PaState::new().expect("应当能建实例");
        // 注入的是生成物里那张表（**不是**测试里手写的数字）
        let expected = platform_errno::HOST_ERRNO.len();
        assert!(expected > 100, "宿主 errno 常量应当有一百多个，实际 {expected}");
        for (name, value) in platform_errno::HOST_ERRNO.iter().take(5) {
            assert_eq!(
                state.platform_constant(name),
                Some(*value),
                "启动注入之后按名字应当能查到 {name}"
            );
        }
        assert_eq!(state.platform_constant("ENOENT"),
                   platform_errno::HOST_ERRNO.iter().find(|(n, _)| *n == "ENOENT").map(|(_, v)| *v));
        assert_eq!(state.platform_constant("绝不存在的名字"), None);
    }

    #[test]
    fn aliases_share_one_number_and_errorcode_keeps_one_name() {
        // CM-19 的别名：EAGAIN 与 EWOULDBLOCK 同值
        let again = platform_errno::HOST_ERRNO.iter().find(|(n, _)| *n == "EAGAIN");
        let would_block = platform_errno::HOST_ERRNO.iter().find(|(n, _)| *n == "EWOULDBLOCK");
        if let (Some((_, a)), Some((_, b))) = (again, would_block) {
            assert_eq!(a, b, "别名必须同值");
        }
        // errorcode 的条目数 = 常量数 − 别名数（> 1 说明确实有别名被合并）
        let constants = platform_errno::HOST_ERRNO.len();
        let codes = pyawa_stdlib::errno_map::ERRNO_ERRORCODE.len();
        assert!(codes <= constants, "errorcode 不会比常量多");
        assert!(codes > 0);
    }
}
