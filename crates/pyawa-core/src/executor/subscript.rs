//! **`subscript` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `subscript_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`builtin_type`、`exception_type`、`index_payload`、`is_type_object`、`new_exception_with_args`、`normalize_index`、`push`、`raise`、`raise_builtin`、`release`、`sequence_items`、`slice_bounds`、`slice_positions`、`values_equal` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{builtin_type, exception_type, index_payload, new_exception_with_args, normalize_index, raise, raise_builtin, release, sequence_items, slice_bounds, slice_positions, values_equal};
use crate::builtin_objects::BytesObject;
use crate::builtin_objects::DictObject;
use crate::executor::ExecError;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::ListObject;
use core::ptr::NonNull;
use crate::builtin_objects::StrObject;
use crate::builtin_objects::TupleObject;


/// 键是 `slice` 时的下标读：`bytes`／`list`／`tuple`／`str` 四族共用边界规则。
pub(crate) fn subscript_slice(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: container 是存活对象。
    let container_type = unsafe { container.as_ref() }.ty();
    if instance.is_subtype(container_type, builtin_type(instance, "bytes")) {
        let value = instance
            .bytes_value(container)
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        let (start, stop, step) = slice_bounds(instance, key, value.len())?;
        let picked: Vec<u8> = slice_positions(start, stop, step)
            .into_iter()
            .map(|position| value[position])
            .collect();
        return Ok(instance.new_bytes(&picked));
    }
    if instance.is_subtype(container_type, builtin_type(instance, "list")) {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        let (start, stop, step) = slice_bounds(instance, key, object.len())?;
        let mut items: Vec<NonNull<Header>> = Vec::new();
        for position in slice_positions(start, stop, step) {
            if let Some(item) = object.item(position) {
                // SAFETY: 值由列表持有，存活；新列表要自己那份。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        return Ok(instance.new_list(items));
    }
    if instance.is_subtype(container_type, builtin_type(instance, "tuple")) {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        let (start, stop, step) = slice_bounds(instance, key, object.len())?;
        let mut items: Vec<NonNull<Header>> = Vec::new();
        for position in slice_positions(start, stop, step) {
            if let Some(item) = object.item(position) {
                // SAFETY: 同上。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        return Ok(instance.new_tuple(items));
    }
    if container_type == instance.singletons().str_type() {
        // SAFETY: 同上。`str` 按**字符**切（不是字节）
        let text = unsafe { &*container.as_ptr().cast::<StrObject>() }.value().to_owned();
        let characters: Vec<char> = text.chars().collect();
        let (start, stop, step) = slice_bounds(instance, key, characters.len())?;
        let picked: String = slice_positions(start, stop, step)
            .into_iter()
            .map(|position| characters[position])
            .collect();
        return Ok(instance.new_str(&picked));
    }
    // **`__getitem__` 协议** ✓（第 626 轮）：参照里 `obj[i:j]` 一律把 **slice 对象**交给**类型上**的
    // `__getitem__` ✓（特殊方法走类型 ✓）⇒ 任何实现了 `__getitem__` 的类型／用户类都能切 ✓。
    // 先前只接内建四种 ✗ ⇒ `Lib/re/_parser.py` 的 `SubPattern`（`def __getitem__` ✓）一被切就报
    // "切片只接线了 bytes／list／tuple／str" ✗（本轮实测 ✓）。
    match crate::executor::attribute_lookup(instance, container, "__getitem__") {
        Ok(crate::executor::Attribute::Method { function, this }) => {
            // 实参按**按值**交出去 ⇒ 给 key 添一份新引用 ✓。
            // SAFETY: key 是帧值栈上的存活对象。
            unsafe { instance.incref_object(key.as_ptr()) };
            return crate::executor::call::call_callable(
                instance,
                function,
                Some(this),
                vec![key],
                Vec::new(),
                opcode,
            );
        }
        Ok(crate::executor::Attribute::Value(method)) | Ok(crate::executor::Attribute::Owned(method)) => {
            // SAFETY: 同上。
            unsafe { instance.incref_object(key.as_ptr()) };
            return crate::executor::call::call_callable(
                instance,
                method,
                Some(container),
                vec![key],
                Vec::new(),
                opcode,
            );
        }
        // `Attribute` 只有三个变体 ⇒ 上面两个 `Ok` 臂已覆盖全部 `Ok` ✓（写 `Ok(_)` 会触发
        // "unreachable pattern" 警告 ✗，而第 1 项闸门要求 0 警告 ✓）。
        Err(_) => {}
    }
    // **报错里带上类型名** ✓（第 626 轮 ✓）：没有它只能看到"尚未接线" ✗，定位要绕远路 ✓。
    Err(ExecError::Unsupported {
        opcode,
        what: Box::leak(
            format!(
                "切片只接线了 bytes／list／tuple／str／`__getitem__`；这里是 '{}'",
                instance.type_name(instance.type_of(container))
            )
            .into_boxed_str(),
        ),
    })
}

/// 下标**读**（`BINARY_OP` ＋ `NB_SUBSCR`，3.14 无 `BINARY_SUBSCR`）。返回**新引用**。
pub(crate) fn subscript_get(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: container 与 key 都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    // **切片**（`P1-12`）：键是 `slice` 时走切片路径（四个序列类型共用一套边界规则）
    if Some(unsafe { key.as_ref() }.ty()) == instance.type_named("slice") {
        return subscript_slice(instance, container, key, opcode);
    }

    if instance.is_subtype(container_type, builtin_type(instance, "tuple")) {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        let index = index_payload(instance, key, opcode)?;
        let position = match normalize_index(index, object.len()) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "tuple index out of range")),
        };
        let value = object.item(position).expect("已经检查过范围");
        // SAFETY: value 由容器持有，存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    if instance.is_subtype(container_type, builtin_type(instance, "list")) {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        let index = index_payload(instance, key, opcode)?;
        let position = match normalize_index(index, object.len()) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "list index out of range")),
        };
        let value = object.item(position).expect("已经检查过范围");
        // SAFETY: 同上。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    if instance.is_subtype(container_type, builtin_type(instance, "dict")) {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        // `KeyError` 的 `args` 就是那个键（实测：`KeyError('nope')`），不是一条消息
        let position = match object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key))
        {
            Some(position) => position,
            None => {
                // SAFETY: key 是帧值栈上的存活对象。
                unsafe { instance.incref_object(key.as_ptr()) };
                let exception = new_exception_with_args(
                    instance,
                    exception_type(instance, "KeyError"),
                    vec![key],
                );
                return Err(raise(instance, exception));
            }
        };
        let (_, value) = object.entry(position).expect("刚查到的位置");
        // SAFETY: 同上。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    // **类型下标** ✓（第 214 轮）：`list[int]` ✓ —— `Lib/types.py` 要 `type(list[int])` ✓（`GenericAlias` ✓）。
    if instance.is_type_object(container) {
        // **借用** ✓：`key` 是帧值栈上的存活对象 ✓，`new_generic_alias` 记账 ✓。
        return Ok(instance.new_generic_alias(container, key));
    }
    let str_type = instance.singletons().str_type();
    if container_type == str_type {
        // SAFETY: 同上。
        let text = unsafe { &*container.as_ptr().cast::<StrObject>() }.value().to_owned();
        let characters: Vec<char> = text.chars().collect();
        let index = index_payload(instance, key, opcode)?;
        let position = match normalize_index(index, characters.len()) {
            Some(position) => position,
            None => {
                return Err(raise_builtin(instance, "IndexError", "string index out of range"))
            }
        };
        let object = instance.alloc(StrObject::new(str_type, characters[position].to_string()));
        return Ok(object.into_raw().cast::<Header>());
    }
    // `bytes`：整数下标给**整数**（`b'abc'[0] == 97`，实测）；切片随 `slice` 类型（M3+）再接线
    if instance.is_subtype(container_type, builtin_type(instance, "bytes")) {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*container.as_ptr().cast::<BytesObject>() }.value().to_vec();
        let index = index_payload(instance, key, opcode)?;
        let position = match normalize_index(index, value.len()) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "index out of range")),
        };
        return Ok(instance.new_int(i64::from(value[position])));
    }
    // **协议回退**（第 514 轮真 bug 修 ✗）：类型自带 `__getitem__` ⇒ 调它 ✓
    // （`os.environ['X']` 的 `_Environ` 正是这条 ✓；与 `iter.rs` 的 `__contains__` 回退同一口径 ✓）。
    if let Some(found) = instance.type_lookup(container_type, "__getitem__") {
        instance.retain(key);
        return crate::executor::call::call_callable(
            instance,
            found,
            Some(container),
            vec![key],
            Vec::new(),
            opcode,
        );
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标只接线了 tuple／list／dict／str／bytes（含 `__getitem__` 协议回退）",
    })
}

