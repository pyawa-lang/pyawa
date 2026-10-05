//! `Instance` 的 fs 能力域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ 不需要放宽任何字段可见性 ✓（`use super::*;` 即可）。

use super::*;

impl Instance {
    /// **经 `fs` 域写一段字节**（`CP-2`／`CP-3`／`CP-5` 的三态在这里落成结果 ✓）。
    ///
    /// 调用方（stdlib 的 `_io`）只管文本层与编码；**平台**在提供者那边 ✓（`CX-4`）。
    pub fn fs_write(&self, handle: u64, bytes: &[u8]) -> Result<usize, CapabilityCallError> {
        let Some(table) = self.fs_vtable() else {
            return Err(CapabilityCallError::NotRegistered);
        };
        let Some(write) = table.write else {
            return Err(CapabilityCallError::NotImplemented);
        };
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

    /// **经 `fs` 域打开文件**（`CP-2`／`CP-3`／`CP-5` 三态同上 ✓）。`path` 按**字节原样**交给提供者 ✓
    /// （平台侧怎么解释路径不归本层管 ✓）。
    /// **经 `fs` 域取元信息** ✓（第 204 轮）：vtable 里**早就有** `stat` 槽 ✓（`fs.rs:98` ✓）、
    /// provider 也早实现 ✓（`fs_posix.rs:40` ✓）—— 缺的只是这一层包装 ✓。
    pub fn fs_stat(&self, path: &[u8]) -> Result<pyawa_capabilities::fs::FileInfo, CapabilityCallError> {
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

    pub fn fs_open(&self, path: &[u8], flags: i32, mode: u32) -> Result<u64, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let open = table.open.ok_or(CapabilityCallError::NotImplemented)?;
        let mut handle = pyawa_capabilities::fs::Handle(0);
        let mut errno = 0i32;
        match open(table.state, path.as_ptr(), path.len(), flags, mode, &mut handle, &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(handle.0),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域读**（把 `buffer` 填满到能读的为止 ✓）。返回实际读到的字节数 ✓。
    pub fn fs_read(&self, handle: u64, buffer: &mut [u8]) -> Result<usize, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let read = table.read.ok_or(CapabilityCallError::NotImplemented)?;
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

    /// **经 `fs` 域关闭句柄**。
    pub fn fs_close(&self, handle: u64) -> Result<(), CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let close = table.close.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match close(table.state, pyawa_capabilities::fs::Handle(handle), &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(()),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    pub fn fs_vtable(&self) -> Option<pyawa_capabilities::fs::CpFsVtable> {
        let pointer = self.capability(pyawa_capabilities::DOMAIN_FS)?;
        // SAFETY: 见函数文档——注册方保证指针有效且布局正确。
        Some(unsafe { *pointer.cast::<pyawa_capabilities::fs::CpFsVtable>() })
    }
}
