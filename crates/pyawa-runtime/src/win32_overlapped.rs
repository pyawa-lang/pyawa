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
//! **P1 文件＋命名 pipe**已接；**socket** 的 `WSARecv`／`WSASend`／`AcceptEx` 与
//! `ReadFileInto` 仍如实 `NotImplementedError`（不假装成功）。因此 Windows Proactor
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
    /// 操作种类：0=无、1=读、2=写、3=socket recv、4=socket send、5=recv_into。
    kind: u8,
    /// `WSARecvInto` 的目标 bytearray（持有引用；`getresult` 写入后释放）。
    target: Option<NonNull<Header>>,
    /// 操作已同步完成时记录字节数（避免等一个不会到来的 IOCP 完成包）。
    completed: Option<u32>,
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
/// **CRT fd 的旁表**：`msvcrt.open_osfhandle` 返回高位合成 fd，避免 `os.close(fd)` 把
/// 能力表里同号段的 socket/pipe 句柄误关（Proactor 自管道曾被误关）。
static NEXT_OSF_FD: AtomicU64 = AtomicU64::new(0x4000_0000);
static OSF_HANDLES: Mutex<Option<HashMap<i32, HandleRaw>>> = Mutex::new(None);

fn osf_table() -> std::sync::MutexGuard<'static, Option<HashMap<i32, HandleRaw>>> {
    OSF_HANDLES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

fn raw_handle(instance: &Instance, value: i64) -> Result<HandleRaw, ExecError> {
    if let Ok((_, raw, _)) = entry_of(instance, value) {
        return Ok(raw);
    }
    if value > 0 {
        return Ok(value as HandleRaw);
    }
    Err(instance.raise_builtin_error("OSError", "[WinError 6] 无效的 Win32 句柄"))
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
    use super::{c_void, HandleRaw, OverlappedInner, ProcessInformation, StartupInfoW};

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
        pub fn CancelIoEx(file: HandleRaw, overlapped: *mut OverlappedInner) -> i32;
        pub fn CreatePipe(
            read_pipe: *mut HandleRaw,
            write_pipe: *mut HandleRaw,
            security: *mut c_void,
            size: u32,
        ) -> i32;
        pub fn GetStdHandle(which: u32) -> HandleRaw;
        pub fn GetCurrentProcess() -> HandleRaw;
        pub fn DuplicateHandle(
            source_process: HandleRaw,
            source_handle: HandleRaw,
            target_process: HandleRaw,
            target_handle: *mut HandleRaw,
            desired_access: u32,
            inherit: i32,
            options: u32,
        ) -> i32;
        pub fn GetFileType(handle: HandleRaw) -> u32;
        pub fn GetExitCodeProcess(handle: HandleRaw, code: *mut u32) -> i32;
        pub fn TerminateProcess(handle: HandleRaw, code: u32) -> i32;
        pub fn InitializeProcThreadAttributeList(
            list: *mut c_void,
            count: u32,
            flags: u32,
            size: *mut usize,
        ) -> i32;
        pub fn UpdateProcThreadAttribute(
            list: *mut c_void,
            flags: u32,
            attribute: usize,
            value: *mut c_void,
            size: usize,
            previous: *mut c_void,
            return_size: *mut usize,
        ) -> i32;
        pub fn RegisterWaitForSingleObject(
            wait_handle: *mut HandleRaw,
            handle: HandleRaw,
            callback: Option<unsafe extern "system" fn(*mut c_void, u8)>,
            context: *mut c_void,
            timeout: u32,
            flags: u32,
        ) -> i32;
        pub fn UnregisterWaitEx(wait_handle: HandleRaw, completion_event: HandleRaw) -> i32;
        pub fn CreateProcessW(
            application_name: *const u16,
            command_line: *mut u16,
            process_attributes: *mut c_void,
            thread_attributes: *mut c_void,
            inherit_handles: i32,
            creation_flags: u32,
            environment: *mut c_void,
            current_directory: *const u16,
            startup_info: *const StartupInfoW,
            process_information: *mut ProcessInformation,
        ) -> i32;
    }
}

#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}

#[repr(C)]
#[derive(Default)]
struct StartupInfoW {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_count_chars: u32,
    y_count_chars: u32,
    fill_attribute: u32,
    flags: u32,
    show_window: u16,
    reserved2_size: u16,
    reserved2: *mut u8,
    std_input: HandleRaw,
    std_output: HandleRaw,
    std_error: HandleRaw,
}

#[repr(C)]
#[derive(Default)]
struct StartupInfoExW {
    info: StartupInfoW,
    attribute_list: *mut c_void,
}

#[repr(C)]
#[derive(Default)]
struct ProcessInformation {
    process: HandleRaw,
    thread: HandleRaw,
    process_id: u32,
    thread_id: u32,
}

#[repr(C)]
struct WaitContext {
    iocp: HandleRaw,
    overlapped: *mut OverlappedInner,
}

static WAIT_CONTEXTS: Mutex<Option<HashMap<HandleRaw, usize>>> = Mutex::new(None);

