#!/usr/bin/env python3
"""从参照实现**探测** Unicode 数据，生成：

- `crates/pyawa-stdlib/src/unicode_tables.rs`：`unidata_version` ＋ 通用类别**区段表**
- `crates/pyawa-stdlib/tests/fixtures/unicode.rs`：对拍夹具（抽样 ＋ 区段边界）

`CM-13` 要求 Unicode 版本**必须**与参照实现一致（以 `unicodedata.unidata_version` 为准）；
`CM-22` 要求数据表**必须**由探测导出（**禁止手写**）；`CM-24` **禁止**拿 Rust 生态的
Unicode crate 当权威——版本不由我们控制。因此本脚本不写死任何版本号或类别：
版本字符串与 `category(chr(cp))` 全部现取，换参照实现重跑即可（`CM-23` 的四步）。

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
# 抽样步长取质数：不与任何常见区段长度共振，能落到各种类别里
SAMPLE_STEP = 997


def general_category(code_point: int) -> str | None:
    """参照实现给的通用类别；代理区给 `None`（`chr` 取不出来）。"""
    try:
        return unicodedata.category(chr(code_point))
    except ValueError:
        return None


def collect_ranges() -> list[tuple[int, int, str]]:
    """按区段压缩：相邻同类别合成一段（覆盖 0..=MAX_CODE_POINT，含代理区）。"""
    ranges: list[tuple[int, int, str]] = []
    previous: str | None = None
    start = 0
    for code_point in range(MAX_CODE_POINT + 1):
        category = general_category(code_point) or "Cs"  # 代理区：照参照实现的类别
        if category != previous:
            if previous is not None:
                ranges.append((start, code_point - 1, previous))
            start = code_point
            previous = category
    ranges.append((start, MAX_CODE_POINT, previous or "Cn"))
    return ranges


def collect_samples() -> list[tuple[int, str]]:
    """抽样：定步长 ＋ 每段的**首尾**（边界是最容易写错的地方）。"""
    points = set(range(0, MAX_CODE_POINT + 1, SAMPLE_STEP))
    for start, end, _ in collect_ranges():
        points.add(start)
        points.add(end)
    samples = []
    for code_point in sorted(points):
        category = general_category(code_point)
        if category is None:
            continue
        samples.append((code_point, category))
    return samples


def render_table(ranges: list[tuple[int, int, str]]) -> str:
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
        "/// 通用类别区段表：`(起始码点, 结束码点, 类别名)`，按码点升序、**连续且不重叠**，",
        "/// 合起来覆盖 `0..=0x10FFFF`（含代理区，类别照参照实现给的 `Cs`）。",
        "pub static GENERAL_CATEGORY_RANGES: &[(u32, u32, &str)] = &[",
    ]
    for start, end, category in ranges:
        lines.append(f'    (0x{start:04X}, 0x{end:04X}, "{category}"),')
    lines.append("];")
    lines.append("")
    lines.append("/// `unicodedata.category(chr(code_point))` 的区段表口径（二分查找）。")
    lines.append("///")
    lines.append("/// 码点超出 `0..=0x10FFFF` 时给 `None`（调用方按越界报错）。")
    lines.append("pub fn general_category(code_point: u32) -> Option<&'static str> {")
    lines.append("    if code_point > 0x10FFFF {")
    lines.append("        return None;")
    lines.append("    }")
    lines.append("    let mut low = 0usize;")
    lines.append("    let mut high = GENERAL_CATEGORY_RANGES.len();")
    lines.append("    while low < high {")
    lines.append("        let middle = (low + high) / 2;")
    lines.append("        let (start, end, category) = GENERAL_CATEGORY_RANGES[middle];")
    lines.append("        if code_point < start {")
    lines.append("            high = middle;")
    lines.append("        } else if code_point > end {")
    lines.append("            low = middle + 1;")
    lines.append("        } else {")
    lines.append("            return Some(category);")
    lines.append("        }")
    lines.append("    }")
    lines.append("    None")
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def render_fixture(
    ranges: list[tuple[int, int, str]], samples: list[tuple[int, str]]
) -> str:
    categories = sorted({category for _, _, category in ranges})
    lines = [
        "//! 由 `tools/gen_unicode.py` 探测参照实现导出；**禁止手改**。",
        f"//! 参照实现 `unidata_version`：{unicodedata.unidata_version}",
        "//!",
        "//! 生成 Rust 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器"
        "（与 `fixtures/builtins.rs` 同一取舍）。",
        "",
        f'pub const UNIDATA_VERSION: &str = "{unicodedata.unidata_version}";',
        f"pub const MAX_CODE_POINT: u32 = 0x{MAX_CODE_POINT:04X};",
        "",
        "/// 参照实现认识的通用类别集合（升序）。",
        "pub static CATEGORIES: &[&str] = &[",
    ]
    for category in categories:
        lines.append(f'    "{category}",')
    lines.append("];")
    lines.append("")
    lines.append("/// 抽样：`(码点, 参照实现给的类别)`——定步长 ＋ 每段首尾。")
    lines.append("pub static SAMPLES: &[(u32, &str)] = &[")
    for code_point, category in samples:
        lines.append(f'    (0x{code_point:04X}, "{category}"),')
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    ranges = collect_ranges()
    samples = collect_samples()
    TABLE_OUT.write_text(render_table(ranges))
    FIXTURE.write_text(render_fixture(ranges, samples))
    print(
        f"已写入 {TABLE_OUT.relative_to(ROOT)}（{len(ranges)} 个区段，"
        f"unidata_version = {unicodedata.unidata_version}）"
    )
    print(
        f"已写入 {FIXTURE.relative_to(ROOT)}（抽样 {len(samples)} 个码点，"
        f"{len({category for _, _, category in ranges})} 个类别）"
    )


if __name__ == "__main__":
    main()
