//! **Windows 注册表的真实 Win32 语义**（`PLAN §12.24` 的"真实语义首批"）。
//!
//! # 分层（`CX-4` 的硬约束）
//!
//! 平台 FFI **只在本 crate**（`pyawa-runtime` 是平台依赖的集中点）。`pyawa-stdlib` 的 `winreg`
//! 模块保留"诚实桩"（名字齐、调用如实 `NotImplementedError`），由本模块在**便利装配路径**
//! （`paL_newstate`／CLI）上把同名函数**替换**成真实实现：
//!
//! * `paL_newstate`（电池版）与 CLI：真实注册表；
//! * 裸 ABI 宿主（自己 `pa_create` ＋ `pyawa_stdlib::install`）：仍是诚实桩——平台钩子要宿主
//!   显式选择（如实登记，见 `PLAN §12.24`）。
//!
//! # 真实语义首批（照 CPython 3.14 `winreg` 的常用面）
//!
//! `OpenKey`／`OpenKeyEx`／`CreateKey`／`CreateKeyEx`／`CloseKey`／`FlushKey`／
//! `SetValue`／`SetValueEx`／`QueryValue`／`QueryValueEx`／`EnumKey`／`EnumValue`／
//! `DeleteValue`／`DeleteKey`。
//!
//! 值类型：`REG_SZ`／`REG_EXPAND_SZ`（str）、`REG_DWORD`／`REG_QWORD`（int）、
//! `REG_BINARY`（bytes）、`REG_MULTI_SZ`（list[str]）。
//!
//! # 如实边界
//!
//! * 异常类型只映射常用的三种（`FileNotFoundError`／`PermissionError`／`OSError`）；
//!   **`OSError.winerror` 属性没有接**（消息里带 `[WinError n]`，属性面见 `§12.24` 剩余）；
//! * 安全描述符（`PySECURITY_ATTRIBUTES`）／`REG_LINK`／`REG_NONE` 等长尾类型未接；
//! * 句柄是 VM 侧旁表里的 `int`（与 `fd`／socket 句柄同一"provider 自管号段"手法，`CP-31`）。

use core::ffi::c_void;
use core::ptr::NonNull;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance};

/// Win32 注册表句柄（`HKEY`；64 位下是指针宽度）。
type Hkey = isize;

/// 预定义根键（`winreg` 模块常量已在 stdlib 桩里登记；这里只需认出它们）。
fn predefined(value: i64) -> Option<Hkey> {
    matches!(value, 0x8000_0000..=0x8000_0004).then_some((value as u32 as i32) as Hkey)
}

const KEY_READ: u32 = 0x20019;
const KEY_ALL_ACCESS: u32 = 0xF003F;
const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const REG_BINARY: u32 = 3;
const REG_DWORD: u32 = 4;
const REG_MULTI_SZ: u32 = 7;
const REG_QWORD: u32 = 11;

/// `ERROR_MORE_DATA`：两段式调用的"先问大小"阶段，Windows 常以它表示"再给缓冲"。
const ERROR_MORE_DATA: i32 = 234;

/// **错误码 → 异常**（常用三种；其余一律 `OSError`，消息带 `[WinError n]`）。
fn raise_win(instance: &Instance, code: i32) -> ExecError {
    let name = match code {
        2 | 3 | 1168 => "FileNotFoundError",
        5 => "PermissionError",
        _ => "OSError",
    };
    instance.raise_builtin_error(name, &format!("[WinError {code}] Windows registry error"))
}

/// 打开键的旁表（`CP-31` 的"provider 自管号段"；从 1 起）。
static KEYS: Mutex<Option<HashMap<u64, Hkey>>> = Mutex::new(None);
static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

fn table() -> std::sync::MutexGuard<'static, Option<HashMap<u64, Hkey>>> {
    KEYS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn store(hkey: Hkey) -> i64 {
    let id = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
    table()
        .get_or_insert_with(HashMap::new)
        .insert(id, hkey);
    id as i64
}

fn handle_of(instance: &Instance, value: i64) -> Result<Hkey, ExecError> {
    if let Some(hkey) = predefined(value) {
        return Ok(hkey);
    }
    let key = u64::try_from(value).ok().filter(|key| *key != 0);
    let Some(hkey) = key.and_then(|key| table().as_ref().and_then(|keys| keys.get(&key).copied())) else {
        return Err(instance.raise_builtin_error("OSError", "无效的注册表键句柄"));
    };
    Ok(hkey)
}

