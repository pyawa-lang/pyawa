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

impl Instance {
    /// **OM-22**：`sys.getrefcount` 的可见语义——返回值**含参数借用**的那一份。
    pub fn getrefcount<T: PyObject>(&self, object: &T) -> u32 {
        object.header().refcount() + 1
    }

    /// 增加一个引用。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中**存活**的对象。
    pub unsafe fn incref_object(&self, ptr: *mut Header) {
        // **诊断**（第 296 轮，只在 `PYAWA_QUARANTINE=1` 时花钱 ✓）：撞上隔离区里的对象时
        // 先把"它原来是什么类型、现在哪一帧在动它"报出来 ✓ —— 光一句"对已释放对象 incref" ✗
        // 查不动（第 295 轮就是靠这条栈才把 `P3-21` 定位到 `subscript_get` 的 ✓）。
        self.quarantine_report(ptr, "incref");
        if self.watch.get() == ptr as usize {
            eprintln!(
                "[watch] incref {ptr:p} → rc={} 现场={}",
                unsafe { &*ptr }.refcount() + 1,
                self.current_site()
            );
        }
        // SAFETY: 由调用方保证 ptr 有效。
        unsafe { &*ptr }.incref();
    }

    /// **OM-20** ①：把**正在终结**的对象复活——计数从 0 回到 1。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中一个正在执行终结器的对象（`FINALIZING` 已置位）。
    pub unsafe fn resurrect_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        let header = unsafe { &*ptr };
        debug_assert!(
            header.has_flag(flags::FINALIZING),
            "只有终结器执行中的对象可以被复活（OM-20）"
        );
        header.set_refcount(header.refcount() + 1);
    }

    pub unsafe fn release_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        // SAFETY: 调用方保证 ptr 有效；**先查活表** ✓（第 273 轮诊断）。
        if self.watch.get() == ptr as usize {
            eprintln!(
                "[watch] decref {ptr:p} → rc={} 现场={}",
                unsafe { &*ptr }.refcount() - 1,
                self.current_site()
            );
        }
        // **按判据盯**（第 115 轮，`PYAWA_WATCH_DICT=1`）：任何 `dict` **掉到 0** 都报现场 ✓
        // —— 第 114 轮查明"死在 @540 的是**另一个类**的命名空间" ✓ ⇒ 盯一个地址不够 ✓，
        // 要用**判据**（类型＝`dict` ✓）把**第一个被打到 0 的那个**逼出来 ✓。
        if flag("PYAWA_WATCH_DICT") && unsafe { &*ptr }.refcount() == 1 {
            let ty = unsafe { &*ptr }.ty();
            // SAFETY: ty 由注册表持有。
            let name = unsafe { ty.as_ref() }.name();
            if name == "dict" {
                eprintln!(
                    "[dict→0] {ptr:p} 掉到 0：现场={}\n{}",
                    self.current_site(),
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        self.assert_live(unsafe { NonNull::new_unchecked(ptr) }, "release_object");
        let header = unsafe { &*ptr };
        // **OM-24**：M1 的 `IMMORTAL` 位恒为 0；这里只是防御，不承担语义。
        if header.is_immortal() {
            return;
        }
        if header.decref() != 0 {
            return;
        }

        // **注册过的类型对象不在这里释放** ✓（第 284 轮）：类型对象**不记活表** ✓、由 `self.types`
        // 注册表持有、**销毁实例时统一释放** ✓（见 `Instance` 的销毁段 ✓）。先前没有这一道 ⇒
        // 一旦某处把**最后一份计数**放掉 ✗（实测：`abc.py` 的类体帧槽就持有 `ABC` 那个类对象 ✓）
        // ⇒ 通用释放路径**当场把注册表里的类型对象释放掉** ✗ ⇒ 之后谁再碰它谁段错误 ✓
        // （`PYAWA_DANGLING=1` 下 `import_posixpath` 的 SIGSEGV 就是这个 ✓）。
        // **如实说** ✗：这一道**挡住的是症状** ✓ —— 真正要查的是"**谁多放了一份**" ✓（欠计数 ✓），
        // 已记进 `P3-19` 与第 284 轮台账 ✓。
        if self
            .types
            .borrow()
            .iter()
            .any(|ty| ty.as_ptr() as usize == ptr as usize)
        {
            return;
        }

        // 回收进行中：不可达对象由本次 `collect` 统一释放，这里只减计数（OM-27 ④）。
        if !self.gc_frozen.borrow().is_empty() && self.gc_frozen.borrow().contains(&(ptr as usize)) {
            return;
        }

        // SAFETY: ptr 非空（调用方保证）。
        self.pending
            .borrow_mut()
            .push(unsafe { NonNull::new_unchecked(ptr) });

        if self.draining.get() {
            // 已经在清空栈里：交给最外层那次调用处理。
            return;
        }
        self.draining.set(true);
        loop {
            let next = self.pending.borrow_mut().pop();
            let Some(object) = next else { break };
            self.release_one(object);
        }
        self.draining.set(false);
    }

}
