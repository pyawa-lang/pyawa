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
use pyawa_capabilities::fs::open_flag;

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
/// `open` **已转真**（task-18 ✓，见 [`open_native`] ✓）⇒ 从本表移出 ✓。
const STUB_FUNCTIONS: [&str; 2] = ["open_code", "text_encoding"];

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
    // **`UnsupportedOperation`** ✓（task-18）：参照是 `OSError`＋`ValueError` 的子类 ✓，但核心这边
    // 造不出可 raise 的异常子类 ✗（见 [`unsupported_error`] 的说明 ✓）⇒ 仍**别名到 `OSError`** ✓。
    if let Some(ty) = instance.type_named("OSError") {
        instance.dict_set(namespace, "UnsupportedOperation", instance.type_value(ty));
    }
    // **占位类型**（每个实例各造一份 —— 类型对象本来就是按实例登记的 ✓）
    // **带上文档串** ✓：参照里每个 `_io` 类型都有 `__doc__` 字符串 ✓
    //（`Lib/io.py:72` 就是 `__doc__ = _io._IOBase.__doc__` ✓；内容与 CPython 的 C 文档串**不是**
    // 逐字相同 ✓ —— 如实记，本层只保证"**有**这个属性且是字符串" ✓）。
    for name in PLACEHOLDER_TYPES {
        // **`BytesIO` 走真实现** ✓（第 726 轮）：见 `build_bytes_io` ✓。
        // **`FileIO` 走真实现** ✓（task-18）：见 `build_file_io` ✓。
        if name == "BytesIO" || name == "FileIO" {
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
    // **`FileIO` 真实现** ✓（task-18）：`open()` 的实例就是它 ✓。
    build_file_io(instance, namespace);
    // **真的那个文本流类型**（`sys.stdout` 那类的类型 ✓）—— 先导出占位类型 ✓
    if let Some(ty) = instance.type_named(TEXT_WRAPPER) {
        instance.dict_set(namespace, "TextIOWrapper", instance.type_value(ty));
    }
    // **读路径真类**覆盖它 ✓（task-19 第 3 轮 ✓）：`Lib/tokenize.py::open` 要
    // `TextIOWrapper(buffer, encoding, line_buffering=True)` ✓ —— 那个占位类型**构造不出**（`new` 槽
    // 拒实参 ✓），故这里建真类 ✓；**如实记** ✗：`type(sys.stdout) is _io.TextIOWrapper` 随之不再为真 ✓
    //（`sys.stdout` 仍是那个占位文本流类型 ✓）。
    build_text_io(instance, namespace);
    // **函数**：`open` 走真实现 ✓（task-18），其余名字齐、调用报未实现 ✓
    let open_function = make_native(instance, "open", open_native as NativeFn);
    instance.dict_set(namespace, "open", open_function);
    for name in STUB_FUNCTIONS {
        let native = make_native(instance, name, not_implemented_native as NativeFn);
        instance.dict_set(namespace, name, native);
    }
    // `__all__` 与 `__name__`（与既有模块同一口径 ✓）
    let exports: Vec<NonNull<Header>> = PLACEHOLDER_TYPES
        .iter()
        .chain(["open"].iter())
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

/// 抛"这个流不支持该操作" ✓。
///
/// **如实** ✗（task-18 实测）：参照的 `_io.UnsupportedOperation` 是 **`OSError` ＋ `ValueError`** 的子类 ✓，
/// 但**异常基类的子类**在核心这边造不出**可 raise 的类型** ✗ —— `type.__new__`／`class X(OSError): pass`
/// 造出的子类 `instance_size` 仍是**普通对象**（56 ✓）⇒ 一 `raise` 就撞 `adopt` 的 debug 断言
/// `128 vs 56`（`ExceptionObject` 的 Rust 布局是 128 ✓）；连 `str(MyErr("x"))` 都给默认 repr ✓。
/// ⇒ 先按 `OSError` 抛 ✓（**消息照参照** ✓：`not writable`／`write` ✓；类名那一半如实记缺 ✗）。
fn unsupported_error(instance: &Instance, message: &str) -> ExecError {
    instance.raise_builtin_error("OSError", message)
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

// ==== `_io.open` 与 `FileIO` 文件对象（task-18 ✓）=================================================
//
// **为什么真做** ✓：`builtins.open` 缺失挡住 `Lib/re/_parser.py`（bytes tokenizer 的 `str(bytes,'latin1')`
// 已另修 ✓）之外的**一整族**：`linecache`／`tokenize.open`／`shutil`／`tempfile` 都压在它上面 ✓。
// **一处真相** ✓：真 I/O 只经 `fs` 能力域 ✓（`CX-4`：本 crate 不碰平台 ✓）；本层只管
// **模式解析／文本层／位置** ✓，与 `posix.open` 走的是同一批 `fs_*` 包装 ✓。
//
// **口径（如实 ✗）**：
// - 读模式在 `open()` 时把**整个文件**读进实例字典 ✓（不做流式 ✓ —— 我们不做真缓冲 ✓）；
// - 写模式**先在内存里改** ✓，`flush()`／`close()` 时才**整段落盘** ✓（`w` 用 `O_TRUNC` ✓ ⇒ 落盘即全文 ✓）；
// - `seek`／`tell` 对**文本**模式不做参照的"只能 0 偏移"限制 ✓（更宽 ✓，不是更窄 ✗）；
// - `encoding` 只认 `decode_with_encoding` 的四档 ✓（其余报 `LookupError` ✓）；`errors`／`newline`／
//   `buffering`／`opener` 如实**未实现** ✗（收了不报错，也不假装生效 ✗）；
// - `type(open(...))` 一律是 `_io.FileIO` ✓（参照文本模式给 `TextIOWrapper` ✗ —— 如实记 ✓）。

/// 打开模式（`r`／`w`／`a`／`x` 四选一 ✓）。
#[derive(Clone, Copy, PartialEq)]
enum ModeKind {
    Read,
    Write,
    Append,
    Create,
}

/// 解析结果（`+` ⇒ 可读＋可写 ✓；`b` ⇒ 二进制 ✓）。
struct ModeSpec {
    kind: ModeKind,
    binary: bool,
    readable: bool,
    writable: bool,
}

/// 解析 `open()` 的 `mode` ✓（报错文案照参照 ✓）。**一处真相** ✓：模式合法性只在这里判 ✓。
fn parse_mode(mode: &str) -> Result<ModeSpec, String> {
    for character in mode.chars() {
        if !matches!(character, 'r' | 'w' | 'a' | 'x' | 'b' | 't' | '+') {
            return Err(format!("invalid mode: '{mode}'"));
        }
    }
    let mut kind: Option<ModeKind> = None;
    let mut binary = false;
    let mut text = false;
    let mut plus = false;
    for character in mode.chars() {
        match character {
            'r' | 'w' | 'a' | 'x' => {
                if kind.is_some() {
                    return Err(
                        "must have exactly one of create/read/write/append mode".to_owned()
                    );
                }
                kind = Some(match character {
                    'r' => ModeKind::Read,
                    'w' => ModeKind::Write,
                    'a' => ModeKind::Append,
                    _ => ModeKind::Create,
                });
            }
            'b' => binary = true,
            't' => text = true,
            '+' => {
                if plus {
                    return Err(format!("invalid mode: '{mode}'"));
                }
                plus = true;
            }
            _ => unreachable!("字符集已在上一个循环里筛过"),
        }
    }
    if binary && text {
        return Err("can't have text and binary mode at once".to_owned());
    }
    let Some(kind) = kind else {
        return Err("must have exactly one of create/read/write/append mode".to_owned());
    };
    let readable = kind == ModeKind::Read || plus;
    let writable = kind != ModeKind::Read || plus;
    Ok(ModeSpec { kind, binary, readable, writable })
}

/// 关键字实参取值 ✓（native 的 `kwargs` 是 `(名字, 值)` 序列 ✓）。
fn kwarg_of(
    instance: &Instance,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    name: &str,
) -> Option<NonNull<Header>> {
    kwargs
        .iter()
        .find(|(key, _)| instance.text_of(*key) == Some(name))
        .map(|(_, value)| *value)
}

/// 实例字典里的整数状态 ✓。
fn file_int(instance: &Instance, this: NonNull<Header>, name: &str) -> i64 {
    attr_of(instance, this, name)
        .and_then(|value| instance.int_of(value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0)
}

/// 实例字典里的布尔状态 ✓。
fn file_flag(instance: &Instance, this: NonNull<Header>, name: &str) -> bool {
    attr_of(instance, this, name)
        .map(|value| instance.truth_of(value))
        .unwrap_or(false)
}

/// 实例字典里的**文本**状态 ✓（`_data` 在文本模式是 `str` ✓）。
fn file_text(instance: &Instance, this: NonNull<Header>, name: &str) -> Option<String> {
    attr_of(instance, this, name).and_then(|value| instance.text_of(value)).map(|text| text.to_owned())
}

/// 实例字典里的**字节**状态 ✓（`_data` 在二进制模式是 `bytes` ✓）。
fn file_bytes(instance: &Instance, this: NonNull<Header>, name: &str) -> Vec<u8> {
    attr_of(instance, this, name)
        .and_then(|value| instance.bytes_value(value).map(|raw| raw.to_vec()))
        .unwrap_or_default()
}

/// 把当前缓冲区**整段落盘** ✓（写模式 ✓；`append` ⇒ 靠 `O_APPEND` ✓，其余先 `seek(0)` ✓）。
fn file_flush_data(instance: &Instance, this: NonNull<Header>) -> Result<(), ExecError> {
    if !file_flag(instance, this, "_writable") || !file_flag(instance, this, "_dirty") {
        return Ok(());
    }
    let handle = file_int(instance, this, "_handle");
    if handle < 0 {
        return Ok(());
    }
    let handle = handle as u64;
    let bytes = if file_flag(instance, this, "_binary") {
        file_bytes(instance, this, "_data")
    } else {
        let text = file_text(instance, this, "_data").unwrap_or_default();
        let encoding = file_text(instance, this, "_encoding").unwrap_or_else(|| "utf-8".to_owned());
        let encoded = instance.encode_with_encoding(&text, &encoding)?;
        let raw = instance.bytes_value(encoded).map(|value| value.to_vec()).unwrap_or_default();
        instance.release(encoded);
        raw
    };
    if !file_flag(instance, this, "_append") {
        instance.fs_seek(handle, 0, 0).map_err(|error| fs_error(instance, error, ""))?;
    }
    write_all(instance, handle, &bytes).map_err(|error| fs_error(instance, error, ""))?;
    let clean = instance.new_bool(false);
    set_attr(instance, this, "_dirty", clean);
    Ok(())
}

/// 读满一个句柄 ✓（读模式在 `open()` 时用 ✓）。
fn read_all(instance: &Instance, handle: u64) -> Result<Vec<u8>, pyawa_core::CapabilityCallError> {
    let mut out = Vec::new();
    loop {
        let mut chunk = vec![0u8; 65536];
        let read = instance.fs_read(handle, &mut chunk)?;
        if read == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..read.min(chunk.len())]);
    }
    Ok(out)
}

/// 写满一个句柄 ✓（`fs_write` 可能短写 ✓）。
fn write_all(
    instance: &Instance,
    handle: u64,
    bytes: &[u8],
) -> Result<(), pyawa_core::CapabilityCallError> {
    let mut offset = 0usize;
    while offset < bytes.len() {
        let written = instance.fs_write(handle, &bytes[offset..])?;
        if written == 0 {
            break;
        }
        offset += written.min(bytes.len() - offset);
    }
    Ok(())
}

/// `errno` 名字 ⇒ 参照的 `strerror` 文案 ✓（**先覆盖常见几档** ✗，表外落 `OSError` ✓）。
fn strerror(name: &str) -> Option<&'static str> {
    Some(match name {
        "ENOENT" => "No such file or directory",
        "EACCES" => "Permission denied",
        "EISDIR" => "Is a directory",
        "EEXIST" => "File exists",
        "ENOTDIR" => "Not a directory",
        "EPERM" => "Operation not permitted",
        // **`readlink`／`realpath` 一族要的**（task-10 ✓；文案照参照 `strerror(3)` 本机实测 ✓）：
        // 非符号链接 ⇒ `EINVAL` ✓、符号链接环 ⇒ `ELOOP` ✓、路径过长 ⇒ `ENAMETOOLONG` ✓。
        "EINVAL" => "Invalid argument",
        "ELOOP" => "Too many levels of symbolic links",
        "ENAMETOOLONG" => "File name too long",
        _ => return None,
    })
}

/// 能力域错误 ⇒ 参照口径的 `OSError` 子类 ✓（`CM-19`／`CM-20`：按 **errno 名**查表 ✓）。
///
/// `display` 是**路径的裸文本** ✓（本函数按参照 `OSError` 文案给它套一层单引号 ✓ —— `str` 路径的
/// 形态）。**已经是引号形态的**（`bytes` 路径 ⇒ `b'…'`）走 [`fs_error_with_display`] ✓。
pub(crate) fn fs_error(
    instance: &Instance,
    error: pyawa_core::CapabilityCallError,
    display: &str,
) -> ExecError {
    fs_error_with_display(instance, error, &format!("'{display}'"))
}

/// 同 [`fs_error`]，但 `display` **原样**拼进消息（**一处真相**：errno ⇒ 类名／文案的映射只在这里 ✓）。
pub(crate) fn fs_error_with_display(
    instance: &Instance,
    error: pyawa_core::CapabilityCallError,
    display: &str,
) -> ExecError {
    match error {
        pyawa_core::CapabilityCallError::Machine(errno) => {
            let name = crate::errno_map::ERRNO_ERRORCODE
                .iter()
                .find(|(number, _)| *number == errno as i64)
                .map(|(_, name)| *name);
            let class = name
                .and_then(|name| {
                    crate::errno_map::ERRNO_TO_CLASS
                        .iter()
                        .find(|(key, _)| *key == name)
                        .map(|(_, class)| *class)
                })
                .unwrap_or("OSError");
            let text = name.and_then(strerror).unwrap_or("unknown error");
            instance.raise_builtin_error(class, &format!("[Errno {errno}] {text}: {display}"))
        }
        pyawa_core::CapabilityCallError::NotRegistered => {
            instance.raise_builtin_error("OSError", "`fs` 域未注册（`CP-2`）")
        }
        pyawa_core::CapabilityCallError::NotImplemented => {
            instance.raise_builtin_error("OSError", "`fs` 域的槽未实现（`CP-5`）")
        }
    }
}

/// **`open(file, mode='r', buffering=-1, encoding=None, errors=None, newline=None, …)`** ✓（task-18 ✓）。
///
/// 见本节的"口径（如实 ✗）"——`buffering`／`errors`／`newline`／`closefd`／`opener` 未实现 ✓。
pub fn open_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(file_value) = args.first().copied().or_else(|| kwarg_of(instance, kwargs, "file")) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "open() missing required argument 'file' (pos 1)",
        ));
    };
    let (path, display) = if let Some(text) = instance.text_of(file_value) {
        (text.as_bytes().to_vec(), text.to_owned())
    } else if let Some(raw) = instance.bytes_value(file_value) {
        (raw.to_vec(), format!("b'{}'", String::from_utf8_lossy(raw)))
    } else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "open() 只收 str／bytes 路径（int 文件描述符随后补 ✗）",
        ));
    };
    let mode = args
        .get(1)
        .copied()
        .or_else(|| kwarg_of(instance, kwargs, "mode"))
        .and_then(|value| instance.text_of(value))
        .unwrap_or("r")
        .to_owned();
    let spec = match parse_mode(&mode) {
        Ok(spec) => spec,
        Err(message) => return Err(instance.raise_builtin_error("ValueError", &message)),
    };
    let encoding = args
        .get(3)
        .copied()
        .or_else(|| kwarg_of(instance, kwargs, "encoding"))
        .and_then(|value| instance.text_of(value))
        .unwrap_or("utf-8")
        .to_owned();
    let kind = match spec.kind {
        ModeKind::Read => open_flag::RDONLY,
        ModeKind::Write | ModeKind::Append | ModeKind::Create => open_flag::WRONLY,
    };
    let base = if spec.readable && spec.writable {
        open_flag::RDWR
    } else {
        kind
    };
    let mut flags = base;
    match spec.kind {
        ModeKind::Read => {}
        ModeKind::Write => flags |= open_flag::CREAT | open_flag::TRUNC,
        ModeKind::Append => flags |= open_flag::CREAT | open_flag::APPEND,
        // **`x`**：能力域**没有** `EXCL` 槽 ✗ ⇒ 先 `stat` 探存在（`FileExistsError` ✓），再以
        // `CREAT|TRUNC` 开 ✓ —— **如实** ✗：这中间有 TOCTOU 窗口（单进程自用无碍 ✓）。
        ModeKind::Create => {
            if instance.fs_stat(&path).is_ok() {
                return Err(instance.raise_builtin_error(
                    "FileExistsError",
                    &format!("[Errno 17] File exists: '{display}'"),
                ));
            }
            flags |= open_flag::CREAT | open_flag::TRUNC;
        }
    }
    let handle = match instance.fs_open(&path, flags, 0o666) {
        Ok(handle) => handle,
        Err(error) => return Err(fs_error(instance, error, &display)),
    };
    let mut raw: Vec<u8> = Vec::new();
    if spec.readable {
        match read_all(instance, handle) {
            Ok(bytes) => raw = bytes,
            Err(error) => {
                let _ = instance.fs_close(handle);
                return Err(fs_error(instance, error, &display));
            }
        }
    }
    let text_data = if spec.binary {
        None
    } else if spec.readable {
        match instance.decode_with_encoding(&raw, &encoding) {
            Ok(decoded) => {
                let text = instance.text_of(decoded).map(|value| value.to_owned()).unwrap_or_default();
                instance.release(decoded);
                Some(text)
            }
            Err(error) => {
                let _ = instance.fs_close(handle);
                return Err(error);
            }
        }
    } else {
        Some(String::new())
    };
    let Some(ty) = instance.type_named("FileIO") else {
        let _ = instance.fs_close(handle);
        return Err(instance.raise_builtin_error("RuntimeError", "`_io.FileIO` 未登记"));
    };
    let attributes = instance.new_dict();
    let handle_value = instance.new_int(handle as i64);
    instance.dict_set(attributes, "_handle", handle_value);
    let data_value = match &text_data {
        Some(text) => instance.new_str(text),
        None => instance.new_bytes(&raw),
    };
    instance.dict_set(attributes, "_data", data_value);
    let position = instance.new_int(0);
    instance.dict_set(attributes, "_pos", position);
    for (name, value) in [
        ("_binary", spec.binary),
        ("_readable", spec.readable),
        ("_writable", spec.writable),
        ("_append", spec.kind == ModeKind::Append),
        ("_closed", false),
        ("closed", false),
        ("_dirty", false),
    ] {
        let flag = instance.new_bool(value);
        instance.dict_set(attributes, name, flag);
    }
    let name_value = instance.new_str(&display);
    instance.dict_set(attributes, "_name", name_value);
    let mode_value = instance.new_str(&mode);
    instance.dict_set(attributes, "mode", mode_value);
    let encoding_value = instance.new_str(&encoding);
    instance.dict_set(attributes, "_encoding", encoding_value);
    let object = instance.alloc(pyawa_core::AttributeObject::new(
        ty,
        core::cell::RefCell::new(Some(attributes)),
    ));
    Ok(object.into_raw().cast::<Header>())
}