fn release_key(value: i64) -> Option<Hkey> {
    let key = u64::try_from(value).ok()?;
    table().as_mut().and_then(|keys| keys.remove(&key))
}

/// UTF-8 `str` ⇒ NUL 结尾 UTF-16（Win32 宽字符 API）。
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(core::iter::once(0)).collect()
}

fn wide_bytes(text: &str) -> Vec<u8> {
    let units = wide(text);
    let mut bytes = Vec::with_capacity(units.len() * 2);
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

/// UTF-16LE 字节 ⇒ `str`（去掉末尾 NUL；`REG_MULTI_SZ` 由调用方先切段）。
fn decode_utf16_le(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    while units.last() == Some(&0) {
        units.pop();
    }
    String::from_utf16_lossy(&units)
}

// --------------------------------------------------------------------------- //
// Win32 FFI（`advapi32`；本 crate 是平台集中点）
// --------------------------------------------------------------------------- //

#[allow(unsafe_code)]
mod sys {
    use super::{c_void, Hkey};

    #[link(name = "advapi32")]
    unsafe extern "system" {
        pub fn RegOpenKeyExW(
            hkey: Hkey,
            sub_key: *const u16,
            options: u32,
            sam: u32,
            out: *mut Hkey,
        ) -> i32;
        pub fn RegCreateKeyExW(
            hkey: Hkey,
            sub_key: *const u16,
            reserved: u32,
            class: *const u16,
            options: u32,
            sam: u32,
            security: *mut c_void,
            out: *mut Hkey,
            disposition: *mut u32,
        ) -> i32;
        pub fn RegCloseKey(hkey: Hkey) -> i32;
        pub fn RegFlushKey(hkey: Hkey) -> i32;
        pub fn RegSetValueExW(
            hkey: Hkey,
            name: *const u16,
            reserved: u32,
            ty: u32,
            data: *const u8,
            count: u32,
        ) -> i32;
        pub fn RegQueryValueExW(
            hkey: Hkey,
            name: *const u16,
            reserved: *mut u32,
            ty: *mut u32,
            data: *mut u8,
            count: *mut u32,
        ) -> i32;
        pub fn RegEnumKeyExW(
            hkey: Hkey,
            index: u32,
            name: *mut u16,
            name_len: *mut u32,
            reserved: *mut u32,
            class: *mut u16,
            class_len: *mut u32,
            last_write: *mut u64,
        ) -> i32;
        pub fn RegEnumValueW(
            hkey: Hkey,
            index: u32,
            name: *mut u16,
            name_len: *mut u32,
            reserved: *mut u32,
            ty: *mut u32,
            data: *mut u8,
            data_len: *mut u32,
        ) -> i32;
        pub fn RegDeleteValueW(hkey: Hkey, name: *const u16) -> i32;
        pub fn RegDeleteKeyExW(hkey: Hkey, sub_key: *const u16, sam: u32, reserved: u32) -> i32;
    }
}

// --------------------------------------------------------------------------- //
// 实参读取（与 stdlib 的 clinic 口径同形：整数/字符串；kwargs 不支持）
// --------------------------------------------------------------------------- //

fn reject_kwargs(instance: &Instance, name: &str, kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<(), ExecError> {
    if kwargs.is_empty() {
        return Ok(());
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("{name}() 不支持关键字实参（§12.24 首批口径）"),
    ))
}

fn arg<'a>(
    instance: &Instance,
    name: &str,
    args: &'a [NonNull<Header>],
    index: usize,
) -> Result<NonNull<Header>, ExecError> {
    args.get(index).copied().ok_or_else(|| {
        instance.raise_builtin_error("TypeError", &format!("{name}() 缺少第 {} 个实参", index + 1))
    })
}

fn int_arg(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    index: usize,
) -> Result<i64, ExecError> {
    let object = arg(instance, name, args, index)?;
    instance.int_value(object).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", &format!("{name}() 的第 {} 个实参应为整数", index + 1))
    })
}

fn optional_int_arg(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    index: usize,
    fallback: i64,
) -> Result<i64, ExecError> {
    match args.get(index) {
        Some(object) => instance.int_value(*object).ok_or_else(|| {
            instance.raise_builtin_error("TypeError", &format!("{name}() 的第 {} 个实参应为整数", index + 1))
        }),
        None => Ok(fallback),
    }
}

fn str_arg(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    index: usize,
) -> Result<String, ExecError> {
    let object = arg(instance, name, args, index)?;
    instance
        .text_of(object)
        .map(str::to_owned)
        .ok_or_else(|| {
            instance.raise_builtin_error("TypeError", &format!("{name}() 的第 {} 个实参应为字符串", index + 1))
        })
}

