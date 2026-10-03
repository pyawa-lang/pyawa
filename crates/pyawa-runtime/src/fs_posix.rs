//! `fs` 域的真实机器实现（`DESIGN.md` §7：本 crate 是平台依赖的**集中点**）。
//!
//! - 形状来自 [`pyawa_capabilities::fs`]（`CP-12`：接口 crate 只含形状）；本文件是**真实实现**。
//! - 未实现的槽位**如实留 `None`**（`CP-3`：只有该操作未实现），绝不假装成功。
//! - 错误分两条通道（`CP-5`）：机器错误带 `errno`，未实现单独一态；本文件返回
//!   [`CapStatus::Machine`] ＋ 出参 `errno`，数值取自宿主平台的 `raw_os_error()`。
//!
//! `unsafe` 按**项**开许可（本 crate 的规矩：不许整文件放开）。

use core::ffi::c_void;
use pyawa_capabilities::fs::{
    open_flag, whence, CapStatus, CpFsVtable, FileInfo, Handle,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// `fs` 域的 provider 状态：句柄表（`CP-17`：`close` 即释放）。
#[derive(Default)]
pub struct PosixFs {
    files: Mutex<HashMap<u64, std::fs::File>>,
    next: AtomicU64,
}

impl PosixFs {
    /// 新建一个 provider（句柄从 1 开始，0 保留给"无效"）。
    pub fn new() -> Self {
        Self { files: Mutex::new(HashMap::new()), next: AtomicU64::new(1) }
    }

    /// 本 provider 的 vtable（`CP-30`：带版本／尺寸字段；`state` 指回自己）。
    ///
    /// **调用方必须保证** `self` 活得比该 vtable 久（CLI 里就是一个栈上局部）。
    pub fn vtable(&self) -> CpFsVtable {
        CpFsVtable {
            version: 1,
            size: core::mem::size_of::<CpFsVtable>() as u32,
            state: (self as *const Self).cast_mut().cast::<c_void>(),
            open: Some(fs_open),
            stat: Some(fs_stat),
            fstat: Some(fs_fstat),
            // 目录句柄与元信息修改面要等 `posix`／`_io` 落地（`CP-21` 的名字解析也在那时钉死）
            listdir: None,
            mkdir: Some(fs_mkdir),
            rmdir: Some(fs_rmdir),
            unlink: Some(fs_unlink),
            rename: Some(fs_rename),
            readlink: None,
            symlink: None,
            chmod: None,
            utime: None,
            read: Some(fs_read),
            write: Some(fs_write),
            seek: Some(fs_seek),
            tell: Some(fs_tell),
            truncate: Some(fs_truncate),
            fsync: Some(fs_fsync),
            close: Some(fs_close),
        }
    }
}

/// 把 `io::Error` 折成 `errno`：优先用宿主的原生值；**没有**原生值的错误（例如名字含 NUL）
/// 借用 `EIO` —— 具体分类随 `posix` 面落地时再细化。
fn errno_of(error: &std::io::Error) -> i32 {
    match error.raw_os_error() {
        Some(value) => value,
        None => fallback_errno("EIO"),
    }
}

fn fallback_errno(name: &str) -> i32 {
    crate::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or(0)
}

/// `name`／`len` 变成路径（UTF-8；非 UTF-8 借 `EINVAL`）。
#[allow(unsafe_code)]
fn path_of(name: *const u8, len: usize) -> Result<std::path::PathBuf, i32> {
    // SAFETY: 调用方按 vtable 契约给出 `name`／`len`（`SPEC-c-abi.md` §7 的出参约定）。
    let bytes = unsafe { core::slice::from_raw_parts(name, len) };
    match core::str::from_utf8(bytes) {
        Ok(text) => Ok(std::path::PathBuf::from(text)),
        Err(_) => Err(fallback_errno("EINVAL")),
    }
}

/// `open(name, flags, mode)`（`SPEC-capabilities.md` §9.1）。
#[allow(unsafe_code)]
extern "C" fn fs_open(
    state: *mut c_void,
    name: *const u8,
    len: usize,
    flags: i32,
    _mode: u32,
    out: *mut Handle,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match path_of(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    let mut options = std::fs::OpenOptions::new();
    let read = flags & 3 == open_flag::RDONLY || flags & 3 == open_flag::RDWR;
    let write = flags & 3 == open_flag::WRONLY || flags & 3 == open_flag::RDWR;
    options.read(read).write(write);
    if flags & open_flag::APPEND != 0 {
        options.append(true);
    }
    if flags & open_flag::TRUNC != 0 {
        options.truncate(true);
    }
    if flags & open_flag::CREAT != 0 {
        options.create(true);
    }
    // SAFETY: provider 状态由 `vtable()` 的 `state` 给出，且比 vtable 活得久。
    let provider = unsafe { &*(state as *const PosixFs) };
    match options.open(&path) {
        Ok(file) => {
            let id = provider.next.fetch_add(1, Ordering::Relaxed);
            provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(id, file);
            // SAFETY: 出参由调用方提供。
            unsafe { *out = Handle(id) };
            CapStatus::Ok
        }
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `stat(name)`。
#[allow(unsafe_code)]
extern "C" fn fs_stat(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    out: *mut FileInfo,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match path_of(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match std::fs::metadata(&path) {
        Ok(meta) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *out = info_of(&meta) };
            CapStatus::Ok
        }
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `fstat(file)`。
#[allow(unsafe_code)]
extern "C" fn fs_fstat(
    state: *mut c_void,
    handle: Handle,
    out: *mut FileInfo,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const PosixFs) };
    let files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    match file.metadata() {
        Ok(meta) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *out = info_of(&meta) };
            CapStatus::Ok
        }
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

fn info_of(meta: &std::fs::Metadata) -> FileInfo {
    FileInfo {
        size: meta.len(),
        mode: 0,
        is_dir: meta.is_dir(),
        dev: 0,
        ino: 0,
    }
}

/// `mkdir(name, mode)`。
#[allow(unsafe_code)]
extern "C" fn fs_mkdir(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    _mode: u32,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match path_of(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match std::fs::create_dir(&path) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `rmdir(name)`。
#[allow(unsafe_code)]
extern "C" fn fs_rmdir(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match path_of(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match std::fs::remove_dir(&path) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `unlink(name)`。
#[allow(unsafe_code)]
extern "C" fn fs_unlink(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match path_of(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match std::fs::remove_file(&path) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `rename(a, b)`。
#[allow(unsafe_code)]
extern "C" fn fs_rename(
    _state: *mut c_void,
    from: *const u8,
    from_len: usize,
    to: *const u8,
    to_len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let source = match path_of(from, from_len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    let target = match path_of(to, to_len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match std::fs::rename(&source, &target) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `read(file, n)` ⇒ 实际读到的字节数经 `out_len` 带回。
#[allow(unsafe_code)]
extern "C" fn fs_read(
    state: *mut c_void,
    handle: Handle,
    out: *mut u8,
    capacity: usize,
    _wanted: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    use std::io::Read;
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const PosixFs) };
    let mut files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get_mut(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    // SAFETY: 缓冲区由调用方按 `capacity` 给出。
    let buffer = unsafe { core::slice::from_raw_parts_mut(out, capacity) };
    match file.read(buffer) {
        Ok(read) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = read };
            CapStatus::Ok
        }
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `write(file, buf)`。
#[allow(unsafe_code)]
extern "C" fn fs_write(
    state: *mut c_void,
    handle: Handle,
    buffer: *const u8,
    len: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    use std::io::Write;
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const PosixFs) };
    let mut files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get_mut(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    // SAFETY: 缓冲区由调用方按 `len` 给出。
    let bytes = unsafe { core::slice::from_raw_parts(buffer, len) };
    match file.write(bytes) {
        Ok(written) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = written };
            CapStatus::Ok
        }
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `seek(file, offset, whence)`。
#[allow(unsafe_code)]
extern "C" fn fs_seek(
    state: *mut c_void,
    handle: Handle,
    offset: i64,
    anchor: i32,
    out: *mut u64,
    errno_out: *mut i32,
) -> CapStatus {
    use std::io::{Seek, SeekFrom};
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const PosixFs) };
    let mut files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get_mut(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    if anchor == whence::SET && offset < 0 {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EINVAL") };
        return CapStatus::Machine;
    }
    let from = match anchor {
        whence::SET => SeekFrom::Start(offset as u64),
        whence::CUR => SeekFrom::Current(offset),
        _ => SeekFrom::End(offset),
    };
    match file.seek(from) {
        Ok(position) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *out = position };
            CapStatus::Ok
        }
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `tell(file)`。
#[allow(unsafe_code)]
extern "C" fn fs_tell(
    state: *mut c_void,
    handle: Handle,
    out: *mut u64,
    errno_out: *mut i32,
) -> CapStatus {
    use std::io::{Seek, SeekFrom};
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const PosixFs) };
    let mut files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get_mut(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    match file.seek(SeekFrom::Current(0)) {
        Ok(position) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *out = position };
            CapStatus::Ok
        }
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `truncate(file, size)`。
#[allow(unsafe_code)]
extern "C" fn fs_truncate(
    state: *mut c_void,
    handle: Handle,
    size: u64,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const PosixFs) };
    let files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    match file.set_len(size) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `fsync(file)`。
#[allow(unsafe_code)]
extern "C" fn fs_fsync(
    state: *mut c_void,
    handle: Handle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const PosixFs) };
    let files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    match file.sync_all() {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `close(handle)`（`CP-17`：释放）。
#[allow(unsafe_code)]
extern "C" fn fs_close(
    state: *mut c_void,
    handle: Handle,
    _errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `fs_open`（只读地取句柄表）。
    let provider = unsafe { &*(state as *const PosixFs) };
    let removed = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&handle.0);
    match removed {
        Some(_file) => CapStatus::Ok,
        None => CapStatus::Machine,
    }
}
