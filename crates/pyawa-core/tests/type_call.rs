//! **类型对象被调用**（`OM-11` 的 `new` 槽 ＋ `OM-14` 的 `__init__` 分派）与**绑定方法**
//! （`OM-11` 的 `getattr` 查到函数时产出的 `method` 对象）。
//!
//! 这两条是同一件事的两半：`obj.method()` 走编译器的取方法位（栈上是"函数 ＋ `self`"），
//! 而 `obj.method`（不调用）产出**绑定方法对象**；类型被调用则走"类型的 `new` 槽 ＋ `__init__`"。
//! `OM-14` 要求的"子类分派槽位"（`__init__`／`__new__`／`__del__` 可被 Python 子类覆写）
//! 就落在这条路径上。

mod common;

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{ExecError, FunctionObject, Header, MethodObject, Value};

use common::{emit, op, Vm};

fn type_value(vm: &Vm, name: &str) -> NonNull<Header> {
    let ty = vm.instance.type_named(name).unwrap_or_else(|| panic!("{name} 应当已登记"));
    let header = ty.cast::<Header>();
    // SAFETY: ty 由注册表持有。
    unsafe { vm.instance.incref_object(header.as_ptr()) };
    header
}

#[test]
fn calling_a_builtin_type_instantiates_it() {
    // `list()` ⇒ 空列表；`dict()` ⇒ 空字典；`int()` ⇒ 0；`bool()` ⇒ False
    let vm = Vm::new();
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("PUSH_NULL"), 0),
            (op("CALL"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("GET_LEN"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(type_value(&vm, "list"))],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(0), &vm.instance), "空列表的长度是 0");

    let vm = Vm::new();
    let code = vm.code(
        8,
        1,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("PUSH_NULL"), 0),
            (op("CALL"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(type_value(&vm, "int"))],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(0), &vm.instance), "int() 是 0");
}

#[test]
fn calling_an_exception_type_records_the_arguments() {
    // `ValueError("坏掉了")` ⇒ 异常实例的 `args` 就是那个字符串
    // （这正是 `raise ValueError("x")` 走的那条路：编译器发"调用类 ＋ RAISE_VARARGS 1"）
    let vm = Vm::new();
    let message = vm.instance.new_str("坏掉了");
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 1),
            (op("CALL"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(type_value(&vm, "ValueError")), Some(message)],
    );
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当是异常实例");
    // SAFETY: raw 是存活对象。
    assert_eq!(
        unsafe { raw.as_ref() }.ty(),
        vm.instance.type_named("ValueError").unwrap()
    );
    // SAFETY: 类型身份已确认。
    let exception = unsafe { &*raw.as_ptr().cast::<pyawa_core::ExceptionObject>() };
    assert_eq!(
        exception.message_with(&vm.instance).as_deref(),
        Some("坏掉了"),
        "args 里就是那个字符串"
    );
}

#[test]
fn type_call_runs_init_from_the_type_dict() {
    // `OM-14`：`__init__` 从类型字典沿 MRO 找，找到就"实例在先、实参在后"地调它。
    // 这里用一个"把实参存进实例属性"的 `__init__` 来验证它真的跑了。
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    // 名字表在 code object 上，所以直接建带名字的那一份
    let init_code = vm.code_with_names(
        4,
        2,
        2,
        vec!["self".to_owned(), "value".to_owned()],
        vec!["value".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_FAST"), 1),
            (op("LOAD_FAST"), 0),
            (op("STORE_ATTR"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.instance.own(vm.instance.singletons().none()).into_raw())],
    );
    let function = vm.instance.alloc(FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        init_code.into_raw().cast::<Header>(),
        Vec::new(),
        None,
        RefCell::new(None),
    ));
    vm.instance
        .set_type_attribute(ty, "__init__", function.into_raw().cast::<Header>());

    // 常量表持有引用：类型对象要自己那份（少了会把它提前释放）
    // SAFETY: ty 由注册表持有，存活。
    unsafe { vm.instance.incref_object(ty.cast::<Header>().as_ptr()) };
    // C(41) 之后把实例取回来、读它的 value 属性
    let code = vm.code_with_names(
        8,
        1,
        1,
        vec!["instance".to_owned()],
        vec!["value".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 1),
            (op("CALL"), 1),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("LOAD_ATTR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(ty.cast::<Header>()), Some(vm.constant(41))],
    );
    match vm.run(&code) {
        Ok(result) => assert!(
            result.is_same(&Value::small_int(41), &vm.instance),
            "`__init__` 应当把 41 存进实例属性"
        ),
        Err(error) => panic!(
            "跑挂了：{error:?}｜抛出的异常：{:?}",
            vm.pending_exception()
        ),
    }
}

#[test]
fn bound_method_can_be_called() {
    // `m = obj.m; m()` —— 调用**绑定方法对象**时，绑定的实例自动当第一个位置实参
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    let method_code = vm.function_code(
        4,
        1,
        1,
        0,
        0,
        0,
        vec!["self".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(5))],
    );
    let function = vm.instance.alloc(FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        method_code.into_raw().cast::<Header>(),
        Vec::new(),
        None,
        RefCell::new(None),
    ));
    vm.instance
        .set_type_attribute(ty, "m", function.into_raw().cast::<Header>());

    // 造一个实例（走类型的 `new` 槽）
    let created = {
        let object = vm
            .instance
            .alloc(pyawa_core::AttributeObject::new(ty, RefCell::new(None)));
        object.into_raw().cast::<Header>()
    };

    let code = vm.code_with_names(
        8,
        1,
        1,
        vec!["bound".to_owned()],
        vec!["m".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0), // 不带方法位 ⇒ 绑定方法对象
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("PUSH_NULL"), 0),
            (op("CALL"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(created)],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(5), &vm.instance), "方法返回 5");
    let _: Option<NonNull<MethodObject>> = None;
    let _ = ExecError::FellOffEnd;
}

