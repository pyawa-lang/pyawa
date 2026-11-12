//! **`dict` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `dict_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`bound_dict`、`container_receiver`、`object_init_native` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::HotId;
use crate::builtin_objects::{DictObject, ListObject, SetObject, TupleObject};
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::{bound_dict, container_receiver};

/// **键可哈希性**（task-46）：参照的 `dict` 用哈希表 ⇒ **不可哈希的键必须报 `TypeError`** ✓；
/// 本层 `dict` 是"值相等线性表"（不哈希）⇒ 不查就会**静默接受** ✗ ——
/// `test_decorators.test_memoize` 的 3≠4 正是这条：`saved[args]` 里 `args = ([10],)` 本该抛
/// `TypeError` 走 `except TypeError` 分支（计数 +1），却"命中缓存"⇒ 少算一次 ✗。
///
/// 返回 `(键的类型名, 真正不可哈希的那个类型名)` —— 文案照参照实测：
/// `cannot use 'tuple' as a dict key (unhashable type: 'list')` ✓（嵌套 tuple 报**最内层** ✗ 的那个）。
pub(crate) fn unhashable_key_type(
    instance: &Instance,
    key: NonNull<Header>,
) -> Option<(String, String)> {
    let key_type = instance.type_of(key);
    let key_name = instance.type_name(key_type);
    for name in ["list", "dict", "set", "bytearray"] {
        if instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(key_type, base))
        {
            return Some((key_name.clone(), key_name));
        }
    }
    // tuple 的哈希是递归的 ⇒ 元素不可哈希时整键也不可哈希 ✓（`([1],)` ⇒ 'list' ✗）。
    if instance.hot_type(HotId::Tuple)
        .is_some_and(|base| instance.is_subtype(key_type, base))
    {
        // SAFETY: tuple 子类沿用 tuple 布局（`AB-58` 族）。
        let items = unsafe { &*key.as_ptr().cast::<TupleObject>() };
        for index in 0..items.len() {
            if let Some(item) = items.item(index) {
                if let Some((_, bad)) = unhashable_key_type(instance, item) {
                    return Some((key_name, bad));
                }
            }
        }
        return None;
    }
    // 用户类 `__hash__ = None` ⇒ 不可哈希 ✓（参照实测：`cannot use 'A' as a dict key (unhashable type: 'A')`）。
    if instance
        .type_lookup(key_type, "__hash__")
        .is_some_and(|hash| hash == instance.singletons().none())
    {
        return Some((key_name.clone(), key_name));
    }
    None
}

/// 不可哈希就报参照文案的 `TypeError`（`dict` 的读／写／`get`／`setdefault`／`in` 共用 ✓）。
pub(crate) fn reject_unhashable_key(
    instance: &Instance,
    key: NonNull<Header>,
) -> Result<(), crate::ExecError> {
    if let Some((key_name, bad)) = unhashable_key_type(instance, key) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("cannot use '{key_name}' as a dict key (unhashable type: '{bad}')"),
        ));
    }
    Ok(())
}

/// **按值找键的下标** ✓（引擎统一比较口径 ✓）。
/// **带用户 `__eq__` 的键查找**（round 71，可失败）：`d[k] = v`／`MAP_ADD` 一族共用 ✓。
/// 与 [`dict_position`] 的差别只有"用户 `__eq__` 抛错要**原样上抛**" ✓（其余同口径 ✓）。
pub(crate) fn dict_position_with_protocol(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
) -> Result<(Option<usize>, Option<u64>), crate::ExecError> {
    // 返回算好的哈希一起给调用方 ✓（插入时复用 ⇒ 每键只算一次 `__hash__` ✓）。
    let Some(hash) = crate::executor::values::key_hash_checked(instance, key)? else {
        return Ok((dict_position(instance, mapping, key), None));
    };
    // SAFETY: 调用方保证 mapping 是存活的 dict。
    let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    object.ensure_index(|candidate| crate::executor::values::key_hash(instance, candidate));
    for position in object.candidates(hash) {
        let Some((candidate, _)) = object.entry(position) else {
            continue;
        };
        let same = if crate::executor::protocol::has_custom_eq(instance, candidate)
            || crate::executor::protocol::has_custom_eq(instance, key)
        {
            crate::executor::protocol::equality_public(instance, candidate, key)?
        } else {
            crate::executor::values::values_equal_public(instance, candidate, key)
        };
        if same {
            return Ok((Some(position), Some(hash)));
        }
    }
    Ok((None, Some(hash)))
}

