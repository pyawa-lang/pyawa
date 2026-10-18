//! `net` 域的真实机器实现（`DESIGN.md` §7：本 crate 是平台依赖的**集中点**）。
//!
//! 形状来自 [`pyawa_capabilities::net`]（`CP-12`：接口 crate 只含形状）；本文件是**真实实现**。
//!
//! # 本片的如实范围（`P3-29` 首片）
//!
//! **已实现**：`socketpair`（`CP-38`）／`socket`／`send`／`recv`／`close`（`CP-17`）。
//! **未实现**：`connect`／`bind`／`listen`／`accept`／`shutdown`／`getsockname`／`getpeername`／
//! `getsockopt`／`setsockopt`／`poll` —— vtable 里**留 `None`**（`CP-3`：只有该操作未实现，
//! 绝不假装成功）。
//!
//! # `fd` 语义（`CP-31`，如实记差距）
//!
//! `CP-31` 要求 `fd` ＝ **实例能力表下标**（分配／释放／复制在 VM 侧）。本片沿用 `fs` 域的先例：
//! provider 自管一个**不透明句柄号段**（**从 3 起**，同 [`crate::fs_posix::PosixFs`]），
//! VM 侧适配层把它当 `fd` 原样交给 Python —— 因此：
//! - 单域内 `fd` 是 int、可关、关后报 `EBADF`（`CP-17`／`CP-18` ✓）；
//! - **跨域统一编号尚未落地**：`fs` 与 `net` 的号段各自从 3 起 ⇒ 同一个 `fd` 数字可能指两个
//!   不同能力（`select`／`os.dup2` 那一族的 `T-CP-3` 前件）⇒ 这是**已知缺口**，等实例能力表
//!   专项（VM 侧 `fd` 表）落地时收口。
//!
//! # 平台口径
//!
//! 直接用 `socketpair(2)`／`socket(2)`／`send(2)`／`recv(2)`／`close(2)` 的 C 符号（本 crate 是
//! 平台集中点，`CX-4` 排除本 crate）；`unsafe` 按**项**开许可（本 crate 的规矩：不许整文件放开）。
//! `send` 带 `MSG_NOSIGNAL`（Linux 取值）以免"对端已关"把进程用 `SIGPIPE` 杀掉 —— 参照实现在
//! 进程级忽略 `SIGPIPE`，本层不能做进程级设置（多实例隔离），故按调用点抑制。
//!
//! # 并发口径
//!
//! 本层只保证 `Mutex` 下句柄表一致；**阻塞调用不持锁**（否则一个 `recv` 会卡住同 provider 的
//! 其它 socket）。M5 的兼容边界是**单线程**（`PLAN-milestones.md` §6），多线程下"一边 `close`
//! 一边 `recv`"的 fd 复用竞态**不在本片承诺内**（如实记）。

use core::ffi::c_void;
use pyawa_capabilities::net::{CapStatus, CpNetVtable, SocketHandle};
use std::collections::HashMap;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// `send(2)` 的 `MSG_NOSIGNAL`（Linux 取值；本 crate 是平台集中点）。
const MSG_NOSIGNAL: i32 = 0x4000;

/// C 符号（`socket`／`socketpair`／`send`／`recv`／`close` 的 POSIX 名）。
#[allow(unsafe_code)]
mod sys {
    use core::ffi::{c_int, c_void};

