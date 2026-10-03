//! `fs` 域的形状 —— `SPEC-capabilities.md` §9.1（`CP-`）。
//!
//! 本文件**只含类型与函数指针**（`CP-12`），真实机器实现在 `pyawa-runtime`（`DESIGN.md` §7 的平台集中点）。
//!
//! 遵循的条文：
//! - **`CP-2`**：vtable 指针为 `null` ⇒ **整域**未实现（禁止在建实例时拒绝）。
//! - **`CP-3`**：槽位为 `None` ⇒ 只有该操作未实现，同域其余槽位不受影响。
//! - **`CP-5`**：每次调用**必须**返回三种结果之一 —— 成功／**机器错误**／**未实现**；
//!   **禁止**把"未实现"编码成某个 `errno`，也**禁止**两者共用一条通道（`AB-22` 同理）。
//! - **`CP-30`**：vtable **必须**带版本／尺寸字段（字段编码与校验时机归 `SPEC-c-abi.md`）。
//! - **`AB-14`**：跨边界一律**不透明句柄** —— 本文件只给 [`Handle`] 包装，不暴露内部布局。
//! - **`CP-21`**：`open` 的**相对名字由 VM 解析器沿根能力逐段解析**；provider 拿到的是解析后的形态
//!   （`posix`／`_io` 落地时再钉死具体载荷，本文件先按"名字字节串"记账）。
//!
//! **不在本文件决定**：跨 ABI 的线格式与注册编码（`SPEC-c-abi.md` §6／§7／§10）、
//! 逐函数的错误信息生命周期（`AB-23`）。

use core::ffi::c_void;

/// 一次能力调用的**三态**结果（`CP-5`）。
///
/// `Machine` 与 `Unimplemented` 是**两条不同的通道**：前者是"真实机器语义的错误"（可还原 `errno`，
/// `CP-6`／`CP-23`），后者是"该嵌入没提供这个槽位"（`AB-22` 要求两者可区分）。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapStatus {
    /// 成功；结果经 `out` 参数带回。
    Ok = 0,
    /// 机器错误；`errno` 经 `errno_out`（或等价出参）带回。
    Machine = 1,
    /// 该槽位未实现（`CP-5`）—— **禁止**用某个 `errno` 表示。
    Unimplemented = 2,
}

/// 不透明句柄（`AB-14`：**禁止**暴露头部／类型对象指针／任何内部布局）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Handle(pub u64);

/// 文件元信息的**字段子集**（字段集归属 `SPEC-capabilities.md` §2）。
///
/// 只放本层已经要用的字段；§2 的完整字段集随 `posix`／`_io` 落地时补（补法：追加字段，不改既有语义）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileInfo {
    pub size: u64,
    pub mode: u32,
    pub is_dir: bool,
    /// `Device`／`INode` 一类留给 `CP-23` 的 `/proc`／`/dev`／`/sys` 语义，本层不猜。
    pub dev: u64,
    pub ino: u64,
}

/// `open(name, flags, mode)`：`O_*` 标志位（`SPEC-capabilities.md` §9.1）。
///
/// 数值**未**在本文件钉死（那是 `posix` 的映射面）；先给名字，避免"发明数字"。
pub mod open_flag {
    /// 只读。
    pub const RDONLY: i32 = 0;
    /// 只写。
    pub const WRONLY: i32 = 1;
    /// 读写。
    pub const RDWR: i32 = 2;
    /// 追加。
    pub const APPEND: i32 = 8;
    /// 截断。
    pub const TRUNC: i32 = 16;
    /// 创建。
    pub const CREAT: i32 = 32;
}

/// `seek` 的 `whence`（`SPEC-capabilities.md` §9.1）。
pub mod whence {
    pub const SET: i32 = 0;
    pub const CUR: i32 = 1;
    pub const END: i32 = 2;
}

