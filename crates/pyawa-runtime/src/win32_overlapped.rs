//! **`_winapi`／`_overlapped` 的最小真实语义**（`PLAN §12.24` 真实语义第二批）。
//!
//! # 分层（`CX-4`）
//!
//! 平台 FFI 只在本 crate（`pyawa-runtime`）；`pyawa-stdlib` 的 `_winapi`／`_overlapped`
//! 桩由**便利装配路径**（`paL_newstate`／CLI）替换/覆盖（与 `win32_reg` 同一手法）。
//!
//! # 真实现面（首批）
//!
//! - `_winapi`：`CreateEvent`／`SetEvent`／`ResetEvent`／`CloseHandle`／
//!   `WaitForSingleObject`／`CreateIoCompletionPort`／`PostQueuedCompletionStatus`／
//!   `GetQueuedCompletionStatus`／`GetLastError`／`GetCurrentProcessId`／`GetCurrentThreadId`
//!   —— 都是**真 Win32 调用**；句柄走本模块的旁表（`int` 编号，`CP-31` provider 自管号段手法）。
//! - `_overlapped.Overlapped`：**真**对象（一个 `OVERLAPPED` 内存 ＋ 真事件句柄），
//!   `Close()`／`event`／`address`／`GetOverlappedResult()` 是真语义。
//! - **P1 文件重叠 I/O（2026-11-22）**：`_winapi.CreateFile`＋`Overlapped.ReadFile`／
//!   `WriteFile`／`GetOverlappedResult`／`getresult` 已接真实 Win32；Windows 测试见
//!   `crates/pyawa-runtime/tests/windows_overlapped.rs`。
//!
//! # 如实边界（`DIV-` 口径）
//!
//! **P1 文件**已接；**pipe/socket** 的 `ReadFileInto`／`WSARecv`／`WSASend`／
//! `ConnectNamedPipe` 仍如实 `NotImplementedError`（不假装成功）。因此 Windows Proactor
//! 事件循环仍**不能**完整跑 subprocess/pipe；剩余见
//! `docs/rounds/spikes/2026-11-22-proactor-dataplane-plan.md`。

use core::ffi::c_void;
use core::ptr::NonNull;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance};

/// `_winapi` 的句柄（Win32 `HANDLE`）。
type HandleRaw = isize;

#[repr(C)]
#[derive(Default)]
struct OverlappedInner {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: HandleRaw,
}

