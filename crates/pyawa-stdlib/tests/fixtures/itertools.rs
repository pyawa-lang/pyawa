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

/// `itertools.repeat(value, times)` 的前若干值（参照实现实测；`times < 0` ⇒ 空）。
pub static REPEAT_SEQUENCES: &[(i64, i64, &[i64])] = &[
    (7, 3, &[7, 7, 7]),
    (7, 0, &[]),
    (7, -1, &[]),
];

/// `itertools.repeat(7)`（无限）的头 3 个值。
pub static REPEAT_INFINITE_FIRST: &[i64] = &[7, 7, 7];

/// `itertools.islice(range(10), ...)` 的结果（`(start, stop, step)`）。
pub static ISLICE_SEQUENCES: &[(i64, i64, i64, &[i64])] = &[
    (0, 5, 1, &[0, 1, 2, 3, 4]),
    (2, 5, 1, &[2, 3, 4]),
    (0, 10, 3, &[0, 3, 6, 9]),
    (5, 2, 1, &[]),
    (0, 10, 1, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
];

/// 短输入：`islice([1, 2], 10)` ⇒ 内层先耗尽。
pub static ISLICE_SHORT_INPUT: &[i64] = &[1, 2];

/// `itertools.islice(count(), 5, 2)` 之后 `next(count())` 的值：`start >= stop` 时**一个都不让出、但仍消费 `start` 个**（参照实测 ⇒ 不是 0 消费）。
pub const ISLICE_CONSUMED_AFTER_EMPTY: i64 = 105;

/// 实测消息：`repeat` 三连。
pub const REFERENCE_REPEAT_MESSAGES: &[&str] = &["TypeError: repeat() missing required argument 'object' (pos 1)", "TypeError: repeat() takes at most 2 arguments (3 given)", "TypeError: 'str' object cannot be interpreted as an integer"];

/// 实测消息：`islice` 三连（参数太少／步长非正／内层不是可迭代）。
pub const REFERENCE_ISLICE_MESSAGES: &[&str] = &["TypeError: islice expected at least 2 arguments, got 1", "ValueError: Step for islice() must be a positive integer or None.", "TypeError: 'int' object is not iterable"];

/// `itertools.chain(*iterables)` 的输入与结果（参照实现实测；元素都是整数序列）。
pub static CHAIN_INPUTS: &[&[&[i64]]] = &[
    &[&[1, 2], &[3]],
    &[],
    &[&[], &[1], &[]],
    &[&[1, 2, 3]],
];

/// [`CHAIN_INPUTS`] 对应的结果。
pub static CHAIN_EXPECTED: &[&[i64]] = &[
    &[1, 2, 3],
    &[],
    &[1],
    &[1, 2, 3],
];

/// 惰性：`chain(count(5), [9])` 的头 3 个（内层无限也不该卡住）。
pub static CHAIN_LAZY_FIRST: &[i64] = &[5, 6, 7];

/// 实测消息：`chain` 的元素不是可迭代对象（**取值时**才报）。
pub const REFERENCE_CHAIN_NOT_ITERABLE: &str = "TypeError: 'int' object is not iterable";

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
