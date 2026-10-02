//! 由 `tools/gen_imp_fixture.py` 探测参照实现导出；**禁止手改**。
//!
//! 生成 Rust 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器（与其它夹具同一取舍）。

/// 参照实现的 `pyc_magic_number_token`——**Pyawa 必须用自定值**，与此不同。
pub const REFERENCE_PYC_MAGIC_NUMBER_TOKEN: i64 = 168627755;

/// 参照实现 `pyc_magic_number_token` 的**低 16 位**（`_bootstrap_external` 算 `MAGIC_NUMBER` 用它）。
pub const REFERENCE_MAGIC_LOW_16: i64 = 3627;

/// 参照实现里 `is_builtin('sys')` 的三态值（`-1` ＝ 内建）。
pub const REFERENCE_IS_BUILTIN_SYS: i64 = -1;

/// 参照实现里 `is_builtin('nope')` 的值（`0` ＝ 不是内建）。
pub const REFERENCE_IS_BUILTIN_UNKNOWN: i64 = 0;

/// 参照实现的 `_imp.__doc__`（与参照同源的一句话）。
pub const REFERENCE_DOC: &str = "(Extremely) low-level import machinery bits as used by importlib.";

/// 参照实现的公开名字清单（本层只落地其中一小部分，逐条见 §5.2.4）。
pub static REFERENCE_NAMES: &[&str] = &[
    "_fix_co_filename",
    "_frozen_module_names",
    "_override_frozen_modules_for_tests",
    "_override_multi_interp_extensions_check",
    "acquire_lock",
    "check_hash_based_pycs",
    "create_builtin",
    "create_dynamic",
    "exec_builtin",
    "exec_dynamic",
    "extension_suffixes",
    "find_frozen",
    "get_frozen_object",
    "init_frozen",
    "is_builtin",
    "is_frozen",
    "is_frozen_package",
    "lock_held",
    "pyc_magic_number_token",
    "release_lock",
    "source_hash",
];
