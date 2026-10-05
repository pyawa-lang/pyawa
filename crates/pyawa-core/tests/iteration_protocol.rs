//! **迭代协议**（`OM-11` 的 `iter` 槽位那一路）：`GET_ITER` 走 `__iter__`、迭代推进走 `__next__`。
//!
//! 此前只有 tuple／list／dict／set／str 的内建迭代器；这一组用例把"用户类型也能被 `for`"
//! 这条路径钉住，兼验没有协议时的**实测**消息（`'int' object is not iterable`）。
//!
//! 类用核心的安全面搭（`new_attribute_type` ＋ `set_type_attribute`），方法用**手写字节码**的
//! `FunctionObject`——属性通道只对 `function` 绑 `self`，原生函数不会被绑定。

mod common;

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{DictObject, Frame, Header, Value};
use common::{assemble, op, Item, Vm};

/// 造一个"可迭代对象"：`__iter__` 返回 self，`__next__` 从**帧全局**的计数里取值，
/// 到 3 就抛 `StopIteration`（计数放全局 ⇒ 每个用例各用一份，互不干扰）。
fn counting_class(vm: &Vm, globals: NonNull<Header>) -> NonNull<Header> {
    let none = vm.instance.singletons().none();

    // `def __iter__(self): return self`
    let iter_code = vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "__iter__",
        "Counter.__iter__".to_owned(),
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
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_FAST_BORROW"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![Some(none)],
        Vec::new(),
    ));
    // `def __next__(self):`
    //     if counter >= 3: raise StopIteration     # COMPARE_OP(>=) → TO_BOOL → POP_JUMP_IF_TRUE
    //     counter = counter + 1                    # STORE_GLOBAL 0
    //     return counter
    let next_code = vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "__next__",
        "Counter.__next__".to_owned(),
        "<t>".to_owned(),
        1,
        8,
        1,
        1,
        0,
        0,
        0,
        vec!["self".to_owned()],
        vec!["counter".to_owned(), "StopIteration".to_owned()],
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_GLOBAL"), 0),
            Item::Instr(op("LOAD_SMALL_INT"), 3),
            // `>=` 的 oparg（`COMPARE_OP` 六元组里第 6 个 ⇒ 5 << 5 ｜ 提示位 12 ＝ 172）
            Item::Instr(op("COMPARE_OP"), 172),
            Item::Instr(op("TO_BOOL"), 0),
            Item::Jump(op("POP_JUMP_IF_TRUE"), "raise"),
            Item::Instr(op("NOT_TAKEN"), 0),
            Item::Instr(op("LOAD_GLOBAL"), 0),
            Item::Instr(op("LOAD_SMALL_INT"), 1),
            Item::Instr(op("BINARY_OP"), 0),
            Item::Instr(op("STORE_GLOBAL"), 0),
            Item::Instr(op("LOAD_GLOBAL"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
            Item::Label("raise"),
            // 3.14 的 `LOAD_GLOBAL` oparg **带低位标志**（`名字下标 << 1 | 压 NULL`）⇒
            // 名字下标 1 要写 2（写 1 会取到下标 0 再压一个 NULL）
            Item::Instr(op("LOAD_GLOBAL"), 2),
            Item::Instr(op("RAISE_VARARGS"), 1),
        ]),
        Vec::new(),
        vec![Some(none)],
        Vec::new(),
    ));
    // 两个方法都包成 `function`（属性通道只对 function 绑 self）
    // `FunctionObject::new` **接手** `__globals__` 的那份引用（`OM-16`）⇒ 两个函数各要一份
    let iter_function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").expect("function 已登记"),
        iter_code.into_raw().cast::<Header>(),
        Vec::new(),
        None,
        RefCell::new(Some(vm.instance.own(globals).into_raw())),
            core::cell::RefCell::new(Vec::new()),
            core::cell::RefCell::new(None),
            core::cell::RefCell::new(None)));
    let next_function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").expect("function 已登记"),
        next_code.into_raw().cast::<Header>(),
        Vec::new(),
        None,
        RefCell::new(Some(vm.instance.own(globals).into_raw())),
            core::cell::RefCell::new(Vec::new()),
            core::cell::RefCell::new(None),
            core::cell::RefCell::new(None)));
    let ty = vm.instance.new_attribute_type("Counter");
    let iter_name = iter_function.into_raw().cast::<Header>();
    vm.instance.set_type_attribute(ty, "__iter__", iter_name);
    let next_name = next_function.into_raw().cast::<Header>();
    vm.instance.set_type_attribute(ty, "__next__", next_name);
    let instance = vm.instance.alloc(pyawa_core::AttributeObject::new(
        ty,
        RefCell::new(None),
    ));
    instance.into_raw().cast::<Header>()
}

