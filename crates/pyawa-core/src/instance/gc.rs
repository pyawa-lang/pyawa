//! `Instance` 的分配后处理/追踪/GC 邻接域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use crate::HotId;
use super::*;

/// `release_one` 的"正在释放"深度守卫（round 256）：Drop 时把深度减回去（嵌套释放也正确 ✓）。
struct ReleasingGuard<'a> {
    instance: &'a Instance,
}

impl<'a> ReleasingGuard<'a> {
    fn new(instance: &'a Instance) -> Self {
        instance.releasing.set(instance.releasing.get() + 1);
        Self { instance }
    }
}

impl Drop for ReleasingGuard<'_> {
    fn drop(&mut self) {
        self.instance.releasing.set(self.instance.releasing.get() - 1);
    }
}

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
        if self.hot_type(HotId::Float) == Some(ty) {
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
        // **孤儿用户类回收**（round 99，实验开关 `PYAWA_ORPHAN_RECLAIM=1` 才开）：清掉"只剩注册表
        // 引用"的用户类字典 ⇒ 其命名空间里的环由后面的普通回收/引用计数级联处理 ✓。
        // 简单夹具已能收环（`test_free_after_iterating` 的风格 ✓），但 `unittest.assertRaises` 那条
        // 构造里类型还多一份引用（`rc=2`）⇒ 当前判据不收；默认关 ⇒ 套件行为与 round 98 前一致 ✓。
        // **默认开**（round 104 起；逃生阀 `PYAWA_ORPHAN_RECLAIM=0`）：用户类类型是 `Box::leak` 分配的、
        // 不进 `gc_headers`，类环（类型→命名空间→函数→cell→迭代器→实例）只能靠这里收 ✓。
        // **回收时机（round 128）**：孤儿回收只在**显式 `gc.collect()`** 时做（`PYAWA_ORPHAN_RECLAIM=1` 可强制每次；
        // `=0` 关闭 ✓）。理由：① 自动回收里做"全类型扫描＋清类字典"成本高（根集合还要扫模块表/命名空间 ✓）；
        // ② 导入期（模块已登记 `sys.modules`、类还在建）反复触发容易踩"半成品类" ✗；③ `free_after` 家族判据都显式
        // 调 `gc.collect()` ✓ ⇒ 语义不丢；④ 让自动回收回归"纯循环垃圾"，类型级清理集中到显式时机 ✓。
        static ORPHAN_RECLAIM: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        let reclaim_env = ORPHAN_RECLAIM.get_or_init(|| std::env::var("PYAWA_ORPHAN_RECLAIM").ok());
        if reclaim_env.as_deref() != Some("0")
            && !self.gc_skip_reclaim.get()
            && (self.gc_explicit.get() || reclaim_env.as_deref() == Some("1"))
        {
            self.reclaim_orphan_user_types();
        }
        // **年轻代候选**（round 157）：`collect(0)`／`collect(1)` 只收"上次回收以来的新对象"。
        let young = self.gc_skip_reclaim.get();
        let candidates = if young {
            self.gc_headers_young()
        } else {
            self.gc_headers()
        };
        let unreachable = self.find_unreachable_in(&candidates);
        if crate::diag::flag_site!("PYAWA_GC_ROOTS_DEBUG") && !unreachable.is_empty() {
            let mut names: Vec<String> = Vec::new();
            for header in &unreachable {
                // SAFETY: 候选存活。
                let ty = unsafe { header.as_ref() }.ty();
                names.push(unsafe { ty.as_ref() }.name().to_owned());
            }
            eprintln!("[gc-unreachable] {} {:?}", unreachable.len(), names);
        }
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
        let mut any_finalized = false;
        for header in &unreachable {
            let ty = unsafe { header.as_ref() }.ty();
            if let Some(finalize) = unsafe { ty.as_ref() }.slots.finalize {
                let header_ref = unsafe { header.as_ref() };
                if !header_ref.has_flag(flags::FINALIZING) {
                    header_ref.set_flag(flags::FINALIZING);
                    any_finalized = true;
                    // SAFETY: header 是本实例的存活对象。
                    unsafe { finalize(header.as_ptr(), self) };
                }
            }
        }

        // 终结器可能复活对象、也可能让别的对象重新变可达（PEP 442）⇒ 重算不可达集合。
        // **没跑过任何终结器 ⇒ 可达性与第一次相同**（round 110：省一次全堆标记 ✓）。
        let unreachable = if any_finalized {
            self.find_unreachable_in(&candidates)
        } else {
            unreachable
        };
        let garbage: crate::fast_hash::FastSet<usize> = unreachable.iter().map(|h| h.as_ptr() as usize).collect();

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
            if crate::diag::flag_site!("PYAWA_GC_AUDIT") {
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
        // **释放中标记**（round 256 内存安全修）：终结器／弱引用回调里重入的 `gc.collect()`
        // 必须等这条释放链排空（见 `Instance::releasing` 的注释）。
        let _releasing_guard = ReleasingGuard::new(self);
        let ty = unsafe { ptr.as_ref() }.ty();
        self.unregister_user_instance(ty, ptr);
        // **`AB-75`**：对象释放 ⇒ 摘掉它的宿主协议钩子（地址复用不会串味）。
        self.forget_object_ops(ptr);
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
                let trace = crate::diag::flag_site!("PYAWA_DEL_TRACE");
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
        // **`mappingproxy` 活视图登记要注销**（round 195 真 bug 修 ✗）：否则代理地址被后续新
        // dict 复用时会被"陈旧登记"误认成活视图（并把老源条目拷进新 dict ✗）。
        if unsafe { ty.as_ref() }.name() == "mappingproxy" {
            self.forget_mappingproxy(ptr.as_ptr());
        }
        // ②′ **OM-14**：另行挂载的实例字典也是本对象持有的一份引用
        if let Some(mapping) = unsafe { ptr.as_ref() }.take_instance_dict() {
            // SAFETY: 这份引用由本对象持有。
            unsafe { self.release_object(mapping.as_ptr()) };
        }

        // **`traceback` 计数**（round 191 诊断）：真正释放前记一笔 ✓。
        if unsafe { ty.as_ref() }.name() == "traceback" {
            self.traceback_freed
                .set(self.traceback_freed.get() + 1);
            static TB_DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            if crate::diag::flag_cached("PYAWA_TB_DEBUG", &TB_DEBUG) {
                eprintln!(
                    "[tb] freed ptr={:p} count={}",
                    ptr.as_ptr(),
                    self.traceback_freed.get()
                );
            }
        }
        // ③ 释放内存。（旁表条目随对象释放摘掉 ✓，round 82）
        if crate::diag::flag_site!("PYAWA_STR_FREE_BT") {
            // SAFETY: ty 由注册表持有。
            let ty_name = unsafe { ty.as_ref() }.name();
            if ty_name == "str" {
                eprintln!("[str-free] ptr={:p}\n{}", ptr.as_ptr(), std::backtrace::Backtrace::force_capture());
            }
        }
        self.forget_python_finalized(ptr.as_ptr() as usize);
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        self.release_accounting(ty, ptr);
        // **小元组进复用池**（round 96）：参照 tuple freelist ✓（`test_genexps` 的 id 复用 doctest ✓）。
        if !quarantine_mode() && !leak_mode() && (self.pool_int(ptr, ty) || self.pool_tuple(ptr, ty)) {
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
        self.unregister_user_instance(unsafe { header.as_ref() }.ty(), header);
        // 旁表条目随对象释放摘掉 ✓（round 82）。
        self.forget_python_finalized(header.as_ptr() as usize);
        // **`AB-75`**：宿主对象协议钩子的条目也随对象释放摘掉（地址复用不会串味 ✓）。
        self.forget_object_ops(header);
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
        // **释放前写毒值**（round 107 诊断；`PYAWA_REFCOUNT_CANARY=1` 才写 ✓）：之后任何
        // `incref`/`release` 碰它就当场 panic，能定位"第二次释放"的 Rust 路径 ✓。
        crate::instance::poison_refcount(header);
        self.unlink(header);
        self.release_accounting(ty, header);
        if !quarantine_mode() && !leak_mode() && (self.pool_int(header, ty) || self.pool_tuple(header, ty)) {
            return;
        }
        if quarantine_mode() {
            self.quarantine_put(header);
        } else if !leak_mode() {
            // SAFETY: 该对象已由可达性分析判为不可达，且 clear 已完成。
            unsafe { dealloc(header.as_ptr()) };
        }
    }

    /// **把已释放的 `int` 放进复用池**（round 251 性能，A2-6；上限 64）：载荷重置成小值
    /// ⇒ 不钉住大整数的堆 ✓（赋值会 drop 旧 `IntValue` ✓）。
    #[inline]
    fn pool_int(&self, ptr: NonNull<Header>, ty: NonNull<crate::TypeObject>) -> bool {
        if !self.hot_type_is(HotId::Int, ty) {
            return false;
        }
        let mut pool = self.int_pool.borrow_mut();
        if pool.len() >= 64 {
            return false;
        }
        // SAFETY: ptr 是刚释放的对象（内存仍有效、尚未 dealloc ✓），类型刚确认是 `int` ✓。
        unsafe {
            (*ptr.cast::<crate::builtin_objects::IntObject>().as_ptr()).value =
                crate::bigint::IntValue::Small(0);
        }
        pool.push(ptr);
        true
    }

    /// **把已释放的 `tuple` 放进复用池**（round 96；上限 20，参照 freelist 同量级 ✓）。
    /// 返回 `true` ⇒ 调用方**不要**再调 `dealloc`（内存留在池里 ✓）。
    #[inline]
    fn pool_tuple(&self, ptr: NonNull<Header>, ty: NonNull<crate::TypeObject>) -> bool {
        if !self.hot_type_is(HotId::Tuple, ty) {
            return false;
        }
        let mut pool = self.tuple_pool.borrow_mut();
        if pool.len() >= 20 {
            return false;
        }
        pool.push(ptr);
        true
    }

    /// **解释器栈根**（round 125）：从 `current_frame()` 起沿 `f_back` 链的全部帧地址 —— 这些帧
    /// （以及它们能到达的对象）必须当**根** ✓（参照口径：解释器栈是根）。先前只按"外部引用数"找根；
    /// 若栈对帧的那份引用**没计数**（`current_frame` 是旁表指针）⇒ 栈上帧可能被判不可达 ✗ ⇒ 过度回收
    /// （round 124 的 `test_rlcompleter` 帧槽悬垂即此类）。这里把整条帧链强制标根 ✓。
    fn interpreter_stack_roots(&self) -> Vec<usize> {
        let mut roots = Vec::new();
        let mut at = self.current_frame();
        let mut guard = 0usize;
        while let Some(frame) = at {
            roots.push(frame.as_ptr() as usize);
            // SAFETY: 当前帧链上的帧都存活（由 `CurrentFrameGuard` 持有 ✓）。
            let frame_ref = unsafe { &*frame.as_ptr().cast::<crate::frame::Frame>() };
            at = frame_ref.f_back();
            guard += 1;
            if guard > 4096 {
                break;
            }
        }
        roots
    }

    /// **模块对象也是根**（round 126）：`sys.modules` 对模块对象的引用是否被计数取决于导入实现；
    /// 若那份引用没计数，模块对象（AttributeObject）及其命名空间 dict 会被判不可达 ⇒ 模块里的**类**
    /// 会被判孤儿 ⇒ 误回收（实测 `test_dict` 下 argparse/unittest 的类被大量回收 ✗）。参照口径：
    /// 模块由 `sys.modules` 持有 ⇒ 恒可达；这里把每个模块对象标根 ✓（其 dict/值随之可达 ✓）。
    fn module_roots(&self) -> Vec<usize> {
        let mut roots = Vec::new();
        let Some(modules) = self.modules() else {
            return roots;
        };
        if let Some(entries) = self.dict_entries(modules) {
            for (_key, value) in entries {
                roots.push(value.as_ptr() as usize);
                // **模块的命名空间 dict 也直接标根**（round 127）：模块对象是 `AttributeObject` 且用
                // **外部实例字典**（不内联）⇒ `AttributeObject::traverse` 只走内联字典 ✗ ⇒ 只标模块对象时
                // 它的命名空间 dict/类仍会被判不可达（实测：`re.RegexFlag`／`signal.Signals` 等 enum 类被
                // 回收 ✗）。这里把 dict 也当根 ✓。
                if let Some(namespace) = crate::executor::protocol::instance_attributes(self, value) {
                    roots.push(namespace.as_ptr() as usize);
                }
            }
        }
        roots
    }

    /// **OM-29**／**OM-30**：求不可达的跟踪对象。
    ///
    /// 两步：先按"引用计数 − 来自跟踪对象内部的引用数"找出根（外部引用 > 0），
    /// 再从根出发按 `traverse` 标记；没被标记的就是不可达集合。

    /// **给定候选集**求不可达（round 157 分代）：`gc.collect(0)` 只把**年轻段**当候选 ✓
    /// —— 老对象不扫、不判、不释放；它们指向年轻对象的引用按"外部引用"计 ⇒ 年轻人不会被误收 ✓。
    pub(crate) fn find_unreachable_in(
        &self,
        candidates: &[NonNull<Header>],
    ) -> Vec<NonNull<Header>> {
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

        // **反向边探针**（round 165，`PYAWA_GC_REVERSE=1`）：记"谁引用了我"（首个候选父）；
        // 用来追"外部计数 >0 却找不到候选父"的对象（= 被**非候选**持有者引用）。
        let reverse = crate::diag::flag_site!("PYAWA_GC_REVERSE");
        let mut parent_of: Vec<Option<usize>> = if reverse {
            vec![None; candidates.len()]
        } else {
            Vec::new()
        };
        for (parent_position, candidate) in candidates.iter().enumerate() {
            for child in self.children_of(*candidate) {
                if let Some(&position) = index.get(&(child as usize)) {
                    external[position] = external[position].saturating_sub(1);
                    if reverse && parent_of[position].is_none() {
                        parent_of[position] = Some(parent_position);
                    }
                }
            }
        }

        let mut marked = vec![false; candidates.len()];
        let mut stack: Vec<usize> = (0..candidates.len()).filter(|i| external[*i] > 0).collect();
        // **解释器栈上的帧 ＋ sys.modules 里的模块对象**都是根（round 125／126）✓。
        let mut extra_roots = self.interpreter_stack_roots();
        extra_roots.extend(self.module_roots());
        for address in extra_roots {
            if let Some(&position) = index.get(&address) {
                if !marked[position] {
                    stack.push(position);
                }
            }
        }
        if reverse {
            let mut shown = 0usize;
            for position in &stack {
                // SAFETY: 候选存活。
                let header = unsafe { candidates[*position].as_ref() };
                let ty = header.ty();
                let name = unsafe { ty.as_ref() }.name();
                let reverse_all = crate::diag::flag_site!("PYAWA_GC_REVERSE_ALL");
                if !reverse_all && (name != "AttributeError" || shown >= 12) {
                    continue;
                }
                if reverse_all && shown >= 80 {
                    continue;
                }
                shown += 1;
                let mut chain: Vec<String> = Vec::new();
                let mut at = *position;
                for _ in 0..6 {
                    match parent_of[at] {
                        Some(parent) => {
                            let pty = unsafe { candidates[parent].as_ref() }.ty();
                            chain.push(unsafe { pty.as_ref() }.name().to_owned());
                            at = parent;
                        }
                        None => {
                            chain.push("<非候选持有者>".to_owned());
                            break;
                        }
                    }
                }
                let header = unsafe { candidates[*position].as_ref() };
                let pointer = candidates[*position].as_ptr() as usize;
                if reverse_all {
                    eprintln!(
                        "[gc-reverse] {name} rc={} ext={} parents={chain:?} holders={:?} site={:?}",
                        header.refcount(),
                        external[*position],
                        self.debug_holder_of(pointer),
                        self.exception_site_of(candidates[*position]),
                    );
                } else {
                    eprintln!(
                        "[gc-reverse] AttributeError rc={} ext={} parents={chain:?} holders={:?} site={:?}",
                        header.refcount(),
                        external[*position],
                        self.debug_holder_of(pointer),
                        self.exception_site_of(candidates[*position])
                    );
                }
            }
        }
        // **回溯/异常"是否可达"诊断**（round 192）：`PYAWA_GC_REVERSE_EXTRA=1` 时把
        // `traceback` 与异常族对象**逐个**打出：`rc`／`ext`／（反向父链）——判断泄漏的回溯
        // 到底是"根"还是"被某个根链拉住"。
        if crate::diag::flag_site!("PYAWA_GC_REVERSE_EXTRA") && reverse {
            let mut shown = 0usize;
            for (position, candidate) in candidates.iter().enumerate() {
                // SAFETY: 候选存活。
                let header = unsafe { candidate.as_ref() };
                let ty = header.ty();
                let name = unsafe { ty.as_ref() }.name();
                let interesting = name == "traceback"
                    || self.instance_name_is_exception(unsafe { ty.as_ref() });
                if !interesting || shown >= 40 {
                    continue;
                }
                // **只打"还在"的**：`rc>0` 且（`ext>0` 或 `parents` 能走到根）✓
                shown += 1;
                let mut chain: Vec<String> = Vec::new();
                let mut at = position;
                for _ in 0..8 {
                    match parent_of[at] {
                        Some(parent) => {
                            let pty = unsafe { candidates[parent].as_ref() }.ty();
                            chain.push(unsafe { pty.as_ref() }.name().to_owned());
                            at = parent;
                        }
                        None => {
                            chain.push("<非候选持有者>".to_owned());
                            break;
                        }
                    }
                }
                eprintln!(
                    "[gc-extra] {name} ptr={:p} rc={} ext={} parents={chain:?}",
                    candidate.as_ptr(),
                    header.refcount(),
                    external[position]
                );
            }
        }
        // **根帧诊断**（round 159）：哪些 frame 还是根（>0 外部引用）⇒ 打出 code 名。
        if crate::diag::flag_site!("PYAWA_GC_ROOT_FRAMES") {
            let mut shown = 0usize;
            for position in &stack {
                // SAFETY: 候选存活。
                let header = unsafe { candidates[*position].as_ref() };
                let ty = header.ty();
                let name = unsafe { ty.as_ref() }.name();
                if name != "frame" {
                    continue;
                }
                if shown >= 40 {
                    break;
                }
                shown += 1;
                // SAFETY: 类型名确认。
                let frame = unsafe { &*candidates[*position].as_ptr().cast::<crate::Frame>() };
                let code = frame.code().map(|code| {
                    // SAFETY: code 由帧持有。
                    let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
                    format!("{}:{}:{}", code.filename(), code.qualname(), code.firstlineno())
                });
                eprintln!("[root-frame] rc={} code={code:?}", header.refcount());
            }
        }
        // **根集合的"只打印汇总"模式**（round 163）：大图（>10 万候选）时逐条打印会刷几十万行 ✗
        // ⇒ `PYAWA_GC_ROOTS_DEBUG=2` 只打**按类型的根数汇总**（诊断"谁被当成根"更快 ✓）。
        if std::env::var("PYAWA_GC_ROOTS_DEBUG").as_deref() == Ok("2") {
            let mut counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for position in &stack {
                // SAFETY: 候选存活。
                let ty = unsafe { candidates[*position].as_ref() }.ty();
                let name = unsafe { ty.as_ref() }.name().to_owned();
                *counts.entry(name).or_insert(0) += 1;
            }
            let mut top: Vec<(String, usize)> = counts.into_iter().collect();
            top.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            let summary: Vec<String> = top
                .iter()
                .take(12)
                .map(|(name, count)| format!("{name}={count}"))
                .collect();
            eprintln!("[gc-root-types] total={} {}", stack.len(), summary.join(" "));
        }
        if std::env::var_os("PYAWA_GC_ROOTS_DEBUG").is_some()
            && std::env::var("PYAWA_GC_ROOTS_DEBUG").as_deref() != Ok("2")
        {
            for position in &stack {
                // SAFETY: 候选存活。
                let ty = unsafe { candidates[*position].as_ref() }.ty();
                eprintln!(
                    "[gc-root] {:p} type={} rc={} external_after={}",
                    candidates[*position].as_ptr(),
                    unsafe { ty.as_ref() }.name(),
                    unsafe { candidates[*position].as_ref() }.refcount(),
                    external[*position]
                );
            }
        }
        let debug_roots = crate::diag::flag_site!("PYAWA_GC_ROOTS_DEBUG");
        // **只在诊断模式下建"谁标记了我"表**（默认零开销 ✓）。
        let mut marked_from: Vec<Option<usize>> = if debug_roots {
            vec![None; candidates.len()]
        } else {
            Vec::new()
        };
        while let Some(position) = stack.pop() {
            if marked[position] {
                continue;
            }
            marked[position] = true;
            for child in self.children_of(candidates[position]) {
                if let Some(&child_position) = index.get(&(child as usize)) {
                    if !marked[child_position] {
                        if debug_roots && marked_from[child_position].is_none() {
                            marked_from[child_position] = Some(position);
                        }
                        stack.push(child_position);
                    }
                }
            }
        }
        if debug_roots {
            for (position, candidate) in candidates.iter().enumerate() {
                // SAFETY: 候选存活。
                let ty = unsafe { candidate.as_ref() }.ty();
                let name = unsafe { ty.as_ref() }.name().to_owned();
                if matches!(name.as_str(), "generator" | "cell" | "function" | "DummyKey" | "AttributeError" | "Thread") && marked[position] {
                    let mut chain: Vec<String> = Vec::new();
                    let mut at = position;
                    for _ in 0..8 {
                        let Some(previous) = marked_from[at] else { break };
                        let pty = unsafe { candidates[previous].as_ref() }.ty();
                        let pname = unsafe { pty.as_ref() }.name().to_owned();
                        if pname == "frame" {
                            // SAFETY: 类型名确认。
                            let frame = unsafe { &*candidates[previous].as_ptr().cast::<crate::Frame>() };
                            let code = frame.code().map(|code| {
                                // SAFETY: code 由帧持有。
                                let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
                                format!("{}:{}", code.qualname(), code.firstlineno())
                            });
                            chain.push(format!("frame{code:?}"));
                        } else {
                            chain.push(pname);
                        }
                        at = previous;
                    }
                    eprintln!("[gc-marked] {name} chain={chain:?}");
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

    /// **含用户类型的动态可达性**（round 99，仅用于"孤儿类型判定"，不用于释放）：候选＝回收链表 ∪
    /// 用户类型；类型的外部计数减 1（注册表那份不算根 ✓）；类型只把**字典**算作计数边（bases／mro 是
    /// 裸指针 ✓）；标记边额外走 `header.ty()` 与类型的 dict／bases／mro ✓。返回**不可达地址集合**。
    fn find_unreachable_dynamic_user_types(&self) -> std::collections::HashSet<usize> {
        let user_types = self.user_type_headers();
        let mut candidates = self.gc_headers();
        for ty in &user_types {
            if !candidates.iter().any(|entry| entry.as_ptr() == ty.as_ptr()) {
                candidates.push(*ty);
            }
        }
        if candidates.is_empty() {
            return std::collections::HashSet::new();
        }
        // **O(1) 判类型**（round 110）：先前对每个候选线性扫 `user_types`（O(H·U)）✗。
        let user_type_set: std::collections::HashSet<usize> =
            user_types.iter().map(|entry| entry.as_ptr() as usize).collect();
        let is_type = |candidate: &NonNull<Header>| {
            user_type_set.contains(&(candidate.as_ptr() as usize))
        };
        let index: std::collections::HashMap<usize, usize> = candidates
            .iter()
            .enumerate()
            .map(|(position, candidate)| (candidate.as_ptr() as usize, position))
            .collect();
        let mut external: Vec<u32> = candidates
            .iter()
            .map(|candidate| {
                // SAFETY: 候选存活。
                let mut count = unsafe { candidate.as_ref() }.refcount();
                if is_type(candidate) {
                    count = count.saturating_sub(1);
                }
                count
            })
            .collect();
        let mut counted: Vec<Vec<*mut Header>> = Vec::with_capacity(candidates.len());
        for candidate in &candidates {
            let children: Vec<*mut Header> = if is_type(candidate) {
                // SAFETY: 候选存活。
                let type_object = unsafe { &*candidate.as_ptr().cast::<crate::TypeObject>() };
                let mut only_dict = Vec::new();
                if let Some(dict) = type_object.dict() {
                    only_dict.push(dict.as_ptr() as *mut Header);
                }
                only_dict
            } else {
                self.children_of(*candidate)
            };
            counted.push(children);
        }
        for children in &counted {
            for child in children {
                let child_address = *child as *mut Header as usize;
                if let Some(&child_position) = index.get(&child_address) {
                    external[child_position] = external[child_position].saturating_sub(1);
                }
            }
        }
        let mut marked = vec![false; candidates.len()];
        let mut stack: Vec<usize> = (0..candidates.len()).filter(|i| external[*i] > 0).collect();
        // **解释器栈上的帧 ＋ sys.modules 里的模块对象**都是根（round 125／126）✓。
        let mut extra_roots = self.interpreter_stack_roots();
        extra_roots.extend(self.module_roots());
        for address in extra_roots {
            if let Some(&position) = index.get(&address) {
                if !marked[position] {
                    stack.push(position);
                }
            }
        }
        let debug_mark = crate::diag::flag_site!("PYAWA_MARK_DEBUG");
        let mut marked_from: Vec<Option<usize>> = if debug_mark {
            vec![None; candidates.len()]
        } else {
            Vec::new()
        };
        while let Some(position) = stack.pop() {
            if marked[position] {
                continue;
            }
            marked[position] = true;
            let candidate = candidates[position];
            let mut children = counted[position].clone();
            // SAFETY: 候选存活。
            let own_type = unsafe { candidate.as_ref() }.ty();
            children.push(own_type.as_ptr() as *mut Header);
            if is_type(&candidate) {
                // SAFETY: 候选是用户类型。
                let type_object = unsafe { &*candidate.as_ptr().cast::<crate::TypeObject>() };
                if let Some(dict) = type_object.dict() {
                    children.push(dict.as_ptr() as *mut Header);
                }
                for base in type_object.bases() {
                    children.push(base.as_ptr() as *mut Header);
                }
                for entry in type_object.mro() {
                    children.push(entry.as_ptr() as *mut Header);
                }
            }
            for child in children {
                let child_address = child as *mut Header as usize;
                if let Some(&child_position) = index.get(&child_address) {
                    if !marked[child_position] {
                        if debug_mark && marked_from[child_position].is_none() {
                            marked_from[child_position] = Some(position);
                        }
                        stack.push(child_position);
                    }
                }
            }
        }
        if debug_mark {
            for (position, candidate) in candidates.iter().enumerate() {
                // SAFETY: 候选存活。
                let ty = unsafe { candidate.as_ref() }.ty();
                let name = unsafe { ty.as_ref() }.name().to_owned();
                if name == "A" && marked[position] {
                    let mut chain: Vec<String> = Vec::new();
                    let mut at = position;
                    let mut root: Option<usize> = None;
                    for _ in 0..10 {
                        let Some(previous) = marked_from[at] else { break };
                        let pty = unsafe { candidates[previous].as_ref() }.ty();
                        let pname = unsafe { pty.as_ref() }.name().to_owned();
                        chain.push(pname);
                        root = Some(previous);
                        at = previous;
                    }
                    let root_info = root
                        .map(|root| {
                            // SAFETY: 候选存活。
                            let header = unsafe { candidates[root].as_ref() };
                            let pty = header.ty();
                            format!(
                                "{:p}:{}:rc={}",
                                candidates[root].as_ptr(),
                                unsafe { pty.as_ref() }.name(),
                                header.refcount()
                            )
                        })
                        .unwrap_or_else(|| "none".to_owned());
                    let mut root_referrers: Vec<String> = Vec::new();
                    if let Some(root) = root {
                        let root_address = candidates[root].as_ptr() as usize;
                        // **先扫 live 表**（含未跟踪对象 ✓）：root 的持有者可能不在 gc 候选里
                        // （round 121 实测：类字典 rc=1、gc 候选里没有引用者 ⇒ 持有者在 live 但未跟踪）。
                        for address in self.live_addresses() {
                            if address == root_address {
                                continue;
                            }
                            // SAFETY: live 表里登记的都是本实例的存活对象头。
                            let header = unsafe {
                                NonNull::new_unchecked(address as *mut Header)
                            };
                            for child in self.children_of(header) {
                                if child as usize == root_address {
                                    let referrer_type = unsafe { header.as_ref() }.ty();
                                    root_referrers.push(format!(
                                        "live:{address:#x}:{}",
                                        unsafe { referrer_type.as_ref() }.name()
                                    ));
                                }
                            }
                        }
                        for referrer in self.gc_headers() {
                            for child in self.children_of(referrer) {
                                if child as usize == root_address {
                                    let referrer_type = unsafe { referrer.as_ref() }.ty();
                                    let referrer_name =
                                        unsafe { referrer_type.as_ref() }.name().to_owned();
                                    if referrer_name == "cell" {
                                        let cell = unsafe {
                                            &*referrer.as_ptr().cast::<crate::cell::CellObject>()
                                        };
                                        let value_info = cell
                                            .value()
                                            .map(|value| {
                                                let vty = unsafe { value.as_ref() }.ty();
                                                format!(
                                                    "{:p}:{}",
                                                    value.as_ptr(),
                                                    unsafe { vty.as_ref() }.name()
                                                )
                                            })
                                            .unwrap_or_else(|| "none".to_owned());
                                        root_referrers.push(format!(
                                            "{:p}:cell(value={value_info})",
                                            referrer.as_ptr()
                                        ));
                                    } else {
                                        root_referrers.push(format!(
                                            "{:p}:{referrer_name}",
                                            referrer.as_ptr()
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    let root_is_some_type_dict = root.map(|root| {
                        let root_address = candidates[root].as_ptr() as usize;
                        self.user_type_headers()
                            .into_iter()
                            .find(|user_ty| {
                                // SAFETY: 名单里类型存活。
                                unsafe { &*user_ty.as_ptr().cast::<crate::TypeObject>() }
                                    .dict()
                                    .is_some_and(|dict| dict.as_ptr() as usize == root_address)
                            })
                            .map(|user_ty| {
                                // SAFETY: 同上。
                                let user_name = unsafe {
                                    &*user_ty.as_ptr().cast::<crate::TypeObject>()
                                }
                                .name()
                                .to_owned();
                                format!("{user_name}:{:p}", user_ty.as_ptr())
                            })
                    });
                    // **实例的类字典 vs root**（round 122 诊断）：直接对比"实例的类型的 dict 指针"与 root 指针。
                    let owner_type_dict = {
                        // SAFETY: 候选存活。
                        let owner_ty = unsafe { candidate.as_ref() }.ty();
                        // SAFETY: 类型由注册表持有。
                        unsafe { &*owner_ty.as_ptr().cast::<crate::TypeObject>() }
                            .dict()
                            .map(|dict| dict.as_ptr() as usize)
                    };
                    let root_address_for_cmp =
                        root.map(|root| candidates[root].as_ptr() as usize);
                    eprintln!(
                        "[mark] {name} owner_type_dict={:x?} root={:x?} same={}",
                        owner_type_dict,
                        root_address_for_cmp,
                        owner_type_dict == root_address_for_cmp
                    );
                    let root_keys = root
                        .map(|root| {
                            self.dict_entries(candidates[root])
                                .map(|entries| {
                                    entries
                                        .iter()
                                        .take(8)
                                        .map(|(key, _)| {
                                            self.text_of(*key)
                                                .map(|text| text.to_owned())
                                                .unwrap_or_else(|| "<?>".to_owned())
                                        })
                                        .collect::<Vec<String>>()
                                })
                                .unwrap_or_default()
                        })
                        .unwrap_or_default();
                    eprintln!(
                        "[mark] {name} at {candidate:p} external={} rc={} chain={chain:?} root={root_info} \
                         root_referrers=[{}] root_type_dict_of={root_is_some_type_dict:?} \
                         root_keys={root_keys:?}",
                        external[position],
                        unsafe { candidate.as_ref() }.refcount(),
                        root_referrers.join(",")
                    );
                    let _ = ();
                }
            }
        }
        candidates
            .iter()
            .zip(marked)
            .filter(|(_, reached)| !*reached)
            .map(|(candidate, _)| candidate.as_ptr() as usize)
            .collect()
    }

    /// **孤儿用户类回收**（round 99）：参照里类型是 GC 对象 ✓；本层**不移动/不释放类型 Box**
    /// （`alloc_type_raw` 用 `Box::leak` 分配、而用户类类型的 `dealloc` 槽是
    /// `AttributeObject::slots()` 那一份（布局不同构）⇒ 真释放会 `free(): invalid pointer` ✗），
    /// 改为**保守判定 + 摘注册表 + 清字典**：
    ///
    /// - 孤儿判据（全部成立）：`rc == 1`（只剩 `types` 注册表那份）；**没有其它类型**以它为
    ///   bases／mro／元类（裸指针 ✓）；动态可达性判它**不可达**；它的**所有活实例**也不可达；
    /// - 动作：先给活实例跑终结器（清字典后 `__del__` 就找不到了 ✓），再摘 `types`／`user_types`／
    ///   按地址索引的缓存、弱引用失效、清类型字典／bases／mro ⇒ 环由**普通引用计数**级联释放 ✓；
    ///   类型对象本体留着（裸 `ty` 指针仍有效 ✓，Box 内存如实泄漏 ✓）。
    pub(crate) fn reclaim_orphan_user_types(&self) {
        let user_types = self.user_type_headers();
        if user_types.is_empty() {
            return;
        }
        // **早退**（round 110）：没有任何 `rc==1` 的用户类型 ⇒ 不可能有孤儿，别做全堆标记 ✓。
        // **诊断模式不早退**（round 119）：`PYAWA_ORPHAN_DEBUG=1` 时照常全扫，便于反查引用者 ✓。
        if !crate::diag::flag_site!("PYAWA_ORPHAN_DEBUG")
            && !user_types.iter().any(|ty| {
                // SAFETY: 名单里的类型都存活。
                unsafe { ty.as_ref() }.refcount() == 1
            })
        {
            return;
        }
        // ① 其它类型以裸指针引用（bases／mro／元类）——跳过"自己 MRO 里的自己" ✓。
        let registry = self.types.borrow().clone();
        let mut referenced_by_types: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for type_ptr in &registry {
            // SAFETY: 注册表里的类型都存活。
            let type_object = unsafe { &*type_ptr.as_ptr() };
            let own_address = type_ptr.as_ptr() as usize;
            for base in type_object.bases() {
                let base_address = base.as_ptr() as usize;
                if base_address != own_address {
                    referenced_by_types.insert(base_address);
                }
            }
            for entry in type_object.mro() {
                let entry_address = entry.as_ptr() as usize;
                if entry_address != own_address {
                    referenced_by_types.insert(entry_address);
                }
            }
            // SAFETY: 同上（类型对象的头部 `ty` 是元类）。
            let header_ref = unsafe { &*type_ptr.as_ptr().cast::<Header>() };
            let metaclass_address = header_ref.ty().as_ptr() as usize;
            if metaclass_address != own_address {
                referenced_by_types.insert(metaclass_address);
            }
        }
        // ② 动态可达性（含用户类型节点）。
        let unreachable = self.find_unreachable_dynamic_user_types();
        // ③ 收集孤儿。
        let mut orphans: Vec<NonNull<Header>> = Vec::new();
        for ty in &user_types {
            let address = ty.as_ptr() as usize;
            // SAFETY: 名单里的类型都存活。
            let header = unsafe { ty.as_ref() };
            let reachable = unreachable.contains(&address);
            if crate::diag::flag_site!("PYAWA_ORPHAN_DEBUG") {
                // SAFETY: ty 存活。
                let name = unsafe { &*ty.as_ptr().cast::<crate::TypeObject>() }.name();
                eprintln!(
                    "[orphan-scan] {name} rc={} raw={} attr_unreach={} ptr={address:#x}",
                    header.refcount(),
                    referenced_by_types.contains(&address),
                    !reachable
                );
                eprintln!(
                    "[orphan-scan] {name} registered_instances={} dict={:p}",
                    self.user_instances_of(*ty).len(),
                    unsafe { &*ty.as_ptr().cast::<crate::TypeObject>() }
                        .dict()
                        .map(|dict| dict.as_ptr())
                        .unwrap_or(core::ptr::null_mut())
                );
                if header.refcount() != 1 || !reachable {
                    // **反查"谁引用它"**（round 119 诊断）：扫全部 gc 候选的 children ＋ 其它类型的字典。
                    let mut referrers: Vec<String> = Vec::new();
                    for candidate in self.gc_headers() {
                        for child in self.children_of(candidate) {
                            if child as usize == address {
                                // SAFETY: 候选存活。
                                let referrer_ty = unsafe { candidate.as_ref() }.ty();
                                let referrer_name = unsafe { referrer_ty.as_ref() }.name().to_owned();
                                if referrer_name == "frame" {
                                    // SAFETY: 候选存活；帧的 code 由帧持有。
                                    let frame = unsafe {
                                        &*candidate.as_ptr().cast::<crate::Frame>()
                                    };
                                    let code_name = frame
                                        .code()
                                        .map(|code| {
                                            unsafe { code.cast::<crate::CodeObject>().as_ref() }
                                                .name()
                                                .to_owned()
                                        })
                                        .unwrap_or_else(|| "?".to_owned());
                                    referrers.push(format!(
                                        "{:p}:frame({code_name})",
                                        candidate.as_ptr()
                                    ));
                                } else {
                                    referrers.push(format!(
                                        "{:p}:{referrer_name}",
                                        candidate.as_ptr()
                                    ));
                                }
                            }
                        }
                    }
                    for other in self.user_type_headers() {
                        if other.as_ptr() as usize == address {
                            continue;
                        }
                        // SAFETY: 名单里类型存活。
                        let type_object = unsafe { &*other.as_ptr().cast::<crate::TypeObject>() };
                        let mut children: Vec<*mut Header> = Vec::new();
                        if let Some(dict) = type_object.dict() {
                            children.push(dict.as_ptr() as *mut Header);
                        }
                        for base in type_object.bases() {
                            children.push(base.as_ptr() as *mut Header);
                        }
                        for entry in type_object.mro() {
                            children.push(entry.as_ptr() as *mut Header);
                        }
                        if children.iter().any(|child| *child as usize == address) {
                            referrers.push(format!("type:{:p}", other.as_ptr()));
                        }
                    }
                    eprintln!(
                        "[orphan-referrers] {name} rc={} addr={address:#x} referrers=[{}]",
                        header.refcount(),
                        referrers.join(",")
                    );
                }
            }
            if referenced_by_types.contains(&address) {
                continue;
            }
            if !unreachable.contains(&address) {
                // **诊断（round 119，`PYAWA_ORPHAN_DEBUG=1`）**：类被判"可达"时，反查**谁引用它**
                // （扫全部 gc 候选的 children ＋ 其它类型的字典），把"多余那份引用的持有者"点出来 ✓。
                if crate::diag::flag_site!("PYAWA_ORPHAN_DEBUG") {
                    let mut referrers: Vec<String> = Vec::new();
                    for candidate in self.gc_headers() {
                        for child in self.children_of(candidate) {
                            if child as usize == address {
                                // SAFETY: 候选存活。
                                let referrer_ty = unsafe { candidate.as_ref() }.ty();
                                referrers.push(format!(
                                    "{:p}:{}",
                                    candidate.as_ptr(),
                                    unsafe { referrer_ty.as_ref() }.name()
                                ));
                            }
                        }
                    }
                    for other in self.user_type_headers() {
                        if other.as_ptr() as usize == address {
                            continue;
                        }
                        // SAFETY: 名单里类型存活。
                        let type_object = unsafe { &*other.as_ptr().cast::<crate::TypeObject>() };
                        let mut children: Vec<*mut Header> = Vec::new();
                        if let Some(dict) = type_object.dict() {
                            children.push(dict.as_ptr() as *mut Header);
                        }
                        for base in type_object.bases() {
                            children.push(base.as_ptr() as *mut Header);
                        }
                        for entry in type_object.mro() {
                            children.push(entry.as_ptr() as *mut Header);
                        }
                        if children.iter().any(|child| *child as usize == address) {
                            referrers.push(format!("type:{:p}", other.as_ptr()));
                        }
                    }
                    // SAFETY: ty 存活。
                    let name = unsafe { &*ty.as_ptr().cast::<crate::TypeObject>() }.name();
                    eprintln!(
                        "[orphan-referrers] {name} rc={} addr={address:#x} referrers=[{}]",
                        header.refcount(),
                        referrers.join(",")
                    );
                }
                continue;
            }
            // **活实例枚举走登记表**（round 119）：不再只扫 `gc_headers` ✗（会漏非跟踪布局的实例 ⇒
            // round 118 的"判据空过"✗）。登记表里的实例若**不在动态不可达集合**里（含"根本没被跟踪、
            // 无法证明不可达"的情形 ✓）⇒ 保守判"可能还活着"，**不回收该类型** ✓。
            let mut all_instances_unreachable = true;
            for instance in self.user_instances_of(*ty) {
                let instance_address = instance.as_ptr() as usize;
                // SAFETY: 登记表里的实例按"活"登记（释放时注销 ✓）。
                let instance_header = unsafe { instance.as_ref() };
                let tracked = instance_header.has_flag(flags::GC_TRACKED);
                let in_unreachable = unreachable.contains(&instance_address);
                if crate::diag::flag_site!("PYAWA_ORPHAN_DEBUG") {
                    eprintln!(
                        "[orphan-instance] ty={:p} inst={instance_address:#x} rc={} tracked={}                          in_unreachable={in_unreachable}",
                        ty.as_ptr(),
                        instance_header.refcount(),
                        tracked
                    );
                }
                if !in_unreachable {
                    all_instances_unreachable = false;
                    break;
                }
            }
            if crate::diag::flag_site!("PYAWA_ORPHAN_DEBUG") {
                // SAFETY: ty 存活。
                let name = unsafe { &*ty.as_ptr().cast::<crate::TypeObject>() }.name();
                eprintln!(
                    "[orphan] {name} rc={} raw={} attr_unreach={} inst_ok={}",
                    header.refcount(),
                    referenced_by_types.contains(&address),
                    unreachable.contains(&address),
                    all_instances_unreachable
                );
            }
            if all_instances_unreachable {
                orphans.push(*ty);
            }
        }
        // ④ 回收：先终结活实例，再摘注册表/缓存、清字典。
        for ty in orphans {
            let address = ty.as_ptr() as usize;
            if crate::diag::flag_site!("PYAWA_ORPHAN_DEBUG") {
                // SAFETY: ty 存活。
                let name = unsafe { &*ty.as_ptr().cast::<crate::TypeObject>() }.name();
                eprintln!("[orphan-reclaim] {name} ptr={address:#x}");
            }
            // SAFETY: 名单里的类型都存活。
            let type_object = unsafe { &*ty.as_ptr().cast::<crate::TypeObject>() };
            // **先给活实例跑终结器**（清字典后 `__del__` 就从 MRO 上消失了 ✗）；实例清单取**登记表** ✓。
            let instances: Vec<NonNull<Header>> = self.user_instances_of(ty);
            if !instances.is_empty() {
                if let Some(finalize) = type_object.slots().finalize {
                    for instance in instances {
                        // SAFETY: instance 存活。
                        let instance_ref = unsafe { instance.as_ref() };
                        if !instance_ref.has_flag(flags::FINALIZING) {
                            instance_ref.set_flag(flags::FINALIZING);
                            // SAFETY: 槽位契约（instance 是本实例的存活对象）。
                            unsafe { finalize(instance.as_ptr(), self) };
                        }
                    }
                }
            }
            // **摘注册表 + 登记 retired**（round 130）：registration 摘掉可以避免"僵尸类"占着名字
            // （round 129 保留注册表的隐患 ✓）；同时把地址记进 `retired_types` ⇒ `assert_live` 认得它 ⇒
            // 仍在别处的计数引用将来释放它时不会被当野指针 ✓（类型本体 Box::leak 保留、裸 `ty` 指针有效 ✓；
            // 注册表那份引用如实泄漏 ⇒ rc 不会再降到 0，不会 invalid-free ✓）。
            self.types
                .borrow_mut()
                .retain(|entry| entry.as_ptr() as usize != address);
            // **名字索引同步摘掉**（round 142）：否则 `type_named` 会返回已退休的类型 ✗。
            self.types_by_name.borrow_mut().retain(|_, entry| entry.as_ptr() as usize != address);
            self.user_types
                .borrow_mut()
                .retain(|entry| entry.as_ptr() != ty.as_ptr());
            self.user_type_set.borrow_mut().remove(&address);
            self.retired_types.borrow_mut().insert(address);
            self.user_instances.borrow_mut().remove(&address);
            let removed: Vec<NonNull<Header>> = {
                let mut cache = self.type_dunders.borrow_mut();
                let keys: Vec<(usize, &'static str)> = cache
                    .keys()
                    .filter(|(key, _)| *key == address)
                    .cloned()
                    .collect();
                keys.into_iter()
                    .filter_map(|key| cache.remove(&key))
                    .collect()
            };
            for value in removed {
                // SAFETY: 缓存交出的引用归调用方。
                unsafe { self.release_object(value.as_ptr()) };
            }
            self.isinstance_cache
                .borrow_mut()
                .retain(|(left, right, _), _| *left != address && *right != address);
            self.type_text_signatures.borrow_mut().remove(&address);
            self.mappingproxy_sources
                .borrow_mut()
                .retain(|_, value| value.as_ptr() as usize != address);
            let callbacks = self.take_weakref_callbacks(ty);
            self.queue_weakref_callbacks(callbacks);
            self.forget_weakref_object(ty);
            if let Some(dict) = type_object.dict() {
                type_object.set_dict(None);
                // SAFETY: 这份引用由类型对象持有。
                unsafe { self.release_object(dict.as_ptr()) };
            }
            type_object.bases.borrow_mut().clear();
            type_object.mro.borrow_mut().clear();
        }
    }

    /// 按 `traverse` 槽位取一个对象的直接引用（**OM-12**／**OM-29**／**OM-36**）。
    /// **图外持有者扫描**（round 170 诊断，`PYAWA_GC_HOLDER_DEBUG=1`）：给定一个"根"对象地址，
    /// 扫 `Instance` 的**非 GC 字段**（`exception_state`／`pending_exception`／`gc_frozen`／
    /// `weakrefs` 注册表／`types`／`user_types`／`retired_types`）里有没有它 ⇒ 命中就说明
    /// 这个对象被"GC 图之外的引擎结构"强引用（那才是"rc>0 却找不到候选父"的来源 ✓）。
    fn instance_name_is_exception(&self, ty: &crate::TypeObject) -> bool {
        let base = match self.hot_type(HotId::BaseException) {
            Some(base) => base,
            None => return false,
        };
        let mut current = Some(ty as *const crate::TypeObject);
        while let Some(pointer) = current {
            // SAFETY: 类型由注册表／基类持有。
            let object = unsafe { &*pointer };
            if std::ptr::eq(object, unsafe { base.as_ref() }) {
                return true;
            }
            current = object
                .bases()
                .first()
                .copied()
                .map(|base| base.as_ptr() as *const crate::TypeObject);
        }
        false
    }

    pub(crate) fn debug_holder_of(&self, pointer: usize) -> Vec<&'static str> {
        let mut hits = Vec::new();
        if self
            .exception_state
            .borrow()
            .iter()
            .any(|value| value.as_ptr() as usize == pointer)
        {
            hits.push("exception_state");
        }
        if self
            .pending_exception()
            .is_some_and(|value| value.as_ptr() as usize == pointer)
        {
            hits.push("pending_exception");
        }
        if self.gc_frozen.borrow().contains(&pointer) {
            hits.push("gc_frozen");
        }
        for holders in self.weakrefs.borrow().values() {
            if holders.iter().any(|value| value.as_ptr() as usize == pointer) {
                hits.push("weakrefs-registry");
                break;
            }
        }
        if self
            .user_types
            .borrow()
            .iter()
            .any(|value| value.as_ptr() as usize == pointer)
        {
            hits.push("user_types");
        }
        if self
            .types
            .borrow()
            .iter()
            .any(|value| value.as_ptr() as usize == pointer)
        {
            hits.push("types");
        }
        if self.retired_types.borrow().contains(&pointer) {
            hits.push("retired_types");
        }
        // **其余可能"持有但不计入候选边"的实例字段**（round 190 诊断）：全局 trace/profile、
        // 模块表、待处理栈、元组复用池 ✓。
        for (label, hit) in [
            (
                "global_trace",
                self.global_trace
                    .borrow()
                    .is_some_and(|value| value.as_ptr() as usize == pointer),
            ),
            (
                "global_profile",
                self.global_profile
                    .borrow()
                    .is_some_and(|value| value.as_ptr() as usize == pointer),
            ),
            (
                "modules",
                self.modules
                    .borrow()
                    .is_some_and(|value| value.as_ptr() as usize == pointer),
            ),
            (
                "pending",
                self.pending
                    .borrow()
                    .iter()
                    .any(|value| value.as_ptr() as usize == pointer),
            ),
            (
                "tuple_pool",
                self.tuple_pool
                    .borrow()
                    .iter()
                    .any(|value| value.as_ptr() as usize == pointer),
            ),
        ] {
            if hit {
                hits.push(label);
            }
        }
        hits
    }

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

    /// **年轻段候选**（round 157）：从链头走到"上次回收记下的边界"为止 ✓（链新对象压头 ✓）。
    /// 边界指针只用于**比较**（可能已被全量回收释放 ⇒ 绝不解引用 ✓）；失配时退化成"整链年轻" ✓（安全）。
    pub(crate) fn gc_headers_young(&self) -> Vec<NonNull<Header>> {
        let mark = self.gc_young_mark.get();
        let mut result = Vec::new();
        let mut cursor = self.gc_head.get();
        while !cursor.is_null() && cursor != mark {
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
