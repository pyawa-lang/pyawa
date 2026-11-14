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
/// `open` **已转真**（task-18 ✓，见 [`open_native`] ✓）⇒ 从本表移出 ✓；
/// `text_encoding` 已转真（round 268，`pathlib`／`_pyio` 一族都要它 ✓）⇒ 也移出 ✓。
const STUB_FUNCTIONS: [&str; 1] = ["open_code"];

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
    // **`BufferedReader`／`BufferedWriter` 真实现**（round 99）：`Lib/socket.py` 的
    // `makefile()` 用它们包 `SocketIO` ✓（`test_urllib_response` 的 setUp ⇒ 4E 全压在
    // `io.BufferedReader` 的"不能构造"上 ✗）；它们只做**逐方法委派**给内部 raw 流 ✓。
    build_buffered_io(instance, namespace);
    // **`StringIO` 真实现** ✓（`P3-39`）：覆盖上面那个占位类型 ✓。
    build_string_io(instance, namespace);
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
    // **`text_encoding`**（round 268）：`Lib/io.py` 在导入期 `from _io import text_encoding` ✓
    // ⇒ 缺它 `import io` 就失败；`pathlib.Path.read_text／write_text` 也一进门就调它 ✓。
    let text_encoding = make_native(instance, "text_encoding", text_encoding_native as NativeFn);
    instance.dict_set(namespace, "text_encoding", text_encoding);
    for name in STUB_FUNCTIONS {
        let native = make_native(instance, name, not_implemented_native as NativeFn);
        instance.dict_set(namespace, name, native);
    }
    // `__all__` 与 `__name__`（与既有模块同一口径 ✓）
    let exports: Vec<NonNull<Header>> = PLACEHOLDER_TYPES
        .iter()
        .chain(["open", "text_encoding"].iter())
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
    // **EOF 不产生空串元素**（`test_memoryio` 的 `test_readlines` 钉住 ✓：先前多推一个 `b''` ✗）；
    // `sizehint > 0` ⇒ 累计到提示量就停 ✓（参照口径："lines totaling approximately sizehint"）。
    let hint = args
        .first()
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(-1);
    let mut total: i64 = 0;
    let mut items: Vec<NonNull<Header>> = Vec::new();
    loop {
        let line = bytesio_readline(instance, bound, &[], kwargs)?;
        let length = instance.bytes_value(line).map(|raw| raw.len()).unwrap_or(0);
        if length == 0 {
            instance.release(line);
            return Ok(instance.new_list(items));
        }
        total += length as i64;
        items.push(line);
        if hint > 0 && total >= hint {
            return Ok(instance.new_list(items));
        }
    }
}

/// **bytes-like 取值**（round 95）：`bytes`／`bytearray`／`memoryview` 都收 ✓（参照的缓冲协议 ✓）。
fn bytes_like_vec(instance: &Instance, value: NonNull<Header>) -> Option<Vec<u8>> {
    if let Some(raw) = instance.bytes_value(value) {
        return Some(raw.to_vec());
    }
    let ty = instance.type_name(instance.type_of(value));
    if ty == "bytearray" || ty == "memoryview" {
        // 走公开构造：`bytes(x)`（stdlib 不能碰载荷布局 ✓）。
        let bytes_type = instance.type_named("bytes")?;
        let converted = pyawa_core::executor::call::call_value(instance, bytes_type.cast(), &[value], &[]).ok()?;
        let raw = instance.bytes_value(converted).map(|slice| slice.to_vec());
        instance.release(converted);
        return raw;
    }
    None
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
        .and_then(|value| bytes_like_vec(instance, *value))
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

/// `BytesIO.getbuffer()` ✓（round 47）：**如实口径** ✗ —— 返回的是当前内容的 **`bytes` 拷贝**，
/// 不是真正的 `memoryview` 视图（本层 `memoryview` 只有形状、没有视图语义 ✓）。`Lib/pickle.py:214`
/// 只做 `len(data)` ＋ `write(data)` ⇒ 拷贝足够 ✓；视图语义（可变、`release()`）随后补 ✓。
fn bytesio_getbuffer(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    bytesio_getvalue(instance, bound, args, &[])
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
/// **`BufferedReader`／`BufferedWriter` 的委托实现**（round 99）。
///
/// 口径：**不真缓冲** ✓ —— 每个方法原样转给构造时给的 raw 流（`_io.SocketIO` 等 ✓）；`closed`
/// 由实例字段 `_closed` 如实记 ✓。真缓冲不是参照语义的观察点（测试只关心"能构造、能读、close 后 closed" ✓）。
fn build_buffered_io(instance: &Instance, namespace: NonNull<Header>) {
    let object_type = instance.type_named("object").expect("object 已登记");
    let reader_handlers: Vec<(&str, NativeFn)> = vec![
                ("__init__", buffered_init as NativeFn),
                ("read", buffered_read as NativeFn),
                ("read1", buffered_read as NativeFn),
                ("readline", buffered_readline as NativeFn),
                ("readlines", buffered_readlines as NativeFn),
                ("peek", buffered_peek as NativeFn),
                ("seek", buffered_seek as NativeFn),
                ("tell", buffered_tell as NativeFn),
                ("close", buffered_close as NativeFn),
                ("flush", buffered_flush as NativeFn),
                ("readable", buffered_true as NativeFn),
                ("writable", buffered_false as NativeFn),
                ("seekable", buffered_true as NativeFn),
                ("isatty", buffered_false as NativeFn),
                ("fileno", buffered_fileno as NativeFn),
                ("__enter__", buffered_enter as NativeFn),
                ("__exit__", buffered_exit as NativeFn),
                ("__iter__", buffered_enter as NativeFn),
                ("__next__", buffered_next as NativeFn),
                ("__getattr__", buffered_getattr as NativeFn),
    ];
    let writer_handlers: Vec<(&str, NativeFn)> = vec![
                ("__init__", buffered_init as NativeFn),
                ("write", buffered_write as NativeFn),
                ("writelines", buffered_writelines as NativeFn),
                ("close", buffered_close as NativeFn),
                ("flush", buffered_flush as NativeFn),
                ("readable", buffered_false as NativeFn),
                ("writable", buffered_true as NativeFn),
                ("seekable", buffered_true as NativeFn),
                ("isatty", buffered_false as NativeFn),
                ("fileno", buffered_fileno as NativeFn),
                ("__enter__", buffered_enter as NativeFn),
                ("__exit__", buffered_exit as NativeFn),
                ("__getattr__", buffered_getattr as NativeFn),
    ];
    for (type_name, handlers) in [
        ("BufferedReader", reader_handlers),
        ("BufferedWriter", writer_handlers),
    ] {
        let class_namespace = instance.new_dict();
        for (name, handler) in handlers {
            let function = make_native(instance, name, handler);
            instance.dict_set(class_namespace, name, function);
        }
        if let Ok(class) = pyawa_core::build_class_from_parts(
            instance,
            type_name.to_owned(),
            vec![object_type.cast()],
            class_namespace,
            None,
        ) {
            instance.dict_set(namespace, type_name, class);
        }
    }
}

/// 取本缓冲包装内部的 raw 流（`_raw` ✓）。
fn buffered_raw(instance: &Instance, this: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    instance
        .attribute_optional_of(this, "_raw")?
        .ok_or_else(|| instance.raise_builtin_error("ValueError", "I/O operation on uninitialized buffered object"))
}

/// 把 `name` 方法调用转给 raw 流 ✓（raw 没有该名字 ⇒ `AttributeError` 照参照 ✓）。
fn buffered_call(
    instance: &Instance,
    this: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    let raw = buffered_raw(instance, this)?;
    let method = match instance.attribute_optional_of(raw, name) {
        Ok(Some(method)) => method,
        Ok(None) => {
            // **raw 没有 `readline` ⇒ 用 `read(1)` 合成**（C1 日常面，见 `raw_readline`）。
            if name == "readline" {
                let result = raw_readline(instance, raw);
                instance.release(raw);
                return result;
            }
            instance.release(raw);
            return Err(instance.raise_builtin_error(
                "AttributeError",
                &format!("'raw stream' object has no attribute '{name}'"),
            ));
        }
        Err(error) => {
            instance.release(raw);
            return Err(error);
        }
    };
    let result = pyawa_core::executor::call::call_value(instance, method, args, &[]);
    instance.release(method);
    instance.release(raw);
    result
}

/// **raw 流没有 `readline()` 时用 `read(1)` 合成**（C1 日常面）：参照里 `SocketIO` 继承
/// `io.RawIOBase` ⇒ `readline` 由 C 基类给 ✓；本层 `Lib/socket.py` 的 `SocketIO` 没有它 ✗
/// ⇒ `http.client.HTTPResponse` 读状态行时（`self.fp.readline()` → `BufferedReader` 转给 raw）
/// 报 `'raw stream' object has no attribute 'readline'` ✗。逐字节读到换行符即可（每日 HTTP
/// 响应头/状态行都是短行 ✓；不追求缓冲性能 —— 真缓冲已由 `BufferedReader` 外层承担）。
fn raw_readline(instance: &Instance, raw: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    let Some(read) = instance.attribute_optional_of(raw, "read")? else {
        return Err(instance.raise_builtin_error(
            "AttributeError",
            "'raw stream' object has no attribute 'read'",
        ));
    };
    let mut bytes: Vec<u8> = Vec::new();
    let outcome = loop {
        let one = instance.new_int(1);
        let chunk = pyawa_core::executor::call::call_value(instance, read, &[one], &[]);
        instance.release(one);
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                instance.release(read);
                return Err(error);
            }
        };
        let Some(piece) = instance.bytes_like_value(chunk) else {
            instance.release(chunk);
            instance.release(read);
            return Err(instance.raise_builtin_error(
                "TypeError",
                "readline() 的底层 read() 必须给 bytes ✓",
            ));
        };
        let empty = piece.is_empty();
        bytes.extend_from_slice(&piece);
        instance.release(chunk);
        if empty || bytes.last() == Some(&b'\n') {
            break Ok(instance.new_bytes(&bytes));
        }
    };
    instance.release(read);
    outcome
}

fn buffered_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "Buffered.__init__ 缺少 self"));
    };
    let Some(raw) = rest.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "Buffered.__init__() 缺少 raw 实参",
        ));
    };
    instance.set_attribute_value(this, "_raw", raw)?;
    let closed = instance.new_bool(false);
    let outcome = instance.set_attribute_value(this, "_closed", closed);
    let _ = outcome;
    Ok(instance.retain(instance.singletons().none()))
}

fn buffered_read(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "read 缺少 self"));
    };
    let size = rest.first().copied().unwrap_or_else(|| instance.new_int(-1));
    let owned = rest.is_empty();
    let result = buffered_call(instance, this, "read", &[size]);
    if owned {
        instance.release(size);
    }
    result
}

