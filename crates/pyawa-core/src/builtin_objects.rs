//! 内建类型的**载荷**（`docs/SPEC-type-system.md` 的 `TS-43`：布局由实现自选，不进 ABI）。
//!
//! **TS-41** 的表（`crate::builtin_types`）只记"类型存在、层次正确"；本文件才是它们的表示。
//! 只有 `OM-23` 点名的那几个才做单例（`None`／`True`／`False`／小整数／空串），
//! 其余类型"每次造一个新对象"——`is` 语义因此与参照实现一致（`OM-39`）。

use crate::py_object;

py_object! {
    /// `None` 的单例载体。*占位*：Python 层类型名与协议随后补。
    pub struct NoneObject {}
}

py_object! {
    /// `True`／`False` 的单例载体。
    pub struct BoolObject {
        /// 真假。
        value: bool,
    }
}

py_object! {
    /// 小整数的单例载体。
    pub struct IntObject {
        /// 数值；一定落在 `SMALL_INT_MIN..=SMALL_INT_MAX`。
        value: i64,
    }
}

py_object! {
    /// `object` 的实例。*占位*：`object()` 不携带状态。
    pub struct PlainObject {}
}

py_object! {
    /// `float` 的实例（C `double`，与参照实现一致）。
    pub struct FloatObject {
        /// 数值；`inf`／`nan` 照旧。
        value: f64,
    }
}

py_object! {
    /// `str` 的实例。*临时*：载荷是 Rust 字符串；字符层面的一致性随 `CM-13` 的 Unicode 数据补。
    pub struct StrObject {
        /// 内容（UTF-8）。
        value: String,
    }
}

impl FloatObject {
    /// 数值。
    pub fn value(&self) -> f64 {
        self.value
    }
}

impl StrObject {
    /// 内容。
    pub fn value(&self) -> &str {
        &self.value
    }

    /// 是否为空串（`OM-23` 的空串单例就是它）。
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}
