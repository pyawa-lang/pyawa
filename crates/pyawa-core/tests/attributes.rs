//! 属性与下标族的**属性**部分（`docs/SPEC-bytecode.md` §10；`OM-11` 的 `getattr` 槽位随类型系统接线）。
//!
//! `LOAD_ATTR` 一族的 oparg 编码是**实测**的：
//!
//! - `LOAD_ATTR`：名字下标 ＝ `oparg >> 1`，**低位 ＝ 取方法**（`dis` 的 argrepr 显示 `+ NULL|self`）
//! - `STORE_ATTR`：名字下标 ＝ `oparg >> 1`；栈是 `[值, 对象]`（**对象在 TOS**）
//! - `DELETE_ATTR`：名字下标 ＝ `oparg`（**不移位**）；栈是 `[对象]`
//!
//! 属性查找顺序（本层口径）：① 实例字典（函数是非数据描述符，故实例属性**遮住**方法）
//! ② 类型字典沿 MRO（查到函数就是取方法） ③ 都没有 ⇒ **真 `AttributeError`**
//! （消息照参照实现：`'int' object has no attribute 'nope'`）。
//!
//! **未接线**：`obj.method`（**不调用**、只取值）要 `method` 类型——`TS-42` 把它排在后面的阶梯；
//! `LOAD_SUPER_ATTR` 要 `super()` 的 `__class__` cell。

mod common;

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{
    AttributeObject, ExecError, FunctionObject, Header, StrObject, TypeObject, Value,
};

use common::{emit, op, Vm};

/// 造一个"返回常量"的方法：`def m(self, *ignored): return <const>`
fn method(vm: &Vm, argcount: usize, varnames: Vec<String>, constant: i64) -> pyawa_core::Owned<'_, pyawa_core::CodeObject> {
    let consts = vec![Some(vm.constant(constant))];
    vm.function_code(
        2,
        argcount,
        argcount,
        0,
        0,
        0,
        varnames,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    )
}

/// 把 code object 包成函数对象并把那份引用交给 `owner`。
fn make_function(vm: &Vm, code: pyawa_core::Owned<'_, pyawa_core::CodeObject>) -> NonNull<Header> {
    let function = vm.instance.alloc(FunctionObject::new(
        vm.instance.type_named("function").unwrap(),
        code.into_raw().cast::<Header>(),
        Vec::new(),
        None,
    ));
    function.into_raw().cast::<Header>()
}

fn new_object(vm: &Vm, ty: NonNull<TypeObject>) -> NonNull<Header> {
    vm.instance
        .alloc(AttributeObject::new(ty, RefCell::new(None)))
        .into_raw()
        .cast::<Header>()
}

