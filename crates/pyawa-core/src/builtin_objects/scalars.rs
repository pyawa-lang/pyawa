//! 本文件是 `builtin_objects` 的 `scalars` 域：标量载荷与方法面（`None`／`bool`／`int`／`float`／`str`／`bytes`／`bytearray`，含 `object` 占位载荷）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;


py_object! {
    /// `None` 的单例载体。*占位*：Python 层类型名与协议随后补。
    pub struct NoneObject {}
}

py_object! {
    /// `True`／`False` 的单例载体。
    pub struct BoolObject {
        /// 真假。
        value: bool,
    }
}

py_object! {
    /// `int` 的载体（`TS-45`）：小整数内联、大整数走堆上的 `BigInt`——
    /// **同一个类型对象**的两种载荷（`type(2**100) is int`）。
    pub struct IntObject {
        /// 载荷。
        value: IntValue,
    }
}

py_object! {
    /// `object` 的实例。*占位*：`object()` 不携带状态。
    pub struct PlainObject {}
}

py_object! {
    /// `float` 的实例（C `double`，与参照实现一致）。
    pub struct FloatObject {
        /// 数值；`inf`／`nan` 照旧。
        value: f64,
    }
}

/// **`str` 的存储表示**（P3-38 S2a）：UTF-8 里加一层"已校验的 WTF-8 字节"分支。
///
/// S2a 只是**把存储升上来**（零行为改动）：现有生产路径仍只构造 [`StrRepr::Utf8`] ✓；
/// [`StrRepr::Wtf8`] 只在显式调用 `Instance::new_str_wtf8` 时出现（测试／S2b 起才大面积用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrRepr {
    /// **合法 UTF-8**（现有形态；`&str` 视图可直接给 ✓）。
    Utf8(String),
    /// **已校验的 WTF-8 字节**（可含孤立代理；**没有** `&str` 视图 ✓）。
    Wtf8(Vec<u8>),
}

/// `str` 的实例（P3-38 S2a：载荷升成能装 WTF-8 的表示）。
///
/// 手写而不用 `py_object!` 的原因：`py_object!` 生成的 `new(ty, String)` 是**既有 8 处调用点**
/// 依赖的签名（`executor/*`、`bootstrap` 等），而本轮**不改那些调用点** ⇒ 这里手写 `new` 保持
/// **同签名**（`String` → [`StrRepr::Utf8`]），另给 [`StrObject::new_wtf8`] 走新分支 ✓。
/// 结构体形状仍守 `OM-5`：`#[repr(C)]`、`header` 是第一字段 ✓。
#[repr(C)]
pub struct StrObject {
    /// **OM-5**：头部必须是第一个字段。
    pub header: crate::Header,
    /// 内容表示（UTF-8 或 WTF-8）。
    repr: StrRepr,
}

// SAFETY: 上面的结构体是 `#[repr(C)]`，且 `header` 是第一个字段（OM-5）。
unsafe impl crate::PyObject for StrObject {
    fn header(&self) -> &crate::Header {
        &self.header
    }
}

py_object! {
    /// `bytes` 的实例（**不可变**字节串；`P1-12` 第一刀）。
    ///
    /// 载荷是 Rust `Vec<u8>`；`TS-43` 说布局自选，所以这里不进 ABI。
    pub struct BytesObject {
        /// 内容。
        value: Vec<u8>,
    }
}

impl BytesObject {
    /// 内容（**借用**）。
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// 见 [`TupleObject::slots`]：载荷里没有对象引用，故只需释放自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
    }
}

/// `bytes` 的 `repr`：`b'abc'`（引号与转义规则见 [`crate::instance::quote_bytes`]，照参照实测）。
pub unsafe fn bytes_repr(_ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*_ptr.cast::<BytesObject>() };
    Ok(crate::instance::quote_bytes(object.value()))
}

/// `bytes` 的 `str`：与 `repr` **同形**（实测 `str(b'abc') == "b'abc'"`），故接同一个实现。
pub unsafe fn bytes_str(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 契约同 `bytes_repr`。
    unsafe { bytes_repr(ptr, instance) }
}

