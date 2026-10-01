//! 两个字典**共享同一个键对象**时的引用计数（很常见：同一个字符串当键出现在多处）。
//!
//! 这个用例是从类创建那条路里逼出来的：`__build_class__` 把类命名空间里的键**原样**放进
//! 类型字典时出现过堆损坏，改成"类型字典用自己造的键"就好了——所以先把这个更基础的性质钉住。

mod common;

use core::cell::RefCell;

use pyawa_core::{DictObject, Header, StrObject};

use common::Vm;

#[test]
fn two_dicts_can_share_a_key_and_both_survive() {
    let vm = Vm::new();
    let dict_type = vm.instance.type_named("dict").unwrap();

    // 键在**两个**字典里都出现（各自持有一份引用，符号上像共享）
    let left = vm.instance.alloc(DictObject::new(dict_type, RefCell::new(Vec::new())));
    let right = vm.instance.alloc(DictObject::new(dict_type, RefCell::new(Vec::new())));
    let key = vm.instance.new_str("k");
    // `insert_raw` **转移**调用方那份引用（不自己 incref）⇒ 每插一次都要各带一份
    // SAFETY: key 由本测试持有，存活。
    unsafe { vm.instance.incref_object(key.as_ptr()) };
    left.get().insert_raw(key, vm.constant(1));
    // SAFETY: 同上。
    unsafe { vm.instance.incref_object(key.as_ptr()) };
    right.get().insert_raw(key, vm.constant(2));
    // SAFETY: key 是存活对象。
    assert_eq!(
        unsafe { key.as_ref() }.refcount(),
        3,
        "两份字典各持一份 ＋ 本测试一份"
    );

    // 放掉一个字典：键的计数回到 2（另一个字典 ＋ 本测试）
    drop(left);
    // SAFETY: key 仍存活。
    assert_eq!(unsafe { key.as_ref() }.refcount(), 2);

    // 另一个字典仍然完好
    assert_eq!(right.get().entries().len(), 1);
    let (stored_key, stored_value) = right.get().entries()[0];
    assert_eq!(stored_key, key);
    // SAFETY: 值是整数。
    assert_eq!(unsafe { &*stored_value.as_ptr().cast::<pyawa_core::IntObject>() }.value, 2);

    // SAFETY: 本测试那份。
    unsafe { vm.instance.release_object(key.as_ptr()) };
    let _: *const Header = stored_key.as_ptr();
}
