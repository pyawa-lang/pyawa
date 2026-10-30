//! 本文件是 `builtin_objects` 的 `iterators` 域：迭代器／生成器／协程的载荷与推进（`ItState`／`count`／`generator`／`asend`）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;


py_object! {
    /// 生成器：一个**挂起的帧** ＋ 是否已跑完。
    ///
    /// 挂起时值栈在帧自己的恢复点里（`BC-47`），故这里只持有帧的引用。
    pub struct GeneratorObject {
        /// 生成器的帧（**本对象持有一份引用**）。
        frame: NonNull<Header>,
        /// 已经跑完（之后的 `FOR_ITER` 直接走耗尽路径）。
        finished: Cell<bool>,
        /// **已经开始过**（`send` 的"刚创建"判定：刚创建的生成器只接受 `send(None)`）。
        started: Cell<bool>,
    }
}

py_object! {
    /// `async_generator.__anext__()` 交出的 **awaitable**（参照实现叫
    /// `async_generator_asend`，实测 `repr` 是 `<async_generator_asend object at 0x…>`）。
    ///
    /// 载荷就是"要推进哪个异步生成器"——本层把它做成一层薄包装，而不是像参照实现那样
    /// 再挂一整套生成器状态（那是它的实现细节；可观察行为一致即可）。
    pub struct AsendObject {
        /// 要推进的异步生成器（**本对象持有一份引用**）。
        generator: NonNull<Header>,
        /// `asend(值)` 送进去的值（`__anext__()` 时为 `None`）。
        sent: RefCell<Option<NonNull<Header>>>,
    }
}

py_object! {
    /// 迭代器：一个被迭代的对象 ＋ 游标。
    ///
    /// *临时*：一个 Rust 载荷支撑表里那几个迭代器类型（`tuple_iterator`／`list_iterator`／
    /// `str_ascii_iterator`／`dict_keyiterator`／`set_iterator`）——`TS-43` 允许布局自选；
    /// 「下一个」按**被迭代对象的类型**分派，不另存 kind。
    pub struct IteratorObject {
        /// 被迭代的对象（**本对象持有一份引用**）。
        target: NonNull<Header>,
        /// 游标（下一个要取的下标）。
        index: Cell<usize>,
    }
}

