//! `itertools` 模块的第一个切片（契约 `docs/SPEC-c-modules.md` §5.2.6）。
//!
//! 本层先落地**无限迭代器**里最基础的一个：`count(start=0, step=1)`。
//!
//! - 语义照参照实现**实测**：`count()` ⇒ 0、1、2…；`count(1, 2)` ⇒ 1、3、5…；
//!   `count(step=-1)` ⇒ 0、−1、−2…；`iter(c) is c`（迭代器是它自己的迭代器）
//! - 用法错误的消息**逐条实测**：`count() takes at most 2 arguments (3 given)`、
//!   `count() got an unexpected keyword argument 'x'`、非数值 ⇒ `a number is required`
//! - **已知边界**（契约里写明）：①本层 `int` 是 `i64` ⇒ 越过 `i64` 时**如实报未接线**
//!   （参照实现是任意精度，口径未裁）；②起始值／步长只收整数，`count(0.5)` 那种浮点情形
//!   参照实现接受 ⇒ 本层**如实报未接线**，不静默按整数处理

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};

/// 模块名。
pub const NAME: &str = "itertools";

/// 模块的 `__doc__`——**照参照实现原文**（`tools/gen_itertools_fixture.py` 探测导出）。
pub const DOC: &str = include_str!("itertools_doc.txt");

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
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

/// 取一个"整数"实参；不是整数时按参照实测的消息报错（或对浮点如实报未接线）。
fn integer_argument(
    instance: &Instance,
    name: &str,
    value: NonNull<Header>,
) -> Result<i64, ExecError> {
    if let Some(number) = instance.int_value(value) {
        return Ok(number);
    }
    let _ = name;
    if instance.float_value(value).is_some() {
        // **不静默按整数处理**：参照实现接受浮点（`count(0.5)` ⇒ 0.5、1.5…），本层
        // `count` 的载荷是整数 ⇒ 如实报"未接线"（`ExecError::Unsupported` 是 VM 级错误，
        // 不是伪造一条 Python 异常）
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "itertools.count 的浮点起始值／步长尚未接线（本层只收整数）",
        });
    }
    // 实测：`itertools.count('a')` ⇒ `TypeError: a number is required`
    Err(instance.raise_builtin_error("TypeError", "a number is required"))
}

/// `itertools.count(start=0, step=1)`。
fn count_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 实测：`count() takes at most 2 arguments (3 given)`
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("count() takes at most 2 arguments ({} given)", args.len()),
        ));
    }
    let mut start = None;
    let mut step = None;
    for (index, value) in args.iter().enumerate() {
        let number = integer_argument(instance, "itertools.count", *value)?;
        if index == 0 {
            start = Some(number);
        } else {
            step = Some(number);
        }
    }
    for (key, value) in kwargs {
        let key = instance.text_value(*key).unwrap_or_default();
        let number = integer_argument(instance, "itertools.count", *value)?;
        match key.as_str() {
            "start" if start.is_none() => start = Some(number),
            "step" if step.is_none() => step = Some(number),
            "start" | "step" => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("count() got multiple values for argument '{key}'"),
                ))
            }
            // 实测：`count() got an unexpected keyword argument 'x'`
            _ => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("count() got an unexpected keyword argument '{key}'"),
                ))
            }
        }
    }
    Ok(instance.new_count_iterator(start.unwrap_or(0), step.unwrap_or(1)))
}

/// `itertools.repeat(object, times=None)`。
fn repeat_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 实测：`repeat() missing required argument 'object' (pos 1)`
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "repeat() missing required argument 'object' (pos 1)",
        ));
    }
    // 实测：`repeat() takes at most 2 arguments (3 given)`
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("repeat() takes at most 2 arguments ({} given)", args.len()),
        ));
    }
    let mut times = None;
    if let Some(value) = args.get(1) {
        times = Some(times_argument(instance, *value)?);
    }
    for (key, value) in kwargs {
        let key = instance.text_value(*key).unwrap_or_default();
        if key != "times" {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("repeat() got an unexpected keyword argument '{key}'"),
            ));
        }
        if times.is_some() {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "repeat() got multiple values for argument 'times'",
            ));
        }
        times = Some(times_argument(instance, *value)?);
    }
    // 实测：`repeat(x)` 无限；`repeat(x, 0)`／`repeat(x, -1)` 都是**空**（负数 ⇒ 0 次）
    let remaining = match times {
        None => -1,
        Some(count) if count < 0 => 0,
        Some(count) => count,
    };
    Ok(instance.new_repeat_iterator(args[0], remaining))
}

