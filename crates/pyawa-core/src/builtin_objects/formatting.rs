//! 本文件是 `builtin_objects` 的 `formatting` 域：`__format__` 的原生实现与类型级描述符（`getset_descriptor`／`GenericAlias` 一族）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;
use crate::format::{self, SpecError};




/// 从原生调用的实参里取格式规格（`__format__(self, spec)` 的 `spec`）。
///
/// **先验类型**再读载荷（`TS-43`：布局自选，故禁止跨类型硬转），返回 **owned** 文本
/// （免得把借用传来传去）。
pub(crate) fn spec_argument(instance: &Instance, args: &[NonNull<Header>]) -> String {
    let Some(first) = args.first().copied() else {
        return String::new();
    };
    // SAFETY: first 由调用方保证存活。
    if unsafe { first.as_ref() }.ty() != instance.singletons().str_type() {
        return String::new();
    }
    // SAFETY: 类型身份已确认。P3-38 S2c：走安全口径 `text_value`（WTF-8 ⇒ 有损文本，不 panic ✓）
    instance.text_value(first).unwrap_or_default()
}

/// 把 `format.rs` 的结果折成"原生返回值或实测消息的异常"。
pub(crate) fn format_outcome(
    instance: &Instance,
    result: Result<String, SpecError>,
    class_name: &str,
    max_str_digits: u32,
) -> Result<NonNull<Header>, crate::ExecError> {
    match result {
        Ok(text) => Ok(instance.new_str(&text)),
        Err(SpecError::NegativeZero) => Err(crate::executor::runtime::raise_builtin(
            instance,
            "ValueError",
            format::NEGATIVE_ZERO_MESSAGE,
        )),
        Err(SpecError::UnknownCode(code)) => {
            let message = format!("Unknown format code '{code}' for object of type '{class_name}'");
            Err(crate::executor::runtime::raise_builtin(instance, "ValueError", &message))
        }
        Err(SpecError::FloatOverflow) => Err(crate::executor::runtime::raise_builtin(
            instance,
            "OverflowError",
            "int too large to convert to float",
        )),
        Err(SpecError::CharTooLarge) => Err(crate::executor::runtime::raise_builtin(
            instance,
            "OverflowError",
            "Python int too large to convert to C long",
        )),
        Err(SpecError::CharOutOfRange) => Err(crate::executor::runtime::raise_builtin(
            instance,
            "OverflowError",
            "%c arg not in range(0x110000)",
        )),
        Err(SpecError::DigitLimit) => Err(instance.raise_builtin_error(
            "ValueError",
            &digit_limit_message(max_str_digits),
        )),
        Err(SpecError::NotImplemented) => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "这条格式化规格本层还没实现（迷你语言的其余部分）",
        }),
    }
}

/// `TS-45` ①的**输出方向**消息（`repr`／`str`／`format` 三处共用一条真相）。
pub(crate) fn digit_limit_message(limit: u32) -> String {
    format!(
        "Exceeds the limit ({limit} digits) for integer string conversion; \
         use sys.set_int_max_str_digits() to increase the limit"
    )
}

/// `object.__format__`（默认）：空规格 ⇒ `str(x)`；非空 ⇒ TypeError（消息实测）。
pub unsafe fn native_format_object(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec = spec_argument(instance, args);
    if !spec.is_empty() {
        // SAFETY: this 由调用方保证存活。
        let class_name = unsafe { this.as_ref().ty().as_ref() }.name();
        let message = format!("unsupported format string passed to {class_name}.__format__");
        return Err(crate::executor::runtime::raise_builtin(instance, "TypeError", &message));
    }
    Ok(instance.new_str(&instance.object_str(this)?))
}

