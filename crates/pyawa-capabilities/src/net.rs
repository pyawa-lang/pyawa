//! `net` 域的形状 —— `SPEC-capabilities.md` §9.2（`CP-`），含 `socketpair` 槽（`CP-38`）。
//!
//! 本文件**只含类型与函数指针**（`CP-12`），真实机器实现在 `pyawa-runtime`（`DESIGN.md` §7 的平台集中点）。
//! 对应 C 层模块：`_socket`、`select`（§4 表）。
//!
//! 遵循的条文与 `fs`／`clock` 域同一套：
//! - **`CP-2`**：vtable 指针为 `null` ⇒ **整域**未实现（禁止在建实例时拒绝）。
//! - **`CP-3`**：槽位为 `None` ⇒ **只有该操作**未实现，同域其余槽位不受影响。
//! - **`CP-5`**：每次调用**必须**返回三种结果之一 —— 成功／**机器错误**／**未实现**；
//!   **禁止**把"未实现"编码成某个 `errno`，也**禁止**两者共用一条通道。
//! - **`CP-9`**：调用同步；可中断经取消令牌（`CP-26`），**禁止**改成异步回调式。
//! - **`CP-10`**／**`CP-26`**：接口面只允许裸数据与不透明句柄，**禁止**泛型／VM 对象。
//! - **`CP-30`**／**`AB-39`**：vtable 带 `version`／`size` 字段；**版本号就是 `PA_ABI_VERSION`**
//!   （三份契约共用）。本 crate 无依赖（`CP-12`）⇒ 这里**不定义第二份版本号**：
//!   provider 填 `pyawa_abi::PA_ABI_VERSION`，本文件的 [`CpNetVtable::UNIMPLEMENTED`] 填 `0`
//!   （"未填"哨兵，与 [`super::fs::CpFsVtable::UNIMPLEMENTED`] 同一手法）。
//! - **`AB-14`**：跨边界一律**不透明句柄** —— 本文件只给 [`SocketHandle`] 包装，不暴露内部布局。
//! - **`CP-31`**：`fd` ＝ 实例能力表下标；v1 的 provider 只给不透明句柄（编号由 provider 自管），
//!   "统一编号的实例能力表"在 VM 侧适配层落地（`crates/pyawa-core/src/instance/net.rs`）。
//! - **`CP-38`**：`socketpair` 槽 —— 返回**一对** socket 能力，local-only、**无地址**，
//!   因此**不适用** §9.2 的地址线格式。
//! - **`CP-39`**：`sockaddr` 的 v1 线格式 —— "地址族编号 ＋ 裸字节"；IPv4 ＝ 4 字节地址 ＋
//!   2 字节端口、**大端**；v1 只 `AF_INET`／`AF_UNIX`；IPv6／其它族与 DNS **不在 v1**。
//!
//! **本片（`P3-29` 首片）落地范围**：`socketpair`／`send`／`recv`／`close` 四槽由
//! `pyawa-runtime::net_posix` 提供真实现；`socket`／`connect`／`bind`／`listen`／`accept`／
//! `shutdown`／`getsockname`／`getpeername`／`getsockopt`／`setsockopt`／`poll` 的**形状**
//! 在此登记但 provider 一律 `None`（`CP-3`：调用时才是"未实现"，**不假装成功**）。
//! 追加槽位只能从表尾追加（`AB-44`）。

use core::ffi::c_void;

pub use super::fs::CapStatus;

/// socket 域的**不透明句柄**（`AB-14`：禁止暴露内部布局）。
///
/// 与 [`super::fs::Handle`] 同一手法；`Handle` 与 `SocketHandle` 分开是为了**不让编译期**
/// 把文件句柄与 socket 句柄混用（两者在 Python 层都是 `int` `fd`，但接口层仍是两种能力）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SocketHandle(pub u64);

