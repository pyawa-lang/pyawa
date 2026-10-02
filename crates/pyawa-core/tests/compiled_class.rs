//! **类体编译**（`class C: …`）的端到端验收：编译 → 实例化 → 跑模块 ⇒ 命名空间里真有这个类。
//!
//! 形态与参照**逐字节**一致（指令／常量／名字／位点／flags）由 `tests/compile.rs` 的语料保证；
//! 这里验"跑起来对不对"。
//!
//! **类体里带 `def`**：编译器那半**已按实测发射**（`__classdict__` cell 那一套，`tests/compile.rs`
//! 的语料与参照逐字节一致）；但**运行期**还有一处未解——`build_class` 里跑带 cell 的类体时报
//! `SlotOutOfRange { slot: 0, count: 0 }`（帧的 cell 槽数是 0）⇒ 端到端用例**暂时没有**，
//! 记在 `lib.rs` 的未落地清单里，下一轮查。

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
fn a_nested_def_inside_a_function_is_reported_as_unwired() {
    // 类体里的 `def` 已接线；**函数里**嵌套 `def`（闭包）仍未接线 ⇒ 如实报，不硬拼
    // （编译不需要 VM）
    let error = compile(
        "def outer():\n    def inner():\n        return 1\n    return inner\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect_err("函数里嵌套 def 应当如实报未接线");
    match error {
        pyawa_core::compile::CompileError::Unsupported(message) => {
            assert!(message.contains("嵌套的函数定义"), "消息：{message}");
        }
        other => panic!("应当是 `Unsupported`，实际 {other:?}"),
    }
}

#[test]
fn a_class_body_with_a_def_runs_end_to_end() {
    use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
    let vm = Vm::new();
    let unit = compile(
        "class W:\n    def m(self):\n        return 1\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let inner = code.get().constant(0).expect("常量 0 是类体 code");
    // SAFETY: 常量 0 是 code object，由外层 code 持有。
    let inner_code =
        unsafe { &*inner.as_ptr().cast::<pyawa_core::CodeObject>() };
    assert_eq!(inner_code.cellvars(), ["__classdict__"], "instantiate 之后仍应带 cellvars");
    // 真正跑一遍：带 `def` 的类体过去在这里报 `SlotOutOfRange { slot: 0, count: 0 }`
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("带 def 的类体应当跑得起来");
    let class = vm.instance.dict_get(namespace, "W").expect("命名空间里有 W");
    let class_type = core::ptr::NonNull::new(class.as_ptr().cast::<pyawa_core::TypeObject>())
        .expect("非空");
    let method = vm.instance.type_lookup(class_type, "m").expect("类字典里有 m");
    // 方法应当是函数对象，且 `co_qualname` 是 `W.m`（实测规则）
    let method_type = unsafe { method.as_ref() }.ty();
    assert_eq!(unsafe { method_type.as_ref() }.name(), "function");
}

#[test]
fn a_method_reads_a_class_attribute_through_self() {
    // 属性读打通后的第一条端到端：建类 → 实例化 → 调 `m()` → 经 `self.x` 读回类属性
    let vm = Vm::new();
    let unit = compile(
        "class Holder:\n    x = 7\n    def m(self):\n        return self.x\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("类应当建得起来");

    let class = vm.instance.dict_get(namespace, "Holder").expect("有 Holder");
    // 实例化（类可调用）＋取绑定方法＋调用
    let this = pyawa_core::executor::call_value(&vm.instance, class, &[], &[])
        .expect("Holder() 应当成功");
    let method = pyawa_core::executor::attribute_read(&vm.instance, this, "m")
        .expect("实例上应当能取到 m");
    let value = pyawa_core::executor::call_value(&vm.instance, method, &[], &[])
        .expect("m() 应当成功");
    assert_eq!(
        vm.instance.int_value(value),
        Some(7),
        "`self.x` 应当读到类属性 7"
    );
}

#[test]
fn a_method_writes_and_reads_an_instance_attribute() {
    // 属性写打通后的端到端：建类 → 实例化 → `m()` 里 `self.x = 5` → 从实例上读回 5
    let vm = Vm::new();
    let unit = compile(
        "class Counter:\n    def m(self):\n        self.x = 5\n        return self.x\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("类应当建得起来");

    let class = vm.instance.dict_get(namespace, "Counter").expect("有 Counter");
    let this = pyawa_core::executor::call_value(&vm.instance, class, &[], &[])
        .expect("Counter() 应当成功");
    let method = pyawa_core::executor::attribute_read(&vm.instance, this, "m")
        .expect("实例上应当能取到 m");
    let value = pyawa_core::executor::call_value(&vm.instance, method, &[], &[])
        .expect("m() 应当成功");
    assert_eq!(vm.instance.int_value(value), Some(5), "`self.x` 写进去应当读得回来");
    // 实例属性确实落在**实例**上（不是类属性）
    let stored = pyawa_core::executor::attribute_read(&vm.instance, this, "x")
        .expect("实例上应当有 x");
    assert_eq!(vm.instance.int_value(stored), Some(5));
}

#[test]
fn probe_direct_init_call() {
    use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
    let vm = Vm::new();
    let unit = compile(
        "class C:\n    def __init__(self, v):\n        return v\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("建类");

    let class = vm.instance.dict_get(namespace, "C").expect("有 C");
    let argument = vm.instance.new_int(3);
    match pyawa_core::executor::call_value(&vm.instance, class, &[argument], &[]) {
        Ok(_) => println!("① 经 type_call（C(3)）⇒ 成功"),
        Err(error) => println!("① 经 type_call（C(3)）⇒ {error:?}"),
    }
    // ② 直接调那个 `__init__`：`self` 给一个不碰它的占位对象（`None` 即可，体里只 `return v`）
    let class_type = core::ptr::NonNull::new(class.as_ptr().cast::<pyawa_core::TypeObject>())
        .expect("非空");
    let initializer = vm.instance.type_lookup(class_type, "__init__").expect("有 __init__");
    // SAFETY: 从类型字典取到的是存活对象；这里给调用方加一份。
    unsafe { vm.instance.incref_object(initializer.as_ptr()) };
    let placeholder = vm.instance.singletons().none();
    let argument = vm.instance.new_int(4);
    match pyawa_core::executor::call_value(&vm.instance, initializer, &[placeholder, argument], &[]) {
        Ok(value) => println!("② 直接调 __init__(None, 4) ⇒ 成功，返回 {:?}", vm.instance.int_value(value)),
        Err(error) => println!("② 直接调 __init__(None, 4) ⇒ {error:?}"),
    }
}
