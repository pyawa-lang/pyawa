//! 本文件是 `builtin_objects` 的 `representations` 域：`OM-11` 的 `repr`／`str` 槽：各内建类型的 `repr`／`str` 与 `tuple` 的 `count`／`index`。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;



/// `bool` 的 `repr`／`str`：`True`／`False`。
pub unsafe fn bool_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<BoolObject>() };
    Ok(if object.value { "True" } else { "False" }.to_owned())
}

/// `NoneType` 的 `repr`：`None`。
pub unsafe fn none_repr(_ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    Ok("None".to_owned())
}

/// `str` 的 `repr`：按参照实现的引号与转义规则（实测：能用单引号就用单引号）。
pub unsafe fn str_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StrObject>() };
    // P3-38 S2a-2：按 **WTF-8 字节**取内容（合法 UTF-8 的 repr 输出逐字节不变 ✓；
    // 孤立代理由 `quote_wtf8` 转义成 `\udXXX` ✓）。
    Ok(crate::instance::quote_wtf8(object.wtf8_bytes(), false))
}

/// `str` 的 `str`：内容本身。
pub unsafe fn str_str(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StrObject>() };
    Ok(object.value().to_owned())
}


/// `tuple` 的 `repr`：空是 `()`、单个是 `(x,)`（实测）。
pub unsafe fn tuple_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TupleObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Ok("(...)".to_owned());
    }
    let mut text = String::from("(");
    for index in 0..object.len() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::protocol::element_repr(
            instance,
            object.item(index).expect("下标在范围内"),
        )?);
    }
    if object.len() == 1 {
        text.push(',');
    }
    text.push(')');
    instance.leave_repr(ptr as usize);
    Ok(text)
}

/// **`tuple` 的方法面** ✓（第 413 轮，task-26 ② ✓）：`count`／`index`。
///
/// 动因：`Lib/dataclasses.py:1290` 的 `f.__code__.co_freevars.index("__class__")`
/// （`import asyncio` 链上）——本层 `hasattr((1,), "index")` 先前是 False ✗（`tuple` 类型连
/// `getattr` 槽都没有 ✓）。参照语义实测：
/// * `count(x)`：相等元素个数；参数个数 ≠ 1 ⇒ `TypeError: tuple.count() takes exactly one argument (N given)`；
/// * `index(x[, start[, end]])`：首个相等下标；范围语义同切片（负数先加长再夹到 `[0, len]`）；
///   找不到 ⇒ `ValueError: tuple.index(x): x not in tuple`；start／end 非整数 ⇒
///   `TypeError: slice indices must be integers or have an __index__ method` ✓。
///
/// **实例／类型级都认** ✓（task-23 的描述符先例 ✓）：实例路径由 `getattr` 槽交**绑定**方法
/// （`bound` 里有 self ✓）；类型级 `tuple.count(t, x)` 走类型字典里的未绑定原生 ⇒ self 是
/// **第一个位置实参** ✓。
pub unsafe fn tuple_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: crate::NativeFn = match name {
        "count" => tuple_count_native,
        "index" => tuple_index_native,
        _ => return None,
    };
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "tuple",
        Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// 取"被调用的那个 tuple"（实例路径在 `bound`、类型级未绑定在 `args[0]` ✓）。
pub(crate) fn tuple_receiver(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    method: &str,
) -> Result<(NonNull<Header>, usize), crate::ExecError> {
    let (this, consumed) = match bound {
        Some(this) => (this, 0),
        None => match args.first() {
            Some(first) => (*first, 1),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("descriptor '{method}' of 'tuple' object needs an argument"),
                ))
            }
        },
    };
    let tuple_type = instance.type_named("tuple").expect("tuple 已登记");
    // SAFETY: this 由调用方保证存活。
    let this_type = unsafe { this.as_ref() }.ty();
    if !instance.is_subtype(this_type, tuple_type) {
        let kind = instance.type_name(this_type);
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("descriptor '{method}' for 'tuple' objects doesn't apply to a '{kind}' object"),
        ));
    }
    Ok((this, consumed))
}