struct OverlappedMemory {
    raw: OverlappedInner,
    /// 重叠 I/O 的缓冲区（必须活到完成包归还）。
    buffer: Vec<u8>,
    /// 发起 I/O 的原始 Win32 句柄（`GetOverlappedResult` 用）。
    handle: HandleRaw,
    /// 操作种类：0=无、1=读、2=写。
    kind: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HandleKind {
    /// 事件（`CreateEvent`）。
    Event,
    /// 完成端口（`CreateIoCompletionPort`）。
    Iocp,
}

struct HandleEntry {
    raw: HandleRaw,
    kind: HandleKind,
}

static HANDLES: Mutex<Option<HashMap<u64, HandleEntry>>> = Mutex::new(None);
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

fn table() -> std::sync::MutexGuard<'static, Option<HashMap<u64, HandleEntry>>> {
    HANDLES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn store(raw: HandleRaw, kind: HandleKind) -> i64 {
    let id = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    table()
        .get_or_insert_with(HashMap::new)
        .insert(id, HandleEntry { raw, kind });
    id as i64
}

fn entry_of(instance: &Instance, value: i64) -> Result<(u64, HandleRaw, HandleKind), ExecError> {
    let id = u64::try_from(value).ok();
    let found = id.and_then(|id| table().as_ref().and_then(|map| map.get(&id).map(|entry| (entry.raw, entry.kind))));
    match (id, found) {
        (Some(id), Some((raw, kind))) => Ok((id, raw, kind)),
        _ => Err(instance.raise_builtin_error("OSError", "[WinError 6] 无效的 Win32 句柄")),
    }
}

fn release_handle(instance: &Instance, value: i64) -> Result<HandleRaw, ExecError> {
    if let Ok((id, raw, _)) = entry_of(instance, value) {
        if table().as_mut().and_then(|map| map.remove(&id)).is_some() {
            return Ok(raw);
        }
    }
    // `CreateFile` 交回的是**裸 Win32 句柄**（CPython `_winapi` 口径），不在旁表里。
    if value > 0 {
        return Ok(value as HandleRaw);
    }
    Err(instance.raise_builtin_error("OSError", "[WinError 6] 无效的 Win32 句柄"))
}

fn raise_win(instance: &Instance, code: u32) -> ExecError {
    instance.raise_builtin_error("OSError", &format!("[WinError {code}] Win32 调用失败"))
}

#[allow(unsafe_code)]
fn last_error() -> u32 {
    // SAFETY: 无参、线程局部读。
    unsafe { sys::GetLastError() }
}

/// UTF-8 ⇒ NUL 结尾 UTF-16（`None` ⇒ NULL）。
fn wide_opt(text: Option<&str>) -> Option<Vec<u16>> {
    text.map(|text| text.encode_utf16().chain(core::iter::once(0)).collect())
}

// --------------------------------------------------------------------------- //
// Win32 FFI（`kernel32`；本 crate 是平台集中点）
// --------------------------------------------------------------------------- //

#[allow(unsafe_code)]
mod sys {
    use super::{c_void, HandleRaw, OverlappedInner};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn CreateEventW(
            attributes: *mut c_void,
            manual_reset: i32,
            initial_state: i32,
            name: *const u16,
        ) -> HandleRaw;
        pub fn SetEvent(handle: HandleRaw) -> i32;
        pub fn ResetEvent(handle: HandleRaw) -> i32;
        pub fn CloseHandle(handle: HandleRaw) -> i32;
        pub fn WaitForSingleObject(handle: HandleRaw, milliseconds: u32) -> u32;
        pub fn GetLastError() -> u32;
        pub fn CreateIoCompletionPort(
            file_handle: HandleRaw,
            existing: HandleRaw,
            key: usize,
            threads: u32,
        ) -> HandleRaw;
        pub fn PostQueuedCompletionStatus(
            iocp: HandleRaw,
            transferred: u32,
            key: usize,
            overlapped: *mut OverlappedInner,
        ) -> i32;
        pub fn GetQueuedCompletionStatus(
            iocp: HandleRaw,
            transferred: *mut u32,
            key: *mut usize,
            overlapped: *mut *mut OverlappedInner,
            timeout: u32,
        ) -> i32;
        pub fn GetCurrentProcessId() -> u32;
        pub fn GetCurrentThreadId() -> u32;
        pub fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: HandleRaw,
        ) -> HandleRaw;
        pub fn ReadFile(
            file: HandleRaw,
            buffer: *mut c_void,
            bytes: u32,
            transferred: *mut u32,
            overlapped: *mut OverlappedInner,
        ) -> i32;
        pub fn WriteFile(
            file: HandleRaw,
            buffer: *const c_void,
            bytes: u32,
            transferred: *mut u32,
            overlapped: *mut OverlappedInner,
        ) -> i32;
        pub fn GetOverlappedResult(
            file: HandleRaw,
            overlapped: *mut OverlappedInner,
            transferred: *mut u32,
            wait: i32,
        ) -> i32;
        pub fn CreateNamedPipeW(
            name: *const u16,
            open_mode: u32,
            pipe_mode: u32,
            max_instances: u32,
            out_buffer_size: u32,
            in_buffer_size: u32,
            default_timeout: u32,
            security: *mut c_void,
        ) -> HandleRaw;
        pub fn ConnectNamedPipe(pipe: HandleRaw, overlapped: *mut OverlappedInner) -> i32;
    }
}

#[repr(C)]
struct WsaBuf {
    len: u32,
    buf: *mut u8,
}

#[allow(unsafe_code)]
mod winsock {
    use super::{c_void, HandleRaw, OverlappedInner, WsaBuf};

    #[link(name = "ws2_32")]
    unsafe extern "system" {
        pub fn WSARecv(
            socket: HandleRaw,
            buffers: *mut WsaBuf,
            count: u32,
            transferred: *mut u32,
            flags: *mut u32,
            overlapped: *mut OverlappedInner,
            completion: *mut c_void,
        ) -> i32;
        pub fn WSASend(
            socket: HandleRaw,
            buffers: *const WsaBuf,
            count: u32,
            transferred: *mut u32,
            flags: u32,
            overlapped: *mut OverlappedInner,
            completion: *mut c_void,
        ) -> i32;
        pub fn WSAGetLastError() -> i32;
    }
}