/// 下标**写**（`STORE_SUBSCR`；`value` 是**新引用**，无论成败都会被接手）。
pub(crate) fn subscript_set(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    value: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // SAFETY: 三个都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    if instance.is_subtype(container_type, builtin_type(instance, "list")) {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };

        // **切片写**（`a[i:j] = …`）：键是 `slice` 时替换那一整段（长度可与原段不同；
        // 带步长的"扩展切片"要求长度相等——消息照参照实测）
        if Some(unsafe { key.as_ref() }.ty()) == instance.type_named("slice") {
            let bounds = slice_bounds(instance, key, object.len());
            let (start, stop, step) = match bounds {
                Ok(bounds) => bounds,
                Err(error) => {
                    release(instance, value);
                    return Err(error);
                }
            };
            let items = match sequence_items(instance, value, opcode) {
                Ok(items) => items,
                Err(error) => {
                    release(instance, value);
                    return Err(error);
                }
            };
            release(instance, value);
            if step == 1 {
                let count = (stop - start).max(0) as usize;
                for _ in 0..count {
                    if let Some(old) = object.remove(start as usize) {
                        release(instance, old);
                    }
                }
                for (offset, item) in items.into_iter().enumerate() {
                    object.insert(start as usize + offset, item);
                }
            } else {
                let positions = slice_positions(start, stop, step);
                if positions.len() != items.len() {
                    let (given, expected) = (items.len(), positions.len());
                    for item in items {
                        release(instance, item);
                    }
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        &format!(
                            "attempt to assign sequence of size {given} to extended slice of size {expected}"
                        ),
                    ));
                }
                for (position, item) in positions.into_iter().zip(items) {
                    if let Some(old) = object.replace(position, item) {
                        release(instance, old);
                    }
                }
            }
            return Ok(());
        }

        let index = match index_payload(instance, key, opcode) {
            Ok(index) => index,
            Err(error) => {
                release(instance, value);
                return Err(error);
            }
        };
        let length = object.len();
        let position = match normalize_index(index, length) {
            Some(position) => position,
            None => {
                release(instance, value);
                return Err(raise_builtin(instance, "IndexError", "list index out of range"));
            }
        };
        if let Some(old) = object.replace(position, value) {
            release(instance, old);
        }
        return Ok(());
    }
    if instance.is_subtype(container_type, builtin_type(instance, "dict")) {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        let position = object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key));
        match position {
            Some(slot) => {
                if let Some(old) = object.replace_value(slot, value) {
                    release(instance, old);
                }
                // 键已在表里：**调用方那份键的引用仍归调用方**（本函数借用键，见下）
            }
            None => {
                // **契约**：`subscript_set` **借用键**、**接管值**。
                // `insert_raw` 是"转移"语义（收下传进去的那份引用），所以这里必须先为字典
                // 新增一份键——否则调用方随后释放自己的键，字典里就留下一个**悬垂键指针**
                // （症状：键对象被释放后地址被别的字符串复用，查键会"命中"不相干的键）。
                // SAFETY: key 由调用方保证存活。
                unsafe { instance.incref_object(key.as_ptr()) };
                object.insert_raw(key, value);
            }
        }
        return Ok(());
    }
    release(instance, value);
    if instance.is_subtype(container_type, builtin_type(instance, "tuple")) {
        return Err(ExecError::Unsupported {
            opcode,
            what: "tuple 不支持下标赋值（不可变）",
        });
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标赋值只接线了 list／dict",
    })
}

