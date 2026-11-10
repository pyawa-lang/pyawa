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
        /// **正在执行**（round 56）：重入（生成器体里调 `next(自己)`）参照报
        /// `ValueError: generator already executing` ✓；先前**不查** ✗ ⇒ 递归恢复同一帧 ⇒
        /// 帧操作 `StackUnderflow`（`test_genexps` 的 `g = (next(me) for i in range(10))` ✓）。
        running: Cell<bool>,
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
        {
            /// **字典迭代的键集版本守卫**（`P3-41` round 42）：首次推进时记下 `dict.version`，
            /// 之后每次推进比对 ⇒ 迭代中改键就报 `RuntimeError: dictionary changed size during
            /// iteration` ✓（参照口径；先前**不检测** ⇒ `for i in d: d[i+1]=1` 永远不结束 ✗）。
            guard: Cell<u64>,
            /// 首个推进时的**长度**（区分"改大小"与"改键集"两档文案 ✓）。
            guard_len: Cell<usize>,
            /// 守卫是否已武装（`Cell<bool>` 的默认值是 `false` ✓）。
            armed: Cell<bool>,
        }
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
    /// **内建 `map(function, *iterables)`**（`P3-41` round 41：由**急求值**改正为惰性 ✓）。
    ///
    /// 语义同 `zip`（各 inner 各取一项，任一耗尽就收摊）＋ 每行交 `function(*row)` ✓。
    Map {
        /// 映射函数（**本对象持有一份引用**）。
        function: NonNull<Header>,
        /// 各内层迭代器（一个 `list`，**本对象持有一份引用**）。
        iterators: NonNull<Header>,
    },
    /// **内建 `filter(function, iterable)`**（同轮改正为惰性 ✓）；`predicate` 是 `None` ⇒ 按真值筛。
    Filter {
        /// 谓词（**本对象持有一份引用**；可能是 `None`）。
        predicate: NonNull<Header>,
        /// 内层迭代器（**本对象持有一份引用**）。
        inner: NonNull<Header>,
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
        ItStateKind::Map {
            function,
            iterators,
        } => {
            visit(function.as_ptr());
            visit(iterators.as_ptr());
        }
        ItStateKind::Filter { predicate, inner } => {
            visit(predicate.as_ptr());
            visit(inner.as_ptr());
        }
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
        ItStateKind::Map {
            function,
            iterators,
        } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(function.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(iterators.as_ptr()) };
        }
        ItStateKind::Filter { predicate, inner } => {
            // SAFETY: 两份引用都由本对象持有。
            unsafe { instance.release_object(predicate.as_ptr()) };
            // SAFETY: 同上。
            unsafe { instance.release_object(inner.as_ptr()) };
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

    /// 步长（round 185：`range_iterator` 的 `__reduce__` 要按它算剩余值）。
    pub fn step(&self) -> i64 {
        self.step.get()
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

    /// 是否**正在执行**（重入守卫）。
    pub fn running(&self) -> bool {
        self.running.get()
    }

    /// 置／清"正在执行"。
    pub fn set_running(&self, value: bool) {
        self.running.set(value);
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

    /// **标记已耗尽**（round 110）：`index = usize::MAX` 是哨兵 —— 参照的迭代器一旦吐过
    /// `StopIteration` 就**永久耗尽** ✓（`list_tests.test_exhausted_iterator`：耗尽后 `append(9)`
    /// 不能再吐 ✓；`test_tier2_invalidates_iterator` ✓）。先前只看"下标 < 当前长度" ✗
    /// ⇒ 追加元素后又"复活" ✗。
    pub fn set_index(&self, value: usize) {
        self.index.set(value);
    }

    pub fn mark_exhausted(&self) {
        self.index.set(usize::MAX);
    }

    /// 是否已被标记耗尽。
    pub fn is_exhausted(&self) -> bool {
        self.index.get() == usize::MAX
    }

    /// **pickle 反序列化回填游标**（round 111）：直接写 `index` ✓（`__setstate__` 用）。
    pub fn set_index_for_pickle(&self, index: usize) {
        self.index.set(index);
    }

    /// **字典迭代的版本守卫**：第一次推进时记下 `(版本, 长度)`；之后版本变 ⇒ 给
    /// `RuntimeError` 的**两档文案**（照参照：长度也变 ⇒ `changed size`；只变键集 ⇒ `changed keys` ✓）。
    /// 非字典迭代器的 `target` 不是 `dict` ⇒ 调用方不调它 ✓。
    pub fn check_dict_version(
        &self,
        instance: &Instance,
    ) -> Result<Option<&'static str>, ()> {
        // SAFETY: target 由本对象持有，存活；调用方只在 `dict_keyiterator` 上调。
        let dict = unsafe { &*self.target.as_ptr().cast::<crate::builtin_objects::DictObject>() };
        let version = dict.version();
        let length = dict.len();
        if self.armed.get() {
            if self.guard.get() != version {
                return Err(());
            }
            return Ok(None);
        }
        self.guard.set(version);
        self.guard_len.set(length);
        self.armed.set(true);
        let _ = instance;
        Ok(None)
    }

    /// 守卫武装时记下的长度（给"两档文案"用 ✓）。
    pub fn guard_len(&self) -> usize {
        self.guard_len.get()
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

/// **内建迭代器的属性面**（round 111）：`__next__`／`__iter__`／`__reduce__`／`__setstate__`。
///
/// 为什么必须有：① `iter(x).__next__` 在参照里可取 ✓（本层先前 AttributeError ✗）；
/// ② **pickle** 在 protocol 0/1 走 `__reduce_ex__` 的二元组 ⇒ `save_global` 按模块名找类型
/// ⇒ `Can't pickle <class 'list_iterator'>: it's not found as builtins.list_iterator` ✗；
/// 参照的迭代器都实现了 `__reduce__`（`(iter, (target,), next_index)` 形 ✓）与 `__setstate__` ✓
///（`list_tests.test_iterator_pickle`／`test_reversed_pickle` ✓）。
#[allow(unsafe_code)]
pub unsafe fn iterator_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    match name {
        "__next__" => {
            let handler: crate::NativeFn = iterator_next_native;
            Some(bind_iterator_method(instance, ptr, "__next__", handler))
        }
        "__iter__" => {
            // 迭代器的 `__iter__` 回自身 ✓。
            // SAFETY: ptr 是本类型的存活对象。
            unsafe { instance.incref_object(ptr) };
            Some(unsafe { NonNull::new_unchecked(ptr) })
        }
        "__reduce__" => {
            let handler: crate::NativeFn = iterator_reduce_native;
            Some(bind_iterator_method(instance, ptr, "__reduce__", handler))
        }
        // **`__reduce_ex__` 也走同一处** ✓（round 119）：pickle 优先调 `__reduce_ex__(proto)` ✓；
        // 先前只在 `__reduce__` 上给 ⇒ 惰性建的 `dict_itemiterator`／`dict_valueiterator` 落回
        // 默认"按类型构造" ⇒ `TypeError: cannot create '…' instances` ✗
        //（`test_itemiterator_pickling`／`test_valuesiterator_pickling` ✓）。
        "__reduce_ex__" => {
            let handler: crate::NativeFn = iterator_reduce_native;
            Some(bind_iterator_method(instance, ptr, "__reduce_ex__", handler))
        }
        "__setstate__" => {
            let handler: crate::NativeFn = iterator_setstate_native;
            Some(bind_iterator_method(instance, ptr, "__setstate__", handler))
        }
        _ => None,
    }
}

/// **`range_iterator`／`longrange_iterator` 的属性面**（round 185）：`__next__`／`__iter__`／
/// `__reduce__`／`__reduce_ex__` —— 后两者是官方 `test_iter` 的 `check_pickle` 一族所需 ✓。
///
/// 载荷是 `ItStateObject`（与 `IteratorObject` 布局不同 ✗）⇒ **不能**走 `iterator_getattr` ✓。
/// `__reduce__` 给 `(iter, (剩余值列表,))`：不 mutate 原迭代器 ✓（按 `Islice{position,start,stop,step}`
/// ＋内层 `count{current,step}` **算术**推出剩余值 ✓；超 1e6 项不接（返回 Unsupported，如实 ✓）。
#[allow(unsafe_code)]
pub unsafe fn range_iterator_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    match name {
        "__next__" => {
            let handler: crate::NativeFn = iterator_next_native;
            Some(bind_iterator_method(instance, ptr, "__next__", handler))
        }
        "__iter__" => {
            // 迭代器的 `__iter__` 回自身 ✓。
            // SAFETY: ptr 是本类型的存活对象。
            unsafe { instance.incref_object(ptr) };
            Some(unsafe { NonNull::new_unchecked(ptr) })
        }
        "__reduce__" | "__reduce_ex__" => {
            let handler: crate::NativeFn = range_iterator_reduce_native;
            Some(bind_iterator_method(instance, ptr, name, handler))
        }
        _ => None,
    }
}

/// `range_iterator.__reduce__()`：`(iter, (剩余值列表,))` ✓（见 [`range_iterator_getattr`] 的口径）。
fn range_iterator_reduce_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(iterator) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__reduce__() 需要迭代器实例"));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    let state = unsafe { &*iterator.as_ptr().cast::<ItStateObject>() };
    let crate::builtin_objects::ItStateKind::Islice {
        inner,
        start,
        position,
        stop,
        step,
    } = state.kind()
    else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "range_iterator 的状态不是 Islice",
        ));
    };
    // 内层必须是 `count`（`range` 就是 `islice(count(...))` ✓；见 bootstrap 的注释 ✓）。
    // SAFETY: inner 由本对象持有，存活；类型名先核过。
    let inner_name = instance.type_name(instance.type_of(inner));
    if inner_name != "count" {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("range_iterator 的内层不是 count（是 '{inner_name}'）"),
        ));
    }
    // SAFETY: 类型名刚确认是 count ⇒ 载荷布局是 `CountIteratorObject` ✓。
    let counter = unsafe { &*inner.as_ptr().cast::<CountIteratorObject>() };
    let (current, stride) = (counter.current(), counter.step());
    let bound = if stop < 0 { i64::MAX } else { start.max(stop) };
    if bound.saturating_sub(position) > 1_000_000 {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "range_iterator.__reduce__：剩余项超过 1e6（如实未接线）",
        });
    }
    let mut remaining: Vec<NonNull<Header>> = Vec::new();
    for index in position..bound {
        let wanted = index >= start
            && (stop < 0 || index < stop)
            && (index - start) % step == 0;
        if !wanted {
            continue;
        }
        let value = current.saturating_add(index.saturating_mul(stride));
        remaining.push(instance.new_int(value));
    }
    let Some(callable) = instance
        .builtins()
        .and_then(|builtins| instance.dict_get(builtins, "iter"))
    else {
        return Err(instance.raise_builtin_error("RuntimeError", "内建 iter 不可用"));
    };
    let list = instance.new_list(remaining);
    let args = instance.new_tuple(vec![list]);
    Ok(instance.new_tuple(vec![instance.retain(callable), args]))
}

