//! **`P3-29` socket I/O 第二片**：AF_INET 的 `bind`／`listen`／`accept`／`connect`／
//! `getsockname`／`getpeername`／`setsockopt`／`shutdown`／`sendall`／超时族 真接线。
//!
//! 路径：`_socket.socket` 方法 → `pyawa-core::Instance::net_*` → `net` 能力域 vtable（`CP-39` 的
//! AF_INET 线格式：4B 地址 ＋ 2B 端口大端）→ `crates/pyawa-runtime/src/net_posix.rs` 的真 syscall。
//! **单线程回环**即可验证（listen → connect → accept 全在一个进程里 ✓）。
//!
//! **如实未接（台账记）** ✗：IPv6／AF_UNIX 地址（`CP-39` v1 只有 AF_INET；其它族 ⇒
//! `OSError(EAFNOSUPPORT)`，`test.support.socket_helper` 的 `except OSError` 能接住 ✓）、
//! 真超时语义（`settimeout` 只记值，收发一律阻塞）、`sendto`／`recvfrom`／`getsockopt`。
//! 连带修：`super()` 的属性查找补**描述符协议**（`Lib/socket.py` 的 `@property family` 读
//! `super().family` 先前拿到 property 对象 ✗）。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-socket-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// TCP 回环：bind／listen／getsockname／connect／accept／sendall／recv／close。
const SOCKET_LOOPBACK_SCRIPT: &str = r#"import socket

server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", 0))
server.listen(1)
host, port = server.getsockname()
print("bound", host, port > 0, server.family == socket.AF_INET, server.type == socket.SOCK_STREAM)

client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
client.connect((host, port))
conn, addr = server.accept()
print("accepted", addr[0] == "127.0.0.1", addr[1] > 0, conn.family == socket.AF_INET)

conn.sendall(b"ping")
print("recv", client.recv(4))
client.sendall(b"pong")
print("recv2", conn.recv(4))
print("peername-differs", conn.getpeername() != conn.getsockname())

conn.close()
client.close()
server.close()
print("closed", server.fileno())"#;

/// 参照输出（逐行）。
const SOCKET_LOOPBACK_EXPECTED: &str = "bound 127.0.0.1 True True True\naccepted True True True\nrecv b'ping'\nrecv2 b'pong'\npeername-differs True\nclosed -1\n";

#[test]
fn tcp_loopback_matches_the_reference() {
    assert_eq!(run("loopback", SOCKET_LOOPBACK_SCRIPT), SOCKET_LOOPBACK_EXPECTED);
}
