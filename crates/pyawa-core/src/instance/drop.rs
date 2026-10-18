use super::*;

impl Default for Instance {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Instance {
    /// **OM-2**：实例销毁**必须**释放其堆内全部内存，**无论循环是否被回收过**——
    /// 不依赖回收器先跑完。
    fn drop(&mut self) {
        // **`CP-19`**：实例销毁**必须**释放全部能力表条目（逐条调 provider `close` 槽，
        // 不依赖回收器先跑完）——与下面的内存回收路径无关，所以放最前面。
        self.capability_close_all();
        // 正常路径下 `live` 已经空了。仍有残留 ⇒ 计数环或未交出的引用，
        // 此时**强制释放**：不调终结器、不再 clear（环的语义已由 §9 的回收器负责，
        // 走到这里说明调用方没有 collect，而不是回收器做不到）。
        //
        // 本层能这样做，是因为生命周期把 `Owned` 钉在 `&Instance` 上：对象载荷里
        // **不可能**存着 `Owned` 守卫（那需要 `&'static Instance`），所以这里释放
        // 任何一个对象都不会回头去碰别的对象。
        // **"只漏不放"也要盖住销毁这一段** ✓（第 240 轮）：进程马上要退出 ✓ ⇒ 漏掉不会有副作用 ✓，
        // 但能**判定**崩溃是否来自"销毁时那一大批释放" ✓。
        if leak_mode() {
            self.live.borrow_mut().clear();
            self.types.borrow_mut().clear();
            self.gc_head.set(ptr::null_mut());
            self.gc_count.set(0);
            self.bytes_allocated.set(0);
            return;
        }
        let live: Vec<usize> = self.live.borrow().iter().copied().collect();
        for address in live {
            let header = unsafe { NonNull::new_unchecked(address as *mut Header) };
            // SAFETY: 每个地址都由本实例分配且尚未释放；类型对象在下一段之前一直存活。
            unsafe { Self::force_free(header) };
        }

        let types = core::mem::take(&mut *self.types.borrow_mut());
        for ty in types {
            // **保留原样** ✓（理由同上 ✓）：类型对象是 `Box` 分配的 ✓ ⇒ 就按 `Box` 释放 ✓。
            // SAFETY: 类型对象由注册表持有，销毁时统一释放（OM-15）。它的类型就是元类型，
            // 可能已被释放，因此直接按 `TypeObject` 释放，不再读 `ty()`。
            unsafe { TypeObject::dealloc(ty.cast::<Header>().as_ptr()) };
        }

        self.gc_head.set(ptr::null_mut());
        self.gc_count.set(0);
        self.bytes_allocated.set(0);
    }
}

impl Instance {
    /// **OM-2** 的实例销毁路径：不调终结器、不做 clear。
    ///
    /// # Safety
    ///
    /// `header` 必须由本实例分配、尚未释放，且其载荷**不得**持有 `Owned` 守卫。
    unsafe fn force_free(header: NonNull<Header>) {
        // SAFETY: 调用方保证 header 有效；类型对象在本次销毁的第二段才释放。
        let ty = unsafe { header.as_ref() }.ty();
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        // SAFETY: 调用方保证。
        unsafe { dealloc(header.as_ptr()) };
    }
}