#[allow(unsafe_code)]
unsafe extern "system" fn wait_callback(context: *mut c_void, _timed_out: u8) {
    // SAFETY: context 是 register_wait_with_queue 放进去的 WaitContext，注销前一直存活。
    let context = unsafe { &*(context as *const WaitContext) };
    // SAFETY: iocp/overlapped 均来自调用方活对象。
    unsafe {
        sys::PostQueuedCompletionStatus(context.iocp, 0, 0, context.overlapped);
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
    // `subprocess` 的 stdio 句柄要能被 child 继承（windows_utils.pipe 的客户端端）。
    let mut security = SecurityAttributes {
        length: core::mem::size_of::<SecurityAttributes>() as u32,
        descriptor: core::ptr::null_mut(),
        inherit: 1,
    };
    // SAFETY: wide 是 NUL 结尾 UTF-16；security 是活局部量且 bInheritHandle=TRUE。
    let handle = unsafe {
        sys::CreateFileW(
            wide.as_ptr(),
            access,
            share,
            (&mut security as *mut SecurityAttributes).cast::<c_void>(),
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
    // subprocess 的 pipe 句柄要能被 child 继承。
    let mut security = SecurityAttributes {
        length: core::mem::size_of::<SecurityAttributes>() as u32,
        descriptor: core::ptr::null_mut(),
        inherit: 1,
    };
    // SAFETY: wide 是 NUL 结尾 UTF-16；security 是活局部量且 bInheritHandle=TRUE。
    let handle = unsafe {
        sys::CreateNamedPipeW(
            wide.as_ptr(),
            open_mode,
            pipe_mode,
            max_instances,
            out_size,
            in_size,
            timeout,
            (&mut security as *mut SecurityAttributes).cast::<c_void>(),
        )
    };
    if handle == -1isize {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(handle as i64))
}

#[allow(unsafe_code)]
fn create_pipe(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "CreatePipe", kwargs)?;
    let size = optional_int(instance, "CreatePipe", args, 1, 0)?;
    if size < 0 || size > u32::MAX as i64 {
        return Err(instance.raise_builtin_error("OverflowError", "CreatePipe 的 size 越界"));
    }
    let mut read: HandleRaw = 0;
    let mut write: HandleRaw = 0;
    // 子进程要继承管道句柄（subprocess 的 stdio 链）。
    let mut security = SecurityAttributes {
        length: core::mem::size_of::<SecurityAttributes>() as u32,
        descriptor: core::ptr::null_mut(),
        inherit: 1,
    };
    // SAFETY: 出参都是活局部量；security 是活局部量且 bInheritHandle=TRUE。
    let ok = unsafe {
        sys::CreatePipe(
            &mut read,
            &mut write,
            (&mut security as *mut SecurityAttributes).cast::<c_void>(),
            size as u32,
        )
    };
    if ok == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_tuple(vec![
        instance.new_int(read as i64),
        instance.new_int(write as i64),
    ]))
}

#[allow(unsafe_code)]
fn sync_read_file(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "_winapi.ReadFile", kwargs)?;
    let handle = int_arg(instance, "_winapi.ReadFile", args, 0)? as HandleRaw;
    let size = int_arg(instance, "_winapi.ReadFile", args, 1)?;
    if size < 0 || size > u32::MAX as i64 {
        return Err(instance.raise_builtin_error("OverflowError", "ReadFile 的 size 越界"));
    }
    let mut buffer = vec![0u8; size as usize];
    let mut transferred = 0u32;
    // SAFETY: 同步读；buffer 长度与 size 一致；overlapped=NULL。
    let ok = unsafe {
        sys::ReadFile(
            handle,
            buffer.as_mut_ptr().cast::<c_void>(),
            size as u32,
            &mut transferred,
            core::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(raise_win(instance, last_error()));
    }
    buffer.truncate(transferred as usize);
    Ok(instance.new_bytes(&buffer))
}

#[allow(unsafe_code)]
fn sync_write_file(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "_winapi.WriteFile", kwargs)?;
    let handle = int_arg(instance, "_winapi.WriteFile", args, 0)? as HandleRaw;
    let data = arg(instance, "_winapi.WriteFile", args, 1)?;
    let bytes = instance
        .bytes_like_value(data)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "WriteFile 需要 bytes-like 对象"))?;
    let mut transferred = 0u32;
    // SAFETY: 同步写；bytes 在调用期间存活；overlapped=NULL。
    let ok = unsafe {
        sys::WriteFile(
            handle,
            bytes.as_ptr().cast::<c_void>(),
            bytes.len() as u32,
            &mut transferred,
            core::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(i64::from(transferred)))
}

#[allow(unsafe_code)]
fn get_std_handle(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "GetStdHandle", kwargs)?;
    let which = int_arg(instance, "GetStdHandle", args, 0)? as u32;
    // SAFETY: 无参句柄查询。
    let handle = unsafe { sys::GetStdHandle(which) };
    if handle == 0 || handle == -1isize {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(handle as i64))
}