/// `bytes()`：空、`bytes(<整数>)`（该长度的零字节）、`bytes(<bytes>)`／`bytes(<可迭代的整数>)`、
/// `bytes(<str>, <编码>)`（第一刀只认 UTF-8；其余编码如实报 `LookupError`，消息照实测）。
///
/// 每一条消息都来自 `tests/fixture-bytes-3.14.json`（`tools/gen_bytes_fixture.py` 实测导出）。
pub unsafe fn bytes_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let build = |value: Vec<u8>| {
        instance
            .alloc(BytesObject::new(class, value))
            .into_raw()
            .cast::<Header>()
    };
    match args {
        [] => Ok(build(Vec::new())),
        [only] => {
            if let Some(count) = instance.int_of(*only).and_then(|value| value.to_i64()) {
                if count < 0 {
                    return Err(instance.raise_builtin_error("ValueError", "negative count"));
                }
                let Ok(length) = usize::try_from(count) else {
                    return Err(crate::ExecError::Unsupported {
                        opcode: 0,
                        what: "bytes(计数)：计数超出 usize 的形态还没接线",
                    });
                };
                // 大计数要一大块内存：如实报 `MemoryError`（实测那句消息为空），不做静默截断
                if length > MAX_BYTES_LENGTH {
                    return Err(instance.raise_builtin_error("MemoryError", ""));
                }
                return Ok(build(vec![0u8; length]));
            }
            if let Some(bytes) = instance.bytes_value(*only) {
                return Ok(build(bytes.to_vec()));
            }
            if instance.text_value(*only).is_some() {
                // 实测：`bytes('abc')` ⇒ 少了编码参数
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "string argument without an encoding",
                ));
            }
            if instance.float_value(*only).is_some() {
                // 实测：`bytes(1.5)` ⇒ 这条
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "cannot convert 'float' object to bytes",
                ));
            }
            // 可迭代的整数
            let mut out: Vec<u8> = Vec::new();
            // **`bytes`／`bytearray` 直接拷原始字节** ✓（task-22 ✓）：`Lib/re/_compiler.py` 的
            // `charmap = bytes(charmap)`（`charmap` 是 65536 长的 `bytearray` ✓）先前落到
            // `collect_iterable` ✗ ⇒ "只接线了 list／tuple／set／frozenset" ✗
            // （实测 `import logging`／`concurrent.futures` 卡这 ✓）。逐元素造 65536 个 int 对象
            // 既慢又没必要 ✓ ⇒ 走**原始字节**那条 ✓（一处真相：`bytes_value`／`bytearray_of` ✓）。
            let raw: Option<Vec<u8>> = if let Some(value) = instance.bytes_value(*only) {
                Some(value.to_vec())
            } else {
                bytearray_of(instance, *only)
                    .map(|data| unsafe { data.as_ref() }.value().to_vec())
            };
            if let Some(raw) = raw {
                out.extend_from_slice(&raw);
                return Ok(build(out));
            }
            // **任意可迭代对象**（第 406 轮 ✓）：`hmac.py` 导入期就写
            // `bytes((x ^ 0x5C) for x in range(256))`（**生成器** ✓）—— 先前只认
            // `list`／`tuple`／`set`／`frozenset` ✗ ⇒ 报"bytes(<可迭代>)：只接线了…" ✓。
            // 走**迭代器协议**（与 `map`／`filter` 同一处 ✓）；元素是**新引用** ⇒ 用完逐个还 ✓。
            let iterator = instance.iter_object(*only)?;
            let mut elements: Vec<NonNull<Header>> = Vec::new();
            while let Some(item) = instance.advance_iterator(iterator)? {
                elements.push(item);
            }
            // SAFETY: iterator 是 `iter_object` 给的新引用。
            unsafe { instance.release_object(iterator.as_ptr()) };
            for item in elements {
                let number = instance.int_of(item).and_then(|value| value.to_i64());
                let conversion_error = match number {
                    Some(number) if (0..=255).contains(&number) => {
                        out.push(number as u8);
                        None
                    }
                    Some(_) => Some(("ValueError", "bytes must be in range(0, 256)".to_owned())),
                    None => {
                        let name = instance.type_name(instance.type_of(item));
                        Some((
                            "TypeError",
                            format!("'{name}' object cannot be interpreted as an integer"),
                        ))
                    }
                };
                // SAFETY: item 是本函数持有的新引用（来自 `advance_iterator`）。
                unsafe { instance.release_object(item.as_ptr()) };
                if let Some((kind, message)) = conversion_error {
                    return Err(instance.raise_builtin_error(kind, &message));
                }
            }
            Ok(build(out))
        }
        [only, encoding] => {
            // **P3-38 S3a**：输入是 WTF-8（可能含孤立代理）⇒ 走核心编码器；两参形态是
            // **strict**（`bytes('\udcff', 'utf-8')` ⇒ `UnicodeEncodeError: surrogates not allowed` ✓）。
            let Some(text) = instance.wtf8_of(*only).map(<[u8]>::to_vec) else {
                let name = instance.type_name(instance.type_of(*only));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("cannot convert '{name}' object to bytes"),
                ));
            };
            let Some(name) = instance.text_value(*encoding) else {
                return Err(instance.raise_builtin_error("TypeError", "encoding must be a string"));
            };
            match name.to_ascii_lowercase().replace('_', "-").as_str() {
                // 第一刀只接 UTF-8（其余编码如实 LookupError ✓）。
                "utf-8" | "utf8" | "u8" => {
                    Ok(build(instance.encode_with_builtin_codec(&text, "utf-8", "strict")?))
                }
                other => Err(instance.raise_builtin_error(
                    "LookupError",
                    &format!("unknown encoding: {other}"),
                )),
            }
        }
        _ => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "bytes_new：三个以上实参的形态还没接线",
        }),
    }
}

