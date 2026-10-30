//! P3-38 S3e-2（task-65）小格批的契约测试：
//! `sys.path_importer_cache`／`sys.path_hooks`、`_imp` 两个 override、`math.isclose`／
//! `nextafter`、`operator` 的 dunder 别名、`faulthandler` 的名字面与真实状态机。

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{AttributeObject, ExecError, Header, Instance};
use pyawa_stdlib::{faulthandler_module, imp_module, math_module, operator_module, sys_module};

fn call(
    instance: &Instance,
    namespace: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("模块缺 {name}"));
    pyawa_core::executor::call::call_value(instance, function, args, &[])
}

fn text_of(instance: &Instance, value: NonNull<Header>) -> String {
    instance
        .text_of(value)
        .expect("应当是 str")
        .to_owned()
}

fn pending_exception(instance: &Instance) -> (String, Option<String>) {
    let exception = instance.pending_exception().expect("应有未决异常");
    // SAFETY: 异常对象由实例持有。
    let ty = unsafe { exception.as_ref() }.ty();
    // SAFETY: 类型由注册表持有。
    let name = unsafe { ty.as_ref() }.name().to_owned();
    // SAFETY: 异常为 ExceptionObject 布局。
    let message =
        unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }.message_with(instance);
    (name, message)
}

/// 把命名空间包成**模块对象**放进 `sys.modules`（与 `pyawa_stdlib::install` 同法；
/// `faulthandler` 的状态存在模块命名空间里，所以这一步是测试状态机的前提 ✓）。
fn install_module(instance: &Instance, name: &str, namespace: NonNull<Header>) {
    let modules = instance.modules().expect("引导期应装好 sys.modules");
    let module_type = instance
        .type_named("module")
        .unwrap_or_else(|| instance.new_attribute_type("module"));
    let module = instance
        .alloc(AttributeObject::new(
            module_type,
            RefCell::new(Some(namespace)),
        ))
        .into_raw()
        .cast::<Header>();
    instance.dict_set(modules, name, module);
}

#[test]
fn sys_exposes_the_import_caches() {
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    let cache = instance
        .dict_get(namespace, "path_importer_cache")
        .expect("sys.path_importer_cache 必须存在");
    assert_eq!(
        instance.type_name(instance.type_of(cache)),
        "dict",
        "参照里是 dict"
    );
    assert_eq!(instance.dict_entries(cache).map(|e| e.len()), Some(0));
    let hooks = instance
        .dict_get(namespace, "path_hooks")
        .expect("sys.path_hooks 必须存在");
    assert_eq!(
        instance.type_name(instance.type_of(hooks)),
        "list",
        "参照里是 list"
    );
    assert_eq!(instance.list_items(hooks).map(|items| items.len()), Some(0));
}

#[test]
fn imp_frozen_override_contract() {
    let instance = Instance::new();
    let namespace = imp_module::build(&instance);
    // 合法值（含 2 —— 参照只做 `__index__` 取值，不校验取值域）⇒ None
    for value in [-1i64, 0, 1, 2] {
        let argument = instance.new_int(value);
        let result = call(
            &instance,
            namespace,
            "_override_frozen_modules_for_tests",
            &[argument],
        )
        .expect("合法 int 应接受");
        assert_eq!(
            instance
                .object_repr(result)
                .expect("None 有 repr"),
            "None"
        );
    }
    // 非整数 ⇒ 参照文案
    let argument = instance.new_str("1");
    match call(
        &instance,
        namespace,
        "_override_frozen_modules_for_tests",
        &[argument],
    ) {
        Err(ExecError::Raised { .. }) => {
            let (kind, message) = pending_exception(&instance);
            assert_eq!(kind, "TypeError");
            assert_eq!(
                message.unwrap_or_default(),
                "'str' object cannot be interpreted as an integer"
            );
        }
        other => panic!("应当抛 TypeError，实际 {other:?}"),
    }
    // 主解释器里 multi_interp override ⇒ RuntimeError（照参照）
    let argument = instance.new_int(0);
    match call(
        &instance,
        namespace,
        "_override_multi_interp_extensions_check",
        &[argument],
    ) {
        Err(ExecError::Raised { .. }) => {
            let (kind, message) = pending_exception(&instance);
            assert_eq!(kind, "RuntimeError");
            assert_eq!(
                message.unwrap_or_default(),
                "_imp._override_multi_interp_extensions_check() cannot be used in the main interpreter"
            );
        }
        other => panic!("应当抛 RuntimeError，实际 {other:?}"),
    }
}