#[allow(unsafe_code)]
fn get_current_process(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "GetCurrentProcess", kwargs)?;
    // SAFETY: 无参。
    Ok(instance.new_int(unsafe { sys::GetCurrentProcess() } as i64))
}

#[allow(unsafe_code)]
fn duplicate_handle(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "DuplicateHandle", kwargs)?;
    // CPython 形状：DuplicateHandle(process, handle, process, desired_access, inherit, options)
    // —— **目标句柄是返回值**，不是入参。
    let source_process = int_arg(instance, "DuplicateHandle", args, 0)? as HandleRaw;
    let source_handle = int_arg(instance, "DuplicateHandle", args, 1)? as HandleRaw;
    let target_process = int_arg(instance, "DuplicateHandle", args, 2)? as HandleRaw;
    let desired_access = optional_int(instance, "DuplicateHandle", args, 3, 0)? as u32;
    let inherit = optional_int(instance, "DuplicateHandle", args, 4, 0)? as i32;
    let options = optional_int(instance, "DuplicateHandle", args, 5, 0)? as u32;
    let mut target: HandleRaw = 0;
    // SAFETY: target 是活局部量。
    let ok = unsafe {
        sys::DuplicateHandle(
            source_process,
            source_handle,
            target_process,
            &mut target,
            desired_access,
            inherit,
            options,
        )
    };
    if ok == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(target as i64))
}

#[allow(unsafe_code)]
fn get_file_type(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "GetFileType", kwargs)?;
    let handle = int_arg(instance, "GetFileType", args, 0)? as HandleRaw;
    // SAFETY: 句柄由宿主给。
    let kind = unsafe { sys::GetFileType(handle) };
    if kind == 0 {
        let code = last_error();
        if code != 0 {
            return Err(raise_win(instance, code));
        }
    }
    Ok(instance.new_int(i64::from(kind)))
}

#[allow(unsafe_code)]
fn get_exit_code_process(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "GetExitCodeProcess", kwargs)?;
    let handle = raw_handle(instance, int_arg(instance, "GetExitCodeProcess", args, 0)?)?;
    let mut code = 0u32;
    // SAFETY: code 是活局部量。
    if unsafe { sys::GetExitCodeProcess(handle, &mut code) } == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(i64::from(code)))
}

