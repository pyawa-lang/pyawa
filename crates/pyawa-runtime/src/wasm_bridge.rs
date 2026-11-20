//! **wasm32 宿主桥（`PLAN §12.21` E2）**：固定槽位 trampoline ＋ JS 填充协议。
//!
//! 背景（`§12.21` 核心难点 1）：wasm 没有"运行时函数指针"——JS 造不出指向 wasm 代码的 C 函数指针。
//! 本模块给出固定槽位表：编译期生成 [`TRAMPOLINE_SLOTS`] 个 wrapper，每个在 wasm 内被调用时把槽位号
//! 交给 JS import（`pyawa_host.pa_host_slot_call`）；JS 按槽位维护自己的回调表。注册流程（JS 侧）：
//!
//! 1. `slot`：JS 自选空闲槽（上限由 [`pyawa_wasm_trampoline_capacity`] 给出）；
//! 2. `fn_ptr = pyawa_wasm_trampoline(slot)`：拿到 wasm 侧 trampoline 的**表索引**，原样传给 `pa_register`；
//! 3. `sig = pyawa_wasm_sig_basic(nparams)`：基础签名（`nparams` 个位置形参、未标注；**泄漏到进程末**，
//!    是 E2 的如实边界）；
//! 4. `pa_register(state, name, fn_ptr, sig)`；JS 在 `hostTable[slot]` 放自己的回调。
//!
//! 回调约定与 C 宿主**同一套**（`pa.h` 的宿主函数返回值约定）：从虚拟栈取实参、把结果留在栈顶、
//! 返回状态码（`0` ＝ `PA_OK`）。JS 的异常**不得**穿回 wasm（wasm32 是 `panic=abort`，异常跨边界即
//! trap）⇒ 回调内自行 `try/catch` 并返回错误码。
//!
//! **本模块只在 wasm32 目标编译**（`crates/pyawa-runtime/src/lib.rs` 的 `#[cfg(target_family = "wasm")]`）；
//! 宿主构建零影响。
//!
//! **如实边界（E2）**：槽位固定 [`TRAMPOLINE_SLOTS`] 个；`pyawa_wasm_sig_basic` 每次注册泄漏一份小分配
//! （无回收——等 E3 的 JS 生命周期协议）；`pyawa_wasm_alloc` 只服务 JS 侧小缓冲区（C 字符串／`int64_t`
//! 出参），**不是**通用分配器 API。

use core::alloc::Layout;
use core::ffi::c_void;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_CLOCK};
use pyawa_abi::host::{param_flags, pa_param, pa_sig};
use pyawa_abi::{pa_setcapability, pa_setcapability_async, pa_state, status};
use pyawa_capabilities::clock::{CapStatus, CpClockVtable};

/// 固定槽位数（编译期常量；JS 用 [`pyawa_wasm_trampoline_capacity`] 取）。
pub const TRAMPOLINE_SLOTS: u32 = 64;

#[link(wasm_import_module = "pyawa_host")]
unsafe extern "C" {
    /// JS 侧宿主调用入口（E2 协议）：`slot` ∈ `0..TRAMPOLINE_SLOTS`，`state` 是 `pa_state` 指针。
    /// 返回状态码；成功时结果应在虚拟栈顶（宿主函数返回值约定，见 `pa.h`）。
    fn pa_host_slot_call(slot: u32, state: *mut c_void) -> i32;

    /// **E4 协议**：JS 提供的**挂钟**纳秒（自 Unix 纪元）。
    ///
    /// 浏览器里由 `performance.timeOrigin + performance.now()`（毫秒 ⇒ 纳秒）一类提供；
    /// 语义与 host 侧 `clock.now_ns` 相同（`CP-5` 的 `Ok` 那一路）。
    fn clock_now_ns() -> i64;

    /// **E4 协议**：JS 提供的**单调钟**纳秒（起点任意，只要单调）。
    ///
    /// 浏览器里由 `performance.now()` 提供；VM 的 `time.sleep` 分片与超时用它算截止时刻。
    fn clock_monotonic_ns() -> i64;

    /// **E4 协议**：JS 提供的**有界睡眠**（纳秒）。VM 侧保证单次 `ns <= 10 ms`（分片，`CP-27`）。
    ///
    /// 浏览器里可用 `Atomics.wait`／忙等实现；本调用返回即表示"已睡过"。
    fn clock_sleep_ns(ns: i64);
}

