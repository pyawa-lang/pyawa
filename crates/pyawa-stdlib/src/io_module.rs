//! `_io` 模块（第 194 轮起步，第 288 轮补到"`Lib/io.py` 能 import"的面）。
//!
//! `Lib/importlib/_bootstrap_external.py`（`import _io`）与 `Lib/site.py`（`import _io as io`）都要
//! **先导入成功**；`Lib/io.py` 是 M3 那一族（`io.DEFAULT_BUFFER_SIZE` ⇒ 上游 **45** 个模块 ✓）的**头**。
//!
//! **本层给什么** ✓（口径 `CM-6`：名字齐、真做不了的**调用时如实报未实现** ✗）：
//! - **常量**：`DEFAULT_BUFFER_SIZE`（`8192` ✓ 参照实测 ✓）；
//! - **异常**：`BlockingIOError`（内建表里就有 ✓）、`UnsupportedOperation`（**如实说** ✗：
//!   参照是 `OSError`＋`ValueError` 的子类 ✓，本层先**指向 `OSError`** ✓ —— 比"名字没有"强 ✓、
//!   比"另造一个异常类型"省 ✓，随后按 `P3-14` 补真类型 ✓）；
//! - **类型**：`_IOBase`／`_RawIOBase`／`_BufferedIOBase`／`_TextIOBase` 与 `FileIO`／`BytesIO`／
//!   `StringIO`／`BufferedReader`／`BufferedWriter`／`BufferedRWPair`／`BufferedRandom`／
//!   `IncrementalNewlineDecoder` —— **占位类型** ✓（能当基类、能 `isinstance` ✓，
//!   实例化按"不能创建实例"报错 ✓）；`TextIOWrapper` 用**真的那个**（`sys.stdout` 那类文本流的类型 ✓）；
//! - **函数**：`open`／`open_code`／`text_encoding` —— 名字齐 ✓、调用时**如实报未实现** ✗
//!   （真文件读写按 `CM-8` 必须走 `fs` 能力域 ✓）。
//!
//! `CX-4`：本 crate 不碰平台 ⇒ 这里不 import 任何平台接口 ✓。

use core::cell::Cell;
use core::ptr::NonNull;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance, NativeFn};

use crate::_io_module::TEXT_WRAPPER;

/// 模块名（`_io`）。
pub const NAME: &str = "_io";

/// `__doc__`（与 `_io_module` 那句一致）。
pub const DOC: &str = "The io module provides the Python interfaces to stream handling.";

/// **占位类型**（能当基类 ✓、实例化如实报错 ✓）——名字照 `Lib/io.py:57` 的 `from _io import (…)` ✓。
const PLACEHOLDER_TYPES: [&str; 12] = [
    "_IOBase",
    "_RawIOBase",
    "_BufferedIOBase",
    "_TextIOBase",
    "FileIO",
    "BytesIO",
    "StringIO",
    "BufferedReader",
    "BufferedWriter",
    "BufferedRWPair",
    "BufferedRandom",
    "IncrementalNewlineDecoder",
];

/// **只占名字**的函数（调用时如实报未实现 ✓，`CM-6`）。
const STUB_FUNCTIONS: [&str; 3] = ["open", "open_code", "text_encoding"];