/// **`CP-39`**：v1 支持的地址族（只有这两族，IPv6 及其它族**不在 v1**）。
pub mod family {
    /// `AF_UNIX`（Linux 取值）。
    pub const AF_UNIX: i32 = 1;
    /// `AF_INET`（Linux 取值）。
    pub const AF_INET: i32 = 2;
}

/// socket 类型（`socket(2)` 的 `type`；v1 首片主要用 `SOCK_STREAM`）。
///
/// 数值取 Linux／POSIX 的公共取值（`AF_UNIX` 上内核也接受 `SOCK_DGRAM`／`SOCK_SEQPACKET`；
/// **能不能用由 provider 转发给内核判定**，本层不预先限制 —— `CP-6`：拒绝要给真实机器错误）。
pub mod socket_type {
    pub const SOCK_STREAM: i32 = 1;
    pub const SOCK_DGRAM: i32 = 2;
    pub const SOCK_RAW: i32 = 3;
    pub const SOCK_SEQPACKET: i32 = 5;
}

/// `shutdown(sock, how)` 的 `how`（`SPEC-capabilities.md` §9.2）。
pub mod shutdown_how {
    pub const SHUT_RD: i32 = 0;
    pub const SHUT_WR: i32 = 1;
    pub const SHUT_RDWR: i32 = 2;
}

/// **`CP-39`**：v1 的 IPv4 地址线长度＝4 字节地址 ＋ 2 字节端口（**大端**）。
pub const SOCKADDR_V1_IPV4_LEN: usize = 6;

/// **`CP-39`**：把 IPv4 地址编码成 v1 线格式（4B 地址 ＋ 2B 端口，大端）。
///
/// 纯格式编解码，不是机器实现（不碰平台）；解析责任仍在 VM 侧（`CP-21`／`CP-32`）。
pub fn encode_ipv4(addr: [u8; 4], port: u16) -> [u8; SOCKADDR_V1_IPV4_LEN] {
    let [high, low] = port.to_be_bytes();
    [addr[0], addr[1], addr[2], addr[3], high, low]
}

/// **`CP-39`**：把 v1 的 IPv4 线格式解回 `(地址, 端口)`；长度不符返回 `None`。
pub fn decode_ipv4(bytes: &[u8]) -> Option<([u8; 4], u16)> {
    if bytes.len() != SOCKADDR_V1_IPV4_LEN {
        return None;
    }
    let mut addr = [0u8; 4];
    addr.copy_from_slice(&bytes[..4]);
    let port = u16::from_be_bytes([bytes[4], bytes[5]]);
    Some((addr, port))
}

/// `poll(fds, timeout_ns)` 的**一项**：进参给 `events`，出参再由 `revents` 带回
/// （`SPEC-capabilities.md` §9.2 的"就绪集合"）。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PollFd {
    /// 要观察的 socket 能力。
    pub handle: SocketHandle,
    /// 关心的就绪事件（`poll(2)` 的位集，取值由 `_socket`／`select` 映射）。
    pub events: i32,
    /// 实际发生的事件（**出参**；`Ok` 时有效）。
    pub revents: i32,
}