/// `fs` 域的 vtable（`SPEC-capabilities.md` §3 的 `CpFsVtable`）。
///
/// 槽位名与语义逐条对应 §9.1；`None` 即"该操作未实现"（`CP-3`）。
///
/// `state` 是 **provider 的实例状态**（不透明指针，`AB-32` 的精神：接口层只存不解释）；
/// 每个槽位把它作为第一个参数收回，于是"函数指针"仍能满足 `CP-12`（本 crate 只含类型与函数指针）。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CpFsVtable {
    /// `CP-30`：版本／尺寸字段（编码与校验时机归 `SPEC-c-abi.md`）。
    pub version: u32,
    /// 本结构体的字节数（`AB-43` 的有界读取精神）。
    pub size: u32,
    /// provider 实例状态（不透明）。
    pub state: *mut c_void,

    /// `open(name, flags, mode)` ⇒ 文件句柄。
    pub open:
        Option<extern "C" fn(*mut c_void, *const u8, usize, i32, u32, *mut Handle, *mut i32) -> CapStatus>,
    /// `stat(name)` ⇒ 元信息。
    pub stat: Option<extern "C" fn(*mut c_void, *const u8, usize, *mut FileInfo, *mut i32) -> CapStatus>,
    /// `fstat(file)` ⇒ 元信息。
    pub fstat: Option<extern "C" fn(*mut c_void, Handle, *mut FileInfo, *mut i32) -> CapStatus>,
    /// `listdir(dir)` ⇒ 名字序列（长度经 `out_len` 带回；缓冲由调用方给）。
    pub listdir: Option<
        extern "C" fn(*mut c_void, Handle, *mut u8, usize, *mut usize, *mut i32) -> CapStatus,
    >,
    /// `mkdir(name, mode)`。
    pub mkdir: Option<extern "C" fn(*mut c_void, *const u8, usize, u32, *mut i32) -> CapStatus>,
    /// `rmdir(name)`。
    pub rmdir: Option<extern "C" fn(*mut c_void, *const u8, usize, *mut i32) -> CapStatus>,
    /// `unlink(name)`。
    pub unlink: Option<extern "C" fn(*mut c_void, *const u8, usize, *mut i32) -> CapStatus>,
    /// `rename(a, b)`。
    pub rename: Option<
        extern "C" fn(*mut c_void, *const u8, usize, *const u8, usize, *mut i32) -> CapStatus,
    >,
    /// `readlink(name)`（目标经 `out` 带回）。
    pub readlink: Option<
        extern "C" fn(*mut c_void, *const u8, usize, *mut u8, usize, *mut usize, *mut i32) -> CapStatus,
    >,
    /// `symlink(target, name)`。
    pub symlink: Option<
        extern "C" fn(*mut c_void, *const u8, usize, *const u8, usize, *mut i32) -> CapStatus,
    >,
    /// `chmod(name, mode)`。
    pub chmod: Option<extern "C" fn(*mut c_void, *const u8, usize, u32, *mut i32) -> CapStatus>,
    /// `utime(name, atime, mtime)`。
    pub utime: Option<
        extern "C" fn(*mut c_void, *const u8, usize, i64, i64, *mut i32) -> CapStatus,
    >,
    /// `read(file, n)` ⇒ 实际读到的字节数经 `out_len` 带回。
    pub read: Option<
        extern "C" fn(*mut c_void, Handle, *mut u8, usize, usize, *mut usize, *mut i32) -> CapStatus,
    >,
    /// `write(file, buf)` ⇒ 实际写入的字节数经 `out_len` 带回。
    pub write: Option<
        extern "C" fn(*mut c_void, Handle, *const u8, usize, *mut usize, *mut i32) -> CapStatus,
    >,
    /// `seek(file, offset, whence)` ⇒ 新位置。
    pub seek: Option<extern "C" fn(*mut c_void, Handle, i64, i32, *mut u64, *mut i32) -> CapStatus>,
    /// `tell(file)` ⇒ 当前位置。
    pub tell: Option<extern "C" fn(*mut c_void, Handle, *mut u64, *mut i32) -> CapStatus>,
    /// `truncate(file, size)`。
    pub truncate: Option<extern "C" fn(*mut c_void, Handle, u64, *mut i32) -> CapStatus>,
    /// `fsync(file)`。
    pub fsync: Option<extern "C" fn(*mut c_void, Handle, *mut i32) -> CapStatus>,
    /// `close(handle)`（`CP-17`：释放）。
    pub close: Option<extern "C" fn(*mut c_void, Handle, *mut i32) -> CapStatus>,
}

impl CpFsVtable {
    /// 一个**整域未实现**的 vtable（所有槽位为 `None`；`CP-2`／`CP-3`）。
    pub const UNIMPLEMENTED: Self = Self {
        version: 0,
        size: core::mem::size_of::<Self>() as u32,
        state: core::ptr::null_mut(),
        open: None,
        stat: None,
        fstat: None,
        listdir: None,
        mkdir: None,
        rmdir: None,
        unlink: None,
        rename: None,
        readlink: None,
        symlink: None,
        chmod: None,
        utime: None,
        read: None,
        write: None,
        seek: None,
        tell: None,
        truncate: None,
        fsync: None,
        close: None,
    };
}

/// `fs` 域的异步分类（`CP-25`／`AB-34`：按域二值，**缺失即注册失败**）——
/// `SPEC-capabilities.md` §9.1 建议"上表全部可异步化"，故为 `PA_ASYNC_OK` 的取值。
pub const ASYNC_CLASSIFICATION: i32 = 0;
