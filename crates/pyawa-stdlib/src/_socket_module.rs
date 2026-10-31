//! `_socket` 模块（**P3-29 socket 战役第一轮** ✓）：`Lib/socket.py` 的 import 面。
//!
//! 上游 `socket.py` 在**导入期**要的恰好是：`import _socket` ＋ `from _socket import *`
//! （⇒ 本模块的 `__all__` 面）＋ `class socket(_socket.socket)`（可被子类化的类对象）
//! ＋ `hasattr(_socket.socket, "sendmsg"/"recvmsg")` 两个分支探测 ＋ `hasattr(_socket, "socketpair")`。
//! 本模块把这面补齐，**且把后续 `Lib/` 会查到的一整片常量一次到位**（census 42 个模块压着这一族 ✓）。
//!
//! ## 真面（本轮落地 ✓）
//!
//! - **456 个整型常量** ＋ `BDADDR_ANY`／`BDADDR_LOCAL` 两个字符串 ＋ **`has_ipv6`**
//!   —— 值**逐个照参照实测抄录**（本机 `python3` 3.14.4／Linux：`P3-29` 的"逐条对参照"口径 ✓）。
//!   这张表是**唯一真相点** ✓：将来按 `errno` 那套"宿主注入"口径收口时只需改这里。
//! - `socket` 类（参照里 `SocketType is socket` ✓）、`error`／`timeout`／`gaierror`／`herror`
//!   异常面：**存在性照参照**（`error is OSError`、`timeout is TimeoutError`、
//!   `gaierror`／`herror` 是 `OSError` 子类 ✓）。
//! - **28 个模块函数的名字与参数形态**：错参数报与参照同型的 `TypeError`
//!   （`_socket.htons()` ⇒ `takes exactly one argument (0 given)` 一类 ✓）。
//!
//! ## 本轮接线面（`P3-29` 首片 ✓：`socketpair` 真 I/O）
//!
//! - **`_socket.socketpair(family=AF_UNIX, type=SOCK_STREAM, proto=0)`**：经 `net` 能力域的
//!   `socketpair` 槽（`CP-38`）真发 `socketpair(2)`，返回**两个** `_socket.socket` 实例
//!   （参照实测：返回的是 socket 对象而非裸 int ✓；`Lib/socket.py` 的 `socketpair()` 依赖
//!   它们的 `detach()` ✓）。
//! - **`_socket.socket` 的构造／`fileno`／`detach`／`send`／`recv`／`close`**：真接线
//!   （状态放**实例属性** `_fd`／`_family`／`_type`／`_proto`，同 `_io`／`array` 先例 ✓）。
//!   `fd` 是 `net` provider 的不透明句柄原样（`CP-31` 的统一实例能力表**未**落地，见
//!   `crates/pyawa-core/src/instance/net.rs` 的模块文档）。
//! - 机器错误 → `OSError` 子类（按 `errno_map` 的**名字**映射 ✓，`CM-19`／`CM-20`）。
//!
//! ## 如实报错面（硬边界 ✓：**禁止假装连接成功**）
//!
//! 网络／描述符 I/O 属**能力域**（`CM-8`：外部世界必须经能力层）。本片**只**接 `socketpair`
//! 一族；其余仍**不做**真 syscall，也不返回假 fd：
//!
//! - **其余模块函数**（`getaddrinfo`／`gethostname`／`create_connection` 一族…）与
//!   `_socket.socket` 的其余方法（`bind`／`connect`／`sendto`…）：参数对了也**调用即**
//!   `NotImplementedError`（纯数据的那几个 —— `htons` 一族／`inet_*`／`CMSG_*` —— 同样如实报
//!   "未接线"，**不返回假值**）；
//! - `flags != 0` 的 `send`／`recv`：如实报未接线（首片只做 flags=0）。
//!
//! ## 未落地（如实记）
//!
//! - `CAPI`：参照是 `PyCapsule`，VM 没有这个载荷 ⇒ **不登记**（不是"给个假的" ✗）。
//! - `socket` 实例属性 `family`／`type`／`proto`／`timeout`：只登记**方法名**；
//!   属性存取会 `AttributeError`（`Lib/socket.py` 只在运行期 `__repr__` 里用它们 ✓）。
//! - `connect`／`bind`／`listen`／`accept`／`sendto`／`recvfrom`／`getsockopt`／`poll` 一族
//!   ＝ `P3-29` 后续片（要地址线 `CP-39` 与 `poll` 槽）。
//!
//! 本模块**不碰平台**（`CX-4`：`pyawa-stdlib/src` 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）。

use core::ptr::NonNull;

use pyawa_core::{build_class_from_parts, Header, Instance, NativeFn};

/// 模块名（`_socket`）。
pub const NAME: &str = "_socket";

/// 模块 `__doc__`（与参照同源的一句话 ✓）。
pub const DOC: &str = "Implementation module for socket operations.\n\nSee the socket module for documentation.";

