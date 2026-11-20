//! Windows `fs` 域的真实机器实现（`PLAN §12.20` W3）。
//!
//! - 形状来自 [`pyawa_capabilities::fs`]（`CP-12`：接口 crate 只含形状）；本文件是**真实实现**，
//!   与 `fs_posix.rs` 逐槽位同语义（签名／出参协议／`errno_out`／`CapStatus::Machine` 全照抄）。
//! - **`CP-21` 的判定在 VM**（`crates/pyawa-core/src/instance/path.rs`）：provider 拿到的名字**已经**
//!   是夹根后的 guest 绝对名（无越界 `..`、无未解析的逃逸链接）⇒ 本文件**只做 guest → 宿主映射**，
//!   绝不再次解释 `..`／绝对性。
//! - 错误分两条通道（`CP-5`）：机器错误走 [`CapStatus::Machine`] ＋ `errno_out`；未实现的槽位
//!   **如实** `None`（`CP-3`）。
//!
//! # guest → 宿主路径映射
//!
//! guest 空间用 `/` 分层（Windows 上 cwd 初值形如 `/C:/Users/x`；见 `path.rs` 的
//! `is_path_separator`）。输入字节按 UTF-8 损失转换（`from_utf8_lossy`；Windows 宿主路径是
//! UTF-16，guest 侧本来就是 UTF-8），`\` 等价于 `/`，然后：
//!
//! 1. 去掉前导 `/`；
//! 2. 剩下部分以 `X:`（盘符）开头 ⇒ 宿主路径 ＝ 把 `/` 换回 `\` 的该串；
//! 3. 否则 ⇒ 宿主路径 ＝ **当前盘的根**（`std::env::current_dir()` 的首段，如 `C:`，用
//!    [`OnceLock`] 缓存）＋ `\` ＋ 该串；空串 ⇒ 当前盘根。取不到当前盘根 ⇒ `EACCES` 形状的
//!    机器错误。
//!
//! `readlink` 出参做**近似逆映射**：绝对宿主目标 `C:\a\b` ⇒ guest `/C:/a/b`（VM 的解析器据此
//! 把它当绝对目标从 `/` 重来）；相对目标**原样**返回（相对链接所在目录，POSIX 语义）。
//! `symlink` 的**相对**目标同样原样保留（`fs_posix` 的 `path_of` 也不把目标绝对化）。
//!
//! # 错误码映射
//!
//! `std::io::Error::raw_os_error()` 在 Windows 上是 **Win32 错误码**，不是 POSIX `errno`；本文件
//! 按 `CM-20` 的口径把它映射成 `pyawa_abi::platform_errno::HOST_ERRNO` 表里**同名的 POSIX
//! `errno` 数值**（表的唯一真相在 `pyawa-abi`，这里只按名字查）。
//!
//! # 如实边界（不假装完成）
//!
//! 1. **`pipe` 给 `None`**（`CP-3`）：Windows 匿名管道属后续轮次；只有该操作未实现，同域其余
//!    槽位不受影响。`CP-5` 禁止把它编码成某个 `errno`。
//! 2. **目录句柄 → 路径**：`listdir` 的 ABI 收**句柄**，而 `fs_posix` 用 `/proc/self/fd/<fd>`
//!    把句柄变回名字；Windows 的 `std` **没有**公开的"句柄 → 路径"面 ⇒ 本实现在 `open` 认出
//!    目录时把宿主名字记在 [`WindowsFs::dir_paths`]，`listdir` 用 `std::fs::read_dir` 重新枚举
//!    （读的是**调用时**的目录内容，语义与 `fs_posix` 一致）。这是对"句柄表只有 `files`"这一
//!    形状的**必要追加**：换成 `GetFinalPathNameByHandleW` 一类 Win32 FFI 可以省掉这份状态，
//!    但那需要在本 crate 里再开一段 kernel32 FFI（`std` 之外），故先按纯 `std` 实现。
//! 3. **`mode`**：Windows 没有 POSIX 权限位。`stat`／`fstat` 报 `S_IFREG | 0o644`（文件）／
//!    `S_IFDIR | 0o755`（目录）——**带类型位**，与 `fs_posix` 直接报宿主 `st_mode`（含类型位）的
//!    口径对齐：stdlib 的 `genericpath.isdir` 走 `stat.S_ISDIR(st.st_mode)`（`Lib/genericpath.py:54`），
//!    缺 `S_IFDIR` 会让 `os.path.isdir` 全假。`mkdir`／`open` 的 `mode` 参数在 Windows 上忽略
//!    （`chmod` 才把"写位 → 只读属性"落下来）。
//! 4. **`chmod`**：只映射"写位 → 只读属性"（`mode & 0o222 == 0` ⇒ readonly），其余位忽略。
//! 5. **`utime`／`lutime`／`utime_ns`**：`File::set_times`（`std::fs::FileTimes`）。写属性需要
//!    `FILE_WRITE_ATTRIBUTES` ⇒ 用 `OpenOptionsExt::access_mode` 打开（`lu` 版另加
//!    `FILE_FLAG_OPEN_REPARSE_POINT` 拿到链接自身）。
//! 6. **`symlink`**：只有 `symlink_file`（`CpFsVtable` 的 `symlink` 槽不带
//!    `target_is_directory`）⇒ 目录链接留 W5／W6。建链接需要特权／开发者模式；失败按机器错误
//!    原样回报（不吞错）。
//! 7. **非 UTF-8 名字**：`from_utf8_lossy`／`to_string_lossy` 会替换非法字节（Windows 宿主名是
//!    UTF-16，无法无损往返任意字节串）；guest 侧本来就是 UTF-8，正常路径零损失。
//!
//! `unsafe` 按**项**开许可（本 crate 的规矩：`lib.rs` 的 `#![deny(unsafe_code)]` 不许整文件放开）。