pub(crate) fn dict_position(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
) -> Option<usize> {
    // **哈希索引 ＋ 用户 `__eq__`**（round 71）：先前**线性扫** ✗ ⇒ `dict(可迭代)`／
    // `dict.__init__`／`update`／下标写都 O(n²)（8000 项 2.6 s；`namedtuple._asdict()` 的
    // 15 万字段因此拖死 `test_collections.test_large_size` ✗）。可哈希走索引 ✓、不可哈希退回线性 ✓。
    let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    if let Some(hash) = crate::executor::values::key_hash(instance, key) {
        object.ensure_index(|candidate| crate::executor::values::key_hash(instance, candidate));
        for position in object.candidates(hash) {
            let Some((candidate, _)) = object.entry(position) else {
                continue;
            };
            let same = if crate::executor::protocol::has_custom_eq(instance, candidate)
                || crate::executor::protocol::has_custom_eq(instance, key)
            {
                crate::executor::protocol::equality_public(instance, candidate, key).unwrap_or(false)
            } else {
                crate::executor::values::values_equal_public(instance, candidate, key)
            };
            if same {
                return Some(position);
            }
        }
        return None;
    }
    // SAFETY: mapping 由调用方保证是本实例的 dict。
    let dict = unsafe { &*mapping.as_ptr().cast::<crate::builtin_objects::DictObject>() };
    dict.find_live_position(|candidate| {
        crate::executor::values::values_equal_public(instance, candidate, key)
    })
}
/// `update(other)`：逐对并入 ✓（**已有的键替换值** ✓，新键插入 ✓；引用规矩照 `insert_raw` ✓）。
pub(crate) fn dict_update_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    // **参照口径** ✓：`dict.update([other], **kwargs)` —— 两个都可有可无 ✓（`d.update()` 是空操作 ✓；
    // 先前"没有位置实参"直接报 `update expected at most 1 argument, got 0` ✗）。
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("update expected at most 1 argument, got {}", args.len()),
        ));
    }
    // **关键字实参也算** ✓（task-25 ① ✓）：`vars(self).update(name=name, value=value, group=group)` ✓
    //（`Lib/importlib/metadata/__init__.py:171` ✓）。
    let mut pairs: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::new();
    // **源 dict 迭代中改键 ⇒ RuntimeError**（round 119）：参照 `d.update(other)` 内部按迭代器
    // 搬 `other`，键的 `__eq__` 若把 `other` 改掉 ⇒ `RuntimeError: dictionary changed size
    // during iteration` ✓（`test_merge_and_mutate` ✓）。这里记一份版本号并逐对比对 ✓；
    // 快照里的键值先各 retain 一份（防源被清空后悬垂 ✓）。
    let mut source_guard: Option<(NonNull<Header>, u64)> = None;
    if let Some(other) = args.first() {
        if let Some(entries) = instance.dict_entries(*other) {
            // SAFETY: other 是存活对象。
            source_guard = Some((
                *other,
                unsafe { &*other.as_ptr().cast::<DictObject>() }.version(),
            ));
            for (key, value) in entries {
                // SAFETY: 键值由源 dict 持有，这里各添一份。
                unsafe {
                    instance.incref_object(key.as_ptr());
                    instance.incref_object(value.as_ptr());
                }
                pairs.push((key, value));
            }
        } else if let Some(keys) = mapping_keys(instance, *other)? {
            // **映射协议** ✓（第 411 轮，task-22 B5-② ✓）：参照的 `dict.update(m)` 接受任何
            // **有 `keys()` 的映射** ✓（真 `enum.py` 的 `__members__` 在参照里是 `mappingproxy` ✓——
            // 本层 `MappingProxyType` 恰好就是 `dict`（既有偏差 ✓）所以这条没被触发，但语义照参照补齐 ✓）。
            for key in keys {
                let value = crate::executor::subscript::subscript_read(instance, *other, key)?;
                pairs.push((key, value));
            }
        } else {
            // **③ 键值对可迭代**（task-25 ① ✓）：与 `dict |=` 走**同一处真相**
            //（[`dict_update_sequence`] ✓）。`multiprocessing/__init__.py:23` 的
            // `globals().update((name, getattr(...)) for name in __all__)` 正压在这 ✓。
            dict_update_sequence(instance, mapping, *other)?;
        }
    }
    // 关键字实参是**借用** ⇒ 各 retain 一份，与上面各分支统一成"本函数持有" ✓。
    for (key, value) in kwargs.iter().copied() {
        // **关键字名必须是 str** ✓（`d.update(**{1: 2})` 参照报 TypeError ✓）。
        if instance.text_of(key).is_none() {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "keywords must be strings",
            ));
        }
        // SAFETY: 关键字实参由调用方保证存活。
        unsafe {
            instance.incref_object(key.as_ptr());
            instance.incref_object(value.as_ptr());
        }
        pairs.push((key, value));
    }
    for (position, (key, value)) in pairs.iter().copied().enumerate() {
        if let Some((source, version)) = source_guard {
            // SAFETY: source 是存活对象（调用方借用）。
            let current = unsafe { &*source.as_ptr().cast::<DictObject>() }.version();
            if current != version {
                // 失败前把余下的键值引用归还 ✓。
                for (left_key, left_value) in &pairs[position..] {
                    // SAFETY: 这些引用由本函数持有。
                    unsafe {
                        instance.release_object(left_key.as_ptr());
                        instance.release_object(left_value.as_ptr());
                    }
                }
                return Err(instance.raise_builtin_error(
                    "RuntimeError",
                    "dictionary changed size during iteration",
                ));
            }
        }
        // **按键的 `==` 协议找位置**（round 120）：键 `__eq__` 抛异常要原样上抛 ✓
        //（`test_bad_key` 的 `d.update({x2: 2})` ✓）。
        match dict_position_with_protocol(instance, mapping, key)?.0 {
            Some(index) => {
                // SAFETY: 上面刚确认是本实例的 dict。
                let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                // 值那份引用**转交**字典 ✓；键已在字典里 ⇒ 手上这份 key 归还引擎 ✓。
                let replaced = object.set_value_at(index, value);
                // SAFETY: key 是 pairs 持有的那份；value 已转交。
                unsafe { instance.release_object(key.as_ptr()) };
                if let Some(old) = replaced {
                    // SAFETY: 旧值那份引用由本对象持有 ⇒ 归还引擎 ✓。
                    unsafe { instance.release_object(old.as_ptr()) };
                }
            }
            None => {
                // 键值两份引用都由本函数持有 ⇒ `insert_raw` 直接接管 ✓。
                instance.dict_insert_raw(mapping, key, value);
            }
        }
    }
    // **最后一对才触发变异**的形态也要报 ✓（`test_merge_and_mutate` 的 X 键在源末尾 ✓）：
    // 逐对检查只在"下一对开始"时看得到；末尾要补一次 ✓。
    if let Some((source, version)) = source_guard {
        // SAFETY: source 由调用方保证存活。
        let current = unsafe { &*source.as_ptr().cast::<DictObject>() }.version();
        if current != version {
            return Err(instance.raise_builtin_error(
                "RuntimeError",
                "dictionary changed size during iteration",
            ));
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}
/// **给异常额外挂一条 note**（PEP 678；round 122）：dict 更新序列的 TypeError 参照会带
/// `Cannot convert dictionary update sequence element #i to a sequence` ✓
/// （`test_dict.test_update_type_error` 的 `__notes__` ✓）。
fn with_update_note(
    instance: &Instance,
    error: crate::ExecError,
    index: usize,
) -> crate::ExecError {
    if let crate::ExecError::Raised { exception } = &error {
        let note = format!("Cannot convert dictionary update sequence element #{index} to a sequence");
        crate::builtin_objects::add_exception_note(instance, *exception, &note);
    }
    error
}

/// 往 `mapping` 写入一对**自有引用**（`key`／`value` 各一份由本函数接管 ✓）。
///
/// 已有键 ⇒ 只替换值（旧值归还引擎 ✓）、手上那份 `key` 也归还 ✓；新键 ⇒ `dict_insert_raw` 接管
/// 两份 ✓。引用账与 `executor::protocol` 的 `|=` 那一份**逐字同一口径** ✓。
fn dict_put_owned(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
    value: NonNull<Header>,
) {
    // SAFETY: 调用方保证 mapping 是本实例的 dict。
    let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    match dict_position(instance, mapping, key) {
        Some(index) => {
            if let Some(old) = object.set_value_at(index, value) {
                // SAFETY: 旧值那份引用由本对象持有 ⇒ 归还引擎 ✓。
                unsafe { instance.release_object(old.as_ptr()) };
            }
            // 键已在字典里（不替换键）⇒ 手上这份 key 归还引擎 ✓。
            // SAFETY: key 由调用方转交。
            unsafe { instance.release_object(key.as_ptr()) };
        }
        None => {
            // **增量维护哈希索引**（round 71）：`insert_raw` 会作废索引 ⇒ 下一次查找又 O(n) ✗。
            match crate::executor::values::key_hash(instance, key) {
                Some(hash) => {
                    object.insert_hashed(key, value, hash);
                }
                None => instance.dict_insert_raw(mapping, key, value),
            }
        }
    }
}

/// **`left | right`（`PEP 584`，task-13）**：**新建**一个 dict —— 先左后右，右侧**覆盖**同名键 ✓；
/// 两侧原对象都**不改** ✓（参照 3.14.4 实测：`{"a": 1} | {"b": 2}` ⇒ `{'a': 1, 'b': 2}` ✓，
/// `{"a": 1, "c": 3} | {"a": 2}` ⇒ `{'a': 2, 'c': 3}` ✓）。
///
/// 与 `dict.update`／`dict.__init__` 走**同一处真相**（[`dict_put_owned`] 的"已存在就换值" ✓）——
/// 不另写一套覆盖／引用账 ✓。两边都必须是 **dict 族**（子类也算 ✓；由调用方
/// [`crate::executor::arithmetic::arithmetic_public`] 用 `dict_entries` 判型 ✓）。
pub(crate) fn dict_merge(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
) -> NonNull<Header> {
    let merged = instance.new_dict();
    for source in [left, right] {
        for (key, value) in instance.dict_entries(source).unwrap_or_default() {
            // **借用 ⇒ 各留一份** ✓：结果字典持有自己的键、值引用 ✓。
            instance.retain(key);
            instance.retain(value);
            dict_put_owned(instance, merged, key, value);
        }
    }
    merged
}

/// **键值对可迭代 ⇒ 逐对并入**（task-25 ① ✓）：`dict.update` 与 `dict |=` 的**同一处真相** ✓
///（`executor/protocol.rs::inplace_dict_merge` 的 ③ 分支直接调本函数 ✓）。
///
/// 文案照参照实测（3.14.4）：
/// - 源不可迭代 ⇒ `TypeError: '<类型>' object is not iterable` ✓（`d.update(5)` ✓）；
/// - 元素不可迭代 ⇒ `TypeError: object is not iterable` ✓（**不带类型名**；`d.update([1])` ✓）；
/// - 元素长度不是 2 ⇒ `ValueError: dictionary update sequence element #i has length n; 2 is required` ✓；
/// 错误之前**已并入的保留** ✓（逐对生效，与参照同 ✓）。
pub(crate) fn dict_update_sequence(
    instance: &Instance,
    mapping: NonNull<Header>,
    source: NonNull<Header>,
) -> Result<(), crate::ExecError> {
    let Some(items) = instance.iterable_items(source)? else {
        let name = instance.type_name(instance.type_of(source));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{name}' object is not iterable"),
        ));
    };
    for (index, pair) in items.iter().enumerate() {
        let Some(parts) = instance.iterable_items(*pair)? else {
            // 参照这条**不带类型名**（实测 `d.update([1])` ⇒ `object is not iterable` ✓）。
            // 失败前把本函数手上的引用（本项起余下的项）归还 ✓。
            for leftover in &items[index..] {
                // SAFETY: 这些引用由 `iterable_items` 给出。
                unsafe { instance.release_object(leftover.as_ptr()) };
            }
            return Err(with_update_note(
                instance,
                instance.raise_builtin_error("TypeError", "object is not iterable"),
                index,
            ));
        };
        if parts.len() != 2 {
            for item in &parts {
                // SAFETY: 同上。
                unsafe { instance.release_object(item.as_ptr()) };
            }
            for leftover in &items[index..] {
                // SAFETY: 同上（本项 `pair` 也在其中 ✓）。
                unsafe { instance.release_object(leftover.as_ptr()) };
            }
            return Err(instance.raise_builtin_error(
                "ValueError",
                &format!(
                    "dictionary update sequence element #{index} has length {}; 2 is required",
                    parts.len()
                ),
            ));
        }
        // key／value 两份转交 ✓；本项集合那份（pair）随之归还 ✓。
        dict_put_owned(instance, mapping, parts[0], parts[1]);
        // SAFETY: pair 那份由 `iterable_items` 给出。
        unsafe { instance.release_object(pair.as_ptr()) };
    }
    Ok(())
}
/// `setdefault(key[, default])`：有就返回**已有值** ✓（借来的引用要 `retain` ✓），没有就插入并返回默认 ✓。
pub(crate) fn dict_setdefault_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let Some(key) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "setdefault expected at least 1 argument, got 0",
        ));
    };
    reject_unhashable_key(instance, *key)?;
    // **按键的 `==` 协议找位置**（round 120）：见 `pop` 同一口径 ✓。
    if let Some(position) = dict_position_with_protocol(instance, mapping, *key)?.0 {
        // **按"槽位"取值，不要拿 `dict_entries()` 快照按槽位索引**（round 203 真 bug 修 ✗）：
        // `dict_position_with_protocol` 给的是**表里的槽位**（含 tombstone 语义）；`dict_entries()`
        // 给的是**活条目快照**（下标与槽位不是一回事 ✗）⇒ 先前 `entries[position]` 在"槽位 == 快照长度"
        // 时直接 panic（`test_collections` 实测：`index out of bounds: len is 54 but index is 54` ✗）。
        // SAFETY: mapping 由调用方保证是 dict 且存活。
        let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let (_, value) = object.entry(position).expect("刚查到位置必然在表里");
        return Ok(instance.retain(value));
    }
    // **引用账**（第 297 轮修真 bug）：返回值那一份要**自己 retain** ✓ ——
    // 先前 `Some(value)` 那条直接返回借来的实参 ✗ ⇒ 调用方释放结果时把**字典里那一份**也放掉了 ✗
    // ⇒ 列表／字典值在仍在字典里时就被释放 ✓（实测：`d.setdefault("k", [])` 之后 `d["k"]`
    // 当场撞隔离区"对已释放对象 incref" ✓；`Lib/enum.py` 的
    // `classdict.setdefault('_ignore_', []).append('_ignore_')` 正是这一手 ✓）。
    let default = match args.get(1) {
        Some(value) => instance.retain(*value),
        None => instance.retain(instance.singletons().none()),
    };
    instance.retain(*key);
    instance.retain(default);
    instance.dict_insert_raw(mapping, *key, default);
    Ok(default)
}
/// `pop(key[, default])`：摘掉一项并返回它的**值** ✓（键那份引用**归还引擎** ✓）。
pub(crate) fn dict_pop_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let Some(key) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "pop expected at least 1 argument, got 0",
        ));
    };
    // **不可哈希的键 ⇒ TypeError**（round 120）：`test_unhashable_key` 的 `d.pop(key)` ✓；
    // 先前直接落到 `KeyError` ✗。
    reject_unhashable_key(instance, *key)?;
    // **按键的 `==` 协议找位置**（round 120）：键 `__eq__` 抛异常要原样上抛 ✓
    //（`test_bad_key` 的 `d.pop(x2)` ✓）。
    if let Some(index) = dict_position_with_protocol(instance, mapping, *key)?.0 {
        // SAFETY: 上面刚确认是本实例的 dict。
        let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        if let Some((removed_key, value)) = object.remove_at(index) {
            unsafe { instance.release_object(removed_key.as_ptr()) };
            return Ok(value);
        }
    }
    match args.get(1) {
        Some(default) => Ok(instance.retain(*default)),
        None => {
            // 参照：`KeyError` 的 `args` 就是那个键（不是空消息 ✓）。
            // SAFETY: key 是调用方保证存活的借用；异常构造添引用 ✓。
            unsafe { instance.incref_object(key.as_ptr()) };
            let exception = crate::executor::new_exception_with_args(
                instance,
                instance.hot_type(HotId::KeyError).expect("KeyError 已登记"),
                vec![*key],
            );
            Err(crate::executor::raise(instance, exception))
        }
    }
}
/// `copy()`（第 154 轮）：**浅拷贝** ✓（`dict_entries` 是**借用** ⇒ 每项 `retain` ✓ —— 第 145 轮的教训 ✓）。
pub(crate) fn dict_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if !args.is_empty() || !kwargs.is_empty() {
        // 参照：`d.copy(1)` ⇒ TypeError ✓（`test_copy` 的 assertRaises ✓）。
        return Err(instance.raise_builtin_error("TypeError", "copy() takes no arguments"));
    }
    let mapping = bound_dict(instance, bound)?;
    let copy = instance.new_dict();
    for (key, value) in instance.dict_entries(mapping).unwrap_or_default() {
        instance.retain(key);
        instance.retain(value);
        instance.dict_insert_raw(copy, key, value);
    }
    Ok(copy)
}
/// `clear()`（第 154 轮）：逐项摘掉并把两份引用**归还引擎** ✓。
pub(crate) fn dict_clear_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if !args.is_empty() || !kwargs.is_empty() {
        // 参照：`d.clear(None)` ⇒ TypeError ✓（`test_clear` 的 assertRaises ✓）。
        return Err(instance.raise_builtin_error("TypeError", "clear() takes no arguments"));
    }
    let mapping = bound_dict(instance, bound)?;
    loop {
        // SAFETY: 绑定的是本实例的 dict。
        let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        // **找第一个活条目**（round 156）：墓碑位上 `remove_at(0)` 会返回 `None` ⇒ 先前提前收手 ✗。
        let Some((position, _)) = object.next_live_entry(0) else {
            break;
        };
        match object.remove_at(position) {
            Some((key, value)) => unsafe {
                instance.release_object(key.as_ptr());
                instance.release_object(value.as_ptr());
            },
            None => break,
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}
/// `popitem()`（第 154 轮）：本层按**插入顺序**存 ✓ ⇒ 给**最后**一项 ✓（参照 3.7+ 也是"最后一项" ✓）。
pub(crate) fn dict_popitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if !args.is_empty() || !kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "popitem() takes no arguments"));
    }
    let mapping = bound_dict(instance, bound)?;
    let length = instance.dict_entries(mapping).map(|e| e.len()).unwrap_or(0);
    if length == 0 {
        return Err(instance.raise_builtin_error("KeyError", "popitem(): dictionary is empty"));
    }
    // SAFETY: 绑定的是本实例的 dict。
    let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    // **最后一个活条目**（round 156）：`length - 1` 可能落在墓碑上 ✗。
    let Some((position, _)) = object.last_live_entry() else {
        return Err(instance.raise_builtin_error("KeyError", "'popitem(): dictionary is empty'"));
    };
    match object.remove_at(position) {
        Some((key, value)) => Ok(instance.new_tuple(vec![key, value])),
        None => Err(instance.raise_builtin_error("KeyError", "popitem(): dictionary is empty")),
    }
}
/// **按键取值**（本层口径 ✓）：`str` 走名字通道 ✓；其余键型走**值相等的线性比较** ✓
/// （与 `dict_setitem`／`dict_put_owned` 的 [`dict_position`] 同一处口径 ✓）。
///
/// 第 104 轮起这里只认 `str`／`int` ✗ ⇒ `d.get(None, default)`／tuple 键等永远返回默认值 ✗。
/// task-5 补通用分支的动因 ✓：`argparse` 的 `_registry_get` 把 **`None` 当键** 存
/// （`register('action', None, _StoreAction)` ✓）再 `.get(None)` 取 ✓ ⇒ 官方 `unittest` 卡在
/// `TypeError: None is not callable` ✗（`Lib/argparse.py:1546-1570` 两处 `_registry_get`）。
pub(crate) fn dict_lookup(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
) -> Result<Option<NonNull<Header>>, crate::ExecError> {
    if let Some(name) = instance.text_of(key) {
        // **字符串快路**：只认"字典里存的也是 str" ✓；否则（存的键是用户对象／str 子类之外 ✓）
        // 落到下面的 `==` 语义查找 ✓（round 116：`test_str_nonstr` 的 `Key3` 键 ✓）。
        if let Some(found) = instance.dict_get(mapping, name) {
            return Ok(Some(found));
        }
    }
    // **通用键型**（`None`／tuple／用户对象…）：按 `==` 语义找位置，再取该位置的值 ✓
    //（`dict_lookup_key` 已含哈希索引 ＋ 不可哈希兜底 ✓）。
    // 返回的是字典**借用**的那一份 ✓（`dict_get_native` 出去前会自己 `retain` ✓）。
    // **按原始位置取条目**（round 156 真 bug 修 ✗）：`dict_entries` 是**过滤后**的列表（墓碑不在），
    // 它的下标不再是原始位置 ⇒ 必须用 `DictObject::entry(position)` ✓（`b.get(3)` 先前给 None ✓）。
    if let Some(position) = instance.dict_lookup_key_checked(mapping, key)? {
        // SAFETY: 调用方保证 mapping 是本实例的 dict。
        let dict = unsafe { &*mapping.as_ptr().cast::<crate::builtin_objects::DictObject>() };
        return Ok(dict.entry(position).map(|(_, value)| value));
    }
    // SAFETY: 同上。
    let dict = unsafe { &*mapping.as_ptr().cast::<crate::builtin_objects::DictObject>() };
    Ok(dict_position(instance, mapping, key).and_then(|position| dict.entry(position).map(|(_, value)| value)))
}
pub(crate) fn dict_get_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    if args.len() > 2 {
        // 参照：`d.get(None, None, None)` ⇒ TypeError ✓（`test_get` 的 assertRaises ✓）。
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("get expected at most 2 arguments, got {}", args.len()),
        ));
    }
    if !_kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "get() takes no keyword arguments"));
    }
    let Some(key) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "get expected at least 1 argument, got 0",
        ));
    };
    reject_unhashable_key(instance, *key)?;
    if let Some(found) = dict_lookup(instance, mapping, *key)? {
        // **借来的引用要还一份**（第 145 轮：`dict_get`／`dict_entries` 都是**借用** ✓；
        //   不 retain ⇒ 调用方释放后 double free ✗ —— 这一族在第 131 轮的 `type()` 上已经栽过 ✓）。
        return Ok(instance.retain(found));
    }
    match args.get(1) {
        Some(default) => Ok(instance.retain(*default)),
        None => Ok(instance.retain(instance.singletons().none())),
    }
}
pub(crate) fn dict_keys_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // 参照：`d.keys(None)` ⇒ TypeError ✓（`test_keys` 的 assertRaises ✓）。
    if !args.is_empty() || !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "keys() takes no arguments",
        ));
    }
    let mapping = bound_dict(instance, bound)?;
    // **真视图**（round 115）：先前交 `list` ✗ ⇒ `d.keys() | {...}` 等全部 TypeError ✗。
    crate::builtin::dict_views::make_view(instance, mapping, 0)
}
pub(crate) fn dict_values_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // 参照：`d.keys(None)` ⇒ TypeError ✓（`test_keys` 的 assertRaises ✓）。
    if !args.is_empty() || !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "values() takes no arguments",
        ));
    }
    let mapping = bound_dict(instance, bound)?;
    crate::builtin::dict_views::make_view(instance, mapping, 2)
}
pub(crate) fn dict_items_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // 参照：`d.keys(None)` ⇒ TypeError ✓（`test_keys` 的 assertRaises ✓）。
    if !args.is_empty() || !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "items() takes no arguments",
        ));
    }
    let mapping = bound_dict(instance, bound)?;
    crate::builtin::dict_views::make_view(instance, mapping, 1)
}
/// **`dict.__or__(self, other)`（PEP 584）**：两边都是 dict 族 ⇒ 新字典（右侧覆盖 ✓）；
/// 否则 `NotImplemented` ✓（`a.__or__(None)`／`()`／`"BAD"` ✓）。
pub(crate) fn dict_or_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(left), Some(right)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error("TypeError", "__or__ expected 2 arguments"));
    };
    let Some(dict_type) = instance.hot_type(HotId::Dict) else {
        return Ok(instance.retain(instance.not_implemented()));
    };
    if !instance.is_subtype(instance.type_of(*right), dict_type) {
        return Ok(instance.retain(instance.not_implemented()));
    }
    Ok(dict_merge(instance, left, *right))
}

