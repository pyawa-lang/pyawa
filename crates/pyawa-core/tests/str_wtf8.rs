//! **`StrObject` 的 WTF-8 存储升级**（P3-38 S2a）· 零行为改动的回归。
//!
//! S1（[`pyawa_core::wtf8`]）给了编解码；S2a 把 `str` 的**存储**升成
//! `StrRepr::{Utf8(String), Wtf8(Vec<u8>)}`，但**现有生产路径仍只构造 `Utf8`** ✓。
//! 这里钉的正是"升级没有改变旧行为"＋"新入口 `Instance::new_str_wtf8`／`wtf8_of` 的口径"：
//!
//! - `new_str` 的老面（`text_of`／`value()`／`wtf8_bytes()`）与升级前一致；
//! - `new_str_wtf8`：合法 **UTF-8** 字节照旧存 UTF-8 表示；含**孤立代理**的 WTF-8 字节才进
//!   WTF-8 表示（`text_of` ⇒ `None`，`value()` ⇒ 守卫 panic，`wtf8_bytes()`／`wtf8_of()` 给原字节）；
//! - 非法 WTF-8 ⇒ `None`（不按有损转换静默收下）；空串走**空串单例**；
//! - **唯一允许的行为改动**：`text_of` 从 `value()` 改成 `as_utf8()`（WTF-8 ⇒ `None`）——
//!   合法 UTF-8 路径结果不变（S2a 生产路径也不会构造 WTF-8 表示 ⇒ 实际无可观测变化）。

use core::ptr::NonNull;

use pyawa_core::{Instance, StrObject};

/// 借一个 `str` 对象看它的 `StrObject` 载荷（测试专用；类型身份先用 `wtf8_of` 间接确认过 ✓）。
///
/// # Safety
///
/// 调用方保证 `object` 是 `str` 实例（测试里都由 `Instance::new_str*` 造 ✓）。
unsafe fn str_object(object: NonNull<pyawa_core::Header>) -> &'static StrObject {
    // SAFETY: 契约见上；对象由 `instance` 持有、测试期间不释放 ✓。
    unsafe { &*object.as_ptr().cast::<StrObject>() }
}

#[test]
fn plain_str_keeps_the_utf8_behaviour() {
    let instance = Instance::new();
    for text in ["abc", "héllo", "你好", "🦀 crab", "a b c"] {
        let object = instance.new_str(text);
        // 旧面：`text_of`／`value()` 与升级前一致 ✓
        assert_eq!(instance.text_of(object), Some(text), "{text:?} 的 text_of");
        // SAFETY: 由 new_str 造出的 str ✓
        assert_eq!(unsafe { str_object(object) }.value(), text);
        // 新面：UTF-8 表示的 WTF-8 字节就是它的 UTF-8 字节 ✓
        assert_eq!(instance.wtf8_of(object), Some(text.as_bytes()));
        // SAFETY: 同上
        assert_eq!(unsafe { str_object(object) }.wtf8_bytes(), text.as_bytes());
        // SAFETY: 同上
        assert_eq!(unsafe { str_object(object) }.as_utf8(), Some(text));
    }
}

#[test]
fn empty_str_still_uses_the_om23_singleton() {
    let instance = Instance::new();
    let first = instance.new_str("");
    let second = instance.new_str("");
    assert_eq!(first.as_ptr(), second.as_ptr(), "空串是单例 ✓");
    // `new_str_wtf8(b"")` 走同一条单例路径 ✓
    let third = instance
        .new_str_wtf8(b"")
        .expect("空字节是合法 WTF-8");
    assert_eq!(third.as_ptr(), first.as_ptr(), "空 WTF-8 也走空串单例 ✓");
    assert_eq!(instance.text_of(third), Some(""));
    assert_eq!(instance.wtf8_of(third), Some(b"".as_slice()));
}

#[test]
fn valid_utf8_bytes_stay_in_the_utf8_representation() {
    let instance = Instance::new();
    let object = instance
        .new_str_wtf8("héllo 🦀".as_bytes())
        .expect("合法 UTF-8 必是合法 WTF-8");
    // **零行为改动**：合法 UTF-8 字节仍存 UTF-8 表示 ⇒ `value()` 视图可用 ✓
    assert_eq!(instance.text_of(object), Some("héllo 🦀"));
    // SAFETY: 由 new_str_wtf8 造出、且是 UTF-8 表示 ✓
    assert_eq!(unsafe { str_object(object) }.value(), "héllo 🦀");
    assert_eq!(instance.wtf8_of(object), Some("héllo 🦀".as_bytes()));
}