#[test]
fn method_call_binds_self() {
    // class C: def add(self, n): return n + 1   ——  然后 obj.add(2)
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    let add_code = method(&vm, 2, vec!["self".to_owned(), "n".to_owned()], 1);
    let function = make_function(&vm, add_code);
    assert!(
        vm.instance.set_type_attribute(ty, "add", function).is_none(),
        "类型字典里原来没有 add"
    );

    let object = new_object(&vm, ty);
    let consts = vec![Some(object), Some(vm.constant(2))];
    let code = vm.code_with_names(
        6,
        0,
        0,
        Vec::new(),
        vec!["add".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            // 名字下标 0 ＋ 低位 1（取方法）
            (op("LOAD_ATTR"), 1),
            (op("LOAD_CONST"), 1),
            (op("CALL"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(1), &vm.instance),
        "方法体返回常量 1"
    );
}

#[test]
fn instance_dict_stores_shadows_and_deletes() {
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    // 类型上挂一个同名方法，用来验证"实例属性遮住方法"
    let shadowed = method(&vm, 1, vec!["self".to_owned()], 99);
    let function = make_function(&vm, shadowed);
    vm.instance.set_type_attribute(ty, "f", function);

    // obj.x = 5（每段程序各有自己的对象：常量表持有引用，OM-40）
    let object = new_object(&vm, ty);
    let consts = vec![Some(object), Some(vm.constant(5))];
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["x".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 0),
            // 名字下标 0 ⇒ STORE_ATTR 的 oparg ＝ 0
            (op("STORE_ATTR"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(5), &vm.instance));

    // obj.f = 7 之后 obj.f 取到 7（实例字典优先于类型上的方法）
    let object = new_object(&vm, ty);
    let consts = vec![Some(object), Some(vm.constant(7))];
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["f".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 0),
            (op("STORE_ATTR"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(7), &vm.instance),
        "实例属性遮住类型上的同名方法"
    );

    // 同一条程序里：先设 obj.f = 7、再 del，然后取 ⇒ 落到类型上的方法
    let object = new_object(&vm, ty);
    let consts = vec![Some(object), Some(vm.constant(7))];
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["f".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 0),
            (op("STORE_ATTR"), 0),
            (op("LOAD_CONST"), 0),
            (op("DELETE_ATTR"), 0),
            (op("LOAD_CONST"), 0),
            // 取方法形态（低位 1）：栈上留 `[函数, self]`，TOS 是 self
            (op("LOAD_ATTR"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当有对象");
    // SAFETY: raw 是存活对象。
    assert_eq!(
        unsafe { raw.as_ref() }.ty(),
        ty,
        "取方法形态的 TOS 是绑上去的 self"
    );
}

#[test]
fn missing_attribute_is_reported() {
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    let object = new_object(&vm, ty);
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["nope".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(object)],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception().map(|(name, _)| name),
        Some("AttributeError".to_owned())
    );
}

#[test]
fn deleting_missing_attribute_is_reported() {
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    let object = new_object(&vm, ty);
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["nope".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("DELETE_ATTR"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(object)],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception().map(|(name, _)| name),
        Some("AttributeError".to_owned())
    );
}

#[test]
fn type_lookup_walks_the_mro() {
    // class Base: def m(self): return 7
    // class Derived(Base): pass
    // Derived().m() 要能沿 MRO 找到
    let vm = Vm::new();
    let base = vm.instance.new_attribute_type("Base");
    let method_code = method(&vm, 1, vec!["self".to_owned()], 7);
    let function = make_function(&vm, method_code);
    vm.instance.set_type_attribute(base, "m", function);

    let derived = vm.instance.new_attribute_type("Derived");
    assert!(
        vm.instance.register_bases(derived, vec![base]).is_some(),
        "C3 应当能算出来"
    );

    let object = new_object(&vm, derived);
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["m".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 1), // 取方法形态
            (op("CALL"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(object)],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(7), &vm.instance));
}

#[test]
fn bound_method_without_the_flag_is_an_object() {
    // 只取值（不调用）需要 `method` 类型——TS-42 排在后面的阶梯，故这里如实报 Unsupported
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    let method_code = method(&vm, 1, vec!["self".to_owned()], 1);
    let function = make_function(&vm, method_code);
    vm.instance.set_type_attribute(ty, "m", function);
    let object = new_object(&vm, ty);

    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["m".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_ATTR"), 0), // **不带**方法位
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(object)],
    );
    // `obj.m`（**不调用**）产出**绑定方法对象**：函数 ＋ 绑定的 `self`
    let result = vm.run(&code).unwrap();
    let raw = result.as_header(&vm.instance).expect("应当是绑定方法对象");
    // SAFETY: raw 是存活对象。
    assert_eq!(
        unsafe { raw.as_ref() }.ty(),
        vm.instance.type_named("method").unwrap(),
        "类型是 method"
    );
    // SAFETY: 类型身份已确认。
    let bound = unsafe { &*raw.as_ptr().cast::<pyawa_core::MethodObject>() };
    assert_eq!(bound.this(), object, "绑的就是那个实例");
}

#[test]
fn deleted_attributes_release_their_values() {
    // 属性值归对象所有：删掉属性 ⇒ 那份引用归还（引用计数回到记之前）
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("C");
    let object = new_object(&vm, ty);
    let str_type = vm.instance.singletons().str_type();
    let text = vm.instance.alloc(StrObject::new(str_type, "hi".to_owned()));
    let text_header = text.as_ptr().cast::<Header>();
    // 常量表也会持有它 ⇒ 必须**新增一份引用**（OM-16）；否则两边都会去释放同一份
    // SAFETY: text 由本测试的守卫保活。
    unsafe { vm.instance.incref_object(text_header.as_ptr()) };
    let before = text.refcount();

    let consts = vec![Some(object), Some(text_header)];
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["x".to_owned()],
        emit(&[
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 0),
            (op("STORE_ATTR"), 0),
            (op("LOAD_CONST"), 0),
            (op("DELETE_ATTR"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    vm.run(&code).unwrap();
    assert_eq!(
        text.refcount(),
        before,
        "存进属性又删掉之后，那份引用要还回来"
    );
}