/// 生成固定槽位 trampoline，并导出"按槽位取函数指针"的查询入口。
///
/// 每个 wrapper 的槽位号是**编译期常量** ⇒ JS 无需传闭包，wasm 侧也不保存任何 JS 状态。
macro_rules! define_trampolines {
    ($($slot:literal => $name:ident),* $(,)?) => {
        $(
            /// 固定槽位的 trampoline（由 [`pyawa_wasm_trampoline`] 按槽位查出）。
            unsafe extern "C-unwind" fn $name(state: *mut pa_state) -> i32 {
                // SAFETY: state 由 VM 的宿主函数派发交回；槽位号是编译期常量。
                unsafe { pa_host_slot_call($slot, state.cast()) }
            }
        )*

        /// 取某槽位的 trampoline **函数指针**（wasm 里就是函数表索引，JS 原样传回 `pa_register`）。
        /// 越界 ⇒ `0`（JS 侧应视为"没有该槽位"，并先查 [`pyawa_wasm_trampoline_capacity`]）。
        #[unsafe(no_mangle)]
        pub extern "C" fn pyawa_wasm_trampoline(slot: u32) -> usize {
            match slot {
                $( $slot => trampoline_ptr($name), )*
                _ => 0,
            }
        }
    };
}

/// 把 trampoline 函数指针换成 wasm 函数表索引。
///
/// wasm32-unknown-unknown 上函数指针**就是**函数表索引（4 字节，与 `usize` 同宽）⇒ JS 拿到这个数后
/// 原样传回 `pa_register` 即等价于 C 宿主传 `pa_host_fn`。宿主目标不编译本模块。
fn trampoline_ptr(function: unsafe extern "C-unwind" fn(*mut pa_state) -> i32) -> usize {
    // SAFETY: 两种表示在 wasm32 上等宽同义（均为函数表索引）；本模块只在 wasm32 编译。
    unsafe { core::mem::transmute(function) }
}

define_trampolines! {
    0 => slot0, 1 => slot1, 2 => slot2, 3 => slot3, 4 => slot4, 5 => slot5, 6 => slot6, 7 => slot7,
    8 => slot8, 9 => slot9, 10 => slot10, 11 => slot11, 12 => slot12, 13 => slot13, 14 => slot14, 15 => slot15,
    16 => slot16, 17 => slot17, 18 => slot18, 19 => slot19, 20 => slot20, 21 => slot21, 22 => slot22, 23 => slot23,
    24 => slot24, 25 => slot25, 26 => slot26, 27 => slot27, 28 => slot28, 29 => slot29, 30 => slot30, 31 => slot31,
    32 => slot32, 33 => slot33, 34 => slot34, 35 => slot35, 36 => slot36, 37 => slot37, 38 => slot38, 39 => slot39,
    40 => slot40, 41 => slot41, 42 => slot42, 43 => slot43, 44 => slot44, 45 => slot45, 46 => slot46, 47 => slot47,
    48 => slot48, 49 => slot49, 50 => slot50, 51 => slot51, 52 => slot52, 53 => slot53, 54 => slot54, 55 => slot55,
    56 => slot56, 57 => slot57, 58 => slot58, 59 => slot59, 60 => slot60, 61 => slot61, 62 => slot62, 63 => slot63,
}

/// 固定槽位上限（JS 注册前查一次；`0..capacity` 是合法槽位，越界由 [`pyawa_wasm_trampoline`] 给 `0`）。
#[unsafe(no_mangle)]
pub extern "C" fn pyawa_wasm_trampoline_capacity() -> u32 {
    TRAMPOLINE_SLOTS
}

/// **JS 侧小缓冲区分配**（返回 wasm 线性内存里的指针；`len == 0` ⇒ `NULL`）。
///
/// 用途：JS 把 C 字符串／`int64_t` 出参放进 wasm 内存（`pa_register` 的 `name`、`pa_exec_string`
/// 的 `source`／`chunkname`／`mode`、`pa_tointeger` 的 `out` 等）。配套 [`pyawa_wasm_free`]。
#[unsafe(no_mangle)]
pub extern "C" fn pyawa_wasm_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return core::ptr::null_mut();
    }
    let Ok(layout) = Layout::from_size_align(len, 1) else {
        return core::ptr::null_mut();
    };
    // SAFETY: layout 合法且 size > 0。
    unsafe { std::alloc::alloc(layout) }
}

/// 释放 [`pyawa_wasm_alloc`] 的缓冲区（`ptr == NULL`／`len == 0` ⇒ no-op）。
#[unsafe(no_mangle)]
pub extern "C" fn pyawa_wasm_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let Ok(layout) = Layout::from_size_align(len, 1) else {
        return;
    };
    // SAFETY: ptr 由同名 alloc 按同一 layout 分配；调用方不得重复释放。
    unsafe { std::alloc::dealloc(ptr, layout) };
}

