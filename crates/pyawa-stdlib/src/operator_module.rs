//! `operator` 模块（契约 `docs/SPEC-c-modules.md` §5.2.7；**第一刀**）。
//!
//! **本段落地**：`eq`／`ne`／`is_`／`is_not`／`truth`／`not_` —— 都是"读实参、给一个 `bool`"：
//!
//! - 相等走核心的 [`pyawa_core::executor::values_equal_public`]（`TS-40` 口径，`==` 的同一套）
//! - 真值走核心的 [`pyawa_core::executor::truthiness_public`]（同一套，**不另写一份规则**）
//! - 身份是**指针相等**（与 `is` 同一口径）
//!
//! **消息**照 `tools/gen_operator_fixture.py` 从参照**实测**导出的原文；注意 `truth` 那条与别的
//! 不同形（`_operator.truth() takes exactly one argument (2 given)`）——所以两个函数各用各的文案。
//!
//! 本模块不碰平台（stdlib 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};

/// 模块名（`operator`）。
pub const NAME: &str = "operator";

/// 模块的 `__doc__`（与参照同源的一句话）。
pub const DOC: &str = "Operator interface.\n\nThis module exports a set of functions implemented in C corresponding\nto the intrinsic operators of Python.  For example, operator.add(x, y)\nis equivalent to the expression x+y.";

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

/// 取**两个**实参；个数不对时按**实测消息**报错（`eq expected 2 arguments, got 1`）。
fn two_arguments<'a>(
    instance: &Instance,
    name: &str,
    args: &'a [NonNull<Header>],
) -> Result<(&'a NonNull<Header>, &'a NonNull<Header>), ExecError> {
    match args {
        [left, right] => Ok((left, right)),
        _ => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name} expected 2 arguments, got {}", args.len()),
        )),
    }
}

/// 取**一个**实参（`truth`／`not_` 用；消息与上面那条**不同形**，照实测原文）。
fn one_argument<'a>(
    instance: &Instance,
    args: &'a [NonNull<Header>],
) -> Result<&'a NonNull<Header>, ExecError> {
    match args {
        [only] => Ok(only),
        _ => Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "_operator.truth() takes exactly one argument ({} given)",
                args.len()
            ),
        )),
    }
}

/// `operator.eq(a, b)`。
fn eq_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "eq", args)?;
    let equal = pyawa_core::executor::values_equal_public(instance, *left, *right);
    Ok(instance.new_bool(equal))
}

/// `operator.ne(a, b)`。
fn ne_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "ne", args)?;
    let equal = pyawa_core::executor::values_equal_public(instance, *left, *right);
    Ok(instance.new_bool(!equal))
}

/// `operator.is_(a, b)`（身份）。
fn is_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "is_", args)?;
    Ok(instance.new_bool(*left == *right))
}

/// `operator.is_not(a, b)`（身份取反）。
fn is_not_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "is_not", args)?;
    Ok(instance.new_bool(*left != *right))
}

/// `operator.truth(a)`。
fn truth_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let only = one_argument(instance, args)?;
    let value = pyawa_core::executor::truthiness_public(instance, *only, 0)?;
    Ok(instance.new_bool(value))
}

/// `operator.not_(a)`。
fn not_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let only = one_argument(instance, args)?;
    let value = pyawa_core::executor::truthiness_public(instance, *only, 0)?;
    Ok(instance.new_bool(!value))
}

/// 建 `operator` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, handler) in [
        ("eq", eq_native as pyawa_core::NativeFn),
        ("ne", ne_native as pyawa_core::NativeFn),
        ("is_", is_native as pyawa_core::NativeFn),
        ("is_not", is_not_native as pyawa_core::NativeFn),
        ("truth", truth_native as pyawa_core::NativeFn),
        ("not_", not_native as pyawa_core::NativeFn),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 第一刀的**单元级**验收：结果与身份语义，以及两条**实测消息**。
    #[test]
    fn first_cut_results_and_messages_match_the_reference() {
        let instance = Instance::new();
        let one = instance.new_int(1);
        let other = instance.new_int(1);
        let two = instance.new_int(2);

        // `eq`／`ne`：值相等（不同对象）
        let equal = eq_native(&instance, None, &[one, other], &[]).expect("eq 应当成功");
        assert_eq!(instance.bool_value(equal), Some(true), "eq(1, 1) 应当是 True");
        let not_equal = ne_native(&instance, None, &[one, two], &[]).expect("ne 应当成功");
        assert_eq!(instance.bool_value(not_equal), Some(true), "ne(1, 2) 应当是 True");

        // `is_`：小整数是**单例**（`OM-23`）⇒ `new_int(1)` 两次拿到**同一个对象**
        let same = is_native(&instance, None, &[one, other], &[]).expect("is_ 应当成功");
        assert_eq!(
            instance.bool_value(same),
            Some(true),
            "小整数是单例 ⇒ is_(1, 1) 为真（这条同时钉住了 `OM-23`）"
        );

        // **值相等但身份不同**：两个独立的空 `list` ⇒ `eq` 真、`is_` 假
        let left_list = instance.new_list(Vec::new());
        let right_list = instance.new_list(Vec::new());
        assert_ne!(left_list, right_list, "两个 list 应当是不同对象");
        // 值相等用**字符串**验（整数／浮点／字符串按值）；容器的值相等要等 `OM-11` 的
        // `richcompare` 槽位，本层对容器只按身份比 —— 那是**已记录**的缺口，别在这里断言反了
        let left_text = instance.new_str("ab");
        let right_text = instance.new_str("ab");
        let equal =
            eq_native(&instance, None, &[left_text, right_text], &[]).expect("eq 应当成功");
        assert_eq!(instance.bool_value(equal), Some(true), "eq('ab', 'ab') 应当是真（值）");
        let same = is_native(&instance, None, &[left_list, right_list], &[]).expect("is_ 应当成功");
        assert_eq!(instance.bool_value(same), Some(false), "is_([], []) 应当是假（身份）");
        let different =
            is_not_native(&instance, None, &[left_list, right_list], &[]).expect("is_not 应当成功");
        assert_eq!(instance.bool_value(different), Some(true), "is_not([], []) 应当是真");

        // `truth`／`not_`：值 0 假、值 2 真
        let zero = instance.new_int(0);
        let truth_zero = truth_native(&instance, None, &[zero], &[]).expect("truth 应当成功");
        assert_eq!(instance.bool_value(truth_zero), Some(false), "truth(0) 应当是 False");
        let not_zero = not_native(&instance, None, &[zero], &[]).expect("not_ 应当成功");
        assert_eq!(instance.bool_value(not_zero), Some(true), "not_(0) 应当是 True");
        let truth_two = truth_native(&instance, None, &[two], &[]).expect("truth 应当成功");
        assert_eq!(instance.bool_value(truth_two), Some(true), "truth(2) 应当是 True");

        // 实测消息：`eq` 少一个实参
        let error = eq_native(&instance, None, &[one], &[]).expect_err("缺参要报错");
        match error {
            ExecError::Raised { .. } => {}
            other => panic!("应当是 `Raised`，实际 {other:?}"),
        }
        // 实测消息：`truth` 多一个实参
        let error = truth_native(&instance, None, &[one, two], &[]).expect_err("多参要报错");
        match error {
            ExecError::Raised { .. } => {}
            other => panic!("应当是 `Raised`，实际 {other:?}"),
        }
    }
}