// --------------------------------------------------------------------------- //
// 各 API 的真实实现
// --------------------------------------------------------------------------- //

#[allow(unsafe_code)]
unsafe fn open_key(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "OpenKey", kwargs)?;
    let root = handle_of(instance, int_arg(instance, "OpenKey", args, 0)?)?;
    let sub_key = str_arg(instance, "OpenKey", args, 1)?;
    let _reserved = optional_int_arg(instance, "OpenKey", args, 2, 0)?;
    let sam = optional_int_arg(instance, "OpenKey", args, 3, i64::from(KEY_READ))?;
    let sub = wide(&sub_key);
    let mut out: Hkey = 0;
    // SAFETY: `sub` 是 NUL 结尾 UTF-16；`out` 是活局部量。
    let status = unsafe { sys::RegOpenKeyExW(root, sub.as_ptr(), 0, sam as u32, &mut out) };
    if status != 0 {
        return Err(raise_win(instance, status));
    }
    Ok(instance.new_int(store(out)))
}

#[allow(unsafe_code)]
unsafe fn open_key_ex(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 同 `open_key`（实参契约由调用方保证）。
    unsafe { open_key(instance, bound, args, kwargs) }
}

#[allow(unsafe_code)]
unsafe fn create_key(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "CreateKey", kwargs)?;
    let root = handle_of(instance, int_arg(instance, "CreateKey", args, 0)?)?;
    let sub_key = str_arg(instance, "CreateKey", args, 1)?;
    let sub = wide(&sub_key);
    let mut out: Hkey = 0;
    let mut disposition: u32 = 0;
    // SAFETY: 同 OpenKey；class／security 传 NULL（首批不接安全描述符）。
    let status = unsafe {
        sys::RegCreateKeyExW(
            root,
            sub.as_ptr(),
            0,
            core::ptr::null(),
            0,
            KEY_ALL_ACCESS,
            core::ptr::null_mut::<c_void>().cast(),
            &mut out,
            &mut disposition,
        )
    };
    if status != 0 {
        return Err(raise_win(instance, status));
    }
    Ok(instance.new_int(store(out)))
}

#[allow(unsafe_code)]
unsafe fn create_key_ex(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // `CreateKeyEx` 的 `class`／`options`／`sam` 面首批按 `CreateKey` 处理（如实边界）。
    // SAFETY: 同 `create_key`（实参契约由调用方保证）。
    unsafe { create_key(instance, bound, args, kwargs) }
}

#[allow(unsafe_code)]
unsafe fn close_key(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "CloseKey", kwargs)?;
    let value = int_arg(instance, "CloseKey", args, 0)?;
    let Some(hkey) = release_key(value) else {
        return Err(instance.raise_builtin_error("OSError", "无效的注册表键句柄"));
    };
    // SAFETY: hkey 来自本模块的旁表，且已摘下（关闭一次）。
    let status = unsafe { sys::RegCloseKey(hkey) };
    if status != 0 {
        return Err(raise_win(instance, status));
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
unsafe fn flush_key(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "FlushKey", kwargs)?;
    let hkey = handle_of(instance, int_arg(instance, "FlushKey", args, 0)?)?;
    // SAFETY: hkey 由旁表／预定义常量给出。
    let status = unsafe { sys::RegFlushKey(hkey) };
    if status != 0 {
        return Err(raise_win(instance, status));
    }
    Ok(instance.new_none())
}

