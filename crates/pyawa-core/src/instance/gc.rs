//! `Instance` 的分配后处理/追踪/GC 邻接域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// **新增一份引用**并交回同一对象（给"按原样返回实参"的原生函数用，`OM-16`）。
    pub fn retain(&self, object: NonNull<Header>) -> NonNull<Header> {
        // SAFETY: 调用方保证 object 存活。
        unsafe { self.incref_object(object.as_ptr()) };
        object
    }

    /// **对象真假**（第 131 轮）：直接复用执行器那份判定 ✓（**一处真相** ✓）——
    /// 内建 `bool()` 要的就是它（`bool_value` 只覆盖 bool／None ✗ ⇒ `bool(0)` 会错 ✗）。
    pub fn truthiness_of(&self, object: NonNull<Header>) -> Result<bool, ExecError> {
        crate::executor::iter::truthiness(self, object, 0)
    }

    /// **真值**（`TO_BOOL` 的同一处真相：`all`／`any` 要用）。
    ///
    /// 假：`None`／`False`／数值零／空串／空容器；其余真（没有 `__bool__`／`__len__` 的对象
    /// 按参照实现是**真**）。
    pub fn truth_of(&self, object: NonNull<Header>) -> bool {
        let ty = self.type_of(object);
        if ty == self.singletons().none_type() {
            return false;
        }
        if let Some(flag) = self.bool_value(object) {
            return flag;
        }
        if ty == self.singletons().int_type() {
            // 大整数不能看 `i64` 那个快路径（`int_value` 对它给 `None` ⇒ 会被当成 0＝假）
            return self.int_of(object).map(|value| !value.is_zero()).unwrap_or(false);
        }
        if self.type_named("float") == Some(ty) {
            return self.float_value(object).unwrap_or(0.0) != 0.0;
        }
        if ty == self.singletons().str_type() {
            // P3-38 S2c：真值走安全口径（两表示都认；WTF-8 不再 panic ✓）
            return !self.str_is_empty(object).unwrap_or(false);
        }
        if let Some(length) = self.length_of(object) {
            return length != 0;
        }
        true
    }

    /// **`OM-22`**：对象的**引用计数**（**安全**读取）。
    ///
    /// 给 stdlib 的 `sys.getrefcount` 用——那个 crate 是 `#![forbid(unsafe_code)]`，
    /// 不能自己去 `as_ref()`。
    /// **开始盯住某个地址**（第 113 轮诊断用 ✓）。
    pub fn watch_address(&self, object: NonNull<Header>) {
        self.watch.set(object.as_ptr() as usize);
    }

    /// **OM-25**：当前参与循环回收（`GC_TRACKED`）的对象数。
    pub fn tracked_objects(&self) -> usize {
        self.gc_count.get()
    }
}

impl Instance {
    /// [`Instance::collect`] 的主体；进入前 `gc_running` 已置位。
    pub(crate) fn collect_inner(&self) -> usize {
        let unreachable = self.find_unreachable();
        self.gc_alloc_count.set(0);
        if unreachable.is_empty() {
            return 0;
        }

        // ② 先清弱引用：§10 尚未接线（`HAS_WEAKREFS` 位也还没人置位），
        //    这里是顺序上的占位点——弱引用回调必须早于终结器（OM-27、PEP 442）。

        // ③ 终结器：对每个不可达对象至多调用一次；终结器可以复活对象（OM-20 ①）。
        //    终结期间同样"冻结"这批对象：终结器可能释放环内引用，提前释放会让我们
        //    手里的指针失效——OM-27 要求先全部终结、再统一释放。
        *self.gc_frozen.borrow_mut() = unreachable
            .iter()
            .map(|header| header.as_ptr() as usize)
            .collect();
        for header in &unreachable {
            let ty = unsafe { header.as_ref() }.ty();
            if let Some(finalize) = unsafe { ty.as_ref() }.slots.finalize {
                let header_ref = unsafe { header.as_ref() };
                if !header_ref.has_flag(flags::FINALIZING) {
                    header_ref.set_flag(flags::FINALIZING);
                    // SAFETY: header 是本实例的存活对象。
                    unsafe { finalize(header.as_ptr(), self) };
                }
            }
        }

        // 终结器可能复活对象、也可能让别的对象重新变可达（PEP 442）⇒ 重算不可达集合。
        let unreachable = self.find_unreachable();
        let garbage: HashSet<usize> = unreachable.iter().map(|h| h.as_ptr() as usize).collect();

        // 复活的对象要清掉 FINALIZING，之后它再次死亡时还能再终结一次（OM-20）。
        for header in self.gc_headers() {
            let header_ref = unsafe { header.as_ref() };
            if header_ref.has_flag(flags::FINALIZING) && !garbage.contains(&(header.as_ptr() as usize))
            {
                header_ref.clear_flag(flags::FINALIZING);
            }
        }

        if unreachable.is_empty() {
            self.gc_frozen.borrow_mut().clear();
            return 0;
        }

        // ④ 释放。先"冻结"这批对象：`clear` 之间的 decref 只减计数，不立即释放——
        //    否则同一环里的对象会被逐个提前释放，而本函数还持有它们的指针。
        *self.gc_frozen.borrow_mut() = garbage;
        for header in &unreachable {
            let ty = unsafe { header.as_ref() }.ty();
            if let Some(clear) = unsafe { ty.as_ref() }.slots.clear {
                // SAFETY: header 是本实例的存活对象，且尚未释放（刚被冻结）。
                unsafe { clear(header.as_ptr(), self) };
            }
            // **OM-14**：另行挂载的实例字典在同一阶段交出去（`OM-27` ④）
            if let Some(mapping) = unsafe { header.as_ref() }.take_instance_dict() {
                // SAFETY: 这份引用由该对象持有。
                unsafe { self.release_object(mapping.as_ptr()) };
            }
        }
        self.gc_frozen.borrow_mut().clear();

        for header in &unreachable {
            self.free_garbage(*header);
        }
        unreachable.len()
    }