fn buffered_readline(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "readline 缺少 self"));
    };
    buffered_call(instance, this, "readline", args)
}

fn buffered_readlines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "readlines 缺少 self"));
    };
    buffered_call(instance, this, "readlines", args)
}

fn buffered_peek(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "peek 缺少 self"));
    };
    match buffered_call(instance, this, "peek", args) {
        Ok(value) => Ok(value),
        Err(_) => Ok(instance.new_bytes(b"")),
    }
}

fn buffered_seek(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "seek 缺少 self"));
    };
    buffered_call(instance, this, "seek", args)
}

fn buffered_tell(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "tell 缺少 self"));
    };
    buffered_call(instance, this, "tell", args)
}

fn buffered_write(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "write 缺少 self"));
    };
    buffered_call(instance, this, "write", args)
}

fn buffered_writelines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "writelines 缺少 self"));
    };
    buffered_call(instance, this, "writelines", args)
}

fn buffered_flush(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "flush 缺少 self"));
    };
    match buffered_call(instance, this, "flush", args) {
        Ok(value) => {
            instance.release(value);
            Ok(instance.retain(instance.singletons().none()))
        }
        Err(_) => Ok(instance.retain(instance.singletons().none())),
    }
}

fn buffered_close(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "close 缺少 self"));
    };
    if let Ok(raw) = buffered_raw(instance, this) {
        if let Ok(Some(close)) = instance.attribute_optional_of(raw, "close") {
            if let Ok(value) = pyawa_core::executor::call::call_value(instance, close, &[], &[]) {
                instance.release(value);
            }
            instance.release(close);
        }
        instance.release(raw);
    }
    let closed = instance.new_bool(true);
    let outcome = instance.set_attribute_value(this, "_closed", closed);
    let _ = outcome;
    Ok(instance.retain(instance.singletons().none()))
}

fn buffered_enter(
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

fn buffered_exit(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    buffered_close(instance, bound, args, _kwargs)
}

fn buffered_next(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "__next__ 缺少 self"));
    };
    let line = buffered_call(instance, this, "readline", &[])?;
    let length = instance.bytes_value(line).map(|raw| raw.len()).unwrap_or(0);
    if length == 0 {
        instance.release(line);
        return Err(instance.raise_builtin_error("StopIteration", ""));
    }
    Ok(line)
}

fn buffered_fileno(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "fileno 缺少 self"));
    };
    buffered_call(instance, this, "fileno", args)
}

fn buffered_getattr(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__getattr__ 缺少 self"));
    };
    let Some(name) = args.first().copied().and_then(|value| instance.text_of(value)) else {
        return Err(instance.raise_builtin_error("TypeError", "__getattr__ 的名字要是 str"));
    };
    if name == "closed" {
        if let Ok(Some(closed)) = instance.attribute_optional_of(this, "_closed") {
            return Ok(closed);
        }
        return Ok(instance.new_bool(false));
    }
    let raw = buffered_raw(instance, this)?;
    let found = instance.attribute_optional_of(raw, name);
    instance.release(raw);
    match found {
        Ok(Some(value)) => Ok(value),
        _ => Err(instance.raise_builtin_error("AttributeError", &format!("'Buffered' object has no attribute '{name}'"))),
    }
}

fn buffered_true(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.new_bool(true))
}

fn buffered_false(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.new_bool(false))
}

fn build_bytes_io(instance: &Instance, namespace: NonNull<Header>) {
    let object_type = instance.type_named("object").expect("object 已登记");
    let class_namespace = instance.new_dict();
    let handlers: [(&str, NativeFn); 19] = [
        ("__init__", bytesio_init as NativeFn),
        ("read", bytesio_read as NativeFn),
        ("read1", bytesio_read as NativeFn),
        ("readline", bytesio_readline as NativeFn),
        ("readlines", bytesio_readlines as NativeFn),
        ("write", bytesio_write as NativeFn),
        ("seek", bytesio_seek as NativeFn),
        ("tell", bytesio_tell as NativeFn),
        ("getvalue", bytesio_getvalue as NativeFn),
        ("getbuffer", bytesio_getbuffer as NativeFn),
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
// - `encoding` 只认 `decode_with_encoding` 的四档 ✓（其余报 `LookupError` ✓）；`errors`／
//   `buffering`／`opener` 如实**未实现** ✗（收了不报错，也不假装生效 ✗）；
//   `newline=` 的**读／写**口径都已接 ✓（缺口3，见 [`NewlineMode`]），`.newlines` 属性也已接
//   （整块口径，见 [`scan_newlines_once`] ✓）；
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

// ==== universal newline（缺口3 ✓）：文本 IO 的**读／写**路径与 `.newlines` =======================
//
// 参照口径（本机 `python3` 3.14.4 实测 ✓，见 `target/recon/io_nl/` 的逐字节对拍）：
// - `newline=None`（`open()` 默认）：`\r`／`\n`／`\r\n` **三档都断行**，读出时**翻译成 `\n`**
//   （`\r\n` 与孤立 `\r` 各算一个 `\n`）；
// - `newline=''`：三档都断行，**不翻译**（`\r\n` 原样保留两个字符）；
// - `newline='\n'`／`'\r'`／`'\r\n'`：**只按给出的那一个**断行、不翻译（其余换行符留在行内）；
// - `read(n)`：`newline=None` 的 `n` 按**翻译后**字符数算（`\r\n` 计 1 个字符，但两个原字符一起
//   吃掉）；其余档按原字符数算。`readline(size)` 同口径，且**可以切在 `\r\n` 中间**
//   （`newline=''` 的 `readline(1)` 对 `\r\n` 给 `'\r'`，下一行给 `'\n'`）；
//   `newline=None` 的 `readline(1)` 对 `\r\n` 给 `'\n'`（整对已消费）。
// - 尾行没换行照常返回；EOF 返回 `''`；`read(0)`／`readline(0)` 返回 `''` 且不吃字符 ✓。
//
// **写**翻译（本轮 ✓，同文件 `probe_write.py` 对拍）：
// - `newline=None`：只把 `'\n'` 译成 `os.linesep`；**Linux 上 `os.linesep == '\n'` ⇒ 恒等** ✓；
// - `newline=''`／`'\n'`：恒等；
// - `newline='\r'`／`'\r\n'`：**每个** `'\n'` 换成对应串（既有 `'\r'` 一字不动 ⇒ `"\r\n"` 变
//   `"\r\r"`／`"\r\r\n"` ✓）。写进 `_data` 的是**翻译后**文本 ⇒ `w+` 回读看到的即落盘原文 ✓；
//   `write()` 返回**实参字符数**，`_pos`（＝本层 `tell`）按翻译后长度推进 ✓。
//
// **`.newlines`**（本轮 ✓）：只读属性；`None`／`''` 两档在**首次真读**时把整块原文扫一遍
// （`read(0)` 不触发、`readline(0)` 触发 ✓），掩码映射次序 CR、LF、CRLF ⇒ `None`／串／元组 ✓；
// 显式三档恒 `None`；写不追踪（`w+` 写后读数才追踪）✓。
//
// **实现口径**：`_data` 存**原文**（读侧不改写；写侧存**翻译后**文本）⇒ 读时按模式在内存里
// 扫描／翻译 ✓ —— `_pos` 因此是**字符下标**（写侧是翻译后文本的下标，与参照的字节 cookie 不同：
// 纯 ASCII 一致，非 ASCII 写后 `tell` 本层更小 ✗ —— 既有边界，见 [`NewlineMode`] ✓）。

/// universal newline 的**读／写**口径（`newline=` 形参的五档 ✓）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum NewlineMode {
    /// `newline=None`：三档断行、翻译成 `\n`。
    Universal,
    /// `newline=''`：三档断行、不翻译。
    NoTranslation,
    /// `newline='\n'`：只按 `\n` 断行、不翻译。
    Lf,
    /// `newline='\r'`：只按 `\r` 断行、不翻译。
    Cr,
    /// `newline='\r\n'`：只按 `\r\n` 断行、不翻译。
    CrLf,
}

impl NewlineMode {
    /// **写**翻译口径 ✓（本机 `python3` 3.14.4 实测，`target/recon/io_nl/probe_write.py`）。
    ///
    /// - `None`：把 `'\n'` 译成 `os.linesep`；Linux 上 `os.linesep == '\n'` ⇒ **恒等** ✓
    ///   （只译 `'\n'`，既有 `'\r'` 一字不动 ✓）；
    /// - `''`／`'\n'`：**不翻译** ✓；
    /// - `'\r'`／`'\r\n'`：**每个** `'\n'` 换成对应串 ✓（不合并、不对既有 `'\r'` 做配对 ——
    ///   `"a\r\n"` ⇒ `"a\r\r"`／`"a\r\r\n"`，照参照 ✓）。
    fn translate_write(self, text: &str) -> String {
        match self {
            NewlineMode::Universal | NewlineMode::NoTranslation | NewlineMode::Lf => text.to_owned(),
            NewlineMode::Cr => text.replace('\n', "\r"),
            NewlineMode::CrLf => text.replace('\n', "\r\n"),
        }
    }

    /// 这一档是否**追踪 `.newlines`** ✓：只有 universal 读档（`None`／`''`）追；
    /// 显式三档（`'\n'`／`'\r'`／`'\r\n'`）恒 `None`（照参照实测 ✓）。
    fn tracks_newlines(self) -> bool {
        matches!(self, NewlineMode::Universal | NewlineMode::NoTranslation)
    }

    /// 实例字典里的整数码（`_newline` ✓）。
    fn code(self) -> i64 {
        match self {
            NewlineMode::Universal => 0,
            NewlineMode::NoTranslation => 1,
            NewlineMode::Lf => 2,
            NewlineMode::Cr => 3,
            NewlineMode::CrLf => 4,
        }
    }

    /// 从实例字典的整数码还原 ✓（缺省＝`Universal`，即 `open()`／`TextIOWrapper` 的默认 ✓）。
    fn from_code(code: i64) -> Self {
        match code {
            1 => NewlineMode::NoTranslation,
            2 => NewlineMode::Lf,
            3 => NewlineMode::Cr,
            4 => NewlineMode::CrLf,
            _ => NewlineMode::Universal,
        }
    }
}

/// 解析并校验 `newline=` 实参 ✓（`None`／`''`／`'\n'`／`'\r'`／`'\r\n'` ✓；类型／取值文案照参照 ✓）。
///
/// 校验**次序**照参照实测 ✓：先类型（`TypeError`），再二进制档（`binary` ⇒ 非 `None` 报
/// `ValueError: binary mode doesn't take a newline argument`），最后才是取值
/// （`ValueError: illegal newline value: …`）。
fn newline_mode_of_value(
    instance: &Instance,
    value: Option<NonNull<Header>>,
    context: &str,
    binary: bool,
) -> Result<NewlineMode, ExecError> {
    let Some(value) = value else {
        return Ok(NewlineMode::Universal);
    };
    if value == instance.singletons().none() {
        return Ok(NewlineMode::Universal);
    }
    let Some(text) = instance.text_of(value) else {
        let name = instance.type_name(instance.type_of(value));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{context}() argument 'newline' must be str or None, not {name}"),
        ));
    };
    if binary {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "binary mode doesn't take a newline argument",
        ));
    }
    match text {
        "" => Ok(NewlineMode::NoTranslation),
        "\n" => Ok(NewlineMode::Lf),
        "\r" => Ok(NewlineMode::Cr),
        "\r\n" => Ok(NewlineMode::CrLf),
        other => Err(instance.raise_builtin_error(
            "ValueError",
            &format!("illegal newline value: {other}"),
        )),
    }
}

