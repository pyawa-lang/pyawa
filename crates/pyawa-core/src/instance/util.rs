//! `instance` 模块的小工具（从 `instance.rs` 搬来；**不是**纯移动：两个函数改为 `pub(super)`，
//! 因为父模块要用它们，而子模块的项默认对父模块不可见）。

use super::*;

pub(super) fn ruler_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| flag("PYAWA_RULER"))
}

/// 一个 `str` 对象的内容是否等于给定的 Rust 字符串（属性名比较用）。
pub(super) fn str_matches(instance: &Instance, raw: NonNull<Header>, expected: &str) -> bool {
    // SAFETY: 调用方保证 raw 是存活对象。
    if unsafe { raw.as_ref() }.ty() != instance.singletons().str_type() {
        return false;
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value() == expected
}