/// 占位 native：调用即**如实报未实现** ✗（真 I/O 走 `fs` 能力域 ✓）。
fn not_implemented_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_io` 的真文件／流实现尚未接线（按 `CM-8` 要走 `fs` 能力域，见 `P3-14`）",
    ))
}

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 建 `_io` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // **常量**：参照实测 `_io.DEFAULT_BUFFER_SIZE == 131072` ✓（128 KiB ✓）
    let size = instance.new_int(131072);
    instance.dict_set(namespace, "DEFAULT_BUFFER_SIZE", size);
    // **异常**
    if let Some(ty) = instance.type_named("BlockingIOError") {
        instance.dict_set(namespace, "BlockingIOError", instance.type_value(ty));
    }
    if let Some(ty) = instance.type_named("OSError") {
        // **如实说** ✗：参照的 `UnsupportedOperation` 是 `OSError`＋`ValueError` 的子类 ✓
        instance.dict_set(namespace, "UnsupportedOperation", instance.type_value(ty));
    }
    // **占位类型**（每个实例各造一份 —— 类型对象本来就是按实例登记的 ✓）
    // **带上文档串** ✓：参照里每个 `_io` 类型都有 `__doc__` 字符串 ✓
    //（`Lib/io.py:72` 就是 `__doc__ = _io._IOBase.__doc__` ✓；内容与 CPython 的 C 文档串**不是**
    // 逐字相同 ✓ —— 如实记，本层只保证"**有**这个属性且是字符串" ✓）。
    for name in PLACEHOLDER_TYPES {
        // **`BytesIO` 走真实现** ✓（第 726 轮）：见 `build_bytes_io` ✓。
        if name == "BytesIO" {
            continue;
        }
        let ty = instance.new_bare_type(name);
        if let Some(type_namespace) = instance.type_namespace(ty.cast()) {
            let doc = instance.new_str(&format!(
                "{name}：Pyawa 的 `_io` 占位类型（真 I/O 要走 `fs` 能力域）"
            ));
            instance.dict_set(type_namespace, "__doc__", doc);
            // **`IOBase` 一族的最小属性面** ✓（第 726 轮）：`Lib/zipfile/__init__.py` 的
            // `_ZipWriteFile.close()` 读 `self.closed`、`with … as dest` 要 `__enter__`／`__exit__` ✗
            // —— 占位类型上缺它们 ⇒ `close()` 当场 `AttributeError` ⇒ `_writing` 永远为真 ✗
            //（实测 `Can't close the ZIP file while there is an open writing handle on it` ✓）。
            let closed = instance.new_bool(false);
            instance.dict_set(type_namespace, "closed", closed);
            let enter = make_native(instance, "__enter__", io_base_enter as NativeFn);
            instance.dict_set(type_namespace, "__enter__", enter);
            let exit = make_native(instance, "__exit__", io_base_exit as NativeFn);
            instance.dict_set(type_namespace, "__exit__", exit);
            // `IOBase.close()`／`flush()`：占位类型的"关掉"＝把实例的 `closed` 置真 ✓
            //（`_ZipWriteFile.close()` 的 `super().close()` 要它 ✗；真资源由各自的类管 ✓）。
            let close = make_native(instance, "close", io_base_close as NativeFn);
            instance.dict_set(type_namespace, "close", close);
            let flush = make_native(instance, "flush", io_base_flush as NativeFn);
            instance.dict_set(type_namespace, "flush", flush);
        }
        instance.dict_set(namespace, name, instance.type_value(ty));
    }
    // **`BytesIO` 真实现** ✓（第 726 轮）：`Lib/io.py` 的 `BytesIO` 由它转发 ✓。
    build_bytes_io(instance, namespace);
    // **真的那个文本流类型**（`sys.stdout` 那类的类型 ✓）
    if let Some(ty) = instance.type_named(TEXT_WRAPPER) {
        instance.dict_set(namespace, "TextIOWrapper", instance.type_value(ty));
    }
    // **函数**：名字齐、调用报未实现 ✓
    for name in STUB_FUNCTIONS {
        let native = make_native(instance, name, not_implemented_native as NativeFn);
        instance.dict_set(namespace, name, native);
    }
    // `__all__` 与 `__name__`（与既有模块同一口径 ✓）
    let exports: Vec<NonNull<Header>> = PLACEHOLDER_TYPES
        .iter()
        .chain(STUB_FUNCTIONS.iter())
        .map(|name| instance.new_str(name))
        .collect();
    let exports = instance.new_list(exports);
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}

/// 类方法原生：`self` 可能在 **`bound`**（属性通道）或 **`args[0]`**（建实例的隐式调用）——两条都接。
fn this_of<'a>(
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> Option<(NonNull<Header>, &'a [NonNull<Header>])> {
    match bound {
        Some(this) => Some((this, args)),
        None => args.split_first().map(|(first, rest)| (*first, rest)),
    }
}

fn attr_of(instance: &Instance, this: NonNull<Header>, name: &str) -> Option<NonNull<Header>> {
    instance.attribute_optional_of(this, name).ok().flatten()
}

fn state_bytes(instance: &Instance, this: NonNull<Header>) -> Vec<u8> {
    attr_of(instance, this, "_buf")
        .and_then(|value| instance.bytes_value(value).map(|raw| raw.to_vec()))
        .unwrap_or_default()
}

fn state_pos(instance: &Instance, this: NonNull<Header>) -> i64 {
    attr_of(instance, this, "_pos")
        .and_then(|value| instance.int_of(value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0)
}

fn state_closed(instance: &Instance, this: NonNull<Header>) -> bool {
    attr_of(instance, this, "_closed").map(|value| instance.truth_of(value)).unwrap_or(false)
}

fn set_attr(instance: &Instance, this: NonNull<Header>, name: &str, value: NonNull<Header>) {
    let _ = instance.set_attribute_value(this, name, value);
}

/// 把状态写回实例字典 ✓（`_buf`／`_pos`／`_closed` ＋ 供属性读的 `closed` 镜像 ✓）。
fn store_state(instance: &Instance, this: NonNull<Header>, buffer: &[u8], position: i64, closed: bool) {
    let bytes = instance.new_bytes(buffer);
    set_attr(instance, this, "_buf", bytes);
    let pos = instance.new_int(position);
    set_attr(instance, this, "_pos", pos);
    let frozen = instance.new_bool(closed);
    set_attr(instance, this, "_closed", frozen);
    let mirror = instance.new_bool(closed);
    set_attr(instance, this, "closed", mirror);
}

fn closed_error(
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error("ValueError", "I/O operation on closed file."))
}

