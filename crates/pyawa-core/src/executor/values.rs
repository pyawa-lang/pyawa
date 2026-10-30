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
    // **复数相等**（task-30）：任一边是 `complex` ⇒ 两边升到复数塔比 ✓（实测
    // `(1+0j) == 1`／`== 1.0`／`== True` 都真 ✓，`(1+2j) == 1` 假 ✓；与 `str` 等比较一律假 ✓）。
    // 注意放在 `numeric_payload` 之前（它不认复数 ⇒ 否则会落到后面的"身份不等" ✗）。
    let complex_side = |value: NonNull<Header>| instance.complex_value(value);
    let left_complex = complex_side(left);
    let right_complex = complex_side(right);
    if left_complex.is_some() || right_complex.is_some() {
        let convert = |parts: Option<(f64, f64)>, value: NonNull<Header>| match parts {
            Some(parts) => Some(parts),
            None => {
                if let Some(number) = instance.float_value(value) {
                    Some((number, 0.0))
                } else {
                    instance
                        .int_of(value)
                        .map(|integer| (integer.to_bigint().to_f64(), 0.0))
                }
            }
        };
        return match (convert(left_complex, left), convert(right_complex, right)) {
            (Some(a), Some(b)) => a.0 == b.0 && a.1 == b.1,
            _ => false,
        };
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
    // **`str` 的子类也按值比** ✓（第 410 轮，task-21 G2 ✓）：先前要求**恰好**是 `str` 类型 ✗
    // ⇒ `class S(str)` 的实例 `S("abc") == "abc"` 实测 **False** ✗（参照 True ✓）—— 真 `enum.py`
    // 的 `StrEnum` 成员是 `str` 子类实例 ⇒ 与普通字符串比较全错 ✗（enum 换真货必需 ✓）。
    let subtype_of_str = |ty: NonNull<crate::TypeObject>| {
        ty == str_type || instance.is_subtype(ty, str_type)
    };
    if subtype_of_str(left_type) && subtype_of_str(right_type) {
        // P3-38 S2a-2：按 **WTF-8 字节**相等判 ✓（合法 UTF-8 与旧口径一致；含孤立代理的串也能
        // 正确比较 ✓）。辅助 API 见 `Instance::str_equals`。
        return instance.str_equals(left, right).unwrap_or(false);
    }
    // `bytes` 按字节逐位比（`P1-12`；实测 `b'ab' == b'ab'` 为真、不同长度直接不等）
    let subtype_of_bytes = |ty: NonNull<crate::TypeObject>| {
        instance
            .type_named("bytes")
            .is_some_and(|base| instance.is_subtype(ty, base))
    };
    if subtype_of_bytes(left_type) && subtype_of_bytes(right_type) {
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
    // **子类也按值比** ✓（task-14 真 bug 修 ✗）：先前按**恰好**是 list／tuple／dict 判 ✗
    // ⇒ `namedtuple` 实例 `Point(1,2) == (1,2)` 实测 **False** ✗（参照 True ✓）。
    // 参照口径 ✓：tuple（含子类）与 tuple（含子类）逐项比 ✓；list 与 tuple **不同族**不等 ✓。
    let subtype_of = |ty: NonNull<crate::TypeObject>, name: &str| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(ty, base))
    };
    let is_list = |ty: NonNull<crate::TypeObject>| subtype_of(ty, "list");
    let is_tuple = |ty: NonNull<crate::TypeObject>| subtype_of(ty, "tuple");
    if is_list(left_type) && is_list(right_type) {
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
    if is_tuple(left_type) && is_tuple(right_type) {
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
    if subtype_of(left_type, "dict") && subtype_of(right_type, "dict") {
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
