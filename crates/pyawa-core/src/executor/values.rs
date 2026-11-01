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
use crate::builtin_objects::WeakRefObject;
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
        // **按哈希索引查**（`P3-41` round 42）：先前对每个键**线性扫右表** ⇒ `d == d2`
        // 是 O(n²)（`test_dict.test_copy_fuzz` 的 10 万项两个 O(n²) 之一 ✓）。
        let right_mapping = right;
        for (key, value) in a.entries() {
            let Some(position) = instance.dict_lookup_key(right_mapping, key) else {
                return false;
            };
            let Some((_, other_value)) = b.entry(position) else {
                return false;
            };
            if !values_equal(instance, value, other_value) {
                return false;
            }
        }
        return true;
    }
    // **`weakref.ref` 按"指称对象"比** ✓（`P3-41` round 40 修 ✗）：参照的 `ref.__eq__` 比的是
    // **目标**（`weakref.ref(a) == weakref.ref(b)` ⇔ `a == b`；同一目标的两个不同 `ref` 对象也相等 ✓）。
    // 先前落到最后那条 `false`（按身份）⇒ `Lib/_weakrefset.py` 的 `WeakSet` **不去重** ✓：
    // `weakref.WeakSet.add` 每次真的塞一条 ⇒ `_abc_cache` 一次 `isinstance` 长一条 ⇒ 成员查询
    // 线性扫、越查越慢 ⇒ **O(n²)**（实测 `test_userstring` 的 6.5 万次循环 >900 s ✗）。
    let weakref_type = instance.type_named("weakref");
    let is_weakref = |ty: NonNull<crate::TypeObject>| {
        weakref_type.is_some_and(|base| instance.is_subtype(ty, base))
    };
    if is_weakref(left_type) && is_weakref(right_type) {
        // SAFETY: 类型身份已确认（弱引用载荷是 `WeakRefObject`）。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<WeakRefObject>(),
                &*right.as_ptr().cast::<WeakRefObject>(),
            )
        };
        return values_equal(instance, a.target(), b.target());
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

/// **`str` 文本的哈希**（FNV-1a 64；与 [`key_hash`] 用同一个 ⇒ 调用方可直接按 `&str` 查字典 ✓）。
pub(crate) fn hash_text(text: &str) -> u64 {
    hash_bytes(text.as_bytes())
}

/// 字节序列的 FNV-1a 64。
pub(crate) fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// 整数键的哈希（`bytes` 哈希同族；`dict_set_int` 一类直接用它 ✓）。
pub(crate) fn hash_i64(value: i64) -> u64 {
    mix64(value as u64)
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// **字典键的哈希**（`P3-41` round 42）：与 [`values_equal`] **同源口径** ——
/// `values_equal(a, b) ⇒ key_hash(a) == key_hash(b)`（反过来不要求：桶内冲突由 `values_equal` 消解 ✓）。
///
/// 覆盖：`None`／数值（`bool`／`int`／`float` 同值同哈希，含 `True == 1` ✓）／`str`（含子类）／
/// `bytes`（含子类）／`tuple`（递归）／`weakref`（按**目标**哈希，与 round 40 的相等口径一致 ✓）；
/// `list`／`dict`／`set`／`bytearray` **不可哈希** ⇒ `None`（调用方退回线性扫；设置键时已由
/// `reject_unhashable_key` 拒掉 ✓）；其余按**身份**哈希（与 `values_equal` 的"其余按身份"一致 ✓）。
pub(crate) fn key_hash(instance: &Instance, object: NonNull<Header>) -> Option<u64> {
    let ty = instance.type_of(object);
    if ty == instance.singletons().none_type() {
        return Some(mix64(0x4e4f_4e45));
    }
    if let Some(number) = numeric_payload(instance, object) {
        // `NaN != NaN`（连自己也不等 ✓）⇒ 哈希按**身份**，保证"同一个 NaN 键"还能按身份查到 ✓。
        if number.is_nan() {
            return Some(mix64(0x4e41_4e00 ^ object.as_ptr() as usize as u64));
        }
        // `-0.0 == 0.0` ⇒ 同哈希 ✓。
        if number == 0.0 {
            return Some(0);
        }
        if number.fract() == 0.0 && number.abs() < 9.223_372_036_854_776e18 {
            return Some(hash_i64(number as i64));
        }
        return Some(mix64(number.to_bits()));
    }
    if let Some(text) = instance.text_of(object) {
        return Some(hash_text(text));
    }
    if let Some(bytes) = instance.bytes_value(object) {
        return Some(hash_bytes(bytes));
    }
    if let Some(items) = instance.tuple_items(object) {
        let mut hash = mix64(0x7475_706c_6500_0000);
        for item in items {
            let lane = key_hash(instance, item)?;
            hash = mix64(hash ^ lane);
        }
        return Some(hash);
    }
    if let Some(target) = crate::builtin_objects::weakref_target(instance, object) {
        return key_hash(instance, target);
    }
    // 不可哈希的容器（参照 `dict` 也拒它们当键）⇒ `None`（线性兜底；正常路径到不了 ✓）。
    let unhashable = ["list", "dict", "set", "bytearray"];
    if unhashable.iter().any(|name| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(ty, base))
    }) {
        return None;
    }
    // 其余按**身份**（与 `values_equal` 的"其余按身份"一致 ✓；用户类自定义 `__eq__` 的缺口另记 ✓）。
    Some(mix64(object.as_ptr() as usize as u64))
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