/// 下标**删**（`DELETE_SUBSCR`）。
pub fn subscript_del(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // SAFETY: 两个都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    if instance.is_subtype(container_type, builtin_type(instance, "list")) {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        // **切片删除**（第 311 轮）：`del x[a:b]`／`del x[a:b:c]` —— 参照与**切片写**同一套边界口径 ✓
        //（`Lib/asyncio/base_events.py:173` 的 `del addrinfos_lists[0][:first - 1]` 正卡在这，
        // 那一族 **35** 个模块 ✓）。先前落到"下标必须是整数"那条 ✗。
        if Some(unsafe { key.as_ref() }.ty()) == instance.type_named("slice") {
            let (start, stop, step) = slice_bounds(instance, key, object.len())?;
            if step == 1 {
                let count = (stop - start).max(0) as usize;
                for _ in 0..count {
                    if let Some(removed) = object.remove(start as usize) {
                        release(instance, removed);
                    }
                }
            } else {
                // **带步长**：从后往前删（下标不会因删除而串位 ✓），长度按参照口径校验 ✓
                let mut positions: Vec<usize> = Vec::new();
                let mut at = start;
                while (step > 0 && at < stop) || (step < 0 && at > stop) {
                    positions.push(at as usize);
                    at += step;
                }
                positions.sort_unstable();
                positions.reverse();
                for position in positions {
                    if let Some(removed) = object.remove(position) {
                        release(instance, removed);
                    }
                }
            }
            return Ok(());
        }
        let index = index_payload(instance, key, opcode)?;
        let length = object.len();
        let position = match normalize_index(index, length) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "list index out of range")),
        };
        if let Some(removed) = object.remove(position) {
            release(instance, removed);
        }
        return Ok(());
    }
    if instance.is_subtype(container_type, builtin_type(instance, "dict")) {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        let position = match object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key))
        {
            Some(position) => position,
            None => {
                // SAFETY: key 是帧值栈上的存活对象。
                unsafe { instance.incref_object(key.as_ptr()) };
                let exception = new_exception_with_args(
                    instance,
                    exception_type(instance, "KeyError"),
                    vec![key],
                );
                return Err(raise(instance, exception));
            }
        };
        if let Some((removed_key, removed_value)) = object.remove(position) {
            release(instance, removed_key);
            release(instance, removed_value);
        }
        return Ok(());
    }
    // **内建类型不支持删除**（第 311 轮）：参照给
    // `TypeError: '<类型>' object does not support item deletion` ✓
    //（实测：`del "abc"[1:2]`／`del (1, 2)[0]` ✓）。用户类那条（`__delitem__`）随后补 ✓。
    for name in ["str", "tuple", "bytes", "int", "float", "bool", "NoneType", "frozenset"] {
        if Some(container_type) == instance.type_named(name) {
            let type_name = instance.type_name(container_type);
            return Err(raise_builtin(
                instance,
                "TypeError",
                &format!("'{type_name}' object does not support item deletion"),
            ));
        }
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标删除只接线了 list／dict",
    })
}

/// **`BC-39` 的 `NB_SUBSCR` 语义**（`pa_gettable` 用）：容器 ＋ 键 ⇒ **新引用**。
///
/// 实参是**借用视图**；异常经 [`ExecError::Raised`] 上抛。
pub fn subscript_read(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    subscript_get(instance, container, key, 0)
}

/// **`STORE_SUBSCR` 语义**（`pa_settable` 用）：容器 ＋ 键 ＋ 值（值为**借用**，写入时接管新引用）。
pub fn subscript_write(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    value: NonNull<Header>,
) -> Result<(), ExecError> {
    // 值要被容器接管 ⇒ 先为容器新增一份
    // SAFETY: 调用方保证 value 存活。
    unsafe { instance.incref_object(value.as_ptr()) };
    subscript_set(instance, container, key, value, 0)
}