/// **`bytes` 的方法面**（`P1-12`；按 oracle 逐批）。
///
/// 机制与生成器族同一个（`OM-11` 的 `getattr` 槽）：**现造**一个绑定方法对象交出去。
/// 每一条的行为与消息都来自 `tests/fixture-bytes-3.14.json` 的 `methods` 段（实测导出）。
pub unsafe fn bytes_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // **字节版覆盖** ✓（第 688 轮）：共享表里的 `translate` 是 **`str` 口径** ✗（收码点映射表 ✓），
    // `bytes`／`bytearray` 要的是**长度 256 的字节表** ✓ ⇒ 这里先换掉 ✓。
    let handler = if name == "translate" {
        Some(bytes_translate_native as NativeFn)
    } else {
        str_method_native(name)
    };
    let Some(handler) = handler else {
        return None;
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(method_type, "bytes", Cell::new(handler)));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}


/// `bytes.translate(table)` ✓（第 688 轮）：**字节版**口径 —— `table` 必须是**长度 256 的字节串** ✓
/// （`table[b]` 给新字节 ✓），不是 `str` 那种码点映射表 ✗（`textwrap` 的 `charmap` 一族靠它 ✓）。
/// **如实记**：`delete` 参数暂未接 ✗（`re`／`textwrap` 都没用到 ✓，随后按需补 ✓）。
pub(crate) fn bytes_translate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = crate::builtin::bytes::bytes_receiver(instance, bound)?;
    let table = match args.first() {
        Some(argument) => instance.bytes_value(*argument).map(<[u8]>::to_vec),
        None => None,
    }
    .ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "a bytes-like object is required for translate")
    })?;
    if table.len() != 256 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "translation table must be 256 characters long",
        ));
    }
    let translated: Vec<u8> = value.iter().map(|byte| table[*byte as usize]).collect();
    Ok(instance.new_bytes(&translated))
}

/// `bytes.startswith(prefix)`／`endswith(suffix)`（实测就是前后缀判断）。
pub(crate) fn starts_ends_with(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    ends: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let other = bytes_argument(instance, args, 0)?;
    let matched = if ends {
        value.ends_with(&other)
    } else {
        value.starts_with(&other)
    };
    Ok(instance.retain(instance.singletons().boolean(matched)))
}

py_object! {
    /// **`bytearray` 的载荷** ✓（第 642 轮）：`RefCell<Vec<u8>>` ✓ —— **可变**是它和 `bytes` 的唯一区别 ✓。
    /// **不持有对象引用** ⇒ 槽只要 `Slots::new(Self::dealloc)` ✓（照 `SliceObject` ✓）。
    pub struct BytearrayObject {
        value: std::cell::RefCell<Vec<u8>>,
    }
}

impl BytearrayObject {
    pub fn value(&self) -> std::cell::RefMut<'_, Vec<u8>> {
        self.value.borrow_mut()
    }
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
    }
}

