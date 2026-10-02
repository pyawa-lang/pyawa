//! `itertools` 的第一刀：`count`（契约 `docs/SPEC-c-modules.md` §5.2.6）。
//!
//! 序列与错误消息都来自**探测夹具**（`tools/gen_itertools_fixture.py`）⇒ 期望值不是手写的。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};
use pyawa_stdlib::itertools_module;

#[path = "fixtures/itertools.rs"]
mod fixture;

use fixture::{
    COUNT_SEQUENCES, REFERENCE_FLOAT_SEQUENCE, REFERENCE_NAMES, REFERENCE_NOT_A_NUMBER,
    REFERENCE_TOO_MANY, REFERENCE_UNKNOWN_KEYWORD,
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