/// **一个切片下标的整数实参**（照参照：非整数 ⇒ 同一条 `TypeError` 文案 ✓）。
pub(crate) fn tuple_index_argument(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<i64, crate::ExecError> {
    match instance.int_of(value).and_then(|number| number.to_i64()) {
        Some(number) => Ok(number),
        None => Err(instance.raise_builtin_error(
            "TypeError",
            "slice indices must be integers or have an __index__ method",
        )),
    }
}

/// `tuple.count(x)`（见 [`tuple_getattr`] 的口径 ✓）。
pub unsafe fn tuple_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (this, consumed) = tuple_receiver(instance, bound, args, "count")?;
    let rest = &args[consumed..];
    if rest.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("tuple.count() takes exactly one argument ({} given)", rest.len()),
        ));
    }
    let wanted = rest[0];
    let items = instance.tuple_items(this).unwrap_or_default();
    let count = items
        .into_iter()
        .filter(|item| crate::executor::values::values_equal_public(instance, *item, wanted))
        .count() as i64;
    Ok(instance.new_int(count))
}

/// `tuple.index(x[, start[, end]])`（见 [`tuple_getattr`] 的口径 ✓）。
pub unsafe fn tuple_index_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (this, consumed) = tuple_receiver(instance, bound, args, "index")?;
    let rest = &args[consumed..];
    if rest.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "tuple.index() takes at least 1 argument (0 given)",
        ));
    }
    if rest.len() > 3 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("tuple.index() takes at most 3 arguments ({} given)", rest.len()),
        ));
    }
    let wanted = rest[0];
    let items = instance.tuple_items(this).unwrap_or_default();
    let length = items.len() as i64;
    let mut start = match rest.get(1) {
        Some(value) => tuple_index_argument(instance, *value)?,
        None => 0,
    };
    let mut stop = match rest.get(2) {
        Some(value) => tuple_index_argument(instance, *value)?,
        None => length,
    };
    if start < 0 {
        start += length;
        if start < 0 {
            start = 0;
        }
    }
    if stop < 0 {
        stop += length;
        if stop < 0 {
            stop = 0;
        }
    }
    if stop > length {
        stop = length;
    }
    for index in start..stop.min(length) {
        // SAFETY: 下标在范围内 ⇒ item 由本对象持有。
        if let Some(item) = items.get(index as usize).copied() {
            if crate::executor::values::values_equal_public(instance, item, wanted) {
                return Ok(instance.new_int(index));
            }
        }
    }
    Err(instance.raise_builtin_error(
        "ValueError",
        "tuple.index(x): x not in tuple",
    ))
}

/// `dict` 的 `repr`：`{k: v}`；自引用给 `{'k': {...}}`（实测）。
pub unsafe fn dict_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Ok("{...}".to_owned());
    }
    let mut text = String::from("{");
    for (index, (key, value)) in object.entries().into_iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::protocol::element_repr(instance, key)?);
        text.push_str(": ");
        text.push_str(&crate::executor::protocol::element_repr(instance, value)?);
    }
    text.push('}');
    instance.leave_repr(ptr as usize);
    Ok(text)
}

/// 类型对象的 `repr`：`<class 'int'>`（实测）。
pub unsafe fn type_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<crate::TypeObject>() };
    Ok(format!("<class '{}'>", object.name()))
}

/// code object 的 `repr`：`<code object demo at 0x…, file "…", line 1>`（实测）。
pub unsafe fn code_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<crate::CodeObject>() };
    Ok(format!(
        "<code object {} at {ptr:p}, file \"{}\", line {}>",
        object.name(),
        object.filename(),
        object.firstlineno()
    ))
}

/// 绑定方法的 `repr`：`<bound method m of <C object at 0x…>>`。
///
/// 实测的形状是 `<bound method C.m of …>`：名字取 **`BC-4` 的 `co_qualname`**
/// （编译器已产出它；类体方法那个 `C.m` 由**类创建钩子**在建类时补写——已落地）。
/// **通用异常 `str`**（第 317 轮 ✓）：从**实例属性**取消息 ✓ —— 只走属性通道 ✓、**不解引用载荷** ✗
/// （这正是"抄基类 `str` 槽"会崩的原因 ✗：基类槽假定基类的载荷布局 ✓）。
/// 口径照参照：有 `message` 用它 ✓；否则看 `args` 是字符串就用它 ✓；都没有 ⇒ 空串 ✓。
pub unsafe fn generic_exception_str(
    ptr: *mut Header,
    instance: &Instance,
) -> Result<String, crate::ExecError> {
    let Some(object) = NonNull::new(ptr) else {
        return Ok(String::new());
    };
    for name in ["message", "text"] {
        if let Ok(Some(value)) =
            crate::executor::attribute::attribute_optional(instance, object, name)
        {
            if let Some(text) = instance.text_of(value) {
                return Ok(text.to_owned());
            }
        }
    }
    Ok(String::new())
}