// ==== `_io.TextIOWrapper`（task-19 第 3 轮 ✓）：**读路径最小面** ✓ ==================================
//
// **动因** ✓：`Lib/tokenize.py::open` 就是 `TextIOWrapper(buffer, encoding, line_buffering=True)` ✓
// （`buffer` 是 `builtins.open(name,'rb')` 的二进制层 ✓）⇒ `linecache.updatecache` 一族（⇒ `traceback`
// 的源码行 ✓）都要它 ✓。**如实范围** ✗：只接**读**（`read`／`readline`／`readlines`／迭代／`with` ✓）；
// **写路径**（`write`／`flush` 落盘 ✓）、`newline=` 的通用换行、`buffering` 语义如实立支 ✓。
//
// 口径 ✓：解码走 [`Instance::decode_with_encoding`] **一处真相** ✓（与 `bytes.decode` 同一处 ✓）；
// 状态全在**实例字典** ✓（`_buffer`／`_encoding`／`_text`／`_pos`／`_closed` ✓，不新增载荷类型 ✓）。

/// 取 `(self, 其余实参)` 或报"缺 self" ✓。
fn text_wrapper_self<'a>(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> Result<(NonNull<Header>, &'a [NonNull<Header>]), ExecError> {
    this_of(bound, args)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "TextIOWrapper 方法缺少 self"))
}

