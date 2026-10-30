//! 本文件是 `builtin_objects` 的 `memoryview` 域：`memoryview` 的最小面（字段、构造、`cast`／`tobytes`／`tolist` 与下标）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;


// **`memoryview` 最小面** ✓（task-22 ✓）

/// 格式字符 ⇒ 每项字节数 ✓（**如实范围** ✗：只认整数格式 ✓，且按**本机小端**解释 ✓）。
pub(crate) fn memoryview_itemsize(format: &str) -> Option<usize> {
    match format {
        "B" | "b" | "c" => Some(1),
        "H" | "h" => Some(2),
        "I" | "i" | "L" | "l" => Some(4),
        "Q" | "q" | "N" | "n" => Some(8),
        _ => None,
    }
}

/// 取内存视图实例的字段（`_source`／`_format`／`_itemsize`／`_offset`／`_nbytes` ✓）。
///
/// 返回 `(源对象, 格式, 每项字节, 起始字节, 字节数)` ✓ —— 源对象是**借用**（实例字典持着它 ✓）。
pub(crate) fn memoryview_fields(
    instance: &Instance,
    object: NonNull<Header>,
) -> Option<(NonNull<Header>, String, usize, usize, usize)> {
    if instance.type_name(instance.type_of(object)) != "memoryview" {
        return None;
    }
    // SAFETY: 类型身份已确认 ⇒ 是 `AttributeObject`（本文件的 `memoryview_new` 就是这么造的 ✓）。
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let dict = view.attributes()?;
    let source = instance.dict_get(dict, "_source")?;
    let format = instance.dict_get(dict, "_format").and_then(|value| instance.text_of(value))?.to_owned();
    let itemsize = instance
        .dict_get(dict, "_itemsize")
        .and_then(|value| instance.int_of(value))
        .and_then(|value| value.to_i64())? as usize;
    let offset = instance
        .dict_get(dict, "_offset")
        .and_then(|value| instance.int_of(value))
        .and_then(|value| value.to_i64())? as usize;
    let nbytes = instance
        .dict_get(dict, "_nbytes")
        .and_then(|value| instance.int_of(value))
        .and_then(|value| value.to_i64())? as usize;
    Some((source, format, itemsize, offset, nbytes))
}

/// 读源对象的**字节副本** ✓（只认 `bytes`／`bytearray` ✓ —— 如实范围 ✗）。
pub(crate) fn memoryview_source_bytes(instance: &Instance, source: NonNull<Header>) -> Option<Vec<u8>> {
    if let Some(value) = instance.bytes_value(source) {
        return Some(value.to_vec());
    }
    bytearray_of(instance, source).map(|data| unsafe { data.as_ref() }.value().to_vec())
}

/// 往源 `bytearray` 写回 ✓（`bytes` 不可写 ⇒ `None` ✓）。
pub(crate) fn memoryview_write_source(instance: &Instance, source: NonNull<Header>, offset: usize, data: &[u8]) -> bool {
    let Some(target) = bytearray_of(instance, source) else {
        return false;
    };
    let mut values = unsafe { target.as_ref() }.value();
    if offset + data.len() > values.len() {
        return false;
    }
    values[offset..offset + data.len()].copy_from_slice(data);
    true
}