/// [`ItStateObject`] 的**种类**（`itertools` 的迭代器状态）。
///
/// 全是 `Copy`（只含整数与指针）⇒ 放进 `Cell` 也能就地改。**持对象引用的分支**
/// 由 `it_state_traverse`／`it_state_clear` 负责（`OM-40`／`OM-20` ②）。
#[derive(Clone, Copy)]
pub enum ItStateKind {
    /// `repeat(value, remaining)`：`remaining < 0` ⇒ 无限。
    Repeat {
        /// 每次吐的那个值（**本对象持有一份引用**）。
        value: NonNull<Header>,
        /// 还能吐几次（负数 ⇒ 无限）。
        remaining: i64,
    },
    /// `islice(inner, start, position, stop, step)`。
    ///
    /// **实测**语义（探测夹具）：让出下标 `i` 满足 `start <= i < stop` 且 `(i - start) % step == 0`；
    /// 消耗到 `max(start, stop)` 才耗尽（`start >= stop` ⇒ 一个都不让出、但**仍消费 `start` 个**）；
    /// `stop < 0` ⇒ **无上界**（`islice(it, start, None)` 那种）。
    Islice {
        /// 内层迭代器（**本对象持有一份引用**）。
        inner: NonNull<Header>,
        /// 起始下标。
        start: i64,
        /// 已取走个数。
        position: i64,
        /// 上界（**负数 ⇒ 无上界**）。
        stop: i64,
        /// 步长（正数）。
        step: i64,
    },
    /// **谓词类**：`takewhile`／`dropwhile`／`filterfalse`（`mode` 区分，`state` 的含义随 mode）。
    ///
    /// - `mode = 0` `takewhile`：`state` ＝ "已经停过"（谓词一旦为假就永久耗尽）
    /// - `mode = 1` `dropwhile`：`state` ＝ "已经出过第一个"（之前一直丢）
    /// - `mode = 2` `filterfalse`：`state` 不用
    FilterLike {
        /// 内层迭代器（**本对象持有一份引用**）。
        inner: NonNull<Header>,
        /// 谓词（**本对象持有一份引用**）。
        predicate: NonNull<Header>,
        /// 0 `takewhile`／1 `dropwhile`／2 `filterfalse`。
        mode: u8,
        /// 见上（随 mode 解释）。
        state: bool,
    },
    /// `itertools.accumulate(iterable[, func])`：`total` 是累计值（`None` ⇒ 还没开始）。
    ///
    /// **实测**：`func` 缺省时是**加法**（本层只做整数——与 `BINARY_OP` 的现状同口径）；
    /// `func` 非可调用不当场报错，**第一次要用**时才报（`accumulate([1], 5)` ⇒ `[1]`）。
    Accumulate {
        /// 内层迭代器（**本对象持有一份引用**）。
        inner: NonNull<Header>,
        /// 累计函数（`None` ⇒ 用加法）。
        function: Option<NonNull<Header>>,
        /// 累计值（**本对象持有一份引用**；`None` ⇒ 还没开始）。
        total: Option<NonNull<Header>>,
    },
    /// `itertools.compress(data, selectors)`：按 `selectors` 的**真假**逐个筛 `data`。
    Compress {
        /// 数据迭代器（**本对象持有一份引用**）。
        data: NonNull<Header>,
        /// 选择器迭代器（**本对象持有一份引用**）。
        selectors: NonNull<Header>,
    },
    /// `itertools.combinations(pool, r)`：池已**物化**成 `list`，下标状态放在另一个 `list` 里。
    ///
    /// `indices` 为空 ⇒ 还没产出过（首个组合靠"从 0…r-1"起步）；`done` ⇒ 已穷尽。
    Combinations {
        /// 物化后的池（一个 `list`，**本对象持有一份引用**）。
        pool: NonNull<Header>,
        /// 取几个。
        r: i64,
        /// 当前下标组合（一个 `list`，**本对象持有一份引用**）。
        indices: NonNull<Header>,
        /// 是否已经产出过（首个组合 = `0..r`）。
        started: bool,
        /// 是否已穷尽。
        done: bool,
        /// `true` ⇒ `combinations_with_replacement`（下标**可重复且非降序**）
        replace: bool,
    },
    /// `itertools.product(*iterables, repeat=1)`：每个输入都**当场物化**成一个 `list`，
    /// `pools` 是"这些 list 的 list"；`indices` 是 odometer 游标。
    Product {
        /// 各输入的物化池（一个 `list`，元素都是 `list`；**本对象持有一份引用**）。
        pools: NonNull<Header>,
        /// odometer 游标（一个 `list`，**本对象持有一份引用**）。
        indices: NonNull<Header>,
        /// 是否已经产出过。
        started: bool,
        /// 是否已穷尽。
        done: bool,
    },
    /// `itertools.permutations(pool, r)`：与 [`ItStateKind::Combinations`] 同族，但下标**互不相同**
    /// 且按**字典序**推进（实测 `permutations([1,2,3])` 的顺序正是它）。
    Permutations {
        /// 物化后的池（一个 `list`，**本对象持有一份引用**）。
        pool: NonNull<Header>,
        /// 取几个。
        r: i64,
        /// 当前下标排列（一个 `list`，**本对象持有一份引用**）。
        indices: NonNull<Header>,
        /// 是否已经产出过。
        started: bool,
        /// 是否已穷尽。
        done: bool,
    },
    /// `itertools.zip_longest(*iterables, fillvalue=None)`：同时走多个迭代器，短的一侧用
    /// `fillvalue` 补；**全**耗尽才停。
    ZipLongest {
        /// 各内层迭代器（一个 `list`，**本对象持有一份引用**；每个元素本身也是迭代器引用）。
        iterators: NonNull<Header>,
        /// 补齐值（**本对象持有一份引用**）。
        fillvalue: NonNull<Header>,
    },
    /// `zip(*iterables)` ✓（第 229 轮）：与 `ZipLongest` **同构** ✓，区别只在"**缺项就收摊**" ✓
    /// （`zip` 取**最短** ✓、`zip_longest` 才补 `fillvalue` ✓）。
    Zip {
        /// 各内层迭代器（一个 `list`，**本对象持有一份引用**；每个元素本身也是迭代器引用）。
        iterators: NonNull<Header>,
    },
    /// `itertools.pairwise(iterable)`：两两成对（`(0,1)`、`(1,2)`…），`previous` 是上一项。
    Pairwise {
        /// 内层迭代器（**本对象持有一份引用**）。
        inner: NonNull<Header>,
        /// 上一项（**本对象持有一份引用**；`None` ⇒ 还没取到第一项）。
        previous: Option<NonNull<Header>>,
    },
    /// `itertools.batched(iterable, n)`：每批最多 `n` 个（末批可短）。
    Batched {
        /// 内层迭代器（**本对象持有一份引用**）。
        inner: NonNull<Header>,
        /// 批大小（`>= 1`，由模块面保证）。
        size: i64,
        /// 内层是否已耗尽（耗尽后再取值 ⇒ 直接 `None`）。
        done: bool,
    },
    /// `itertools.cycle(iterable)`：先把内层**边取边缓存**，取完就一直重放缓存。
    ///
    /// **实测**：惰性（取多少消费多少）；内层为空 ⇒ 立刻耗尽（重放空缓存也是空）。
    Cycle {
        /// 内层迭代器（**本对象持有一份引用**）。
        inner: NonNull<Header>,
        /// 缓存（一个 `list`，**本对象持有一份引用**）。
        cache: NonNull<Header>,
        /// 还在从内层取（取完转重放）。
        filling: bool,
        /// 重放游标。
        index: i64,
    },
    /// `itertools.starmap(function, iterable)`：每次把元素**展开**成实参调用。
    Starmap {
        /// 内层迭代器（**本对象持有一份引用**）。
        inner: NonNull<Header>,
        /// 被调用的可调用对象（**本对象持有一份引用**）。
        function: NonNull<Header>,
    },
    /// `itertools.chain(*iterables)`：`outer` 是"参数表"的迭代器，`current` 是当前内层
    /// （`None` ⇒ 该换下一个了）。两个字段都可能持对象引用 ⇒ 见 `it_state_traverse`／`clear`。
    Chain {
        /// 外层迭代器（**本对象持有一份引用**）。
        outer: NonNull<Header>,
        /// 当前内层迭代器（**本对象持有一份引用**；`None` ⇒ 还没开始或刚用完）。
        current: Option<NonNull<Header>>,
    },
}

