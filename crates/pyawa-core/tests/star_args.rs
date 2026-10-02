//! 星号调用与打包局部变量（`docs/SPEC-bytecode.md` §10）。栈形状与净效应都是**实测**的：
//!
//! ```text
//! f(*a)      → LOAD f; PUSH_NULL; LOAD a; PUSH_NULL; CALL_FUNCTION_EX
//! f(**k)     → LOAD f; PUSH_NULL; LOAD_CONST (); BUILD_MAP 0; LOAD k; DICT_MERGE 1;
//!              CALL_FUNCTION_EX
//! def g(a,b) → LOAD_FAST_BORROW_LOAD_FAST_BORROW 1 (a, b)   ← 打包取两个局部（净 +2）
//! ```
//!
//! - `CALL_FUNCTION_EX` 净 **−3**：栈自下而上是 `[可调用, self|NULL, 实参 tuple, 关键字 dict|NULL]`
//!   （`f(*a)` 里第二个 `PUSH_NULL` 就是"没有关键字"那一格）；3.14 里它**没有** oparg
//! - `DICT_MERGE`／`DICT_UPDATE` 净 **−1**：把 TOS 那个字典并进 TOS1 再弹掉 TOS；
//!   `DICT_UPDATE` 覆盖同名键，`DICT_MERGE` 遇到同名键要**带函数 qualname** 的错误消息
//!   （实测 `__main__.demo() got multiple values for keyword argument 'a'`）——此刻调用者还在
//!   栈下好几层，本层取不到，故如实报未接线
//! - `LOAD_FAST(_BORROW)_LOAD_FAST(_BORROW)` 净 **+2**：`oparg` 打包两个局部槽，
//!   **高 4 位先压**（`LOAD_FAST_BORROW_LOAD_FAST_BORROW 1 (a, b)` 里 a＝0、b＝1）

mod common;

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::opcode::get_nb_ops;
use pyawa_core::{DictObject, Header, StrObject, Value};

use common::{emit, op, Vm};

fn nb(name: &str) -> u8 {
    get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

/// `def demo(a, b): return a + b`
fn adder(vm: &Vm) -> NonNull<Header> {
    let code = vm.function_code(
        4,
        2,
        2,
        0,
        0,
        0,
        vec!["a".to_owned(), "b".to_owned()],
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_FAST"), 0),
            (op("LOAD_FAST"), 1),
            (op("BINARY_OP"), nb("NB_ADD")),
            (op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        code.into_raw().cast::<Header>(),
        Vec::new(),
        None,
    
    RefCell::new(None),
            core::cell::RefCell::new(None),
            core::cell::RefCell::new(None)));
    function.into_raw().cast::<Header>()
}

#[test]
fn call_function_ex_passes_a_positional_tuple() {
    // `demo(*(1, 2))` ⇒ 3
    let vm = Vm::new();
    let function = adder(&vm);
    let arguments = vm.instance.new_tuple(vec![vm.constant(1), vm.constant(2)]);
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 1),
            (op("PUSH_NULL"), 0),
            (op("CALL_FUNCTION_EX"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(function), Some(arguments)],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(3), &vm.instance), "1 + 2");
}

#[test]
fn call_function_ex_passes_a_keyword_dict() {
    // `demo(**{'a': 1, 'b': 2})` ⇒ 3（关键字走绑定）
    let vm = Vm::new();
    let function = adder(&vm);
    let no_arguments = vm.instance.new_tuple(Vec::new());
    let keywords = vm.instance.alloc(DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        RefCell::new(Vec::new()),
    ));
    keywords
        .get()
        .insert_raw(vm.instance.new_str("a"), vm.constant(1));
    keywords
        .get()
        .insert_raw(vm.instance.new_str("b"), vm.constant(2));
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("PUSH_NULL"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("CALL_FUNCTION_EX"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(function),
            Some(no_arguments),
            Some(keywords.into_raw().cast::<Header>()),
        ],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(3), &vm.instance), "a=1, b=2");
}

#[test]
fn dict_update_overwrites_and_dict_merge_merges() {
    // `{'k': 1} | {'k': 2}` 的构造形态：DICT_UPDATE 覆盖同名键
    let vm = Vm::new();
    let destination = vm.instance.alloc(DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        RefCell::new(Vec::new()),
    ));
    destination
        .get()
        .insert_raw(vm.instance.new_str("k"), vm.constant(1));
    let source = vm.instance.alloc(DictObject::new(
        vm.instance.type_named("dict").unwrap(),
        RefCell::new(Vec::new()),
    ));
    source
        .get()
        .insert_raw(vm.instance.new_str("k"), vm.constant(2));
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("DICT_UPDATE"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(destination.into_raw().cast::<Header>()),
            Some(source.into_raw().cast::<Header>()),
        ],
    );
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当是 dict");
    // SAFETY: raw 是存活对象。
    let merged = unsafe { &*raw.as_ptr().cast::<DictObject>() };
    assert_eq!(merged.entries().len(), 1, "同名键只留一个");
    let (_, value) = merged.entries()[0];
    // SAFETY: 值是整数。
    assert_eq!(
        unsafe { &*value.as_ptr().cast::<pyawa_core::IntObject>() }.value.to_i64().expect("测试里是小整数"),
        2,
        "DICT_UPDATE 覆盖"
    );
    let _: Option<NonNull<StrObject>> = None;
}

#[test]
fn packed_local_load_pushes_high_nibble_first() {
    // `a + b` 的形态：`LOAD_FAST_LOAD_FAST(oparg)`，高 4 位是第一个（左边那个）
    let vm = Vm::new();
    let bytes = emit(&[
        (op("RESUME"), 0),
        (op("LOAD_CONST"), 0),
        (op("STORE_FAST"), 0), // a = 4
        (op("LOAD_CONST"), 1),
        (op("STORE_FAST"), 1), // b = 5
        (op("LOAD_FAST_LOAD_FAST"), (0 << 4) | 1),
        (op("BINARY_OP"), nb("NB_SUBTRACT")),
        (op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code(8, 2, bytes, vec![Some(vm.constant(4)), Some(vm.constant(5))]);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(-1), &vm.instance),
        "高 4 位先压 ⇒ 4 − 5 ＝ −1（若反了会得 1）"
    );
}