fn new_globals(vm: &Vm, counter: i64) -> NonNull<Header> {
    let globals = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    let value = vm.instance.new_int(counter);
    vm.instance.dict_set(globals, "counter", value);
    let stop = vm.instance.type_value(
        vm.instance
            .type_named("StopIteration")
            .expect("StopIteration 在内建表里"),
    );
    vm.instance.dict_set(globals, "StopIteration", stop);
    globals
}

/// 跑一小段程序、返回结果（用给定的命名空间）。
fn run_in<'a>(
    vm: &'a Vm,
    namespace: NonNull<Header>,
    program: Vec<Item>,
    names: Vec<String>,
    consts: Vec<Option<NonNull<Header>>>,
) -> Result<Value<'a>, pyawa_core::ExecError> {
    let code = vm.code_with_names(8, 0, 0, Vec::new(), names, assemble(&program), consts);
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame)? {
        pyawa_core::ExecOutcome::Returned(value) => Ok(value),
        pyawa_core::ExecOutcome::Yielded(_) => panic!("顶层程序不该 yield"),
    }
}

#[test]
fn get_iter_uses_the_dunder_protocol() {
    let vm = Vm::new();
    let globals = new_globals(&vm, 0);
    // 命名空间就借用这份 globals
    let object = counting_class(&vm, globals);
    let iterator = run_in(
        &vm,
        globals,
        vec![
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("GET_ITER"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
        Vec::new(),
        vec![Some(object)],
    )
    .expect("GET_ITER 应当走 __iter__ 成功");
    let iterator = iterator.as_header(&vm.instance).expect("有返回值");
    assert_eq!(iterator, object, "__iter__ 返回 self ⇒ 迭代器就是它自己");

    // 依次推进：1、2、3（`__next__` 先自增再返回），然后耗尽（抛 StopIteration ⇒ `None`）
    for expected in [1, 2, 3] {
        let item = pyawa_core::executor::runtime::advance(&vm.instance, iterator)
            .expect("推进应当成功")
            .expect("还没耗尽");
        assert_eq!(vm.instance.int_value(item), Some(expected));
    }
    match pyawa_core::executor::runtime::advance(&vm.instance, iterator) {
        Ok(None) => {}
        other => {
            let pending = vm.pending_exception();
            panic!("StopIteration ⇒ 耗尽（None），实际 {other:?}／pending={pending:?}");
        }
    }
}

#[test]
fn a_value_without_iter_gives_the_measured_message() {
    let vm = Vm::new();
    let namespace = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    let number = vm.instance.new_int(5);
    let error = run_in(
        &vm,
        namespace,
        vec![
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("GET_ITER"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
        Vec::new(),
        vec![Some(number)],
    )
    .expect_err("没有 __iter__ 的整数不能被迭代");
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "TypeError");
    assert_eq!(
        message.as_deref(),
        Some("'int' object is not iterable"),
        "实测消息"
    );
    let _ = error;
}

#[test]
fn an_iterator_is_its_own_iterator() {
    // 实测：`iter(itertools.count()) is` 它自己 ⇒ `GET_ITER` 对**迭代器**原样返回（不再包一层）
    let vm = Vm::new();
    let iterator = vm.instance.new_count_iterator(3, 2);
    let namespace = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    let result = run_in(
        &vm,
        namespace,
        vec![
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("GET_ITER"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
        Vec::new(),
        vec![Some(iterator)],
    )
    .expect("迭代器的 GET_ITER 应当成功");
    let returned = result.as_header(&vm.instance).expect("有返回值");
    assert_eq!(returned, iterator, "iter(迭代器) 就是它自己");
    // 顺带验一下载荷：3、5、7…
    let first = pyawa_core::executor::runtime::advance(&vm.instance, iterator)
        .expect("推进应当成功")
        .expect("count 无限");
    assert_eq!(vm.instance.int_value(first), Some(3));
}
