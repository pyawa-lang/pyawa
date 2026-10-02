//! `OM-11` 的 `repr`／`str` 槽（语义见 `SPEC-type-system.md` §8 的表：`str` 省略时**回退到 `repr`**）。
//!
//! 形状**逐条实测**（本机 3.14.4）：
//!
//! ```text
//! int／bool／None   42 / True / None
//! float             1.0、0.1、1e+16（指数带符号）、inf、nan
//! str               'hi'（能用单引号就用；内容有单引号而无双引号时改用双引号）
//! 容器              [] () {} set() (1,) [1, 'a'] {'k': 1}
//! 自引用            [[...]]、{'k': {...}}
//! 类型              <class 'int'>
//! 生成器／函数      <generator object gen at 0x…>、<function demo at 0x…>
//! code              <code object demo at 0x…, file "…", line 1>
//! 绑定方法          <bound method m of <C object at 0x…>>（参照实现带 qualname `C.m`）
//! 异常              ValueError()、ValueError('x')
//! 迭代器            默认形式 `<list_iterator object at 0x…>` **正好**是对的，故不设槽
//! ```

mod common;

use core::cell::RefCell;

use pyawa_core::{DictObject, Header};

use common::{emit, op, Vm};

#[test]
fn scalars_repr_like_the_reference() {
    let vm = Vm::new();
    for (value, expected) in [
        (vm.constant(42), "42"),
        (vm.instance.own(vm.instance.singletons().boolean(true)).into_raw(),
         "True"),
        (vm.instance.own(vm.instance.singletons().none()).into_raw(), "None"),
        (vm.instance.new_str("hi"), "'hi'"),
        (vm.instance.new_str("it's"), "\"it's\""),
        (vm.instance.new_str(""), "''"),
        (vm.instance.alloc(pyawa_core::FloatObject::new(
            vm.instance.type_named("float").unwrap(), 1e16)).into_raw().cast::<Header>(),
         "1e+16"),
        (vm.instance.alloc(pyawa_core::FloatObject::new(
            vm.instance.type_named("float").unwrap(), 0.1)).into_raw().cast::<Header>(),
         "0.1"),
        (vm.instance.alloc(pyawa_core::FloatObject::new(
            vm.instance.type_named("float").unwrap(), f64::INFINITY)).into_raw().cast::<Header>(),
         "inf"),
    ] {
        assert_eq!(vm.instance.object_repr(value).expect("repr"), expected);
    }
}

#[test]
fn containers_repr_with_the_recursion_guard() {
    let vm = Vm::new();
    let list_type = vm.instance.type_named("list").unwrap();
    let empty = vm.instance.alloc(pyawa_core::ListObject::new(list_type, RefCell::new(Vec::new())));
    assert_eq!(
        vm.instance.object_repr(empty.as_ptr().cast::<Header>()).expect("repr"),
        "[]"
    );
    let pair = vm.instance.alloc(pyawa_core::ListObject::new(
        list_type,
        RefCell::new(vec![vm.constant(1), vm.instance.new_str("a")]),
    ));
    assert_eq!(
        vm.instance.object_repr(pair.as_ptr().cast::<Header>()).expect("repr"),
        "[1, 'a']"
    );
    // 自引用 ⇒ `[[...]]`（实测）
    let recursive = vm.instance.alloc(pyawa_core::ListObject::new(
        list_type,
        RefCell::new(Vec::new()),
    ));
    // 列表要**持有**那份引用（append 接手的是新引用）
    // SAFETY: recursive 由本测试持有，这里新增一份交给它自己。
    unsafe {
        vm.instance
            .incref_object(recursive.as_ptr().cast::<Header>().as_ptr())
    };
    recursive.get().append(recursive.as_ptr().cast::<Header>());
    assert_eq!(
        vm.instance.object_repr(recursive.as_ptr().cast::<Header>()).expect("repr"),
        "[[...]]"
    );

    // 空字典／空集合／单元素元组
    let dict = vm.instance.alloc(DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        RefCell::new(Vec::new()),
    ));
    assert_eq!(vm.instance.object_repr(dict.as_ptr().cast::<Header>()).expect("repr"), "{}");
    let set = vm.instance.alloc(pyawa_core::SetObject::new(
        vm.instance.type_named("set").unwrap(),
        RefCell::new(Vec::new()),
    ));
    assert_eq!(vm.instance.object_repr(set.as_ptr().cast::<Header>()).expect("repr"), "set()");
    let one = vm.instance.new_tuple(vec![vm.constant(1)]);
    assert_eq!(vm.instance.object_repr(one).expect("repr"), "(1,)");
    let none_tuple = vm.instance.new_tuple(Vec::new());
    assert_eq!(vm.instance.object_repr(none_tuple).expect("repr"), "()");
}

