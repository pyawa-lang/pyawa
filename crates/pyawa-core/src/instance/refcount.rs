//! `Instance` 的引用计数与释放域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓（模块级自由函数仍留在 instance.rs ✓）。

use super::*;

impl Instance {
    /// 归还一份引用（[`Self::retain`] 的配对）。
    ///
    /// 与 `retain` 一样是**安全函数**：契约（"这份引用确实是你持有的"）由调用方保证——
    /// `#![forbid(unsafe_code)]` 的 stdlib 要管理中途丢弃的中间数量，必须有这条配对。
    pub fn release(&self, object: NonNull<Header>) {
        // SAFETY: 调用方保证这份引用归它所有（见本函数的契约）。
        unsafe { self.release_object(object.as_ptr()) };
    }

    pub fn refcount_of(&self, object: NonNull<Header>) -> u32 {
        // SAFETY: 调用方按 `OM-16` 保证 object 是本实例里的存活对象。
        unsafe { object.as_ref() }.refcount()
    }

    /// **OM-40**：从裸引用**现取**一个守卫（取得一份新引用），用完即还。
    ///
    /// 载荷里只能存裸引用；要真正使用它，必须经这个访问器借出守卫。
    pub fn own(&self, raw: NonNull<Header>) -> PyRef<'_> {
        // SAFETY: 调用方（载荷的 traverse／clear）保证 raw 指向本实例的存活对象；
        // 这里为它新增一份引用，交给守卫负责归还。
        unsafe { self.incref_object(raw.as_ptr()) };
        // SAFETY: 同上。
        unsafe { PyRef::from_raw(raw, self) }
    }

    /// 释放一个**新引用**（**OM-16**）；计数归零时按 **OM-20** 的顺序处理：
    /// ① 终结器（可复活）→ ② `clear` → ③ 释放。清空走 **OM-21** 的待处理栈，不朴素递归。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中**存活**的对象，且调用方交出的是一份**新引用**。
    /// **悬垂哨兵** ✓（第 273 轮诊断）：见 [`dangling_mode`] ✓。
    pub fn assert_live(&self, ptr: NonNull<Header>, site: &str) {
        if !dangling_mode() {
            return;
        }
        let address = ptr.as_ptr() as usize;
        if self.live.borrow().contains(&address) {
            return;
        }
        // **类型对象不记活表** ✓（`live_objects()` 的口径："普通对象数，类型对象不计" ✓）——
        // 判据必须用**注册表**（`self.types` ✓ 所有类型对象都在那里 ✓）而**不能解引用** ✗：
        // 哨兵手里的指针可能**真的已经死了** ✓，读它的 `ty()` 会当场段错误（第 284 轮实测：
        // `PYAWA_DANGLING=1` 下 `import_posixpath_surface` 直接 SIGSEGV、连 panic 都没来得及打 ✗）。
        if self
            .types
            .borrow()
            .iter()
            .any(|ty| ty.as_ptr() as usize == address)
        {
            return;
        }
        {
            panic!(
                "[悬垂] {site} 要碰 {:#x}，但它**不在活表里** ✗ ⇒ 这个指针**已经被释放过** ✓",
                address
            );
        }
    }
}
