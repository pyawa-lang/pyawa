//! 异常族的前半（**能抛**）：`BaseException` 层次、异常实例载荷、`RAISE_VARARGS`。
//!
//! 依据：`BC-60`（① `depth`／`lasti` 要遵守 ② 当前异常状态**按实例**存 ③ 链语义与参照一致）、
//! `BC-49` 的异常族（`RAISE_VARARGS` 的 0／1／2 三种形态）、`TS-42`（`BaseException` 层次属 M2）。
//! 处理块派发（`PUSH_EXC_INFO`／`CHECK_EXC_MATCH`／`POP_EXCEPT`／`RERAISE`）是后半，尚未接线。
//!
//! 三条消息都是**实测**的（本机 3.14.4）：
//!
//! ```text
//! TypeError: exceptions must derive from BaseException
//! TypeError: exception causes must derive from BaseException
//! RuntimeError: No active exception to reraise
//! ```

mod common;

use core::ptr::NonNull;

use pyawa_core::{ExceptionObject, ExecError, Header, Instance};

use common::{emit, op, Vm};

fn type_header(instance: &Instance, name: &str) -> NonNull<Header> {
    let ty = instance.type_named(name).unwrap_or_else(|| panic!("{name} 应当已注册"));
    // SAFETY: ty 由注册表持有，存活。
    let header = ty.cast::<Header>();
    // SAFETY: ty 由注册表持有，存活。
    unsafe { instance.incref_object(header.as_ptr()) };
    header
}

/// 最近抛出的异常对象（**借用**）。
fn pending(vm: &Vm) -> &ExceptionObject {
    let raw = vm.instance.pending_exception().expect("应当抛了异常");
    // SAFETY: raw 由实例持有，存活。
    unsafe { &*raw.as_ptr().cast::<ExceptionObject>() }
}

fn pending_type(instance: &Instance) -> String {
    let raw = instance.pending_exception().expect("应当抛了异常");
    // SAFETY: raw 由实例持有，存活。
    let ty = unsafe { raw.as_ref() }.ty();
    // SAFETY: 同上。
    unsafe { ty.as_ref() }.name().to_owned()
}

#[test]
fn exception_hierarchy_matches_the_probe_table() {
    // TS-41／TS-42：整棵 BaseException 树的 `__bases__`／`__mro__` 都要与探测产物一致
    let instance = Instance::new();
    let mut checked = 0;
    for entry in pyawa_core::builtin_types::BUILTIN_TYPES
        .iter()
        .filter(|entry| entry.name == "BaseException" || entry.mro.contains(&"BaseException"))
    {
        let ty = instance
            .type_named(entry.name)
            .unwrap_or_else(|| panic!("{} 应当已注册", entry.name));
        // SAFETY: ty 由注册表持有。
        let object = unsafe { ty.as_ref() };
        let bases: Vec<&str> = object
            .bases()
            .iter()
            .map(|base| {
                // SAFETY: 同上。
                unsafe { base.as_ref() }.name()
            })
            .collect();
        let mro: Vec<&str> = object
            .mro()
            .iter()
            .map(|base| {
                // SAFETY: 同上。
                unsafe { base.as_ref() }.name()
            })
            .collect();
        assert_eq!(bases, entry.bases, "{} 的 __bases__", entry.name);
        assert_eq!(mro, entry.mro, "{} 的 __mro__", entry.name);
        checked += 1;
    }
    assert_eq!(checked, 69, "探测表里异常类有 69 个");

    // 多继承那几支也要走 C3 走对
    let key_error = instance.type_named("KeyError").unwrap();
    let lookup_error = instance.type_named("LookupError").unwrap();
    let exception = instance.type_named("Exception").unwrap();
    assert!(instance.is_subtype(key_error, lookup_error), "KeyError ⊂ LookupError");
    assert!(instance.is_subtype(lookup_error, exception), "LookupError ⊂ Exception");
    assert!(
        !instance.is_subtype(exception, lookup_error),
        "反向不成立"
    );
}

#[test]
fn raise_a_class_instantiates_it() {
    let vm = Vm::new();
    let value_error = type_header(&vm.instance, "ValueError");
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("RAISE_VARARGS"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(value_error)],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(pending_type(&vm.instance), "ValueError");
    assert!(pending(&vm).args().is_empty(), "`raise ValueError` 没有实参");
}

