//! **`protocol` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `protocol_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`line_at_offset`、`render_label`、`set_items_of`、`type_name_of` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{line_at_offset, render_label, set_items_of, type_name_of};
use crate::executor::Attribute;
use crate::builtin_objects::AttributeObject;
use crate::code::CodeObject;
use crate::builtin_objects::DictObject;
use crate::executor::ExecError;
use crate::frame::Frame;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::ListObject;
use core::ptr::NonNull;
use core::cell::RefCell;
use crate::builtin_objects::StrObject;
use crate::builtin_objects::TupleObject;
use crate::type_object::TypeObject;
use crate::executor::arithmetic_public;
use crate::executor::attribute_lookup;
use crate::executor::builtin_type;
use crate::executor::call_callable;
use crate::executor::call_object_method;
use crate::executor::concat_public;
use crate::executor::contains;
use crate::executor::is_type_object;
use crate::executor::opcode_of;
use crate::executor::push;
use crate::executor::raise_builtin;
use crate::executor::release;
use crate::executor::sequence_items;
use crate::executor::str_matches_public;
use crate::executor::values_equal_public;


/// 把一个整数压栈（**任意精度**，`TS-45`）：单例表覆盖到的走单例（`OM-23`），
/// 其余交给 `Instance::new_int` 分配。
///
/// 早先这里要求"必须是单例"，于是 `x = 200 + 100`（结果 300 不在单例表里）与
/// `x = 9223372036854775807`（字面量）都会报 `IntOutOfRange`——那是 `P1-11` 之前的
/// i64／单例假设残留，2026-10-02 由新加的 `big_int_add` 对拍语料**抓出来**的。
pub(crate) fn push_int(instance: &Instance, frame: &Frame, value: i64) -> Result<(), ExecError> {
    let raw = instance.new_int(value);
    push(instance, frame, raw)
}

/// 造一个切片对象：**一律经 `slice` 类型的构造槽**（`OM-11` 的 `new` 槽）——
/// 字段校验、`None` 的含义、失败消息全都跟着 `slice(...)` 那条路走（**一处真相**）。
pub(crate) fn build_slice(
    instance: &Instance,
    arguments: &[NonNull<Header>],
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    let slice_type = builtin_type(instance, "slice");
    let callable = instance.type_value(slice_type);
    match crate::executor::call_value(instance, callable, arguments, &[]) {
        Ok(slice) => Ok(slice),
        Err(ExecError::Unsupported { .. }) => Err(ExecError::Unsupported {
            opcode,
            what: "BUILD_SLICE／BINARY_SLICE 的实参形态还没接线",
        }),
        Err(error) => Err(error),
    }
}

/// 在**映射**（`dict`）里按名字查一项（**新引用**交给调用方；没查到给 `None`）。
pub(crate) fn lookup_in_mapping(
    instance: &Instance,
    mapping: NonNull<Header>,
    name: &str,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 mapping 是本实例里存活的 dict。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .iter()
        .position(|(existing, _)| str_matches_public(instance, *existing, name))?;
    let (_, value) = dict.entry(position)?;
    // SAFETY: value 由字典持有，存活；调用方要自己那份。
    unsafe { instance.incref_object(value.as_ptr()) };
    Some(value)
}

/// 对象的属性字典（只有带 [`crate::HAS_INSTANCE_DICT`] 的实例才有）。
pub(crate) fn instance_attributes(instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
    // SAFETY: object 是存活对象。
    let header = unsafe { object.as_ref() };
    let ty = header.ty();
    // SAFETY: ty 由注册表持有。
    let type_object = unsafe { ty.as_ref() };
    if type_object.type_flags() & crate::HAS_INSTANCE_DICT == 0 {
        return None;
    }
    // **类型对象**：它的"实例字典"就是它的**命名空间** ✓（`TypeObject.dict` ✓，**一处真相** ✓）。
    //
    // **第 201 轮真 bug 的落点** ✗：元类型（`type`）曾被错置"内联实例字典"位 ✗ ⇒ 于是把
    // `TypeObject` 当 `AttributeObject` 读 ✗ ⇒ 取出来的字典指针是**垃圾**（实测 `0x6` ✓）⇒
    // `Lib/os.py` 一类**一取类属性就段错误** ✗（且随堆布局时隐时现 ✓）。
    if instance.is_type_object(object) {
        // SAFETY: 刚判过它是类型对象 ⇒ 头部就在同一地址上。
        return unsafe { &*object.as_ptr().cast::<crate::TypeObject>() }.dict();
    }
    if type_object.has_inline_instance_dict() {
        // SAFETY: 这一位保证载荷就是 AttributeObject。
        return unsafe { &*object.as_ptr().cast::<AttributeObject>() }.attributes();
    }
    // **OM-14**：固定布局的实例（宿主／子类）把字典另行挂在头部那一格上
    header.instance_dict()
}

