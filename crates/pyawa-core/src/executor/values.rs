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
    // **`bytes`／`bytearray`（含子类）按字节内容相等** ✓（round 100）：参照 `bytearray(b"ab") ==
    // b"ab"` 为真 ✓、`bytearray(b"a") == bytearray(b"a")` 真 ✓ —— 先前只认精确 `bytes` ✗。
    if let (Some(left_bytes), Some(right_bytes)) = (
        instance.bytes_like_value(left),
        instance.bytes_like_value(right),
    ) {
        return left_bytes == right_bytes;
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
    // **`deque` 按值比**（round 86）：参照的 `deque.__eq__` 逐项比 ✓；同一对象已在函数开头
    // 用身份短路 ✓（`test_contains.test_nonreflexive` 的 `nan`／`NEVER_EQ` 依赖它 ✓）。
    if subtype_of(left_type, "deque") && subtype_of(right_type, "deque") {
        // SAFETY: 类型身份已确认，两侧载荷同为 `DequeObject` ✓。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<crate::builtin_objects::DequeObject>(),
                &*right.as_ptr().cast::<crate::builtin_objects::DequeObject>(),
            )
        };
        if a.len() != b.len() {
            return false;
        }
        let left_items = a.items();
        let right_items = b.items();
        return left_items
            .iter()
            .zip(right_items.iter())
            .all(|(left, right)| values_equal(instance, *left, *right));
    }
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
        // **真弱引用**（round 59）：两边都活着 ⇒ 比**目标**；任一边已死 ⇒ 只有"同一个弱引用对象"
        // 才相等（身份；上面 `left == right` 已先判过 ⇒ 这里一律不等 ✓，与参照一致 ✓）。
        return match (a.target(), b.target()) {
            (Some(left_target), Some(right_target)) => {
                values_equal(instance, left_target, right_target)
            }
            _ => false,
        };
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
    // **CPython 口径**（round 70）：与 `hash()`／用户 `__hash__` 必须**同值** ✓ —— 先前这里是
    // FNV ✗ ⇒ `{'halibut', HalibutProxy()}`（`HalibutProxy.__hash__` 返回 `hash('halibut')` ✓）
    // 两边桶不同 ⇒ 去重失败 ✗（`test_numeric_tower.test_hash_normalization` ✓）。
    if text.is_empty() {
        return 0;
    }
    let mut largest = 0u32;
    for character in text.chars() {
        largest = largest.max(character as u32);
    }
    let bytes: Vec<u8> = if largest <= 0xFF {
        text.chars().map(|character| character as u8).collect()
    } else if largest <= 0xFFFF {
        text.encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect()
    } else {
        text.chars()
            .flat_map(|character| (character as u32).to_le_bytes())
            .collect()
    };
    hash_bytes(&bytes)
}

/// 字节序列的哈希（SipHash-1-3 零密钥，与 `hash()` **同值** ✓；空串 ⇒ 0）。
pub(crate) fn hash_bytes(bytes: &[u8]) -> u64 {
    if bytes.is_empty() {
        return 0;
    }
    let mut value = siphash13_zero(bytes) as i64;
    if value == -1 {
        value = -2;
    }
    value as u64
}

