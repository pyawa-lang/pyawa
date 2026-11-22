//! **Windows-only C 模块的最小导入面**（`PLAN §12.24`；只在 `windows` 目标编译）。
//!
//! 背景：W6 的 conformance Windows 收敛里，剩余最大一类是 `asyncio`／`concurrent.futures`／
//! `code` 一族的 Windows 分支在导入期就要 `winreg`／`_winapi`／`msvcrt`／`_overlapped`
//! ⇒ `ModuleNotFoundError` 挡住整条导入链。本层给的是**诚实桩**：
//! 1. **导入成功**且 `sys.builtin_module_names` 里有它们（模块表＝一处真相）；
//! 2. 未接调用**如实** `NotImplementedError`（`CM-8`：不静默给假值）；`winreg` 在
//!    `paL_newstate` 便利路径上已是**真实实现**（§12.24），这里改钉"缺键 → FileNotFoundError"；
//! 3. `_winapi` 的**导入期常量**（AST 扫描出的 4 个）是真整数值，不是函数对象。
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
fn windows_only_c_modules_import_surface_end_to_end_through_the_abi() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = r#"
import sys
import winreg, _winapi, msvcrt, _overlapped, _multiprocessing

builtin_ok = 1 if all(
    name in sys.builtin_module_names
    for name in ('winreg', '_winapi', 'msvcrt', '_overlapped', '_multiprocessing')
) else 0
constants_ok = 1 if (
    _winapi.NULL == 0
    and _winapi.INFINITE == 0xFFFFFFFF
    and _winapi.ERROR_BROKEN_PIPE == 109
    and _winapi.ERROR_NETNAME_DELETED == 64
) else 0
# `paL_newstate` 是**便利装配路径** ⇒ `winreg` 已是真实 Win32 实现（§12.24 首批）：
# 缺键应报 `FileNotFoundError`，而不是桩的 `NotImplementedError`。
try:
    winreg.OpenKey(winreg.HKEY_CURRENT_USER, 'Software\\PyawaW32RegTestMissingZZZ')
except FileNotFoundError:
    winreg_real = 1
except BaseException:
    winreg_real = 0
else:
    winreg_real = 0
try:
    msvcrt.get_osfhandle(0)
except NotImplementedError:
    msvcrt_raises = 1
except BaseException:
    msvcrt_raises = 0
else:
    msvcrt_raises = 0
names_ok = 1 if ('Overlapped' in dir(_overlapped) and 'OpenKey' in dir(winreg)) else 0

# `Lib/subprocess.py`／`multiprocessing/connection.py` 的 from-import 面（缺一个就 ImportError）
from _winapi import CREATE_NEW_CONSOLE, CREATE_NO_WINDOW, WAIT_TIMEOUT, INFINITE
from_constants_ok = 1 if (
    CREATE_NEW_CONSOLE == 0x10
    and CREATE_NO_WINDOW == 0x08000000
    and WAIT_TIMEOUT == 258
    and INFINITE == 0xFFFFFFFF
) else 0
# `Lib/test` 的 `from winreg import (...)` 面
from winreg import HKEY_CURRENT_USER, REG_SZ, KEY_ALL_ACCESS
winreg_constants_ok = 1 if (
    HKEY_CURRENT_USER == 0x80000001
    and REG_SZ == 1
    and KEY_ALL_ACCESS == 0xF003F
) else 0
# `ntpath` 必须走它自带的纯 Python fallback（我们**故意不提供** LCMapStringEx 三件套）
import ntpath
normcase_ok = 1 if ntpath.normcase('A/B') == 'a\\b' else 0
# `Lib/subprocess.py` 的**函数默认参数**在导入期求值（实测 AttributeError 来源）
getexitcode_ok = 1 if hasattr(_winapi, 'GetExitCodeProcess') else 0
multiprocessing_ok = 1 if hasattr(_multiprocessing, 'SemLock') else 0
"#;
    let rc = abi_exec(state, script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    for name in [
        "builtin_ok",
        "constants_ok",
        "winreg_real",
        "msvcrt_raises",
        "names_ok",
        "from_constants_ok",
        "winreg_constants_ok",
        "normcase_ok",
        "getexitcode_ok",
        "multiprocessing_ok",
    ] {
        assert_eq!(
            abi_global_int(state, name),
            Some(1),
            "{name} 不等于 1（§12.24 的最小导入面不完整？）：{}",
            abi_errmsg(state)
        );
    }
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
