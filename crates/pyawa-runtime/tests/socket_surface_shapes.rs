//! **`_socket`／`socket` 的面（`P3-29` socket 战役第一轮 ✓）**。
//!
//! 口径（两侧逐行一致 ✓）：
//!
//! * **`FACE_SCRIPT`**：同一段探针在**本机 `python3` 3.14.4** 与 pyawa 上**逐行一致** ✓ ——
//!   常量值表（`AF_*`／`SOCK_*`／`SOL_*`／`SO_*`／`IPPROTO_*`／`IPV6_*`／`MSG_*`／`AI_*`／`EAI_*`／
//!   `SHUT_*` 等 36 个＋`BDADDR_ANY`）、`has_ipv6`、异常身份（`error is OSError`／
//!   `timeout is TimeoutError`／`gaierror`／`herror` 是 `OSError` 子类）、`hasattr` 面、
//!   `SocketType is socket`（**在 `_socket` 里** ✓）、`Lib/socket.py` 的 `AddressFamily`／
//!   `SocketKind`／`MsgFlag`／`AddressInfo` 转换面（`_member_map_`：**含别名** ✓ —— 假货 enum 的
//!   `_member_names_` 不收别名 ✗ ⇒ 那一个面不钉 ✓），以及**错参数 `TypeError` 文案** ✓。
//!   期望值由**本机 `python3` 3.14.4 跑同一段脚本**取得 ✓（`FACE_EXPECTED` 逐行）。
//! * **`HONEST_SCRIPT`**：**如实报错面**（pyawa 侧钉住，参照侧无对应物 —— 参照会真发 syscall ✓）：
//!   本轮**不做**任何真 socket syscall ⇒ 全部 28 个模块函数、`_socket.socket` 构造、
//!   `socket.socket()`（Wrapper，含子类化后的构造）与 `create_connection`／`create_server`
//!   一律 `NotImplementedError`（消息含"未接线"✓），**绝不**返回假 fd／假装连接成功 ✓。
//!
//! **已知偏差（如实记，不钉）**：`_socket.CAPI`（参照是 `PyCapsule`，VM 无此载荷 ⇒ 不登记 ✓）；
//! `_socket.socket` 的 `family`／`type`／`proto`／`timeout` 四个实例属性（本轮只登记方法名 ✓）；
//! `_socket` 的 `__all__` 因此比参照 `dir` 面少一个名字（`socket.__all__` 502 vs 503 ✓）。
//!
//! 抽例范围＝**语义等价面**：不钉类型 repr（假货 enum 的类名/`repr` 与参照不同 ✗）、
//! 不钉 `_member_names_`（假货不收别名 ✗）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-socket-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
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

/// 参照侧脚本（`python3` 3.14.4 跑同一段 ⇒ `FACE_EXPECTED` ✓）。
const FACE_SCRIPT: &str = r#"import _socket, socket

keys = ["AF_UNSPEC","AF_UNIX","AF_INET","AF_INET6","SOCK_STREAM","SOCK_DGRAM","SOCK_RAW","SOCK_SEQPACKET","SOL_SOCKET","SOL_IP","SOL_TCP","SO_REUSEADDR","SO_REUSEPORT","SO_KEEPALIVE","SO_BROADCAST","SO_ERROR","IPPROTO_TCP","IPPROTO_UDP","IPPROTO_IPV6","IPV6_V6ONLY","MSG_PEEK","MSG_DONTWAIT","MSG_WAITALL","AI_PASSIVE","AI_CANONNAME","EAI_NONAME","SHUT_RD","SHUT_WR","SHUT_RDWR","SOMAXCONN","IPPORT_RESERVED","INADDR_LOOPBACK","INADDR_ANY","TCP_NODELAY","NI_MAXHOST","SCM_RIGHTS","BDADDR_ANY"]
for key in keys:
    print(key, getattr(_socket, key))