/// 从 `kwargs` 取一个字符串参数 ✓。
fn text_wrapper_kwarg(
    instance: &Instance,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    name: &str,
) -> Option<String> {
    kwargs
        .iter()
        .find(|(key, _)| instance.text_of(*key) == Some(name))
        .and_then(|(_, value)| instance.text_of(*value))
        .map(|text| text.to_owned())
}

/// 实例上的字符串属性 ✓。
fn text_wrapper_text(instance: &Instance, this: NonNull<Header>, name: &str) -> Option<String> {
    instance
        .attribute_optional_of(this, name)
        .ok()
        .flatten()
        .and_then(|value| instance.text_of(value))
        .map(|text| text.to_owned())
}

fn text_wrapper_closed(instance: &Instance, this: NonNull<Header>) -> bool {
    instance
        .attribute_optional_of(this, "_closed")
        .ok()
        .flatten()
        .and_then(|value| instance.truthiness_of(value).ok())
        .unwrap_or(false)
}

/// **`TextIOWrapper(buffer, encoding=None, errors=None, newline=None, …)`** ✓。
fn text_wrapper_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = text_wrapper_self(instance, bound, args)?;
    let Some(buffer) = rest.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "TextIOWrapper 要一个二进制层（buffer）",
        ));
    };
    let encoding = rest
        .get(1)
        .and_then(|value| instance.text_of(*value))
        .map(|text| text.to_owned())
        .or_else(|| text_wrapper_kwarg(instance, kwargs, "encoding"))
        .unwrap_or_else(|| "utf-8".to_owned());
    let buffer = instance.retain(buffer);
    set_attr(instance, this, "_buffer", buffer);
    let encoding_value = instance.new_str(&encoding);
    set_attr(instance, this, "_encoding", encoding_value);
    let encoding_mirror = instance.new_str(&encoding);
    set_attr(instance, this, "encoding", encoding_mirror);
    // **不预设 `mode`** ✓（参照的 `TextIOWrapper` 就没这个属性 ✓ —— `tokenize.open` 自己会赋
    // `text.mode = 'r'` ✓；实例字典可写 ✓）。
    let zero = instance.new_int(0);
    set_attr(instance, this, "_pos", zero);
    let closed = instance.new_bool(false);
    set_attr(instance, this, "_closed", closed);
    // **`closed` 的实例镜像** ✓（基类那份是**类级**的 ✓ ⇒ 关掉后读实例属性还是 False ✗ ——
    // 实测 `t.close(); t.closed` 参照给 True ✓）。
    let mirror = instance.new_bool(false);
    set_attr(instance, this, "closed", mirror);
    let nothing = instance.singletons().none();
    set_attr(instance, this, "_text", nothing);
    Ok(instance.retain(this))
}