/// `bytearray([源码])` ✓（第 642 轮）：空 ✓／整数 ⇒ 那么多个零字节 ✓／`bytes` ⇒ 拷贝 ✓／`str` ⇒ UTF-8 ✓。
/// `Lib/re/_compiler.py:252` 的 `bytearray(256)` 与 `:382` 的 `bytearray()` 都要它 ✓。
pub fn bytearray_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let bytes = match args.first().copied() {
        None => Vec::new(),
        Some(value) => {
            let ty = instance.type_of(value);
            if Some(ty) == instance.type_named("int") || ty == instance.singletons().bool_type() {
                let count = instance.index_value(value)?.unwrap_or(0);
                let Some(count) = usize::try_from(count).ok().filter(|count| *count <= 1 << 30) else {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "bytearray 的长度不合法",
                    ));
                };
                vec![0u8; count]
            } else if instance.type_name(ty) == "bytes" {
                // SAFETY: 类型身份已确认。
                unsafe { &*value.as_ptr().cast::<BytesObject>() }.value().to_vec()
            } else if let Some(text) = instance.text_of(value) {
                text.as_bytes().to_vec()
            } else {
                // **任意可迭代对象** ✓（task-22 ✓）：`bytearray(range(256))` 是 `re/_compiler.py` 上游
                // 的形状之一 ✓，先前只认 整数／bytes／str ✗ ⇒ 报到"参数要整数、bytes 或 str" ✗。
                // 元素按 `__index__` 取整数 ✓（与 `bytes(<可迭代>)` 同口径 ✓）；**迭代用统一入口**
                // （`iter_object` ＋ `advance_iterator` ✓ —— 与 `map`／`enumerate` 同一处真相 ✓）。
                let inner = instance.iter_object(value)?;
                let mut out: Vec<u8> = Vec::new();
                while let Some(item) = instance.advance_iterator(inner)? {
                    let Some(number) = instance.index_value(item).ok().flatten() else {
                        let name = instance.type_name(instance.type_of(item));
                        instance.release(inner);
                        return Err(instance.raise_builtin_error(
                            "TypeError",
                            &format!("'{name}' object cannot be interpreted as an integer"),
                        ));
                    };
                    if !(0..=255).contains(&number) {
                        instance.release(inner);
                        return Err(instance.raise_builtin_error(
                            "ValueError",
                            "byte must be in range(0, 256)",
                        ));
                    }
                    out.push(number as u8);
                }
                instance.release(inner);
                out
            }
        }
    };
    Ok(instance
        .alloc(BytearrayObject::new(class, std::cell::RefCell::new(bytes)))
        .into_raw()
        .cast::<Header>())
}

