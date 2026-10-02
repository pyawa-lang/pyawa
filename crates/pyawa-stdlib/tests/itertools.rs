//! `itertools` 的第一刀：`count`（契约 `docs/SPEC-c-modules.md` §5.2.6）。
//!
//! 序列与错误消息都来自**探测夹具**（`tools/gen_itertools_fixture.py`）⇒ 期望值不是手写的。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};
use pyawa_stdlib::itertools_module;

#[path = "fixtures/itertools.rs"]
mod fixture;

use fixture::{
    ACCUMULATE_MUL, ACCUMULATE_SINGLE, ACCUMULATE_SUM, BATCHED_THREE, BATCHED_TWO,
    CHAIN_EXPECTED, COMBINATIONS_TWO, COMBINATIONS_ZERO, COMPRESS_RESULT, COMPRESS_SHORT,
    CYCLE_EMPTY,
    CYCLE_FIRST_FIVE, REFERENCE_CYCLE_ARG_COUNT, REFERENCE_CYCLE_KEYWORDS,
    REFERENCE_BATCHED_MISSING_N, REFERENCE_BATCHED_NOT_INT, REFERENCE_BATCHED_TOO_MANY,
    REFERENCE_BATCHED_ZERO, REFERENCE_CYCLE_NOT_ITERABLE, REFERENCE_PAIRWISE_ARG_COUNT,
    PAIRWISE_RESULT, PAIRWISE_SHORT, REFERENCE_ZIP_LONGEST_UNKNOWN_KEYWORD, ZIP_LONGEST_EMPTY,
    ZIP_LONGEST_FILL, ZIP_LONGEST_TWO,
    REFERENCE_ACCUMULATE_MISSING, REFERENCE_ACCUMULATE_NONCALLABLE_SINGLE,
    REFERENCE_STARMAP_ARG_COUNT, REFERENCE_STARMAP_NOT_ITERABLE, STARMAP_POW, DROPWHILE_RESULT, FILTERFALSE_RESULT, REFERENCE_FILTER_LIKE_ARG_COUNT,
    REFERENCE_FILTER_LIKE_NOT_CALLABLE, REFERENCE_FILTER_LIKE_NOT_ITERABLE, TAKEWHILE_RESULT, CHAIN_INPUTS, CHAIN_LAZY_FIRST, COUNT_SEQUENCES, ISLICE_CONSUMED_AFTER_EMPTY, ISLICE_SEQUENCES, ISLICE_SHORT_INPUT, REFERENCE_FLOAT_SEQUENCE,
    REFERENCE_ISLICE_MESSAGES, REFERENCE_NAMES, REFERENCE_NOT_A_NUMBER, REFERENCE_REPEAT_MESSAGES,
    REFERENCE_CHAIN_NOT_ITERABLE, REFERENCE_TOO_MANY, REFERENCE_UNKNOWN_KEYWORD,
    REPEAT_INFINITE_FIRST, REPEAT_SEQUENCES,
    CWR_OVER, CWR_TWO, REFERENCE_CWR_MISSING_ITERABLE, REFERENCE_CWR_MISSING_R,
    PRODUCT_RESULT, REFERENCE_PRODUCT_NEGATIVE, REFERENCE_PRODUCT_NOT_INT,
    REFERENCE_PRODUCT_UNKNOWN_KEYWORD, PERMUTATIONS_THREE, PERMUTATIONS_TWO, PERMUTATIONS_ZERO, REFERENCE_PERMUTATIONS_MISSING,
    REFERENCE_PERMUTATIONS_NEGATIVE, REFERENCE_PERMUTATIONS_NOT_INT,
    REFERENCE_COMPRESS_MISSING, REFERENCE_COMBINATIONS_MISSING_R,
    REFERENCE_COMBINATIONS_NOT_INT, REFERENCE_COMBINATIONS_NEGATIVE,
};

fn count(instance: &Instance, args: &[i64]) -> Result<NonNull<Header>, ExecError> {
    let namespace = itertools_module::build(instance);
    let function = instance
        .dict_get(namespace, "count")
        .expect("§5.2.6：`itertools.count` 必须存在");
    let arguments: Vec<NonNull<Header>> = args.iter().map(|v| instance.new_int(*v)).collect();
    // SAFETY: function 是本实例里存活的原生可调用对象。
    let handler = unsafe {
        (*function
            .as_ptr()
            .cast::<pyawa_core::BuiltinFunctionObject>())
        .function()
    };
    // SAFETY: 实参都是调用方持有的引用（handler 只借用）。
    unsafe { handler(instance, None, &arguments, &[]) }
}

fn message_of(instance: &Instance, error: ExecError) -> String {
    match error {
        ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let text = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(instance)
                .unwrap_or_default();
            // SAFETY: 类型名读的是类型对象。
            let name = unsafe { &*exception.as_ptr() }.ty();
            format!("{}: {text}", instance.type_name(name))
        }
        other => panic!("应当是脚本异常，实际 {other:?}"),
    }
}

#[test]
fn count_walks_the_reference_sequences() {
    for (start, step, expected) in COUNT_SEQUENCES {
        let instance = Instance::new();
        let iterator = count(&instance, &[*start, *step]).expect("两个整数实参应当成功");
        let mut seen = Vec::new();
        for _ in 0..expected.len() {
            let value = pyawa_core::executor::advance(&instance, iterator)
                .expect("推进应当成功")
                .expect("count 是无限的");
            seen.push(instance.int_value(value).expect("count 吐整数"));
        }
        assert_eq!(
            seen, expected.to_vec(),
            "count({start}, {step}) 的前 {} 个值与参照一致",
            expected.len()
        );
    }
    // 默认值：`count()` ⇒ 0、1、2…（参照 `count(0, 1)` 那一行就是它）
    let instance = Instance::new();
    let iterator = count(&instance, &[]).expect("0 个实参应当成功");
    let mut seen = Vec::new();
    for _ in 0..5 {
        let value = pyawa_core::executor::advance(&instance, iterator)
            .expect("推进应当成功")
            .expect("无限");
        seen.push(instance.int_value(value).expect("整数"));
    }
    assert_eq!(seen, COUNT_SEQUENCES[0].2.to_vec());
}

