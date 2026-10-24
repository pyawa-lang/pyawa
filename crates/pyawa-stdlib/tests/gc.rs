//! `gc` 模块**最小诚实面**（task-27，`P3-37` 第一片）的契约测试。
//!
//! 期望值来自本机 `python3` 3.14.4 实测（见各断言注释）；但**分代数字不照抄**——本层单代回收，
//! `get_count`／`get_stats` 如实未实现，测试只钉"能真做的真做、做不了的如实报"这条边界。
//!
//! 为什么要 `pyawa_stdlib::install`：`gc` 的开关/阈值状态放在**模块命名空间**的 `_state` 里
//! （参照里那是解释器状态），而 natives 经 `sys.modules['gc']` 反查它 ⇒ 测试必须先把模块装配进
//! 模块表（与 `tests/imp.rs` 的装法同源）。

use core::ptr::NonNull;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance};
use pyawa_stdlib::gc_module;

/// 取一个原生入口的处理函数（与 `tests/sys.rs` 的 `native()` 同款）。
fn native(instance: &Instance, namespace: NonNull<Header>, name: &str) -> pyawa_core::NativeFn {
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("`gc.{name}` 必须存在（P3-37 的面）"));
    // SAFETY: `gc` 里放进去的都是原生可调用对象。
    unsafe { (*function.as_ptr().cast::<BuiltinFunctionObject>()).function() }
}

fn call(
    instance: &Instance,
    handler: pyawa_core::NativeFn,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 实参由本测试持有（handler 只借用）。
    unsafe { handler(instance, None, args, kwargs) }
}

fn bool_of(instance: &Instance, value: NonNull<Header>) -> bool {
    instance.bool_value(value).expect("布尔对象")
}

fn int_of(instance: &Instance, value: NonNull<Header>) -> i64 {
    instance.int_value(value).expect("整数对象")
}

/// 脚本异常的 `"类名: 消息"`（与 `tests/sys.rs` 的 `error_text` 同款）。
fn error_text(instance: &Instance, error: ExecError) -> String {
    match error {
        ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            let name = unsafe { ty.as_ref() }.name().to_owned();
            // SAFETY: exception 是存活对象。
            let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(instance)
                .unwrap_or_default();
            format!("{name}: {message}")
        }
        other => panic!("应当是脚本异常，实际 {other:?}"),
    }
}

/// 装好组合根（`install` 会把 `gc` 放进模块表），返回其实例与 `gc` 命名空间。
fn mounted() -> (Instance, NonNull<Header>) {
    let instance = Instance::new();
    pyawa_stdlib::install(&instance, "[test]", &[]);
    let modules = instance.modules().expect("install 之后 sys.modules 必须存在");
    let module = instance
        .dict_get(modules, gc_module::NAME)
        .expect("`gc` 必须已装进模块表（P3-37 第一片）");
    let namespace = pyawa_core::mounted_instance_dict(&instance, module)
        .expect("`gc` 模块对象必须挂着命名空间");
    (instance, namespace)
}

