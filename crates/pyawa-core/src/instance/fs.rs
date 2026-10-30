//! `Instance` 的 `fs` 能力域方法（VM 侧 POSIX 适配层）。
//!
//! 调用方（stdlib 的 `_io`／`posix`）只管文本层与编码；**平台**在提供者那边（`CX-4`）。
//!
//! # `fd` 语义：`CP-31` 的实例能力表
//!
//! 本层收发的 `fd` **一律是实例能力表的下标**（`CP-14`：Python 只见 `int`），不是 provider 的
//! 不透明句柄：
//!
//! ```text
//! fs_open(path)  → VM 路径解析（CP-21/CP-32）→ provider.open(guest 绝对名) → provider 句柄
//!                → 能力表分配 fd → 返回 fd(int)
//! fs_read(fd)    → 能力表查 fd → (fs 域, provider 句柄) → provider.read(句柄)
//! fs_close(fd)   → 能力表释放表项 → 别名计数归零时才调 provider.close（`CP-17`）
//! ```
//!
//! 名字类方法（`open`／`stat`／`readlink`／`unlink`／`rmdir`）一律先经 `instance/path.rs` 的
//! VM 解析器（`CP-21`／`CP-32`：`..`／符号链接夹根在 VM，provider 只做根映射）。
//!
//! **标准流（0／1／2）不占表槽**（沿用 `_io` 的 `STDOUT_HANDLE`／`STDERR_HANDLE` 约定）：
//! 这些 `fd` 直接当 provider 句柄透传（`PosixFs` 的 `write` 特判句柄 1／2）。
//!
//! 语义条文：**`CP-2`**（域未注册 ⇒ 调用时报未实现）、**`CP-3`**（槽位未实现 ⇒ 只有该操作未实现）、
//! **`CP-5`**（成功／机器错误／未实现三态，**禁止**用 `errno` 表示未实现）、
//! **`CP-18`**（已关闭／非法 `fd` ⇒ `EBADF` 形状的机器错误，**不是** UB）。

use super::capability_table::FIRST_DYNAMIC_FD;
use super::*;

impl Instance {
    /// `fd` → **provider 句柄**：`fd < 3` 是标准流（表外透传）；否则必须是 `fs` 域的表项。
    ///
    /// 表里没有／域不符 ⇒ `EBADF`（`CP-18`）。
    fn fs_provider_handle(&self, fd: u64) -> Result<u64, CapabilityCallError> {
        if fd < FIRST_DYNAMIC_FD {
            return Ok(fd);
        }
        self.capability_provider_handle(fd, pyawa_capabilities::DOMAIN_FS)
    }

