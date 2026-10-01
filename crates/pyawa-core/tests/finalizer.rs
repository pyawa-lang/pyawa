//! **槽位回调执行器**（第一批的另一半）：
//!
//! - `__del__` 覆写（`OM-20` ①）：终结器按 **`TS-44`** 走**属性通道**找 `__del__` 并调用
//! - 容器元素的 `repr`／`str` 也走属性通道（`repr([x])` 要尊重 `x` 的 `__repr__` 覆写）
//!
//! 观察手段：测试自己的一个 `static AtomicBool`（`CX-3` 的静态扫描只覆盖各 crate 的 `src/`，
//! 测试里的探针不算 VM 状态）。

mod common;

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};

use pyawa_core::{Header, Value};

use common::Vm;

static FINALIZER_RAN: AtomicBool = AtomicBool::new(false);
static NATIVE_SEEN: AtomicBool = AtomicBool::new(false);

/// 一个原生函数：把探针置上（供 `__del__` 调用）。
unsafe fn mark_seen(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    NATIVE_SEEN.store(true, Ordering::SeqCst);
    Ok(instance.new_int(0))
}

use core::ptr::NonNull;
use pyawa_core::Instance;

#[test]
fn a_python_level_del_runs_when_the_object_dies() {
    FINALIZER_RAN.store(false, Ordering::SeqCst);
    NATIVE_SEEN.store(false, Ordering::SeqCst);
    let vm = Vm::new();

    // `__del__` 的实现：调用一个原生函数（把探针置上）
    let native = {
        let object = vm.instance.alloc(pyawa_core::BuiltinFunctionObject::new(
            vm.instance
                .type_named("builtin_function_or_method")
                .unwrap(),
            "mark_seen",
            core::cell::Cell::new(mark_seen as pyawa_core::NativeFn),
        ));
        object.into_raw().cast::<Header>()
    };
    // 直接用类型字典构造：`__del__` 是一个**原生可调用对象**（最简形态）
    let ty = vm.instance.new_attribute_type("WithDel");
    vm.instance
        .set_type_attribute(ty, "__del__", native);

    // 造一个实例再放掉 ⇒ 终结器应当跑
    let instance = vm.instance.alloc(pyawa_core::AttributeObject::new(
        ty,
        RefCell::new(None),
    ));
    let header = instance.into_raw().cast::<Header>();
    // SAFETY: header 由本测试持有。
    unsafe { vm.instance.release_object(header.as_ptr()) };
    assert!(
        NATIVE_SEEN.load(Ordering::SeqCst),
        "`__del__`（原生形态）应当被终结器调用"
    );
    let _ = FINALIZER_RAN.load(Ordering::SeqCst);
}

#[test]
fn container_repr_honours_a_repr_override() {
    // `repr([x])` 里元素走属性通道：`x.__repr__` 覆盖了原生槽位
    let vm = Vm::new();
    let ty = vm.instance.new_attribute_type("Loud");
    // `__repr__` 用原生函数返回一个固定字符串：这里先造一个返回常量字符串的原生
    unsafe fn shout(
        instance: &Instance,
        _bound: Option<NonNull<Header>>,
        _args: &[NonNull<Header>],
        _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    ) -> Result<NonNull<Header>, pyawa_core::ExecError> {
        Ok(instance.new_str("<loud>"))
    }
    let native = {
        let object = vm.instance.alloc(pyawa_core::BuiltinFunctionObject::new(
            vm.instance
                .type_named("builtin_function_or_method")
                .unwrap(),
            "__repr__",
            core::cell::Cell::new(shout as pyawa_core::NativeFn),
        ));
        object.into_raw().cast::<Header>()
    };
    vm.instance.set_type_attribute(ty, "__repr__", native);

    let element = vm
        .instance
        .alloc(pyawa_core::AttributeObject::new(ty, RefCell::new(None)));
    let element_header = element.into_raw().cast::<Header>();
    // SAFETY: element 由本测试持有，列表要自己那份引用。
    unsafe { vm.instance.incref_object(element_header.as_ptr()) };
    let list = vm.instance.alloc(pyawa_core::ListObject::new(
        vm.instance.type_named("list").unwrap(),
        RefCell::new(vec![element_header]),
    ));
    let list_header = list.into_raw().cast::<Header>();
    assert_eq!(
        vm.instance.object_repr(list_header),
        "[<loud>]",
        "元素的 `__repr__` 覆写要在容器 `repr` 里生效（TS-44）"
    );
    let _ = Value::small_int(0);
}