/// **整段解码并缓存** ✓：第一次读时把底层 `read()` 全取来 ✓，按 `encoding` 解码 ✓（一处真相 ✓）。
fn text_wrapper_ensure_text(instance: &Instance, this: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    let cached = instance.attribute_optional_of(this, "_text")?;
    if let Some(value) = cached {
        if value != instance.singletons().none() {
            return Ok(value);
        }
    }
    let buffer = instance
        .attribute_optional_of(this, "_buffer")?
        .ok_or_else(|| instance.raise_builtin_error("ValueError", "I/O operation on closed file."))?;
    let read = instance
        .attribute_optional_of(buffer, "read")?
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "底层的 buffer 没有 read"))?;
    let raw = pyawa_core::executor::call::call_value(instance, read, &[], &[])?;
    let encoding = text_wrapper_text(instance, this, "_encoding").unwrap_or_else(|| "utf-8".to_owned());
    let text = if instance.bytes_value(raw).is_some() {
        let raw_bytes = instance.bytes_value(raw).map(|bytes| bytes.to_vec()).unwrap_or_default();
        let decoded = instance.decode_with_encoding(&raw_bytes, &encoding)?;
        decoded
    } else {
        instance.retain(raw)
    };
    set_attr(instance, this, "_text", text);
    let zero = instance.new_int(0);
    set_attr(instance, this, "_pos", zero);
    Ok(text)
}