py_object! {
    /// `itertools.repeat`／`islice` 的迭代器载荷。
    ///
    /// **持对象引用**（与 `CountIteratorObject` 不同）⇒ 必须挂 `traverse`／`clear`
    /// （`OM-40`／`OM-20` ②：`dealloc` 只释内存，引用由 `clear` 交出）。
    pub struct ItStateObject {
        kind: Cell<ItStateKind>,
    }
}

impl ItStateObject {
    /// 见 [`TupleObject::slots`]：载荷里的值可能指回迭代器自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(it_state_traverse)
            .with_clear(it_state_clear)
    }

    /// 当前状态。
    pub fn kind(&self) -> ItStateKind {
        self.kind.get()
    }

    /// 换状态（状态是 `Copy` ⇒ 用 `Cell` 就地改）。
    pub fn set_kind(&self, kind: ItStateKind) {
        self.kind.set(kind);
    }
}

/// `OM-40`：列出迭代器持有的引用。
pub(crate) unsafe fn it_state_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ItStateObject>() };
    match object.kind() {
        ItStateKind::Repeat { value, .. } => visit(value.as_ptr()),
        ItStateKind::Islice { inner, .. } => visit(inner.as_ptr()),
        ItStateKind::Chain { outer, current } => {
            visit(outer.as_ptr());
            if let Some(inner) = current {
                visit(inner.as_ptr());
            }
        }
        ItStateKind::FilterLike {
            inner, predicate, ..
        } => {
            visit(inner.as_ptr());
            visit(predicate.as_ptr());
        }
        ItStateKind::Accumulate {
            inner,
            function,
            total,
        } => {
            visit(inner.as_ptr());
            if let Some(value) = function {
                visit(value.as_ptr());
            }
            if let Some(value) = total {
                visit(value.as_ptr());
            }
        }
        ItStateKind::Starmap { inner, function } => {
            visit(inner.as_ptr());
            visit(function.as_ptr());
        }
        ItStateKind::Cycle { inner, cache, .. } => {
            visit(inner.as_ptr());
            visit(cache.as_ptr());
        }
        ItStateKind::Pairwise { inner, previous } => {
            visit(inner.as_ptr());
            if let Some(value) = previous {
                visit(value.as_ptr());
            }
        }
        ItStateKind::Batched { inner, .. } => visit(inner.as_ptr()),
        ItStateKind::ZipLongest {
            iterators,
            fillvalue,
        } => {
            visit(iterators.as_ptr());
            visit(fillvalue.as_ptr());
        }
        ItStateKind::Zip { iterators } => visit(iterators.as_ptr()),
        ItStateKind::Compress { data, selectors } => {
            visit(data.as_ptr());
            visit(selectors.as_ptr());
        }
        ItStateKind::Combinations {
            pool, indices, ..
        } => {
            visit(pool.as_ptr());
            visit(indices.as_ptr());
        }
        ItStateKind::Permutations {
            pool, indices, ..
        } => {
            visit(pool.as_ptr());
            visit(indices.as_ptr());
        }
        ItStateKind::Product {
            pools, indices, ..
        } => {
            visit(pools.as_ptr());
            visit(indices.as_ptr());
        }
    }
}