/// provider 句柄 ⇒ 原生 SOCKET；非 net 句柄按原值当 raw（`_winapi` 直建 socket 的路径）。
fn resolve_socket(handle: i64) -> HandleRaw {
    crate::net_windows::raw_socket_for_handle(handle).map(|socket| socket as HandleRaw).unwrap_or(handle as HandleRaw)
}

// --------------------------------------------------------------------------- //
// 参数工具（与 winreg 同口径：整数/字符串/None；不支持 kwargs）
// --------------------------------------------------------------------------- //

fn no_kwargs(instance: &Instance, name: &str, kwargs: &[(NonNull<Header>, NonNull<Header>)]) -> Result<(), ExecError> {
    if kwargs.is_empty() {
        Ok(())
    } else {
        Err(instance.raise_builtin_error("TypeError", &format!("{name}() 不支持关键字实参")))
    }
}

fn arg(instance: &Instance, name: &str, args: &[NonNull<Header>], index: usize) -> Result<NonNull<Header>, ExecError> {
    args.get(index).copied().ok_or_else(|| {
        instance.raise_builtin_error("TypeError", &format!("{name}() 缺少第 {} 个实参", index + 1))
    })
}

fn int_arg(instance: &Instance, name: &str, args: &[NonNull<Header>], index: usize) -> Result<i64, ExecError> {
    let object = arg(instance, name, args, index)?;
    instance.int_value(object).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", &format!("{name}() 的第 {} 个实参应为整数", index + 1))
    })
}

fn optional_int(instance: &Instance, name: &str, args: &[NonNull<Header>], index: usize, fallback: i64) -> Result<i64, ExecError> {
    match args.get(index) {
        Some(object) => instance.int_value(*object).ok_or_else(|| {
            instance.raise_builtin_error("TypeError", &format!("{name}() 的第 {} 个实参应为整数", index + 1))
        }),
        None => Ok(fallback),
    }
}

fn optional_text(instance: &Instance, name: &str, args: &[NonNull<Header>], index: usize) -> Result<Option<String>, ExecError> {
    match args.get(index) {
        None => Ok(None),
        Some(object) => {
            if *object == instance.singletons().none() {
                return Ok(None);
            }
            instance
                .text_of(*object)
                .map(|text| Some(text.to_owned()))
                .ok_or_else(|| {
                    instance.raise_builtin_error("TypeError", &format!("{name}() 的第 {} 个实参应为 str 或 None", index + 1))
                })
        }
    }
}

// --------------------------------------------------------------------------- //
// `_winapi` 真实函数
// --------------------------------------------------------------------------- //

#[allow(unsafe_code)]
fn create_event(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "CreateEvent", kwargs)?;
    let _security = arg(instance, "CreateEvent", args, 0)?; // 首批只接受 None（安全描述符未接）
    let manual = optional_int(instance, "CreateEvent", args, 1, 0)?;
    let initial = optional_int(instance, "CreateEvent", args, 2, 0)?;
    let name = optional_text(instance, "CreateEvent", args, 3)?;
    let name_wide = wide_opt(name.as_deref());
    let name_ptr = name_wide.as_ref().map_or(core::ptr::null(), |text| text.as_ptr());
    // SAFETY: name_ptr 要么 NULL、要么 NUL 结尾 UTF-16（上面刚构造）。
    let handle = unsafe { sys::CreateEventW(core::ptr::null_mut(), manual as i32, initial as i32, name_ptr) };
    if handle == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(store(handle, HandleKind::Event)))
}

#[allow(unsafe_code)]
fn create_file(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "CreateFile", kwargs)?;
    let path = optional_text(instance, "CreateFile", args, 0)?
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "CreateFile 需要路径"))?;
    // 常用默认：GENERIC_READ|GENERIC_WRITE；其余参数按 CPython 调用点给。
    let access = optional_int(instance, "CreateFile", args, 1, 0x8000_0000 | 0x4000_0000)? as u32;
    let share = optional_int(instance, "CreateFile", args, 2, 0)? as u32;
    let _security = args.get(3).copied();
    let disposition = optional_int(instance, "CreateFile", args, 4, 3)? as u32; // OPEN_EXISTING
    let flags = optional_int(instance, "CreateFile", args, 5, 0)? as u32;
    let wide = wide_opt(Some(&path)).expect("路径非 None");
    // SAFETY: wide 是 NUL 结尾 UTF-16；security/template 传 NULL/0。
    let handle = unsafe {
        sys::CreateFileW(
            wide.as_ptr(),
            access,
            share,
            core::ptr::null_mut(),
            disposition,
            flags,
            0,
        )
    };
    if handle == -1isize {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(handle as i64))
}