/// **`dict.__ror__(self, other)`**：`other | d` —— 镜像 seq：右侧（self）覆盖 ✓。
pub(crate) fn dict_ror_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(right_dict), Some(left)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error("TypeError", "__ror__ expected 2 arguments"));
    };
    let Some(dict_type) = instance.hot_type(HotId::Dict) else {
        return Ok(instance.retain(instance.not_implemented()));
    };
    if !instance.is_subtype(instance.type_of(*left), dict_type) {
        return Ok(instance.retain(instance.not_implemented()));
    }
    Ok(dict_merge(instance, *left, right_dict))
}

/// `dict.__getitem__(self, key)`（`obj[key]` 的同一实现 ✓）。
pub fn dict_getitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(container), Some(key)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error("TypeError", "__getitem__ expected 2 arguments"));
    };
    crate::executor::subscript::subscript_read(instance, container, *key)
}
/// `dict.__delitem__(self, key)`（`del obj[key]` 的同一实现 ✓）。
pub fn dict_delitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(container), Some(key)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error("TypeError", "__delitem__ expected 2 arguments"));
    };
    crate::executor::subscript::subscript_del_inner(instance, container, *key, 0)?;
    Ok(instance.retain(instance.singletons().none()))
}
/// `dict.__eq__(self, other)`：与 `==` **同一处实现** ✓（`values_equal`）。
pub fn dict_eq_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(left), Some(right)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error("TypeError", "__eq__ expected 2 arguments"));
    };
    let Some(dict_type) = instance.hot_type(HotId::Dict) else {
        return Ok(instance.retain(instance.not_implemented()));
    };
    // **活视图代理先同步快照**（round 125）：`.mapping` 是源字典的活视图 ✓。
    // **只对 `mappingproxy` 操作数调**（round 195 真 bug 修 ✗）：先前无条件对两个操作数调 ⇒ 普通 dict 若
    // 复用了已释放代理的地址，会被"陈旧登记"当成活视图而**被写入老源的条目** ✗（`{} != {'foo':'bar'}`）。
    let proxy_type = instance.hot_type(HotId::MappingProxy);
    if proxy_type.is_some_and(|proxy| instance.type_of(left) == proxy) {
        instance.sync_mappingproxy(left);
    }
    if proxy_type.is_some_and(|proxy| instance.type_of(*right) == proxy) {
        instance.sync_mappingproxy(*right);
    }
    if !instance.is_subtype(instance.type_of(*right), dict_type) {
        // 非 dict ⇒ NotImplemented（反射走对方的 `__eq__` ✓，与参照一致 ✓）。
        return Ok(instance.retain(instance.not_implemented()));
    }
    // **键按 `==` 协议查、值递归**（round 120）：先前走 `values_equal_public` 的**纯结构**口径 ✗
    // ⇒ 键的 `__eq__` 抛异常被吞 ✗（`test_dict.test_eq` 的 `d1 == d2` ✓）。
    let a = instance.dict_entries(left).unwrap_or_default();
    let b = instance.dict_entries(*right).unwrap_or_default();
    if a.len() != b.len() {
        return Ok(instance.new_bool(false));
    }
    // **按"表槽位"取值，不要拿 `dict_entries()` 活条目快照按下标索引**（round 215 真 bug 修 ✗）：
    // `dict_lookup_key_checked` 给的是**表槽位**（含 tombstone 语义 ✓）；`dict_entries()` 是活条目
    // 压缩版 ⇒ 有删除的字典（`Counter` 里 `del` 过 ✓）槽位与下标错位 ⇒ 取到错项/None ⇒ `==` 假 ✗
    //（`test_collections.TestCounter.test_basics` 的 `c == dict(...)` 实测 ✓）。
    // SAFETY: right 是存活 dict（上面判过子类型）。
    let right_object = unsafe { &*right.as_ptr().cast::<crate::builtin_objects::DictObject>() };
    for (key, value) in a {
        let Some(position) = instance.dict_lookup_key_checked(*right, key)? else {
            return Ok(instance.new_bool(false));
        };
        let Some((_, other_value)) = right_object.entry(position) else {
            return Ok(instance.new_bool(false));
        };
        if !crate::executor::protocol::equality_public(instance, value, other_value)? {
            return Ok(instance.new_bool(false));
        }
    }
    Ok(instance.new_bool(true))
}
/// `dict.__setitem__(self, key, value)`（`obj[key] = v` 的同一实现 ✓）。
pub fn dict_setitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(container), Some(key), Some(value)) = (receiver, rest.first(), rest.get(1)) else {
        return Err(instance.raise_builtin_error("TypeError", "__setitem__ expected 3 arguments"));
    };
    reject_unhashable_key(instance, *key)?;
    // **内建实现直奔"原始写"** ✓（第 702 轮 ✗ 修）：`dict.__setitem__` 是**内建实现** ✓ ——
    // 覆盖版 `__setitem__` 结尾常写 `dict.__setitem__(self, k, v)` ✓（上游 `enum._EnumDict` 就是 ✓）
    // ⇒ 这里若再走协议 ✗（`subscript_write` 就是"incref ＋ 转协议"✗）⇒ **自递归** ✓（实测
    // `target/recon/nsdict.py` 报 `RecursionError` ✓）。契约照快路：**借用键、接管值** ✓
    // ⇒ 容器先拿一份值的引用 ✓（实参那份归调用方 ✓）；键只在**插入**那一支添引用 ✓。
    // SAFETY: container／key／value 都由调用方保证存活。
    let object = unsafe { &*container.as_ptr().cast::<crate::builtin_objects::DictObject>() };
    let position = object.find_live_position(|existing| {
        crate::executor::values_equal_public(instance, existing, *key)
    });
    unsafe { instance.incref_object(value.as_ptr()) };
    match position {
        Some(slot) => {
            if let Some(old) = object.replace_value(slot, *value) {
                unsafe { instance.release_object(old.as_ptr()) };
            }
        }
        None => {
            unsafe { instance.incref_object(key.as_ptr()) };
            instance.dict_insert_raw(container, *key, *value);
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}
/// **`dict.__init__`**（第 104 轮真实现；task-1 补**关键字实参**）：源可以是**映射**（dict 族 ✓），
/// 也可以是**成对的可迭代** ✓（`dict([("a", 1)])` ✓）；**关键字实参最后并入、同名键覆盖** ✓
/// （`dict({"a": 1}, a=2, b=3)` ⇒ `{"a": 2, "b": 3}` ✓）；无实参 ⇒ 什么都不做 ✓。
/// 返回 `None` ✓（`__init__` 的口径 ✓）。
///
/// 动因 ✓：`dict.__new__` 只管建**空映射** ✓（第 99 轮照参照改的 ✓）⇒ 填内容必须由 `__init__` 做 ✓，
/// 而 `dict` 先前**没有自己的 `__init__`** ✗（继承 `object` 的空操作 ✗）⇒ `D([("a", 1)])` 会**静默**给出
/// 空字典 ✗（比报错更糟 ✓，探针当场抓到 ✓）。
///
/// **关键字实参为什么必须有**（task-1）：官方 `Lib/argparse.py` 的
/// `dict(kwargs, dest=dest, option_strings=[])` 走**这一处** ✓ —— 先前 `_kwargs` 整份没读 ✗
/// ⇒ 建出来的字典缺 `dest` ⇒ 官方 `unittest.main()` 当场 `KeyError: 'dest'` ✗。
pub fn dict_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // **`self` 从哪来** ✓：本层的**实例化**会把实例**也放进 `args[0]`** ✓（`bound` 另有其一 ✓，
    // 见 `object_init_native` 的说明 ✓）⇒ 两条都要认 ✓；用户实参是 `self` 之后的那些 ✓。
    let (target, rest) = match bound {
        Some(this) => (this, args),
        None => match args.split_first() {
            Some((this, rest)) => (*this, rest),
            None => {
                return Err(instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
            }
        },
    };
    // **位置实参最多 1 个** ✓（参照 3.14.4 实测：`dict({}, {})` ⇒
    // `TypeError: dict expected at most 1 argument, got 2` ✓）。先判个数再碰源 ✓。
    if rest.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("dict expected at most 1 argument, got {}", rest.len()),
        ));
    }
    if let Some(source) = rest.first().copied() {
        // **映射协议**（`keys()`）：参照的 `dict(m)` 接受**任何有 `keys()` 的映射** ✓ ——
        // dict 子类（`OrderedDict`／`CustomReversedDict`，键**顺序**要沿用 ✓ round 123）与
        // `MutableMapping` 实现（`os.environ` 的 `_Environ.copy()` 就是 `dict(self)` ✓）都走这条
        // （`test_graphlib` 的 `os.environ.copy()` 先前落进"可迭代对"分支 ✗ ⇒
        // `ValueError: dictionary update sequence element does not have length 2` ✓）。
        // 精确 `dict` 仍走 entries 快路 ✓。
        let exact_dict = Some(instance.type_of(source)) == instance.hot_type(HotId::Dict);
        let mapping_source = if exact_dict {
            None
        } else {
            mapping_keys(instance, source)?
        };
        if let Some(keys) = mapping_source {
            for key in keys {
                let value = crate::executor::subscript::subscript_read(instance, source, key)?;
                instance.retain(key);
                dict_put_owned(instance, target, key, value);
            }
        } else if let Some(entries) = instance.dict_entries(source) {
            for (key, value) in entries {
                // SAFETY: 键值由源字典持有；目标要自己那两份 ✓。
                unsafe {
                    instance.incref_object(key.as_ptr());
                    instance.incref_object(value.as_ptr());
                }
                // **按"已存在就换值"写** ✓：`dict.__init__` 是 update 语义 ✓
                //（对**已有内容的字典**再 `__init__` 不该长出重复键 ✓）。
                dict_put_owned(instance, target, key, value);
            }
        } else {
            let Some(items) = instance.iterable_items(source)? else {
                let name = instance.type_name(instance.type_of(source));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("'{name}' object is not iterable"),
                ));
            };
            for (index, pair) in items.iter().enumerate() {
                let parts = match instance.iterable_items(*pair) {
                    Ok(Some(parts)) => parts,
                    Ok(None) => {
                        return Err(with_update_note(
                            instance,
                            instance.raise_builtin_error(
                                "TypeError",
                                "cannot convert dictionary update sequence element to a sequence",
                            ),
                            index,
                        ))
                    }
                    Err(error) => return Err(with_update_note(instance, error, index)),
                };
                if parts.len() != 2 {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "dictionary update sequence element does not have length 2",
                    ));
                }
                // SAFETY: 两个元素由 `pair` 持有；目标要自己那两份 ✓。
                unsafe {
                    instance.incref_object(parts[0].as_ptr());
                    instance.incref_object(parts[1].as_ptr());
                }
                // 同上 ✓：成对可迭代里的**重复键 ⇒ 后者覆盖** ✓（参照实测：
                // `dict([("a",1),("a",2)])` ⇒ `{"a": 2}` ✓）。
                dict_put_owned(instance, target, parts[0], parts[1]);
            }
        }
    }
    // **关键字实参最后并入** ✓（参照实测：`dict(mapping, **kw)` 里 kw **覆盖**同名键 ✓）。
    // 引用账照源字典那段 ✓：键、值各留一份再交给字典 ✓；`dict_put_owned` 在"键已存在"时
    // 归还多留的那份键并释放旧值 ✓。
    for (key, value) in kwargs {
        // **关键字名必须是 str** ✓（round 120）：`dict(**{1: 2})` 参照报 TypeError ✓
        //（`test_invalid_keyword_arguments` ✓）。
        if instance.text_of(*key).is_none() {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "keywords must be strings",
            ));
        }
        instance.retain(*key);
        instance.retain(*value);
        dict_put_owned(instance, target, *key, *value);
    }
    Ok(instance.retain(instance.singletons().none()))
}
pub fn dict_fromkeys_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(source) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "fromkeys expected at least 1 argument, got 0",
        ));
    };
    let value = match args.get(1) {
        Some(given) => {
            // SAFETY: given 是存活对象；下面交给字典时要多一份引用 ✓。
            unsafe { instance.incref_object(given.as_ptr()) };
            *given
        }
        None => instance.retain(instance.singletons().none()),
    };
    let source_ty = instance.type_name(instance.type_of(*source));
    // **键一律收成"本函数持有一份引用"的形态** ✓（下面 `dict_insert_raw` 接管一份 ✓）——
    // 容器分支从容器**借用** ⇒ 收集时 incref ✓；其它可迭代分支（`P3-41` round 41：通用迭代器协议 ✓）
    // 交出来的本来就是新引用 ✓。
    let keys: Vec<NonNull<Header>> = match source_ty.as_str() {
        "list" | "tuple" | "set" | "frozenset" | "dict" => {
            let borrowed: Vec<NonNull<Header>> = match source_ty.as_str() {
                "list" => unsafe { &*source.as_ptr().cast::<ListObject>() }.items().to_vec(),
                "tuple" => unsafe { &*source.as_ptr().cast::<TupleObject>() }.items().to_vec(),
                "set" | "frozenset" => {
                    unsafe { &*source.as_ptr().cast::<SetObject>() }.items().to_vec()
                }
                _ => unsafe { &*source.as_ptr().cast::<DictObject>() }
                    .entries()
                    .into_iter()
                    .map(|(key, _)| key)
                    .collect::<Vec<NonNull<Header>>>(),
            };
            for key in &borrowed {
                // SAFETY: key 由源容器持有，存活；这里新增一份交给本函数。
                unsafe { instance.incref_object(key.as_ptr()) };
            }
            borrowed
        }
        _ => {
            // **通用可迭代**（`dict.fromkeys(range(3))`、生成器、`itertools.count()` 一类 ✓）：
            // 走执行器**同一处**的迭代协议 ✓（`test_tempfile` 的 `_RandomNameSequence` 就卡在这 ✓）。
            let iterator = match instance.iter_object(*source) {
                Ok(iterator) => iterator,
                Err(error) => {
                    // SAFETY: value 的这份引用由本函数持有。
                    unsafe { instance.release_object(value.as_ptr()) };
                    return Err(error);
                }
            };
            let mut collected: Vec<NonNull<Header>> = Vec::new();
            loop {
                match instance.advance_iterator(iterator)? {
                    Some(item) => collected.push(item),
                    None => break,
                }
            }
            // SAFETY: iterator 的这份引用由本函数持有。
            unsafe { instance.release_object(iterator.as_ptr()) };
            collected
        }
    };
    // **类方法口径**（round 122）：`dictlike.fromkeys(...)` 要交 **dictlike 实例** ✓、
    // `mydict.fromkeys(...)`（覆写 `__new__` 返回 UserDict）要交 UserDict ✓ —— 先前一律
    // `instance.new_dict()` ✗（`test_fromkeys` 的 `assertIsInstance` ✓）。做法：按绑定类
    // **走正常实例化**（`call_value`：`cls(...)`，`__new__`／`__init__` 覆写都生效 ✓）。
    let class_header = match bound.and_then(|value| instance.as_type(value)) {
        Some(ty) => ty.cast::<Header>(),
        None => match instance.hot_type(HotId::Dict) {
            Some(ty) => ty.cast::<Header>(),
            None => return Err(instance.raise_builtin_error("RuntimeError", "dict 类型不可用")),
        },
    };
    let mapping = crate::executor::call::call_value(instance, class_header, &[], &[])?;
    for key in keys {
        // **重复键自然合并**（`d[k] = v` 语义 ✓）：`dict.fromkeys([1, 1])` ⇒ 一个键 ✓。
        // **走统一的属性写通道**（dict 与任意映射子类都适用：`UserDict.__setitem__` ✓）；
        // `subscript_write` 自己 retain 一份 value ✓。
        if let Err(error) = crate::executor::subscript::subscript_write(instance, mapping, key, value) {
            // SAFETY: key 这份引用由本函数持有。
            unsafe { instance.release_object(key.as_ptr()) };
            unsafe { instance.release_object(value.as_ptr()) };
            return Err(error);
        }
        // SAFETY: key 这份引用由本函数持有。
        unsafe { instance.release_object(key.as_ptr()) };
    }
    // 最初那份 value 归还 ✓。
    unsafe { instance.release_object(value.as_ptr()) };
    Ok(mapping)
}

