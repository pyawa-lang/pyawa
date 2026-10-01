//! 内建类型表与层次的可测性质（`docs/SPEC-type-system.md` 的 **TS-41**／**TS-42**／**TS-43**，
//! `docs/SPEC-object-model.md` 的 **OM-13**／**OM-23**）。
//!
//! 期望值来自**探测产物** `crate::builtin_types`（由 `tools/gen_builtin_types.py` 生成，
//! 数值／层次一律不手写）——这正是 `TS-41` 的"禁止手写枚举"。

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::builtin_types::{builtin_type, Ladder};
use pyawa_core::{
    py_object, FloatObject, Instance, PlainObject, StrObject,
};

#[test]
fn registered_step1_types_match_the_probed_table() {
    // TS-41：已实现的那一批，其 `__bases__` 与 `__mro__` 必须与探测产物逐项一致
    let instance = Instance::new();
    let mut checked = 0;

    for name in ["object", "type", "NoneType", "bool", "int", "float", "str"] {
        let entry = builtin_type(name).unwrap_or_else(|| panic!("探测表里应当有 {name}"));
        assert_eq!(entry.ladder, Ladder::Step1, "TS-42：{name} 属第一阶梯");

        let registered = instance
            .type_named(name)
            .unwrap_or_else(|| panic!("{name} 应当已注册"));

        // SAFETY: 类型由实例的注册表持有（OM-15）。
        let ty = unsafe { registered.as_ref() };
        let bases: Vec<&str> = ty
            .bases()
            .iter()
            .map(|base| {
                // SAFETY: 同上。
                unsafe { base.as_ref() }.name()
            })
            .collect();
        let mro: Vec<&str> = ty
            .mro()
            .iter()
            .map(|entry| {
                // SAFETY: 同上。
                unsafe { entry.as_ref() }.name()
            })
            .collect();

        assert_eq!(bases, entry.bases, "TS-41：{name} 的 __bases__");
        assert_eq!(mro, entry.mro, "TS-41：{name} 的 __mro__");
        checked += 1;
    }

    assert_eq!(checked, 7, "第一阶梯的七个类型都要查到");
}

#[test]
fn every_subtype_sees_object_in_its_mro() {
    // OM-13 的 C3 产物让"任何类型 ⊂ object"自动成立（不再靠"没有基类"糊过去）
    let instance = Instance::new();
    let object_type = instance.type_named("object").unwrap();
    for name in ["type", "NoneType", "bool", "int", "float", "str"] {
        let ty = instance.type_named(name).unwrap();
        assert!(
            instance.is_subtype(ty, object_type),
            "TS-29／TS-40：{name} ⊂ object"
        );
    }
    // TS-40 点名的那条仍然成立，且现在是走 MRO 得到的
    assert!(instance.is_subtype(
        instance.type_named("bool").unwrap(),
        instance.type_named("int").unwrap()
    ));
    assert!(!instance.is_subtype(
        instance.type_named("float").unwrap(),
        instance.type_named("int").unwrap()
    ));
}

#[test]
fn mro_is_c3_not_depth_first() {
    // OM-13：钻石继承必须是 [D, B, C, A, object]；深度优先会给出 [D, B, A, C, object]
    let instance = Instance::new();
    let a = instance.new_type("A", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));
    let b = instance.new_type("B", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));
    let c = instance.new_type("C", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));

    assert!(instance.register_bases(b, vec![a]).is_some());
    assert!(instance.register_bases(c, vec![a]).is_some());
    let d = instance.new_type("D", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));
    let mro = instance.register_bases(d, vec![b, c]).expect("钻石继承应当可线性化");

    let names: Vec<&str> = mro
        .iter()
        .map(|entry| {
            // SAFETY: 类型由实例的注册表持有。
            unsafe { entry.as_ref() }.name()
        })
        .collect();
    assert_eq!(
        names,
        vec!["D", "B", "C", "A", "object"],
        "OM-13：C3 线性化，不是深度优先"
    );
}

