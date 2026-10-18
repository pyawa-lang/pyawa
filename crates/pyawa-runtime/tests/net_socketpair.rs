//! `net` 域 provider 级验收（`P3-29` 首片）：`CpNetVtable` 的 `socketpair` 真 I/O。
//!
//! 只经 **provider 的 vtable** 调用（不经过 VM／stdlib）⇒ 验的是能力接口那层的形状与语义：
//! - `CP-38`：`socketpair` 返回**一对**能力，local-only；
//! - `CP-17`／`CP-18`／`T-CP-12`：`close` **幂等**；已关句柄再操作 ⇒ **机器错误**（`EBADF` 形状）；
//! - `CP-5`：成功／机器错误/未实现三条通道可分（未实现槽位 `None`）；
//! - `CP-30`／`AB-39`：`version` ＝ `PA_ABI_VERSION`、`size` ＝ 结构体字节数。
//!
//! `send`／`recv` 走阻塞 `fd`；本文件不会在"对端无数据"时 `recv`（除 EOF 与容量 0 两例）。

use pyawa_capabilities::net::{CapStatus, CpNetVtable, SocketHandle};
use pyawa_runtime::net_posix::PosixNet;

/// 按**名字**取宿主 errno（`CM-20`：映射按名字，不硬编码数字）。
fn errno_named(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .expect("宿主 errno 表里应当有这个常量")
}

fn pair(provider: &PosixNet, family: i32, ty: i32) -> Result<(SocketHandle, SocketHandle), i32> {
    let table = provider.vtable();
    let socketpair = table.socketpair.expect("本片应当提供 socketpair 槽");
    let mut a = SocketHandle(0);
    let mut b = SocketHandle(0);
    let mut errno = 0i32;
    match socketpair(table.state, family, ty, 0, &mut a, &mut b, &mut errno) {
        CapStatus::Ok => Ok((a, b)),
        CapStatus::Machine => Err(errno),
        CapStatus::Unimplemented => panic!("socketpair 槽不应是未实现"),
    }
}

fn send(provider: &PosixNet, handle: SocketHandle, bytes: &[u8]) -> Result<usize, i32> {
    let table = provider.vtable();
    let send = table.send.expect("本片应当提供 send 槽");
    let mut sent = 0usize;
    let mut errno = 0i32;
    match send(table.state, handle, bytes.as_ptr(), bytes.len(), &mut sent, &mut errno) {
        CapStatus::Ok => Ok(sent),
        CapStatus::Machine => Err(errno),
        CapStatus::Unimplemented => panic!("send 槽不应是未实现"),
    }
}

fn recv(provider: &PosixNet, handle: SocketHandle, capacity: usize) -> Result<Vec<u8>, i32> {
    let table = provider.vtable();
    let recv = table.recv.expect("本片应当提供 recv 槽");
    let mut buffer = vec![0u8; capacity];
    let mut received = 0usize;
    let mut errno = 0i32;
    match recv(table.state, handle, buffer.as_mut_ptr(), capacity, &mut received, &mut errno) {
        CapStatus::Ok => {
            buffer.truncate(received);
            Ok(buffer)
        }
        CapStatus::Machine => Err(errno),
        CapStatus::Unimplemented => panic!("recv 槽不应是未实现"),
    }
}

fn close(provider: &PosixNet, handle: SocketHandle) -> Result<(), i32> {
    let table = provider.vtable();
    let close = table.close.expect("本片应当提供 close 槽");
    let mut errno = 0i32;
    match close(table.state, handle, &mut errno) {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        CapStatus::Unimplemented => panic!("close 槽不应是未实现"),
    }
}

/// `CP-30`／`AB-39`：provider 的 vtable 带**共用版本号**与自身尺寸；本片未实现的槽位如实为
/// `None`（`CP-3`）。
#[test]
fn the_vtable_declares_the_shared_abi_version_and_its_own_size() {
    let provider = PosixNet::new();
    let table: CpNetVtable = provider.vtable();
    assert_eq!(table.version, pyawa_abi::PA_ABI_VERSION, "CP-30／AB-39：共用版本号");
    assert_eq!(table.size as usize, core::mem::size_of::<CpNetVtable>(), "AB-43 的尺寸字段");
    assert!(!table.state.is_null(), "state 必须指回 provider");
    // 本片落地四槽
    assert!(table.socketpair.is_some() && table.send.is_some() && table.recv.is_some());
    assert!(table.close.is_some());
    // 本片**未**落地的槽位：留 None（CP-3），不是假装成功
    assert!(table.connect.is_none(), "connect 不在首片");
    assert!(table.bind.is_none(), "bind 不在首片");
    assert!(table.listen.is_none(), "本片没有 listen 槽");
    assert!(table.poll.is_none(), "poll 不在首片");
}