/// **`callable_iterator` 的属性面**（round 185）：`__next__`／`__iter__`／`__reduce__`。
///
/// 参照的 reduce 形态是 `(iter, (callable, sentinel))`（**无 state** ✓）：进度状态由**被 pickle 的
/// callable 自身**携带 ✓（`test_iter.test_iter_callable` 的 `check_pickle` ✓）。
#[allow(unsafe_code)]
pub unsafe fn callable_iterator_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    match name {
        "__next__" => {
            let handler: crate::NativeFn = iterator_next_native;
            Some(bind_iterator_method(instance, ptr, "__next__", handler))
        }
        "__iter__" => {
            // SAFETY: ptr 是本类型的存活对象。
            unsafe { instance.incref_object(ptr) };
            Some(unsafe { NonNull::new_unchecked(ptr) })
        }
        "__reduce__" | "__reduce_ex__" => {
            let handler: crate::NativeFn = callable_iterator_reduce_native;
            Some(bind_iterator_method(instance, ptr, name, handler))
        }
        _ => None,
    }
}

fn callable_iterator_reduce_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(iterator) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__reduce__() 需要迭代器实例"));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*iterator.as_ptr().cast::<CallableIteratorObject>() };
    let Some(callable) = instance
        .builtins()
        .and_then(|builtins| instance.dict_get(builtins, "iter"))
    else {
        return Err(instance.raise_builtin_error("RuntimeError", "内建 iter 不可用"));
    };
    let args = instance.new_tuple(vec![
        instance.retain(object.callable()),
        instance.retain(object.sentinel()),
    ]);
    Ok(instance.new_tuple(vec![instance.retain(callable), args]))
}

