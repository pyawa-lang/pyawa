//! 实例级内存、释放协议与循环回收（`docs/SPEC-object-model.md` §4、§7、§9）。
//!
//! 一个 [`Instance`] 就是 VM 侧一切可变状态的宿主（`DESIGN.md` §3 不变量 2）：
//! 对象堆、字节记账、类型注册表、回收链表与待处理栈都挂在它上面，**没有进程级全局状态**。

use core::cell::{Cell, OnceCell, RefCell};
use core::ptr;
use core::ptr::NonNull;
use std::collections::{HashMap, HashSet};

use crate::flags;
use crate::header::{Header, PyObject};
use crate::refcount::{Owned, PyRef};
use crate::singleton::{IntObject, NoneObject, BoolObject, Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
use crate::type_object::{Slots, TypeObject};

/// **OM-26**：回收阈值，**三元组**形态。
///
/// 参照实现（本机 CPython 3.14.4 实测）：`gc.get_threshold() == (2000, 10, 0)`。
/// 本层**单代**：只有 `.0` 生效，后两位**存而不生效**——这一"存而不生效"是**临时**的，
/// 等真分代落地（`SPEC-object-model.md` 的 `OM-26`、`DESIGN.md` §13-18）。
pub const DEFAULT_GC_THRESHOLD: (usize, usize, usize) = (2000, 10, 0);

/// **OM-1**／**OM-3**／**OM-4**：一个实例的对象堆与记账。
pub struct Instance {
    /// **OM-3**：每实例字节计数器（预算职责留在 VM 侧，禁止下放给能力接口）。
    bytes_allocated: Cell<usize>,
    /// 本实例分配、尚未释放的普通对象（`usize` = 头部地址；**O(1)** 增删）。
    live: RefCell<HashSet<usize>>,
    /// **OM-15**：类型注册表按实例存放；注册表持有每个类型对象的一份引用。
    types: RefCell<Vec<NonNull<TypeObject>>>,
    /// 元类型（类型对象的类型，自指）。
    metatype: Cell<Option<NonNull<TypeObject>>>,
    /// **OM-23**：本实例的单例表（引导期填好，之后只读）。
    singletons: OnceCell<Singletons>,
    /// **OM-21**：待处理栈——计数归零的对象在这里排队，由最外层调用逐个清空（禁止朴素递归）。
    pending: RefCell<Vec<NonNull<Header>>>,
    /// 是否正在清空待处理栈（重入检测）。
    draining: Cell<bool>,
    /// **OM-25**：`GC_TRACKED` 对象的侵入式链表头（借头部的 `gc_prev`／`gc_next`）。
    gc_head: Cell<*mut Header>,
    /// 链表中当前的跟踪对象数。
    gc_count: Cell<usize>,
    /// **OM-26**：阈值可配置。
    gc_threshold: Cell<(usize, usize, usize)>,
    /// 自上次回收以来的分配计数。
    gc_alloc_count: Cell<usize>,
    /// 回收进行中：这些对象只减计数、由本次回收统一释放（见 [`Instance::collect`]）。
    gc_frozen: RefCell<HashSet<usize>>,
    /// 回收是否正在进行：终结器／`clear` 里再触发回收时不得嵌套（否则会动到外层手里的指针）。
    gc_running: Cell<bool>,
}

impl Instance {
    /// 创建一个实例，并引导它的**元类型**。
    ///
    /// **OM-1**：每个实例有自己的堆与单例表；本函数不触碰任何进程级状态。
    pub fn new() -> Self {
        let this = Self {
            bytes_allocated: Cell::new(0),
            live: RefCell::new(HashSet::new()),
            types: RefCell::new(Vec::new()),
            metatype: Cell::new(None),
            singletons: OnceCell::new(),
            pending: RefCell::new(Vec::new()),
            draining: Cell::new(false),
            gc_head: Cell::new(ptr::null_mut()),
            gc_count: Cell::new(0),
            gc_threshold: Cell::new(DEFAULT_GC_THRESHOLD),
            gc_alloc_count: Cell::new(0),
            gc_frozen: RefCell::new(HashSet::new()),
            gc_running: Cell::new(false),
        };

        // 元类型自指：类型对象的类型就是它自己（与 CPython 的 `PyType_Type` 同理）。
        // 此处 `ty` 先落在 `NonNull::dangling()` 上，写完自指后立即成为正常类型对象。
        let metatype = this.alloc_type_raw(
            "type",
            core::mem::size_of::<TypeObject>(),
            Slots::new(TypeObject::dealloc),
        );
        // SAFETY: metatype 刚分配、尚未交给任何其他代码；写入自指后它才被引用。
        unsafe { metatype.as_ref().header.set_ty(metatype) };
        this.metatype.set(Some(metatype));

        this.bootstrap_singletons();
        this
    }

    /// **OM-23**：按实例创建单例（`None`／`True`／`False`／小整数）。
    ///
    /// 引导期还不能借出 `&Instance` 造 `Owned` 守卫，所以走 [`Instance::adopt`]：
    /// 引用由实例自己持有，随实例销毁一起释放（`OM-2`）。
    fn bootstrap_singletons(&self) {
        let none_type = self.alloc_type_raw(
            "NoneType",
            core::mem::size_of::<NoneObject>(),
            Slots::new(NoneObject::dealloc),
        );
        let bool_type = self.alloc_type_raw(
            "bool",
            core::mem::size_of::<BoolObject>(),
            Slots::new(BoolObject::dealloc),
        );
        let int_type = self.alloc_type_raw(
            "int",
            core::mem::size_of::<IntObject>(),
            Slots::new(IntObject::dealloc),
        );

        let none = self.adopt(NoneObject::new(none_type)).cast::<Header>();
        let true_ = self.adopt(BoolObject::new(bool_type, true)).cast::<Header>();
        let false_ = self.adopt(BoolObject::new(bool_type, false)).cast::<Header>();

        let count = (SMALL_INT_MAX - SMALL_INT_MIN + 1) as usize;
        let mut small_ints = Vec::with_capacity(count);
        for value in SMALL_INT_MIN..=SMALL_INT_MAX {
            small_ints.push(self.adopt(IntObject::new(int_type, value)).cast::<Header>());
        }

        assert!(
            self.singletons
                .set(Singletons::new(
                    none_type,
                    bool_type,
                    int_type,
                    none,
                    true_,
                    false_,
                    small_ints,
                ))
                .is_ok(),
            "单例表在 Instance::new 里只设一次"
        );
    }

    /// **OM-23**：本实例的单例表。
    pub fn singletons(&self) -> &Singletons {
        self.singletons
            .get()
            .expect("单例表在 Instance::new 中引导，必然存在")
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

    /// 元类型：类型对象自身的类型。
    pub fn metatype(&self) -> NonNull<TypeObject> {
        self.metatype
            .get()
            .expect("元类型在 Instance::new 中引导，必然存在")
    }

    /// **OM-3**：本实例当前占用的字节数（能力接口不承担预算，见 `CP-8`）。
    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated.get()
    }

    /// 本实例中尚未释放的普通对象数（类型对象不计）。
    pub fn live_objects(&self) -> usize {
        self.live.borrow().len()
    }

    /// **OM-25**：当前参与循环回收（`GC_TRACKED`）的对象数。
    pub fn tracked_objects(&self) -> usize {
        self.gc_count.get()
    }

    /// **OM-15**：本实例注册的类型对象数。
    pub fn type_count(&self) -> usize {
        self.types.borrow().len()
    }

    /// **OM-26**：回收阈值三元组。默认值见 [`DEFAULT_GC_THRESHOLD`]。
    ///
    /// 后两位**存而不生效**（单代，**临时**）；它们照样要能读回来，纯 Python 层会解三元组。
    pub fn gc_threshold(&self) -> (usize, usize, usize) {
        self.gc_threshold.get()
    }

    /// **OM-26**：设置阈值三元组。`t0` **0 会被拒绝**——那等于每次分配都回收；
    /// `t1`／`t2` 只存不生效（单代，**临时**）。
    pub fn set_gc_threshold(&self, threshold: (usize, usize, usize)) {
        assert!(threshold.0 > 0, "OM-26：阈值必须可配置且不为 0");
        self.gc_threshold.set(threshold);
    }

    /// 在**本实例**的堆上分配一个对象，返回**新引用**（**OM-16**）。
    ///
    /// `value` 由 `T::new(ty, …)` 构造（见 [`crate::py_object!`]）；类型取自它的头部。
    /// **OM-1**：对象只属于本实例，不能跨实例共享。
    /// **OM-26**：分配计数达阈值时自动触发一次回收。
    pub fn alloc<'a, T: PyObject>(&'a self, value: T) -> Owned<'a, T> {
        let ptr = self.adopt(value);
        self.gc_alloc_count.set(self.gc_alloc_count.get() + 1);
        if self.gc_alloc_count.get() >= self.gc_threshold.get().0 {
            // 新对象此刻计数为 1、还没有交出去，因此在可达性分析里是根（不会被误回收）。
            self.collect();
        }

        Owned::new(ptr, self)
    }

    /// 把一个已构造好的对象交给本实例托管（记账 ＋ 入链 ＋ 标 `GC_TRACKED`），
    /// 返回它的指针；**引用由实例自己持有**。
    ///
    /// 引导期（`Instance::new` 造单例时）用不了 `Owned`——那需要先借出 `&Instance`。
    fn adopt<T: PyObject>(&self, value: T) -> NonNull<T> {
        let ty = value.header().ty();
        let size = core::mem::size_of::<T>();
        debug_assert_eq!(
            size,
            // SAFETY: ty 由某个实例的类型注册表持有（OM-15），在实例存活期间有效；
            // 这里在 debug 下用它校验类型元数据与 Rust 布局一致。
            unsafe { ty.as_ref() }.instance_size,
            "类型的 instance_size 与 Rust 布局不一致"
        );

        let ptr = NonNull::from(Box::leak(Box::new(value)));
        let header = ptr.cast::<Header>();

        // **OM-12**：可成环的类型必须标记 GC_TRACKED。本层用"是否提供 traverse 槽位"判定；
        // 类型对象自身也会成环（bases／dict），但它的 traverse／clear 待接线后再补标记。
        let tracked = unsafe { ty.as_ref() }.slots.traverse.is_some();
        if tracked {
            // SAFETY: header 指向刚刚分配、尚未交给其他代码的对象。
            unsafe { header.as_ref() }.set_flag(flags::GC_TRACKED);
        }

        self.live.borrow_mut().insert(header.as_ptr() as usize);
        self.bytes_allocated.set(self.bytes_allocated.get() + size);
        if tracked {
            self.link_gc(header);
        }
        ptr
    }

    /// **OM-15**：注册一个新类型。
    ///
    /// 返回的指针在本实例存活期间**稳定**：类型对象由注册表持有一份引用，不随普通对象回收。
    pub fn new_type(
        &self,
        name: &'static str,
        instance_size: usize,
        slots: Slots,
    ) -> NonNull<TypeObject> {
        self.alloc_type_raw(name, instance_size, slots)
    }

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

    /// 释放一个**新引用**（**OM-16**）；计数归零时按 **OM-20** 的顺序处理：
    /// ① 终结器（可复活）→ ② `clear` → ③ 释放。清空走 **OM-21** 的待处理栈，不朴素递归。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中**存活**的对象，且调用方交出的是一份**新引用**。
    pub unsafe fn release_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        let header = unsafe { &*ptr };
        // **OM-24**：M1 的 `IMMORTAL` 位恒为 0；这里只是防御，不承担语义。
        if header.is_immortal() {
            return;
        }
        if header.decref() != 0 {
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

    /// **OM-25**…**OM-30**：跑一次标记-清除，返回本次释放的对象数。
    ///
    /// 顺序按 **OM-27** 固定：① 求不可达集合 → ② 先清弱引用 → ③ 调终结器 → ④ 释放。
    /// 回收范围仅限 `GC_TRACKED` 对象（**OM-25**）；不可达但尚未释放的对象**禁止**暴露（**OM-30**）。
    pub fn collect(&self) -> usize {
        if self.gc_running.get() {
            // 终结器／clear 里又触发了一次回收：本次让路，交给外层那次。
            return 0;
        }
        self.gc_running.set(true);
        let freed = self.collect_inner();
        self.gc_running.set(false);
        freed
    }

    /// [`Instance::collect`] 的主体；进入前 `gc_running` 已置位。
    fn collect_inner(&self) -> usize {
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
        }
        self.gc_frozen.borrow_mut().clear();

        for header in &unreachable {
            self.free_garbage(*header);
        }
        unreachable.len()
    }

    /// **OM-20**：单个对象的释放三步（正常引用计数路径）。
    fn release_one(&self, ptr: NonNull<Header>) {
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

        // ③ 释放内存。
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(ptr);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        // SAFETY: 计数为 0，且 clear 已把持有的引用交出（OM-20 ③ 的前提）。
        unsafe { dealloc(ptr.as_ptr()) };
    }

    /// **OM-27** ④：释放一个不可达对象（`clear` 已经跑过，这里不再调终结器）。
    fn free_garbage(&self, header: NonNull<Header>) {
        let ty = unsafe { header.as_ref() }.ty();
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(header);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        // SAFETY: 该对象已由可达性分析判为不可达，且 clear 已完成。
        unsafe { dealloc(header.as_ptr()) };
    }

    /// **OM-29**／**OM-30**：求不可达的跟踪对象。
    ///
    /// 两步：先按"引用计数 − 来自跟踪对象内部的引用数"找出根（外部引用 > 0），
    /// 再从根出发按 `traverse` 标记；没被标记的就是不可达集合。
    fn find_unreachable(&self) -> Vec<NonNull<Header>> {
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
    fn children_of(&self, header: NonNull<Header>) -> Vec<*mut Header> {
        let ty = unsafe { header.as_ref() }.ty();
        let mut children = Vec::new();
        if let Some(traverse) = unsafe { ty.as_ref() }.slots.traverse {
            // SAFETY: header 是本实例的存活对象；回调只收集指针，不做解引用。
            unsafe { traverse(header.as_ptr(), &mut |child| children.push(child)) };
        }
        children
    }

    /// 回收链表上的全部对象（**OM-25**：只有 `GC_TRACKED` 入链）。
    fn gc_headers(&self) -> Vec<NonNull<Header>> {
        let mut result = Vec::with_capacity(self.gc_count.get());
        let mut cursor = self.gc_head.get();
        while !cursor.is_null() {
            // SAFETY: 链上的指针都由本实例分配且尚未释放。
            result.push(unsafe { NonNull::new_unchecked(cursor) });
            cursor = unsafe { (*cursor).gc_next() };
        }
        result
    }

    fn link_gc(&self, header: NonNull<Header>) {
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

    fn unlink_gc(&self, header: NonNull<Header>) {
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

    /// 从"存活集合"与回收链表上同时摘除。
    fn unlink(&self, header: NonNull<Header>) {
        self.live.borrow_mut().remove(&(header.as_ptr() as usize));
        // SAFETY: header 尚未释放。
        if unsafe { header.as_ref() }.has_flag(flags::GC_TRACKED) {
            self.unlink_gc(header);
        }
    }

    fn alloc_type_raw(
        &self,
        name: &'static str,
        instance_size: usize,
        slots: Slots,
    ) -> NonNull<TypeObject> {
        // 引导期（元类型自身）还没有类型可指，先用悬垂但非空的指针占位；随后立刻写回自指。
        let ty = self
            .metatype
            .get()
            .unwrap_or_else(NonNull::<TypeObject>::dangling);
        let object = TypeObject::new(
            ty,
            name,
            RefCell::new(Vec::new()),
            RefCell::new(Vec::new()),
            Cell::new(0),
            slots,
            RefCell::new(None),
            instance_size,
        );
        let ptr = NonNull::from(Box::leak(Box::new(object)));
        // 类型对象也走同一本账（OM-3），但由注册表持有：不进 `live`，销毁时统一释放（OM-2）。
        self.bytes_allocated
            .set(self.bytes_allocated.get() + core::mem::size_of::<TypeObject>());
        self.types.borrow_mut().push(ptr);
        ptr
    }
}

impl Default for Instance {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Instance {
    /// **OM-2**：实例销毁**必须**释放其堆内全部内存，**无论循环是否被回收过**——
    /// 不依赖回收器先跑完。
    fn drop(&mut self) {
        // 正常路径下 `live` 已经空了。仍有残留 ⇒ 计数环或未交出的引用，
        // 此时**强制释放**：不调终结器、不再 clear（环的语义已由 §9 的回收器负责，
        // 走到这里说明调用方没有 collect，而不是回收器做不到）。
        //
        // 本层能这样做，是因为生命周期把 `Owned` 钉在 `&Instance` 上：对象载荷里
        // **不可能**存着 `Owned` 守卫（那需要 `&'static Instance`），所以这里释放
        // 任何一个对象都不会回头去碰别的对象。
        let live: Vec<usize> = self.live.borrow().iter().copied().collect();
        for address in live {
            let header = unsafe { NonNull::new_unchecked(address as *mut Header) };
            // SAFETY: 每个地址都由本实例分配且尚未释放；类型对象在下一段之前一直存活。
            unsafe { Self::force_free(header) };
        }

        let types = core::mem::take(&mut *self.types.borrow_mut());
        for ty in types {
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