/// `CP-38`：`socketpair` 之后 **send／recv 真往返**（AF_UNIX ＋ SOCK_STREAM）。
#[test]
fn socketpair_roundtrips_bytes_through_the_capability_slots() {
    let provider = PosixNet::new();
    let (a, b) = pair(&provider, pyawa_capabilities::net::family::AF_UNIX,
                      pyawa_capabilities::net::socket_type::SOCK_STREAM)
        .expect("AF_UNIX 上 socketpair 必须成功");
    assert_ne!(a, b, "CP-38：必须返回**一对**能力（两个不同句柄）");
    assert_eq!(send(&provider, a, b"hi").expect("send"), 2);
    assert_eq!(recv(&provider, b, 2).expect("recv"), b"hi");
    // 反方向也要通（两个能力地位对称）
    assert_eq!(send(&provider, b, b"back").expect("send"), 4);
    assert_eq!(recv(&provider, a, 4).expect("recv"), b"back");
    close(&provider, a).expect("close a");
    close(&provider, b).expect("close b");
}

/// `CP-38` 的 `type` 由内核判定：`AF_UNIX` 上 `SOCK_DGRAM` 也是一对可收发能力。
#[test]
fn socketpair_works_for_dgram_on_unix() {
    let provider = PosixNet::new();
    let (a, b) = pair(
        &provider,
        pyawa_capabilities::net::family::AF_UNIX,
        pyawa_capabilities::net::socket_type::SOCK_DGRAM,
    )
    .expect("AF_UNIX/SOCK_DGRAM 的 socketpair 必须成功");
    assert_eq!(send(&provider, a, b"datagram").expect("send"), 8);
    assert_eq!(recv(&provider, b, 8).expect("recv"), b"datagram");
    close(&provider, a).expect("close a");
    close(&provider, b).expect("close b");
}

/// `T-CP-12`／`CP-17`／`CP-18`：`close` **幂等**；已关句柄的**再操作**返回机器错误（`EBADF` 形状）。
#[test]
fn close_is_idempotent_and_closed_sockets_report_ebadf() {
    let provider = PosixNet::new();
    let (a, b) = pair(
        &provider,
        pyawa_capabilities::net::family::AF_UNIX,
        pyawa_capabilities::net::socket_type::SOCK_STREAM,
    )
    .expect("pair");
    let ebadf = errno_named("EBADF");

    close(&provider, a).expect("第一次 close");
    close(&provider, a).expect("CP-17：close 必须幂等（第二次仍成功）");

    assert_eq!(send(&provider, a, b"x"), Err(ebadf), "已关句柄 send ⇒ EBADF 形状");
    assert_eq!(recv(&provider, a, 1), Err(ebadf), "已关句柄 recv ⇒ EBADF 形状");

    // 未知句柄：close 幂等成功；其它操作 EBADF
    let unknown = SocketHandle(999_999);
    close(&provider, unknown).expect("CP-17：未知句柄的 close 也按幂等处理");
    assert_eq!(send(&provider, unknown, b"x"), Err(ebadf));

    // 一个句柄关闭不影响另一个：b 仍可操作（对端已关 ⇒ recv 是 EOF）
    assert_eq!(recv(&provider, b, 1).expect("b 仍可用（EOF 是成功）"), b"");
    close(&provider, b).expect("close b");
}

/// `CP-6`（无权限语义：拒绝要给真实机器错误）＋ `CP-39`（v1 只支持 `AF_INET`／`AF_UNIX`）：
/// `AF_INET` 的 `socketpair` 在 Linux 上由内核拒绝 ⇒ 机器错误、两个出参**都不写**。
#[test]
fn unsupported_family_is_a_machine_error_and_writes_no_handles() {
    let provider = PosixNet::new();
    let table = provider.vtable();
    let socketpair = table.socketpair.expect("socketpair 槽");
    let mut a = SocketHandle(0);
    let mut b = SocketHandle(0);
    let mut errno = 0i32;
    let status = socketpair(
        table.state,
        pyawa_capabilities::net::family::AF_INET,
        pyawa_capabilities::net::socket_type::SOCK_STREAM,
        0,
        &mut a,
        &mut b,
        &mut errno,
    );
    assert_eq!(status, CapStatus::Machine, "AF_INET 的 socketpair 内核会拒绝");
    assert_eq!(errno, errno_named("EOPNOTSUPP"), "真实机器错误（不是'未实现'编码）");
    assert_eq!(a, SocketHandle(0), "失败时禁止写出参");
    assert_eq!(b, SocketHandle(0), "失败时禁止写出参");
}

/// `CP-5`：未实现的槽位用 `None` 表示（不是某个 errno）；本片没有 `poll`／`connect`。
#[test]
fn unimplemented_slots_are_absent_not_fake_success() {
    let provider = PosixNet::new();
    let table = provider.vtable();
    for (name, present) in [
        ("connect", table.connect.is_some()),
        ("bind", table.bind.is_some()),
        ("listen", table.listen.is_some()),
        ("accept", table.accept.is_some()),
        ("shutdown", table.shutdown.is_some()),
        ("getsockname", table.getsockname.is_some()),
        ("getpeername", table.getpeername.is_some()),
        ("getsockopt", table.getsockopt.is_some()),
        ("setsockopt", table.setsockopt.is_some()),
        ("poll", table.poll.is_some()),
    ] {
        assert!(!present, "{name} 不在本片（CP-3：留 None 即未实现）");
    }
}