fn text_wrapper_position(instance: &Instance, this: NonNull<Header>) -> usize {
    instance
        .attribute_optional_of(this, "_pos")
        .ok()
        .flatten()
        .and_then(|value| instance.int_value(value))
        .unwrap_or(0)
        .max(0) as usize
}

/// `read(size=-1)` ✓。
fn text_wrapper_read(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = text_wrapper_self(instance, bound, args)?;
    if text_wrapper_closed(instance, this) {
        return closed_error(instance);
    }
    let text = text_wrapper_ensure_text(instance, this)?;
    let Some(all) = instance.text_of(text).map(|value| value.to_owned()) else {
        return Err(instance.raise_builtin_error("TypeError", "TextIOWrapper 的内容不是 str"));
    };
    let characters: Vec<char> = all.chars().collect();
    let start = text_wrapper_position(instance, this).min(characters.len());
    let size = rest.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    let end = if size < 0 {
        characters.len()
    } else {
        (start + size as usize).min(characters.len())
    };
    let piece: String = characters[start..end].iter().collect();
    let position = instance.new_int(end as i64);
    set_attr(instance, this, "_pos", position);
    Ok(instance.new_str(&piece))
}

/// `readline(size=-1)` ✓（按 `\n` 收尾 ✓；末尾没换行也给这一段 ✓）。
fn text_wrapper_readline(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = text_wrapper_self(instance, bound, args)?;
    if text_wrapper_closed(instance, this) {
        return closed_error(instance);
    }
    let text = text_wrapper_ensure_text(instance, this)?;
    let Some(all) = instance.text_of(text).map(|value| value.to_owned()) else {
        return Err(instance.raise_builtin_error("TypeError", "TextIOWrapper 的内容不是 str"));
    };
    let characters: Vec<char> = all.chars().collect();
    let start = text_wrapper_position(instance, this).min(characters.len());
    let mut end = characters.len();
    for index in start..characters.len() {
        if characters[index] == '\n' {
            end = index + 1;
            break;
        }
    }
    if let Some(limit) = rest.first().and_then(|value| instance.int_value(*value)) {
        if limit >= 0 && start + limit as usize <= end {
            end = start + limit as usize;
        }
    }
    let piece: String = characters[start..end].iter().collect();
    let position = instance.new_int(end as i64);
    set_attr(instance, this, "_pos", position);
    Ok(instance.new_str(&piece))
}

