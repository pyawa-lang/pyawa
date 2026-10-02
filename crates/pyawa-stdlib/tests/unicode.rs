//! `P3-15` 的第一张表：`unidata_version` 锁定 ＋ 通用类别区段表（`CM-13`／`CM-22`／`CM-24`）。
//!
//! 表本身是**生成产物**（`tools/gen_unicode.py` 探测参照实现导出）；这里验三件事：
//! ① 版本字符串与参照实现一致；② 抽样码点的类别与参照实现一致；
//! ③ 区段表的**结构**（升序、连续、不重叠、覆盖全域、类别都在参照实现的类别集合里）——
//! 结构不变量是"生成脚本出错"的哨兵，抽样是"探测口径出错"的哨兵。

use pyawa_stdlib::unicode_tables::{
    bidirectional, combining, east_asian_width, general_category, BIDIRECTIONAL_RANGES,
    COMBINING_RANGES, EAST_ASIAN_WIDTH_RANGES, GENERAL_CATEGORY_RANGES, UNIDATA_VERSION,
};

#[path = "fixtures/unicode.rs"]
mod fixture;

use fixture::{
    BIDIRECTIONAL_SAMPLES, CATEGORIES, CATEGORY_SAMPLES, COMBINING_SAMPLES,
    EAST_ASIAN_WIDTH_SAMPLES, MAX_CODE_POINT,
};

#[test]
fn the_version_string_is_locked_to_the_reference() {
    // `CM-13`：以 `unicodedata.unidata_version` 为准，必须报同一字符串
    assert_eq!(UNIDATA_VERSION, fixture::UNIDATA_VERSION);
}

#[test]
fn sampled_code_points_match_the_reference() {
    let mut checked = 0usize;
    for (code_point, expected) in CATEGORY_SAMPLES {
        assert_eq!(
            general_category(*code_point),
            Some(*expected),
            "码点 U+{code_point:04X} 的类别"
        );
        checked += 1;
    }
    // `BC-59` 的口径：跳过的样本要少 ⇒ 抽样密度得够（每约 155 个码点一个样本）
    assert!(
        checked * 200 >= MAX_CODE_POINT as usize,
        "抽样太稀：{checked} 个样本对 {MAX_CODE_POINT} 个码点"
    );
}

#[test]
fn the_other_three_tables_match_the_reference() {
    // 双向类别／组合类／东亚宽度：同样是"抽样 ＋ 每段首尾"对拍（`CM-22`）
    for (code_point, expected) in BIDIRECTIONAL_SAMPLES {
        assert_eq!(
            bidirectional(*code_point),
            Some(*expected),
            "U+{code_point:04X} 的双向类别"
        );
    }
    for (code_point, expected) in COMBINING_SAMPLES {
        assert_eq!(
            combining(*code_point),
            Some(*expected),
            "U+{code_point:04X} 的组合类"
        );
    }
    for (code_point, expected) in EAST_ASIAN_WIDTH_SAMPLES {
        assert_eq!(
            east_asian_width(*code_point),
            Some(*expected),
            "U+{code_point:04X} 的东亚宽度"
        );
    }
    // 越界一律 `None`
    assert_eq!(bidirectional(MAX_CODE_POINT + 1), None);
    assert_eq!(combining(MAX_CODE_POINT + 1), None);
    assert_eq!(east_asian_width(MAX_CODE_POINT + 1), None);
}

#[test]
fn every_table_covers_the_whole_range() {
    // 结构不变量对四张表都成立（生成脚本出错时这里先红）
    for (name, ranges) in [
        ("general_category", GENERAL_CATEGORY_RANGES.len()),
        ("bidirectional", BIDIRECTIONAL_RANGES.len()),
        ("combining", COMBINING_RANGES.len()),
        ("east_asian_width", EAST_ASIAN_WIDTH_RANGES.len()),
    ] {
        assert!(ranges > 100, "{name} 的区段数太少（{ranges}）");
    }
    let ends: Vec<u32> = COMBINING_RANGES.iter().map(|(_, end, _)| *end).collect();
    let starts: Vec<u32> = COMBINING_RANGES.iter().map(|(start, _, _)| *start).collect();
    assert_eq!(starts[0], 0, "组合类表要从 U+0000 起");
    assert_eq!(*ends.last().expect("非空"), MAX_CODE_POINT, "要盖到最后一个码点");
    for (pair, end) in starts.windows(2).zip(ends.iter()) {
        assert_eq!(pair[1], end + 1, "区段必须连续（U+{end:04X} 之后）");
    }
}

#[test]
fn the_range_table_is_well_formed() {
    let maximum = MAX_CODE_POINT;
    let mut expected_start = 0u32;
    for (start, end, category) in GENERAL_CATEGORY_RANGES {
        assert_eq!(*start, expected_start, "区段要从 {expected_start:04X} 接着来");
        assert!(end >= start, "区段 {start:04X}..{end:04X} 不能倒过来");
        assert!(CATEGORIES.contains(category), "类别 {category} 不在参照集合里");
        expected_start = end + 1;
    }
    assert_eq!(
        expected_start,
        maximum + 1,
        "区段表必须覆盖到 U+{maximum:04X}"
    );
    // 越界给 `None`（调用方按越界报错）
    assert_eq!(general_category(maximum + 1), None);
}

#[test]
fn every_category_the_reference_knows_is_reachable() {
    // 表里出现过的类别集合 = 参照实现的类别集合（漏一类说明压缩时写错了）
    let mut seen: Vec<&str> = GENERAL_CATEGORY_RANGES
        .iter()
        .map(|(_, _, category)| *category)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, CATEGORIES.to_vec(), "类别集合必须与参照实现一致");
}
