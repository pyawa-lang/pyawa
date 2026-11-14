//! Pyawa 独立运行时：真实机器能力实现与入口（REPL/CLI）。
//!
//! 工作区内平台依赖的**集中点**，见本 crate 的 `README.md`。

// `CX-4` 的静态扫描**排除**本 crate（`docs/CONSTRAINTS.md` §3.1："它**就是**平台依赖的集中点"）
// ⇒ 这里允许定点使用平台相关代码（FFI／libc），但**逐项**开许可、不许整文件放开。
#![deny(unsafe_code)]

use core::ffi::{c_void, c_char};
use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_CLOCK, PA_DOMAIN_FS, PA_DOMAIN_NET};
use pyawa_abi::{
    pa_error, pa_setcapability, pa_setcapability_async, pa_host, pa_state, safe, status,
};

pub mod clock_system;
pub mod fs_posix;
pub mod net_posix;
pub mod pyac;

/// **组合根电池**（`§12.13 #1`／路线 A）：`paL_newstate` 用的真实机器 provider ＋ 它们的 vtable。
///
/// **生命周期（如实）**：C 便捷入口没有"实例销毁回调"可挂（`pa_destroy` 是 ABI 的），
/// provider／vtable 的指针由 `pa_setcapability` **原样保存**、必须在实例存活期内有效 ⇒
/// 这里**按实例泄漏**一份（进程生命周期；宿主需要精确回收时改用 `pa_create` ＋ 自己注册能力）。
/// 这是组合根便捷入口的边界，不是静默泄漏：见 `PLAN §12.13` 的"如实边界"。
#[allow(unsafe_code)]
struct MachineBattery {
    // 字段本身不被读：只负责**保活**（vtable 的 `state` 指回它们；泄漏到进程末）。
    _fs: fs_posix::PosixFs,
    _clock: clock_system::SystemClock,
    _net: net_posix::PosixNet,
    fs_vtable: pyawa_capabilities::fs::CpFsVtable,
    clock_vtable: pyawa_capabilities::clock::CpClockVtable,
    net_vtable: pyawa_capabilities::net::CpNetVtable,
}

/// 建电池并注册 `fs`／`clock`／`net` 三个真实机器能力域（每域先声明异步分类，`AB-34`）。
///
/// 成功 `PA_OK`；任一注册失败返回该处的状态码（实例仍可用，只是该域未注册）。
#[allow(unsafe_code)]
fn install_machine_capabilities(state: *mut pa_state) -> i32 {
    let mut fs = fs_posix::PosixFs::new();
    let mut clock = clock_system::SystemClock::new();
    let mut net = net_posix::PosixNet::new();
    // vtable 内的 `state` 指回 provider ⇒ provider 必须先稳定（泄漏的 Box）。
    // SAFETY: 指针指向真实存活对象；`pa_setcapability` 保存该指针。
    let fs_vtable = unsafe { core::ptr::read(&fs.vtable()) };
    let clock_vtable = unsafe { core::ptr::read(&clock.vtable()) };
    let net_vtable = unsafe { core::ptr::read(&net.vtable()) };
    let battery = Box::into_raw(Box::new(MachineBattery {
        _fs: core::mem::replace(&mut fs, fs_posix::PosixFs::new()),
        _clock: core::mem::replace(&mut clock, clock_system::SystemClock::new()),
        _net: core::mem::replace(&mut net, net_posix::PosixNet::new()),
        fs_vtable,
        clock_vtable,
        net_vtable,
    }));
    // SAFETY: battery 泄漏到进程末 ⇒ vtable 指针在实例存活期内一直有效。
    let (battery_ref, state_ptr) = unsafe { (&*battery, state) };
    let fs_impl = core::ptr::addr_of!(battery_ref.fs_vtable).cast::<c_void>();
    let clock_impl = core::ptr::addr_of!(battery_ref.clock_vtable).cast::<c_void>();
    let net_impl = core::ptr::addr_of!(battery_ref.net_vtable).cast::<c_void>();
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