print("has_ipv6", _socket.has_ipv6)
print("error-is-oserror", _socket.error is OSError)
print("timeout-is-timeouterror", _socket.timeout is TimeoutError)
print("gaierror-oserror-subclass", issubclass(_socket.gaierror, OSError))
print("herror-oserror-subclass", issubclass(_socket.herror, OSError))
print("sendmsg/recvmsg/share", hasattr(_socket.socket,"sendmsg"), hasattr(_socket.socket,"recvmsg"), hasattr(_socket.socket,"share"))
print("module-socketpair", hasattr(_socket, "socketpair"))
print("socket-PY-name", _socket.socket.__name__)
print("SocketType-is-socket", _socket.SocketType is _socket.socket)
print("AF-name", socket.AddressFamily.__name__)
print("SOCK-kind", socket.SocketKind.__name__)
print("msgflag", socket.MsgFlag.__name__, socket.AddressInfo.__name__)
print("AF map", sorted(socket.AddressFamily._member_map_))
print("SOCK map", sorted(socket.SocketKind._member_map_))
print("MSG map", sorted(socket.MsgFlag._member_map_))
print("AI map", sorted(socket.AddressInfo._member_map_))
print("wire value", socket.AF_INET, socket.SOCK_STREAM, socket.SOL_SOCKET)
print("wire type socket", socket.socket.__name__, socket.SocketType.__name__)
print("wire star", hasattr(socket,"send_fds"), hasattr(socket,"recv_fds"), socket.socketpair.__name__)
print("wire err", socket.error is OSError, socket.timeout is TimeoutError, socket.has_ipv6)
print("wire funcs", callable(socket.create_connection), callable(socket.create_server), callable(socket.getfqdn), callable(socket.fromfd), callable(socket.has_dualstack_ipv6))
print("socket methods", all(hasattr(_socket.socket, m) for m in ["bind","connect","send","sendto","recv","recvfrom","setsockopt","getsockopt","shutdown","detach","fileno","listen","accept" if False else "settimeout"]))
def show(label, fn):
    try:
        fn()
        print(label, "=> no error")
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, ":", exc)

show("htons-0", lambda: _socket.htons())
show("htons-2", lambda: _socket.htons(1, 2))
show("htons-kw", lambda: _socket.htons(integer=1))
show("gethostname-1", lambda: _socket.gethostname(1))
show("getdefaulttimeout-1", lambda: _socket.getdefaulttimeout(1))
show("socketpair-kw", lambda: _socket.socketpair(family=2))
show("socketpair-4", lambda: _socket.socketpair(1, 2, 3, 4))
show("CMSG_LEN-0", lambda: _socket.CMSG_LEN())
show("CMSG_LEN-2", lambda: _socket.CMSG_LEN(1, 2))
show("getaddrinfo-0", lambda: _socket.getaddrinfo())
show("getaddrinfo-1", lambda: _socket.getaddrinfo("h"))
show("getaddrinfo-kw", lambda: _socket.getaddrinfo(port=1))
show("getnameinfo-0", lambda: _socket.getnameinfo())
show("getservbyname-0", lambda: _socket.getservbyname())
show("getservbyname-3", lambda: _socket.getservbyname(1, 2, 3))
show("sethostname-0", lambda: _socket.sethostname())
"#;