/// 把原生函数绑到迭代器实例上（与 `list_getattr` 同一手法 ✓）。
#[allow(unsafe_code)]
fn bind_iterator_method(
    instance: &Instance,
    ptr: *mut Header,
    name: &str,
    handler: crate::NativeFn,
) -> NonNull<Header> {
    let builtin_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(crate::BuiltinFunctionObject::new(
        builtin_type,
        Box::leak(name.to_string().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(crate::MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        unsafe { NonNull::new_unchecked(ptr) },
    ));
    bound.into_raw().cast::<Header>()
}

/// `it.__next__()` 的原生实现：推进本迭代器（与 `advance_iterator` 同一处真相 ✓）。
#[allow(unsafe_code)]
fn iterator_next_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(iterator) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__next__() 需要迭代器实例",
        ));
    };
    match crate::executor::advance_iterator(instance, iterator, 0)? {
        Some(value) => Ok(value),
        None => Err(instance.raise_builtin_error("StopIteration", "")),
    }
}

/// `it.__setstate__(state)`：`pickle` 反序列化回填游标 ✓（正向 = next 下标；反向 = next+1 ✓）。
#[allow(unsafe_code)]
fn iterator_setstate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(iterator) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__setstate__() 需要迭代器实例",
        ));
    };
    let Some(state) = args
        .first()
        .copied()
        .and_then(|value| instance.int_of(value))
        .and_then(|number| number.to_i64())
    else {
        return Err(instance.raise_builtin_error("TypeError", "__setstate__() 要整数"));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*iterator.as_ptr().cast::<IteratorObject>() };
    let type_name = instance.type_name(instance.type_of(iterator));
    let reverse = matches!(type_name.as_str(), "list_reverseiterator" | "reversed");
    let index = if reverse { state.saturating_add(1) } else { state };
    if index < 0 {
        return Err(instance.raise_builtin_error("TypeError", "__setstate__() 游标为负"));
    }
    // 直接写游标（`IteratorObject` 没有 set_index；用 Cell 的方法经 `advance` 不适用于此）。
    object.set_index_for_pickle(index as usize);
    Ok(instance.retain(instance.singletons().none()))
}

