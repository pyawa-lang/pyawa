//! 单例表与值表示的可测性质（`docs/SPEC-object-model.md` §8、§12：**OM-23**／**OM-39**）。
//!
//! 小整数区间取 CPython 3.14 的实测边界（`-5..=256`，见 `singleton.rs` 的注释）；
//! `Value` 的内联规则由 `OM-39` 钉住：**只有单例表覆盖到的值才允许内联**。

mod common;

use core::ptr::NonNull;

use pyawa_core::flags;
use pyawa_core::{
    py_object, Header, Instance, IntObject, NoneObject, Slots, Value, SMALL_INT_MAX,
    SMALL_INT_MIN,
};

py_object! {
    /// 与单例同构的普通对象：用来验证"值相等 ≠ `is`"。
    struct PlainInt {
        value: i64,
    }
}

fn plain_int_type(instance: &Instance) -> NonNull<pyawa_core::TypeObject> {
    instance.new_type(
        "PlainInt",
        core::mem::size_of::<PlainInt>(),
        Slots::new(PlainInt::dealloc),
    )
}

#[test]
fn singletons_are_per_instance() {
    let left = Instance::new();
    let right = Instance::new();

    // OM-1／OM-23：单例是"对象"，而对象只属于一个实例
    assert_ne!(left.singletons().none(), right.singletons().none());
    assert_ne!(left.singletons().boolean(true), right.singletons().boolean(true));
    assert_ne!(
        left.singletons().small_int(1).unwrap(),
        right.singletons().small_int(1).unwrap()
    );

    // 每个实例自己那份单例是稳定的：取两次是同一个对象
    assert_eq!(left.singletons().none(), left.singletons().none());
    assert_eq!(
        left.singletons().small_int(42).unwrap(),
        left.singletons().small_int(42).unwrap()
    );
}

#[test]
fn small_int_range_matches_the_measured_oracle() {
    let instance = Instance::new();

    for value in SMALL_INT_MIN..=SMALL_INT_MAX {
        assert!(
            instance.singletons().small_int(value).is_some(),
            "OM-23：{value} 应当在小整数单例表里"
        );
    }
    assert_eq!(
        instance.singletons().small_int(SMALL_INT_MIN - 1),
        None,
        "区间外不得有单例"
    );
    assert_eq!(instance.singletons().small_int(SMALL_INT_MAX + 1), None);
    assert_eq!(
        instance.singletons().small_ints().len() as i64,
        SMALL_INT_MAX - SMALL_INT_MIN + 1
    );
}

#[test]
fn value_is_semantics_follow_the_singleton_table() {
    let instance = Instance::new();

    // 单例：同类同值即同一个（内联不改变语义，OM-39）
    assert!(Value::None.is_same(&Value::None, &instance));
    assert!(Value::Bool(true).is_same(&Value::Bool(true), &instance));
    assert!(Value::Bool(false).is_same(&Value::Bool(false), &instance));
    assert!(!Value::Bool(true).is_same(&Value::Bool(false), &instance));
    assert!(Value::small_int(7).is_same(&Value::small_int(7), &instance));

    // `True is 1` 在 CPython 里是 False ⇒ 单例表里它们是两个不同的对象
    assert!(!Value::Bool(true).is_same(&Value::small_int(1), &instance));
    assert!(!Value::None.is_same(&Value::Bool(false), &instance));

    // 对象与它对应的单例：指向同一个对象 ⇒ `is` 为真
    let none = instance.own(instance.singletons().none());
    assert!(Value::Object(none).is_same(&Value::None, &instance));

    // 值相等但对象不同 ⇒ `is` 为假（OM-39：不是值相等）
    let ty = plain_int_type(&instance);
    let left = instance.alloc(PlainInt::new(ty, 7));
    let right = instance.alloc(PlainInt::new(ty, 7));
    let left_ref = instance.own(left.as_ptr().cast::<Header>());
    let right_ref = instance.own(right.as_ptr().cast::<Header>());
    assert!(!Value::Object(left_ref).is_same(&Value::Object(right_ref), &instance));
}

