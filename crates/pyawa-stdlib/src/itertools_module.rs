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

/// `takewhile`／`dropwhile`／`filterfalse` 三者形状相同：`(谓词, 可迭代)`。
fn filter_like_native(
    instance: &Instance,
    name: &'static str,
    mode: u8,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    // 实测：`takewhile expected 2 arguments, got 1`（三个函数各报各的名字）
    if args.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name} expected 2 arguments, got {}", args.len()),
        ));
    }
    // 内层不可迭代时由 `iter_value` 报实测消息（`'int' object is not iterable`）
    let inner = pyawa_core::executor::iter_value(instance, args[1])?;
    let iterator = instance.new_filter_like_iterator(mode, inner, args[0]);
    instance.release(inner);
    Ok(iterator)
}

fn takewhile_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    filter_like_native(instance, "takewhile", 0, args)
}

fn dropwhile_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    filter_like_native(instance, "dropwhile", 1, args)
}

fn filterfalse_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    filter_like_native(instance, "filterfalse", 2, args)
}

/// `itertools.accumulate(iterable[, func])`。
fn accumulate_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 实测：`accumulate() missing required argument 'iterable' (pos 1)`
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "accumulate() missing required argument 'iterable' (pos 1)",
        ));
    }
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("accumulate() takes at most 2 arguments ({} given)", args.len()),
        ));
    }
    // 内层不可迭代 ⇒ 由 `iter_value` 报实测消息
    let inner = pyawa_core::executor::iter_value(instance, args[0])?;
    let iterator = instance.new_accumulate_iterator(inner, args.get(1).copied());
    instance.release(inner);
    Ok(iterator)
}

/// `itertools.starmap(function, iterable)`。
fn starmap_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 实测：`starmap expected 2 arguments, got 1`
    if args.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("starmap expected 2 arguments, got {}", args.len()),
        ));
    }
    let inner = pyawa_core::executor::iter_value(instance, args[1])?;
    let iterator = instance.new_starmap_iterator(inner, args[0]);
    instance.release(inner);
    Ok(iterator)
}

/// `itertools.cycle(iterable)`。
fn cycle_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 实测：`cycle() takes no keyword arguments`；`cycle expected 1 argument, got 0`
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "cycle() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("cycle expected 1 argument, got {}", args.len()),
        ));
    }
    let inner = pyawa_core::executor::iter_value(instance, args[0])?;
    let iterator = instance.new_cycle_iterator(inner);
    instance.release(inner);
    Ok(iterator)
}

/// `itertools.pairwise(iterable)`。
fn pairwise_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 实测：`pairwise expected 1 argument, got 0`
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("pairwise expected 1 argument, got {}", args.len()),
        ));
    }
    let inner = pyawa_core::executor::iter_value(instance, args[0])?;
    let iterator = instance.new_pairwise_iterator(inner);
    instance.release(inner);
    Ok(iterator)
}

/// `itertools.batched(iterable, n)`。
fn batched_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 实测三连：缺 `n`／参数给多了／`n` 不是整数
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "batched() missing required argument 'n' (pos 2)",
        ));
    }
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "batched() takes exactly 2 positional arguments ({} given)",
                args.len()
            ),
        ));
    }
    let size = match instance.int_value(args[1]) {
        Some(value) => value,
        None => {
            let type_name = instance.type_name(instance.type_of(args[1]));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("'{type_name}' object cannot be interpreted as an integer"),
            ));
        }
    };
    // 实测：`ValueError: n must be at least one`
    if size < 1 {
        return Err(instance.raise_builtin_error("ValueError", "n must be at least one"));
    }
    let inner = pyawa_core::executor::iter_value(instance, args[0])?;
    let iterator = instance.new_batched_iterator(inner, size);
    instance.release(inner);
    Ok(iterator)
}

/// `itertools.zip_longest(*iterables, fillvalue=None)`。
fn zip_longest_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let mut fillvalue = instance.retain(instance.singletons().none());
    for (key, value) in kwargs {
        let key_text = instance.text_value(*key).unwrap_or_default();
        if key_text != "fillvalue" {
            // 实测的消息**不带**名字：`zip_longest() got an unexpected keyword argument`
            return Err(instance.raise_builtin_error(
                "TypeError",
                "zip_longest() got an unexpected keyword argument",
            ));
        }
        instance.release(fillvalue);
        fillvalue = instance.retain(*value);
    }
    // 每个实参都先变成迭代器（非可迭代 ⇒ `iter_value` 报实测消息）
    let mut iterators: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        match pyawa_core::executor::iter_value(instance, *argument) {
            Ok(iterator) => iterators.push(iterator),
            Err(error) => {
                for iterator in iterators {
                    instance.release(iterator);
                }
                instance.release(fillvalue);
                return Err(error);
            }
        }
    }
    let list = instance.new_list(iterators);
    let iterator = instance.new_zip_longest_iterator(list, fillvalue);
    instance.release(list);
    instance.release(fillvalue);
    Ok(iterator)
}