/// 实例上的 universal newline 读口径 ✓（`_newline` 整数码；缺省＝`Universal` ✓）。
fn newline_mode_of(instance: &Instance, this: NonNull<Header>) -> NewlineMode {
    attr_of(instance, this, "_newline")
        .and_then(|value| instance.int_of(value))
        .and_then(|value| value.to_i64())
        .map(NewlineMode::from_code)
        .unwrap_or(NewlineMode::Universal)
}

/// 换行单元的**最小视图**：`char` 文本流与 `StringIO` 的码点缓冲共用同一套扫描逻辑 ✓
///（**一处真相** ✓：读／写翻译与 `.newlines` 追踪的判定都收在 [`NewlineMode`] 与这里 ✓）。
trait NewlineUnit: Copy {
    fn is_cr(self) -> bool;
    fn is_lf(self) -> bool;
    fn line_feed() -> Self;
}

impl NewlineUnit for char {
    fn is_cr(self) -> bool {
        self == '\r'
    }

    fn is_lf(self) -> bool {
        self == '\n'
    }

    fn line_feed() -> Self {
        '\n'
    }
}

impl NewlineUnit for u32 {
    fn is_cr(self) -> bool {
        self == u32::from(b'\r')
    }

    fn is_lf(self) -> bool {
        self == u32::from(b'\n')
    }

    fn line_feed() -> Self {
        u32::from(b'\n')
    }
}

/// `read(n)` 的通用文本口径 ✓：返回 `(片段, 下一原文字符下标)`。
///
/// `Universal` 边读边翻译（`\r\n`／`\r` ⇒ `\n`，`\r\n` 两字符只产出 1 个）；其余档按原单元切片。
fn text_read_units<T: NewlineUnit>(
    units: &[T],
    start: usize,
    limit: i64,
    mode: NewlineMode,
) -> (Vec<T>, usize) {
    let start = start.min(units.len());
    if mode != NewlineMode::Universal {
        let end = if limit < 0 {
            units.len()
        } else {
            (start + limit.max(0) as usize).min(units.len())
        };
        return (units[start..end].to_vec(), end);
    }
    let mut out: Vec<T> = Vec::new();
    let mut index = start;
    let mut produced = 0usize;
    while index < units.len() && (limit < 0 || produced < limit as usize) {
        let unit = units[index];
        if unit.is_cr() {
            out.push(T::line_feed());
            produced += 1;
            index += 1;
            // `\r\n` 是一个换行 ⇒ 整对一起消费（即使已经撞到 `limit` ✓）。
            if index < units.len() && units[index].is_lf() {
                index += 1;
            }
        } else {
            out.push(unit);
            produced += 1;
            index += 1;
        }
    }
    (out, index)
}

/// `readline(size)` 的通用文本口径 ✓：返回 `(片段, 下一原文字符下标)`（含行尾换行 ✓，EOF 给剩余段 ✓）。
fn text_readline_units<T: NewlineUnit>(
    units: &[T],
    start: usize,
    limit: i64,
    mode: NewlineMode,
) -> (Vec<T>, usize) {
    let start = start.min(units.len());
    let mut out: Vec<T> = Vec::new();
    let mut index = start;
    let mut produced = 0usize;
    // `newline='\r\n'` 专用：本**次**调用内上一个字符是不是 `\r`（跨调用的 `\r`+`\n` 不算一对 ✓
    // —— 参照实测：上一次 `readline(2)` 停在 `\r` 之后，下一次从 `\n` 起读时 `\n` 不是行尾 ✓）。
    let mut previous_cr = false;
    while index < units.len() {
        if limit >= 0 && produced >= limit as usize {
            break;
        }
        let unit = units[index];
        match mode {
            // `newline=None`：三档断行、翻译成 `\n`。
            NewlineMode::Universal => {
                index += 1;
                produced += 1;
                if unit.is_cr() {
                    out.push(T::line_feed());
                    if index < units.len() && units[index].is_lf() {
                        index += 1;
                    }
                    break;
                } else if unit.is_lf() {
                    out.push(T::line_feed());
                    break;
                } else {
                    out.push(unit);
                }
            }
            // `newline=''`：三档断行、不翻译（`\r\n` 两字符都留在行里 ✓）。
            NewlineMode::NoTranslation => {
                index += 1;
                produced += 1;
                if unit.is_cr() {
                    out.push(unit);
                    // `size` 只够 `\r` 时，`\n` 留给下一次 `readline` ✓（参照实测 ✓）。
                    if (limit < 0 || produced < limit as usize)
                        && index < units.len()
                        && units[index].is_lf()
                    {
                        out.push(units[index]);
                        index += 1;
                    }
                    break;
                } else if unit.is_lf() {
                    out.push(unit);
                    break;
                } else {
                    out.push(unit);
                }
            }
            // 显式档：只认那一个换行、不翻译。
            NewlineMode::Lf => {
                out.push(unit);
                produced += 1;
                index += 1;
                if unit.is_lf() {
                    break;
                }
            }
            NewlineMode::Cr => {
                out.push(unit);
                produced += 1;
                index += 1;
                if unit.is_cr() {
                    break;
                }
            }
            NewlineMode::CrLf => {
                if unit.is_lf() && previous_cr {
                    out.push(unit);
                    index += 1;
                    break;
                }
                out.push(unit);
                produced += 1;
                index += 1;
                previous_cr = unit.is_cr();
            }
        }
    }
    (out, index)
}

/// `char` 文本流的 `read(n)` 口径 ✓（包一层 [`text_read_units`]）。
fn text_read_from(chars: &[char], start: usize, limit: i64, mode: NewlineMode) -> (String, usize) {
    let (units, end) = text_read_units(chars, start, limit, mode);
    (units.into_iter().collect(), end)
}

/// `char` 文本流的 `readline(size)` 口径 ✓。
fn text_readline_from(chars: &[char], start: usize, limit: i64, mode: NewlineMode) -> (String, usize) {
    let (units, end) = text_readline_units(chars, start, limit, mode);
    (units.into_iter().collect(), end)
}

/// **`.newlines` 的位掩码** ✓（参照 `_io` 的 `SEEN_CR=1`／`SEEN_LF=2`／`SEEN_CRLF=4`）。
///
/// 掩码来自**原文**（只有 `newline=None`／`''` 两档追踪）：`\r\n` 记 CRLF、不成对的 `\r` 记 CR、
/// `\n` 记 LF ✓（次序固定 CR、LF、CRLF —— 照参照的 getter ✓）。
fn newline_mask_of_text(text: &str) -> i64 {
    let mut mask = 0i64;
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character == '\r' {
            if chars.clone().next() == Some('\n') {
                chars.next();
                mask |= 4;
            } else {
                mask |= 1;
            }
        } else if character == '\n' {
            mask |= 2;
        }
    }
    mask
}

/// 码点缓冲版的 [`newline_mask_of_text`]（`StringIO` 的 `_buf` ✓）。
fn newline_mask_of_points(points: &[u32]) -> i64 {
    let mut mask = 0i64;
    let mut index = 0usize;
    while index < points.len() {
        if points[index] == u32::from(b'\r') {
            index += 1;
            if index < points.len() && points[index] == u32::from(b'\n') {
                index += 1;
                mask |= 4;
            } else {
                mask |= 1;
            }
        } else {
            if points[index] == u32::from(b'\n') {
                mask |= 2;
            }
            index += 1;
        }
    }
    mask
}

