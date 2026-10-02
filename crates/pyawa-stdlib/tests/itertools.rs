//! `itertools` 的第一刀：`count`（契约 `docs/SPEC-c-modules.md` §5.2.6）。
//!
//! 序列与错误消息都来自**探测夹具**（`tools/gen_itertools_fixture.py`）⇒ 期望值不是手写的。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};
use pyawa_stdlib::itertools_module;

#[path = "fixtures/itertools.rs"]
mod fixture;

use fixture::{
    CHAIN_EXPECTED, CHAIN_INPUTS, CHAIN_LAZY_FIRST, COUNT_SEQUENCES, ISLICE_CONSUMED_AFTER_EMPTY, ISLICE_SEQUENCES, ISLICE_SHORT_INPUT, REFERENCE_FLOAT_SEQUENCE,
    REFERENCE_ISLICE_MESSAGES, REFERENCE_NAMES, REFERENCE_NOT_A_NUMBER, REFERENCE_REPEAT_MESSAGES,
    REFERENCE_CHAIN_NOT_ITERABLE, REFERENCE_TOO_MANY, REFERENCE_UNKNOWN_KEYWORD,
    REPEAT_INFINITE_FIRST, REPEAT_SEQUENCES,
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
