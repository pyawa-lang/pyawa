//! `select` 模块（**task-15 辅助格** ✓）：`import subprocess` 的模块级 `import select`，
//! 以及 `Lib/selectors.py` 的 `SelectSelector` 回退路径（`_select = select.select` 在**类体**求值 ✓）。
//!
//! ## 为什么只有一小面（**刻意收窄** ✓）
//!
//! 参照本机 `dir(select)` 公共名 33 个（`EPOLL*` 17／`POLL*` 12／`PIPE_BUF`／`epoll`／`error`／
//! `poll`／`select`）。本轮**只导出** `PIPE_BUF`（真常量，`getattr(select, 'PIPE_BUF', 512)` 有它
//! ⇒ `subprocess._PIPE_BUF` 与参照同值）与 `select`（名字＋元数校验，调用如实报未接线）。
//!
//! **不导出** `poll`／`epoll`／`devpoll`／`kqueue`：`Lib/selectors.py` 用 `hasattr(select, 'poll')`
//! 一类守卫选实现 —— 导出外壳会让它去 `_can_use('epoll')` 里**真的构造** selector，
//! 而 `NotImplementedError` **不在** `_can_use` 的 `except (TypeError, AttributeError, OSError)` 里
//! ⇒ 反而会挡 `import selectors` ✗。收窄后它走 `SelectSelector` 回退（**如实偏差**：参照本机
//! `DefaultSelector` 是 `EpollSelector`；语义少接线，不是假装有 ✓）。
//!
//! ## 如实报错面（硬边界 ✓）
//!
//! 真 `select(2)`／`poll(2)`／`epoll_create(2)` 是能力域问题（`CM-8`）⇒ `select.select` 元数与
//! 关键字校验过后**调用即** `NotImplementedError`（消息含"未接线"）——不返回假 ready 列表。
//!
//! 本模块**不碰平台**（`CX-4` ⇒ `#![forbid(unsafe_code)]`）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, NativeFn};

/// 模块名（`select`）。
pub const NAME: &str = "select";

/// 模块 `__doc__`（**照参照逐字抄** ✓）。
pub const DOC: &str = "This module supports asynchronous I/O on multiple file descriptors.\n\n*** IMPORTANT NOTICE ***\nOn Windows, only sockets are supported; on Unix, all file descriptors.";

/// `PIPE_BUF`（参照本机实测 **4096** ✓；`subprocess.py:243` 的 `getattr(select, 'PIPE_BUF', 512)` 读它 ✓）。
const PIPE_BUF: i64 = 4096;

/// 建 `select` 的命名空间（**新引用**的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    instance.dict_set(namespace, "PIPE_BUF", instance.new_int(PIPE_BUF));
    let select = make_native(instance, "select", select_native);
    instance.dict_set(namespace, "select", select);
    let exports = instance.new_list(vec![
        instance.new_str("PIPE_BUF"),
        instance.new_str("select"),
    ]);
    instance.dict_set(namespace, "__all__", exports);
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    namespace
}

fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// `select.select(rlist, wlist, xlist, timeout=None)`：参照实测
/// `select expected at least 3 arguments, got N`／`select expected at most 4 arguments, got N`、
/// 关键字一律拒（`select.select() takes no keyword arguments`）⇒ 校验过后如实报未接线。
fn select_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "select.select() takes no keyword arguments",
        ));
    }
    if args.len() < 3 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("select expected at least 3 arguments, got {}", args.len()),
        ));
    }
    if args.len() > 4 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("select expected at most 4 arguments, got {}", args.len()),
        ));
    }
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`select.select`：真 select(2) 属能力域（CM-8）问题，本轮未接线",
    ))
}