/// `SipHash-1-3`（**零密钥**；CPython 的 `_Py_HashBytes` 同算法）。
fn siphash13_zero(data: &[u8]) -> u64 {
    fn round(state: &mut [u64; 4]) {
        state[0] = state[0].wrapping_add(state[1]);
        state[1] = state[1].rotate_left(13);
        state[1] ^= state[0];
        state[0] = state[0].rotate_left(32);
        state[2] = state[2].wrapping_add(state[3]);
        state[3] = state[3].rotate_left(16);
        state[3] ^= state[2];
        state[0] = state[0].wrapping_add(state[3]);
        state[3] = state[3].rotate_left(21);
        state[3] ^= state[0];
        state[2] = state[2].wrapping_add(state[1]);
        state[1] = state[1].rotate_left(17);
        state[1] ^= state[2];
        state[2] = state[2].rotate_left(32);
    }
    let mut state = [
        0x736f_6d65_7073_6575u64,
        0x646f_7261_6e64_6f6d,
        0x6c79_6765_6e65_7261,
        0x7465_6462_7974_6573,
    ];
    let mut chunks = data.chunks_exact(8);
    for chunk in &mut chunks {
        let block = u64::from_le_bytes(chunk.try_into().expect("8 字节块"));
        state[3] ^= block;
        round(&mut state);
        state[0] ^= block;
    }
    let remainder = chunks.remainder();
    let mut tail = ((data.len() as u64) & 0xFF) << 56;
    for (index, byte) in remainder.iter().enumerate() {
        tail |= u64::from(*byte) << (8 * index);
    }
    state[3] ^= tail;
    round(&mut state);
    state[0] ^= tail;
    state[2] ^= 0xFF;
    for _ in 0..3 {
        round(&mut state);
    }
    state[0] ^ state[1] ^ state[2] ^ state[3]
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
    // **弱引用**（round 59）：有缓存 ⇒ 用缓存（目标死后仍能删键 ✓）；活着 ⇒ 按目标算并缓存；
    // 已死且没缓存 ⇒ 按身份（它没进过任何字典 ⇒ 一致 ✓）。
    if let Some(state) = crate::builtin_objects::weakref_key_hash_state(instance, object) {
        if let Some(cached) = state {
            // 缓存的是**原始 u64**（不再二次 mix ✓ —— 否则"索引建立时按缓存算、查找时按目标算"
            // 两套口径不一致 ⇒ 临时弱引用键永远查不到 ✗）。
            return Some(cached);
        }
        match crate::builtin_objects::weakref_state(instance, object).flatten() {
            Some(target) => {
                let hash = key_hash(instance, target)?;
                crate::builtin_objects::weakref_store_key_hash(instance, object, hash);
                return Some(hash);
            }
            None => return Some(mix64(object.as_ptr() as usize as u64)),
        }
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
    // **用户 `__hash__`**（round 70）：参照的容器哈希就是 `hash(对象)` ✓ —— 先前"其余按身份" ✗
    // ⇒ `{'halibut', HalibutProxy()}`（`HalibutProxy.__hash__` 返回 `hash('halibut')` ✓）里两项
    // 桶不同 ⇒ 去重失败 ✗（`test_numeric_tower.test_hash_normalization` 实测 2 != 1 ✓）。
    // 口径：类型字典里**自己**有 `__hash__`（不是 `object` 的那份）⇒ 调它，取整数结果；
    // 返回 `None` 表示不可哈希 ⇒ `None` ✓。结果用**原始位模式**（不 mix）—— 与本函数对
    // `str`／`int` 的口径必须**一致**：`hash('halibut')` 走本文件的 `hash_text` ⇒ 同一条路径 ✓。
    let object_hash = instance
        .type_named("object")
        .and_then(|object_type| instance.type_lookup(object_type, "__hash__"));
    let own_hash = instance.type_lookup(ty, "__hash__");
    if let Some(hasher) = own_hash.filter(|found| Some(*found) != object_hash) {
        // SAFETY: ty 由注册表持有。
        let ty_object = unsafe { ty.as_ref() };
        if ty_object.name() == "type" {
            // 类型对象的 `__hash__` 是**元类**的 —— 交给身份兜底（与既有行为一致 ✓）。
        } else {
            // SAFETY: hasher 由类型字典持有。
            unsafe { instance.incref_object(hasher.as_ptr()) };
            let outcome = crate::executor::call::call_callable(
                instance,
                hasher,
                Some(object),
                Vec::new(),
                Vec::new(),
                0,
            );
            // SAFETY: 上面新增的那份引用由本函数归还。
            unsafe { instance.release_object(hasher.as_ptr()) };
            let returned = outcome.ok()?;
            let hash = instance
                .int_of(returned)
                .and_then(|value| value.to_i64());
            // SAFETY: 返回值是新引用，这里消费。
            unsafe { instance.release_object(returned.as_ptr()) };
            if let Some(hash) = hash {
                return Some(hash as u64);
            }
            return None;
        }
    }
    // 其余按**身份**（与 `values_equal` 的"其余按身份"一致 ✓）。
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