/// `net` 域的 vtable（`SPEC-capabilities.md` §3 的 `CpNetVtable`）。
///
/// 槽位名与语义逐条对应 §9.2；`None` 即"该操作未实现"（`CP-3`）。参数摆放沿用
/// "`state` 在前、出参在尾、机器错误走 `errno_out`"的既有约定（与 `fs` 域同款）。
///
/// `state` 是 **provider 的实例状态**（不透明指针；每个槽位把它作为第一个参数收回），
/// 于是"函数指针"仍满足 `CP-12`（本 crate 只含类型与函数指针）。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CpNetVtable {
    /// `CP-30`／`AB-39`：版本字段 —— provider 填 `PA_ABI_VERSION`（三份契约共用的那个号）。
    pub version: u32,
    /// 本结构体的字节数（`AB-43` 的有界读取精神）。
    pub size: u32,
    /// provider 实例状态（不透明）。
    pub state: *mut c_void,

    /// `socket(family, type, proto)` ⇒ socket 能力。
    pub socket: Option<
        extern "C" fn(*mut c_void, i32, i32, i32, *mut SocketHandle, *mut i32) -> CapStatus,
    >,
    /// `connect(sock, addr)`：地址是"地址族编号 ＋ 裸字节"（`CP-39`）。
    pub connect: Option<
        extern "C" fn(*mut c_void, SocketHandle, i32, *const u8, usize, *mut i32) -> CapStatus,
    >,
    /// `bind(sock, addr)`：地址格式同上。
    pub bind: Option<
        extern "C" fn(*mut c_void, SocketHandle, i32, *const u8, usize, *mut i32) -> CapStatus,
    >,
    /// `listen(sock, backlog)`。
    pub listen: Option<extern "C" fn(*mut c_void, SocketHandle, i32, *mut i32) -> CapStatus>,
    /// `accept(sock)` ⇒（socket 能力，地址）。地址出参两段式：`out_len` 回**总字节数**，
    /// provider 只写 `min(总长, 容量)`；调用方见总长超容量就扩缓冲重试（同 `fs` 的 `readlink`）。
    pub accept: Option<
        extern "C" fn(
            *mut c_void,
            SocketHandle,
            *mut SocketHandle,
            *mut i32,
            *mut u8,
            usize,
            *mut usize,
            *mut i32,
        ) -> CapStatus,
    >,
    /// `send(sock, buf)` ⇒ 实际发出的字节数经 `out_sent` 带回。
    pub send: Option<
        extern "C" fn(*mut c_void, SocketHandle, *const u8, usize, *mut usize, *mut i32) -> CapStatus,
    >,
    /// `recv(sock, n)` ⇒ 实际收到的字节数经 `out_len` 带回（缓冲由调用方给）。
    pub recv: Option<
        extern "C" fn(*mut c_void, SocketHandle, *mut u8, usize, *mut usize, *mut i32) -> CapStatus,
    >,
    /// `shutdown(sock, how)`。
    pub shutdown: Option<extern "C" fn(*mut c_void, SocketHandle, i32, *mut i32) -> CapStatus>,
    /// `getsockname(sock)` ⇒ 地址（两段式出参，同 `accept`）。
    pub getsockname: Option<
        extern "C" fn(
            *mut c_void,
            SocketHandle,
            *mut i32,
            *mut u8,
            usize,
            *mut usize,
            *mut i32,
        ) -> CapStatus,
    >,
    /// `getpeername(sock)` ⇒ 地址（两段式出参，同 `accept`）。
    pub getpeername: Option<
        extern "C" fn(
            *mut c_void,
            SocketHandle,
            *mut i32,
            *mut u8,
            usize,
            *mut usize,
            *mut i32,
        ) -> CapStatus,
    >,
    /// `getsockopt(sock, level, name)` ⇒ 值字节（两段式出参，同 `accept`）。
    pub getsockopt: Option<
        extern "C" fn(
            *mut c_void,
            SocketHandle,
            i32,
            i32,
            *mut u8,
            usize,
            *mut usize,
            *mut i32,
        ) -> CapStatus,
    >,
    /// `setsockopt(sock, level, name, value)`。
    pub setsockopt: Option<
        extern "C" fn(*mut c_void, SocketHandle, i32, i32, *const u8, usize, *mut i32) -> CapStatus,
    >,
    /// `poll(fds, timeout_ns)` ⇒ 就绪集合。出参两段式：`out_len` 回**就绪项总数**，
    /// provider 只写 `min(总数, 容量)`（同 `fs` 的 `listdir` 约定）。
    pub poll: Option<
        extern "C" fn(
            *mut c_void,
            *const PollFd,
            usize,
            i64,
            *mut PollFd,
            usize,
            *mut usize,
            *mut i32,
        ) -> CapStatus,
    >,
    /// **`CP-38`**：`socketpair(family, type, proto)` ⇒ **一对** socket 能力（local-only，**无地址**）。
    ///
    /// 两个出参都是新分配的句柄（**两个都要**写进实例能力表，各占一个 `fd`）；
    /// 失败时必须两者都不写（调用方按 `Machine` 处理）。
    pub socketpair: Option<
        extern "C" fn(
            *mut c_void,
            i32,
            i32,
            i32,
            *mut SocketHandle,
            *mut SocketHandle,
            *mut i32,
        ) -> CapStatus,
    >,
    /// `close(handle)`（`CP-17`：释放；**必须幂等**）。
    pub close: Option<extern "C" fn(*mut c_void, SocketHandle, *mut i32) -> CapStatus>,
}

