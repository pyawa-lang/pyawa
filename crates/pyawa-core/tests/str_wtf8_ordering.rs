//! **Task-66（BL）**：含孤立代理的 `str` 参与排序/大小比较 —— 按**码点序**（代理各 1 个码点），
//! 合法 UTF-8 路径结果零变化；`<`／`<=`／`>`／`>=` 与 `sorted`／`min`／`max`／`list.sort`
//! 走的 `order_of`／`compare_public` 同一处真相都不再 panic。

use core::ptr::NonNull;
use core::cmp::Ordering;
use pyawa_core::executor::compare_public;
use pyawa_core::Header;
use pyawa_core::Instance;

#[test]
fn str_order_compares_code_points_including_surrogates() {
    let instance = Instance::new();
    let lone_start = instance.new_str_wtf8(b"\xED\xA0\x80").expect("U+D800"); // U+D800
    let lone_end = instance.new_str_wtf8(b"\xED\xBF\xBF").expect("U+DFFF"); // U+DFFF
    let e000 = instance.new_str("\u{E000}");
    let a = instance.new_str("a");
    let e_acute = instance.new_str("é");
    // `'a'` < `'é'` < U+D800 < U+DFFF < U+E000（码点序；UTF-8 保序 ✓）
    let ascending = [a, e_acute, lone_start, lone_end, e000];
    for window in ascending.windows(2) {
        assert_eq!(
            instance.str_order_cmp(window[0], window[1]),
            Some(Ordering::Less),
            "码点序：前者应更小"
        );
        assert_eq!(
            instance.str_order_cmp(window[1], window[0]),
            Some(Ordering::Greater)
        );
    }
    // 前缀规则：短的小 ✓（含代理）
    let with_suffix = instance.new_str_wtf8(b"\xED\xA0\x80a").expect("U+D800 + a");
    assert_eq!(instance.str_order_cmp(lone_start, with_suffix), Some(Ordering::Less));
    assert_eq!(
        instance.str_order_cmp(with_suffix, lone_start),
        Some(Ordering::Greater)
    );
    // 相等（WTF-8 字节相等）
    let same = instance.new_str_wtf8(b"\xED\xA0\x80").expect("U+D800");
    assert_eq!(instance.str_order_cmp(lone_start, same), Some(Ordering::Equal));
    // 非 str ⇒ None（不是 panic ✓）
    let number = instance.new_int(1);
    assert_eq!(instance.str_order_cmp(lone_start, number), None);
}

#[test]
fn compare_public_supports_all_four_operators_on_wtf8_str() {
    let instance = Instance::new();
    let lone = instance.new_str_wtf8(b"\xED\xA0\x80").expect("U+D800");
    let a = instance.new_str("a");
    let compare = |left, right, symbol| {
        compare_public(&instance, left, right, symbol, 0).expect("str 比较不应报错")
    };
    assert!(compare(a, lone, "<"));
    assert!(compare(a, lone, "<="));
    assert!(!compare(a, lone, ">"));
    assert!(!compare(a, lone, ">="));
    assert!(compare(lone, a, ">"));
    assert!(compare(lone, a, ">="));
    assert!(!compare(lone, a, "<"));
    assert!(compare(lone, lone, "<="));
    assert!(compare(lone, lone, ">="));
    assert!(!compare(lone, lone, "<"));
    assert!(!compare(lone, lone, ">"));
    // U+D800 < U+DFFF（代理之间也按码点 ✓）
    let lone_end = instance.new_str_wtf8(b"\xED\xBF\xBF").expect("U+DFFF");
    assert!(compare(lone, lone_end, "<"));
    assert!(compare(lone_end, lone, ">"));
    // 与合法 BMP 字符比：U+D800 < U+E000 ✓
    let e000 = instance.new_str("\u{E000}");
    assert!(compare(lone, e000, "<"));
    assert!(compare(e000, lone_end, ">"));
}

#[test]
fn order_of_handles_wtf8_str_for_sorted_and_list_sort() {
    let instance = Instance::new();
    let lone = instance.new_str_wtf8(b"\xED\xA0\x80").expect("U+D800");
    let lone_end = instance.new_str_wtf8(b"\xED\xBF\xBF").expect("U+DFFF");
    let a = instance.new_str("a");
    let e_acute = instance.new_str("é");
    let e000 = instance.new_str("\u{E000}");
    // 模拟 `sorted([...])`：`order_of` 是 `sorted`／`min`／`max`／`list.sort` 的同一处真相 ✓
    let mut items = vec![e000, lone, a, e_acute, lone_end];
    items.sort_by(|left, right| {
        instance
            .order_of(*left, *right)
            .expect("含代理的 str 之间必须可比")
    });
    let expected = [
        (b"a".as_slice()),
        ("é".as_bytes()),
        (b"\xED\xA0\x80".as_slice()),
        (b"\xED\xBF\xBF".as_slice()),
        ("\u{E000}".as_bytes()),
    ];
    for (item, expected_bytes) in items.iter().zip(expected) {
        assert_eq!(
            instance.str_wtf8_bytes(*item),
            Some(expected_bytes),
            "排序位置"
        );
    }
}

#[test]
fn legal_utf8_ordering_is_unchanged_vs_rust_str_cmp() {
    // 合法 UTF-8 的回归：`str_order_cmp` 与旧的 `String::cmp`／`str::cmp` 逐对相同 ✓
    let instance = Instance::new();
    let texts = ["", "a", "ab", "abc", "abd", "b", "héllo", "hí", "hello", "中", "🦀", "é"];
    let objects: Vec<NonNull<Header>> = texts.iter().map(|text| instance.new_str(text)).collect();
    for (index, left) in texts.iter().enumerate() {
        for (other_index, right) in texts.iter().enumerate() {
            assert_eq!(
                instance.str_order_cmp(objects[index], objects[other_index]),
                Some(left.cmp(right)),
                "{left:?} vs {right:?}"
            );
            // `compare_public` 四个运算符与 Rust 结果一致 ✓
            for (symbol, expected) in [
                ("<", left < right),
                ("<=", left <= right),
                (">", left > right),
                (">=", left >= right),
            ] {
                assert_eq!(
                    compare_public(&instance, objects[index], objects[other_index], symbol, 0)
                        .expect("比较应成功"),
                    expected,
                    "{left:?} {symbol} {right:?}"
                );
            }
        }
    }
}