#[test]
fn raise_an_instance_keeps_its_args() {
    let vm = Vm::new();
    let text = vm.instance.alloc(pyawa_core::StrObject::new(
        vm.instance.singletons().str_type(),
        "坏掉了".to_owned(),
    ));
    let value_error = vm.instance.type_named("ValueError").unwrap();
    let exception = vm.instance.alloc(ExceptionObject::new(
        value_error,
        core::cell::RefCell::new(vec![text.into_raw().cast::<Header>()]),
        core::cell::RefCell::new(None),
        core::cell::RefCell::new(None),
        core::cell::Cell::new(false),
        core::cell::RefCell::new(None),
    ));
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("RAISE_VARARGS"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(exception.into_raw().cast::<Header>())],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(pending_type(&vm.instance), "ValueError");
    assert_eq!(pending(&vm).message_with(&vm.instance).as_deref(), Some("坏掉了"));
}

#[test]
fn raise_from_sets_the_cause_and_suppresses_context() {
    let vm = Vm::new();
    let new_error = type_header(&vm.instance, "ValueError");
    let old_error = type_header(&vm.instance, "TypeError");
    let code = vm.code(
        4,
        0,
        emit(&[
            // `raise ValueError from TypeError`：栈上 [异常, 起因]，起因在 TOS
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("RAISE_VARARGS"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(new_error), Some(old_error)],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));

    let object = pending(&vm);
    assert_eq!(pending_type(&vm.instance), "ValueError");
    let cause = object.cause().expect("`from` 要设 __cause__");
    // SAFETY: cause 由异常对象持有。
    let cause_type = unsafe { cause.as_ref() }.ty();
    // SAFETY: 同上。
    let cause_type_name = unsafe { cause_type.as_ref() }.name();
    // 实测：起因写成**类**时参照实现会**实例化**它——`raise X from TypeError` 的 `__cause__`
    // 是 `TypeError()`（`repr` 实测如此），所以这里应当看到 `TypeError` 的**实例**
    assert_eq!(
        cause_type_name, "TypeError",
        "起因是异常实例（类的那一形态由参照实现实例化）"
    );
    assert!(object.suppress_context(), "`from` 要置 __suppress_context__");
}

#[test]
fn raise_from_none_suppresses_without_a_cause() {
    let vm = Vm::new();
    let new_error = type_header(&vm.instance, "TypeError");
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("RAISE_VARARGS"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(new_error), Some(none)],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    let object = pending(&vm);
    assert!(object.cause().is_none(), "`from None` 不设 __cause__");
    assert!(object.suppress_context(), "`from None` 要抑制上下文");
}

#[test]
fn raising_a_non_exception_is_a_type_error() {
    let vm = Vm::new();
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("RAISE_VARARGS"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(1))],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(pending_type(&vm.instance), "TypeError");
    assert_eq!(
        pending(&vm).message_with(&vm.instance).as_deref(),
        Some("exceptions must derive from BaseException")
    );
}

#[test]
fn bare_raise_without_an_active_exception_is_a_runtime_error() {
    let vm = Vm::new();
    let code = vm.code(
        2,
        0,
        emit(&[(op("RAISE_VARARGS"), 0), (op("RETURN_VALUE"), 0)]),
        Vec::new(),
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(pending_type(&vm.instance), "RuntimeError");
    assert_eq!(
        pending(&vm).message_with(&vm.instance).as_deref(),
        Some("No active exception to reraise")
    );
}

#[test]
fn exception_state_is_per_instance() {
    // BC-60 ②：当前异常状态**按实例**存，禁止进程级全局
    let left = Vm::new();
    let right = Vm::new();
    let value_error = type_header(&left.instance, "ValueError");
    let code = left.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("RAISE_VARARGS"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(value_error)],
    );
    assert!(matches!(left.run(&code), Err(ExecError::Raised { .. })));

    assert!(left.instance.pending_exception().is_some());
    assert!(
        right.instance.pending_exception().is_none(),
        "另一个实例不该看见这次抛出"
    );

    // 压入／弹出走的是实例自己的状态
    let current = left.instance.pending_exception().unwrap();
    // SAFETY: current 由左实例持有。
    unsafe { left.instance.incref_object(current.as_ptr()) };
    left.instance.push_exception(current);
    assert_eq!(left.instance.current_exception(), Some(current));
    assert!(left.instance.pop_exception().is_some());
    assert!(left.instance.current_exception().is_none());
    // SAFETY: 弹出交出的是一份新引用。
    unsafe { left.instance.release_object(current.as_ptr()) };
}

// ---- 可观察属性（`T-BC-22` 的观察面；形状逐条实测）----