use core::ffi::c_void;
use pyawa_capabilities::fs::{
    open_flag, whence, CapStatus, CpFsVtable, FileInfo, FileTimes, Handle,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// `FILE_FLAG_BACKUP_SEMANTICS`（Win32 `0x0200_0000`）：`CreateFile` 打开**目录句柄**必需。
///
/// `std::fs::File::open` 打开的目录句柄不带它 ⇒ `listdir` 拿不到目录句柄；本 provider 的
/// `open` 一律带上（普通文件忽略该标志）。
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
/// `FILE_FLAG_OPEN_REPARSE_POINT`（Win32 `0x0020_0000`）：打开**链接自身**（`lutime` 用）。
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
/// `FILE_WRITE_ATTRIBUTES`（Win32 `0x0000_0100`）：`SetFileTime` 需要的访问权（`utime` 一族）。
const FILE_WRITE_ATTRIBUTES: u32 = 0x0000_0100;

/// `S_IFREG`（`Lib/stat.py:34` 的口径）：普通文件类型位。
const S_IFREG: u32 = 0o100_000;
/// `S_IFDIR`（`Lib/stat.py:36` 的口径）：目录类型位。
const S_IFDIR: u32 = 0o040_000;
/// 文件的权限位（Windows 没有真实权限位，只给参照的常见值）。
const FILE_MODE: u32 = 0o644;
/// 目录的权限位（同上）。
const DIR_MODE: u32 = 0o755;

/// `fs` 域的 Windows provider 状态：句柄表（`CP-17`：`close` 即释放）。
pub struct WindowsFs {
    files: Mutex<HashMap<u64, std::fs::File>>,
    /// 目录句柄 → 宿主路径（`listdir` 用；见模块文档"如实边界"第 2 条）。
    ///
    /// 只记**目录**句柄（`open` 时按 `Metadata::is_dir` 判定）；句柄随 `close` 一起删除。
    dir_paths: Mutex<HashMap<u64, PathBuf>>,
    next: AtomicU64,
}

impl WindowsFs {
    /// 新建一个 provider（句柄从 **3** 起 ✓）。
    ///
    /// **句柄号段**与 `fs_posix` 同口径：`0` ＝ 无效 ✗；`1` ＝ `stdout` ✓、`2` ＝ `stderr` ✓
    /// （`_io_module::STDOUT_HANDLE`／`STDERR_HANDLE`；`fs_write` 特判）⇒ **真文件从 `3` 起** ✓。
    pub fn new() -> Self {
        Self {
            files: Mutex::new(HashMap::new()),
            dir_paths: Mutex::new(HashMap::new()),
            next: AtomicU64::new(3),
        }
    }

    /// 当前**打开着**的句柄数（`CP-17`／`CP-19` 的残留观察面；测试与诊断用）。
    ///
    /// 只数句柄表里的条目；标准流句柄 `1`／`2`（`fs_write` 特判）不计入。
    pub fn open_handle_count(&self) -> usize {
        self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).len()
    }

    /// 本 provider 的 vtable（`CP-30`：带版本／尺寸字段；`state` 指回自己）。
    ///
    /// **调用方必须保证** `self` 活得比该 vtable 久（组合根里泄漏到进程末，见 `lib.rs`）。
    pub fn vtable(&self) -> CpFsVtable {
        CpFsVtable {
            // **`CP-30`／`AB-39`**：能力接口与嵌入 API 共用 `PA_ABI_VERSION`。
            version: pyawa_abi::PA_ABI_VERSION,
            size: core::mem::size_of::<CpFsVtable>() as u32,
            state: (self as *const Self).cast_mut().cast::<c_void>(),
            open: Some(fs_open),
            stat: Some(fs_stat),
            fstat: Some(fs_fstat),
            listdir: Some(fs_listdir),
            mkdir: Some(fs_mkdir),
            rmdir: Some(fs_rmdir),
            unlink: Some(fs_unlink),
            rename: Some(fs_rename),
            readlink: Some(fs_readlink),
            symlink: Some(fs_symlink),
            chmod: Some(fs_chmod),
            utime: Some(fs_utime),
            read: Some(fs_read),
            write: Some(fs_write),
            seek: Some(fs_seek),
            tell: Some(fs_tell),
            truncate: Some(fs_truncate),
            fsync: Some(fs_fsync),
            close: Some(fs_close),
            // **`CP-30` 的追加槽**：读文件时间戳（`os.stat_result` 的 `st_mtime` 一族）。
            stat_times: Some(fs_stat_times),
            // **`CP-3`／`CP-5`**：Windows 匿名管道属后续轮次 ⇒ 只有该操作未实现（不是某个 errno）。
            pipe: None,
            // **round 109 追加槽**：不跟随符号链接的 `utime`（`shutil.copy2` 的 symlink 分支）。
            lutime: Some(fs_lutime),
            // **round 109 追加槽**：纳秒级、跟随叶子的 `utime`。
            utime_ns: Some(fs_utime_ns),
        }
    }
}