/// `BytesIO(初始字节=b'')` ✓（**如实口径** ✗：只收 `bytes`；`str` 的编码口径未接 ✓）。
fn bytesio_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.__init__ 缺少 self"));
    };
    let initial = rest
        .first()
        .and_then(|value| instance.bytes_value(*value).map(|raw| raw.to_vec()))
        .unwrap_or_default();
    store_state(instance, this, &initial, 0, false);
    Ok(instance.retain(instance.singletons().none()))
}

fn bytesio_read(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.read 缺少 self"));
    };
    if state_closed(instance, this) {
        return closed_error(instance);
    }
    let size = rest
        .first()
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .unwrap_or(-1);
    let buffer = state_bytes(instance, this);
    let start = state_pos(instance, this).max(0) as usize;
    let end = if size < 0 {
        buffer.len()
    } else {
        (start + size as usize).min(buffer.len())
    };
    let slice = if start >= buffer.len() { &[][..] } else { &buffer[start..end.max(start)] };
    let result = instance.new_bytes(slice);
    store_state(instance, this, &buffer, end.max(start) as i64, false);
    Ok(result)
}

fn bytesio_readline(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.readline 缺少 self"));
    };
    if state_closed(instance, this) {
        return closed_error(instance);
    }
    let size = rest
        .first()
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .unwrap_or(-1);
    let buffer = state_bytes(instance, this);
    let start = state_pos(instance, this).max(0) as usize;
    let mut end = buffer.len();
    if let Some(offset) = buffer[start.min(buffer.len())..].iter().position(|byte| *byte == b'\n') {
        end = start + offset + 1;
    }
    if size >= 0 {
        end = end.min(start + size as usize);
    }
    let slice = if start >= buffer.len() { &[][..] } else { &buffer[start..end.max(start)] };
    let result = instance.new_bytes(slice);
    store_state(instance, this, &buffer, end.max(start) as i64, false);
    Ok(result)
}

fn bytesio_readlines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let mut items: Vec<NonNull<Header>> = Vec::new();
    loop {
        let line = bytesio_readline(instance, bound, args, kwargs)?;
        let empty = instance.bytes_value(line).map(|raw| raw.is_empty()).unwrap_or(true);
        items.push(line);
        if empty {
            return Ok(instance.new_list(items));
        }
    }
}

fn bytesio_write(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.write 缺少 self"));
    };
    if state_closed(instance, this) {
        return closed_error(instance);
    }
    let Some(data) = rest
        .first()
        .and_then(|value| instance.bytes_value(*value).map(|raw| raw.to_vec()))
    else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "a bytes-like object is required, not 'object'",
        ));
    };
    let mut buffer = state_bytes(instance, this);
    let start = state_pos(instance, this).max(0) as usize;
    if start > buffer.len() {
        buffer.resize(start, 0);
    }
    if start + data.len() > buffer.len() {
        buffer.resize(start + data.len(), 0);
    }
    buffer[start..start + data.len()].copy_from_slice(&data);
    let position = (start + data.len()) as i64;
    store_state(instance, this, &buffer, position, false);
    Ok(instance.new_int(data.len() as i64))
}

fn bytesio_seek(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.seek 缺少 self"));
    };
    if state_closed(instance, this) {
        return closed_error(instance);
    }
    let offset = rest
        .first()
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0);
    let whence = rest
        .get(1)
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0);
    let base = match whence {
        0 => 0,
        1 => state_pos(instance, this),
        2 => state_bytes(instance, this).len() as i64,
        _ => {
            return Err(instance.raise_builtin_error("ValueError", "invalid whence"));
        }
    };
    let position = base + offset;
    if position < 0 {
        return Err(instance.raise_builtin_error("ValueError", "negative seek value"));
    }
    let buffer = state_bytes(instance, this);
    store_state(instance, this, &buffer, position, false);
    Ok(instance.new_int(position))
}

