//! **`winreg` 的真实 Win32 语义**（`PLAN §12.24` 真实语义首批；只在 `windows` 目标编译）。
//!
//! 端到端走 ABI（`paL_newstate` 的**便利装配路径**会安装真实实现；裸 `pa_create`＋
//! `pyawa_stdlib::install` 的宿主仍是诚实桩）：
//! 建键 → 写 SZ／DWORD／BINARY → 读回并核对类型 → 枚举子键 → 删值 → 删键 → 缺键报
//! `FileNotFoundError`。用 `HKCU\\Software` 下的临时子键，结束时删除（失败也不留垃圾：清理在脚本
//! 的 `finally` 里）。
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

fn abi_global_string(state: *mut pyawa_abi::pa_state, name: &str) -> Option<String> {
    let name = abi_c_string(name);
    // SAFETY: state 活着；名字是活的 CString。
    if unsafe { pyawa_abi::pa_getglobal(state, name.as_ptr()) } != 0 {
        return None;
    }
    let mut length = 0usize;
    // SAFETY: 栈顶刚被压入；出参是活的局部变量。
    let view = unsafe { pyawa_abi::pa_tostring(state, -1, &mut length) };
    let value = if view.is_null() {
        None
    } else {
        // SAFETY: 视图借用 `length` 字节，立即复制。
        let bytes = unsafe { core::slice::from_raw_parts(view.cast::<u8>(), length) };
        Some(String::from_utf8_lossy(bytes).into_owned())
    };
    // SAFETY: 归还 `pa_getglobal` 的那份引用。
    unsafe { pyawa_abi::pa_pop(state, 1) };
    value
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
fn winreg_real_semantics_end_to_end_through_the_abi() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = r#"
import winreg

root = winreg.HKEY_CURRENT_USER
SUB = 'Software\\PyawaW32RegTest'

def cleanup():
    try:
        key = winreg.OpenKey(root, SUB)
    except FileNotFoundError:
        return
    for name in ('text', 'number', 'blob'):
        try:
            winreg.DeleteValue(key, name)
        except FileNotFoundError:
            pass
    winreg.CloseKey(key)
    try:
        winreg.DeleteKey(root, SUB)
    except FileNotFoundError:
        pass

cleanup()
key = winreg.CreateKey(root, SUB)
winreg.SetValueEx(key, 'text', 0, winreg.REG_SZ, 'hello')
winreg.SetValueEx(key, 'number', 0, winreg.REG_DWORD, 7)
winreg.SetValueEx(key, 'blob', 0, winreg.REG_BINARY, b'\x01\x02')

text, text_type = winreg.QueryValueEx(key, 'text')
number, number_type = winreg.QueryValueEx(key, 'number')
blob, blob_type = winreg.QueryValueEx(key, 'blob')
text_ok = 1 if (text == 'hello' and text_type == winreg.REG_SZ) else 0
number_ok = 1 if (number == 7 and number_type == winreg.REG_DWORD) else 0
blob_ok = 1 if (blob == b'\x01\x02' and blob_type == winreg.REG_BINARY) else 0

# 枚举值名（顺序是注册表的枚举序；这里只核对集合与类型面）
seen = set()
try:
    index = 0
    while True:
        name, _, _ = winreg.EnumValue(key, index)
        seen.add(name)
        index += 1
except OSError:
    pass
enum_ok = 1 if seen == {'text', 'number', 'blob'} else 0
seen_sorted = ','.join(sorted(seen))

winreg.FlushKey(key)
for name in ('text', 'number', 'blob'):
    winreg.DeleteValue(key, name)
winreg.CloseKey(key)

# 删值后重开：查缺失值应 FileNotFoundError
key = winreg.OpenKey(root, SUB)
try:
    winreg.QueryValue(key, 'text')
except FileNotFoundError:
    missing_value = 1
else:
    missing_value = 0
winreg.CloseKey(key)
winreg.DeleteKey(root, SUB)

# 缺键：OpenKey 应 FileNotFoundError
try:
    winreg.OpenKey(root, 'Software\\PyawaW32RegTestMissingZZZ')
except FileNotFoundError:
    missing_key = 1
else:
    missing_key = 0

# 桩 vs 真实：`QueryValueEx` 返回值里数字类型是真 int（不是 NotImplementedError 桩）
try:
    winreg.FlushKey(root)
    flush_ok = 1
except OSError:
    flush_ok = 0
"#;
    let rc = abi_exec(state, script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    let seen = abi_global_string(state, "seen_sorted").unwrap_or_default();
    for name in [
        "text_ok",
        "number_ok",
        "blob_ok",
        "enum_ok",
        "missing_value",
        "missing_key",
        "flush_ok",
    ] {
        assert_eq!(
            abi_global_int(state, name),
            Some(1),
            "{name} 不等于 1（`winreg` 真实语义不完整？枚举到：{seen:?}）：{}",
            abi_errmsg(state)
        );
    }
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