/// 若对象是**带 `keys()` 的映射** ⇒ 返回它的键列表（新引用，调用方负责释放）；否则 `None`。
fn mapping_keys(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<Option<Vec<NonNull<Header>>>, crate::ExecError> {
    // **探测失败要还异常**（round 256 真泄漏修 ✗；round 77 同族）：`attribute_lookup` 失败时
    // `Err(Raised)` 持有一份异常引用（`pending_exception` 槽另有一份）—— 这里先前 `let Ok(...) else`
    // 直接丢掉 ✗ ⇒ 每次探测失败漏一颗 `AttributeError`（其回溯链再钉住整帧）。统一 `discard_error` ✓。
    let found = match crate::executor::attribute_lookup(instance, object, "keys") {
        Ok(found) => found,
        Err(error) => {
            crate::executor::runtime::discard_error(instance, &error);
            return Ok(None);
        }
    };
    let view = match found {
        crate::executor::Attribute::Method { function, this } => {
            crate::executor::call::call_callable(instance, function, Some(this), Vec::new(), Vec::new(), 0)?
        }
        crate::executor::Attribute::Value(method) | crate::executor::Attribute::Owned(method) => {
            crate::executor::call::call_callable(instance, method, Some(object), Vec::new(), Vec::new(), 0)?
        }
    };
    let iterator = match crate::executor::iter::iter_value(instance, view) {
        Ok(iterator) => iterator,
        Err(_) => {
            // 参照实测（3.14.4）：`{}.update(M())` 且 `M.keys()` 返回非可迭代 ⇒
            // `TypeError: M.keys() returned a non-iterable (type int)` ✓
            let object_name = instance.type_name(instance.type_of(object));
            let view_name = instance.type_name(instance.type_of(view));
            // SAFETY: view 是本函数刚拿到的新引用。
            unsafe { instance.release_object(view.as_ptr()) };
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("{object_name}.keys() returned a non-iterable (type {view_name})"),
            ));
        }
    };
    unsafe { instance.release_object(view.as_ptr()) };
    let mut keys = Vec::new();
    loop {
        match crate::executor::advance_iterator(instance, iterator, 0)? {
            Some(item) => keys.push(item),
            None => break,
        }
    }
    unsafe { instance.release_object(iterator.as_ptr()) };
    Ok(Some(keys))
}

