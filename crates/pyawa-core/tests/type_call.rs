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
