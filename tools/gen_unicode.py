#!/usr/bin/env python3
"""从参照实现**探测** Unicode 数据，生成：

- `crates/pyawa-stdlib/src/unicode_tables.rs`：`unidata_version` ＋ 四张区段表
  （通用类别、双向类别、组合类、东亚宽度）
- `crates/pyawa-stdlib/tests/fixtures/unicode.rs`：对拍夹具（抽样 ＋ 每段首尾）

`CM-13` 要求 Unicode 版本**必须**与参照实现一致（以 `unicodedata.unidata_version` 为准）；
`CM-22` 要求数据表**必须**由探测导出（**禁止手写**）；`CM-24` **禁止**拿 Rust 生态的
Unicode crate 当权威——版本不由我们控制。因此本脚本不写死任何版本号或取值：
版本字符串与每个码点的取值全部现取，换参照实现重跑即可（`CM-23` 的四步）。

夹具生成 **Rust** 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器
（与 `fixtures/builtins.rs` 同一取舍），一份数据只放一个地方。

用法::

    python3 tools/gen_unicode.py
"""

from __future__ import annotations

import pathlib
import unicodedata

ROOT = pathlib.Path(__file__).resolve().parents[1]
TABLE_OUT = ROOT / "crates/pyawa-stdlib/src/unicode_tables.rs"
FIXTURE = ROOT / "crates/pyawa-stdlib/tests/fixtures/unicode.rs"

MAX_CODE_POINT = 0x10FFFF
# 抽样步长取质数：不与任何常见区段长度共振。类别表用得最早，给最密的步长；
# 其余三张表靠"每段首尾"覆盖边界，定步长只作兜底。
CATEGORY_SAMPLE_STEP = 997
OTHER_SAMPLE_STEP = 8009

# 四张表：(键, 访问器名, 区段常量名, 参照实现里取值的那一头, 取不出来时的缺省值)
TABLES = [
    ("category", "general_category", "GENERAL_CATEGORY_RANGES", "category", "Cn"),
    (
        "bidirectional",
        "bidirectional",
        "BIDIRECTIONAL_RANGES",
        "bidirectional",
        "",
    ),
    ("combining", "combining", "COMBINING_RANGES", "combining", 0),
    (
        "east_asian_width",
        "east_asian_width",
        "EAST_ASIAN_WIDTH_RANGES",
        "east_asian_width",
        "N",
    ),
]


def probe(function_name: str, value, default):
    """取参照实现的值；取不出来（代理区一类）就给这张表的缺省值。"""
    try:
        return getattr(unicodedata, function_name)(chr(value))
    except ValueError:
        return default


def collect_ranges(function_name: str, default) -> list[tuple[int, int, object]]:
    """按区段压缩：相邻同值合成一段；结果覆盖 `0..=MAX_CODE_POINT`，连续且不重叠。"""
    ranges: list[tuple[int, int, object]] = []
    previous = None
    start = 0
    for code_point in range(MAX_CODE_POINT + 1):
        value = probe(function_name, code_point, default)
        if value != previous:
            if previous is not None:
                ranges.append((start, code_point - 1, previous))
            start, previous = code_point, value
    ranges.append((start, MAX_CODE_POINT, previous))
    return ranges


def collect_samples(
    function_name: str, ranges: list[tuple[int, int, object]], step: int
) -> list[tuple[int, object]]:
    """抽样：定步长 ＋ 每段的**首尾**（边界最容易写错）。"""
    points = set(range(0, MAX_CODE_POINT + 1, step))
    for start, end, _ in ranges:
        points.add(start)
        points.add(end)
    samples = []
    for code_point in sorted(points):
        try:
            value = getattr(unicodedata, function_name)(chr(code_point))
        except ValueError:
            continue
        samples.append((code_point, value))
    return samples