#[allow(unsafe_code)]
fn create_named_pipe(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "CreateNamedPipe", kwargs)?;
    let name = optional_text(instance, "CreateNamedPipe", args, 0)?
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "CreateNamedPipe 需要名字"))?;
    let open_mode = optional_int(instance, "CreateNamedPipe", args, 1, 3)? as u32; // PIPE_ACCESS_DUPLEX
    let pipe_mode = optional_int(instance, "CreateNamedPipe", args, 2, 0)? as u32;
    let max_instances = optional_int(instance, "CreateNamedPipe", args, 3, 1)? as u32;
    let out_size = optional_int(instance, "CreateNamedPipe", args, 4, 4096)? as u32;
    let in_size = optional_int(instance, "CreateNamedPipe", args, 5, 4096)? as u32;
    let timeout = optional_int(instance, "CreateNamedPipe", args, 6, 0)? as u32;
    let wide = wide_opt(Some(&name)).expect("名字非 None");
    // SAFETY: wide 是 NUL 结尾 UTF-16；security 传 NULL。
    let handle = unsafe {
        sys::CreateNamedPipeW(
            wide.as_ptr(),
            open_mode,
            pipe_mode,
            max_instances,
            out_size,
            in_size,
            timeout,
            core::ptr::null_mut(),
        )
    };
    if handle == -1isize {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(handle as i64))
}

#[allow(unsafe_code)]
fn set_event(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "SetEvent", kwargs)?;
    let (_, raw, kind) = entry_of(instance, int_arg(instance, "SetEvent", args, 0)?)?;
    if kind != HandleKind::Event {
        return Err(instance.raise_builtin_error("OSError", "[WinError 6] 句柄不是事件对象"));
    }
    // SAFETY: raw 来自本模块旁表（事件句柄）。
    if unsafe { sys::SetEvent(raw) } == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn reset_event(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "ResetEvent", kwargs)?;
    let (_, raw, kind) = entry_of(instance, int_arg(instance, "ResetEvent", args, 0)?)?;
    if kind != HandleKind::Event {
        return Err(instance.raise_builtin_error("OSError", "[WinError 6] 句柄不是事件对象"));
    }
    // SAFETY: 同上。
    if unsafe { sys::ResetEvent(raw) } == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn close_handle(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "CloseHandle", kwargs)?;
    let raw = release_handle(instance, int_arg(instance, "CloseHandle", args, 0)?)?;
    // SAFETY: raw 已从旁表摘下（只关一次）。
    if unsafe { sys::CloseHandle(raw) } == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn wait_for_single_object(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "WaitForSingleObject", kwargs)?;
    let (_, raw, _) = entry_of(instance, int_arg(instance, "WaitForSingleObject", args, 0)?)?;
    let timeout = u32::try_from(int_arg(instance, "WaitForSingleObject", args, 1)?).map_err(|_| {
        instance.raise_builtin_error("OverflowError", "WaitForSingleObject 的超时超出 32 位")
    })?;
    // SAFETY: raw 来自旁表。
    let status = unsafe { sys::WaitForSingleObject(raw, timeout) };
    Ok(instance.new_int(i64::from(status)))
}

#[allow(unsafe_code)]
fn create_io_completion_port(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "CreateIoCompletionPort", kwargs)?;
    // 参数：file_handle（-1＝INVALID_HANDLE_VALUE）／existing iocp（0＝新建）／key／threads
    let file = int_arg(instance, "CreateIoCompletionPort", args, 0)?;
    let existing = optional_int(instance, "CreateIoCompletionPort", args, 1, 0)?;
    let key = optional_int(instance, "CreateIoCompletionPort", args, 2, 0)?;
    let threads = optional_int(instance, "CreateIoCompletionPort", args, 3, 0)?;
    let file_raw: HandleRaw = file as i32 as HandleRaw;
    let existing_raw: HandleRaw = if existing == 0 {
        0
    } else {
        entry_of(instance, existing)?.1
    };
    // SAFETY: 句柄来自调用方约定的 Win32 句柄值（INVALID_HANDLE_VALUE 或本模块旁表）。
    let handle = unsafe {
        sys::CreateIoCompletionPort(file_raw, existing_raw, key as usize, threads as u32)
    };
    if handle == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(store(handle, HandleKind::Iocp)))
}

