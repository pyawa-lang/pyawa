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
        0,
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
        0,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // 先在这个命名空间里定义 Base
    let base_unit = compile("class Base:\n    b = 1\n", "<t>", Mode::PurePython, CheckTier::Shallow, 0)
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
fn a_nested_def_without_capture_leaves_cellvars_empty() {
    // **闭包分析的第一步（元数据）**：本层会算 `co_cellvars`（被内层 `def` 引用的外层局部）。
    // 这里先验**不误报**那一面：内层不引用任何外层局部 ⇒ `cellvars` 必须为空。
    //
    // 捕获型（`def inner(): return x`）此刻**编不过** —— 发射侧（`MAKE_CELL`／`STORE_DEREF`／
    // `SET_FUNCTION_ATTRIBUTE closure`）尚未接线，第 279 轮起就**如实报错**（不静默发错代码 ✗）；
    // 等发射侧接线后，那条会改成断言 `cellvars == ["x"]`（实测参照外层 `cellvars=('x',)`）。
    let unit = compile(
        "def outer():\n    x = 1\n    def inner():\n        return 1\n    return inner()\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
    )
    .expect("不捕获的嵌套 def 应当编得过");
    let outer = unit
        .constants
        .iter()
        .find_map(|constant| match constant {
            pyawa_core::compile::Constant::Code(code) => Some(code.as_ref()),
            _ => None,
        })
        .expect("模块常量里应当有 outer 的 code");
    assert!(
        outer.cellvars.is_empty(),
        "内层没有引用外层局部 ⇒ cellvars 应当为空，实际 {:?}",
        outer.cellvars
    );
}

#[test]
fn a_closure_carries_cells_and_freevars() {
    // **闭包（第 292 轮接线）**：实测参照 `def outer(): x = 1; def inner(): return x` ⇒
    // 外层 `cellvars=('x',)`、`varnames=('inner',)`、`MAKE_CELL 1`；内层 `freevars=('x',)`、
    // `COPY_FREE_VARS 1`。第 279 轮起这里原本断言"如实报错"，接线后改成正面断言。
    let unit = compile(
        "def outer():\n    x = 1\n    def inner():\n        return x\n    return inner()\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
    )
    .expect("闭包现在已经接线，应当编得过");
    let units: Vec<&pyawa_core::compile::CompiledUnit> = unit
        .constants
        .iter()
        .filter_map(|constant| match constant {
            pyawa_core::compile::Constant::Code(code) => Some(code.as_ref()),
            _ => None,
        })
        .collect();
    let outer = units.first().expect("外层单元");
    assert_eq!(outer.cellvars, vec!["x".to_owned()], "外层 cellvars");
    assert_eq!(outer.varnames, vec!["inner".to_owned()], "cell 名要从 varnames 移出");
    let inner = outer
        .constants
        .iter()
        .find_map(|constant| match constant {
            pyawa_core::compile::Constant::Code(code) => Some(code.as_ref()),
            _ => None,
        })
        .expect("内层单元（在外层单元的常量表里）");
    assert_eq!(inner.freevars, vec!["x".to_owned()], "内层 freevars");
}
#[test]
fn a_nested_def_inside_a_function_compiles() {
    // 类体里的 `def` 已接线；**函数里**嵌套 `def` 于第 278 轮接线（无闭包）⇒ 不再报未接线。
    // **闭包**（内层引用外层局部）尚未接线、且**未拦截**：那类名字会按全局发（运行期 `NameError`）。
    let unit = compile(
        "def outer():\n    def inner():\n        return 1\n    return inner\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
    )
    .expect("函数里嵌套 def 现在应当编得过");
    // `unit` 是**模块**单元；`outer` 是它的一个 code 常量 —— 断言要落在 `outer` 上
    // （外层把内层函数名记成**局部**，实测参照 `co_varnames = ('inner',)`）
    let outer = unit
        .constants
        .iter()
        .find_map(|constant| match constant {
            pyawa_core::compile::Constant::Code(code) => Some(code.as_ref()),
            _ => None,
        })
        .expect("模块常量里应当有 `outer` 的 code");
    assert!(
        outer.varnames.iter().any(|name| name == "inner"),
        "`outer` 应当把 inner 记成局部，实际 varnames = {:?}",
        outer.varnames
    );
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
        0,
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
        0,
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
    let this = pyawa_core::executor::call::call_value(&vm.instance, class, &[], &[])
        .expect("Holder() 应当成功");
    let method = pyawa_core::executor::attribute::attribute_read(&vm.instance, this, "m")
        .expect("实例上应当能取到 m");
    let value = pyawa_core::executor::call::call_value(&vm.instance, method, &[], &[])
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
        0,
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
    let this = pyawa_core::executor::call::call_value(&vm.instance, class, &[], &[])
        .expect("Counter() 应当成功");
    let method = pyawa_core::executor::attribute::attribute_read(&vm.instance, this, "m")
        .expect("实例上应当能取到 m");
    let value = pyawa_core::executor::call::call_value(&vm.instance, method, &[], &[])
        .expect("m() 应当成功");
    assert_eq!(vm.instance.int_value(value), Some(5), "`self.x` 写进去应当读得回来");
    // 实例属性确实落在**实例**上（不是类属性）
    let stored = pyawa_core::executor::attribute::attribute_read(&vm.instance, this, "x")
        .expect("实例上应当有 x");
    assert_eq!(vm.instance.int_value(stored), Some(5));
}

