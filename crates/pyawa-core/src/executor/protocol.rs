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
use crate::executor::set_operation;
use crate::executor::str_matches_public;
use crate::executor::subscript_get;
use crate::executor::unsupported_operand;
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

/// **`set` 的就地位运算**（`|=`／`&=`／`-=`／`^=`）：在 `left`（`set` 族、**可变**）上就地改，
/// 返回**同一对象的新引用** ✓（`s is s_old` 为真 ✓，参照实测 ✓）。
///
/// 右侧只认 `set`／`frozenset`（含子类）——参照实测 `s |= [4]` 报
/// `TypeError: unsupported operand type(s) for |=: 'set' and 'list'` ✓，
/// **不是** `set.update` 那条"任意可迭代"的路 ✗（别把两处混成一件事）。
fn inplace_set_operation(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 调用方刚核过 left 是 `set` 族 ⇒ 载荷就是 `SetObject` ✓。
    let left_set = unsafe { &*left.as_ptr().cast::<crate::builtin_objects::SetObject>() };
    // 借用视图（**不**持有引用 ✓）。右侧不是集合族 ⇒ 报同位运算的 `TypeError`（**符号带 `=`** ✓，
    // 参照 `set |= list` 的消息里就是 `|=` ✓）。
    let Some(right_items) = instance.set_items(right) else {
        return Err(unsupported_operand(instance, left, right, &format!("{symbol}=")));
    };
    if symbol == "|" {
        // **并集**＝就地 `update` ✓：右侧里左侧没有的元素搬进来。`set_items` 给的是**借用** ✓
        // ⇒ 每项先 `retain` 再交给 `SetObject` 持有 ✓（`OM-16`）。
        for item in right_items {
            let known = left_set
                .position_of(|candidate| values_equal_public(instance, candidate, item))
                .is_some();
            if !known {
                instance.retain(item);
                left_set.insert_raw(item);
            }
        }
    } else {
        // **交／差／对称差**：先按**基运算**算出新集合（**一处真相**：`set_operation` ✓，
        // 它照参照给一个**自有引用**的新 `set` ✓），再把 `left` 的内容**整体换掉** ✓ ——
        // 这样 `&= s`／`^= s` 这类自身别名也安全（两边都读完之后才动 `left` ✓）。
        let fresh = set_operation(instance, left, right, symbol);
        let fresh_items = instance.set_items(fresh).expect("刚由 set_operation 造出的 set");
        // 清空 `left`：每个元素那份引用由本对象持有 ⇒ 归还引擎 ✓（照 `OM-20` 的纪律）。
        while let Some(removed) = left_set.remove(0) {
            // SAFETY: removed 由 left 持有。
            unsafe { instance.release_object(removed.as_ptr()) };
        }
        for item in fresh_items {
            // fresh 持有的那些引用还在 ⇒ 这里再 `retain` 一份交给 left ✓。
            instance.retain(item);
            left_set.insert_raw(item);
        }
        // fresh 自己那份（连同它持有的元素引用）归还 ✓。
        // SAFETY: fresh 是 `set_operation` 刚给的新引用。
        unsafe { instance.release_object(fresh.as_ptr()) };
    }
    let _ = opcode;
    // 就地改完 ⇒ 回**同一个对象**的新引用（调用方随后 `STORE` 回去 ✓；`OM-16`）。
    Ok(instance.retain(left))
}

/// 往 `mapping` 写入一对**自有引用**（`key`／`value` 各一份由本函数接管 ✓）。
///
/// 已有键 ⇒ 只替换值（旧值归还引擎 ✓）、手上那份 `key` 也归还 ✓；新键 ⇒ `insert_raw` 接管两份 ✓。
/// 照 `dict.update`（`crate::builtin::dict::dict_update_native`）的同一套引用账。
fn dict_put_owned(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
    value: NonNull<Header>,
) {
    // SAFETY: 调用方保证 mapping 是本实例的 dict。
    let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    match crate::builtin::dict::dict_position(instance, mapping, key) {
        Some(index) => {
            if let Some(old) = object.set_value_at(index, value) {
                // SAFETY: 旧值那份引用由本对象持有 ⇒ 归还引擎 ✓。
                unsafe { instance.release_object(old.as_ptr()) };
            }
            // 键已在字典里（不替换键）⇒ 我们手上这份 key 归还引擎 ✓。
            // SAFETY: key 由调用方转交。
            unsafe { instance.release_object(key.as_ptr()) };
        }
        None => object.insert_raw(key, value),
    }
}