/// `bytearray` 的方法面 ✓（第 642 轮）：`__len__`／`__getitem__`／`__setitem__`／`append`／`extend` ✓。
/// 切片的**读**给 `bytes` ✓（`re._compiler` 要拿它做 `int(bits, 2)`／字典键 ✓）；写支持整数下标与切片 ✓。
/// `bytearray` 的 `repr`／`str` ✓（第 642 轮）：`bytearray(b'xy')` ✓ —— **不能用 `bytes` 那两个** ✗
/// （载荷类型不同 ⇒ 会把 `BytearrayObject` 当 `BytesObject` 读 ✓，实测直接**栈溢出** ✗）。
/// 参照里 `str(bytearray(b"xy"))` 与 `repr(...)` 同文 ✓。转义照 `bytes` 的规矩：可打印 ASCII 原样 ✓、
/// 其余 `\xNN` ✓，引号与反斜杠转义 ✓。
pub unsafe fn bytearray_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, crate::ExecError> {
    // SAFETY: 契约保证 ptr 是 `bytearray` 实例 ✓。
    let data = unsafe { &*ptr.cast::<BytearrayObject>() };
    let values = data.value();
    let mut out = String::from("bytearray(b'");
    for byte in values.iter() {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\'' => out.push_str("\\'"),
            0x20..=0x7e => out.push(*byte as char),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out.push_str("')");
    Ok(out)
}

pub(crate) fn bytearray_iter_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let object = bound.ok_or(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.__iter__ 没有绑定实例" })?;
    let Some(data) = bytearray_of(instance, object) else {
        return Err(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.__iter__ 拿到了别的类型" });
    };
    let bytes = unsafe { data.as_ref() }.value().clone();
    crate::executor::iter_value(instance, instance.new_bytes(&bytes))
}

pub unsafe fn bytearray_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // **`bytes` 的方法面整体复用** ✓（第 677 轮）：`re._compiler` 要 `bytearray.find` ✗ —— 做法是把内容
    // **拷成一份 `bytes`** ✓、再把方法绑到**那一份**上 ✓（`str_method_native` 那张表原样用 ✓，不改一行 ✓）。
    // **如实记**：返回值是 `bytes` 而不是参照的 `bytearray` ✗（`find`／`index`／`count` 这类只回数值的
    // 不受影响 ✓；`split`／`strip` 一类会回 `bytes` ✗ —— 随后按需再收口 ✓）。
    if str_method_native(name).is_some() || name == "translate" {
        // `translate` **不**在 `str_method_native` 那张表里 ✓（它归 `str`／`bytes` 各自的表 ✓），
        // 但 `bytes` 版实现就在本文件里 ✓ ⇒ 一并走"拷成 `bytes` 再绑"这条复用路 ✓。
        // SAFETY: 契约保证 ptr 是 `bytearray` 实例。
        let content = unsafe { &*ptr.cast::<BytearrayObject>() }.value().clone();
        let copy = instance.new_bytes(&content);
        // SAFETY: copy 是刚建好的 `bytes` 对象。
        return unsafe { bytes_getattr(copy.as_ptr(), name, instance) };
    }
    let handler: NativeFn = match name {
        "__len__" => container_len_native,
        "__getitem__" => bytearray_getitem_native,
        "__setitem__" => bytearray_setitem_native,
        // `__iter__`：**借用 `bytes` 的迭代器** ✓（`iter(bytearray)` 在参照里给 `bytearray_iterator` ✗ ——
        // 我们给 `bytes_iterator` ✓，**如实记**这一处偏离 ✗；对 `re` 的用法无差别 ✓）。
        "__iter__" => bytearray_iter_native,
        "append" => bytearray_append_native,
        "extend" => bytearray_extend_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(method_type, "bytearray", Cell::new(handler)));
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

pub(crate) fn bytearray_of(instance: &Instance, object: NonNull<Header>) -> Option<NonNull<crate::builtin_objects::BytearrayObject>> {
    if instance.type_name(instance.type_of(object)) != "bytearray" {
        return None;
    }
    // SAFETY: 类型身份已确认。
    unsafe { Some(NonNull::new_unchecked(object.as_ptr().cast::<BytearrayObject>())) }
}

pub(crate) fn bytearray_getitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let object = bound.ok_or(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.__getitem__ 没有绑定实例" })?;
    let Some(data) = bytearray_of(instance, object) else {
        return Err(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.__getitem__ 拿到了别的类型" });
    };
    let key = args.first().copied().ok_or(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.__getitem__ 要一个实参" })?;
    let values = unsafe { data.as_ref() }.value();
    if instance.type_name(instance.type_of(key)) == "slice" {
        // SAFETY: 类型身份已确认。
        let slice = unsafe { &*key.as_ptr().cast::<SliceObject>() };
        let (start, stop, step) = crate::executor::slice_bounds(instance, key, values.len())?;
        let picked: Vec<u8> = crate::executor::slice_positions(start, stop, step)
            .into_iter()
            .map(|position| values[position])
            .collect();
        let _ = slice;
        return Ok(instance.new_bytes(&picked));
    }
    let Some(index) = instance.index_value(key)? else {
        return Err(instance.raise_builtin_error("TypeError", "bytearray indices must be integers or slices"));
    };
    let len = values.len() as i64;
    let index = if index < 0 { index + len } else { index };
    if index < 0 || index >= len {
        return Err(instance.raise_builtin_error("IndexError", "bytearray index out of range"));
    }
    Ok(instance.new_int(i64::from(values[index as usize])))
}

pub(crate) fn bytearray_setitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let object = bound.ok_or(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.__setitem__ 没有绑定实例" })?;
    let Some(data) = bytearray_of(instance, object) else {
        return Err(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.__setitem__ 拿到了别的类型" });
    };
    if args.len() < 2 {
        return Err(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.__setitem__ 要两个实参" });
    }
    let (key, value) = (args[0], args[1]);
    if instance.type_name(instance.type_of(key)) == "slice" {
        let mut values = unsafe { data.as_ref() }.value();
        let (start, stop, step) = crate::executor::slice_bounds(instance, key, values.len())?;
        let items = crate::executor::sequence_items(instance, value, 0)?;
        let mut bytes = Vec::with_capacity(items.len());
        for item in items {
            let Some(number) = instance.index_value(item)? else {
                return Err(instance.raise_builtin_error("TypeError", "bytearray 的元素要是 0..256 的整数"));
            };
            if !(0..=255).contains(&number) {
                return Err(instance.raise_builtin_error("ValueError", "byte must be in range(0, 256)"));
            }
            bytes.push(number as u8);
        }
        if step == 1 {
            let start = start.max(0) as usize;
            let stop = stop.max(0) as usize;
            let stop = stop.min(values.len());
            values.splice(start..stop, bytes);
        } else {
            let positions: Vec<usize> = crate::executor::slice_positions(start, stop, step);
            if positions.len() != bytes.len() {
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    "bytearray 的扩展切片赋值要求长度相等",
                ));
            }
            for (position, byte) in positions.into_iter().zip(bytes) {
                values[position] = byte;
            }
        }
        return Ok(instance.retain(instance.singletons().none()));
    }
    let Some(index) = instance.index_value(key)? else {
        return Err(instance.raise_builtin_error("TypeError", "bytearray indices must be integers or slices"));
    };
    let Some(number) = instance.index_value(value)? else {
        return Err(instance.raise_builtin_error("TypeError", "bytearray 的元素要是 0..256 的整数"));
    };
    if !(0..=255).contains(&number) {
        return Err(instance.raise_builtin_error("ValueError", "byte must be in range(0, 256)"));
    }
    let mut values = unsafe { data.as_ref() }.value();
    let len = values.len() as i64;
    let index = if index < 0 { index + len } else { index };
    if index < 0 || index >= len {
        return Err(instance.raise_builtin_error("IndexError", "bytearray assignment index out of range"));
    }
    values[index as usize] = number as u8;
    Ok(instance.retain(instance.singletons().none()))
}

pub(crate) fn bytearray_append_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let object = bound.ok_or(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.append 没有绑定实例" })?;
    let Some(data) = bytearray_of(instance, object) else {
        return Err(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.append 拿到了别的类型" });
    };
    let value = args.first().copied().ok_or(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.append 要一个实参" })?;
    let Some(number) = instance.index_value(value)? else {
        return Err(instance.raise_builtin_error("TypeError", "bytearray 的元素要是 0..256 的整数"));
    };
    if !(0..=255).contains(&number) {
        return Err(instance.raise_builtin_error("ValueError", "byte must be in range(0, 256)"));
    }
    unsafe { data.as_ref() }.value().push(number as u8);
    Ok(instance.retain(instance.singletons().none()))
}

pub(crate) fn bytearray_extend_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let object = bound.ok_or(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.extend 没有绑定实例" })?;
    let Some(data) = bytearray_of(instance, object) else {
        return Err(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.extend 拿到了别的类型" });
    };
    let value = args.first().copied().ok_or(crate::ExecError::Unsupported { opcode: 0, what: "bytearray.extend 要一个实参" })?;
    let items = crate::executor::sequence_items(instance, value, 0)?;
    let mut bytes = Vec::with_capacity(items.len());
    for item in items {
        let Some(number) = instance.index_value(item)? else {
            return Err(instance.raise_builtin_error("TypeError", "bytearray 的元素要是 0..256 的整数"));
        };
        if !(0..=255).contains(&number) {
            return Err(instance.raise_builtin_error("ValueError", "byte must be in range(0, 256)"));
        }
        bytes.push(number as u8);
    }
    unsafe { data.as_ref() }.value().extend(bytes);
    Ok(instance.retain(instance.singletons().none()))
}

/// **`str` 方法面的"名字 → native"查表** ✓（第 212 轮抽出 ✓，**一处真相** ✓）。
///
/// 两处共用它 ✓：① `str_getattr`（取**绑定**方法 ✓）；② 把某个名字挂进 **`str` 的类型字典** ✓
/// —— `Lib/types.py:52` 要 `type(str.join)` ✓，而方法面只挂在 `getattr` 槽上 ✗ ⇒ 类级取法取不到 ✗。
pub fn str_method_native(name: &str) -> Option<NativeFn> {
    Some(match name {
        "hex" => bytes_hex_native,
        "decode" => bytes_decode_native,
        "startswith" => bytes_startswith_native,
        "endswith" => bytes_endswith_native,
        "find" => bytes_find_native,
        "count" => bytes_count_native,
        "replace" => bytes_replace_native,
        "upper" => bytes_upper_native,
        "lower" => bytes_lower_native,
        "strip" => bytes_strip_native,
        "split" => bytes_split_native,
        "join" => bytes_join_native,
        "rfind" => bytes_rfind_native,
        "index" => bytes_index_native,
        "rindex" => bytes_rindex_native,
        "removeprefix" => bytes_removeprefix_native,
        "removesuffix" => bytes_removesuffix_native,
        "lstrip" => bytes_lstrip_native,
        "rstrip" => bytes_rstrip_native,
        "zfill" => bytes_zfill_native,
        // **`ljust`／`rjust`／`center`**（task-46；`Lib/hmac.py:136` 要 `bytes.ljust` ✓）。
        "ljust" => crate::builtin::bytes::bytes_ljust_native,
        "rjust" => crate::builtin::bytes::bytes_rjust_native,
        "center" => crate::builtin::bytes::bytes_center_native,
        "splitlines" => bytes_splitlines_native,
        "isdigit" => bytes_isdigit_native,
        "isspace" => bytes_isspace_native,
        // **与 `in` 同一处实现**（第 303 轮）：`bytes_contains_native` 只认 bytes 类实参 ✗ ⇒
        // `b"abc".__contains__(98)` 会报"a bytes-like object is required" ✗（参照给 `True` ✓）。
        "__contains__" => container_contains_native,
        _ => return None,
    })
}

pub unsafe fn str_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "upper" => str_upper_native,
        // **`str.format`**（第 722 轮 ✓）：`Lib/base64.py:246` **导入期**就调它 ✗ ⇒
        // 先前 `'str' object has no attribute 'format'` 压住 `base64` ⇒ email 一族 ✓。
        "format" => str_format_native,
        "__contains__" => container_contains_native,
        "lower" => str_lower_native,
        "strip" => str_strip_native,
        "startswith" => str_startswith_native,
        "endswith" => str_endswith_native,
        "join" => str_join_native,
        "split" => str_split_native,
        "replace" => str_replace_native,
        "find" => str_find_native,
        // **第 349 轮补**：`rfind`／`index`／`rindex`／`rpartition` —— 与 `find`／`partition` 同源 ✓
        //（`Lib/` 里常用 ✓，先前一律 AttributeError ✗）。
        "rfind" => str_rfind_native,
        "index" => str_index_native,
        "rindex" => str_rindex_native,
        "rpartition" => str_rpartition_native,
        // **第 351 轮补**：`translate`（`maketrans` 早已在 ✓，对拍时发现 `translate` 缺 ✗）。
        "translate" => str_translate_native,
        // **第 350 轮补**：`isprintable`（`"\t"` 不算可打印 ✓）＋ `istitle`（"每个词首字母大写、
        // 其余小写" ✓，`"A1b".istitle()` ⇒ `False` ✓ 照参照实测 ✓）。
        "encode" => str_encode_native,
        "isprintable" => str_isprintable_native,
        "istitle" => str_istitle_native,
        "count" => str_count_native,
        "isdigit" => str_isdigit_native,
        "isalpha" => str_isalpha_native,
        // **第 335 轮补**：`isidentifier`（上限榜上 70 个模块卡它 ✓）＋ 常一起用的 `isascii` ✓。
        "isidentifier" => str_isidentifier_native,
        "isascii" => str_isascii_native,
        // **第 344 轮补的一批**（`Lib/` 里到处都是 ✓）：大小写、数字、字母数字、交换大小写、
        // casefold、expandtabs ✓。
        "isupper" => str_isupper_native,
        "islower" => str_islower_native,
        "isnumeric" => str_isnumeric_native,
        "isdecimal" => str_isdecimal_native,
        "isalnum" => str_isalnum_native,
        "isspace" => crate::builtin::str::str_isspace_native,
        "swapcase" => str_swapcase_native,
        "casefold" => str_casefold_native,
        "expandtabs" => str_expandtabs_native,
        "zfill" => str_zfill_native,
        "splitlines" => str_splitlines_native,
        "removeprefix" => str_removeprefix_native,
        "removesuffix" => str_removesuffix_native,
        "rjust" => str_rjust_native,
        "ljust" => str_ljust_native,
        "center" => str_center_native,
        "partition" => str_partition_native,
        "rsplit" => str_rsplit_native,
        "lstrip" => str_lstrip_native,
        "rstrip" => str_rstrip_native,
        "title" => str_title_native,
        "capitalize" => str_capitalize_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "str",
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

/// 取绑定 `self` 的字符串（方法契约保证有 ✓）。
pub(crate) fn bound_text(instance: &Instance, bound: Option<NonNull<Header>>) -> Result<String, crate::ExecError> {
    let Some(bound) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor needs an argument"));
    };
    // SAFETY: 绑定的是本类型的存活对象。P3-38 S2c：安全口径（WTF-8 ⇒ 有损文本，不 panic ✓）
    Ok(instance.text_value(bound).unwrap_or_default())
}

/// 取一个**字符串实参**（不是 str ⇒ 与参照同形的 `TypeError` ✓）。
pub(crate) fn text_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    index: usize,
    what: &str,
) -> Result<String, crate::ExecError> {
    let Some(value) = args.get(index) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{what}() takes at least {} argument", index + 1),
        ));
    };
    match instance.text_of(*value) {
        Some(text) => Ok(text.to_owned()),
        None => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{what}(): expected str"),
        )),
    }
}

