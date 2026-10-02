//! **形参默认值**（编译器那半）：`def f(a, b=x)` 与参照**逐字节**一致（见 `tests/compile.rs`
//! 的语料），这里验**端到端**——默认值在 `def` 那一刻求值、调用时绑定。

mod common;

use core::ptr::NonNull;

use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
use pyawa_core::{DictObject, Frame, Header};

use common::Vm;

#[test]
fn a_default_is_evaluated_at_def_time_and_bound_at_call_time() {
    let vm = Vm::new();
    let module = compile(
        "def f(a, b=x):\n    return b\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &module);
    let namespace = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            core::cell::RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // `def` 那一刻读到的 `x`
    let x_value = vm.instance.new_int(7);
    vm.instance.dict_set(namespace, "x", x_value);
    // SAFETY: namespace 由本测试持有（常量表与帧各要一份）。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("定义 f 应当成功");

    let function = vm.instance.dict_get(namespace, "f").expect("命名空间里应当有 f");
    let call = |args: Vec<NonNull<Header>>| -> Option<i64> {
        let value = pyawa_core::executor::call_value(&vm.instance, function, &args, &[])
            .unwrap_or_else(|error| {
                panic!("调用 f 失败：{error:?}／pending={:?}", vm.pending_exception())
            });
        vm.instance.int_value(value)
    };

    // `f(1)`：`b` 省略 ⇒ 取 `def` 那一刻算出的默认值（7）
    assert_eq!(call(vec![vm.instance.new_int(1)]), Some(7), "省略的实参取默认值");
    // `f(1, 2)`：给了就覆盖
    assert_eq!(
        call(vec![vm.instance.new_int(1), vm.instance.new_int(2)]),
        Some(2),
        "给了实参就覆盖默认值"
    );
}

#[test]
fn star_parameters_collect_into_a_tuple_and_a_dict() {
    // 对照参照：`def f(*args): return args` ⇒ `f(1, 2)` 得到 `(1, 2)`；
    // `def f(**kw): return kw` ⇒ `f(x=1)` 得到 `{'x': 1}`。
    let vm = Vm::new();
    let module = compile(
        "def f(*args):\n    return args\ndef g(**kw):\n    return kw\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &module);
    let namespace = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            core::cell::RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("定义两个函数应当成功");

    let f = vm.instance.dict_get(namespace, "f").expect("有 f");
    let one = vm.instance.new_int(1);
    let two = vm.instance.new_int(2);
    let result = pyawa_core::executor::call_value(&vm.instance, f, &[one, two], &[])
        .expect("调用 f 应当成功");
    // SAFETY: 返回的是元组。
    let tuple = unsafe { &*result.as_ptr().cast::<pyawa_core::TupleObject>() };
    assert_eq!(tuple.len(), 2, "*args 收成元组");
    assert_eq!(vm.instance.int_value(tuple.item(0).unwrap()), Some(1));
    assert_eq!(vm.instance.int_value(tuple.item(1).unwrap()), Some(2));

    let g = vm.instance.dict_get(namespace, "g").expect("有 g");
    let key = vm.instance.new_str("x");
    let value = vm.instance.new_int(9);
    let result = pyawa_core::executor::call_value(&vm.instance, g, &[], &[(key, value)])
        .expect("调用 g 应当成功");
    // SAFETY: 返回的是 dict。
    let mapping = unsafe { &*result.as_ptr().cast::<DictObject>() };
    let stored = vm
        .instance
        .dict_get(result, "x")
        .expect("**kw 收成字典，键在");
    assert_eq!(vm.instance.int_value(stored), Some(9));
    assert_eq!(mapping.entries().len(), 1);
}

#[test]
fn keyword_only_parameters_take_defaults_and_reject_extra_positionals() {
    // 对照参照：`def f(a, *, c=3): return c` ⇒ `f(1)` 得 3、`f(1, c=9)` 得 9、
    // `f(1, 2)` 报错（`c` 只能按关键字给）。
    let vm = Vm::new();
    let module = compile(
        "def f(a, *, c=3):\n    return c\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &module);
    let namespace = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            core::cell::RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("定义 f 应当成功");

    let f = vm.instance.dict_get(namespace, "f").expect("有 f");
    let one = vm.instance.new_int(1);
    let result = pyawa_core::executor::call_value(&vm.instance, f, &[one], &[])
        .expect("调用 f(1) 应当成功");
    assert_eq!(vm.instance.int_value(result), Some(3), "仅关键字形参取默认值");

    let name = vm.instance.new_str("c");
    let nine = vm.instance.new_int(9);
    let result = pyawa_core::executor::call_value(&vm.instance, f, &[one], &[(name, nine)])
        .expect("调用 f(1, c=9) 应当成功");
    assert_eq!(vm.instance.int_value(result), Some(9), "关键字实参覆盖默认值");

    // 多给的**位置**实参必须报错（`c` 是仅关键字）
    let two = vm.instance.new_int(2);
    assert!(
        pyawa_core::executor::call_value(&vm.instance, f, &[one, two], &[]).is_err(),
        "仅关键字形参不接受位置实参"
    );
}