/// `int.__format__`：空规格 ⇒ `str(self)`（于是 `bool` 走 `True`／`False`），否则数值规格。
pub unsafe fn native_format_int(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    if spec_text.is_empty() {
        return Ok(instance.new_str(&instance.object_str(this)?));
    }
    // `bool` 继承 `int.__format__`（实测 `bool.__dict__` 里**没有** `__format__`），
    // 所以这里必须按**实际类型**读载荷：`BoolObject` 与 `IntObject` 是两个布局。
    // SAFETY: this 是存活对象。
    let this_header = unsafe { this.as_ref() };
    let type_name = unsafe { this_header.ty().as_ref() }.name();
    let payload = if type_name == "bool" {
        // SAFETY: 类型身份已确认。
        IntValue::Small(i64::from(unsafe { &*this.as_ptr().cast::<BoolObject>() }.value))
    } else {
        // SAFETY: 同上。
        unsafe { &*this.as_ptr().cast::<IntObject>() }.value.clone()
    };
    let limit = instance.int_max_str_digits();
    match format::parse(&spec_text) {
        Ok(spec) => {
            let outcome = format::format_big_int(&payload.to_bigint(), &spec, limit);
            format_outcome(instance, outcome, type_name, limit)
        }
        Err(error) => format_outcome(instance, Err(error), type_name, limit),
    }
}

/// `float.__format__`：空规格 ⇒ `str(self)`，否则浮点规格。
pub unsafe fn native_format_float(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    if spec_text.is_empty() {
        return Ok(instance.new_str(&instance.object_str(this)?));
    }
    // SAFETY: 契约上 this 是 float 实例。
    let value = unsafe { &*this.as_ptr().cast::<FloatObject>() }.value;
    match format::parse(&spec_text) {
        Ok(spec) => {
            let outcome = format::format_float(value, &spec);
            format_outcome(instance, outcome, "float", instance.int_max_str_digits())
        }
        Err(error) => format_outcome(instance, Err(error), "float", instance.int_max_str_digits()),
    }
}

/// `str.__format__`：空规格 ⇒ 内容本身；否则只认对齐／宽度／精度。
pub unsafe fn native_format_str(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    let spec = match format::parse(&spec_text) {
        Ok(spec) => spec,
        Err(error) => return format_outcome(instance, Err(error), "str", instance.int_max_str_digits()),
    };
    if let Some(code) = spec.ty {
        if code != 's' {
            return format_outcome(instance, Err(SpecError::UnknownCode(code)), "str", instance.int_max_str_digits());
        }
    }
    // P3-38 S2c：`str.__format__` 直接调用（不经 FORMAT_WITH_SPEC）时，WTF-8 含代理的 self 也走
    // **码点层**格式化（`format(s, '>5')` 与参照逐字符一致 ✓；不再有损 ✓）；不支持的类别 ⇒ 有损兜底。
    if instance.text_of(this).is_none() {
        if spec_text.is_empty() {
            return Ok(instance.retain(this));
        }
        if let Some(formatted) =
            crate::executor::format::format_wtf8_str(instance, this, &spec)
        {
            return Ok(formatted);
        }
        let text = instance.text_value(this).unwrap_or_default();
        return format_outcome(
            instance,
            format::format_str(&text, &spec),
            "str",
            instance.int_max_str_digits(),
        );
    }
    // 合法 UTF-8：内容原样（旧行为不变 ✓）
    let text = instance.text_value(this).unwrap_or_default();
    format_outcome(
        instance,
        format::format_str(&text, &spec),
        "str",
        instance.int_max_str_digits(),
    )
}


/// **`type.__dict__["__annotations__"]` 的 `__get__`** ✓（task-20 ✓）：`annotationlib` 第 1141 行
/// 就是 `type.__dict__["__annotations__"].__get__` ✓ ⇒ 先把类或模块的注解字典取出来 ✓
/// （**一处真相**：口径在 [`Instance::annotations_of`] ✓）。
pub fn annotations_descriptor_get(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(target) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__annotations__' of 'type' object needs an argument",
        ));
    };
    if target == instance.singletons().none() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__annotations__' for 'type' objects doesn't apply to a 'NoneType' object",
        ));
    }
    match instance.annotations_of(target) {
        Some(value) => Ok(value),
        None => Err(instance.raise_builtin_error(
            "AttributeError",
            "'__annotations__' 只接类型对象与模块对象",
        )),
    }
}

