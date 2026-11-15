//! **`protocol` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `protocol_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`line_at_offset`、`render_label`、`set_items_of`、`type_name_of` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::HotId;
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
use crate::builtin_objects::TupleObject;
use crate::type_object::TypeObject;
use crate::executor::arithmetic_public;
use crate::executor::attribute_lookup;
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
    let slice_type = instance.hot_type(HotId::Slice).expect("slice 已登记");
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
    // **哈希索引查**（round 250 性能，A2-5）：先前 `find_live_position` 对**每个**全局/内建名
    // 都线性扫整个映射 ✗（`str_matches_public` 因此进热榜）。`dict_get` 走 `hash_text`＋桶 ✓。
    let value = instance.dict_get(mapping, name)?;
    // **借用**：value 由字典持有 ✓（`P3-41` round 43 修 ✗）：先前这里 **incref** 而
    // 调用点一律 `push(instance, frame, value)`（`push` 自己会 `own()` 再加一份 ✓）⇒
    // **每次名字加载漏一份引用** ⇒ 对象永不释放（实测 `c = a` 循环 100 次 ⇒ `a` 的 refcount +100；
    // `test_weakref` 因此把 70000 个 `DummyKey`／`DummyValue` 全留在堆上、GC 日志 `freed≈0`、
    // `live` 涨到 66.7 万 ⇒ 单次 `gc.collect` 372 ms ⇒ 整文件超时 ✗）。
    // 改成借用后：调用点 `push` 照旧（它自己 retain ✓）、需要自己那份的地方显式 `incref` ✓
    //（`LOAD_ASSERTION_ERROR` 那格本来就是这么写的 ✓）。
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
            instance.hot_type(HotId::Dict).expect("dict 已登记"),
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
    // **异常对象的"计算属性"写入校验**（round 239）：参照里 `args`／`__cause__`／`__context__` 都是
    // **属性**（不是实例字典项 ✓），且各有类型校验 ✓；先前一律当普通属性写进实例字典 ✗ ⇒
    // `e.args = 1`／`e.__cause__ = 1` 静默成功 ✗（`test_exceptions.ExceptionTests.test_invalid_setattr` ✓）。
    // SAFETY: object 是存活对象。
    let object_ty = unsafe { object.as_ref() }.ty();
    if crate::executor::values::is_exception_type(instance, object_ty) {
        // **载荷形状核验**（round 239）：只有真正的 `ExceptionObject` 布局才能碰载荷
        //（`binascii.Error` 一类**用户建类**的异常子类可能走通用分配 ⇒ 强转读垃圾 ⇒ SIGSEGV ✗）。
        let is_exception_payload = unsafe { object_ty.as_ref() }.instance_size()
            == core::mem::size_of::<crate::builtin_objects::ExceptionObject>();
        if !is_exception_payload && crate::diag::flag_site!("PYAWA_EXC_PAYLOAD_DEBUG") {
            eprintln!("[dbg] non-payload exception name={name}");
        }
        if is_exception_payload {
        match name {
            "args" => {
                // 参照：`args` 接受**任意可迭代对象**并转成 tuple ✓（不可迭代 ⇒ `TypeError: 'int'
                // object is not iterable` ✓，由 `iter_object` 的原话给出 ✓）。
                let items: Vec<NonNull<Header>> = if let Some(items) = instance.tuple_items(value) {
                    items.iter().map(|item| instance.retain(*item)).collect()
                } else {
                    let iterator = instance.iter_object(value)?;
                    let mut items = Vec::new();
                    loop {
                        match instance.advance_iterator(iterator) {
                            Ok(Some(item)) => items.push(item),
                            Ok(None) => break,
                            Err(error) => {
                                instance.release(iterator);
                                return Err(error);
                            }
                        }
                    }
                    instance.release(iterator);
                    items
                };
                // SAFETY: 类型身份已确认，载荷是 `ExceptionObject`。
                let payload = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::ExceptionObject>() };
                let old = payload.replace_args(items);
                for item in old {
                    instance.release(item);
                }
                if let Some(tuple) = payload.take_args_tuple() {
                    instance.release(tuple);
                }
                return Ok(());
            }
            "__cause__" | "__context__" => {
                // SAFETY: 类型身份已确认，载荷是 `ExceptionObject`。
                let payload = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::ExceptionObject>() };
                let is_none = value == instance.singletons().none();
                // SAFETY: value 是存活对象。
                let value_ty = unsafe { value.as_ref() }.ty();
                let base = instance.hot_type(HotId::BaseException)
                    .expect("BaseException 在引导期已登记");
                if !is_none && !instance.is_subtype(value_ty, base) {
                    let word = if name == "__cause__" { "cause" } else { "context" };
                    return Err(raise_builtin(
                        instance,
                        "TypeError",
                        &format!("exception {word} must be None or derive from BaseException"),
                    ));
                }
                // 调用方给的是**借用** ⇒ payload 要自己那份。
                // SAFETY: value 存活。
                unsafe { instance.incref_object(value.as_ptr()) };
                let old = if name == "__cause__" {
                    payload.set_cause(if is_none { None } else { Some(value) })
                } else {
                    payload.set_context(if is_none { None } else { Some(value) })
                };
                if let Some(old) = old {
                    // SAFETY: 被顶下来的那份由本函数消费。
                    unsafe { instance.release_object(old.as_ptr()) };
                }
                return Ok(());
            }
            _ => {}
        }
        }
    }

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
            if unsafe { value.as_ref() }.ty() != instance.hot_type(HotId::Dict).expect("dict 已登记") {
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
        // **类型属性的写校验**（task-13 #d，把"没有 `getset.__set__`"的差距收窄到**存储**一处）：
        // 参照 3.14.4 实测——
        // - 静态（内建/扩展）类型的 `__name__`／`__qualname__`／`__module__` **不可写** ⇒
        //   `cannot set '<name>' attribute of immutable type '<类型名>'`（`type` 自己例外：它是 heap 型 ✓）；
        // - 用户类（heap type）的 `__name__`／`__qualname__` 只收 `str` ⇒
        //   `can only assign string to C.__name__, not 'int'`；`__module__` 不校验类型 ✓。
        // 读侧口径在 `Instance::type_name_value`／`type_qualname_value`（**一处真相**）。
        let target_type = object.cast::<crate::TypeObject>();
        let is_metatype = target_type == instance.metatype();
        if matches!(name, "__name__" | "__qualname__" | "__module__")
            && !is_metatype
            && !instance.is_user_type_object(target_type)
        {
            let current = instance.type_name_value(object);
            release(instance, value);
            return Err(raise_builtin(
                instance,
                "TypeError",
                &format!("cannot set '{name}' attribute of immutable type '{current}'"),
            ));
        }
        if name == "__name__" || name == "__qualname__" {
            if instance.text_of(value).is_none() {
                let actual = instance.type_name(instance.type_of(value));
                let current = instance.type_name_value(object);
                release(instance, value);
                return Err(raise_builtin(
                    instance,
                    "TypeError",
                    &format!("can only assign string to {current}.{name}, not '{actual}'"),
                ));
            }
        }
        let Some(namespace) = instance.type_namespace(object) else {
            release(instance, value);
            return Err(raise_builtin(instance, "TypeError", "类型对象没有命名空间"));
        };
        // **`__qualname__` 的存放位置**（task-13 #d）：参照 3.14 把它收进类型槽、类字典里没有这个
        // 公开键 ⇒ 本层存私有键（读侧 `Instance::type_qualname_value` 一处真相）；rename 后
        // `inspect.getattr_static` 仍能拿到 `type.__dict__` 的描述符。
        let storage_name = if name == "__qualname__" {
            crate::classes::TYPE_QUALNAME_KEY
        } else if name == "__name__" {
            // 同上：rename 覆盖值不进类字典（读侧 `Instance::type_name_value` 一处真相）。
            crate::classes::TYPE_NAME_KEY
        } else {
            name
        };
        // **哈希索引写**（round 250 性能，A2-5）：接管 `value`、原位替换/末尾插入 ✓。
        instance.dict_set_owned_by_name(namespace, storage_name, value);
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
    // **`__slots__` 强制**（round 147）：有槽名表 ⇒ 写非槽名要报参照原话
    // `'X' object has no attribute 'y' and no __dict__ for setting new attributes` ✓
    // （`test_descrtut` tut2 的 `a.x1 = 1` ✓；数据描述符那条在上面已先走 ✓）。
    // **只对用户定义类生效**（round 147 真 bug 修 ✗）：把这条用在**引擎内部对象**（异常／traceback
    // 一类，写属性是 `attach_traceback` 的内部动作）会触发"报错 → 挂 traceback → 又写属性 → 又报错"
    // 的**无限递归**（实测 gdb 栈 20+ 层 `instance_attribute_set → raise_builtin → attach_traceback` ✓）。
    let type_ptr = header.ty().cast::<crate::TypeObject>();
    // **判据换成"类型命名空间里有 `__slots__`"**（round 218）：`namedtuple` 是 `type()` 三参
    // 造的，不在"用户类"登记表里 ✗ ⇒ 先前 `is_user_type_object` 把它挡掉 ⇒ 只读属性限制失效
    // ✗（`p.x = 5` 能成功 ✓）。内建／引擎内部对象没有 `__slots__` 名字 ⇒ 不受影响 ✓。
    // SAFETY: type_ptr 由注册表持有、存活。
    let type_ref = unsafe { type_ptr.as_ref() };
    // **`__slots__` 检查缓存**（round 250 性能，A2-5）：答案在类型创建后不变 ⇒ 查一次记类型位 ✓
    //（先前**每次属性写**都要 `type_namespace`＋`dict_get("__slots__")`：哈希＋桶分配 ✗）。
    let declares_slots = match type_ref.slots_declaration() {
        Some(declared) => declared,
        None => {
            // **保留 `type_namespace` 的惰性创建副作用**（与旧口径一致 ✓）。
            let declared = instance
                .type_namespace(type_ptr.cast())
                .is_some_and(|namespace| instance.dict_get(namespace, "__slots__").is_some());
            type_ref.mark_slots_declaration(declared);
            declared
        }
    };
    if name != "__dict__" && declares_slots {
        if let Some(in_slot) = instance.type_slot_contains(type_ptr, name) {
            if !in_slot {
                release(instance, value);
                let message = format!(
                    "'{}' object has no attribute '{name}' and no __dict__ for setting new attributes",
                    type_object.name()
                );
                return Err(raise_builtin(instance, "AttributeError", &message));
            }
        }
    }
    // **`AB-73`／`AB-75`**：宿主对象的**属性回退写**——普通规则（类型数据描述符/`__slots__` 等）
    // 已处理完，默认实例字典写之前给宿主一次机会（自身条目 → 类型态条目，统一入口）。
    // `None`／`Ok(false)` ＝ 未处理（继续默认）；`Err` 原样上抛。
    if let Some(handled) = instance.host_object_setattr(object, name, Some(value))? {
        if handled {
            // 宿主已处理：传给它的那份值是**借用**（AB-15）⇒ 这里归还；
            // 写入本身由宿主负责（载荷归宿主逻辑管理，AB-58）。
            // SAFETY: value 是调用方交出的新引用。
            unsafe { instance.release_object(value.as_ptr()) };
            return Ok(());
        }
    }

    // **`obj.__dict__ = {…}`**（实测）：**整体替换**挂载的字典；值不是字典就是实测那条
    // `TypeError: __dict__ must be set to a dictionary, not a 'int'`。
    if name == "__dict__" && has_instance_dict {
        let is_dict = unsafe { value.as_ref() }.ty() == instance.hot_type(HotId::Dict).expect("dict 已登记");
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
    // **哈希索引写**（round 250 性能，A2-5）：属性写不再线性扫实例字典 ✓。
    instance.dict_set_owned_by_name(mapping, name, value);
    let _ = opcode;
    Ok(())
}

/// 删掉实例属性字典里的一项（`DELETE_ATTR`；参照实现只删实例属性，不碰类型）。
pub fn instance_attribute_delete(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<(), ExecError> {
    instance_attribute_delete_inner(instance, object, name, false)
}

/// [`instance_attribute_delete`] 的**跳过"用户 `__delattr__` 优先"**版本（round 95）：
/// `object.__delattr__` 的默认实现要走到"删实例字典"这条底层路 ✓ —— 它本身已经处在
/// `__delattr__` 分派**之内** ✗，再问一次用户钩子就会**无限递归** ✗
/// （实测 `class A: def __delattr__(...): object.__delattr__(self, name)` ⇒ RecursionError ✗）。
pub(crate) fn instance_attribute_delete_default(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<(), ExecError> {
    instance_attribute_delete_inner(instance, object, name, true)
}

fn instance_attribute_delete_inner(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    skip_user_hook: bool,
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
    // **用户 `__delattr__` 优先**（round 95）：参照里 `del obj.attr` = `type(obj).__delattr__(obj, name)` ✓
    // —— 先前直接删实例字典 ✗ ⇒ `unittest.mock` 的 `MagicMock.__delattr__`（清理 `_mock_children` 等）
    // 根本不被调 ⇒ `_mock_set_magics` 的 `delattr(self, entry)` 抛 `AttributeError: 'MagicMock' object
    // has no attribute '__lt__'` ✗（官方 `test__colorize` 末条 ✓）。
    // 守卫：`__delattr__` 自身不再递归 ✓；**`object` 的默认实现**不在这里调（它就是"删实例字典"，即本函数下文 ✓）。
    if !skip_user_hook && name != "__delattr__" {
        // SAFETY: object 是存活对象。
        let object_type = unsafe { object.as_ref() }.ty();
        let object_default = instance.hot_type(HotId::Object);
        if let Some((owner, hook)) = instance.type_lookup_owner(object_type, "__delattr__") {
            let is_object_default = object_default.is_some_and(|default| default == owner);
            if !is_object_default {
                let name_object = instance.new_str(name);
                let returned = call_callable(
                    instance,
                    hook,
                    Some(object),
                    vec![name_object],
                    Vec::new(),
                    0,
                );
                // 形参所有权（round 98 起默认）：帧已归还实参 ⇒ 不再补这一份（`PYAWA_FRAME_OWN=0` 才补）。
                if !crate::instance::frame_own_params() {
                    instance.release(name_object);
                }
                release(instance, returned?);
                return Ok(());
            }
        }
    }
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
    // **类型属性的删除**（task-13 #d）：参照里 `del C.__name__` 一律 TypeError ——
    // heap type（用户类）⇒ `cannot delete '<name>' attribute of immutable type '<名>'`；
    // 静态类型（含 `type` 自己）⇒ `cannot set '<name>' attribute of immutable type '<名>'`（实测原话是 set）。
    if instance.is_type_object(object) && matches!(name, "__name__" | "__qualname__" | "__module__") {
        let current = instance.type_name_value(object);
        let target_type = object.cast::<crate::TypeObject>();
        if instance.is_user_type_object(target_type) {
            return Err(raise_builtin(
                instance,
                "TypeError",
                &format!("cannot delete '{name}' attribute of immutable type '{current}'"),
            ));
        }
        return Err(raise_builtin(
            instance,
            "TypeError",
            &format!("cannot set '{name}' attribute of immutable type '{current}'"),
        ));
    }
    let mapping = instance_attributes(instance, object).ok_or_else(missing)?;
    // **哈希索引删**（round 250 性能，A2-5）：不再线性扫 ✓。
    let (key, value) = instance.dict_remove_by_name(mapping, name).ok_or_else(missing)?;
    release(instance, key);
    release(instance, value);
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
/// **就地运算的用户 dunder 派发**（round 109，PEP 203 口径）：`x op= y` 先看
/// `type(x)` 的 MRO 上有没有 `__iop__`（**不含 `object` 默认**——它本来就不定义这些）；
/// 有就调：返回 `NotImplemented` ⇒ 落回"基运算 + 重新绑定" ✓；没有 ⇒ `None`（调用方走快路）✓。
fn inplace_dunder(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    dunder: &str,
) -> Result<Option<NonNull<Header>>, ExecError> {
    // **宿主对象**：类型 MRO 没有 `__iadd__` 等条目，但 `getattr_fn` 可能就是唯一实现。
    if !(instance.has_host_object_ops(left)
        || resolve_dunder(instance, left, dunder).overrides(instance))
    {
        return Ok(None);
    }
    // 容器快路（list／set／dict）没在类型字典里登记这些 dunder ⇒ 到这里只会是**用户覆写** ✓。
    let Some(result) = call_protocol_method(instance, left, dunder, &[right])? else {
        return Ok(None);
    };
    if result == instance.not_implemented() {
        // SAFETY: result 是新引用。
        unsafe { instance.release_object(result.as_ptr()) };
        return Ok(None);
    }
    Ok(Some(result))
}

/// 符号 ⇒ 就地 dunder 名（`None` ⇒ 该符号没有就地协议 ✓）。
fn inplace_dunder_name(symbol: &str) -> Option<&'static str> {
    Some(match symbol {
        "+" => "__iadd__",
        "-" => "__isub__",
        "*" => "__imul__",
        "/" => "__itruediv__",
        "//" => "__ifloordiv__",
        "%" => "__imod__",
        "**" => "__ipow__",
        "<<" => "__ilshift__",
        ">>" => "__irshift__",
        "&" => "__iand__",
        "|" => "__ior__",
        "^" => "__ixor__",
        "@" => "__imatmul__",
        _ => return None,
    })
}

pub(crate) fn inplace_add(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // **list 的 `+=` 特殊口径**（round 114，照参照实测）：左侧是 **list 族且 `__iadd__` 的落地类型是
    // 内建 `list`**（用户没覆写）时——右侧是 list／tuple ⇒ 就地扩展 ✓；**否则回退 `+` 的反射协议** ✓
    // （`l = list('spam'); l += UserList('eggs')` ⇒ 结果类型 **UserList** ✓、`l += 'eggs'` ⇒ TypeError ✓；
    // 先前统一先调 `list.__iadd__`（收任意可迭代）⇒ 就地扩展、类型仍是 list ✗）。
    let left_ty = unsafe { left.as_ref() }.ty();
    let list_ty = instance.hot_type(HotId::List);
    let builtin_list_iadd = list_ty.is_some_and(|list_ty| {
        instance.is_subtype(left_ty, list_ty)
            && match instance.type_lookup_owner(left_ty, "__iadd__") {
                // `list` 的 `__iadd__` **不在类型字典里**（走本层的 dunder 分派表）⇒ `None` 也是内建口径 ✓；
                // 用户子类显式覆写 ⇒ owner 是子类 ⇒ 走通用 dunder 派发 ✓。
                Some((owner, _)) => owner == list_ty,
                None => true,
            }
    });
    if builtin_list_iadd {
        let right_ty = unsafe { right.as_ref() }.ty();
        let is_list = list_ty.is_some_and(|list_ty| instance.is_subtype(right_ty, list_ty));
        let is_tuple = instance.hot_type(HotId::Tuple)
            .is_some_and(|tuple_ty| instance.is_subtype(right_ty, tuple_ty));
        // **右侧自带用户 `__radd__` ⇒ 走 `+` 的反射协议**（参照实测：`list += UserList("x")`／
        // `list += R()`（R 有 `__radd__`）⇒ **反射结果胜出**；没有 `__radd__` 的可迭代 ⇒ 就地扩展 ✓）。
        let user_radd = instance
            .type_lookup_owner(right_ty, "__radd__")
            .is_some_and(|(owner, _)| {
                !instance.hot_type_is_any(
                    owner,
                    &[
                        HotId::Int,
                        HotId::Bool,
                        HotId::Float,
                        HotId::Complex,
                        HotId::Str,
                        HotId::Bytes,
                        HotId::List,
                        HotId::Tuple,
                        HotId::Dict,
                    ],
                )
            });
        if !is_list && !is_tuple && user_radd {
            return concat_public(instance, left, right, opcode);
        }
    }
    // **用户 `__iadd__` 优先**（round 109）：`x += y` 先问 `type(x).__iadd__` ✓。
    if let Some(result) = inplace_dunder(instance, left, right, "__iadd__")? {
        return Ok(result);
    }
    if Some(unsafe { left.as_ref() }.ty()) == instance.hot_type(HotId::List) {
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
        let Some(key_items) = instance.iterable_items(keys)? else {
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
    // **用户 `__iop__` 优先**（round 109，PEP 203）：有覆写就调；`NotImplemented` 才落回基运算 ✓。
    if let Some(dunder) = inplace_dunder_name(symbol) {
        if let Some(result) = inplace_dunder(instance, left, right, dunder)? {
            return Ok(result);
        }
    }
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
        // **`*=`（就地重复）**（round 105）：参照对 `list`／`bytearray` 的 `*=` 都是**就地**扩展 ✓
        // （保持对象身份 ✓）。官方 `test_list` 的 `test_imul` 一族 ✓ —— 先前如实报未接线 ✗。
        if symbol == "*" {
            let count = instance.int_of(right).map(|value| value.to_i64().unwrap_or(0)).unwrap_or(0);
            let count = count.max(0) as usize;
            if subtype_of("list") {
                // SAFETY: 类型身份刚确认是 list 族（载荷同构 ✓）。
                let list = unsafe {
                    &*left.as_ptr().cast::<crate::builtin_objects::ListObject>()
                };
                let original = list.take_items();
                // **上限守卫**（round 105，照 `sequence_repeat` 的同一口径 ✓）：`lst *= sys.maxsize`
                // 参照报 `MemoryError`／`OverflowError` ✓ —— 先前直接 `Vec::with_capacity` ⇒
                // **容量溢出 panic** ✗（官方 `test_list.test_list_resize_overflow` ✓）。
                if original.len().saturating_mul(count) > crate::executor::MAX_REPEAT_ITEMS {
                    for item in original {
                        release(instance, item);
                    }
                    return Err(instance.raise_builtin_error(
                        "MemoryError",
                        "sequence repetition too large",
                    ));
                }
                let mut repeated: Vec<NonNull<Header>> =
                    Vec::with_capacity(original.len().saturating_mul(count));
                for _ in 0..count {
                    for item in &original {
                        // SAFETY: 元素由列表持有，存活；重复项各要一份。
                        unsafe { instance.incref_object(item.as_ptr()) };
                        repeated.push(*item);
                    }
                }
                for item in original {
                    release(instance, item);
                }
                list.extend(repeated);
                return Ok(instance.retain(left));
            }
            // `bytearray *= n`
            // SAFETY: 类型身份刚确认。
            let data = unsafe {
                &*left
                    .as_ptr()
                    .cast::<crate::builtin_objects::BytearrayObject>()
            };
            let mut value = data.value();
            let original = value.clone();
            if original.len().saturating_mul(count) > crate::executor::MAX_REPEAT_BYTES {
                return Err(instance.raise_builtin_error(
                    "MemoryError",
                    "bytes repetition too large",
                ));
            }
            value.clear();
            for _ in 0..count {
                value.extend_from_slice(&original);
            }
            drop(value);
            return Ok(instance.retain(left));
        }
        return Err(unsupported_operand(instance, left, right, &format!("{symbol}=")));
    }
    arithmetic_public(instance, left, right, symbol, opcode)
}


/// **内建容器自己的比较实现**（`dict.__eq__` 等）：`overrides_compare` 的"跳过"判据。
///
/// 它们在本层走**容器递归／原生路径**（[`equality_public`] 的 list／tuple／dict 分支、
/// [`compare_public`] 的 set 分支），**不**按用户富比较路径派发 —— 否则继承来的容器实现会把
/// 用户子类的覆写遮住、或把容器递归绕开 ✗。
const BUILTIN_CONTAINER_OWNERS: [HotId; 8] = [
    HotId::List,
    HotId::Tuple,
    HotId::Dict,
    HotId::Set,
    HotId::FrozenSet,
    HotId::Str,
    HotId::Bytes,
    HotId::ByteArray,
];

/// **一个协议 dunder 的统一解析结果**（`PLAN §12.12` 步 2；`SPEC-c-abi` §17「语义唯一」）。
///
/// 一处回答三个问题：
/// - **是否自有**：`Owned`（owner 不是 `object` 的默认实现）⇒ 可以派发；
/// - **是否应跳过富比较派发**：[`DunderResolution::overrides`] 为假（`dict.__eq__` 等内建容器
///   自己的实现）⇒ 走容器递归／原生路径；
/// - **解析到的函数**：[`DunderResolution::function`]（`Default` 也带着 ⇒ `has_custom_eq` 可
///   与 `object.__eq__` 逐值比对）。
///
/// 解析**只看类型 MRO**（与参照的特殊方法查找口径一致 ✓）：不碰实例字典、不触发
/// `__getattribute__`／`__getattr__` ⇒ 纯查询可以安全放进热路径的快路判定；
/// 真正的调用由 [`call_special_method`] 走统一属性通道完成（**唯一调用入口** ✓）。
pub(crate) enum DunderResolution {
    /// 类型 MRO 上没有这个名字（连 `object` 默认实现也没有）。
    Missing,
    /// 解析到 `object` 的默认实现 ⇒ **不是自有**，富比较路径不得派发。
    Default { function: NonNull<Header> },
    /// 解析到某个非 `object` 类型（`owner`）的自有实现。
    Owned {
        owner: NonNull<TypeObject>,
        function: NonNull<Header>,
    },
}

impl DunderResolution {
    /// **自有**（owner 不是 `object` 默认实现）？—— 原 `type_has_own_dunder` 的判据。
    #[inline]
    pub(crate) fn is_owned(&self) -> bool {
        matches!(self, DunderResolution::Owned { .. })
    }

    /// **内建容器自己的实现**？—— 原 `overrides_compare` 里"跳过"那一半的判据。
    #[inline]
    pub(crate) fn is_builtin_container(&self, instance: &Instance) -> bool {
        match self {
            DunderResolution::Owned { owner, .. } => {
                instance.hot_type_is_any(*owner, &BUILTIN_CONTAINER_OWNERS)
            }
            _ => false,
        }
    }

    /// **按用户富比较派发**？—— 原 `overrides_compare` 的完整判据（自有且不是内建容器）。
    #[inline]
    fn overrides(&self, instance: &Instance) -> bool {
        self.is_owned() && !self.is_builtin_container(instance)
    }

    /// 解析到的函数（`Missing` ⇒ `None`）。
    #[inline]
    fn function(&self) -> Option<NonNull<Header>> {
        match self {
            DunderResolution::Missing => None,
            DunderResolution::Default { function } | DunderResolution::Owned { function, .. } => {
                Some(*function)
            }
        }
    }
}

/// **统一解析一个协议 dunder**（`PLAN §12.12` 步 2）。
///
/// `object` 是**接收者**（实例或类对象）；`name` 是 dunder 名（如 `"__eq__"`）。解析沿
/// `type(object)` 的类型 MRO（类对象即元类 MRO ✓，与 [`call_special_method`] 的类对象口径
/// 一致 ✓），**不**走属性通道 ⇒ 不产生副作用、不会被重复调用 ✗。
pub(crate) fn resolve_dunder(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> DunderResolution {
    let ty = instance.type_of(object);
    let Some((owner, function)) = instance.type_lookup_owner(ty, name) else {
        return DunderResolution::Missing;
    };
    if Some(owner) == instance.hot_type(HotId::Object) {
        return DunderResolution::Default { function };
    }
    DunderResolution::Owned { owner, function }
}

/// **类型的"自有" dunder**（`__eq__`／`__ne__`）：只有**不是 `object` 默认实现**才算 ✓。
///
/// `object.__eq__`／`__ne__` 在本层是挂上去的原生实现（转调 `values_equal`）⇒ 若无条件按
/// dunder 派发就会**无限递归** ⇒ 必须先看 owner ✓。判据已收敛到 [`resolve_dunder`]。
#[inline]
fn type_has_own_dunder(instance: &Instance, object: NonNull<Header>, name: &str) -> bool {
    resolve_dunder(instance, object, name).is_owned()
}

/// 调一次比较 dunder（`__eq__`／`__ne__`）：没有、或返回 `NotImplemented` ⇒ `None` ✓（交给下一跳）。
fn call_compare_dunder(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    other: NonNull<Header>,
) -> Result<Option<bool>, ExecError> {
    if !type_has_own_dunder(instance, object, name) && !instance.has_host_object_ops(object) {
        return Ok(None);
    }
    let Some(result) = call_protocol_method(instance, object, name, &[other])? else {
        return Ok(None);
    };
    if result == instance.not_implemented() {
        release(instance, result);
        return Ok(None);
    }
    let truth = instance.truthiness_of(result);
    release(instance, result);
    Ok(Some(truth?))
}

/// **富比较派发结果**（round 139）：`Value`＝拿到用户 dunder 的结果对象（直接当表达式值）；
/// `Fallback`＝**已经派发过**、两侧都 `NotImplemented` ⇒ 调用方按参照兜底（`==`/`!=` 身份判定、
/// 大小比较 `TypeError`），**不得再走 `compare_public`**（否则用户 dunder 会被重复调用 ✗，
/// `test_binop.test_comparison_orders` 就是数调用次数的 ✓）。
pub enum RichCompare {
    Value(NonNull<Header>),
    Fallback,
}

/// **相等比较的"富结果"语义**（round 138）：参照的 `==`／`!=` 也**返回用户 `__eq__`／`__ne__` 的结果对象**
/// （`test_richcmp.Vector` 的 `a == b` 返回 Vector ✓）；`None` ⇒ 走既有 `equality_public`（bool）兜底 ✓。
pub fn equality_compare_result(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<Option<RichCompare>, ExecError> {
    let (direct, reflected) = match symbol {
        "==" => ("__eq__", "__eq__"),
        // **`!=` 的富路径只看"自己的 `__ne__`"**（round 139 真 bug 修 ✗）：没有自己 `__ne__` 的类
        // 走 `object.__ne__` 默认（委托 `__eq__` 再取反 ⇒ **bool** ✓），这条语义在 `compare_public`
        // 里已经完整（含"左侧默认 ne 先命中、不会先碰右侧 `__ne__ = None`"的次序 ✓）⇒ 这里返回
        // `None` 交回去 ✓（否则 `e != xn` 会错误地先调 `XN.__ne__ = None` 抛 TypeError ✗）。
        "!=" => ("__ne__", "__ne__"),
        _ => return Ok(None),
    };

    let left_type = instance.type_of(left);
    let right_type = instance.type_of(right);
    let right_first = right_type != left_type && instance.is_subtype(right_type, left_type);
    let order: [(&str, NonNull<Header>, NonNull<Header>); 2] = if right_first {
        [(reflected, right, left), (direct, left, right)]
    } else {
        [(direct, left, right), (reflected, right, left)]
    };
    // **`!=` 的富路径只在自己 `__ne__` 会**先命中**时启用**（round 139 修正）：
    // 参照次序里，若首个操作数**没有自己的 `__ne__` 但有 `__eq__`** ⇒ `object.__ne__` 的默认
    // "委托 `__eq__` 再取反"会先给出结果（不会再问右侧 `__ne__`）⇒ 交回 `compare_public` ✓；
    // 顺序扫描 order：先碰到"自己的 `__ne__`" ⇒ 用富路径；先碰到"自己的 `__eq__`" ⇒ `None`。
    if symbol == "!=" {
        for (_name, operand, _other) in order {
            if type_has_own_dunder(instance, operand, "__ne__") {
                break;
            }
            if type_has_own_dunder(instance, operand, "__eq__") {
                return Ok(None);
            }
        }
    }
    let mut attempted = false;
    // **有"内建一侧"被跳过**（round 215 真 bug 修 ✗）：`Counter(...) == dict` 里 `Counter.__eq__`
    // 返回 `NotImplemented` 后，参照会问**右侧内建 `dict.__eq__`**（返回 True ✓）；先前把内建侧
    // 直接 `continue` ✗、最后又给 `Fallback`（身份兜底）⇒ 判 False ✗（`test_collections`
    // `TestCounter.test_basics` 实测 `Counter(...) != {...}` ✓）。⇒ 记录"跳过过内建侧"，
    // 此时**返回 `None`**（交回 `compare_public`／`equality_public` 走完整协议 ✓）。
    let mut skipped_builtin = false;
    for (name, operand, other) in order {
        // **只对用户定义类启用富结果**（round 138 真 bug 修）：内建容器（`dict` 的
        // `test_equal_operator_modifying_operand` 会在 `__eq__` 里改字典 ✓）仍走既有
        // `equality_public` 的结构比较路径 ✗ ⇒ 否则会踩到"比较中途改动"的旧假设而崩 ✗。
        if !instance.is_user_type_object(instance.type_of(operand)) {
            skipped_builtin = true;
            continue;
        }
        if name == "__ne__" && !type_has_own_dunder(instance, operand, name) {
            // **`!=` 的 object.__ne__ 默认**（round 150 真 bug 修 ✗）：自己没有 `__ne__`、
            // 但有 `__eq__` ⇒ 参照走"委托 `__eq__` 再取反"（**只问这个操作数自己的 `__eq__`** ✓；
            // `test_compare.test_ne_low_priority` 钉住：`Base() != Derived()` 期望
            // `['Derived.__ne__', 'Base.__eq__']` —— 先前这里直接跳过 ⇒ 少一次 `Base.__eq__` ✗）。
            if type_has_own_dunder(instance, operand, "__eq__") {
                attempted = true;
                if let Some(result) = call_special_method(instance, operand, "__eq__", &[other])? {
                    if result == instance.not_implemented() {
                        release(instance, result);
                    } else {
                        let truth = instance.truthiness_of(result);
                        release(instance, result);
                        let negated = instance.new_bool(!truth?);
                        return Ok(Some(RichCompare::Value(negated)));
                    }
                }
            }
            continue;
        }
        if !type_has_own_dunder(instance, operand, name) {
            continue;
        }
        attempted = true;
        let Some(result) = call_protocol_method(instance, operand, name, &[other])? else {
            continue;
        };
        if result == instance.not_implemented() {
            release(instance, result);
            continue;
        }
        let _ = opcode;
        return Ok(Some(RichCompare::Value(result)));
    }
    Ok(if attempted && !skipped_builtin {
        Some(RichCompare::Fallback)
    } else {
        None
    })
}

/// **大小比较的"富结果"语义**（round 138，P3-42）：参照的 `<`／`<=`／`>`／`>=` **返回用户
/// `__lt__` 等方法的结果对象**（不是强制 bool ✓）——`test_richcmp.Vector` 就靠它做逐元素向量比较 ✓。
/// 返回 `Some(轻结果)` ⇒ 调用方直接把它当表达式结果；`None` ⇒ 按既有 `compare_public`（bool）兜底 ✓。
///
/// 派发口径与参照 `do_richcompare` 同：子类优先（右侧类型是左侧子类 ⇒ 反射先 ✓）；两侧都没有
/// `NotImplemented` 之外的结果 ⇒ `None` ✓（由 `compare_public` 报原有的 `TypeError` ✓）。
pub fn order_compare_result(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<Option<RichCompare>, ExecError> {
    let (direct, reflected) = match symbol {
        "<" => ("__lt__", "__gt__"),
        "<=" => ("__le__", "__ge__"),
        ">" => ("__gt__", "__lt__"),
        ">=" => ("__ge__", "__le__"),
        _ => return Ok(None),
    };
    let left_type = instance.type_of(left);
    let right_type = instance.type_of(right);
    let right_first = right_type != left_type && instance.is_subtype(right_type, left_type);
    let order: [(&str, NonNull<Header>, NonNull<Header>); 2] = if right_first {
        [(reflected, right, left), (direct, left, right)]
    } else {
        [(direct, left, right), (reflected, right, left)]
    };
    let mut attempted = false;
    for (name, operand, other) in order {
        // **只派发"自己的"实现** ✓（`object` 的默认 `__lt__` 只返回 `NotImplemented` ✓；
        // 内建容器自己的实现走容器递归／原生路径 ✓）。
        if !(resolve_dunder(instance, operand, name).overrides(instance)
            || instance.has_host_object_ops(operand))
        {
            continue;
        }
        attempted = true;
        let Some(result) = call_protocol_method(instance, operand, name, &[other])? else {
            continue;
        };
        if result == instance.not_implemented() {
            release(instance, result);
            continue;
        }
        let _ = opcode;
        return Ok(Some(RichCompare::Value(result)));
    }
    Ok(if attempted {
        Some(RichCompare::Fallback)
    } else {
        None
    })
}

/// **`==` 的完整语义**（`P3-42` 特殊方法派发，round 44）：用户类先问 `__eq__`；容器**逐元素**
/// 递归（元素上的 `__eq__` 同样生效 ✓）；最后退回结构相等（`values_equal`）。
///
/// 为什么必须有这一层：先前 `==` 一直是"纯结构" ⇒ `UserString('ab') == 'ab'` 为 **False** ✗
/// （参照 True ✓）⇒ `test_userstring` 一行断言全红、`unittest.assertEqual` 也跟着失真 ✓。
#[inline]
fn overrides_compare(instance: &Instance, object: NonNull<Header>, name: &str) -> bool {
    // **宿主对象**没有类型 MRO 条目，但 `getattr_fn` 可能就是唯一实现（`AB-73`/`AB-75`）；
    // 有宿主钩子 ⇒ 一律尝试属性通道（`call_special_method`），`PA_ERR_NOTFOUND` 会自然落回默认。
    resolve_dunder(instance, object, name).overrides(instance)
        || instance.has_host_object_ops(object)
}

/// 这个值**自己**覆写了 `__eq__`（不是 `object` 的默认实现）吗？（round 66：容器去重要按参照的
/// **完整相等协议**走用户 `__eq__` ✓ —— 只在覆写时才走可失败路径 ✓，纯内建值仍走快路 ✓）。
///
/// 判据已收敛到 [`resolve_dunder`]：解析到的**函数值**与 `object.__eq__` 不同 ⇒ 自定义
/// （这与"owner 不是 `object`"略有差别：显式 `__eq__ = object.__eq__` 的类在此**不算**自定义，
/// 保持原有行为 ✓）。
pub(crate) fn has_custom_eq(instance: &Instance, value: NonNull<Header>) -> bool {
    let object_eq = instance
        .hot_type(HotId::Object)
        .and_then(|object| instance.type_lookup(object, "__eq__"));
    resolve_dunder(instance, value, "__eq__")
        .function()
        .is_some_and(|found| Some(found) != object_eq)
}

pub(crate) fn equality_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
) -> Result<bool, ExecError> {
    if left == right {
        return Ok(true);
    }
    let left_ty = instance.type_of(left);
    let right_ty = instance.type_of(right);
    let subtype = |ty: NonNull<TypeObject>, name: &str| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(ty, base))
    };
    let overridden = overrides_compare(instance, left, "__eq__")
        || overrides_compare(instance, right, "__eq__");
    // **容器递归**（用户子类覆写 `__eq__` 时让覆写优先 ✓）。
    if !overridden {
        if subtype(left_ty, "list") && subtype(right_ty, "list") {
            // **活体双游标**（round 110/111，照参照 `list_richcompare` 的循环骨架 ✓）：
            // ① 每轮重读**两侧当前长度** ✓；② 元素比较前各 `Py_INCREF` ✓；
            // ③ 比较为假就 `break`，但**退出后按"当前长度"判定**：两侧都已走完 ⇒ 相等 ✓
            //（`test_equal_operator_modifying_operand`：X/Y 的 `__eq__` 把两侧清空 ⇒ 循环因"都走完"结束
            // ⇒ 参照给 **True** ✓；先前一遇 `__eq__` 假就回 False ✗）。
            // **长度不同 ⇒ 直接不等**（round 139 真 bug 修 ✗）：参照 `list_richcompare` 的 `Py_EQ` 分支
            // 先比长度、不同立即返回 False ✓；先前**逐元素比**到首个不匹配才看长度 ⇒ 递归容器
            // （`UserList.__eq__` 的 `self.data == other.data`，两侧 data 均含对面 ⇒ 无限互递归 ✗）
            // 会一路递归到解释器深度上限 ✗（`test_richcmp.test_recursion` 实测 RecursionError 逃逸 ✓）。
            let left_len = instance.list_items(left).map(|items| items.len()).unwrap_or(0);
            let right_len = instance.list_items(right).map(|items| items.len()).unwrap_or(0);
            if left_len != right_len {
                return Ok(false);
            }
            let mut index = 0usize;
            loop {
                let left_items = instance.list_items(left).unwrap_or_default();
                let right_items = instance.list_items(right).unwrap_or_default();
                if index >= left_items.len() || index >= right_items.len() {
                    return Ok(index >= left_items.len() && index >= right_items.len());
                }
                let first = left_items[index];
                let second = right_items[index];
                // SAFETY: 两个元素由各自列表持有；比较期间本函数各持一份 ✓。
                unsafe {
                    instance.incref_object(first.as_ptr());
                    instance.incref_object(second.as_ptr());
                }
                let verdict = equality_public(instance, first, second);
                unsafe {
                    instance.release_object(first.as_ptr());
                    instance.release_object(second.as_ptr());
                }
                if !verdict? {
                    // **离开循环后按"当前长度"判**（照参照 `break` ＋ 事后比较 ✓）：
                    // `__eq__` 可能把两侧都清空 ⇒ 循环此刻已"两侧走完" ⇒ 相等 ✓
                    //（`test_equal_operator_modifying_operand` 的 list1/list2 档 ✓）。
                    let left_len = instance.list_items(left).map(|items| items.len()).unwrap_or(0);
                    let right_len = instance.list_items(right).map(|items| items.len()).unwrap_or(0);
                    return Ok(index >= left_len && index >= right_len);
                }
                index += 1;
            }
        }
        if subtype(left_ty, "tuple") && subtype(right_ty, "tuple") {
            let a = instance.tuple_items(left).unwrap_or_default();
            let b = instance.tuple_items(right).unwrap_or_default();
            if a.len() != b.len() {
                return Ok(false);
            }
            // 与 list 同口径：比较期间各持一份 ✓（元素的 `__eq__` 可能改外部容器 ✓）。
            let mut retained: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::with_capacity(a.len());
            for (first, second) in a.iter().zip(b.iter()) {
                // SAFETY: 两个元素由各自元组持有；比较期间本函数各持一份 ✓。
                unsafe {
                    instance.incref_object(first.as_ptr());
                    instance.incref_object(second.as_ptr());
                }
                retained.push((*first, *second));
            }
            let mut answer = Ok(true);
            for (first, second) in &retained {
                match equality_public(instance, *first, *second) {
                    Ok(true) => {}
                    Ok(false) => {
                        answer = Ok(false);
                        break;
                    }
                    Err(error) => {
                        answer = Err(error);
                        break;
                    }
                }
            }
            for (first, second) in retained {
                unsafe {
                    instance.release_object(first.as_ptr());
                    instance.release_object(second.as_ptr());
                }
            }
            return answer;
        }
        // **`dict`**：键用结构查（用户键的 `__eq__` 属 P3-42 后续格），**值**递归（`__eq__` 生效 ✓）。
        if subtype(left_ty, "dict") && subtype(right_ty, "dict") {
            // **活视图代理先同步快照**（round 125）；**只对 `mappingproxy` 操作数调**（round 195 真 bug 修 ✗）。
            let proxy_type = instance.hot_type(HotId::MappingProxy);
            if proxy_type.is_some_and(|proxy| instance.type_of(left) == proxy) {
                instance.sync_mappingproxy(left);
            }
            if proxy_type.is_some_and(|proxy| instance.type_of(right) == proxy) {
                instance.sync_mappingproxy(right);
            }
            let a = instance.dict_entries(left).unwrap_or_default();
            let b = instance.dict_entries(right).unwrap_or_default();
            if a.len() != b.len() {
                return Ok(false);
            }
            // **比较期间各持一份**（round 97，照 bpo-27945 的事故面 ✓）：键／值的 `__eq__`／`__del__`
            // 可能在比较中途**清空任一操作数**（官方 `test_dict.test_equal_operator_modifying_operand`
            // 的 `X.__eq__` / `X.__del__` ✓）⇒ 先前直接借用快照指针 ✗ ⇒ 释放后再读是 UAF ✓
            // （`PYAWA_FRAME_OWN=1` 实验路径下实测段错误 ✓）。
            let mut kept: Vec<NonNull<Header>> = Vec::with_capacity((a.len() + b.len()) * 2);
            for (key, value) in a.iter().chain(b.iter()) {
                // SAFETY: 键值由各自字典持有；比较期间本函数各持一份 ✓。
                unsafe {
                    instance.incref_object(key.as_ptr());
                    instance.incref_object(value.as_ptr());
                }
                kept.push(*key);
                kept.push(*value);
            }
            let answer = (|| -> Result<bool, ExecError> {
                for (key, value) in &a {
                    // 优先按**活字典**查（顺序／哈希语义照旧 ✓）；字典已被清空时退回**快照**查找 ✓
                    // （清空是用户 `__eq__` 的合法副作用，参照不因此崩溃 ✓）。
                    let other_value = match instance.dict_lookup_key_checked(right, *key)? {
                        // **按原始位置取活条目**（round 156 真 bug 修 ✗）：`b` 是**过滤后**的快照，
                        // 它的下标不是原始位置（墓碑不在）⇒ 必须查活字典 ✓。
                        Some(position) => {
                            // SAFETY: right 由调用方保证存活（本函数各持一份引用）。
                            let dict = unsafe {
                                &*right.as_ptr().cast::<crate::builtin_objects::DictObject>()
                            };
                            dict.entry(position).map(|(_, value)| value)
                        }
                        None => b
                            .iter()
                            .find(|(other_key, _)| {
                                values_equal_public(instance, *key, *other_key)
                            })
                            .map(|(_, value)| *value),
                    };
                    let Some(other_value) = other_value else {
                        return Ok(false);
                    };
                    if !equality_public(instance, *value, other_value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            })();
            for value in kept {
                // SAFETY: 上面各持一份。
                unsafe { instance.release_object(value.as_ptr()) };
            }
            return answer;
        }
    }
    // 用户类的 `__eq__`（两侧都问；`NotImplemented` 交给下一跳 ✓）。
    // **子类优先级照参照 `do_richcompare`**（round 112）：右侧类型是左侧的**严格子类** ⇒ **先问右侧**
    // （`eq(B(), C())` 里 C 是 B 子类 ⇒ 期望 `C.__eq__` 先、`B.__eq__` 后 ✓；`test_binop` 的
    // `test_comparison_orders`／`test_fallback_rmethod_blocking` 都钉这一条 ✓——
    // 「子类优先」还负责"`__eq__ = None` 阻断"：`SupEq() == S()` 里 S 的 `__eq__ = None` 会先被调用 ⇒
    // `TypeError: 'NoneType' object is not callable` ✓，而先问左侧会提前拿到 True ✗）。
    let right_first = right_ty != left_ty && instance.is_subtype(right_ty, left_ty);
    let order = if right_first {
        [(right, left), (left, right)]
    } else {
        [(left, right), (right, left)]
    };
    for (object, other) in order {
        if let Some(verdict) = call_compare_dunder(instance, object, "__eq__", other)? {
            return Ok(verdict);
        }
    }
    Ok(values_equal_public(instance, left, right))
}

pub fn compare_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<bool, ExecError> {
    // 相等／不等：**完整协议**（`__eq__`／`__ne__` 派发 ＋ 容器逐元素 ＋ 结构兜底 ✓，round 44）。
    match symbol {
        "==" => return equality_public(instance, left, right),
        "!=" => {
            // **快路**（round 100）：两侧都没有**自己**的 `__eq__`／`__ne__` ⇒ 直接按值相等取反 ✓
            // （`str`／`int`／`bytes` 等内建值类型走这里 ✓ —— 它们的 `object.__eq__` 默认本来是
            // "身份相等否则 NotImplemented" ✗，不适合直接走下面的"默认 __ne__"分派 ✓）。
            let left_type = instance.type_of(left);
            let right_type = instance.type_of(right);
            // 注意：**只看两侧有没有自己的 `__ne__`** ✓ —— 有自己 `__eq__`、没有 `__ne__` 的类
            // （`_pydecimal.Decimal` ✓）也走这条快路：语义与"默认 `__ne__` 委托 `__eq__` 再取反"
            // 同向 ✓，但少掉两次类型字典查找（`test_decimal` 的墙钟实测：走完整协议时 147 s ✗
            // ⇒ 快路把它拉回阈值内 ✓）。
            let overrides_ne = |object: NonNull<Header>| {
                type_has_own_dunder(instance, object, "__ne__")
                    // **宿主对象**必须进完整协议（快路会走 `values_equal_public`、绕过 `getattr_fn`）。
                    || instance.has_host_object_ops(object)
            };
            if !overrides_ne(left) && !overrides_ne(right) {
                return Ok(!equality_public(instance, left, right)?);
            }
            // **`!=` 的完整协议**（round 100，照参照 `do_richcompare`／`object.__ne__` 默认语义 ✓）：
            // 1. `__ne__` **按子类优先级**问（右侧类型是左侧子类 ⇒ 反射先 ✓）；
            // 2. 没有**自己**的 `__ne__` ⇒ `object.__ne__` 默认 = **委托给 `__eq__` 再取反** ✓
            //    （`__eq__` 给 `NotImplemented` ⇒ `__ne__` 也 `NotImplemented` ✓）；
            // 3. 两侧都没结论 ⇒ **身份不等**兜底 ✓（`x != x` False ✓、不同对象 True ✓；不再重跑
            //    `__eq__` ⇒ 调用序列与参照一致 ✓，见 `test_compare` 的两条 ne 优先级测试）。
            let right_first = right_type != left_type && instance.is_subtype(right_type, left_type);
            let order = if right_first {
                [(right, left), (left, right)]
            } else {
                [(left, right), (right, left)]
            };
            for (operand, other) in order {
                // **宿主对象**：类型 MRO 没有 `__ne__` 条目，但 `getattr_fn` 可能就是唯一实现
                // （`object.__ne__` 的默认会先命中类型通道、永远走不到宿主回退）⇒ 先问统一入口。
                if instance.has_host_object_ops(operand) {
                    if let Some(verdict) =
                        call_compare_dunder(instance, operand, "__ne__", other)?
                    {
                        return Ok(verdict);
                    }
                    continue;
                }
                // 有**自己的** `__ne__` ⇒ 调它 ✓。
                if type_has_own_dunder(instance, operand, "__ne__") {
                    if let Some(verdict) = call_compare_dunder(instance, operand, "__ne__", other)? {
                        return Ok(verdict);
                    }
                    continue;
                }
                // 否则走 `object.__ne__` 默认：**只有自己有 `__eq__`** 才委托它再取反 ✓
                // （`object.__eq__` 的默认是"身份相等否则 NotImplemented" ⇒ 不能把它当普通 eq 用 ✗）。
                if !type_has_own_dunder(instance, operand, "__eq__") {
                    continue;
                }
                if let Some(result) = call_special_method(instance, operand, "__eq__", &[other])? {
                    if result == instance.not_implemented() {
                        release(instance, result);
                        continue;
                    }
                    let truth = crate::executor::truthiness_public(instance, result, opcode)?;
                    release(instance, result);
                    return Ok(!truth);
                }
            }
            return Ok(left != right);
        }
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
        if left_sequence.is_some() && right_sequence.is_some() {
            // **活体双游标**（round 96，照参照 `list_richcompare` 的循环骨架 ✓）：先前一次快照
            // 之后一直用**借用**元素 ✗ ⇒ 元素的 `__lt__` 把列表清空时（`test_list` 的
            // `test_lt_operator_modifying_operand`：`evil.__lt__` 里 `other.clear()`）元素被释放、
            // 快照里的指针悬垂 ⇒ 下一轮用它就是 UAF ✗（`PYAWA_FRAME_OWN` 实验模型下实测抛出
            // `[悬垂-incref] call_object_method` ✓）。现在：每轮**重读**两侧列表、比较前各
            // `incref` 一份、比完归还 ✓（与上面的 `==` 路径同一口径 ✓）。
            let mut index = 0usize;
            loop {
                let left_items = if left_is_tuple {
                    instance.tuple_items(left).unwrap_or_default()
                } else {
                    instance.list_items(left).unwrap_or_default()
                };
                let right_items = if right_is_tuple {
                    instance.tuple_items(right).unwrap_or_default()
                } else {
                    instance.list_items(right).unwrap_or_default()
                };
                if index >= left_items.len() || index >= right_items.len() {
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
                let first = left_items[index];
                let second = right_items[index];
                // SAFETY: 两个元素由各自容器持有；比较期间本函数各持一份 ✓。
                unsafe {
                    instance.incref_object(first.as_ptr());
                    instance.incref_object(second.as_ptr());
                }
                let equal = values_equal_public(instance, first, second);
                let outcome = if equal {
                    Ok(None)
                } else {
                    compare_public(instance, first, second, "<", opcode).map(Some)
                };
                // SAFETY: 比较期间本函数各持一份 ⇒ 归还 ✓。
                unsafe {
                    instance.release_object(first.as_ptr());
                    instance.release_object(second.as_ptr());
                }
                match outcome? {
                    None => index += 1,
                    Some(first_is_less) => {
                        return Ok(match symbol {
                            "<" | "<=" => first_is_less,
                            ">" | ">=" => !first_is_less,
                            _ => false,
                        })
                    }
                }
            }
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
    // **`str` 的子类也按码点序** ✓（round 100）：先前只认两个**恰 str** ✗ ⇒
    // `class S(str)` 实例的 `<` 报错（`test_compare` 的 `test_str_subclass` ✓）。
    if instance.is_str_object(left) && instance.is_str_object(right) {
        if let (Some(a), Some(b)) = (instance.wtf8_of(left), instance.wtf8_of(right)) {
            let ordering = a.cmp(b);
            return Ok(match symbol {
                "<" => ordering.is_lt(),
                "<=" => ordering.is_le(),
                ">" => ordering.is_gt(),
                ">=" => ordering.is_ge(),
                _ => false,
            });
        }
    }
    // **`bytes`／`bytearray`（含子类）按字节字典序** ✓（round 100；先前只认恰 `bytes` ✗）。
    if let (Some(a), Some(b)) = (
        instance.bytes_like_value(left),
        instance.bytes_like_value(right),
    ) {
        let ordering = a.cmp(&b);
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
    // **子类优先级照参照 `do_richcompare`**（round 112）：右侧类型是左侧的**严格子类** ⇒ **反射先问**
    // （`le(B(), C())` 里 C 是 B 子类 ⇒ 期望 `C.__ge__` 先、`B.__le__` 后 ✓；`test_binop` 的
    // `test_comparison_orders` 钉住 ✓）。
    let left_ty = instance.type_of(left);
    let right_ty = instance.type_of(right);
    let right_first = right_ty != left_ty && instance.is_subtype(right_ty, left_ty);
    let order = if right_first {
        [(reflected, right, left), (forward, left, right)]
    } else {
        [(forward, left, right), (reflected, right, left)]
    };
    for (name, object, other) in order {
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
/// **协议派发的统一入口（含宿主对象）**：先问宿主属性钩子（`AB-73`／`AB-75` 的读序：
/// 对象自身条目 → 类型态条目），宿主 `PA_ERR_NOTFOUND` 才落回类型/实例属性通道
/// （[`call_special_method`]）。
///
/// 为什么不能只靠 `call_special_method`：属性查找在**类型 MRO 命中 `object` 的默认方法**时就会
/// 返回（`object.__eq__`＆C.），**不会**再走到宿主回退 ⇒ 宿主 `__eq__`／`__hash__`／`__contains__`
/// 等永远没机会被问（`PLAN` §12.12 oracle 抓到的旁路）。本函数把"问宿主"提到默认实现之前。
pub(crate) fn call_protocol_method(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<Option<NonNull<Header>>, ExecError> {
    if instance.has_host_object_ops(object) {
        if let Some(found) = instance.host_object_getattr(object, name)? {
            let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
            for argument in args {
                // SAFETY: 调用方保证实参存活；`call_callable` 自会 retain。
                unsafe { instance.incref_object(argument.as_ptr()) };
                call_args.push(*argument);
            }
            // 宿主 getattr 交出的是**已绑定**方法（`AB-73` 的 `HostBoundMethod`）⇒ 不再补 self。
            let outcome = call_callable(instance, found, None, call_args, Vec::new(), 0);
            // SAFETY: `found` 是属性通道交出的新引用。
            unsafe { instance.release_object(found.as_ptr()) };
            return Ok(Some(outcome?));
        }
    }
    call_special_method(instance, object, name, args)
}

pub(crate) fn call_special_method(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<Option<NonNull<Header>>, ExecError> {
    if !instance.is_type_object(object) {
        // **`staticmethod` 包裹的特殊方法**（round 110）：参照对 `__repr__ = staticmethod(f)` 的调用是
        // **不带 self** 的 ✓（实测 `class Obj: @staticmethod def __repr__(): return 'obj'` ⇒
        // `repr([Obj()])` 给 `[obj]` ✓）；先前当普通实例方法绑 self ✗
        //（`Obj.__repr__() takes 0 positional arguments but 1 was given` ✗，`test_list.test_repr_mutate` ✓）。
        let ty = instance.type_of(object);
        if let Some(found) = instance.type_lookup(ty, name) {
            if instance.type_name(instance.type_of(found)) == "staticmethod" {
                // SAFETY: 类型身份刚确认 ⇒ `StaticMethodObject` 载荷；内层函数由它持有，存活。
                let function = unsafe {
                    &*found.as_ptr().cast::<crate::builtin_objects::StaticMethodObject>()
                }
                .function();
                let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
                for argument in args {
                    // SAFETY: 调用方保证实参存活；`call_callable` 自会 retain。
                    unsafe { instance.incref_object(argument.as_ptr()) };
                    call_args.push(*argument);
                }
                let result = call_callable(instance, function, None, call_args, Vec::new(), 0)?;
                return Ok(Some(result));
            }
        }
        return call_object_method(instance, object, name, args);
    }
    // 类对象：只在**元类** MRO 上找 ✓（`type(obj).__mro__` ✓）。
    let metaclass = instance.type_of(object);
    let Some((owner, found)) = instance.type_lookup_owner(metaclass, name) else {
        return Ok(None);
    };
    // `type`／`object` 上的是**属性面**（不是格式化覆写 ✓，与 `override_text` 同口径 ✓）
    if Some(owner) == instance.hot_type(HotId::Type) || Some(owner) == instance.hot_type(HotId::Object) {
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
    let text = |key: &str| {
        instance
            .type_lookup(object.cast::<crate::TypeObject>(), key)
            .and_then(|value| instance.text_of(value))
            .map(str::to_owned)
    };
    // **`__qualname__` 的一处口径**（task-13）：编译器按定义处作用域写的完整限定名（`N2.D`／
    // `outer.<locals>.K`）——存私有键，与属性通道共用 `Instance::type_qualname_value` ✓。
    let qualname = instance.type_qualname_value(object);
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
    // **对象语义唯一入口＝属性查找**（`PLAN` §12.12 机检）：**不能**先按类型 MRO 判门 ✗——
    // 宿主对象没有 `__str__`／`__repr__` 的类型字典条目，但它的 `getattr_fn` 可能就是唯一实现。
    // 顺序：类型上**用户自有**实现 → 宿主协议钩子 → 默认（`object.__str__`／`__repr__` 不算覆写）。
    let ty = instance.type_of(object);
    let user_owner = instance
        .type_lookup_owner(ty, name)
        .is_some_and(|(owner, _)| Some(owner) != instance.hot_type(HotId::Object));
    if user_owner {
        return match call_special_method(instance, object, name, &[])? {
            Some(result) => {
                // SAFETY: result 是新引用，存活。
                let text = instance.text_value(result);
                release(instance, result);
                Ok(text)
            }
            None => Ok(None),
        };
    }
    // **宿主对象**：直接问属性钩子（`AB-73`／`AB-75` 的统一入口；`Owned` 是已绑定方法/值）。
    if instance.has_host_object_ops(object) {
        let Some(found) = instance.host_object_getattr(object, name)? else {
            return Ok(None);
        };
        // 宿主返回值按"无额外 self"调用（`AB-73` 的 getattr 返回已绑定方法；参数个数由宿主
        // 回调自己验，`AB-25`）——与 `host_binary_dunder` 的 `Owned` 口径一致。
        let outcome = call_callable(instance, found, None, Vec::new(), Vec::new(), 0);
        // SAFETY: `found` 是属性通道交出的新引用。
        unsafe { instance.release_object(found.as_ptr()) };
        let result = outcome?;
        // SAFETY: result 是新引用，存活。
        let text = instance.text_value(result);
        release(instance, result);
        return Ok(text);
    }
    Ok(None)
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
        Err(error) => {
            // **探针丢异常要还引用**（round 198 真泄漏修 ✗）。
            crate::executor::runtime::discard_error(instance, &error);
            return Ok(None);
        }
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
    if signature_type != instance.hot_type(HotId::Tuple).expect("tuple 已登记") {
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
    if label_type != instance.hot_type(HotId::Tuple).expect("tuple 已登记") {
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
    if label_type != instance.hot_type(HotId::Tuple).expect("tuple 已登记") {
        return None;
    }
    // SAFETY: 类型身份刚确认。
    let parts = unsafe { &*label.as_ptr().cast::<TupleObject>() };
    let head = parts.item(0)?;
    // SAFETY: 元组元素存活。
    if unsafe { head.as_ref() }.ty() != instance.hot_type(HotId::Str).expect("str 已登记") {
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
    if label_type == instance.hot_type(HotId::Str).expect("str 已登记") {
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
    if label_type == instance.hot_type(HotId::Tuple).expect("tuple 已登记") {
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
    if actual_type == instance.hot_type(HotId::List).expect("list 已登记") {
        // SAFETY: 类型身份刚确认。
        let list = unsafe { &*actual.as_ptr().cast::<crate::ListObject>() };
        return (0..list.len()).all(|index| {
            list.item(index)
                .map(|item| boundary_accepts(instance, item, inner))
                .unwrap_or(true)
        });
    }
    if actual_type == instance.hot_type(HotId::Tuple).expect("tuple 已登记") {
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
    let ty = instance.hot_type(HotId::BuiltinFunctionOrMethod)
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
    let Some(function_type) = instance.hot_type(HotId::Function) else {
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
        ("__defaults__", "defaults"),
        ("__kwdefaults__", "kwdefaults"),
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

/// **`function.__code__`／`__globals__` 的类属性换成真正的描述符实例**（task-8／缺口4）。
///
/// 为什么必须换：`Lib/types.py:63-64` 的
/// `GetSetDescriptorType = type(FunctionType.__code__)` 与
/// `MemberDescriptorType = type(FunctionType.__globals__)` 原先取到的是**原生函数**
/// （`builtin_function_or_method`）✗ ⇒ `types` 的两个别名都错 ✗ ⇒ 上游 `inspect._shadowed_dict`
/// 的 `type(class_dict) is types.GetSetDescriptorType` 永假 ⇒ `_check_class` 恒返回 sentinel
/// ⇒ `inspect.getattr_static` 恒 MISSING ✗。
///
/// 换成 `getset_descriptor`／`member_descriptor` 实例（**与 `type.__dict__['__dict__']` 同类型** ✓）
/// 后：`type(FunctionType.__code__).__name__ == "getset_descriptor"`、
/// `type(FunctionType.__globals__).__name__ == "member_descriptor"`（与参照 3.14.4 实测一致 ✓）。
///
/// **实例属性语义不变**：`f.__code__`／`f.__globals__` 仍走 `function` 的 `getattr` 槽
/// （[`crate::builtin::function::function_getattr`]）——类型字典这几项只在**类级取用**与
/// `type(...)` 上可见 ✓；`.__get__` 走 [`getset_descriptor_get`](crate::builtin_objects::getset_descriptor_get)
/// 的 `code`／`globals` 分派，与实例读**同一处真相** ✓。
fn install_function_dunder_descriptors(instance: &Instance) {
    let Some(function_type) = instance.hot_type(HotId::Function) else {
        return;
    };
    let Some(getset_type) = instance.hot_type(HotId::GetSetDescriptor) else {
        return;
    };
    let Some(member_type) = instance.hot_type(HotId::MemberDescriptor) else {
        return;
    };
    for (python_name, kind, descriptor_type) in [
        ("__code__", "code", getset_type),
        ("__globals__", "globals", member_type),
    ] {
        // **幂等**：已经是本类型字典自己的、且类型正确的描述符 ⇒ 不重装 ✓。
        let already = instance
            .type_lookup_owner(function_type, python_name)
            .is_some_and(|(owner, value)| {
                owner == function_type && instance.type_of(value) == descriptor_type
            });
        if already {
            continue;
        }
        let descriptor = instance.descriptor_instance(
            descriptor_type,
            kind,
            python_name,
            Some(function_type.cast::<crate::TypeObject>()),
            &[],
        );
        if let Some(previous) = instance.set_type_attribute(function_type, python_name, descriptor) {
            // SAFETY: `set_type_attribute` 交回的是被顶下来的旧值那份引用。
            unsafe { instance.release_object(previous.as_ptr()) };
        }
    }
}

/// **property 的数据描述符面**：`__set__`／`__delete__`（`__get__` 若 stdlib 已装则不重复 ✓）。
fn install_property_descriptors(instance: &Instance) {
    let Some(property_type) = instance.hot_type(HotId::Property) else {
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
    if let Some(ty) = instance.hot_type(HotId::StaticMethod) {
        install_type_method(
            instance,
            ty,
            "__get__",
            crate::builtin_objects::staticmethod_descriptor_get as crate::NativeFn,
        );
    }
    if let Some(ty) = instance.hot_type(HotId::ClassMethod) {
        // **补齐基类**（`P3-17` 顺带修的真缺口 ✓）：`classmethod` 类型在引导期是
        // `alloc_type_raw` 建的，**漏了** `register_bases` ✗ ⇒ MRO 为空 ⇒
        // `type_lookup(classmethod, "__get__")` 连**自己**的命名空间都看不到 ✗
        //（参照里 `classmethod` 继承 `object` ✓；`staticmethod`／`property` 都注册了基类 ✓）。
        // 修复放在**本安装器**里（不动引导期代码 ✓）：只在 MRO 为空时补一次 ✓，幂等 ✓。
        let mro_is_empty = unsafe { ty.as_ref() }.mro().is_empty();
        if mro_is_empty {
            if let Some(object_type) = instance.hot_type(HotId::Object) {
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
    install_function_dunder_descriptors(instance);
    install_property_descriptors(instance);
    install_static_and_classmethod_descriptors(instance);
}
