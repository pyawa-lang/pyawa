//! `Instance` 的 net 能力域方法（`P3-29` 首片；与 `fs.rs`／`platform.rs` 同一手法）。
//!
//! 子模块看得见父模块的私有字段 ⇒ 不需要放宽任何字段可见性 ✓（`use super::*;` 即可）。
//!
//! 条文：**`CP-2`**（域未注册 ⇒ 调用时报"未实现"）、**`CP-3`**（槽位未实现 ⇒ 只有该操作未实现）、
//! **`CP-5`**（成功／机器错误／未实现三态，**禁止**用 `errno` 表示"未实现"）。
//!
//! **`fd` 语义（`CP-31`，如实记差距）**：本层把 provider 的不透明句柄**原样当 `fd`** 交回
//! （与 `fs.rs` 同一条口径）。`CP-31` 要求的"实例能力表统一编号（分配/释放/复制在本层）"
//! **尚未落地**——`fs` 与 `net` 的号段各自由 provider 管理 ⇒ 跨域 `fd` 不可区分，
//! `select`／`os.dup2` 那一族的 `T-CP-3` 前件还不成立（建议专项收口，不在本片）。

use super::*;

impl Instance {
    /// **`net` 域的形状视图**（`SPEC-capabilities.md` §9.2）：把宿主注册的不透明指针按
    /// [`pyawa_capabilities::net::CpNetVtable`] 解释。`None` ＝ 该域未提供（`CP-2`）。
    ///
    /// # Safety
    ///
    /// 注册方（宿主）必须保证：该指针指向一个**在实例存活期间有效**的 `CpNetVtable`
    /// （`AB-16`／`AB-17` 的借用纪律由调用方遵守）。
    pub fn net_vtable(&self) -> Option<pyawa_capabilities::net::CpNetVtable> {
        let pointer = self.capability(pyawa_capabilities::DOMAIN_NET)?;
        // SAFETY: 见函数文档——注册方保证指针有效且布局正确。
        Some(unsafe { *pointer.cast::<pyawa_capabilities::net::CpNetVtable>() })
    }

    /// **经 `net` 域造一对 socket**（`CP-38` 的 `socketpair` 槽）。
    ///
    /// 成功返回**两个**不透明句柄（调用方当 `fd` 用）；`CP-2`／`CP-3`／`CP-5` 三态在这里落成结果。
    pub fn net_socketpair(
        &self,
        family: i32,
        kind: i32,
        protocol: i32,
    ) -> Result<(u64, u64), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let socketpair = table.socketpair.ok_or(CapabilityCallError::NotImplemented)?;
        let mut a = pyawa_capabilities::net::SocketHandle(0);
        let mut b = pyawa_capabilities::net::SocketHandle(0);
        let mut errno = 0i32;
        match socketpair(
            table.state,
            family,
            kind,
            protocol,
            &mut a,
            &mut b,
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok((a.0, b.0)),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `net` 域申请一枚 socket**（`socket(family, type, proto)` 槽）。
    pub fn net_socket(
        &self,
        family: i32,
        kind: i32,
        protocol: i32,
    ) -> Result<u64, CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let socket = table.socket.ok_or(CapabilityCallError::NotImplemented)?;
        let mut handle = pyawa_capabilities::net::SocketHandle(0);
        let mut errno = 0i32;
        match socket(table.state, family, kind, protocol, &mut handle, &mut errno) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(handle.0),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `net` 域发送**（`send(sock, buf)` 槽）：返回实际发出的字节数。
    pub fn net_send(&self, handle: u64, bytes: &[u8]) -> Result<usize, CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let send = table.send.ok_or(CapabilityCallError::NotImplemented)?;
        let mut sent = 0usize;
        let mut errno = 0i32;
        match send(
            table.state,
            pyawa_capabilities::net::SocketHandle(handle),
            bytes.as_ptr(),
            bytes.len(),
            &mut sent,
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(sent),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `net` 域接收**（`recv(sock, n)` 槽）：把 `buffer` 填到能收的为止，返回实际字节数。
    pub fn net_recv(&self, handle: u64, buffer: &mut [u8]) -> Result<usize, CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let recv = table.recv.ok_or(CapabilityCallError::NotImplemented)?;
        let mut received = 0usize;
        let mut errno = 0i32;
        match recv(
            table.state,
            pyawa_capabilities::net::SocketHandle(handle),
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut received,
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(received),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `net` 域关闭句柄**（`CP-17`：幂等）。
    pub fn net_close(&self, handle: u64) -> Result<(), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let close = table.close.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match close(
            table.state,
            pyawa_capabilities::net::SocketHandle(handle),
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(()),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }
}
