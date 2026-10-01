//! `OM-14` 的"**另行挂载**的实例字典"：布局固定的实例（宿主类型、`list`／`dict` 一类的子类）
//! 携带属性字典时，字典**不能**内联在载荷里，必须另挂一处，并且要在 `OM-11` 的
//! `getattr`／`setattr` 通道上生效。
//!
//! 本层的挂载点是 [`pyawa_core::Header`] 的 `dict` 那一格（头部 32 → 40 字节；
//! 取舍记录见 `header.rs` 的文档：另一条路是实例侧侧表，代价是有字典的对象要付哈希桶、
//! 每次访问多一次哈希，并且释放与标记两处都要挂钩子）。

mod common;

use core::cell::RefCell;

use pyawa_core::{ExecError, Header, ListObject, StrObject, Value};

use common::{emit, op, Vm};

/// 造一个"`list` 的子类"类型：载荷是列表（布局固定），字典另行挂载。
fn list_subclass(vm: &Vm, name: &'static str) -> core::ptr::NonNull<pyawa_core::TypeObject> {
    let list = vm.instance.type_named("list").expect("list 在引导期已登记");
    let ty = vm.instance.new_type(
        name,
        core::mem::size_of::<ListObject>(),
        ListObject::slots(),
    );
    // OM-13：登记基类（C3 线性化）
    vm.instance.register_bases(ty, vec![list]);
    // OM-14：带实例字典，但**不**内联（载荷是列表）
    // SAFETY: ty 由注册表持有。
    unsafe { ty.as_ref() }.mark_external_instance_dict();
    ty
}

#[test]
fn external_dict_is_created_lazily_and_found_again() {
    // 用 name 表走一遍：`obj.f = 7` 之后 `obj.f` 能读回 7，且**载荷没变**（还是空列表）
    let vm = Vm::new();
    let ty = list_subclass(&vm, "MyList2");
    let instance = vm.instance.alloc(ListObject::new(ty, RefCell::new(Vec::new())));
    let instance_raw = instance.into_raw().cast::<Header>();
    // SAFETY: instance 刚分配，本测试持有。
    unsafe { vm.instance.incref_object(instance_raw.as_ptr()) };

    let bytes = emit(&[
        (op("RESUME"), 0),
        (op("LOAD_CONST"), 1),
        (op("LOAD_CONST"), 0),
        (op("STORE_ATTR"), 0), // 名字下标 = oparg（STORE_ATTR 不移位，BC-57）
        (op("LOAD_CONST"), 0),
        (op("LOAD_ATTR"), 0),
        (op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec!["f".to_owned()],
        bytes,
        vec![Some(instance_raw), Some(vm.constant(7))],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::small_int(7), &vm.instance), "obj.f 读回 7");

    // 载荷仍是那个空列表；字典在头部那一格上
    // SAFETY: instance_raw 已交给常量表，仍然存活。
    let stored = unsafe { &*instance_raw.as_ptr().cast::<ListObject>() };
    assert!(stored.is_empty(), "列表载荷没有被字典污染");

    // 实例被字典指着走 `OM-36` 的遍历：字典里的值也是它的直接引用
    let mapping = unsafe { instance_raw.as_ref() }.instance_dict().expect("字典应当已挂上");
    // SAFETY: mapping 由该对象持有。
    let dict = unsafe { &*mapping.as_ptr().cast::<pyawa_core::DictObject>() };
    assert_eq!(dict.entries().len(), 1, "字典里应当是 f=7");
    let (key, value) = dict.entries()[0];
    // SAFETY: 键是 str。
    assert_eq!(unsafe { &*key.as_ptr().cast::<StrObject>() }.value(), "f");
    // SAFETY: 值是 int。
    assert_eq!(unsafe { &*value.as_ptr().cast::<pyawa_core::IntObject>() }.value, 7);
}

#[test]
fn deleting_the_attribute_removes_it_from_the_external_dict() {
    let vm = Vm::new();
    let ty = list_subclass(&vm, "MyList3");
    let instance = vm.instance.alloc(ListObject::new(ty, RefCell::new(Vec::new())));
    let instance_raw = instance.into_raw().cast::<Header>();
    // SAFETY: instance 刚分配，本测试持有。
    unsafe { vm.instance.incref_object(instance_raw.as_ptr()) };

    // obj.f = 7; del obj.f; obj.f  ⇒ AttributeError
    let bytes = emit(&[
        (op("RESUME"), 0),
        (op("LOAD_CONST"), 1),
        (op("LOAD_CONST"), 0),
        (op("STORE_ATTR"), 0),
        (op("LOAD_CONST"), 0),
        (op("DELETE_ATTR"), 0),
        (op("LOAD_CONST"), 0),
        (op("LOAD_ATTR"), 0),
        (op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec!["f".to_owned()],
        bytes,
        vec![Some(instance_raw), Some(vm.constant(7))],
    );
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    assert_eq!(
        vm.pending_exception().map(|(name, _)| name),
        Some("AttributeError".to_owned())
    );
}

#[test]
fn external_dict_does_not_leak() {
    // 字典挂在头部那一格上，释放时必须交出去（否则是永久泄漏）
    let vm = Vm::new();
    let ty = list_subclass(&vm, "MyList4");
    let baseline = vm.instance.live_objects();
    {
        let instance = vm.instance.alloc(ListObject::new(ty, RefCell::new(Vec::new())));
        // `into_raw` 把那一份引用交出来：测试结束时释放一次即可（不额外 incref）
        let header = instance.into_raw().cast::<Header>();
        // 造一份字典挂上去（等价于一次 STORE_ATTR 的惰性建表）
        let dict = vm.instance.alloc(pyawa_core::DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            RefCell::new(Vec::new()),
        ));
        // SAFETY: header 存活。
        unsafe { header.as_ref() }.store_instance_dict(dict.into_raw().cast::<Header>());
        assert!(vm.instance.live_objects() >= baseline + 2);
        // SAFETY: 交出本测试那份引用（字典那份会随对象一起放）
        unsafe { vm.instance.release_object(header.as_ptr()) };
    }
    assert_eq!(
        vm.instance.live_objects(),
        baseline,
        "对象与其另行挂载的字典都要收回去"
    );
}