/// **取或惰性创建**实例字典（`OM-14`：参照实现里 `obj.__dict__` 一读就给出 `{}`）。
///
/// 返回**借用**（由实例持有）；类型不带实例字典时给 `None`（调用方按缺属性报错）。
pub fn mounted_instance_dict(instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
    if let Some(mapping) = instance_attributes(instance, object) {
        return Some(mapping);
    }
    // SAFETY: object 是存活对象。
    let header = unsafe { object.as_ref() };
    let ty = header.ty();
    // SAFETY: ty 由注册表持有。
    let type_object = unsafe { ty.as_ref() };
    if type_object.type_flags() & crate::HAS_INSTANCE_DICT == 0 {
        return None;
    }
    let created = instance
        .alloc(DictObject::new(
            builtin_type(instance, "dict"),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    if type_object.has_inline_instance_dict() {
        // SAFETY: 这一位保证载荷就是 `AttributeObject`；`set_attributes` 接手新引用。
        let previous = unsafe { &*object.as_ptr().cast::<AttributeObject>() }
            .set_attributes(Some(created));
        debug_assert!(previous.is_none(), "上面确认过还没有字典");
        let _ = previous;
    } else {
        header.store_instance_dict(created);
    }
    Some(created)
}

/// 往实例的属性字典里写一项（`value` 是**新引用**，由字典接手；旧值被释放）。
pub fn instance_attribute_set(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    value: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // **数据描述符 `__set__`** ✓（第 192 轮）：类型 MRO 上若有 `__set__` ⇒ **调它** ✓
    //   —— 数据描述符**优先于实例字典** ✓（`__dict__` 本身另走下面那条 ✓，这里跳过 ✓）。
    if name != "__dict__" {
        // SAFETY: object 是存活对象。
        let object_type = unsafe { object.as_ref() }.ty();

        if let Some(found) = instance.type_lookup(object_type, name) {
            // SAFETY: found 由类型字典持有。
            let found_ty = unsafe { found.as_ref() }.ty();
            if let Some(setter) = instance.type_lookup(found_ty, "__set__") {
                let this = instance.retain(object);
                instance.retain(value);
                let returned = call_callable(
                    instance,
                    setter,
                    Some(found),
                    vec![this, value],
                    Vec::new(),
                    opcode,
                )?;
                release(instance, returned);
                return Ok(());
            }
        }
    }

    // **类型对象**：属性写进它的**命名空间** ✓（与 `instance_attributes` 同款口径 ✓，**一处真相** ✓）。
    if instance.is_type_object(object) {
        if name == "__dict__" {
            if unsafe { value.as_ref() }.ty() != builtin_type(instance, "dict") {
                unsafe { instance.release_object(value.as_ptr()) };
                // SAFETY: value 是存活对象。
                let value_type = unsafe { value.as_ref() }.ty();
                // SAFETY: 类型名由注册表持有。
                let value_type_name = unsafe { value_type.as_ref() }.name();
                let message = format!("__dict__ must be set to a dictionary, not a '{value_type_name}'");
                return Err(raise_builtin(instance, "TypeError", &message));
            }
            // SAFETY: object 是类型对象。
            let type_object = unsafe { &*object.as_ptr().cast::<crate::TypeObject>() };
            // 先取出旧命名空间（**借用**，不要跨 `set_dict` 持借 ✓），再把新的一份交出去 ✓。
            let previous = type_object.dict();
            type_object.set_dict(Some(value));
            if let Some(previous) = previous {
                // SAFETY: 被顶下来的那份由本函数消费。
                unsafe { instance.release_object(previous.as_ptr()) };
            }
            return Ok(());
        }
        let Some(namespace) = instance.type_namespace(object) else {
            release(instance, value);
            return Err(raise_builtin(instance, "TypeError", "类型对象没有命名空间"));
        };
        // SAFETY: namespace 是本实例里的 dict。
        let dict = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
        let position = dict
            .entries()
            .into_iter()
            .position(|(existing, _)| str_matches_public(instance, existing, name));
        match position {
            Some(slot) => {
                if let Some(old) = dict.replace_value(slot, value) {
                    release(instance, old);
                }
            }
            None => {
                let key = instance
                    .alloc(StrObject::new(instance.singletons().str_type(), name.to_owned()))
                    .into_raw()
                    .cast::<Header>();
                dict.insert_raw(key, value);
            }
        }
        let _ = opcode;
        return Ok(());
    }

    // **OM-14**：只有带实例字典的类型才收属性写入；否则报 `AttributeError`
    // （实测原话：`'dict' object has no attribute 'answer' and no __dict__ for setting new attributes`）
    // SAFETY: object 是存活对象。
    let header = unsafe { object.as_ref() };
    // SAFETY: ty 由注册表持有。
    let type_object = unsafe { header.ty().as_ref() };
    let has_instance_dict = type_object.type_flags() & crate::HAS_INSTANCE_DICT != 0;
    // **`obj.__dict__ = {…}`**（实测）：**整体替换**挂载的字典；值不是字典就是实测那条
    // `TypeError: __dict__ must be set to a dictionary, not a 'int'`。
    if name == "__dict__" && has_instance_dict {
        let is_dict = unsafe { value.as_ref() }.ty() == builtin_type(instance, "dict");
        if !is_dict {
            // SAFETY: value 是调用方交出的新引用，这里消费掉。
            unsafe { instance.release_object(value.as_ptr()) };
            // SAFETY: value 是存活对象。
            let value_type = unsafe { value.as_ref() }.ty();
            // SAFETY: 类型名由注册表持有。
            let value_type_name = unsafe { value_type.as_ref() }.name();
            let message = format!("__dict__ must be set to a dictionary, not a '{value_type_name}'");
            return Err(raise_builtin(instance, "TypeError", &message));
        }
        let replaced = if type_object.has_inline_instance_dict() {
            // SAFETY: 这一位保证载荷就是 `AttributeObject`。
            unsafe { &*object.as_ptr().cast::<AttributeObject>() }.set_attributes(Some(value))
        } else {
            // SAFETY: 上面确认过这个实例带（另行挂载的）实例字典。
            let previous = header.take_instance_dict();
            header.store_instance_dict(value);
            previous
        };
        if let Some(previous) = replaced {
            // SAFETY: 被顶下来的那份由本函数消费。
            unsafe { instance.release_object(previous.as_ptr()) };
        }
        return Ok(());
    }
    if !has_instance_dict {
        release(instance, value);
        let message = format!(
            "'{}' object has no attribute '{name}' and no __dict__ for setting new attributes",
            type_object.name()
        );
        return Err(raise_builtin(instance, "AttributeError", &message));
    }
    // **`OM-14`**：取或惰性创建（一处真相——`obj.__dict__` 一读就要给出 `{}`）
    let mapping = mounted_instance_dict(instance, object).expect("上面确认过这个类型带实例字典");
    // SAFETY: mapping 由对象或本函数持有。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .into_iter()
        .position(|(existing, _)| str_matches_public(instance, existing, name));
    match position {
        Some(slot) => {
            if let Some(old) = dict.replace_value(slot, value) {
                release(instance, old);
            }
        }
        None => {
            let key = instance
                .alloc(StrObject::new(instance.singletons().str_type(), name.to_owned()))
                .into_raw()
                .cast::<Header>();
            dict.insert_raw(key, value);
        }
    }
    let _ = opcode;
    Ok(())
}

/// 删掉实例属性字典里的一项（`DELETE_ATTR`；参照实现只删实例属性，不碰类型）。
pub fn instance_attribute_delete(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<(), ExecError> {
    let missing = || {
        // SAFETY: object 是存活对象。
        let type_name = unsafe { object.as_ref().ty().as_ref() }.name();
        raise_builtin(
            instance,
            "AttributeError",
            &format!("'{type_name}' object has no attribute '{name}'"),
        )
    };
    // **数据描述符 `__delete__`** ✓（第 192 轮）：类型 MRO 上若有 `__delete__` ⇒ 调它 ✓（优先于实例字典 ✓）。
    {
        // SAFETY: object 是存活对象。
        let object_type = unsafe { object.as_ref() }.ty();
        if let Some(found) = instance.type_lookup(object_type, name) {
            // SAFETY: found 由类型字典持有。
            let found_ty = unsafe { found.as_ref() }.ty();
            if let Some(deleter) = instance.type_lookup(found_ty, "__delete__") {
                let this = instance.retain(object);
                let returned = call_callable(
                    instance,
                    deleter,
                    Some(found),
                    vec![this],
                    Vec::new(),
                    0,
                )?;
                release(instance, returned);
                return Ok(());
            }
        }
    }
    let mapping = instance_attributes(instance, object).ok_or_else(missing)?;
    // SAFETY: mapping 是属性字典（dict）。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .into_iter()
        .position(|(key, _)| str_matches_public(instance, key, name))
        .ok_or_else(missing)?;
    if let Some((key, value)) = dict.remove(position) {
        release(instance, key);
        release(instance, value);
    }
    Ok(())
}

/// **`in` 的公开入口**（`operator.contains` 用；与字节码 `CONTAINS_OP` 共用同一份实现）。
///
/// 参数顺序照参照：`contains(容器, 项)`。
pub fn contains_public(
    instance: &Instance,
    container: NonNull<Header>,
    item: NonNull<Header>,
    opcode: u8,
) -> Result<bool, ExecError> {
    contains(instance, container, item, opcode)
}

/// `+=` 的**就地**语义（`NB_INPLACE_ADD`）：`list` 是**就地 extend**（别名可见，实测），
/// 其余类型退化为基运算 `+`（不可变 ⇒ 重新绑定与就地不可区分）。
pub(crate) fn inplace_add(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    if Some(unsafe { left.as_ref() }.ty()) == instance.type_named("list") {
        let items = sequence_items(instance, right, opcode)?;
        // SAFETY: 类型身份已确认，且 left 是帧值栈上的存活对象。
        let list = unsafe { &*left.as_ptr().cast::<ListObject>() };
        for item in items {
            list.append(item);
        }
        // SAFETY: 就地改动后返回**同一个对象**（新引用），调用方随后 `STORE` 回去
        unsafe { instance.incref_object(left.as_ptr()) };
        return Ok(left);
    }
    // 非 `list`：走 `+` 的**公开入口** `concat_public`（`str`／`bytes`／`tuple` 的拼接在那儿，
    // 其余落 `arithmetic_public`）——第一版这里写的是 `arithmetic_public`，被语料 `s += 'b'` 打回
    concat_public(instance, left, right, opcode)
}

/// 其余就地运算：不可变类型等价于基运算；**可变容器**（`set`／`dict`）的就地语义不同 ⇒ 如实报。
pub(crate) fn inplace_arithmetic(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    let container = unsafe { left.as_ref() }.ty();
    let mutable = Some(container) == instance.type_named("set")
        || Some(container) == instance.type_named("dict")
        || Some(container) == instance.type_named("list")
        || Some(container) == instance.type_named("bytearray");
    if mutable {
        return Err(ExecError::Unsupported {
            opcode,
            what: "可变容器的就地运算（`set`／`dict`／`bytearray` 的 `|=` 一族）尚未接线",
        });
    }
    arithmetic_public(instance, left, right, symbol, opcode)
}


pub fn compare_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<bool, ExecError> {
    // 相等／不等：走与 `==` 同一套（整数／浮点／字符串按值，其余按身份）
    match symbol {
        "==" => return Ok(values_equal_public(instance, left, right)),
        "!=" => return Ok(!values_equal_public(instance, left, right)),
        _ => {}
    }
    // 大小比较：两边都必须是**同一族**的标量（int／bool 一族、str 一族、bytes 一族）
    // **集合比较＝子集／超集** ✓（第 205 轮）：`Lib/os.py` 的 `_have_functions` 登记用 `<=` ✓
    // ⇒ 先前只有**标量**那一支 ✗ ⇒ 报 `TypeError: '<=' not supported between instances of 'set' and 'set'` ✗。
    // 口径与参照一致 ✓：`<=` 子集、`<` 真子集、`>=` 超集、`>` 真超集、`==` 两边互相包含 ✓、`!=` 取反 ✓。
    if let (Some(left_items), Some(right_items)) = (set_items_of(instance, left), set_items_of(instance, right)) {
        let contains = |haystack: &[NonNull<Header>], needle: NonNull<Header>| {
            haystack
                .iter()
                .any(|other| values_equal_public(instance, needle, *other))
        };
        let left_subset = left_items.iter().all(|item| contains(&right_items, *item));
        let right_subset = right_items.iter().all(|item| contains(&left_items, *item));
        return match symbol {
            "==" => Ok(left_subset && right_subset),
            "!=" => Ok(!(left_subset && right_subset)),
            "<=" => Ok(left_subset),
            "<" => Ok(left_subset && !right_subset),
            ">=" => Ok(right_subset),
            ">" => Ok(right_subset && !left_subset),
            _ => Ok(false),
        };
    }
    // **数值混比**（`float` 与 `int`／`bool`）✓（第 280 轮修 ✗）：参照里 `1.0 > 0` 为真 ✓，
    // 而先前"两边必须同一族" ✗ ⇒ 报 `TypeError: '>' not supported between instances of 'float' and 'int'` ✗
    //（实测：`_thread.TIMEOUT_MAX > 0` 当场撞上 ✓）。**NaN** 参与时参照给 `False`（**不是** `TypeError` ✓）。
    // 大整数超出 `i64` 时本层仍报 `TypeError`（如实 ✓；`P1-11` 的比较面随后补 ✓）。
    let left_float = instance.float_value(left);
    let right_float = instance.float_value(right);
    if left_float.is_some() || right_float.is_some() {
        let as_float = |value: NonNull<Header>, float: Option<f64>| {
            float.or_else(|| instance.int_value(value).map(|integer| integer as f64))
        };
        if let (Some(a), Some(b)) = (as_float(left, left_float), as_float(right, right_float)) {
            let Some(ordering) = a.partial_cmp(&b) else {
                return Ok(false);
            };
            return Ok(match symbol {
                "<" => ordering.is_lt(),
                "<=" => ordering.is_le(),
                ">" => ordering.is_gt(),
                ">=" => ordering.is_ge(),
                _ => false,
            });
        }
    }
    let left_int = instance.int_of(left);
    let right_int = instance.int_of(right);
    let left_text = instance.text_value(left);
    let right_text = instance.text_value(right);
    let left_bytes = instance.bytes_value(left);
    let right_bytes = instance.bytes_value(right);
    let ordering = match (left_int, right_int, left_text, right_text, left_bytes, right_bytes) {
        (Some(a), Some(b), _, _, _, _) => Some(a.cmp(&b)),
        (_, _, Some(a), Some(b), _, _) => a.partial_cmp(&b),
        // `bytes` 按**字节**字典序（`P1-12`；实测 `b'ab' < b'b'` 为真）
        (_, _, _, _, Some(a), Some(b)) => Some(a.cmp(b)),
        _ => None,
    };
    let Some(ordering) = ordering else {
        let left_name = instance.type_name(instance.type_of(left));
        let right_name = instance.type_name(instance.type_of(right));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{symbol}' not supported between instances of '{left_name}' and '{right_name}'"),
        ));
    };
    let _ = opcode;
    Ok(match symbol {
        "<" => ordering.is_lt(),
        "<=" => ordering.is_le(),
        ">" => ordering.is_gt(),
        ">=" => ordering.is_ge(),
        _ => {
            return Err(ExecError::Unsupported {
                opcode,
                what: "compare_public 收到了没见过的比较符号",
            })
        }
    })
}

/// **`TS-44`**：类型字典里若定义了某个 dunder（`__repr__`／`__str__`），就调用它并取文本。
///
/// 找不到定义 ⇒ `None`（调用方走槽位路径）。**只在类型字典里有定义时才调用**，所以内建类型
/// （它们靠槽位）零开销、行为不变；用户类的覆写则**一致地**在顶层 `repr(obj)`／`str(obj)` 与
/// 容器元素上都生效。
///
/// 覆写抛异常时：**如实上抛**（`OM-11` 扩之后 `repr`／`str` 槽能表达失败了 ⇒ 不再吞掉；
/// 此前"吞掉 + 记在实例上"的偏差随之消失）。
pub(crate) fn override_text(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Option<String>, ExecError> {
    let ty = instance.type_of(object);
    // 类型字典里没有这个名字 ⇒ 没有覆写，直接走槽位路径（**不是**"调用失败"）
    let Some((owner, _)) = instance.type_lookup_owner(ty, name) else {
        return Ok(None);
    };
    // **`object` 自己那条不算覆写** ✓（第 211 轮 ✗）：`object.__str__`／`object.__repr__` 是本层新挂的
    // **属性面**（`Lib/types.py` 的 `type(object.__str__)` 要它 ✓）⇒ 若当覆写 ⇒ **每个**对象都会命中
    // 它 ⇒ 把 `str`／`int` 自带的 `str` 槽带跑 ✗（实测 `f"{x}"` 给 `\'1\'` ✗、四条 f-string 语料红 ✓）。
    if Some(owner) == instance.type_named("object") {
        return Ok(None);
    }
    match call_object_method(instance, object, name, &[])? {
        Some(result) => {
            // SAFETY: result 是新引用，存活。
            let text = instance.text_value(result);
            release(instance, result);
            Ok(text)
        }
        None => Ok(None),
    }
}

/// **`TS-44`**：元素的 `repr` —— 先走属性通道的 `__repr__`，没有才落到原生槽位／默认实现。
///
/// 容器载荷的 `repr` 槽用它（`repr([x])` 里的 `x` 也要尊重 Python 级覆写）。
pub(crate) fn element_repr(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<String, ExecError> {
    match call_object_method(instance, object, "__repr__", &[])? {
        Some(result) => {
            // SAFETY: result 是新引用，存活。
            let is_str = unsafe { result.as_ref() }.ty() == instance.singletons().str_type();
            let text = if is_str {
                // SAFETY: 类型身份已确认。
                Some(unsafe { &*result.as_ptr().cast::<StrObject>() }.value().to_owned())
            } else {
                None
            };
            release(instance, result);
            match text {
                Some(text) => Ok(text),
                None => instance.object_repr(object),
            }
        }
        None => instance.object_repr(object),
    }
}

/// **`TS-44`**：元素的 `str` —— 同上，走 `__str__`。
#[allow(dead_code)] // 容器 `str`（`str([x])`）接线时用它；现在只剩 `repr` 那条在用
pub(crate) fn element_str(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<String, ExecError> {
    match call_object_method(instance, object, "__str__", &[])? {
        Some(result) => {
            // SAFETY: result 是新引用，存活。
            let is_str = unsafe { result.as_ref() }.ty() == instance.singletons().str_type();
            let text = if is_str {
                // SAFETY: 类型身份已确认。
                Some(unsafe { &*result.as_ptr().cast::<StrObject>() }.value().to_owned())
            } else {
                None
            };
            release(instance, result);
            match text {
                Some(text) => Ok(text),
                None => instance.object_str(object),
            }
        }
        None => instance.object_str_native(object),
    }
}

/// **`TS-44`**：语义走**属性通道**——先查类型字典里的同名 dunder（返回 `str` 的文本），
/// 查不到就返回 `None`（调用方落到原生槽位／默认实现）。
pub(crate) fn dunder_text(
    instance: &Instance,
    value: NonNull<Header>,
    name: &str,
    opcode: u8,
) -> Result<Option<String>, ExecError> {
    let found = match attribute_lookup(instance, value, name) {
        Ok(found) => found,
        Err(_) => return Ok(None),
    };
    let (callable, this) = match found {
        Attribute::Method { function, this } => (function, this),
        Attribute::Value(method) | Attribute::Owned(method) => (method, value),
    };
    let result = call_callable(instance, callable, Some(this), Vec::new(), Vec::new(), opcode)?;
    // SAFETY: result 是新引用，存活。
    let result_type = unsafe { result.as_ref() }.ty();
    if result_type != instance.singletons().str_type() {
        // 实测：`TypeError: __str__ returned non-string (type int)`
        // SAFETY: 类型名由注册表持有。
        let type_name = unsafe { result_type.as_ref() }.name();
        release(instance, result);
        let message = format!("{name} returned non-string (type {type_name})");
        return Err(raise_builtin(instance, "TypeError", &message));
    }
    // SAFETY: 类型身份已确认。
    let text = unsafe { &*result.as_ptr().cast::<StrObject>() }.value().to_owned();
    release(instance, result);
    Ok(Some(text))
}

/// `ascii()` 的转义：非 ASCII 字符按 `\xNN`／`\uNNNN`／`\UNNNNNNNN` 写出来。
pub(crate) fn escape_non_ascii(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars() {
        if character.is_ascii() {
            out.push(character);
            continue;
        }
        let code = character as u32;
        if code <= 0xFF {
            out.push_str(&format!("\\x{code:02x}"));
        } else if code <= 0xFFFF {
            out.push_str(&format!("\\u{code:04x}"));
        } else {
            out.push_str(&format!("\\U{code:08x}"));
        }
    }
    out
}

/// 边界检查：`CHECK_BOUNDARY_IN` 查**入参**、`CHECK_BOUNDARY_OUT` 查**返回值**。
///
/// * 归责方向（`TS-10`）：入参失败归**调用方**、返回值失败归**被调用方**——消息里带方向，
///   捕获方能区分（`TS-12`）
/// * 签名条目（`BC-24`）：`oparg` 是**常量表下标**，常量**必须**是**标签元组**。
///   `IN` 按顺序比对帧的局部槽 `0..`（形参），`OUT` 比对**栈顶**（**不**弹出——后面紧跟
///   `RETURN_VALUE`）
/// * 标签形态：类型对象 ⇒ 子类型判定（`TS-29`）；字符串 `"Any"` ⇒ 双向相容（`TS-28`）；
///   二元组 `(外类型, 内标签)` ⇒ 只看**外类型**（`TS-13` 默认浅层；`TS-31` 的深层档位随后补，
///   `TS-30` 的不变性因此只体现在外类型上）
/// * 失败抛 `TypeBoundaryError`，消息含 `TS-11` 的四要素：方向、期望、实际、位置（文件名＋行号）
pub(crate) fn boundary_check(
    instance: &Instance,
    frame: &Frame,
    oparg: u8,
    opcode: u8,
) -> Result<(), ExecError> {
    let code_raw = frame.code().ok_or(ExecError::Unsupported {
        opcode,
        what: "边界检查需要 code object",
    })?;
    // SAFETY: 帧持有一份 code object 引用，存活。
    let code = unsafe { &*code_raw.as_ptr().cast::<CodeObject>() };
    let signature = code.constant(oparg as usize).ok_or(ExecError::Unsupported {
        opcode,
        what: "边界检查的签名条目下标越界",
    })?;
    // SAFETY: 常量由常量表持有，存活。
    let signature_type = unsafe { signature.as_ref() }.ty();
    if signature_type != builtin_type(instance, "tuple") {
        return Err(ExecError::Unsupported {
            opcode,
            what: "边界检查的签名条目必须是标签元组（编译器保证）",
        });
    }
    // SAFETY: 类型身份刚确认。
    let labels = unsafe { &*signature.as_ptr().cast::<TupleObject>() };
    let incoming = opcode == opcode_of("CHECK_BOUNDARY_IN") as u8;
    let mut checked = 0usize;
    for index in 0..labels.len() {
        let label = labels.item(index).expect("下标在范围内");
        let actual = if incoming {
            // 形参不够（实参更少）时没有可查的值 ⇒ 交给调用绑定那条路去报错
            match frame.local(index) {
                Ok(Some(value)) => value,
                _ => continue,
            }
        } else {
            // 返回值在栈顶：**不**弹出
            let value = frame.peek().map_err(|_| ExecError::Unsupported {
                opcode,
                what: "边界检查要在栈顶取值，但栈是空的",
            })?;
            if index + 1 < labels.len() {
                // `OUT` 的签名条目只有一个标签；多给了就按"取第一个"处理会悄悄放行 ⇒ 如实报错
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "返回值边界检查的签名条目只能是**一个**标签",
                });
            }
            value
        };
        checked += 1;
        if boundary_accepts(instance, actual, label) {
            continue;
        }
        // SAFETY: actual 是存活对象。
        let actual_name = type_name_of(instance, unsafe { actual.as_ref() }.ty());
        let expected = render_label(instance, label);
        let line = line_at_offset(code, frame.instruction_pointer());
        let direction = if incoming { "argument" } else { "return" };
        let message = format!(
            "{direction} boundary check failed: expected {expected}, got {actual_name} \
             ({}: line {line})",
            code.filename()
        );
        return Err(raise_builtin(instance, "TypeBoundaryError", &message));
    }
    let _ = checked;
    Ok(())
}

/// 一条标签是否接受这个**实际值**（`TS-28`…`TS-30`／`TS-13` 的浅层口径）。
pub(crate) fn boundary_accepts(instance: &Instance, actual: NonNull<Header>, label: NonNull<Header>) -> bool {
    // SAFETY: actual 由调用方保证存活。
    let actual_type = unsafe { actual.as_ref() }.ty();
    // SAFETY: label 由常量表持有，存活。
    let label_type = unsafe { label.as_ref() }.ty();
    if label_type == builtin_type(instance, "str") {
        // SAFETY: 类型身份刚确认。
        return unsafe { &*label.as_ptr().cast::<StrObject>() }.value() == "Any";
    }
    if is_type_object(instance, label_type) {
        // SAFETY: label 是类型对象。
        let expected = label.cast::<TypeObject>();
        return instance.is_subtype(actual_type, expected);
    }
    if label_type == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份刚确认。
        let parts = unsafe { &*label.as_ptr().cast::<TupleObject>() };
        if parts.len() != 2 {
            return false;
        }
        let outer = parts.item(0).expect("下标在范围内");
        let inner = parts.item(1).expect("下标在范围内");
        if !is_type_object(instance, unsafe { outer.as_ref() }.ty()) {
            return false;
        }
        if !instance.is_subtype(actual_type, outer.cast::<TypeObject>()) {
            return false;
        }
        // **`TS-31` 的深层档位**：标签**带了内层**就递归比元素。浅层编译只发**裸**类型标签
        // （见 `compile::CheckTier`），所以这条分支只在深层产物里出现——`TS-13` 的
        // "默认浅层"因此是**代码生成**的结果，而不是运行期开关。
        return elements_accepted(instance, actual, actual_type, inner);
    }
    false
}

/// **`TS-31`**：容器元素逐个比（深层档位）。只深入**恰好是** `list`／`tuple` 的实际值——
/// 别的容器（子类、`dict` 等）没有统一的元素视图，**放行**并在文档里写明这条边界。
pub(crate) fn elements_accepted(
    instance: &Instance,
    actual: NonNull<Header>,
    actual_type: NonNull<TypeObject>,
    inner: NonNull<Header>,
) -> bool {
    if actual_type == builtin_type(instance, "list") {
        // SAFETY: 类型身份刚确认。
        let list = unsafe { &*actual.as_ptr().cast::<crate::ListObject>() };
        return (0..list.len()).all(|index| {
            list.item(index)
                .map(|item| boundary_accepts(instance, item, inner))
                .unwrap_or(true)
        });
    }
    if actual_type == builtin_type(instance, "tuple") {
        // SAFETY: 同上。
        let tuple = unsafe { &*actual.as_ptr().cast::<TupleObject>() };
        return (0..tuple.len()).all(|index| {
            tuple
                .item(index)
                .map(|item| boundary_accepts(instance, item, inner))
                .unwrap_or(true)
        });
    }
    true
}