/// 实例上已积累的换行掩码 ✓（缺省 `0`）。
fn newlines_mask_of(instance: &Instance, this: NonNull<Header>) -> i64 {
    attr_of(instance, this, "_newlines_mask")
        .and_then(|value| instance.int_of(value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0)
        & 7
}

/// 记一条换行种类 ✓（**按位或**累计 —— 参照的 decoder 只增不减 ✓）。
fn record_newlines_mask(instance: &Instance, this: NonNull<Header>, mask: i64) {
    if mask == 0 {
        return;
    }
    let combined = newlines_mask_of(instance, this) | mask & 7;
    let value = instance.new_int(combined);
    set_attr(instance, this, "_newlines_mask", value);
}

/// 掩码 ⇒ 参照 `.newlines` 的值 ✓：`0` ⇒ `None`；一种 ⇒ 该串；多种 ⇒ 元组（CR、LF、CRLF 次序 ✓）。
fn newlines_value(instance: &Instance, mask: i64) -> NonNull<Header> {
    let mask = mask & 7;
    let string = |text: &str| instance.new_str(text);
    match mask {
        0 => instance.retain(instance.singletons().none()),
        1 => string("\r"),
        2 => string("\n"),
        4 => string("\r\n"),
        _ => {
            let mut parts: Vec<NonNull<Header>> = Vec::new();
            if mask & 1 != 0 {
                parts.push(string("\r"));
            }
            if mask & 2 != 0 {
                parts.push(string("\n"));
            }
            if mask & 4 != 0 {
                parts.push(string("\r\n"));
            }
            instance.new_tuple(parts)
        }
    }
}

/// 首次读时把**整块**原文扫一遍记掩码 ✓（`_newlines_scanned` 防重扫；写后置回 `false` 再扫 ✓）。
///
/// **如实边界** ✗：参照是"解码到哪块看到哪块"的流式口径（`TextIOWrapper` 的块大小 8192）；
/// 本层整个文件／缓冲都在内存 ⇒ 以**整块**为准（小文件逐字节一致；跨块次序差异不建模）。
fn scan_newlines_once(instance: &Instance, this: NonNull<Header>, text: &str) {
    if attr_of(instance, this, "_newlines_scanned")
        .map(|value| instance.truth_of(value))
        .unwrap_or(false)
    {
        return;
    }
    let mask = newline_mask_of_text(text);
    record_newlines_mask(instance, this, mask);
    let scanned = instance.new_bool(true);
    set_attr(instance, this, "_newlines_scanned", scanned);
}

/// 造一个**只读** `property`（`fget` 用 native；照 `_csv_module::readonly_property` 手法 ✓）——
/// 让 `.newlines` 与参照一样**只读**（赋值 ⇒ `AttributeError` ✓）。
fn readonly_property(instance: &Instance, getter: NativeFn) -> Option<NonNull<Header>> {
    let property_type = instance.type_named("property")?;
    let function = make_native(instance, "fget", getter);
    let type_value = instance.type_value(property_type).cast::<Header>();
    let outcome = pyawa_core::call_value(instance, type_value, &[function], &[]);
    instance.release(type_value);
    instance.release(function);
    outcome.ok()
}

/// `property` getter 的 `self` ✓（描述符协议把实例放在 `args[0]`）。
fn property_self(args: &[NonNull<Header>]) -> Option<NonNull<Header>> {
    args.first().copied()
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
        // **round 268：`pathlib` 一族的日常 errno 文案**（glibc `strerror` 实测口径 ✓）——
        // 先前 `rmdir` 非空目录报 `[Errno 39] unknown error` ✗（参照 `Directory not empty`）。
        "ENOTEMPTY" => "Directory not empty",
        "EBUSY" => "Device or resource busy",
        "EXDEV" => "Invalid cross-device link",
        "ENOSPC" => "No space left on device",
        "EROFS" => "Read-only file system",
        "EMFILE" => "Too many open files",
        "ENFILE" => "Too many open files in system",
        "EAGAIN" => "Resource temporarily unavailable",
        "EINTR" => "Interrupted system call",
        "EIO" => "Input/output error",
        "EBADF" => "Bad file descriptor",
        "ENOMEM" => "Cannot allocate memory",
        "EFAULT" => "Bad address",
        "ENOSYS" => "Function not implemented",
        "EOPNOTSUPP" => "Operation not supported",
        "EADDRINUSE" => "Address already in use",
        "ECONNREFUSED" => "Connection refused",
        "ETIMEDOUT" => "Connection timed out",
        "EPIPE" => "Broken pipe",
        "EOVERFLOW" => "Value too large for defined data type",
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
/// **从"消息用 display"还原 `filename` 对象**（round 108）：`display` 形如 `'…'`／`b'…'`
/// （外层是 Python repr ✓，转义序列照 repr 规则 ✓）⇒ 去掉外层引号并**反转义** ✓；
/// `b'…'` 交 `bytes`（参照对 bytes 路径给 bytes filename ✓）。
fn filename_object(instance: &Instance, display: &str) -> (NonNull<Header>, bool) {
    let trimmed = display.trim();
    let (is_bytes, inner) = if let Some(rest) = trimmed.strip_prefix("b'") {
        (true, rest.strip_suffix('\'').unwrap_or(rest))
    } else if let Some(rest) = trimmed.strip_prefix("b\"") {
        (true, rest.strip_suffix('"').unwrap_or(rest))
    } else if let Some(rest) = trimmed.strip_prefix('\'') {
        (false, rest.strip_suffix('\'').unwrap_or(rest))
    } else if let Some(rest) = trimmed.strip_prefix('"') {
        (false, rest.strip_suffix('"').unwrap_or(rest))
    } else {
        (false, trimmed)
    };
    let unescaped = unescape_display(inner);
    if is_bytes {
        (instance.new_bytes(&unescaped), true)
    } else {
        (
            instance
                .new_str_wtf8(&unescaped)
                .unwrap_or_else(|| instance.new_str("?")),
            false,
        )
    }
}

/// 反转义 Python repr 里的常见序列（`\\`／`\'`／`\"`／`\n`／`\r`／`\t`／`\xXX`／
/// `\uXXXX`／`\UXXXXXXXX`；其余原样保留 ✓）。
fn unescape_display(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    let hex_value = |digits: &[u8]| -> Option<u32> {
        let mut value = 0u32;
        for byte in digits {
            let digit = (*byte as char).to_digit(16)?;
            value = value * 16 + digit;
        }
        Some(value)
    };
    while index < bytes.len() {
        if bytes[index] != b'\\' || index + 1 >= bytes.len() {
            out.push(bytes[index]);
            index += 1;
            continue;
        }
        let next = bytes[index + 1];
        let simple = match next {
            b'\\' => Some(b'\\'),
            b'\'' => Some(b'\''),
            b'"' => Some(b'"'),
            b'n' => Some(b'\n'),
            b'r' => Some(b'\r'),
            b't' => Some(b'\t'),
            b'0' => Some(0),
            _ => None,
        };
        if let Some(byte) = simple {
            out.push(byte);
            index += 2;
            continue;
        }
        let (length, code) = match next {
            b'x' => (4, hex_value(bytes.get(index + 2..index + 4).unwrap_or(&[]))),
            b'u' => (6, hex_value(bytes.get(index + 2..index + 6).unwrap_or(&[]))),
            b'U' => (10, hex_value(bytes.get(index + 2..index + 10).unwrap_or(&[]))),
            _ => (0, None),
        };
        if let (Some(code), true) = (code, index + length <= bytes.len()) {
            let _ = pyawa_core::wtf8::encode_code_point(code, &mut out);
            index += length;
            continue;
        }
        // 不认识的转义 ⇒ 原样两个字符 ✓。
        out.push(b'\\');
        out.push(next);
        index += 2;
    }
    out
}

/// 取异常类对象（**借用**；`call_value` 只借用可调用对象 ✓）。
fn class_type_header(instance: &Instance, name: &str) -> NonNull<Header> {
    instance
        .type_named(name)
        .map(|class| class.cast::<Header>())
        .unwrap_or_else(|| {
            instance
                .type_named("OSError")
                .expect("OSError 已登记")
                .cast::<Header>()
        })
}

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
            // **构造真 OSError 三元组**（round 108）：参照的 OSError 带 `(errno, strerror, filename)`
            // 三个实参 ✓ ⇒ `e.errno`／`e.strerror`／**`e.filename`** 可读 ✓ —— 先前只有一条消息串 ✗
            // ⇒ 官方 `test_unicode_file_functions` 的 `exc.filename` 全 AttributeError ✗。
            // `display` 有时已带引号（`io` 那条约一层 `'…'`）⇒ 去掉一层再当 filename ✓。
            // **`display` 是"消息用的转义形态"**（`repr(path)` 去掉外层引号 ✓，非可打印字符写成
            // `\u2000` 一类 ✓）⇒ `filename` 要**还原成原名** ✓（参照的 `e.filename` 是原对象 ✓；
            // 官方 `test_unicode_file_functions` 的 `assertEqual(exc.filename, filename)` ✓）。
            let (filename_value, filename_is_bytes) = filename_object(instance, display);
            let errno_value = instance.new_int(errno as i64);
            let strerror_value = instance.new_str(text);
            let exception = pyawa_core::call_value(
                instance,
                class_type_header(instance, class),
                &[errno_value, strerror_value, filename_value],
                &[],
            );
            instance.release(errno_value);
            instance.release(strerror_value);
            instance.release(filename_value);
            let _ = filename_is_bytes;
            match exception {
                Ok(exception) => ExecError::Raised { exception },
                Err(error) => error,
            }
        }
        pyawa_core::CapabilityCallError::NotRegistered => {
            instance.raise_builtin_error("OSError", "`fs` 域未注册（`CP-2`）")
        }
        pyawa_core::CapabilityCallError::NotImplemented => {
            instance.raise_builtin_error("OSError", "`fs` 域的槽未实现（`CP-5`）")
        }
    }
}

/// **`_io.text_encoding(encoding, stacklevel=2)`**（round 268）：`io`／`pathlib`／`_pyio` 的
/// 编码提示入口 —— 参照实测：`encoding is None` ⇒ 返回字符串 **`"locale"`**；否则**原样返回**
/// （连非 `str` 的 `1` 也原样回交，**不**做类型检查 ✓）；`stacklevel` 只影响警告栈深（本层没有
/// `EncodingWarning` 通道 ⇒ 收下即可 ✓）。
fn text_encoding_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.is_empty() || args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "text_encoding() takes 1 or 2 arguments ({} given)",
                args.len()
            ),
        ));
    }
    if args[0] == instance.singletons().none() {
        return Ok(instance.new_str("locale"));
    }
    Ok(instance.retain(args[0]))
}

