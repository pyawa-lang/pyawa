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

/// 谓词类：以 `x < 3` 为谓词、输入 `[1, 2, 3, 4, 1]` 的结果（参照实测）。
pub static TAKEWHILE_RESULT: &[i64] = &[1, 2];
pub static DROPWHILE_RESULT: &[i64] = &[3, 4, 1];
pub static FILTERFALSE_RESULT: &[i64] = &[3, 4];

/// 谓词类共用的实测消息（参数个数不对）。
pub const REFERENCE_FILTER_LIKE_ARG_COUNT: &str = "TypeError: takewhile expected 2 arguments, got 1";

/// 实测消息：谓词不可调用（**第一次取值**时才报）。
pub const REFERENCE_FILTER_LIKE_NOT_CALLABLE: &str = "TypeError: 'int' object is not callable";

/// 实测消息：内层不是可迭代对象。
pub const REFERENCE_FILTER_LIKE_NOT_ITERABLE: &str = "TypeError: 'int' object is not iterable";

/// `itertools.accumulate` 的结果（参照实测；`func` 缺省是加法）。
pub static ACCUMULATE_SUM: &[i64] = &[1, 3, 6];
pub static ACCUMULATE_MUL: &[i64] = &[1, 2, 6];
pub static ACCUMULATE_SINGLE: &[i64] = &[5];

/// 实测：`accumulate` 的缺参消息，以及「非可调用 func ＋ 单元素」**不报错**这一条。
pub const REFERENCE_ACCUMULATE_MISSING: &str = "TypeError: accumulate() missing required argument 'iterable' (pos 1)";
pub static REFERENCE_ACCUMULATE_NONCALLABLE_SINGLE: &[i64] = &[1];

/// `itertools.starmap` 的结果 ＋ 两条实测消息。
pub static STARMAP_POW: &[i64] = &[8, 32];
pub const REFERENCE_STARMAP_ARG_COUNT: &str = "TypeError: starmap expected 2 arguments, got 1";
pub const REFERENCE_STARMAP_NOT_ITERABLE: &str = "TypeError: 'int' object is not iterable";

/// `itertools.cycle`：头 5 个（`[1, 2]` 循环）与空输入。
pub static CYCLE_FIRST_FIVE: &[i64] = &[1, 2, 1, 2, 1];
pub static CYCLE_EMPTY: &[i64] = &[];

/// 实测：`cycle` 的三条用法错误消息。
pub const REFERENCE_CYCLE_ARG_COUNT: &str = "TypeError: cycle expected 1 argument, got 0";
pub const REFERENCE_CYCLE_KEYWORDS: &str = "TypeError: cycle() takes no keyword arguments";
pub const REFERENCE_CYCLE_NOT_ITERABLE: &str = "TypeError: 'int' object is not iterable";

/// `itertools.pairwise` 与 `batched` 的结果（参照实测；整数序列）。
pub static PAIRWISE_RESULT: &[(i64, i64)] = &[(1, 2), (2, 3), (3, 4)];
pub static PAIRWISE_SHORT: &[(i64, i64)] = &[];

/// `batched([1..5], 2)` 与 `batched([1..6], 3)` 的每批长度与内容（记成扁平＋分组）。
pub static BATCHED_TWO: &[&[i64]] = &[&[1, 2], &[3, 4], &[5]];

pub static BATCHED_THREE: &[&[i64]] = &[&[1, 2, 3], &[4, 5, 6]];

/// 实测：`pairwise`／`batched` 的用法错误消息（五条）。
pub const REFERENCE_PAIRWISE_ARG_COUNT: &str = "TypeError: pairwise expected 1 argument, got 0";
pub const REFERENCE_BATCHED_MISSING_N: &str = "TypeError: batched() missing required argument 'n' (pos 2)";
pub const REFERENCE_BATCHED_ZERO: &str = "ValueError: n must be at least one";
pub const REFERENCE_BATCHED_NOT_INT: &str = "TypeError: 'str' object cannot be interpreted as an integer";
pub const REFERENCE_BATCHED_TOO_MANY: &str = "TypeError: batched() takes exactly 2 positional arguments (3 given)";

/// `itertools.zip_longest` 的结果（参照实测）。
pub static ZIP_LONGEST_TWO: &[&[Option<i64>]] = &[&[Some(1), Some(4)], &[Some(2), Some(5)], &[Some(3), None]];
pub static ZIP_LONGEST_FILL: &[&[Option<i64>]] = &[&[Some(1), Some(3)], &[Some(2), Some(0)]];
pub static ZIP_LONGEST_EMPTY: &[&[Option<i64>]] = &[&[None, Some(1)]];

pub const REFERENCE_ZIP_LONGEST_UNKNOWN_KEYWORD: &str = "TypeError: zip_longest() got an unexpected keyword argument";

/// `itertools.compress` 的结果（参照实测）。
pub static COMPRESS_RESULT: &[i64] = &[1, 3, 5];
pub static COMPRESS_SHORT: &[i64] = &[1];

/// `itertools.combinations` 的结果（参照实测；`r` 由下标给出）。
pub static COMBINATIONS_TWO: &[&[i64]] = &[&[1, 2], &[1, 3], &[1, 4], &[2, 3], &[2, 4], &[3, 4]];
pub static COMBINATIONS_ZERO: &[&[i64]] = &[&[]];

/// 实测：`compress`／`combinations` 四条消息。
pub const REFERENCE_COMPRESS_MISSING: &str = "TypeError: compress() missing required argument 'selectors' (pos 2)";
pub const REFERENCE_COMBINATIONS_MISSING_R: &str = "TypeError: combinations() missing required argument 'r' (pos 2)";
pub const REFERENCE_COMBINATIONS_NOT_INT: &str = "TypeError: 'str' object cannot be interpreted as an integer";
pub const REFERENCE_COMBINATIONS_NEGATIVE: &str = "ValueError: r must be non-negative";

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