#[test]
fn an_init_takes_an_argument_and_stores_it() {
    // 第 100 轮抓到根因（函数缺隐式返回），修好后这条链跑通：`C(9)` ⇒ `__init__(self, v)` ⇒ `self.v = v`
    let vm = Vm::new();
    let unit = compile(
        "class P:\n    def __init__(self, v):\n        self.v = v\n    def get(self):\n        return self.v\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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

    let class = vm.instance.dict_get(namespace, "P").expect("有 P");
    let argument = vm.instance.new_int(9);
    let this = pyawa_core::executor::call::call_value(&vm.instance, class, &[argument], &[])
        .expect("P(9) 应当成功（要调 __init__）");
    let stored = pyawa_core::executor::attribute::attribute_read(&vm.instance, this, "v")
        .expect("实例上应当有 v");
    assert_eq!(vm.instance.int_value(stored), Some(9), "`__init__` 应当把 9 存进 self.v");
    let method = pyawa_core::executor::attribute::attribute_read(&vm.instance, this, "get")
        .expect("实例上应当能取到 get");
    let value = pyawa_core::executor::call::call_value(&vm.instance, method, &[], &[])
        .expect("get() 应当成功");
    assert_eq!(vm.instance.int_value(value), Some(9), "`get()` 应当读回 9");
}

#[test]
fn static_attributes_are_collected_from_methods() {
    // `__static_attributes__` 的静态收集（实测：字母序去重；只读不算；嵌套里的赋值也算）
    let vm = Vm::new();
    let unit = compile(
        "class S:\n    def m(self):\n        self.b = 2\n        self.a = 1\n    def n(self):\n        self.c = 3\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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

    let class = vm.instance.dict_get(namespace, "S").expect("有 S");
    let class_type = core::ptr::NonNull::new(class.as_ptr().cast::<pyawa_core::TypeObject>())
        .expect("非空");
    let attributes = vm
        .instance
        .type_lookup(class_type, "__static_attributes__")
        .expect("类字典里有 __static_attributes__");
    // SAFETY: 是元组。
    let tuple = unsafe { &*attributes.as_ptr().cast::<pyawa_core::TupleObject>() };
    let names: Vec<String> = (0..tuple.len())
        .map(|index| vm.instance.text_value(tuple.item(index).unwrap()).unwrap())
        .collect();
    assert_eq!(names, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
}

#[test]
fn a_function_without_return_gives_none() {
    // 第 100 轮抓到的缺陷就是这一格：**能落到末尾**的函数以前漏发 `LOAD_CONST None; RETURN_VALUE`
    // ⇒ 调用时 `FellOffEnd`。这里从**运行期**钉住：`f()` 必须正常返回 `None`。
    let vm = Vm::new();
    let unit = compile(
        "def f(a):\n    b = a\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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
    pyawa_core::execute(&vm.instance, &frame).expect("模块应当跑得起来");

    let function = vm.instance.dict_get(namespace, "f").expect("有 f");
    let argument = vm.instance.new_int(3);
    let value = pyawa_core::executor::call::call_value(&vm.instance, function, &[argument], &[])
        .expect("落空到末尾的函数应当正常返回");
    assert_eq!(
        value,
        vm.instance.singletons().none(),
        "落空到末尾的函数应当返回 None"
    );
}

#[test]
fn set_name_is_called_for_own_namespace_items() {
    // 实测：对本类命名空间按**插入序**调 `__set_name__(类对象, 属性名)`；继承项不再调；抛错传播
    let vm = Vm::new();
    let unit = compile(
        "class D:\n    def __set_name__(self, owner, name):\n        self.owner = owner\n        self.name = name\nclass C:\n    d = D()\n    e = D()\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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

    let class = vm.instance.dict_get(namespace, "C").expect("有 C");
    for name in ["d", "e"] {
        let class_type = core::ptr::NonNull::new(class.as_ptr().cast::<pyawa_core::TypeObject>())
            .expect("非空");
        let holder = vm.instance.type_lookup(class_type, name).expect("类字典里有该项");
        let stored = pyawa_core::executor::attribute::attribute_read(&vm.instance, holder, "name")
            .expect("__set_name__ 应当把名字存进 self.name");
        assert_eq!(vm.instance.text_value(stored).as_deref(), Some(name));
    }
}

#[test]
fn set_name_is_not_recalled_for_inherited_items() {
    // 实测：`__set_name__` 只对**本类自己**命名空间的项调一次；继承来的项**不再调**
    // ⇒ 判据：`Base.b` 那个描述符的 `owner` 应当仍是 `Base`（若被重调，会被覆写成 `Sub`）
    let vm = Vm::new();
    let unit = compile(
        "class D:\n    def __set_name__(self, owner, name):\n        self.owner = owner\n        self.name = name\nclass Base:\n    b = D()\nclass Sub(Base):\n    x = 1\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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
    pyawa_core::execute(&vm.instance, &frame).expect("三个类都应当建得起来");

    let base = vm.instance.dict_get(namespace, "Base").expect("有 Base");
    let base_type = core::ptr::NonNull::new(base.as_ptr().cast::<pyawa_core::TypeObject>())
        .expect("非空");
    let holder = vm.instance.type_lookup(base_type, "b").expect("Base 里有 b");
    let owner = pyawa_core::executor::attribute::attribute_read(&vm.instance, holder, "owner")
        .expect("__set_name__ 应当存过 owner");
    assert_eq!(owner, base, "`owner` 应当还是 Base（继承项不该被重调）");
}

#[test]
fn set_name_errors_propagate() {
    // 实测：`__set_name__` 抛的错**原样传播**出类创建（不包装、不吞掉）
    let vm = Vm::new();
    let unit = compile(
        // `raise` 语句本层还没接线 ⇒ 用"读未定义的全局名"来抛（`NameError`），
        // 效果一样：`__set_name__` 里抛出的用户异常必须**原样传播**出类创建
        "class D:\n    def __set_name__(self, owner, name):\n        return missing_global\nclass C:\n    d = D()\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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
    let outcome = pyawa_core::execute(&vm.instance, &frame);
    match outcome {
        Err(pyawa_core::ExecError::Raised { exception }) => {
            // 传播的应当是**用户那个** ValueError（类名对得上就行）
            let ty = unsafe { exception.as_ref() }.ty();
            assert_eq!(unsafe { ty.as_ref() }.name(), "NameError");
        }
        Ok(_) => panic!("应当把 `__set_name__` 的异常抛出来，却正常跑完了"),
        Err(other) => panic!("应当是用户那个 `Raised`，实际是别的错误：{other:?}"),
    }
}

#[test]
fn raise_propagates_the_user_exception() {
    // `raise ValueError(1)` 端到端：执行器早就有 `RAISE_VARARGS`，编译器那半在第 128 轮补上。
    // **测试实例要装内建**（否则 `ValueError` 解析不到，会先报 `NameError` —— 第 111 轮踩过）。
    let vm = Vm::new();
    let builtins = vm.instance.new_dict();
    let value_error = vm
        .instance
        .type_value(vm.instance.type_named("ValueError").expect("ValueError 在内建表里"));
    vm.instance.dict_set(builtins, "ValueError", value_error);
    vm.instance.set_builtins(Some(builtins));

    let unit = compile(
        "raise ValueError(1)\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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
    match pyawa_core::execute(&vm.instance, &frame) {
        Err(pyawa_core::ExecError::Raised { exception }) => {
            let ty = unsafe { exception.as_ref() }.ty();
            assert_eq!(unsafe { ty.as_ref() }.name(), "ValueError");
        }
        Ok(_) => panic!("`raise` 应当把异常抛出来"),
        Err(other) => panic!("应当是 `Raised`，实际 {other:?}"),
    }
}

#[test]
fn none_and_bool_literals_resolve_at_runtime() {
    // `None`／`True`／`False` 现在是**字面量常量**（第 131／132 轮）⇒ 不再当名字读
    // （以前会报 `NameError`）。这里从**运行期**钉住：它们绑到的是**单例**。
    let vm = Vm::new();
    let unit = compile(
        "x = None\ny = True\nz = False\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
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
    pyawa_core::execute(&vm.instance, &frame).expect("三个字面量都应当跑得起来");

    let x = vm.instance.dict_get(namespace, "x").expect("有 x");
    assert_eq!(x, vm.instance.singletons().none(), "`None` 应当绑到单例");
    let y = vm.instance.dict_get(namespace, "y").expect("有 y");
    assert_eq!(y, vm.instance.singletons().boolean(true), "`True` 应当绑到单例");
    let z = vm.instance.dict_get(namespace, "z").expect("有 z");
    assert_eq!(z, vm.instance.singletons().boolean(false), "`False` 应当绑到单例");
    assert_eq!(vm.instance.bool_value(y), Some(true));
    assert_eq!(vm.instance.bool_value(z), Some(false));
}