/// 整型常量（**值照参照实测** ✓：本机 CPython 3.14.4／Linux）。
const INT_CONSTANTS: &[(&str, i64)] = &[
    ("AF_ALG", 38), ("AF_APPLETALK", 5), ("AF_ASH", 18), ("AF_ATMPVC", 8),
    ("AF_ATMSVC", 20), ("AF_AX25", 3), ("AF_BLUETOOTH", 31), ("AF_BRIDGE", 7),
    ("AF_CAN", 29), ("AF_DECnet", 12), ("AF_ECONET", 19), ("AF_INET", 2),
    ("AF_INET6", 10), ("AF_IPX", 4), ("AF_IRDA", 23), ("AF_KEY", 15),
    ("AF_LLC", 26), ("AF_NETBEUI", 13), ("AF_NETLINK", 16), ("AF_NETROM", 6),
    ("AF_PACKET", 17), ("AF_PPPOX", 24), ("AF_QIPCRTR", 42), ("AF_RDS", 21),
    ("AF_ROSE", 11), ("AF_ROUTE", 16), ("AF_SECURITY", 14), ("AF_SNA", 22),
    ("AF_TIPC", 30), ("AF_UNIX", 1), ("AF_UNSPEC", 0), ("AF_VSOCK", 40),
    ("AF_WANPIPE", 25), ("AF_X25", 9), ("AI_ADDRCONFIG", 32), ("AI_ALL", 16),
    ("AI_CANONNAME", 2), ("AI_NUMERICHOST", 4), ("AI_NUMERICSERV", 1024), ("AI_PASSIVE", 1),
    ("AI_V4MAPPED", 8), ("ALG_OP_DECRYPT", 0), ("ALG_OP_ENCRYPT", 1), ("ALG_OP_SIGN", 2),
    ("ALG_OP_VERIFY", 3), ("ALG_SET_AEAD_ASSOCLEN", 4), ("ALG_SET_AEAD_AUTHSIZE", 5), ("ALG_SET_IV", 2),
    ("ALG_SET_KEY", 1), ("ALG_SET_OP", 3), ("ALG_SET_PUBKEY", 6), ("BDADDR_BREDR", 0),
    ("BDADDR_LE_PUBLIC", 1), ("BDADDR_LE_RANDOM", 2), ("BTPROTO_HCI", 1), ("BTPROTO_L2CAP", 0),
    ("BTPROTO_RFCOMM", 3), ("BTPROTO_SCO", 2), ("BT_CHANNEL_POLICY", 10), ("BT_CHANNEL_POLICY_BREDR_ONLY", 0),
    ("BT_CHANNEL_POLICY_BREDR_PREFERRED", 1), ("BT_CODEC", 19), ("BT_DEFER_SETUP", 7), ("BT_FLUSHABLE", 8),
    ("BT_FLUSHABLE_OFF", 0), ("BT_FLUSHABLE_ON", 1), ("BT_ISO_QOS", 17), ("BT_MODE", 15),
    ("BT_MODE_BASIC", 0), ("BT_MODE_ERTM", 1), ("BT_MODE_EXT_FLOWCTL", 4), ("BT_MODE_LE_FLOWCTL", 3),
    ("BT_MODE_STREAMING", 2), ("BT_PHY", 14), ("BT_PHY_BR_1M_1SLOT", 1), ("BT_PHY_BR_1M_3SLOT", 2),
    ("BT_PHY_BR_1M_5SLOT", 4), ("BT_PHY_EDR_2M_1SLOT", 8), ("BT_PHY_EDR_2M_3SLOT", 16), ("BT_PHY_EDR_2M_5SLOT", 32),
    ("BT_PHY_EDR_3M_1SLOT", 64), ("BT_PHY_EDR_3M_3SLOT", 128), ("BT_PHY_EDR_3M_5SLOT", 256), ("BT_PHY_LE_1M_RX", 1024),
    ("BT_PHY_LE_1M_TX", 512), ("BT_PHY_LE_2M_RX", 4096), ("BT_PHY_LE_2M_TX", 2048), ("BT_PHY_LE_CODED_RX", 16384),
    ("BT_PHY_LE_CODED_TX", 8192), ("BT_PKT_STATUS", 16), ("BT_POWER", 9), ("BT_POWER_FORCE_ACTIVE_OFF", 0),
    ("BT_POWER_FORCE_ACTIVE_ON", 1), ("BT_RCVMTU", 13), ("BT_SECURITY", 4), ("BT_SECURITY_FIPS", 4),
    ("BT_SECURITY_HIGH", 3), ("BT_SECURITY_LOW", 1), ("BT_SECURITY_MEDIUM", 2), ("BT_SECURITY_SDP", 0),
    ("BT_SNDMTU", 12), ("BT_VOICE", 11), ("BT_VOICE_CVSD_16BIT", 96), ("BT_VOICE_TRANSPARENT", 3),
    ("BT_VOICE_TRANSPARENT_16BIT", 99), ("CAN_BCM", 2), ("CAN_BCM_CAN_FD_FRAME", 2048), ("CAN_BCM_RX_ANNOUNCE_RESUME", 256),
    ("CAN_BCM_RX_CHANGED", 12), ("CAN_BCM_RX_CHECK_DLC", 64), ("CAN_BCM_RX_DELETE", 6), ("CAN_BCM_RX_FILTER_ID", 32),
    ("CAN_BCM_RX_NO_AUTOTIMER", 128), ("CAN_BCM_RX_READ", 7), ("CAN_BCM_RX_RTR_FRAME", 1024), ("CAN_BCM_RX_SETUP", 5),
    ("CAN_BCM_RX_STATUS", 10), ("CAN_BCM_RX_TIMEOUT", 11), ("CAN_BCM_SETTIMER", 1), ("CAN_BCM_STARTTIMER", 2),
    ("CAN_BCM_TX_ANNOUNCE", 8), ("CAN_BCM_TX_COUNTEVT", 4), ("CAN_BCM_TX_CP_CAN_ID", 16), ("CAN_BCM_TX_DELETE", 2),
    ("CAN_BCM_TX_EXPIRED", 9), ("CAN_BCM_TX_READ", 3), ("CAN_BCM_TX_RESET_MULTI_IDX", 512), ("CAN_BCM_TX_SEND", 4),
    ("CAN_BCM_TX_SETUP", 1), ("CAN_BCM_TX_STATUS", 8), ("CAN_EFF_FLAG", 2147483648), ("CAN_EFF_MASK", 536870911),
    ("CAN_ERR_FLAG", 536870912), ("CAN_ERR_MASK", 536870911), ("CAN_ISOTP", 6), ("CAN_J1939", 7),
    ("CAN_RAW", 1), ("CAN_RAW_ERR_FILTER", 2), ("CAN_RAW_FD_FRAMES", 5), ("CAN_RAW_FILTER", 1),
    ("CAN_RAW_JOIN_FILTERS", 6), ("CAN_RAW_LOOPBACK", 3), ("CAN_RAW_RECV_OWN_MSGS", 4), ("CAN_RTR_FLAG", 1073741824),
    ("CAN_SFF_MASK", 2047), ("EAI_ADDRFAMILY", -9), ("EAI_AGAIN", -3), ("EAI_BADFLAGS", -1),
    ("EAI_FAIL", -4), ("EAI_FAMILY", -6), ("EAI_MEMORY", -10), ("EAI_NODATA", -5),
    ("EAI_NONAME", -2), ("EAI_OVERFLOW", -12), ("EAI_SERVICE", -8), ("EAI_SOCKTYPE", -7),
    ("EAI_SYSTEM", -11), ("ETHERTYPE_ARP", 2054), ("ETHERTYPE_IP", 2048), ("ETHERTYPE_IPV6", 34525),
    ("ETHERTYPE_VLAN", 33024), ("ETH_P_ALL", 3), ("HCI_CHANNEL_CONTROL", 3), ("HCI_CHANNEL_LOGGING", 4),
    ("HCI_CHANNEL_MONITOR", 2), ("HCI_CHANNEL_RAW", 0), ("HCI_CHANNEL_USER", 1), ("HCI_DATA_DIR", 1),
    ("HCI_DEV_NONE", 65535), ("HCI_FILTER", 2), ("HCI_TIME_STAMP", 3), ("INADDR_ALLHOSTS_GROUP", 3758096385),
    ("INADDR_ANY", 0), ("INADDR_BROADCAST", 4294967295), ("INADDR_LOOPBACK", 2130706433), ("INADDR_MAX_LOCAL_GROUP", 3758096639),
    ("INADDR_NONE", 4294967295), ("INADDR_UNSPEC_GROUP", 3758096384), ("IOCTL_VM_SOCKETS_GET_LOCAL_CID", 1977), ("IPPORT_RESERVED", 1024),
    ("IPPORT_USERRESERVED", 5000), ("IPPROTO_AH", 51), ("IPPROTO_DSTOPTS", 60), ("IPPROTO_EGP", 8),
    ("IPPROTO_ESP", 50), ("IPPROTO_FRAGMENT", 44), ("IPPROTO_GRE", 47), ("IPPROTO_HOPOPTS", 0),
    ("IPPROTO_ICMP", 1), ("IPPROTO_ICMPV6", 58), ("IPPROTO_IDP", 22), ("IPPROTO_IGMP", 2),
    ("IPPROTO_IP", 0), ("IPPROTO_IPIP", 4), ("IPPROTO_IPV6", 41), ("IPPROTO_MPTCP", 262),
    ("IPPROTO_NONE", 59), ("IPPROTO_PIM", 103), ("IPPROTO_PUP", 12), ("IPPROTO_RAW", 255),
    ("IPPROTO_ROUTING", 43), ("IPPROTO_RSVP", 46), ("IPPROTO_SCTP", 132), ("IPPROTO_TCP", 6),
    ("IPPROTO_TP", 29), ("IPPROTO_UDP", 17), ("IPPROTO_UDPLITE", 136), ("IPV6_CHECKSUM", 7),
    ("IPV6_DONTFRAG", 62), ("IPV6_DSTOPTS", 59), ("IPV6_HOPLIMIT", 52), ("IPV6_HOPOPTS", 54),
    ("IPV6_JOIN_GROUP", 20), ("IPV6_LEAVE_GROUP", 21), ("IPV6_MULTICAST_HOPS", 18), ("IPV6_MULTICAST_IF", 17),
    ("IPV6_MULTICAST_LOOP", 19), ("IPV6_NEXTHOP", 9), ("IPV6_PATHMTU", 61), ("IPV6_PKTINFO", 50),
    ("IPV6_RECVDSTOPTS", 58), ("IPV6_RECVERR", 25), ("IPV6_RECVHOPLIMIT", 51), ("IPV6_RECVHOPOPTS", 53),
    ("IPV6_RECVPATHMTU", 60), ("IPV6_RECVPKTINFO", 49), ("IPV6_RECVRTHDR", 56), ("IPV6_RECVTCLASS", 66),
    ("IPV6_RTHDR", 57), ("IPV6_RTHDRDSTOPTS", 55), ("IPV6_RTHDR_TYPE_0", 0), ("IPV6_TCLASS", 67),
    ("IPV6_UNICAST_HOPS", 16), ("IPV6_V6ONLY", 26), ("IP_ADD_MEMBERSHIP", 35), ("IP_ADD_SOURCE_MEMBERSHIP", 39),
    ("IP_BIND_ADDRESS_NO_PORT", 24), ("IP_BLOCK_SOURCE", 38), ("IP_DEFAULT_MULTICAST_LOOP", 1), ("IP_DEFAULT_MULTICAST_TTL", 1),
    ("IP_DROP_MEMBERSHIP", 36), ("IP_DROP_SOURCE_MEMBERSHIP", 40), ("IP_FREEBIND", 15), ("IP_HDRINCL", 3),
    ("IP_MAX_MEMBERSHIPS", 20), ("IP_MULTICAST_IF", 32), ("IP_MULTICAST_LOOP", 34), ("IP_MULTICAST_TTL", 33),
    ("IP_OPTIONS", 4), ("IP_PKTINFO", 8), ("IP_RECVERR", 11), ("IP_RECVOPTS", 6),
    ("IP_RECVORIGDSTADDR", 20), ("IP_RECVRETOPTS", 7), ("IP_RECVTOS", 13), ("IP_RECVTTL", 12),
    ("IP_RETOPTS", 7), ("IP_TOS", 1), ("IP_TRANSPARENT", 19), ("IP_TTL", 2),
    ("IP_UNBLOCK_SOURCE", 37), ("J1939_EE_INFO_NONE", 0), ("J1939_EE_INFO_TX_ABORT", 1), ("J1939_FILTER_MAX", 512),
    ("J1939_IDLE_ADDR", 254), ("J1939_MAX_UNICAST_ADDR", 253), ("J1939_NLA_BYTES_ACKED", 1), ("J1939_NLA_PAD", 0),
    ("J1939_NO_ADDR", 255), ("J1939_NO_NAME", 0), ("J1939_NO_PGN", 262144), ("J1939_PGN_ADDRESS_CLAIMED", 60928),
    ("J1939_PGN_ADDRESS_COMMANDED", 65240), ("J1939_PGN_MAX", 262143), ("J1939_PGN_PDU1_MAX", 261888), ("J1939_PGN_REQUEST", 59904),
    ("L2CAP_LM", 3), ("L2CAP_LM_AUTH", 2), ("L2CAP_LM_ENCRYPT", 4), ("L2CAP_LM_MASTER", 1),
    ("L2CAP_LM_RELIABLE", 16), ("L2CAP_LM_SECURE", 32), ("L2CAP_LM_TRUSTED", 8), ("MSG_CMSG_CLOEXEC", 1073741824),
    ("MSG_CONFIRM", 2048), ("MSG_CTRUNC", 8), ("MSG_DONTROUTE", 4), ("MSG_DONTWAIT", 64),
    ("MSG_EOR", 128), ("MSG_ERRQUEUE", 8192), ("MSG_FASTOPEN", 536870912), ("MSG_MORE", 32768),
    ("MSG_NOSIGNAL", 16384), ("MSG_OOB", 1), ("MSG_PEEK", 2), ("MSG_TRUNC", 32),
    ("MSG_WAITALL", 256), ("NETLINK_CRYPTO", 21), ("NETLINK_DNRTMSG", 14), ("NETLINK_FIREWALL", 3),
    ("NETLINK_IP6_FW", 13), ("NETLINK_NFLOG", 5), ("NETLINK_ROUTE", 0), ("NETLINK_USERSOCK", 2),
    ("NETLINK_XFRM", 6), ("NI_DGRAM", 16), ("NI_IDN", 32), ("NI_MAXHOST", 1025),
    ("NI_MAXSERV", 32), ("NI_NAMEREQD", 8), ("NI_NOFQDN", 4), ("NI_NUMERICHOST", 1),
    ("NI_NUMERICSERV", 2), ("PACKET_BROADCAST", 1), ("PACKET_FASTROUTE", 6), ("PACKET_HOST", 0),
    ("PACKET_LOOPBACK", 5), ("PACKET_MULTICAST", 2), ("PACKET_OTHERHOST", 3), ("PACKET_OUTGOING", 4),
    ("PF_CAN", 29), ("PF_PACKET", 17), ("PF_RDS", 21), ("SCM_CREDENTIALS", 2),
    ("SCM_J1939_DEST_ADDR", 1), ("SCM_J1939_DEST_NAME", 2), ("SCM_J1939_ERRQUEUE", 4), ("SCM_J1939_PRIO", 3),
    ("SCM_RIGHTS", 1), ("SHUT_RD", 0), ("SHUT_RDWR", 2), ("SHUT_WR", 1),
    ("SOCK_CLOEXEC", 524288), ("SOCK_DGRAM", 2), ("SOCK_NONBLOCK", 2048), ("SOCK_RAW", 3),
    ("SOCK_RDM", 4), ("SOCK_SEQPACKET", 5), ("SOCK_STREAM", 1), ("SOL_ALG", 279),
    ("SOL_BLUETOOTH", 274), ("SOL_CAN_BASE", 100), ("SOL_CAN_RAW", 101), ("SOL_HCI", 0),
    ("SOL_IP", 0), ("SOL_L2CAP", 6), ("SOL_RDS", 276), ("SOL_RFCOMM", 18),
    ("SOL_SCO", 17), ("SOL_SOCKET", 1), ("SOL_TCP", 6), ("SOL_TIPC", 271),
    ("SOL_UDP", 17), ("SOMAXCONN", 4096), ("SO_ACCEPTCONN", 30), ("SO_BINDTODEVICE", 25),
    ("SO_BINDTOIFINDEX", 62), ("SO_BROADCAST", 6), ("SO_DEBUG", 1), ("SO_DOMAIN", 39),
    ("SO_DONTROUTE", 5), ("SO_ERROR", 4), ("SO_INCOMING_CPU", 49), ("SO_J1939_ERRQUEUE", 4),
    ("SO_J1939_FILTER", 1), ("SO_J1939_PROMISC", 2), ("SO_J1939_SEND_PRIO", 3), ("SO_KEEPALIVE", 9),
    ("SO_LINGER", 13), ("SO_MARK", 36), ("SO_OOBINLINE", 10), ("SO_ORIGINAL_DST", 80),
    ("SO_PASSCRED", 16), ("SO_PASSSEC", 34), ("SO_PEERCRED", 17), ("SO_PEERSEC", 31),
    ("SO_PRIORITY", 12), ("SO_PROTOCOL", 38), ("SO_RCVBUF", 8), ("SO_RCVLOWAT", 18),
    ("SO_RCVTIMEO", 20), ("SO_REUSEADDR", 2), ("SO_REUSEPORT", 15), ("SO_SNDBUF", 7),
    ("SO_SNDLOWAT", 19), ("SO_SNDTIMEO", 21), ("SO_TYPE", 3), ("SO_VM_SOCKETS_BUFFER_MAX_SIZE", 2),
    ("SO_VM_SOCKETS_BUFFER_MIN_SIZE", 1), ("SO_VM_SOCKETS_BUFFER_SIZE", 0), ("TCP_CC_INFO", 26), ("TCP_CONGESTION", 13),
    ("TCP_CORK", 3), ("TCP_DEFER_ACCEPT", 9), ("TCP_FASTOPEN", 23), ("TCP_FASTOPEN_CONNECT", 30),
    ("TCP_FASTOPEN_KEY", 33), ("TCP_FASTOPEN_NO_COOKIE", 34), ("TCP_INFO", 11), ("TCP_INQ", 36),
    ("TCP_KEEPCNT", 6), ("TCP_KEEPIDLE", 4), ("TCP_KEEPINTVL", 5), ("TCP_LINGER2", 8),
    ("TCP_MAXSEG", 2), ("TCP_MD5SIG", 14), ("TCP_MD5SIG_EXT", 32), ("TCP_NODELAY", 1),
    ("TCP_NOTSENT_LOWAT", 25), ("TCP_QUEUE_SEQ", 21), ("TCP_QUICKACK", 12), ("TCP_REPAIR", 19),
    ("TCP_REPAIR_OPTIONS", 22), ("TCP_REPAIR_QUEUE", 20), ("TCP_REPAIR_WINDOW", 29), ("TCP_SAVED_SYN", 28),
    ("TCP_SAVE_SYN", 27), ("TCP_SYNCNT", 7), ("TCP_THIN_DUPACK", 17), ("TCP_THIN_LINEAR_TIMEOUTS", 16),
    ("TCP_TIMESTAMP", 24), ("TCP_TX_DELAY", 37), ("TCP_ULP", 31), ("TCP_USER_TIMEOUT", 18),
    ("TCP_WINDOW_CLAMP", 10), ("TCP_ZEROCOPY_RECEIVE", 35), ("TIPC_ADDR_ID", 3), ("TIPC_ADDR_NAME", 2),
    ("TIPC_ADDR_NAMESEQ", 1), ("TIPC_CFG_SRV", 0), ("TIPC_CLUSTER_SCOPE", 2), ("TIPC_CONN_TIMEOUT", 130),
    ("TIPC_CRITICAL_IMPORTANCE", 3), ("TIPC_DEST_DROPPABLE", 129), ("TIPC_HIGH_IMPORTANCE", 2), ("TIPC_IMPORTANCE", 127),
    ("TIPC_LOW_IMPORTANCE", 0), ("TIPC_MEDIUM_IMPORTANCE", 1), ("TIPC_NODE_SCOPE", 3), ("TIPC_PUBLISHED", 1),
    ("TIPC_SRC_DROPPABLE", 128), ("TIPC_SUBSCR_TIMEOUT", 3), ("TIPC_SUB_CANCEL", 4), ("TIPC_SUB_PORTS", 1),
    ("TIPC_SUB_SERVICE", 2), ("TIPC_TOP_SRV", 1), ("TIPC_WAIT_FOREVER", -1), ("TIPC_WITHDRAWN", 2),
    ("TIPC_ZONE_SCOPE", 1), ("UDPLITE_RECV_CSCOV", 11), ("UDPLITE_SEND_CSCOV", 10), ("VMADDR_CID_ANY", 4294967295),
    ("VMADDR_CID_HOST", 2), ("VMADDR_CID_LOCAL", 1), ("VMADDR_PORT_ANY", 4294967295), ("VM_SOCKETS_INVALID_VERSION", 4294967295),
];