/// **`memoryview` 的属性面** ✓（task-22 ✓；与 `bytearray_getattr` 同款 `getattr` 槽手法 ✓ —— 本层
/// `alloc_type_raw` 建的内建类型**不靠 MRO 查字典** ✗ ⇒ 方法要走 `getattr` 槽 ✓）。
pub unsafe fn memoryview_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 契约保证 ptr 是本类型的存活对象。
    let object = unsafe { NonNull::new_unchecked(ptr) };
    let fields = memoryview_fields(instance, object)?;
    let (source, format, itemsize, _offset, nbytes) = fields;
    match name {
        // **简单属性** ✓（`_bytes_to_codes` 就要 `itemsize` ✓）。
        "itemsize" => Some(instance.new_int(itemsize as i64)),
        "nbytes" => Some(instance.new_int(nbytes as i64)),
        "format" => Some(instance.new_str(&format)),
        // 源是 `bytes` ⇒ **只读** ✓；源是 `bytearray` ⇒ 可写 ✓（如实：不做导出锁 ✓）。
        "readonly" => Some(if instance.bytes_value(source).is_some() {
            instance.new_bool(true)
        } else {
            instance.new_bool(false)
        }),
        "obj" => {
            // SAFETY: source 由视图持有，存活；这里交出新的一份。
            unsafe { instance.incref_object(source.as_ptr()) };
            Some(source)
        }
        _ => {
            let handler: crate::NativeFn = match name {
                "cast" => memoryview_cast_native,
                "tobytes" => memoryview_tobytes_native,
                "tolist" => memoryview_tolist_native,
                "release" => memoryview_release_native,
                "__len__" => memoryview_len_native,
                "__getitem__" => memoryview_getitem_native,
                "__setitem__" => memoryview_setitem_native,
                _ => return None,
            };
            let method_type = instance
                .type_named("builtin_function_or_method")
                .expect("引导期已登记");
            // 名字取**静态**串 ✓（`BuiltinFunctionObject` 要 `&'static str` ✓ —— 与 `bytearray_getattr`
            // 同款：函数名给类型名 ✓，方法名由 `MethodObject` 那边呈现 ✓）。
            let native = instance.alloc(BuiltinFunctionObject::new(
                method_type,
                "memoryview",
                core::cell::Cell::new(handler),
            ));
            let native_raw = native.into_raw().cast::<Header>();
            // SAFETY: 方法对象要自己那份 self（`OM-16`）。
            unsafe { instance.incref_object(ptr) };
            let bound = instance.alloc(MethodObject::new(
                instance.type_named("method").expect("method 已登记"),
                native_raw,
                object,
            ));
            Some(bound.into_raw().cast::<Header>())
        }
    }
}

/// **`memoryview(<bytes|bytearray>)` 的实例化槽** ✓（与 `bytearray_new` 同签名 ✓）。
pub fn memoryview_call_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(source) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "memoryview() missing required argument 'object' (pos 1)",
        ));
    };
    memoryview_new(instance, source)
}

