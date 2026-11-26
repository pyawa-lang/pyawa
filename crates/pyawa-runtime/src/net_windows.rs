//! Windows `net` 域的真实机器实现（`PLAN §12.20` W4）。
//!
//! 形状来自 [`pyawa_capabilities::net`]（`CP-12`：接口 crate 只含形状）；本文件是**真实实现**，
//! **逐槽位语义照** `net_posix.rs`：参数摆放（`state` 在前、出参在尾、`errno_out` 机器错误通道）、
//! `accept`／`getsockname`／`getpeername` 的**两段式**地址出参（`out_len` 回总长、只写
//! `min(总长, 容量)`）、`poll` 的两段式就绪集合（`out_len` 回就绪项**总数**）、
//! `timeout_ns < 0` 无限／`0` 轮询／`> 0` 按 ns → ms **向上取整**。
//!
//! # 平台面（本 crate 是平台依赖的**集中点**，`CX-4` 排除本 crate）
//!
//! 直接声明 **ws2_32** 的 C 符号（`extern "system"`）：`WSAStartup`／`socket`／`bind`／
//! `listen`／`accept`／`connect`／`send`／`recv`／`shutdown`／`getsockname`／`getpeername`／
//! `setsockopt`／`getsockopt`／`closesocket`／`WSAPoll`。`SOCKET` ＝ `usize`
//! （`INVALID_SOCKET` ＝ `usize::MAX`）；`WSAStartup(0x0202, …)` 用 [`OnceLock`] **进程内只做一次**
//! （Winsock 的启动是进程级引用计数；失败时每次调用都按机器错误回报，**不 panic、不假装成功**，
//! `CP-5`）。不在 provider 析构时 `WSACleanup`：组合根把 provider 泄漏到进程末（`lib.rs`），
//! 提前 cleanup 会让其它实例的 socket 失效 —— 如实不做。
//!
//! # errno 映射（`CM-20`：数字随平台、映射**按名字**）
//!
//! `WSAGetLastError()` 返回 **Winsock 错误码**（10004…），不是 POSIX `errno`；本文件按名字把它
//! 翻成 `pyawa_abi::platform_errno::HOST_ERRNO` 里的数值（表的唯一真相在 `pyawa-abi`）：
//! `10013→EACCES`、`10048→EADDRINUSE`、`10061→ECONNREFUSED`、`10054→ECONNRESET`、
//! `10060→ETIMEDOUT`、`10057→ENOTCONN`、`10038→ENOTSOCK`、`10053→ECONNABORTED`、
//! `10035→EWOULDBLOCK`、`10040→EMSGSIZE`、`10022→EINVAL`、`10014→EFAULT`、`10024→EMFILE`、
//! `10055→ENOBUFS`，其余（含 `WSAEINTR=10004` 之外的未知码）→ `EIO`。
//!
//! # 地址线（`CP-39`）
//!
//! 只有 `AF_INET`：`decode_ipv4` ⇒ WinSock `SOCKADDR_IN`（`sin_family=2`、`sin_port` 网络序、
//! `sin_addr` 4 字节）；反方向用 `encode_ipv4` 回写。IPv6／`AF_UNIX` **不在 v1**
//! （`socket` 对其它族回 `EAFNOSUPPORT`）。
//!
//! # `poll`（`SPEC-capabilities.md` §9.2）
//!
//! 真 `WSAPoll` ＋ **Linux poll 位集 ↔ Winsock 位集**翻译。两套位集同名常量**数值不同**
//! （Linux：`POLLIN=0x001`／`POLLOUT=0x004`／`POLLERR=0x008`／`POLLHUP=0x010`；
//! Winsock：`POLLRDNORM=0x100`／`POLLWRNORM=0x010`／`POLLERR=0x001`／`POLLHUP=0x002`）：
//! 进参把 Linux 位翻成 Winsock 位，出参 `revents` 再翻回 Linux 位。
//! `POLLPRI` **忽略**：Microsoft Winsock provider 不支持它（进参置了会让 `WSAPoll` 报
//! `WSAEINVAL`），出参也从不返回（`winsock2.h` 的 `POLLPRI=0x0400` 在本实现里不出现）。
//! `WSAPoll` 要求数组里**至少一个有效 socket**（空数组/全无效 ⇒ `WSAEINVAL`）⇒ `count == 0` 时
//! 本实现自己做**等价睡眠**（`< 0` 无限、`0` 立即、`> 0` 向上取整毫秒）。
//!
//! # `setsockopt`／`getsockopt`（`CP-3` 允许未实现；这里按 v1 需要的选项**如实接线**）
//!
//! guest 侧（`pyawa-stdlib` 的 `_socket_module`）传的是 **POSIX 选项号**；Winsock 的选项号
//! 是另一套（`SOL_SOCKET=0xFFFF` 等）⇒ 必须翻译，**不能原样转发**（转错会静默设错选项）。
//! 本片接：`SOL_SOCKET` 的 `SO_REUSEADDR`／`SO_TYPE`／`SO_ERROR`／`SO_SNDBUF`／`SO_RCVBUF`／
//! `SO_SNDTIMEO`／`SO_RCVTIMEO` 与 `IPPROTO_TCP` 的 `TCP_NODELAY`；其余按机器错误
//! `ENOPROTOOPT` 如实拒绝（`CP-6`：不假装成功）。超时选项按 Win32 **`DWORD` 毫秒**位传值
//! （guest 的 int 毫秒 ⇒ `u32`；负值夹 `0`＝无超时）。
//!
//! # 如实边界
//!
//! 1. **`socketpair` 槽给 `None`**（`CP-3`）：Windows 没有 `socketpair(2)`；用 loopback 模拟
//!    （一对已连接 TCP）留后续轮次。`CP-5` 禁止把它编码成某个 `errno`。
//! 2. **`close` 对已关／未知句柄报 `Machine` ＋ `EBADF`**（`CP-18`）。注意这与 `net_posix`
//!    的"幂等 `Ok`"**不同**：实例能力表的幂等释放由 VM 侧 `Instance::net_close` 的
//!    `capability_release` 保证（重复释放不会走到 provider）。
//! 3. **单线程兼容边界**（`PLAN-milestones.md` §6）：`Mutex` 只护句柄表，**阻塞调用不持锁**；
//!    "一边 `close` 一边 `recv`"的句柄复用竞态不在本片承诺内（同 `net_posix`）。
//! 4. **`poll` 的零 socket 无限等待不可被唤醒**：Windows 没有 POSIX 信号那种可中断睡眠面 ⇒
//!    分片睡眠（本路径拿不到取消令牌，`CP-26` 的令牌化留 VM 侧适配）。
//!
//! `unsafe` 按**项**开许可（本 crate 的规矩：`lib.rs` 的 `#![deny(unsafe_code)]` 不许整文件放开）；
//! 每处 FFI 调用带 `SAFETY:`。

use core::ffi::c_void;
use pyawa_capabilities::net::{
    decode_ipv4, encode_ipv4, family, socket_type, CapStatus, CpNetVtable, PollFd, SocketHandle,
    SOCKADDR_V1_IPV4_LEN,
};
use pyawa_core::Instance;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// `WSAStartup` 请求的版本：`MAKEWORD(2, 2)`（Winsock 2.2）。
const WSA_VERSION: u16 = 0x0202;
/// Winsock 的无效 socket 哨兵 `INVALID_SOCKET`（`(SOCKET)(~0)`）。
const INVALID_SOCKET: usize = usize::MAX;
/// `WSADATA` 出参缓冲的字节数：真结构 x64 上 408B、x86 上 400B ⇒ 512B ＋ 8 字节对齐足够。
const WSA_DATA_BYTES: usize = 512;
/// WinSock `SOCKADDR_IN` 的字节数（`bind`／`connect`／`accept` 的地址长度）。
const SOCKADDR_IN_LEN: i32 = 16;