/// **`d.__ior__(other)`**（round 118）：`|=` 的**属性面**（`test_merge_operator` 的
/// `a.__ior__(None)`／`a.__ior__(())`／`a.__ior__("BAD")` ✓）；原地并入并交回自身 ✓。
pub(crate) fn dict_ior_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let Some(mapping) = receiver else {
        return Err(instance.raise_builtin_error("TypeError", "__ior__ expected 2 arguments"));
    };
    let Some(other) = rest.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "__ior__ expected 2 arguments"));
    };
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "__ior__ takes no keyword arguments"));
    }
    // **统一走 `dict_update_native`**（round 109 真 bug 修 ✗）：它按"dict 条目 → 有 `keys()` 的映射 →
    // 可迭代键值对"三档处理 ✓。先前这里对"非 dict"直接落 `dict_update_sequence` ✗ ⇒ 对**有 `keys()`
    // 但没有 `__iter__`** 的映射（`Mapping` 只实现 `keys`／`__getitem__`）会走**旧式序列协议**
    // （`__getitem__(0), __getitem__(1), …` 永不 `IndexError`）⇒ **死循环** ✗
    // （`test_dict` 的 `d |= Mapping()`／inplace_shapes 的 `dict_inplace_or_agrees` 实测挂起 ✓）。
    let result = dict_update_native(instance, Some(mapping), &[other], kwargs)?;
    instance.release(result);
    Ok(instance.retain(mapping))
}

