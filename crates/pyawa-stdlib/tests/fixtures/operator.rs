//! 由 `tools/gen_operator_fixture.py` 从本机参照实现导出；**禁止手改**。

/// 参照实现里 `operator` 的非下划线公开名（共 57 个）。
pub static REFERENCE_NAMES: &[&str] = &[
    "abs",
    "add",
    "and_",
    "attrgetter",
    "call",
    "concat",
    "contains",
    "countOf",
    "delitem",
    "eq",
    "floordiv",
    "ge",
    "getitem",
    "gt",
    "iadd",
    "iand",
    "iconcat",
    "ifloordiv",
    "ilshift",
    "imatmul",
    "imod",
    "imul",
    "index",
    "indexOf",
    "inv",
    "invert",
    "ior",
    "ipow",
    "irshift",
    "is_",
    "is_none",
    "is_not",
    "is_not_none",
    "isub",
    "itemgetter",
    "itruediv",
    "ixor",
    "le",
    "length_hint",
    "lshift",
    "lt",
    "matmul",
    "methodcaller",
    "mod",
    "mul",
    "ne",
    "neg",
    "not_",
    "or_",
    "pos",
    "pow",
    "rshift",
    "setitem",
    "sub",
    "truediv",
    "truth",
    "xor",
];

/// 实测结果：`(函数名, 左, 右, 期望)`——`None` 表示只调一个实参。
pub static RESULTS: &[(&str, i64, Option<i64>, bool)] = &[
    ("eq", 1, Some(1), true),
    ("eq", 1, Some(2), false),
    ("ne", 1, Some(1), false),
    ("ne", 1, Some(2), true),
    ("is_", 1, Some(1), true),
    ("lt", 1, Some(2), true),
    ("lt", 2, Some(1), false),
    ("le", 1, Some(1), true),
    ("ge", 1, Some(1), true),
    ("gt", 1, Some(2), false),
];

/// 实测：单实参那几个（`truth`／`not_`）对 `0` 与 `1` 的结果。
pub const TRUTH_ZERO: bool = false;
pub const TRUTH_ONE: bool = true;
pub const NOT_ZERO: bool = true;
pub const NOT_ONE: bool = false;

/// 实测：参数个数不对时的消息。
pub const REFERENCE_EQ_MISSING: &str = "TypeError: eq expected 2 arguments, got 1";
pub const REFERENCE_TRUTH_TOO_MANY: &str = "TypeError: _operator.truth() takes exactly one argument (2 given)";
pub const REFERENCE_LT_NOT_SUPPORTED: &str = "TypeError: '<' not supported between instances of 'int' and 'str'";
pub const REFERENCE_LT_MISSING: &str = "TypeError: lt expected 2 arguments, got 1";
