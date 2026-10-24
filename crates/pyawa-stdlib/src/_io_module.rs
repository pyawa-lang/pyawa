//! `_io` 模块（**最小面**）：`sys.stdout`／`sys.stderr` 那类**文本流**的形状与
//! `write`／`writeln`／`flush` ✓。
//!
//! 分工（用户钉死的边界）：`print` ⇒ `sys.stdout` ⇒ **本模块** ⇒ `fs` 域的 `write` 槽 ✓。
//! 本模块**不碰平台**（`CX-4`）：字节交给 `Instance::fs_vtable()`（`CP-12` 的形状 ✓），
//! 真正的系统调用在 `pyawa-runtime` 的平台集中点 ✓。
//!
//! **两套入口、一处落点** ✓：
//! - `print` 走内建**快路**（`builtins` 名字空间里的 `__stdout__` / `__stdout_handle__` ✓，
//!   见 `builtins_module::print_native` ✓）——本模块**不碰**它 ✓；
//! - `sys.stdout.write(...)` 走**对象属性通道** ✓：每个流对象**各挂一组**原生函数，
//!   句柄**烤在函数里** ✓（`stdout`／`stderr` 各一组 ✓），对象字典里**不存句柄** ✗（见 [`make_stream`]）。

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{AttributeObject, ExecError, Header, Instance, NativeFn};

/// 模块名（`_io`）。
pub const NAME: &str = "_io";

/// `__doc__`（一句话；完整面待 `P3-` 立项）。
pub const DOC: &str = "The io module provides the Python interfaces to stream handling.";

/// **标准输出句柄**（提供者约定：`1` ⇒ `stdout`、`2` ⇒ `stderr`；见 `PosixFs` 的文档 ✓）。
pub const STDOUT_HANDLE: u64 = 1;
/// 标准错误句柄。
pub const STDERR_HANDLE: u64 = 2;

/// 文本流类型名（`_io.TextIOWrapper` 的**最小面**：类型归属在这里 ✓）。
pub const TEXT_WRAPPER: &str = "_io.TextIOWrapper";

/// 把**已经编码好**的字节经 `fs` 域写出去（平台在提供者那边 ✓，本层不碰 ✓ `CX-4`）。
pub fn write_bytes(
    instance: &Instance,
    handle: u64,
    bytes: &[u8],
) -> Result<usize, ExecError> {
    instance
        .fs_write(handle, bytes)
        .map_err(|error| ExecError::Unsupported {
            opcode: 0,
            what: match error {
                pyawa_core::CapabilityCallError::NotRegistered => {
                    "`fs` 域未注册：`sys.stdout` 没有输出落点（`CP-2`）"
                }
                pyawa_core::CapabilityCallError::NotImplemented => {
                    "`fs` 域的 `write` 槽未实现（`CP-5`）"
                }
                pyawa_core::CapabilityCallError::Machine(_) => {
                    "`fs` 域写标准流遇到机器错误（`CP-3`）"
                }
            },
        })
}

// ==== 文本流对象的方法（task-10 ✓）：`write`／`writeln`／`flush` ================================
//
// **动因** ✓：官方 `Lib/unittest/runner.py` 的 `TextTestResult` 直接 `self.stream.write(…)`／
// `.flush()`（`_WritelnDecorator.writeln` 也是两次 `write` ✓）⇒ 流对象缺 `write` 时
// `unittest.main()` 当场 `AttributeError: '_io.TextIOWrapper' object has no attribute 'write'` ✗
// —— 这是 `P3-32` 验收②（最小 `unittest.main()` 退出码 `0`）的最后一格 ✓。
//
// **口径（如实 ✗）**：
// - 编码**固定 `utf-8`** ✓（参照的标准流编码随 locale／`PYTHONUTF8` 走；本层只接最小面 ✓）；
// - **不缓冲** ✓：一次 `write` 就是一次 `fs` 落点 ⇒ `flush` **真的无事可做** ✓（返回 `None` ✓）；
// - `write` 只收 `str` ✓（`bytes` 等按参照报 `TypeError` ✓）；返回**字符数** ✓（参照口径 ✓）；
// - `writeln` 是**本层的便利方法** ✓（参照的文本流没有它；`unittest` 的那份在 Python 层
//   `_WritelnDecorator` 里 ✓）——`writeln(str="")` ⇒ `write(str)` ＋ `"\n"` ✓，返回 `None` ✓。

/// 文本流的编码（**如实** ✗：只接 `utf-8` 一档，见上面的"口径"）。
const STREAM_ENCODING: &str = "utf-8";

/// 把一段文本编码后经 `fs` 域写出去，返回**字符数** ✓（参照 `TextIOWrapper.write` 的返回口径 ✓）。
fn write_text(instance: &Instance, handle: u64, text: &str) -> Result<usize, ExecError> {
    let characters = text.chars().count();
    let encoded = instance.encode_with_encoding(text, STREAM_ENCODING)?;
    let bytes = instance
        .bytes_value(encoded)
        .map(|raw| raw.to_vec())
        .unwrap_or_default();
    instance.release(encoded);
    // **不缓冲** ✓：`print` 那条快路也是直接落一次 `fs` ✓（一处落点 ✓）。
    write_bytes(instance, handle, &bytes)?;
    Ok(characters)
}

/// 非 `str` 实参的 `TypeError` ✓（文案照参照：`write() argument must be str, not int` ✓）。
fn not_str_error(instance: &Instance, method: &str, value: NonNull<Header>) -> ExecError {
    let type_name = instance.type_name(instance.type_of(value));
    instance.raise_builtin_error(
        "TypeError",
        &format!("{method}() argument must be str, not {type_name}"),
    )
}

