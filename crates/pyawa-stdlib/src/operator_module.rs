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

/// 比较族共用的实现（`lt`／`le`／`ge`／`gt` 都走它 ⇒ 一处规则）。
fn ordering_native(
    instance: &Instance,
    name: &str,
    symbol: &str,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, name, args)?;
    let truth = pyawa_core::executor::compare_public(instance, *left, *right, symbol, 0)?;
    Ok(instance.new_bool(truth))
}

/// `operator.lt(a, b)`。
fn lt_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    ordering_native(instance, "lt", "<", args)
}

/// `operator.le(a, b)`。
fn le_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    ordering_native(instance, "le", "<=", args)
}

/// `operator.ge(a, b)`。
fn ge_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    ordering_native(instance, "ge", ">=", args)
}

/// `operator.gt(a, b)`。
fn gt_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    ordering_native(instance, "gt", ">", args)
}

/// `operator.add(a, b)`（本层 `int` 是 `i64`；越界如实报未接线）。
fn add_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "add", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "+", 0)
}

/// `operator.sub(a, b)`。
fn sub_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "sub", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "-", 0)
}

/// `operator.mul(a, b)`。
fn mul_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "mul", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "*", 0)
}

/// `operator.floordiv(a, b)`。
fn floordiv_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "floordiv", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "//", 0)
}

/// `operator.mod(a, b)`。
fn mod_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "mod", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "%", 0)
}

/// `operator.pow(a, b)`（本层只做整数指数）。
fn pow_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "pow", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "**", 0)
}

/// `operator.neg(a)`。
fn neg_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let only = one_argument(instance, args)?;
    pyawa_core::executor::unary_public(instance, *only, "-", 0)
}

/// `operator.pos(a)`。
fn pos_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let only = one_argument(instance, args)?;
    pyawa_core::executor::unary_public(instance, *only, "+", 0)
}

/// `operator.abs(a)`。
fn abs_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let only = one_argument(instance, args)?;
    pyawa_core::executor::unary_public(instance, *only, "abs", 0)
}

/// `operator.invert(a)`。
fn invert_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let only = one_argument(instance, args)?;
    pyawa_core::executor::unary_public(instance, *only, "~", 0)
}

/// `operator.and_(a, b)`。
fn and_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "and_", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "&", 0)
}

/// `operator.or_(a, b)`。
fn or_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "or_", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "|", 0)
}

/// `operator.xor(a, b)`。
fn xor_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "xor", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "^", 0)
}

/// `operator.lshift(a, b)`。
fn lshift_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "lshift", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, "<<", 0)
}

/// `operator.rshift(a, b)`。
fn rshift_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (left, right) = two_arguments(instance, "rshift", args)?;
    pyawa_core::executor::arithmetic_public(instance, *left, *right, ">>", 0)
}

/// `operator.is_none(a)`（3.14 新增；判**身份**是不是 `None`）。
fn is_none_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let only = one_argument(instance, args)?;
    Ok(instance.new_bool(*only == instance.singletons().none()))
}

/// `operator.is_not_none(a)`。
fn is_not_none_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let only = one_argument(instance, args)?;
    Ok(instance.new_bool(*only != instance.singletons().none()))
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
        ("lt", lt_native as pyawa_core::NativeFn),
        ("le", le_native as pyawa_core::NativeFn),
        ("ge", ge_native as pyawa_core::NativeFn),
        ("gt", gt_native as pyawa_core::NativeFn),
        ("add", add_native as pyawa_core::NativeFn),
        ("sub", sub_native as pyawa_core::NativeFn),
        ("mul", mul_native as pyawa_core::NativeFn),
        ("floordiv", floordiv_native as pyawa_core::NativeFn),
        ("mod", mod_native as pyawa_core::NativeFn),
        ("pow", pow_native as pyawa_core::NativeFn),
        ("neg", neg_native as pyawa_core::NativeFn),
        ("pos", pos_native as pyawa_core::NativeFn),
        ("abs", abs_native as pyawa_core::NativeFn),
        ("invert", invert_native as pyawa_core::NativeFn),
        ("and_", and_native as pyawa_core::NativeFn),
        ("or_", or_native as pyawa_core::NativeFn),
        ("xor", xor_native as pyawa_core::NativeFn),
        ("lshift", lshift_native as pyawa_core::NativeFn),
        ("rshift", rshift_native as pyawa_core::NativeFn),
        ("is_none", is_none_native as pyawa_core::NativeFn),
        ("is_not_none", is_not_none_native as pyawa_core::NativeFn),
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

