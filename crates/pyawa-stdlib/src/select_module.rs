//! `select` 模块（**task-15 辅助格**起；round 11 起 `select.select` **真接线** ✓）。
//!
//! ## 本轮落地（`T-CP-3` 的 Python 可见面）
//!
//! `select.select(rlist, wlist, xlist, timeout=None)` ⇒ `Instance::capability_select`
//! （`CP-31` 统一实例能力表：`fd` 是表下标，`fs` 域 fd 恒就绪、`net` 域 fd 走 `poll` 槽）：
//! - 返回**三个 `list`**（可读／可写／异常），元素是**调用方传进来的原对象**（`int` 或带
//!   `fileno()` 的对象 —— 与参照 `PyObject_AsFileDescriptor` 同口径 ✓）；
//! - `timeout=None` ⇒ 无限等待；`0` ⇒ 轮询；`>0` ⇒ 秒（`float`／`int`，内部换算成纳秒）；
//! - 错误形状照参照实测：负 fd ⇒ `ValueError: file descriptor cannot be a negative integer (N)`；
//!   `fd >= 1024` ⇒ `ValueError: filedescriptor out of range in select()`；
//!   非法元素 ⇒ `TypeError: argument must be an int, or have a fileno() method.`；
//!   非序列 ⇒ `TypeError: arguments 1-3 must be sequences`；
//!   负 timeout ⇒ `ValueError: timeout must be non-negative`；`NaN` ⇒ `ValueError: Invalid value NaN (not a number)`；
//!   机器错误（已关 fd）⇒ `OSError`（`CP-18` 的 `EBADF` 形状）。
//!
//! ## 可取消等待（`T-CP-10`，round 12）
//!
//! 等待经 `Instance::capability_select_cancellable`：**空 fd 列表 ＋ 正 timeout 真睡**（`clock.sleep_ns`
//! 分片，与参照同量级）；`net` fd 的 `poll` 也按片调用。执行中宿主 `pa_interrupt` ⇒ 调用返回"已取消"
//! ⇒ 本模块转成 `ExecError::Interrupted` ⇒ `pa_exec_*` 返回 `PA_ERR_INTERRUPT`（**既有中断口径**）。
//! **如实差异**：参照的 Ctrl-C 会投 Python `KeyboardInterrupt`，本层的 `pa_interrupt` 是宿主级中断，
//! 脚本里 `except KeyboardInterrupt` 捕不到（`T-CP-10` 的验收口径是"已取消返回、不悬挂、无后续副作用"）。
//!
//! ## 如实偏差（不假装 ✓）
//!
//! - `net` 的**无 timeout 阻塞槽位**（`recv` 等）仍不可取消（见 `instance/wait.rs` 的"尚未覆盖"）。
//! - **不导出** `poll`／`epoll`／`devpoll`／`kqueue`（`Lib/selectors.py` 的 `hasattr(select, 'poll')`
//!   守卫会去真构造 selector；`NotImplementedError` 不在 `_can_use` 的 `except` 里 ⇒ 反而会挡
//!   `import selectors` ✗）。收窄后它走 `SelectSelector` 回退（**如实偏差**：参照本机
//!   `DefaultSelector` 是 `EpollSelector`）。
//! - `select.error`（＝`OSError`）按参照登记 ✓。
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

/// 参照 `select(2)` 的 `FD_SETSIZE`（超过它的 fd 报 `ValueError`，照参照实测 ✓）。
const SELECT_FD_LIMIT: i64 = 1024;

