//! `sys.monitoring` 最小 **import 面**（round 19，task-39）的契约测试。
//!
//! 期望值全部来自本机 `python3` 3.14.4 实测（`sys.monitoring` 的 4 个工具 id、`events` 的 20 个
//! 整数位、`MISSING`／`DISABLE` 是互不相同的 `object()` 哨兵）。边界口径：
//! - 常量必须是**真 int**（`bdb.py:20` 在类体里做 `E.PY_START | …` 与字典键）；
//! - 工具 API（`use_tool_id` 一族）**如实未实现** ⇒ 真调用必须 `NotImplementedError`（不假返回）；
//! - `sys.monitoring` 是 module 对象（`sys._jit` 同款）；`events` 是本层的属性命名空间对象
//!   （参照是 `types.SimpleNamespace` ⇒ 只保证属性读法，不保证 `type` 名／`repr`）。

use core::ptr::NonNull;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance};
use pyawa_stdlib::sys_module;

fn attribute(instance: &Instance, object: NonNull<Header>, name: &str) -> NonNull<Header> {
    instance
        .attribute_optional_of(object, name)
        .unwrap_or_else(|_| panic!("读 `{name}` 时抛异常"))
        .unwrap_or_else(|| panic!("`{name}` 必须存在（sys.monitoring 的 import 面）"))
}

fn int_of(instance: &Instance, object: NonNull<Header>) -> i64 {
    instance
        .int_value(object)
        .unwrap_or_else(|| panic!("`{:?}` 必须是整数对象", object))
}

fn native(instance: &Instance, object: NonNull<Header>, name: &str) -> pyawa_core::NativeFn {
    let function = attribute(instance, object, name);
    // SAFETY: monitoring 的工具 API 都是放进去的原生可调用对象。
    unsafe { (*function.as_ptr().cast::<BuiltinFunctionObject>()).function() }
}

fn type_name(instance: &Instance, object: NonNull<Header>) -> String {
    instance.type_name(instance.type_of(object))
}

#[test]
fn monitoring_constants_match_the_reference() {
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    let monitoring = instance
        .dict_get(namespace, "monitoring")
        .expect("sys.monitoring 必须在 sys 命名空间里");

    // ① 工具 id（3.14.4 实测：0/1/2/5）。
    for (name, expected) in [
        ("DEBUGGER_ID", 0),
        ("COVERAGE_ID", 1),
        ("PROFILER_ID", 2),
        ("OPTIMIZER_ID", 5),
    ] {
        assert_eq!(int_of(&instance, attribute(&instance, monitoring, name)), expected, "{name}");
    }

    // ② `MISSING`／`DISABLE`：两个**互不相同**的 `object()` 哨兵（不是 None、不是 int）。
    let missing = attribute(&instance, monitoring, "MISSING");
    let disable = attribute(&instance, monitoring, "DISABLE");
    assert_eq!(type_name(&instance, missing), "object");
    assert_eq!(type_name(&instance, disable), "object");
    assert_ne!(missing, disable, "MISSING 与 DISABLE 必须是不同实例");

    // ③ `events` 的 20 个整数位（照 python3 3.14.4 逐条实测）。
    let events = attribute(&instance, monitoring, "events");
    for (name, expected) in [
        ("NO_EVENTS", 0),
        ("PY_START", 1),
        ("PY_RESUME", 2),
        ("PY_RETURN", 4),
        ("PY_YIELD", 8),
        ("CALL", 16),
        ("LINE", 32),
        ("INSTRUCTION", 64),
        ("JUMP", 128),
        ("BRANCH_LEFT", 256),
        ("BRANCH_RIGHT", 512),
        ("STOP_ITERATION", 1024),
        ("RAISE", 2048),
        ("EXCEPTION_HANDLED", 4096),
        ("PY_UNWIND", 8192),
        ("PY_THROW", 16384),
        ("RERAISE", 32768),
        ("C_RETURN", 65536),
        ("C_RAISE", 131072),
        ("BRANCH", 262144),
    ] {
        assert_eq!(int_of(&instance, attribute(&instance, events, name)), expected, "{name}");
    }
}

#[test]
fn monitoring_tool_api_is_honestly_unimplemented() {
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    let monitoring = instance
        .dict_get(namespace, "monitoring")
        .expect("sys.monitoring 必须在 sys 命名空间里");
    for name in [
        "use_tool_id",
        "free_tool_id",
        "get_tool",
        "set_events",
        "get_events",
        "set_local_events",
        "get_local_events",
        "register_callback",
        "restart_events",
        "clear_tool_id",
    ] {
        let handler = native(&instance, monitoring, name);
        // SAFETY: 无实参调用（handler 只借用实参表）。
        let error = unsafe { handler(&instance, None, &[], &[]) }
            .expect_err("工具 API 真调用必须如实 NotImplementedError");
        let message = match error {
            ExecError::Raised { exception } => {
                // SAFETY: exception 是存活对象。
                let ty = unsafe { exception.as_ref() }.ty();
                // SAFETY: ty 由注册表持有。
                let type_name = unsafe { ty.as_ref() }.name().to_owned();
                let value = instance
                    .object_str(exception)
                    .expect("异常对象要能转文本");
                assert_eq!(type_name, "NotImplementedError", "{name}");
                value
            }
            other => panic!("`{name}` 应抛 Python 异常，实际 {other:?}"),
        };
        assert!(message.contains("sys.monitoring"), "{name}: {message}");
    }
}