/// 把 Python 值编码成注册表数据（`type` 的 winreg 常量）。
fn encode_value(
    instance: &Instance,
    ty: i64,
    value: NonNull<Header>,
) -> Result<Vec<u8>, ExecError> {
    match ty as u32 {
        REG_SZ | REG_EXPAND_SZ => {
            let text = instance.text_of(value).ok_or_else(|| {
                instance.raise_builtin_error("TypeError", "注册表字符串值应为 str")
            })?;
            Ok(wide_bytes(text))
        }
        REG_DWORD => {
            let number = instance.int_value(value).ok_or_else(|| {
                instance.raise_builtin_error("TypeError", "REG_DWORD 值应为 int")
            })?;
            let dword = u32::try_from(number).map_err(|_| {
                instance.raise_builtin_error("OverflowError", "REG_DWORD 值超出 32 位")
            })?;
            Ok(dword.to_le_bytes().to_vec())
        }
        REG_QWORD => {
            let number = instance.int_value(value).ok_or_else(|| {
                instance.raise_builtin_error("TypeError", "REG_QWORD 值应为 int")
            })?;
            Ok((number as u64).to_le_bytes().to_vec())
        }
        REG_BINARY => {
            let bytes = instance.bytes_value(value).ok_or_else(|| {
                instance.raise_builtin_error("TypeError", "REG_BINARY 值应为 bytes")
            })?;
            Ok(bytes.to_vec())
        }
        REG_MULTI_SZ => {
            let Some(items) = instance.list_items(value) else {
                return Err(instance.raise_builtin_error("TypeError", "REG_MULTI_SZ 值应为 list[str]"));
            };
            let mut bytes = Vec::new();
            for item in items {
                let text = instance.text_of(item).ok_or_else(|| {
                    instance.raise_builtin_error("TypeError", "REG_MULTI_SZ 值应为 list[str]")
                })?;
                bytes.extend_from_slice(&wide_bytes(text));
            }
            // 空表也要以双 NUL 收尾（`REG_MULTI_SZ` 的规范）。
            bytes.extend_from_slice(&[0, 0]);
            Ok(bytes)
        }
        _ => Err(instance.raise_builtin_error(
            "NotImplementedError",
            "该注册表值类型尚未接线（§12.24 首批：SZ／EXPAND_SZ／DWORD／QWORD／BINARY／MULTI_SZ）",
        )),
    }
}

#[allow(unsafe_code)]
unsafe fn set_value_ex(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "SetValueEx", kwargs)?;
    let hkey = handle_of(instance, int_arg(instance, "SetValueEx", args, 0)?)?;
    let value_name = str_arg(instance, "SetValueEx", args, 1)?;
    let _reserved = optional_int_arg(instance, "SetValueEx", args, 2, 0)?;
    let ty = int_arg(instance, "SetValueEx", args, 3)?;
    let value = arg(instance, "SetValueEx", args, 4)?;
    let data = encode_value(instance, ty, value)?;
    let name = wide(&value_name);
    // SAFETY: `name` 是 NUL 结尾 UTF-16；`data` 在调用期间存活。
    let status = unsafe {
        sys::RegSetValueExW(
            hkey,
            name.as_ptr(),
            0,
            ty as u32,
            data.as_ptr(),
            data.len() as u32,
        )
    };
    if status != 0 {
        return Err(raise_win(instance, status));
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
unsafe fn set_value(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "SetValue", kwargs)?;
    // 参照口径：`SetValue(key, value_name, type, value)`（无 reserved）⇒ 转调同一个实现。
    let hkey = int_arg(instance, "SetValue", args, 0)?;
    let value_name = str_arg(instance, "SetValue", args, 1)?;
    let ty = int_arg(instance, "SetValue", args, 2)?;
    let value = arg(instance, "SetValue", args, 3)?;
    let forwarded = [
        instance.new_int(hkey),
        instance.new_str(&value_name),
        instance.new_int(ty),
        value,
    ];
    // forwarded 里的临时对象由这里释放；`value` 是调用方的借用（不计入释放）。
    // SAFETY: 转发实参都是本函数持有的活对象；`value` 是调用方的借用。
    let result = unsafe { set_value_ex(instance, None, &forwarded, &[]) };
    for object in forwarded.iter().take(3) {
        // SAFETY: 三个临时对象是上面新建的（引用计数 1），这里归还。
        unsafe { instance.release_object(object.as_ptr()) };
    }
    result
}