    unsafe extern "C" {
        /// `socket(2)`。
        pub fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
        /// `socketpair(2)`。
        pub fn socketpair(domain: c_int, ty: c_int, protocol: c_int, sv: *mut c_int) -> c_int;
        /// `send(2)`。
        pub fn send(fd: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize;
        /// `recv(2)`。
        pub fn recv(fd: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize;
        /// `close(2)`。
        pub fn close(fd: c_int) -> c_int;
    }
}

/// `net` 域的 provider 状态：句柄表（`CP-17`：`close` 即释放）。
#[derive(Default)]
pub struct PosixNet {
    sockets: Mutex<HashMap<u64, OwnedFd>>,
    next: AtomicU64,
}

impl PosixNet {
    /// 新建一个 provider（句柄从 **3** 起；`0`／`1`／`2` 留给 `fs` 域的标准流约定）。
    pub fn new() -> Self {
        Self { sockets: Mutex::new(HashMap::new()), next: AtomicU64::new(3) }
    }

    /// 本 provider 的 vtable（`CP-30`／`AB-39`：`version` ＝ `PA_ABI_VERSION`）。
    ///
    /// **调用方必须保证** `self` 活得比该 vtable 久（CLI 里就是一个栈上局部）。
    pub fn vtable(&self) -> CpNetVtable {
        CpNetVtable {
            version: pyawa_abi::PA_ABI_VERSION,
            size: core::mem::size_of::<CpNetVtable>() as u32,
            state: (self as *const Self).cast_mut().cast::<c_void>(),
            socket: Some(net_socket),
            connect: None,
            bind: None,
            listen: None,
            accept: None,
            send: Some(net_send),
            recv: Some(net_recv),
            shutdown: None,
            getsockname: None,
            getpeername: None,
            getsockopt: None,
            setsockopt: None,
            poll: None,
            socketpair: Some(net_socketpair),
            close: Some(net_close),
        }
    }

    /// 句柄 ⇒ 原始 `fd`；**已关／未知 ⇒ `None`**（`CP-18` 的 `EBADF` 形状由调用方给）。
    fn fd_of(&self, handle: SocketHandle) -> Option<RawFd> {
        self.sockets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&handle.0)
            .map(OwnedFd::as_raw_fd)
    }

    /// 收下一枚新 `fd`，返回不透明句柄（`CP-31` 的句柄号段见模块文档）。
    fn insert(&self, fd: OwnedFd) -> SocketHandle {
        let handle = self.next.fetch_add(1, Ordering::Relaxed);
        self.sockets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(handle, fd);
        SocketHandle(handle)
    }
}

/// 一次失败调用之后取宿主 `errno`（`std::io::Error::last_os_error` 直接读本线程 `errno`）。
fn last_errno() -> i32 {
    errno_of(&std::io::Error::last_os_error())
}

/// 把 `io::Error` 折成 `errno`：优先用宿主的原生值；没有原生值的借用 `EIO`（同 `fs_posix`）。
fn errno_of(error: &std::io::Error) -> i32 {
    match error.raw_os_error() {
        Some(value) => value,
        None => fallback_errno("EIO"),
    }
}

fn fallback_errno(name: &str) -> i32 {
    // 宿主 errno 表的**唯一真相**在 `pyawa_abi`（`CM-19`／`CM-20`）。
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or(0)
}

fn ebadf() -> i32 {
    fallback_errno("EBADF")
}

fn eintr() -> i32 {
    fallback_errno("EINTR")
}

/// `socket(family, type, proto)`（`SPEC-capabilities.md` §9.2）：申请一枚 socket `fd`。
#[allow(unsafe_code)]
extern "C" fn net_socket(
    state: *mut c_void,
    family: i32,
    ty: i32,
    proto: i32,
    out: *mut SocketHandle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: provider 状态由 `vtable()` 的 `state` 给出，且比 vtable 活得久。
    let provider = unsafe { &*(state as *const PosixNet) };
    // SAFETY: 上面那个生 provider 的契约；三个整数参数原样转发给内核。
    let raw = unsafe { sys::socket(family, ty, proto) };
    if raw < 0 {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = last_errno() };
        return CapStatus::Machine;
    }
    // SAFETY: `raw` 是刚由 `socket(2)` 交回、且尚未交给别人的 fd。
    let owned = unsafe { OwnedFd::from_raw_fd(raw) };
    let handle = provider.insert(owned);
    // SAFETY: 出参由调用方提供。
    unsafe { *out = handle };
    CapStatus::Ok
}

/// `socketpair(family, type, proto)`（`CP-38`）：返回**一对** socket 能力（local-only，无地址）。
///
/// 两个 `fd` 都由内核在本调用内建好；任一失败则**两者都不写**（调用方按 `Machine` 处理）。
#[allow(unsafe_code)]
extern "C" fn net_socketpair(
    state: *mut c_void,
    family: i32,
    ty: i32,
    proto: i32,
    out_a: *mut SocketHandle,
    out_b: *mut SocketHandle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: provider 状态由 `vtable()` 的 `state` 给出，且比 vtable 活得久。
    let provider = unsafe { &*(state as *const PosixNet) };
    let mut fds: [RawFd; 2] = [-1; 2];
    // SAFETY: `fds` 是长度 2 的可写缓冲；family／type／proto 原样转发给内核
    // （`AF_INET` 一类 v1 不支持的族由内核回真实机器错误，不在这里编错误类别，`CP-6`）。
    let result = unsafe { sys::socketpair(family, ty, proto, fds.as_mut_ptr()) };
    if result < 0 {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = last_errno() };
        return CapStatus::Machine;
    }
    // SAFETY: 两个 fd 都是刚由 `socketpair(2)` 交回、尚未交给别人的所有权。
    let (owned_a, owned_b) = unsafe {
        (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1]))
    };
    let a = provider.insert(owned_a);
    let b = provider.insert(owned_b);
    // SAFETY: 出参由调用方提供。
    unsafe {
        *out_a = a;
        *out_b = b;
    }
    CapStatus::Ok
}

