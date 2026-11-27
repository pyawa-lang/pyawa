//! **Windows `net` provider 形状／回环测试**（`PLAN §12.20` W4；只在 `windows` 目标编译）。
//!
//! 直接测 provider 的 vtable（不依赖 CLI／ABI／VM 适配层）：
//!
//! 1. **vtable 形状**：`version`／`size`／`state` 正确；`socketpair` 为 `None`（`CP-3`：Windows 没有
//!    `socketpair(2)`），其余槽位全部 `Some`；
//! 2. **回环全链**：IPv4（`bind(127.0.0.1:0)`）与 IPv6（`bind([::1]:0)`；`CP-47`）各一条 ——
//!    `socket` → `bind` → `listen(1)` → `getsockname` 取真实端口 → 客户端 `socket` ＋ `connect` →
//!    `accept` → 双向 `send`／`recv`（都按**已知字节数**收，绝不无限阻塞）→ `shutdown` → `close`；
//!    结束时句柄表清空、重复 `close` 报 `EBADF`；
//! 3. **选项翻译**：`setsockopt(SOL_SOCKET, SO_REUSEADDR, 1)`（POSIX 编号 → Winsock 编号）、
//!    `getsockopt(SO_TYPE)`⇒`SOCK_STREAM`、`SO_ERROR`⇒`0`；未接选项 ⇒ `ENOPROTOOPT`；
//! 4. **未知／已关句柄**的 `send`／`close` ⇒ `Machine` ＋ `EBADF`（`CP-18`）；
//! 5. **`poll`**：连接建立且对端先发一个字节后，客户端填 `POLLIN|POLLOUT` ⇒ 两个都就绪；
//!    排空后 `timeout=0` 轮询 ⇒ 只余 `POLLOUT`；并验证**两段式**出参（容量 0 时 `out_len` 回总数）。
//!
//! 避免阻塞挂死的口径：先 `connect` 再 `accept`（连接已在 backlog 里）；`recv` 只收已知字节数；
//! `poll` 都带有限超时。
//!
//! errno 期望值一律按**名字**从 `pyawa_abi::platform_errno::HOST_ERRNO` 查（`CM-20`：一处真相，
//! 不手写数字）。
#![cfg(windows)]

use pyawa_capabilities::net::{
    decode_ipv4, decode_ipv6, encode_ipv4, encode_ipv6, family, shutdown_how, socket_type, CapStatus,
    CpNetVtable, PollFd, SocketHandle, SOCKADDR_V1_IPV4_LEN, SOCKADDR_V1_IPV6_LEN,
};
use pyawa_core::Instance;
use pyawa_runtime::net_windows::WindowsNet;

/// 按名字取宿主 errno（与 provider 的 `errno_named` 同一个真相源）。
fn host_errno(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .expect("宿主 errno 表里应当有这个常量")
}

// guest（POSIX）侧的选项号：与 `pyawa-stdlib` `_socket_module` 的常量表一致。
const SOL_SOCKET_POSIX: i32 = 1;
const SO_REUSEADDR_POSIX: i32 = 2;
const SO_TYPE_POSIX: i32 = 3;
const SO_ERROR_POSIX: i32 = 4;
const IPPROTO_IPV6_POSIX: i32 = 41;
const IPV6_V6ONLY_POSIX: i32 = 26;

/// IPv6 回环地址 `::1` 的 16 字节网络序表示（`CP-47`）。
const IPV6_LOOPBACK: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

// --------------------------------------------------------------------------- //
// vtable 调用小工具（槽位是**安全**的 `extern "C" fn` 指针；`state` 由 provider 给）
// --------------------------------------------------------------------------- //

fn do_socket(
    vtable: &CpNetVtable,
    af: i32,
    ty: i32,
    proto: i32,
) -> Result<SocketHandle, i32> {
    let mut handle = SocketHandle(0);
    let mut errno = 0i32;
    let status = (vtable.socket.expect("socket 槽已接线"))(
        vtable.state,
        af,
        ty,
        proto,
        &mut handle,
        &mut errno,
    );
    match status {
        CapStatus::Ok => Ok(handle),
        CapStatus::Machine => Err(errno),
        other => panic!("socket 返回了意外状态 {other:?}"),
    }
}