/// 按 winreg 常量解码注册表数据（`REG_*`）。
fn decode_value(
    instance: &Instance,
    ty: u32,
    bytes: &[u8],
) -> Result<NonNull<Header>, ExecError> {
    match ty {
        REG_SZ | REG_EXPAND_SZ => Ok(instance.new_str(&decode_utf16_le(bytes))),
        REG_DWORD => {
            let value = bytes
                .get(..4)
                .map(|slice| u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
                .unwrap_or(0);
            Ok(instance.new_int(i64::from(value)))
        }
        REG_QWORD => {
            let value = bytes
                .get(..8)
                .map(|slice| u64::from_le_bytes(slice.try_into().expect("长度已核")))
                .unwrap_or(0);
            Ok(instance.new_int(value as i64))
        }
        REG_BINARY => Ok(instance.new_bytes(bytes)),
        REG_MULTI_SZ => {
            let mut items: Vec<NonNull<Header>> = Vec::new();
            let mut current: Vec<u8> = Vec::new();
            for pair in bytes.chunks_exact(2) {
                let unit = u16::from_le_bytes([pair[0], pair[1]]);
                if unit == 0 {
                    if current.is_empty() {
                        break;
                    }
                    items.push(instance.new_str(&decode_utf16_le(&current)));
                    current.clear();
                } else {
                    current.extend_from_slice(pair);
                }
            }
            Ok(instance.new_list(items))
        }
        _ => Err(instance.raise_builtin_error(
            "NotImplementedError",
            "该注册表值类型尚未接线（§12.24 首批）",
        )),
    }
}

/// 读一个值（两段式：先问长度，再读缓冲）。
#[allow(unsafe_code)]
fn read_value(
    instance: &Instance,
    hkey: Hkey,
    value_name: &str,
) -> Result<(NonNull<Header>, u32), ExecError> {
    let name = wide(value_name);
    let mut ty: u32 = 0;
    let mut size: u32 = 0;
    // SAFETY: 名字是 NUL 结尾 UTF-16；先取类型与长度（data 传 NULL）。
    let status = unsafe {
        sys::RegQueryValueExW(
            hkey,
            name.as_ptr(),
            core::ptr::null_mut(),
            &mut ty,
            core::ptr::null_mut(),
            &mut size,
        )
    };
    if status != 0 && status != ERROR_MORE_DATA {
        return Err(raise_win(instance, status));
    }
    let mut buffer = vec![0u8; size as usize];
    if size > 0 {
        // SAFETY: buffer 按 size 给足。
        let status = unsafe {
            sys::RegQueryValueExW(
                hkey,
                name.as_ptr(),
                core::ptr::null_mut(),
                &mut ty,
                buffer.as_mut_ptr(),
                &mut size,
            )
        };
        if status != 0 {
            return Err(raise_win(instance, status));
        }
    }
    buffer.truncate(size as usize);
    Ok((decode_value(instance, ty, &buffer)?, ty))
}

#[allow(unsafe_code)]
unsafe fn query_value_ex(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "QueryValueEx", kwargs)?;
    let hkey = handle_of(instance, int_arg(instance, "QueryValueEx", args, 0)?)?;
    let value_name = str_arg(instance, "QueryValueEx", args, 1)?;
    let (value, ty) = read_value(instance, hkey, &value_name)?;
    Ok(instance.new_tuple(vec![value, instance.new_int(i64::from(ty))]))
}

#[allow(unsafe_code)]
unsafe fn query_value(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "QueryValue", kwargs)?;
    let hkey = handle_of(instance, int_arg(instance, "QueryValue", args, 0)?)?;
    let value_name = str_arg(instance, "QueryValue", args, 1)?;
    Ok(read_value(instance, hkey, &value_name)?.0)
}

#[allow(unsafe_code)]
unsafe fn enum_key(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "EnumKey", kwargs)?;
    let hkey = handle_of(instance, int_arg(instance, "EnumKey", args, 0)?)?;
    let index = int_arg(instance, "EnumKey", args, 1)?;
    let index = u32::try_from(index)
        .map_err(|_| instance.raise_builtin_error("OverflowError", "EnumKey 的索引超出 32 位"))?;
    // 名字缓冲：`ERROR_MORE_DATA` 时按返回长度扩容重试（最多 64 KiB，防死循环）。
    let mut name = vec![0u16; 512];
    loop {
        let mut length: u32 = name.len() as u32;
        // SAFETY: name 缓冲按 length 给足；其余出参为 NULL。
        let status = unsafe {
            sys::RegEnumKeyExW(
                hkey,
                index,
                name.as_mut_ptr(),
                &mut length,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };
        if status == ERROR_MORE_DATA && (length as usize) <= 32 * 1024 {
            name = vec![0u16; length as usize + 1];
            continue;
        }
        if status != 0 {
            return Err(raise_win(instance, status));
        }
        let text = String::from_utf16_lossy(&name[..length as usize]);
        return Ok(instance.new_str(&text));
    }
}