/// `times`：整数或 `None`（`None` ⇒ 无限）；别的按参照实测的消息报错。
fn times_argument(instance: &Instance, value: NonNull<Header>) -> Result<i64, ExecError> {
    if value == instance.singletons().none() {
        // `None` 在调用处另有含义（无限），这里用 -1 代表
        return Ok(-1);
    }
    if let Some(number) = instance.int_value(value) {
        return Ok(number);
    }
    // 实测：`itertools.repeat(1, 'a')` ⇒ `'str' object cannot be interpreted as an integer`
    let type_name = instance.type_name(instance.type_of(value));
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("'{type_name}' object cannot be interpreted as an integer"),
    ))
}

/// `stop`／`start` 一类：整数或 `None`（`None` ⇒ 无上界 ⇒ 用 `i64::MAX` 代表，见 §5.2.6）。
fn bound_argument(instance: &Instance, value: NonNull<Header>) -> Result<i64, ExecError> {
    if value == instance.singletons().none() {
        // `None` ⇒ 无上界（核心那台状态机按 `stop < 0` 理解）
        return Ok(-1);
    }
    if let Some(number) = instance.int_value(value) {
        return Ok(number);
    }
    let type_name = instance.type_name(instance.type_of(value));
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("'{type_name}' object cannot be interpreted as an integer"),
    ))
}

/// `itertools.islice(iterable, stop)` ／ `islice(iterable, start, stop[, step])`。
fn islice_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        let key = instance
            .text_value(kwargs[0].0)
            .unwrap_or_default();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("islice() takes no keyword arguments (got '{key}')"),
        ));
    }
    // 实测：`islice expected at least 2 arguments, got 1`
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("islice expected at least 2 arguments, got {}", args.len()),
        ));
    }
    if args.len() > 4 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("islice() takes at most 4 arguments ({} given)", args.len()),
        ));
    }
    let (start, stop, step) = match args.len() {
        2 => (0, bound_argument(instance, args[1])?, 1),
        3 => (
            bound_argument(instance, args[1])?,
            bound_argument(instance, args[2])?,
            1,
        ),
        _ => (
            bound_argument(instance, args[1])?,
            bound_argument(instance, args[2])?,
            bound_argument(instance, args[3])?,
        ),
    };
    // 实测：`ValueError: Step for islice() must be a positive integer or None.`
    if step <= 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "Step for islice() must be a positive integer or None.",
        ));
    }
    // 内层：把可迭代对象变成迭代器（与 `GET_ITER` 同一处实现 ⇒ 消息不会分叉；
    // `islice(5, 1)` 的 `'int' object is not iterable` 就是这里来的）
    let inner = pyawa_core::executor::iter_value(instance, args[0])?;
    // `stop` 的表示：`None` ⇒ 无上界（-1）。实测语义与消费点数全在核心那台状态机里
    // （`start >= stop` 时**仍消费 `start` 个**，夹具记着这一点）
    // `new_islice_iterator` **借用**入参（构造器自己加一份）⇒ 这里归还 `iter_value` 交出来的那份
    let iterator = instance.new_islice_iterator(inner, start, stop, step);
    instance.release(inner);
    Ok(iterator)
}

/// `itertools.chain(*iterables)`。
///
/// 参数收进一个 `list`，再 `iter_value` 成"逐个吐可迭代对象"的外层迭代器——于是
/// **惰性**成立（`chain(count(5), [9])` 不会被无限的内层卡住），与参照实测一致。
/// `chain.from_iterable(...)`（参照的类方法）**未落地**，见 §5.2.6。
fn chain_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "chain() takes no keyword arguments",
        ));
    }
    let items: Vec<NonNull<Header>> = args.to_vec();
    let arguments = instance.new_list(items);
    let outer = match pyawa_core::executor::iter_value(instance, arguments) {
        Ok(outer) => outer,
        Err(error) => {
            instance.release(arguments);
            return Err(error);
        }
    };
    instance.release(arguments);
    let iterator = instance.new_chain_iterator(outer);
    instance.release(outer);
    Ok(iterator)
}

/// 建 `itertools` 的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, handler) in [
        ("count", count_native as pyawa_core::NativeFn),
        ("repeat", repeat_native as pyawa_core::NativeFn),
        ("islice", islice_native as pyawa_core::NativeFn),
        ("chain", chain_native as pyawa_core::NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