def render_table(tables: dict[str, list[tuple[int, int, object]]]) -> str:
    lines = [
        "//! **Unicode 数据表**（生成产物；`CM-13`／`CM-22`／`CM-24`）。",
        "//!",
        "//! 由 `tools/gen_unicode.py` **探测参照实现**导出——**禁止手写**，"
        "**禁止**改用 Rust 生态的 Unicode crate",
        "//! （版本不由我们控制 ⇒ 版本与行为会双重不一致）。参照实现升补丁版本且",
        "//! `unidata_version` 变化时，按 `CM-23` 重跑导出并在提交说明里写明差异。",
        "//!",
        f"//! 本文件由脚本生成于参照实现 `unicodedata.unidata_version == "
        f"{unicodedata.unidata_version}`。",
        "",
        "/// 参照实现的 `unicodedata.unidata_version`（`CM-13`：**必须**报同一字符串）。",
        f'pub const UNIDATA_VERSION: &str = "{unicodedata.unidata_version}";',
        "",
        "/// 四张表共用的二分查找：区段按码点升序、连续且不重叠，合起来覆盖 `0..=0x10FFFF`。",
        "/// 码点越界给 `None`（调用方按越界报错）。",
        "fn lookup<T: Copy>(ranges: &[(u32, u32, T)], code_point: u32) -> Option<T> {",
        "    if code_point > 0x10FFFF {",
        "        return None;",
        "    }",
        "    let mut low = 0usize;",
        "    let mut high = ranges.len();",
        "    while low < high {",
        "        let middle = (low + high) / 2;",
        "        let (start, end, value) = ranges[middle];",
        "        if code_point < start {",
        "            high = middle;",
        "        } else if code_point > end {",
        "            low = middle + 1;",
        "        } else {",
        "            return Some(value);",
        "        }",
        "    }",
        "    None",
        "}",
        "",
        "/// 通用类别（`unicodedata.category`）：代理区照参照实现给 `Cs`。",
        "pub static GENERAL_CATEGORY_RANGES: &[(u32, u32, &str)] = &[",
    ]
    for start, end, value in tables["category"]:
        lines.append(f'    (0x{start:04X}, 0x{end:04X}, "{value}"),')
    lines.append("];")
    lines.append("")
    lines.append("/// `unicodedata.category(chr(code_point))`。")
    lines.append("pub fn general_category(code_point: u32) -> Option<&'static str> {")
    lines.append("    lookup(GENERAL_CATEGORY_RANGES, code_point)")
    lines.append("}")
    lines.append("")
    lines.append("/// 双向类别（`unicodedata.bidirectional`）；缺省值是空串。")
    lines.append("pub static BIDIRECTIONAL_RANGES: &[(u32, u32, &str)] = &[")
    for start, end, value in tables["bidirectional"]:
        lines.append(f'    (0x{start:04X}, 0x{end:04X}, "{value}"),')
    lines.append("];")
    lines.append("")
    lines.append("/// `unicodedata.bidirectional(chr(code_point))`。")
    lines.append("pub fn bidirectional(code_point: u32) -> Option<&'static str> {")
    lines.append("    lookup(BIDIRECTIONAL_RANGES, code_point)")
    lines.append("}")
    lines.append("")
    lines.append("/// 组合类（`unicodedata.combining`）；缺省值是 `0`。")
    lines.append("pub static COMBINING_RANGES: &[(u32, u32, u32)] = &[")
    for start, end, value in tables["combining"]:
        lines.append(f"    (0x{start:04X}, 0x{end:04X}, {value}),")
    lines.append("];")
    lines.append("")
    lines.append("/// `unicodedata.combining(chr(code_point))`。")
    lines.append("pub fn combining(code_point: u32) -> Option<u32> {")
    lines.append("    lookup(COMBINING_RANGES, code_point)")
    lines.append("}")
    lines.append("")
    lines.append("/// 东亚宽度（`unicodedata.east_asian_width`）；缺省值是 `N`。")
    lines.append("pub static EAST_ASIAN_WIDTH_RANGES: &[(u32, u32, &str)] = &[")
    for start, end, value in tables["east_asian_width"]:
        lines.append(f'    (0x{start:04X}, 0x{end:04X}, "{value}"),')
    lines.append("];")
    lines.append("")
    lines.append("/// `unicodedata.east_asian_width(chr(code_point))`。")
    lines.append("pub fn east_asian_width(code_point: u32) -> Option<&'static str> {")
    lines.append("    lookup(EAST_ASIAN_WIDTH_RANGES, code_point)")
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def render_fixture(samples: dict[str, list[tuple[int, object]]]) -> str:
    lines = [
        "//! 由 `tools/gen_unicode.py` 探测参照实现导出；**禁止手改**。",
        f"//! 参照实现 `unidata_version`：{unicodedata.unidata_version}",
        "//!",
        "//! 生成 Rust 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器"
        "（与 `fixtures/builtins.rs` 同一取舍），一份数据只放一个地方。",
        "",
        f'pub const UNIDATA_VERSION: &str = "{unicodedata.unidata_version}";',
        f"pub const MAX_CODE_POINT: u32 = 0x{MAX_CODE_POINT:04X};",
        "",
        "/// 参照实现认识的通用类别集合（升序）。",
        "pub static CATEGORIES: &[&str] = &[",
    ]
    for category in sorted({value for _, _, value in samples_ranges_category}):
        lines.append(f'    "{category}",')
    lines.append("];")
    lines.append("")
    for key, doc, value_type in [
        ("category", "`unicodedata.category` 的抽样", "&'static str"),
        ("bidirectional", "`unicodedata.bidirectional` 的抽样", "&'static str"),
        ("combining", "`unicodedata.combining` 的抽样", "u32"),
        ("east_asian_width", "`unicodedata.east_asian_width` 的抽样", "&'static str"),
    ]:
        lines.append(f"/// {doc}：`(码点, 参照实现给的值)`——定步长 ＋ 每段首尾。")
        lines.append(f"pub static {key.upper()}_SAMPLES: &[(u32, {value_type})] = &[")
        for code_point, value in samples[key]:
            if isinstance(value, str):
                lines.append(f'    (0x{code_point:04X}, "{value}"),')
            else:
                lines.append(f"    (0x{code_point:04X}, {value}),")
        lines.append("];")
        lines.append("")
    return "\n".join(lines)


def main() -> None:
    global samples_ranges_category
    tables = {
        key: collect_ranges(function, default) for key, _, _, function, default in TABLES
    }
    samples_ranges_category = tables["category"]
    samples = {
        key: collect_samples(
            function,
            tables[key],
            CATEGORY_SAMPLE_STEP if key == "category" else OTHER_SAMPLE_STEP,
        )
        for key, _, _, function, _ in TABLES
    }
    TABLE_OUT.write_text(render_table(tables))
    FIXTURE.write_text(render_fixture(samples))
    print(
        f"已写入 {TABLE_OUT.relative_to(ROOT)}（"
        + "／".join(f"{key} {len(value)} 段" for key, value in tables.items())
        + f"；unidata_version = {unicodedata.unidata_version}）"
    )
    print(
        f"已写入 {FIXTURE.relative_to(ROOT)}（抽样 "
        + "／".join(f"{key} {len(value)}" for key, value in samples.items())
        + "）"
    )


if __name__ == "__main__":
    main()