/// `startswith`／`endswith` 的**前缀／后缀集** ✓（第 195 轮，**一处真相** ✓）：`str` 直接给 ✓；
/// **`tuple`** 逐个取文本 ✓（CPython 只收元组 ✓，别的类型照样报 `expected str` ✓）。
pub(crate) fn text_prefixes(
    instance: &Instance,
    args: &[NonNull<Header>],
    name: &str,
) -> Result<Vec<String>, crate::ExecError> {
    let Some(first) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes at least 1 argument"),
        ));
    };
    let is_tuple = instance.type_name(unsafe { first.as_ref() }.ty()) == "tuple";
    if is_tuple {
        let Some(items) = instance.iterable_items(*first) else {
            return Err(instance.raise_builtin_error("TypeError", &format!("{name}(): expected str")));
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let Some(text) = instance.text_of(item) else {
                return Err(
                    instance.raise_builtin_error("TypeError", &format!("{name}(): expected str"))
                );
            };
            out.push(text.to_owned());
        }
        return Ok(out);
    }
    let Some(text) = instance.text_of(*first) else {
        return Err(instance.raise_builtin_error("TypeError", &format!("{name}(): expected str")));
    };
    Ok(vec![text.to_owned()])
}


/// **`bytes` 的长度上限**（实现上限，写进规格的"未定"栏）：`1 << 30` ＝ 1 GiB。
///
/// 与位移那处同一个道理：Rust 的分配失败是**中止进程**，不能拿它当错误通道，
/// 所以先自设一条线，超线报实测同款的 `MemoryError`（消息为空）。
pub(crate) const MAX_BYTES_LENGTH: usize = 1 << 30;