/// `bind(sock, family, wire_addr)`（`wire_addr` 是 `CP-39` 的 6 字节 v1 线格式）。
fn do_bind(
    vtable: &CpNetVtable,
    handle: SocketHandle,
    af: i32,
    wire_addr: &[u8],
) -> Result<(), i32> {
    let mut errno = 0i32;
    let status = (vtable.bind.expect("bind 槽已接线"))(
        vtable.state,
        handle,
        af,
        wire_addr.as_ptr(),
        wire_addr.len(),
        &mut errno,
    );
    match status {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        other => panic!("bind 返回了意外状态 {other:?}"),
    }
}

fn do_connect(
    vtable: &CpNetVtable,
    handle: SocketHandle,
    af: i32,
    wire_addr: &[u8],
) -> Result<(), i32> {
    let mut errno = 0i32;
    let status = (vtable.connect.expect("connect 槽已接线"))(
        vtable.state,
        handle,
        af,
        wire_addr.as_ptr(),
        wire_addr.len(),
        &mut errno,
    );
    match status {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        other => panic!("connect 返回了意外状态 {other:?}"),
    }
}

fn do_listen(vtable: &CpNetVtable, handle: SocketHandle, backlog: i32) -> Result<(), i32> {
    let mut errno = 0i32;
    let status = (vtable.listen.expect("listen 槽已接线"))(
        vtable.state,
        handle,
        backlog,
        &mut errno,
    );
    match status {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        other => panic!("listen 返回了意外状态 {other:?}"),
    }
}

/// `accept(listener)` ⇒ `(新句柄, canonical 地址族, v1 线格式地址)`。
///
/// `accept` 的地址出参是两段式的，但**不能**为扩缓冲重试一次（重试会等下一个连接）⇒ 本测试
/// 直接给足够容量（18B ≥ IPv6 的 18B）一次取回；总长按族断言 6／18（`CP-39`／`CP-47`）。
fn do_accept(
    vtable: &CpNetVtable,
    listener: SocketHandle,
) -> Result<(SocketHandle, i32, Vec<u8>), i32> {
    let mut handle = SocketHandle(0);
    let mut af = 0i32;
    let mut wire = [0u8; SOCKADDR_V1_IPV6_LEN];
    let mut out_len = 0usize;
    let mut errno = 0i32;
    let status = (vtable.accept.expect("accept 槽已接线"))(
        vtable.state,
        listener,
        &mut handle,
        &mut af,
        wire.as_mut_ptr(),
        wire.len(),
        &mut out_len,
        &mut errno,
    );
    match status {
        CapStatus::Ok => {
            assert!(
                out_len == SOCKADDR_V1_IPV4_LEN || out_len == SOCKADDR_V1_IPV6_LEN,
                "CP-39／CP-47：地址线总长只有 6／18 字节（实际 {out_len}）"
            );
            Ok((handle, af, wire[..out_len].to_vec()))
        }
        CapStatus::Machine => Err(errno),
        other => panic!("accept 返回了意外状态 {other:?}"),
    }
}

/// `getsockname`／`getpeername`：**两段式**出参（先容量 0 验总长，再按总长重取）。
fn do_sockname(
    vtable: &CpNetVtable,
    handle: SocketHandle,
    peer: bool,
) -> Result<(i32, Vec<u8>), i32> {
    let query = if peer {
        vtable.getpeername.expect("getpeername 槽已接线")
    } else {
        vtable.getsockname.expect("getsockname 槽已接线")
    };
    let mut af = 0i32;
    let mut wire = [0u8; SOCKADDR_V1_IPV6_LEN];
    let mut out_len = 0usize;
    let mut errno = 0i32;
    // 第一段：容量 0 ⇒ 只回总长，不写项。
    let status = query(
        vtable.state,
        handle,
        &mut af,
        wire.as_mut_ptr(),
        0,
        &mut out_len,
        &mut errno,
    );
    match status {
        CapStatus::Ok => assert!(
            out_len == SOCKADDR_V1_IPV4_LEN || out_len == SOCKADDR_V1_IPV6_LEN,
            "两段式：容量 0 时 out_len 回总长（6／18）"
        ),
        CapStatus::Machine => return Err(errno),
        other => panic!("地址查询返回了意外状态 {other:?}"),
    }
    // 第二段：按总长重取。
    let status = query(
        vtable.state,
        handle,
        &mut af,
        wire.as_mut_ptr(),
        wire.len(),
        &mut out_len,
        &mut errno,
    );
    match status {
        CapStatus::Ok => {
            assert!(out_len <= wire.len(), "两段式：out_len 不超过容量");
            Ok((af, wire[..out_len].to_vec()))
        }
        CapStatus::Machine => Err(errno),
        other => panic!("地址查询返回了意外状态 {other:?}"),
    }
}