impl Default for WindowsFs {
    fn default() -> Self {
        Self::new()
    }
}

// --------------------------------------------------------------------------- //
// 名字 ↔ 宿主路径、errno 映射
// --------------------------------------------------------------------------- //

/// 把 `io::Error` 折成 **POSIX `errno` 数值**（Windows 上是 Win32 错误码，见模块文档）。
fn errno_of(error: &std::io::Error) -> i32 {
    match error.raw_os_error() {
        Some(code) => win32_errno(code),
        None => fallback_errno("EIO"),
    }
}

/// Win32 错误码 ⇒ POSIX `errno` 名字（`CM-20`：映射**按名字**，数字一律查 `pyawa-abi` 的表）。
fn win32_errno(code: i32) -> i32 {
    let name = match code {
        // `ERROR_FILE_NOT_FOUND`／`ERROR_PATH_NOT_FOUND`
        2 | 3 => "ENOENT",
        // `ERROR_ACCESS_DENIED`；`ERROR_SHARING_VIOLATION`／`ERROR_LOCK_VIOLATION`
        5 | 32 | 33 => "EACCES",
        // `ERROR_INVALID_HANDLE`
        6 => "EBADF",
        // `ERROR_FILE_EXISTS`／`ERROR_ALREADY_EXISTS`
        80 | 183 => "EEXIST",
        // `ERROR_DIR_NOT_EMPTY`
        145 => "ENOTEMPTY",
        // `ERROR_DIRECTORY`（目标不是目录）
        267 => "ENOTDIR",
        // `ERROR_INVALID_NAME`／`ERROR_INVALID_PARAMETER`／`ERROR_INVALID_FUNCTION`
        123 | 87 | 1 => "EINVAL",
        // `ERROR_DISK_FULL`／`ERROR_HANDLE_DISK_FULL`
        112 => "ENOSPC",
        // `ERROR_FILENAME_EXCED_RANGE`
        206 => "ENAMETOOLONG",
        // `ERROR_PRIVILEGE_NOT_HELD`（建符号链接无特权）
        1314 => "EPERM",
        // 重解析点探测：`ERROR_NOT_A_REPARSE_POINT` 一族（`std::fs::read_link` 对普通文件报它；
        // VM 的符号链接解析靠 `EINVAL` 判"不是链接"，`CP-21`）
        4390 | 4392 | 4393 => "EINVAL",
        // 其余一律 `EIO`（不编造更精确的语义：`CP-5` 只承诺"机器错误"这一态）。
        _ => "EIO",
    };
    fallback_errno(name)
}