/// 字符串常量（参照实测 ✓）。
const STR_CONSTANTS: &[(&str, &str)] = &[
    ("BDADDR_ANY", "00:00:00:00:00:00"),
    ("BDADDR_LOCAL", "00:00:00:FF:FF:FF"),
];

/// 函数参数形态（**照参照实测的 TypeError 文案分型** ✓）。
#[derive(Clone, Copy)]
enum Arity {
    /// `METH_NOARGS`：参照文案带 `_socket.` 前缀（`_socket.gethostname() takes no arguments (1 given)`）。
    PrefixNoArgs,
    /// `METH_O`：参照文案带 `_socket.` 前缀、`one` 拼写（`_socket.htons() takes exactly one argument`）。
    PrefixExactlyOne,
    /// 数字版 exactly（`getnameinfo() takes exactly 2 arguments`，**不带** `_socket.` 前缀）。
    DigitExactly(usize),
    /// 两端都有界（`getservbyname`／`getservbyport`：`takes at least 1 argument`／
    /// `takes at most 2 arguments` ✓）。
    DigitBetween(usize, usize),
    /// `takes at most N argument(s)`（`socketpair` 的上界）。
    DigitAtMost(usize),
    /// `getaddrinfo` 的 clinic 形态（`host`／`port` 必填，关键字只认 6 个名字）。
    Getaddrinfo,
}

/// 未接线的如实报错类别（一处口径 ✓）。
#[derive(Clone, Copy)]
enum Unwired {
    /// 真 I/O ⇒ 能力域（`CM-8`）。
    Capability,
    /// 纯数据函数 ⇒ 同样"未接线"，**不返回假值**。
    Pure,
}

