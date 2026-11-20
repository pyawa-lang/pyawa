//! **Windows `os`／`sys.platform` 形状测试**（`PLAN §12.20` W5；只在 `windows` 目标编译）。
//!
//! 端到端走 ABI（**不经 CLI**）：`paL_newstate`（Windows 组合根 = 真实 fs／clock／net 能力 ＋
//! stdlib 装配）→ `pa_exec_string` 跑脚本 → `pa_getglobal` ＋ `pa_tointeger`／`pa_tostring` 读回。
//!
//! 验收（W5）：
//! 1. `sys.platform == 'win32'`，且 `sys.builtin_module_names` 里有 `nt`、**没有** `posix`
//!    （表由 `pyawa-stdlib` 的 `RUST_MODULES` 派生 ⇒ 模块表换了名字，这里就必须跟着变）；
//! 2. `Lib/os.py` 走 `elif 'nt' in _names:` 分支 ⇒ `os.name == 'nt'`、`os.path is ntpath`、
//!    `os.sep == '\\'`、`os.path.join('a', 'b') == 'a\\b'`；
//! 3. `os.getcwd()` 非空、`os.listdir('.')` 可枚举 ⇒ `fs` provider（W3）与 `sys.path` 的
//!    guest 名转换（`D:\…` ⇒ `/D:/…`）都真接上了（否则 `import os` 第一步就找不到 `Lib/os.py`）。
#![cfg(windows)]
// ABI 入口是 `unsafe extern "C"`；按任务约定逐次写 `unsafe {}` ＋ `SAFETY:` 注释（与
// `windows_fs_shapes.rs`／`windows_clock_shapes.rs` 同口径）⇒ 关掉"多余 unsafe"提醒。
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

fn abi_global_string(state: *mut pyawa_abi::pa_state, name: &str) -> Option<String> {
    let name = abi_c_string(name);
    // SAFETY: state 活着；名字是活的 CString。
    if unsafe { pyawa_abi::pa_getglobal(state, name.as_ptr()) } != 0 {
        return None;
    }
    let mut length = 0usize;
    // SAFETY: 栈顶刚被压入；出参是活的局部变量。
    let view = unsafe { pyawa_abi::pa_tostring(state, -1, &mut length) };
    // 视图是**借用**（`AB-48`）：下一次可能改写它的调用之前必须复制走 ⇒ 立刻拷成 String。
    let value = if view.is_null() {
        None
    } else {
        // SAFETY: `pa_tostring` 约定前 `length` 字节可读；本调用后立即复制，不再持有。
        let bytes = unsafe { core::slice::from_raw_parts(view.cast::<u8>(), length) };
        Some(String::from_utf8_lossy(bytes).into_owned())
    };
    // SAFETY: 归还 `pa_getglobal` 的那份引用。
    unsafe { pyawa_abi::pa_pop(state, 1) };
    value
}

/// guest 空间里的 cwd 形态（`path.rs` 的 `normalize_lexically` 在 Windows 上把宿主 cwd
/// `D:\a\b` 归一成 guest 绝对名 `/D:/a/b`）⇒ 以 `/` 开头；按任务口径**放宽**为也接受
/// 盘符开头（如 `D:…`），只看"非空且不是相对名"。
fn is_absoluteish(path: &str) -> bool {
    if path.starts_with('/') {
        return true;
    }
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[test]
fn windows_os_shapes_end_to_end_through_the_abi() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = r#"
import sys, os
platform_ok = 1 if sys.platform == 'win32' else 0
nt_builtin_ok = 1 if 'nt' in sys.builtin_module_names else 0
no_posix_ok = 1 if 'posix' not in sys.builtin_module_names else 0
name_ok = 1 if os.name == 'nt' else 0
path_is_ntpath_ok = 1 if os.path is sys.modules['ntpath'] else 0
sep_ok = 1 if os.sep == '\\' else 0
joined_ok = 1 if os.path.join('a', 'b') == 'a\\b' else 0
cwd = os.getcwd()
count = len(os.listdir('.'))
"#;
    let rc = abi_exec(state, script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    for name in [
        "platform_ok",
        "nt_builtin_ok",
        "no_posix_ok",
        "name_ok",
        "path_is_ntpath_ok",
        "sep_ok",
        "joined_ok",
    ] {
        assert_eq!(
            abi_global_int(state, name),
            Some(1),
            "{name} 不等于 1（`import os` 走的不是 nt 分支？）：{}",
            abi_errmsg(state)
        );
    }
    let cwd = abi_global_string(state, "cwd")
        .unwrap_or_else(|| panic!("读不到 os.getcwd() 的返回值：{}", abi_errmsg(state)));
    assert!(!cwd.is_empty(), "os.getcwd() 不得为空：{}", abi_errmsg(state));
    assert!(
        is_absoluteish(&cwd),
        "os.getcwd() 应是 guest 绝对名（形如 /D:/…）或盘符开头，实际 {cwd:?}：{}",
        abi_errmsg(state)
    );
    let count = abi_global_int(state, "count")
        .unwrap_or_else(|| panic!("读不到 os.listdir('.') 的长度：{}", abi_errmsg(state)));
    assert!(
        count >= 0,
        "os.listdir('.') 应返回可枚举的目录项（fs provider 未接？），实际长度 {count}：{}",
        abi_errmsg(state)
    );
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