/// 按名字取宿主 `errno`（宿主表的唯一真相在 `pyawa-abi`；`pa_create` 也用它注入实例）。
fn fallback_errno(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or(0)
}

/// 是否 `X:`／`X:\…` 形态（盘符）。
fn is_drive_prefixed(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// 当前盘的根（如 `C:`；UNC 前缀则形如 `\\server\share`）。
///
/// 缓存一次（进程 cwd 变了也不会重算——guest 空间的根是稳定的，见 `path.rs` 的 `cwd` 口径）。
fn drive_root() -> Result<String, i32> {
    static ROOT: OnceLock<String> = OnceLock::new();
    let root = ROOT.get_or_init(|| current_drive_root().unwrap_or_default());
    if root.is_empty() {
        // 取不到当前盘（cwd 不可用）⇒ 机器错误（不当成"未实现"，`CP-5`）。
        return Err(fallback_errno("EACCES"));
    }
    Ok(root.clone())
}

/// 从 `std::env::current_dir()` 的首段取盘根（`Component::Prefix`）。
fn current_drive_root() -> Option<String> {
    use std::path::Component;
    let cwd = std::env::current_dir().ok()?;
    match cwd.components().next()? {
        Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().into_owned()),
        _ => None,
    }
}

/// guest 名字（`*const u8` ＋ `len`）→ 宿主路径。
#[allow(unsafe_code)]
fn host_path(name: *const u8, len: usize) -> Result<PathBuf, i32> {
    if len == 0 {
        // 空名字由 VM 先挡（`ENOENT`）；真到这里就当 guest 根（空串 ⇒ 当前盘根，见模块文档）。
        return host_path_from_bytes(&[]);
    }
    if name.is_null() {
        // 违反 vtable 出参契约 ⇒ 机器错误（而不是 UB）。
        return Err(fallback_errno("EINVAL"));
    }
    // SAFETY: 调用方按 vtable 契约给出 `name`／`len`，且缓冲区在本调用期间有效。
    let bytes = unsafe { core::slice::from_raw_parts(name, len) };
    host_path_from_bytes(bytes)
}

/// guest 字节串 → 宿主路径（映射规则见模块文档；**不**解释 `..`／绝对性）。
fn host_path_from_bytes(bytes: &[u8]) -> Result<PathBuf, i32> {
    // Windows 宿主名是 UTF-16；guest 侧是 UTF-8 ⇒ 非法字节只能损失转换（见模块文档第 7 条）。
    let text = String::from_utf8_lossy(bytes);
    if text.contains('\0') {
        // 宿主路径里禁止内嵌 NUL ⇒ `EINVAL`（`strlen` 一族会截断，不能静默截断）。
        return Err(fallback_errno("EINVAL"));
    }
    // guest 空间统一 `/`；输入侧的 `\` 等价归一（与 `path.rs` 的 `is_path_separator` 同口径）。
    let unified = text.replace('\\', "/");
    let trimmed = unified.trim_start_matches('/');
    if is_drive_prefixed(trimmed) {
        return Ok(PathBuf::from(trimmed.replace('/', "\\")));
    }
    let root = drive_root()?;
    if trimmed.is_empty() {
        // guest 根 ⇒ 当前盘根（`C:\`；`C:` 单独一层不是根）。
        return Ok(PathBuf::from(format!("{root}\\")));
    }
    Ok(PathBuf::from(format!("{root}\\{}", trimmed.replace('/', "\\"))))
}