/// `OM-40`／`OM-20` ②：交出持有的那份引用。
pub(crate) unsafe fn it_state_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ItStateObject>() };
    match object.kind() {
        ItStateKind::Repeat { value, .. } => {
            // SAFETY: 该引用由本对象持有。
            unsafe { instance.release_object(value.as_ptr()) };
        }
        ItStateKind::Islice { inner, .. } => {
            // SAFETY: 同上。
            unsafe { instance.release_object(inner.as_ptr()) };
        }
        ItStateKind::Chain { outer, current } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(outer.as_ptr()) };
            if let Some(inner) = current {
                // SAFETY: 同上。
                unsafe { instance.release_object(inner.as_ptr()) };
            }
        }
        ItStateKind::FilterLike {
            inner, predicate, ..
        } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(inner.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(predicate.as_ptr()) };
        }
        ItStateKind::Accumulate {
            inner,
            function,
            total,
        } => {
            // SAFETY: 这些引用都由本对象持有。
            unsafe { instance.release_object(inner.as_ptr()) };
            if let Some(value) = function {
                // SAFETY: 同上。
                unsafe { instance.release_object(value.as_ptr()) };
            }
            if let Some(value) = total {
                // SAFETY: 同上。
                unsafe { instance.release_object(value.as_ptr()) };
            }
        }
        ItStateKind::Starmap { inner, function } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(inner.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(function.as_ptr()) };
        }
        ItStateKind::Cycle { inner, cache, .. } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(inner.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(cache.as_ptr()) };
        }
        ItStateKind::Pairwise { inner, previous } => {
            // SAFETY: 这些引用都由本对象持有。
            unsafe { instance.release_object(inner.as_ptr()) };
            if let Some(value) = previous {
                // SAFETY: 同上。
                unsafe { instance.release_object(value.as_ptr()) };
            }
        }
        ItStateKind::Batched { inner, .. } => {
            // SAFETY: 该引用由本对象持有。
            unsafe { instance.release_object(inner.as_ptr()) };
        }
        ItStateKind::ZipLongest {
            iterators,
            fillvalue,
        } => {
            // SAFETY: 两份引用都由本对象持有（列表里的迭代器引用由列表自己管）。
            unsafe { instance.release_object(iterators.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(fillvalue.as_ptr()) };
        }
        ItStateKind::Zip { iterators } => {
            // SAFETY: 该引用由本对象持有（列表里的迭代器引用由列表自己管）。
            unsafe { instance.release_object(iterators.as_ptr()) };
        }
        ItStateKind::Compress { data, selectors } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(data.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(selectors.as_ptr()) };
        }
        ItStateKind::Combinations {
            pool, indices, ..
        } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(pool.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(indices.as_ptr()) };
        }
        ItStateKind::Permutations {
            pool, indices, ..
        } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(pool.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(indices.as_ptr()) };
        }
        ItStateKind::Product {
            pools, indices, ..
        } => {
            // SAFETY: 两份引用都由本对象持有（池里那些 list 由它们自己管）。
            unsafe { instance.release_object(pools.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(indices.as_ptr()) };
        }
    }
}

