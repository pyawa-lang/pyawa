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