/// 宿主路径 → guest 名字（`readlink` 出参的近似逆映射，见模块文档）。
fn guest_name_of_host(path: &Path) -> Vec<u8> {
    let slashed = path.to_string_lossy().replace('\\', "/");
    // 去掉 `\\?\` 标识（verbatim 形态是 `//?/`）。
    let text = if let Some(rest) = slashed.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else if let Some(rest) = slashed.strip_prefix("//?/") {
        rest.to_owned()
    } else {
        slashed
    };
    if is_drive_prefixed(&text) {
        // `C:/a/b` ⇒ `/C:/a/b`（VM 当绝对 guest 名，从根重来）。
        let mut guest = String::with_capacity(text.len() + 1);
        guest.push('/');
        guest.push_str(&text);
        guest.into_bytes()
    } else {
        // 相对目标原样（相对链接所在目录）。
        text.into_bytes()
    }
}

/// `symlink` 的**目标**：绝对／盘符形态 ⇒ 按映射规则；相对形态 ⇒ 原样保留（POSIX 语义）。
#[allow(unsafe_code)]
fn symlink_target(target: *const u8, len: usize) -> Result<PathBuf, i32> {
    if len == 0 {
        // 参照 `symlink("")` ⇒ `ENOENT`。
        return Err(fallback_errno("ENOENT"));
    }
    if target.is_null() {
        return Err(fallback_errno("EINVAL"));
    }
    // SAFETY: 调用方按 vtable 契约给出 `target`／`len`，且缓冲区在本调用期间有效。
    let bytes = unsafe { core::slice::from_raw_parts(target, len) };
    let text = String::from_utf8_lossy(bytes);
    if text.contains('\0') {
        return Err(fallback_errno("EINVAL"));
    }
    let unified = text.replace('\\', "/");
    if unified.starts_with('/') || is_drive_prefixed(&unified) {
        return host_path_from_bytes(bytes);
    }
    // 相对目标**不**绝对化（`fs_posix` 的 `path_of` 同样原样交给 `symlink(2)`）。
    Ok(PathBuf::from(unified.replace('/', "\\")))
}

// --------------------------------------------------------------------------- //
// 元信息与时间戳
// --------------------------------------------------------------------------- //

/// 宿主 `Metadata` ⇒ [`FileInfo`]（`mode` 带类型位，见模块文档第 3 条）。
fn info_of(meta: &std::fs::Metadata) -> FileInfo {
    let is_dir = meta.is_dir();
    FileInfo {
        size: meta.len(),
        mode: if is_dir { S_IFDIR | DIR_MODE } else { S_IFREG | FILE_MODE },
        is_dir,
        // **`dev`／`ino` 如实给 0**（`CP-23` 的保留面；不编造）：`volume_serial_number()`／
        // `file_index()` 在 std 1.98 仍是 **unstable**（`windows_by_handle`，issue #63010）⇒
        // 稳定版 provider 不能用；等它们稳定（或本 crate 另开 `GetFileInformationByHandle` FFI）
        // 再填真值。**已知影响**：`os.path.samefile` 在 Windows 上因此不可信（与 `fs_posix` 的
        // 真 `st_dev`／`st_ino` 不同口径）。
        dev: 0,
        ino: 0,
    }
}

/// `SystemTime` ⇒ Unix epoch 起的纳秒（取不到 ⇒ `0`；同 `fs_posix` 的 `unix_ns`）。
fn unix_ns(time: std::io::Result<std::time::SystemTime>) -> i64 {
    time.ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok())
        .unwrap_or(0)
}

fn times_of(meta: &std::fs::Metadata) -> FileTimes {
    FileTimes {
        atime_ns: unix_ns(meta.accessed()),
        mtime_ns: unix_ns(meta.modified()),
        // Windows 有真正的 **creation time** ⇒ `st_ctime` 在这里是"创建时间"（与 POSIX 的
        // inode 变更时间语义不同；`fs_posix` 用 `created()` 也是同样近似，见其注释）。
        ctime_ns: unix_ns(meta.created()),
    }
}