/// **`dict` 的 `|=`**（`NB_INPLACE_OR`）：就地并入，返回时**同一个对象** ✓（参照：`d is d_old` ✓）。
///
/// 右侧三种形态照参照（本机 python3 3.14 实测 ✓）：
///   ① `dict`（含子类）⇒ 逐对并入（**一处真相**：`dict.update` 那份实现 ✓）；
///   ② 有 `keys()` 的**映射** ⇒ `keys()` 逐键、再 `右侧[键]` 取值并入 ✓；
///   ③ 其余**键值对可迭代**（`[(k, v)]`／生成器／`"ab"` 一类）⇒ 逐元素摊成两项 ✓；
///      元素不是两项 ⇒ `ValueError: dictionary update sequence element #i has length n; 2 is required` ✓；
///      元素不可迭代 ⇒ `TypeError: object is not iterable` ✓（前面已并入的**保留** ✓，与参照同 ✓）。
fn inplace_dict_merge(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // ① `dict`：直接复用 `update` 的同一份实现（返回值是 `None` 的新引用 ⇒ 这里归还 ✓）。
    if instance.dict_entries(right).is_some() {
        let result = crate::builtin::dict::dict_update_native(instance, Some(left), &[right], &[])?;
        // SAFETY: result 是本函数刚拿到的新引用。
        unsafe { instance.release_object(result.as_ptr()) };
        return Ok(());
    }
    // ② 映射协议：`keys()` 存在就按它逐键取值（`iterable_items` 给**新引用** ⇒ 用完逐个归还 ✓）。
    if let Some(keys) = call_object_method(instance, right, "keys", &[])? {
        let key_items = instance.iterable_items(keys);
        let Some(key_items) = key_items else {
            let right_name = instance.type_name(instance.type_of(right));
            let keys_name = instance.type_name(instance.type_of(keys));
            // SAFETY: keys 是本函数刚拿到的新引用。
            unsafe { instance.release_object(keys.as_ptr()) };
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("{right_name}.keys() returned a non-iterable (type {keys_name})"),
            ));
        };
        for (position, key) in key_items.iter().enumerate() {
            let value = match subscript_get(instance, right, *key, opcode) {
                Ok(value) => value,
                Err(error) => {
                    // 失败前把本函数手上的引用（余下的键 ＋ keys）归还 ✓，再原样上抛。
                    for leftover in &key_items[position..] {
                        // SAFETY: 这些引用由 `iterable_items` 给出。
                        unsafe { instance.release_object(leftover.as_ptr()) };
                    }
                    // SAFETY: 同上。
                    unsafe { instance.release_object(keys.as_ptr()) };
                    return Err(error);
                }
            };
            // key／value 两份引用都转交出去 ✓。
            dict_put_owned(instance, left, *key, value);
        }
        // SAFETY: keys 是本函数刚拿到的新引用。
        unsafe { instance.release_object(keys.as_ptr()) };
        return Ok(());
    }
    // ③ 键值对序列：与 `dict.update` **同一处真相**（task-25 ① ✓）——
    // 文案与引用纪律只在 `crate::builtin::dict::dict_update_sequence` 里有一份 ✓。
    crate::builtin::dict::dict_update_sequence(instance, left, right)?;
    Ok(())
}