#[test]
fn values_hold_references() {
    let instance = Instance::new();
    let none = instance.singletons().none();

    // SAFETY: none 由实例持有，实例存活期间有效。
    let before = instance.getrefcount(unsafe { &*none.as_ptr().cast::<NoneObject>() });

    let value = Value::Object(instance.own(none));
    // SAFETY: 同上。
    assert_eq!(
        instance.getrefcount(unsafe { &*none.as_ptr().cast::<NoneObject>() }),
        before + 1,
        "OM-16：值持有一个新引用"
    );

    drop(value);
    // SAFETY: 同上。
    assert_eq!(
        instance.getrefcount(unsafe { &*none.as_ptr().cast::<NoneObject>() }),
        before,
        "OM-20：归还"
    );
}

#[test]
fn singletons_are_not_gc_tracked_and_never_collected() {
    let instance = Instance::new();
    // 单例都是叶子对象（内部无引用），**本身**不进回收链表、也不会被回收。
    // （引导期链表里可能已经有东西——类型字典就是——所以这里逐个查单例，不查总数。）
    // SAFETY: 单例由实例持有。
    let none = unsafe { instance.singletons().none().as_ref() };
    assert!(!none.has_flag(flags::GC_TRACKED));
    assert!(!none.is_immortal(), "OM-24：IMMORTAL 位 M1 保持 0");
    let before_collect = instance.getrefcount(unsafe { &*instance.singletons().none().as_ptr().cast::<NoneObject>() });
    instance.collect();
    assert_eq!(
        instance.getrefcount(unsafe { &*instance.singletons().none().as_ptr().cast::<NoneObject>() }),
        before_collect,
        "回收不动单例"
    );
    // 其它几个常用单例同样不入链
    for singleton in [
        instance.singletons().none_type().cast::<Header>(),
        instance.singletons().boolean(true),
        instance.singletons().empty_str(),
    ] {
        assert!(
            !unsafe { singleton.as_ref() }.has_flag(flags::GC_TRACKED),
            "单例不入回收链表"
        );
    }
}

#[test]
fn plain_int_objects_are_not_singletons() {
    let instance = Instance::new();
    let ty = plain_int_type(&instance);
    let object = instance.alloc(IntObject::new(ty, 7));

    assert_ne!(
        object.as_ptr().cast::<Header>(),
        instance.singletons().small_int(7).unwrap(),
        "普通对象不是单例：`is` 必须为假"
    );
}

#[test]
fn empty_tuples_are_one_object() {
    // 建两次空元组 ⇒ **同一个指针**（并且每次都给一份新引用，所有权照 `OM-16`）
    let instance = common::Vm::new();
    let first = instance.instance.new_tuple(Vec::new());
    let second = instance.instance.new_tuple(Vec::new());
    assert_eq!(first, second, "OM-23：空元组必须是单例");
    // SAFETY: 两份都是本测试持有的新引用。
    unsafe {
        instance.instance.release_object(first.as_ptr());
        instance.instance.release_object(second.as_ptr());
    }
}

#[test]
fn empty_tuple_literals_and_tuple_call_are_the_singleton() {
    // 从**字节码**看：`BUILD_TUPLE 0` 两次 ＋ `IS_OP 0` ⇒ True；`tuple()` 同理
    let vm = common::Vm::new();
    let bytes = common::assemble(&[
        common::Item::Instr(common::op("RESUME"), 0),
        common::Item::Instr(common::op("BUILD_TUPLE"), 0),
        common::Item::Instr(common::op("BUILD_TUPLE"), 0),
        common::Item::Instr(common::op("IS_OP"), 0),
        common::Item::Instr(common::op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code_with_names(8, 0, 0, Vec::new(), Vec::new(), bytes, vec![Some(vm.instance.singletons().none())]);
    let result = vm.run(&code).expect("跑得动");
    let header = result.as_header(&vm.instance).expect("有返回值");
    assert!(
        vm.instance.truth_of(header),
        "OM-23：() is () 必须为真"
    );
}

#[test]
fn the_singleton_table_holds_the_empty_tuple() {
    // 单例表里那一份，就是 `new_tuple(Vec::new())` 与 `BUILD_TUPLE 0` 给的那一份
    let vm = common::Vm::new();
    let from_table = vm.instance.singletons().empty_tuple();
    let from_new = vm.instance.new_tuple(Vec::new());
    assert_eq!(from_table, from_new, "OM-23：空元组只此一份");
    // SAFETY: from_new 是本测试持有的新引用。
    unsafe { vm.instance.release_object(from_new.as_ptr()) };
}