/// 秒（Unix epoch 起，可为负）⇒ `SystemTime`（同 `fs_posix::fs_utime` 的口径）。
fn time_of_secs(seconds: i64) -> std::time::SystemTime {
    use std::time::{Duration, UNIX_EPOCH};
    if seconds >= 0 {
        UNIX_EPOCH + Duration::from_secs(seconds as u64)
    } else {
        UNIX_EPOCH - Duration::from_secs(seconds.unsigned_abs())
    }
}

/// 纳秒（Unix epoch 起，可为负）⇒ `SystemTime`。
fn time_of_ns(ns: i64) -> std::time::SystemTime {
    use std::time::{Duration, UNIX_EPOCH};
    if ns >= 0 {
        UNIX_EPOCH + Duration::from_nanos(ns as u64)
    } else {
        UNIX_EPOCH - Duration::from_nanos(ns.unsigned_abs())
    }
}

/// 给路径回填访问／修改时间：`follow = false` ⇒ 打开**链接自身**（`lutime`）。
///
/// `SetFileTime` 要 `FILE_WRITE_ATTRIBUTES` ⇒ 用 `OpenOptionsExt::access_mode` 打开；目录另加
/// `FILE_FLAG_BACKUP_SEMANTICS`（`std::fs::set_times` 在 Windows 上就是这套组合）。
fn set_times_at(
    path: &Path,
    atime: std::time::SystemTime,
    mtime: std::time::SystemTime,
    follow: bool,
) -> std::io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    options.access_mode(FILE_WRITE_ATTRIBUTES);
    let mut flags = FILE_FLAG_BACKUP_SEMANTICS;
    if !follow {
        flags |= FILE_FLAG_OPEN_REPARSE_POINT;
    }
    options.custom_flags(flags);
    let file = options.open(path)?;
    let times = std::fs::FileTimes::new().set_accessed(atime).set_modified(mtime);
    file.set_times(times)
}

// --------------------------------------------------------------------------- //
// 槽位实现（逐条对齐 `fs_posix.rs`）
// --------------------------------------------------------------------------- //