// --------------------------------------------------------------------------- //
// POSIX（guest 侧）选项号：与 `pyawa-stdlib` `_socket_module` 的常量表同一口径
// --------------------------------------------------------------------------- //

/// `SOL_SOCKET`（POSIX 取值；`_socket_module` 的常量表）。
const SOL_SOCKET_POSIX: i32 = 1;
/// `SO_REUSEADDR`（POSIX 取值）。
const SO_REUSEADDR_POSIX: i32 = 2;
/// `SO_TYPE`（POSIX 取值）。
const SO_TYPE_POSIX: i32 = 3;
/// `SO_ERROR`（POSIX 取值）。
const SO_ERROR_POSIX: i32 = 4;
/// `SO_SNDBUF`（POSIX 取值）。
const SO_SNDBUF_POSIX: i32 = 7;
/// `SO_RCVBUF`（POSIX 取值）。
const SO_RCVBUF_POSIX: i32 = 8;
/// `SO_RCVTIMEO`（POSIX 取值）。
const SO_RCVTIMEO_POSIX: i32 = 20;
/// `SO_SNDTIMEO`（POSIX 取值）。
const SO_SNDTIMEO_POSIX: i32 = 21;
/// `IPPROTO_TCP`（POSIX 取值；与 Winsock 同值）。
const IPPROTO_TCP_POSIX: i32 = 6;
/// `TCP_NODELAY`（POSIX 取值；与 Winsock 同值）。
const TCP_NODELAY_POSIX: i32 = 1;

// --------------------------------------------------------------------------- //
// Winsock 选项号（`winsock2.h`）：与 POSIX 编号**不同**，必须翻译
// --------------------------------------------------------------------------- //

/// Winsock `SOL_SOCKET`（`0xFFFF`，不是 POSIX 的 `1`）。
const WSA_SOL_SOCKET: i32 = 0xFFFF;
/// Winsock `SO_REUSEADDR`。
const WSA_SO_REUSEADDR: i32 = 0x0004;
/// Winsock `SO_SNDBUF`。
const WSA_SO_SNDBUF: i32 = 0x1001;
/// Winsock `SO_RCVBUF`。
const WSA_SO_RCVBUF: i32 = 0x1002;
/// Winsock `SO_SNDTIMEO`（值是 `DWORD` 毫秒）。
const WSA_SO_SNDTIMEO: i32 = 0x1005;
/// Winsock `SO_RCVTIMEO`（值是 `DWORD` 毫秒）。
const WSA_SO_RCVTIMEO: i32 = 0x1006;
/// Winsock `SO_ERROR`。
const WSA_SO_ERROR: i32 = 0x1007;
/// Winsock `SO_TYPE`。
const WSA_SO_TYPE: i32 = 0x1008;

// --------------------------------------------------------------------------- //
// Winsock poll 位（`winsock2.h`）：语义与 Linux 同名位一样，**数值不同**
// --------------------------------------------------------------------------- //

/// Winsock `POLLERR`（`0x0001`；Linux 的是 `0x008`）。
const WSA_POLLERR: i16 = 0x0001;
/// Winsock `POLLHUP`（`0x0002`；Linux 的是 `0x010`）。
const WSA_POLLHUP: i16 = 0x0002;
/// Winsock `POLLNVAL`（`0x0004`）。
const WSA_POLLNVAL: i16 = 0x0004;
/// Winsock `POLLWRNORM`（`0x0010`；对应 Linux `POLLOUT=0x004`）。
const WSA_POLLWRNORM: i16 = 0x0010;
/// Winsock `POLLRDNORM`（`0x0100`；对应 Linux `POLLIN=0x001`）。
const WSA_POLLRDNORM: i16 = 0x0100;
/// Winsock `POLLRDBAND`（`0x0200`；带外数据，本实现归到可读）。
const WSA_POLLRDBAND: i16 = 0x0200;

// --------------------------------------------------------------------------- //
// ws2_32 FFI
// --------------------------------------------------------------------------- //

/// ws2_32 的 C 符号（`extern "system"`：Win32 ABI；`SOCKET` ＝ `usize`）。
#[allow(unsafe_code)]
mod sys {
    use core::ffi::c_void;

    #[link(name = "ws2_32")]
    unsafe extern "system" {
        /// `WSAStartup`（`lpWSAData` 指向 ≥ `sizeof(WSADATA)` 的可写缓冲）。
        pub fn WSAStartup(w_version_requested: u16, wsa_data: *mut c_void) -> i32;
        /// `socket(af, type, protocol)`；失败返回 `INVALID_SOCKET`。
        pub fn socket(af: i32, ty: i32, protocol: i32) -> usize;
        pub fn WSASocketW(
            af: i32,
            ty: i32,
            protocol: i32,
            protocol_info: *mut core::ffi::c_void,
            group: u32,
            flags: u32,
        ) -> usize;
        /// `bind(s, name, namelen)`。
        pub fn bind(s: usize, name: *const u8, namelen: i32) -> i32;
        /// `listen(s, backlog)`。
        pub fn listen(s: usize, backlog: i32) -> i32;
        /// `accept(s, addr, addrlen)`；新 socket 或 `INVALID_SOCKET`。
        pub fn accept(s: usize, addr: *mut u8, addrlen: *mut i32) -> usize;
        /// `connect(s, name, namelen)`。
        pub fn connect(s: usize, name: *const u8, namelen: i32) -> i32;
        /// `send(s, buf, len, flags)`；返回字节数或 `SOCKET_ERROR`。
        pub fn send(s: usize, buf: *const u8, len: i32, flags: i32) -> i32;
        /// `recv(s, buf, len, flags)`；返回字节数或 `SOCKET_ERROR`。
        pub fn recv(s: usize, buf: *mut u8, len: i32, flags: i32) -> i32;
        /// `shutdown(s, how)`（`SD_RECEIVE=0`／`SD_SEND=1`／`SD_BOTH=2`）。
        pub fn shutdown(s: usize, how: i32) -> i32;
        /// `getsockname(s, name, namelen)`。
        pub fn getsockname(s: usize, name: *mut u8, namelen: *mut i32) -> i32;
        /// `getpeername(s, name, namelen)`。
        pub fn getpeername(s: usize, name: *mut u8, namelen: *mut i32) -> i32;
        /// `setsockopt(s, level, optname, optval, optlen)`（`optlen` 是 `int`）。
        pub fn setsockopt(s: usize, level: i32, optname: i32, optval: *const u8, optlen: i32)
        -> i32;
        /// `getsockopt(s, level, optname, optval, optlen)`（`optlen` 是 `int*`）。
        pub fn getsockopt(s: usize, level: i32, optname: i32, optval: *mut u8, optlen: *mut i32)
        -> i32;
        /// `closesocket(s)`。
        pub fn closesocket(s: usize) -> i32;
        /// `WSAPoll(fdArray, fds, timeout)`：注意第二参是 `ULONG`（不是 BSD 的 `nfds_t`）。
        pub fn WSAPoll(fd_array: *mut super::WsaPollFd, fds: u32, timeout: i32) -> i32;
        /// `WSAGetLastError()`：本线程最近一次 Winsock 错误码。
        pub fn WSAGetLastError() -> i32;
    }
}