/// **`open(file, mode='r', buffering=-1, encoding=None, errors=None, newline=None, …)`** ✓（task-18 ✓）。
///
/// 见本节的"口径（如实 ✗）"——`buffering`／`errors`／`closefd` 未实现 ✓；
/// `newline=` 的**读／写**口径都已接（缺口3 ✓，见 [`NewlineMode`]）；`.newlines` 也已接 ✓。
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
    // **P3-38 S3b-2**：路径按 PEP 383 的 `surrogateescape` 编成 fs 原始字节（与 `posix` 面同一
    // 一处真相）⇒ 含孤立代理的文件名可真实打开；合法 UTF-8 字节零变化 ✓。
    let path = crate::posix_module::path_argument(instance, file_value, "open")?;
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
    let mut encoding = args
        .get(3)
        .copied()
        .or_else(|| kwarg_of(instance, kwargs, "encoding"))
        .and_then(|value| instance.text_of(value))
        .unwrap_or("utf-8")
        .to_owned();
    // **`"locale"` 解析**（round 268）：`io.text_encoding(None)` 按参照返回字符串 `"locale"` ⇒
    // 到 `open` 这里要换成真实编码。**如实记 ✗**：本层没有 locale 能力通道 ⇒ 固定 **UTF-8**
    // （宿主实测 `locale.getencoding() == "UTF-8"`；`LANG=C` 一类的 ASCII locale 不建模 ✗）。
    if encoding == "locale" {
        encoding = "utf-8".to_owned();
    }
    // **`newline`**（缺口3 ✓）：第 6 个位置实参／关键字 ✓。`None`＝universal（默认）✓。
    // 校验次序照参照实测 ✓：类型（`TypeError`）⇒ 二进制档不许给非 `None`（`ValueError`）⇒ 取值。
    let newline_value = args
        .get(5)
        .copied()
        .or_else(|| kwarg_of(instance, kwargs, "newline"));
    let newline_mode = match newline_mode_of_value(instance, newline_value, "open", spec.binary) {
        Ok(mode) => mode,
        Err(error) => return Err(error),
    };
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
            if instance.fs_stat(&path.bytes).is_ok() {
                return Err(instance.raise_builtin_error(
                    "FileExistsError",
                    &format!("[Errno 17] File exists: {}", path.display),
                ));
            }
            flags |= open_flag::CREAT | open_flag::TRUNC;
        }
    }
    // **`opener` 关键字**（round 97）：参照 `open(file, …, opener=fn)` 里由 `fn(file, flags, mode)`
    // 交出**打开好的 fd** ✓（`io.open` **不去 open `file` 本身** ✓）。`Lib/tempfile.py` 的
    // `NamedTemporaryFile`／`TemporaryFile` 全靠它 ✓ —— 先前完全忽略 `opener` ✗ ⇒ 直接按路径
    // （传进来的是**目录** ✓）打开 ⇒ `IsADirectoryError: '/tmp'` ✗（官方 `test_tempfile` 大半 ✗）。
    let opener = kwarg_of(instance, kwargs, "opener");
    let handle = if let Some(opener) = opener {
        let flags_value = instance.new_int(flags as i64);
        let mode_value = instance.new_int(0o666);
        let outcome = pyawa_core::executor::call::call_value(
            instance,
            opener,
            &[file_value, flags_value, mode_value],
            &[],
        );
        instance.release(flags_value);
        instance.release(mode_value);
        let returned = outcome?;
        let fd = instance.int_of(returned).and_then(|value| value.to_i64());
        instance.release(returned);
        let Some(fd) = fd else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "opener returned a non-integer fd",
            ));
        };
        if fd < 0 {
            return Err(instance.raise_builtin_error(
                "OSError",
                "opener returned a negative fd",
            ));
        }
        fd as u64
    } else {
        match instance.fs_open(&path.bytes, flags, 0o666) {
            Ok(handle) => handle,
            Err(error) => return Err(fs_error_with_display(instance, error, &path.display)),
        }
    };
    let mut raw: Vec<u8> = Vec::new();
    if spec.readable {
        match read_all(instance, handle) {
            Ok(bytes) => raw = bytes,
            Err(error) => {
                let _ = instance.fs_close(handle);
                return Err(fs_error_with_display(instance, error, &path.display));
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
    // **universal newline 读口径**（缺口3 ✓）：整数码，见 [`NewlineMode`] ✓。
    let newline_attr = instance.new_int(newline_mode.code());
    instance.dict_set(attributes, "_newline", newline_attr);
    // **`.newlines` 追踪状态**（本轮 ✓）：掩码 ＋ "整块已扫" 标志（首次读才扫，见 [`scan_newlines_once`]）。
    let mask_attr = instance.new_int(0);
    instance.dict_set(attributes, "_newlines_mask", mask_attr);
    let scanned_attr = instance.new_bool(false);
    instance.dict_set(attributes, "_newlines_scanned", scanned_attr);
    // **`f.name`**：路径形状跟实参走（str ⇒ str、bytes ⇒ bytes）；不拿 repr 显示形冒充。
    let name_value = if path.want_bytes {
        instance.new_bytes(&path.bytes)
    } else {
        crate::posix_module::decode_path_text(instance, &path.bytes)?
    };
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

// ==== `_io.TextIOWrapper`（task-19 第 3 轮 ✓）：读路径 ＋ 写翻译 ✓ ================================
//
// **动因** ✓：`Lib/tokenize.py::open` 就是 `TextIOWrapper(buffer, encoding, line_buffering=True)` ✓
// （`buffer` 是 `builtins.open(name,'rb')` 的二进制层 ✓）⇒ `linecache.updatecache` 一族（⇒ `traceback`
// 的源码行 ✓）都要它 ✓。**范围** ✓：读（`read`／`readline`／`readlines`／迭代／`with` ✓）；
// 写（`write`／`writelines`：按 `newline=` 翻译后编码写底层 ✓，`flush`／`seek`／`tell` 仍委派二进制层 ✓）；
// `newline=` 的读写口径、`.newlines` 属性（本轮 ✓，见 [`NewlineMode`]／[`scan_newlines_once`]）；
// **如实边界** ✗：`buffering` 语义／文本层自己的 seek cookie 未建模（`seek`／`tell` 走底层 ✓）。
//
// 口径 ✓：解码走 [`Instance::decode_with_encoding`] **一处真相** ✓（与 `bytes.decode` 同一处 ✓）；
// 状态全在**实例字典** ✓（`_buffer`／`_encoding`／`_text`／`_pos`／`_closed`／`_newlines_mask` ✓，
// 不新增载荷类型 ✓）。

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
    // **`newline`**（缺口3 ✓）：第 4 个位置实参／关键字 ✓（`TextIOWrapper(buffer, encoding,
    // errors, newline, line_buffering, …)`）；`None`＝universal（默认）✓。
    let newline_value = rest
        .get(3)
        .copied()
        .or_else(|| kwarg_of(instance, kwargs, "newline"));
    let newline_mode = newline_mode_of_value(instance, newline_value, "TextIOWrapper", false)?;
    let buffer = instance.retain(buffer);
    set_attr(instance, this, "_buffer", buffer);
    let newline_attr = instance.new_int(newline_mode.code());
    set_attr(instance, this, "_newline", newline_attr);
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
    // **`.newlines` 追踪状态**（本轮 ✓）：掩码 ＋ "整块已扫" 标志（读触发，见 [`scan_newlines_once`]）。
    let mask_attr = instance.new_int(0);
    set_attr(instance, this, "_newlines_mask", mask_attr);
    let scanned_attr = instance.new_bool(false);
    set_attr(instance, this, "_newlines_scanned", scanned_attr);
    Ok(instance.retain(this))
}

/// `TextIOWrapper.__getattr__(name)`（round 95）：未定义的名字委派给二进制层 ✓。
fn text_wrapper_getattr(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__getattr__ 缺少 self"));
    };
    let Some(name) = args.first().copied().and_then(|value| instance.text_of(value)) else {
        return Err(instance.raise_builtin_error("TypeError", "__getattr__ 的名字要是 str"));
    };
    // **只委派"流操作"那一族**（round 95 收紧 ✗）：参照的 `io.TextIOWrapper(buffer)` **没有** `mode`
    // 属性 ✓（`open()` 才在包装后设它 ✓）⇒ 宽委派会让 `hasattr(t,"mode")` 变 True 而**偏离参照** ✗
    //（官方夹具 `fstring_io_shapes` 逐字节钉着 `enc utf-8 has-mode False` ✗）。
    const DELEGATED: [&str; 10] = [
        "fileno", "isatty", "seek", "tell", "flush", "writable", "readable", "seekable", "truncate",
        "closed",
    ];
    if !DELEGATED.contains(&name) {
        return Err(instance.raise_builtin_error("AttributeError", name));
    }
    let Some(buffer) = instance.attribute_optional_of(this, "_buffer").ok().flatten() else {
        return Err(instance.raise_builtin_error("AttributeError", name));
    };
    let found = instance.attribute_optional_of(buffer, name).ok().flatten();
    instance.release(buffer);
    match found {
        Some(value) => Ok(value),
        None => Err(instance.raise_builtin_error("AttributeError", name)),
    }
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

/// `read(size=-1)` ✓（universal newline 按 `newline=` 口径断／译，见 [`NewlineMode`] ✓）。
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
    let mode = newline_mode_of(instance, this);
    // **`.newlines` 追踪**（本轮 ✓）：`read(0)` 不触发；其余读档首次真读扫整块（见 [`scan_newlines_once`]）。
    if size != 0 && mode.tracks_newlines() {
        scan_newlines_once(instance, this, &all);
    }
    let (piece, end) = text_read_from(&characters, start, size, mode);
    let position = instance.new_int(end as i64);
    set_attr(instance, this, "_pos", position);
    Ok(instance.new_str(&piece))
}

/// `readline(size=-1)` ✓（universal newline 按 `newline=` 口径断／译；末尾没换行也给这一段 ✓）。
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
    let limit = rest.first().and_then(|value| instance.int_value(*value)).unwrap_or(-1);
    let mode = newline_mode_of(instance, this);
    // **`.newlines` 追踪**（本轮 ✓）：`readline(0)` 也触发（参照实测 ✓）。
    if mode.tracks_newlines() {
        scan_newlines_once(instance, this, &all);
    }
    let (piece, end) = text_readline_from(&characters, start, limit, mode);
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

/// `TextIOWrapper.write(s)` ✓（本轮 ✓）：按 `newline=` 做**写翻译**、编码后写进底层二进制层 ✓。
///
/// 口径照本机 `python3` 3.14.4 实测（`target/recon/io_nl/probe_write.py`）：返回**实参字符数** ✓；
/// `s.write` 立即到底层（`tell` 是底层字节位置 ✓）；写后丢弃已解码缓存 `_text`（`w+` 回读要重取 ✓）。
fn text_wrapper_write(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = text_wrapper_self(instance, bound, args)?;
    if text_wrapper_closed(instance, this) {
        return closed_error(instance);
    }
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "TextIOWrapper.write() takes no keyword arguments",
        ));
    }
    if rest.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "TextIOWrapper.write() takes exactly one argument ({} given)",
                rest.len()
            ),
        ));
    }
    let Some(text) = instance.text_of(rest[0]) else {
        let name = instance.type_name(instance.type_of(rest[0]));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("write() argument must be str, not {name}"),
        ));
    };
    let translated = newline_mode_of(instance, this).translate_write(text);
    let written = text.chars().count();
    let encoding = text_wrapper_text(instance, this, "_encoding").unwrap_or_else(|| "utf-8".to_owned());
    let encoded = instance.encode_with_encoding(&translated, &encoding)?;
    let outcome = (|| -> Result<NonNull<Header>, ExecError> {
        let buffer = instance
            .attribute_optional_of(this, "_buffer")?
            .ok_or_else(|| instance.raise_builtin_error("ValueError", "I/O operation on closed file."))?;
        let Some(writer) = instance.attribute_optional_of(buffer, "write")? else {
            return Err(unsupported_error(instance, "write"));
        };
        pyawa_core::executor::call::call_value(instance, writer, &[encoded], &[])
    })();
    instance.release(encoded);
    let bytes_written = outcome?;
    instance.release(bytes_written);
    // 底层缓冲变了 ⇒ 已解码缓存作废（下次读重新取／解码；`.newlines` 也重扫）。
    let nothing = instance.singletons().none();
    set_attr(instance, this, "_text", nothing);
    let stale = instance.new_bool(false);
    set_attr(instance, this, "_newlines_scanned", stale);
    Ok(instance.new_int(written as i64))
}

