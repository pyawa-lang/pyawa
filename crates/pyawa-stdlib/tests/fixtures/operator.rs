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
pub static RESULTS: &[(&str, i64, Option<i64>, i64)] = &[
    ("eq", 1, Some(1), 1),
    ("eq", 1, Some(2), 0),
    ("ne", 1, Some(1), 0),
    ("ne", 1, Some(2), 1),
    ("is_", 1, Some(1), 1),
    ("lt", 1, Some(2), 1),
    ("lt", 2, Some(1), 0),
    ("le", 1, Some(1), 1),
    ("ge", 1, Some(1), 1),
    ("gt", 1, Some(2), 0),
    ("add", 1, Some(2), 3),
    ("sub", 3, Some(1), 2),
    ("mul", 2, Some(3), 6),
    ("floordiv", 7, Some(2), 3),
    ("mod", 7, Some(2), 1),
    ("pow", 2, Some(10), 1024),
    ("and_", 6, Some(3), 2),
    ("or_", 6, Some(3), 7),
    ("xor", 6, Some(3), 5),
    ("lshift", 1, Some(4), 16),
    ("rshift", 16, Some(2), 4),
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
pub const REFERENCE_ADD_NOT_SUPPORTED: &str = "TypeError: unsupported operand type(s) for +: 'int' and 'str'";
pub const REFERENCE_ADD_MISSING: &str = "TypeError: add expected 2 arguments, got 1";
pub const REFERENCE_FLOORDIV_ZERO: &str = "ZeroDivisionError: division by zero";
pub const REFERENCE_NEG_NOT_SUPPORTED: &str = "TypeError: bad operand type for unary -: 'str'";
pub const REFERENCE_LSHIFT_NEGATIVE: &str = "ValueError: negative shift count";
