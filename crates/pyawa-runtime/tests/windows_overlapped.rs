//! **`_winapi`／`_overlapped` 的最小真实语义**（`PLAN §12.24` 真实语义第二批；`windows` 目标）。
//!
//! 端到端走 ABI（`paL_newstate` 便利路径会安装真实面；裸 ABI 宿主仍是 stdlib 诚实桩）：
//! ① `CreateEvent`／`SetEvent`／`ResetEvent`／`WaitForSingleObject`／`CloseHandle` 真跑并核
//!    `WAIT_OBJECT_0`／`WAIT_TIMEOUT`；
//! ② `CreateIoCompletionPort` ＋ `PostQueuedCompletionStatus` ＋ `GetQueuedCompletionStatus`
//!    真投递／真取回 `(bytes, key, overlapped=None)`；
//! ③ `_overlapped.Overlapped()` 是**真对象**（真事件句柄＋地址），`Close()` 幂等性如实报错；
//! ④ 重叠 I/O 数据面（`ReadFile` 等）**如实** `NotImplementedError`（不假装成功）。
#![cfg(windows)]
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
fn winapi_and_overlapped_minimal_real_semantics_through_the_abi() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = r#"
import _winapi
import _overlapped

event = _winapi.CreateEvent(None, 1, 0, None)
event_is_int = 1 if isinstance(event, int) and not isinstance(event, bool) else 0
_winapi.SetEvent(event)
wait_set = _winapi.WaitForSingleObject(event, 0)
_winapi.ResetEvent(event)
wait_reset = _winapi.WaitForSingleObject(event, 10)
_winapi.CloseHandle(event)
waits_ok = 1 if (wait_set == _winapi.WAIT_OBJECT_0 and wait_reset == _winapi.WAIT_TIMEOUT) else 0

iocp = _winapi.CreateIoCompletionPort(_winapi.INVALID_HANDLE_VALUE, 0, 0, 1)
_winapi.PostQueuedCompletionStatus(iocp, 42, 7, None)
transferred, key, overlapped = _winapi.GetQueuedCompletionStatus(iocp, 1000)
_winapi.CloseHandle(iocp)
queue_ok = 1 if (transferred == 42 and key == 7 and overlapped is None) else 0

overlapped_object = _overlapped.Overlapped()
object_ok = 1 if (
    isinstance(overlapped_object.event, int)
    and isinstance(overlapped_object.address, int)
    and overlapped_object.address != 0
) else 0
try:
    overlapped_object.ReadFile(_winapi.INVALID_HANDLE_VALUE, b'x')
except NotImplementedError:
    unwired_ok = 1
else:
    unwired_ok = 0
try:
    overlapped_object.GetOverlappedResult(0)
except NotImplementedError:
    get_result_unwired = 1
else:
    get_result_unwired = 0
overlapped_object.Close()
closed_ok = 1
try:
    overlapped_object.Close()
except OSError:
    double_close_ok = 1
else:
    double_close_ok = 0

pid = _winapi.GetCurrentProcessId()
tid = _winapi.GetCurrentThreadId()
ids_ok = 1 if (pid > 0 and tid > 0) else 0
"#;
    let rc = abi_exec(state, script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    for name in [
        "event_is_int",
        "waits_ok",
        "queue_ok",
        "object_ok",
        "unwired_ok",
        "get_result_unwired",
        "closed_ok",
        "double_close_ok",
        "ids_ok",
    ] {
        assert_eq!(
            abi_global_int(state, name),
            Some(1),
            "{name} 不等于 1（`_winapi`／`_overlapped` 最小真实语义不完整？）：{}",
            abi_errmsg(state)
        );
    }
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
