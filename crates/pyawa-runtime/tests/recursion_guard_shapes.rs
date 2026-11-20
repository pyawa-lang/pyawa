//! **递归上限／回调深度守卫的形状**（`PLAN §12.22` 的 S1／S3 前置；跨平台）。
//!
//! 现状（`DIV-11` 如实登记）：本层的 Python 调用是 **Rust 递归** ⇒ 上限 `MAX_CALL_DEPTH=64`
//! （参照 1000），`sys.setrecursionlimit` **只降不升**。S1 要钉的是**守卫行为**：
//! ① 深递归在到达上限时抛**可捕获**的 `RecursionError`（不是原生栈崩）；
//! ② **内建 → 脚本回调**这条路径也吃同一个守卫（`sorted`／`map` 里无限自递归不能崩宿主）；
//! ③ 异常之后实例仍可用（没有栈／引用计数损坏）；
//! ④ `sys.getrecursionlimit`／`setrecursionlimit` 的**现行口径**（可降；升被夹回上限）。
//!
//! 这些断言在 S2（显式帧栈）后应更新为参照口径（默认 1000、可升），本条目前钉住"不崩"。
#![allow(unused_unsafe)]

fn abi_c_string(text: &str) -> std::ffi::CString {
    std::ffi::CString::new(text).expect("脚本／名字不含 NUL")
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
fn recursion_guard_is_catchable_and_keeps_the_instance_usable() {
    // **必须在"大栈"线程里跑**（同 `conformance` 的 `pyawa_side_runner`）：本层的 Python 调用是
    // Rust 递归，libtest 的测试线程栈偏小（实测默认测试线程上**守卫之前**就顶穿 2 MiB）。
    // 这只给够栈，不改守卫语义（`MAX_CALL_DEPTH` 仍应更早触发）。
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(body)
        .expect("起大栈线程")
        .join()
        .expect("大栈线程不该 panic");
}

fn body() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = r#"
import sys

limit = sys.getrecursionlimit()

def down(n):
    if n == 0:
        return 0
    return down(n - 1) + 1

# S2：显式帧栈 ⇒ 深递归（900 层）在默认上限 1000 内应正常返回（不再吃原生栈）
deep = down(900)
shallow = down(30)

def infinite(n):
    return infinite(n + 1)

try:
    infinite(0)
except RecursionError:
    deep_caught = 1
else:
    deep_caught = 0

def via_callback(n):
    return list(map(via_callback, [n + 1]))

try:
    via_callback(0)
except RecursionError:
    callback_caught = 1
else:
    callback_caught = 0

# 守卫触发之后：实例仍可用、栈没有损坏
after = down(30) + 12

# S3：可升（纯脚本递归只吃堆帧）＋安全上限 10000（超限如实夹住）
sys.setrecursionlimit(40)
lowered = sys.getrecursionlimit()
sys.setrecursionlimit(1500)
raised = sys.getrecursionlimit()
sys.setrecursionlimit(20000)
clamped = sys.getrecursionlimit()
"#;
    let rc = abi_exec(state, script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "deep"), Some(900), "900 层递归应通过（S2 显式帧栈）：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "shallow"), Some(30), "上限内递归应正常：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "deep_caught"), Some(1), "无限深递归应抛可捕获 RecursionError：{}", abi_errmsg(state));
    assert_eq!(
        abi_global_int(state, "callback_caught"),
        Some(1),
        "内建→脚本回调路径应被原生深度守卫拦住（可捕获）：{}",
        abi_errmsg(state)
    );
    assert_eq!(abi_global_int(state, "after"), Some(42), "守卫触发后实例应仍可用：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "limit"), Some(1000), "默认上限应为参照口径 1000：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "lowered"), Some(40), "setrecursionlimit 应能降到 40：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "raised"), Some(1500), "setrecursionlimit 应能升（S3 可升）：{}", abi_errmsg(state));
    assert_eq!(
        abi_global_int(state, "clamped"),
        Some(10000),
        "超过安全上限应如实夹到 10000（S3 安全上限）：{}",
        abi_errmsg(state)
    );
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