/// **`type.__dict__["__mro__"]` 的 `__get__`** ✓（task-20 第 2 轮 ✓）：`Lib/inspect.py:1673` 就是
/// `_static_getmro = type.__dict__['__mro__'].__get__` ✓ ⇒ 类对象上取 MRO 元组 ✓（口径与
/// `attribute.rs` 的 `cls.__mro__` **同一处真相**：注册表算好的 MRO ✓）。
/// **getset 描述符的 `__get__` 分派** ✓（task-20 第 2 轮 ✓）：两个描述符（`__annotations__`／`__mro__`）
/// 共用**一个**类型 ✓ ⇒ 按描述符实例里记的 `_kind` 分派 ✓（`_tuplegetter` 记 `_index` 同一手法 ✓）。
pub fn getset_descriptor_get(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // **`descr.__get__(None, owner)` ⇒ 交回描述符自己**（参照 getset／member 描述符的 C 口径：
    // `obj` 为空时返回描述符本身；类级取用 `FunctionType.__code__` 这一类就走这里 ✓）；
    // `None, None` ⇒ 参照原话报错 ✓（与既有 `function_attribute_descriptor_get` 同款 ✓）。
    let none = instance.singletons().none();
    if args.first() == Some(&none) {
        let owner_missing = args.len() < 2 || args[1] == none;
        if owner_missing {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "__get__(None, None) is invalid",
            ));
        }
        if let Some(descriptor) = bound {
            return Ok(instance.retain(descriptor));
        }
    }
    let kind = bound
        .and_then(|this| instance.attribute_optional_discarding(this, "_kind"))
        .and_then(|value| instance.text_of(value))
        .unwrap_or("annotations")
        .to_owned();
    match kind.as_str() {
        "mro" => mro_descriptor_get(instance, bound, args, kwargs),
        "dict" => dunder_dict_descriptor_get(instance, bound, args, kwargs),
        // **`function.__code__`／`function.__globals__` 的 getset／member 描述符**（task-8）：
        // 类属性是描述符实例（`Lib/types.py` 的 `GetSetDescriptorType`／`MemberDescriptorType`
        // 就是这两个的 `type(...)`）⇒ `.__get__(f, function)` 走这里，语义与
        // `f.__code__`／`f.__globals__` **同一处真相**（转发到既有的原生访问器 ✓）。
        "code" => crate::builtin::function::function_code_native(instance, None, args, kwargs),
        "globals" => crate::builtin::function::function_globals_native(instance, None, args, kwargs),
        // **`type.__dict__` 的类型属性面**（task-13 #d）：`__name__`／`__qualname__`／`__module__`
        // ／`__bases__` 是真 getset 描述符实例 ⇒ `__get__(类, type)` 读 `TypeObject` 字段／命名空间
        // （与属性通道 `attribute_lookup_specials` 共用 `Instance::type_*_value` **一处真相**）。
        "typename" | "qualname" | "module" | "bases" => {
            type_dunder_descriptor_get(instance, args, &kind)
        }
        // **`__slots__` 槽的 `member_descriptor`**（task-13 #b）：本层槽值仍存在**实例字典**的槽键
        // 里（完整"无实例字典"语义未接，见台账）⇒ `__get__` 从实例字典取；未设置 ⇒ 参照原话。
        "member" => member_descriptor_get(instance, bound, args),
        _ => annotations_descriptor_get(instance, bound, args, kwargs),
    }
}