/// 28 个模块函数：名字 → 参数形态 → 实现 → 未接线类别（顺序照参照 `dir` 排序 ✓）。
const FUNCTIONS: &[(&str, Arity, NativeFn, Unwired)] = &[
    ("CMSG_LEN", Arity::DigitExactly(1), cmsg_len_stub, Unwired::Pure),
    ("CMSG_SPACE", Arity::DigitExactly(1), cmsg_space_stub, Unwired::Pure),
    ("close", Arity::PrefixExactlyOne, close_stub, Unwired::Capability),
    ("dup", Arity::PrefixExactlyOne, dup_stub, Unwired::Capability),
    ("getaddrinfo", Arity::Getaddrinfo, getaddrinfo_stub, Unwired::Capability),
    ("getdefaulttimeout", Arity::PrefixNoArgs, getdefaulttimeout_stub, Unwired::Capability),
    ("gethostbyaddr", Arity::DigitExactly(1), gethostbyaddr_stub, Unwired::Capability),
    ("gethostbyname", Arity::DigitExactly(1), gethostbyname_stub, Unwired::Capability),
    ("gethostbyname_ex", Arity::DigitExactly(1), gethostbyname_ex_stub, Unwired::Capability),
    ("gethostname", Arity::PrefixNoArgs, gethostname_stub, Unwired::Capability),
    ("getnameinfo", Arity::DigitExactly(2), getnameinfo_stub, Unwired::Capability),
    ("getprotobyname", Arity::DigitExactly(1), getprotobyname_stub, Unwired::Capability),
    ("getservbyname", Arity::DigitBetween(1, 2), getservbyname_stub, Unwired::Capability),
    ("getservbyport", Arity::DigitBetween(1, 2), getservbyport_stub, Unwired::Capability),
    ("htonl", Arity::PrefixExactlyOne, htonl_stub, Unwired::Pure),
    ("htons", Arity::PrefixExactlyOne, htons_stub, Unwired::Pure),
    ("if_indextoname", Arity::PrefixExactlyOne, if_indextoname_stub, Unwired::Capability),
    ("if_nameindex", Arity::PrefixNoArgs, if_nameindex_stub, Unwired::Capability),
    ("if_nametoindex", Arity::PrefixExactlyOne, if_nametoindex_stub, Unwired::Capability),
    ("inet_aton", Arity::PrefixExactlyOne, inet_aton_stub, Unwired::Pure),
    ("inet_ntoa", Arity::PrefixExactlyOne, inet_ntoa_stub, Unwired::Pure),
    ("inet_ntop", Arity::DigitExactly(2), inet_ntop_stub, Unwired::Pure),
    ("inet_pton", Arity::DigitExactly(2), inet_pton_stub, Unwired::Pure),
    ("ntohl", Arity::PrefixExactlyOne, ntohl_stub, Unwired::Pure),
    ("ntohs", Arity::PrefixExactlyOne, ntohs_stub, Unwired::Pure),
    ("setdefaulttimeout", Arity::PrefixExactlyOne, setdefaulttimeout_stub, Unwired::Capability),
    ("sethostname", Arity::DigitExactly(1), sethostname_stub, Unwired::Capability),
    ("socketpair", Arity::DigitAtMost(3), socketpair_native, Unwired::Capability),
];

/// `_socket.socket` 的**公开方法面**（照参照 `dir(_socket.socket)` ✓；只登记、调用如实报错）。
///
/// 含 `sendmsg`／`recvmsg`（参照 Linux 有 ⇒ `Lib/socket.py:557`／`569` 的两个
/// `hasattr` 分支会定义 `send_fds`／`recv_fds` ✓）；**不含** `share`（参照 Linux 没有 ✓）。
const SOCKET_METHODS: &[&str] = &[
    "bind",
    "close",
    "connect",
    "connect_ex",
    "detach",
    "fileno",
    "getblocking",
    "getpeername",
    "getsockname",
    "getsockopt",
    "gettimeout",
    "listen",
    "recv",
    "recv_into",
    "recvfrom",
    "recvfrom_into",
    "recvmsg",
    "recvmsg_into",
    "send",
    "sendall",
    "sendmsg",
    "sendmsg_afalg",
    "sendto",
    "setblocking",
    "setsockopt",
    "settimeout",
    "shutdown",
];