#[allow(unsafe_code)]
fn post_queued_completion_status(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "PostQueuedCompletionStatus", kwargs)?;
    let (_, raw, kind) = entry_of(instance, int_arg(instance, "PostQueuedCompletionStatus", args, 0)?)?;
    if kind != HandleKind::Iocp {
        return Err(instance.raise_builtin_error("OSError", "[WinError 6] 句柄不是完成端口"));
    }
    let transferred = u32::try_from(int_arg(instance, "PostQueuedCompletionStatus", args, 1)?).map_err(|_| {
        instance.raise_builtin_error("OverflowError", "字节数超出 32 位")
    })?;
    let key = int_arg(instance, "PostQueuedCompletionStatus", args, 2)?;
    let overlapped = arg(instance, "PostQueuedCompletionStatus", args, 3)?;
    if !(overlapped == instance.singletons().none() || instance.int_value(overlapped) == Some(0)) {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "PostQueuedCompletionStatus 的 overlapped 参数只支持 None／0（§12.24 首批）",
        ));
    }
    // SAFETY: 同上；overlapped 传 NULL。
    if unsafe { sys::PostQueuedCompletionStatus(raw, transferred, key as usize, core::ptr::null_mut()) } == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn get_queued_completion_status(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "GetQueuedCompletionStatus", kwargs)?;
    let (_, raw, kind) = entry_of(instance, int_arg(instance, "GetQueuedCompletionStatus", args, 0)?)?;
    if kind != HandleKind::Iocp {
        return Err(instance.raise_builtin_error("OSError", "[WinError 6] 句柄不是完成端口"));
    }
    let timeout = u32::try_from(optional_int(instance, "GetQueuedCompletionStatus", args, 1, u32::MAX as i64)?)
        .map_err(|_| instance.raise_builtin_error("OverflowError", "超时超出 32 位"))?;
    let mut transferred: u32 = 0;
    let mut key: usize = 0;
    let mut overlapped: *mut OverlappedInner = core::ptr::null_mut();
    // SAFETY: 出参都是活局部量。
    let status = unsafe {
        sys::GetQueuedCompletionStatus(raw, &mut transferred, &mut key, &mut overlapped, timeout)
    };
    if status == 0 {
        return Err(raise_win(instance, last_error()));
    }
    if !overlapped.is_null() {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "GetQueuedCompletionStatus 的 overlapped 出参只支持 NULL（§12.24 首批）",
        ));
    }
    let overlapped_object = instance.new_none();
    Ok(instance.new_tuple(vec![
        instance.new_int(i64::from(transferred)),
        instance.new_int(key as i64),
        overlapped_object,
    ]))
}

#[allow(unsafe_code)]
fn get_last_error(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "GetLastError", kwargs)?;
    Ok(instance.new_int(i64::from(last_error())))
}

#[allow(unsafe_code)]
fn current_process_id(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "GetCurrentProcessId", kwargs)?;
    // SAFETY: 无参。
    Ok(instance.new_int(i64::from(unsafe { sys::GetCurrentProcessId() })))
}

#[allow(unsafe_code)]
fn current_thread_id(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "GetCurrentThreadId", kwargs)?;
    // SAFETY: 无参。
    Ok(instance.new_int(i64::from(unsafe { sys::GetCurrentThreadId() })))
}

// --------------------------------------------------------------------------- //
// `_overlapped.Overlapped` 类
// --------------------------------------------------------------------------- //

fn overlapped_ptr(instance: &Instance, this: NonNull<Header>) -> Result<*mut OverlappedMemory, ExecError> {
    let value = instance
        .attribute_optional_of(this, "_ptr")
        .ok()
        .flatten()
        .and_then(|object| instance.int_value(object))
        .ok_or_else(|| instance.raise_builtin_error("OSError", "Overlapped 对象已关闭"))?;
    let address = usize::try_from(value)
        .map_err(|_| instance.raise_builtin_error("OSError", "Overlapped 对象已关闭"))?;
    // `Close()` 之后 `_ptr` 置 0 ⇒ **0 必须当"已关闭"**（绝不能把 null 交给 `Box::from_raw`：
    // `NonNull::new_unchecked` 的 UB 检查会直接 abort——Windows CI 实测抓到）。
    if address == 0 {
        return Err(instance.raise_builtin_error("OSError", "Overlapped 对象已关闭"));
    }
    Ok(address as *mut OverlappedMemory)
}

fn set_int_attr(instance: &Instance, this: NonNull<Header>, name: &str, value: i64) {
    let object = instance.new_int(value);
    let _ = instance.set_attribute_value(this, name, object);
    instance.release(object);
}