/// **JS 侧基础签名构造**：`nparams` 个位置形参（未标注、无默认值），供 `pa_register`（`AB-25`
/// 要求签名必须给）。
///
/// 返回的 `pa_sig` 与形参数组一起**泄漏到进程末**（E2 的如实边界：注册次数极少，回收留 E3 的
/// JS 生命周期协议）。`nparams > 256` ⇒ `NULL`（与 `read_signature` 的读取上限一致）。
#[unsafe(no_mangle)]
pub extern "C" fn pyawa_wasm_sig_basic(nparams: u32) -> *const pa_sig {
    if nparams > 256 {
        return core::ptr::null();
    }
    let params: Vec<pa_param> = (0..nparams)
        .map(|_| pa_param {
            size: core::mem::size_of::<pa_param>(),
            name: core::ptr::null(),
            type_expr: core::ptr::null(),
            flags: param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        })
        .collect();
    let params = Box::leak(params.into_boxed_slice());
    let signature = Box::new(pa_sig {
        size: core::mem::size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: params.len(),
        params: params.as_ptr(),
    });
    Box::into_raw(signature)
}

// --------------------------------------------------------------------------- //
// E4：JS 提供的 `clock` 能力域（`PLAN §12.21` E4）
//
// 与 host 侧 `clock_system` 的 provider **同一套 vtable 契约**（`CP-`）；差别只在实现来源：
// `now_ns`／`monotonic_ns`／`sleep_ns` 走 JS import，日历／时区三槽不接（`CP-3`：槽位 `None`）。
// `fs`／`net` **不注册**（浏览器没有 ⇒ 如实"未提供"，`CP-2`）。
// --------------------------------------------------------------------------- //

/// 挂钟：JS 给纳秒；出参 `NULL` 属于违反契约，按机器错误拒绝（不写指针）。
extern "C" fn js_clock_now_ns(_state: *mut c_void, out: *mut i64) -> CapStatus {
    if out.is_null() {
        return CapStatus::Machine;
    }
    // SAFETY: 上面已判空；`out` 由核心按 `CP-5` 的契约给出。
    unsafe { *out = clock_now_ns() };
    CapStatus::Ok
}

/// 单调钟：JS 给纳秒（VM 的 `time.sleep` 分片用它算截止时刻）。
extern "C" fn js_clock_monotonic_ns(_state: *mut c_void, out: *mut i64) -> CapStatus {
    if out.is_null() {
        return CapStatus::Machine;
    }
    // SAFETY: 上面已判空。
    unsafe { *out = clock_monotonic_ns() };
    CapStatus::Ok
}

/// 有界睡眠：VM 已把长等待切成 ≤10 ms 的片（`CP-27`），这里直接交给 JS。
extern "C" fn js_clock_sleep_ns(_state: *mut c_void, ns: i64, _errno_out: *mut i32) -> CapStatus {
    if ns <= 0 {
        return CapStatus::Ok;
    }
    // SAFETY: import 由 JS 宿主提供（E4 协议），调用即"睡够 ns"。
    unsafe { clock_sleep_ns(ns) };
    CapStatus::Ok
}

/// `clock` 域 vtable（`CP-30`：带版本／尺寸；版本照 `AB-39` 用 `PA_ABI_VERSION`）。
fn js_clock_vtable() -> CpClockVtable {
    CpClockVtable {
        version: pyawa_abi::PA_ABI_VERSION,
        size: core::mem::size_of::<CpClockVtable>() as u32,
        state: core::ptr::null_mut(),
        now_ns: Some(js_clock_now_ns),
        monotonic_ns: Some(js_clock_monotonic_ns),
        sleep_ns: Some(js_clock_sleep_ns),
        // 日历／时区三槽：E4 不接（`CP-3`）⇒ stdlib 的 `localtime` 等走既有 UTC 回退。
        localtime: None,
        strftime: None,
        mktime: None,
    }
}

/// **E4 入口**：把 JS clock 注册进实例（`fs`／`net` 保持未注册）。
///
/// 生命周期与 host 组合根同口径：vtable **泄漏到进程末**（实例存续期内指针有效；宿主需要
/// 精确回收时改用 `pa_create` ＋ 自己的 provider）。
pub(crate) fn install_js_clock(state: *mut pa_state) -> i32 {
    let table = Box::into_raw(Box::new(js_clock_vtable()));
    // `CP-25`：先声明异步分类，再给实现。
    let code = unsafe { pa_setcapability_async(state, PA_DOMAIN_CLOCK, PA_ASYNC_OK) };
    if code != status::PA_OK {
        return code;
    }
    // SAFETY: `table` 泄漏到进程末，实例存活期内指针有效。
    unsafe { pa_setcapability(state, PA_DOMAIN_CLOCK, table.cast::<c_void>()) }
}