impl FloatObject {
    /// 数值。
    pub fn value(&self) -> f64 {
        self.value
    }
}

impl StrObject {
    /// 构造一个 **UTF-8** 形态的 `str`（与 `py_object!` 生成的 `new` **同签名** ✓）。
    pub fn new(ty: NonNull<crate::TypeObject>, value: String) -> Self {
        Self {
            header: crate::Header::new(ty),
            repr: StrRepr::Utf8(value),
        }
    }

    /// 构造一个 **WTF-8** 形态的 `str`：`bytes` 必须是合法 WTF-8（用 S1 的
    /// [`crate::wtf8::validate`] 校验）；非法返回 `None`（**不**静默按有损转换收下 ✓）。
    ///
    /// **合法 UTF-8 的字节照旧存 [`StrRepr::Utf8`]**（零行为改动：`value()` 视图仍可用 ✓）；
    /// 只有**含孤立代理**这类"WTF-8 合法、UTF-8 非法"的字节才进 [`StrRepr::Wtf8`] ✓。
    pub fn new_wtf8(ty: NonNull<crate::TypeObject>, bytes: Vec<u8>) -> Option<Self> {
        crate::wtf8::validate(&bytes).ok()?;
        let repr = match String::from_utf8(bytes) {
            Ok(text) => StrRepr::Utf8(text),
            Err(error) => StrRepr::Wtf8(error.into_bytes()),
        };
        Some(Self {
            header: crate::Header::new(ty),
            repr,
        })
    }

