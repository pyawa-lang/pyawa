//! 值的内部表示（`docs/SPEC-object-model.md` §12：**OM-38**／**OM-39**）。
//!
//! **OM-38**：M1 用带标签的枚举（判别式 ＋ 载荷），**不建议** M1 就做 NaN-boxing；
//! 表示**不跨 ABI**（`OM-6`），`pyawa-abi` 那边只见不透明句柄。
//!
//! **OM-39**：内联**不得**改变 `is` 语义——所以只有**单例表覆盖到的值**才允许内联
//! （`None`／`True`／`False`／小整数）；区间外的整数必须用 [`Value::Object`] 表示，
//! 否则两个不同的整数会被内联成"同一个"，`is` 立刻失真。

use core::ptr::NonNull;

use crate::header::Header;
use crate::instance::Instance;
use crate::refcount::PyRef;
use crate::singleton::{SMALL_INT_MAX, SMALL_INT_MIN};

/// **OM-38**：带标签的值。
pub enum Value<'a> {
    /// `None`（单例表索引）。
    None,
    /// `True`／`False`（单例表索引）。
    Bool(bool),
    /// **只在** `SMALL_INT_MIN..=SMALL_INT_MAX` 内；用 [`Value::small_int`] 构造。
    Int(i64),
    /// 任意对象的一份**新引用**（`OM-16`）。
    Object(PyRef<'a>),
}

impl<'a> Value<'a> {
    /// 造一个内联小整数。`OM-39`：区间外**必须**用 [`Value::Object`]，这里会 `debug_assert`。
    pub fn small_int(value: i64) -> Self {
        debug_assert!(
            (SMALL_INT_MIN..=SMALL_INT_MAX).contains(&value),
            "OM-39：区间外的整数必须用对象表示，否则 is 语义会失真"
        );
        Value::Int(value)
    }

    /// 这个值落到哪个具体对象上（单例与对象都有；区间外的内联整数不可能存在）。
    pub fn as_header(&self, instance: &Instance) -> Option<NonNull<Header>> {
        match self {
            Value::None => Some(instance.singletons().none()),
            Value::Bool(value) => Some(instance.singletons().boolean(*value)),
            Value::Int(value) => instance.singletons().small_int(*value),
            Value::Object(reference) => Some(reference.as_ptr()),
        }
    }

    /// **OM-39**：`is` 的语义＝**单例表 ＋ 对象身份**（不是值相等）。
    ///
    /// - 表覆盖到的值（`None`／`True`／`False`／小整数）按它们在**本实例**里的那个对象比
    /// - 其余按对象指针比——同一份对象才 `is`，数值相等不算
    pub fn is_same(&self, other: &Value<'_>, instance: &Instance) -> bool {
        match (self.as_header(instance), other.as_header(instance)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }
}