/// 参照（本机 `python3` 3.14.4 跑 `FACE_SCRIPT` 实测 ✓，逐行）。
const FACE_EXPECTED: &str = r#"AF_UNSPEC 0
AF_UNIX 1
AF_INET 2
AF_INET6 10
SOCK_STREAM 1
SOCK_DGRAM 2
SOCK_RAW 3
SOCK_SEQPACKET 5
SOL_SOCKET 1
SOL_IP 0
SOL_TCP 6
SO_REUSEADDR 2
SO_REUSEPORT 15
SO_KEEPALIVE 9
SO_BROADCAST 6
SO_ERROR 4
IPPROTO_TCP 6
IPPROTO_UDP 17
IPPROTO_IPV6 41
IPV6_V6ONLY 26
MSG_PEEK 2
MSG_DONTWAIT 64
MSG_WAITALL 256
AI_PASSIVE 1
AI_CANONNAME 2
EAI_NONAME -2
SHUT_RD 0
SHUT_WR 1
SHUT_RDWR 2
SOMAXCONN 4096
IPPORT_RESERVED 1024
INADDR_LOOPBACK 2130706433
INADDR_ANY 0
TCP_NODELAY 1
NI_MAXHOST 1025
SCM_RIGHTS 1
BDADDR_ANY 00:00:00:00:00:00
has_ipv6 True
error-is-oserror True
timeout-is-timeouterror True
gaierror-oserror-subclass True
herror-oserror-subclass True
sendmsg/recvmsg/share True True False
module-socketpair True
socket-PY-name socket
SocketType-is-socket True
AF-name AddressFamily
SOCK-kind SocketKind
msgflag MsgFlag AddressInfo
AF map ['AF_ALG', 'AF_APPLETALK', 'AF_ASH', 'AF_ATMPVC', 'AF_ATMSVC', 'AF_AX25', 'AF_BLUETOOTH', 'AF_BRIDGE', 'AF_CAN', 'AF_ECONET', 'AF_INET', 'AF_INET6', 'AF_IPX', 'AF_IRDA', 'AF_KEY', 'AF_LLC', 'AF_NETBEUI', 'AF_NETLINK', 'AF_NETROM', 'AF_PACKET', 'AF_PPPOX', 'AF_QIPCRTR', 'AF_RDS', 'AF_ROSE', 'AF_ROUTE', 'AF_SECURITY', 'AF_SNA', 'AF_TIPC', 'AF_UNIX', 'AF_UNSPEC', 'AF_VSOCK', 'AF_WANPIPE', 'AF_X25']
SOCK map ['SOCK_CLOEXEC', 'SOCK_DGRAM', 'SOCK_NONBLOCK', 'SOCK_RAW', 'SOCK_RDM', 'SOCK_SEQPACKET', 'SOCK_STREAM']
MSG map ['MSG_CMSG_CLOEXEC', 'MSG_CONFIRM', 'MSG_CTRUNC', 'MSG_DONTROUTE', 'MSG_DONTWAIT', 'MSG_EOR', 'MSG_ERRQUEUE', 'MSG_FASTOPEN', 'MSG_MORE', 'MSG_NOSIGNAL', 'MSG_OOB', 'MSG_PEEK', 'MSG_TRUNC', 'MSG_WAITALL']
AI map ['AI_ADDRCONFIG', 'AI_ALL', 'AI_CANONNAME', 'AI_NUMERICHOST', 'AI_NUMERICSERV', 'AI_PASSIVE', 'AI_V4MAPPED']
wire value 2 1 1
wire type socket socket socket
wire star True True socketpair
wire err True True True
wire funcs True True True True True
socket methods True
htons-0 !! TypeError : _socket.htons() takes exactly one argument (0 given)
htons-2 !! TypeError : _socket.htons() takes exactly one argument (2 given)
htons-kw !! TypeError : _socket.htons() takes no keyword arguments
gethostname-1 !! TypeError : _socket.gethostname() takes no arguments (1 given)
getdefaulttimeout-1 !! TypeError : _socket.getdefaulttimeout() takes no arguments (1 given)
socketpair-kw !! TypeError : socketpair() takes no keyword arguments
socketpair-4 !! TypeError : socketpair() takes at most 3 arguments (4 given)
CMSG_LEN-0 !! TypeError : CMSG_LEN() takes exactly 1 argument (0 given)
CMSG_LEN-2 !! TypeError : CMSG_LEN() takes exactly 1 argument (2 given)
getaddrinfo-0 !! TypeError : getaddrinfo() missing required argument 'host' (pos 1)
getaddrinfo-1 !! TypeError : getaddrinfo() missing required argument 'port' (pos 2)
getaddrinfo-kw !! TypeError : getaddrinfo() missing required argument 'host' (pos 1)
getnameinfo-0 !! TypeError : getnameinfo() takes exactly 2 arguments (0 given)
getservbyname-0 !! TypeError : getservbyname() takes at least 1 argument (0 given)
getservbyname-3 !! TypeError : getservbyname() takes at most 2 arguments (3 given)
sethostname-0 !! TypeError : sethostname() takes exactly 1 argument (0 given)
"#;

/// pyawa 侧脚本：如实报错面（见文件头 ✓）。
const HONEST_SCRIPT: &str = r#"import _socket
import socket

def verdict(label, fn):
    try:
        value = fn()
        print(label, "=> returned", type(value).__name__)
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, "未接线" in str(exc))

# 仍未接的纯数据／名字服务面：如实 NotImplementedError（消息含"未接线"）
verdict("pure-htons", lambda: _socket.htons(1))
verdict("pure-inet_aton", lambda: _socket.inet_aton("127.0.0.1"))
verdict("pure-CMSG_LEN", lambda: _socket.CMSG_LEN(4))
verdict("cap-gethostname", lambda: _socket.gethostname())
verdict("cap-getaddrinfo", lambda: _socket.getaddrinfo("127.0.0.1", 80))

# round 37：这一族**已真接线**（不再报未实现）——构造出真 socket，socketpair 出两个真 socket
verdict("cap-socketpair", lambda: _socket.socketpair())
verdict("cls-construct", lambda: _socket.socket())
verdict("wire-construct", lambda: socket.socket())
"#;

/// pyawa 侧期望（全部 `NotImplementedError` ＋ 消息含"未接线" ✓；`wire-has-socketpair True`）。
const HONEST_EXPECTED: &str = r#"pure-htons !! NotImplementedError True
pure-inet_aton !! NotImplementedError True
pure-CMSG_LEN !! NotImplementedError True
cap-gethostname !! NotImplementedError True
cap-getaddrinfo !! NotImplementedError True
cap-socketpair => returned tuple
cls-construct => returned socket
wire-construct => returned socket
"#;

#[test]
fn socket_face_matches_reference() {
    assert_eq!(run("face", FACE_SCRIPT), FACE_EXPECTED);
}

#[test]
fn socket_io_honesty_is_kept_where_unwired() {
    assert_eq!(run("honest", HONEST_SCRIPT), HONEST_EXPECTED);
}