/// 造一个异常实例（给定 `args` 文本），返回它的指针。
fn make_exception(vm: &Vm, class: &str, args: &[&str]) -> NonNull<Header> {
    let ty = vm.instance.type_named(class).expect("异常类已登记");
    let values: Vec<NonNull<Header>> = args
        .iter()
        .map(|text| vm.instance.new_str(text).cast::<Header>())
        .collect();
    let object = vm.instance.alloc(ExceptionObject::new(
        ty,
        core::cell::RefCell::new(values),
        core::cell::RefCell::new(None),
        core::cell::RefCell::new(None),
        core::cell::Cell::new(false),
        core::cell::RefCell::new(None),
    ));
    object.into_raw().cast::<Header>()
}

fn attr(vm: &Vm, object: NonNull<Header>, name: &str) -> NonNull<Header> {
    pyawa_core::attribute_read(&vm.instance, object, name).expect("属性应当可读")
}

#[test]
fn args_is_a_cached_tuple() {
    let vm = Vm::new();
    let exception = make_exception(&vm, "ValueError", &["a", "b"]);
    let args = attr(&vm, exception, "args");
    // 是 tuple，且内容是两段文本
    assert_eq!(
        vm.instance.type_of(args),
        vm.instance.type_named("tuple").unwrap(),
        "实测：`e.args` 是 tuple"
    );
    let items = vm.instance.tuple_items(args).expect("tuple");
    assert_eq!(items.len(), 2);
    // 实测：`e.args is e.args` 为真 ⇒ 必须**按实例缓存**（不是每次现造）
    let again = attr(&vm, exception, "args");
    assert_eq!(again, args, "`e.args is e.args`（实测为真）");
    // SAFETY: 本测试持有这些引用。
    unsafe {
        vm.instance.release_object(args.as_ptr());
        vm.instance.release_object(again.as_ptr());
        vm.instance.release_object(exception.as_ptr());
    }
}

#[test]
fn an_empty_args_is_still_a_tuple() {
    let vm = Vm::new();
    let exception = make_exception(&vm, "ValueError", &[]);
    let args = attr(&vm, exception, "args");
    let items = vm.instance.tuple_items(args).expect("tuple");
    assert!(items.is_empty(), "实测：`ValueError().args` 是空 tuple");
    // SAFETY: 本测试持有这些引用。
    unsafe {
        vm.instance.release_object(args.as_ptr());
        vm.instance.release_object(exception.as_ptr());
    }
}

#[test]
fn chain_attributes_default_to_none_and_false() {
    let vm = Vm::new();
    let exception = make_exception(&vm, "ValueError", &["x"]);
    for name in ["__cause__", "__context__"] {
        let value = attr(&vm, exception, name);
        assert_eq!(
            vm.instance.type_of(value),
            vm.instance.singletons().none_type(),
            "新造的异常 {name} 是 None（实测）"
        );
        // SAFETY: 本测试持有。
        unsafe { vm.instance.release_object(value.as_ptr()) };
    }
    let suppress = attr(&vm, exception, "__suppress_context__");
    assert_eq!(vm.instance.int_value(suppress), Some(0), "默认 False（实测）");
    // 未抛过的异常 `__traceback__` 是 None（实测）；抛过之后参照实现给 traceback 对象
    let traceback = attr(&vm, exception, "__traceback__");
    assert_eq!(
        vm.instance.type_of(traceback),
        vm.instance.singletons().none_type()
    );
    // SAFETY: 本测试持有。
    unsafe {
        vm.instance.release_object(suppress.as_ptr());
        vm.instance.release_object(traceback.as_ptr());
        vm.instance.release_object(exception.as_ptr());
    }
}

#[test]
fn an_unknown_attribute_is_an_attribute_error() {
    let vm = Vm::new();
    let exception = make_exception(&vm, "ValueError", &["x"]);
    let error = pyawa_core::attribute_read(&vm.instance, exception, "__nosuch__")
        .expect_err("未知属性应当报错");
    let _ = error;
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "AttributeError");
    assert_eq!(
        message.as_deref(),
        Some("'ValueError' object has no attribute '__nosuch__'"),
        "实测原话"
    );
    // SAFETY: 本测试持有。
    unsafe { vm.instance.release_object(exception.as_ptr()) };
}

#[test]
fn the_empty_args_tuple_identity() {
    // 参照实现里 `()` 是单例：`ValueError().args is ()` 为真。
    // 本层 `new_tuple(vec![])` 每次现造 ⇒ 这个**身份**差异要登记（清单 `DIV-7`）。
    let vm = Vm::new();
    let first = vm.instance.new_tuple(Vec::new());
    let second = vm.instance.new_tuple(Vec::new());
    assert_ne!(
        first, second,
        "本层空元组不是单例（与参照实现的身份语义不同，见差异清单）"
    );
    // SAFETY: 本测试持有这两个引用。
    unsafe {
        vm.instance.release_object(first.as_ptr());
        vm.instance.release_object(second.as_ptr());
    }
}