/// `it.__reduce__()`：返回 `(iter|reversed, (target,), state)` ✓（形状照参照实测 ✓）。
#[allow(unsafe_code)]
fn iterator_reduce_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(iterator) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__reduce__() 需要迭代器实例",
        ));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*iterator.as_ptr().cast::<IteratorObject>() };
    let type_name = instance.type_name(instance.type_of(iterator));
    let reverse = matches!(type_name.as_str(), "list_reverseiterator" | "reversed");
    let callable_name = if reverse { "reversed" } else { "iter" };
    let Some(callable) = instance
        .builtins()
        .and_then(|builtins| instance.dict_get(builtins, callable_name))
    else {
        return Err(instance.raise_builtin_error(
            "RuntimeError",
            "内建 iter／reversed 不可用",
        ));
    };
    // **耗尽形态**照参照：`(iter|reversed, ([],))` —— **二**元组、参数是**空列表**、没有状态 ✓
    //（实测 `reversed([...])` 耗尽后 `__reduce__()` ⇒ `(<class 'reversed'>, ([],))` ✓；正反向同款 ✓）。
    // 这一档必须单列：耗尽游标（正向 `usize::MAX`／反向 0）直接当 state 传给 `__setstate__` 会
    // 越界（先前反向 `saturating_sub` 给 0 ⇒ 反序列化后多吐一个元素 ✗，`test_reversed_pickle` 的
    // "empty iterator" 档 `[10] != []` ✓）。
    let exhausted = if reverse {
        object.index() == 0
    } else {
        object.is_exhausted()
    };
    if exhausted {
        let empty = instance.new_list(Vec::new());
        return Ok(instance.new_tuple(vec![
            instance.retain(callable),
            instance.new_tuple(vec![empty]),
        ]));
    }
    // **dict 的 items()／values() 迭代器在参照里被 pickle 成"列表快照"** ✓（round 119）：
    // 实测 `iter(d.items()).__reduce_ex__(4)` ⇒ `(<built-in function iter>, ([('a', 1), …],))`，
    // 反序列化得到 **list_iterator** ✓（视图本身**不可** pickle ✓，直接交视图会 `PicklingError` ✗）。
    let target = if type_name == "dict_itemiterator" {
        let entries = instance
            .dict_entries(object.target())
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| {
                instance.new_tuple(vec![instance.retain(key), instance.retain(value)])
            })
            .collect();
        instance.new_list(entries)
    } else if type_name == "dict_valueiterator" {
        let values = instance
            .dict_entries(object.target())
            .unwrap_or_default()
            .into_iter()
            .map(|(_, value)| instance.retain(value))
            .collect();
        instance.new_list(values)
    } else {
        instance.retain(object.target())
    };
    let args = instance.new_tuple(vec![target]);
    let state = if reverse {
        object.index().saturating_sub(1) as i64
    } else {
        object.index() as i64
    };
    Ok(instance.new_tuple(vec![
        instance.retain(callable),
        args,
        instance.new_int(state),
    ]))
}