/// `WSAStartup` 的出参缓冲（**不透明**）：真 `WSADATA` 是 x64 408B／x86 400B，
/// 取 512B ＋ 8 字节对齐即可容纳（不依赖字段布局，避免手抄结构体出错）。
#[repr(C, align(8))]
struct WsaData {
    bytes: [u8; WSA_DATA_BYTES],
}

/// 内核 `SOCKADDR_IN`（`winsock2.h`；16 字节）。
///
/// `port` 用整数存**网络序**（见 [`port_to_net`]／[`port_from_net`]）：在 x86/x64 这类小端宿主上，
/// 写进去的数值内存形态就是大端字节序；`addr` 直接用字节数组，避开主机字节序。
#[repr(C)]
#[derive(Clone, Copy)]
struct SockaddrIn {
    /// `sin_family`（`AF_INET` ＝ 2）。
    family: u16,
    /// `sin_port`（网络字节序的数值，见 [`port_to_net`]）。
    port: u16,
    /// `sin_addr`（4 字节地址）。
    addr: [u8; 4],
    /// `sin_zero`（填充）。
    zero: [u8; 8],
}

// `SOCKADDR_IN` 是 16 字节；WinSock 的 `bind`／`connect`／`accept` 都按这个长度收地址。
const _: () = assert!(core::mem::size_of::<SockaddrIn>() == SOCKADDR_IN_LEN as usize);

/// Winsock `WSAPOLLFD`（`SOCKET fd; SHORT events; SHORT revents;`）。
///
/// x64 上 `SOCKET` 是 8 字节 ⇒ C 结构 16 字节（含尾部填充），Rust `#[repr(C)]` 同布局。
#[repr(C)]
#[derive(Clone, Copy)]
struct WsaPollFd {
    /// `SOCKET`（`usize`）。
    fd: usize,
    /// 关心的事件（**Winsock 位**，见模块文档）。
    events: i16,
    /// 发生的事件（**Winsock 位**，出参）。
    revents: i16,
}

#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::size_of::<WsaPollFd>() == 16);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::size_of::<WsaPollFd>() == 8);

// --------------------------------------------------------------------------- //
// errno
// --------------------------------------------------------------------------- //

/// 按名字取宿主 `errno` 数值（`CM-20`：表的唯一真相在 `pyawa-abi`）。
fn errno_named(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or(0)
}

/// Winsock 错误码 ⇒ POSIX `errno` 数值（至少覆盖 `SPEC-capabilities.md` §9.2 的网络错误面）。
fn wsa_errno(code: i32) -> i32 {
    let name = match code {
        // `WSAEINTR`
        10004 => "EINTR",
        // `WSAEBADF`
        10009 => "EBADF",
        // `WSAEACCES`
        10013 => "EACCES",
        // `WSAEFAULT`
        10014 => "EFAULT",
        // `WSAEINVAL`
        10022 => "EINVAL",
        // `WSAEMFILE`
        10024 => "EMFILE",
        // `WSAEWOULDBLOCK`
        10035 => "EWOULDBLOCK",
        // `WSAENOTSOCK`
        10038 => "ENOTSOCK",
        // `WSAEMSGSIZE`
        10040 => "EMSGSIZE",
        // `WSAENOPROTOOPT`
        10042 => "ENOPROTOOPT",
        // `WSAEPROTONOSUPPORT`
        10043 => "EPROTONOSUPPORT",
        // `WSAEAFNOSUPPORT`
        10047 => "EAFNOSUPPORT",
        // `WSAEADDRINUSE`
        10048 => "EADDRINUSE",
        // `WSAEADDRNOTAVAIL`
        10049 => "EADDRNOTAVAIL",
        // `WSAENETUNREACH`
        10051 => "ENETUNREACH",
        // `WSAECONNABORTED`
        10053 => "ECONNABORTED",
        // `WSAECONNRESET`
        10054 => "ECONNRESET",
        // `WSAENOBUFS`
        10055 => "ENOBUFS",
        // `WSAENOTCONN`
        10057 => "ENOTCONN",
        // `WSAESHUTDOWN`
        10058 => "ESHUTDOWN",
        // `WSAETIMEDOUT`
        10060 => "ETIMEDOUT",
        // `WSAECONNREFUSED`
        10061 => "ECONNREFUSED",
        // `WSAEHOSTUNREACH`
        10065 => "EHOSTUNREACH",
        // 其余一律 `EIO`（`CP-5` 只承诺"机器错误"这一态，不编造更精确的语义）。
        _ => "EIO",
    };
    errno_named(name)
}

/// 本线程最近一次 Winsock 错误 ⇒ POSIX `errno`（**紧跟失败调用**调用才有意义）。
#[allow(unsafe_code)]
fn last_wsa_errno() -> i32 {
    // SAFETY: `WSAGetLastError` 无参数、只读本线程的 Winsock 错误状态。
    let code = unsafe { sys::WSAGetLastError() };
    wsa_errno(code)
}

/// 是否 `EINTR`（`connect`／`accept`／`send`／`recv`／`poll` 的重试口径同 `net_posix`）。
fn is_eintr(errno: i32) -> bool {
    errno == errno_named("EINTR")
}

/// 写 `errno_out`（`CP-5`：机器错误走这条通道）。
#[allow(unsafe_code)]
fn write_errno(errno_out: *mut i32, errno: i32) {
    // SAFETY: 出参由调用方按 vtable 契约提供（非空）。
    unsafe { *errno_out = errno };
}

/// 写一个 `Copy` 出参（`SocketHandle`／`usize`／`i32` 等）。
#[allow(unsafe_code)]
fn write_out<T>(out: *mut T, value: T) {
    // SAFETY: 出参由调用方按 vtable 契约提供（非空、对 `T` 对齐）。
    unsafe { *out = value };
}

// --------------------------------------------------------------------------- //
// WSAStartup（进程内一次）与地址转换
// --------------------------------------------------------------------------- //

/// `WSAStartup`：`OnceLock` 保证**进程内只做一次**；失败时缓存错误码，之后每次调用都按机器错误回报。
#[allow(unsafe_code)]
fn wsa_startup() -> Result<(), i32> {
    static STATE: OnceLock<Result<(), i32>> = OnceLock::new();
    *STATE.get_or_init(|| {
        let mut data = WsaData { bytes: [0u8; WSA_DATA_BYTES] };
        // SAFETY: `data` 是 ≥ `sizeof(WSADATA)` 且 8 字节对齐的可写缓冲；版本号是 WS2.2。
        let code = unsafe { sys::WSAStartup(WSA_VERSION, data.bytes.as_mut_ptr().cast()) };
        if code == 0 {
            Ok(())
        } else {
            // `WSAStartup` 失败时返回值本身就是错误码（不保证设置 last error）。
            Err(wsa_errno(code))
        }
    })
}

/// 主机字节序端口 ⇒ 网络字节序（写进 [`SockaddrIn::port`] 的数值）。
///
/// 小端宿主上 `port.to_be()` 会让这个 `u16` 的**内存字节**变成大端 ⇒ 正是 `sin_port` 要的线格式。
fn port_to_net(port: u16) -> u16 {
    port.to_be()
}

/// 网络字节序（[`SockaddrIn::port`] 的数值）⇒ 主机端口。
fn port_from_net(port: u16) -> u16 {
    u16::from_be(port)
}