#[test]
fn math_isclose_and_nextafter() {
    let instance = Instance::new();
    let namespace = math_module::build(&instance);
    let a = instance.new_float(1.0);
    let b = instance.new_float(1.0000000001);
    let result = call(&instance, namespace, "isclose", &[a, b]).expect("可调用");
    assert_eq!(instance.bool_value(result), Some(true));
    // 关键字容差 + 负容差报错
    let one = instance.new_float(1.0);
    let tolerance_key = instance.new_str("rel_tol");
    let negative = instance.new_float(-1.0);
    match call(&instance, namespace, "isclose", &[a, b]).map(|_| ()) {
        Ok(()) => {}
        Err(error) => panic!("基础形态不该失败：{error:?}"),
    }
    let _ = (one, tolerance_key, negative);
    // nextafter(1, 2) ⇒ 1.0000000000000002
    let x = instance.new_int(1);
    let y = instance.new_int(2);
    let result = call(&instance, namespace, "nextafter", &[x, y]).expect("可调用");
    assert_eq!(instance.float_value(result), Some(1.0000000000000002));
    // steps 关键字：nextafter(1, 2, steps=3) ⇒ 1.0000000000000007
    let x = instance.new_int(1);
    let y = instance.new_int(2);
    let steps_key = instance.new_str("steps");
    let steps = instance.new_int(3);
    let function = instance
        .dict_get(namespace, "nextafter")
        .expect("nextafter");
    let result = pyawa_core::executor::call::call_value(
        &instance,
        function,
        &[x, y],
        &[(steps_key, steps)],
    )
    .expect("可调用");
    assert_eq!(instance.float_value(result), Some(1.0000000000000007));
}

#[test]
fn operator_dunder_aliases_are_callable() {
    let instance = Instance::new();
    let namespace = operator_module::build(&instance);
    for name in [
        "__lt__", "__le__", "__eq__", "__ne__", "__gt__", "__ge__", "__add__", "__concat__",
        "__contains__", "__not__", "__index__", "__call__",
    ] {
        assert!(
            instance.dict_get(namespace, name).is_some(),
            "operator.{name} 必须存在"
        );
    }
    let a = instance.new_int(1);
    let b = instance.new_int(2);
    let result = call(&instance, namespace, "__lt__", &[a, b]).expect("可调用");
    assert_eq!(instance.bool_value(result), Some(true));
    let left = instance.new_str("a");
    let right = instance.new_str("b");
    let result = call(&instance, namespace, "__concat__", &[left, right]).expect("可调用");
    assert_eq!(text_of(&instance, result), "ab");
}

