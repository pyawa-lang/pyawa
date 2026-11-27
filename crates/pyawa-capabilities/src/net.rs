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
//! - **`CP-47`**：IPv6 的 v1.1 线格式 —— canonical（guest／线格式）族号 `AF_INET6`＝**10**；
//!   16 字节地址（网络序原样）＋ 2 字节端口（**大端**）＝ **18 字节**；`flowinfo`／`scope_id`
//!   **不在 v1**；`AF_UNIX` 仍按 `CP-21`；旧 provider 对 IPv6 仍 `EAFNOSUPPORT`。本文件只做
//!   **纯数据编解码**（无平台、无状态），provider 数据面归 `R2`。
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

/// **`CP-39`**：v1 支持的地址族（IPv6 见 `CP-47`；其它族**不在 v1**）。
pub mod family {
    /// `AF_UNIX`（Linux 取值）。
    pub const AF_UNIX: i32 = 1;
    /// `AF_INET`（Linux 取值）。
    pub const AF_INET: i32 = 2;
    /// `AF_INET6`（**canonical** 取值＝POSIX 的 **10**，`CP-47`）。
    ///
    /// Windows／Winsock 的 `AF_INET6`＝23 只是 provider 适配层的映射，**不进入** guest／线格式。
    pub const AF_INET6: i32 = 10;
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

/// **`CP-47`**：IPv6 的 v1.1 地址线长度＝16 字节地址 ＋ 2 字节端口（**大端**）。
pub const SOCKADDR_V1_IPV6_LEN: usize = 18;

/// **`CP-47`**：把 IPv6 地址编码成 v1.1 线格式（16B 地址原样 ＋ 2B 端口，大端）。
///
/// 纯格式编解码，不是机器实现（不碰平台）；`flowinfo`／`scope_id` **不在 v1**，
/// 地址按网络序原样透传。
pub fn encode_ipv6(addr: [u8; 16], port: u16) -> [u8; SOCKADDR_V1_IPV6_LEN] {
    let mut out = [0u8; SOCKADDR_V1_IPV6_LEN];
    out[..16].copy_from_slice(&addr);
    let [high, low] = port.to_be_bytes();
    out[16] = high;
    out[17] = low;
    out
}

/// **`CP-47`**：把 v1.1 的 IPv6 线格式解回 `(地址, 端口)`；长度不符返回 `None`。
pub fn decode_ipv6(bytes: &[u8]) -> Option<([u8; 16], u16)> {
    if bytes.len() != SOCKADDR_V1_IPV6_LEN {
        return None;
    }
    let mut addr = [0u8; 16];
    addr.copy_from_slice(&bytes[..16]);
    let port = u16::from_be_bytes([bytes[16], bytes[17]]);
    Some((addr, port))
}

/// 解析一个 IPv6 文本组（1–4 个十六进制字符）⇒ `u16`；空段／超长／非法字符返回 `None`。
fn parse_ipv6_hex_group(segment: &str) -> Option<u16> {
    if segment.is_empty() || segment.len() > 4 {
        return None;
    }
    let mut value = 0u16;
    for byte in segment.bytes() {
        let digit = match byte {
            b'0'..=b'9' => u16::from(byte - b'0'),
            b'a'..=b'f' => u16::from(byte - b'a') + 10,
            b'A'..=b'F' => u16::from(byte - b'A') + 10,
            _ => return None,
        };
        value = value * 16 + digit;
    }
    Some(value)
}

/// 解析 IPv6 文本末尾内嵌的点分 IPv4（`::ffff:127.0.0.1`）⇒ 两个 `u16` 组；不是点分形式返回 `None`。
fn parse_ipv4_tail(segment: &str) -> Option<(u16, u16)> {
    let mut octets = [0u8; 4];
    let mut seen = 0usize;
    for part in segment.split('.') {
        if seen >= 4 || part.is_empty() || part.len() > 3 {
            return None;
        }
        let mut value = 0u16;
        for byte in part.bytes() {
            if !byte.is_ascii_digit() {
                return None;
            }
            value = value * 10 + u16::from(byte - b'0');
        }
        if value > 255 {
            return None;
        }
        octets[seen] = value as u8;
        seen += 1;
    }
    if seen != 4 {
        return None;
    }
    Some((
        u16::from_be_bytes([octets[0], octets[1]]),
        u16::from_be_bytes([octets[2], octets[3]]),
    ))
}

/// **`CP-47`**：IPv6 文本（RFC 4291 的十六进制形式）⇒ 16 字节网络序地址；非法输入返回 `None`。
///
/// 这是**纯数据**解析（`CX-4`：不碰 `std::net`、不碰平台）：接受 1–8 个十六进制组、可用**一次**
/// `::` 压缩（至少吞掉一组），末段允许内嵌点分 IPv4（`::ffff:127.0.0.1`）。IPv6 scope id
/// （`%eth0`）**不在 v1**（`CP-47`：不带 `scope_id`）⇒ 含 `%` 一律 `None`。
pub fn parse_ipv6_literal(text: &str) -> Option<[u8; 16]> {
    if text.is_empty() || text.contains('%') {
        return None;
    }
    let (head, tail, compressed) = match text.find("::") {
        Some(index) => {
            let tail = &text[index + 2..];
            if tail.contains("::") {
                return None; // 至多一次 `::`
            }
            (&text[..index], tail, true)
        }
        None => (text, "", false),
    };
    let head_parts: Vec<&str> = if head.is_empty() {
        Vec::new()
    } else {
        head.split(':').collect()
    };
    let tail_parts: Vec<&str> = if tail.is_empty() {
        Vec::new()
    } else {
        tail.split(':').collect()
    };
    let total = head_parts.len() + tail_parts.len();
    if total == 0 {
        // `::`（空串已在上面拒绝）⇒ 全零。
        return compressed.then_some([0u8; 16]);
    }
    let mut groups: Vec<u16> = Vec::with_capacity(8);
    let mut head_group_count = 0usize;
    for (index, segment) in head_parts.iter().enumerate() {
        if segment.is_empty() {
            return None;
        }
        let is_last_overall = index + 1 == total;
        if is_last_overall {
            if let Some((high, low)) = parse_ipv4_tail(segment) {
                groups.push(high);
                groups.push(low);
                head_group_count += 2;
                continue;
            }
        }
        groups.push(parse_ipv6_hex_group(segment)?);
        head_group_count += 1;
    }
    for (index, segment) in tail_parts.iter().enumerate() {
        if segment.is_empty() {
            return None;
        }
        let is_last_overall = head_parts.len() + index + 1 == total;
        if is_last_overall {
            if let Some((high, low)) = parse_ipv4_tail(segment) {
                groups.push(high);
                groups.push(low);
                continue;
            }
        }
        groups.push(parse_ipv6_hex_group(segment)?);
    }
    if compressed {
        if groups.len() > 7 {
            return None; // `::` 至少吞掉一组
        }
    } else if groups.len() != 8 {
        return None;
    }
    let mut address = [0u8; 16];
    for (index, group) in groups[..head_group_count.min(groups.len())].iter().enumerate() {
        address[index * 2..index * 2 + 2].copy_from_slice(&group.to_be_bytes());
    }
    let tail_groups = &groups[head_group_count.min(groups.len())..];
    let start = 8 - tail_groups.len();
    for (index, group) in tail_groups.iter().enumerate() {
        let position = start + index;
        address[position * 2..position * 2 + 2].copy_from_slice(&group.to_be_bytes());
    }
    Some(address)
}

/// **`CP-47`**：16 字节网络序地址 ⇒ IPv6 文本（CPython `inet_ntop` 口径：小写十六进制、
/// **最长零段**压缩、`::`；最长零段并列时取**最左**）。
///
/// 与 `CPython`（libc／Winsock 的 `inet_ntop`）一致，IPv4-mapped 地址
/// （`::ffff:a.b.c.d`）给**点分尾巴**；其它地址一律十六进制形式（不特判 v4-compatible）。
pub fn format_ipv6(addr: &[u8; 16]) -> String {
    // IPv4-mapped（`::ffff:a.b.c.d`）：与 CPython 的 `inet_ntop` 同口径。
    if addr[..10].iter().all(|byte| *byte == 0) && addr[10] == 0xff && addr[11] == 0xff {
        return format!(
            "::ffff:{}.{}.{}.{}",
            addr[12], addr[13], addr[14], addr[15]
        );
    }
    let mut groups = [0u16; 8];
    for (index, group) in groups.iter_mut().enumerate() {
        *group = u16::from_be_bytes([addr[index * 2], addr[index * 2 + 1]]);
    }
    // 最长零段（至少 2 组；并列取最左）。
    let mut best_start = 0usize;
    let mut best_len = 0usize;
    let mut index = 0usize;
    while index < groups.len() {
        if groups[index] != 0 {
            index += 1;
            continue;
        }
        let start = index;
        while index < groups.len() && groups[index] == 0 {
            index += 1;
        }
        let length = index - start;
        if length > best_len {
            best_start = start;
            best_len = length;
        }
    }
    let hex = |slice: &[u16]| {
        slice
            .iter()
            .map(|group| format!("{group:x}"))
            .collect::<Vec<String>>()
            .join(":")
    };
    if best_len < 2 {
        return hex(&groups);
    }
    let left = hex(&groups[..best_start]);
    let right = hex(&groups[best_start + best_len..]);
    match (left.is_empty(), right.is_empty()) {
        (true, true) => "::".to_owned(),
        (true, false) => format!("::{right}"),
        (false, true) => format!("{left}::"),
        (false, false) => format!("{left}::{right}"),
    }
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

    /// `CP-47`：IPv6 线格式是 16B 地址（原样）＋ 2B 端口（**大端**），共 18B；往返不丢信息。
    #[test]
    fn ipv6_wire_format_is_sixteen_bytes_then_big_endian_port() {
        let addr = [
            0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        ];
        let encoded = encode_ipv6(addr, 8080);
        assert_eq!(SOCKADDR_V1_IPV6_LEN, 18, "CP-47：16B 地址 ＋ 2B 端口");
        assert_eq!(&encoded[..16], &addr, "地址网络序原样透传（无 scope_id／flowinfo）");
        assert_eq!(&encoded[16..], &[0x1F, 0x90], "端口大端");
        assert_eq!(decode_ipv6(&encoded), Some((addr, 8080)));
        // 端口大端：0x0102 ⇒ 字节 0x01 在前
        assert_eq!(encode_ipv6([0u8; 16], 0x0102)[16..], [0x01, 0x02]);
        // 长度不符（IPv4 的 6B／17B／19B）⇒ 拒绝
        assert_eq!(decode_ipv6(&[0u8; 6]), None, "6B ⇒ 拒绝");
        assert_eq!(decode_ipv6(&[0u8; 17]), None, "17B ⇒ 拒绝");
        assert_eq!(decode_ipv6(&[0u8; 19]), None, "19B ⇒ 拒绝");
    }

    /// `CP-47`：canonical 族号取 POSIX 的 10（Winsock 23 的映射归 provider，不进线格式）。
    #[test]
    fn ipv6_canonical_family_number_is_posix_ten() {
        assert_eq!(family::AF_INET6, 10);
        assert_eq!(family::AF_INET, 2);
        assert_eq!(family::AF_UNIX, 1);
    }

    /// `CP-47`：IPv6 文本 ⇔ 16 字节地址的解析／格式化（`format_ipv6` 与 CPython `inet_ntop` 同口径）。
    #[test]
    fn ipv6_text_round_trips_through_wire_format() {
        // 常见形态（`::1`／`::`／`fe80::1`／全零段压缩）。
        let cases: &[(&str, [u8; 16])] = &[
            ("::1", {
                let mut bytes = [0u8; 16];
                bytes[15] = 1;
                bytes
            }),
            ("::", [0u8; 16]),
            ("fe80::1", {
                let mut bytes = [0u8; 16];
                bytes[0] = 0xfe;
                bytes[1] = 0x80;
                bytes[15] = 1;
                bytes
            }),
            ("2001:db8::1", {
                let mut bytes = [0u8; 16];
                bytes[0] = 0x20;
                bytes[1] = 0x01;
                bytes[2] = 0x0d;
                bytes[3] = 0xb8;
                bytes[15] = 1;
                bytes
            }),
            ("1:2:3:4:5:6:7:8", [0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8]),
        ];
        for (text, address) in cases {
            assert_eq!(parse_ipv6_literal(text), Some(*address), "解析 {text}");
            assert_eq!(format_ipv6(address), *text, "格式化 {address:?}");
            // 与 wire 编解码一致：地址段原样进 18B 线格式再取回。
            let wire = encode_ipv6(*address, 8080);
            assert_eq!(decode_ipv6(&wire).map(|(addr, _)| addr), Some(*address));
        }
        // 全零段压缩：最长零段（并列取最左）；单组 0 不压缩。
        assert_eq!(format_ipv6(&parse_ipv6_literal("1:0:0:2:0:0:0:3").unwrap()), "1:0:0:2::3");
        assert_eq!(format_ipv6(&parse_ipv6_literal("0:0:1:0:0:0:0:1").unwrap()), "0:0:1::1");
        assert_eq!(
            format_ipv6(&parse_ipv6_literal("1:0:2:0:3:0:4:0").unwrap()),
            "1:0:2:0:3:0:4:0"
        );
        // IPv4-mapped：CPython 的 `inet_ntop` 给点分尾巴。
        let mapped = parse_ipv6_literal("::ffff:127.0.0.1").expect("v4-mapped");
        assert_eq!(format_ipv6(&mapped), "::ffff:127.0.0.1");
        // 末段内嵌点分 IPv4 的解析。
        let mut dotted = [0u8; 16];
        dotted[10] = 0xff;
        dotted[11] = 0xff;
        dotted[12..].copy_from_slice(&[127, 0, 0, 1]);
        assert_eq!(parse_ipv6_literal("::ffff:127.0.0.1"), Some(dotted));
    }

    /// `CP-47`：非法 IPv6 文本 ⇒ `None`（不 panic、不猜）。
    #[test]
    fn ipv6_text_rejects_invalid_input() {
        for text in [
            "",
            ":",
            "::1:",
            "1::2::3",
            "1:2:3:4:5:6:7",     // 不足 8 组且无 `::`
            "1:2:3:4:5:6:7:8:9", // 超过 8 组
            "12345::1",          // 组超 4 个十六进制字符
            "fe80::1%eth0",      // scope id 不在 v1
            "::g",
            "1.2.3.4",
            "1:2:3:4:5:6:7:8::", // `::` 至少吞一组
            "::1.2.3.256",
        ] {
            assert_eq!(parse_ipv6_literal(text), None, "应拒绝 {text:?}");
        }
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