/// `readlines()` ✓。
fn text_wrapper_readlines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = text_wrapper_self(instance, bound, args)?;
    let mut lines: Vec<NonNull<Header>> = Vec::new();
    loop {
        let line = text_wrapper_readline(instance, Some(this), &[], &[])?;
        let empty = instance.text_of(line).map(|text| text.is_empty()).unwrap_or(true);
        if empty {
            break;
        }
        lines.push(line);
    }
    Ok(instance.new_list(lines))
}

fn text_wrapper_self_ref(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = text_wrapper_self(instance, bound, args)?;
    Ok(instance.retain(this))
}

fn text_wrapper_next(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = text_wrapper_self(instance, bound, args)?;
    let line = text_wrapper_readline(instance, Some(this), &[], &[])?;
    let empty = instance.text_of(line).map(|text| text.is_empty()).unwrap_or(true);
    if empty {
        return Err(instance.raise_builtin_error("StopIteration", ""));
    }
    Ok(line)
}

/// `close()` ✓：底层 `close()` 一并关 ✓（有才调 ✓）。
fn text_wrapper_close(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = text_wrapper_self(instance, bound, args)?;
    if !text_wrapper_closed(instance, this) {
        if let Some(buffer) = instance.attribute_optional_of(this, "_buffer")? {
            if let Some(close) = instance.attribute_optional_of(buffer, "close")? {
                let _ = pyawa_core::executor::call::call_value(instance, close, &[], &[]);
            }
        }
    }
    let closed = instance.new_bool(true);
    set_attr(instance, this, "_closed", closed);
    let mirror = instance.new_bool(true);
    set_attr(instance, this, "closed", mirror);
    Ok(instance.singletons().none())
}

/// `__exit__` ✓：关掉 ✓、**不吞异常** ✓（回 `None` ✓）。
fn text_wrapper_exit(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = text_wrapper_self(instance, bound, args)?;
    text_wrapper_close(instance, Some(this), &[], kwargs)?;
    Ok(instance.singletons().none())
}

/// 建 `_io.TextIOWrapper` **真类** ✓（读路径 ✓）。
fn build_text_io(instance: &Instance, namespace: NonNull<Header>) {
    let base = instance
        .type_named("_TextIOBase")
        .or_else(|| instance.type_named("TextIOBase"))
        .or_else(|| instance.type_named("_IOBase"))
        .or_else(|| instance.type_named("object"))
        .expect("`object` 已登记");
    let class_namespace = instance.new_dict();
    let handlers: &[(&str, NativeFn)] = &[
        ("__init__", text_wrapper_init as NativeFn),
        ("read", text_wrapper_read as NativeFn),
        ("readline", text_wrapper_readline as NativeFn),
        ("readlines", text_wrapper_readlines as NativeFn),
        ("__iter__", text_wrapper_self_ref as NativeFn),
        ("__next__", text_wrapper_next as NativeFn),
        ("__enter__", text_wrapper_self_ref as NativeFn),
        ("__exit__", text_wrapper_exit as NativeFn),
        ("close", text_wrapper_close as NativeFn),
    ];
    for (name, handler) in handlers {
        let function = make_native(instance, name, *handler);
        instance.dict_set(class_namespace, name, function);
    }
    if let Ok(class) = pyawa_core::build_class_from_parts(
        instance,
        "TextIOWrapper".to_owned(),
        vec![base.cast()],
        class_namespace,
        None,
    ) {
        instance.dict_set(namespace, "TextIOWrapper", class);
    }
}

