//! **`values` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `values_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`ITERATOR_TYPE_NAMES`、`builtin_type`、`exception_type`、`integer_payload`、`numeric_payload` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{ITERATOR_TYPE_NAMES, builtin_type, exception_type, integer_payload, numeric_payload};
use crate::builtin_objects::BytesObject;
use crate::builtin_objects::DictObject;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::ListObject;
use core::ptr::NonNull;
use crate::builtin_objects::SetObject;
use crate::builtin_objects::StrObject;
use crate::builtin_objects::TupleObject;
use crate::type_object::TypeObject;


/// **值相等**（*临时*：只管道 `None`／`bool`／`int`／`float`／`str`）。
///
/// 参照实现的 `==` 走 `__eq__` 槽位（随类型系统接线）；本层先按载荷比，
/// 但**必须**保留 `TS-40` 的可观察后果（`True == 1`、`1 == 1.0` 为真）——
/// 否则 `{1: 'a', True: 'b'}` 这类字面量会多出一个键，属于对拍里的新差异。
pub(crate) fn values_equal(instance: &Instance, left: NonNull<Header>, right: NonNull<Header>) -> bool {
    if left == right {
        return true;
    }
    let (left_int, right_int) = (
        integer_payload(instance, left),
        integer_payload(instance, right),
    );
    if let (Some(a), Some(b)) = (left_int, right_int) {
        // 走 `BigInt` 比：`IntValue` 的两种载荷（内联／大整数）数值相等就是相等
        return a.to_bigint() == b.to_bigint();
    }
    let (left_number, right_number) = (
        numeric_payload(instance, left),
        numeric_payload(instance, right),
    );
    if let (Some(a), Some(b)) = (left_number, right_number) {
        // *临时*：整数与浮点比时按 f64 走（超大整数与浮点混用时会有精度话题，随协议槽位收口）
        return a == b;
    }
    let str_type = instance.singletons().str_type();
    // SAFETY: 两个都是存活对象。
    let (left_type, right_type) = unsafe { (left.as_ref().ty(), right.as_ref().ty()) };
    if left_type == str_type && right_type == str_type {
        // SAFETY: 类型身份已确认。
        let (left_text, right_text) = unsafe {
            (
                &*left.as_ptr().cast::<StrObject>(),
                &*right.as_ptr().cast::<StrObject>(),
            )
        };
        return left_text.value() == right_text.value();
    }
    // `bytes` 按字节逐位比（`P1-12`；实测 `b'ab' == b'ab'` 为真、不同长度直接不等）
    if Some(left_type) == instance.type_named("bytes") && Some(right_type) == instance.type_named("bytes") {
        // SAFETY: 类型身份已确认。
        let (left_bytes, right_bytes) = unsafe {
            (
                &*left.as_ptr().cast::<BytesObject>(),
                &*right.as_ptr().cast::<BytesObject>(),
            )
        };
        return left_bytes.value() == right_bytes.value();
    }
    // **容器按值比**（实测 3.14.4）：`list` 与 `list`、`tuple` 与 `tuple` **递归逐项**比；
    // **不同种类**一律不等（`[1] == (1,)` ⇒ `False`）。`dict`／`set` 仍需 `OM-11` 的
    // `richcompare` 槽位（本层暂按身份），这条缺口另记。
    // SAFETY: 两个都是存活对象（调用方保证）。
    let (left_type, right_type) = unsafe { (left.as_ref().ty(), right.as_ref().ty()) };
    let list_type = instance.type_named("list");
    let tuple_type = instance.type_named("tuple");
    if Some(left_type) == list_type && Some(right_type) == list_type {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe { (&*left.as_ptr().cast::<ListObject>(), &*right.as_ptr().cast::<ListObject>()) };
        if a.len() != b.len() {
            return false;
        }
        return (0..a.len()).all(|index| match (a.item(index), b.item(index)) {
            (Some(x), Some(y)) => values_equal(instance, x, y),
            _ => false,
        });
    }
    if Some(left_type) == tuple_type && Some(right_type) == tuple_type {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe { (&*left.as_ptr().cast::<TupleObject>(), &*right.as_ptr().cast::<TupleObject>()) };
        if a.len() != b.len() {
            return false;
        }
        return (0..a.len()).all(|index| match (a.item(index), b.item(index)) {
            (Some(x), Some(y)) => values_equal(instance, x, y),
            _ => false,
        });
    }

    // **`dict` 按值比**（实测）：长度相等 ＋ 每个键在右边**按键值相等**找到、且对应值递归相等。
    let dict_type = instance.type_named("dict");
    if Some(left_type) == dict_type && Some(right_type) == dict_type {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<DictObject>(),
                &*right.as_ptr().cast::<DictObject>(),
            )
        };
        if a.len() != b.len() {
            return false;
        }
        let right_entries = b.entries();
        for (key, value) in a.entries() {
            let mut matched = false;
            for (other_key, other_value) in &right_entries {
                if values_equal(instance, key, *other_key) {
                    if !values_equal(instance, value, *other_value) {
                        return false;
                    }
                    matched = true;
                    break;
                }
            }
            if !matched {
                return false;
            }
        }
        return true;
    }
    // **`set`／`frozenset`**：同族（含跨 `set`／`frozenset`——Python 允许，`{1} == frozenset({1})`
    // 为真）时"长度相等 ＋ 左的每一项在右里找得到"（双向包含由长度 ＋ 单向包含推出）。
    let set_type = instance.type_named("set");
    let frozen_type = instance.type_named("frozenset");
    let left_is_set = Some(left_type) == set_type || Some(left_type) == frozen_type;
    let right_is_set = Some(right_type) == set_type || Some(right_type) == frozen_type;
    if left_is_set && right_is_set {
        // SAFETY: 类型身份已确认（两种集合在实现上是同一个载荷）。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<SetObject>(),
                &*right.as_ptr().cast::<SetObject>(),
            )
        };
        if a.len() != b.len() {
            return false;
        }
        return (0..a.len()).all(|index| match a.item(index) {
            Some(item) => (0..b.len()).any(|other| match b.item(other) {
                Some(candidate) => values_equal(instance, item, candidate),
                None => false,
            }),
            None => false,
        });
    }

    false
}

/// 一个对象是不是本层接线的迭代器。
pub(crate) fn is_iterator_type(instance: &Instance, ty: NonNull<TypeObject>) -> bool {
    ITERATOR_TYPE_NAMES
        .iter()
        .any(|name| instance.type_named(name) == Some(ty))
}

/// **相等性**（本层的临时口径，随 `richcompare` 槽位收口）：给宿主面用。
pub fn values_equal_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
) -> bool {
    values_equal(instance, left, right)
}

/// 这个类型是不是异常类（MRO 里有 `BaseException`）。
pub(crate) fn is_exception_type(instance: &Instance, ty: NonNull<TypeObject>) -> bool {
    instance.is_subtype(ty, exception_type(instance, "BaseException"))
}

/// 这个类型是不是"类型对象"（`type` 及其子类，`TS-41` 的层次说了算）。
pub(crate) fn is_type_object(instance: &Instance, ty: NonNull<TypeObject>) -> bool {
    instance.is_subtype(ty, builtin_type(instance, "type"))
}
