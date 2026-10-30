//! **Task-69（S2c 收尾）**：含孤立代理的 `str` 在**用户可触发的 core 文本面**上**不 panic**。
//!
//! `StrObject::value()` 对 WTF-8 表示是守卫（panic）；本文件钉住它之外的**安全入口**：
//! `object_str`／`object_repr`／`object_ascii`／`text_value`（有损）／`text_of`（合法视图 None）／
//! 排序比较／长度／迭代——全部返回结果或 `None`，**不得 panic**（有损内容如实断言）。
//!
//! 运行期 e2e（`str(s)`／`f"{s:>5}"`／`"%s" % s`／`"{:>5}".format(s)`／match／argparse／子类）
//! 见 `target/recon/t69_matrix.py` 与 report（python3 3.14.4 对拍）。

use core::ptr::NonNull;
use pyawa_core::Header;
use pyawa_core::Instance;

/// `a` + U+D800 + `b` 的 WTF-8 对象。
fn surrogate(instance: &Instance) -> NonNull<Header> {
    instance
        .new_str_wtf8(b"a\xED\xA0\x80b")
        .expect("合法 WTF-8")
}

#[test]
fn text_views_on_wtf8_are_safe() {
    let instance = Instance::new();
    let object = surrogate(&instance);
    // 合法视图：含代理 ⇒ None（不是 panic ✓）
    assert_eq!(instance.text_of(object), None);
    // owned 视图：有损（U+FFFD），绝不 panic ✓
    assert_eq!(instance.text_value(object), Some("a\u{FFFD}b".to_owned()));
    // repr／ascii：转义成合法 UTF-8 ASCII ✓
    assert_eq!(instance.object_repr(object).expect("repr 应当成功"), "'a\\ud800b'");
    assert_eq!(instance.object_ascii(object).expect("ascii 应当成功"), "'a\\ud800b'");
    // str 槽（`str(x)` 的回落路径）也有损但不 panic ✓
    let text = instance.object_str(object).expect("str 应当成功");
    assert!(text.contains('\u{FFFD}'), "有损文本：{text:?}");
    // 容器内嵌的 repr 也安全 ✓
    let list = instance.new_list(vec![instance.retain(object)]);
    assert_eq!(
        instance.object_repr(list).expect("容器 repr 应当成功"),
        "['a\\ud800b']"
    );
}

#[test]
fn lengths_and_code_points_on_wtf8_are_safe() {
    let instance = Instance::new();
    let object = surrogate(&instance);
    assert_eq!(instance.str_code_point_len(object), Some(3));
    assert_eq!(instance.length_with_protocol(object), Ok(Some(3)));
    let points = instance.str_code_point_vec(object).expect("码点表");
    assert_eq!(points, vec![u32::from('a'), 0xD800, u32::from('b')]);
    assert_eq!(instance.str_code_point_at(object, 1), Some(0xD800));
    assert_eq!(instance.str_code_point_at(object, 3), None);
}

#[test]
fn ordering_on_wtf8_is_safe() {
    let instance = Instance::new();
    let object = surrogate(&instance);
    let a = instance.new_str("a");
    let plain = instance.new_str("a");
    // `compare_public` 的四个运算符都不 panic ✓
    for symbol in ["<", "<=", ">", ">=", "==", "!="] {
        let verdict = pyawa_core::executor::compare_public(&instance, plain, object, symbol, 0)
            .expect("str 比较应当成功");
        let expected = match symbol {
            "<" | "<=" | "!=" => true,
            _ => false,
        };
        assert_eq!(verdict, expected, "{symbol}");
    }
    // `order_of`（sorted/min/max/list.sort 的同一处真相）也不 panic ✓
    assert_eq!(
        instance.order_of(a, object),
        Some(core::cmp::Ordering::Less)
    );
    assert_eq!(
        instance.order_of(object, a),
        Some(core::cmp::Ordering::Greater)
    );
}