fn do_send(
    vtable: &CpNetVtable,
    handle: SocketHandle,
    payload: &[u8],
) -> Result<usize, i32> {
    let mut sent = 0usize;
    let mut errno = 0i32;
    let status = (vtable.send.expect("send 槽已接线"))(
        vtable.state,
        handle,
        payload.as_ptr(),
        payload.len(),
        &mut sent,
        &mut errno,
    );
    match status {
        CapStatus::Ok => Ok(sent),
        CapStatus::Machine => Err(errno),
        other => panic!("send 返回了意外状态 {other:?}"),
    }
}

/// 收到**恰好** `wanted` 字节（循环 `recv`；对端提前关闭 ⇒ 测试失败，而不是挂死）。
fn do_recv_exact(
    vtable: &CpNetVtable,
    handle: SocketHandle,
    wanted: usize,
) -> Result<Vec<u8>, i32> {
    let mut buffer = vec![0u8; wanted];
    let mut got = 0usize;
    while got < wanted {
        let mut received = 0usize;
        let mut errno = 0i32;
        let status = (vtable.recv.expect("recv 槽已接线"))(
            vtable.state,
            handle,
            buffer[got..].as_mut_ptr(),
            wanted - got,
            &mut received,
            &mut errno,
        );
        match status {
            CapStatus::Ok => {
                assert!(received > 0, "对端已关闭：还差 {} 字节", wanted - got);
                got += received;
            }
            CapStatus::Machine => return Err(errno),
            other => panic!("recv 返回了意外状态 {other:?}"),
        }
    }
    Ok(buffer)
}

fn do_shutdown(vtable: &CpNetVtable, handle: SocketHandle, how: i32) -> Result<(), i32> {
    let mut errno = 0i32;
    let status = (vtable.shutdown.expect("shutdown 槽已接线"))(
        vtable.state,
        handle,
        how,
        &mut errno,
    );
    match status {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        other => panic!("shutdown 返回了意外状态 {other:?}"),
    }
}

fn do_close(vtable: &CpNetVtable, handle: SocketHandle) -> Result<(), i32> {
    let mut errno = 0i32;
    let status = (vtable.close.expect("close 槽已接线"))(vtable.state, handle, &mut errno);
    match status {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        other => panic!("close 返回了意外状态 {other:?}"),
    }
}

fn do_setsockopt(
    vtable: &CpNetVtable,
    handle: SocketHandle,
    level: i32,
    name: i32,
    value: &[u8],
) -> Result<(), i32> {
    let mut errno = 0i32;
    let status = (vtable.setsockopt.expect("setsockopt 槽已接线"))(
        vtable.state,
        handle,
        level,
        name,
        value.as_ptr(),
        value.len(),
        &mut errno,
    );
    match status {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        other => panic!("setsockopt 返回了意外状态 {other:?}"),
    }
}

fn do_getsockopt(
    vtable: &CpNetVtable,
    handle: SocketHandle,
    level: i32,
    name: i32,
) -> Result<Vec<u8>, i32> {
    let mut value = [0u8; 16];
    let mut out_len = 0usize;
    let mut errno = 0i32;
    let status = (vtable.getsockopt.expect("getsockopt 槽已接线"))(
        vtable.state,
        handle,
        level,
        name,
        value.as_mut_ptr(),
        value.len(),
        &mut out_len,
        &mut errno,
    );
    match status {
        CapStatus::Ok => {
            assert!(out_len <= value.len(), "两段式：out_len 不会超过容量");
            Ok(value[..out_len].to_vec())
        }
        CapStatus::Machine => Err(errno),
        other => panic!("getsockopt 返回了意外状态 {other:?}"),
    }
}