    /// **经 `fs` 域写一段字节**（`CP-2`／`CP-3`／`CP-5` 的三态在这里落成结果）。
    pub fn fs_write(&self, fd: u64, bytes: &[u8]) -> Result<usize, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let write = table.write.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.fs_provider_handle(fd)?;
        let mut written = 0usize;
        let mut errno = 0i32;
        match write(
            table.state,
            pyawa_capabilities::fs::Handle(handle),
            bytes.as_ptr(),
            bytes.len(),
            &mut written,
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(written),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => {
                Err(CapabilityCallError::Machine(errno))
            }
        }
    }

    /// **经 `fs` 域打开文件**：名字先经 **VM 路径解析器**（`CP-21`／`CP-32`），再交给 provider；
    /// provider 句柄 → **能力表分配的新 `fd`**（`CP-31`）。
    pub fn fs_open(&self, path: &[u8], flags: i32, mode: u32) -> Result<u64, CapabilityCallError> {
        let resolved = self.resolve_path(path)?;
        self.fs_open_raw(&resolved, flags, mode)
    }

    /// `fs_open` 的 provider 直达面（**名字已解析**；只允许本层的适配方法调用）。
    fn fs_open_raw(&self, path: &[u8], flags: i32, mode: u32) -> Result<u64, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let open = table.open.ok_or(CapabilityCallError::NotImplemented)?;
        let mut handle = pyawa_capabilities::fs::Handle(0);
        let mut errno = 0i32;
        match open(table.state, path.as_ptr(), path.len(), flags, mode, &mut handle, &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(self.capability_register(
                pyawa_capabilities::DOMAIN_FS,
                handle.0,
            )),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域取元信息**（`stat(name)`；按名字，不经过能力表；名字先经 VM 解析器）。
    pub fn fs_stat(&self, path: &[u8]) -> Result<pyawa_capabilities::fs::FileInfo, CapabilityCallError> {
        let resolved = self.resolve_path(path)?;
        self.fs_stat_raw(&resolved)
    }

    /// `fs_stat` 的 provider 直达面（**名字已解析**）。
    fn fs_stat_raw(
        &self,
        path: &[u8],
    ) -> Result<pyawa_capabilities::fs::FileInfo, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let stat = table.stat.ok_or(CapabilityCallError::NotImplemented)?;
        let mut info = pyawa_capabilities::fs::FileInfo {
            size: 0,
            mode: 0,
            is_dir: false,
            dev: 0,
            ino: 0,
        };
        let mut errno = 0i32;
        match stat(table.state, path.as_ptr(), path.len(), &mut info, &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(info),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域取文件时间戳**（`stat_times(name)`；`CP-30` 的**追加槽**，`AB-41` 次版本 2）。
    ///
    /// 名字先经 VM 解析器（与 [`Instance::fs_stat`] 同一条口径）；槽缺失（1.1 的旧宿主）或
    /// provider 给 `None` ⇒ `NotImplemented`（`CP-3`：只有这个操作未实现，调用方据此"如实缺名"）。
    pub fn fs_stat_times(
        &self,
        path: &[u8],
    ) -> Result<pyawa_capabilities::fs::FileTimes, CapabilityCallError> {
        let resolved = self.resolve_path(path)?;
        self.fs_stat_times_raw(&resolved)
    }

    /// `fs_stat_times` 的 provider 直达面（**名字已解析**）。
    fn fs_stat_times_raw(
        &self,
        path: &[u8],
    ) -> Result<pyawa_capabilities::fs::FileTimes, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let stat_times = table.stat_times.ok_or(CapabilityCallError::NotImplemented)?;
        let mut times = pyawa_capabilities::fs::FileTimes::default();
        let mut errno = 0i32;
        match stat_times(table.state, path.as_ptr(), path.len(), &mut times, &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(times),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域读符号链接目标**（`readlink`；按名字，不经过能力表）。
    ///
    /// 名字先经 VM 解析器（**不跟随最后一段** —— `readlink` 要的就是链接本身）。
    pub fn fs_readlink(&self, path: &[u8]) -> Result<Vec<u8>, CapabilityCallError> {
        let resolved = self.resolve_path_parent(path)?;
        self.fs_readlink_raw(&resolved)
    }

    /// `readlink` 的 provider 直达面（**名字已解析**）—— VM 解析器自己也要用它探测符号链接。
    ///
    /// **出参两段式**（与 `listdir` 同一约定）：`out_len` 回的是**目标的总字节数** ⇒ provider 只写
    /// `min(总长, 容量)`，调用方看到总长超容量就按新容量重试（`runtime/src/fs_posix.rs` 同款契约）。
    pub(crate) fn fs_readlink_raw(&self, path: &[u8]) -> Result<Vec<u8>, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let readlink = table.readlink.ok_or(CapabilityCallError::NotImplemented)?;
        // 初始容量取 Linux 的 `PATH_MAX` 一带（超长目标按 provider 报的总长再要一次）。
        let mut capacity = 4096usize;
        loop {
            let mut buffer: Vec<u8> = vec![0u8; capacity];
            let mut needed = 0usize;
            let mut errno = 0i32;
            match readlink(
                table.state,
                path.as_ptr(),
                path.len(),
                buffer.as_mut_ptr(),
                capacity,
                &mut needed,
                &mut errno,
            ) {
                pyawa_capabilities::fs::CapStatus::Ok => {
                    if needed <= capacity {
                        buffer.truncate(needed);
                        return Ok(buffer);
                    }
                    capacity = needed;
                }
                pyawa_capabilities::fs::CapStatus::Unimplemented => {
                    return Err(CapabilityCallError::NotImplemented)
                }
                pyawa_capabilities::fs::CapStatus::Machine => {
                    return Err(CapabilityCallError::Machine(errno))
                }
            }
        }
    }

    /// **经 `fs` 域读**：`fd` 先经能力表换成 provider 句柄；返回实际读到的字节数。
    pub fn fs_read(&self, fd: u64, buffer: &mut [u8]) -> Result<usize, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let read = table.read.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.fs_provider_handle(fd)?;
        let mut read_bytes = 0usize;
        let mut errno = 0i32;
        match read(
            table.state,
            pyawa_capabilities::fs::Handle(handle),
            buffer.as_mut_ptr(),
            buffer.len(),
            buffer.len(),
            &mut read_bytes,
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(read_bytes),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域定位**（`seek(fd, offset, whence)`）。
    pub fn fs_seek(&self, fd: u64, offset: i64, whence: i32) -> Result<u64, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let seek = table.seek.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.fs_provider_handle(fd)?;
        let mut position = 0u64;
        let mut errno = 0i32;
        match seek(
            table.state,
            pyawa_capabilities::fs::Handle(handle),
            offset,
            whence,
            &mut position,
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(position),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域取当前位置**（与 [`Instance::fs_seek`] 同一条口径）。
    pub fn fs_tell(&self, fd: u64) -> Result<u64, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let tell = table.tell.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.fs_provider_handle(fd)?;
        let mut position = 0u64;
        let mut errno = 0i32;
        match tell(
            table.state,
            pyawa_capabilities::fs::Handle(handle),
            &mut position,
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(position),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域关闭 fd**（`CP-17`：幂等）。
    ///
    /// 能力表释放表项；同一 provider 句柄还有别名（`dup`／`dup2`）时不调 provider `close`
    /// —— **计数归零**才真释放。已关／表外 `fd` ⇒ 成功（幂等），不是 `EBADF`。
    pub fn fs_close(&self, fd: u64) -> Result<(), CapabilityCallError> {
        // 域未注册 ⇒ 与其它 fs 方法同一口径（`CP-2`：调用时报未实现）。
        let _ = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        if fd < FIRST_DYNAMIC_FD {
            // 标准流不占表槽：`close` 是幂等空操作（`_io` 的约定）。
            return Ok(());
        }
        self.capability_release(fd)
    }

    /// **经 `fs` 域列目录**（P3-38 S3b-2）：返回**原始字节名字**列表（顺序＝ provider 顺序）。
    ///
    /// `listdir` 槽的 ABI 收**目录句柄** ⇒ 这里先经 `open` 槽打开目录，再按两段式出参读
    /// （`NUL` 分隔；`out_len` 回总字节数，容量不够重试）。名字**不做 UTF-8 假设** ✓
    /// —— 非 UTF-8 文件名由调用方按 PEP 383 的 `surrogateescape` 解成 str。
    pub fn fs_listdir(&self, path: &[u8]) -> Result<Vec<Vec<u8>>, CapabilityCallError> {
        let resolved = self.resolve_path(path)?;
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let listdir = table.listdir.ok_or(CapabilityCallError::NotImplemented)?;
        // `O_RDONLY` ＝ 0（`open_flag` 的域位约定同 `posix` 面）。
        let fd = self.fs_open_raw(&resolved, 0, 0)?;
        let provider_handle = self.fs_provider_handle(fd)?;
        let outcome = (|| {
            let mut capacity = 4096usize;
            loop {
                let mut buffer: Vec<u8> = vec![0u8; capacity];
                let mut needed = 0usize;
                let mut errno = 0i32;
                match listdir(
                    table.state,
                    pyawa_capabilities::fs::Handle(provider_handle),
                    buffer.as_mut_ptr(),
                    capacity,
                    &mut needed,
                    &mut errno,
                ) {
                    pyawa_capabilities::fs::CapStatus::Ok => {
                        if needed <= capacity {
                            buffer.truncate(needed);
                            return Ok(buffer
                                .split(|byte| *byte == 0)
                                .filter(|name| !name.is_empty())
                                .map(|name| name.to_vec())
                                .collect());
                        }
                        capacity = needed;
                    }
                    pyawa_capabilities::fs::CapStatus::Unimplemented => {
                        return Err(CapabilityCallError::NotImplemented)
                    }
                    pyawa_capabilities::fs::CapStatus::Machine => {
                        return Err(CapabilityCallError::Machine(errno))
                    }
                }
            }
        })();
        // 目录句柄用完即还（与 `_io` 的 `open` 同口径：能力表引用计数归零才调 provider close）。
        let _ = self.fs_close(fd);
        outcome
    }

    /// **经 `fs` 域重命名**（P3-38 S3b-2；两个名字都按字节、先经 VM 解析器）。
    pub fn fs_rename(&self, from: &[u8], to: &[u8]) -> Result<(), CapabilityCallError> {
        let resolved_from = self.resolve_path_parent(from)?;
        let resolved_to = self.resolve_path_parent(to)?;
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let rename = table.rename.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match rename(
            table.state,
            resolved_from.as_ptr(),
            resolved_from.len(),
            resolved_to.as_ptr(),
            resolved_to.len(),
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(()),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域建符号链接**（P3-38 S3b-2）：`target` 原样（链接内容是**数据**，不做解析），
    /// `name` 按 VM 解析器映射（只解析父级，末段是新建的链接本身）。
    pub fn fs_symlink(&self, target: &[u8], name: &[u8]) -> Result<(), CapabilityCallError> {
        let resolved_name = self.resolve_path_parent(name)?;
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let symlink = table.symlink.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match symlink(
            table.state,
            target.as_ptr(),
            target.len(),
            resolved_name.as_ptr(),
            resolved_name.len(),
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(()),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **provider `close` 槽的直达入口**（能力表／实例销毁兜底用；吃 provider 句柄，不吃 `fd`）。
    pub(crate) fn fs_provider_close(&self, provider_handle: u64) -> Result<(), CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let close = table.close.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match close(
            table.state,
            pyawa_capabilities::fs::Handle(provider_handle),
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(()),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域删除文件**（按名字，不经过能力表；名字先经 VM 解析器，**不跟随最后一段**）。
    pub fn fs_unlink(&self, path: &[u8]) -> Result<(), CapabilityCallError> {
        let resolved = self.resolve_path_parent(path)?;
        self.fs_unlink_raw(&resolved)
    }

    /// `fs_unlink` 的 provider 直达面（**名字已解析**）。
    fn fs_unlink_raw(&self, path: &[u8]) -> Result<(), CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let unlink = table.unlink.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match unlink(table.state, path.as_ptr(), path.len(), &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(()),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域删除目录**（按名字；与 [`Instance::fs_unlink`] 同一条口径）。
    pub fn fs_rmdir(&self, path: &[u8]) -> Result<(), CapabilityCallError> {
        let resolved = self.resolve_path_parent(path)?;
        self.fs_rmdir_raw(&resolved)
    }

    /// `fs_rmdir` 的 provider 直达面（**名字已解析**）。
    fn fs_rmdir_raw(&self, path: &[u8]) -> Result<(), CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let rmdir = table.rmdir.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match rmdir(table.state, path.as_ptr(), path.len(), &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(()),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// `fs` 域的形状视图（指针为 `null` ⇒ 该域未提供，`CP-2`）。
    ///
    /// # Safety
    ///
    /// 注册方（宿主）保证该指针指向一个在实例存活期间有效的 `CpFsVtable`
    /// （`AB-16`／`AB-17` 的借用纪律由调用方遵守）。
    pub fn fs_vtable(&self) -> Option<pyawa_capabilities::fs::CpFsVtable> {
        let pointer = self.capability(pyawa_capabilities::DOMAIN_FS)?;
        // SAFETY: 见函数文档——注册方保证指针有效且布局正确。
        Some(unsafe { *pointer.cast::<pyawa_capabilities::fs::CpFsVtable>() })
    }
}
