//! **类体编译**（`class C: …`）的端到端验收：编译 → 实例化 → 跑模块 ⇒ 命名空间里真有这个类。
//!
//! 形态与参照**逐字节**一致（指令／常量／名字／位点／flags）由 `tests/compile.rs` 的语料保证；
//! 这里验"跑起来对不对"。
//!
//! **已知缺口**：类体里出现 `def` 时，参照还会铺 `__classdict__` cell（`MAKE_CELL`／`LOAD_LOCALS`／
//! `STORE_DEREF`／`__classdictcell__`）⇒ 那一支尚未接线，如实报 `Unsupported`（本文件最后一条钉住）。

mod common;

use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
use pyawa_core::Frame;

use common::Vm;

#[test]
fn a_compiled_class_lands_in_the_namespace_with_its_attributes() {
    let vm = Vm::new();
    let unit = compile(
        "class K:\n    x = 1\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // `Frame::for_code_with_namespace` **接手**一份命名空间引用 ⇒ 先补一份（`OM-16`）
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("跑得完");

    let class = vm.instance.dict_get(namespace, "K").expect("命名空间里有 K");
    // **类自己就是类型对象**（`Header::ty()` 取到的是元类型 `type`）⇒ 直接当 `TypeObject` 用
    let class_type = core::ptr::NonNull::new(class.as_ptr().cast::<pyawa_core::TypeObject>())
        .expect("类的指针非空");
    let attribute = vm
        .instance
        .type_lookup(class_type, "x")
        .expect("类字典里有 x");
    assert_eq!(vm.instance.int_value(attribute), Some(1));
}

#[test]
fn a_class_body_docstring_and_a_base_class_work() {
    let vm = Vm::new();
    // 基类 + 派生类：两个都在**同一个**命名空间里跑
    let unit = compile(
        "class Derived(Base):\n    \"ddoc\"\n    d = 2\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // 先在这个命名空间里定义 Base
    let base_unit = compile("class Base:\n    b = 1\n", "<t>", Mode::PurePython, CheckTier::Shallow)
        .expect("编得过");
    let base_code = instantiate(&vm.instance, &base_unit);
    // SAFETY: namespace 由本测试持有；每建一个帧都要补一份（帧接手新引用）
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &base_code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("Base 应当建得起来");
    // SAFETY: 同上。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("Derived 应当建得起来");

    let base = vm.instance.dict_get(namespace, "Base").expect("有 Base");
    let derived = vm.instance.dict_get(namespace, "Derived").expect("有 Derived");
    // 类自己就是类型对象
    let base_type = core::ptr::NonNull::new(base.as_ptr().cast::<pyawa_core::TypeObject>())
        .expect("非空");
    let derived_type =
        core::ptr::NonNull::new(derived.as_ptr().cast::<pyawa_core::TypeObject>())
            .expect("非空");
    assert!(
        vm.instance.is_subtype(derived_type, base_type),
        "Derived ⊂ Base"
    );
    // 类文档串：`Derived.__doc__` 是那个字符串（类体铺了 `STORE_NAME __doc__`）
    let doc = vm
        .instance
        .type_lookup(derived_type, "__doc__")
        .expect("类字典里有 __doc__");
    assert_eq!(vm.instance.text_value(doc).as_deref(), Some("ddoc"));
    // 划重点：两条都该有 `__qualname__`
    let qualname = vm
        .instance
        .type_lookup(derived_type, "__qualname__")
        .expect("类字典里有 __qualname__");
    assert_eq!(vm.instance.text_value(qualname).as_deref(), Some("Derived"));
}

#[test]
fn a_class_body_with_a_method_is_reported_as_unwired() {
    // 体里有 `def` 时参照会多铺 `__classdict__` cell ⇒ 本层**如实**报未接线，不硬拼
    // （编译不需要 VM）
    let error = compile(
        "class M:\n    def m(self):\n        return 1\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect_err("带方法的类体应当如实报未接线");
    match error {
        pyawa_core::compile::CompileError::Unsupported(message) => {
            assert!(message.contains("嵌套的函数定义"), "消息：{message}");
        }
        other => panic!("应当是 `Unsupported`，实际 {other:?}"),
    }
}