#[test]
fn c3_refuses_inconsistent_bases() {
    // OM-13：基类顺序矛盾时必须**报错**（返回 None），禁止静默给一个错的 MRO
    let instance = Instance::new();
    let a = instance.new_type("A2", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));
    let b = instance.new_type("B2", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));
    let x = instance.new_type("X2", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));
    let y = instance.new_type("Y2", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));

    assert!(instance.register_bases(x, vec![a, b]).is_some());
    assert!(instance.register_bases(y, vec![b, a]).is_some());

    let z = instance.new_type("Z2", core::mem::size_of::<PlainObject>(), pyawa_core::Slots::new(PlainObject::dealloc));
    assert_eq!(
        instance.register_bases(z, vec![x, y]),
        None,
        "OM-13：X 要 A 在 B 前、Y 要 B 在 A 前 ⇒ 无解"
    );
    // SAFETY: z 由注册表持有，且上面的失败没有写进任何基类。
    assert_eq!(unsafe { z.as_ref() }.mro().len(), 2, "失败的登记不得留下半个 MRO");
}

py_object! {
    /// `TS-43`：载荷布局由实现自选——这里用一个带 `RefCell` 的自定义类型证明
    /// "类型存在"与"载荷长什么样"是两件事。
    struct Custom {
        slot: RefCell<Option<NonNull<pyawa_core::Header>>>,
    }
}

#[test]
fn payload_layout_is_the_implementation_choice() {
    // TS-43：表里只管"类型存在、层次正确"；float／str 的载荷由实现定
    let instance = Instance::new();
    let float_type = instance.type_named("float").unwrap();
    let str_type = instance.type_named("str").unwrap();

    let number = instance.alloc(FloatObject::new(float_type, 1.5));
    assert_eq!(number.get().value(), 1.5);

    let text = instance.alloc(StrObject::new(str_type, "你好".to_owned()));
    assert_eq!(text.get().value(), "你好");
    assert!(!text.get().is_empty());

    // 自定义类型走同一条路（载荷里放裸引用，见 OM-40）
    let custom_type = instance.new_type(
        "Custom",
        core::mem::size_of::<Custom>(),
        pyawa_core::Slots::new(Custom::dealloc),
    );
    let custom = instance.alloc(Custom::new(custom_type, RefCell::new(None)));
    assert!(custom.get().slot.borrow().is_none());
}

#[test]
fn empty_string_is_a_per_instance_singleton() {
    // OM-23：空串是单例（按实例创建）
    let left = Instance::new();
    let right = Instance::new();

    let empty = left.singletons().empty_str();
    // SAFETY: 单例由实例持有。
    let text = unsafe { &*empty.as_ptr().cast::<StrObject>() };
    assert_eq!(text.value(), "", "OM-23：空串单例的内容是空");
    assert!(text.is_empty());

    assert_eq!(left.singletons().empty_str(), left.singletons().empty_str());
    assert_ne!(
        left.singletons().empty_str(),
        right.singletons().empty_str(),
        "OM-1：单例不跨实例共享"
    );
    assert_eq!(
        // SAFETY: 单例由实例持有。
        unsafe { empty.as_ref() }.ty(),
        left.singletons().str_type(),
        "空串的类型就是 str"
    );
}

#[test]
fn m2_ladder_is_partially_wired() {
    // TS-42：M2 阶梯正在逐个接线——容器已就位，函数／迭代器／异常层次还没有。
    // 这条断言随接线推进而更新；全表就位时它应当变成"必须全部存在"（T-TS-11 的完整形态）。
    let instance = Instance::new();
    let wired = [
        "tuple",
        "list",
        "dict",
        "set",
        "function",
        "tuple_iterator",
        "list_iterator",
        "str_ascii_iterator",
        "dict_keyiterator",
        "set_iterator",
    ];

    for name in wired {
        assert!(
            instance.type_named(name).is_some(),
            "TS-42：{name} 属 M2，接线后必须存在"
        );
    }

    for name in ["BaseException", "ValueError"] {
        assert!(
            instance.type_named(name).is_none(),
            "TS-42：{name} 尚未接线——接线时请更新这条断言"
        );
    }
}