/// 造一个 `memoryview`（**共享底层** ✓：只存源对象 ＋ 偏移／格式 ✓）。
pub fn memoryview_new(
    instance: &Instance,
    source: NonNull<Header>,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(bytes) = memoryview_source_bytes(instance, source) else {
        let name = instance.type_name(instance.type_of(source));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("memoryview: a bytes-like object is required, not '{name}'"),
        ));
    };
    let view_type = instance
        .type_named("memoryview")
        .unwrap_or_else(|| instance.new_attribute_type("memoryview"));
    // **惰性装方法** ✓（幂等 ✓，与 `GenericAlias.__mro_entries__` 同款手法 ✓）。
    if let Some(namespace) = instance.type_namespace(view_type.cast()) {
        if instance.dict_get(namespace, "cast").is_none() {
            let function_type = instance
                .type_named("builtin_function_or_method")
                .expect("builtin_function_or_method 在引导期已登记");
            for (name, function) in [
                ("cast", memoryview_cast_native as crate::NativeFn),
                ("tobytes", memoryview_tobytes_native as crate::NativeFn),
                ("tolist", memoryview_tolist_native as crate::NativeFn),
                ("release", memoryview_release_native as crate::NativeFn),
                ("__len__", memoryview_len_native as crate::NativeFn),
                ("__getitem__", memoryview_getitem_native as crate::NativeFn),
                ("__setitem__", memoryview_setitem_native as crate::NativeFn),
            ] {
                let builtin = instance.alloc(BuiltinFunctionObject::new(
                    function_type,
                    name,
                    core::cell::Cell::new(function),
                ));
                instance.dict_set(namespace, name, builtin.into_raw().cast::<Header>());
            }
        }
    }
    let object = instance
        .alloc(AttributeObject::new(
            view_type,
            core::cell::RefCell::new(Some(instance.new_dict())),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: `memoryview_new` 刚造的就是 `AttributeObject`。
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let Some(dict) = view.attributes() else {
        return Err(instance.raise_builtin_error("RuntimeError", "memoryview 没有实例字典"));
    };
    // 源对象**留一份引用** ✓（视图要活过调用方那份 ✓）。
    // SAFETY: source 由调用方保证存活。
    unsafe { instance.incref_object(source.as_ptr()) };
    instance.dict_set(dict, "_source", source);
    let format = instance.new_str("B");
    instance.dict_set(dict, "_format", format);
    let itemsize = instance.new_int(1);
    instance.dict_set(dict, "_itemsize", itemsize);
    let offset = instance.new_int(0);
    instance.dict_set(dict, "_offset", offset);
    let nbytes = instance.new_int(bytes.len() as i64);
    instance.dict_set(dict, "_nbytes", nbytes);
    Ok(object)
}

/// **`memoryview.cast(fmt)`** ✓：重解释视图（**共享底层** ✓，只换格式／项宽 ✓）。
pub(crate) fn memoryview_cast_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "cast 没有绑定实例"));
    };
    let Some((source, _format, _itemsize, offset, nbytes)) = memoryview_fields(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "cast 只接 memoryview"));
    };
    let Some(format) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "cast 要一个格式串"));
    };
    let Some(itemsize) = memoryview_itemsize(format) else {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("memoryview: destination format must be a native single character format, not '{format}'"),
        ));
    };
    if nbytes % itemsize != 0 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "memoryview: length is not a multiple of itemsize",
        ));
    }
    let object = memoryview_new(instance, source)?;
    // SAFETY: 刚造的就是 `AttributeObject`。
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let Some(dict) = view.attributes() else {
        return Err(instance.raise_builtin_error("RuntimeError", "memoryview 没有实例字典"));
    };
    let format_value = instance.new_str(format);
    instance.dict_set(dict, "_format", format_value);
    let itemsize_value = instance.new_int(itemsize as i64);
    instance.dict_set(dict, "_itemsize", itemsize_value);
    let offset_value = instance.new_int(offset as i64);
    instance.dict_set(dict, "_offset", offset_value);
    if offset + nbytes > 0 {
        let nbytes_value = instance.new_int(nbytes as i64);
        instance.dict_set(dict, "_nbytes", nbytes_value);
    }
    Ok(object)
}

/// **`memoryview.tobytes()`** ✓：把（偏移／长度范围内的）字节拷出来 ✓。
pub(crate) fn memoryview_tobytes_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "tobytes 没有绑定实例"));
    };
    let Some((source, _format, _itemsize, offset, nbytes)) = memoryview_fields(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "tobytes 只接 memoryview"));
    };
    let Some(bytes) = memoryview_source_bytes(instance, source) else {
        return Err(instance.raise_builtin_error("TypeError", "tobytes 的源不可读"));
    };
    let end = (offset + nbytes).min(bytes.len());
    Ok(instance.new_bytes(&bytes[offset.min(end)..end]))
}

/// **`memoryview.tolist()`** ✓：按格式逐项取整数 ✓（`_bytes_to_codes` 要它 ✓）。
pub(crate) fn memoryview_tolist_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "tolist 没有绑定实例"));
    };
    let Some((source, _format, itemsize, offset, nbytes)) = memoryview_fields(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "tolist 只接 memoryview"));
    };
    let Some(bytes) = memoryview_source_bytes(instance, source) else {
        return Err(instance.raise_builtin_error("TypeError", "tolist 的源不可读"));
    };
    let end = (offset + nbytes).min(bytes.len());
    let slice = &bytes[offset.min(end)..end];
    let mut items: Vec<NonNull<Header>> = Vec::new();
    for chunk in slice.chunks_exact(itemsize.max(1)) {
        // **本机小端** ✓（如实范围 ✗：不做端序参数 ✓）。
        let mut value: u64 = 0;
        for (index, byte) in chunk.iter().enumerate() {
            value |= (*byte as u64) << (8 * index);
        }
        items.push(instance.new_int(value as i64));
    }
    Ok(instance.new_list(items))
}