/// 夹具只在测试里编译（普通构建里没有使用者 ⇒ 否则报"未使用"）。
#[cfg(test)]
#[path = "../tests/fixtures/operator.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;

    /// **夹具驱动**：`RESULTS` 里每一行都来自生成脚本对参照的实测导出（禁手写）。
    #[test]
    fn fixture_results_hold_for_every_row() {
        let instance = Instance::new();
        for (name, left, right, expected) in fixture::RESULTS {
            let left_value = instance.new_int(*left);
            let right_value = instance.new_int(right.unwrap_or(*left));
            let args = [left_value, right_value];
            let arithmetic = matches!(
                *name,
                "add" | "sub" | "mul" | "floordiv" | "mod" | "pow" | "and_" | "or_" | "xor"
                    | "lshift" | "rshift"
            );
            let result = match *name {
                "add" => add_native(&instance, None, &args, &[]),
                "sub" => sub_native(&instance, None, &args, &[]),
                "mul" => mul_native(&instance, None, &args, &[]),
                "floordiv" => floordiv_native(&instance, None, &args, &[]),
                "mod" => mod_native(&instance, None, &args, &[]),
                "pow" => pow_native(&instance, None, &args, &[]),
                "and_" => and_native(&instance, None, &args, &[]),
                "or_" => or_native(&instance, None, &args, &[]),
                "xor" => xor_native(&instance, None, &args, &[]),
                "lshift" => lshift_native(&instance, None, &args, &[]),
                "rshift" => rshift_native(&instance, None, &args, &[]),
                "eq" => eq_native(&instance, None, &args, &[]),
                "ne" => ne_native(&instance, None, &args, &[]),
                "lt" => lt_native(&instance, None, &args, &[]),
                "le" => le_native(&instance, None, &args, &[]),
                "gt" => gt_native(&instance, None, &args, &[]),
                "ge" => ge_native(&instance, None, &args, &[]),
                "is_" => is_native(&instance, None, &args, &[]),
                other => panic!("夹具里出现了没接线的函数名：{other}"),
            }
            .unwrap_or_else(|error| panic!("{name} 应当成功，实际 {error:?}"));
            if arithmetic {
                assert_eq!(
                    instance.int_value(result),
                    Some(*expected),
                    "夹具那一行：{name}({left}, {right:?}) ⇒ {expected}"
                );
            } else {
                assert_eq!(
                    instance.bool_value(result),
                    Some(*expected != 0),
                    "夹具那一行：{name}({left}, {right:?}) ⇒ {expected}"
                );
            }
        }
    }

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
        // **容器按值比**（本轮新接线）：两个空 list **值相等**、但身份不同
        let equal = eq_native(&instance, None, &[left_list, right_list], &[]).expect("eq 应当成功");
        assert_eq!(instance.bool_value(equal), Some(true), "eq([], []) 应当是真（值）");
        // 不同种类一律不等：`[1] == (1,)` ⇒ False（实测）
        let one_int = instance.new_int(1);
        // `new_list`／`new_tuple` **接手**一份引用 ⇒ 用安全的 `retain` 各给一份
        let as_list = instance.new_list(vec![instance.retain(one_int)]);
        let as_tuple = instance.new_tuple(vec![instance.retain(one_int)]);
        let equal = eq_native(&instance, None, &[as_list, as_tuple], &[]).expect("eq 应当成功");
        assert_eq!(
            instance.bool_value(equal),
            Some(false),
            "eq([1], (1,)) 应当是假（不同种类）"
        );

        // **`dict` 按值比**（本轮接线）：长度 ＋ 每个键（按键值相等）对应值递归相等
        let empty_left = instance.new_dict();
        let empty_right = instance.new_dict();
        let equal =
            eq_native(&instance, None, &[empty_left, empty_right], &[]).expect("eq 应当成功");
        assert_eq!(instance.bool_value(equal), Some(true), "两个空 dict 应当值相等（真）");
        let left_dict = instance.new_dict();
        let right_dict = instance.new_dict();
        let two = instance.new_int(2);
        let three = instance.new_int(3);
        let key = instance.new_int(1);
        instance.dict_set(left_dict, "k", two);
        instance.dict_set(right_dict, "k", instance.retain(two));
        let equal = eq_native(&instance, None, &[left_dict, right_dict], &[]).expect("eq 应当成功");
        assert_eq!(instance.bool_value(equal), Some(true), "eq({{'k': 2}}, {{'k': 2}}) 应当是真");
        let other_dict = instance.new_dict();
        instance.dict_set(other_dict, "k", three);
        let equal =
            eq_native(&instance, None, &[left_dict, other_dict], &[]).expect("eq 应当成功");
        assert_eq!(
            instance.bool_value(equal),
            Some(false),
            "值不同 ⇒ 假（0 与 1 都是整数，这里用 2 与 3 区分）"
        );
        // 不同种类一律不等：`[1] == {1}` ⇒ False
        let equal = eq_native(&instance, None, &[as_list, left_dict], &[]).expect("eq 应当成功");
        assert_eq!(
            instance.bool_value(equal),
            Some(false),
            "list 与 dict 不同种类 ⇒ 假"
        );
        let _ = key;

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

        // **实测消息**（与夹具逐字比）：`lt(1, "a")` 与 `lt(1)`
        assert!(
            fixture::REFERENCE_LT_NOT_SUPPORTED.contains("not supported between instances"),
            "夹具里那条消息应当来自参照"
        );
        // 不可比：`int` 与 `str`（**消息的逐字比对**留给 `tests/` 那层 —— 本 crate 是
        // `#![forbid(unsafe_code)]`，在 `src/` 里读不到异常对象的载荷；这里钉"确实报错"＋
        // 夹具里那条消息确实来自参照）
        let error = lt_native(&instance, None, &[one, instance.new_str("a")], &[])
            .expect_err("int 与 str 不可比，应当报错");
        match error {
            ExecError::Raised { .. } => {}
            other => panic!("应当是 `Raised`，实际 {other:?}"),
        }
        // 3.14 新增的 `is_none`／`is_not_none`（身份判定，一条实测值直断言）
        let none = instance.singletons().none();
        let is_none = is_none_native(&instance, None, &[none], &[]).expect("is_none 应当成功");
        assert_eq!(instance.bool_value(is_none), Some(true), "is_none(None) 应当是 True");
        let a_number = instance.new_int(5);
        let not_none =
            is_not_none_native(&instance, None, &[a_number], &[]).expect("is_not_none 应当成功");
        assert_eq!(instance.bool_value(not_none), Some(true), "is_not_none(5) 应当是 True");

        // 一元四条（实测：`neg(5)=-5`／`pos(5)=5`／`abs(-5)=5`／`invert(5)=-6`）
        let five = instance.new_int(5);
        let minus_five = instance.new_int(-5);
        for (label, value, expected) in [
            ("neg", five, -5i64),
            ("pos", five, 5),
            ("abs", minus_five, 5),
            ("invert", five, -6),
        ] {
            let result = match label {
                "neg" => neg_native(&instance, None, &[value], &[]),
                "pos" => pos_native(&instance, None, &[value], &[]),
                "abs" => abs_native(&instance, None, &[value], &[]),
                _ => invert_native(&instance, None, &[value], &[]),
            }
            .unwrap_or_else(|error| panic!("{label} 应当成功：{error:?}"));
            assert_eq!(instance.int_value(result), Some(expected), "{label}");
        }
        assert!(
            fixture::REFERENCE_NEG_NOT_SUPPORTED.contains("bad operand type for unary -"),
            "夹具：{}",
            fixture::REFERENCE_NEG_NOT_SUPPORTED
        );
        assert!(
            fixture::REFERENCE_LSHIFT_NEGATIVE.ends_with("negative shift count"),
            "夹具：{}",
            fixture::REFERENCE_LSHIFT_NEGATIVE
        );
        // 除零那条（本轮新增）
        assert!(
            fixture::REFERENCE_FLOORDIV_ZERO.ends_with("division by zero"),
            "夹具：{}",
            fixture::REFERENCE_FLOORDIV_ZERO
        );
        // 算术族的两条实测消息（本轮新增）
        assert!(
            fixture::REFERENCE_ADD_NOT_SUPPORTED
                .starts_with("TypeError: unsupported operand type(s) for +"),
            "夹具：{}",
            fixture::REFERENCE_ADD_NOT_SUPPORTED
        );
        assert!(
            fixture::REFERENCE_ADD_MISSING.ends_with("add expected 2 arguments, got 1"),
            "夹具：{}",
            fixture::REFERENCE_ADD_MISSING
        );
        // 另两条实测消息（`eq` 缺参／`truth` 多参）也钉一下"确实来自参照"
        assert!(
            fixture::REFERENCE_EQ_MISSING.ends_with("eq expected 2 arguments, got 1"),
            "夹具：{}",
            fixture::REFERENCE_EQ_MISSING
        );
        assert!(
            fixture::REFERENCE_TRUTH_TOO_MANY.contains("takes exactly one argument"),
            "夹具：{}",
            fixture::REFERENCE_TRUTH_TOO_MANY
        );
        // 真值四条也来自夹具（`truth`／`not_` 对 `0`／`1`）
        assert!(!fixture::TRUTH_ZERO && fixture::TRUTH_ONE, "夹具的真值四条");
        assert!(fixture::NOT_ZERO && !fixture::NOT_ONE, "夹具的真值四条");
        assert_eq!(fixture::REFERENCE_NAMES.len(), 57, "参照的公开名个数");
        for name in ["eq", "ne", "is_", "is_not", "truth", "not_", "lt", "le", "ge", "gt"] {
            assert!(
                fixture::REFERENCE_NAMES.contains(&name),
                "夹具里应当有 {name}"
            );
        }
        assert!(
            !fixture::REFERENCE_LT_MISSING.is_empty(),
            "夹具里应当有 `lt` 缺参那条实测消息：{}",
            fixture::REFERENCE_LT_MISSING
        );

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
