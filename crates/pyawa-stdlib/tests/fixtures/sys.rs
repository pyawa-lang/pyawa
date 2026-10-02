//! 由 `tools/gen_sys_fixture.py` 探测参照实现导出；**禁止手改**。
//! 参照实现：3.14.4（`sys.version_info` ＝ (3, 14, 4, 'final', 0)）
//!
//! 生成 Rust 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器
//! （与 `fixtures/builtins.rs`、`fixtures/unicode.rs` 同一取舍）。

/// 参照实现的 `sys.version_info`（**语言级别**：Pyawa 必须报同一组数，供特性检测）。
pub const REFERENCE_VERSION_INFO: (u32, u32, u32, &str, u32) = (3, 14, 4, "final", 0);

/// 参照实现的 `sys.hexversion`（＝ 上一条的整数编码）。
pub const REFERENCE_HEXVERSION: i64 = 51250416;

/// 参照实现的 `sys.maxunicode`。
pub const REFERENCE_MAXUNICODE: i64 = 1114111;

/// 参照实现的 `sys.maxsize`。
pub const REFERENCE_MAXSIZE: i64 = 9223372036854775807;

/// 参照实现的 `sys.byteorder`（宿主端序）。
pub const REFERENCE_BYTEORDER: &str = "little";

/// 参照实现的 `sys.implementation.name`——Pyawa **必须**报 `pyawa`，它与本值**必须不同**（`CX-13`）。
pub const REFERENCE_IMPLEMENTATION_NAME: &str = "cpython";

/// 参照实现的 `sys.implementation.cache_tag`——Pyawa 用自己的值，与此**必须不同**。
pub const REFERENCE_CACHE_TAG: &str = "cpython-314";

/// 参照实现的 `sys.version` 串——Pyawa 的构建串**必须含 `pyawa`**，与此**必须不同**。
pub const REFERENCE_VERSION: &str = "3.14.4 (main, Aug 20 2026, 10:41:58) [GCC 15.2.0]";

/// 参照实现的 `sys.get_int_max_str_digits()` 默认值（`TS-45` ①）。
pub const REFERENCE_INT_MAX_STR_DIGITS: i64 = 4300;

/// 参照实现的 `sys.int_info.str_digits_check_threshold`（`set_` 允许的最小非零值）。
pub const REFERENCE_STR_DIGITS_THRESHOLD: i64 = 640;

/// `sys.set_int_max_str_digits(threshold - 1)` 的实测消息。
pub const REFERENCE_SET_BELOW_MESSAGE: Option<&str> = Some("ValueError: maxdigits must be >= 640 or 0 for unlimited");

/// `sys.set_int_max_str_digits(-1)` 的实测消息（与上面同一句）。
pub const REFERENCE_SET_NEGATIVE_MESSAGE: Option<&str> = Some("ValueError: maxdigits must be >= 640 or 0 for unlimited");

/// `sys.set_int_max_str_digits('x')` 的实测消息。
pub const REFERENCE_SET_NOT_INTEGER_MESSAGE: Option<&str> = Some("TypeError: 'str' object cannot be interpreted as an integer");

/// `sys.set_int_max_str_digits(2**40)` 的实测消息（超出 C `int`）。
pub const REFERENCE_SET_HUGE_MESSAGE: Option<&str> = Some("OverflowError: Python int too large to convert to C int");

/// `sys.set_int_max_str_digits()` 少给实参的实测消息。
pub const REFERENCE_SET_NO_ARGS_MESSAGE: Option<&str> = Some("TypeError: set_int_max_str_digits() missing required argument 'maxdigits' (pos 1)");

/// `sys.set_int_max_str_digits(1000, 2000)` 多给实参的实测消息。
pub const REFERENCE_SET_TWO_ARGS_MESSAGE: Option<&str> = Some("TypeError: set_int_max_str_digits() takes at most 1 argument (2 given)");

/// `sys.get_int_max_str_digits(1)` 给了实参的实测消息。
pub const REFERENCE_GET_WITH_ARGS_MESSAGE: Option<&str> = Some("TypeError: sys.get_int_max_str_digits() takes no arguments (1 given)");

/// `sys.set_int_max_str_digits(0)` 是否成功（`0` ＝ 不限）。
pub const REFERENCE_SETTING_ZERO_SUCCEEDS: bool = true;

/// `set_(0)` 之后 `get_()` 是否报 `0`。
pub const REFERENCE_ZERO_MEANS_UNLIMITED: bool = true;
