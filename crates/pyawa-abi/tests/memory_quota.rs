//! **每实例内存配额 ABI 钩子**（`AB-65`／`AB-66`）的验收。
//!
//! 口径：宿主经 `pa_set_memory_limit` 设上限；超限 ⇒ 脚本里抛**可捕获的** `MemoryError`
//! （不是 abort／panic ✓）；`pa_memory_used` 回读当前占用（宿主做指标／看门狗用）。

use std::ffi::CString;
use core::mem::size_of;

use pyawa_abi::status::*;
use pyawa_abi::*;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

fn create() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: host／state 是本测试的局部变量，按 AB-55 的契约传。
    let created = unsafe { pa_create(&host, &mut state) };
    assert_eq!(created, PA_OK);
    assert!(!state.is_null());
    state
}

fn exec_script(state: *mut pa_state, source: &str) -> i32 {
    let source_c = CString::new(source).expect("脚本不含 NUL");
    let mode = CString::new("python").expect("mode");
    // SAFETY: state 是 pa_create 交回且尚未销毁的指针；source／mode 都是 NUL 结尾局部串；
    // chunkname／options 传 NULL（默认口径，AB-60／AB-61）。
    unsafe {
        pa_exec_string(
            state,
            source_c.as_ptr(),
            -1,
            core::ptr::null(),
            mode.as_ptr(),
            core::ptr::null(),
        )
    }
}

/// 读一个整型全局（`pa_getglobal` ＋ `pa_tointeger` ＋ 复原栈）。
fn read_int_global(state: *mut pa_state, name: &str) -> i64 {
    // SAFETY: state 存活；name 是 NUL 结尾局部串。
    let top = unsafe { pa_gettop(state) };
    let name_c = CString::new(name).expect("名字不含 NUL");
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_getglobal(state, name_c.as_ptr()) }, PA_OK);
    let mut value: i64 = 0;
    // SAFETY: out 指向本地 i64。
    let converted = unsafe { pa_tointeger(state, -1, &mut value) };
    // SAFETY: 复原栈（-2 表示去掉刚压入的一格）。
    unsafe { pa_settop(state, top) };
    assert_eq!(converted, PA_OK, "全局 {name} 应是整数");
    value
}

#[test]
fn quota_limits_allocation_and_reports_usage() {
    let state = create();
    let mut used: u64 = 0;
    // SAFETY: out 指向本地 u64。
    assert_eq!(unsafe { pa_memory_used(state, &mut used) }, PA_OK);
    assert!(used > 0, "刚建好的实例已有引导期分配（实际 {used}）");
    let limit = used + 256 * 1024;
    // SAFETY: state 存活。
    assert_eq!(unsafe { pa_set_memory_limit(state, limit) }, PA_OK);

    let script = r#"
data = []
n = 0
try:
    while n < 200000:
        data.append("x" * 1000)
        n = n + 1
except:
    n = -1
"#;
    assert_eq!(exec_script(state, script), PA_OK, "脚本应干净结束（异常被捕获）");
    assert_eq!(read_int_global(state, "n"), -1, "超限必须抛 MemoryError");

    let mut after: u64 = 0;
    // SAFETY: out 指向本地 u64。
    assert_eq!(unsafe { pa_memory_used(state, &mut after) }, PA_OK);
    let slack = (limit / 16).max(64 * 1024);
    assert!(
        after <= limit + slack + 64 * 1024,
        "宿主可见占用应落在 limit+slack 量级（实际 {after}，limit {limit}）"
    );

    // 取消限制仍可工作（0 ＝ 不限）。
    // SAFETY: state 存活。
    assert_eq!(unsafe { pa_set_memory_limit(state, 0) }, PA_OK);
    // SAFETY: state 由 pa_create 交回、尚未销毁（AB-56／AB-57）。
    unsafe { pa_destroy(state) };
}

#[test]
fn quota_hook_validates_arguments() {
    // NULL state：两个钩子都必须拒绝（不崩）。
    // SAFETY: 显式传 NULL 是契约允许的用法错误。
    assert_eq!(unsafe { pa_set_memory_limit(core::ptr::null_mut(), 1024) }, PA_ERR_INVALID);
    let mut used: u64 = 0;
    // SAFETY: 同上。
    assert_eq!(
        unsafe { pa_memory_used(core::ptr::null_mut(), &mut used) },
        PA_ERR_INVALID
    );
    let state = create();
    // SAFETY: out 传 NULL ⇒ 用法错误（不写）。
    assert_eq!(unsafe { pa_memory_used(state, core::ptr::null_mut()) }, PA_ERR_INVALID);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn uncaught_over_quota_reports_runtime_error_not_abort() {
    let state = create();
    let mut used: u64 = 0;
    // SAFETY: out 指向本地 u64。
    assert_eq!(unsafe { pa_memory_used(state, &mut used) }, PA_OK);
    // SAFETY: state 存活。
    assert_eq!(unsafe { pa_set_memory_limit(state, used + 128 * 1024) }, PA_OK);
    let script = r#"
data = []
n = 0
while n < 200000:
    data.append("x" * 1000)
    n = n + 1
"#;
    // 未捕获 ⇒ 宿主看到 PA_ERR_RUNTIME（脚本异常级，不是 abort／panic ✓）。
    let status = exec_script(state, script);
    assert_eq!(
        status, PA_ERR_RUNTIME,
        "未捕获的 MemoryError 应是脚本异常（实际状态 {status}）"
    );
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

#[test]
fn abi_size_and_version_unchanged_except_hooks() {
    // 新钩子不改宿主结构体尺寸／主版本（`AB-65`／`AB-66` 是**函数**，不是结构字段 ✓）。
    assert_eq!(pa_abi_version(), PA_ABI_VERSION);
    assert_eq!(PA_ABI_SIZE, size_of::<pa_host>());
    // 函数指针存在且可寻址（编译期即证；这里防"被 cfg 掉"）。
    let _typed: unsafe extern "C" fn(*mut pa_state, u64) -> i32 = pa_set_memory_limit;
    let _typed: unsafe extern "C" fn(*mut pa_state, *mut u64) -> i32 = pa_memory_used;
}