    /// **OM-20**：单个对象的释放三步（正常引用计数路径）。
    pub(crate) fn release_one(&self, ptr: NonNull<Header>) {
        let ty = unsafe { ptr.as_ref() }.ty();

        // ① 终结器（`__del__`）：置 FINALIZING 防止重入；可以复活（把计数改回 > 0）。
        if let Some(finalize) = unsafe { ty.as_ref() }.slots.finalize {
            let header = unsafe { ptr.as_ref() };
            if !header.has_flag(flags::FINALIZING) {
                header.set_flag(flags::FINALIZING);
                // SAFETY: ptr 是本实例的存活对象，计数已归零且仍在待处理栈上。
                unsafe { finalize(ptr.as_ptr(), self) };
                let header = unsafe { ptr.as_ref() };
                if header.refcount() != 0 {
                    // 复活：清除 FINALIZING 并**放弃释放**（OM-20）。
                    header.clear_flag(flags::FINALIZING);
                    return;
                }
            }
        }

        // ② 清空持有的引用：释放它们（可能再入待处理栈）。
        if let Some(clear) = unsafe { ty.as_ref() }.slots.clear {
            // SAFETY: 同 ①。
            unsafe { clear(ptr.as_ptr(), self) };
        }
        // ②′ **OM-14**：另行挂载的实例字典也是本对象持有的一份引用
        if let Some(mapping) = unsafe { ptr.as_ref() }.take_instance_dict() {
            // SAFETY: 这份引用由本对象持有。
            unsafe { self.release_object(mapping.as_ptr()) };
        }

        // ③ 释放内存。
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(ptr);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        // **实验（第 238 轮）**：暂不真释放 ✓ ⇒ 崩溃消失即证明"释放后仍被用／写" ✗。
        if quarantine_mode() {
            self.quarantine_put(ptr);
        } else if !leak_mode() {
            // SAFETY: 计数为 0，且 clear 已把持有的引用交出（OM-20 ③ 的前提）。
            unsafe { dealloc(ptr.as_ptr()) };
        }
    }

    /// **OM-27** ④：释放一个不可达对象（`clear` 已经跑过，这里不再调终结器）。
    pub(crate) fn free_garbage(&self, header: NonNull<Header>) {
        let ty = unsafe { header.as_ref() }.ty();
        // **OM-14**：正常路径下 clear 阶段已经把这一格交出去了；万一还在，这里补一次释放
        if let Some(mapping) = unsafe { header.as_ref() }.take_instance_dict() {
            // SAFETY: 这份引用由该对象持有（或曾经持有）。
            unsafe { self.release_object(mapping.as_ptr()) };
        }
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(header);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        if quarantine_mode() {
            self.quarantine_put(header);
        } else if !leak_mode() {
            // SAFETY: 该对象已由可达性分析判为不可达，且 clear 已完成。
            unsafe { dealloc(header.as_ptr()) };
        }
    }

    /// **OM-29**／**OM-30**：求不可达的跟踪对象。
    ///
    /// 两步：先按"引用计数 − 来自跟踪对象内部的引用数"找出根（外部引用 > 0），
    /// 再从根出发按 `traverse` 标记；没被标记的就是不可达集合。
    pub(crate) fn find_unreachable(&self) -> Vec<NonNull<Header>> {
        let candidates = self.gc_headers();
        if candidates.is_empty() {
            return Vec::new();
        }

        let index: HashMap<usize, usize> = candidates
            .iter()
            .enumerate()
            .map(|(position, candidate)| (candidate.as_ptr() as usize, position))
            .collect();

        let mut external: Vec<u32> = candidates
            .iter()
            // SAFETY: 候选都在本实例的回收链表上，即尚未释放。
            .map(|candidate| unsafe { candidate.as_ref() }.refcount())
            .collect();

        for candidate in &candidates {
            for child in self.children_of(*candidate) {
                if let Some(&position) = index.get(&(child as usize)) {
                    external[position] = external[position].saturating_sub(1);
                }
            }
        }

        let mut marked = vec![false; candidates.len()];
        let mut stack: Vec<usize> = (0..candidates.len()).filter(|i| external[*i] > 0).collect();
        while let Some(position) = stack.pop() {
            if marked[position] {
                continue;
            }
            marked[position] = true;
            for child in self.children_of(candidates[position]) {
                if let Some(&child_position) = index.get(&(child as usize)) {
                    if !marked[child_position] {
                        stack.push(child_position);
                    }
                }
            }
        }

        candidates
            .iter()
            .zip(marked)
            .filter(|(_, reached)| !*reached)
            .map(|(candidate, _)| *candidate)
            .collect()
    }