/// `itertools.compress(data, selectors)`。
fn compress_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 实测：`compress() missing required argument 'selectors' (pos 2)`
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "compress() missing required argument 'selectors' (pos 2)",
        ));
    }
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("compress() takes at most 2 arguments ({} given)", args.len()),
        ));
    }
    let data = pyawa_core::executor::iter_value(instance, args[0])?;
    let selectors = match pyawa_core::executor::iter_value(instance, args[1]) {
        Ok(value) => value,
        Err(error) => {
            instance.release(data);
            return Err(error);
        }
    };
    let iterator = instance.new_compress_iterator(data, selectors);
    instance.release(data);
    instance.release(selectors);
    Ok(iterator)
}

/// `itertools.combinations(iterable, r)`：池**当场物化**（实测如此）。
fn combinations_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "combinations() missing required argument 'iterable' (pos 1)",
        ));
    }
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "combinations() missing required argument 'r' (pos 2)",
        ));
    }
    let r = match instance.int_value(args[1]) {
        Some(value) => value,
        None => {
            let type_name = instance.type_name(instance.type_of(args[1]));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("'{type_name}' object cannot be interpreted as an integer"),
            ));
        }
    };
    // 实测：`ValueError: r must be non-negative`
    if r < 0 {
        return Err(instance.raise_builtin_error("ValueError", "r must be non-negative"));
    }
    // 池**当场物化**（实测：`combinations(gen, r)` 会把生成器一次取完）
    let inner = pyawa_core::executor::iter_value(instance, args[0])?;
    let mut items: Vec<NonNull<Header>> = Vec::new();
    loop {
        match pyawa_core::executor::runtime::advance(instance, inner) {
            Ok(Some(item)) => items.push(item),
            Ok(None) => break,
            Err(error) => {
                instance.release(inner);
                for item in items {
                    instance.release(item);
                }
                return Err(error);
            }
        }
    }
    instance.release(inner);
    let pool = instance.new_list(items);
    let iterator = instance.new_combinations_iterator(pool, r, false);
    instance.release(pool);
    Ok(iterator)
}

/// `itertools.combinations_with_replacement(iterable, r)`：与 `combinations` 同族，但下标
/// **可重复且非降序**（实测 `([1, 2], 3)` 有 4 个结果）。
fn combinations_with_replacement_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "combinations_with_replacement() missing required argument 'iterable' (pos 1)",
        ));
    }
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "combinations_with_replacement() missing required argument 'r' (pos 2)",
        ));
    }
    let r = match instance.int_value(args[1]) {
        Some(value) => value,
        None => {
            let type_name = instance.type_name(instance.type_of(args[1]));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("'{type_name}' object cannot be interpreted as an integer"),
            ));
        }
    };
    if r < 0 {
        return Err(instance.raise_builtin_error("ValueError", "r must be non-negative"));
    }
    let inner = pyawa_core::executor::iter_value(instance, args[0])?;
    let mut items: Vec<NonNull<Header>> = Vec::new();
    loop {
        match pyawa_core::executor::runtime::advance(instance, inner) {
            Ok(Some(item)) => items.push(item),
            Ok(None) => break,
            Err(error) => {
                instance.release(inner);
                for item in items {
                    instance.release(item);
                }
                return Err(error);
            }
        }
    }
    instance.release(inner);
    let pool = instance.new_list(items);
    let iterator = instance.new_combinations_iterator(pool, r, true);
    instance.release(pool);
    Ok(iterator)
}