#[test]
fn types_functions_and_code_objects_repr() {
    let vm = Vm::new();
    let int_type = vm.instance.type_named("int").unwrap();
    assert_eq!(
        vm.instance.object_repr(int_type.cast::<Header>()).expect("repr"),
        "<class 'int'>"
    );

    let code = vm.code(
        2,
        0,
        emit(&[(op("RESUME"), 0), (op("RETURN_VALUE"), 0)]),
        Vec::new(),
    );
    let code_repr = vm
        .instance
        .object_repr(code.as_ptr().cast::<Header>())
        .expect("code 的 repr");
    assert!(
        code_repr.starts_with("<code object demo at 0x") && code_repr.contains("line 0"),
        "实际 {code_repr}"
    );
}

#[test]
fn str_falls_back_to_repr_when_the_slot_is_absent() {
    // `SPEC-type-system.md` §8 的表：`str` 省略时回退到 `repr`
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("Plain");
    let object = vm.instance.alloc(pyawa_core::AttributeObject::new(
        ty,
        RefCell::new(None),
    ));
    let raw = object.as_ptr().cast::<Header>();
    let text = vm.instance.object_str(raw).expect("str");
    assert!(
        text.starts_with("<Plain object at 0x") && text.ends_with('>'),
        "默认形式由类型对象给出（TS §8），实际 {text}"
    );
    assert_eq!(text, vm.instance.object_repr(raw).expect("repr"), "str 回退到 repr");
}

#[test]
fn a_bound_method_repr_uses_the_code_qualname() {
    // `BC-4`：绑定方法的 `repr` 取 **`co_qualname`**（参照：`<bound method C.m of …>`）。
    // 编译器已为模块级 `def` 产出 qualname（`f`）；类体方法那个 `C.m` 由类创建钩子补写，
    // 所以这里直接造一份 qualname ＝ `C.m` 的 code object，把这条链路钉住。
    let vm = Vm::new();
    let none = vm.instance.singletons().none();
    let bytes = emit(&[
        (op("RESUME"), 0),
        (op("LOAD_CONST"), 0),
        (op("RETURN_VALUE"), 0),
    ]);
    let code = vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "m",
        "C.m".to_owned(),
        "<t>".to_owned(),
        1,
        4,
        1,
        1,
        0,
        0,
        0,
        vec!["self".to_owned()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        bytes,
        Vec::new(),
        vec![Some(none)],
        Vec::new(),
    ));
    let function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").expect("function 已登记"),
        code.into_raw().cast::<Header>(),
        Vec::new(),
        None,
        RefCell::new(None),
            core::cell::RefCell::new(None),
            core::cell::RefCell::new(None)));
    let class = vm.instance.new_attribute_type("C");
    let receiver = vm
        .instance
        .alloc(pyawa_core::AttributeObject::new(class, RefCell::new(None)));
    let method = vm.instance.alloc(pyawa_core::MethodObject::new(
        vm.instance.type_named("method").expect("method 已登记"),
        function.into_raw().cast::<Header>(),
        receiver.into_raw().cast::<Header>(),
    ));
    let text = vm.instance.object_repr(method.into_raw().cast::<Header>()).expect("repr");
    assert!(
        text.starts_with("<bound method C.m of "),
        "绑定方法的 repr 必须用 co_qualname：{text}"
    );
}