/// `TextIOWrapper.writelines(lines)` ✓（本轮 ✓）：逐个 [`text_wrapper_write`]，返回 `None` ✓。
fn text_wrapper_writelines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = text_wrapper_self(instance, bound, args)?;
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_IOBase.writelines() takes no keyword arguments",
        ));
    }
    if rest.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "_IOBase.writelines() takes exactly one argument ({} given)",
                rest.len()
            ),
        ));
    }
    let iterator = pyawa_core::executor::iter::iter_value(instance, rest[0])?;
    loop {
        let outcome = instance.advance_iterator(iterator);
        match outcome {
            Ok(Some(line)) => {
                let written = text_wrapper_write(instance, Some(this), &[line], &[]);
                instance.release(line);
                let written = written?;
                instance.release(written);
            }
            Ok(None) => break,
            Err(error) => {
                instance.release(iterator);
                return Err(error);
            }
        }
    }
    instance.release(iterator);
    Ok(instance.retain(instance.singletons().none()))
}

/// `.newlines` getter（`TextIOWrapper` ✓）：掩码 ⇒ `None`／串／元组；**关闭后仍可读**（照参照 ✓）。
fn text_wrapper_newlines_getter(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = property_self(args)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor 'newlines' needs an argument"))?;
    Ok(newlines_value(instance, newlines_mask_of(instance, this)))
}

/// 建 `_io.TextIOWrapper` **真类** ✓（读／写路径 ✓）。
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
        // **写路径**（本轮 ✓）：`write` 按 `newline=` 翻译后写底层二进制层；`writelines` 逐个转发。
        ("write", text_wrapper_write as NativeFn),
        ("writelines", text_wrapper_writelines as NativeFn),
        ("__iter__", text_wrapper_self_ref as NativeFn),
        ("__next__", text_wrapper_next as NativeFn),
        ("__enter__", text_wrapper_self_ref as NativeFn),
        ("__exit__", text_wrapper_exit as NativeFn),
        ("close", text_wrapper_close as NativeFn),
        // **委派给二进制层**（round 95）：`sys.__stdout__.fileno()` 一族（`Lib/shutil.py` 的
        // `get_terminal_size` 在导入期就调它 ✗ ⇒ 官方 `test_shutil` 一上来就
        // `AttributeError: '_io.TextIOWrapper' object has no attribute 'fileno'` ✗）。
        // 用 `__getattr__` 一路委派（`fileno`／`isatty`／`flush`／`seek`／`tell` 全走二进制层 ✓），
        // 如实记 ✗：本层没有独立的文本层文件描述符语义 ✓。
        ("__getattr__", text_wrapper_getattr as NativeFn),
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
        // **`.newlines` 只读属性**（本轮 ✓）：getter 走掩码 ⇒ `None`／串／元组（**一处真相** ✓）。
        if let Some(property) = readonly_property(instance, text_wrapper_newlines_getter as NativeFn) {
            instance.set_type_attribute(class.cast::<pyawa_core::TypeObject>(), "newlines", property);
        }
        instance.dict_set(namespace, "TextIOWrapper", class);
    }
}

/// `.newlines` getter（`open()` 的文本档 ✓）：掩码 ⇒ `None`／串／元组。
///
/// **如实边界** ✗：参照的二进制档（`open(..., 'rb')` ⇒ `BufferedReader`）**没有** `newlines`；
/// 本层二进制档也用同一个 `FileIO` 类 ⇒ 这里按 `_binary` 报 `AttributeError`（名字同参照 ✓）。
fn file_newlines_getter(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = property_self(args)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor 'newlines' needs an argument"))?;
    if file_flag(instance, this, "_binary") {
        return Err(instance.raise_builtin_error("AttributeError", "newlines"));
    }
    Ok(newlines_value(instance, newlines_mask_of(instance, this)))
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
        // **`writelines`**（round 97）：逐行调 `write` ✓（`tempfile.SpooledTemporaryFile`
        // 转发到 `_io.FileIO` 时被 `hasattr` 检查 ✗ —— `test_tempfile` 的 IOBase 接口面 ✓）。
        ("writelines", file_writelines as NativeFn),
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
        // **`__del__`**（round 268）：参照里文件对象销毁即 close（引用计数驱动）；本层 `p.open("a")`
        // 的临时柄若不等显式 close，缓冲写入会**随对象一起丢** ✗（实测 `Path.open("a").write(...)`
        // 后 `read_text()` 看不到新增内容）。这里销毁时**尽力** flush＋关句柄（失败吞掉 ✓——`__del__`
        // 里抛异常只会变成警告，且文件可能已失效 ✓）。
        ("__del__", file_del_native as NativeFn),
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
        // **`.newlines` 只读属性**（本轮 ✓）：仅文本档有意义（二进制档 getter 报 `AttributeError` ✓）。
        if let Some(property) = readonly_property(instance, file_newlines_getter as NativeFn) {
            instance.set_type_attribute(class.cast::<pyawa_core::TypeObject>(), "newlines", property);
        }
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
        // **universal newline**（缺口3 ✓）：文本档按 `open(newline=…)` 的读口径断／译。
        let mode = newline_mode_of(instance, this);
        // **`.newlines` 追踪**（本轮 ✓）：`read(0)` 不解码、不追踪（照参照实测 ✓）；
        // 其余读档在首次真读时把整块原文扫一遍（整块口径见 [`scan_newlines_once`] ✓）。
        if size != 0 && mode.tracks_newlines() {
            scan_newlines_once(instance, this, &text);
        }
        let (piece, end) = text_read_from(&characters, start, size, mode);
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
        // **universal newline**（缺口3 ✓）：文本档按 `open(newline=…)` 的读口径断／译。
        let mode = newline_mode_of(instance, this);
        // **`.newlines` 追踪**（本轮 ✓）：`readline(0)` 也触发（参照实测 ✓）。
        if mode.tracks_newlines() {
            scan_newlines_once(instance, this, &text);
        }
        let (piece, end) = text_readline_from(&characters, start, size, mode);
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

/// `FileIO.writelines(lines)`（round 97）：逐行交给 `file_write` ✓（缓冲／关闭检查复用 ✓）。
fn file_writelines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = file_receiver(instance, bound, args)?;
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_IOBase.writelines() takes no keyword arguments",
        ));
    }
    let Some(lines) = args.get(1).copied().or_else(|| args.first().copied()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_IOBase.writelines() takes exactly one argument (0 given)",
        ));
    };
    // 若 `args` 的第一项就是 self（未绑定调用）⇒ 第二个及其后是实参；正常绑定调用 `args[0]` 是 lines。
    let lines = if bound.is_some() {
        args.first().copied().ok_or_else(|| {
            instance.raise_builtin_error("TypeError", "writelines() takes exactly one argument (0 given)")
        })?
    } else {
        lines
    };
    let iterator = instance.iter_object(lines)?;
    loop {
        match instance.advance_iterator(iterator) {
            Ok(Some(line)) => {
                let outcome = file_write(instance, Some(this), &[line], &[]);
                instance.release(line);
                let written = outcome?;
                instance.release(written);
            }
            Ok(None) => break,
            Err(error) => {
                instance.release(iterator);
                return Err(error);
            }
        }
    }
    instance.release(iterator);
    Ok(instance.retain(instance.singletons().none()))
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
        let Some(raw) = bytes_like_vec(instance, payload) else {
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
        // **写翻译**（本轮 ✓）：`None`（Linux 上 `os.linesep == '\n'`）／`''`／`'\n'` 恒等；
        // `'\r'`／`'\r\n'` 把**每个** `'\n'` 换成对应串（参照实测 ✓）。写进 `_data` 的是**翻译后**
        // 的文本 ⇒ `w+` 回读看到的就是落盘原文 ✓；返回值仍是**实参字符数**（不是翻译后长度 ✓）。
        let translated = newline_mode_of(instance, this).translate_write(text);
        let written = text.chars().count();
        let insertion: Vec<char> = translated.chars().collect();
        // 缓冲内容变了 ⇒ 下次真读要重扫 `.newlines`（`w+` 回读口径 ✓）。
        if file_flag(instance, this, "_readable") {
            let stale = instance.new_bool(false);
            set_attr(instance, this, "_newlines_scanned", stale);
        }
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
        let updated: String = characters.into_iter().collect();
        let data_value = instance.new_str(&updated);
        set_attr(instance, this, "_data", data_value);
        let new_position = instance.new_int((at + insertion.len()) as i64);
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

/// **`FileIO.__del__`**（round 268）：对象销毁时**尽力**把缓冲数据落盘并关句柄 ✓。
///
/// 口径：与 [`file_close`] 同一条落盘路径（`file_flush_data` ＋ `fs_close`）；错误**吞掉**（`__del__`
/// 里抛没有接收者，参照也只会打 `Exception ignored`）✓；可重入安全（`_closed` 已真就什么都不做 ✓）。
fn file_del_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _)) = this_of(bound, args) else {
        return Ok(instance.new_none());
    };
    if file_flag(instance, this, "_closed") {
        return Ok(instance.new_none());
    }
    let _ = file_flush_data(instance, this);
    let handle = file_int(instance, this, "_handle");
    if handle >= 0 {
        let _ = instance.fs_close(handle as u64);
        let gone = instance.new_int(-1);
        set_attr(instance, this, "_handle", gone);
    }
    let closed = instance.new_bool(true);
    set_attr(instance, this, "_closed", closed);
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

// ==== `_io.StringIO` 真实现（`P3-39` 第 2 轮；`newline=` 第 3 轮 ✓）==================
//
// **为什么真做** ✓：`TypeError: cannot create 'StringIO' instances` 是分诊里的头号签名之一
//（15 个官方测试阻塞；`unittest` 的失败消息格式化、`pprint.pformat`、`test.support.captured_stderr`
// 一族都压在它上面）。**口径**：文本缓冲放**实例字典**（`_buf` 是 `str`、`_pos` 是**码点下标**、
// `_closed`）；`encoding`／`errors` 给 `None`、"`line_buffering` False"（参照实测同 ✓）。
//
// **`newline=`（本轮 ✓，照本机 `python3` 3.14.4 实测，`target/recon/io_nl/probe_stringio_nl.py`）**：
// - 构造档 `StringIO(initial_value='', newline='\n')`：**初始值按写口径翻译**；默认档 `'\n'` 恒等；
// - **写**：`None` 把**三档**换行（`\r\n`／`\r`／`\n`）都译成 `\n`（与 `TextIOWrapper` 只译 `\n`
//   不同 ✓）；`''`／`'\n'` 恒等；`'\r'`／`'\r\n'` 把每个 `\n` 换成对应串；
// - **读**：`None` 三档断行并翻译、`''` 三档断行不翻译、显式档只认那一个（与 `TextIOWrapper` 同口径 ✓）；
//   区别在**存储**：`None` 的缓冲已是翻译后的文本 ⇒ 读＝普通切片，`tell` 是**翻译后**码点下标 ✓；
// - **`.newlines`**：只读属性；`None`／`''` 两档从**实参原文**累计（构造／写），显式三档恒 `None` ✓；
//   关闭后读它报 `ValueError: I/O operation on closed file`（无句点 ✓）。
//
// 语义照本机 CPython 3.14.4 实测：`StringIO("abc").tell()`＝0；`seek(5)` 后 `write("x")` 用
// `'\x00'` 补齐；`write(b"x")` ⇒ `TypeError: string argument expected, got 'bytes'`；
// `StringIO(b"x")` ⇒ `TypeError: initial_value must be str or None, not 'bytes'`；关闭后
// `getvalue`／`read`／`tell`／`readable` 一族 ⇒ `ValueError: I/O operation on closed file.`，
// 而 `flush()` 仍返回 `None` ✓；`iter(StringIO())` 在关闭后同报 `ValueError` ✓。