/// 取 `self`（两种调用形态）：**方法调用**时在 `bound`；**类实例化**的 `__init__` 时是
/// 第一个位置实参（与 stdlib `_socket` 一族的约定一致）。
fn split_self<'a>(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> Result<(NonNull<Header>, &'a [NonNull<Header>]), ExecError> {
    match bound {
        Some(this) => Ok((this, args)),
        None => {
            let this = args.first().copied().ok_or_else(|| {
                instance.raise_builtin_error("TypeError", "缺少 self 实参")
            })?;
            Ok((this, &args[1..]))
        }
    }
}

#[allow(unsafe_code)]
fn overlapped_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.__init__", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    // 已有事件句柄则由调用方给（CPython 口径：`Overlapped(event)`；0 ⇒ 自己建一个）。
    let event = optional_int(instance, "Overlapped.__init__", rest, 0, 0)?;
    let (handle, owned) = if event == 0 {
        // SAFETY: 参数都是常量；返回句柄由本对象接管。
        let raw = unsafe { sys::CreateEventW(core::ptr::null_mut(), 1, 0, core::ptr::null()) };
        if raw == 0 {
            return Err(raise_win(instance, last_error()));
        }
        (raw, 1)
    } else {
        (event as HandleRaw, 0)
    };
    let memory = Box::new(OverlappedMemory {
        raw: OverlappedInner { event: handle, ..OverlappedInner::default() },
        buffer: Vec::new(),
        handle: 0,
        kind: 0,
    });
    let address = Box::into_raw(memory) as usize;
    set_int_attr(instance, this, "_ptr", address as i64);
    // CPython 的 `Overlapped.address` 是只读 int（这里作为普通实例属性如实暴露同一地址）。
    set_int_attr(instance, this, "address", address as i64);
    set_int_attr(instance, this, "event", handle as i64);
    set_int_attr(instance, this, "_owned_event", owned);
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn overlapped_close(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.Close", kwargs)?;
    let (this, _rest) = split_self(instance, bound, args)?;
    let pointer = overlapped_ptr(instance, this)?;
    // SAFETY: pointer 来自 `Box::into_raw`，且 `_ptr` 清零保证只释放一次。
    let memory = unsafe { Box::from_raw(pointer) };
    if memory.raw.event != 0 {
        // 自己建的事件由自己关；外部给的事件不关（CPython 口径：调用方负责）。
        let owned = instance
            .attribute_optional_of(this, "_owned_event")
            .ok()
            .flatten()
            .and_then(|object| instance.int_value(object))
            .unwrap_or(0);
        if owned != 0 {
            // SAFETY: 句柄由本对象创建且尚未关闭。
            unsafe { sys::CloseHandle(memory.raw.event) };
        }
    }
    set_int_attr(instance, this, "_ptr", 0);
    Ok(instance.new_none())
}


