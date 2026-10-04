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
        let ty = instance.new_bare_type(name);
        if let Some(type_namespace) = instance.type_namespace(ty.cast()) {
            let doc = instance.new_str(&format!(
                "{name}：Pyawa 的 `_io` 占位类型（真 I/O 要走 `fs` 能力域）"
            ));
            instance.dict_set(type_namespace, "__doc__", doc);
        }
        instance.dict_set(namespace, name, instance.type_value(ty));
    }
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