/// `socket` 槽（`CP-3` 同域其余槽不受影响）：能申请一枚 socket；对端已关的流上 `send` 得
/// **机器错误** `EPIPE` 而不是把进程用 `SIGPIPE` 杀掉（provider 的 `MSG_NOSIGNAL` 口径）。
#[test]
fn socket_slot_creates_a_socket_and_send_does_not_kill_the_process() {
    let provider = PosixNet::new();
    let table = provider.vtable();
    let socket = table.socket.expect("socket 槽");
    let mut handle = SocketHandle(0);
    let mut errno = 0i32;
    let status = socket(
        table.state,
        pyawa_capabilities::net::family::AF_UNIX,
        pyawa_capabilities::net::socket_type::SOCK_STREAM,
        0,
        &mut handle,
        &mut errno,
    );
    assert_eq!(status, CapStatus::Ok, "AF_UNIX/SOCK_STREAM 的 socket(2) 应当成功");
    // 未连接的流 socket 上 send ⇒ 真实机器错误（Linux：ENOTCONN；有的内核给 EPIPE）。
    // 关键：进程活着才走得到这行 —— provider 用 MSG_NOSIGNAL 抑制了 SIGPIPE。
    match send(&provider, handle, b"x") {
        Err(errno) => assert!(
            errno == errno_named("ENOTCONN") || errno == errno_named("EPIPE"),
            "应当是真实机器错误，实际 errno {errno}"
        ),
        Ok(_) => panic!("未连接的流 socket 上 send 不应成功"),
    }
    close(&provider, handle).expect("close");
}

/// `CP-18` 的正面：对端关闭后 `recv` 是 EOF（成功 ＋ 0 字节），不是机器错误。
#[test]
fn recv_after_peer_close_is_eof() {
    let provider = PosixNet::new();
    let (a, b) = pair(
        &provider,
        pyawa_capabilities::net::family::AF_UNIX,
        pyawa_capabilities::net::socket_type::SOCK_STREAM,
    )
    .expect("pair");
    close(&provider, a).expect("关掉对端");
    assert_eq!(recv(&provider, b, 4).expect("EOF 是成功"), b"");
    close(&provider, b).expect("close b");
}

// ---- 端到端（CLI 子进程）：`P3-29` 首片的真验收 ------------------------------------------------
//
// 走**与用户完全相同的路径**（`.py` 源文件 → CLI → import 系统 → `Lib/socket.py` → `_socket` →
// `net` 能力域 → provider 的 `socketpair(2)`），比对退出码与 stdout/stderr。

use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_pyawa")
}

fn run_socket_script(tag: &str, source: &str) -> std::process::Output {
    let directory = std::env::temp_dir().join(format!(
        "pyawa-socketpair-e2e-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("建临时目录");
    let path = directory.join("script.py");
    std::fs::write(&path, source).expect("写脚本");
    let output = Command::new(binary()).arg(&path).output().expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&directory);
    output
}

fn assert_ok(output: &std::process::Output, tag: &str) {
    assert!(
        output.status.success(),
        "{tag}: 退出状态 {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `import socket; a,b=socket.socketpair(); a.send(b"hi"); assert b.recv(2)==b"hi"` —— 真往返。
#[test]
fn e2e_socket_socketpair_send_recv_roundtrip() {
    let output = run_socket_script(
        "roundtrip",
        r#"import socket

a, b = socket.socketpair()
assert isinstance(a, socket.socket), type(a)
assert isinstance(b, socket.socket), type(b)
sent = a.send(b"hi")
assert sent == 2, sent
assert b.recv(2) == b"hi"
assert a.fileno() >= 0
assert b.fileno() >= 0
a.close()
b.close()
print("roundtrip ok")
"#,
    );
    assert_ok(&output, "roundtrip");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("roundtrip ok"),
        "stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// `T-CP-12` 的 Python 侧形状：关掉之后再 `send` ⇒ `OSError`（`EBADF`）；`close` 幂等。
#[test]
fn e2e_closed_socket_raises_oserror_ebadf_and_close_is_idempotent() {
    let output = run_socket_script(
        "ebadf",
        r#"import socket

a, b = socket.socketpair()
a.close()
a.close()
try:
    a.send(b"x")
except OSError:
    pass
else:
    raise AssertionError("send after close must raise OSError")
b.close()
print("ebadf ok")
"#,
    );
    assert_ok(&output, "ebadf");
}

/// `_socket` 面的低层形状：`socketpair()` 返回两个**对象**、`fileno()` 是 int、
/// `detach()` 交出 fd 并把对象置为 `fileno() == -1`（参照语义 ✓）。
#[test]
fn e2e_socketpair_returns_objects_and_detach_hands_out_an_int() {
    let output = run_socket_script(
        "detach",
        r#"import _socket

a, b = _socket.socketpair()
first = a.fileno()
assert isinstance(first, int), type(first)
detached = b.detach()
assert isinstance(detached, int), type(detached)
assert b.fileno() == -1, b.fileno()
a.close()
_socket_close = getattr(_socket, "close", None)
assert _socket_close is not None
print("detach ok")
"#,
    );
    assert_ok(&output, "detach");
}