/// `poll(fds, timeout_ns)` ⇒ `(就绪项, out_len 回的总数)`；`capacity` 用于验证两段式出参。
fn do_poll(
    vtable: &CpNetVtable,
    fds: &[PollFd],
    timeout_ns: i64,
    capacity: usize,
) -> Result<(Vec<PollFd>, usize), i32> {
    let mut out = vec![
        PollFd {
            handle: SocketHandle(0),
            events: 0,
            revents: 0,
        };
        capacity
    ];
    let mut out_len = 0usize;
    let mut errno = 0i32;
    let status = (vtable.poll.expect("poll 槽已接线"))(
        vtable.state,
        fds.as_ptr(),
        fds.len(),
        timeout_ns,
        out.as_mut_ptr(),
        capacity,
        &mut out_len,
        &mut errno,
    );
    match status {
        CapStatus::Ok => {
            out.truncate(out_len.min(capacity));
            Ok((out, out_len))
        }
        CapStatus::Machine => Err(errno),
        other => panic!("poll 返回了意外状态 {other:?}"),
    }
}

/// 建一对已连接的回环 socket：`(listener, client, accepted)`（`listener` 给清理用）。
fn connected_pair(vtable: &CpNetVtable) -> (SocketHandle, SocketHandle, SocketHandle) {
    let listener = do_socket(vtable, family::AF_INET, socket_type::SOCK_STREAM, 0)
        .expect("server socket");
    do_bind(
        vtable,
        listener,
        family::AF_INET,
        &encode_ipv4([127, 0, 0, 1], 0),
    )
    .expect("bind 127.0.0.1:0");
    do_listen(vtable, listener, 1).expect("listen(1)");
    let (_, wire) = do_sockname(vtable, listener, false).expect("getsockname(listener)");
    let (_, port) = decode_ipv4(&wire).expect("6 字节地址线");
    assert_ne!(port, 0, "bind(port=0) 后内核应分配真实端口");
    let client = do_socket(vtable, family::AF_INET, socket_type::SOCK_STREAM, 0)
        .expect("client socket");
    // **先 connect 再 accept**：连接已在 backlog 里，accept 不会无限等待。
    do_connect(vtable, client, family::AF_INET, &encode_ipv4([127, 0, 0, 1], port))
        .expect("connect 回环监听方");
    let (accepted, af, peer_wire) = do_accept(vtable, listener).expect("accept");
    assert_eq!(af, family::AF_INET, "IPv4 回 out_family＝AF_INET（CP-39）");
    let (peer_addr, peer_port) = decode_ipv4(&peer_wire).expect("6 字节地址线");
    assert_eq!(peer_addr, [127, 0, 0, 1], "回环对端地址");
    assert_ne!(peer_port, 0, "对端临时端口应真实存在");
    (listener, client, accepted)
}

/// 建一对已连接的 **IPv6** 回环 socket：`(listener, client, accepted)`（`CP-47`）。
fn connected_pair_ipv6(vtable: &CpNetVtable) -> (SocketHandle, SocketHandle, SocketHandle) {
    let listener = do_socket(vtable, family::AF_INET6, socket_type::SOCK_STREAM, 0)
        .expect("IPv6 server socket（guest 10 ⇒ Winsock 23）");
    do_bind(
        vtable,
        listener,
        family::AF_INET6,
        &encode_ipv6(IPV6_LOOPBACK, 0),
    )
    .expect("bind [::1]:0");
    do_listen(vtable, listener, 1).expect("listen(1)");
    let (af, wire) = do_sockname(vtable, listener, false).expect("getsockname(listener)");
    assert_eq!(af, family::AF_INET6, "out_family 是 canonical 10（不是 Winsock 23）");
    let (address, port) = decode_ipv6(&wire).expect("18 字节 IPv6 地址线");
    assert_eq!(address, IPV6_LOOPBACK, "bind([::1]:0)");
    assert_ne!(port, 0, "bind(port=0) 后内核应分配真实端口");
    let client = do_socket(vtable, family::AF_INET6, socket_type::SOCK_STREAM, 0)
        .expect("IPv6 client socket");
    // **先 connect 再 accept**：连接已在 backlog 里，accept 不会无限等待。
    do_connect(vtable, client, family::AF_INET6, &encode_ipv6(IPV6_LOOPBACK, port))
        .expect("connect [::1] 监听方");
    let (accepted, peer_af, peer_wire) = do_accept(vtable, listener).expect("accept");
    assert_eq!(peer_af, family::AF_INET6, "accept 的 out_family 是 canonical 10");
    let (peer_addr, peer_port) = decode_ipv6(&peer_wire).expect("18 字节 IPv6 地址线");
    assert_eq!(peer_addr, IPV6_LOOPBACK, "回环对端地址");
    assert_ne!(peer_port, 0, "对端临时端口应真实存在");
    (listener, client, accepted)
}