#[allow(unsafe_code)]
fn overlapped_read_file(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.ReadFile", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    let handle = int_arg(instance, "Overlapped.ReadFile", rest, 0)? as HandleRaw;
    let size = int_arg(instance, "Overlapped.ReadFile", rest, 1)?;
    if size < 0 || size > u32::MAX as i64 {
        return Err(instance.raise_builtin_error("OverflowError", "ReadFile 的 size 越界"));
    }
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: memory 来自 Box；本对象在 Close 前一直持有。
    let memory = unsafe { &mut *memory };
    let mut buffer = vec![0u8; size as usize];
    let mut transferred = 0u32;
    let ok = unsafe {
        sys::ReadFile(
            handle,
            buffer.as_mut_ptr().cast::<c_void>(),
            size as u32,
            &mut transferred,
            &mut memory.raw,
        )
    };
    if ok == 0 {
        let code = last_error();
        // ERROR_IO_PENDING = 997：已投递，等完成。
        if code != 997 {
            return Err(raise_win(instance, code));
        }
    }
    memory.buffer = buffer;
    memory.handle = handle;
    memory.kind = 1;
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn overlapped_write_file(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.WriteFile", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    let handle = int_arg(instance, "Overlapped.WriteFile", rest, 0)? as HandleRaw;
    let data = arg(instance, "Overlapped.WriteFile", rest, 1)?;
    let bytes = instance.bytes_like_value(data).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "WriteFile 需要 bytes-like 对象")
    })?;
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: 同上。
    let memory = unsafe { &mut *memory };
    let mut transferred = 0u32;
    let ok = unsafe {
        sys::WriteFile(
            handle,
            bytes.as_ptr().cast::<c_void>(),
            bytes.len() as u32,
            &mut transferred,
            &mut memory.raw,
        )
    };
    if ok == 0 {
        let code = last_error();
        if code != 997 {
            return Err(raise_win(instance, code));
        }
    }
    memory.buffer = bytes;
    memory.handle = handle;
    memory.kind = 2;
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn overlapped_wsa_recv(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.WSARecv", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    let handle = int_arg(instance, "Overlapped.WSARecv", rest, 0)?;
    let size = int_arg(instance, "Overlapped.WSARecv", rest, 1)?;
    let flags = optional_int(instance, "Overlapped.WSARecv", rest, 2, 0)? as u32;
    if size < 0 || size > u32::MAX as i64 {
        return Err(instance.raise_builtin_error("OverflowError", "WSARecv 的 size 越界"));
    }
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: memory 来自 Box，存活到 Close。
    let memory = unsafe { &mut *memory };
    let mut buffer = vec![0u8; size as usize];
    let mut wsa = WsaBuf { len: size as u32, buf: buffer.as_mut_ptr() };
    let mut transferred = 0u32;
    let mut recv_flags = flags;
    let ok = unsafe {
        winsock::WSARecv(
            resolve_socket(handle),
            &mut wsa,
            1,
            &mut transferred,
            &mut recv_flags,
            &mut memory.raw,
            core::ptr::null_mut(),
        )
    };
    if ok != 0 {
        let code = unsafe { winsock::WSAGetLastError() };
        if code != 997 && code != 10035 {
            return Err(raise_win(instance, code as u32));
        }
    }
    memory.buffer = buffer;
    memory.handle = resolve_socket(handle);
    memory.kind = 3;
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn overlapped_wsa_send(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.WSASend", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    let handle = int_arg(instance, "Overlapped.WSASend", rest, 0)?;
    let data = arg(instance, "Overlapped.WSASend", rest, 1)?;
    let flags = optional_int(instance, "Overlapped.WSASend", rest, 2, 0)? as u32;
    let bytes = instance.bytes_like_value(data).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "WSASend 需要 bytes-like 对象")
    })?;
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: 同上。
    let memory = unsafe { &mut *memory };
    let wsa = WsaBuf { len: bytes.len() as u32, buf: bytes.as_ptr() as *mut u8 };
    let mut transferred = 0u32;
    let ok = unsafe {
        winsock::WSASend(
            resolve_socket(handle),
            &wsa,
            1,
            &mut transferred,
            flags,
            &mut memory.raw,
            core::ptr::null_mut(),
        )
    };
    if ok != 0 {
        let code = unsafe { winsock::WSAGetLastError() };
        if code != 997 && code != 10035 {
            return Err(raise_win(instance, code as u32));
        }
    }
    memory.buffer = bytes;
    memory.handle = resolve_socket(handle);
    memory.kind = 4;
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn overlapped_get_result_common(
    instance: &Instance,
    this: NonNull<Header>,
    wait: i32,
    bytes_for_read: bool,
) -> Result<NonNull<Header>, ExecError> {
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: 同上。
    let memory = unsafe { &mut *memory };
    let mut transferred = 0u32;
    let ok = unsafe {
        sys::GetOverlappedResult(memory.handle, &mut memory.raw, &mut transferred, wait)
    };
    if ok == 0 {
        return Err(raise_win(instance, last_error()));
    }
    if bytes_for_read && (memory.kind == 1 || memory.kind == 3) {
        let end = (transferred as usize).min(memory.buffer.len());
        Ok(instance.new_bytes(&memory.buffer[..end]))
    } else {
        Ok(instance.new_int(i64::from(transferred)))
    }
}

#[allow(unsafe_code)]
fn overlapped_connect_named_pipe(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.ConnectNamedPipe", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    let pipe = int_arg(instance, "Overlapped.ConnectNamedPipe", rest, 0)? as HandleRaw;
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: memory 来自 Box，存活到 Close。
    let memory = unsafe { &mut *memory };
    let ok = unsafe { sys::ConnectNamedPipe(pipe, &mut memory.raw) };
    if ok == 0 {
        let code = last_error();
        if code == 535 {
            // ERROR_PIPE_CONNECTED：客户端已在等待，连接立即完成，不需要等完成包。
            memory.handle = pipe;
            memory.kind = 0;
            return Ok(instance.new_int(1));
        }
        if code != 997 {
            return Err(raise_win(instance, code));
        }
    }
    memory.handle = pipe;
    memory.kind = 0;
    Ok(instance.new_int(0))
}