py_object! {
    /// **序列协议（`__getitem__`）的惰性迭代器**（round 85；参照名 `iterator`）：
    /// 每次 `__next__` 用**当前下标**调 `target[index]`，`IndexError` ⇒ `StopIteration`（耗尽）✓。
    /// 先前是"`iter()` 里一次性把整个序列物化" ✗ ⇒ 对**无限序列**（`__getitem__` 永不抛
    /// IndexError，如 `test_iter` 的 `UnlimitedSequenceClass`）当场死循环 ✗。
    pub struct SequenceIteratorObject {
        /// 被索引的对象（**本对象持有一份引用**）。
        target: NonNull<Header>,
        /// 下一个要取的下标 ✓。
        index: core::cell::Cell<usize>,
    }
}

impl SequenceIteratorObject {
    /// 槽位表：`dealloc` ＋ `traverse`／`clear`。
    pub fn slots() -> crate::type_object::Slots {
        crate::type_object::Slots::new(Self::dealloc)
            .with_traverse(sequence_iterator_traverse)
            .with_clear(sequence_iterator_clear)
            // **`__next__`／`__iter__`／`__setstate__`** 属性面（round 85）✓。
            .with_getattr(sequence_iterator_getattr)
    }

    /// 被索引的对象（借用）。
    pub fn target(&self) -> NonNull<Header> {
        self.target
    }

    /// 当前下标（借用）。
    pub fn index(&self) -> usize {
        self.index.get()
    }

    /// 写回下标（`__setstate__`／推进用）✓。
    pub fn set_index(&self, index: usize) {
        self.index.set(index);
    }
}

unsafe fn sequence_iterator_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<SequenceIteratorObject>() };
    // 访问器形态（`gc_field_coverage` 的判据要求 `object.<字段>(` ✓）。
    visit(object.target().as_ptr());
}

unsafe fn sequence_iterator_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &mut *ptr.cast::<SequenceIteratorObject>() };
    // SAFETY: 这份引用由本对象持有（`clear` 只会跑一次 ✓）。
    unsafe { instance.release_object(object.target.as_ptr()) };
}