py_object! {
    /// `itertools.count(start, step)` 的迭代器载荷。
    ///
    /// **只含整数** ⇒ 不持任何对象引用（`traverse` 面为零，`OM-40` 那套不用挂）。
    /// `itertools` 其余几个（`repeat`／`islice`／`chain`…）会持引用，届时各自处理 GC
    /// ——不硬塞进 `IteratorObject`（那会改动既有的引用遍历语义）。
    pub struct CountIteratorObject {
        current: Cell<i64>,
        step: Cell<i64>,
    }
}

impl CountIteratorObject {
    /// 见 [`TupleObject::slots`]：本类型不持对象引用 ⇒ 只有释放。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
    }

    /// 当前值。
    pub fn current(&self) -> i64 {
        self.current.get()
    }

    /// 推进一格；超出 `i64` 给 `None`（**不静默回绕**——任意精度的口径还没裁，见 `§5.2.6`）。
    pub fn bump(&self) -> Option<i64> {
        let next = self.current.get().checked_add(self.step.get())?;
        self.current.set(next);
        Some(next)
    }
}

impl AsendObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(asend_traverse)
            .with_clear(asend_clear)
            .with_repr(asend_repr)
    }

    /// 要推进的异步生成器（**借用**）。
    pub fn generator(&self) -> NonNull<Header> {
        self.generator
    }

    /// 送进去的值（**借用**）。
    pub fn sent(&self) -> Option<NonNull<Header>> {
        *self.sent.borrow()
    }
}

/// `OM-40`：列出 asend 持有的引用。
pub(crate) unsafe fn asend_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<AsendObject>() };
    visit(object.generator().as_ptr());
    if let Some(sent) = object.sent() {
        visit(sent.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出 asend 持有的引用。
pub(crate) unsafe fn asend_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &mut *ptr.cast::<AsendObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.generator().as_ptr()) };
    if let Some(sent) = object.sent.replace(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(sent.as_ptr()) };
    }
}

/// 实测 `repr`：`<async_generator_asend object at 0x…>`（**没有**类型名）。
pub(crate) unsafe fn asend_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    Ok(format!("<async_generator_asend object at {ptr:p}>"))
}

impl GeneratorObject {
    /// 见 [`TupleObject::slots`]：帧里的局部槽可能指回生成器自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(generator_traverse)
            .with_clear(generator_clear)
    }

    /// 生成器的帧（**借用**）。
    pub fn frame(&self) -> NonNull<Header> {
        self.frame
    }

    /// 是否已跑完。
    pub fn finished(&self) -> bool {
        self.finished.get()
    }

    /// 置"已跑完"。
    pub fn mark_finished(&self) {
        self.finished.set(true);
    }

    /// **是否已经启动过**（`next()`／`send(None)` 跑过第一次之后为真）。
    pub fn started(&self) -> bool {
        self.started.get()
    }

    /// 置"已经启动过"。
    pub fn mark_started(&self) {
        self.started.set(true);
    }
}