/// `itertools.permutations(iterable, r=None)`：池**当场物化**；`r` 缺省是池长。
fn permutations_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "permutations() missing required argument 'iterable' (pos 1)",
        ));
    }
    // 池先物化（`r` 缺省要池长）
    let inner = pyawa_core::executor::iter_value(instance, args[0])?;
    let mut items: Vec<NonNull<Header>> = Vec::new();
    loop {
        match pyawa_core::executor::runtime::advance(instance, inner) {
            Ok(Some(item)) => items.push(item),
            Ok(None) => break,
            Err(error) => {
                instance.release(inner);
                for item in items {
                    instance.release(item);
                }
                return Err(error);
            }
        }
    }
    instance.release(inner);
    // 物化时的长度就是池长（stdlib 侧禁 `unsafe`，读不了 list 的载荷）
    let pool_len = items.len() as i64;
    let pool = instance.new_list(items);
    let r = match args.get(1) {
        Some(value) => match instance.int_value(*value) {
            Some(number) => number,
            None => {
                // 实测：与 `combinations` **不同**，这里报 `Expected int as r`
                instance.release(pool);
                return Err(instance.raise_builtin_error("TypeError", "Expected int as r"));
            }
        },
        None => pool_len,
    };
    // 实测：`ValueError: r must be non-negative`
    if r < 0 {
        instance.release(pool);
        return Err(instance.raise_builtin_error("ValueError", "r must be non-negative"));
    }
    let iterator = instance.new_permutations_iterator(pool, r);
    instance.release(pool);
    Ok(iterator)
}

/// `itertools.product(*iterables, repeat=1)`：每个输入**当场物化**（`repeat` 时**复用同一份池**）。
fn product_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let mut repeat = 1i64;
    for (key, value) in kwargs {
        let key_text = instance.text_value(*key).unwrap_or_default();
        if key_text != "repeat" {
            // 实测：消息**带**名字（`product() got an unexpected keyword argument 'nope'`）
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("product() got an unexpected keyword argument '{key_text}'"),
            ));
        }
        repeat = match instance.int_value(*value) {
            Some(number) => number,
            None => {
                let type_name = instance.type_name(instance.type_of(*value));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("'{type_name}' object cannot be interpreted as an integer"),
                ));
            }
        };
    }
    // 实测：`ValueError: repeat argument cannot be negative`
    if repeat < 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "repeat argument cannot be negative",
        ));
    }
    // 物化每个输入（非可迭代 ⇒ `iter_value` 报实测消息）
    let mut pools: Vec<NonNull<Header>> = Vec::new();
    for argument in args {
        let inner = match pyawa_core::executor::iter_value(instance, *argument) {
            Ok(iterator) => iterator,
            Err(error) => {
                for pool in pools {
                    instance.release(pool);
                }
                return Err(error);
            }
        };
        let mut items: Vec<NonNull<Header>> = Vec::new();
        loop {
            match pyawa_core::executor::runtime::advance(instance, inner) {
                Ok(Some(item)) => items.push(item),
                Ok(None) => break,
                Err(error) => {
                    instance.release(inner);
                    for item in items {
                        instance.release(item);
                    }
                    for pool in pools {
                        instance.release(pool);
                    }
                    return Err(error);
                }
            }
        }
        instance.release(inner);
        pools.push(instance.new_list(items));
    }
    // `repeat`：把同一份池**复用** `repeat` 次
    let mut repeated: Vec<NonNull<Header>> = Vec::new();
    for _ in 0..repeat {
        for pool in &pools {
            repeated.push(instance.retain(*pool));
        }
    }
    let pool_list = instance.new_list(repeated);
    let iterator = instance.new_product_iterator(pool_list);
    instance.release(pool_list);
    for pool in pools {
        instance.release(pool);
    }
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
        ("takewhile", takewhile_native as pyawa_core::NativeFn),
        ("dropwhile", dropwhile_native as pyawa_core::NativeFn),
        ("filterfalse", filterfalse_native as pyawa_core::NativeFn),
        ("accumulate", accumulate_native as pyawa_core::NativeFn),
        ("starmap", starmap_native as pyawa_core::NativeFn),
        ("cycle", cycle_native as pyawa_core::NativeFn),
        ("pairwise", pairwise_native as pyawa_core::NativeFn),
        ("batched", batched_native as pyawa_core::NativeFn),
        ("zip_longest", zip_longest_native as pyawa_core::NativeFn),
        ("compress", compress_native as pyawa_core::NativeFn),
        ("combinations", combinations_native as pyawa_core::NativeFn),
        ("permutations", permutations_native as pyawa_core::NativeFn),
        ("product", product_native as pyawa_core::NativeFn),
        (
            "combinations_with_replacement",
            combinations_with_replacement_native as pyawa_core::NativeFn,
        ),
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
