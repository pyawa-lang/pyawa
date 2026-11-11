//! 闭包 cell（`docs/SPEC-bytecode.md` **BC-45**）。
//!
//! `BC-45`：cell 与 free 变量**必须**用**独立槽数组**（不挤在 `locals` 里），
//! 且 cell **必须**是 `GC_TRACKED` 对象——递归函数经 cell 引用自身是经典成环来源。
//!
//! 载荷按 `OM-40` 存**裸引用**：只在 `clear`／`traverse`／`dealloc` 里释放。

use core::cell::RefCell;
use core::ptr::NonNull;

use crate::header::Header;
use crate::instance::Instance;
use crate::py_object;
use crate::type_object::Slots;

py_object! {
    /// 一个 cell 槽。可成环 ⇒ 带 `traverse`／`clear`（`OM-12`），`alloc` 会据此标 `GC_TRACKED`。
    pub struct CellObject {
        /// cell 里装的引用：**本对象持有它的一份引用**（`OM-16`）。
        value: RefCell<Option<NonNull<Header>>>,
    }
}

impl CellObject {
    /// 注册这个类型时的槽位表：`dealloc` ＋ `traverse`／`clear`（`BC-45`：cell 可成环）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(cell_traverse)
            .with_clear(cell_clear)
    }

    /// 取出裸引用（**借用**，不转移所有权）。
    pub fn value(&self) -> Option<NonNull<Header>> {
        match self.value.try_borrow() {
            Ok(value) => *value,
            Err(_) => {
                if crate::diag::flag_site!("PYAWA_CELL_DEBUG") {
                    let header = self as *const Self as *const Header;
                    // SAFETY: 诊断路径；调用方保证 cell 存活（若已释放，读类型名可能已不可信 ✓）。
                    let kind = unsafe { (*header).ty().as_ref() }.name().to_owned();
                    eprintln!(
                        "[cell] value() 借用冲突 addr={:p} kind={kind}\n{}",
                        self as *const Self,
                        std::backtrace::Backtrace::force_capture()
                    );
                }
                *self.value.borrow()
            }
        }
    }

    /// 写入：返回被顶下来的旧引用，**调用方负责释放**（用 [`Instance::release_object`]）。
    pub fn replace(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        if crate::diag::flag_site!("PYAWA_CELL_DEBUG") {
            eprintln!("[cell] replace 开始 addr={:p}", self as *const Self);
        }
        let outcome = core::mem::replace(&mut *self.value.borrow_mut(), value);
        if crate::diag::flag_site!("PYAWA_CELL_DEBUG") {
            eprintln!("[cell] replace 结束 addr={:p}", self as *const Self);
        }
        outcome
    }
}

/// `OM-40`：载荷里的引用只能在 `clear`／`traverse`／`dealloc` 内释放。
unsafe fn cell_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let cell = unsafe { &*ptr.cast::<CellObject>() };
    if let Some(value) = cell.value() {
        visit(value.as_ptr());
    }
}

/// `BC-45`／`OM-20` ②：交出并释放 cell 持有的引用。
unsafe fn cell_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let cell = unsafe { &*ptr.cast::<CellObject>() };
    // **先把借用放掉再减引用**（round 106 真 bug 修 ✗）：`if let Some(value) = cell.value.borrow_mut().take()`
    // 的 `RefMut` 临时**活到整个 `if let` 体结束** ✗ ⇒ 体内 `release_object` 若触发
    // **GC／traverse**（`cell_traverse` 会 `cell.value()`）⇒ 同一 `RefCell` **可变借用未放** ⇒
    // `already mutably borrowed` **panic** ✗（官方 `test_listcomps.test_cell_inner_free_outer`
    // 实测 ✓）。改成：括号里先 `take` 掉借用，再释放 ✓。
    let taken = { cell.value.borrow_mut().take() };
    if let Some(value) = taken {
        // SAFETY: 该引用由本 cell 持有（新引用语义），这里交还一份。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}
