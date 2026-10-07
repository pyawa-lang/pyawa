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
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let Some(other) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "update expected at most 1 argument, got 0",
        ));
    };
    let pairs = instance
        .dict_entries(*other)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "update() 目前只接字典"))?;
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
/// **按键取值**（本层口径 ✓）：`str` 走名字通道 ✓；`int` 走线性比较 ✓（其余键型随后补 ✗）。
pub(crate) fn dict_lookup(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
) -> Option<NonNull<Header>> {
    if let Some(name) = instance.text_of(key) {
        return instance.dict_get(mapping, name);
    }
    if let Some(wanted) = instance.int_value(key) {
        for (candidate, value) in instance.dict_entries(mapping)? {
            if instance.int_value(candidate) == Some(wanted) {
                return Some(value);
            }
        }
    }
    None
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
/// **`dict.__init__`**（第 104 轮真实现）：源可以是**映射**（dict 族 ✓），也可以是**成对的可迭代** ✓
/// （`dict([("a", 1)])` ✓）；无实参 ⇒ 什么都不做 ✓。返回 `None` ✓（`__init__` 的口径 ✓）。
///
/// 动因 ✓：`dict.__new__` 只管建**空映射** ✓（第 99 轮照参照改的 ✓）⇒ 填内容必须由 `__init__` 做 ✓，
/// 而 `dict` 先前**没有自己的 `__init__`** ✗（继承 `object` 的空操作 ✗）⇒ `D([("a", 1)])` 会**静默**给出
/// 空字典 ✗（比报错更糟 ✓，探针当场抓到 ✓）。
pub fn dict_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
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
    let Some(source) = rest.first().copied() else {
        return Ok(instance.retain(instance.singletons().none()));
    };
    // SAFETY: 目标由调用方保证存活；`dict` 族的载荷就是 `DictObject` ✓。
    let object = unsafe { &*target.as_ptr().cast::<DictObject>() };
    if let Some(entries) = instance.dict_entries(source) {
        for (key, value) in entries {
            // SAFETY: 键值由源字典持有；目标要自己那两份 ✓。
            unsafe {
                instance.incref_object(key.as_ptr());
                instance.incref_object(value.as_ptr());
            }
            object.insert_raw(key, value);
        }
        return Ok(instance.retain(instance.singletons().none()));
    }
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
        object.insert_raw(parts[0], parts[1]);
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