/// **类级访问的适配器**（round 118）：`dict.items(d)` 一类**未绑定**调用要能工作 ✓ ——
/// 参照的 dict 方法都是 `METH` 形态，可以从类型上取 ✓（`test_container_iterator` 的
/// `dict.items(container)` ✓）。适配器只把"未绑定形态的 args[0]"挪成接收者，逻辑全在原实现 ✓。
macro_rules! class_level_dict_method {
    ($name:ident, $inner:path) => {
        fn $name(
            instance: &Instance,
            bound: Option<NonNull<Header>>,
            args: &[NonNull<Header>],
            kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, crate::ExecError> {
            let (receiver, rest) = container_receiver(bound, args);
            $inner(instance, receiver, rest, kwargs)
        }
    };
}
class_level_dict_method!(class_get_native, dict_get_native);
class_level_dict_method!(class_keys_native, dict_keys_native);
class_level_dict_method!(class_values_native, dict_values_native);
class_level_dict_method!(class_items_native, dict_items_native);
class_level_dict_method!(class_clear_native, dict_clear_native);
class_level_dict_method!(class_copy_native, dict_copy_native);
class_level_dict_method!(class_pop_native, dict_pop_native);
class_level_dict_method!(class_popitem_native, dict_popitem_native);
class_level_dict_method!(class_setdefault_native, dict_setdefault_native);
class_level_dict_method!(class_update_native, dict_update_native);

/// 类级注册表（名字 → 适配器）：给 `instance/bootstrap.rs` 挂 dict 类型命名空间用 ✓。
pub(crate) const CLASS_LEVEL_METHODS: &[(&str, crate::NativeFn)] = &[
    ("get", class_get_native as crate::NativeFn),
    ("keys", class_keys_native as crate::NativeFn),
    ("values", class_values_native as crate::NativeFn),
    ("items", class_items_native as crate::NativeFn),
    ("clear", class_clear_native as crate::NativeFn),
    ("copy", class_copy_native as crate::NativeFn),
    ("pop", class_pop_native as crate::NativeFn),
    ("popitem", class_popitem_native as crate::NativeFn),
    ("setdefault", class_setdefault_native as crate::NativeFn),
    ("update", class_update_native as crate::NativeFn),
    ("__ior__", dict_ior_native as crate::NativeFn),
];