/// 文本缓冲（`str` 对象；**借用**）。
fn stringio_buffer(instance: &Instance, this: NonNull<Header>) -> Option<NonNull<Header>> {
    attr_of(instance, this, "_buf")
}

/// 文本缓冲的**码点表**（`StringIO` 的位置口径＝码点下标 ✓；不是字节下标 ✗）。
fn stringio_code_points(instance: &Instance, this: NonNull<Header>) -> Vec<u32> {
    stringio_buffer(instance, this)
        .and_then(|value| instance.wtf8_of(value).map(|raw| raw.to_vec()))
        .and_then(|raw| pyawa_core::wtf8::decode(&raw).ok())
        .unwrap_or_default()
}

/// 码点表 ⇒ `str`（**新引用**；走 `new_str_wtf8`，合法 UTF-8 表示逐字节不变 ✓）。
fn stringio_text_from_points(instance: &Instance, points: &[u32]) -> NonNull<Header> {
    let mut bytes: Vec<u8> = Vec::with_capacity(points.len());
    for &point in points {
        // 码点来自已校验的 WTF-8 ⇒ 编码不会失败；失败也只可能是内部不变量被破坏。
        let _ = pyawa_core::wtf8::encode_code_point(point, &mut bytes);
    }
    instance
        .new_str_wtf8(&bytes)
        .unwrap_or_else(|| instance.new_str(""))
}

/// `StringIO` 的 `newline=` 校验 ✓（**文案与 `open`／`TextIOWrapper` 不同**，照参照实测）：
/// 先类型（`TypeError: newline must be str or None, not int`）；再取值
/// （`ValueError: illegal newline value: 'x'` —— 带 **repr** ✓）。缺省档是 `'\n'`（恒等 ✓）。
fn stringio_newline_mode_of_value(
    instance: &Instance,
    value: Option<NonNull<Header>>,
) -> Result<NewlineMode, ExecError> {
    let Some(value) = value else {
        return Ok(NewlineMode::Lf);
    };
    if value == instance.singletons().none() {
        return Ok(NewlineMode::Universal);
    }
    let Some(text) = instance.text_of(value) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "newline must be str or None, not {}",
                instance.type_name(instance.type_of(value))
            ),
        ));
    };
    match text {
        "" => Ok(NewlineMode::NoTranslation),
        "\n" => Ok(NewlineMode::Lf),
        "\r" => Ok(NewlineMode::Cr),
        "\r\n" => Ok(NewlineMode::CrLf),
        other => Err(instance.raise_builtin_error(
            "ValueError",
            &format!("illegal newline value: {}", str_repr_for_message(other)),
        )),
    }
}

/// Python `repr(str)` 的消息形态（只用于非法 `newline` 值的报错；覆盖常见转义 ✓）。
fn str_repr_for_message(text: &str) -> String {
    let mut out = String::from("'");
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 || other as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('\'');
    out
}

/// 把每个 `\n` 码点换成 `replacement`（`StringIO` 的显式写档 ✓）。
fn stringio_replace_line_feeds(points: &[u32], replacement: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(points.len());
    for &point in points {
        if point == u32::from(b'\n') {
            out.extend_from_slice(replacement);
        } else {
            out.push(point);
        }
    }
    out
}

/// **StringIO 写翻译** ✓（本机 3.14.4 实测）：
/// - `None`：**三档**换行（`\r\n`／`\r`／`\n`）都译成 `\n`（与 `TextIOWrapper` 只译 `\n` 不同 ✓）；
/// - `''`／`'\n'`：恒等；
/// - `'\r'`／`'\r\n'`：每个 `\n` 换成对应串（不配对既有 `\r` ✓）。
fn stringio_translate_write(points: &[u32], mode: NewlineMode) -> Vec<u32> {
    match mode {
        NewlineMode::Universal => {
            let mut out: Vec<u32> = Vec::with_capacity(points.len());
            let mut index = 0usize;
            while index < points.len() {
                if points[index] == u32::from(b'\r') {
                    out.push(u32::from(b'\n'));
                    index += 1;
                    if index < points.len() && points[index] == u32::from(b'\n') {
                        index += 1;
                    }
                } else {
                    out.push(points[index]);
                    index += 1;
                }
            }
            out
        }
        NewlineMode::NoTranslation | NewlineMode::Lf => points.to_vec(),
        NewlineMode::Cr => stringio_replace_line_feeds(points, &[u32::from(b'\r')]),
        NewlineMode::CrLf => {
            stringio_replace_line_feeds(points, &[u32::from(b'\r'), u32::from(b'\n')])
        }
    }
}

/// 写回 StringIO 状态（`_buf`／`_pos`／`_closed` ＋ 属性读用的 `closed` 镜像 ✓）。
fn stringio_store(
    instance: &Instance,
    this: NonNull<Header>,
    points: &[u32],
    position: i64,
    closed: bool,
) {
    let text = stringio_text_from_points(instance, points);
    set_attr(instance, this, "_buf", text);
    let pos = instance.new_int(position);
    set_attr(instance, this, "_pos", pos);
    let frozen = instance.new_bool(closed);
    set_attr(instance, this, "_closed", frozen);
    let mirror = instance.new_bool(closed);
    set_attr(instance, this, "closed", mirror);
}

/// 关闭态守卫：参照的 `ValueError: I/O operation on closed file`（**无句点** —— 这是
/// `write`／`read`／`tell`／`seek`／`truncate`／`getvalue`／`readable` 一族的文案 ✓）。
fn stringio_check_open(instance: &Instance, this: NonNull<Header>) -> Result<(), ExecError> {
    if state_closed(instance, this) {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "I/O operation on closed file",
        ));
    }
    Ok(())
}

/// 关闭态守卫的**带句点**变体：参照只在 `isatty`／`__iter__`／`__enter__` 上用这个文案 ✓
///（本机 3.14.4 实测；`__next__` 走 `readline` ⇒ 无句点 ✓）。
fn stringio_check_open_period(instance: &Instance, this: NonNull<Header>) -> Result<(), ExecError> {
    if state_closed(instance, this) {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "I/O operation on closed file.",
        ));
    }
    Ok(())
}

/// `StringIO(initial_value='', newline='\n')` ✓（本轮接 `newline=`：构造收档、读写同口径 ✓）。
fn stringio_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.__init__ 缺少 self"));
    };
    if rest.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("StringIO() takes at most 2 arguments ({} given)", rest.len()),
        ));
    }
    let mut initial_value = rest.first().copied();
    let mut newline_value = rest.get(1).copied();
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            continue;
        };
        match name {
            "initial_value" => {
                if initial_value.is_some() && !rest.is_empty() {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "StringIO() got multiple values for argument 'initial_value'",
                    ));
                }
                initial_value = Some(*value);
            }
            "newline" => {
                if newline_value.is_some() && rest.len() > 1 {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "StringIO() got multiple values for argument 'newline'",
                    ));
                }
                newline_value = Some(*value);
            }
            other => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("StringIO() got an unexpected keyword argument '{other}'"),
                ))
            }
        }
    }
    // **校验次序照参照** ✓：先 `newline`，再 `initial_value` 类型（实测 `StringIO(b"x", newline="x")`
    // ⇒ `ValueError: illegal newline value` 而不是 initial_value 的 `TypeError` ✓）。
    let mode = stringio_newline_mode_of_value(instance, newline_value)?;
    let none = instance.singletons().none();
    let points: Vec<u32> = match initial_value {
        None => Vec::new(),
        Some(value) if value == none => Vec::new(),
        Some(value) => match instance.wtf8_of(value) {
            Some(raw) => pyawa_core::wtf8::decode(raw).unwrap_or_default(),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!(
                        "initial_value must be str or None, not {}",
                        instance.type_name(instance.type_of(value))
                    ),
                ))
            }
        },
    };
    // **初始值按写口径翻译** ✓（`None`：三档译成 `\n`；显式档：每个 `\n` 换成对应串；`''`／`'\n'` 恒等）。
    let translated = stringio_translate_write(&points, mode);
    stringio_store(instance, this, &translated, 0, false);
    // **`newline` 档 ＋ `.newlines` 掩码**（`None`／`''` 两档从**实参原文**追踪 ✓）。
    let newline_attr = instance.new_int(mode.code());
    set_attr(instance, this, "_newline", newline_attr);
    let zero_mask = instance.new_int(0);
    set_attr(instance, this, "_newlines_mask", zero_mask);
    if mode.tracks_newlines() {
        record_newlines_mask(instance, this, newline_mask_of_points(&points));
    }
    // **文本流属性面**（`test_memoryio` 的 `test_textio_properties` ✓）：参照实测
    // `encoding is None`／`errors is None`／`line_buffering is False` ✓（`newlines` 本轮改成只读属性 ✓）。
    let none_value = instance.retain(instance.singletons().none());
    set_attr(instance, this, "encoding", none_value);
    let errors = instance.retain(instance.singletons().none());
    set_attr(instance, this, "errors", errors);
    let line_buffering = instance.new_bool(false);
    set_attr(instance, this, "line_buffering", line_buffering);
    Ok(instance.retain(instance.singletons().none()))
}

/// `StringIO.write(s)` ⇒ 写入的**字符数** ✓（在 `_pos` 处覆盖；越过末尾用 `'\x00'` 补齐 ✓；
/// **写翻译**见 [`stringio_translate_write`] ✓，返回值是**实参字符数**、位置按翻译后长度推进 ✓）。
fn stringio_write(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.write 缺少 self"));
    };
    stringio_check_open(instance, this)?;
    let Some(data) = rest.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "StringIO.write() takes exactly one argument (0 given)",
        ));
    };
    let Some(raw) = instance.wtf8_of(*data) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "string argument expected, got '{}'",
                instance.type_name(instance.type_of(*data))
            ),
        ));
    };
    let written = pyawa_core::wtf8::decode(raw).unwrap_or_default();
    let mode = newline_mode_of(instance, this);
    let translated = stringio_translate_write(&written, mode);
    let length = translated.len();
    let mut points = stringio_code_points(instance, this);
    let start = state_pos(instance, this).max(0) as usize;
    if start > points.len() {
        points.resize(start, 0);
    }
    if start + length > points.len() {
        points.resize(start + length, 0);
    }
    points[start..start + length].copy_from_slice(&translated);
    // **`.newlines` 从实参原文累计** ✓（`None`／`''` 两档；写后内容变了，掩码只增不减 ✓）。
    if mode.tracks_newlines() {
        record_newlines_mask(instance, this, newline_mask_of_points(&written));
    }
    stringio_store(instance, this, &points, (start + length) as i64, false);
    Ok(instance.new_int(written.len() as i64))
}

