//! `Instance` 的 `net` 能力域方法（`P3-29`；与 `fs.rs` 同一手法）。
//!
//! 条文：**`CP-2`**（域未注册 ⇒ 调用时报未实现）、**`CP-3`**（槽位未实现 ⇒ 只有该操作未实现）、
//! **`CP-5`**（成功／机器错误／未实现三态，**禁止**用 `errno` 表示未实现）。
//!
//! # `fd` 语义：`CP-31` 的实例能力表（与 `fs` 域**同一张**表）
//!
//! `net_socketpair`／`net_socket` 返回的是**实例能力表分配的 `fd`**；`net_send`／`net_recv`／
//! `net_close` 收 `fd`、内部查表换成 provider 的不透明 `SocketHandle` 再调 vtable。
//! 于是 `fs` 与 `net` 的 `fd` 落在**同一个编号空间**（不再各自从 3 起 ⇒ 同一个 `int` 不会
//! 同时指两个能力），`select`／`os.dup2` 的 `T-CP-3` 前件因此成立。

use super::capability_table::FIRST_DYNAMIC_FD;
use super::*;

impl Instance {
    /// `fd` → **provider 的 socket 句柄**：必须是 `net` 域的表项（`CP-18`：否则 `EBADF`）。
    fn net_provider_handle(&self, fd: u64) -> Result<u64, CapabilityCallError> {
        if fd < FIRST_DYNAMIC_FD {
            // 0／1／2 是标准流（`fs` 域约定），不是 socket。
            return Err(CapabilityCallError::Machine(
                self.platform_constant("EBADF").map_or(9, |value| value as i32),
            ));
        }
        self.capability_provider_handle(fd, pyawa_capabilities::DOMAIN_NET)
    }

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
    /// 成功返回**两个**表分配的 `fd`（`CP-31`）；三态在这里落成结果。
    pub fn net_socketpair(
        &self,
        family: i32,
        kind: i32,
        protocol: i32,
    ) -> Result<(u64, u64), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let socketpair = table.socketpair.ok_or(CapabilityCallError::NotImplemented)?;
        let mut first = pyawa_capabilities::net::SocketHandle(0);
        let mut second = pyawa_capabilities::net::SocketHandle(0);
        let mut errno = 0i32;
        match socketpair(
            table.state,
            family,
            kind,
            protocol,
            &mut first,
            &mut second,
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => {
                let a = self.capability_register(pyawa_capabilities::DOMAIN_NET, first.0);
                let b = self.capability_register(pyawa_capabilities::DOMAIN_NET, second.0);
                Ok((a, b))
            }
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `net` 域申请一枚 socket**（`socket(family, type, proto)` 槽）⇒ 表分配的 `fd`。
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
            pyawa_capabilities::net::CapStatus::Ok => Ok(self
                .capability_register(pyawa_capabilities::DOMAIN_NET, handle.0)),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `net` 域发送**（`send(fd, buf)` 槽）：返回实际发出的字节数。
    pub fn net_send(&self, fd: u64, bytes: &[u8]) -> Result<usize, CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let send = table.send.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
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

    /// **经 `net` 域接收**（`recv(fd, n)` 槽）：把 `buffer` 填到能收的为止，返回实际字节数。
    pub fn net_recv(&self, fd: u64, buffer: &mut [u8]) -> Result<usize, CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let recv = table.recv.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
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

    /// **`bind(sock, family, addr)`**（`CP-39` 的 v1 线格式：`AF_INET` 是 4B 地址＋2B 端口）。
    pub fn net_bind(
        &self,
        fd: u64,
        family: i32,
        addr: &[u8],
    ) -> Result<(), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let bind = table.bind.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
        let mut errno = 0i32;
        match bind(
            table.state,
            pyawa_capabilities::net::SocketHandle(handle),
            family,
            addr.as_ptr(),
            addr.len(),
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(()),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **`connect(sock, family, addr)`**。
    pub fn net_connect(
        &self,
        fd: u64,
        family: i32,
        addr: &[u8],
    ) -> Result<(), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let connect = table.connect.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
        let mut errno = 0i32;
        match connect(
            table.state,
            pyawa_capabilities::net::SocketHandle(handle),
            family,
            addr.as_ptr(),
            addr.len(),
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(()),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **`listen(sock, backlog)`**。
    pub fn net_listen(&self, fd: u64, backlog: i32) -> Result<(), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let listen = table.listen.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
        let mut errno = 0i32;
        match listen(
            table.state,
            pyawa_capabilities::net::SocketHandle(handle),
            backlog,
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(()),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **`accept(sock)`** ⇒ `(新 fd, 地址族, `CP-39` 线格式地址)`（两段式出参：总长超容量就扩缓冲重试）。
    pub fn net_accept(
        &self,
        fd: u64,
    ) -> Result<(u64, i32, Vec<u8>), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let accept = table.accept.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
        let mut capacity = 64usize;
        loop {
            let mut new_handle = pyawa_capabilities::net::SocketHandle(0);
            let mut family = 0i32;
            let mut buffer = vec![0u8; capacity];
            let mut total = 0usize;
            let mut errno = 0i32;
            match accept(
                table.state,
                pyawa_capabilities::net::SocketHandle(handle),
                &mut new_handle,
                &mut family,
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut total,
                &mut errno,
            ) {
                pyawa_capabilities::net::CapStatus::Ok => {
                    if total > buffer.len() {
                        capacity = total;
                        continue;
                    }
                    buffer.truncate(total);
                    let allocated = self
                        .capability_register(pyawa_capabilities::DOMAIN_NET, new_handle.0);
                    return Ok((allocated, family, buffer));
                }
                pyawa_capabilities::net::CapStatus::Unimplemented => {
                    return Err(CapabilityCallError::NotImplemented)
                }
                pyawa_capabilities::net::CapStatus::Machine => {
                    return Err(CapabilityCallError::Machine(errno))
                }
            }
        }
    }

    /// `getsockname`／`getpeername` 的共同实现（两段式出参）。
    fn net_socket_address(
        &self,
        fd: u64,
        peer: bool,
    ) -> Result<(i32, Vec<u8>), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let slot = if peer { table.getpeername } else { table.getsockname };
        let resolve = slot.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
        let mut capacity = 64usize;
        loop {
            let mut family = 0i32;
            let mut buffer = vec![0u8; capacity];
            let mut total = 0usize;
            let mut errno = 0i32;
            match resolve(
                table.state,
                pyawa_capabilities::net::SocketHandle(handle),
                &mut family,
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut total,
                &mut errno,
            ) {
                pyawa_capabilities::net::CapStatus::Ok => {
                    if total > buffer.len() {
                        capacity = total;
                        continue;
                    }
                    buffer.truncate(total);
                    return Ok((family, buffer));
                }
                pyawa_capabilities::net::CapStatus::Unimplemented => {
                    return Err(CapabilityCallError::NotImplemented)
                }
                pyawa_capabilities::net::CapStatus::Machine => {
                    return Err(CapabilityCallError::Machine(errno))
                }
            }
        }
    }

    /// **`getsockname(sock)`** ⇒ `(地址族, `CP-39` 线格式地址)`。
    pub fn net_getsockname(&self, fd: u64) -> Result<(i32, Vec<u8>), CapabilityCallError> {
        self.net_socket_address(fd, false)
    }

    /// **`getpeername(sock)`** ⇒ `(地址族, `CP-39` 线格式地址)`。
    pub fn net_getpeername(&self, fd: u64) -> Result<(i32, Vec<u8>), CapabilityCallError> {
        self.net_socket_address(fd, true)
    }

    /// **`shutdown(sock, how)`**。
    pub fn net_shutdown(&self, fd: u64, how: i32) -> Result<(), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let shutdown = table.shutdown.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
        let mut errno = 0i32;
        match shutdown(
            table.state,
            pyawa_capabilities::net::SocketHandle(handle),
            how,
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(()),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **`setsockopt(sock, level, name, value)`**（值按裸字节转发）。
    pub fn net_setsockopt(
        &self,
        fd: u64,
        level: i32,
        name: i32,
        value: &[u8],
    ) -> Result<(), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let setsockopt = table.setsockopt.ok_or(CapabilityCallError::NotImplemented)?;
        let handle = self.net_provider_handle(fd)?;
        let mut errno = 0i32;
        match setsockopt(
            table.state,
            pyawa_capabilities::net::SocketHandle(handle),
            level,
            name,
            value.as_ptr(),
            value.len(),
            &mut errno,
        ) {
            pyawa_capabilities::net::CapStatus::Ok => Ok(()),
            pyawa_capabilities::net::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::net::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `net` 域关闭 fd**（`CP-17`：幂等）。表项释放；别名计数归零才调 provider `close`。
    pub fn net_close(&self, fd: u64) -> Result<(), CapabilityCallError> {
        // 域未注册 ⇒ 与其它 net 方法同一口径（`CP-2`）。
        let _ = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        if fd < FIRST_DYNAMIC_FD {
            return Ok(());
        }
        self.capability_release(fd)
    }

    /// **provider `close` 槽的直达入口**（能力表／实例销毁兜底用；吃 provider 句柄，不吃 `fd`）。
    pub(crate) fn net_provider_close(&self, provider_handle: u64) -> Result<(), CapabilityCallError> {
        let table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let close = table.close.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match close(
            table.state,
            pyawa_capabilities::net::SocketHandle(provider_handle),
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