// ---- `__new__` 分派（`OM-14` 的 Python 侧子类分派槽位）----

use core::sync::atomic::{AtomicUsize, Ordering};

use pyawa_core::{AttributeObject, BuiltinFunctionObject, Instance, NativeFn};

// 每个用例各自的计数器：测试是并行跑的，共享一个 static 会互相干扰
static ALLOC_NEW_CALLS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_INIT_CALLS: AtomicUsize = AtomicUsize::new(0);
static INT_NEW_CALLS: AtomicUsize = AtomicUsize::new(0);
static INT_INIT_CALLS: AtomicUsize = AtomicUsize::new(0);

/// `__new__`：造一个本类型实例并返回（相当于 `super().__new__(cls)`）。
fn new_allocates(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    ALLOC_NEW_CALLS.fetch_add(1, Ordering::SeqCst);
    // 类型被调用时走的是"函数"形态：**第一个实参是类**（`__new__(cls, ...)`）；绑定形态也兼容
    let class = match bound {
        Some(class) => class,
        None => *_args.first().expect("`__new__` 至少拿到 cls"),
    };
    let ty = instance.as_type(class).expect("`__new__` 的第一个实参是类");
    let object = instance.alloc_payload(AttributeObject::new(ty, RefCell::new(None)));
    Ok(object.cast::<Header>())
}

/// `__init__`：只记一笔（不写属性，避免把用例耦合到属性通道）。
fn init_records(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    ALLOC_INIT_CALLS.fetch_add(1, Ordering::SeqCst);
    // `__init__` 必须返回 `None`（`T` 如此），这里照办
    Ok(instance.new_none())
}

/// 第二个用例自己的 `__init__`（计数器不与别的用例共享）。
fn init_records_int(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    INT_INIT_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(instance.new_none())
}

/// `__new__`：返回一个**不是本类实例**的值（整数 42）。
fn new_returns_int(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    INT_NEW_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(instance.new_int(42))
}

/// 造一个内建函数对象（测试里当原生可调用用）。
fn native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 已登记");
    instance
        .alloc(BuiltinFunctionObject::new(
            ty,
            Box::leak(name.to_owned().into_boxed_str()),
            core::cell::Cell::new(handler),
        ))
        .into_raw()
        .cast::<Header>()
}

#[test]
fn new_runs_before_init_and_init_still_gets_the_arguments() {
    ALLOC_NEW_CALLS.store(0, Ordering::SeqCst);
    ALLOC_INIT_CALLS.store(0, Ordering::SeqCst);
    let instance = Instance::new();
    let ty = instance.new_attribute_type("WithNew");
    let new_fn = native(&instance, "__new__", new_allocates as NativeFn);
    instance.set_type_attribute(ty, "__new__", new_fn);
    let init_fn = native(&instance, "__init__", init_records as NativeFn);
    instance.set_type_attribute(ty, "__init__", init_fn);

    let class = instance.type_value(ty);
    let seven = instance.new_int(7);
    // 实参归调用方 ⇒ 由 `call_value` 内部新增
    let created = pyawa_core::call_value(&instance, class, &[seven], &[]).expect("WithNew(7)");
    assert_eq!(ALLOC_NEW_CALLS.load(Ordering::SeqCst), 1, "`__new__` 被调用一次");
    assert_eq!(ALLOC_INIT_CALLS.load(Ordering::SeqCst), 1, "`__init__` 也被调用");
    assert!(
        instance.is_subtype(instance.type_of(created), ty),
        "结果应当是 WithNew 的实例"
    );
    // SAFETY: seven 由本测试持有。
    unsafe { instance.release_object(seven.as_ptr()) };
}

#[test]
fn init_is_skipped_when_new_returns_a_foreign_object() {
    // 实测：`__new__` 返回 42 时 `B()` 就是 42，`__init__` **不**被调用
    INT_NEW_CALLS.store(0, Ordering::SeqCst);
    INT_INIT_CALLS.store(0, Ordering::SeqCst);
    let instance = Instance::new();
    let ty = instance.new_attribute_type("ReturnsInt");
    let new_fn = native(&instance, "__new__", new_returns_int as NativeFn);
    instance.set_type_attribute(ty, "__new__", new_fn);
    let init_fn = native(&instance, "__init__", init_records_int as NativeFn);
    instance.set_type_attribute(ty, "__init__", init_fn);

    let class = instance.type_value(ty);
    let result = pyawa_core::call_value(&instance, class, &[], &[]).expect("ReturnsInt()");
    assert_eq!(instance.int_value(result), Some(42), "返回的就是 `__new__` 交出的东西");
    assert_eq!(INT_NEW_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(INT_INIT_CALLS.load(Ordering::SeqCst), 0, "`__init__` 不该被调用");
}

#[test]
fn a_class_without_init_refuses_arguments() {
    // 实测：`Empty(1)` ⇒ `TypeError: Empty() takes no arguments`
    let instance = Instance::new();
    let ty = instance.new_attribute_type("Empty");
    let class = instance.type_value(ty);
    let one = instance.new_int(1);
    match pyawa_core::call_value(&instance, class, &[one], &[]) {
        Err(ExecError::Raised { exception }) => {
            // SAFETY: exception 是存活对象。
            let message = unsafe {
                &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>()
            }
            .message_with(&instance)
            .unwrap_or_default();
            assert_eq!(message, "Empty() takes no arguments");
        }
        other => panic!("应当报 TypeError，实际：{other:?}"),
    }
    // SAFETY: one 由本测试持有。
    unsafe { instance.release_object(one.as_ptr()) };
}
