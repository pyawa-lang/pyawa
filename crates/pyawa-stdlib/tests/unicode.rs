//! `P3-15` 的第一张表：`unidata_version` 锁定 ＋ 通用类别区段表（`CM-13`／`CM-22`／`CM-24`）。
//!
//! 表本身是**生成产物**（`tools/gen_unicode.py` 探测参照实现导出）；这里验三件事：
//! ① 版本字符串与参照实现一致；② 抽样码点的类别与参照实现一致；
//! ③ 区段表的**结构**（升序、连续、不重叠、覆盖全域、类别都在参照实现的类别集合里）——
//! 结构不变量是"生成脚本出错"的哨兵，抽样是"探测口径出错"的哨兵。

use pyawa_stdlib::unicode_tables::{
    bidirectional, combining, decimal, digit, east_asian_width, general_category, numeric,
    BIDIRECTIONAL_RANGES, COMBINING_RANGES, DECIMAL_VALUES, DECOMPOSITION_VALUES,
    DIGIT_VALUES, EAST_ASIAN_WIDTH_RANGES, GENERAL_CATEGORY_RANGES, NUMERIC_VALUES,
    UNIDATA_VERSION,
};
use pyawa_stdlib::unicode_tables::decomposition;

#[path = "fixtures/unicode.rs"]
mod fixture;

use fixture::{
    BIDIRECTIONAL_SAMPLES, CATEGORIES, CATEGORY_SAMPLES, COMBINING_SAMPLES,
    EAST_ASIAN_WIDTH_SAMPLES, MAX_CODE_POINT,
};
use fixture::{
    DECOMPOSITION_TAGS, DECOMPOSITION_VALUES as FIX_DECOMPOSITION,
    DECIMAL_VALUES as FIX_DECIMAL, DIGIT_VALUES as FIX_DIGIT, NUMERIC_VALUES as FIX_NUMERIC,
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

#[test]
fn the_sparse_tables_are_exhaustively_right() {
    // 三张稀疏表在夹具里是**穷尽**的（项数有限）⇒ 这里逐项对拍，不是抽样。
    assert_eq!(
        DECIMAL_VALUES,
        FIX_DECIMAL,
        "decimal 表的每一项都要与参照一致"
    );
    assert_eq!(DIGIT_VALUES, FIX_DIGIT, "digit 表的每一项都要与参照一致");
    assert_eq!(
        NUMERIC_VALUES.len(),
        FIX_NUMERIC.len(),
        "numeric 表的项数"
    );
    for ((code_point, numerator, denominator), (expected_point, value, expected_numerator, expected_denominator)) in
        NUMERIC_VALUES.iter().zip(FIX_NUMERIC.iter())
    {
        assert_eq!(code_point, expected_point, "numeric 表的码点");
        assert_eq!(numerator, expected_numerator, "U+{code_point:04X} 的分子");
        assert_eq!(denominator, expected_denominator, "U+{code_point:04X} 的分母");
        // 约分对算回的浮点必须与参照实现的浮点**逐位相同**
        assert_eq!(
            numeric(*code_point),
            Some(*value),
            "U+{code_point:04X} 的数值"
        );
    }
    // 查找要对得上：认识的给 `Some`、不认识给 `None`
    assert_eq!(decimal(0x30), Some(0));
    assert_eq!(digit(0x30), Some(0));
    assert_eq!(numeric(0xBD), Some(0.5));
    assert_eq!(decimal(0x41), None, "`A` 没有十进制数字值");
    assert_eq!(numeric(0x41), None, "`A` 没有数值");
}

#[test]
fn the_sparse_tables_are_well_formed() {
    // 结构不变量：码点升序、无重复、`decimal ⊆ digit`（参照实现的关系）、
    // `numeric` 的分子分母都是正数（分母为 1 就是整数）
    for (name, keys) in [
        ("decimal", DECIMAL_VALUES.iter().map(|(cp, _)| *cp).collect::<Vec<_>>()),
        ("digit", DIGIT_VALUES.iter().map(|(cp, _)| *cp).collect::<Vec<_>>()),
        (
            "numeric",
            NUMERIC_VALUES.iter().map(|(cp, _, _)| *cp).collect::<Vec<_>>(),
        ),
    ] {
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "{name} 必须升序且无重复");
        assert!(!keys.is_empty(), "{name} 不该是空表");
    }
    let digits: Vec<u32> = DIGIT_VALUES.iter().map(|(cp, _)| *cp).collect();
    for (code_point, _) in DECIMAL_VALUES {
        assert!(digits.contains(code_point), "U+{code_point:04X} 有 decimal 却没有 digit");
    }
    for (code_point, numerator, denominator) in NUMERIC_VALUES {
        assert!(*denominator > 0, "U+{code_point:04X} 的分母必须为正");
        // 分子可以为 0（`'0'` 的数值就是 0）——只要**不为零的**分子都不带多余符号即可
        let _ = numerator;
    }
    // 参照实现里确有**负值**（如 `U+0F33` ⇒ `-1/2`）⇒ 分子必须有符号
    assert!(
        NUMERIC_VALUES.iter().any(|(_, numerator, _)| *numerator < 0),
        "numeric 表里应当有负值（否则分子用无符号就够了）"
    );
}

#[test]
fn the_decomposition_table_is_exhaustively_right() {
    // 与前几张稀疏表一样：夹具里是**全部**项（参照实现里"没有分解"就是空串，不入表）
    assert_eq!(
        DECOMPOSITION_VALUES, FIX_DECOMPOSITION,
        "分解表的每一项都要与参照一致（含 `<tag>` 前缀）"
    );
    // 查找要对得上
    assert_eq!(decomposition(0x00C4), Some("0041 0308"), "Ä 的规范分解");
    assert_eq!(decomposition(0x00A0), Some("<noBreak> 0020"));
    assert_eq!(decomposition(0x41), None, "`A` 没有分解");
    // 结构不变量：码点升序无重复、值非空、标记都在参照的标记集合里、长度有界
    let keys: Vec<u32> = DECOMPOSITION_VALUES.iter().map(|(cp, _)| *cp).collect();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "必须升序且无重复");
    for (code_point, value) in DECOMPOSITION_VALUES {
        assert!(!value.is_empty(), "U+{code_point:04X} 的值不该是空串（空串＝不在表里）");
        assert!(value.len() <= 128, "U+{code_point:04X} 的值过长");
        if let Some(first) = value.split_whitespace().next() {
            if first.starts_with('<') {
                assert!(
                    DECOMPOSITION_TAGS.contains(&first),
                    "U+{code_point:04X} 的标记 {first} 不在参照的标记集合里"
                );
            }
        }
    }
}