#[allow(unsafe_code)]
fn terminate_process(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "TerminateProcess", kwargs)?;
    let handle = raw_handle(instance, int_arg(instance, "TerminateProcess", args, 0)?)?;
    let code = optional_int(instance, "TerminateProcess", args, 1, 1)? as u32;
    // SAFETY: 句柄由宿主给。
    if unsafe { sys::TerminateProcess(handle, code) } == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn create_process(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "CreateProcess", kwargs)?;
    let application = optional_text(instance, "CreateProcess", args, 0)?;
    let command_line = optional_text(instance, "CreateProcess", args, 1)?
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "CreateProcess 需要命令行"))?;
    let mut inherit = optional_int(instance, "CreateProcess", args, 4, 0)? as i32;
    let mut flags = optional_int(instance, "CreateProcess", args, 5, 0)? as u32;
    // 环境块：dict ⇒ 排序后的 `KEY=VALUE\0…\0` UTF-16；None ⇒ NULL（继承）。
    let env = args.get(6).copied();
    let env_block: Option<Vec<u16>> = match env {
        Some(object) if object != instance.singletons().none() => {
            let entries = instance.dict_entries(object).ok_or_else(|| {
                instance.raise_builtin_error("TypeError", "CreateProcess 的 env 必须是 dict")
            })?;
            let mut pairs: Vec<(String, String)> = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                let key = instance.text_of(key).ok_or_else(|| {
                    instance.raise_builtin_error("TypeError", "环境变量名必须是 str")
                })?;
                let value = instance.text_of(value).ok_or_else(|| {
                    instance.raise_builtin_error("TypeError", "环境变量值必须是 str")
                })?;
                pairs.push((key.to_owned(), value.to_owned()));
            }
            pairs.sort_by(|a, b| a.0.to_uppercase().cmp(&b.0.to_uppercase()));
            let mut block = Vec::new();
            for (key, value) in pairs {
                block.extend(key.encode_utf16());
                block.push(b'=' as u16);
                block.extend(value.encode_utf16());
                block.push(0);
            }
            block.push(0);
            Some(block)
        }
        _ => None,
    };
    if env_block.is_some() {
        // CREATE_UNICODE_ENVIRONMENT：UTF-16 环境块必须带这个标志。
        flags |= 0x0000_0400;
    }
    let cwd = optional_text(instance, "CreateProcess", args, 7)?;
    let startup = args.get(8).copied();

    let mut startup_info = StartupInfoW {
        cb: core::mem::size_of::<StartupInfoW>() as u32,
        ..StartupInfoW::default()
    };
    let mut handle_list: Vec<HandleRaw> = Vec::new();
    if let Some(startup) = startup {
        let read = |name: &str| -> i64 {
            instance
                .attribute_optional_of(startup, name)
                .ok()
                .flatten()
                .and_then(|object| instance.int_value(object))
                .unwrap_or(0)
        };
        startup_info.flags = read("dwFlags") as u32;
        startup_info.show_window = read("wShowWindow") as u16;
        startup_info.std_input = read("hStdInput") as HandleRaw;
        startup_info.std_output = read("hStdOutput") as HandleRaw;
        startup_info.std_error = read("hStdError") as HandleRaw;
        if let Ok(Some(attributes)) = instance.attribute_optional_of(startup, "lpAttributeList") {
            if let Some(entries) = instance.dict_entries(attributes) {
                for (key, value) in entries {
                    if instance.text_of(key) == Some("handle_list") {
                        if let Some(items) = instance.list_items(value) {
                            for item in items {
                                if let Some(raw) = instance.int_value(item) {
                                    if raw > 0 {
                                        handle_list.push(raw as HandleRaw);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let mut attribute_storage: Vec<u8> = Vec::new();
    let mut startup_ex = StartupInfoExW::default();
    if !handle_list.is_empty() {
        // Windows 的 PROC_THREAD_ATTRIBUTE_HANDLE_LIST 仍要求 bInheritHandles=TRUE。
        inherit = 1;
    }
    let startup_pointer: *const StartupInfoW;
    if handle_list.is_empty() {
        startup_pointer = &startup_info;
    } else {
        let mut size = 0usize;
        // SAFETY: 第一次调用只取所需字节数。
        unsafe { sys::InitializeProcThreadAttributeList(core::ptr::null_mut(), 1, 0, &mut size) };
        attribute_storage.resize(size, 0);
        // SAFETY: storage 长度 >= size；list 指针有效。
        let ok = unsafe {
            sys::InitializeProcThreadAttributeList(
                attribute_storage.as_mut_ptr().cast::<c_void>(),
                1,
                0,
                &mut size,
            )
        };
        if ok == 0 {
            return Err(raise_win(instance, last_error()));
        }
        // SAFETY: 句柄数组在调用期间存活；attribute 0x00020002 = HANDLE_LIST。
        let update = unsafe {
            sys::UpdateProcThreadAttribute(
                attribute_storage.as_mut_ptr().cast::<c_void>(),
                0,
                0x0002_0002,
                handle_list.as_mut_ptr().cast::<c_void>(),
                handle_list.len() * core::mem::size_of::<HandleRaw>(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };
        if update == 0 {
            return Err(raise_win(instance, last_error()));
        }
        startup_ex.info = startup_info;
        startup_ex.info.cb = core::mem::size_of::<StartupInfoExW>() as u32;
        startup_ex.attribute_list = attribute_storage.as_mut_ptr().cast::<c_void>();
        flags |= 0x0008_0000; // EXTENDED_STARTUPINFO_PRESENT
        startup_pointer = &startup_ex.info;
    }

    let application_wide = application.as_deref().and_then(|text| wide_opt(Some(text)));
    let mut command_wide = wide_opt(Some(&command_line)).expect("命令行非 None");
    let cwd_wide = cwd.as_deref().and_then(|text| wide_opt(Some(text)));
    let mut process_info = ProcessInformation::default();
    // SAFETY: 宽字符串／STARTUPINFO／PROCESS_INFORMATION 都是活局部量。
    let ok = unsafe {
        sys::CreateProcessW(
            application_wide
                .as_ref()
                .map_or(core::ptr::null(), |text| text.as_ptr()),
            command_wide.as_mut_ptr(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            inherit,
            flags,
            env_block
                .as_ref()
                .map_or(core::ptr::null_mut(), |block| block.as_ptr().cast_mut().cast::<c_void>()),
            cwd_wide
                .as_ref()
                .map_or(core::ptr::null(), |text| text.as_ptr()),
            startup_pointer,
            &mut process_info,
        )
    };
    if ok == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_tuple(vec![
        instance.new_int(process_info.process as i64),
        instance.new_int(process_info.thread as i64),
        instance.new_int(i64::from(process_info.process_id)),
        instance.new_int(i64::from(process_info.thread_id)),
    ]))
}

#[allow(unsafe_code)]
#[allow(unsafe_code)]
fn msvcrt_open_osfhandle(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "msvcrt.open_osfhandle", kwargs)?;
    let handle = int_arg(instance, "msvcrt.open_osfhandle", args, 0)? as HandleRaw;
    if handle == 0 || handle == -1isize {
        return Err(instance.raise_builtin_error("OSError", "open_osfhandle 收到无效句柄"));
    }
    let fd = NEXT_OSF_FD.fetch_add(1, Ordering::Relaxed) as i32;
    osf_table()
        .get_or_insert_with(HashMap::new)
        .insert(fd, handle);
    Ok(instance.new_int(i64::from(fd)))
}

#[allow(unsafe_code)]
fn msvcrt_get_osfhandle(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "msvcrt.get_osfhandle", kwargs)?;
    let fd = int_arg(instance, "msvcrt.get_osfhandle", args, 0)? as i32;
    let handle = osf_table().as_ref().and_then(|map| map.get(&fd).copied());
    match handle {
        Some(handle) => Ok(instance.new_int(handle as i64)),
        None => Err(instance.raise_builtin_error("OSError", "get_osfhandle 收到未知 fd")),
    }
}

#[allow(unsafe_code)]
fn msvcrt_close(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "msvcrt._close", kwargs)?;
    let fd = int_arg(instance, "msvcrt._close", args, 0)? as i32;
    let handle = osf_table().as_mut().and_then(|map| map.remove(&fd));
    if let Some(handle) = handle {
        // SAFETY: 句柄由 open_osfhandle 收下；关闭一次。
        unsafe { sys::CloseHandle(handle) };
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn register_wait_with_queue(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "_overlapped.RegisterWaitWithQueue", kwargs)?;
    let handle = int_arg(instance, "_overlapped.RegisterWaitWithQueue", args, 0)? as HandleRaw;
    let iocp = int_arg(instance, "_overlapped.RegisterWaitWithQueue", args, 1)? as HandleRaw;
    let overlapped = int_arg(instance, "_overlapped.RegisterWaitWithQueue", args, 2)? as usize
        as *mut OverlappedInner;
    let timeout = u32::try_from(optional_int(
        instance,
        "_overlapped.RegisterWaitWithQueue",
        args,
        3,
        u32::MAX as i64,
    )?)
    .map_err(|_| instance.raise_builtin_error("OverflowError", "超时超出 32 位"))?;
    let context = Box::into_raw(Box::new(WaitContext { iocp, overlapped }));
    let mut wait: HandleRaw = 0;
    // SAFETY: context 交给 Win32，注销时回收；callback 只 post 完成包。
    let ok = unsafe {
        sys::RegisterWaitForSingleObject(
            &mut wait,
            handle,
            Some(wait_callback),
            context.cast::<c_void>(),
            timeout,
            0,
        )
    };
    if ok == 0 {
        // SAFETY: 注册失败，context 仍由本函数拥有。
        drop(unsafe { Box::from_raw(context) });
        return Err(raise_win(instance, last_error()));
    }
    WAIT_CONTEXTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(wait, context as usize);
    Ok(instance.new_int(wait as i64))
}

#[allow(unsafe_code)]
fn unregister_wait_ex(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "_overlapped.UnregisterWaitEx", kwargs)?;
    let wait = int_arg(instance, "_overlapped.UnregisterWaitEx", args, 0)? as HandleRaw;
    let event = optional_int(instance, "_overlapped.UnregisterWaitEx", args, 1, -1)? as HandleRaw;
    // SAFETY: 等待句柄来自 RegisterWaitWithQueue。
    let _ = unsafe { sys::UnregisterWaitEx(wait, event) };
    let context = WAIT_CONTEXTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_mut()
        .and_then(|map| map.remove(&wait));
    if let Some(context) = context {
        // SAFETY: 注册时 Box::into_raw 的唯一所有权；注销后回收。
        drop(unsafe { Box::from_raw(context as *mut WaitContext) });
    }
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn overlapped_create_event(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "_overlapped.CreateEvent", kwargs)?;
    let manual_reset = optional_int(instance, "_overlapped.CreateEvent", args, 1, 0)? as i32;
    let initial_state = optional_int(instance, "_overlapped.CreateEvent", args, 2, 0)? as i32;
    // SAFETY: security/name 传 NULL（事件不命名）。
    let handle = unsafe {
        sys::CreateEventW(
            core::ptr::null_mut(),
            manual_reset,
            initial_state,
            core::ptr::null(),
        )
    };
    if handle == 0 {
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
    let raw = raw_handle(instance, int_arg(instance, "WaitForSingleObject", args, 0)?)?;
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

#[allow(unsafe_code)]
fn overlapped_module_create_iocp(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "_overlapped.CreateIoCompletionPort", kwargs)?;
    let file = int_arg(instance, "_overlapped.CreateIoCompletionPort", args, 0)? as HandleRaw;
    let existing = optional_int(instance, "_overlapped.CreateIoCompletionPort", args, 1, 0)? as HandleRaw;
    let key = optional_int(instance, "_overlapped.CreateIoCompletionPort", args, 2, 0)? as usize;
    let threads = optional_int(instance, "_overlapped.CreateIoCompletionPort", args, 3, 0)? as u32;
    // provider 句柄（socket）要换成原生 SOCKET 才能注册到 IOCP。
    // provider 句柄全奇数、原生 HANDLE 4 对齐 ⇒ 不会撞号。
    let file = if file == -1 {
        file
    } else {
        crate::net_windows::raw_socket_for_handle(file as i64)
            .map(|raw| raw as HandleRaw)
            .unwrap_or(file)
    };
    // SAFETY: 句柄按 Win32 契约；existing 为 0 或原生 IOCP。
    let mut handle = unsafe { sys::CreateIoCompletionPort(file, existing, key, threads) };
    if handle == 0 {
        return Err(raise_win(instance, last_error()));
    }
    Ok(instance.new_int(handle as i64))
}

#[allow(unsafe_code)]
fn overlapped_module_get_queued(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "_overlapped.GetQueuedCompletionStatus", kwargs)?;
    let iocp = int_arg(instance, "_overlapped.GetQueuedCompletionStatus", args, 0)? as HandleRaw;
    let timeout = u32::try_from(optional_int(instance, "_overlapped.GetQueuedCompletionStatus", args, 1, u32::MAX as i64)?)
        .map_err(|_| instance.raise_builtin_error("OverflowError", "超时超出 32 位"))?;
    let mut transferred: u32 = 0;
    let mut key: usize = 0;
    let mut overlapped: *mut OverlappedInner = core::ptr::null_mut();
    // SAFETY: 出参都是活局部量。
    let status = unsafe {
        sys::GetQueuedCompletionStatus(iocp, &mut transferred, &mut key, &mut overlapped, timeout)
    };
    if status == 0 {
        let code = last_error();
        if code == 258 {
            return Ok(instance.new_none());
        }
        // IOCP 失败但仍带 overlapped（典型 ERROR_OPERATION_ABORTED=995）：
        // 按 CPython `_overlapped.GetQueuedCompletionStatus` 返回四元组，让回调自己
        // 走 `getresult()` 的异常路径；这里不能直接抛。
        if overlapped.is_null() {
            return Err(raise_win(instance, code));
        }
        return Ok(instance.new_tuple(vec![
            instance.new_int(i64::from(code)),
            instance.new_int(i64::from(transferred)),
            instance.new_int(key as i64),
            instance.new_int(overlapped as usize as i64),
        ]));
    }
    Ok(instance.new_tuple(vec![
        instance.new_int(0),
        instance.new_int(i64::from(transferred)),
        instance.new_int(key as i64),
        instance.new_int(overlapped as usize as i64),
    ]))
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

fn set_bool_attr(instance: &Instance, this: NonNull<Header>, name: &str, value: bool) {
    set_int_attr(instance, this, name, i64::from(value));
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
        target: None,
        completed: None,
    });
    let address = Box::into_raw(memory) as usize;
    set_int_attr(instance, this, "_ptr", address as i64);
    // CPython 的 `Overlapped.address` 是只读 int（这里作为普通实例属性如实暴露同一地址）。
    set_int_attr(instance, this, "address", address as i64);
    set_int_attr(instance, this, "event", handle as i64);
    set_int_attr(instance, this, "_owned_event", owned);
    set_int_attr(instance, this, "pending", 0);
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
    if let Some(target) = memory.target {
        // 引用由本对象持有。
        instance.release(target);
    }
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
    set_bool_attr(instance, this, "pending", true);
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn overlapped_read_file_into(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.ReadFileInto", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    let handle = int_arg(instance, "Overlapped.ReadFileInto", rest, 0)? as HandleRaw;
    let buffer = arg(instance, "Overlapped.ReadFileInto", rest, 1)?;
    let data = instance.bytes_like_value(buffer).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "ReadFileInto 需要 bytes-like 对象")
    })?;
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: memory 来自 Box，存活到 Close。
    let memory = unsafe { &mut *memory };
    let mut read = vec![0u8; data.len()];
    let mut transferred = 0u32;
    let ok = unsafe {
        sys::ReadFile(
            handle,
            read.as_mut_ptr().cast::<c_void>(),
            read.len() as u32,
            &mut transferred,
            &mut memory.raw,
        )
    };
    if ok == 0 {
        let code = last_error();
        if code != 997 {
            return Err(raise_win(instance, code));
        }
    } else {
        // ReadFile 同步完成：Proactor 不会再收到完成包，必须把 pending 置假，
        // 否则 `_register` 会永远等一个不会到来的 IOCP 包。
        memory.completed = Some(transferred);
    }
    // 目标 buffer 由本 Overlapped 持有到 getresult/Close；新增一份引用。
    unsafe { instance.incref_object(buffer.as_ptr()) };
    memory.buffer = read;
    memory.handle = handle;
    memory.kind = 5;
    memory.target = Some(buffer);
    set_bool_attr(instance, this, "pending", memory.completed.is_none());
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
    set_bool_attr(instance, this, "pending", true);
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
    set_bool_attr(instance, this, "pending", true);
    Ok(instance.new_none())
}

#[allow(unsafe_code)]
fn overlapped_wsa_recv_into(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.WSARecvInto", kwargs)?;
    let (this, rest) = split_self(instance, bound, args)?;
    let handle = int_arg(instance, "Overlapped.WSARecvInto", rest, 0)?;
    let target = arg(instance, "Overlapped.WSARecvInto", rest, 1)?;
    let flags = optional_int(instance, "Overlapped.WSARecvInto", rest, 2, 0)? as u32;
    let size = instance.bytes_like_value(target).map(|bytes| bytes.len());
    let size = size.ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "WSARecvInto 需要可写 bytes-like 对象")
    })?;
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: memory 来自 Box，存活到 Close。
    let memory = unsafe { &mut *memory };
    let mut buffer = vec![0u8; size];
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
    // 目标 buffer 由本 Overlapped 持有到 getresult/Close；新增一份引用。
    unsafe { instance.incref_object(target.as_ptr()) };
    memory.buffer = buffer;
    memory.handle = resolve_socket(handle);
    memory.kind = 5;
    set_bool_attr(instance, this, "pending", true);
    memory.target = Some(target);
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
    set_bool_attr(instance, this, "pending", true);
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
    let completed = memory.completed.take();
    if let Some(bytes) = completed {
        transferred = bytes;
    } else {
        let ok = unsafe {
            sys::GetOverlappedResult(memory.handle, &mut memory.raw, &mut transferred, wait)
        };
        if ok == 0 {
            return Err(raise_win(instance, last_error()));
        }
    }
    set_bool_attr(instance, this, "pending", false);
    if bytes_for_read && (memory.kind == 1 || memory.kind == 3) {
        let end = (transferred as usize).min(memory.buffer.len());
        Ok(instance.new_bytes(&memory.buffer[..end]))
    } else if memory.kind == 5 {
        let end = (transferred as usize).min(memory.buffer.len());
        if let Some(target) = memory.target.take() {
            // 把内核读到的字节写进目标 bytearray，再归还本对象持有的引用。
            let _ = instance.write_bytearray_at(target, 0, &memory.buffer[..end]);
            // 引用由本对象持有（上面 take 出来）。
            instance.release(target);
        }
        Ok(instance.new_int(i64::from(transferred)))
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
            // 但 windows_utils 仍会 `ov.GetOverlappedResult(True)` ⇒ 把事件置位，
            // 否则它会永远等一个不会到来的完成包。
            unsafe { sys::SetEvent(memory.raw.event) };
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

#[allow(unsafe_code)]
fn overlapped_cancel(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "Overlapped.cancel", kwargs)?;
    let (this, _rest) = split_self(instance, bound, args)?;
    let memory = overlapped_ptr(instance, this)?;
    // SAFETY: memory 来自 Box，存活到 Close；handle 为 0 时没有在途操作。
    let memory = unsafe { &mut *memory };
    if memory.handle != 0 {
        let ok = unsafe { sys::CancelIoEx(memory.handle, &mut memory.raw) };
        if ok == 0 {
            let code = last_error();
            // ERROR_NOT_FOUND=1168：没有待取消的 I/O，按幂等处理。
            if code != 1168 && code != 995 {
                return Err(raise_win(instance, code));
            }
        }
        memory.handle = 0;
    }
    set_bool_attr(instance, this, "pending", false);
    Ok(instance.new_none())
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

/// Windows Proactor 的 `_signal.set_wakeup_fd` 兼容：selft-pipe fd 是 provider 句柄，
/// `_signal` 的记账层无法校验“宿主 fd 有效且非阻塞”；这里只接受并返回前值占位（-1），
/// 让 Proactor 的自唤醒设置不被记账层挡住。selector 模式不走这条。
#[allow(unsafe_code)]
fn signal_set_wakeup_fd(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "_signal.set_wakeup_fd", kwargs)?;
    let fd = int_arg(instance, "_signal.set_wakeup_fd", args, 0)?;
    if fd < -1 {
        return Err(instance.raise_builtin_error("ValueError", "invalid fd"));
    }
    Ok(instance.new_int(-1))
}

#[allow(unsafe_code)]
fn winapi_connect_named_pipe(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let handle = int_arg(instance, "_winapi.ConnectNamedPipe", args, 0)?;
    let Some(namespace) = module_namespace(instance, "_overlapped") else {
        return Err(instance.raise_builtin_error("RuntimeError", "_overlapped 不可用"));
    };
    let Some(class) = instance.dict_get(namespace, "Overlapped") else {
        return Err(instance.raise_builtin_error("RuntimeError", "_overlapped.Overlapped 不可用"));
    };
    // 用类型调用走正常的构造/分配路径（不能手工猜 instance_size）。
    let object = pyawa_core::call_value(instance, class, &[], &[])?;
    let handle_object = instance.new_int(handle);
    let result = overlapped_connect_named_pipe(instance, Some(object), &[handle_object], &[])?;
    instance.release(result);
    Ok(object)
}

/// **安装真实面**（只由便利装配路径调用；裸 ABI 宿主仍拿到 stdlib 的诚实桩）。
pub fn install(instance: &Instance) {
    if let Some(namespace) = module_namespace(instance, "_winapi") {
        for (name, handler) in [
            ("CreateFile", create_file as pyawa_core::NativeFn),
            ("CreatePipe", create_pipe as pyawa_core::NativeFn),
            ("GetStdHandle", get_std_handle as pyawa_core::NativeFn),
            ("GetCurrentProcess", get_current_process as pyawa_core::NativeFn),
            ("DuplicateHandle", duplicate_handle as pyawa_core::NativeFn),
            ("GetFileType", get_file_type as pyawa_core::NativeFn),
            ("GetExitCodeProcess", get_exit_code_process as pyawa_core::NativeFn),
            ("TerminateProcess", terminate_process as pyawa_core::NativeFn),
            ("CreateProcess", create_process as pyawa_core::NativeFn),
            ("ReadFile", sync_read_file as pyawa_core::NativeFn),
            ("WriteFile", sync_write_file as pyawa_core::NativeFn),
            ("CreateNamedPipe", create_named_pipe as pyawa_core::NativeFn),
            ("ConnectNamedPipe", winapi_connect_named_pipe as pyawa_core::NativeFn),
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
        // `_winapi.INFINITE` 必须是 Win32 的 0xFFFFFFFF（stdlib 占位桩给了 -1；
        // Proactor 的 `math.ceil(timeout*1e3) >= INFINITE` 会因此误报 timeout too big）。
        let infinite = instance.new_int(i64::from(u32::MAX));
        instance.dict_set(namespace, "INFINITE", infinite);
    }
    if let Some(namespace) = module_namespace(instance, "_overlapped") {
        for (name, handler) in [
            ("CreateIoCompletionPort", overlapped_module_create_iocp as pyawa_core::NativeFn),
            ("GetQueuedCompletionStatus", overlapped_module_get_queued as pyawa_core::NativeFn),
            ("RegisterWaitWithQueue", register_wait_with_queue as pyawa_core::NativeFn),
            ("CreateEvent", overlapped_create_event as pyawa_core::NativeFn),
            ("UnregisterWaitEx", unregister_wait_ex as pyawa_core::NativeFn),
            ("UnregisterWait", unregister_wait_ex as pyawa_core::NativeFn),
        ] {
            let function = make_native(instance, name, handler);
            instance.dict_set(namespace, name, function);
        }
        if let Some(class) = build_overlapped_class(instance) {
            instance.dict_set(namespace, "Overlapped", class);
        }
    }
}

#[allow(unsafe_code)]
fn cli_os_close(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    no_kwargs(instance, "os.close", kwargs)?;
    let fd = int_arg(instance, "os.close", args, 0)?;
    let handle = osf_table()
        .as_mut()
        .and_then(|map| map.remove(&(fd as i32)));
    if let Some(handle) = handle {
        // SAFETY: 句柄由 msvcrt.open_osfhandle 收下。
        unsafe { sys::CloseHandle(handle) };
        return Ok(instance.new_none());
    }
    // 普通 fs fd 仍走能力域。
    let _ = instance.fs_close(fd.max(0) as u64);
    Ok(instance.new_none())
}

/// **CLI 专用**的 msvcrt 句柄转换面（`asyncio.windows_utils.Popen` 需要把管道 HANDLE 变 CRT fd）。
pub fn install_cli_msvcrt_compat(instance: &Instance) {
    if let Some(namespace) = module_namespace(instance, "msvcrt") {
        for (name, handler) in [
            ("open_osfhandle", msvcrt_open_osfhandle as pyawa_core::NativeFn),
            ("get_osfhandle", msvcrt_get_osfhandle as pyawa_core::NativeFn),
            ("_close", msvcrt_close as pyawa_core::NativeFn),
        ] {
            let function = make_native(instance, name, handler);
            instance.dict_set(namespace, name, function);
        }
    }
    if let Some(namespace) = module_namespace(instance, "os") {
        let function = make_native(instance, "close", cli_os_close as pyawa_core::NativeFn);
        instance.dict_set(namespace, "close", function);
    }
}

/// **Proactor 专用**的 `_signal.set_wakeup_fd` 兼容安装（仅 CLI 便利路径调用）。
/// 不放进通用 `install`：裸 `paL_newstate` 的 `_signal` 记账语义测试仍保持诚实桩。
pub fn install_proactor_signal_compat(instance: &Instance) {
    if let Some(namespace) = module_namespace(instance, "_signal") {
        let function = make_native(instance, "set_wakeup_fd", signal_set_wakeup_fd as pyawa_core::NativeFn);
        instance.dict_set(namespace, "set_wakeup_fd", function);
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
        ("cancel", overlapped_cancel as pyawa_core::NativeFn),
        // `GetOverlappedResult` 也属于"数据面"（没有真实投递过的 OVERLAPPED ⇒ 如实未接线，
        // 不用事件句柄硬调出假语义）。
        ("GetOverlappedResult", overlapped_get_overlapped_result as pyawa_core::NativeFn),
        ("getresult", overlapped_getresult as pyawa_core::NativeFn),
        ("ReadFile", overlapped_read_file as pyawa_core::NativeFn),
        ("WriteFile", overlapped_write_file as pyawa_core::NativeFn),
        ("ReadFileInto", overlapped_read_file_into as pyawa_core::NativeFn),
        ("WSARecv", overlapped_wsa_recv as pyawa_core::NativeFn),
        ("WSARecvInto", overlapped_wsa_recv_into as pyawa_core::NativeFn),
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