    /// 按 `traverse` 槽位取一个对象的直接引用（**OM-12**／**OM-29**／**OM-36**）。
    pub(crate) fn children_of(&self, header: NonNull<Header>) -> Vec<*mut Header> {
        let ty = unsafe { header.as_ref() }.ty();
        let mut children = Vec::new();
        // **OM-14**／**OM-36**：另行挂载的实例字典也算本对象的直接引用（漏报会永久泄漏）
        if let Some(mapping) = unsafe { header.as_ref() }.instance_dict() {
            children.push(mapping.as_ptr());
        }
        if let Some(traverse) = unsafe { ty.as_ref() }.slots.traverse {
            // SAFETY: header 是本实例的存活对象；回调只收集指针，不做解引用。
            unsafe { traverse(header.as_ptr(), &mut |child| children.push(child)) };
        }
        children
    }

    /// 回收链表上的全部对象（**OM-25**：只有 `GC_TRACKED` 入链）。
    pub(crate) fn gc_headers(&self) -> Vec<NonNull<Header>> {
        let mut result = Vec::with_capacity(self.gc_count.get());
        let mut cursor = self.gc_head.get();
        while !cursor.is_null() {
            // SAFETY: 链上的指针都由本实例分配且尚未释放。
            result.push(unsafe { NonNull::new_unchecked(cursor) });
            cursor = unsafe { (*cursor).gc_next() };
        }
        result
    }

    pub(crate) fn link_gc(&self, header: NonNull<Header>) {
        let head = self.gc_head.get();
        // SAFETY: header 刚分配；head 若非空则它是链上存活对象。
        unsafe {
            header.as_ref().set_gc_prev(ptr::null_mut());
            header.as_ref().set_gc_next(head);
            if !head.is_null() {
                (*head).set_gc_prev(header.as_ptr());
            }
        }
        self.gc_head.set(header.as_ptr());
        self.gc_count.set(self.gc_count.get() + 1);
    }

    pub(crate) fn unlink_gc(&self, header: NonNull<Header>) {
        // SAFETY: header 在本实例的回收链表上。
        let (prev, next) = unsafe {
            let header_ref = header.as_ref();
            (header_ref.gc_prev(), header_ref.gc_next())
        };
        if prev.is_null() {
            self.gc_head.set(next);
        } else {
            // SAFETY: prev 是链上存活对象。
            unsafe { (*prev).set_gc_next(next) };
        }
        if !next.is_null() {
            // SAFETY: next 是链上存活对象。
            unsafe { (*next).set_gc_prev(prev) };
        }
        // SAFETY: 同上。
        unsafe {
            header.as_ref().set_gc_prev(ptr::null_mut());
            header.as_ref().set_gc_next(ptr::null_mut());
        }
        self.gc_count.set(self.gc_count.get() - 1);
    }

    /// **尺子** ✓（第 184 轮把当时那次**临时**手法**常驻**下来 ✓）：释放前问"**还有谁指着这个地址**" ✓。
    ///
    /// **与布局无关** ✓：靠每个类型的 `traverse` 槽（`OM-12` ✓）**扫全图** ✓；由 `PYAWA_RULER=1` 门控 ✓。
    /// **注意** ✗：成环的成员之间会**互相指** ✓ ⇒ 输出里出现环成员是**预期**的 ✓；
    /// 但若出现"**该对象已无人引用**却仍被某个类型指着" ✗，那就是**释放后用**的现场 ✓。
    pub(crate) fn who_points_at(&self, address: usize) -> Vec<(usize, String)> {
        let live: Vec<usize> = self.live.borrow().iter().copied().collect();
        let mut found = Vec::new();
        for item in live {
            let header = item as *mut Header;
            // SAFETY: `live` 里的地址都是存活对象 ✓。
            let ty = unsafe { &*header }.ty();
            // SAFETY: ty 由注册表持有 ✓。
            let Some(traverse) = (unsafe { ty.as_ref() }).slots.traverse else {
                continue;
            };
            let mut hit = false;
            // SAFETY: 槽位由类型提供，契约见 OM-12 ✓。
            unsafe {
                traverse(header, &mut |child| {
                    if child as usize == address {
                        hit = true;
                    }
                });
            }
            if hit {
                found.push((item, self.type_name(ty)));
            }
        }
        found
    }

}