#[test]
fn the_module_has_the_reference_names_and_honest_constants() {
    let (instance, namespace) = mounted();
    // 参照 3.14.4 的 `dir(gc)`（去掉 `__` 开头的）——名字面要齐（调用不了 ≠ 名字缺席）
    for name in [
        "collect",
        "disable",
        "enable",
        "freeze",
        "garbage",
        "get_count",
        "get_debug",
        "get_freeze_count",
        "get_objects",
        "get_referents",
        "get_referrers",
        "get_stats",
        "get_threshold",
        "is_finalized",
        "is_tracked",
        "isenabled",
        "set_debug",
        "set_threshold",
        "unfreeze",
        "callbacks",
        "DEBUG_STATS",
        "DEBUG_COLLECTABLE",
        "DEBUG_UNCOLLECTABLE",
        "DEBUG_SAVEALL",
        "DEBUG_LEAK",
    ] {
        assert!(
            instance.dict_get(namespace, name).is_some(),
            "`gc.{name}` 必须存在（参照公开名；未实现也要有名字）"
        );
    }
    // `DEBUG_*` 数值照参照 3.14.4 实测（`DEBUG_LEAK` = 2|4|32 = 38）。
    for (name, expected) in [
        ("DEBUG_STATS", 1i64),
        ("DEBUG_COLLECTABLE", 2),
        ("DEBUG_UNCOLLECTABLE", 4),
        ("DEBUG_SAVEALL", 32),
        ("DEBUG_LEAK", 38),
    ] {
        let value = instance.dict_get(namespace, name).expect("常量存在");
        assert_eq!(int_of(&instance, value), expected, "`gc.{name}` 与参照同值");
    }
    // `garbage`／`callbacks`：都是**空列表**（本层不产生 uncollectable；没有回调机制）
    for name in ["garbage", "callbacks"] {
        let value = instance.dict_get(namespace, name).expect("表存在");
        let items = instance.list_items(value).unwrap_or_else(|| panic!("`gc.{name}` 是 list"));
        assert!(items.is_empty(), "`gc.{name}` 初始为空表");
    }
    assert_eq!(
        instance.text_of(instance.dict_get(namespace, "__name__").expect("__name__ 存在")),
        Some(gc_module::NAME)
    );
}

#[test]
fn enable_disable_and_thresholds_are_real() {
    let (instance, namespace) = mounted();
    let isenabled = native(&instance, namespace, "isenabled");
    let enable = native(&instance, namespace, "enable");
    let disable = native(&instance, namespace, "disable");
    let get_threshold = native(&instance, namespace, "get_threshold");
    let set_threshold = native(&instance, namespace, "set_threshold");

    // 参照默认：`gc.isenabled()` ⇒ True；阈值 (2000, 10, 0)
    let enabled = call(&instance, isenabled, &[], &[]).expect("isenabled()");
    assert!(bool_of(&instance, enabled), "默认启用（与参照一致）");
    let threshold = call(&instance, get_threshold, &[], &[]).expect("get_threshold()");
    let items = instance.tuple_items(threshold).expect("阈值三元组");
    let values: Vec<i64> = items.iter().map(|item| int_of(&instance, *item)).collect();
    assert_eq!(values, vec![2000, 10, 0], "默认阈值照参照 (2000, 10, 0)");

    // `disable()` **真的**停掉自动回收：用户可见状态为假，且 core 阈值被顶到 `usize::MAX`
    call(&instance, disable, &[], &[]).expect("disable()");
    let enabled = call(&instance, isenabled, &[], &[]).expect("isenabled()");
    assert!(!bool_of(&instance, enabled), "disable() 之后 isenabled() ⇒ False");
    assert_eq!(
        instance.gc_threshold().0,
        usize::MAX,
        "disable() 必须真的停掉自动回收（core 阈值顶到 MAX），不是假状态"
    );
    // 用户可见阈值不因 disable 而改（参照同：`get_threshold()` 仍是原值）
    let threshold = call(&instance, get_threshold, &[], &[]).expect("get_threshold()");
    let items = instance.tuple_items(threshold).expect("阈值三元组");
    assert_eq!(
        items.iter().map(|item| int_of(&instance, *item)).collect::<Vec<_>>(),
        vec![2000, 10, 0],
        "disable() 不改变用户可见阈值"
    );

    // `enable()` 恢复 core 阈值（真恢复）
    call(&instance, enable, &[], &[]).expect("enable()");
    let enabled = call(&instance, isenabled, &[], &[]).expect("isenabled()");
    assert!(bool_of(&instance, enabled), "enable() 之后 isenabled() ⇒ True");
    assert_eq!(instance.gc_threshold().0, 2000, "enable() 恢复 core 阈值");

    // `set_threshold(100, 5)`：用户可见 (100, 5, 0) 且 core 真的变成 100
    let t0 = instance.new_int(100);
    let t1 = instance.new_int(5);
    call(&instance, set_threshold, &[t0, t1], &[]).expect("set_threshold(100, 5)");
    assert_eq!(instance.gc_threshold().0, 100, "set_threshold 真落到 core");
    let threshold = call(&instance, get_threshold, &[], &[]).expect("get_threshold()");
    let items = instance.tuple_items(threshold).expect("阈值三元组");
    assert_eq!(
        items.iter().map(|item| int_of(&instance, *item)).collect::<Vec<_>>(),
        vec![100, 5, 0],
        "get_threshold() 读回 set_threshold 的值"
    );
    // 还原（不影响后续用例）
    let restore = [instance.new_int(2000), instance.new_int(10), instance.new_int(0)];
    call(&instance, set_threshold, &restore, &[]).expect("还原阈值");

    // `t0 <= 0`（参照接受）⇒ 自动回收停止（core 阈值 MAX），但用户可见值如实保存
    let zero = instance.new_int(0);
    call(&instance, set_threshold, &[zero], &[]).expect("set_threshold(0)");
    assert_eq!(instance.gc_threshold().0, usize::MAX, "t0=0 ⇒ 自动回收停");
    let threshold = call(&instance, get_threshold, &[], &[]).expect("get_threshold()");
    let items = instance.tuple_items(threshold).expect("阈值三元组");
    assert_eq!(int_of(&instance, items[0]), 0, "用户可见 t0 如实保留 0");
}

