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
    // SAFETY: 类型身份已确认。
    unsafe { &*first.as_ptr().cast::<StrObject>() }.value().to_owned()
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
    // SAFETY: 契约上 this 是 str 实例。
    let text = unsafe { &*this.as_ptr().cast::<StrObject>() }.value().to_owned();
    let spec = match format::parse(&spec_text) {
        Ok(spec) => spec,
        Err(error) => return format_outcome(instance, Err(error), "str", instance.int_max_str_digits()),
    };
    if let Some(code) = spec.ty {
        if code != 's' {
            return format_outcome(instance, Err(SpecError::UnknownCode(code)), "str", instance.int_max_str_digits());
        }
    }
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
    let kind = bound
        .and_then(|this| instance.attribute_optional_of(this, "_kind").ok().flatten())
        .and_then(|value| instance.text_of(value))
        .unwrap_or("annotations")
        .to_owned();
    match kind.as_str() {
        "mro" => mro_descriptor_get(instance, bound, args, kwargs),
        "dict" => dunder_dict_descriptor_get(instance, bound, args, kwargs),
        _ => annotations_descriptor_get(instance, bound, args, kwargs),
    }
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
    // **新引用** ✓（`new_tuple` 接管 ✓）。
    Ok(instance.new_tuple(vec![instance.retain(origin)]))
}

/// **`GenericAlias.__getattr__`** ✓（task-24 尾件）：别名上缺的属性**转给 `__origin__`** ✓
/// （参照 `_GenericAlias.__getattr__` ✓，实测 `typing.Generic[T].__name__ == 'Generic'` ✓、
/// `list[int].__name__ == 'list'` ✓）。
///
/// 为什么需要 ✗：`Lib/typing.py` 的 `_ProtocolMeta.__new__`（1968 行起）对**未解析的基类**读
/// `base.__name__` ✓ —— `class SupportsAbs[T](Protocol)` 的基里带一个 `Generic[T]` 别名 ⇒
/// 先前报 `'GenericAlias' object has no attribute '__name__'` ⇒ `import typing` 卡在 2913 行 ✓。
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
    // **别把 `__getattr__` 自己转出去** ✗（否则自递归 ✓）。
    if name != "__getattr__" {
        if let Some(origin) = instance.attribute_optional_of(this, "__origin__")? {
            // `attribute_optional_of` 走完整属性通道 ✓（对**别名**会再落到本函数 ⇒ 逐层转发 ✓）。
            if let Some(found) = instance.attribute_optional_of(origin, &name)? {
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