/// `CP-39` 的 v1 线格式 ⇒ WinSock `SOCKADDR_IN`。
///
/// 只有 `AF_INET` 在 v1 里：其它族按真实机器语义回 `EAFNOSUPPORT`（`CP-6`）；长度不符回 `EINVAL`。
#[allow(unsafe_code)]
fn sockaddr_in_of(af: i32, addr: *const u8, addr_len: usize) -> Result<SockaddrIn, i32> {
    if af != family::AF_INET {
        return Err(errno_named("EAFNOSUPPORT"));
    }
    if addr.is_null() || addr_len != SOCKADDR_V1_IPV4_LEN {
        return Err(errno_named("EINVAL"));
    }
    // SAFETY: 上面已判空且长度恰为 v1 IPv4 线长度；调用方按 vtable 契约保证缓冲在调用期间有效。
    let wire = unsafe { core::slice::from_raw_parts(addr, addr_len) };
    let Some((address, port)) = decode_ipv4(wire) else {
        return Err(errno_named("EINVAL"));
    };
    Ok(SockaddrIn {
        family: family::AF_INET as u16,
        port: port_to_net(port),
        addr: address,
        zero: [0; 8],
    })
}

/// `SOCKADDR_IN` 出参 ⇒ `CP-39` 线格式（**两段式**：只写 `min(总长, 容量)`，`out_len` 回总长）。
#[allow(unsafe_code)]
fn write_sockaddr_out(
    sockaddr: &SockaddrIn,
    out: *mut u8,
    capacity: usize,
    out_len: *mut usize,
) -> CapStatus {
    let wire = encode_ipv4(sockaddr.addr, port_from_net(sockaddr.port));
    let written = wire.len().min(capacity);
    if !out.is_null() && written > 0 {
        // SAFETY: `out` 由调用方按 `capacity` 给出；源（局部数组）与目标不重叠。
        unsafe { core::ptr::copy_nonoverlapping(wire.as_ptr(), out, written) };
    }
    // SAFETY: 出参由调用方提供。
    unsafe { *out_len = wire.len() };
    CapStatus::Ok
}

/// `timeout_ns` ⇒ `WSAPoll` 的毫秒（**向上取整**：不许比请求更早返回；`< 0` ⇒ 无限）。
fn timeout_ns_to_ms(timeout_ns: i64) -> i32 {
    if timeout_ns < 0 {
        return -1;
    }
    let millis = timeout_ns / 1_000_000 + i64::from(timeout_ns % 1_000_000 != 0);
    millis.min(i64::from(i32::MAX)) as i32
}