    /// 内容（**`&str` 兼容视图**）。
    ///
    /// # Panics
    ///
    /// [`StrRepr::Wtf8`] 分支**没有** `&str` 视图（孤立代理装不进 Rust `str`）⇒ 会 panic。
    /// **S2a 的生产路径不会构造该分支** ✓；代码里要用"可能是 WTF-8"的对象时走
    /// [`StrObject::as_utf8`]（`Option`）或 [`StrObject::wtf8_bytes`]（总是可用）✓。
    pub fn value(&self) -> &str {
        match &self.repr {
            StrRepr::Utf8(text) => text,
            StrRepr::Wtf8(_) => panic!(
                "StrObject::value() 不适用于 WTF-8 表示（含孤立代理；S2a 生产路径不构造该分支；\
                 请改用 as_utf8()/wtf8_bytes()）"
            ),
        }
    }

    /// **WTF-8 字节**（总是可用：UTF-8 就是合法 WTF-8 ✓；WTF-8 分支给原始字节 ✓）。
    pub fn wtf8_bytes(&self) -> &[u8] {
        match &self.repr {
            StrRepr::Utf8(text) => text.as_bytes(),
            StrRepr::Wtf8(bytes) => bytes,
        }
    }

    /// **合法 UTF-8 视图**：UTF-8 分支给 `Some(&str)`；WTF-8 分支给 `None` ✓。
    pub fn as_utf8(&self) -> Option<&str> {
        match &self.repr {
            StrRepr::Utf8(text) => Some(text),
            StrRepr::Wtf8(_) => None,
        }
    }

    /// 内部表示（诊断／测试用；**不**是稳定 API 面 ✓）。
    pub fn repr(&self) -> &StrRepr {
        &self.repr
    }

    /// 是否为空串（`OM-23` 的空串单例就是它）。
    pub fn is_empty(&self) -> bool {
        match &self.repr {
            StrRepr::Utf8(text) => text.is_empty(),
            StrRepr::Wtf8(bytes) => bytes.is_empty(),
        }
    }

    /// **OM-11** 的 `dealloc` 槽位（与 `py_object!` 生成物同形 ✓）。
    ///
    /// # Safety
    ///
    /// `ptr` 必须来自本类型的一次 `Instance::alloc`；引用计数已归零。
    pub unsafe fn dealloc(ptr: *mut crate::Header) {
        // SAFETY: 由调用方保证（OM-5 保证头部就在对象地址上）。
        unsafe { core::mem::drop(Box::from_raw(ptr.cast::<StrObject>())) }
    }
}
