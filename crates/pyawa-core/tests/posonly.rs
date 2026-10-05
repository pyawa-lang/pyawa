//! **仅位置形参**（`def f(a, /, b)`）：编译器只改元数据（`co_posonlyargcount`，**不产生指令**），
//! 绑定规则由 `bind_arguments` 负责——关键字传给仅位置形参要按参照**实测**的消息报错。

mod common;

use core::ptr::NonNull;

use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
use pyawa_core::{DictObject, Frame, Header};

use common::Vm;

#[test]
fn positional_only_parameters_reject_keyword_arguments() {
    let vm = Vm::new();
    let module = compile(
        "def f(a, /, b):\n    return a\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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

    // `f(1, b=2)` ⇒ 正常
    let one = vm.instance.new_int(1);
    let key = vm.instance.new_str("b");
    let two = vm.instance.new_int(2);
    let result = pyawa_core::executor::call::call_value(&vm.instance, f, &[one], &[(key, two)])
        .expect("`b` 可以按关键字给");
    assert_eq!(vm.instance.int_value(result), Some(1));

    // `f(1, 2)` ⇒ 正常（都按位置给）
    let two = vm.instance.new_int(2);
    let result = pyawa_core::executor::call::call_value(&vm.instance, f, &[one, two], &[])
        .expect("两个位置实参应当成功");
    assert_eq!(vm.instance.int_value(result), Some(1));

    // `f(a=1, b=2)` ⇒ `a` 是仅位置形参，按关键字给要报错（消息照参照实测）
    let key = vm.instance.new_str("a");
    let one_again = vm.instance.new_int(1);
    let key_b = vm.instance.new_str("b");
    let two_again = vm.instance.new_int(2);
    let error = pyawa_core::executor::call::call_value(
        &vm.instance,
        f,
        &[],
        &[(key, one_again), (key_b, two_again)],
    )
    .expect_err("仅位置形参不能按关键字给");
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(&vm.instance)
                .unwrap_or_default();
            assert_eq!(
                message,
                "f() got some positional-only arguments passed as keyword arguments: 'a'"
            );
        }
        other => panic!("应当是脚本异常，实际 {other:?}"),
    }
    let _: Option<NonNull<Header>> = None;
}
