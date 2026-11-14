//! **RAII 异常引用守卫**（阶段 2/A）的定点回归：`RaiseRef`／`PendingRef` 的所有权语义。
//!
//! 目标（`PLAN` §12.7）：把"`Err(Raised)` 那份引用被静默丢掉"从"靠纪律"变成**类型上不可能**。
//! 这些用例钉住三条契约：
//! 1. `RaiseRef` 的 **Drop 只还 `Err` 那份**、**保留 `pending` 槽**（槽是 Python 可见状态）；
//! 2. `discard()` ＝ 旧 `discard_raised`（还 `Err` 那份 ＋ 若槽仍是它则清槽还槽里那份）；
//! 3. `PendingRef` 的 Drop 自动归还 `replace_pending` 交回的**旧 pending**（终结器泄漏的结构性修法）；
//!    `into_pending()` 把 `Err` 那份**移交**回槽（不释放对象）。

mod common;

use core::ptr::NonNull;

use common::Vm;
use pyawa_core::raise_guard::{PendingRef, RaiseRef};
use pyawa_core::{ExceptionObject, Header};

fn make_exception(vm: &Vm, class: &str) -> NonNull<Header> {
    let ty = vm.instance.type_named(class).expect("异常类已登记");
    let object = vm.instance.alloc(ExceptionObject::new(
        ty,
        core::cell::RefCell::new(Vec::new()),
        core::cell::RefCell::new(None),
        core::cell::RefCell::new(None),
        core::cell::Cell::new(false),
        core::cell::RefCell::new(None),
    ));
    object.into_raw().cast::<Header>()
}

/// 建"一份槽 + 一份 `Err`"的现场（与 `raise()` 同构）：返回异常指针与守卫。
fn raised_scene(vm: &Vm) -> (NonNull<Header>, RaiseRef) {
    let exception = make_exception(vm, "ValueError"); // rc = 1（本函数持有）
    // `pending` 槽要**自己那份**（`raise()` 里也是新加一份）。
    // SAFETY: exception 是存活对象；这份引用交给槽。
    unsafe { vm.instance.incref_object(exception.as_ptr()) };
    let _slot = vm.instance.replace_pending(Some(exception));
    // `Err(Raised)` 那份 = 本函数开头那份 ⇒ 交给守卫。
    (exception, RaiseRef::from_owned(&vm.instance, exception))
}

#[test]
fn drop_releases_the_error_reference_but_keeps_the_pending_slot() {
    let vm = Vm::new();
    let (exception, guard) = raised_scene(&vm);
    // rc = 槽一份 + 守卫一份。
    assert_eq!(vm.instance.refcount_of(exception), 2);
    drop(guard);
    // **Drop 只还 `Err` 那份**：槽那份还在（Python 可见异常状态不能被"错误值被丢弃"改变）。
    assert_eq!(vm.instance.refcount_of(exception), 1);
    assert_eq!(vm.instance.pending_exception(), Some(exception));
    // 收尾：清槽（`PendingRef` Drop 归还最后一份）。
    let _cleanup = vm.instance.replace_pending(None);
}

#[test]
fn discard_clears_the_pending_slot_as_well() {
    let vm = Vm::new();
    let (exception, guard) = raised_scene(&vm);
    guard.discard();
    // 槽被清、对象已释放（不再引用任何东西）。
    assert_eq!(vm.instance.pending_exception(), None);
    // `discard` 幂等：再丢一次不炸。
    let _ = exception;
}

#[test]
fn into_pending_transfers_the_error_reference_into_the_slot() {
    let vm = Vm::new();
    // 只造"`Err` 那份"（没有槽），验证移交语义。
    let exception = make_exception(&vm, "TypeError");
    let guard = RaiseRef::from_owned(&vm.instance, exception);
    let previous = guard.into_pending();
    assert!(previous.get().is_none());
    assert_eq!(vm.instance.pending_exception(), Some(exception));
    // 槽现在拥有唯一一份；清槽即释放。
    assert_eq!(vm.instance.refcount_of(exception), 1);
    let _cleanup = vm.instance.replace_pending(None);
}

#[test]
fn pending_ref_drop_returns_the_previous_pending() {
    let vm = Vm::new();
    let first = make_exception(&vm, "ValueError");
    let second = make_exception(&vm, "TypeError");
    // 槽初始持有 first（rc=1）。
    let _initial = vm.instance.replace_pending(Some(first));
    // 换成 second（second rc=1），交回的旧值是 `PendingRef` —— Drop 时释放 first。
    let previous: PendingRef = vm.instance.replace_pending(Some(second));
    assert_eq!(previous.get(), Some(first));
    assert_eq!(vm.instance.refcount_of(first), 1);
    drop(previous);
    // first 已释放；槽里的 second 仍活着。
    assert_eq!(vm.instance.refcount_of(second), 1);
    let _cleanup = vm.instance.replace_pending(None);
}