pub unsafe fn method_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    let function = object.function();
    // **被绑的可能是 native**（第 269 轮加固 ✓）：`dict.__setitem__`／`list.append` 这类是
    // `builtin_function_or_method` ✓，**没有** `code()` ✗ —— 先前一律按 `FunctionObject` 取
    // `code()` ✗ ⇒ 把 native 载荷当 `CodeObject` 读 ⇒ 野读（实测 `print(d.__setitem__)`
    // ⇒ `memory allocation of 8386098843153034355 bytes failed` ✗）。这里先判类型 ✓。
    // SAFETY: function 由方法对象持有。
    let function_type = unsafe { function.as_ref() }.ty();
    let qualname = if Some(function_type) == instance.type_named("function") {
        // SAFETY: 刚确认是 Python 函数。
        let function_ref = unsafe { &*function.as_ptr().cast::<FunctionObject>() };
        // SAFETY: 函数持有 code object 的一份引用。
        let code = function_ref.code();
        // SAFETY: 同上。
        unsafe { code.cast::<crate::CodeObject>().as_ref() }.qualname().to_owned()
    } else {
        // native：取它的 `__qualname__`／`__name__`（都在属性面上 ✓），都没有就用类型名 ✓。
        let from_attribute = |name: &str| -> Option<String> {
            match crate::executor::attribute::attribute_lookup(instance, function, name) {
                Ok(crate::executor::Attribute::Owned(value))
                | Ok(crate::executor::Attribute::Value(value)) => {
                    instance.text_of(value).map(|text| text.to_owned())
                }
                _ => None,
            }
        };
        from_attribute("__qualname__")
            .or_else(|| from_attribute("__name__"))
            .unwrap_or_else(|| instance.type_name(function_type).to_owned())
    };
    Ok(format!(
        "<bound method {qualname} of {}>",
        instance.object_repr(object.this())?
    ))
}

/// 异常实例的 `repr`／`str`：`ValueError('x')`（实测：无参是 `ValueError()`）。
/// **`OM-11` 的 `str` 槽**（异常实例）——**与 `repr` 不同**，形状实测：
///
/// - 没有实参 ⇒ 空串（`str(ValueError())` == `''`）
/// - **一个**实参 ⇒ 那个实参的 `str`（`str(ValueError('x'))` == `'x'`）
/// - 多个实参 ⇒ 实参元组的 `repr`（`str(ValueError('a','b'))` == `"('a', 'b')"`）
///
/// 这条差异是 `T-BC-22` 的行为夹具抓出来的：此前 `str` 槽直接挂了 `exception_repr`，
/// 于是 `str(e)` 与 `repr(e)` 一模一样（`str(ValueError('x'))` 给的是 `"ValueError('x')"`）。
///
/// # Safety
///
/// 契约见 `StrFn`。
pub unsafe fn exception_str(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let header = unsafe { &*ptr };
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    let args = object.args();
    // `KeyError` 是**唯一**在单实参上改口径的：`str(KeyError('k'))` == `"'k'"`（实参的 repr）。
    // 实测：0 个实参仍是 `''`、≥2 个仍是实参元组的 repr（与基类同）⇒ 只改单实参那一格。
    // 子类继承（参照实现也继承）。
    let key_error_style = match instance.type_named("KeyError") {
        // SAFETY: 类型对象由注册表持有。
        Some(key_error) => instance.is_subtype(header.ty(), key_error),
        None => false,
    };
    match args.len() {
        0 => Ok(String::new()),
        1 if key_error_style => instance.object_repr(args[0]),
        1 => instance.object_str(args[0]),
        _ => {
            let mut rendered = Vec::with_capacity(args.len());
            for argument in args {
                rendered.push(instance.object_repr(argument)?);
            }
            Ok(format!("({})", rendered.join(", ")))
        }
    }
}

/// **`OM-11` 的 `repr` 槽**（异常实例）：`ValueError('x')`／`ValueError()`（实测）。
///
/// # Safety
///
/// 契约见 `ReprFn`。
pub unsafe fn exception_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let header = unsafe { &*ptr };
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    // SAFETY: 类型名由注册表持有。
    let name = unsafe { header.ty().as_ref() }.name();
    let args = object.args();
    let mut rendered = Vec::with_capacity(args.len());
    for argument in args {
        rendered.push(instance.object_repr(argument)?);
    }
    Ok(format!("{name}({})", rendered.join(", ")))
}