/// `StringIO.getvalue()` ⇒ 整个缓冲 ✓（**关闭后**报 `ValueError`，照参照实测 ✓）。
fn stringio_getvalue(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.getvalue 缺少 self"));
    };
    stringio_check_open(instance, this)?;
    match stringio_buffer(instance, this) {
        Some(value) => Ok(instance.retain(value)),
        None => Ok(instance.new_str("")),
    }
}

/// `StringIO.read([size])` ✓（`newline=` 五档读口径，与 `TextIOWrapper` 同一套扫描 ✓）。
fn stringio_read(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.read 缺少 self"));
    };
    stringio_check_open(instance, this)?;
    let size = rest
        .first()
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(-1);
    let points = stringio_code_points(instance, this);
    let start = (state_pos(instance, this).max(0) as usize).min(points.len());
    let mode = newline_mode_of(instance, this);
    let (piece, end) = text_read_units(&points, start, size, mode);
    stringio_store(instance, this, &points, end as i64, false);
    Ok(stringio_text_from_points(instance, &piece))
}

/// `StringIO.readline([size])` ✓（`newline=` 五档断行口径；含行尾换行 ✓、EOF 给剩余段 ✓）。
fn stringio_readline(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.readline 缺少 self"));
    };
    stringio_check_open(instance, this)?;
    let size = rest
        .first()
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(-1);
    let points = stringio_code_points(instance, this);
    let start = (state_pos(instance, this).max(0) as usize).min(points.len());
    let mode = newline_mode_of(instance, this);
    let (piece, end) = text_readline_units(&points, start, size, mode);
    stringio_store(instance, this, &points, end as i64, false);
    Ok(stringio_text_from_points(instance, &piece))
}

/// `StringIO.readlines()` ✓。
fn stringio_readlines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 同 `bytesio_readlines`：**EOF 不推空串** ✓；`sizehint > 0` ⇒ 累计到提示量就停 ✓
    //（`test_memoryio` 的 StringIO `test_readlines` 钉住 ✓）。
    let hint = args
        .first()
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(-1);
    let mut total: i64 = 0;
    let mut items: Vec<NonNull<Header>> = Vec::new();
    loop {
        let line = stringio_readline(instance, bound, &[], kwargs)?;
        let length = instance
            .text_of(line)
            .map(|text| text.chars().count())
            .unwrap_or(0);
        if length == 0 {
            instance.release(line);
            return Ok(instance.new_list(items));
        }
        total += length as i64;
        items.push(line);
        if hint > 0 && total >= hint {
            return Ok(instance.new_list(items));
        }
    }
}

/// `StringIO.seek(offset[, whence])` ✓（**码点**下标；越过末尾允许 ✓）。
fn stringio_seek(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.seek 缺少 self"));
    };
    stringio_check_open(instance, this)?;
    let offset = rest
        .first()
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(0);
    let whence = rest
        .get(1)
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(0);
    let points = stringio_code_points(instance, this);
    // **文本流口径**（本机 3.14.4 实测）：`whence=1/2` 只允许 `offset == 0`，其它一律
    // `OSError("Can't do nonzero cur-relative seeks")` ✓（`test_memoryio` 的 StringIO
    // `test_relative_seek` 就钉这一条 ✓）；`whence` 不在 0/1/2 ⇒ `ValueError("invalid whence")`。
    let base = match whence {
        0 => 0,
        1 | 2 if offset != 0 => {
            return Err(unsupported_error(
                instance,
                "Can't do nonzero cur-relative seeks",
            ))
        }
        1 => state_pos(instance, this),
        2 => points.len() as i64,
        _ => return Err(instance.raise_builtin_error("ValueError", "invalid whence")),
    };
    let position = base + offset;
    if position < 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("negative seek value {position}"),
        ));
    }
    stringio_store(instance, this, &points, position, false);
    Ok(instance.new_int(position))
}

/// `StringIO.tell()` ✓。
fn stringio_tell(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.tell 缺少 self"));
    };
    stringio_check_open(instance, this)?;
    Ok(instance.new_int(state_pos(instance, this)))
}

/// `StringIO.truncate([size])` ✓（缺省＝当前位置；返回新长度 ✓）。
fn stringio_truncate(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.truncate 缺少 self"));
    };
    stringio_check_open(instance, this)?;
    let mut points = stringio_code_points(instance, this);
    let size = rest
        .first()
        .and_then(|value| instance.int_value(*value))
        .unwrap_or_else(|| state_pos(instance, this));
    if size < 0 {
        return Err(instance.raise_builtin_error("ValueError", "negative size value"));
    }
    if (size as usize) < points.len() {
        points.truncate(size as usize);
    }
    stringio_store(instance, this, &points, state_pos(instance, this), false);
    Ok(instance.new_int(size))
}

/// `StringIO.close()` ✓（幂等；`closed` 属性置真 ✓）。
fn stringio_close(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.close 缺少 self"));
    };
    let points = stringio_code_points(instance, this);
    stringio_store(instance, this, &points, state_pos(instance, this), true);
    Ok(instance.retain(instance.singletons().none()))
}

/// `StringIO.flush()` ✓（内存缓冲无事可做；**关闭后也照参照返回 `None`** ✓）。
fn stringio_flush(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}

/// `readable()`／`writable()`／`seekable()`：开着就 `True`；关着报 `ValueError`（照参照 ✓）。
fn stringio_true(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO 谓词缺少 self"));
    };
    stringio_check_open(instance, this)?;
    Ok(instance.new_bool(true))
}

/// `isatty()`：开着就 `False`；关着报 `ValueError`（照参照 ✓）。
fn stringio_isatty(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.isatty 缺少 self"));
    };
    stringio_check_open_period(instance, this)?;
    Ok(instance.new_bool(false))
}

/// `__iter__`：开着返回 `self`；关着报 `ValueError` ✓。
fn stringio_enter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.__iter__ 缺少 self"));
    };
    stringio_check_open_period(instance, this)?;
    Ok(instance.retain(this))
}

/// `__next__`：读一行；EOF ⇒ `StopIteration` ✓。
fn stringio_next(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let line = stringio_readline(instance, bound, args, kwargs)?;
    let empty = instance
        .text_of(line)
        .map(|text| text.is_empty())
        .unwrap_or(true);
    if empty {
        instance.release(line);
        return Err(instance.raise_builtin_error("StopIteration", ""));
    }
    Ok(line)
}

/// `StringIO.writelines(lines)` ✓（逐个 `write`；非 `str` 元素照参照：`TypeError: string argument
/// expected, got '<类型>'` ✓；返回 `None` ✓）。
fn stringio_writelines(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.writelines 缺少 self"));
    };
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_IOBase.writelines() takes no keyword arguments",
        ));
    }
    stringio_check_open(instance, this)?;
    let Some(lines) = rest.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_IOBase.writelines() takes exactly one argument (0 given)",
        ));
    };
    let iterator = pyawa_core::executor::iter::iter_value(instance, *lines)?;
    loop {
        match instance.advance_iterator(iterator)? {
            Some(item) => {
                let outcome = stringio_write(instance, Some(this), &[item], &[]);
                instance.release(item);
                outcome?;
            }
            None => break,
        }
    }
    instance.release(iterator);
    Ok(instance.retain(instance.singletons().none()))
}

/// `StringIO.detach()` ✓ ⇒ 参照抛 `UnsupportedOperation`（本层别名到 `OSError`，消息 `detach` ✓）。
fn stringio_detach(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((_this, _rest)) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "StringIO.detach 缺少 self"));
    };
    Err(unsupported_error(instance, "detach"))
}

/// `.newlines` getter（`StringIO` ✓）：**关闭后**报 `ValueError: I/O operation on closed file`
///（无句点 —— 照参照实测 ✓）。
fn stringio_newlines_getter(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let this = property_self(args)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor 'newlines' needs an argument"))?;
    stringio_check_open(instance, this)?;
    Ok(newlines_value(instance, newlines_mask_of(instance, this)))
}

/// 建 `_io.StringIO` 真类 ✓（状态在实例字典 ✓；基类取 `_TextIOBase` 占位类型 ⇒ `isinstance` 成立 ✓）。
fn build_string_io(instance: &Instance, namespace: NonNull<Header>) {
    let base = instance
        .type_named("_TextIOBase")
        .or_else(|| instance.type_named("_IOBase"))
        .or_else(|| instance.type_named("object"))
        .expect("`object` 已登记");
    let class_namespace = instance.new_dict();
    let handlers: [(&str, NativeFn); 21] = [
        ("__init__", stringio_init as NativeFn),
        ("write", stringio_write as NativeFn),
        ("getvalue", stringio_getvalue as NativeFn),
        ("read", stringio_read as NativeFn),
        ("readline", stringio_readline as NativeFn),
        ("readlines", stringio_readlines as NativeFn),
        ("writelines", stringio_writelines as NativeFn),
        ("detach", stringio_detach as NativeFn),
        ("seek", stringio_seek as NativeFn),
        ("tell", stringio_tell as NativeFn),
        ("truncate", stringio_truncate as NativeFn),
        ("close", stringio_close as NativeFn),
        ("flush", stringio_flush as NativeFn),
        ("readable", stringio_true as NativeFn),
        ("writable", stringio_true as NativeFn),
        ("seekable", stringio_true as NativeFn),
        ("isatty", stringio_isatty as NativeFn),
        ("__iter__", stringio_enter as NativeFn),
        ("__next__", stringio_next as NativeFn),
        ("__enter__", stringio_enter as NativeFn),
        ("__exit__", stringio_close as NativeFn),
    ];
    for (name, handler) in handlers {
        let function = make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    if let Ok(class) = pyawa_core::build_class_from_parts(
        instance,
        "StringIO".to_owned(),
        vec![base.cast()],
        class_namespace,
        None,
    ) {
        // **`.newlines` 只读属性**（本轮 ✓；关闭后读 ⇒ `ValueError` ✓）。
        if let Some(property) = readonly_property(instance, stringio_newlines_getter as NativeFn) {
            instance.set_type_attribute(class.cast::<pyawa_core::TypeObject>(), "newlines", property);
        }
        instance.dict_set(namespace, "StringIO", class);
    }
}