/// 建 `select` 的命名空间（**新引用**的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    instance.dict_set(namespace, "PIPE_BUF", instance.new_int(PIPE_BUF));
    let select = make_native(instance, "select", select_native);
    instance.dict_set(namespace, "select", select);
    // 参照：`select.error is OSError` ✓。
    if let Some(oserror) = instance.type_named("OSError") {
        instance.dict_set(namespace, "error", instance.type_value(oserror));
    }
    let exports = instance.new_list(vec![
        instance.new_str("PIPE_BUF"),
        instance.new_str("select"),
        instance.new_str("error"),
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

/// `select.select(rlist, wlist, xlist, timeout=None)` —— 真接线（见模块文档 ✓）。
///
/// 元数与关键字校验沿用参照实测文案（`select expected at least 3 arguments, got N` 一类 ✓）。
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
    let timeout_ns = select_timeout_ns(instance, args.get(3).copied())?;

    // 三份列表：元素 → `fd`（同时保留**原对象**，返回时要原样交回 ✓）。
    let mut lists: [Vec<(i64, NonNull<Header>)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for index in 0..3 {
        match collect_fds(instance, args[index]) {
            Ok(entries) => lists[index] = entries,
            Err(error) => {
                // 已收下的元素是 `iterable_items` 给的**新引用** ⇒ 出错路径也要归还 ✓。
                for list in &lists {
                    release_entries(instance, list);
                }
                return Err(error);
            }
        }
    }

    // `fd` 范围校验照参照（放在 core 之前 ⇒ 与参照的 `ValueError` 形状一致 ✓）。
    let mut inputs: [Vec<i64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for index in 0..3 {
        for (fd, _) in &lists[index] {
            if let Err(error) = check_fd_range(instance, *fd) {
                for list in &lists {
                    release_entries(instance, list);
                }
                return Err(error);
            }
            inputs[index].push(*fd);
        }
    }

    // VM 侧入口（`fs` 恒就绪 ／ `net` 走 poll 槽；错误形状见 `capability_table.rs` ✓）。
    // **`T-CP-10`**：可取消版——分片等待，片间查实例中断；被中断 ⇒ 归还输入引用后转
    // `ExecError::Interrupted`（`PA_ERR_INTERRUPT`，CP-28 的"已取消"、无后续副作用）。
    let ready = match instance.capability_select_cancellable(
        &inputs[0],
        &inputs[1],
        &inputs[2],
        timeout_ns,
    ) {
        Ok(Some(ready)) => ready,
        Ok(None) => {
            for list in &lists {
                release_entries(instance, list);
            }
            return Err(pyawa_core::ExecError::Interrupted);
        }
        Err(error) => {
            for list in &lists {
                release_entries(instance, list);
            }
            return Err(crate::posix_module::capability_error(instance, error));
        }
    };
    let ready_lists = [&ready.0, &ready.1, &ready.2];

    // 按**输入序**回填原对象（参照就是按输入序生成三个列表 ✓；core 的 ready 列表去重、顺序不承诺）。
    let mut outputs: [Vec<NonNull<Header>>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for index in 0..3 {
        for (fd, object) in &lists[index] {
            if ready_lists[index].contains(fd) {
                // 输出列表要自己那份引用；输入列表那份随后统一归还 ✓。
                outputs[index].push(instance.retain(*object));
            }
        }
    }
    for list in &lists {
        release_entries(instance, list);
    }
    let [read, write, error] = outputs.map(|items| instance.new_list(items));
    Ok(instance.new_tuple(vec![read, write, error]))
}

/// 归还 `iterable_items` 给的那批**新引用**（`std::mem` 口径：每项一份 ✓）。
fn release_entries(instance: &Instance, entries: &[(i64, NonNull<Header>)]) {
    for (_, object) in entries {
        instance.release(*object);
    }
}

/// 把 `select` 的一个参数摊成 `(fd, 原对象)` 列表。
///
/// 元素口径照参照 `PyObject_AsFileDescriptor`：`int` ⇒ 直接用；否则**有 `fileno()` 就调它**；
/// 都没有 ⇒ `TypeError: argument must be an int, or have a fileno() method.`。
fn collect_fds(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<Vec<(i64, NonNull<Header>)>, pyawa_core::ExecError> {
    let Some(items) = instance.iterable_items(value) else {
        return Err(instance.raise_builtin_error("TypeError", "arguments 1-3 must be sequences"));
    };
    let mut entries: Vec<(i64, NonNull<Header>)> = Vec::with_capacity(items.len());
    for item in items {
        match file_descriptor_of(instance, item) {
            Ok(fd) => entries.push((fd, item)),
            Err(error) => {
                instance.release(item);
                release_entries(instance, &entries);
                return Err(error);
            }
        }
    }
    Ok(entries)
}

/// 一个元素 ⇒ `fd`（`int` 或带 `fileno()` 的对象；口径见 [`collect_fds`]）。
fn file_descriptor_of(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<i64, pyawa_core::ExecError> {
    if let Some(fd) = instance.int_value(value) {
        return Ok(fd);
    }
    if let Some(fileno) = instance.attribute_optional_of(value, "fileno")? {
        let outcome = pyawa_core::call_value(instance, fileno, &[], &[]);
        instance.release(fileno);
        let result = outcome?;
        let converted = instance.int_value(result);
        instance.release(result);
        if let Some(fd) = converted {
            return Ok(fd);
        }
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        "argument must be an int, or have a fileno() method.",
    ))
}

/// `fd` 的范围校验（照参照实测：负值与 `>= FD_SETSIZE` 都是 `ValueError`）。
fn check_fd_range(instance: &Instance, fd: i64) -> Result<(), pyawa_core::ExecError> {
    if fd < 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("file descriptor cannot be a negative integer ({fd})"),
        ));
    }
    if fd >= SELECT_FD_LIMIT {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "filedescriptor out of range in select()",
        ));
    }
    Ok(())
}

/// `timeout` ⇒ 纳秒（`None` ＝ 无限）。形状照参照实测（见模块文档 ✓）。
fn select_timeout_ns(
    instance: &Instance,
    value: Option<NonNull<Header>>,
) -> Result<Option<i64>, pyawa_core::ExecError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value == instance.singletons().none() {
        return Ok(None);
    }
    let seconds = if let Some(integer) = instance.int_value(value) {
        integer as f64
    } else if let Some(number) = instance.float_value(value) {
        number
    } else {
        return Err(instance.raise_builtin_error("TypeError", "timeout must be a float or None"));
    };
    if seconds.is_nan() {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "Invalid value NaN (not a number)",
        ));
    }
    if seconds < 0.0 {
        return Err(instance.raise_builtin_error("ValueError", "timeout must be non-negative"));
    }
    // `time_t` 的秒上限（i64 纳秒换回秒）以外照参照报溢出 ✓。
    if !seconds.is_finite() || seconds * 1e9 >= i64::MAX as f64 {
        return Err(instance.raise_builtin_error(
            "OverflowError",
            "timestamp out of range for platform time_t",
        ));
    }
    // **向上取整**：不许比请求更早返回（与 `_socket` 的 ms 换算同一口径 ✓）。
    Ok(Some((seconds * 1e9).ceil() as i64))
}