// --------------------------------------------------------------------------- //
// 1) vtable 形状
// --------------------------------------------------------------------------- //

#[test]
fn vtable_exposes_every_slot_but_socketpair() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    assert_eq!(table.version, pyawa_abi::PA_ABI_VERSION, "CP-30／AB-39");
    assert_eq!(table.size as usize, core::mem::size_of::<CpNetVtable>());
    assert!(!table.state.is_null(), "state 必须指回 provider");
    // W4 补齐（2026-11-19）：Windows 的 `socketpair` 用 loopback TCP 对真实现（与 CPython 同法）。
    assert!(table.socketpair.is_some(), "socketpair");
    assert!(table.socket.is_some(), "socket");
    assert!(table.connect.is_some(), "connect");
    assert!(table.bind.is_some(), "bind");
    assert!(table.listen.is_some(), "listen");
    assert!(table.accept.is_some(), "accept");
    assert!(table.send.is_some(), "send");
    assert!(table.recv.is_some(), "recv");
    assert!(table.shutdown.is_some(), "shutdown");
    assert!(table.getsockname.is_some(), "getsockname");
    assert!(table.getpeername.is_some(), "getpeername");
    assert!(table.getsockopt.is_some(), "getsockopt");
    assert!(table.setsockopt.is_some(), "setsockopt");
    assert!(table.poll.is_some(), "poll");
    assert!(table.close.is_some(), "close");
}

#[test]
fn socket_accepts_ipv6_but_rejects_unsupported_family_and_type() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    // `CP-47`：IPv6 已接（canonical 10 映射 Winsock 23）；能建就能关。
    let ipv6 = do_socket(&table, family::AF_INET6, socket_type::SOCK_STREAM, 0)
        .expect("AF_INET6 socket 已接线（CP-47）");
    do_close(&table, ipv6).expect("close IPv6 socket");
    // `AF_UNIX` 等其它族仍按机器错误如实拒绝（`socketpair` 的 IPv4-only 例外见 `CP-38`）。
    assert_eq!(
        do_socket(&table, family::AF_UNIX, socket_type::SOCK_STREAM, 0),
        Err(host_errno("EAFNOSUPPORT")),
    );
    // 在 Windows 上只接 TCP；SOCK_DGRAM 等如实拒绝。
    assert_eq!(
        do_socket(&table, family::AF_INET, socket_type::SOCK_DGRAM, 0),
        Err(host_errno("EPROTONOSUPPORT")),
    );
    assert_eq!(provider.open_handle_count(), 0, "被拒绝的 socket 不占句柄");
}

#[test]
fn sockaddr_wire_length_and_family_are_validated() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    let ipv6 = do_socket(&table, family::AF_INET6, socket_type::SOCK_STREAM, 0)
        .expect("AF_INET6 socket");
    // 族与线格式长度必须匹配（`CP-39`：v4 6B；`CP-47`：v6 18B）⇒ 不符回 EINVAL。
    assert_eq!(
        do_bind(&table, ipv6, family::AF_INET6, &encode_ipv4([127, 0, 0, 1], 0)),
        Err(host_errno("EINVAL")),
        "AF_INET6 收 6B ⇒ EINVAL",
    );
    assert_eq!(
        do_bind(&table, ipv6, family::AF_INET, &encode_ipv6(IPV6_LOOPBACK, 0)),
        Err(host_errno("EINVAL")),
        "AF_INET 收 18B ⇒ EINVAL",
    );
    // 未知族 ⇒ EAFNOSUPPORT（不假装成功）。
    assert_eq!(
        do_bind(&table, ipv6, family::AF_UNIX, &encode_ipv4([127, 0, 0, 1], 0)),
        Err(host_errno("EAFNOSUPPORT")),
    );
    do_close(&table, ipv6).expect("close");
    assert_eq!(provider.open_handle_count(), 0);
}

