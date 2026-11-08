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
    /// **`gc.get_debug()`**（round 92）：返回 `set_debug` 设的位（默认 0 ✓）。
    pub fn gc_debug(&self) -> u64 {
        self.gc_debug.get()
    }

    /// **`gc.set_debug(flags)`**（round 92）：忠实保存（本层没有调试通道 ⇒ 只有可观测的读回 ✓）。
    pub fn set_gc_debug(&self, flags: u64) {
        self.gc_debug.set(flags);
    }

    /// **`gc.get_count()`**（round 92）：`(自上次回收以来的分配数, 0, 0)` —— 本层是**单代**回收
    /// （「无分代计数器」是已登记的偏差 ✗）⇒ 第 2／3 格恒 0 ✓（如实登记，不编假值 ✓）。
    pub fn gc_get_count(&self) -> (usize, usize, usize) {
        (self.gc_alloc_count.get(), 0, 0)
    }

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
            // **诊断（round 53）**：`PYAWA_GC_AUDIT=1` 时，若"已 clear 完"的对象计数仍非 0
            // （正常应恰为 0：环内引用已互相释放；非 0 ⇒ 有**环外仍可达**的引用 ⇒ 一旦释放就是
            // 先用后放 ✗）⇒ 打印现场并**跳过释放**（宁可泄漏，先把崩溃变成可观察的诊断 ✓）。
            if crate::diag::flag("PYAWA_GC_AUDIT") {
                let remaining = unsafe { header.as_ref() }.refcount();
                if remaining != 0 {
                    // SAFETY: header 尚未释放（本循环里我们会跳过它）。
                    let ty = unsafe { header.as_ref() }.ty();
                    let name = unsafe { &*ty.as_ref() }.name();
                    eprintln!(
                        "[gc-audit] 不可达对象计数={remaining} 类型={name} 地址={:#x}",
                        header.as_ptr() as usize
                    );
                    continue;
                }
            }
            self.free_garbage(*header);
        }
        unreachable.len()
    }

    /// **OM-20**：单个对象的释放三步（正常引用计数路径）。
    pub(crate) fn release_one(&self, ptr: NonNull<Header>) {
        let ty = unsafe { ptr.as_ref() }.ty();
        // **终结延迟判定的重放**（round 82）：`release_one` 正常情况下只在计数 0 时被调用；
        // 但 "终结器自身帧的形参延迟释放" 那条路会把对象**先入队、后重放** —— 重放时计数若已
        // 回不到 0，就是**真复活** ⇒ 只清 `FINALIZING`、不释放（照 `OM-20` 复活语义，之后再死
        // 还能再终结一次 ✓）；计数为 0 ⇒ 终结已做过（`FINALIZING` 保持），直接走清引用/释放 ✓。
        {
            let header = unsafe { ptr.as_ref() };
            if header.has_flag(flags::FINALIZING) && header.refcount() != 0 {
                header.clear_flag(flags::FINALIZING);
                return;
            }
        }

        // ① 终结器（`__del__`）：置 FINALIZING 防止重入；可以复活（把计数改回 > 0）。
        // **口径照本层 `OM-20`**：复活的对象清掉 FINALIZING ⇒ 之后再次死亡时**还能**再终结一次
        // （`object_model` 的 `resurrection_aborts_the_release` 等用例钉住这条；参照的 `tp_finalize`
        // 是"至多一次" ⇒ 这是本层**已登记**的偏差，别在这里改成 once-only ✗）。
        if let Some(finalize) = unsafe { ty.as_ref() }.slots.finalize {
            let header = unsafe { ptr.as_ref() };
            if !header.has_flag(flags::FINALIZING) {
                header.set_flag(flags::FINALIZING);
                // **终结窗口里临时自持一份**（CPython `PyObject_CallFinalizerFromDealloc` 同款；
                // `P3-41` round 43 修 ✗✗）：`__del__` 的帧会持有 `self` 形参（当时口径：形参所有权
                // **不在帧**）⇒ 之前计数会**在终结器返回前回到 0** ⇒ 内层 `release_object` 重入
                // `release_one` ⇒ 当场 `clear`＋释放正在终结的对象 ⇒ 外层复活判断读到悬垂指针／
                // 用例里 `__del__` 被反复调用（实测 `test_dict` 的 `test_clear_reentrant_delete`
                // 无限打 `del-key/keyerror`）。自持一份后，终结器里的 decref 最低也停在 1 ✓；
                // 返回后**直接减计数**（不走释放流程）再看是否真复活 ✓。
                // **诊断**（round 90，门控 `PYAWA_DEL_TRACE=1`）：终结前后打印计数与引用类型，
                // 用于"帧所有权统一"实验期间定位复活判定（那次实验已撤回，见 round 90 台账 ✓）。
                let trace = crate::diag::flag("PYAWA_DEL_TRACE");
                if trace {
                    // SAFETY: ptr 是存活对象；类型对象由注册表持有 ✓。
                    let ty_name = unsafe { (*ptr.as_ptr()).ty().as_ref().name() };
                    eprintln!(
                        "[del] 终结前 ptr={:p} rc={} ty={ty_name}",
                        ptr.as_ptr(),
                        header.refcount()
                    );
                    if ty_name == "A" {
                        self.watch_address(ptr);
                    }
                }
                // SAFETY: ptr 是本实例的存活对象，计数已归零且仍在待处理栈上。
                unsafe { self.incref_object(ptr.as_ptr()) };
                // SAFETY: 同上。
                unsafe { finalize(ptr.as_ptr(), self) };
                let header = unsafe { ptr.as_ref() };
                let remaining = header.decref();
                if trace {
                    eprintln!("[del] 终结后 rc={remaining}");
                }
                if remaining != 0 {
                    // **先别下"复活"结论**（round 82 真 bug 修 ✗）：终结器自己的 Python 帧可能还
                    // 持有 `self` 形参（形参所有权归帧 ✓），而它的 `release_object` 在**清空栈重入**
                    // 时被**延迟**到本轮 `release_one` 之后 ⇒ 这里看到的 `remaining` 是"临时引用"
                    // 而非复活（实测：`保活后 rc=1`／`finalize 返回后 rc=2`／`终结后 rc=1`，随后
                    // 帧释放才把 rc 打到 0 ⇒ 旧代码清 FINALIZING ⇒ 下一轮 `release_one` 再终结
                    // ⇒ `__del__` 无限重复）。
                    // 做法：**保持 `FINALIZING`**，把对象塞回待处理栈**队首**（让终结器帧先被释放）；
                    // 重放时 `release_one` 顶部会按彼时计数判定真复活还是直接释放 ✓。
                    self.pending.borrow_mut().insert(0, ptr);
                    return;
                }
            }
        }

        // **弱引用失效**（round 59；参照的顺序：终结器之后、真正清引用之前 ✓）：
        // ① 指向本对象的弱引用全部置死并**立即**调回调（复活路径已在上面的 `return` 里让开 ✓）；
        // ② 本对象若是弱引用本体，从目标的登记表里摘掉自己 ✓。
        self.invalidate_weakrefs(ptr);
        self.forget_weakref_object(ptr);

        // **先摘出 GC 链表再清引用**（`P3-41` round 43 修 ✗✗）：`clear` 会释放持有的引用，
        // 里面可能跑 `__del__`（`test_dict.test_store_evilattr` 的 `__del__` 里就 `gc.collect()`）⇒
        // 若对象仍挂在回收链表上、且计数已是 0 ⇒ 回收器把它当"不可达"再 `clear` 一次 ⇒
        // 同一个 `Frame` 的 `code: RefCell` 二次借用（实测 `RefCell already borrowed` 的 panic／
        // release 下直接段错误 ✓）。CPython 的 `tp_dealloc` 同样先 `PyObject_GC_UnTrack` ✓。
        self.unlink(ptr);

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

        // ③ 释放内存。（旁表条目随对象释放摘掉 ✓，round 82）
        self.forget_python_finalized(ptr.as_ptr() as usize);
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        // **小元组进复用池**（round 96）：参照 tuple freelist ✓（`test_genexps` 的 id 复用 doctest ✓）。
        if !quarantine_mode() && !leak_mode() && self.pool_tuple(ptr, ty) {
            return;
        }
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
        // 旁表条目随对象释放摘掉 ✓（round 82）。
        self.forget_python_finalized(header.as_ptr() as usize);
        let ty = unsafe { header.as_ref() }.ty();
        // **弱引用失效**（round 59，GC 路径）：回调**排队**，等整个回收循环结束再调 ✓
        // （避免在 clear／free 的借用窗口里跑任意 Python 代码 ✗）。
        let callbacks = self.take_weakref_callbacks(header);
        self.queue_weakref_callbacks(callbacks);
        self.forget_weakref_object(header);
        // **OM-14**：正常路径下 clear 阶段已经把这一格交出去了；万一还在，这里补一次释放
        if let Some(mapping) = unsafe { header.as_ref() }.take_instance_dict() {
            // SAFETY: 这份引用由该对象持有（或曾经持有）。
            unsafe { self.release_object(mapping.as_ptr()) };
        }
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(header);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        if !quarantine_mode() && !leak_mode() && self.pool_tuple(header, ty) {
            return;
        }
        if quarantine_mode() {
            self.quarantine_put(header);
        } else if !leak_mode() {
            // SAFETY: 该对象已由可达性分析判为不可达，且 clear 已完成。
            unsafe { dealloc(header.as_ptr()) };
        }
    }

    /// **把已释放的 `tuple` 放进复用池**（round 96；上限 20，参照 freelist 同量级 ✓）。
    /// 返回 `true` ⇒ 调用方**不要**再调 `dealloc`（内存留在池里 ✓）。
    fn pool_tuple(&self, ptr: NonNull<Header>, ty: NonNull<crate::TypeObject>) -> bool {
        // SAFETY: ty 由注册表持有；ptr 是刚释放的对象（内存仍有效 ✓）。
        if unsafe { ty.as_ref() }.name() != "tuple" {
            return false;
        }
        let mut pool = self.tuple_pool.borrow_mut();
        if pool.len() >= 20 {
            return false;
        }
        pool.push(ptr);
        true
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