/// 建 `_io.FileIO` 真类 ✓（状态全放**实例字典** ✓，不新增载荷类型 ✓）。
fn build_file_io(instance: &Instance, namespace: NonNull<Header>) {
    let base = instance
        .type_named("_IOBase")
        .or_else(|| instance.type_named("object"))
        .expect("`object` 已登记");
    let class_namespace = instance.new_dict();
    let handlers: &[(&str, NativeFn)] = &[
        ("read", file_read as NativeFn),
        ("readline", file_readline as NativeFn),
        ("readlines", file_readlines as NativeFn),
        ("write", file_write as NativeFn),
        ("flush", file_flush as NativeFn),
        ("close", file_close as NativeFn),
        ("seek", file_seek as NativeFn),
        ("tell", file_tell as NativeFn),
        ("readable", file_readable as NativeFn),
        ("writable", file_writable as NativeFn),
        ("seekable", file_seekable as NativeFn),
        ("isatty", file_isatty as NativeFn),
        ("fileno", file_fileno as NativeFn),
        ("__enter__", file_enter as NativeFn),
        ("__exit__", file_exit as NativeFn),
        ("__iter__", file_enter as NativeFn),
        ("__next__", file_next as NativeFn),
    ];
    for (name, handler) in handlers {
        let function = make_native(instance, name, *handler);
        instance.dict_set(class_namespace, name, function);
    }
    if let Ok(class) = pyawa_core::build_class_from_parts(
        instance,
        "FileIO".to_owned(),
        vec![base.cast()],
        class_namespace,
        None,
    ) {
        instance.dict_set(namespace, "FileIO", class);
    }
}

fn file_receiver(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    match this_of(bound, args) {
        Some((this, _)) => Ok(this),
        None => Err(instance.raise_builtin_error("TypeError", "文件方法缺少 self")),
    }
}

fn file_read(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = match this_of(bound, args) {
        Some(pair) => pair,
        None => return Err(instance.raise_builtin_error("TypeError", "read 缺少 self")),
    };
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    if !file_flag(instance, this, "_readable") {
        return Err(unsupported_error(instance, "not readable"));
    }
    let size = rest.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    let position = file_int(instance, this, "_pos").max(0) as usize;
    if file_flag(instance, this, "_binary") {
        let data = file_bytes(instance, this, "_data");
        let start = position.min(data.len());
        let end = if size < 0 { data.len() } else { (start + size as usize).min(data.len()) };
        let new_position = instance.new_int(end as i64);
        set_attr(instance, this, "_pos", new_position);
        Ok(instance.new_bytes(&data[start..end]))
    } else {
        let text = file_text(instance, this, "_data").unwrap_or_default();
        let characters: Vec<char> = text.chars().collect();
        let start = position.min(characters.len());
        let end = if size < 0 { characters.len() } else { (start + size as usize).min(characters.len()) };
        let piece: String = characters[start..end].iter().collect();
        let new_position = instance.new_int(end as i64);
        set_attr(instance, this, "_pos", new_position);
        Ok(instance.new_str(&piece))
    }
}

fn file_readline(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = match this_of(bound, args) {
        Some(pair) => pair,
        None => return Err(instance.raise_builtin_error("TypeError", "readline 缺少 self")),
    };
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    if !file_flag(instance, this, "_readable") {
        return Err(unsupported_error(instance, "not readable"));
    }
    let size = rest.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    let position = file_int(instance, this, "_pos").max(0) as usize;
    if file_flag(instance, this, "_binary") {
        let data = file_bytes(instance, this, "_data");
        let start = position.min(data.len());
        let mut end = data.len();
        if let Some(offset) = data[start..].iter().position(|byte| *byte == b'\n') {
            end = start + offset + 1;
        }
        if size >= 0 {
            end = end.min(start + size as usize);
        }
        let new_position = instance.new_int(end as i64);
        set_attr(instance, this, "_pos", new_position);
        Ok(instance.new_bytes(&data[start..end]))
    } else {
        let text = file_text(instance, this, "_data").unwrap_or_default();
        let characters: Vec<char> = text.chars().collect();
        let start = position.min(characters.len());
        let mut end = characters.len();
        if let Some(offset) = characters[start..].iter().position(|character| *character == '\n') {
            end = start + offset + 1;
        }
        if size >= 0 {
            end = end.min(start + size as usize);
        }
        let piece: String = characters[start..end].iter().collect();
        let new_position = instance.new_int(end as i64);
        set_attr(instance, this, "_pos", new_position);
        Ok(instance.new_str(&piece))
    }
}

fn file_readlines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    let rest: Vec<NonNull<Header>> = match bound {
        Some(_) => args.to_vec(),
        None => args.iter().skip(1).copied().collect(),
    };
    let mut items: Vec<NonNull<Header>> = Vec::new();
    loop {
        let line = file_readline(instance, Some(this), &rest, kwargs)?;
        let empty = if file_flag(instance, this, "_binary") {
            instance.bytes_value(line).map(|raw| raw.is_empty()).unwrap_or(true)
        } else {
            instance.text_of(line).map(|text| text.is_empty()).unwrap_or(true)
        };
        if empty {
            // **句尾那个空串不进列表** ✓（参照 `readlines()` 不把 EOF 的空读算一行 ✓；
            // 空文件 ⇒ `[]` ✓）。
            instance.release(line);
            return Ok(instance.new_list(items));
        }
        items.push(line);
    }
}