/// `type.__dict__['__name__'/'__qualname__'/'__module__'/'__bases__']` 的 `__get__`（task-13 #d）。
///
/// 参照 3.14.4 实测：`descr.__get__(C, type)` 给对应值；目标不是类型对象 ⇒
/// `TypeError: descriptor '<name>' for 'type' objects doesn't apply to a '<type>' object`。
/// 值与属性通道共用 [`Instance::type_name_value`]／[`Instance::type_qualname_value`]／
/// [`Instance::type_module_value`]（**一处真相**）——`__bases__` 直接取注册表里的基类（每项 retain ✓）。
fn type_dunder_descriptor_get(
    instance: &Instance,
    args: &[NonNull<Header>],
    kind: &str,
) -> Result<NonNull<Header>, ExecError> {
    let python_name = match kind {
        "typename" => "__name__",
        "qualname" => "__qualname__",
        "module" => "__module__",
        _ => "__bases__",
    };
    let Some(target) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("descriptor '{python_name}' of 'type' object needs an argument"),
        ));
    };
    if !instance.is_type_object(target) {
        let actual = instance.type_name(instance.type_of(target));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "descriptor '{python_name}' for 'type' objects doesn't apply to a '{actual}' object"
            ),
        ));
    }
    match kind {
        "typename" => Ok(instance.new_str(&instance.type_name_value(target))),
        "qualname" => Ok(instance.new_str(&instance.type_qualname_value(target))),
        "module" => match instance.type_module_value(target) {
            Some(value) => Ok(value),
            None => Ok(instance.new_str("builtins")),
        },
        _ => {
            // SAFETY: 刚判过 target 是存活的类型对象。
            let bases = unsafe { target.cast::<crate::TypeObject>().as_ref() }.bases();
            let items: Vec<NonNull<Header>> = bases
                .into_iter()
                .map(|base| {
                    let header = base.cast::<Header>();
                    // SAFETY: base 由注册表持有、存活；元组要自己那份。
                    unsafe { instance.incref_object(header.as_ptr()) };
                    header
                })
                .collect();
            Ok(instance.new_tuple(items))
        }
    }
}

/// **`__slots__` 槽的 `member_descriptor.__get__`**（task-13 #b）。
///
/// 参照：`C.__dict__['slot'].__get__(c, C)` 给槽值（未设置 ⇒
/// `AttributeError: 'C' object has no attribute 'slot'`；目标类型不符 ⇒ 参照原话）。
/// 本层槽值存在**实例字典**的槽键里（`__slots__` 登记见 `Instance::register_type_slots`），
/// 写路径保持既有（实例字典；`member_descriptor` 目前**不带 `__set__`**，见工单边界）。
fn member_descriptor_get(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    let Some(descriptor) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__get__' 需要 member_descriptor 实例",
        ));
    };
    let name = instance
        .attribute_optional_discarding(descriptor, "__name__")
        .and_then(|value| instance.text_of(value).map(str::to_owned))
        .unwrap_or_default();
    let Some(target) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__get__' needs an argument",
        ));
    };
    // `__objclass__` 决定"哪些对象能取"（参照原话：`descriptor 'g' for 'Dyn' objects doesn't
    // apply to a 'int' object`）。
    let owner_type = instance
        .attribute_optional_discarding(descriptor, "__objclass__")
        .and_then(|value| instance.as_type(value));
    if let Some(owner_type) = owner_type {
        if !instance.is_subtype(instance.type_of(target), owner_type) {
            let owner_name = instance.type_name(owner_type);
            let actual = instance.type_name(instance.type_of(target));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "descriptor '{name}' for '{owner_name}' objects doesn't apply to a '{actual}' object"
                ),
            ));
        }
    }
    if let Some(mapping) = crate::mounted_instance_dict(instance, target) {
        if let Some(value) = instance.dict_get(mapping, &name) {
            return Ok(instance.retain(value));
        }
    }
    let actual = instance.type_name(instance.type_of(target));
    Err(instance.raise_builtin_error(
        "AttributeError",
        &format!("'{actual}' object has no attribute '{name}'"),
    ))
}