#[test]
fn lone_surrogate_bytes_land_in_the_wtf8_representation() {
    let instance = Instance::new();
    // WTF-8：a + U+D800（ED A0 80）+ b。**合法 WTF-8、非法 UTF-8** ⇒ WTF-8 表示 ✓
    let bytes: &[u8] = b"a\xED\xA0\x80b";
    let object = instance.new_str_wtf8(bytes).expect("合法 WTF-8 应能构造");
    // `text_of`（唯一允许的行为改动点）：WTF-8 表示 ⇒ 如实 `None` ✓
    assert_eq!(instance.text_of(object), None);
    // `wtf8_of`：总是给原始字节 ✓
    assert_eq!(instance.wtf8_of(object), Some(bytes));
    // SAFETY: 由 new_str_wtf8 造出的 str ✓
    let object_ref = unsafe { str_object(object) };
    assert_eq!(object_ref.wtf8_bytes(), bytes);
    assert_eq!(object_ref.as_utf8(), None, "WTF-8 表示没有 &str 视图");
    // 内部表示就是 WTF-8 分支（不直接引 `StrRepr`：它不在 crate 根的再导出清单里 ✓；
    // `as_utf8() == None` ＋ 原字节完整 ⇒ 已足以钉住"WTF-8 表示"这一事实 ✓）
    assert!(object_ref.as_utf8().is_none());
    assert!(!object_ref.is_empty());
}

#[test]
#[should_panic(expected = "StrObject::value() 不适用于 WTF-8 表示")]
fn value_guards_against_the_wtf8_representation() {
    let instance = Instance::new();
    let object = instance
        .new_str_wtf8(b"\xED\xA0\x80")
        .expect("孤立代理是合法 WTF-8");
    // SAFETY: 由 new_str_wtf8 造出的 str ✓ —— 这条**必须 panic**（WTF-8 没有 &str 视图 ✓）
    let _ = unsafe { str_object(object) }.value();
}

#[test]
fn other_wtf8_shapes_round_trip_through_the_storage() {
    let instance = Instance::new();
    let cases: [&[u8]; 5] = [
        // 孤立代理终点 U+DFFF
        b"\xED\xBF\xBF",
        // a + U+D800 + é + 🦀 + b
        b"a\xED\xA0\x80\xC3\xA9\xF0\x9F\xA6\x80b",
        // 只有 ASCII（合法 UTF-8 ⇒ 存 UTF-8 表示 ✓）
        b"plain",
        // 非 BMP 上限 U+10FFFF
        b"\xF4\x8F\xBF\xBF",
        // 连续两个不同代理
        b"\xED\xA0\x80\xED\xB0\x80",
    ];
    for bytes in cases {
        let object = instance.new_str_wtf8(bytes).expect("合法 WTF-8 应能构造");
        assert_eq!(instance.wtf8_of(object), Some(bytes), "{bytes:?} 的字节往返");
        // 合法 UTF-8 的几组：`text_of` 仍给 Some ✓；含代理的：`None` ✓
        let expected_text = std::str::from_utf8(bytes).ok();
        assert_eq!(instance.text_of(object), expected_text, "{bytes:?} 的 text_of");
    }
}

#[test]
fn invalid_wtf8_is_rejected_not_lossily_accepted() {
    let instance = Instance::new();
    let bad: [&[u8]; 6] = [
        b"\xED\xA0",               // 截断的 3 字节代理序列
        b"\xC0\x80",               // 过长编码（C0 首字节非法）
        b"\xF8\x88\x80\x80\x80",   // 5 字节形式（非法首字节）
        b"\xFF",                   // 非法首字节
        b"\xE1\x41\xA0",           // 坏续字节
        b"\xF4\x90\x80\x80",       // 超出 U+10FFFF
    ];
    for bytes in bad {
        assert!(
            instance.new_str_wtf8(bytes).is_none(),
            "{bytes:?} 不是合法 WTF-8 ⇒ 必须 None（不许有损收下）"
        );
    }
}

#[test]
fn non_str_objects_have_no_wtf8_bytes() {
    let instance = Instance::new();
    let integer = instance.new_int(1);
    assert_eq!(instance.wtf8_of(integer), None);
    assert_eq!(instance.text_of(integer), None);
}
