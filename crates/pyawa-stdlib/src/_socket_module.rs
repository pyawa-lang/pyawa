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
//! ## 如实报错面（硬边界 ✓：**禁止假装连接成功**）
//!
//! 网络／描述符 I/O 属**能力域**（`CM-8`：外部世界必须经能力层）——本轮**不做**任何真 syscall，
//! 也不返回假 fd。所以：
//!
//! - **全部 28 个模块函数**：参数对了也**调用即** `NotImplementedError`（纯数据的那几个
//!   —— `htons` 一族／`inet_*`／`CMSG_*` —— 同样如实报"未接线"，**不返回假值**）；
//! - **`_socket.socket` 的一切方法**（`bind`／`connect`／`send`…）与**构造**：同样 `NotImplementedError`
//!   （构造在参照里就要发 `socket(2)`）。
//!
//! ## 未落地（如实记）
//!
//! - `CAPI`：参照是 `PyCapsule`，VM 没有这个载荷 ⇒ **不登记**（不是"给个假的" ✗）。
//! - `socket` 实例属性 `family`／`type`／`proto`／`timeout`：本轮只登记**方法名**；
//!   属性存取会 `AttributeError`（`Lib/socket.py` 只在运行期 `__repr__` 里用它们 ✓）。
//! - 真 I/O 接线（`socketpair`／`create_connection` 一族）＝ `P3-29` 后续轮，走能力域槽位。
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
    ("socketpair", Arity::DigitAtMost(3), socketpair_stub, Unwired::Capability),
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
fn build_socket_class(instance: &Instance) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let base = instance
        .type_named("object")
        .expect("object 在引导期已登记")
        .cast::<Header>();
    let class_namespace = instance.new_dict();
    let init = make_native(instance, "__init__", socket_init_stub);
    instance.dict_set(class_namespace, "__init__", init);
    for name in SOCKET_METHODS {
        let function = make_native(instance, name, socket_method_stub);
        instance.dict_set(class_namespace, name, function);
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
socket_stub_fn!(socketpair_stub, "socketpair", Arity::DigitAtMost(3), Unwired::Capability);

/// `_socket.socket.__init__`：构造在参照里就要发 `socket(2)` ⇒ 本轮如实报未接线。
fn socket_init_stub(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_socket.socket()`：构造需要 socket(2) 系统调用（能力域 CM-8），本轮未接线",
    ))
}

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