fn bytesio_tell(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, _args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.tell 缺少 self"));
    };
    if state_closed(instance, this) {
        return closed_error(instance);
    }
    Ok(instance.new_int(state_pos(instance, this)))
}

fn bytesio_getvalue(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.getvalue 缺少 self"));
    };
    if state_closed(instance, this) {
        return closed_error(instance);
    }
    let buffer = state_bytes(instance, this);
    Ok(instance.new_bytes(&buffer))
}

fn bytesio_truncate(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.truncate 缺少 self"));
    };
    if state_closed(instance, this) {
        return closed_error(instance);
    }
    let size = rest
        .first()
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .unwrap_or_else(|| state_pos(instance, this));
    if size < 0 {
        return Err(instance.raise_builtin_error("ValueError", "negative size value"));
    }
    let mut buffer = state_bytes(instance, this);
    buffer.resize(size as usize, 0);
    let position = state_pos(instance, this);
    store_state(instance, this, &buffer, position, false);
    Ok(instance.new_int(size))
}

fn bytesio_close(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.close 缺少 self"));
    };
    let buffer = state_bytes(instance, this);
    let position = state_pos(instance, this);
    store_state(instance, this, &buffer, position, true);
    Ok(instance.retain(instance.singletons().none()))
}

fn bytesio_true(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let _ = this_of(bound, args);
    Ok(instance.new_bool(true))
}

fn bytesio_false(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let _ = this_of(bound, args);
    Ok(instance.new_bool(false))
}

fn bytesio_enter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "BytesIO.__enter__ 缺少 self"));
    };
    Ok(instance.retain(this))
}

fn bytesio_none(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}

/// 建 `_io.BytesIO` 真类 ✓（内存缓冲 ＋ 位置 ＋ 关闭位都放**实例字典** ✓，不另开载荷类型 ✓）。
fn build_bytes_io(instance: &Instance, namespace: NonNull<Header>) {
    let object_type = instance.type_named("object").expect("object 已登记");
    let class_namespace = instance.new_dict();
    let handlers: [(&str, NativeFn); 18] = [
        ("__init__", bytesio_init as NativeFn),
        ("read", bytesio_read as NativeFn),
        ("read1", bytesio_read as NativeFn),
        ("readline", bytesio_readline as NativeFn),
        ("readlines", bytesio_readlines as NativeFn),
        ("write", bytesio_write as NativeFn),
        ("seek", bytesio_seek as NativeFn),
        ("tell", bytesio_tell as NativeFn),
        ("getvalue", bytesio_getvalue as NativeFn),
        ("truncate", bytesio_truncate as NativeFn),
        ("close", bytesio_close as NativeFn),
        ("flush", bytesio_none as NativeFn),
        ("readable", bytesio_true as NativeFn),
        ("writable", bytesio_true as NativeFn),
        ("seekable", bytesio_true as NativeFn),
        ("isatty", bytesio_false as NativeFn),
        // **不定义 `__len__`** ✓（照参照实测：`len(io.BytesIO())` 报 `TypeError` ✗）——
        // 于是真值走默认（恒真 ✓），`Lib/zipfile/__init__.py` 的 `if not self.fp` 才成立 ✓。
        ("__enter__", bytesio_enter as NativeFn),
        ("__exit__", bytesio_none as NativeFn),
    ];
    for (name, handler) in handlers {
        let function = make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    if let Ok(class) = pyawa_core::build_class_from_parts(
        instance,
        "BytesIO".to_owned(),
        vec![object_type.cast()],
        class_namespace,
        None,
    ) {
        instance.dict_set(namespace, "BytesIO", class);
    }
}

/// `IOBase.__enter__()` ⇒ 返回 `self` ✓（参照口径 ✓）。
fn io_base_enter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "__enter__ 缺少 self"));
    };
    Ok(instance.retain(this))
}

/// `IOBase.__exit__(…)` ⇒ 调本实例的 `close()` ✓（有就调 ✓）、返回 `None` ✓。
fn io_base_exit(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "__exit__ 缺少 self"));
    };
    if let Some(close) = instance.attribute_optional_of(this, "close")? {
        let result = pyawa_core::executor::call::call_value(instance, close, &[], &[])?;
        instance.release(result);
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// `IOBase.close()` ✓（占位口径：把实例的 `closed` 置真 ✓，不碰真资源 ✓）。
fn io_base_close(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "close 缺少 self"));
    };
    let closed = instance.new_bool(true);
    set_attr(instance, this, "closed", closed);
    Ok(instance.retain(instance.singletons().none()))
}

/// `IOBase.flush()` ✓（本层无缓冲 ⇒ **无事可做** ✓，不是假装 ✗）。
fn io_base_flush(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}