// --------------------------------------------------------------------------- //
// 2) 回环全链
// --------------------------------------------------------------------------- //

#[test]
fn loopback_connect_accept_send_recv_shutdown_close() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    let (listener, client, accepted) = connected_pair(&table);

    // 双向收发：都按已知字节数收，不会无限阻塞。
    assert_eq!(do_send(&table, client, b"ping").expect("send ping"), 4);
    assert_eq!(
        do_recv_exact(&table, accepted, 4).expect("recv ping"),
        b"ping".to_vec(),
    );
    assert_eq!(do_send(&table, accepted, b"pong").expect("send pong"), 4);
    assert_eq!(
        do_recv_exact(&table, client, 4).expect("recv pong"),
        b"pong".to_vec(),
    );

    // `getpeername(client)` ⇒ 监听方的真实端口（`getsockname(listener)` 取到的同一个）。
    let (listener_af, listener_wire) =
        do_sockname(&table, listener, false).expect("getsockname(listener)");
    let (_, listener_port) = decode_ipv4(&listener_wire).expect("6 字节地址线");
    assert_eq!(listener_af, family::AF_INET);
    let (peer_af, peer_wire) = do_sockname(&table, client, true).expect("getpeername(client)");
    let (peer_addr, peer_port) = decode_ipv4(&peer_wire).expect("6 字节地址线");
    assert_eq!(peer_af, family::AF_INET);
    assert_eq!(peer_addr, [127, 0, 0, 1]);
    assert_eq!(peer_port, listener_port, "客户端对端就是监听方地址");

    // shutdown：客户端半关写、服务端全关（两套 ABI 的 how 同值 ⇒ 直接生效）。
    do_shutdown(&table, client, shutdown_how::SHUT_WR).expect("shutdown(SHUT_WR)");
    do_shutdown(&table, accepted, shutdown_how::SHUT_RDWR).expect("shutdown(SHUT_RDWR)");

    // close 全部 ⇒ 句柄表清空。
    do_close(&table, client).expect("close client");
    do_close(&table, accepted).expect("close accepted");
    do_close(&table, listener).expect("close listener");
    assert_eq!(provider.open_handle_count(), 0, "两端句柄表应清空");
    // 已关闭句柄再 close ⇒ EBADF（CP-18）。
    assert_eq!(
        do_close(&table, client),
        Err(host_errno("EBADF")),
        "重复 close ⇒ Machine ＋ EBADF",
    );
}

#[test]
fn ipv6_loopback_connect_accept_send_recv() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    let (listener, client, accepted) = connected_pair_ipv6(&table);

    // 双向收发：都按已知字节数收，不会无限阻塞。
    assert_eq!(do_send(&table, client, b"ping6").expect("send ping6"), 5);
    assert_eq!(
        do_recv_exact(&table, accepted, 5).expect("recv ping6"),
        b"ping6".to_vec(),
    );
    assert_eq!(do_send(&table, accepted, b"pong6").expect("send pong6"), 5);
    assert_eq!(
        do_recv_exact(&table, client, 5).expect("recv pong6"),
        b"pong6".to_vec(),
    );

    // `getpeername(client)` ⇒ 监听方真实的 `[::1]:port`（wire 18B、out_family canonical 10）。
    let (listener_af, listener_wire) =
        do_sockname(&table, listener, false).expect("getsockname(listener)");
    assert_eq!(listener_af, family::AF_INET6);
    let (_, listener_port) = decode_ipv6(&listener_wire).expect("18 字节地址线");
    let (peer_af, peer_wire) = do_sockname(&table, client, true).expect("getpeername(client)");
    assert_eq!(peer_af, family::AF_INET6);
    let (peer_addr, peer_port) = decode_ipv6(&peer_wire).expect("18 字节地址线");
    assert_eq!(peer_addr, IPV6_LOOPBACK);
    assert_eq!(peer_port, listener_port, "客户端对端就是监听方地址");

    // `getsockname(client)`：本地地址是 `::1`（到 `::1` 的路由源地址），临时端口非 0。
    let (local_af, local_wire) = do_sockname(&table, client, false).expect("getsockname(client)");
    assert_eq!(local_af, family::AF_INET6);
    let (local_addr, local_port) = decode_ipv6(&local_wire).expect("18 字节地址线");
    assert_eq!(local_addr, IPV6_LOOPBACK);
    assert_ne!(local_port, 0);

    do_shutdown(&table, client, shutdown_how::SHUT_WR).expect("shutdown(SHUT_WR)");
    do_shutdown(&table, accepted, shutdown_how::SHUT_RDWR).expect("shutdown(SHUT_RDWR)");
    do_close(&table, client).expect("close client");
    do_close(&table, accepted).expect("close accepted");
    do_close(&table, listener).expect("close listener");
    assert_eq!(provider.open_handle_count(), 0);
}