/// **getset／member 描述符的 `repr`**（task-13 #b/#d；与参照 3.14.4 同形）：
/// `<attribute '__name__' of 'type' objects>`／`<member 'g' of 'Dyn' objects>`。
/// 依据是实例内联属性字典里的 `__name__`／`__objclass__`（`descriptor_instance` 写入；
/// `_shadowed_dict` 也要这两项 ✓）。
pub(crate) unsafe fn descriptor_repr(
    ptr: *mut Header,
    instance: &Instance,
) -> Result<String, ExecError> {
    let Some(object) = NonNull::new(ptr) else {
        return Ok("<descriptor object>".to_owned());
    };
    let name = instance
        .attribute_optional_discarding(object, "__name__")
        .and_then(|value| instance.text_of(value).map(str::to_owned))
        .unwrap_or_else(|| "?".to_owned());
    let owner = instance
        .attribute_optional_discarding(object, "__objclass__")
        .and_then(|value| instance.as_type(value))
        .map(|owner| instance.type_name_value(owner.cast::<Header>()))
        .unwrap_or_else(|| "object".to_owned());
    let kind = instance.type_name(instance.type_of(object));
    let word = if kind == "member_descriptor" { "member" } else { "attribute" };
    Ok(format!("<{word} '{name}' of '{owner}' objects>"))
}

/// **`type.__dict__["__dict__"]` 的 `__get__`** ✓（task-20 第 2 轮 ✓）：`Lib/inspect.py:1674` 的
/// `_get_dunder_dict_of_class = type.__dict__["__dict__"].__get__` 要它 ✓ ⇒ 交回那个类的**命名空间**
/// （本层的 `__dict__` 就是命名空间本身 ✓ —— 与 CPython 的 `mappingproxy` 之差**已登记** ✓）。
pub fn dunder_dict_descriptor_get(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(target) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__dict__' of 'type' object needs an argument",
        ));
    };
    if !instance.is_type_object(target) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__dict__' for 'type' objects doesn't apply to a non-type object",
        ));
    }
    let namespace = instance
        .type_namespace(target)
        .ok_or_else(|| instance.raise_builtin_error("AttributeError", "类型没有命名空间"))?;
    Ok(instance.retain(namespace))
}

pub fn mro_descriptor_get(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(target) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__mro__' of 'type' object needs an argument",
        ));
    };
    if !instance.is_type_object(target) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__mro__' for 'type' objects doesn't apply to a non-type object",
        ));
    }
    // SAFETY: 刚判过它是类型对象，且由注册表持有。**每项 retain 一份** ✓（`new_tuple` 会吃掉 ✓）。
    let items: Vec<NonNull<Header>> = unsafe { target.cast::<crate::TypeObject>().as_ref() }
        .mro()
        .into_iter()
        .map(|base| instance.retain(base.cast::<Header>()))
        .collect();
    Ok(instance.new_tuple(items))
}

/// **`types.GenericAlias.__mro_entries__(bases)`** ✓（task-20 第 2 轮 ✓，PEP 560）：
/// `class T(Mapping[str, str])` 的下标结果**不能**直接当基类 ✗ ⇒ 参照先调它换出真正的基类 ✓
/// （CPython 的实现就是交回 `(origin,)` ✓）。`Lib/_colorize.py:115` 正是这个形状 ✓。
pub fn generic_alias_mro_entries(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__mro_entries__ 缺少 self"));
    };
    let origin = instance
        .attribute_optional_of(this, "__origin__")?
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "别名没有 __origin__"))?;
    // **新引用** ✓：`origin` 本身就是新引用 ⇒ 直接交给 `new_tuple`（它接新引用 ✓）——
    // 先前又 `retain` 一次 ⇒ 每次 `__mro_entries__` 漏一份 ✗（round 227 修 ✓）。
    Ok(instance.new_tuple(vec![origin]))
}

