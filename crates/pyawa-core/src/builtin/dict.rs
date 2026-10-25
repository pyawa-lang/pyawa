//! **`dict` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `dict_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`bound_dict`、`container_receiver`、`object_init_native` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

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
    if instance
        .type_named("tuple")
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
pub(crate) fn dict_position(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
) -> Option<usize> {
    let entries = instance.dict_entries(mapping)?;
    entries
        .iter()
        .position(|(candidate, _)| crate::executor::values::values_equal_public(instance, *candidate, key))
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
    if let Some(other) = args.first() {
        if let Some(entries) = instance.dict_entries(*other) {
            pairs.extend(entries);
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
    pairs.extend(kwargs.iter().copied());
    for (key, value) in pairs {
        match dict_position(instance, mapping, key) {
            Some(index) => {
                // SAFETY: 上面刚确认是本实例的 dict。
                let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                instance.retain(value);
                if let Some(old) = object.set_value_at(index, value) {
                    // 旧值那份引用由本对象持有 ⇒ 归还引擎 ✓
                    unsafe { instance.release_object(old.as_ptr()) };
                }
            }
            None => {
                instance.retain(key);
                instance.retain(value);
                instance.dict_insert_raw(mapping, key, value);
            }
        }
    }
    Ok(instance.retain(instance.singletons().none()))
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
        None => instance.dict_insert_raw(mapping, key, value),
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
    let Some(items) = instance.iterable_items(source) else {
        let name = instance.type_name(instance.type_of(source));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{name}' object is not iterable"),
        ));
    };
    for (index, pair) in items.iter().enumerate() {
        let Some(parts) = instance.iterable_items(*pair) else {
            // 参照这条**不带类型名**（实测 `d.update([1])` ⇒ `object is not iterable` ✓）。
            // 失败前把本函数手上的引用（本项起余下的项）归还 ✓。
            for leftover in &items[index..] {
                // SAFETY: 这些引用由 `iterable_items` 给出。
                unsafe { instance.release_object(leftover.as_ptr()) };
            }
            return Err(instance.raise_builtin_error("TypeError", "object is not iterable"));
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
    if let Some(index) = dict_position(instance, mapping, *key) {
        let entries = instance.dict_entries(mapping).unwrap_or_default();
        return Ok(instance.retain(entries[index].1));
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
    if let Some(index) = dict_position(instance, mapping, *key) {
        // SAFETY: 上面刚确认是本实例的 dict。
        let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        if let Some((removed_key, value)) = object.remove_at(index) {
            unsafe { instance.release_object(removed_key.as_ptr()) };
            return Ok(value);
        }
    }
    match args.get(1) {
        Some(default) => Ok(instance.retain(*default)),
        None => Err(instance.raise_builtin_error("KeyError", "")),
    }
}
/// `copy()`（第 154 轮）：**浅拷贝** ✓（`dict_entries` 是**借用** ⇒ 每项 `retain` ✓ —— 第 145 轮的教训 ✓）。
pub(crate) fn dict_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
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
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    loop {
        // SAFETY: 绑定的是本实例的 dict。
        let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        match object.remove_at(0) {
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
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let length = instance.dict_entries(mapping).map(|e| e.len()).unwrap_or(0);
    if length == 0 {
        return Err(instance.raise_builtin_error("KeyError", "popitem(): dictionary is empty"));
    }
    // SAFETY: 绑定的是本实例的 dict。
    let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    match object.remove_at(length - 1) {
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
) -> Option<NonNull<Header>> {
    if let Some(name) = instance.text_of(key) {
        return instance.dict_get(mapping, name);
    }
    // **通用键型**（`None`／tuple／用户对象…）：按值相等找位置，再取该位置的值 ✓。
    // 返回的是字典**借用**的那一份 ✓（`dict_get_native` 出去前会自己 `retain` ✓）。
    let position = dict_position(instance, mapping, key)?;
    instance.dict_entries(mapping)?.get(position).map(|(_, value)| *value)
}
pub(crate) fn dict_get_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let Some(key) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "get expected at least 1 argument, got 0",
        ));
    };
    reject_unhashable_key(instance, *key)?;
    if let Some(found) = dict_lookup(instance, mapping, *key) {
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
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let entries = instance
        .dict_entries(mapping)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "not a dict"))?;
    let keys: Vec<NonNull<Header>> = entries
        .into_iter()
        // `entries()` 给的是**借用** ✓，而 `new_list` 会**接管** ⇒ 每项先还一份 ✓
        .map(|(key, _)| instance.retain(key))
        .collect();
    Ok(instance.new_list(keys))
}
pub(crate) fn dict_values_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let entries = instance
        .dict_entries(mapping)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "not a dict"))?;
    let values: Vec<NonNull<Header>> = entries
        .into_iter()
        .map(|(_, value)| instance.retain(value))
        .collect();
    Ok(instance.new_list(values))
}
pub(crate) fn dict_items_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let entries = instance
        .dict_entries(mapping)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "not a dict"))?;
    let pairs: Vec<NonNull<Header>> = entries
        .into_iter()
        .map(|(key, value)| instance.new_tuple(vec![instance.retain(key), instance.retain(value)]))
        .collect();
    Ok(instance.new_list(pairs))
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
    crate::executor::subscript::subscript_del(instance, container, *key, 0)?;
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
    let outcome = crate::executor::values::values_equal_public(instance, left, *right);
    Ok(instance.new_bool(outcome))
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
    let position = object.entries().iter().position(|(existing, _)| {
        crate::executor::values_equal_public(instance, *existing, *key)
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
        if let Some(entries) = instance.dict_entries(source) {
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
            let Some(items) = instance.iterable_items(source) else {
                let name = instance.type_name(instance.type_of(source));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("'{name}' object is not iterable"),
                ));
            };
            for pair in items {
                let Some(parts) = instance.iterable_items(pair) else {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "cannot convert dictionary update sequence element to a sequence",
                    ));
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
        instance.retain(*key);
        instance.retain(*value);
        dict_put_owned(instance, target, *key, *value);
    }
    Ok(instance.retain(instance.singletons().none()))
}
pub fn dict_fromkeys_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
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
    let keys: Vec<NonNull<Header>> = match source_ty.as_str() {
        "list" => unsafe { &*source.as_ptr().cast::<ListObject>() }.items().to_vec(),
        "tuple" => unsafe { &*source.as_ptr().cast::<TupleObject>() }.items().to_vec(),
        "set" | "frozenset" => unsafe { &*source.as_ptr().cast::<SetObject>() }.items().to_vec(),
        "dict" => unsafe { &*source.as_ptr().cast::<DictObject>() }
            .entries()
            .into_iter()
            .map(|(key, _)| key)
            .collect::<Vec<NonNull<Header>>>(),
        _ => {
            unsafe { instance.release_object(value.as_ptr()) };
            return Err(crate::ExecError::Unsupported {
                opcode: 0,
                what: "dict.fromkeys：这个可迭代对象的形态随后补",
            });
        }
    };
    let mapping = instance.new_dict();
    for key in keys {
        // SAFETY: key 由源容器持有，存活。
        unsafe { instance.incref_object(key.as_ptr()) };
        instance.dict_insert_raw(mapping, key, value);
    }
    // 每个键都接管了一份 value ✓ ⇒ 这里还掉最初那一份 ✓。
    unsafe { instance.release_object(value.as_ptr()) };
    Ok(mapping)
}

/// 若对象是**带 `keys()` 的映射** ⇒ 返回它的键列表（新引用，调用方负责释放）；否则 `None`。
fn mapping_keys(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<Option<Vec<NonNull<Header>>>, crate::ExecError> {
    let Ok(found) = crate::executor::attribute_lookup(instance, object, "keys") else {
        return Ok(None);
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
