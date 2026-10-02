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

/// 建 `itertools` 的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let count = make_native(instance, "count", count_native);
    instance.dict_set(namespace, "count", count);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
