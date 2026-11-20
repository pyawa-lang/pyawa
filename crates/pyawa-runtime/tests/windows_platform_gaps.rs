//! **Windows 平台面补齐第一批**（`PLAN §12.20` W2／W3／W4 的如实缺口；只在 `windows` 目标编译）。
//!
//! 端到端走 ABI（`paL_newstate` ＝ Windows 组合根的真实 fs／clock／net 能力 ＋ stdlib 装配）：
//! ① `clock` 日历三槽（`localtime`／`strftime`／`mktime`）真实现——本地时间字段合理、
//!    `strftime("%Y-%m-%d")` 与字段一致、`mktime(localtime(t))` 往返一致、`tm_gmtoff` 是整数；
//! ② `fs` 的 `pipe` 槽真实现——`os.pipe()` 写入可读回（读端先、写端后）；
//! ③ `net` 的 `socketpair` 槽真实现——loopback TCP 对上 `sendall`／`recv` 往返。
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
fn windows_platform_gaps_first_batch_end_to_end_through_the_abi() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = r#"
import os
import socket
import time

# ① clock 日历三槽（真实现；非 UTC 时区也走同一路径）
now = int(time.time())
tm = time.localtime(now)
# `struct_time.tm_year` 已是四位数、`tm_mon` 是 **1-based**（参照口径）。
clock_fields_ok = 1 if (
    1970 <= tm.tm_year <= 2100
    and 1 <= tm.tm_mon <= 12
    and 1 <= tm.tm_mday <= 31
    and 0 <= tm.tm_hour <= 23
) else 0
formatted = time.strftime("%Y-%m-%d", tm)
strftime_ok = 1 if formatted.startswith(str(tm.tm_year)) else 0
roundtrip = time.mktime(tm)
mktime_ok = 1 if -2 <= roundtrip - now <= 2 else 0
gmtoff_ok = 1 if isinstance(tm.tm_gmtoff, int) else 0

# ② fs pipe（匿名管道；读端先、写端后）
read_fd, write_fd = os.pipe()
os.write(write_fd, b"hi")
payload = os.read(read_fd, 2)
os.close(read_fd)
os.close(write_fd)
pipe_ok = 1 if payload == b"hi" else 0

# ④ dev/ino 真值（卷序列号／文件索引；拿当前的 guest 根目录验）
root_stat = os.stat(".")
devino_ok = 1 if (root_stat.st_dev != 0 and root_stat.st_ino != 0) else 0

# ③ net socketpair（loopback TCP 对）
left, right = socket.socketpair()
left.sendall(b"ping")
received = right.recv(4)
left.close()
right.close()
socketpair_ok = 1 if received == b"ping" else 0
"#;
    let rc = abi_exec(state, script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    for name in [
        "clock_fields_ok",
        "strftime_ok",
        "mktime_ok",
        "gmtoff_ok",
        "pipe_ok",
        "socketpair_ok",
        "devino_ok",
    ] {
        assert_eq!(
            abi_global_int(state, name),
            Some(1),
            "{name} 不等于 1（Windows 平台面补齐不完整？）：{}",
            abi_errmsg(state)
        );
    }
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