/// **`GenericAlias.__getattr__`** ✓（task-24 尾件）：别名上缺的属性**转给 `__origin__`** ✓
/// （参照 `_GenericAlias.__getattr__` ✓，实测 `typing.Generic[T].__name__ == 'Generic'` ✓、
/// `list[int].__name__ == 'list'` ✓）。
///
/// 为什么需要 ✗：`Lib/typing.py` 的 `_ProtocolMeta.__new__`（1968 行起）对**未解析的基类**读
/// `base.__name__` ✓ —— `class SupportsAbs[T](Protocol)` 的基里带一个 `Generic[T]` 别名 ⇒
/// 先前报 `'GenericAlias' object has no attribute '__name__'` ⇒ `import typing` 卡在 2913 行 ✓。
/// **`GenericAlias.__call__`**（round 216）：参照里别名**可调用**，转调 `origin(*args)` ✓
/// （`Group[int, list[int]](1, [2])` ⇒ `Group(1,[2])` ✓，`test_collections.TestNamedTuple`
/// `test_non_generic_subscript` ✓）。别名类型没有 `call` 槽 ⇒ 挂 Python 级 `__call__` ✓
///（调用方的"无 call 槽 ⇒ 找 `type(obj).__call__`"那一路会命中 ✓）。
pub fn generic_alias_call(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__call__ 缺少 self"));
    };
    let Some(origin) = instance.attribute_optional_of(this, "__origin__")? else {
        return Err(instance.raise_builtin_error("TypeError", "别名没有 __origin__"));
    };
    // **实参所有权**（round 224 真 bug 修 ✗）：`call_callable` **接管**传进去的 `Vec`
    // 里每一项的引用（`call.rs` 的注释与实现：`take_args`／native 路径调用完逐项 release ✓）——
    // 先前注释写成"只借不接管" ✗ ⇒ 这里 retain 一份、传 `clone()` 进去、**再 release 自己那份**
    // ⇒ 每条实参净 **−1** ⇒ 被调用方（如 `namedtuple.__new__`）保存的引用成悬垂 ✗
    // ⇒ 偶发 `malloc(): unaligned tcache chunk detected` ✗（`Group[int,list[int]](1,[2])` 循环 3000 次
    // 5/5 崩 ✓；round 216 的"别名调用"首崩也是这一条 ✓，当时把方向误判成"要 retain"✓）。
    // 正确形状与 `call.rs` 的 `call_value` 同型：retain 后把 Vec **移进去**，不要再 release。
    let retained_args: Vec<NonNull<Header>> = args
        .iter()
        .map(|value| {
            // SAFETY: 实参由调用方保证存活。
            unsafe { instance.incref_object(value.as_ptr()) };
            *value
        })
        .collect();
    let mut retained_kwargs: Vec<(NonNull<Header>, NonNull<Header>)> =
        Vec::with_capacity(kwargs.len());
    for (name, value) in kwargs {
        // SAFETY: 同上。
        unsafe {
            instance.incref_object(name.as_ptr());
            instance.incref_object(value.as_ptr());
        }
        retained_kwargs.push((*name, *value));
    }
    // `attribute_optional_of` 给的是**新引用** ⇒ 调完要还。
    let outcome = crate::executor::call::call_callable(
        instance,
        origin,
        None,
        retained_args,
        retained_kwargs,
        0,
    );
    // 不在这里 release：`call_callable` 已接管上面 retain 出来的那几份 ✓。
    // SAFETY: 上面那份新引用由本函数持有。
    unsafe { instance.release_object(origin.as_ptr()) };
    outcome
}

