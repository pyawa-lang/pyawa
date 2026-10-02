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