#[allow(unsafe_code)]
unsafe fn enum_value(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "EnumValue", kwargs)?;
    let hkey = handle_of(instance, int_arg(instance, "EnumValue", args, 0)?)?;
    let index = int_arg(instance, "EnumValue", args, 1)?;
    let index = u32::try_from(index)
        .map_err(|_| instance.raise_builtin_error("OverflowError", "EnumValue 的索引超出 32 位"))?;
    let mut name = vec![0u16; 512];
    let mut name_length: u32 = name.len() as u32;
    let mut ty: u32 = 0;
    let mut size: u32 = 0;
    // SAFETY: name 缓冲按 name_length 给足；两段式先问数据长度。
    let status = unsafe {
        sys::RegEnumValueW(
            hkey,
            index,
            name.as_mut_ptr(),
            &mut name_length,
            core::ptr::null_mut(),
            &mut ty,
            core::ptr::null_mut(),
            &mut size,
        )
    };
    if status != 0 && status != ERROR_MORE_DATA {
        return Err(raise_win(instance, status));
    }
    let mut data = vec![0u8; size as usize];
    if size > 0 {
        // 第二次调用：两段式出参的**缓冲区长度**必须重置为容量（首轮会把它改小成实际长度）。
        name_length = name.len() as u32;
        // SAFETY: data 按 size 给足。
        let status = unsafe {
            sys::RegEnumValueW(
                hkey,
                index,
                name.as_mut_ptr(),
                &mut name_length,
                core::ptr::null_mut(),
                &mut ty,
                data.as_mut_ptr(),
                &mut size,
            )
        };
        if status != 0 {
            return Err(raise_win(instance, status));
        }
    }
    data.truncate(size as usize);
    let value_name = String::from_utf16_lossy(&name[..name_length as usize]);
    let value = decode_value(instance, ty, &data)?;
    Ok(instance.new_tuple(vec![
        instance.new_str(&value_name),
        value,
        instance.new_int(i64::from(ty)),
    ]))
}

#[allow(unsafe_code)]
unsafe fn delete_value(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "DeleteValue", kwargs)?;
    let hkey = handle_of(instance, int_arg(instance, "DeleteValue", args, 0)?)?;
    let value_name = str_arg(instance, "DeleteValue", args, 1)?;
    let name = wide(&value_name);
    // SAFETY: name 是 NUL 结尾 UTF-16。
    let status = unsafe { sys::RegDeleteValueW(hkey, name.as_ptr()) };
    if status != 0 {
        return Err(raise_win(instance, status));
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
unsafe fn delete_key(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_kwargs(instance, "DeleteKey", kwargs)?;
    let hkey = handle_of(instance, int_arg(instance, "DeleteKey", args, 0)?)?;
    let sub_key = str_arg(instance, "DeleteKey", args, 1)?;
    let sub = wide(&sub_key);
    // SAFETY: sub 是 NUL 结尾 UTF-16。
    let status = unsafe { sys::RegDeleteKeyExW(hkey, sub.as_ptr(), KEY_ALL_ACCESS, 0) };
    if status != 0 {
        return Err(raise_win(instance, status));
    }
    Ok(instance.new_none())
}

// --------------------------------------------------------------------------- //
// 安装：把 stdlib `winreg` 模块里的同名桩替换成真实实现
// --------------------------------------------------------------------------- //

/// 造一个 `builtin_function_or_method`（与 stdlib 的 `make_native` 同形）。
fn native(instance: &Instance, name: &str, function: pyawa_core::NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(function),
    ));
    object.into_raw().cast::<Header>()
}

/// 本 crate 提供的真实面（名字 ⇒ native）。
const FUNCTIONS: &[(&str, pyawa_core::NativeFn)] = &[
    ("OpenKey", open_key),
    ("OpenKeyEx", open_key_ex),
    ("CreateKey", create_key),
    ("CreateKeyEx", create_key_ex),
    ("CloseKey", close_key),
    ("FlushKey", flush_key),
    ("SetValue", set_value),
    ("SetValueEx", set_value_ex),
    ("QueryValue", query_value),
    ("QueryValueEx", query_value_ex),
    ("EnumKey", enum_key),
    ("EnumValue", enum_value),
    ("DeleteValue", delete_value),
    ("DeleteKey", delete_key),
];

/// **把 `winreg` 模块替换成真实实现**（只在本 crate 的便利装配路径调用）。
///
/// 没装过 stdlib 的实例（或 `winreg` 模块缺失）⇒ 什么都不做（如实：调用面仍是诚实桩／不存在）。
pub fn install(instance: &Instance) {
    let Some(modules) = instance.modules() else {
        return;
    };
    let Some(module) = instance.dict_get(modules, "winreg") else {
        return;
    };
    let Some(namespace) = pyawa_core::mounted_instance_dict(instance, module) else {
        return;
    };
    for (name, function) in FUNCTIONS {
        let object = native(instance, name, *function);
        instance.dict_set(namespace, name, object);
    }
}