/// 取 `write` 的那个**唯一文本实参** ✓（多了少了都按参照报 `TypeError` ✓）。
fn one_text_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    method: &str,
) -> Result<String, ExecError> {
    match args {
        [value] => match instance.text_of(*value) {
            Some(text) => Ok(text.to_owned()),
            None => Err(not_str_error(instance, method, *value)),
        },
        [] => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{method}() takes exactly one argument (0 given)"),
        )),
        _ => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{method}() takes exactly one argument ({} given)", args.len()),
        )),
    }
}

/// `write(str)` 的公共实现 ✓：`handle` 由**调用者烤进去** ✓（逐对象绑定口径 ✓）。
fn stream_write(
    instance: &Instance,
    handle: u64,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    let text = one_text_argument(instance, args, "write")?;
    let written = write_text(instance, handle, &text)?;
    Ok(instance.new_int(written as i64))
}

/// `writeln(str="")` 的公共实现 ✓：文本 ＋ 换行**一次**写出去 ✓（次序不拆 ✓），返回 `None` ✓。
fn stream_writeln(
    instance: &Instance,
    handle: u64,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("writeln() takes at most one argument ({} given)", args.len()),
        ));
    }
    let mut text = String::new();
    if let Some(value) = args.first() {
        // `None` 与"不给实参"同义 ✓（参照的 `print(end=None)` 一类写法口径 ✓）。
        if *value != instance.singletons().none() {
            match instance.text_of(*value) {
                Some(given) => text.push_str(given),
                None => return Err(not_str_error(instance, "writeln", *value)),
            }
        }
    }
    text.push('\n');
    write_text(instance, handle, &text)?;
    Ok(instance.retain(instance.singletons().none()))
}

/// `flush()` ✓：本层**没有缓冲**（每次 `write` 都已经经 `fs` 出去 ✓）⇒ 无事可做 ✓ ——
/// **如实**返回 `None` ✓，不是"假装刷新" ✗（确实没有东西可刷 ✓）。
fn stream_flush(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}

// **句柄烤进函数里** ✓：`NativeFn` 只能收 `(实例, self 槽, 实参, 关键字)`，没有"闭包"格 ✓
// ⇒ `stdout`／`stderr` 各写一个**只有 handle 不同**的薄壳 ✓（`write` 与 `writeln` 各两个 ✓）。

fn stdout_write(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    stream_write(instance, STDOUT_HANDLE, args)
}

fn stderr_write(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    stream_write(instance, STDERR_HANDLE, args)
}

fn stdout_writeln(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    stream_writeln(instance, STDOUT_HANDLE, args)
}

fn stderr_writeln(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    stream_writeln(instance, STDERR_HANDLE, args)
}

/// 造一个文本流对象（`sys.stdout` 的**身份**）：类型归属 `_io.TextIOWrapper` ✓，
/// 并给**这个对象**挂上 `write`／`writeln`／`flush` ✓。
///
/// **句柄不放在对象里** ✗（第 93 轮实测：把 `__handle__` 塞进对象字典会崩 ✗）——本层的做法是
/// 把句柄**烤进原生函数** ✓（[`stdout_write`]／[`stderr_write`] 一族 ✓），对象字典只放可调用 ✓。
/// 句柄另有 `print` 快路的那一份（组合根装的 `__stdout_handle__` ✓），两处**同源** ✓。
///
/// **逐对象**挂方法 ✓：`stdout` 与 `stderr` 是**同一种类型**（类型只登记一次 ✓），但**句柄不同**
/// ⇒ 方法必须按对象分 ✓ —— 塞进共享类型字典会把两个句柄混成一个 ✗。
pub fn make_stream(instance: &Instance, handle: u64) -> NonNull<Header> {
    // **类型只登记一次**：`stdout` 与 `stderr` 是同一种文本流 ✓。重复 `new_attribute_type`
    // 用同名会写坏类型注册表 ✗（第 94 轮实测：子进程退出时 `tcache_thread_shutdown():
    // unaligned tcache chunk detected` ✓ —— 堆在退出时被查出损坏 ✓）。
    let stream_type = match instance.type_named(TEXT_WRAPPER) {
        Some(existing) => existing,
        None => instance.new_attribute_type(TEXT_WRAPPER),
    };
    let (write, writeln): (NativeFn, NativeFn) = match handle {
        STDERR_HANDLE => (stderr_write as NativeFn, stderr_writeln as NativeFn),
        // 目前只有 `1`／`2` 两个调用点（`sys_module::build` ✓）；其余句柄按 `stdout` 口径处理 ✓。
        _ => (stdout_write as NativeFn, stdout_writeln as NativeFn),
    };
    let attributes = instance.new_dict();
    for (name, handler) in [
        ("write", write),
        ("writeln", writeln),
        ("flush", stream_flush as NativeFn),
    ] {
        let native = crate::builtins_module::make_native(instance, name, handler);
        // `dict_set` 是**借用**语义 ✓（它自己 `incref` 一份 ✓）⇒ 这里新造的那份要归还 ✓。
        instance.dict_set(attributes, name, native);
        instance.release(native);
    }
    let object = instance.alloc(AttributeObject::new(
        stream_type,
        RefCell::new(Some(attributes)),
    ));
    object.into_raw().cast::<Header>()
}