/// 其余就地运算：不可变类型等价于基运算；**可变容器**按类型分派。
///
/// * `set`：四个位运算**就地** ✓（`inplace_set_operation`）；其余形态照参照报 `TypeError` ✓
///   （消息里的符号是**就地**形态，如 `*=` ✓ —— 参照 `s *= 2` 给 `*=` ✓）。
/// * `dict`：只有 `|=` 有就地语义 ✓；其余同上报 `TypeError` ✓。
/// * `list`／`bytearray`：`&=`／`^=`／`-=` 一族参照本就**不支持** ⇒ 照参照报 `TypeError` ✓；
///   `*=`（就地重复）参照支持、本层仍**未接线** ⇒ 如实报未接线（**不冒充**成功 ✓）。
/// * 其余（`int`／`str`／`tuple` 一类不可变）⇒ 基运算 ✓（现有路，不动）。
pub(crate) fn inplace_arithmetic(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    let container = unsafe { left.as_ref() }.ty();
    let subtype_of = |name: &str| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(container, base))
    };
    // `set`（**不含** `frozenset`：它不可变 ⇒ 走下面的基运算 ✓）
    if subtype_of("set") {
        if matches!(symbol, "&" | "|" | "-" | "^") {
            return inplace_set_operation(instance, left, right, symbol, opcode);
        }
        return Err(unsupported_operand(instance, left, right, &format!("{symbol}=")));
    }
    // `dict`（含 `defaultdict` 一类子类）
    if subtype_of("dict") {
        if symbol == "|" {
            inplace_dict_merge(instance, left, right, opcode)?;
            return Ok(instance.retain(left));
        }
        return Err(unsupported_operand(instance, left, right, &format!("{symbol}=")));
    }
    // `list`／`bytearray`
    if subtype_of("list") || subtype_of("bytearray") {
        if matches!(symbol, "&" | "|" | "^" | "-" | "/" | "//" | "%" | "**" | "<<" | ">>" | "@") {
            return Err(unsupported_operand(instance, left, right, &format!("{symbol}=")));
        }
        // `*`（就地重复）：参照支持 ⇒ 本层**如实报未接线**（别拿 `TypeError` 冒充参照行为 ✗）。
        return Err(ExecError::Unsupported {
            opcode,
            what: "序列的就地重复 `*=` 尚未接线",
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
    // **序列字典序** ✓（task-23 ✓；`heapq.nsmallest`／`nlargest` 立据 ✓）：
    //  - `tuple` 之间、`list` 之间（含各自子类 ✓）逐元素比 ✓：**首个不等元素**定序 ✓
    //    （递归走本函数 ✓ ⇒ 元素不可比时自然报 `'<' not supported between instances of 'str' and 'int'` ✓）；
    //  - 一方是另一方**前缀** ⇒ 短的更小 ✓（`(1,2) < (1,2,0)` ⇒ `True` ✓）；
    //  - **混合**（`tuple` vs `list`）⇒ **落到下面那条 `TypeError`** ✓（参照实测
    //    `(1,2) < [1,3]` ⇒ `'<' not supported between instances of 'tuple' and 'list'` ✓）；
    //  - `==`／`!=` 不走这里 ✓（上面已按"值／身份"判 ✓，参照 `(1,2) == [1,2]` ⇒ `False` ✓）。
    let left_is_tuple = instance.tuple_items(left).is_some();
    let right_is_tuple = instance.tuple_items(right).is_some();
    let left_sequence = if left_is_tuple { instance.tuple_items(left) } else { instance.list_items(left) };
    let right_sequence = if right_is_tuple { instance.tuple_items(right) } else { instance.list_items(right) };
    if left_is_tuple == right_is_tuple {
        if let (Some(left_items), Some(right_items)) = (left_sequence, right_sequence) {
            for (first, second) in left_items.iter().zip(right_items.iter()) {
                if values_equal_public(instance, *first, *second) {
                    continue;
                }
                // 首个不等的那对**定序** ✓（两边必不可比时才报错 ✓；此处二者已**不相等** ✓
                // ⇒ 只判"前者更小吗"就够 ✓，`<` 为假即后者更小 ✓）。
                let first_is_less = compare_public(instance, *first, *second, "<", opcode)?;
                let _ = opcode;
                return Ok(match symbol {
                    "<" | "<=" => first_is_less,
                    ">" | ">=" => !first_is_less,
                    _ => false,
                });
            }
            let shorter = left_items.len() < right_items.len();
            let same_length = left_items.len() == right_items.len();
            let _ = opcode;
            return Ok(match symbol {
                "<" => shorter,
                "<=" => shorter || same_length,
                ">" => !shorter && !same_length,
                ">=" => !shorter,
                _ => false,
            });
        }
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
    // **`str` 的码点序**（Task-66 ✓）：两个恰 `str` 就走这里（含孤立代理也按码点比 ✓，
    // 不再经 `value()` ⇒ 不 panic）；合法 UTF-8 路径与旧的 `String::cmp` 结果逐例相同 ✓。
    if let Some(ordering) = instance.str_order_cmp(left, right) {
        return Ok(match symbol {
            "<" => ordering.is_lt(),
            "<=" => ordering.is_le(),
            ">" => ordering.is_gt(),
            ">=" => ordering.is_ge(),
            _ => false,
        });
    }
    // `text_of` 是**安全**视图（WTF-8 表示 ⇒ `None`；上面已处理两个恰 str 的情形 ✓）
    let left_text = instance.text_of(left);
    let right_text = instance.text_of(right);
    let left_bytes = instance.bytes_value(left);
    let right_bytes = instance.bytes_value(right);
    let ordering = match (left_int, right_int, left_text, right_text, left_bytes, right_bytes) {
        (Some(a), Some(b), _, _, _, _) => Some(a.cmp(&b)),
        (_, _, Some(a), Some(b), _, _) => Some(a.cmp(b)),
        // `bytes` 按**字节**字典序（`P1-12`；实测 `b'ab' < b'b'` 为真）
        (_, _, _, _, Some(a), Some(b)) => Some(a.cmp(b)),
        _ => None,
    };
    let Some(ordering) = ordering else {
        // **富比较协议**（task-12 ✓）：内建快速路径判不出可比性之后才走这里 ✓
        // （`int`／`str`／`bytes`／`tuple`／`set` 的热路径在上面已经返回 ⇒ 这一步不加重它们的开销 ✓）。
        if let Some(verdict) = rich_compare(instance, left, right, symbol, opcode)? {
            return Ok(verdict);
        }
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

/// **用户类的富比较协议**（task-12 ✓）：`<`／`<=`／`>`／`>=` 的**正向 ＋ 反向**派发。
///
/// 参照口径（CPython `do_richcompare`）：
/// - `a < b` ⇒ 先 `type(a).__lt__(a, b)` ✓；返回 `NotImplemented` ⇒ 再 `type(b).__gt__(b, a)` ✓；
/// - 两边都 `NotImplemented`（或**都没有**这个方法）⇒ `None` ⇒ 调用方照原样报 `TypeError` ✓。
///
/// **只在内建快速路径判不出之后调** ✓（`int`／`str`／`bytes`／`tuple`／`set` 那一族在上面就返回了
/// ⇒ 热路径不多走一步 ✓）。方法本身经属性通道取（`call_special_method`；找不到给 `Ok(None)` ✓）。
fn rich_compare(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<Option<bool>, ExecError> {
    // 符号 ⇒（正向方法，反向方法）。反向方法挂在**右操作数**上 ✓。
    let (forward, reflected) = match symbol {
        "<" => ("__lt__", "__gt__"),
        "<=" => ("__le__", "__ge__"),
        ">" => ("__gt__", "__lt__"),
        ">=" => ("__ge__", "__le__"),
        _ => return Ok(None),
    };
    let not_implemented = instance.not_implemented();
    for (name, object, other) in [(forward, left, right), (reflected, right, left)] {
        let Some(result) = call_special_method(instance, object, name, &[other])? else {
            continue;
        };
        // **结果新引用** ⇒ 两条出口都要 release ✓。
        if result == not_implemented {
            release(instance, result);
            continue;
        }
        let verdict = crate::executor::truthiness_public(instance, result, opcode)?;
        release(instance, result);
        return Ok(Some(verdict));
    }
    Ok(None)
}

/// **`TS-44`**：类型字典里若定义了某个 dunder（`__repr__`／`__str__`），就调用它并取文本。
///
/// 找不到定义 ⇒ `None`（调用方走槽位路径）。**只在类型字典里有定义时才调用**，所以内建类型
/// （它们靠槽位）零开销、行为不变；用户类的覆写则**一致地**在顶层 `repr(obj)`／`str(obj)` 与
/// 容器元素上都生效。
///
/// 覆写抛异常时：**如实上抛**（`OM-11` 扩之后 `repr`／`str` 槽能表达失败了 ⇒ 不再吞掉；
/// 此前"吞掉 + 记在实例上"的偏差随之消失）。
/// **对一个对象调用它的特殊方法**（`__repr__`／`__str__` 一族）——**类对象**走元类 MRO ✓。
///
/// 参照口径（第 410 轮，task-21 G4 ✓）：`repr(SomeClass)` 用的是 `type(SomeClass).__repr__`
/// （**元类 MRO** ✓），**绝不**落到类自己的实例方法 `C.__repr__` ✗。先前这里一律走
/// `attribute_lookup`（普通属性通道）✗ ⇒ 枚举类 `Flag` 的 `repr()` 命中的是 **`Flag` 类体里的
/// `__repr__`**（那是给**成员实例**用的 ✓）⇒ 里面 `self.__class__._value_repr_` 把类当成实例读 ✗
/// ⇒ `AttributeError: 'type' object has no attribute '_value_repr_'` ✗（真 `enum.py` 第 39 条
/// `class IntFlag(...)` 当场炸 —— 调试信息里当前帧是 `Flag.__repr__` ✓）。
pub(crate) fn call_special_method(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<Option<NonNull<Header>>, ExecError> {
    if !instance.is_type_object(object) {
        return call_object_method(instance, object, name, args);
    }
    // 类对象：只在**元类** MRO 上找 ✓（`type(obj).__mro__` ✓）。
    let metaclass = instance.type_of(object);
    let Some((owner, found)) = instance.type_lookup_owner(metaclass, name) else {
        return Ok(None);
    };
    // `type`／`object` 上的是**属性面**（不是格式化覆写 ✓，与 `override_text` 同口径 ✓）
    if Some(owner) == instance.type_named("type") || Some(owner) == instance.type_named("object") {
        return Ok(None);
    }
    let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        call_args.push(*argument);
    }
    let result = call_callable(instance, found, Some(object), call_args, Vec::new(), 0)?;
    Ok(Some(result))
}

/// **类对象的 `repr` 文本**：`<class '模块.限定名'>`（内建类型不带模块 ✓）。
pub(crate) fn class_object_repr(
    instance: &Instance,
    object: NonNull<Header>,
) -> String {
    // SAFETY: 调用方保证 object 是类对象（`is_type_object` ✓）。
    let type_object = unsafe { &*object.as_ptr().cast::<crate::TypeObject>() };
    let name = type_object.name().to_owned();
    let text = |key: &str| {
        instance
            .type_lookup(object.cast::<crate::TypeObject>(), key)
            .and_then(|value| instance.text_of(value))
            .map(str::to_owned)
    };
    let qualname = text("__qualname__").or_else(|| text("__name__")).unwrap_or(name);
    match text("__module__") {
        Some(module) if module != "builtins" => format!("<class '{module}.{qualname}'>"),
        _ => format!("<class '{qualname}'>"),
    }
}

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
    match call_special_method(instance, object, name, &[])? {
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
    match call_special_method(instance, object, "__repr__", &[])? {
        Some(result) => {
            // SAFETY: result 是新引用，存活。
            // P3-38 S2c：`__repr__` 返回的 str 走**安全口径**（WTF-8 ⇒ 有损文本，不 panic ✓）
            let text = instance.text_value(result);
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
    match call_special_method(instance, object, "__str__", &[])? {
        Some(result) => {
            // SAFETY: result 是新引用，存活。
            // P3-38 S2c：`__str__` 返回的 str 同样走安全口径 ✓
            let text = instance.text_value(result);
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
    // P3-38 S2c：dunder 返回的 str 走安全口径（WTF-8 ⇒ 有损文本，不 panic ✓）
    let text = instance.text_value(result).unwrap_or_default();
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
///   union 标签 `("Union", 成员…)` ⇒ **任一成员**相容即可（`TS-46`；形态定义见 `BC-24`）；
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

/// `TS-47` 的**运行期**赋值检查：`CHECK_ASSIGNMENT`（`BC-65`）。
///
/// * oparg（`BC-65`）是**常量表下标**，常量**必须**是**单标签元组**——标签形态与
///   `CHECK_BOUNDARY_*` 完全一致（类型对象／`"Any"`／`(外, 内)`／union，`BC-24`），因此**复用**
///   [`boundary_accepts`] 的相容判定（`TS-28`…`TS-31`／`TS-13`）
/// * 被检查的是**栈顶**（即将赋给已标注变量的值）：读而**不**弹出——后面那条
///   `STORE_FAST`／`STORE_NAME` 还要用它
/// * 失败抛 `TypeAssignmentError`（`TS-47`：`TypeError` 的子类；与 `TS-12` 的
///   `TypeBoundaryError` **同族、分工不同**），消息含 expected、got 与位置（文件名＋行号）
pub(crate) fn assignment_check(
    instance: &Instance,
    frame: &Frame,
    oparg: usize,
    opcode: u8,
) -> Result<(), ExecError> {
    let code_raw = frame.code().ok_or(ExecError::Unsupported {
        opcode,
        what: "赋值检查需要 code object",
    })?;
    // SAFETY: 帧持有一份 code object 引用，存活。
    let code = unsafe { &*code_raw.as_ptr().cast::<CodeObject>() };
    let label = code.constant(oparg).ok_or(ExecError::Unsupported {
        opcode,
        what: "赋值检查的标签下标越界",
    })?;
    // SAFETY: 常量由常量表持有，存活。
    let label_type = unsafe { label.as_ref() }.ty();
    if label_type != builtin_type(instance, "tuple") {
        return Err(ExecError::Unsupported {
            opcode,
            what: "赋值检查的标签常量必须是单标签元组（编译器保证）",
        });
    }
    // SAFETY: 类型身份刚确认。
    let labels = unsafe { &*label.as_ptr().cast::<TupleObject>() };
    if labels.len() != 1 {
        return Err(ExecError::Unsupported {
            opcode,
            what: "赋值检查的标签元组只能有**一个**标签",
        });
    }
    let expected_label = labels.item(0).expect("下标在范围内");
    // 栈顶**不**弹出：紧随的存储指令还要写它。
    let actual = frame.peek().map_err(|_| ExecError::Unsupported {
        opcode,
        what: "赋值检查要在栈顶取值，但栈是空的",
    })?;
    if boundary_accepts(instance, actual, expected_label) {
        return Ok(());
    }
    // SAFETY: actual 是存活对象。
    let actual_name = type_name_of(instance, unsafe { actual.as_ref() }.ty());
    let expected = render_label(instance, expected_label);
    let line = line_at_offset(code, frame.instruction_pointer());
    let message = format!(
        "assignment check failed: expected {expected}, got {actual_name} ({}: line {line})",
        code.filename()
    );
    Err(raise_builtin(instance, "TypeAssignmentError", &message))
}

/// **`TS-46`／union 标签**的形态判定：`("Union", 成员…)` —— 元组且**首项是字符串 `"Union"`**。
///
/// * 是 union ⇒ 返回那个元组（首项 `"Union"`、其后 ≥1 个成员标签）。
/// * 不是 union ⇒ `None`。**两种元组形态靠首项的类型区分**：首项是**类型对象**的是既有复合标签
///   `(外, 内)`（`TS-31` 深层档位），首项是**字符串** `"Union"` 的才是 union——所以既有复合标签
///   的语义（含 `elements_accepted` 的内层递归）**不受影响**。
///
/// 首项判**字符串值**（不是"任意 str"）：别的字符串首项（如 `("Any", int)`）仍按原来的未知标签拒绝。
fn union_members<'a>(instance: &Instance, label: NonNull<Header>) -> Option<&'a TupleObject> {
    // SAFETY: label 由常量表／栈持有，存活；返回的引用只在本次判定内使用。
    let label_type = unsafe { label.as_ref() }.ty();
    if label_type != builtin_type(instance, "tuple") {
        return None;
    }
    // SAFETY: 类型身份刚确认。
    let parts = unsafe { &*label.as_ptr().cast::<TupleObject>() };
    let head = parts.item(0)?;
    // SAFETY: 元组元素存活。
    if unsafe { head.as_ref() }.ty() != builtin_type(instance, "str") {
        return None;
    }
    // P3-38 S2c：内部标签名比较走**合法视图**（WTF-8 ⇒ `None` ⇒ 不等于 ASCII 常量 ⇒ 安全返回 ✓）
    // SAFETY: 类型身份刚确认。
    if instance.text_of(head) != Some("Union") {
        return None;
    }
    Some(parts)
}

/// 一条标签是否接受这个**实际值**（`TS-28`…`TS-30`／`TS-13` 的浅层口径）。
pub(crate) fn boundary_accepts(instance: &Instance, actual: NonNull<Header>, label: NonNull<Header>) -> bool {
    // SAFETY: actual 由调用方保证存活。
    let actual_type = unsafe { actual.as_ref() }.ty();
    // SAFETY: label 由常量表持有，存活。
    let label_type = unsafe { label.as_ref() }.ty();
    if label_type == builtin_type(instance, "str") {
        // P3-38 S2c：WTF-8 标签不可能是内部 ASCII 标签 ⇒ 合法视图比较即安全 ✓
        return instance.text_of(label) == Some("Any");
    }
    // **`TS-46`**：union 标签（`T?` ＝ `T | None` 的运行期形态）——**任一成员**接受即接受。
    // 成员可递归：类型对象／`"Any"`／嵌套 union／既有 `(外, 内)` 复合标签（成员级语义照旧）。
    if let Some(members) = union_members(instance, label) {
        // 首项之后必须**至少一个**成员（接口契约）；只有首项的畸形标签不放行。
        return (1..members.len())
            .filter_map(|index| members.item(index))
            .any(|member| boundary_accepts(instance, actual, member));
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

// --------------------------------------------------------------------------- #
// **类型级描述符安装**（`P3-17` 同族 ＋ `functools.wraps.__name__` 的正路）
// --------------------------------------------------------------------------- #
//
// 内建类型的"方法面"分两处：`OM-11` 的 `getattr` 槽（实例属性）＋**类型字典**（描述符协议）。
// 后者的既有安装点全在 `pyawa-stdlib` 的 `builtins_module`（`function.__get__`、
// `property.__get__`、各类型 `__new__` …）；本次新增的四族描述符
// （function 的可写 getset、property 的 `__set__`／`__delete__`、
// staticmethod／classmethod 的 `__get__`）集中在**这里**做一个**幂等安装器** ✓，
// 由 `builtins_module::build` **追加一行**调用 ✓。
//
// 为什么描述符的 native 实现在 core、安装器也在 core：`FunctionObject` 的覆盖字段与
// `AttributeObject` 的 `_kind` 都是 core 的私有载荷 ✓ ⇒ stdlib 只负责"建 `builtins` 时调一下" ✓
// （依赖方向不变：stdlib → core ✓）。

/// 函数可写属性的描述符类型名。
///
/// **如实登记（方案 A 的类别偏差）**：参照里 `function.__dict__['__name__']` 的类型是
/// `getset_descriptor`（`__doc__` 是 `member_descriptor`），本层四合一用自建类型 ⇒
/// `type(f.__class__.__dict__['__name__']).__name__` 与参照**不同** ✗；语义面（可写、
/// 数据描述符、错误文案）逐条对齐 ✓。抽例**不钉** `type(...)` 类别 ✓。
pub const FUNCTION_ATTRIBUTE_DESCRIPTOR_TYPE: &str = "function_attribute_descriptor";

/// 造一个原生函数对象（类型字典里的"方法"用 ✓；借用者负责交一份新引用 ✓）。
fn descriptor_native(
    instance: &Instance,
    name: &'static str,
    handler: crate::NativeFn,
) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    instance
        .alloc(crate::BuiltinFunctionObject::new(
            ty,
            name,
            core::cell::Cell::new(handler),
        ))
        .into_raw()
        .cast::<Header>()
}

/// 幂等：把 `handler` 装进 `ty` **自己的**类型字典（已有自己的就跳过 ✓）。
fn install_type_method(
    instance: &Instance,
    ty: NonNull<TypeObject>,
    name: &'static str,
    handler: crate::NativeFn,
) {
    if instance
        .type_lookup_owner(ty, name)
        .is_some_and(|(owner, _)| owner == ty)
    {
        return;
    }
    let native = descriptor_native(instance, name, handler);
    instance.set_type_attribute(ty, name, native);
}

/// **function 的四个可写属性**：把 `getset` 数据描述符装进 `function` 的类型字典 ✓
/// （`f.__name__ = …` 走 `instance_attribute_set` 的数据描述符分支 ⇒ 调这里的 `__set__` ✓）。
fn install_function_attribute_descriptors(instance: &Instance) {
    let Some(function_type) = instance.type_named("function") else {
        return;
    };
    // 幂等哨兵：`function` **自己的**命名空间里已经有 `__name__` 就不再装 ✓。
    if instance
        .type_lookup_owner(function_type, "__name__")
        .is_some_and(|(owner, _)| owner == function_type)
    {
        return;
    }
    let descriptor_type = match instance.type_named(FUNCTION_ATTRIBUTE_DESCRIPTOR_TYPE) {
        Some(existing) => existing,
        None => instance.new_attribute_type(FUNCTION_ATTRIBUTE_DESCRIPTOR_TYPE),
    };
    install_type_method(
        instance,
        descriptor_type,
        "__get__",
        crate::builtin::function::function_attribute_descriptor_get as crate::NativeFn,
    );
    install_type_method(
        instance,
        descriptor_type,
        "__set__",
        crate::builtin::function::function_attribute_descriptor_set as crate::NativeFn,
    );
    install_type_method(
        instance,
        descriptor_type,
        "__delete__",
        crate::builtin::function::function_attribute_descriptor_delete as crate::NativeFn,
    );
    for (attribute, kind) in [
        ("__name__", "name"),
        ("__qualname__", "qualname"),
        ("__doc__", "doc"),
        ("__module__", "module"),
        // **`__annotate__` 暂不装** ✗（`P3-17` 如实登记）：参照里它也可写，但本层 `cell` 对象
        // 还没有 `cell_contents`（`Lib/dataclasses.py:1298` 的 slots 路径要用它 ✗）⇒ 一旦
        // `fn.__annotate__ = …` 真的写进字段，`dataclasses` 的 `_update_func_cell_for__class__`
        // 就会踩到那个缺口 ⇒ `import asyncio`（`asyncio.graph` 的 `slots=True` dataclass）
        // 当场 `AttributeError: 'cell' object has no attribute 'cell_contents'` ✗。
        // 保持 HEAD 的"写进实例字典、读仍落到 code/字段默认值"的旧行为，直到 cell 面接线 ✓。
    ] {
        // 描述符实例的 `_kind` 存在**内联属性字典**里 ✓（与 `getset_descriptor` 的既有手法同形 ✓）。
        let kind_mapping = instance.new_dict();
        let kind_text = instance.new_str(kind);
        // `dict_set` 是**借用**口径（自己另存一份 ✓）⇒ 交出去之后这份要还 ✓（`OM-16`）。
        instance.dict_set(kind_mapping, "_kind", kind_text);
        // SAFETY: kind_text 是上面刚造的新引用、字典已另存一份。
        unsafe { instance.release_object(kind_text.as_ptr()) };
        let descriptor = instance.alloc(crate::AttributeObject::new(
            descriptor_type,
            RefCell::new(Some(kind_mapping)),
        ));
        instance.set_type_attribute(
            function_type,
            attribute,
            descriptor.into_raw().cast::<Header>(),
        );
    }
}

/// **property 的数据描述符面**：`__set__`／`__delete__`（`__get__` 若 stdlib 已装则不重复 ✓）。
fn install_property_descriptors(instance: &Instance) {
    let Some(property_type) = instance.type_named("property") else {
        return;
    };
    install_type_method(
        instance,
        property_type,
        "__get__",
        crate::builtin::property::property_descriptor_get as crate::NativeFn,
    );
    install_type_method(
        instance,
        property_type,
        "__set__",
        crate::builtin::property::property_descriptor_set as crate::NativeFn,
    );
    install_type_method(
        instance,
        property_type,
        "__delete__",
        crate::builtin::property::property_descriptor_delete as crate::NativeFn,
    );
}

/// **staticmethod／classmethod 的 `__get__`**（`Lib/abc.py` 的
/// `class abstractclassmethod(classmethod)` 靠 classmethod 的类型字典项经 MRO 继承 ✓）。
fn install_static_and_classmethod_descriptors(instance: &Instance) {
    if let Some(ty) = instance.type_named("staticmethod") {
        install_type_method(
            instance,
            ty,
            "__get__",
            crate::builtin_objects::staticmethod_descriptor_get as crate::NativeFn,
        );
    }
    if let Some(ty) = instance.type_named("classmethod") {
        // **补齐基类**（`P3-17` 顺带修的真缺口 ✓）：`classmethod` 类型在引导期是
        // `alloc_type_raw` 建的，**漏了** `register_bases` ✗ ⇒ MRO 为空 ⇒
        // `type_lookup(classmethod, "__get__")` 连**自己**的命名空间都看不到 ✗
        //（参照里 `classmethod` 继承 `object` ✓；`staticmethod`／`property` 都注册了基类 ✓）。
        // 修复放在**本安装器**里（不动引导期代码 ✓）：只在 MRO 为空时补一次 ✓，幂等 ✓。
        let mro_is_empty = unsafe { ty.as_ref() }.mro().is_empty();
        if mro_is_empty {
            if let Some(object_type) = instance.type_named("object") {
                instance.register_bases(ty, vec![object_type]);
            }
        }
        install_type_method(
            instance,
            ty,
            "__get__",
            crate::builtin_objects::classmethod_descriptor_get as crate::NativeFn,
        );
    }
}

/// **`P3-17` 同族 ＋ `functools.wraps.__name__` 正路的幂等安装器**（每个 `Instance` 各装一次 ✓）。
///
/// 调用点：`pyawa-stdlib` 的 `builtins_module::build`（`OM-15`：类型表按实例存放 ✓，
/// 所以**不能**做成进程级一次性 ✓）。
pub fn install_builtin_descriptors(instance: &Instance) {
    install_function_attribute_descriptors(instance);
    install_property_descriptors(instance);
    install_static_and_classmethod_descriptors(instance);
}
