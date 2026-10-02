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