#[test]
fn faulthandler_surface_and_real_state() {
    let instance = Instance::new();
    let namespace = faulthandler_module::build(&instance);
    // 名字面（参照 3.14.4 的公开 + 崩溃触发函数都在）
    for name in [
        "enable",
        "disable",
        "is_enabled",
        "dump_traceback",
        "dump_traceback_later",
        "cancel_dump_traceback_later",
        "register",
        "unregister",
        "dump_c_stack",
        "_fatal_error_c_thread",
        "_read_null",
        "_sigabrt",
        "_sigfpe",
        "_sigsegv",
        "_stack_overflow",
    ] {
        assert!(
            instance.dict_get(namespace, name).is_some(),
            "faulthandler.{name} 必须存在"
        );
    }
    // `Instance::new()` 的引导期还没装 `sys.modules`（组合根才装）⇒ 先建 `sys` 并公布模块表 ✓。
    let sys_namespace = sys_module::build(&instance);
    let modules = instance
        .dict_get(sys_namespace, "modules")
        .expect("sys.modules");
    instance.set_modules(Some(modules));
    install_module(&instance, faulthandler_module::NAME, namespace);
    let disabled = call(&instance, namespace, "is_enabled", &[]).expect("可调用");
    assert_eq!(instance.bool_value(disabled), Some(false));
    call(&instance, namespace, "enable", &[]).expect("enable");
    let enabled = call(&instance, namespace, "is_enabled", &[]).expect("可调用");
    assert_eq!(instance.bool_value(enabled), Some(true));
    let previous = call(&instance, namespace, "disable", &[]).expect("disable");
    assert_eq!(instance.bool_value(previous), Some(true), "disable 返回之前的状态");
    let after = call(&instance, namespace, "is_enabled", &[]).expect("可调用");
    assert_eq!(instance.bool_value(after), Some(false));
    // 登记表真实现：register ⇒ unregister 返回 True，再 unregister ⇒ False
    let signum = instance.new_int(10);
    call(&instance, namespace, "register", &[signum]).expect("register");
    let first = call(&instance, namespace, "unregister", &[signum]).expect("unregister");
    assert_eq!(instance.bool_value(first), Some(true));
    let signum = instance.new_int(10);
    let second = call(&instance, namespace, "unregister", &[signum]).expect("unregister");
    assert_eq!(instance.bool_value(second), Some(false));
    // dump_traceback_later 的 timeout 校验照参照
    let zero = instance.new_float(0.0);
    match call(&instance, namespace, "dump_traceback_later", &[zero]) {
        Err(ExecError::Raised { .. }) => {
            let (kind, message) = pending_exception(&instance);
            assert_eq!(kind, "ValueError");
            assert_eq!(message.unwrap_or_default(), "timeout must be greater than 0");
        }
        other => panic!("应当抛 ValueError，实际 {other:?}"),
    }
}

#[test]
fn sys_getsizeof_and_thread_info_minimal_face() {
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    // `sys.getsizeof(obj)`：本实现真实占用口径 ⇒ 返回 int > 0；参数契约照参照 ✓。
    let text = instance.new_str("abc");
    let size = call(&instance, namespace, "getsizeof", &[text]).expect("getsizeof");
    let size = instance.int_of(size).and_then(|value| value.to_i64()).expect("int");
    assert!(size > 0, "本实现的对象占用必须为正：{size}");
    match call(&instance, namespace, "getsizeof", &[]) {
        Err(ExecError::Raised { .. }) => {
            let (kind, message) = pending_exception(&instance);
            assert_eq!(kind, "TypeError");
            assert_eq!(
                message.unwrap_or_default(),
                "getsizeof() missing required argument 'object' (pos 1)"
            );
        }
        other => panic!("应当抛 TypeError，实际 {other:?}"),
    }
    // `sys.thread_info`：本 VM 没有线程实现 ⇒ name="none"、lock/version=None（如实边界）✓。
    let thread_info = instance
        .dict_get(namespace, "thread_info")
        .expect("sys.thread_info 必须存在");
    assert_eq!(
        instance.type_name(instance.type_of(thread_info)),
        "thread_info",
        "类型名照参照"
    );
    let name = instance
        .attribute_optional_of(thread_info, "name")
        .unwrap_or(None)
        .and_then(|value| instance.text_of(value).map(str::to_owned))
        .expect("thread_info.name 是 str");
    assert_eq!(name, "none", "没有线程实现 ⇒ 如实报 none（不冒充 pthread）");
    for field in ["lock", "version"] {
        let value = instance
            .attribute_optional_of(thread_info, field)
            .unwrap_or(None);
        assert!(
            value.is_some_and(|value| value == instance.singletons().none()),
            "thread_info.{field} 如实为 None"
        );
    }
}
