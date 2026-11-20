//! **Windows `clock` provider 形状测试**（`PLAN §12.20` W2；只在 `windows` 目标编译）。
//!
//! 直接测 provider 的 vtable（不依赖 CLI／ABI）：`now_ns` 与系统时间同源、`monotonic_ns` 单调、
//! `sleep_ns` 真推进、日历三槽如实 `None`（`CP-3`）。端到端 `time` 面（含 UTC 回退）在 Linux 的
//! `time_shapes` 守；Windows 侧的真实时区面留 W5／W6（见 `clock_windows.rs` 的如实边界）。
#![cfg(windows)]
// vtable 槽位是安全的 `extern "C" fn` ⇒ 调用不需要 unsafe；保留 `unsafe {}` 与 SAFETY 注释是为了
// 将来槽位改成 `unsafe fn` 时不漏（与 windows_fs_shapes 同口径）。
#![allow(unused_unsafe)]

use core::ffi::c_void;
use std::time::{SystemTime, UNIX_EPOCH};

use pyawa_capabilities::clock::{CapStatus, CpClockVtable};
use pyawa_runtime::clock_windows::WindowsClock;

fn call_now(vtable: &CpClockVtable) -> (CapStatus, i64) {
    let mut out = 0i64;
    // SAFETY: vtable 的 now_ns 由本 provider 给出；出参是活的局部变量。
    let status = unsafe { (vtable.now_ns.expect("now_ns 已接线"))(vtable.state, &mut out) };
    (status, out)
}

fn call_monotonic(vtable: &CpClockVtable) -> (CapStatus, i64) {
    let mut out = 0i64;
    // SAFETY: 同上（monotonic_ns 已接线，出参有效）。
    let status = unsafe { (vtable.monotonic_ns.expect("monotonic_ns 已接线"))(vtable.state, &mut out) };
    (status, out)
}

#[test]
fn vtable_shape_matches_the_capability_contract() {
    let clock = WindowsClock::new();
    let vtable = clock.vtable();
    assert_eq!(vtable.version, pyawa_abi::PA_ABI_VERSION);
    assert_eq!(vtable.size as usize, core::mem::size_of::<CpClockVtable>());
    assert!(!vtable.state.is_null());
    assert!(vtable.now_ns.is_some());
    assert!(vtable.monotonic_ns.is_some());
    assert!(vtable.sleep_ns.is_some());
    // W2 补齐（2026-11-19）：日历／时区三槽已是真实现（UCRT；见 clock_windows 模块文档）。
    assert!(vtable.localtime.is_some());
    assert!(vtable.strftime.is_some());
    assert!(vtable.mktime.is_some());
}

#[test]
fn now_ns_is_wall_clock_in_nanoseconds_since_the_epoch() {
    let clock = WindowsClock::new();
    let vtable = clock.vtable();
    let (status, value) = call_now(&vtable);
    assert_eq!(status, CapStatus::Ok);
    let expected = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系统时间晚于纪元")
        .as_nanos() as i64;
    // 允许 10 秒调度窗口（同一台机器、同一时刻的两次读数）。
    assert!(
        (value - expected).abs() < 10_000_000_000,
        "now_ns={value} 与 SystemTime={expected} 相差过大"
    );

    // `CP-5`：出参 NULL 属违反契约 ⇒ 机器错误（不写指针、不 UB）。
    // SAFETY: 故意传 NULL 出参；provider 必须先判空再写。
    let null_status = unsafe { (vtable.now_ns.unwrap())(vtable.state, core::ptr::null_mut()) };
    assert_eq!(null_status, CapStatus::Machine);
}

#[test]
fn monotonic_ns_advances_and_sleep_really_sleeps() {
    let clock = WindowsClock::new();
    let vtable = clock.vtable();
    let (first_status, first) = call_monotonic(&vtable);
    assert_eq!(first_status, CapStatus::Ok);

    // 有界睡眠（VM 会切片；这里直接睡 2 ms）。
    let mut errno = 0i32;
    // SAFETY: sleep_ns 已接线；errno 出参是活的局部变量。
    let sleep_status = unsafe { (vtable.sleep_ns.expect("sleep_ns 已接线"))(vtable.state, 2_000_000, &mut errno) };
    assert_eq!(sleep_status, CapStatus::Ok);

    let (second_status, second) = call_monotonic(&vtable);
    assert_eq!(second_status, CapStatus::Ok);
    assert!(second >= first, "单调钟不得后退：{first} ⇒ {second}");
    assert!(
        second - first >= 1_000_000,
        "睡 2 ms 后单调钟至少前进 1 ms，实际 {first} ⇒ {second}"
    );

    // `ns <= 0` ⇒ 立即成功（不睡）。
    // SAFETY: 同上。
    let zero_status = unsafe { (vtable.sleep_ns.unwrap())(vtable.state, 0, &mut errno) };
    assert_eq!(zero_status, CapStatus::Ok);
    let _ = core::ptr::null::<c_void>();
}

// --------------------------------------------------------------------------- //
// W2 的端到端验收：`paL_newstate`（Windows 组合根已注册 clock）→ 跑 `time.time_ns()`
// --------------------------------------------------------------------------- //

fn abi_c_string(text: &str) -> std::ffi::CString {
    std::ffi::CString::new(text).expect("脚本不含 NUL")
}

fn abi_errmsg(state: *mut pyawa_abi::pa_state) -> String {
    // SAFETY: state 活着（调用方保证）。
    let ptr = unsafe { pyawa_abi::pa_errmsg(state) };
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: `pa_errmsg` 交回 NUL 结尾的借用字符串。
    unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

fn abi_exec(state: *mut pyawa_abi::pa_state, source: &str) -> i32 {
    let source = abi_c_string(source);
    let mode = abi_c_string("python");
    // SAFETY: 源码／模式名都是活的 CString；options 传 NULL（`AB-61`）。
    unsafe {
        pyawa_abi::pa_exec_string(
            state,
            source.as_ptr(),
            -1,
            core::ptr::null(),
            mode.as_ptr(),
            core::ptr::null(),
        )
    }
}

fn abi_global_int(state: *mut pyawa_abi::pa_state, name: &str) -> Option<i64> {
    let name = abi_c_string(name);
    // SAFETY: state 活着；名字是活的 CString。
    if unsafe { pyawa_abi::pa_getglobal(state, name.as_ptr()) } != 0 {
        return None;
    }
    let mut value = 0i64;
    // SAFETY: 栈顶刚被压入；出参是活的局部变量。
    let rc = unsafe { pyawa_abi::pa_tointeger(state, -1, &mut value) };
    // SAFETY: 归还 `pa_getglobal` 的那份引用。
    unsafe { pyawa_abi::pa_pop(state, 1) };
    (rc == 0).then_some(value)
}

#[test]
fn time_ns_end_to_end_through_the_abi() {
    // `paL_newstate` 在 Windows 组合根里会注册 clock（`install_windows_capabilities`）。
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let rc = abi_exec(state, "import time\nt = time.time_ns()");
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    let value = abi_global_int(state, "t").expect("读回 time.time_ns()");
    let expected = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系统时间晚于纪元")
        .as_nanos() as i64;
    assert!(
        (value - expected).abs() < 20_000_000_000,
        "ABI 端到端 time_ns={value} 与 SystemTime={expected} 相差过大"
    );
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
