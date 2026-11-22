//! **`_winapi`／`_overlapped` 的最小真实语义**（`PLAN §12.24` 真实语义第二批；`windows` 目标）。
//!
//! 端到端走 ABI（`paL_newstate` 便利路径会安装真实面；裸 ABI 宿主仍是 stdlib 诚实桩）：
//! ① `CreateEvent`／`SetEvent`／`ResetEvent`／`WaitForSingleObject`／`CloseHandle` 真跑并核
//!    `WAIT_OBJECT_0`／`WAIT_TIMEOUT`；
//! ② `CreateIoCompletionPort` ＋ `PostQueuedCompletionStatus` ＋ `GetQueuedCompletionStatus`
//!    真投递／真取回 `(bytes, key, overlapped=None)`；
//! ③ `_overlapped.Overlapped()` 是**真对象**（真事件句柄＋地址），`Close()` 幂等性如实报错；
//! ④ P1：`CreateFile`＋`Overlapped` 的文件重叠读写已真跑（pipe/socket 仍未接线）。
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


#[test]
fn overlapped_file_read_write_through_the_abi() {
    let path = std::env::temp_dir().join(format!("pyawa-overlapped-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let path_text = path.to_string_lossy().replace('\\', "/");
    let script = format!(r#"
import _winapi
import _overlapped

h = _winapi.CreateFile({path:?}, 0x80000000 | 0x40000000, 0, None, 2, 0x40000000, None)
ovw = _overlapped.Overlapped()
ovw.WriteFile(h, b"hello")
written = ovw.GetOverlappedResult(1)
ovr = _overlapped.Overlapped()
ovr.ReadFile(h, 5)
data = ovr.getresult()
_winapi.CloseHandle(h)
file_io_ok = 1 if (written == 5 and data == b"hello") else 0
"#, path = path_text);
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null());
    assert_eq!(abi_exec(state, &script), 0, "exec 失败：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "file_io_ok"), Some(1));
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
    let _ = std::fs::remove_file(&path);
}


#[test]
fn overlapped_named_pipe_read_write_through_the_abi() {
    let pipe_name = format!(r"\\.\pipe\pyawa-pipe-{}", std::process::id());
    let script = format!(r#"
import _winapi
import _overlapped

srv = _winapi.CreateNamedPipe({pipe_name:?}, 3 | 0x40000000, 0, 1, 4096, 4096, 0, None)
ovc = _overlapped.Overlapped()
connected_immediately = ovc.ConnectNamedPipe(srv)
cli = _winapi.CreateFile({pipe_name:?}, 0xC0000000, 0, None, 3, 0x40000000, None)
ovr = _overlapped.Overlapped()
ovr.ReadFile(srv, 5)
ovw = _overlapped.Overlapped()
ovw.WriteFile(cli, b"hello")
data = ovr.getresult()
n = ovw.getresult()
_winapi.CloseHandle(cli)
_winapi.CloseHandle(srv)
pipe_io_ok = 1 if (data == b"hello" and n == 5) else 0
"#, pipe_name = pipe_name);
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null());
    assert_eq!(abi_exec(state, &script), 0, "exec 失败：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "pipe_io_ok"), Some(1));
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}


#[test]
fn overlapped_socket_recv_send_through_the_abi() {
    let script = r#"
import _socket
import _overlapped

a, b = _socket.socketpair(2)
ovr = _overlapped.Overlapped()
ovr.WSARecv(a.fileno(), 5, 0)
ovw = _overlapped.Overlapped()
ovw.WSASend(b.fileno(), b"hello", 0)
data = ovr.getresult()
n = ovw.getresult()
a.close()
b.close()
socket_io_ok = 1 if (data == b"hello" and n == 5) else 0
"#;
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null());
    assert_eq!(abi_exec(state, script), 0, "exec 失败：{}", abi_errmsg(state));
    assert_eq!(abi_global_int(state, "socket_io_ok"), Some(1));
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