/// **`GenericAlias.__eq__`**（round 222）：参照里 `list[int] == list[int]` 为真 ✓（比
/// `__origin__` 与 `__args__`）。先前别名没有相等比较 ⇒ 走身份 ⇒ 两个独立别名不等 ✗
/// （`test_collections.TestNamedTuple.test_non_generic_subscript` 的
/// `(Group[int, list[int]].__args__ == (int, list[int]))` ✓）。
pub fn generic_alias_eq(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Ok(instance.not_implemented());
    };
    let Some(other) = args.first().copied() else {
        return Ok(instance.not_implemented());
    };
    // 对方也是别名（有 `__origin__`／`__args__`）才比 ✓；否则 `NotImplemented`（让 Python 回退身份 ✓）。
    let (Some(our_origin), Some(our_args)) = (
        instance.attribute_optional_of(this, "__origin__")?,
        instance.attribute_optional_of(this, "__args__")?,
    ) else {
        return Ok(instance.not_implemented());
    };
    let (Some(their_origin), Some(their_args)) = (
        instance.attribute_optional_of(other, "__origin__")?,
        instance.attribute_optional_of(other, "__args__")?,
    ) else {
        return Ok(instance.not_implemented());
    };
    let origins_equal = crate::executor::values::values_equal_public(instance, our_origin, their_origin);
    let items = instance.tuple_items(our_args);
    let their_items = instance.tuple_items(their_args);
    let equal = origins_equal
        && match (items, their_items) {
            (Some(items), Some(their_items)) => {
                items.len() == their_items.len()
                    && items
                        .iter()
                        .zip(their_items.iter())
                        .all(|(left, right)| {
                            crate::executor::values::values_equal_public(instance, *left, *right)
                        })
            }
            _ => false,
        };
    // 四个属性各是新引用 ⇒ 比完逐个还 ✓（先前漏还 ⇒ 每次别名比较漏 4 份 ✗，round 222 实测
    // 在 `test_collections` 里累积到 `malloc(): unaligned tcache chunk detected` ✗）。
    for value in [our_origin, our_args, their_origin, their_args] {
        instance.release(value);
    }
    Ok(instance.new_bool(equal))
}

pub fn generic_alias_getattr(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__getattr__ 缺少 self"));
    };
    let Some(name) = args
        .first()
        .and_then(|value| instance.text_of(*value))
        .map(str::to_owned)
    else {
        return Err(instance.raise_builtin_error("TypeError", "__getattr__ 的名字要是 str"));
    };
    // **`__parameters__`**（round 216）：非泛型 `GenericAlias` 的 `__parameters__` 是 **`()`** ✓
    //（`Group = namedtuple(...); Group[int, list[int]].__parameters__ == ()` ✓；
    // `test_collections.TestNamedTuple.test_non_generic_subscript`）。转发给 origin 会丢（namedtuple
    // 类本身没有 `__parameters__` ✓）⇒ 这里直接给空元组 ✓。
    if name == "__parameters__" {
        return Ok(instance.new_tuple(Vec::new()));
    }
    // **别把 `__getattr__` 自己转出去** ✗（否则自递归 ✓）。
    if name != "__getattr__" {
        if let Some(origin) = instance.attribute_optional_of(this, "__origin__")? {
            // `attribute_optional_of` 走完整属性通道 ✓（对**别名**会再落到本函数 ⇒ 逐层转发 ✓）。
            let found = instance.attribute_optional_of(origin, &name);
            // **origin 是新引用** ⇒ 不论转不转得出去都要还 ✓（round 227 修泄漏 ✗）。
            instance.release(origin);
            if let Some(found) = found? {
                return Ok(found);
            }
        }
    }
    // 照参照的报错形态（`'GenericAlias' object has no attribute 'x'` ✓）。
    // SAFETY: `this` 是绑定进来的存活对象 ✓。
    let type_name = instance.type_name(unsafe { this.as_ref() }.ty());
    Err(instance.raise_builtin_error(
        "AttributeError",
        &format!("'{type_name}' object has no attribute '{name}'"),
    ))
}