/// `count == 0` 的 `poll`（`WSAPoll` 不接受空数组）⇒ 等价睡眠：`< 0` 无限、`0` 立即、`> 0` 毫秒。
fn sleep_without_sockets(timeout_ns: i64) {
    if timeout_ns < 0 {
        // POSIX `poll(NULL, 0, -1)` 是无限等待；Windows 没有可被信号打断的睡眠面 ⇒ 分片睡
        // （如实记：本路径不可被唤醒，与 POSIX 的 `EINTR` 语义有差，见模块文档第 4 条）。
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    let millis = timeout_ns_to_ms(timeout_ns);
    if millis > 0 {
        std::thread::sleep(std::time::Duration::from_millis(millis as u64));
    }
}

// --------------------------------------------------------------------------- //
// poll 位集翻译（Linux `poll(2)` ↔ Winsock）
// --------------------------------------------------------------------------- //

/// Linux `poll(2)` 事件位 ⇒ Winsock 事件位（进参）。
///
/// `POLLPRI` 与 `POLLERR`／`POLLHUP`／`POLLNVAL` **不置**：前者 Microsoft Winsock provider 不支持，
/// 后三者是"总是汇报"的出参条件，进参置了没有意义。
fn wsa_events(events: i32) -> i16 {
    let mut win = 0i16;
    if events & Instance::POLLIN != 0 {
        // 普通数据归 `POLLRDNORM`；不置 `POLLRDBAND`（带外数据本片不请求）。
        win |= WSA_POLLRDNORM;
    }
    if events & Instance::POLLOUT != 0 {
        win |= WSA_POLLWRNORM;
    }
    win
}

/// Winsock `revents` ⇒ Linux `poll(2)` 事件位（出参）。
fn linux_events(win_revents: i16) -> i32 {
    let mut events = 0;
    if win_revents & WSA_POLLRDNORM != 0 || win_revents & WSA_POLLRDBAND != 0 {
        events |= Instance::POLLIN;
    }
    if win_revents & WSA_POLLWRNORM != 0 {
        events |= Instance::POLLOUT;
    }
    if win_revents & WSA_POLLERR != 0 {
        events |= Instance::POLLERR;
    }
    if win_revents & WSA_POLLHUP != 0 {
        events |= Instance::POLLHUP;
    }
    if win_revents & WSA_POLLNVAL != 0 {
        events |= Instance::POLLNVAL;
    }
    events
}

// --------------------------------------------------------------------------- //
// 选项翻译（POSIX 选项号 ⇒ Winsock 选项号）
// --------------------------------------------------------------------------- //

/// POSIX（guest 侧）`(level, optname)` ⇒ Winsock `(level, optname)`；不认识的选项返回 `None`
/// （调用方按 `ENOPROTOOPT` 如实拒绝，**不原样转发**，见模块文档）。
fn wsa_socket_option(level: i32, name: i32) -> Option<(i32, i32)> {
    match level {
        SOL_SOCKET_POSIX => match name {
            SO_REUSEADDR_POSIX => Some((WSA_SOL_SOCKET, WSA_SO_REUSEADDR)),
            SO_TYPE_POSIX => Some((WSA_SOL_SOCKET, WSA_SO_TYPE)),
            SO_ERROR_POSIX => Some((WSA_SOL_SOCKET, WSA_SO_ERROR)),
            SO_SNDBUF_POSIX => Some((WSA_SOL_SOCKET, WSA_SO_SNDBUF)),
            SO_RCVBUF_POSIX => Some((WSA_SOL_SOCKET, WSA_SO_RCVBUF)),
            SO_SNDTIMEO_POSIX => Some((WSA_SOL_SOCKET, WSA_SO_SNDTIMEO)),
            SO_RCVTIMEO_POSIX => Some((WSA_SOL_SOCKET, WSA_SO_RCVTIMEO)),
            _ => None,
        },
        // `TCP_NODELAY` 在两套 ABI 里同值（1），`IPPROTO_TCP` 也同值（6）。
        IPPROTO_TCP_POSIX => match name {
            TCP_NODELAY_POSIX => Some((IPPROTO_TCP_POSIX, TCP_NODELAY_POSIX)),
            _ => None,
        },
        _ => None,
    }
}

// --------------------------------------------------------------------------- //
// provider 状态
// --------------------------------------------------------------------------- //

/// `net` 域的 Windows provider 状态：句柄表（`CP-17`：`close` 即释放）。
///
/// 句柄号段从 **3** 起（同 `fs`／`net_posix` 的标准流约定：`0` 无效、`1`／`2` 留标准流）。
/// **Proactor 桥（P2）**：不透明句柄 ⇒ 原生 WinSock `SOCKET`。
///
/// `_socket.fileno()` 给的是 provider 句柄；`_overlapped.WSARecv` 需要原生 SOCKET。
/// 句柄号用**进程级**计数器保证跨实例唯一，桥表只为这一层服务，不改变脚本可见的 fd 语义。
static NEXT_SOCKET_HANDLE: AtomicU64 = AtomicU64::new(3);
static RAW_SOCKETS: Mutex<Option<HashMap<u64, usize>>> = Mutex::new(None);

fn raw_sockets() -> std::sync::MutexGuard<'static, Option<HashMap<u64, usize>>> {
    RAW_SOCKETS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 把 `_socket.fileno()` 的 provider 句柄解析成原生 SOCKET（读 `_overlapped.WSARecv` 用）。
pub fn raw_socket_for_handle(handle: i64) -> Option<usize> {
    let handle = u64::try_from(handle).ok()?;
    raw_sockets().as_ref().and_then(|map| map.get(&handle).copied())
}

pub struct WindowsNet {
    /// 不透明句柄 ⇒ WinSock `SOCKET`。
    sockets: Mutex<HashMap<u64, usize>>,
}

impl WindowsNet {
    /// 新建一个 provider（句柄从 **3** 起）。
    pub fn new() -> Self {
        Self {
            sockets: Mutex::new(HashMap::new()),
        }
    }

    /// 当前**打开着**的句柄数（`CP-17`／`CP-19` 的残留观察面；测试与诊断用）。
    pub fn open_handle_count(&self) -> usize {
        self.sockets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    /// 本 provider 的 vtable（`CP-30`／`AB-39`：`version` ＝ `PA_ABI_VERSION`，`state` 指回自己）。
    ///
    /// **调用方必须保证** `self` 活得比该 vtable 久（组合根里泄漏到进程末，见 `lib.rs`）。
    pub fn vtable(&self) -> CpNetVtable {
        CpNetVtable {
            version: pyawa_abi::PA_ABI_VERSION,
            size: core::mem::size_of::<CpNetVtable>() as u32,
            state: (self as *const Self).cast_mut().cast::<c_void>(),
            socket: Some(net_socket),
            connect: Some(net_connect),
            bind: Some(net_bind),
            listen: Some(net_listen),
            accept: Some(net_accept),
            send: Some(net_send),
            recv: Some(net_recv),
            shutdown: Some(net_shutdown),
            getsockname: Some(net_getsockname),
            getpeername: Some(net_getpeername),
            getsockopt: Some(net_getsockopt),
            setsockopt: Some(net_setsockopt),
            poll: Some(net_poll),
            // **`CP-38`**：Windows 没有 `socketpair(2)` ⇒ loopback TCP 对模拟（与参照实现同法）。
            socketpair: Some(net_socketpair),
            close: Some(net_close),
        }
    }

    /// 句柄 ⇒ 原始 `SOCKET`；**已关／未知 ⇒ `None`**（`CP-18` 的 `EBADF` 形状由调用方给）。
    fn fd_of(&self, handle: SocketHandle) -> Option<usize> {
        self.sockets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&handle.0)
            .copied()
    }

    /// 收下一枚新 `SOCKET`，返回不透明句柄（`CP-31` 的句柄号段见模块文档）。
    fn insert(&self, socket: usize) -> SocketHandle {
        let handle = NEXT_SOCKET_HANDLE.fetch_add(1, Ordering::Relaxed);
        self.sockets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(handle, socket);
        raw_sockets()
            .get_or_insert_with(HashMap::new)
            .insert(handle, socket);
        SocketHandle(handle)
    }
}

impl Default for WindowsNet {
    fn default() -> Self {
        Self::new()
    }
}

// --------------------------------------------------------------------------- //
// 槽位实现（逐条对齐 net_posix.rs）
// --------------------------------------------------------------------------- //

/// `socket(af, type, proto)`：只接受 `AF_INET` ＋ `SOCK_STREAM`（v1 的 TCP 面）。
///
/// 其它族／类型**如实拒绝**（`EAFNOSUPPORT`／`EPROTONOSUPPORT`），不假装成功（`CP-6`）。
#[allow(unsafe_code)]
extern "C" fn net_socket(
    state: *mut c_void,
    af: i32,
    ty: i32,
    proto: i32,
    out: *mut SocketHandle,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    if af != family::AF_INET {
        write_errno(errno_out, errno_named("EAFNOSUPPORT"));
        return CapStatus::Machine;
    }
    if ty != socket_type::SOCK_STREAM {
        write_errno(errno_out, errno_named("EPROTONOSUPPORT"));
        return CapStatus::Machine;
    }
    // `AF_INET`／`SOCK_STREAM` 的数值与 Winsock 同值（2／1）⇒ 原样转发；`proto` 交给内核判定。
    // **WSA_FLAG_OVERLAPPED**：AcceptEx/ConnectEx 要求 overlapped socket
    // （CPython 的 socket.socket 也这样建）。普通阻塞 I/O 不受影响。
    // SAFETY: 三个整数＋空协议信息转发给 Winsock。
    let raw = unsafe {
        sys::WSASocketW(af, ty, proto, core::ptr::null_mut(), 0, 1)
    };
    if raw == INVALID_SOCKET {
        write_errno(errno_out, last_wsa_errno());
        return CapStatus::Machine;
    }
    // SAFETY: provider 状态由 `vtable()` 的 `state` 给出，且比 vtable 活得久。
    let provider = unsafe { &*(state as *const WindowsNet) };
    write_out(out, provider.insert(raw));
    CapStatus::Ok
}

/// `bind(sock, af, addr)`：`AF_INET` 的 `CP-39` 线格式 ⇒ 真 `bind`。
#[allow(unsafe_code)]
extern "C" fn net_bind(
    state: *mut c_void,
    socket: SocketHandle,
    af: i32,
    addr: *const u8,
    addr_len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    let sockaddr = match sockaddr_in_of(af, addr, addr_len) {
        Ok(value) => value,
        Err(errno) => {
            write_errno(errno_out, errno);
            return CapStatus::Machine;
        }
    };
    // SAFETY: `sockaddr` 是有效的 WinSock `SOCKADDR_IN`（16B），在调用期间活着。
    let result = unsafe {
        sys::bind(
            fd,
            (&sockaddr as *const SockaddrIn).cast::<u8>(),
            SOCKADDR_IN_LEN,
        )
    };
    if result == 0 {
        CapStatus::Ok
    } else {
        write_errno(errno_out, last_wsa_errno());
        CapStatus::Machine
    }
}

/// `connect(sock, af, addr)`：`AF_INET` 的 `CP-39` 线格式 ⇒ 真 `connect`。
#[allow(unsafe_code)]
extern "C" fn net_connect(
    state: *mut c_void,
    socket: SocketHandle,
    af: i32,
    addr: *const u8,
    addr_len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    let sockaddr = match sockaddr_in_of(af, addr, addr_len) {
        Ok(value) => value,
        Err(errno) => {
            write_errno(errno_out, errno);
            return CapStatus::Machine;
        }
    };
    loop {
        // SAFETY: 同 `net_bind`；`EINTR` 重试口径同 `net_posix`。
        let result = unsafe {
            sys::connect(
                fd,
                (&sockaddr as *const SockaddrIn).cast::<u8>(),
                SOCKADDR_IN_LEN,
            )
        };
        if result == 0 {
            return CapStatus::Ok;
        }
        let errno = last_wsa_errno();
        if is_eintr(errno) {
            continue;
        }
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
}

/// `listen(sock, backlog)`。
#[allow(unsafe_code)]
extern "C" fn net_listen(
    state: *mut c_void,
    socket: SocketHandle,
    backlog: i32,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    // SAFETY: `fd` 来自句柄表；`backlog` 原样转发（负值在 WinSock 上按默认处理）。
    let result = unsafe { sys::listen(fd, backlog) };
    if result == 0 {
        CapStatus::Ok
    } else {
        write_errno(errno_out, last_wsa_errno());
        CapStatus::Machine
    }
}

/// `accept(sock)` ⇒ 新 socket 能力 ＋ 地址（**两段式**出参，同 `net_posix`）。
#[allow(unsafe_code)]
extern "C" fn net_accept(
    state: *mut c_void,
    socket: SocketHandle,
    out_socket: *mut SocketHandle,
    out_family: *mut i32,
    out_addr: *mut u8,
    out_capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    let mut sockaddr = SockaddrIn {
        family: 0,
        port: 0,
        addr: [0; 4],
        zero: [0; 8],
    };
    let mut length = SOCKADDR_IN_LEN;
    loop {
        // SAFETY: `sockaddr`／`length` 是本函数持有的可写出参（尺寸与 WinSock 一致）。
        let raw = unsafe {
            sys::accept(
                fd,
                (&mut sockaddr as *mut SockaddrIn).cast::<u8>(),
                &mut length,
            )
        };
        if raw == INVALID_SOCKET {
            let errno = last_wsa_errno();
            if is_eintr(errno) {
                continue;
            }
            write_errno(errno_out, errno);
            return CapStatus::Machine;
        }
        let handle = provider.insert(raw);
        write_out(out_socket, handle);
        write_out(out_family, family::AF_INET);
        return write_sockaddr_out(&sockaddr, out_addr, out_capacity, out_len);
    }
}

/// `getsockname(sock)` ⇒ 本地地址（**两段式**出参）。
#[allow(unsafe_code)]
extern "C" fn net_getsockname(
    state: *mut c_void,
    socket: SocketHandle,
    out_family: *mut i32,
    out_addr: *mut u8,
    out_capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    let mut sockaddr = SockaddrIn {
        family: 0,
        port: 0,
        addr: [0; 4],
        zero: [0; 8],
    };
    let mut length = SOCKADDR_IN_LEN;
    // SAFETY: `sockaddr`／`length` 是本函数持有的可写出参。
    let result = unsafe {
        sys::getsockname(
            fd,
            (&mut sockaddr as *mut SockaddrIn).cast::<u8>(),
            &mut length,
        )
    };
    if result != 0 {
        write_errno(errno_out, last_wsa_errno());
        return CapStatus::Machine;
    }
    write_out(out_family, family::AF_INET);
    write_sockaddr_out(&sockaddr, out_addr, out_capacity, out_len)
}

/// `getpeername(sock)` ⇒ 对端地址（**两段式**出参）。
#[allow(unsafe_code)]
extern "C" fn net_getpeername(
    state: *mut c_void,
    socket: SocketHandle,
    out_family: *mut i32,
    out_addr: *mut u8,
    out_capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    let mut sockaddr = SockaddrIn {
        family: 0,
        port: 0,
        addr: [0; 4],
        zero: [0; 8],
    };
    let mut length = SOCKADDR_IN_LEN;
    // SAFETY: `sockaddr`／`length` 是本函数持有的可写出参。
    let result = unsafe {
        sys::getpeername(
            fd,
            (&mut sockaddr as *mut SockaddrIn).cast::<u8>(),
            &mut length,
        )
    };
    if result != 0 {
        write_errno(errno_out, last_wsa_errno());
        return CapStatus::Machine;
    }
    write_out(out_family, family::AF_INET);
    write_sockaddr_out(&sockaddr, out_addr, out_capacity, out_len)
}

/// `shutdown(sock, how)`：`SHUT_RD=0`／`SHUT_WR=1`／`SHUT_RDWR=2` 与 WinSock
/// `SD_RECEIVE`／`SD_SEND`／`SD_BOTH` **同值** ⇒ 原样转发。
#[allow(unsafe_code)]
extern "C" fn net_shutdown(
    state: *mut c_void,
    socket: SocketHandle,
    how: i32,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    // SAFETY: `fd` 来自句柄表；`how` 两套 ABI 同值，原样转发。
    let result = unsafe { sys::shutdown(fd, how) };
    if result == 0 {
        CapStatus::Ok
    } else {
        write_errno(errno_out, last_wsa_errno());
        CapStatus::Machine
    }
}

/// `send(sock, buf)` ⇒ 实际发出的字节数。
#[allow(unsafe_code)]
extern "C" fn net_send(
    state: *mut c_void,
    socket: SocketHandle,
    buffer: *const u8,
    len: usize,
    out_sent: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    if buffer.is_null() && len > 0 {
        write_errno(errno_out, errno_named("EFAULT"));
        return CapStatus::Machine;
    }
    // WinSock 的 `send` 长度是 `int`；超过 `i32::MAX` 不可能一次发完 ⇒ 如实拒绝（`EINVAL`）。
    let Ok(length) = i32::try_from(len) else {
        write_errno(errno_out, errno_named("EINVAL"));
        return CapStatus::Machine;
    };
    loop {
        // SAFETY: 调用方按 vtable 契约给出 `buffer`／`len`（`len == 0` 时指针可为任意值，
        // 上面已在 `len > 0` 时判空）；缓冲在调用期间有效。
        let written = unsafe { sys::send(fd, buffer, length, 0) };
        if written < 0 {
            let errno = last_wsa_errno();
            if is_eintr(errno) {
                continue;
            }
            write_errno(errno_out, errno);
            return CapStatus::Machine;
        }
        write_out(out_sent, written as usize);
        return CapStatus::Ok;
    }
}

/// `recv(sock, n)` ⇒ 实际收到的字节数。
#[allow(unsafe_code)]
extern "C" fn net_recv(
    state: *mut c_void,
    socket: SocketHandle,
    buffer: *mut u8,
    capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    if capacity == 0 {
        write_out(out_len, 0);
        return CapStatus::Ok;
    }
    if buffer.is_null() {
        write_errno(errno_out, errno_named("EFAULT"));
        return CapStatus::Machine;
    }
    let Ok(length) = i32::try_from(capacity) else {
        write_errno(errno_out, errno_named("EINVAL"));
        return CapStatus::Machine;
    };
    loop {
        // SAFETY: 调用方按 vtable 契约给出可写 `buffer`／`capacity`（上面已判空）。
        let received = unsafe { sys::recv(fd, buffer, length, 0) };
        if received < 0 {
            let errno = last_wsa_errno();
            if is_eintr(errno) {
                continue;
            }
            write_errno(errno_out, errno);
            return CapStatus::Machine;
        }
        write_out(out_len, received as usize);
        return CapStatus::Ok;
    }
}

/// `setsockopt(sock, level, name, value)`：POSIX 选项号 ⇒ Winsock 选项号（见模块文档）。
///
/// 超时选项按 Win32 `DWORD` 毫秒位传值；其余值按裸字节转发（布尔/整数选项由 Winsock 按 `int` 解释）。
#[allow(unsafe_code)]
extern "C" fn net_setsockopt(
    state: *mut c_void,
    socket: SocketHandle,
    level: i32,
    name: i32,
    value: *const u8,
    value_len: usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    let Some((win_level, win_name)) = wsa_socket_option(level, name) else {
        // 不认识的选项**不原样转发**（两套编号不同，转错会静默设错选项）⇒ 如实拒绝。
        write_errno(errno_out, errno_named("ENOPROTOOPT"));
        return CapStatus::Machine;
    };
    if value.is_null() && value_len > 0 {
        write_errno(errno_out, errno_named("EFAULT"));
        return CapStatus::Machine;
    }
    // SAFETY: 调用方按 vtable 契约给出 `value`／`value_len`（上面在长度非零时判空）。
    let bytes = if value_len == 0 {
        &[][..]
    } else {
        unsafe { core::slice::from_raw_parts(value, value_len) }
    };
    let result = if name == SO_RCVTIMEO_POSIX || name == SO_SNDTIMEO_POSIX {
        // 超时：guest 的 int 毫秒 ⇒ Win32 `DWORD` 毫秒（负值夹 0＝不设超时）。
        if bytes.len() < 4 {
            write_errno(errno_out, errno_named("EINVAL"));
            return CapStatus::Machine;
        }
        let millis = i32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).max(0) as u32;
        // SAFETY: `&millis` 是本函数持有的 4 字节 `DWORD` 缓冲，调用期间活着。
        unsafe {
            sys::setsockopt(
                fd,
                win_level,
                win_name,
                (&millis as *const u32).cast::<u8>(),
                4,
            )
        }
    } else {
        let Ok(optlen) = i32::try_from(bytes.len()) else {
            write_errno(errno_out, errno_named("EINVAL"));
            return CapStatus::Machine;
        };
        // SAFETY: `bytes` 是调用方给的裸字节缓冲，调用期间有效（空切片指针不会被解引用）。
        unsafe { sys::setsockopt(fd, win_level, win_name, bytes.as_ptr(), optlen) }
    };
    if result == 0 {
        CapStatus::Ok
    } else {
        write_errno(errno_out, last_wsa_errno());
        CapStatus::Machine
    }
}

/// `getsockopt(sock, level, name)` ⇒ 值字节（**两段式**出参：`out_len` 回总长、只写
/// `min(总长, 容量)` 字节）。
///
/// `SO_ERROR` 的值是 Winsock 错误码 ⇒ 翻成 POSIX `errno` 数值，与 provider 其它错误同一口径；
/// `SO_TYPE`／缓冲区尺寸／超时两枚的数值语义两套 ABI 一致 ⇒ 原样回。
#[allow(unsafe_code)]
extern "C" fn net_getsockopt(
    state: *mut c_void,
    socket: SocketHandle,
    level: i32,
    name: i32,
    out: *mut u8,
    capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let Some(fd) = provider.fd_of(socket) else {
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    let Some((win_level, win_name)) = wsa_socket_option(level, name) else {
        write_errno(errno_out, errno_named("ENOPROTOOPT"));
        return CapStatus::Machine;
    };
    // 16 字节足够容纳本片接的所有选项（int／DWORD）。
    let mut value = [0u8; 16];
    let mut value_len: i32 = value.len() as i32;
    // SAFETY: `value`／`value_len` 是本函数持有的可写出参；Winsock 最多写 16 字节。
    let result = unsafe {
        sys::getsockopt(
            fd,
            win_level,
            win_name,
            value.as_mut_ptr(),
            &mut value_len,
        )
    };
    if result != 0 {
        write_errno(errno_out, last_wsa_errno());
        return CapStatus::Machine;
    }
    let total = usize::try_from(value_len)
        .unwrap_or(0)
        .min(value.len());
    if name == SO_ERROR_POSIX && total >= 4 {
        let code = i32::from_ne_bytes([value[0], value[1], value[2], value[3]]);
        let errno = if code == 0 { 0 } else { wsa_errno(code) };
        value[..4].copy_from_slice(&errno.to_ne_bytes());
    }
    // **两段式出参**：只写 `min(总长, 容量)`，`out_len` 回总长（同 `fs` 的 `readlink`／`listdir`）。
    let written = total.min(capacity);
    if !out.is_null() && written > 0 {
        // SAFETY: `out` 由调用方按 `capacity` 给出；源（局部数组）与目标不重叠。
        unsafe { core::ptr::copy_nonoverlapping(value.as_ptr(), out, written) };
    }
    write_out(out_len, total);
    CapStatus::Ok
}

/// `poll(fds, timeout_ns)`：真 `WSAPoll` ＋ Linux ↔ Winsock 位集翻译（见模块文档）。
///
/// 未知／已关句柄 ⇒ `EBADF`（`CP-18`，同 `net_posix`）；出参**两段式**：`out_len` 回就绪项总数，
/// 只写 `min(总数, 容量)` 项（输入序）。
#[allow(unsafe_code)]
extern "C" fn net_poll(
    state: *mut c_void,
    fds: *const PollFd,
    count: usize,
    timeout_ns: i64,
    out: *mut PollFd,
    out_capacity: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if let Err(errno) = wsa_startup() {
        write_errno(errno_out, errno);
        return CapStatus::Machine;
    }
    // SAFETY: provider 状态由 `vtable()` 的 `state` 给出，且比 vtable 活得久。
    let provider = unsafe { &*(state as *const WindowsNet) };
    if count > 0 && fds.is_null() {
        write_errno(errno_out, errno_named("EFAULT"));
        return CapStatus::Machine;
    }
    // SAFETY: 调用方按 vtable 契约给出 `count` 项的只读 `fds`（上面已判空）。
    let entries: &[PollFd] = if count == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(fds, count) }
    };
    let mut kernel: Vec<WsaPollFd> = Vec::with_capacity(count);
    for entry in entries {
        let Some(fd) = provider.fd_of(entry.handle) else {
            write_errno(errno_out, errno_named("EBADF"));
            return CapStatus::Machine;
        };
        kernel.push(WsaPollFd {
            fd,
            events: wsa_events(entry.events),
            revents: 0,
        });
    }
    if count == 0 {
        // `WSAPoll` 要求至少一个有效 socket（空数组会 `WSAEINVAL`）⇒ 自己做等价睡眠。
        sleep_without_sockets(timeout_ns);
        write_out(out_len, 0);
        return CapStatus::Ok;
    }
    // 截止时刻只在"有限等待"时存在；`EINTR` 重试时按它重算剩余时间（同 `net_posix`）。
    let deadline = (timeout_ns > 0).then(|| {
        std::time::Instant::now() + std::time::Duration::from_nanos(timeout_ns as u64)
    });
    let mut timeout = timeout_ns_to_ms(timeout_ns);
    loop {
        // SAFETY: `kernel` 是长度 `count` 的可写 `WSAPOLLFD` 数组（布局与 WinSock 一致）；
        // 其中 fd 都来自本 provider 的句柄表。
        let ready = unsafe { sys::WSAPoll(kernel.as_mut_ptr(), kernel.len() as u32, timeout) };
        if ready < 0 {
            let errno = last_wsa_errno();
            if is_eintr(errno) {
                if let Some(deadline) = deadline {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    let remaining_ns = remaining.as_nanos().min(i64::MAX as u128) as i64;
                    timeout = timeout_ns_to_ms(remaining_ns);
                }
                continue;
            }
            write_errno(errno_out, errno);
            return CapStatus::Machine;
        }
        break;
    }
    // `revents` 是 Winsock 位 ⇒ 翻回 Linux 位集；两段式写回（输入序）。
    let mut total = 0usize;
    for entry in kernel.iter_mut() {
        let translated = linux_events(entry.revents);
        entry.revents = translated as i16;
        if translated != 0 {
            total += 1;
        }
    }
    write_out(out_len, total);
    let writable = total.min(out_capacity);
    if writable > 0 {
        // SAFETY: 调用方按 vtable 契约给出至少 `out_capacity` 项的可写 `out`。
        let out_entries = unsafe { core::slice::from_raw_parts_mut(out, writable) };
        let mut written = 0usize;
        for (index, entry) in kernel.iter().enumerate() {
            if entry.revents == 0 {
                continue;
            }
            out_entries[written] = PollFd {
                handle: entries[index].handle,
                events: entries[index].events,
                revents: i32::from(entry.revents),
            };
            written += 1;
            if written >= writable {
                break;
            }
        }
    }
    CapStatus::Ok
}