/// 协程的 `repr`：与生成器同一套逻辑，只是词不同（实测 `<coroutine object f at 0x…>`）。
pub unsafe fn coroutine_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    unsafe { generator_repr_named(ptr, instance, "coroutine") }
}

/// 异步生成器的 `repr`（实测 `<async_generator object f at 0x…>`）。
pub unsafe fn async_generator_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    unsafe { generator_repr_named(ptr, instance, "async_generator") }
}

/// `async_generator.__anext__()`：交出一个 awaitable（`AsendObject`）。
pub(crate) unsafe fn async_generator_anext_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("__anext__ 是绑定方法，必须有 self");
    make_asend(instance, generator, None)
}

/// `async_generator.asend(value)`：同上，但把值带进去。
pub(crate) unsafe fn async_generator_asend_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("asend 是绑定方法，必须有 self");
    make_asend(instance, generator, args.first().copied())
}

/// 造一个 `AsendObject`（**新引用**）。
pub(crate) fn make_asend(
    instance: &Instance,
    generator: NonNull<Header>,
    sent: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    let asend_type = instance.type_named("async_generator_asend").expect("引导期已登记");
    // SAFETY: generator 由调用方保证存活，本对象要自己那份。
    unsafe { instance.incref_object(generator.as_ptr()) };
    let sent_reference = match sent {
        Some(value) => {
            // SAFETY: value 由调用方保证存活。
            unsafe { instance.incref_object(value.as_ptr()) };
            Some(value)
        }
        None => None,
    };
    let object = instance.alloc(AsendObject::new(asend_type, generator, RefCell::new(sent_reference)));
    Ok(object.into_raw().cast::<Header>())
}

/// 生成器方法共用的入口：`send` 对"刚创建"的生成器只接受 `None`。
pub(crate) unsafe fn resume_with_sent(
    instance: &Instance,
    generator: NonNull<Header>,
    sent: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 槽位契约保证这是本实例里存活的生成器。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    if object.finished() {
        return Err(stop_iteration(instance, None));
    }
    let just_started = !object.started();
    if just_started {
        if let Some(value) = sent {
            // SAFETY: value 是存活对象。
            let is_none = unsafe { value.as_ref() }.ty() == instance.singletons().none_type();
            if !is_none {
                // 词随类型走（实测：生成器说 `generator`、协程说 `coroutine`）
                let word = instance.type_name(instance.type_of(generator));
                return Err(crate::executor::runtime::raise_builtin(
                    instance,
                    "TypeError",
                    &format!("can't send non-None value to a just-started {word}"),
                ));
            }
        }
    }
    object.mark_started();
    match crate::executor::resume_generator(instance, generator, sent)? {
        crate::executor::GeneratorOutcome::Yielded(value) => Ok(value),
        crate::executor::GeneratorOutcome::Returned(value) => {
            // 返回值**属于本函数**（新引用），交给异常当实参
            Err(stop_iteration(instance, Some(value)))
        }
    }
}

impl IteratorObject {
    /// 见 [`TupleObject::slots`]：被迭代的对象可能指回迭代器自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(iterator_traverse)
            .with_clear(iterator_clear)
    }

    /// 被迭代的对象（**借用**）。
    pub fn target(&self) -> NonNull<Header> {
        self.target
    }

    /// 游标。
    pub fn index(&self) -> usize {
        self.index.get()
    }

    /// 推进游标。
    pub fn advance(&self) {
        self.index.set(self.index.get() + 1);
    }
}

/// `OM-40`：列出迭代器持有的引用。
pub(crate) unsafe fn iterator_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IteratorObject>() };
    visit(object.target().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出被迭代对象。
pub(crate) unsafe fn iterator_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IteratorObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.target().as_ptr()) };
}
