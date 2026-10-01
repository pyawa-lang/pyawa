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
        *self.value.borrow()
    }

    /// 写入：返回被顶下来的旧引用，**调用方负责释放**（用 [`Instance::release_object`]）。
    pub fn replace(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut *self.value.borrow_mut(), value)
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
    if let Some(value) = cell.value.borrow_mut().take() {
        // SAFETY: 该引用由本 cell 持有（新引用语义），这里交还一份。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}