#[allow(unsafe_code)]
fn overlapped_getresult(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.getresult", kwargs)?;
    let (this, _rest) = split_self(instance, bound, args)?;
    overlapped_get_result_common(instance, this, 1, true)
}

#[allow(unsafe_code)]
fn overlapped_get_overlapped_result(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.GetOverlappedResult", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    let wait = optional_int(instance, "Overlapped.GetOverlappedResult", rest, 0, 1)? as i32;
    overlapped_get_result_common(instance, this, wait, false)
}

/// 数据面未接线的方法：**如实** `NotImplementedError`（不假装成功）。
#[allow(unsafe_code)]
fn overlapped_unwired(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "重叠 I/O 数据面尚未接线（§12.24 剩余：ReadFile／WriteFile／WSARecv／WSASend）",
    ))
}

fn make_native(instance: &Instance, name: &str, function: pyawa_core::NativeFn) -> NonNull<Header> {
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

fn module_namespace(instance: &Instance, name: &str) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, name)?;
    pyawa_core::mounted_instance_dict(instance, module)
}

/// **安装真实面**（只由便利装配路径调用；裸 ABI 宿主仍拿到 stdlib 的诚实桩）。
pub fn install(instance: &Instance) {
    if let Some(namespace) = module_namespace(instance, "_winapi") {
        for (name, handler) in [
            ("CreateFile", create_file as pyawa_core::NativeFn),
            ("CreateNamedPipe", create_named_pipe as pyawa_core::NativeFn),
            ("CreateEvent", create_event as pyawa_core::NativeFn),
            ("SetEvent", set_event as pyawa_core::NativeFn),
            ("ResetEvent", reset_event as pyawa_core::NativeFn),
            ("CloseHandle", close_handle as pyawa_core::NativeFn),
            ("WaitForSingleObject", wait_for_single_object as pyawa_core::NativeFn),
            ("CreateIoCompletionPort", create_io_completion_port as pyawa_core::NativeFn),
            ("PostQueuedCompletionStatus", post_queued_completion_status as pyawa_core::NativeFn),
            ("GetQueuedCompletionStatus", get_queued_completion_status as pyawa_core::NativeFn),
            ("GetLastError", get_last_error as pyawa_core::NativeFn),
            ("GetCurrentProcessId", current_process_id as pyawa_core::NativeFn),
            ("GetCurrentThreadId", current_thread_id as pyawa_core::NativeFn),
        ] {
            let function = make_native(instance, name, handler);
            instance.dict_set(namespace, name, function);
        }
    }
    if let Some(namespace) = module_namespace(instance, "_overlapped") {
        if let Some(class) = build_overlapped_class(instance) {
            instance.dict_set(namespace, "Overlapped", class);
        }
    }
}

fn build_overlapped_class(instance: &Instance) -> Option<NonNull<Header>> {
    let base = instance
        .type_named("object")
        .expect("object 在引导期已登记")
        .cast::<Header>();
    let class_namespace = instance.new_dict();
    for (name, handler) in [
        ("__init__", overlapped_init as pyawa_core::NativeFn),
        ("Close", overlapped_close as pyawa_core::NativeFn),
        // `GetOverlappedResult` 也属于"数据面"（没有真实投递过的 OVERLAPPED ⇒ 如实未接线，
        // 不用事件句柄硬调出假语义）。
        ("GetOverlappedResult", overlapped_get_overlapped_result as pyawa_core::NativeFn),
        ("getresult", overlapped_getresult as pyawa_core::NativeFn),
        ("ReadFile", overlapped_read_file as pyawa_core::NativeFn),
        ("WriteFile", overlapped_write_file as pyawa_core::NativeFn),
        ("WSARecv", overlapped_wsa_recv as pyawa_core::NativeFn),
        ("WSASend", overlapped_wsa_send as pyawa_core::NativeFn),
        ("ConnectNamedPipe", overlapped_connect_named_pipe as pyawa_core::NativeFn),
        ("DisconnectNamedPipe", overlapped_unwired as pyawa_core::NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    pyawa_core::build_class_from_parts(
        instance,
        "Overlapped".to_owned(),
        vec![base],
        class_namespace,
        None,
    )
    .ok()
}
