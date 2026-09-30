//! 引用计数的 RAII 协议（`docs/SPEC-object-model.md` §7，**OM-16**…**OM-22**）。
//!
//! **OM-17** 要求 Rust 侧持有引用一律经守卫，**禁止**在业务代码里裸写 incref／decref；
//! 本模块就是那两个守卫。**OM-18**：它不是 `Rc`／`Arc`——计数由实例的释放协议处理，
//! 因此能表达循环，也提供 `sys.getrefcount` 要求的可观测性。

use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::header::{Header, PyObject};
use crate::instance::Instance;

/// **OM-16**：**新引用**。
///
/// - `Clone` 增加计数，`Drop` 减少计数（**OM-17**）
/// - 生命周期绑定 `&Instance`：守卫**不可能**活过它所属的实例
pub struct Owned<'a, T: PyObject> {
    ptr: NonNull<T>,
    instance: &'a Instance,
}

impl<'a, T: PyObject> Owned<'a, T> {
    pub(crate) fn new(ptr: NonNull<T>, instance: &'a Instance) -> Self {
        Self { ptr, instance }
    }

    /// 裸指针（内部表示；**OM-6**：**禁止**出现在 C ABI 签名里）。
    pub fn as_ptr(&self) -> NonNull<T> {
        self.ptr
    }

    /// 头部（**OM-5**）。
    pub fn header(&self) -> &Header {
        self.get().header()
    }

    /// 对象访问（借用，不增计数）。
    pub fn get(&self) -> &T {
        // SAFETY: 本守卫持有一个新引用，对象在 self 存活期间必然有效（OM-16）。
        unsafe { self.ptr.as_ref() }
    }

    /// **OM-22**：`sys.getrefcount` 看到的计数。
    pub fn refcount(&self) -> u32 {
        self.header().refcount()
    }

    /// **OM-16**／**OM-19**：需要借用语义时**必须**显式取借用，**禁止**隐式借用返回值。
    pub fn borrow(&self) -> Borrowed<'a, T> {
        Borrowed {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }

    /// 交出裸指针并**不**释放这份引用（所有权转移给调用方）。
    ///
    /// 调用方随后必须用 [`Instance::incref_object`]／[`Instance::release_object`] 记账。
    pub fn into_raw(self) -> NonNull<T> {
        let ptr = self.ptr;
        core::mem::forget(self);
        ptr
    }
}

impl<T: PyObject> Clone for Owned<'_, T> {
    fn clone(&self) -> Self {
        // SAFETY: self.ptr 有效（见 Owned::get），clone 取得一个新引用。
        unsafe {
            self.instance
                .incref_object(self.ptr.cast::<Header>().as_ptr())
        };
        Self {
            ptr: self.ptr,
            instance: self.instance,
        }
    }
}

impl<T: PyObject> Drop for Owned<'_, T> {
    fn drop(&mut self) {
        // SAFETY: self.ptr 有效，且本守卫持有的正是一份新引用。
        unsafe {
            self.instance
                .release_object(self.ptr.cast::<Header>().as_ptr())
        };
    }
}

/// **OM-19**：**借用引用**——不增加计数，只在"持有者存活"的窗口内有效；
/// 窗口由生命周期 `'a` 表达，而不是靠注释。
pub struct Borrowed<'a, T: PyObject> {
    ptr: NonNull<T>,
    _marker: PhantomData<&'a T>,
}

impl<'a, T: PyObject> Borrowed<'a, T> {
    /// 裸指针（内部表示；**OM-6**：**禁止**出现在 C ABI 签名里）。
    pub fn as_ptr(&self) -> NonNull<T> {
        self.ptr
    }

    /// 对象访问。借用**不**延长有效期。
    pub fn get(&self) -> &'a T {
        // SAFETY: 借用引用的窗口由 'a 表达（OM-19），持有者在此窗口内必然存活。
        unsafe { self.ptr.as_ref() }
    }

    /// 头部（**OM-5**）。
    pub fn header(&self) -> &'a Header {
        self.get().header()
    }
}

// 手写 Clone／Copy：derive 会给 `T` 加上不必要的 `Clone`／`Copy` 约束。
impl<T: PyObject> Clone for Borrowed<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: PyObject> Copy for Borrowed<'_, T> {}
