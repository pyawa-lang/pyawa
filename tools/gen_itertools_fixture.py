#!/usr/bin/env python3
"""从参照实现**探测** `itertools` 的可观测面，生成两件：

- `crates/pyawa-stdlib/src/itertools_doc.txt`：`itertools.__doc__` 原文（模块用 `include_str!`）
- `crates/pyawa-stdlib/tests/fixtures/itertools.rs`：对拍夹具（`count` 的序列与三条实测消息）

契约见 `docs/SPEC-c-modules.md` §5.2.6。**注意**：夹具里的浮点序列只用来**记录参照的行为**，
本层 `count` 只收整数 ⇒ 那条是"未接线"的边界（不许拿它当期望去凑）。

用法::

    python3 tools/gen_itertools_fixture.py
"""

from __future__ import annotations

import itertools
import operator
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
DOC = ROOT / "crates/pyawa-stdlib/src/itertools_doc.txt"
FIXTURE = ROOT / "crates/pyawa-stdlib/tests/fixtures/itertools.rs"


def error_message(call) -> str:
    try:
        call()
    except Exception as exc:  # noqa: BLE001 - 探测用
        return f"{type(exc).__name__}: {exc}"
    raise SystemExit("这条调用本该报错")


def main() -> None:
    DOC.write_text(itertools.__doc__)
    cases = [(0, 1), (1, 2), (5, 3), (0, -1), (-7, 4)]
    lines = [
        "//! 由 `tools/gen_itertools_fixture.py` 探测参照实现导出；**禁止手改**。",
        "",
        "/// `itertools.count(start, step)` 的前 5 个值（参照实现实测）。",
        "pub static COUNT_SEQUENCES: &[(i64, i64, [i64; 5])] = &[",
    ]
    for start, step in cases:
        values = list(itertools.islice(itertools.count(start, step), 5))
        rendered = ", ".join(str(value) for value in values)
        lines.append(f"    ({start}, {step}, [{rendered}]),")
    lines.append("];")
    lines.append("")
    lines.append("/// 参照实现在**浮点**起始值下的行为（本层 `count` 只收整数 ⇒ 这是边界，不是期望）。")
    float_values = list(itertools.islice(itertools.count(0.5, 0.5), 3))
    lines.append(
        "pub static REFERENCE_FLOAT_SEQUENCE: &[f64] = &["
        + ", ".join(repr(value) for value in float_values)
        + "];"
    )
    lines.append("")
    lines.append("/// 实测消息：位置实参给多了。")
    lines.append(
        'pub const REFERENCE_TOO_MANY: &str = "'
        + error_message(lambda: itertools.count(1, 2, 3))
        + '";'
    )
    lines.append("")
    lines.append("/// 实测消息：未知关键字。")
    lines.append(
        'pub const REFERENCE_UNKNOWN_KEYWORD: &str = "'
        + error_message(lambda: itertools.count(x=1))
        + '";'
    )
    lines.append("")
    lines.append("/// 实测消息：不是数值。")
    lines.append(
        'pub const REFERENCE_NOT_A_NUMBER: &str = "'
        + error_message(lambda: itertools.count("a"))
        + '";'
    )
    lines.append("")
    lines.append("/// `itertools.repeat(value, times)` 的前若干值（参照实现实测；`times < 0` ⇒ 空）。")
    repeat_cases = [(7, 3), (7, 0), (7, -1)]
    lines.append("pub static REPEAT_SEQUENCES: &[(i64, i64, &[i64])] = &[")
    for value, times in repeat_cases:
        items = list(itertools.repeat(value, times))
        rendered = ", ".join(str(item) for item in items)
        lines.append(f"    ({value}, {times}, &[{rendered}]),")
    lines.append("];")
    lines.append("")
    lines.append("/// `itertools.repeat(7)`（无限）的头 3 个值。")
    infinite = list(itertools.islice(itertools.repeat(7), 3))
    lines.append(
        "pub static REPEAT_INFINITE_FIRST: &[i64] = &["
        + ", ".join(str(item) for item in infinite)
        + "];"
    )
    lines.append("")
    lines.append("/// `itertools.islice(range(10), ...)` 的结果（`(start, stop, step)`）。")
    islice_cases = [(0, 5, 1), (2, 5, 1), (0, 10, 3), (5, 2, 1), (0, 10, 1)]
    lines.append("pub static ISLICE_SEQUENCES: &[(i64, i64, i64, &[i64])] = &[")
    for start, stop, step in islice_cases:
        items = list(itertools.islice(range(10), start, stop, step))
        rendered = ", ".join(str(item) for item in items)
        lines.append(f"    ({start}, {stop}, {step}, &[{rendered}]),")
    lines.append("];")
    lines.append("")
    lines.append("/// 短输入：`islice([1, 2], 10)` ⇒ 内层先耗尽。")
    short = list(itertools.islice([1, 2], 10))
    lines.append(
        "pub static ISLICE_SHORT_INPUT: &[i64] = &["
        + ", ".join(str(item) for item in short)
        + "];"
    )
    lines.append("")
    lines.append(
        "/// `itertools.islice(count(), 5, 2)` 之后 `next(count())` 的值：`start >= stop` 时"
        "**一个都不让出、但仍消费 `start` 个**（参照实测 ⇒ 不是 0 消费）。"
    )
    probe = itertools.count(100)
    list(itertools.islice(probe, 5, 2))
    lines.append(f"pub const ISLICE_CONSUMED_AFTER_EMPTY: i64 = {next(probe)};")
    lines.append("")
    lines.append("/// 实测消息：`repeat` 三连。")
    lines.append(
        'pub const REFERENCE_REPEAT_MESSAGES: &[&str] = &['
        + ", ".join(
            f'"{message}"'
            for message in [
                error_message(lambda: itertools.repeat()),
                error_message(lambda: itertools.repeat(1, 2, 3)),
                error_message(lambda: itertools.repeat(1, "a")),
            ]
        )
        + "];"
    )
    lines.append("")
    lines.append("/// 实测消息：`islice` 三连（参数太少／步长非正／内层不是可迭代）。")
    lines.append(
        'pub const REFERENCE_ISLICE_MESSAGES: &[&str] = &['
        + ", ".join(
            f'"{message}"'
            for message in [
                error_message(lambda: itertools.islice([1])),
                error_message(lambda: itertools.islice([1, 2, 3], 0, 3, 0)),
                error_message(lambda: itertools.islice(5, 1)),
            ]
        )
        + "];"
    )
    lines.append("")
    lines.append("/// `itertools.chain(*iterables)` 的输入与结果（参照实现实测；元素都是整数序列）。")
    chain_cases = [([[1, 2], [3]], [1, 2, 3]), ([], []), ([[], [1], []], [1]), ([[1, 2, 3]], [1, 2, 3])]
    lines.append("pub static CHAIN_INPUTS: &[&[&[i64]]] = &[")
    for inputs, _ in chain_cases:
        rendered = ", ".join("&[" + ", ".join(str(v) for v in values) + "]" for values in inputs)
        lines.append(f"    &[{rendered}],")
    lines.append("];")
    lines.append("")
    lines.append("/// [`CHAIN_INPUTS`] 对应的结果。")
    lines.append("pub static CHAIN_EXPECTED: &[&[i64]] = &[")
    for _, expected in chain_cases:
        lines.append("    &[" + ", ".join(str(v) for v in expected) + "],")
    lines.append("];")
    lines.append("")
    lines.append("/// 惰性：`chain(count(5), [9])` 的头 3 个（内层无限也不该卡住）。")
    lazy = list(itertools.islice(itertools.chain(itertools.count(5), [9]), 3))
    lines.append(
        "pub static CHAIN_LAZY_FIRST: &[i64] = &[" + ", ".join(str(v) for v in lazy) + "];"
    )
    lines.append("")
    lines.append("/// 实测消息：`chain` 的元素不是可迭代对象（**取值时**才报）。")
    lines.append(
        'pub const REFERENCE_CHAIN_NOT_ITERABLE: &str = "'
        + error_message(lambda: list(itertools.chain([1], 7)))
        + '";'
    )
    lines.append("")
    lines.append("/// 谓词类：以 `x < 3` 为谓词、输入 `[1, 2, 3, 4, 1]` 的结果（参照实测）。")
    predicate_input = [1, 2, 3, 4, 1]
    less3 = lambda x: x < 3
    for name, function in [
        ("TAKEWHILE", itertools.takewhile),
        ("DROPWHILE", itertools.dropwhile),
        ("FILTERFALSE", itertools.filterfalse),
    ]:
        values = list(function(less3, predicate_input))
        lines.append(
            f"pub static {name}_RESULT: &[i64] = &["
            + ", ".join(str(value) for value in values)
            + "];"
        )
    lines.append("")
    lines.append("/// 谓词类共用的实测消息（参数个数不对）。")
    lines.append(
        'pub const REFERENCE_FILTER_LIKE_ARG_COUNT: &str = "'
        + error_message(lambda: itertools.takewhile(less3))
        + '";'
    )
    lines.append("")
    lines.append("/// 实测消息：谓词不可调用（**第一次取值**时才报）。")
    lines.append(
        'pub const REFERENCE_FILTER_LIKE_NOT_CALLABLE: &str = "'
        + error_message(lambda: list(itertools.takewhile(5, [1])))
        + '";'
    )
    lines.append("")
    lines.append("/// 实测消息：内层不是可迭代对象。")
    lines.append(
        'pub const REFERENCE_FILTER_LIKE_NOT_ITERABLE: &str = "'
        + error_message(lambda: list(itertools.dropwhile(less3, 5)))
        + '";'
    )
    lines.append("")
    lines.append("/// `itertools.accumulate` 的结果（参照实测；`func` 缺省是加法）。")
    lines.append(
        "pub static ACCUMULATE_SUM: &[i64] = &["
        + ", ".join(str(v) for v in itertools.accumulate([1, 2, 3]))
        + "];"
    )
    lines.append(
        "pub static ACCUMULATE_MUL: &[i64] = &["
        + ", ".join(str(v) for v in itertools.accumulate([1, 2, 3], operator.mul))
        + "];"
    )
    lines.append(
        "pub static ACCUMULATE_SINGLE: &[i64] = &["
        + ", ".join(str(v) for v in itertools.accumulate([5]))
        + "];"
    )
    lines.append("")
    lines.append(
        "/// 实测：`accumulate` 的缺参消息，以及「非可调用 func ＋ 单元素」**不报错**这一条。"
    )
    lines.append(
        'pub const REFERENCE_ACCUMULATE_MISSING: &str = "'
        + error_message(lambda: itertools.accumulate())
        + '";'
    )
    lines.append(
        "pub static REFERENCE_ACCUMULATE_NONCALLABLE_SINGLE: &[i64] = &["
        + ", ".join(str(v) for v in itertools.accumulate([1], 5))
        + "];"
    )
    lines.append("")
    lines.append("/// `itertools.starmap` 的结果 ＋ 两条实测消息。")
    lines.append(
        "pub static STARMAP_POW: &[i64] = &["
        + ", ".join(str(v) for v in itertools.starmap(pow, [(2, 3), (2, 5)]))
        + "];"
    )
    lines.append(
        'pub const REFERENCE_STARMAP_ARG_COUNT: &str = "'
        + error_message(lambda: itertools.starmap(pow))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_STARMAP_NOT_ITERABLE: &str = "'
        + error_message(lambda: list(itertools.starmap(pow, [1])))
        + '";'
    )
    lines.append("")
    lines.append("/// `itertools.cycle`：头 5 个（`[1, 2]` 循环）与空输入。")
    lines.append(
        "pub static CYCLE_FIRST_FIVE: &[i64] = &["
        + ", ".join(str(v) for v in itertools.islice(itertools.cycle([1, 2]), 5))
        + "];"
    )
    lines.append(
        "pub static CYCLE_EMPTY: &[i64] = &["
        + ", ".join(str(v) for v in itertools.cycle([]))
        + "];"
    )
    lines.append("")
    lines.append("/// 实测：`cycle` 的三条用法错误消息。")
    lines.append(
        'pub const REFERENCE_CYCLE_ARG_COUNT: &str = "'
        + error_message(lambda: itertools.cycle())
        + '";'
    )
    lines.append(
        'pub const REFERENCE_CYCLE_KEYWORDS: &str = "'
        + error_message(lambda: itertools.cycle(x=1))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_CYCLE_NOT_ITERABLE: &str = "'
        + error_message(lambda: itertools.cycle(1))
        + '";'
    )
    lines.append("")
    lines.append("/// `itertools.pairwise` 与 `batched` 的结果（参照实测；整数序列）。")
    lines.append(
        "pub static PAIRWISE_RESULT: &[(i64, i64)] = &["
        + ", ".join(f"({a}, {b})" for a, b in itertools.pairwise([1, 2, 3, 4]))
        + "];"
    )
    lines.append(
        "pub static PAIRWISE_SHORT: &[(i64, i64)] = &["
        + ", ".join(f"({a}, {b})" for a, b in itertools.pairwise([1]))
        + "];"
    )
    lines.append("")
    lines.append("/// `batched([1..5], 2)` 与 `batched([1..6], 3)` 的每批长度与内容（记成扁平＋分组）。")
    for name, source, size in [
        ("BATCHED_TWO", [1, 2, 3, 4, 5], 2),
        ("BATCHED_THREE", [1, 2, 3, 4, 5, 6], 3),
    ]:
        groups = [list(group) for group in itertools.batched(source, size)]
        inner = ", ".join(
            "&[" + ", ".join(str(v) for v in group) + "]" for group in groups
        )
        lines.append(f"pub static {name}: &[&[i64]] = &[{inner}];")
        lines.append("")
    lines.append("/// 实测：`pairwise`／`batched` 的用法错误消息（五条）。")
    lines.append(
        'pub const REFERENCE_PAIRWISE_ARG_COUNT: &str = "'
        + error_message(lambda: itertools.pairwise())
        + '";'
    )
    lines.append(
        'pub const REFERENCE_BATCHED_MISSING_N: &str = "'
        + error_message(lambda: itertools.batched([1]))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_BATCHED_ZERO: &str = "'
        + error_message(lambda: itertools.batched([1], 0))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_BATCHED_NOT_INT: &str = "'
        + error_message(lambda: itertools.batched([1], "a"))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_BATCHED_TOO_MANY: &str = "'
        + error_message(lambda: itertools.batched([1], 2, 3))
        + '";'
    )
    lines.append("")
    lines.append("/// `itertools.zip_longest` 的结果（参照实测）。")
    cases = [
        ("ZIP_LONGEST_TWO", ([1, 2, 3], [4, 5]), None),
        ("ZIP_LONGEST_FILL", ([1, 2], [3]), 0),
        ("ZIP_LONGEST_EMPTY", ([], [1]), None),
    ]
    for name, inputs, fill in cases:
        rows = [list(row) for row in itertools.zip_longest(*inputs, fillvalue=fill)]
        inner = ", ".join(
            "&["
            + ", ".join("None" if item is None else f"Some({item})" for item in row)
            + "]"
            for row in rows
        )
        lines.append(f"pub static {name}: &[&[Option<i64>]] = &[{inner}];")
    lines.append("")
    lines.append(
        'pub const REFERENCE_ZIP_LONGEST_UNKNOWN_KEYWORD: &str = "'
        + error_message(lambda: itertools.zip_longest([1], nope=1))
        + '";'
    )
    lines.append("")
    lines.append("/// `itertools.compress` 的结果（参照实测）。")
    lines.append(
        "pub static COMPRESS_RESULT: &[i64] = &["
        + ", ".join(str(v) for v in itertools.compress([1, 2, 3, 4, 5], [1, 0, 1, 0, 1]))
        + "];"
    )
    lines.append(
        "pub static COMPRESS_SHORT: &[i64] = &["
        + ", ".join(str(v) for v in itertools.compress([1, 2, 3], [1]))
        + "];"
    )
    lines.append("")
    lines.append("/// `itertools.combinations` 的结果（参照实测；`r` 由下标给出）。")
    for name, pool, r in [("COMBINATIONS_TWO", [1, 2, 3, 4], 2), ("COMBINATIONS_ZERO", [1, 2, 3], 0)]:
        groups = [list(group) for group in itertools.combinations(pool, r)]
        inner = ", ".join("&[" + ", ".join(str(v) for v in g) + "]" for g in groups)
        lines.append(f"pub static {name}: &[&[i64]] = &[{inner}];")
    lines.append("")
    lines.append("/// 实测：`compress`／`combinations` 四条消息。")
    lines.append(
        'pub const REFERENCE_COMPRESS_MISSING: &str = "'
        + error_message(lambda: itertools.compress([1]))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_COMBINATIONS_MISSING_R: &str = "'
        + error_message(lambda: itertools.combinations([1, 2]))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_COMBINATIONS_NOT_INT: &str = "'
        + error_message(lambda: itertools.combinations([1, 2], "a"))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_COMBINATIONS_NEGATIVE: &str = "'
        + error_message(lambda: itertools.combinations([1, 2], -1))
        + '";'
    )
    lines.append("")
    lines.append("/// `itertools.permutations` 的结果（参照实测）。")
    for name, pool, r in [
        ("PERMUTATIONS_THREE", [1, 2, 3], None),
        ("PERMUTATIONS_TWO", [1, 2, 3], 2),
        ("PERMUTATIONS_ZERO", [1, 2], 0),
    ]:
        groups = [list(g) for g in (itertools.permutations(pool) if r is None else itertools.permutations(pool, r))]
        inner = ", ".join("&[" + ", ".join(str(v) for v in g) + "]" for g in groups)
        lines.append(f"pub static {name}: &[&[i64]] = &[{inner}];")
    lines.append("")
    lines.append("/// 实测：`permutations` 三条消息（注意非整数 `r` 与 `combinations` **不同**）。")
    lines.append(
        'pub const REFERENCE_PERMUTATIONS_MISSING: &str = "'
        + error_message(lambda: itertools.permutations())
        + '";'
    )
    lines.append(
        'pub const REFERENCE_PERMUTATIONS_NOT_INT: &str = "'
        + error_message(lambda: itertools.permutations([1, 2], "a"))
        + '";'
    )
    lines.append(
        'pub const REFERENCE_PERMUTATIONS_NEGATIVE: &str = "'
        + error_message(lambda: itertools.permutations([1, 2], -1))
        + '";'
    )
    lines.append("")
    lines.append("/// `itertools.combinations_with_replacement` 的结果（参照实测）。")
    for name, pool, r in [
        ("CWR_TWO", [1, 2, 3], 2),
        ("CWR_OVER", [1, 2], 3),
    ]:
        groups = [list(g) for g in itertools.combinations_with_replacement(pool, r)]
        inner = ", ".join("&[" + ", ".join(str(v) for v in g) + "]" for g in groups)
        lines.append(f"pub static {name}: &[&[i64]] = &[{inner}];")
    lines.append("")
    lines.append("/// 实测：`combinations_with_replacement` 两条消息（名字长，逐字比对）。")
    lines.append(
        'pub const REFERENCE_CWR_MISSING_ITERABLE: &str = "'
        + error_message(lambda: itertools.combinations_with_replacement())
        + '";'
    )
    lines.append(
        'pub const REFERENCE_CWR_MISSING_R: &str = "'
        + error_message(lambda: itertools.combinations_with_replacement([1, 2]))
        + '";'
    )
    lines.append("")
    lines.append("/// 参照实现导出的公开名（本层只落地 `count`，逐条见 §5.2.6）。")
    names = sorted(name for name in dir(itertools) if not name.startswith("_"))
    lines.append("pub static REFERENCE_NAMES: &[&str] = &[")
    for name in names:
        lines.append(f'    "{name}",')
    lines.append("];")
    lines.append("")
    FIXTURE.write_text("\n".join(lines))
    print(
        f"已写入 {DOC.relative_to(ROOT)}（{len(itertools.__doc__)} 字节）与 "
        f"{FIXTURE.relative_to(ROOT)}（{len(cases)} 组序列、{len(names)} 个公开名）"
    )


if __name__ == "__main__":
    main()