/// `close(handle)`：从表里取走并 `closesocket`；**已关／未知 ⇒ `Machine` ＋ `EBADF`**（`CP-18`）。
#[allow(unsafe_code)]
/// **`socketpair`**（`CP-38`；`PLAN §12.20` W4 补齐）：Windows 没有 `socketpair(2)` ⇒
/// **loopback TCP 对**模拟（与 CPython 在同一平台的做法同法：只支持 `AF_INET` ＋ `SOCK_STREAM`，
/// 其它族／类型按真实机器语义回 `EAFNOSUPPORT`／`EPROTONOSUPPORT`）。
///
/// 做法：监听 `127.0.0.1:0` → 客户端连接 → accept 出另一端 → 关监听。两端都是**真** Winsock
/// socket（`getsockname`／`select`／`send`／`recv` 全走既有 provider 路径）。
#[allow(unsafe_code)]
extern "C" fn net_socketpair(
    state: *mut c_void,
    af: i32,
    kind: i32,
    protocol: i32,
    first: *mut SocketHandle,
    second: *mut SocketHandle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同其它槽位（provider 状态由 vtable 的 `state` 给出）。
    let provider = unsafe { &*(state as *const WindowsNet) };
    if first.is_null() || second.is_null() {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = errno_named("EFAULT") };
        return CapStatus::Machine;
    }
    if af != family::AF_INET {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = errno_named("EAFNOSUPPORT") };
        return CapStatus::Machine;
    }
    if kind != socket_type::SOCK_STREAM {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = errno_named("EPROTONOSUPPORT") };
        return CapStatus::Machine;
    }
    let _ = protocol; // TCP 的 protocol＝0／IPPROTO_TCP；Winsock 接受 0
    if let Err(errno) = wsa_startup() {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = errno };
        return CapStatus::Machine;
    }
    let mut created: Vec<usize> = Vec::new();
    let fail = |code: i32, created: &mut Vec<usize>, errno_out: *mut i32| -> CapStatus {
        for socket in created.drain(..) {
            // SAFETY: 这些 socket 都是本函数创建、尚未交出。
            unsafe { sys::closesocket(socket) };
        }
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = wsa_errno(code) };
        CapStatus::Machine
    };
    // SAFETY: `WSASocketW(..., WSA_FLAG_OVERLAPPED)`。
    let listener = unsafe {
        sys::WSASocketW(family::AF_INET, kind, 0, core::ptr::null_mut(), 0, 1)
    };
    if listener == INVALID_SOCKET {
        // SAFETY: 无创建的 socket 可回收。
        return fail(unsafe { sys::WSAGetLastError() }, &mut created, errno_out);
    }
    created.push(listener);
    let mut address = SockaddrIn {
        family: family::AF_INET as u16,
        port: port_to_net(0),
        addr: [127, 0, 0, 1],
        zero: [0; 8],
    };
    // SAFETY: `address` 是 16 字节 `SOCKADDR_IN`；监听失败按机器错误回收。
    let bound = unsafe {
        sys::bind(listener, (&raw const address).cast::<u8>(), SOCKADDR_IN_LEN)
    };
    if bound != 0 {
        // SAFETY: 取得本线程最近错误码。
        return fail(unsafe { sys::WSAGetLastError() }, &mut created, errno_out);
    }
    // SAFETY: 同上。
    let listening = unsafe { sys::listen(listener, 1) };
    if listening != 0 {
        // SAFETY: 取得本线程最近错误码。
        return fail(unsafe { sys::WSAGetLastError() }, &mut created, errno_out);
    }
    // **读回内核分配的端口**（`getsockname`）。
    let mut length = SOCKADDR_IN_LEN;
    // SAFETY: `address` 是活局部量、长度如实给出。
    let named = unsafe {
        sys::getsockname(listener, (&raw mut address).cast::<u8>(), &mut length)
    };
    if named != 0 {
        // SAFETY: 取得本线程最近错误码。
        return fail(unsafe { sys::WSAGetLastError() }, &mut created, errno_out);
    }
    // SAFETY: `socket(family=AF_INET, type=SOCK_STREAM, proto=0)`。
    let client = unsafe { sys::socket(family::AF_INET, kind, 0) };
    if client == INVALID_SOCKET {
        // SAFETY: 取得本线程最近错误码。
        return fail(unsafe { sys::WSAGetLastError() }, &mut created, errno_out);
    }
    created.push(client);
    // SAFETY: `address` 是 16 字节 `SOCKADDR_IN`（刚由 `getsockname` 填好端口）。
    let connected = unsafe {
        sys::connect(client, (&raw const address).cast::<u8>(), SOCKADDR_IN_LEN)
    };
    if connected != 0 {
        // SAFETY: 取得本线程最近错误码。
        return fail(unsafe { sys::WSAGetLastError() }, &mut created, errno_out);
    }
    let mut peer_length = SOCKADDR_IN_LEN;
    let mut peer = SockaddrIn {
        family: family::AF_INET as u16,
        port: 0,
        addr: [0; 4],
        zero: [0; 8],
    };
    // SAFETY: 出参是活局部量、长度如实给出。
    let accepted = unsafe {
        sys::accept(
            listener,
            (&raw mut peer).cast::<u8>(),
            &mut peer_length,
        )
    };
    if accepted == INVALID_SOCKET {
        // SAFETY: 取得本线程最近错误码。
        return fail(unsafe { sys::WSAGetLastError() }, &mut created, errno_out);
    }
    created.push(accepted);
    // 监听 socket 不再需要：先关掉（不在句柄表里）。
    // SAFETY: `listener` 是本函数创建、尚未交出的 socket。
    unsafe { sys::closesocket(listener) };
    let _ = created.pop(); // 同上，已手动关闭
    let first_handle = provider.insert(client);
    let second_handle = provider.insert(accepted);
    // SAFETY: 出参由调用方提供。
    unsafe {
        *first = first_handle;
        *second = second_handle;
    }
    CapStatus::Ok
}

#[allow(unsafe_code)]
extern "C" fn net_close(
    state: *mut c_void,
    socket: SocketHandle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `net_socket`。
    let provider = unsafe { &*(state as *const WindowsNet) };
    let taken = provider
        .sockets
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&socket.0);
    let Some(raw) = taken else {
        // `CP-18`：已关／未知句柄 ⇒ 机器错误（见模块文档第 2 条：与 `net_posix` 的幂等 `Ok` 不同，
        // 幂等由 VM 实例能力表的释放路径保证）。
        write_errno(errno_out, errno_named("EBADF"));
        return CapStatus::Machine;
    };
    if let Some(map) = raw_sockets().as_mut() {
        map.remove(&socket.0);
    }
    // SAFETY: `raw` 由句柄表独占持有，取出后立即关闭（不会二次 close）。
    let result = unsafe { sys::closesocket(raw) };
    if result != 0 {
        write_errno(errno_out, last_wsa_errno());
        return CapStatus::Machine;
    }
    CapStatus::Ok
}
