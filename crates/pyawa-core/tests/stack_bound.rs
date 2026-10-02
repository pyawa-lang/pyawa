//! **`co_stacksize` 的保守上界**（`BC-43`：值栈深度以它为上界；越界必须报错，禁止 UB）。
//!
//! 规格只要求"它是上界、必须被遵守"，**没有**要求与参照实现的精确值相等（`SPEC-bytecode.md`
//! §2.4 把它列为"元信息"）⇒ 本层给保守上界，差异登记在差异清单的 `DIV-8`。
//!
//! 这组用例盯两件事：①小的程序也有**够用**的容量（此前占位值曾让 `def` 一跑就 `StackOverflow`）；
//! ②程序越大、上界不减（保守性）。

mod common;

use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};

use common::Vm;

#[test]
fn a_compiled_unit_gets_a_usable_stack_bound() {
    let vm = Vm::new();
    // 带注解的 `def` 在模块层要用到 2 格（`__annotate__` ＋ 函数）——正是此前被占位值坑到的形状
    let unit = compile(
        "def f(a: int) -> int:\n    return a\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    // SAFETY: code 由本测试持有。
    let stacksize = unsafe { code.as_ptr().as_ref() }.stacksize();
    assert!(stacksize >= 2, "至少要容得下 `__annotate__` ＋ 函数：{stacksize}");

    // 真的跑一遍（模块体执行完不报 StackOverflow）
    let namespace = vm.instance.new_dict();
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("模块体应当跑得完");
}

#[test]
fn a_bigger_program_does_not_get_a_smaller_bound() {
    let vm = Vm::new();
    let small = compile("x = 1", "<t>", Mode::PurePython, CheckTier::Shallow, 0).expect("编得过");
    let big = compile(
        "x = f(1, 2, 3, 4, 5, 6)\ny = g(a, b, c)\nz = x\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
    )
    .expect("编得过");
    let small_code = instantiate(&vm.instance, &small);
    let big_code = instantiate(&vm.instance, &big);
    // SAFETY: 两个 code 都由本测试持有。
    let (small_size, big_size) = unsafe {
        (
            small_code.as_ptr().as_ref().stacksize(),
            big_code.as_ptr().as_ref().stacksize(),
        )
    };
    assert!(
        big_size >= small_size,
        "参数更多的调用不该得到更小的上界：{small_size} vs {big_size}"
    );
    assert!(small_size >= 4, "上界有下限（4）：{small_size}");
}