#[test]
fn the_error_messages_are_the_measured_ones() {
    let instance = Instance::new();
    let error = count(&instance, &[1, 2, 3]).expect_err("3 个实参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_TOO_MANY);
    // `Instance` 没有关键字那条入口 ⇒ 直接调 handler（同一份实现）
    let namespace = itertools_module::build(&instance);
    let function = instance
        .dict_get(namespace, "count")
        .expect("count 在模块里");
    // SAFETY: function 是本实例里存活的原生可调用对象。
    let handler = unsafe {
        (*function
            .as_ptr()
            .cast::<pyawa_core::BuiltinFunctionObject>())
        .function()
    };
    let key = instance.new_str("x");
    let value = instance.new_int(1);
    // SAFETY: 实参都是调用方持有的引用。
    let error = unsafe { handler(&instance, None, &[], &[(key, value)]) }
        .expect_err("未知关键字要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_UNKNOWN_KEYWORD);
    let text = instance.new_str("a");
    // SAFETY: 同上。
    let error = unsafe { handler(&instance, None, &[text], &[]) }.expect_err("非数值要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_NOT_A_NUMBER);
}

#[test]
fn floats_are_reported_as_unwired_not_silently_truncated() {
    // 参照实现接受浮点（夹具里记着那条序列）；本层 `count` 的载荷是整数 ⇒ **如实报未接线**，
    // 绝不静默按整数处理（那会造出一个参照没有的行为）
    assert!(!REFERENCE_FLOAT_SEQUENCE.is_empty(), "夹具里应当记着参照的浮点行为");
    let instance = Instance::new();
    let namespace = itertools_module::build(&instance);
    let function = instance.dict_get(namespace, "count").expect("count 在模块里");
    // SAFETY: 同上。
    let handler = unsafe {
        (*function
            .as_ptr()
            .cast::<pyawa_core::BuiltinFunctionObject>())
        .function()
    };
    let half = instance.new_float(0.5);
    // SAFETY: 同上。
    let error = unsafe { handler(&instance, None, &[half], &[]) }.expect_err("浮点要如实报未接线");
    assert!(matches!(error, ExecError::Unsupported { .. }), "不是伪造的 Python 异常");
    assert!(REFERENCE_NAMES.contains(&"count"));
    assert!(REFERENCE_NAMES.len() >= 20, "参照的公开面远不止 count");
}

#[test]
fn the_module_identity_matches_the_reference() {
    let instance = Instance::new();
    let namespace = itertools_module::build(&instance);
    let name = instance.dict_get(namespace, "__name__").expect("有 __name__");
    assert_eq!(instance.text_value(name).as_deref(), Some("itertools"));
    let doc = instance.dict_get(namespace, "__doc__").expect("有 __doc__");
    assert_eq!(instance.text_value(doc).as_deref(), Some(itertools_module::DOC));
    assert!(itertools_module::DOC.starts_with("Functional tools for creating and using iterators."));
}

/// 取一个具名原生函数。
fn native(instance: &Instance, name: &str) -> NonNull<Header> {
    let namespace = itertools_module::build(instance);
    instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("§5.2.6：`itertools.{name}` 必须存在"))
}

/// 调一个原生函数（实参是**借用**视图）。
fn call_with(
    instance: &Instance,
    function: NonNull<Header>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: function 是本实例里存活的原生可调用对象。
    let handler = unsafe {
        (*function
            .as_ptr()
            .cast::<pyawa_core::BuiltinFunctionObject>())
        .function()
    };
    // SAFETY: 实参都是调用方持有的引用（handler 只借用）。
    unsafe { handler(instance, None, args, kwargs) }
}

/// 把一个迭代器取到耗尽（返回取到的整数）。
fn drain(instance: &Instance, iterator: NonNull<Header>) -> Vec<i64> {
    let mut seen = Vec::new();
    while let Some(item) = pyawa_core::executor::advance(instance, iterator).expect("推进应当成功") {
        seen.push(instance.int_value(item).expect("这里只取整数"));
    }
    seen
}

fn int_list(instance: &Instance, values: &[i64]) -> NonNull<Header> {
    let items: Vec<NonNull<Header>> = values.iter().map(|v| instance.new_int(*v)).collect();
    instance.new_list(items)
}

#[test]
fn repeat_walks_the_reference_sequences() {
    for (value, times, expected) in REPEAT_SEQUENCES {
        let instance = Instance::new();
        let function = native(&instance, "repeat");
        let object = instance.new_int(*value);
        let count = instance.new_int(*times);
        let iterator = call_with(&instance, function, &[object, count], &[]).expect("应当成功");
        assert_eq!(
            drain(&instance, iterator),
            expected.to_vec(),
            "repeat({value}, {times}) 与参照一致"
        );
    }
    // `times=None` ⇒ 无限（夹具记的是头 3 个）
    let instance = Instance::new();
    let function = native(&instance, "repeat");
    let object = instance.new_int(7);
    let iterator = call_with(&instance, function, &[object], &[]).expect("应当成功");
    let mut seen = Vec::new();
    for _ in 0..REPEAT_INFINITE_FIRST.len() {
        let item = pyawa_core::executor::advance(&instance, iterator)
            .expect("推进应当成功")
            .expect("无限");
        seen.push(instance.int_value(item).expect("整数"));
    }
    assert_eq!(seen, REPEAT_INFINITE_FIRST.to_vec());
}

#[test]
fn repeat_error_messages_are_the_measured_ones() {
    let instance = Instance::new();
    let function = native(&instance, "repeat");
    // 0 个实参
    let error = call_with(&instance, function, &[], &[]).expect_err("缺参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_REPEAT_MESSAGES[0]);
    // 3 个实参
    let three: Vec<NonNull<Header>> = (0..3).map(|v| instance.new_int(v)).collect();
    let error = call_with(&instance, function, &three, &[]).expect_err("多参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_REPEAT_MESSAGES[1]);
    // 非整数
    let text = instance.new_str("a");
    let one = instance.new_int(1);
    let error = call_with(&instance, function, &[one, text], &[]).expect_err("非整数要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_REPEAT_MESSAGES[2]);
}

#[test]
fn islice_walks_the_reference_sequences() {
    for (start, stop, step, expected) in ISLICE_SEQUENCES {
        let instance = Instance::new();
        let function = native(&instance, "islice");
        let source = int_list(&instance, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let arguments = [
            source,
            instance.new_int(*start),
            instance.new_int(*stop),
            instance.new_int(*step),
        ];
        let iterator = call_with(&instance, function, &arguments, &[]).expect("应当成功");
        assert_eq!(
            drain(&instance, iterator),
            expected.to_vec(),
            "islice(list, {start}, {stop}, {step}) 与参照（range(10)）一致"
        );
    }
    // 短输入：内层先耗尽
    let instance = Instance::new();
    let function = native(&instance, "islice");
    let source = int_list(&instance, &[1, 2]);
    let stop = instance.new_int(10);
    let iterator = call_with(&instance, function, &[source, stop], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), ISLICE_SHORT_INPUT.to_vec());
    // 两参形式：`islice(seq, stop)` ⇒ 前 stop 个
    let instance = Instance::new();
    let function = native(&instance, "islice");
    let source = int_list(&instance, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let stop = instance.new_int(5);
    let iterator = call_with(&instance, function, &[source, stop], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), ISLICE_SEQUENCES[0].3.to_vec());
}

#[test]
fn islice_consumes_up_to_start_even_when_start_meets_stop() {
    // 参照实测：`islice(count(100), 5, 2)` 一个都不让出，但**仍消费 5 个**
    // ⇒ 之后 `next(count())` 是 105（不是 100）
    let instance = Instance::new();
    let function = native(&instance, "islice");
    let inner = instance.new_count_iterator(100, 1);
    let start = instance.new_int(5);
    let stop = instance.new_int(2);
    let iterator = call_with(&instance, function, &[inner, start, stop], &[]).expect("应当成功");
    assert!(drain(&instance, iterator).is_empty(), "start >= stop ⇒ 空");
    let first = pyawa_core::executor::advance(&instance, inner)
        .expect("推进应当成功")
        .expect("count 无限");
    assert_eq!(
        instance.int_value(first),
        Some(ISLICE_CONSUMED_AFTER_EMPTY),
        "start >= stop 时仍消费 start 个（照参照实测）"
    );
}

#[test]
fn islice_error_messages_are_the_measured_ones() {
    let instance = Instance::new();
    let function = native(&instance, "islice");
    // 参数太少
    let one = int_list(&instance, &[1]);
    let error = call_with(&instance, function, &[one], &[]).expect_err("太少要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_ISLICE_MESSAGES[0]);
    // 步长非正
    let three = int_list(&instance, &[1, 2, 3]);
    let zero = instance.new_int(0);
    let limit = instance.new_int(3);
    let error = call_with(&instance, function, &[three, zero, limit, zero], &[])
        .expect_err("步长 0 要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_ISLICE_MESSAGES[1]);
    // 内层不是可迭代对象（消息与 `GET_ITER` 同一处实现）
    let number = instance.new_int(5);
    let stop = instance.new_int(1);
    let error = call_with(&instance, function, &[number, stop], &[]).expect_err("要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_ISLICE_MESSAGES[2]);
}

#[test]
fn chain_walks_the_reference_sequences() {
    for (inputs, expected) in CHAIN_INPUTS.iter().zip(CHAIN_EXPECTED.iter()) {
        let instance = Instance::new();
        let function = native(&instance, "chain");
        let arguments: Vec<NonNull<Header>> = inputs
            .iter()
            .map(|values| int_list(&instance, values))
            .collect();
        let iterator = call_with(&instance, function, &arguments, &[]).expect("应当成功");
        assert_eq!(
            drain(&instance, iterator),
            expected.to_vec(),
            "chain({inputs:?}) 与参照一致"
        );
    }
}

#[test]
fn chain_is_lazy_and_reports_non_iterables_on_demand() {
    // 惰性：内层是无限的 `count`，但只取头几个 ⇒ 不该卡住（与参照实测的头 3 个一致）
    let instance = Instance::new();
    let function = native(&instance, "chain");
    let infinite = instance.new_count_iterator(5, 1);
    let tail = int_list(&instance, &[9]);
    let iterator = call_with(&instance, function, &[infinite, tail], &[]).expect("应当成功");
    let mut seen = Vec::new();
    for _ in 0..CHAIN_LAZY_FIRST.len() {
        let item = pyawa_core::executor::advance(&instance, iterator)
            .expect("推进应当成功")
            .expect("还没耗尽");
        seen.push(instance.int_value(item).expect("整数"));
    }
    assert_eq!(seen, CHAIN_LAZY_FIRST.to_vec());

    // 元素不是可迭代对象：**取值时**才报（构造 `chain([1], 7)` 本身不报）
    let instance = Instance::new();
    let function = native(&instance, "chain");
    let head = int_list(&instance, &[1]);
    let bad = instance.new_int(7);
    let iterator = call_with(&instance, function, &[head, bad], &[]).expect("构造不该报错");
    let first = pyawa_core::executor::advance(&instance, iterator)
        .expect("第一个元素应当取到")
        .expect("有值");
    assert_eq!(instance.int_value(first), Some(1));
    let error = pyawa_core::executor::advance(&instance, iterator).expect_err("第二个元素要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_CHAIN_NOT_ITERABLE);
}

#[test]
fn predicate_iterators_walk_the_reference_sequences() {
    // 谓词用**原生可调用对象**（本层的编译器还不支持 lambda，但 stdlib 收任意可调用对象）
    let instance = Instance::new();
    // 一个 `x < 3` 的谓词：用 builtins 里的函数不方便 ⇒ 直接搭一个原生函数
    // 用**安全**函数：Rust 允许安全 fn 强转成 unsafe fn 指针（stdlib 侧不许写 unsafe）
    fn less_than_three(
        instance: &Instance,
        _bound: Option<NonNull<Header>>,
        args: &[NonNull<Header>],
        _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    ) -> Result<NonNull<Header>, pyawa_core::ExecError> {
        let value = args
            .first()
            .and_then(|item| instance.int_value(*item))
            .unwrap_or_default();
        Ok(instance.new_bool(value < 3))
    }
    let predicate = {
        let ty = instance
            .type_named("builtin_function_or_method")
            .expect("内建可调用类型");
        let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
            ty,
            "less_than_three",
            core::cell::Cell::new(less_than_three as pyawa_core::NativeFn),
        ));
        object.into_raw().cast::<Header>()
    };
    let source = int_list(&instance, &[1, 2, 3, 4, 1]);
    for (name, expected) in [
        ("takewhile", TAKEWHILE_RESULT),
        ("dropwhile", DROPWHILE_RESULT),
        ("filterfalse", FILTERFALSE_RESULT),
    ] {
        let function = native(&instance, name);
        let iterator = call_with(&instance, function, &[predicate, source], &[])
            .unwrap_or_else(|error| panic!("{name} 应当成功：{error:?}"));
        assert_eq!(drain(&instance, iterator), expected.to_vec(), "{name} 的结果");
    }
}

#[test]
fn predicate_iterator_errors_are_the_measured_ones() {
    let instance = Instance::new();
    let function = native(&instance, "takewhile");
    // 参数个数不对（夹具记的就是"只给 1 个实参"那条：`takewhile expected 2 arguments, got 1`）
    let only_predicate = instance.new_int(1);
    let error = call_with(&instance, function, &[only_predicate], &[]).expect_err("缺参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_FILTER_LIKE_ARG_COUNT);
    // 内层不是可迭代对象
    let predicate = instance.new_int(1);
    let not_iterable = instance.new_int(5);
    let error = call_with(&instance, function, &[predicate, not_iterable], &[])
        .expect_err("内层要报错");
    assert_eq!(
        message_of(&instance, error),
        REFERENCE_FILTER_LIKE_NOT_ITERABLE
    );
    // 谓词不可调用：**取值那一刻**才报
    let not_callable = instance.new_int(5);
    let source = int_list(&instance, &[1]);
    let iterator = call_with(&instance, function, &[not_callable, source], &[])
        .expect("构造时不该报错");
    let error = pyawa_core::executor::advance(&instance, iterator).expect_err("取值时要报错");
    assert_eq!(
        message_of(&instance, error),
        REFERENCE_FILTER_LIKE_NOT_CALLABLE
    );
}

#[test]
fn accumulate_walks_the_reference_sequences() {
    let instance = Instance::new();
    // `accumulate([1, 2, 3])`（无 func ⇒ 加法）
    let function = native(&instance, "accumulate");
    let source = int_list(&instance, &[1, 2, 3]);
    let iterator = call_with(&instance, function, &[source], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), ACCUMULATE_SUM.to_vec());

    // 单元素：`total` 还没建立 ⇒ **不调用** func（实测 `accumulate([1], 5)` ⇒ `[1]`）
    let instance = Instance::new();
    let function = native(&instance, "accumulate");
    let source = int_list(&instance, &[1]);
    let not_callable = instance.new_int(5);
    let iterator = call_with(&instance, function, &[source, not_callable], &[]).expect("应当成功");
    assert_eq!(
        drain(&instance, iterator),
        REFERENCE_ACCUMULATE_NONCALLABLE_SINGLE.to_vec(),
        "非可调用 func ＋ 单元素：不该报错"
    );

    // `accumulate([1, 2, 3], mul)`（有 func ⇒ 走调用）
    let instance = Instance::new();
    fn multiply(
        instance: &Instance,
        _bound: Option<NonNull<Header>>,
        args: &[NonNull<Header>],
        _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    ) -> Result<NonNull<Header>, pyawa_core::ExecError> {
        let product: i64 = args
            .iter()
            .map(|item| instance.int_value(*item).unwrap_or(1))
            .product();
        Ok(instance.new_int(product))
    }
    let multiply_fn = {
        let ty = instance
            .type_named("builtin_function_or_method")
            .expect("内建可调用类型");
        instance
            .alloc(pyawa_core::BuiltinFunctionObject::new(
                ty,
                "multiply",
                core::cell::Cell::new(multiply as pyawa_core::NativeFn),
            ))
            .into_raw()
            .cast::<Header>()
    };
    let function = native(&instance, "accumulate");
    let source = int_list(&instance, &[1, 2, 3]);
    let iterator = call_with(&instance, function, &[source, multiply_fn], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), ACCUMULATE_MUL.to_vec());

    // 单元素（无 func）也照夹具
    let instance = Instance::new();
    let function = native(&instance, "accumulate");
    let source = int_list(&instance, &[5]);
    let iterator = call_with(&instance, function, &[source], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), ACCUMULATE_SINGLE.to_vec());

    // 缺参的消息照实测
    let instance = Instance::new();
    let function = native(&instance, "accumulate");
    let error = call_with(&instance, function, &[], &[]).expect_err("缺参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_ACCUMULATE_MISSING);
}

#[test]
fn starmap_expands_each_item_into_arguments() {
    let instance = Instance::new();
    let function = native(&instance, "starmap");
    // 被调用的函数：一个原生"求和"（本层编译器还不支持 lambda）
    fn add_two(
        instance: &Instance,
        _bound: Option<NonNull<Header>>,
        args: &[NonNull<Header>],
        _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    ) -> Result<NonNull<Header>, pyawa_core::ExecError> {
        let total: i64 = args
            .iter()
            .map(|item| instance.int_value(*item).unwrap_or_default())
            .sum();
        Ok(instance.new_int(total))
    }
    let callee = {
        let ty = instance
            .type_named("builtin_function_or_method")
            .expect("内建可调用类型");
        instance
            .alloc(pyawa_core::BuiltinFunctionObject::new(
                ty,
                "add_two",
                core::cell::Cell::new(add_two as pyawa_core::NativeFn),
            ))
            .into_raw()
            .cast::<Header>()
    };
    // 元素是二元组 ⇒ 展开成两个实参
    let first = instance.new_tuple(vec![instance.new_int(2), instance.new_int(3)]);
    let second = instance.new_tuple(vec![instance.new_int(2), instance.new_int(5)]);
    let source = instance.new_list(vec![first, second]);
    let iterator = call_with(&instance, function, &[callee, source], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), vec![5, 7]);

    // 再用一个"幂"的调用对象对一次夹具里的参照结果（`pow(2,3)=8`、`pow(2,5)=32`）
    fn power(
        instance: &Instance,
        _bound: Option<NonNull<Header>>,
        args: &[NonNull<Header>],
        _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    ) -> Result<NonNull<Header>, pyawa_core::ExecError> {
        let base = args.first().and_then(|item| instance.int_value(*item)).unwrap_or(0);
        let exponent = args.get(1).and_then(|item| instance.int_value(*item)).unwrap_or(0);
        let mut result: i64 = 1;
        for _ in 0..exponent {
            result = result.saturating_mul(base);
        }
        Ok(instance.new_int(result))
    }
    let power_fn = {
        let ty = instance
            .type_named("builtin_function_or_method")
            .expect("内建可调用类型");
        instance
            .alloc(pyawa_core::BuiltinFunctionObject::new(
                ty,
                "power",
                core::cell::Cell::new(power as pyawa_core::NativeFn),
            ))
            .into_raw()
            .cast::<Header>()
    };
    let first = instance.new_tuple(vec![instance.new_int(2), instance.new_int(3)]);
    let second = instance.new_tuple(vec![instance.new_int(2), instance.new_int(5)]);
    let source = instance.new_list(vec![first, second]);
    let iterator = call_with(&instance, function, &[power_fn, source], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), STARMAP_POW.to_vec());

    // 元素不是可展开的 ⇒ 实测消息
    let instance = Instance::new();
    let function = native(&instance, "starmap");
    let callee = instance.new_int(1);
    let bad = int_list(&instance, &[1]);
    let iterator = call_with(&instance, function, &[callee, bad], &[]).expect("构造不该报错");
    let error = pyawa_core::executor::advance(&instance, iterator).expect_err("取值要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_STARMAP_NOT_ITERABLE);

    // 参数个数
    let instance = Instance::new();
    let function = native(&instance, "starmap");
    let only_callee = instance.new_int(1);
    let error = call_with(&instance, function, &[only_callee], &[]).expect_err("缺参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_STARMAP_ARG_COUNT);
}

#[test]
fn cycle_caches_the_inner_and_replays_it() {
    let instance = Instance::new();
    let function = native(&instance, "cycle");
    let source = int_list(&instance, &[1, 2]);
    let iterator = call_with(&instance, function, &[source], &[]).expect("应当成功");
    let mut seen = Vec::new();
    for _ in 0..CYCLE_FIRST_FIVE.len() {
        let item = pyawa_core::executor::advance(&instance, iterator)
            .expect("推进应当成功")
            .expect("循环不会耗尽");
        seen.push(instance.int_value(item).expect("整数"));
    }
    assert_eq!(seen, CYCLE_FIRST_FIVE.to_vec());

    // 空输入 ⇒ 立刻耗尽
    let instance = Instance::new();
    let function = native(&instance, "cycle");
    let source = int_list(&instance, &[]);
    let iterator = call_with(&instance, function, &[source], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), CYCLE_EMPTY.to_vec());

    // 三条用法错误消息
    let instance = Instance::new();
    let function = native(&instance, "cycle");
    let error = call_with(&instance, function, &[], &[]).expect_err("缺参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_CYCLE_ARG_COUNT);
    let key = instance.new_str("x");
    let value = instance.new_int(1);
    let error = call_with(&instance, function, &[], &[(key, value)]).expect_err("关键字要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_CYCLE_KEYWORDS);
    let number = instance.new_int(1);
    let error = call_with(&instance, function, &[number], &[]).expect_err("非可迭代要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_CYCLE_NOT_ITERABLE);
}

#[test]
fn pairwise_and_batched_walk_the_reference_sequences() {
    // `pairwise`：两两成对（从第二个元素起）
    let instance = Instance::new();
    let function = native(&instance, "pairwise");
    let source = int_list(&instance, &[1, 2, 3, 4]);
    let iterator = call_with(&instance, function, &[source], &[]).expect("应当成功");
    let mut seen: Vec<(i64, i64)> = Vec::new();
    while let Some(item) = pyawa_core::executor::advance(&instance, iterator).expect("推进") {
        // SAFETY: 产出的是二元组。
        let pair = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
        let left = instance.int_value(pair.item(0).unwrap()).unwrap();
        let right = instance.int_value(pair.item(1).unwrap()).unwrap();
        seen.push((left, right));
    }
    assert_eq!(seen, PAIRWISE_RESULT.to_vec());

    // 短输入 ⇒ 空（期望值同样来自夹具）
    let instance = Instance::new();
    let function = native(&instance, "pairwise");
    let source = int_list(&instance, &[1]);
    let iterator = call_with(&instance, function, &[source], &[]).expect("应当成功");
    let mut short: Vec<(i64, i64)> = Vec::new();
    while let Some(item) = pyawa_core::executor::advance(&instance, iterator).expect("推进") {
        // SAFETY: 产出的是二元组。
        let pair = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
        short.push((
            instance.int_value(pair.item(0).unwrap()).unwrap(),
            instance.int_value(pair.item(1).unwrap()).unwrap(),
        ));
    }
    assert_eq!(short, PAIRWISE_SHORT.to_vec());

    // `batched`：每批最多 n 个、末批可短
    for (name, source_values, expected) in [
        ("batched", vec![1, 2, 3, 4, 5], BATCHED_TWO),
        ("batched3", vec![1, 2, 3, 4, 5, 6], BATCHED_THREE),
    ] {
        let size = if name == "batched" { 2 } else { 3 };
        let instance = Instance::new();
        let function = native(&instance, "batched");
        let source = int_list(&instance, &source_values);
        let n = instance.new_int(size);
        let iterator = call_with(&instance, function, &[source, n], &[]).expect("应当成功");
        let mut groups: Vec<Vec<i64>> = Vec::new();
        while let Some(item) = pyawa_core::executor::advance(&instance, iterator).expect("推进") {
            // SAFETY: 每批是元组。
            let group = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
            groups.push(
                (0..group.len())
                    .map(|index| instance.int_value(group.item(index).unwrap()).unwrap())
                    .collect(),
            );
        }
        let expected: Vec<Vec<i64>> = expected.iter().map(|group| group.to_vec()).collect();
        assert_eq!(groups, expected, "{name} 的分批");
    }
}

#[test]
fn pairwise_and_batched_errors_are_the_measured_ones() {
    let instance = Instance::new();
    let function = native(&instance, "pairwise");
    let error = call_with(&instance, function, &[], &[]).expect_err("缺参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_PAIRWISE_ARG_COUNT);

    let instance = Instance::new();
    let function = native(&instance, "batched");
    let source = int_list(&instance, &[1]);
    let error = call_with(&instance, function, &[source], &[]).expect_err("缺 n 要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_BATCHED_MISSING_N);
    let zero = instance.new_int(0);
    let error = call_with(&instance, function, &[source, zero], &[]).expect_err("n=0 要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_BATCHED_ZERO);
    let text = instance.new_str("a");
    let error = call_with(&instance, function, &[source, text], &[]).expect_err("n 非整数要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_BATCHED_NOT_INT);
    let two = instance.new_int(2);
    let three = instance.new_int(3);
    let error =
        call_with(&instance, function, &[source, two, three], &[]).expect_err("参数多要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_BATCHED_TOO_MANY);
}

#[test]
fn zip_longest_walks_the_reference_sequences() {
    // 期望值来自夹具（`None` 表示补齐）
    let cases: Vec<(Vec<Vec<i64>>, Option<i64>, &[&[Option<i64>]])> = vec![
        (vec![vec![1, 2, 3], vec![4, 5]], None, ZIP_LONGEST_TWO),
        (vec![vec![1, 2], vec![3]], Some(0), ZIP_LONGEST_FILL),
        (vec![vec![], vec![1]], None, ZIP_LONGEST_EMPTY),
    ];
    for (inputs, fill, expected) in cases {
        let instance = Instance::new();
        let function = native(&instance, "zip_longest");
        let arguments: Vec<NonNull<Header>> = inputs
            .iter()
            .map(|values| int_list(&instance, values))
            .collect();
        // `fillvalue` 只能按**关键字**给
        let iterator = match fill {
            Some(value) => {
                let key = instance.new_str("fillvalue");
                let number = instance.new_int(value);
                call_with(&instance, function, &arguments, &[(key, number)])
                    .expect("应当成功")
            }
            None => call_with(&instance, function, &arguments, &[]).expect("应当成功"),
        };
        let mut rows: Vec<Vec<Option<i64>>> = Vec::new();
        while let Some(item) = pyawa_core::executor::advance(&instance, iterator).expect("推进") {
            // SAFETY: 每行是元组。
            let row = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
            rows.push(
                (0..row.len())
                    .map(|index| {
                        let cell = row.item(index).unwrap();
                        if cell == instance.singletons().none() {
                            None
                        } else {
                            instance.int_value(cell)
                        }
                    })
                    .collect(),
            );
        }
        let expected: Vec<Vec<Option<i64>>> =
            expected.iter().map(|row| row.to_vec()).collect();
        assert_eq!(rows, expected, "zip_longest({inputs:?}, fill={fill:?})");
    }
}

#[test]
fn zip_longest_edge_cases_and_messages() {
    // 无参数 ⇒ `[]`（实测**不报错**）
    let instance = Instance::new();
    let function = native(&instance, "zip_longest");
    let iterator = call_with(&instance, function, &[], &[]).expect("应当成功");
    assert!(pyawa_core::executor::advance(&instance, iterator)
        .expect("推进")
        .is_none());
    // 非可迭代实参
    let number = instance.new_int(1);
    let error = call_with(&instance, function, &[number], &[]).expect_err("要报错");
    assert_eq!(
        message_of(&instance, error),
        "TypeError: 'int' object is not iterable"
    );
    // 未知关键字（消息**不带**名字，照实测）
    let source = int_list(&instance, &[1]);
    let key = instance.new_str("nope");
    let value = instance.new_int(2);
    let error = call_with(&instance, function, &[source], &[(key, value)]).expect_err("要报错");
    assert_eq!(
        message_of(&instance, error),
        REFERENCE_ZIP_LONGEST_UNKNOWN_KEYWORD
    );
}

#[test]
fn compress_and_combinations_walk_the_reference_sequences() {
    // `compress`：按选择器的真假筛；短选择器就停
    let instance = Instance::new();
    let function = native(&instance, "compress");
    let data = int_list(&instance, &[1, 2, 3, 4, 5]);
    let selectors = int_list(&instance, &[1, 0, 1, 0, 1]);
    let iterator = call_with(&instance, function, &[data, selectors], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), COMPRESS_RESULT.to_vec());

    let instance = Instance::new();
    let function = native(&instance, "compress");
    let data = int_list(&instance, &[1, 2, 3]);
    let selectors = int_list(&instance, &[1]);
    let iterator = call_with(&instance, function, &[data, selectors], &[]).expect("应当成功");
    assert_eq!(drain(&instance, iterator), COMPRESS_SHORT.to_vec());

    // `combinations`
    for (pool_values, r, expected) in [
        (vec![1, 2, 3, 4], 2, COMBINATIONS_TWO),
        (vec![1, 2, 3], 0, COMBINATIONS_ZERO),
        (vec![1, 2, 3], 4, &[][..]),
    ] {
        let instance = Instance::new();
        let function = native(&instance, "combinations");
        let pool = int_list(&instance, &pool_values);
        let count = instance.new_int(r);
        let iterator = call_with(&instance, function, &[pool, count], &[]).expect("应当成功");
        let mut groups: Vec<Vec<i64>> = Vec::new();
        while let Some(item) = pyawa_core::executor::advance(&instance, iterator).expect("推进") {
            // SAFETY: 每个组合是元组。
            let group = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
            groups.push(
                (0..group.len())
                    .map(|index| instance.int_value(group.item(index).unwrap()).unwrap())
                    .collect(),
            );
        }
        let expected: Vec<Vec<i64>> = expected.iter().map(|group| group.to_vec()).collect();
        assert_eq!(groups, expected, "combinations({pool_values:?}, {r})");
    }
}

#[test]
fn compress_and_combinations_errors_are_the_measured_ones() {
    let instance = Instance::new();
    let function = native(&instance, "compress");
    let only_data = int_list(&instance, &[1]);
    let error = call_with(&instance, function, &[only_data], &[]).expect_err("缺选择器要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_COMPRESS_MISSING);

    let instance = Instance::new();
    let function = native(&instance, "combinations");
    let pool = int_list(&instance, &[1, 2]);
    let error = call_with(&instance, function, &[pool], &[]).expect_err("缺 r 要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_COMBINATIONS_MISSING_R);
    let text = instance.new_str("a");
    let error = call_with(&instance, function, &[pool, text], &[]).expect_err("r 非整数要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_COMBINATIONS_NOT_INT);
    let negative = instance.new_int(-1);
    let error = call_with(&instance, function, &[pool, negative], &[]).expect_err("r 负数要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_COMBINATIONS_NEGATIVE);
}

#[test]
fn permutations_walks_the_reference_sequences() {
    for (pool_values, r, expected) in [
        (vec![1, 2, 3], None, PERMUTATIONS_THREE),
        (vec![1, 2, 3], Some(2), PERMUTATIONS_TWO),
        (vec![1, 2], Some(0), PERMUTATIONS_ZERO),
        (vec![1, 2], Some(3), &[][..]),
    ] {
        let instance = Instance::new();
        let function = native(&instance, "permutations");
        let pool = int_list(&instance, &pool_values);
        let mut arguments = vec![pool];
        if let Some(r) = r {
            arguments.push(instance.new_int(r));
        }
        let iterator = call_with(&instance, function, &arguments, &[]).expect("应当成功");
        let mut groups: Vec<Vec<i64>> = Vec::new();
        while let Some(item) = pyawa_core::executor::advance(&instance, iterator).expect("推进") {
            // SAFETY: 每个排列是元组。
            let group = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
            groups.push(
                (0..group.len())
                    .map(|index| instance.int_value(group.item(index).unwrap()).unwrap())
                    .collect(),
            );
        }
        let expected: Vec<Vec<i64>> = expected.iter().map(|group| group.to_vec()).collect();
        assert_eq!(groups, expected, "permutations({pool_values:?}, {r:?})");
    }
}

#[test]
fn permutations_errors_are_the_measured_ones() {
    let instance = Instance::new();
    let function = native(&instance, "permutations");
    let error = call_with(&instance, function, &[], &[]).expect_err("缺参要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_PERMUTATIONS_MISSING);
    let pool = int_list(&instance, &[1, 2]);
    let text = instance.new_str("a");
    let error = call_with(&instance, function, &[pool, text], &[]).expect_err("r 非整数要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_PERMUTATIONS_NOT_INT);
    let negative = instance.new_int(-1);
    let error = call_with(&instance, function, &[pool, negative], &[]).expect_err("r 负数要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_PERMUTATIONS_NEGATIVE);
}

#[test]
fn combinations_with_replacement_walks_the_reference_sequences() {
    for (pool_values, r, expected) in [
        (vec![1, 2, 3], 2, CWR_TWO),
        (vec![1, 2], 3, CWR_OVER),
        (vec![1, 2, 3], 0, COMBINATIONS_ZERO),
    ] {
        let instance = Instance::new();
        let function = native(&instance, "combinations_with_replacement");
        let pool = int_list(&instance, &pool_values);
        let count = instance.new_int(r);
        let iterator = call_with(&instance, function, &[pool, count], &[]).expect("应当成功");
        let mut groups: Vec<Vec<i64>> = Vec::new();
        while let Some(item) = pyawa_core::executor::advance(&instance, iterator).expect("推进") {
            // SAFETY: 每个组合是元组。
            let group = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
            groups.push(
                (0..group.len())
                    .map(|index| instance.int_value(group.item(index).unwrap()).unwrap())
                    .collect(),
            );
        }
        let expected: Vec<Vec<i64>> = expected.iter().map(|group| group.to_vec()).collect();
        assert_eq!(groups, expected, "cwr({pool_values:?}, {r})");
    }
}

#[test]
fn combinations_with_replacement_errors_are_the_measured_ones() {
    let instance = Instance::new();
    let function = native(&instance, "combinations_with_replacement");
    let error = call_with(&instance, function, &[], &[]).expect_err("缺 iterable 要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_CWR_MISSING_ITERABLE);
    let pool = int_list(&instance, &[1, 2]);
    let error = call_with(&instance, function, &[pool], &[]).expect_err("缺 r 要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_CWR_MISSING_R);
    let text = instance.new_str("a");
    let error = call_with(&instance, function, &[pool, text], &[]).expect_err("r 非整数要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_COMBINATIONS_NOT_INT);
    let negative = instance.new_int(-1);
    let error = call_with(&instance, function, &[pool, negative], &[]).expect_err("r 负数要报错");
    assert_eq!(message_of(&instance, error), REFERENCE_COMBINATIONS_NEGATIVE);
}

#[test]
fn product_walks_the_reference_sequences() {
    // `PRODUCT_RESULT` 的行依次是：`([1,2], 'ab')`、`([1,2], repeat=2)`、`([1], [])`
    // 前两行的元素既有整数又有字符串 ⇒ 统一按"对象的 `str` 值"比对
    for (row, inputs, repeat) in [
        (0usize, vec![(vec![1i64, 2], "ints"), (vec![], "strs")], None),
        (1usize, vec![(vec![1, 2], "ints")], Some(2)),
        (2usize, vec![(vec![1], "ints"), (vec![], "ints")], None),
    ] {
        let instance = Instance::new();
        let function = native(&instance, "product");
        let mut arguments: Vec<NonNull<Header>> = Vec::new();
        for (values, kind) in &inputs {
            if *kind == "ints" {
                arguments.push(int_list(&instance, values));
            } else {
                let strings: Vec<NonNull<Header>> =
                    ["a", "b"].iter().map(|text| instance.new_str(text)).collect();
                arguments.push(instance.new_list(strings));
            }
        }
        let iterator = match repeat {
            Some(count) => {
                let key = instance.new_str("repeat");
                let value = instance.new_int(count);
                call_with(&instance, function, &arguments, &[(key, value)]).expect("应当成功")
            }
            None => call_with(&instance, function, &arguments, &[]).expect("应当成功"),
        };
        let mut groups: Vec<Vec<String>> = Vec::new();
        while let Some(item) = pyawa_core::executor::advance(&instance, iterator).expect("推进") {
            // SAFETY: 每个结果都是元组。
            let group = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
            groups.push(
                (0..group.len())
                    .map(|index| {
                        let cell = group.item(index).unwrap();
                        instance
                            .text_value(cell)
                            .unwrap_or_else(|| instance.int_value(cell).unwrap_or_default().to_string())
                    })
                    .collect(),
            );
        }
        let expected: Vec<Vec<String>> = PRODUCT_RESULT[row]
            .iter()
            .map(|group| group.iter().map(|item| (*item).to_owned()).collect())
            .collect();
        assert_eq!(groups, expected, "product 第 {row} 组");
    }

    // `product()` ⇒ `[()]`（一个空元组）
    let instance = Instance::new();
    let function = native(&instance, "product");
    let iterator = call_with(&instance, function, &[], &[]).expect("应当成功");
    let item = pyawa_core::executor::advance(&instance, iterator)
        .expect("推进")
        .expect("应当有一个空元组");
    // SAFETY: 结果是元组。
    let group = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
    assert_eq!(group.len(), 0);
    assert!(pyawa_core::executor::advance(&instance, iterator)
        .expect("推进")
        .is_none());
}

#[test]
fn product_errors_are_the_measured_ones() {
    let instance = Instance::new();
    let function = native(&instance, "product");
    let key = instance.new_str("repeat");
    let text = instance.new_str("a");
    let error = call_with(&instance, function, &[], &[(key, text)]).expect_err("repeat 非整数");
    assert_eq!(message_of(&instance, error), REFERENCE_PRODUCT_NOT_INT);
    let key = instance.new_str("repeat");
    let negative = instance.new_int(-1);
    let error =
        call_with(&instance, function, &[], &[(key, negative)]).expect_err("repeat 负数");
    assert_eq!(message_of(&instance, error), REFERENCE_PRODUCT_NEGATIVE);
    let key = instance.new_str("nope");
    let value = instance.new_int(1);
    let error = call_with(&instance, function, &[], &[(key, value)]).expect_err("未知关键字");
    assert_eq!(
        message_of(&instance, error),
        REFERENCE_PRODUCT_UNKNOWN_KEYWORD
    );
}