fn file_write(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = match this_of(bound, args) {
        Some(pair) => pair,
        None => return Err(instance.raise_builtin_error("TypeError", "write 缺少 self")),
    };
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    let Some(payload) = rest.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "write() 要 1 个实参"));
    };
    // **检查顺序照参照** ✓（实测 ✓）：**二进制**先判可写（消息就是 `write` ✓）；
    // **文本**先判实参是不是 `str` ✓（`TypeError` ✓），再判可写（`not writable` ✓）。
    if file_flag(instance, this, "_binary") && !file_flag(instance, this, "_writable") {
        return Err(unsupported_error(instance, "write"));
    }
    let position = file_int(instance, this, "_pos").max(0) as usize;
    let append = file_flag(instance, this, "_append");
    if file_flag(instance, this, "_binary") {
        let Some(raw) = instance.bytes_value(payload) else {
            let name = instance.type_name(instance.type_of(payload));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("a bytes-like object is required, not '{name}'"),
            ));
        };
        let insertion = raw.to_vec();
        let mut data = file_bytes(instance, this, "_data");
        let at = if append { data.len() } else { position };
        if at > data.len() {
            data.resize(at, 0);
        }
        if at + insertion.len() > data.len() {
            data.resize(at + insertion.len(), 0);
        }
        data[at..at + insertion.len()].copy_from_slice(&insertion);
        let written = insertion.len();
        let data_value = instance.new_bytes(&data);
        set_attr(instance, this, "_data", data_value);
        let new_position = instance.new_int((at + written) as i64);
        set_attr(instance, this, "_pos", new_position);
        let dirty = instance.new_bool(true);
        set_attr(instance, this, "_dirty", dirty);
        Ok(instance.new_int(written as i64))
    } else {
        let Some(text) = instance.text_of(payload) else {
            let name = instance.type_name(instance.type_of(payload));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("write() argument must be str, not {name}"),
            ));
        };
        if !file_flag(instance, this, "_writable") {
            return Err(unsupported_error(instance, "not writable"));
        }
        let insertion: Vec<char> = text.chars().collect();
        let current = file_text(instance, this, "_data").unwrap_or_default();
        let mut characters: Vec<char> = current.chars().collect();
        let at = if append { characters.len() } else { position };
        if at > characters.len() {
            characters.resize(at, '\0');
        }
        if at + insertion.len() > characters.len() {
            characters.resize(at + insertion.len(), '\0');
        }
        for (offset, character) in insertion.iter().enumerate() {
            characters[at + offset] = *character;
        }
        let written = insertion.len();
        let updated: String = characters.into_iter().collect();
        let data_value = instance.new_str(&updated);
        set_attr(instance, this, "_data", data_value);
        let new_position = instance.new_int((at + written) as i64);
        set_attr(instance, this, "_pos", new_position);
        let dirty = instance.new_bool(true);
        set_attr(instance, this, "_dirty", dirty);
        Ok(instance.new_int(written as i64))
    }
}

fn file_flush(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    file_flush_data(instance, this)?;
    Ok(instance.new_none())
}

fn file_close(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    if file_flag(instance, this, "_closed") {
        return Ok(instance.new_none());
    }
    file_flush_data(instance, this)?;
    let handle = file_int(instance, this, "_handle");
    if handle >= 0 {
        instance.fs_close(handle as u64).map_err(|error| fs_error(instance, error, ""))?;
    }
    let closed = instance.new_bool(true);
    set_attr(instance, this, "_closed", closed);
    let mirror = instance.new_bool(true);
    set_attr(instance, this, "closed", mirror);
    let gone = instance.new_int(-1);
    set_attr(instance, this, "_handle", gone);
    Ok(instance.new_none())
}

fn file_seek(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = match this_of(bound, args) {
        Some(pair) => pair,
        None => return Err(instance.raise_builtin_error("TypeError", "seek 缺少 self")),
    };
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    let offset = rest.first().and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let whence = rest.get(1).and_then(|value| instance.int_value(*value)).unwrap_or(0);
    let base = match whence {
        0 => 0,
        1 => file_int(instance, this, "_pos"),
        2 => {
            if file_flag(instance, this, "_binary") {
                file_bytes(instance, this, "_data").len() as i64
            } else {
                file_text(instance, this, "_data").unwrap_or_default().chars().count() as i64
            }
        }
        _ => return Err(instance.raise_builtin_error("ValueError", "invalid whence")),
    };
    let position = base + offset;
    if position < 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("negative seek value {position}"),
        ));
    }
    let value = instance.new_int(position);
    set_attr(instance, this, "_pos", value);
    Ok(instance.new_int(position))
}

fn file_tell(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    Ok(instance.new_int(file_int(instance, this, "_pos")))
}

fn file_readable(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    Ok(instance.new_bool(file_flag(instance, this, "_readable")))
}

fn file_writable(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    Ok(instance.new_bool(file_flag(instance, this, "_writable")))
}

fn file_seekable(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    Ok(instance.new_bool(true))
}

fn file_isatty(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let _ = file_receiver(instance, bound, args)?;
    Ok(instance.new_bool(false))
}

fn file_fileno(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    if file_flag(instance, this, "_closed") {
        return closed_error(instance);
    }
    Ok(instance.new_int(file_int(instance, this, "_handle")))
}

fn file_enter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    Ok(instance.retain(this))
}

fn file_exit(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    file_close(instance, Some(this), &[], kwargs)?;
    Ok(instance.new_none())
}

fn file_next(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    let line = file_readline(instance, Some(this), &[], kwargs)?;
    let empty = if file_flag(instance, this, "_binary") {
        instance.bytes_value(line).map(|raw| raw.is_empty()).unwrap_or(true)
    } else {
        instance.text_of(line).map(|text| text.is_empty()).unwrap_or(true)
    };
    if empty {
        instance.release(line);
        return Err(instance.raise_builtin_error("StopIteration", ""));
    }
    Ok(line)
}
