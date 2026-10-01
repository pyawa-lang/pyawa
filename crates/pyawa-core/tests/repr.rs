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

use pyawa_core::{DictObject, Header, Value};

use common::{emit, op, Vm};

fn repr_of(vm: &Vm, result: &Value<'_>) -> String {
    let raw = result.as_header(&vm.instance).expect("应当是 str");
    // SAFETY: raw 是存活对象。
    vm.instance.object_str(raw)
}

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
        assert_eq!(vm.instance.object_repr(value), expected);
    }
}

#[test]
fn containers_repr_with_the_recursion_guard() {
    let vm = Vm::new();
    let list_type = vm.instance.type_named("list").unwrap();
    let empty = vm.instance.alloc(pyawa_core::ListObject::new(list_type, RefCell::new(Vec::new())));
    assert_eq!(
        vm.instance.object_repr(empty.as_ptr().cast::<Header>()),
        "[]"
    );
    let pair = vm.instance.alloc(pyawa_core::ListObject::new(
        list_type,
        RefCell::new(vec![vm.constant(1), vm.instance.new_str("a")]),
    ));
    assert_eq!(
        vm.instance.object_repr(pair.as_ptr().cast::<Header>()),
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
        vm.instance.object_repr(recursive.as_ptr().cast::<Header>()),
        "[[...]]"
    );

    // 空字典／空集合／单元素元组
    let dict = vm.instance.alloc(DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        RefCell::new(Vec::new()),
    ));
    assert_eq!(vm.instance.object_repr(dict.as_ptr().cast::<Header>()), "{}");
    let set = vm.instance.alloc(pyawa_core::SetObject::new(
        vm.instance.type_named("set").unwrap(),
        RefCell::new(Vec::new()),
    ));
    assert_eq!(vm.instance.object_repr(set.as_ptr().cast::<Header>()), "set()");
    let one = vm.instance.new_tuple(vec![vm.constant(1)]);
    assert_eq!(vm.instance.object_repr(one), "(1,)");
    let none_tuple = vm.instance.new_tuple(Vec::new());
    assert_eq!(vm.instance.object_repr(none_tuple), "()");
}

#[test]
fn types_functions_and_code_objects_repr() {
    let vm = Vm::new();
    let int_type = vm.instance.type_named("int").unwrap();
    assert_eq!(
        vm.instance.object_repr(int_type.cast::<Header>()),
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
        .object_repr(code.as_ptr().cast::<Header>());
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
    let text = vm.instance.object_str(raw);
    assert!(
        text.starts_with("<Plain object at 0x") && text.ends_with('>'),
        "默认形式由类型对象给出（TS §8），实际 {text}"
    );
    assert_eq!(text, vm.instance.object_repr(raw), "str 回退到 repr");
}