/// `sequence_iterator` 的属性面：`__next__`／`__iter__`／`__setstate__` ✓
/// （`__reduce__` 暂不给：需要 `iter(target)` 形式的重建器，留后续轮 ✓ —— 如实）。
pub unsafe fn sequence_iterator_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    match name {
        "__next__" => {
            let handler: crate::NativeFn = iterator_next_native;
            Some(bind_iterator_method(instance, ptr, "__next__", handler))
        }
        "__iter__" => {
            // SAFETY: ptr 是本类型的存活对象。
            unsafe { instance.incref_object(ptr) };
            Some(unsafe { NonNull::new_unchecked(ptr) })
        }
        "__setstate__" => {
            let handler: crate::NativeFn = sequence_iterator_setstate_native;
            Some(bind_iterator_method(instance, ptr, "__setstate__", handler))
        }
        // **`__reduce__`／`__reduce_ex__`**（round 185）：参照 `iterator` 的形态是
        // `(iter, (target,), index)` ✓（`test_iter` 的 seq-class pickle 用例 ✓；`__setstate__` 已在上面 ✓）。
        "__reduce__" | "__reduce_ex__" => {
            let handler: crate::NativeFn = sequence_iterator_reduce_native;
            Some(bind_iterator_method(instance, ptr, name, handler))
        }
        _ => None,
    }
}

/// `sequence_iterator.__reduce__()`：`(iter, (target,), index)` ✓。
fn sequence_iterator_reduce_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(iterator) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__reduce__() 需要迭代器实例"));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*iterator.as_ptr().cast::<SequenceIteratorObject>() };
    let Some(callable) = instance
        .builtins()
        .and_then(|builtins| instance.dict_get(builtins, "iter"))
    else {
        return Err(instance.raise_builtin_error("RuntimeError", "内建 iter 不可用"));
    };
    let args = instance.new_tuple(vec![instance.retain(object.target())]);
    Ok(instance.new_tuple(vec![
        instance.retain(callable),
        args,
        instance.new_int(object.index() as i64),
    ]))
}

/// `it.__setstate__(state)`（序列迭代器，round 85）：负数**夹到 0**（参照行为：
/// `test_iter_neg_setstate` 里 `setstate(-42)` 之后 `next` 给 `0, 1` ✓）。
fn sequence_iterator_setstate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(iterator) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__setstate__() 需要迭代器实例",
        ));
    };
    let Some(state) = args
        .first()
        .copied()
        .and_then(|value| instance.int_of(value))
        .and_then(|number| number.to_i64())
    else {
        return Err(instance.raise_builtin_error("TypeError", "__setstate__() 要整数"));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*iterator.as_ptr().cast::<SequenceIteratorObject>() };
    object.set_index(usize::try_from(state).unwrap_or(0));
    Ok(instance.retain(instance.singletons().none()))
}

py_object! {
    /// **`iter(callable, sentinel)` 的迭代器**（round 85，参照名 `callable_iterator`）：
    /// 每次 `__next__` 调 `callable()`，结果 `== sentinel` 就 `StopIteration`（结果本身不再交出）✓。
    /// 两个字段都**持有引用** ⇒ 挂 `traverse`／`clear` ✓（`OM-40`／`OM-20` ②）。
    pub struct CallableIteratorObject {
        /// 被反复调用的可调用对象（**本对象持有一份引用**）。
        callable: NonNull<Header>,
        /// 停止哨兵值（**本对象持有一份引用**）。
        sentinel: NonNull<Header>,
    }
}

impl CallableIteratorObject {
    /// 槽位表：`dealloc` ＋ `traverse`／`clear`。
    pub fn slots() -> crate::type_object::Slots {
        crate::type_object::Slots::new(Self::dealloc)
            .with_traverse(callable_iterator_traverse)
            .with_clear(callable_iterator_clear)
    }

    /// 被调用的可调用对象（借用）。
    pub fn callable(&self) -> NonNull<Header> {
        self.callable
    }

    /// 停止哨兵（借用）。
    pub fn sentinel(&self) -> NonNull<Header> {
        self.sentinel
    }
}

/// 遍历两格引用（生成器/迭代器可成环 ⇒ 必须走 GC ✓）。
unsafe fn callable_iterator_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<CallableIteratorObject>() };
    visit(object.callable().as_ptr());
    visit(object.sentinel().as_ptr());
}

/// 交出两格引用（`OM-21`：逐个释放）。
unsafe fn callable_iterator_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &mut *ptr.cast::<CallableIteratorObject>() };
    // SAFETY: 这两份引用由本对象持有。
    unsafe {
        instance.release_object(object.callable.as_ptr());
        instance.release_object(object.sentinel.as_ptr());
    }
}