/// 建 `_socket` 的命名空间（**新引用**的 `dict`；`CM-6`：模块**存在**即可 import）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let mut exports: Vec<&'static str> = Vec::with_capacity(INT_CONSTANTS.len() + 8);
    for (name, value) in INT_CONSTANTS {
        instance.dict_set(namespace, name, instance.new_int(*value));
        exports.push(name);
    }
    for (name, value) in STR_CONSTANTS {
        instance.dict_set(namespace, name, instance.new_str(value));
        exports.push(name);
    }
    // `has_ipv6`：参照本机为 `True` ✓（Linux 构建带 IPv6）——**真值**，不是探测桩 ✓。
    instance.dict_set(namespace, "has_ipv6", instance.new_bool(true));
    exports.push("has_ipv6");
    for (name, _arity, handler, _unwired) in FUNCTIONS {
        let function = make_native(instance, name, *handler);
        instance.dict_set(namespace, name, function);
        exports.push(name);
    }
    // **默认超时两口**（`P3-29` 第二片）：`Lib/socket.py` 的 `accept()`／`socketpair()` 会调
    // `getdefaulttimeout()`；真超时语义未接（收发一律阻塞）⇒ 这里只做**值面**（默认 `None` ✓，
    // `setdefaulttimeout` 记值、不改行为 —— 台账如实记 ✗）。
    for (name, handler) in [
        ("getdefaulttimeout", socket_getdefaulttimeout_native as NativeFn),
        ("setdefaulttimeout", socket_setdefaulttimeout_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    // ---- 类与异常面 --------------------------------------------------------------
    if let Ok(socket_class) = build_socket_class(instance) {
        instance.dict_set(namespace, "socket", socket_class);
        // 参照实测：`_socket.SocketType is _socket.socket` ✓（同一对象，`dict_set` 借用计数 ✓）
        instance.dict_set(namespace, "SocketType", socket_class);
        exports.push("socket");
        exports.push("SocketType");
    }
    if let Some(oserror) = instance.type_named("OSError") {
        // 参照实测：`_socket.error is OSError` ✓
        instance.dict_set(namespace, "error", instance.type_value(oserror));
        exports.push("error");
        // `gaierror`／`herror`：参照是 `OSError` 子类（名字要存在；raise 面本轮用不到 ✓）
        for name in ["gaierror", "herror"] {
            if let Some(class) = build_exception_class(instance, name, instance.type_value(oserror)) {
                instance.dict_set(namespace, name, class);
                exports.push(name);
            }
        }
    }
    if let Some(timeout_error) = instance.type_named("TimeoutError") {
        // 参照实测：`_socket.timeout is TimeoutError` ✓
        instance.dict_set(namespace, "timeout", instance.type_value(timeout_error));
        exports.push("timeout");
    }
    // ---- 模块元数据 --------------------------------------------------------------
    let exports = instance.new_list(exports.iter().map(|name| instance.new_str(name)).collect());
    instance.dict_set(namespace, "__all__", exports);
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    namespace
}

/// 建 `socket` 类（照 `_random.Random` 的手法：`build_class_from_parts` ✓）。
///
/// 基类 `object`、方法全挂 native（**调用即如实报未接线**）。**可被子类化** ✓ ——
/// `Lib/socket.py:218` 的 `class socket(_socket.socket)` 靠它。
/// **`_socket.socket` 的只读数据属性**（`family`／`type`／`proto`）的 getter 工厂。
///
/// **为什么不是实例属性** ✗：`Lib/socket.py:524` 的
/// `@property def family(self): return _intenum_converter(super().family, AddressFamily)`
/// 读的是 **`super().family`** —— `super` 绕开实例字典，只看**类型 MRO** ⇒ 基类必须有
/// **类级数据描述符**（参照里是 C 的 getset）✓。这里用 `property`（本层已接描述符协议 ✓）
/// 包一个 native getter：读的是实例的 `_family`／`_type`／`_proto` ✓。
fn socket_readonly_property(
    instance: &Instance,
    getter: NativeFn,
) -> Option<NonNull<Header>> {
    let property_type = instance.type_named("property")?;
    let callable = make_native(instance, "get", getter);
    let outcome = pyawa_core::call_value(
        instance,
        instance.type_value(property_type).cast::<Header>(),
        &[callable],
        &[],
    );
    instance.release(callable);
    outcome.ok()
}

fn socket_family_getter(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(object) = args.first().copied() else {
        return Err(type_error(instance, "descriptor 'family' needs an argument".to_owned()));
    };
    let value = socket_int_attribute(instance, object, "_family")
        .unwrap_or(i64::from(constant("AF_INET")));
    Ok(instance.new_int(value))
}

fn socket_type_getter(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(object) = args.first().copied() else {
        return Err(type_error(instance, "descriptor 'type' needs an argument".to_owned()));
    };
    let value = socket_int_attribute(instance, object, "_type")
        .unwrap_or(i64::from(constant("SOCK_STREAM")));
    Ok(instance.new_int(value))
}

fn socket_proto_getter(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(object) = args.first().copied() else {
        return Err(type_error(instance, "descriptor 'proto' needs an argument".to_owned()));
    };
    let value = socket_int_attribute(instance, object, "_proto").unwrap_or(0);
    Ok(instance.new_int(value))
}

fn build_socket_class(instance: &Instance) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let base = instance
        .type_named("object")
        .expect("object 在引导期已登记")
        .cast::<Header>();
    let class_namespace = instance.new_dict();
    // **其余方法面**先按"如实未接线"登记（`P3-29` 首片只接 `socketpair` 一族 ✓）。
    for name in SOCKET_METHODS {
        let function = make_native(instance, name, socket_method_stub);
        instance.dict_set(class_namespace, name, function);
    }
    // **真 I/O 面**（`P3-29` 首片 ✓）：覆盖同名的如实桩 —— `Lib/socket.py` 的 `socketpair()`
    // 一族要的正好是这几个 ✓（状态放实例属性，见 `socket_init_native` 的注释 ✓）。
    for (name, handler) in [
        ("__init__", socket_init_native as NativeFn),
        ("close", socket_close_native as NativeFn),
        ("detach", socket_detach_native as NativeFn),
        ("fileno", socket_fileno_native as NativeFn),
        ("send", socket_send_native as NativeFn),
        ("recv", socket_recv_native as NativeFn),
        // **`P3-29` 第二片**：TCP 面向（`CP-39` 的 AF_INET 线格式；IPv6／UNIX 族如实
        // 回 `OSError(EAFNOSUPPORT)` —— 与"本机没有该地址族"的真实机器语义同形 ✓）。
        ("bind", socket_bind_native as NativeFn),
        ("connect", socket_connect_native as NativeFn),
        ("listen", socket_listen_native as NativeFn),
        ("accept", socket_accept_native as NativeFn),
        ("getsockname", socket_getsockname_native as NativeFn),
        ("getpeername", socket_getpeername_native as NativeFn),
        ("shutdown", socket_shutdown_native as NativeFn),
        ("setsockopt", socket_setsockopt_native as NativeFn),
        // **`Lib/socket.py` 的子类样板要的那几个**（`_accept`／超时族／`sendall`／`connect_ex`）。
        ("_accept", socket_accept_fd_native as NativeFn),
        ("gettimeout", socket_gettimeout_native as NativeFn),
        ("settimeout", socket_settimeout_native as NativeFn),
        ("setblocking", socket_setblocking_native as NativeFn),
        ("getblocking", socket_getblocking_native as NativeFn),
        ("sendall", socket_sendall_native as NativeFn),
        ("connect_ex", socket_connect_ex_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    for (name, getter) in [
        ("family", socket_family_getter as NativeFn),
        ("type", socket_type_getter as NativeFn),
        ("proto", socket_proto_getter as NativeFn),
    ] {
        if let Some(property) = socket_readonly_property(instance, getter) {
            instance.dict_set(class_namespace, name, property);
        }
    }
    build_class_from_parts(
        instance,
        "socket".to_owned(),
        vec![base],
        class_namespace,
        None,
    )
}

/// 建 `OSError` 的子类（`gaierror`／`herror` 的名字面 ✓）。
fn build_exception_class(
    instance: &Instance,
    name: &str,
    base: NonNull<Header>,
) -> Option<NonNull<Header>> {
    let class_namespace = instance.new_dict();
    build_class_from_parts(instance, name.to_owned(), vec![base], class_namespace, None).ok()
}

fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

// ---- 参数形态检查（照参照实测的文案分型 ✓） -----------------------------------------

fn type_error(instance: &Instance, text: String) -> pyawa_core::ExecError {
    instance.raise_builtin_error("TypeError", &text)
}

fn reject_keywords(
    instance: &Instance,
    name: &str,
    prefix: bool,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<(), pyawa_core::ExecError> {
    if kwargs.is_empty() {
        return Ok(());
    }
    let shown = if prefix {
        format!("_socket.{name}")
    } else {
        name.to_owned()
    };
    Err(type_error(
        instance,
        format!("{shown}() takes no keyword arguments"),
    ))
}

fn check_arity(
    instance: &Instance,
    name: &str,
    arity: Arity,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<(), pyawa_core::ExecError> {
    let given = args.len();
    match arity {
        Arity::PrefixNoArgs => {
            reject_keywords(instance, name, true, kwargs)?;
            if given != 0 {
                return Err(type_error(
                    instance,
                    format!("_socket.{name}() takes no arguments ({given} given)"),
                ));
            }
        }
        Arity::PrefixExactlyOne => {
            reject_keywords(instance, name, true, kwargs)?;
            if given != 1 {
                return Err(type_error(
                    instance,
                    format!("_socket.{name}() takes exactly one argument ({given} given)"),
                ));
            }
        }
        Arity::DigitExactly(expected) => {
            reject_keywords(instance, name, false, kwargs)?;
            if given != expected {
                let plural = if expected == 1 { "argument" } else { "arguments" };
                return Err(type_error(
                    instance,
                    format!("{name}() takes exactly {expected} {plural} ({given} given)"),
                ));
            }
        }
        Arity::DigitBetween(minimum, maximum) => {
            reject_keywords(instance, name, false, kwargs)?;
            if given < minimum {
                let plural = if minimum == 1 { "argument" } else { "arguments" };
                return Err(type_error(
                    instance,
                    format!("{name}() takes at least {minimum} {plural} ({given} given)"),
                ));
            }
            if given > maximum {
                let plural = if maximum == 1 { "argument" } else { "arguments" };
                return Err(type_error(
                    instance,
                    format!("{name}() takes at most {maximum} {plural} ({given} given)"),
                ));
            }
        }
        Arity::DigitAtMost(maximum) => {
            reject_keywords(instance, name, false, kwargs)?;
            if given > maximum {
                let plural = if maximum == 1 { "argument" } else { "arguments" };
                return Err(type_error(
                    instance,
                    format!("{name}() takes at most {maximum} {plural} ({given} given)"),
                ));
            }
        }
        Arity::Getaddrinfo => {
            check_getaddrinfo(instance, args, kwargs)?;
        }
    }
    Ok(())
}

/// `getaddrinfo` 的 clinic 形态：`host`／`port` 必填（位置或关键字），最多 6 个位置参数，
/// 关键字只认 `host`／`port`／`family`／`type`／`proto`／`flags`（照参照实测的报错顺序 ✓）。
fn check_getaddrinfo(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<(), pyawa_core::ExecError> {
    let keyword = |wanted: &str| {
        kwargs
            .iter()
            .any(|(key, _)| instance.text_of(*key).is_some_and(|text| text == wanted))
    };
    let has_host = !args.is_empty() || keyword("host");
    let has_port = args.len() >= 2 || keyword("port");
    if !has_host {
        return Err(type_error(
            instance,
            "getaddrinfo() missing required argument 'host' (pos 1)".to_owned(),
        ));
    }
    if !has_port {
        return Err(type_error(
            instance,
            "getaddrinfo() missing required argument 'port' (pos 2)".to_owned(),
        ));
    }
    if args.len() > 6 {
        return Err(type_error(
            instance,
            format!("getaddrinfo() takes at most 6 arguments ({} given)", args.len()),
        ));
    }
    for (key, _) in kwargs {
        let Some(text) = instance.text_of(*key) else {
            return Err(type_error(instance, "keywords must be strings".to_owned()));
        };
        if !matches!(text, "host" | "port" | "family" | "type" | "proto" | "flags") {
            return Err(type_error(
                instance,
                format!("getaddrinfo() got an unexpected keyword argument '{text}'"),
            ));
        }
    }
    Ok(())
}

// ---- 未接线的如实报错（**不返回假值、不假装成功** ✓） ---------------------------------

fn not_wired(instance: &Instance, name: &str, unwired: Unwired) -> pyawa_core::ExecError {
    let detail = match unwired {
        Unwired::Capability => "真实套接字系统调用／网络 I/O 属能力域（CM-8），本轮未接线",
        Unwired::Pure => "纯数据函数本轮未接线（只落名字/参数形态面；不返回假值，P3-29 后续轮接）",
    };
    instance.raise_builtin_error("NotImplementedError", &format!("`_socket.{name}`：{detail}"))
}

macro_rules! socket_stub_fn {
    ($function:ident, $name:literal, $arity:expr, $unwired:expr) => {
        fn $function(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            args: &[NonNull<Header>],
            kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, pyawa_core::ExecError> {
            check_arity(instance, $name, $arity, args, kwargs)?;
            Err(not_wired(instance, $name, $unwired))
        }
    };
}

socket_stub_fn!(cmsg_len_stub, "CMSG_LEN", Arity::DigitExactly(1), Unwired::Pure);
socket_stub_fn!(cmsg_space_stub, "CMSG_SPACE", Arity::DigitExactly(1), Unwired::Pure);
socket_stub_fn!(close_stub, "close", Arity::PrefixExactlyOne, Unwired::Capability);
socket_stub_fn!(dup_stub, "dup", Arity::PrefixExactlyOne, Unwired::Capability);
socket_stub_fn!(getaddrinfo_stub, "getaddrinfo", Arity::Getaddrinfo, Unwired::Capability);
socket_stub_fn!(getdefaulttimeout_stub, "getdefaulttimeout", Arity::PrefixNoArgs, Unwired::Capability);
socket_stub_fn!(gethostbyaddr_stub, "gethostbyaddr", Arity::DigitExactly(1), Unwired::Capability);
socket_stub_fn!(gethostbyname_stub, "gethostbyname", Arity::DigitExactly(1), Unwired::Capability);
socket_stub_fn!(gethostbyname_ex_stub, "gethostbyname_ex", Arity::DigitExactly(1), Unwired::Capability);
socket_stub_fn!(gethostname_stub, "gethostname", Arity::PrefixNoArgs, Unwired::Capability);
socket_stub_fn!(getnameinfo_stub, "getnameinfo", Arity::DigitExactly(2), Unwired::Capability);
socket_stub_fn!(getprotobyname_stub, "getprotobyname", Arity::DigitExactly(1), Unwired::Capability);
socket_stub_fn!(getservbyname_stub, "getservbyname", Arity::DigitBetween(1, 2), Unwired::Capability);
socket_stub_fn!(getservbyport_stub, "getservbyport", Arity::DigitBetween(1, 2), Unwired::Capability);
socket_stub_fn!(htonl_stub, "htonl", Arity::PrefixExactlyOne, Unwired::Pure);
socket_stub_fn!(htons_stub, "htons", Arity::PrefixExactlyOne, Unwired::Pure);
socket_stub_fn!(if_indextoname_stub, "if_indextoname", Arity::PrefixExactlyOne, Unwired::Capability);
socket_stub_fn!(if_nameindex_stub, "if_nameindex", Arity::PrefixNoArgs, Unwired::Capability);
socket_stub_fn!(if_nametoindex_stub, "if_nametoindex", Arity::PrefixExactlyOne, Unwired::Capability);
socket_stub_fn!(inet_aton_stub, "inet_aton", Arity::PrefixExactlyOne, Unwired::Pure);
socket_stub_fn!(inet_ntoa_stub, "inet_ntoa", Arity::PrefixExactlyOne, Unwired::Pure);
socket_stub_fn!(inet_ntop_stub, "inet_ntop", Arity::DigitExactly(2), Unwired::Pure);
socket_stub_fn!(inet_pton_stub, "inet_pton", Arity::DigitExactly(2), Unwired::Pure);
socket_stub_fn!(ntohl_stub, "ntohl", Arity::PrefixExactlyOne, Unwired::Pure);
socket_stub_fn!(ntohs_stub, "ntohs", Arity::PrefixExactlyOne, Unwired::Pure);
socket_stub_fn!(setdefaulttimeout_stub, "setdefaulttimeout", Arity::PrefixExactlyOne, Unwired::Capability);
socket_stub_fn!(sethostname_stub, "sethostname", Arity::DigitExactly(1), Unwired::Capability);

/// `_socket.socket` 的方法面：调用即如实报未接线（**不假装连接成功、不返回假 fd**）。
fn socket_method_stub(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_socket.socket` 的 I/O 方法：真实套接字系统调用属能力域（CM-8），本轮未接线",
    ))
}

// ==== `P3-29` 首片：真 I/O（`socketpair` 一族）====================================================
//
// 状态全放**实例属性**（`_fd`／`_family`／`_type`／`_proto`）：与 `_io.FileIO`／`array.array`
// 同一先例（**不**新增载荷类型 ✓；`AttributeObject` 的实例字典由类创建时的
// `mark_external_instance_dict` 提供 ✓）。
//
// `fd` 是 provider 的不透明句柄**原样** —— `CP-31` 的"实例能力表统一编号"未落地
// （见 `crates/pyawa-core/src/instance/net.rs` 的模块文档），本片如实记差距。

/// 本文件常量表里按名字取一个整数（`INT_CONSTANTS` 是**唯一真相** ✓，禁止在实现里另抄数字）。
fn constant(name: &str) -> i32 {
    INT_CONSTANTS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or(0)
}

/// `self` 可能在 **`bound`**（属性通道）或 **`args[0]`**（建实例的隐式调用）——两条都接。
fn this_of<'a>(
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> Option<(NonNull<Header>, &'a [NonNull<Header>])> {
    match bound {
        Some(this) => Some((this, args)),
        None => args.split_first().map(|(first, rest)| (*first, rest)),
    }
}

fn socket_this(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    match this_of(bound, args) {
        Some((this, _)) => Ok(this),
        None => Err(type_error(
            instance,
            "`_socket.socket` 方法缺少 self".to_owned(),
        )),
    }
}

fn is_none_value(instance: &Instance, value: NonNull<Header>) -> bool {
    value == instance.singletons().none()
}

/// 从实例属性读一个整数（缺失 ⇒ `None`）。
fn socket_int_attribute(instance: &Instance, this: NonNull<Header>, name: &str) -> Option<i64> {
    instance
        .attribute_optional_of(this, name)
        .ok()
        .flatten()
        .and_then(|value| instance.int_value(value))
}

/// 往实例属性写一个整数（`set_attribute_value` 是借用语义 ⇒ 自己那份要归还 ✓）。
fn set_socket_int(instance: &Instance, this: NonNull<Header>, name: &str, value: i64) {
    let object = instance.new_int(value);
    let outcome = instance.set_attribute_value(this, name, object);
    instance.release(object);
    let _ = outcome;
}

/// 一条 socket 当前的 `fd`（`-1` ⇒ 已 close／detach，参照的"无 fd"形态 ✓）。
///
/// `_family`／`_type`／`_proto` 也写进实例属性（构造时 ✓）——本片没有读它们的公开面
/// （参照的 `family`／`type`／`proto` 数据描述符不在首片），故不在这里取。
fn socket_fd(instance: &Instance, this: NonNull<Header>) -> i64 {
    socket_int_attribute(instance, this, "_fd").unwrap_or(-1)
}

/// 取整数实参；不是整数 ⇒ 照参照报 `TypeError`（`'str' object cannot be interpreted as an integer`）。
fn socket_int(instance: &Instance, value: NonNull<Header>) -> Result<i64, pyawa_core::ExecError> {
    if let Some(number) = instance.int_value(value) {
        return Ok(number);
    }
    let name = instance.type_name(instance.type_of(value));
    Err(type_error(
        instance,
        format!("'{name}' object cannot be interpreted as an integer"),
    ))
}

/// 机器错误 ⇒ 参照口径的 `OSError` 子类（`CM-19`：按 **errno 名**查表 ✓；`CM-20`：映射按名字 ✓）。
///
/// 这**不是**第二份 errno 表：数字 → 名字 → 类名全部走 `errno_map`（生成物 ✓）；
/// 只有 `strerror` 文案是本地一小张（`io_module` 的那张是私有的，跨模块不加宽可见性 ✓）。
fn net_error(
    instance: &Instance,
    error: pyawa_core::CapabilityCallError,
) -> pyawa_core::ExecError {
    match error {
        pyawa_core::CapabilityCallError::NotRegistered => instance.raise_builtin_error(
            "NotImplementedError",
            "`_socket`：`net` 能力域未注册（`CP-2`），本轮未接线",
        ),
        pyawa_core::CapabilityCallError::NotImplemented => instance.raise_builtin_error(
            "NotImplementedError",
            "`_socket`：`net` 域的该槽未实现（`CP-5`），本轮未接线",
        ),
        pyawa_core::CapabilityCallError::Machine(errno) => {
            let name = crate::errno_map::ERRNO_ERRORCODE
                .iter()
                .find(|(number, _)| *number == errno as i64)
                .map(|(_, name)| *name);
            let class = name
                .and_then(|name| {
                    crate::errno_map::ERRNO_TO_CLASS
                        .iter()
                        .find(|(key, _)| *key == name)
                        .map(|(_, class)| *class)
                })
                .unwrap_or("OSError");
            let text = name.and_then(socket_strerror).unwrap_or("unknown error");
            instance.raise_builtin_error(class, &format!("[Errno {errno}] {text}"))
        }
    }
}

fn socket_strerror(name: &str) -> Option<&'static str> {
    Some(match name {
        "EBADF" => "Bad file descriptor",
        "EOPNOTSUPP" => "Operation not supported",
        "ENOTCONN" => "Transport endpoint is not connected",
        "EPIPE" => "Broken pipe",
        "ECONNRESET" => "Connection reset by peer",
        "ECONNREFUSED" => "Connection refused",
        "EINVAL" => "Invalid argument",
        "EAFNOSUPPORT" => "Address family not supported by protocol",
        "EAGAIN" => "Resource temporarily unavailable",
        "EACCES" => "Permission denied",
        "EMFILE" => "Too many open files",
        "ENFILE" => "Too many open files in system",
        "ENOMEM" => "Cannot allocate memory",
        _ => return None,
    })
}

/// 已 close／detach 的 socket 上操作 ⇒ `OSError(EBADF)`（`CP-18` 的 Python 侧形状 ✓）。
fn raise_ebadf(instance: &Instance) -> pyawa_core::ExecError {
    let errno = instance
        .platform_constant("EBADF")
        .unwrap_or(9) as i32;
    net_error(instance, pyawa_core::CapabilityCallError::Machine(errno))
}

/// **`_socket.socket.__init__(self, family=-1, type=-1, proto=-1, fileno=None)`**。
///
/// - `fileno` 给了 ⇒ **接管**该 fd（`socketpair` 的 wrapper 路径：`a.detach()` 之后 ✓）；
/// - 没给 ⇒ 经 `net` 域的 `socket` 槽真发 `socket(2)`（`-1` 取参照默认 `AF_INET/SOCK_STREAM/0` ✓）。
fn socket_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some((this, positional)) = this_of(bound, args) else {
        return Err(type_error(
            instance,
            "`_socket.socket.__init__` 缺少 self".to_owned(),
        ));
    };
    if positional.len() > 4 {
        return Err(type_error(
            instance,
            format!(
                "`_socket.socket.__init__` takes at most 4 arguments ({} given)",
                positional.len()
            ),
        ));
    }
    // 关键字面：family／type／proto／fileno（照参照签名 ✓）；与位置参数重复 ⇒ TypeError ✓。
    let mut slots: [Option<NonNull<Header>>; 4] = [None; 4];
    const KEYWORDS: [&str; 4] = ["family", "type", "proto", "fileno"];
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            return Err(type_error(instance, "keywords must be strings".to_owned()));
        };
        let Some(index) = KEYWORDS.iter().position(|candidate| *candidate == name) else {
            return Err(type_error(
                instance,
                format!("`_socket.socket.__init__` got an unexpected keyword argument '{name}'"),
            ));
        };
        if slots[index].is_some() || positional.get(index).is_some() {
            return Err(type_error(
                instance,
                format!("`_socket.socket.__init__` got multiple values for argument '{name}'"),
            ));
        }
        slots[index] = Some(*value);
    }
    let argument = |index: usize| slots[index].or_else(|| positional.get(index).copied());
    // 缺省 -1（参照 `_socket` 的 C 签名默认值 ✓）；`None` 只对 fileno 有意义 ✓。
    let as_int = |value: Option<NonNull<Header>>, default: i64| -> Result<i64, pyawa_core::ExecError> {
        match value {
            Some(value) if !is_none_value(instance, value) => socket_int(instance, value),
            _ => Ok(default),
        }
    };
    let family = as_int(argument(0), -1)?;
    let kind = as_int(argument(1), -1)?;
    let protocol = as_int(argument(2), -1)?;
    let fileno_argument = argument(3).filter(|value| !is_none_value(instance, *value));
    let (fd, family, kind, protocol) = match fileno_argument {
        Some(value) => {
            let fd = socket_int(instance, value)?;
            (fd, family as i32, kind as i32, protocol as i32)
        }
        None => {
            let family = if family < 0 { constant("AF_INET") } else { family as i32 };
            let kind = if kind < 0 { constant("SOCK_STREAM") } else { kind as i32 };
            let protocol = if protocol < 0 { 0 } else { protocol as i32 };
            match instance.net_socket(family, kind, protocol) {
                Ok(handle) => (handle as i64, family, kind, protocol),
                Err(error) => return Err(net_error(instance, error)),
            }
        }
    };
    set_socket_int(instance, this, "_fd", fd);
    set_socket_int(instance, this, "_family", i64::from(family));
    set_socket_int(instance, this, "_type", i64::from(kind));
    set_socket_int(instance, this, "_proto", i64::from(protocol));
    Ok(instance.retain(instance.singletons().none()))
}

/// **`_socket.socket.fileno()`**：返回 fd（已 close／detach ⇒ `-1`，参照一致 ✓）。
fn socket_fileno_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let this = socket_this(instance, bound, args)?;
    Ok(instance.new_int(socket_fd(instance, this)))
}

/// **`_socket.socket.detach()`**：交出 fd 并把对象置为"无 fd"（**不** close ✓，参照语义）。
fn socket_detach_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let this = socket_this(instance, bound, args)?;
    let fd = socket_fd(instance, this);
    set_socket_int(instance, this, "_fd", -1);
    Ok(instance.new_int(fd))
}

/// **`_socket.socket.close()`**：经 `net` 域 `close` 槽释放（`CP-17` 幂等 ✓）。
fn socket_close_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let this = socket_this(instance, bound, args)?;
    let fd = socket_fd(instance, this);
    if fd >= 0 {
        if let Err(error) = instance.net_close(fd as u64) {
            return Err(net_error(instance, error));
        }
        set_socket_int(instance, this, "_fd", -1);
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`_socket.socket.send(data, flags=0)`**：经 `net` 域 `send` 槽发送；返回字节数。
fn socket_send_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(type_error(instance, "`_socket.socket.send` 缺少 self".to_owned()));
    };
    let Some(data_value) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!("send() takes at least 1 argument ({} given)", rest.len()),
        ));
    };
    // flags：首片只支持 0（其余如实报未接线，不假装支持 ✓）。
    let flags = match rest.get(1).copied() {
        Some(value) => socket_int(instance, value)?,
        None => 0,
    };
    if flags != 0 {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "`_socket.socket.send`：`flags != 0` 的真 I/O 本轮未接线（首片只做 flags=0）",
        ));
    }
    let Some(data) = instance.bytes_value(data_value) else {
        let name = instance.type_name(instance.type_of(data_value));
        return Err(type_error(
            instance,
            format!("a bytes-like object is required, not '{name}'"),
        ));
    };
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_send(fd as u64, data) {
        Ok(sent) => Ok(instance.new_int(sent as i64)),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// **`_socket.socket.recv(bufsize, flags=0)`**：经 `net` 域 `recv` 槽接收；返回 `bytes`。
fn socket_recv_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(type_error(instance, "`_socket.socket.recv` 缺少 self".to_owned()));
    };
    let Some(size_value) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!("recv() takes at least 1 argument ({} given)", rest.len()),
        ));
    };
    let size = socket_int(instance, size_value)?;
    if size < 0 {
        return Err(instance.raise_builtin_error("ValueError", "negative buffersize in recv"));
    }
    let flags = match rest.get(1).copied() {
        Some(value) => socket_int(instance, value)?,
        None => 0,
    };
    if flags != 0 {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "`_socket.socket.recv`：`flags != 0` 的真 I/O 本轮未接线（首片只做 flags=0）",
        ));
    }
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    let mut buffer = vec![0u8; size as usize];
    match instance.net_recv(fd as u64, &mut buffer) {
        Ok(received) => Ok(instance.new_bytes(&buffer[..received])),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// 取本 socket 的地址族（实例属性 `_family`；缺 ⇒ `AF_INET`）。
fn socket_family(instance: &Instance, this: NonNull<Header>) -> i32 {
    socket_int_attribute(instance, this, "_family").unwrap_or(i64::from(constant("AF_INET"))) as i32
}

/// **Python 地址元组 ⇒ `CP-39` 线格式**（v1 只有 `AF_INET`：`(host, port)`）。
///
/// `host` 是点分四段字符串（`""`／`None` ＝ `0.0.0.0`，参照的 INADDR_ANY 口径 ✓）；
/// 其它地址族 ⇒ `OSError(EAFNOSUPPORT)`（provider 的真实机器语义；IPv6 不在 `CP-39` v1 ✓）。
fn socket_address_wire(
    instance: &Instance,
    this: NonNull<Header>,
    value: NonNull<Header>,
    function: &str,
) -> Result<(i32, Vec<u8>), pyawa_core::ExecError> {
    let family = socket_family(instance, this);
    if family != constant("AF_INET") {
        let errno = instance.platform_constant("EAFNOSUPPORT").unwrap_or(97) as i32;
        return Err(net_error(
            instance,
            pyawa_core::CapabilityCallError::Machine(errno),
        ));
    }
    let Some(items) = instance.tuple_items(value) else {
        let name = instance.type_name(instance.type_of(value));
        return Err(type_error(
            instance,
            format!("{function}() AF_INET address must be tuple, not {name}"),
        ));
    };
    if items.len() < 2 {
        return Err(type_error(
            instance,
            format!("{function}() AF_INET address must be a pair"),
        ));
    }
    let mut address = [0u8; 4];
    match items[0] {
        host if is_none_value(instance, host) => {}
        host => {
            let Some(text) = instance.text_of(host) else {
                let name = instance.type_name(instance.type_of(host));
                return Err(type_error(
                    instance,
                    format!("{function}() AF_INET address must be a pair, not {name}"),
                ));
            };
            if !text.is_empty() {
                let mut seen = 0usize;
                for part in text.split('.') {
                    let Ok(number) = part.parse::<u8>() else {
                        return Err(net_error(
                            instance,
                            pyawa_core::CapabilityCallError::Machine(
                                instance.platform_constant("EINVAL").unwrap_or(22) as i32,
                            ),
                        ));
                    };
                    if seen >= 4 {
                        return Err(instance.raise_builtin_error("ValueError", "got more than 4 bytes in inet_aton()"));
                    }
                    address[seen] = number;
                    seen += 1;
                }
                if seen != 4 {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "invalid IPv4 address",
                    ));
                }
            }
        }
    }
    let port = socket_int(instance, items[1])?;
    if !(0..=65535).contains(&port) {
        return Err(instance.raise_builtin_error(
            "OverflowError",
            &format!("{function}(): port must be 0-65535."),
        ));
    }
    Ok((
        family,
        pyawa_capabilities::net::encode_ipv4(address, port as u16).to_vec(),
    ))
}

/// `CP-39` 线格式 ⇒ Python 地址元组 `(dotted, port)` ✓。
fn socket_address_tuple(
    instance: &Instance,
    wire: &[u8],
) -> NonNull<Header> {
    let (address, port) = pyawa_capabilities::net::decode_ipv4(wire).unwrap_or(([0u8; 4], 0));
    let text = instance.new_str(&format!(
        "{}.{}.{}.{}",
        address[0], address[1], address[2], address[3]
    ));
    let port = instance.new_int(i64::from(port));
    instance.new_tuple(vec![text, port])
}

fn socket_receiver<'a>(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
    function: &str,
) -> Result<(NonNull<Header>, &'a [NonNull<Header>]), pyawa_core::ExecError> {
    match this_of(bound, args) {
        Some((this, rest)) => Ok((this, rest)),
        None => Err(type_error(instance, format!("{function} 缺少 self"))),
    }
}

/// **`bind(address)`**：`AF_INET` 的 `CP-39` 线格式 ⇒ `net.bind` 槽。
fn socket_bind_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "bind()")?;
    let Some(address) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!("bind() takes exactly one argument ({} given)", rest.len()),
        ));
    };
    let (family, wire) = socket_address_wire(instance, this, address, "bind")?;
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_bind(fd as u64, family, &wire) {
        Ok(()) => Ok(instance.retain(instance.singletons().none())),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// **`connect(address)`**。
fn socket_connect_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "connect()")?;
    let Some(address) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!("connect() takes exactly one argument ({} given)", rest.len()),
        ));
    };
    let (family, wire) = socket_address_wire(instance, this, address, "connect")?;
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_connect(fd as u64, family, &wire) {
        Ok(()) => Ok(instance.retain(instance.singletons().none())),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// **`listen([backlog])`**（缺省 backlog ＝ 0，照参照签名 ✓）。
fn socket_listen_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "listen()")?;
    let backlog = match rest.first().copied() {
        Some(value) if !is_none_value(instance, value) => socket_int(instance, value)? as i32,
        _ => 0,
    };
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_listen(fd as u64, backlog) {
        Ok(()) => Ok(instance.retain(instance.singletons().none())),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// 造一个接管既有 fd 的 `_socket.socket`（`accept` 用；走类的 `fileno=` 属性 ✓）。
fn socket_from_fd(
    instance: &Instance,
    fd: u64,
    family: i32,
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(class) = socket_class_of(instance) else {
        return Err(instance.raise_builtin_error(
            "RuntimeError",
            "`_socket.socket` 未登记（`_socket` 模块的类面缺失）",
        ));
    };
    let fd_value = instance.new_int(fd as i64);
    let family_value = instance.new_int(i64::from(family));
    let kwargs = [
        (instance.new_str("fileno"), fd_value),
        (instance.new_str("family"), family_value),
    ];
    let outcome = pyawa_core::call_value(instance, class, &[], &kwargs);
    for (key, value) in kwargs {
        instance.release(key);
        instance.release(value);
    }
    outcome
}

/// **`accept()`** ⇒ `(conn_socket, address)`。
fn socket_accept_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, _rest) = socket_receiver(instance, bound, args, "accept()")?;
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_accept(fd as u64) {
        Ok((new_fd, family, wire)) => {
            let connection = socket_from_fd(instance, new_fd, family)?;
            let address = socket_address_tuple(instance, &wire);
            Ok(instance.new_tuple(vec![connection, address]))
        }
        Err(error) => Err(net_error(instance, error)),
    }
}

/// **`getsockname()`**。
fn socket_getsockname_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, _rest) = socket_receiver(instance, bound, args, "getsockname()")?;
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_getsockname(fd as u64) {
        Ok((_family, wire)) => Ok(socket_address_tuple(instance, &wire)),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// **`getpeername()`**。
fn socket_getpeername_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, _rest) = socket_receiver(instance, bound, args, "getpeername()")?;
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_getpeername(fd as u64) {
        Ok((_family, wire)) => Ok(socket_address_tuple(instance, &wire)),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// **`shutdown(how)`**（`SHUT_RD`／`SHUT_WR`／`SHUT_RDWR`）。
fn socket_shutdown_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "shutdown()")?;
    let Some(how) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!("shutdown() takes exactly one argument ({} given)", rest.len()),
        ));
    };
    let how = socket_int(instance, how)? as i32;
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_shutdown(fd as u64, how) {
        Ok(()) => Ok(instance.retain(instance.singletons().none())),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// **`setsockopt(level, optname, value)`**：整数选项按宿主 `int` 的字节形态转发（照 C 语义 ✓），
/// `bytes` 原样转发。
fn socket_setsockopt_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "setsockopt()")?;
    if rest.len() != 3 {
        return Err(type_error(
            instance,
            format!("setsockopt() takes exactly 3 arguments ({} given)", rest.len()),
        ));
    }
    let level = socket_int(instance, rest[0])? as i32;
    let name = socket_int(instance, rest[1])? as i32;
    let payload: Vec<u8> = if let Some(number) = instance.int_value(rest[2]) {
        (number as i32).to_ne_bytes().to_vec()
    } else if let Some(bytes) = instance.bytes_value(rest[2]) {
        bytes.to_vec()
    } else {
        let kind = instance.type_name(instance.type_of(rest[2]));
        return Err(type_error(
            instance,
            format!("a bytes-like object is required, not '{kind}'"),
        ));
    };
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_setsockopt(fd as u64, level, name, &payload) {
        Ok(()) => Ok(instance.retain(instance.singletons().none())),
        Err(error) => Err(net_error(instance, error)),
    }
}

/// `_timeout` 槽：`None` ＝ 阻塞（参照默认 ✓）。**如实** ✗：真超时（`SO_RCVTIMEO`／`select` 轮询）
/// 尚未接线 —— 本层收发一律阻塞；`settimeout` 只记值，不改行为（台账已记）。
fn socket_timeout_attribute(instance: &Instance, this: NonNull<Header>) -> Option<NonNull<Header>> {
    instance.attribute_optional_of(this, "_timeout").ok().flatten()
}

/// **`_accept()`**：`Lib/socket.py` 的 `accept()` 覆盖用它 ⇒ `(fd, address)`。
fn socket_accept_fd_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, _rest) = socket_receiver(instance, bound, args, "accept()")?;
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    match instance.net_accept(fd as u64) {
        Ok((new_fd, _family, wire)) => {
            let fd_value = instance.new_int(new_fd as i64);
            let address = socket_address_tuple(instance, &wire);
            Ok(instance.new_tuple(vec![fd_value, address]))
        }
        Err(error) => Err(net_error(instance, error)),
    }
}

/// **`gettimeout()`** ⇒ `None`（阻塞）／记录的秒数。
fn socket_gettimeout_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, _rest) = socket_receiver(instance, bound, args, "gettimeout()")?;
    Ok(socket_timeout_attribute(instance, this)
        .map(|value| instance.retain(value))
        .unwrap_or_else(|| instance.new_none()))
}

/// **`settimeout(value)`**：记值（`None` ⇒ 阻塞）；**不改真 I/O 行为**（如实 ✗，见上）。
fn socket_settimeout_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "settimeout()")?;
    let Some(value) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!(
                "settimeout() takes exactly one argument ({} given)",
                rest.len()
            ),
        ));
    };
    if !is_none_value(instance, value) && instance.float_value(value).is_none() {
        let name = instance.type_name(instance.type_of(value));
        return Err(type_error(
            instance,
            format!("'{name}' object cannot be interpreted as an integer"),
        ));
    }
    instance.set_attribute_value(this, "_timeout", value)?;
    if is_none_value(instance, value) {
        let none = instance.new_bool(false);
        instance.set_attribute_value(this, "_blocking", none)?;
        instance.release(none);
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`setblocking(flag)`**：`flag` ⇒ `_timeout = None`；否则 `0.0`。
fn socket_setblocking_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "setblocking()")?;
    let Some(flag) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!(
                "setblocking() takes exactly one argument ({} given)",
                rest.len()
            ),
        ));
    };
    let blocking = instance.truth_of(flag);
    let value = if blocking {
        instance.new_none()
    } else {
        instance.new_float(0.0)
    };
    let outcome = instance.set_attribute_value(this, "_timeout", value);
    instance.release(value);
    outcome?;
    Ok(instance.retain(instance.singletons().none()))
}

/// **`getblocking()`** ⇒ `_timeout is None`。
fn socket_getblocking_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, _rest) = socket_receiver(instance, bound, args, "getblocking()")?;
    let blocking = match socket_timeout_attribute(instance, this) {
        None => true,
        Some(value) => is_none_value(instance, value),
    };
    Ok(instance.new_bool(blocking))
}

/// **`sendall(data)`**：循环 `send` 直到发完（`Net` 层的 `send` 已处理 `EINTR` ✓）。
fn socket_sendall_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "sendall()")?;
    let Some(data_value) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!("sendall() takes exactly one argument ({} given)", rest.len()),
        ));
    };
    let Some(data) = instance.bytes_value(data_value) else {
        let name = instance.type_name(instance.type_of(data_value));
        return Err(type_error(
            instance,
            format!("a bytes-like object is required, not '{name}'"),
        ));
    };
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    let mut offset = 0usize;
    while offset < data.len() {
        match instance.net_send(fd as u64, &data[offset..]) {
            Ok(0) => break,
            Ok(written) => offset += written,
            Err(error) => return Err(net_error(instance, error)),
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`connect_ex(address)`**：成功 `0`，失败回 **errno 整数**（不是抛异常，照参照 ✓）。
fn socket_connect_ex_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, rest) = socket_receiver(instance, bound, args, "connect_ex()")?;
    let Some(address) = rest.first().copied() else {
        return Err(type_error(
            instance,
            format!(
                "connect_ex() takes exactly one argument ({} given)",
                rest.len()
            ),
        ));
    };
    let (family, wire) = socket_address_wire(instance, this, address, "connect_ex")?;
    let fd = socket_fd(instance, this);
    if fd < 0 {
        return Err(raise_ebadf(instance));
    }
    let code = match instance.net_connect(fd as u64, family, &wire) {
        Ok(()) => 0,
        Err(pyawa_core::CapabilityCallError::Machine(errno)) => i64::from(errno),
        Err(_) => instance.platform_constant("EOPNOTSUPP").unwrap_or(95),
    };
    Ok(instance.new_int(code))
}

/// `_socket` 模块命名空间（模块表里那个模块的挂载 dict ✓）。
fn socket_module_namespace(instance: &Instance) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    pyawa_core::mounted_instance_dict(instance, module)
}

/// **`_socket.getdefaulttimeout()`** ⇒ 记录的默认超时（初始 `None` ＝ 阻塞 ✓）。
fn socket_getdefaulttimeout_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !args.is_empty() {
        return Err(type_error(
            instance,
            format!(
                "_socket.getdefaulttimeout() takes no arguments ({} given)",
                args.len()
            ),
        ));
    }
    let stored = socket_module_namespace(instance)
        .and_then(|namespace| instance.dict_get(namespace, "_default_timeout"));
    Ok(match stored {
        Some(value) => instance.retain(value),
        None => instance.new_none(),
    })
}

/// **`_socket.setdefaulttimeout(value)`**：记值（`None` ＝ 阻塞）。**不改真 I/O 行为** ✗。
fn socket_setdefaulttimeout_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(type_error(
            instance,
            "_socket.setdefaulttimeout() takes exactly one argument (0 given)".to_owned(),
        ));
    };
    if !is_none_value(instance, value) && instance.float_value(value).is_none() {
        let name = instance.type_name(instance.type_of(value));
        return Err(type_error(
            instance,
            format!("'{name}' object cannot be interpreted as an integer"),
        ));
    }
    if let Some(namespace) = socket_module_namespace(instance) {
        let stored = instance.retain(value);
        instance.dict_set(namespace, "_default_timeout", stored);
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`_socket.socketpair(family=AF_UNIX, type=SOCK_STREAM, proto=0)`**（`CP-38`）。
///
/// 返回**两个** `_socket.socket` 实例（参照实测：返回 socket 对象而非裸 int ✓）。
fn socketpair_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    check_arity(instance, "socketpair", Arity::DigitAtMost(3), args, kwargs)?;
    let mut parameters = [constant("AF_UNIX"), constant("SOCK_STREAM"), 0i32];
    for (index, value) in args.iter().take(3).enumerate() {
        parameters[index] = socket_int(instance, *value)? as i32;
    }
    let (first, second) = match instance.net_socketpair(parameters[0], parameters[1], parameters[2])
    {
        Ok(pair) => pair,
        Err(error) => return Err(net_error(instance, error)),
    };
    let Some(class) = socket_class_of(instance) else {
        return Err(instance.raise_builtin_error(
            "RuntimeError",
            "`_socket.socket` 未登记（`_socket` 模块的类面缺失）",
        ));
    };
    let mut created: Vec<NonNull<Header>> = Vec::with_capacity(2);
    for fd in [first, second] {
        let call_args = [
            instance.new_int(i64::from(parameters[0])),
            instance.new_int(i64::from(parameters[1])),
            instance.new_int(i64::from(parameters[2])),
            instance.new_int(fd as i64),
        ];
        let outcome = pyawa_core::call_value(instance, class, &call_args, &[]);
        for argument in call_args {
            instance.release(argument);
        }
        match outcome {
            Ok(object) => created.push(object),
            Err(error) => {
                for object in created {
                    instance.release(object);
                }
                return Err(error);
            }
        }
    }
    Ok(instance.new_tuple(created))
}

/// 取 `_socket.socket` 类对象（**按实例的模块表查** —— 不建进程级静态表，`CP-13`／`OM-1` ✓）。
fn socket_class_of(instance: &Instance) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    let namespace = pyawa_core::mounted_instance_dict(instance, module)?;
    instance.dict_get(namespace, "socket")
}