/// `send(sock, buf)`（`SPEC-capabilities.md` §9.2）：返回实际发出的字节数。
#[allow(unsafe_code)]
extern "C" fn net_send(
    state: *mut c_void,
    socket: SocketHandle,
    buffer: *const u8,
    len: usize,
    out_sent: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同上。
    let provider = unsafe { &*(state as *const PosixNet) };
    let Some(fd) = provider.fd_of(socket) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = ebadf() };
        return CapStatus::Machine;
    };
    // SAFETY: 调用方按 vtable 契约给出 `buffer`／`len`（`SPEC-c-abi.md` 的出参约定）。
    let bytes = if len == 0 {
        &[][..]
    } else {
        unsafe { core::slice::from_raw_parts(buffer, len) }
    };
    loop {
        // SAFETY: `fd` 由上面的句柄表给出（单线程口径见模块文档）；`bytes` 活到调用结束。
        let written = unsafe {
            sys::send(fd, bytes.as_ptr().cast::<c_void>(), bytes.len(), MSG_NOSIGNAL)
        };
        if written < 0 {
            let errno = last_errno();
            if errno == eintr() {
                continue;
            }
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
        // SAFETY: 出参由调用方提供。
        unsafe { *out_sent = written as usize };
        return CapStatus::Ok;
    }
}

/// `recv(sock, n)`（`SPEC-capabilities.md` §9.2）：返回实际收到的字节数。
#[allow(unsafe_code)]
extern "C" fn net_recv(
    state: *mut c_void,
    socket: SocketHandle,
    buffer: *mut u8,
    capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同上。
    let provider = unsafe { &*(state as *const PosixNet) };
    let Some(fd) = provider.fd_of(socket) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = ebadf() };
        return CapStatus::Machine;
    };
    if capacity == 0 {
        // SAFETY: 出参由调用方提供。
        unsafe { *out_len = 0 };
        return CapStatus::Ok;
    }
    loop {
        // SAFETY: 调用方按 vtable 契约给出可写 `buffer`／`capacity`；`fd` 同上。
        let received = unsafe { sys::recv(fd, buffer.cast::<c_void>(), capacity, 0) };
        if received < 0 {
            let errno = last_errno();
            if errno == eintr() {
                continue;
            }
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = errno };
            return CapStatus::Machine;
        }
        // SAFETY: 出参由调用方提供。
        unsafe { *out_len = received as usize };
        return CapStatus::Ok;
    }
}

/// `close(handle)`（`CP-17`：释放，**必须幂等**）—— 已关／未知句柄也返回成功。
#[allow(unsafe_code)]
extern "C" fn net_close(
    state: *mut c_void,
    socket: SocketHandle,
    _errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同上。
    let provider = unsafe { &*(state as *const PosixNet) };
    let taken = provider
        .sockets
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&socket.0);
    if let Some(owned) = taken {
        // 显式落一次 `close(2)`（不让"释放"依赖 `Drop` 的时机——`CP-17` 的语义是此刻释放）。
        let raw = owned.as_raw_fd();
        // SAFETY: 这个 fd 由 `owned` 独占持有；调用后立刻 `forget` 所有权（否则会二次 close）。
        unsafe {
            sys::close(raw);
        }
        core::mem::forget(owned);
    }
    CapStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vtable_carries_the_shared_abi_version_and_its_own_size() {
        let provider = PosixNet::new();
        let table = provider.vtable();
        assert_eq!(table.version, pyawa_abi::PA_ABI_VERSION, "CP-30／AB-39");
        assert_eq!(table.size as usize, core::mem::size_of::<CpNetVtable>());
        assert!(!table.state.is_null());
        assert!(table.socketpair.is_some() && table.send.is_some() && table.recv.is_some());
        assert!(table.close.is_some());
        // 本片未实现的槽位如实留 None（CP-3）
        assert!(table.connect.is_none() && table.bind.is_none() && table.poll.is_none());
    }
}