/// `open(name, flags, mode)`（`SPEC-capabilities.md` §9.1）。
///
/// `mode` 在 Windows 上忽略（没有 POSIX 权限位；见模块文档第 3 条）。
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
    use std::os::windows::fs::OpenOptionsExt;
    let path = match host_path(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    let mut options = std::fs::OpenOptions::new();
    let access = flags & 3;
    let read = access == open_flag::RDONLY || access == open_flag::RDWR;
    let write = access == open_flag::WRONLY || access == open_flag::RDWR;
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
    // 打开**目录**句柄必需（`listdir` 的 ABI 收目录句柄；见模块文档第 2 条）。
    options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    match options.open(&path) {
        Ok(file) => {
            // SAFETY: provider 状态由 `vtable()` 的 `state` 给出，且比 vtable 活得久。
            let provider = unsafe { &*(state as *const WindowsFs) };
            let id = provider.next.fetch_add(1, Ordering::Relaxed);
            let is_dir = file.metadata().map(|meta| meta.is_dir()).unwrap_or(false);
            provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(id, file);
            if is_dir {
                provider
                    .dir_paths
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(id, path);
            }
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
    let path = match host_path(name, len) {
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
    let provider = unsafe { &*(state as *const WindowsFs) };
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

/// **目录名列表**（`listdir` 槽的共用实现）：`NUL` 分隔的两段式出参（`out_len` 回**总字节数**）。
///
/// 与 `fs_posix` 用 `/proc/self/fd/<fd>` 同职：句柄 → 可枚举的名字（这里走 `open` 时记下的
/// `dir_paths`，见模块文档第 2 条）。
#[allow(unsafe_code)]
fn listdir_names(state: *mut c_void, handle: Handle) -> Result<Vec<Vec<u8>>, i32> {
    // SAFETY: provider 由注册方保证活到 vtable 使用期（契约同 `WindowsFs::vtable`）。
    let provider = unsafe { &*(state as *const WindowsFs) };
    // 先判句柄是否存在（不存在 ⇒ `EBADF`；存在但不是目录 ⇒ `ENOTDIR`，同 `fs_posix` 的
    // `/proc/self/fd/<fd>` 打开普通文件再 `read_dir` 的机器错误）。
    let present = provider
        .files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains_key(&handle.0);
    if !present {
        return Err(fallback_errno("EBADF"));
    }
    let path = {
        let dirs = provider.dir_paths.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match dirs.get(&handle.0) {
            Some(path) => path.clone(),
            None => return Err(fallback_errno("ENOTDIR")),
        }
    };
    let entries = std::fs::read_dir(&path).map_err(|error| errno_of(&error))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| errno_of(&error))?;
        // 只要文件名那一段（Windows 宿主名是 UTF-16 ⇒ 损失转换成 UTF-8 字节）。
        names.push(entry.file_name().to_string_lossy().into_owned().into_bytes());
    }
    Ok(names)
}

/// `listdir(dir)`：**两段式出参**（`NUL` 分隔、`out_len` 回总字节数；容量不够只写 `capacity`）。
#[allow(unsafe_code)]
extern "C" fn fs_listdir(
    state: *mut c_void,
    handle: Handle,
    out: *mut u8,
    capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    let names = match listdir_names(state, handle) {
        Ok(names) => names,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    let mut blob = Vec::new();
    for name in &names {
        blob.extend_from_slice(name);
        blob.push(0);
    }
    let written = blob.len().min(capacity);
    if !out.is_null() && written > 0 {
        // SAFETY: `out` 由调用方按 `capacity` 给出；源与目标不重叠。
        unsafe { core::ptr::copy_nonoverlapping(blob.as_ptr(), out, written) };
    }
    // SAFETY: 出参由调用方提供。
    unsafe { *out_len = blob.len() };
    CapStatus::Ok
}

/// `mkdir(name, mode)`（`mode` 在 Windows 上忽略：没有 POSIX 权限位）。
#[allow(unsafe_code)]
extern "C" fn fs_mkdir(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    _mode: u32,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match host_path(name, len) {
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

/// `rmdir(name)`（非空目录 ⇒ `ERROR_DIR_NOT_EMPTY` ⇒ `ENOTEMPTY`）。
#[allow(unsafe_code)]
extern "C" fn fs_rmdir(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match host_path(name, len) {
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
    let path = match host_path(name, len) {
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

/// `rename(a, b)`（`std::fs::rename` 在 Windows 上对应 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`，
/// 与 POSIX `rename(2)` 的"替换已存在目标"语义一致）。
#[allow(unsafe_code)]
extern "C" fn fs_rename(
    _state: *mut c_void,
    from: *const u8,
    from_len: usize,
    to: *const u8,
    to_len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let source = match host_path(from, from_len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    let target = match host_path(to, to_len) {
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

/// `readlink(name)` ⇒ 目标经 `out`／`out_len` 带回。
///
/// **两段式出参约定**（与 `listdir` 同款）：只写 `min(目标总长, capacity)` 字节，`out_len` 回
/// **目标的总长** ⇒ 总长超容量时调用方按新容量再要一次。目标做 guest → 宿主映射的**逆向**，
/// 见模块文档（相对目标原样）。
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
    let path = match host_path(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match std::fs::read_link(&path) {
        Ok(target) => {
            let bytes = guest_name_of_host(&target);
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

/// `symlink(target, name)`：Windows 上只有**文件**符号链接（见模块文档第 6 条）。
///
/// 无特权（`ERROR_PRIVILEGE_NOT_HELD`）时按机器错误原样回报（`EPERM` 形状），由调用方处理。
#[allow(unsafe_code)]
extern "C" fn fs_symlink(
    _state: *mut c_void,
    target: *const u8,
    target_len: usize,
    name: *const u8,
    name_len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    let target_path = match symlink_target(target, target_len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    let name_path = match host_path(name, name_len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match std::os::windows::fs::symlink_file(&target_path, &name_path) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `chmod(name, mode)`：只映射"写位 → 只读属性"（`mode & 0o222 == 0` ⇒ readonly），其余位忽略。
#[allow(unsafe_code)]
extern "C" fn fs_chmod(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    mode: u32,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match host_path(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    let meta = match std::fs::metadata(&path) {
        Ok(meta) => meta,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            return CapStatus::Machine;
        }
    };
    let mut permissions = meta.permissions();
    permissions.set_readonly(mode & 0o222 == 0);
    match std::fs::set_permissions(&path, permissions) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `utime(name, atime, mtime)`：**秒级**、跟随叶子（`File::set_times`；见模块文档第 5 条）。
#[allow(unsafe_code)]
extern "C" fn fs_utime(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    atime: i64,
    mtime: i64,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match host_path(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match set_times_at(&path, time_of_secs(atime), time_of_secs(mtime), true) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// **`utime_ns(name, atime_ns, mtime_ns)`**（round 109 追加槽）：纳秒级、**跟随**叶子。
#[allow(unsafe_code)]
extern "C" fn fs_utime_ns(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    atime_ns: i64,
    mtime_ns: i64,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match host_path(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match set_times_at(&path, time_of_ns(atime_ns), time_of_ns(mtime_ns), true) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// **`lutime(name, atime_ns, mtime_ns)`**（round 109 追加槽）：**不跟随**最后一段符号链接
/// （打开链接自身：`FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT`）。
#[allow(unsafe_code)]
extern "C" fn fs_lutime(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    atime_ns: i64,
    mtime_ns: i64,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match host_path(name, len) {
        Ok(path) => path,
        Err(errno) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
    };
    match set_times_at(&path, time_of_ns(atime_ns), time_of_ns(mtime_ns), false) {
        Ok(()) => CapStatus::Ok,
        Err(error) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno_of(&error) };
            CapStatus::Machine
        }
    }
}

/// `stat_times(name)`（`CP-30` 的**追加槽**）：路径 ⇒ 时间戳纳秒。
#[allow(unsafe_code)]
extern "C" fn fs_stat_times(
    _state: *mut c_void,
    name: *const u8,
    len: usize,
    out: *mut FileTimes,
    errno_out: *mut i32,
) -> CapStatus {
    let path = match host_path(name, len) {
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
            unsafe { *out = times_of(&meta) };
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
    if capacity == 0 {
        // SAFETY: 出参由调用方提供。
        unsafe { *out_len = 0 };
        return CapStatus::Ok;
    }
    if out.is_null() {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EFAULT") };
        return CapStatus::Machine;
    }
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const WindowsFs) };
    let mut files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get_mut(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    // SAFETY: 缓冲区由调用方按 `capacity` 给出（上面已判空）。
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
    if len == 0 {
        // SAFETY: 出参由调用方提供。
        unsafe { *out_len = 0 };
        return CapStatus::Ok;
    }
    if buffer.is_null() {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EFAULT") };
        return CapStatus::Machine;
    }
    // **标准流句柄**（`1` ＝ `stdout`、`2` ＝ `stderr`）：不走句柄表（与 `fs_posix` 同口径；
    // `_io_module` 的 `STDOUT_HANDLE`／`STDERR_HANDLE` 直接当 provider 句柄用）。
    if handle.0 == 1 || handle.0 == 2 {
        // SAFETY: 缓冲区由调用方按 `len` 给出（上面已判空）。
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
                // SAFETY: 出参由调用方提供。
                unsafe { *errno_out = errno_of(&error) };
                CapStatus::Machine
            }
        };
    }
    // SAFETY: 同 `fs_open`。
    let provider = unsafe { &*(state as *const WindowsFs) };
    let mut files = provider.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get_mut(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = fallback_errno("EBADF") };
        return CapStatus::Machine;
    };
    // SAFETY: 缓冲区由调用方按 `len` 给出（上面已判空）。
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

/// `seek(file, offset, whence)`（与 `fs_posix` 同口径：未知 `anchor` 按 `END` 处理）。
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
    let provider = unsafe { &*(state as *const WindowsFs) };
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
    let provider = unsafe { &*(state as *const WindowsFs) };
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
    let provider = unsafe { &*(state as *const WindowsFs) };
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
    let provider = unsafe { &*(state as *const WindowsFs) };
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

/// `close(handle)`（`CP-17`：释放；**必须幂等**：已不在句柄表里也返回成功）。
#[allow(unsafe_code)]
extern "C" fn fs_close(
    state: *mut c_void,
    handle: Handle,
    _errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `fs_open`（只读地取句柄表）。
    let provider = unsafe { &*(state as *const WindowsFs) };
    let _removed = provider
        .files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&handle.0);
    let _removed_path = provider
        .dir_paths
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&handle.0);
    CapStatus::Ok
}