// --------------------------------------------------------------------------- //
// 3) 选项翻译（setsockopt／getsockopt）
// --------------------------------------------------------------------------- //

#[test]
fn socket_options_are_translated_to_winsock() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    let socket = do_socket(&table, family::AF_INET, socket_type::SOCK_STREAM, 0).expect("socket");

    // `SO_REUSEADDR`：guest 的 POSIX 编号（`SOL_SOCKET=1`、`SO_REUSEADDR=2`）必须翻成 Winsock 的
    // （`0xFFFF`／`0x0004`）；值 `1` 是 int 的裸字节。
    let one = 1i32.to_ne_bytes();
    do_setsockopt(&table, socket, SOL_SOCKET_POSIX, SO_REUSEADDR_POSIX, &one)
        .expect("setsockopt(SO_REUSEADDR, 1)");

    // `SO_TYPE` ⇒ `SOCK_STREAM`(1)：两套 ABI 对这个选项的数值语义一致。
    let value =
        do_getsockopt(&table, socket, SOL_SOCKET_POSIX, SO_TYPE_POSIX).expect("getsockopt(SO_TYPE)");
    assert_eq!(value.len(), 4, "int 选项是 4 字节");
    let kind = i32::from_ne_bytes([value[0], value[1], value[2], value[3]]);
    assert_eq!(kind, socket_type::SOCK_STREAM, "SO_TYPE 回 SOCK_STREAM");

    // `SO_ERROR` ⇒ 0（还没有待取错误；provider 把 Winsock 错误码翻成 POSIX errno 数值）。
    let value = do_getsockopt(&table, socket, SOL_SOCKET_POSIX, SO_ERROR_POSIX)
        .expect("getsockopt(SO_ERROR)");
    let pending = i32::from_ne_bytes([value[0], value[1], value[2], value[3]]);
    assert_eq!(pending, 0, "新 socket 上没有待取错误");

    // 没接的选项按 `ENOPROTOOPT` 如实拒绝（CP-6：**不**原样转发 POSIX 选项号给 Winsock）。
    assert_eq!(
        do_setsockopt(&table, socket, SOL_SOCKET_POSIX, 999, &one),
        Err(host_errno("ENOPROTOOPT")),
        "未知选项 ⇒ Machine ＋ ENOPROTOOPT",
    );
    assert_eq!(
        do_getsockopt(&table, socket, SOL_SOCKET_POSIX, 999),
        Err(host_errno("ENOPROTOOPT")),
    );

    do_close(&table, socket).expect("close");
    assert_eq!(provider.open_handle_count(), 0);
}

#[test]
fn ipv6_v6only_is_translated_to_winsock() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    let socket = do_socket(&table, family::AF_INET6, socket_type::SOCK_STREAM, 0)
        .expect("AF_INET6 socket");
    // guest 的 `IPPROTO_IPV6=41` 与 Winsock 同值，但 `IPV6_V6ONLY` 是 **26 ⇒ 27**。
    let one = 1i32.to_ne_bytes();
    do_setsockopt(&table, socket, IPPROTO_IPV6_POSIX, IPV6_V6ONLY_POSIX, &one)
        .expect("setsockopt(IPV6_V6ONLY, 1)");
    let value = do_getsockopt(&table, socket, IPPROTO_IPV6_POSIX, IPV6_V6ONLY_POSIX)
        .expect("getsockopt(IPV6_V6ONLY)");
    assert!(value.len() >= 4, "Winsock 以 int 回该选项");
    let enabled = i32::from_ne_bytes([value[0], value[1], value[2], value[3]]);
    assert_eq!(enabled, 1, "IPV6_V6ONLY 回读为 1（未接的值会 ENOPROTOOPT）");
    do_close(&table, socket).expect("close");
    assert_eq!(provider.open_handle_count(), 0);
}

