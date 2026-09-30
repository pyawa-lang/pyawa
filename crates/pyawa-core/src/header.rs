//! 对象头（`docs/SPEC-object-model.md` §5，**OM-5**…**OM-8**）。
//!
//! 字段顺序即内存布局（`#[repr(C)]`），**OM-5** 要求所有对象以同一头部开头。
//! **OM-6**：头部**禁止**出现在任何 C ABI 签名里，宿主只见不透明句柄。

use core::cell::Cell;
use core::ptr;
use core::ptr::NonNull;

use crate::flags;
use crate::type_object::TypeObject;

/// **OM-5**：M1 统一头部，不做"仅容器带 gc 链"的拆分（**OM-8**）。
///
/// ```text
/// refcount : u32      // 非原子——"先 GIL"决策的直接后果（DESIGN §4）
/// flags    : u32      // 位分配见 OM-7
/// ty       : *Type    // 类型对象指针
/// gc_prev  : *Header  // 循环回收链表（仅 GC_TRACKED 有意义）
/// gc_next  : *Header
/// ```
#[repr(C)]
pub struct Header {
    refcount: Cell<u32>,
    flags: Cell<u32>,
    ty: Cell<NonNull<TypeObject>>,
    gc_prev: Cell<*mut Header>,
    gc_next: Cell<*mut Header>,
}

impl Header {
    /// 新对象的头部：计数从 1 开始（**OM-16**：分配即一个新引用）。
    ///
    /// 由 `py_object!` 生成的构造函数调用。**OM-5**：对象必须把头部放在第一个字段，
    /// 否则 [`crate::PyObject`] 的契约不成立。
    pub fn new(ty: NonNull<TypeObject>) -> Self {
        Self {
            refcount: Cell::new(1),
            flags: Cell::new(0),
            ty: Cell::new(ty),
            gc_prev: Cell::new(ptr::null_mut()),
            gc_next: Cell::new(ptr::null_mut()),
        }
    }

    /// 当前引用计数。**OM-22**：`sys.getrefcount` 看到的是它 **+1**。
    pub fn refcount(&self) -> u32 {
        self.refcount.get()
    }

    /// 裸 `flags` 值（调试与测试用；位语义见 [`crate::flags`]）。
    pub fn flags(&self) -> u32 {
        self.flags.get()
    }

    /// **OM-7**：位测试。
    pub fn has_flag(&self, bit: u32) -> bool {
        self.flags.get() & bit != 0
    }

    /// **OM-24**：M1 只预留 `IMMORTAL` 位并保持 0，实现**禁止**依赖它做正确性判断。
    pub fn is_immortal(&self) -> bool {
        self.has_flag(flags::IMMORTAL)
    }

    /// 类型对象指针。
    pub fn ty(&self) -> NonNull<TypeObject> {
        self.ty.get()
    }

    pub(crate) fn set_ty(&self, ty: NonNull<TypeObject>) {
        self.ty.set(ty);
    }

    /// 置位。**OM-7**：bit 4–7 是预留区，**禁止**占用——这里用断言把它变成会响的约束。
    pub(crate) fn set_flag(&self, bit: u32) {
        debug_assert_eq!(
            bit & flags::RESERVED_MASK,
            0,
            "OM-7：flags 的 4–7 位是预留区，禁止占用"
        );
        self.flags.set(self.flags.get() | bit);
    }

    pub(crate) fn clear_flag(&self, bit: u32) {
        self.flags.set(self.flags.get() & !bit);
    }

    /// 增加计数（**OM-16**：新引用）。**禁止**在业务代码里直接使用——走 [`crate::Owned`]。
    pub(crate) fn incref(&self) {
        debug_assert!(self.refcount.get() > 0, "对已释放对象 incref");
        self.refcount.set(self.refcount.get() + 1);
    }

    /// 减少计数并返回新值。归零后的释放协议在 `Instance::release_object`（**OM-20**）。
    pub(crate) fn decref(&self) -> u32 {
        debug_assert!(self.refcount.get() > 0, "对已释放对象 decref");
        let next = self.refcount.get() - 1;
        self.refcount.set(next);
        next
    }

    /// 复活用：**OM-20** ① 的终结器可以把计数改回大于 0。
    #[allow(dead_code)] // 供 §9 的回收器与外部终结器使用；M1 骨架先保留入口
    pub(crate) fn set_refcount(&self, value: u32) {
        self.refcount.set(value);
    }

    // ---- 循环回收链表（OM-5 的字段；算法见 §9，本层只保留位置） ----

    #[allow(dead_code)]
    pub(crate) fn gc_prev(&self) -> *mut Header {
        self.gc_prev.get()
    }

    #[allow(dead_code)]
    pub(crate) fn gc_next(&self) -> *mut Header {
        self.gc_next.get()
    }

    #[allow(dead_code)]
    pub(crate) fn set_gc_prev(&self, prev: *mut Header) {
        self.gc_prev.set(prev);
    }

    #[allow(dead_code)]
    pub(crate) fn set_gc_next(&self, next: *mut Header) {
        self.gc_next.set(next);
    }
}

/// **OM-5**：以头部开头的对象。
///
/// # Safety
///
/// 实现者**必须**是 `#[repr(C)]` 结构体，且**第一个字段**是 [`Header`]。
/// 对象模型内部依赖"头部就是对象地址"这一点（`NonNull<T>` → `NonNull<Header>` 的转换）。
pub unsafe trait PyObject {
    /// 头部只读访问。
    fn header(&self) -> &Header;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Instance;
    use core::mem::{offset_of, size_of};

    #[test]
    fn layout_is_repr_c_in_declared_order() {
        assert_eq!(offset_of!(Header, refcount), 0);
        assert_eq!(offset_of!(Header, flags), 4);
        assert_eq!(offset_of!(Header, ty), 8);
        assert_eq!(offset_of!(Header, gc_prev), 16);
        assert_eq!(offset_of!(Header, gc_next), 24);
        assert_eq!(size_of::<Header>(), 32);
    }

    #[test]
    fn fresh_header_is_immortal_free_and_holds_one_reference() {
        let instance = Instance::new();
        let ty = instance.metatype();
        let header = Header::new(ty);
        assert_eq!(header.refcount(), 1);
        assert_eq!(header.flags(), 0);
        assert!(!header.is_immortal(), "OM-24：M1 的 IMMORTAL 位必须保持 0");
        assert_eq!(header.ty(), ty);
        assert!(header.gc_prev().is_null() && header.gc_next().is_null());
    }

    #[test]
    #[should_panic(expected = "禁止占用")]
    fn reserved_flag_bits_are_rejected() {
        let instance = Instance::new();
        let header = Header::new(instance.metatype());
        header.set_flag(flags::RESERVED_MASK);
    }
}
