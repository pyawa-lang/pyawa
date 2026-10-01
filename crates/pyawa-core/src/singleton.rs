//! 单例表（`docs/SPEC-object-model.md` §8：**OM-23**／**OM-24**）。
//!
//! **OM-23**：`None`／`True`／`False`／小整数／空串等单例**必须**按实例创建，
//! **禁止**做成进程级全局——单例是"对象"，而对象只属于一个实例（`OM-1`）。
//!
//! 小整数区间取 CPython 3.14 的**实测**边界 `-5..=256`（`make(x) is make(x)` 的拐点，
//! 与 `BC-30` 同一种"向运行时取数"的纪律）。
//!
//! **尚未接线**：这些对象在 Python 层的类型名与协议（属性访问必须走 `OM-11` 的 `getattr` 槽）。

use core::ptr::NonNull;

use crate::header::Header;

/// **OM-23** 的小整数区间下界（含）——实测 CPython 3.14。
pub const SMALL_INT_MIN: i64 = -5;
/// **OM-23** 的小整数区间上界（含）——实测 CPython 3.14。
pub const SMALL_INT_MAX: i64 = 256;

/// **OM-23**：一个实例自己的单例表（载荷见 [`crate::builtin_objects`]）。
///
/// 表里的指针由**实例**持有（每项一份引用），随实例销毁一起释放（`OM-2`）。
pub struct Singletons {
    none_type: NonNull<crate::TypeObject>,
    bool_type: NonNull<crate::TypeObject>,
    int_type: NonNull<crate::TypeObject>,
    str_type: NonNull<crate::TypeObject>,
    none: NonNull<Header>,
    true_: NonNull<Header>,
    false_: NonNull<Header>,
    /// **OM-23** 的空串（唯一一份）。
    empty_str: NonNull<Header>,
    /// 下标 0 对应 `SMALL_INT_MIN`。
    small_ints: Vec<NonNull<Header>>,
}

impl Singletons {
    pub(crate) fn new(
        none_type: NonNull<crate::TypeObject>,
        bool_type: NonNull<crate::TypeObject>,
        int_type: NonNull<crate::TypeObject>,
        str_type: NonNull<crate::TypeObject>,
        empty_str: NonNull<Header>,
        none: NonNull<Header>,
        true_: NonNull<Header>,
        false_: NonNull<Header>,
        small_ints: Vec<NonNull<Header>>,
    ) -> Self {
        Self {
            none_type,
            bool_type,
            int_type,
            str_type,
            none,
            empty_str,
            true_,
            false_,
            small_ints,
        }
    }

    /// **OM-23** 的**空串**单例。
    pub fn empty_str(&self) -> NonNull<Header> {
        self.empty_str
    }

    /// `str` 的类型对象。
    pub fn str_type(&self) -> NonNull<crate::TypeObject> {
        self.str_type
    }

    /// `NoneType` 的类型对象。
    pub fn none_type(&self) -> NonNull<crate::TypeObject> {
        self.none_type
    }

    /// `bool` 的类型对象。
    pub fn bool_type(&self) -> NonNull<crate::TypeObject> {
        self.bool_type
    }

    /// `int` 的类型对象。
    ///
    /// *临时*：执行器现在按**类型身份**判定"这是不是小整数"（`OM-11` 的运算符槽位接线后
    /// 应改走协议）。
    pub fn int_type(&self) -> NonNull<crate::TypeObject> {
        self.int_type
    }

    /// `None` 的单例对象。
    pub fn none(&self) -> NonNull<Header> {
        self.none
    }

    /// `True`／`False` 的单例对象。
    pub fn boolean(&self, value: bool) -> NonNull<Header> {
        if value { self.true_ } else { self.false_ }
    }

    /// 落在小整数区间内时返回它的单例对象；区间外返回 `None`（那必须用对象表示，`OM-39`）。
    pub fn small_int(&self, value: i64) -> Option<NonNull<Header>> {
        if !(SMALL_INT_MIN..=SMALL_INT_MAX).contains(&value) {
            return None;
        }
        let index = (value - SMALL_INT_MIN) as usize;
        self.small_ints.get(index).copied()
    }

    /// 区间内的全部小整数单例（升序，供遍历与调试）。
    pub fn small_ints(&self) -> &[NonNull<Header>] {
        &self.small_ints
    }
}