impl CpNetVtable {
    /// 一个**整域未实现**的 vtable（所有槽位为 `None`；`CP-2`／`CP-3`）。
    ///
    /// `version: 0` ＝ "未填"哨兵（同 [`super::fs::CpFsVtable::UNIMPLEMENTED`]）——
    /// 真 provider **必须**填 `pyawa_abi::PA_ABI_VERSION`（`CP-30`／`AB-39`）。
    pub const UNIMPLEMENTED: Self = Self {
        version: 0,
        size: core::mem::size_of::<Self>() as u32,
        state: core::ptr::null_mut(),
        socket: None,
        connect: None,
        bind: None,
        listen: None,
        accept: None,
        send: None,
        recv: None,
        shutdown: None,
        getsockname: None,
        getpeername: None,
        getsockopt: None,
        setsockopt: None,
        poll: None,
        socketpair: None,
        close: None,
    };
}

/// `net` 域的异步分类（`CP-25`／`AB-34`：按域二值，**缺失即注册失败**）。
///
/// `SPEC-capabilities.md` §4／§9.2 给 `net` 的建议值是**可异步化**（`PA_ASYNC_OK` ⇒ 取值为 `0`，
/// 与 [`super::fs::ASYNC_CLASSIFICATION`] 同一口径）。注意 `CP-37`：**按域二值**，
/// `poll` 的"等待"也走取消令牌（`CP-27`），**不设**槽位级分类。
pub const ASYNC_CLASSIFICATION: i32 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    /// `CP-39`：IPv4 线格式是 4B 地址 ＋ 2B 端口、**大端**；往返不丢信息。
    #[test]
    fn ipv4_wire_format_is_four_bytes_then_big_endian_port() {
        let encoded = encode_ipv4([127, 0, 0, 1], 8080);
        assert_eq!(encoded, [127, 0, 0, 1, 0x1F, 0x90]);
        assert_eq!(decode_ipv4(&encoded), Some(([127, 0, 0, 1], 8080)));
        // 端口大端：0x0102 ⇒ 字节 0x01 在前
        assert_eq!(encode_ipv4([0, 0, 0, 0], 0x0102)[4..], [0x01, 0x02]);
        assert_eq!(decode_ipv4(&[127, 0, 0, 1]), None, "长度不符 ⇒ 拒绝");
    }

    /// `CP-12`：形状层只有类型与函数指针 ⇒ 未实现 vtable 的槽位全 `None`（`CP-2`／`CP-3`）。
    #[test]
    fn unimplemented_vtable_has_no_slots() {
        let table = CpNetVtable::UNIMPLEMENTED;
        assert_eq!(table.version, 0, "UNIMPLEMENTED 是'未填'哨兵");
        assert_eq!(table.size as usize, core::mem::size_of::<CpNetVtable>());
        assert!(table.socket.is_none() && table.socketpair.is_none() && table.close.is_none());
        assert_eq!(ASYNC_CLASSIFICATION, 0, "net 域按 §4 表建议值可异步化");
    }
}