/// **`memoryview.release()`** ✓：最小面（本层视图不持导出锁 ✓，如实登记 ✓）。
pub(crate) fn memoryview_release_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if bound.is_none() {
        return Err(instance.raise_builtin_error("TypeError", "release 没有绑定实例"));
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`len(memoryview)`** ✓：**项数** ＝ 字节数 ÷ 项宽 ✓。
pub(crate) fn memoryview_len_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "len 没有绑定实例"));
    };
    let Some((_source, _format, itemsize, _offset, nbytes)) = memoryview_fields(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "len 只接 memoryview"));
    };
    Ok(instance.new_int((nbytes / itemsize.max(1)) as i64))
}

/// **`memoryview[i]`** ✓：整数 ⇒ 一项整数 ✓；切片 ⇒ 列表（**如实范围** ✗：参照给新 `memoryview` ✓）。
pub(crate) fn memoryview_getitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__getitem__ 没有绑定实例"));
    };
    let Some((source, _format, itemsize, offset, nbytes)) = memoryview_fields(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "__getitem__ 只接 memoryview"));
    };
    let Some(bytes) = memoryview_source_bytes(instance, source) else {
        return Err(instance.raise_builtin_error("TypeError", "__getitem__ 的源不可读"));
    };
    let count = nbytes / itemsize.max(1);
    let end = (offset + nbytes).min(bytes.len());
    let slice = &bytes[offset.min(end)..end];
    let Some(index) = args.first().and_then(|value| instance.index_value(*value).ok().flatten()) else {
        return Err(instance.raise_builtin_error("TypeError", "memoryview 只接整数下标（切片随后补）"));
    };
    let index = if index < 0 { index + count as i64 } else { index };
    if index < 0 || index as usize >= count {
        return Err(instance.raise_builtin_error("IndexError", "index out of bounds"));
    }
    let start = index as usize * itemsize;
    let chunk = &slice[start..(start + itemsize).min(slice.len())];
    let mut value: u64 = 0;
    for (position, byte) in chunk.iter().enumerate() {
        value |= (*byte as u64) << (8 * position);
    }
    Ok(instance.new_int(value as i64))
}

/// **`memoryview[i] = v`** ✓：写回源 `bytearray` ✓（**镜像** ✓；`bytes` 源 ⇒ `TypeError` ✓）。
pub(crate) fn memoryview_setitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__setitem__ 没有绑定实例"));
    };
    let Some((source, _format, itemsize, offset, nbytes)) = memoryview_fields(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "__setitem__ 只接 memoryview"));
    };
    let count = nbytes / itemsize.max(1);
    let Some(index) = args
        .first()
        .and_then(|value| instance.index_value(*value).ok().flatten())
    else {
        return Err(instance.raise_builtin_error("TypeError", "memoryview 下标要是整数"));
    };
    let Some(value) = args.get(1).and_then(|value| instance.index_value(*value).ok().flatten()) else {
        return Err(instance.raise_builtin_error("TypeError", "memoryview 赋值要是整数"));
    };
    let index = if index < 0 { index + count as i64 } else { index };
    if index < 0 || index as usize >= count {
        return Err(instance.raise_builtin_error("IndexError", "index out of bounds"));
    }
    let mut raw = vec![0u8; itemsize];
    for (position, byte) in raw.iter_mut().enumerate() {
        *byte = ((value as u64) >> (8 * position)) as u8;
    }
    let start = offset + index as usize * itemsize;
    if !memoryview_write_source(instance, source, start, &raw) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "cannot modify read-only memory",
        ));
    }
    Ok(instance.retain(instance.singletons().none()))
}
