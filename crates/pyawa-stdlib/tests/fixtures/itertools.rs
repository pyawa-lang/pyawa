//! 由 `tools/gen_itertools_fixture.py` 探测参照实现导出；**禁止手改**。

/// `itertools.count(start, step)` 的前 5 个值（参照实现实测）。
pub static COUNT_SEQUENCES: &[(i64, i64, [i64; 5])] = &[
    (0, 1, [0, 1, 2, 3, 4]),
    (1, 2, [1, 3, 5, 7, 9]),
    (5, 3, [5, 8, 11, 14, 17]),
    (0, -1, [0, -1, -2, -3, -4]),
    (-7, 4, [-7, -3, 1, 5, 9]),
];

/// 参照实现在**浮点**起始值下的行为（本层 `count` 只收整数 ⇒ 这是边界，不是期望）。
pub static REFERENCE_FLOAT_SEQUENCE: &[f64] = &[0.5, 1.0, 1.5];

/// 实测消息：位置实参给多了。
pub const REFERENCE_TOO_MANY: &str = "TypeError: count() takes at most 2 arguments (3 given)";

/// 实测消息：未知关键字。
pub const REFERENCE_UNKNOWN_KEYWORD: &str = "TypeError: count() got an unexpected keyword argument 'x'";

/// 实测消息：不是数值。
pub const REFERENCE_NOT_A_NUMBER: &str = "TypeError: a number is required";

/// 参照实现导出的公开名（本层只落地 `count`，逐条见 §5.2.6）。
pub static REFERENCE_NAMES: &[&str] = &[
    "accumulate",
    "batched",
    "chain",
    "combinations",
    "combinations_with_replacement",
    "compress",
    "count",
    "cycle",
    "dropwhile",
    "filterfalse",
    "groupby",
    "islice",
    "pairwise",
    "permutations",
    "product",
    "repeat",
    "starmap",
    "takewhile",
    "tee",
    "zip_longest",
];
