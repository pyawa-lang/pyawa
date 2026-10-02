//! `P3-15` 的第一张表：`unidata_version` 锁定 ＋ 通用类别区段表（`CM-13`／`CM-22`／`CM-24`）。
//!
//! 表本身是**生成产物**（`tools/gen_unicode.py` 探测参照实现导出）；这里验三件事：
//! ① 版本字符串与参照实现一致；② 抽样码点的类别与参照实现一致；
//! ③ 区段表的**结构**（升序、连续、不重叠、覆盖全域、类别都在参照实现的类别集合里）——
//! 结构不变量是"生成脚本出错"的哨兵，抽样是"探测口径出错"的哨兵。

use pyawa_stdlib::unicode_tables::{general_category, GENERAL_CATEGORY_RANGES, UNIDATA_VERSION};

#[path = "fixtures/unicode.rs"]
mod fixture;

use fixture::{CATEGORIES, MAX_CODE_POINT, SAMPLES};

#[test]
fn the_version_string_is_locked_to_the_reference() {
    // `CM-13`：以 `unicodedata.unidata_version` 为准，必须报同一字符串
    assert_eq!(UNIDATA_VERSION, fixture::UNIDATA_VERSION);
}

#[test]
fn sampled_code_points_match_the_reference() {
    let mut checked = 0usize;
    for (code_point, expected) in SAMPLES {
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