#[test]
fn collect_is_a_real_collection_and_validates_the_generation() {
    let (instance, namespace) = mounted();
    let collect = native(&instance, namespace, "collect");
    // 真跑一次（返回 int；本层**单代** ⇒ 数量是整趟释放数，若没有环则为 0 —— 语义如模块头）
    let freed = call(&instance, collect, &[], &[]).expect("collect()");
    assert!(int_of(&instance, freed) >= 0, "collect() 返回释放对象数（int ≥ 0）");
    // `generation=` 关键字收（参照实测 `gc.collect(generation=0)` 合法）
    let zero = instance.new_int(0);
    let keyword = instance.new_str("generation");
    call(&instance, collect, &[], &[(keyword, zero)]).expect("collect(generation=0)");
    // 合法代 0／1／2；3 与 -1 ⇒ ValueError（文案照参照）
    for (value, expected) in [
        (instance.new_int(3), "ValueError: invalid generation"),
        (instance.new_int(-1), "ValueError: invalid generation"),
    ] {
        let error = call(&instance, collect, &[value], &[]).expect_err("非法代要报错");
        assert_eq!(error_text(&instance, error), expected);
    }
    // 非整数 ⇒ TypeError；两个实参 ⇒ at most 1 argument
    let text = instance.new_str("x");
    let error = call(&instance, collect, &[text], &[]).expect_err("非整数要报错");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: 'str' object cannot be interpreted as an integer"
    );
    let a = instance.new_int(0);
    let b = instance.new_int(1);
    let error = call(&instance, collect, &[a, b], &[]).expect_err("两个实参要报错");
    assert_eq!(
        error_text(&instance, error),
        "TypeError: collect() takes at most 1 argument (2 given)"
    );
}

#[test]
fn unimplemented_faces_say_so_and_get_freeze_count_is_zero() {
    let (instance, namespace) = mounted();
    // 未实现的名字：调用必须**如实** `NotImplementedError`（消息点名能力 + P3-37）
    for name in [
        "get_objects",
        "get_referrers",
        "get_referents",
        "is_tracked",
        "is_finalized",
        "freeze",
        "unfreeze",
        "get_count",
        "get_stats",
        "get_debug",
        "set_debug",
    ] {
        let handler = native(&instance, namespace, name);
        let argument = instance.new_int(0);
        let error = call(&instance, handler, &[argument], &[]).expect_err("必须如实未实现");
        let text = error_text(&instance, error);
        assert!(
            text.starts_with("NotImplementedError: ") && text.contains("P3-37"),
            "`gc.{name}` 要如实 NotImplementedError 并挂账：{text}"
        );
    }
    // `get_freeze_count()`：没有 freeze 机制 ⇒ 如实 0（这是真值，不是占位）
    let handler = native(&instance, namespace, "get_freeze_count");
    let value = call(&instance, handler, &[], &[]).expect("get_freeze_count()");
    assert_eq!(int_of(&instance, value), 0, "没有冻结区 ⇒ 冻结数 0");
}
