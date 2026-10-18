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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// `fs` 域的 provider 状态：句柄表（`CP-17`：`close` 即释放）。
#[derive(Default)]
pub struct PosixFs {
    files: Mutex<HashMap<u64, std::fs::File>>,
    next: AtomicU64,
}

impl PosixFs {
    /// 新建一个 provider（句柄从 **3** 开始 ✓）。
    ///
    /// **句柄号段** ✓：`0` ＝ 无效 ✗；`1` ＝ `stdout` ✓、`2` ＝ `stderr` ✓（见 `fs_write` 的特判 ✓，
    /// `_io_module::STDOUT_HANDLE`／`STDERR_HANDLE` ✓）⇒ **真文件从 `3` 起** ✓。
    /// **真 bug**（task-18 实测）：先前从 `1` 起 ✗ ⇒ 进程里**头两个**打开的文件拿到的正是
    /// `1`／`2` 号 ✓ ⇒ 写落进 `stdout`／`stderr`（`with open(…)` 里数据跑到终端 ✓）、读报 `EBADF` ✓。
    pub fn new() -> Self {
        Self { files: Mutex::new(HashMap::new()), next: AtomicU64::new(3) }
    }

    /// 当前**打开着**的句柄数（`CP-17`／`CP-19` 的残留观察面；测试与诊断用）。
    ///
    /// 只数句柄表里的条目；标准流句柄 `1`／`2`（`fs_write` 特判）不计入。
    pub fn open_handle_count(&self) -> usize {
        self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).len()
    }

    /// 本 provider 的 vtable（`CP-30`：带版本／尺寸字段；`state` 指回自己）。
    ///
    /// **调用方必须保证** `self` 活得比该 vtable 久（CLI 里就是一个栈上局部）。
    pub fn vtable(&self) -> CpFsVtable {
        CpFsVtable {
            // **`CP-30`／`AB-39`**：能力接口与嵌入 API／宿主对象契约**共用** `PA_ABI_VERSION`
            // （`ABI` 侧的注册校验由别的线接；这里只填对值）。
            version: pyawa_abi::PA_ABI_VERSION,
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
            readlink: Some(fs_readlink),
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
    // 宿主 errno 表的**唯一真相**在 `pyawa-abi`（task-20 搬家；`pa_create` 也用它注入实例）。
    pyawa_abi::platform_errno::HOST_ERRNO
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

/// `readlink(name)` ⇒ 目标路径经 `out`／`out_len` 带回。
///
/// **两段式出参约定**（与 `listdir` 同款，见 `pyawa-core/src/instance/fs.rs::fs_readlink` 的调用侧）：
/// provider 只写 `min(目标总长, capacity)` 字节，`out_len` 回**目标的总长** ⇒ 总长超容量时调用方
/// 按新容量再要一次 ✓。**目标按字节原样回**（`std::os::unix` 的 `OsStrExt` ⇒ 非 UTF-8 也不丢 ✓；
/// 本 crate 是平台集中点，`CX-4` 不扫它 ✓）。
#[allow(unsafe_code)]
extern "C" fn fs_readlink(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    out: *mut u8,
    capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    use std::os::unix::ffi::OsStrExt;
    let path = match path_of(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match std::fs::read_link(&path) {
        Ok(target) => {
            let bytes = target.as_os_str().as_bytes();
            let written = bytes.len().min(capacity);
            if !out.is_null() && written > 0 {
                // SAFETY: `out` 由调用方按 `capacity` 给出；源与目标不重叠。
                unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, written) };
            }
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = bytes.len() };
            CapStatus::Ok
        }
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
    // **标准流句柄**（`1` ＝ `stdout`、`2` ＝ `stderr`）：不走句柄表——照参照实现，`sys.stdout`
    // 由解释器启动时装好 ✓。平台集中点在这里（`DESIGN.md` §7：写标准流用 `std::io` ✓）；
    // stdlib 侧只出**形状与文本层** ✓（`CX-4`）。
    if handle.0 == 1 || handle.0 == 2 {
        // SAFETY: 缓冲区由调用方按 `len` 给出。
        let bytes = unsafe { core::slice::from_raw_parts(buffer, len) };
        let outcome = if handle.0 == 1 {
            std::io::stdout().write_all(bytes).map(|()| len)
        } else {
            std::io::stderr().write_all(bytes).map(|()| len)
        };
        return match outcome {
            Ok(written) => {
                // SAFETY: 出参由调用方提供。
                unsafe { *out_len = written };
                CapStatus::Ok
            }
            Err(error) => {
                let errno = error.raw_os_error().unwrap_or_else(|| fallback_errno("EIO"));
                // SAFETY: 出参由调用方提供。
                unsafe { *errno_out = errno };
                CapStatus::Machine
            }
        };
    }
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

/// `close(handle)`（`CP-17`：释放；**必须幂等**）。
///
/// **`CP-17` 要求 `close` 幂等** ⇒ 对"已经不在句柄表里"的 handle（第二次 close／从未成功的
/// open）同样返回成功：资源本来就不在，`close` 的语义目标已达成。**`CP-18` 管的是别的操作**：
/// 对已关闭句柄的 `read`／`write`／`seek`／`tell`／`fstat`／`truncate`／`fsync` 一律返回
/// `EBADF` 形状的机器错误（见各槽位；**不**是 UB）。
#[allow(unsafe_code)]
extern "C" fn fs_close(
    state: *mut c_void,
    handle: Handle,
    _errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `fs_open`（只读地取句柄表）。
    let provider = unsafe { &*(state as *const PosixFs) };
    let _removed = provider
        .files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&handle.0);
    CapStatus::Ok
}

// --------------------------------------------------------------------------- #
// 受限 provider（M5 判据② 的"受限档"；`T-CP-4`／`T-CP-5`）
// --------------------------------------------------------------------------- #

/// **受限 provider**：根能力 ＝ 一个宿主机目录（chroot 语义的**名字空间映射层**）。
///
/// 形状照 `DESIGN.md` §7.4 与 `SPEC-capabilities.md` §7：
///
/// - **判定在 VM**（`CP-7`／`CP-21`／`CP-32`）：`..` 归一、符号链接解析、根处夹住都由
///   `pyawa-core` 的 `instance/path.rs` 完成 ⇒ 本 provider 收到的名字**已经是** guest 绝对名
///   （不含越界 `..`、不含未解析的逃逸链接）。本类型**不再做**任何夹根判定 —— 只把
///   guest 名字空间**映射**到自己的权威目录；
/// - 映射：命中某个允许根的**宿主绝对前缀**（如 `<root>/a`、`<Lib>/os.py`）⇒ 用那个根；
///   其它绝对路径 ⇒ chroot 语义（`/proc` ⇒ `<根>/proc`）；相对名字 ⇒ 相对主根；
/// - 额外的**只读根**（如工作区的 `Lib/`）供 import 读模块文件；落在只读根上的写 ⇒ `EACCES`
///   （真实机器错误，不是"未实现"，`CP-5`）——这是**根权限策略**，不是路径判定；
/// - `CP-23`：根下没有的路径（`/proc`、`/dev`、`/sys`）就是 `ENOENT`（看不见 ＝ 不存在）。
pub struct RestrictedFs {
    /// 根能力（canonical 主根，可写）。
    root: PathBuf,
    /// 额外只读根（canonical；例如 `Lib/`）。
    read_only_roots: Vec<PathBuf>,
    /// 真实现：句柄表全在它里面；名字类槽位先解析、再委托给它。
    inner: PosixFs,
}

impl RestrictedFs {
    /// 建一个受限 provider。
    ///
    /// `root` 必须存在（chroot 的"根"）；`read_only_roots` 里每一项也必须存在。
    /// 任一不存在 ⇒ `None`（不猜、不降级成"全权"——`CP-20`：根能力必须显式给出）。
    pub fn new(root: impl Into<PathBuf>, read_only_roots: Vec<PathBuf>) -> Option<Self> {
        let root = std::fs::canonicalize(root.into()).ok()?;
        let mut read_only = Vec::with_capacity(read_only_roots.len());
        for entry in read_only_roots {
            read_only.push(std::fs::canonicalize(entry).ok()?);
        }
        Some(Self { root, read_only_roots: read_only, inner: PosixFs::new() })
    }

    /// 根能力对应的宿主目录（canonical 形态；测试／诊断用）。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 当前打开着的句柄数（`CP-19` 的残留观察面；测试与诊断用）。
    pub fn open_handle_count(&self) -> usize {
        self.inner.open_handle_count()
    }

    /// 内层真实现的 `state` 指针（句柄类槽位直接委托）。
    fn inner_state(&self) -> *mut c_void {
        (&self.inner as *const PosixFs).cast_mut().cast::<c_void>()
    }

    /// 在允许根里找 `name` 的宿主绝对前缀 ⇒ `(base, rest)`。
    ///
    /// - 命中某个允许根的**宿主绝对路径**（如 `<root>/a`、`<Lib>/os.py`）⇒ 用那个根；
    /// - 其它绝对路径 ⇒ chroot 语义：按"主根下的同名路径"解析（`/proc` ⇒ `<root>/proc`）；
    /// - 相对路径 ⇒ 相对主根。
    fn base_of(&self, name: &str) -> (PathBuf, PathBuf) {
        let raw = Path::new(name);
        if raw.is_absolute() {
            for root in std::iter::once(&self.root).chain(self.read_only_roots.iter()) {
                if let Ok(rest) = raw.strip_prefix(root) {
                    return (root.clone(), rest.to_path_buf());
                }
            }
            let mut rest = PathBuf::new();
            for component in raw.components() {
                if let std::path::Component::Normal(part) = component {
                    rest.push(part);
                }
            }
            return (self.root.clone(), rest);
        }
        (self.root.clone(), raw.to_path_buf())
    }

    /// 把 **VM 已解析的 guest 名字**映射成宿主路径（**不做** `..`／符号链接判定——
    /// 那是 VM 的职责：`CP-21`／`CP-32`）。
    ///
    /// `leaf` 参数保留只为对齐 vtable 的叶子语义（VM 已在解析器里处理"跟不跟随最后一段"）；
    /// 映射规则见类型文档。**只读根上的写**返回真实机器错误 `EACCES`（根权限策略）。
    fn resolve(&self, name: &str, _leaf: bool, for_write: bool) -> Result<PathBuf, i32> {
        let (base, rest) = self.base_of(name);
        let mapped = base.join(rest);
        if for_write && !mapped.starts_with(&self.root) {
            return Err(fallback_errno("EACCES"));
        }
        Ok(mapped)
    }

    /// 本 provider 的 vtable（`CP-30`；`state` 指回自己，名字类槽位做根映射）。
    pub fn vtable(&self) -> CpFsVtable {
        let mut table = self.inner.vtable();
        table.state = (self as *const Self).cast_mut().cast::<c_void>();
        // 名字类槽位：把 VM 解析后的 guest 名字映射到根，再交给内层真实现。
        table.open = Some(restricted_open);
        table.stat = Some(restricted_stat);
        table.mkdir = Some(restricted_mkdir);
        table.rmdir = Some(restricted_rmdir);
        table.unlink = Some(restricted_unlink);
        table.rename = Some(restricted_rename);
        table.readlink = Some(restricted_readlink);
        // 句柄类槽位：不解析名字，直接把内层 `state` 交过去（`CP-31` 的 `fd` 仍是表下标）。
        table.fstat = Some(restricted_fstat);
        table.read = Some(restricted_read);
        table.write = Some(restricted_write);
        table.seek = Some(restricted_seek);
        table.tell = Some(restricted_tell);
        table.truncate = Some(restricted_truncate);
        table.fsync = Some(restricted_fsync);
        table.close = Some(restricted_close);
        table
    }
}

/// `name`／`len` → 文本（UTF-8；非 UTF-8 借 `EINVAL`；与 [`path_of`] 同一条口径）。
#[allow(unsafe_code)]
fn text_of(name: *const u8, len: usize) -> Result<String, i32> {
    // SAFETY: 调用方按 vtable 契约给出 `name`／`len`（`SPEC-c-abi.md` §7 的出参约定）。
    let bytes = unsafe { core::slice::from_raw_parts(name, len) };
    core::str::from_utf8(bytes).map(|text| text.to_owned()).map_err(|_| fallback_errno("EINVAL"))
}

/// 宿主路径 → 交给内层真实现的字节串（`std::os::unix` 的 `OsStrExt` ⇒ 非 UTF-8 也不丢）。
fn os_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

/// `open(flags)` 里是否含"要写"的意图（访问模式或建／截／追加位）。
fn write_intent(flags: i32) -> bool {
    let access = flags & 3;
    access == open_flag::WRONLY
        || access == open_flag::RDWR
        || flags & (open_flag::APPEND | open_flag::TRUNC | open_flag::CREAT) != 0
}

/// 名字类槽位的映射样板：把 VM 已解析的 guest 名字映射成宿主路径；出错就把 `errno`
/// 写进出参并返回 `Machine`（`CP-5` 两条通道之一）。
macro_rules! resolve_name {
    ($state:expr, $name:expr, $len:expr, $leaf:expr, $for_write:expr, $errno_out:expr) => {{
        // SAFETY: provider 由注册方保证活到 vtable 使用期（契约同 `PosixFs::vtable`）。
        let provider = unsafe { &*($state as *const RestrictedFs) };
        let text = match text_of($name, $len) {
            Ok(text) => text,
            Err(errno) => {
                // SAFETY: 出参由调用方提供。
                unsafe { *$errno_out = errno };
                return CapStatus::Machine;
            }
        };
        match provider.resolve(&text, $leaf, $for_write) {
            Ok(path) => (provider, path),
            Err(errno) => {
                // SAFETY: 出参由调用方提供。
                unsafe { *$errno_out = errno };
                return CapStatus::Machine;
            }
        }
    }};
}

#[allow(unsafe_code)]
extern "C" fn restricted_open(
    state: *mut c_void,
    name: *const u8,
    len: usize,
    flags: i32,
    mode: u32,
    out: *mut Handle,
    errno_out: *mut i32,
) -> CapStatus {
    let (provider, path) =
        resolve_name!(state, name, len, false, write_intent(flags), errno_out);
    let bytes = os_bytes(&path);
    fs_open(provider.inner_state(), bytes.as_ptr(), bytes.len(), flags, mode, out, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_stat(
    state: *mut c_void,
    name: *const u8,
    len: usize,
    out: *mut FileInfo,
    errno_out: *mut i32,
) -> CapStatus {
    let (provider, path) = resolve_name!(state, name, len, false, false, errno_out);
    let bytes = os_bytes(&path);
    fs_stat(provider.inner_state(), bytes.as_ptr(), bytes.len(), out, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_mkdir(
    state: *mut c_void,
    name: *const u8,
    len: usize,
    mode: u32,
    errno_out: *mut i32,
) -> CapStatus {
    let (provider, path) = resolve_name!(state, name, len, true, true, errno_out);
    let bytes = os_bytes(&path);
    fs_mkdir(provider.inner_state(), bytes.as_ptr(), bytes.len(), mode, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_rmdir(
    state: *mut c_void,
    name: *const u8,
    len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let (provider, path) = resolve_name!(state, name, len, true, true, errno_out);
    let bytes = os_bytes(&path);
    fs_rmdir(provider.inner_state(), bytes.as_ptr(), bytes.len(), errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_unlink(
    state: *mut c_void,
    name: *const u8,
    len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let (provider, path) = resolve_name!(state, name, len, true, true, errno_out);
    let bytes = os_bytes(&path);
    fs_unlink(provider.inner_state(), bytes.as_ptr(), bytes.len(), errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_rename(
    state: *mut c_void,
    from: *const u8,
    from_len: usize,
    to: *const u8,
    to_len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let (_source_provider, source) = resolve_name!(state, from, from_len, true, true, errno_out);
    let (provider, target) = resolve_name!(state, to, to_len, true, true, errno_out);
    let source = os_bytes(&source);
    let target = os_bytes(&target);
    fs_rename(
        provider.inner_state(),
        source.as_ptr(),
        source.len(),
        target.as_ptr(),
        target.len(),
        errno_out,
    )
}

#[allow(unsafe_code)]
extern "C" fn restricted_readlink(
    state: *mut c_void,
    name: *const u8,
    len: usize,
    out: *mut u8,
    capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    let (provider, path) = resolve_name!(state, name, len, true, false, errno_out);
    let bytes = os_bytes(&path);
    fs_readlink(
        provider.inner_state(),
        bytes.as_ptr(),
        bytes.len(),
        out,
        capacity,
        out_len,
        errno_out,
    )
}

#[allow(unsafe_code)]
extern "C" fn restricted_fstat(
    state: *mut c_void,
    handle: Handle,
    out: *mut FileInfo,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `resolve_name!`。
    let provider = unsafe { &*(state as *const RestrictedFs) };
    fs_fstat(provider.inner_state(), handle, out, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_read(
    state: *mut c_void,
    handle: Handle,
    out: *mut u8,
    capacity: usize,
    wanted: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `resolve_name!`。
    let provider = unsafe { &*(state as *const RestrictedFs) };
    fs_read(provider.inner_state(), handle, out, capacity, wanted, out_len, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_write(
    state: *mut c_void,
    handle: Handle,
    buffer: *const u8,
    len: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `resolve_name!`。
    let provider = unsafe { &*(state as *const RestrictedFs) };
    fs_write(provider.inner_state(), handle, buffer, len, out_len, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_seek(
    state: *mut c_void,
    handle: Handle,
    offset: i64,
    anchor: i32,
    out: *mut u64,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `resolve_name!`。
    let provider = unsafe { &*(state as *const RestrictedFs) };
    fs_seek(provider.inner_state(), handle, offset, anchor, out, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_tell(
    state: *mut c_void,
    handle: Handle,
    out: *mut u64,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `resolve_name!`。
    let provider = unsafe { &*(state as *const RestrictedFs) };
    fs_tell(provider.inner_state(), handle, out, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_truncate(
    state: *mut c_void,
    handle: Handle,
    size: u64,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `resolve_name!`。
    let provider = unsafe { &*(state as *const RestrictedFs) };
    fs_truncate(provider.inner_state(), handle, size, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_fsync(
    state: *mut c_void,
    handle: Handle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `resolve_name!`。
    let provider = unsafe { &*(state as *const RestrictedFs) };
    fs_fsync(provider.inner_state(), handle, errno_out)
}

#[allow(unsafe_code)]
extern "C" fn restricted_close(
    state: *mut c_void,
    handle: Handle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `resolve_name!`。
    let provider = unsafe { &*(state as *const RestrictedFs) };
    fs_close(provider.inner_state(), handle, errno_out)
}