// --------------------------------------------------------------------------- //
// 4) 未知／已关句柄
// --------------------------------------------------------------------------- //

#[test]
fn unknown_and_closed_handles_report_ebadf() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    // 根本没分配过的句柄号。
    let ghost = SocketHandle(9999);
    assert_eq!(
        do_send(&table, ghost, b"x"),
        Err(host_errno("EBADF")),
        "未知句柄 send ⇒ Machine ＋ EBADF（CP-18）",
    );
    assert_eq!(
        do_close(&table, ghost),
        Err(host_errno("EBADF")),
        "未知句柄 close ⇒ Machine ＋ EBADF（CP-18）",
    );
    // 真实分配后关闭的句柄：句柄号不会被复用 ⇒ 同样 EBADF。
    let socket = do_socket(&table, family::AF_INET, socket_type::SOCK_STREAM, 0)
        .expect("socket");
    assert!(provider.open_handle_count() >= 1);
    do_close(&table, socket).expect("close");
    assert_eq!(do_send(&table, socket, b"x"), Err(host_errno("EBADF")));
    assert_eq!(do_close(&table, socket), Err(host_errno("EBADF")));
    assert_eq!(provider.open_handle_count(), 0);
}

// --------------------------------------------------------------------------- //
// 5) poll
// --------------------------------------------------------------------------- //

#[test]
fn poll_reports_readable_and_writable() {
    let provider = WindowsNet::new();
    let table = provider.vtable();
    let (listener, client, accepted) = connected_pair(&table);

    // 服务端先发一个字节 ⇒ 客户端可读；已连接的 TCP 客户端恒可写。
    assert_eq!(do_send(&table, accepted, b"!").expect("send"), 1);
    let fds = [PollFd {
        handle: client,
        events: Instance::POLLIN | Instance::POLLOUT,
        revents: 0,
    }];

    // 两段式出参：容量 0 时 `out_len` 仍回就绪项**总数**，但不写项。
    let (empty, total) = do_poll(&table, &fds, 2_000_000_000, 0).expect("poll(capacity=0)");
    assert_eq!(total, 1, "两段式：out_len 回就绪项总数");
    assert!(empty.is_empty(), "容量 0 时一项也不写");

    // 正常容量：读、写两个事件都应就绪。
    let (ready, total) = do_poll(&table, &fds, 2_000_000_000, fds.len()).expect("poll");
    assert_eq!(total, 1, "只有 1 个 socket 就绪");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].handle, client, "出参按输入序带回句柄");
    let revents = ready[0].revents;
    assert_ne!(revents & Instance::POLLIN, 0, "对端已发字节 ⇒ POLLIN 就绪");
    assert_ne!(revents & Instance::POLLOUT, 0, "已连接 TCP ⇒ POLLOUT 就绪");
    assert_eq!(revents & Instance::POLLERR, 0, "没有错误条件");
    assert_eq!(revents & Instance::POLLHUP, 0, "没有挂断");

    // 排空后 `timeout=0` 轮询：读不再就绪，写仍就绪。
    assert_eq!(
        do_recv_exact(&table, client, 1).expect("recv"),
        b"!".to_vec(),
    );
    let (ready, total) = do_poll(&table, &fds, 0, fds.len()).expect("poll(timeout=0)");
    assert_eq!(total, 1, "写就绪应当仍是唯一就绪项");
    assert_ne!(ready[0].revents & Instance::POLLOUT, 0, "仍可写");
    assert_eq!(ready[0].revents & Instance::POLLIN, 0, "已无数据可读");

    // 未知句柄 ⇒ Machine ＋ EBADF（CP-18）。
    let ghost = [PollFd {
        handle: SocketHandle(9999),
        events: Instance::POLLIN,
        revents: 0,
    }];
    assert_eq!(
        do_poll(&table, &ghost, 0, 1),
        Err(host_errno("EBADF")),
    );

    do_close(&table, client).expect("close client");
    do_close(&table, accepted).expect("close accepted");
    do_close(&table, listener).expect("close listener");
    assert_eq!(provider.open_handle_count(), 0);
}
