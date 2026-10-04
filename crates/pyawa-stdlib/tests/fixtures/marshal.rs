//! 由 `tools/gen_marshal_fixture.py` 探测参照实现导出；**禁止手改**。
//! 参照实现：3.14.4
//!
//! `CM-27`：`marshal` **自有**格式、`loads(dumps(x))` 必须往返、**不追**字节兼容。
//! 所以这里记的是**参照的可观测面**（版本号、错误消息、哪些类型往返得动），
//! **不是**参照的字节流。

/// 参照实现的 `marshal.version`（**它的**格式版本）。
///
/// **`CM-27`**：Pyawa 的 marshal 用自己的版本号 ⇒ 与本值**必须不同**才算守住「自有格式」。
pub const REFERENCE_VERSION: i64 = 5;

/// 循环引用：参照实测的消息（`CM-27` 没要求逐字相同，但我们照它报）。
pub const REFERENCE_CIRCULAR_MESSAGE: Option<&str> = None;

/// `loads(b'')` 的实测消息。
pub const REFERENCE_EMPTY_MESSAGE: Option<&str> = Some("EOFError: EOF read where object expected");

/// `loads(b'not marshal data')` 的实测消息。
pub const REFERENCE_GARBAGE_MESSAGE: Option<&str> = Some("ValueError: bad marshal data (unknown type code)");

/// 被截断的输入的实测消息。
pub const REFERENCE_TRUNCATED_MESSAGE: Option<&str> = Some("EOFError: marshal data too short");

/// 未知类型码的实测消息。
pub const REFERENCE_BAD_TYPE_MESSAGE: Option<&str> = Some("ValueError: bad marshal data (unknown type code)");

/// 参照实测**往返得动**的类型名（我们的义务面至少覆盖这些）。
pub const REFERENCE_ROUND_TRIP: &[&str] = &[
    "none",
    "true",
    "false",
    "int_small",
    "int_negative",
    "int_i64_max",
    "int_big",
    "float",
    "float_nan",
    "str_ascii",
    "str_unicode",
    "bytes",
    "tuple",
    "list",
    "dict",
    "set",
    "nested",
];
