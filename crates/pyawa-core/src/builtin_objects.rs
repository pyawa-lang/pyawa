//! 内建类型的**载荷**（`docs/SPEC-type-system.md` 的 `TS-43`：布局由实现自选，不进 ABI）。
//!
//! **TS-41** 的表（`crate::builtin_types`）只记"类型存在、层次正确"；本文件才是它们的表示。
//! 只有 `OM-23` 点名的那几个才做单例（`None`／`True`／`False`／小整数／空串），
//! 其余类型"每次造一个新对象"——`is` 语义因此与参照实现一致（`OM-39`）。

use core::cell::{Cell, RefCell};

use crate::bigint::{BigInt, IntValue};
use crate::executor::ExecError;
use core::ptr::NonNull;

use crate::header::Header;
use crate::instance::Instance;
use crate::py_object;
use crate::type_object::Slots;

py_object! {
    /// `None` 的单例载体。*占位*：Python 层类型名与协议随后补。
    pub struct NoneObject {}
}

py_object! {
    /// `True`／`False` 的单例载体。
    pub struct BoolObject {
        /// 真假。
        value: bool,
    }
}

py_object! {
    /// `int` 的载体（`TS-45`）：小整数内联、大整数走堆上的 `BigInt`——
    /// **同一个类型对象**的两种载荷（`type(2**100) is int`）。
    pub struct IntObject {
        /// 载荷。
        value: IntValue,
    }
}

py_object! {
    /// `object` 的实例。*占位*：`object()` 不携带状态。
    pub struct PlainObject {}
}

/// 原生（Rust 实现）可调用的签名（`AB-24` 的宿主函数最终也走这条）。
///
/// 实参是**借用视图**——要留住的必须自己 incref；返回值是**新引用**。
/// 出错时返回 [`crate::ExecError`]（脚本异常经它冒泡）。
pub type NativeFn = unsafe fn(
    &Instance,
    Option<NonNull<Header>>,
    &[NonNull<Header>],
    &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError>;

py_object! {
    /// **原生可调用对象**（`builtin_function_or_method`）：Rust 函数 ＋ 一个名字。
    ///
    /// 它是 `AB-24`／`AB-25` 的宿主函数、`__build_class__` 一类内建函数的落点；
    /// 绑定了 `self` 的形态（`[].append`）只是多带一个 `self`（本层用 `MethodObject` 表达绑定，
    /// 故这里只存函数与名字）。
    pub struct BuiltinFunctionObject {
        /// 名字（`repr` 用；最终应当是 `str` 对象）。
        name: &'static str,
        /// Rust 实现。
        function: Cell<NativeFn>,
    }
}

py_object! {
    /// **绑定方法**：函数 ＋ 要绑上去的 `self`（两者都持有一份引用）。
    ///
    /// `OM-11` 的 `getattr` 在类型字典里查到函数时产出它：`obj.method`（**不调用**）拿到的是
    /// 这个对象，而 `obj.method()`（编译器的取方法位）仍然走"函数 ＋ `self`"的栈形态。
    /// 有了它，`OM-14` 的子类分派（`__init__`／`__new__`／`__del__` 的覆写）与生成器方法
    /// （`send`／`throw`／`close`）才有落点。
    pub struct MethodObject {
        /// 函数对象（**本对象持有一份引用**）。
        function: NonNull<Header>,
        /// 绑定的实例（**本对象持有一份引用**）。
        this: NonNull<Header>,
    }
}

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
    /// 异常实例：`args` ＋ 链（`__cause__`／`__context__`）＋ 抑制标志。
    ///
    /// 一个 Rust 载荷支撑表里那整棵 `BaseException` 树（`TS-43`：布局自选）。
    /// `BC-60` ② 要求"当前异常状态按实例存"，那条状态在 [`crate::Instance`] 上，不在这里。
    pub struct ExceptionObject {
        /// 构造实参（`e.args`）。
        args: RefCell<Vec<NonNull<Header>>>,
        /// `raise X from Y` 里的 `Y`（**本对象持有一份引用**）。
        cause: RefCell<Option<NonNull<Header>>>,
        /// 隐式上下文（正在处理的那个异常）。
        context: RefCell<Option<NonNull<Header>>>,
        /// `raise X from None` 会把上下文抑制掉（参照实现里 `__suppress_context__`）。
        suppress_context: Cell<bool>,
        /// `e.args` 读出来的那个 `tuple`（**本对象持有一份引用**）。
        ///
        /// 实测：`e.args is e.args` 为真（每个实例缓一个），而两个不同实例的 `args`
        /// 不是同一对象 ⇒ 必须按实例缓存，不能每次现造。
        args_tuple: RefCell<Option<NonNull<Header>>>,
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
unsafe fn it_state_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
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
unsafe fn it_state_clear(ptr: *mut Header, instance: &Instance) {
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

py_object! {
    /// **用户定义的类**的实例载荷：带一个属性字典（`STORE_ATTR` 写这里）。
    ///
    /// `TS-43`：载荷布局由实现自选。**为什么单独一个类型**：参照实现里 `object()` **没有**
    /// `__dict__`，而用户类的实例有——把字典挂在 `PlainObject` 上就会把 `object()` 也带偏。
    pub struct AttributeObject {
        /// 属性字典（惰性创建）。
        attributes: RefCell<Option<NonNull<Header>>>,
    }
}

py_object! {
    /// `float` 的实例（C `double`，与参照实现一致）。
    pub struct FloatObject {
        /// 数值；`inf`／`nan` 照旧。
        value: f64,
    }
}

py_object! {
    /// `function` 的实例：一个 code object ＋ 默认值。
    ///
    /// **注解与 `__qualname__` 都已就位**（`SET_FUNCTION_ATTRIBUTE` 的 16／2 与 `code_with_qualname`）；
    /// 仍缺的是**闭包**（`COPY_FREE_VARS` 一族）；下面这行是历史沿革
    /// 的其余标志位（实测 8 ＝ closure、16 ＝ annotate）与属性族补齐。
    pub struct FunctionObject {
        /// 被执行的 code object（**本对象持有一份引用**）。
        code: NonNull<Header>,
        /// 位置参数默认值（对齐到**尾部**若干位置参数，与参照实现一致）。
        defaults: Vec<NonNull<Header>>,
        /// 仅关键字参数默认值（`dict`，可为空）。
        kwdefaults: Option<NonNull<Header>>,
        /// **`__globals__`**：函数**定义处**的全局映射（**本对象持有一份引用**）。
        ///
        /// `MAKE_FUNCTION` 时从当前帧取（`BC-57` 的 `LOAD_GLOBAL` 要它）；
        /// 模块体的帧没有单独的全局表，此时取它的**命名空间**。
        globals: RefCell<Option<NonNull<Header>>>,
        /// **闭包**（`SET_FUNCTION_ATTRIBUTE` 的 bit3 `closure(8)`；第 82 轮接线）：
        /// 值是 **cell 元组**，建帧时装进自由槽（`CPython` 3.11+ 在建帧阶段做）。
        closure: RefCell<Vec<NonNull<Header>>>,
        /// **`__annotate__`**（`SET_FUNCTION_ATTRIBUTE` 的 bit4；3.14 的**延迟注解**协议，
        /// `SPEC-bytecode.md` §… 的表与 `SPEC-type-system.md` 都要求它存在）。
        ///
        /// 值是那个"按 `format` 参数产出注解字典"的**可调用对象**（本对象持一份引用）；
        /// `__annotations__` 已落地（惰性调用 ＋ 缓存）；`__annotate_func__`／
        /// `__annotations_cache__` 在参照实现里**不是**函数属性（那是 `typing` 自己对象上的名字）。
        annotate: RefCell<Option<NonNull<Header>>>,
        /// **`__annotations__` 的缓存**（实测：同一函数的 `f.__annotations__` 是**同一对象**，
        /// 且 `__annotate__` 只被调用**一次**）。
        annotations_cache: RefCell<Option<NonNull<Header>>>,
    }
}

py_object! {
    /// `str` 的实例。*临时*：载荷是 Rust 字符串；字符层面的一致性随 `CM-13` 的 Unicode 数据补。
    pub struct StrObject {
        /// 内容（UTF-8）。
        value: String,
    }
}

py_object! {
    /// `bytes` 的实例（**不可变**字节串；`P1-12` 第一刀）。
    ///
    /// 载荷是 Rust `Vec<u8>`；`TS-43` 说布局自选，所以这里不进 ABI。
    pub struct BytesObject {
        /// 内容。
        value: Vec<u8>,
    }
}

impl BytesObject {
    /// 内容（**借用**）。
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// 见 [`TupleObject::slots`]：载荷里没有对象引用，故只需释放自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
    }
}

/// `bytes` 的 `repr`：`b'abc'`（引号与转义规则见 [`crate::instance::quote_bytes`]，照参照实测）。
pub unsafe fn bytes_repr(_ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*_ptr.cast::<BytesObject>() };
    Ok(crate::instance::quote_bytes(object.value()))
}

/// `bytes` 的 `str`：与 `repr` **同形**（实测 `str(b'abc') == "b'abc'"`），故接同一个实现。
pub unsafe fn bytes_str(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 契约同 `bytes_repr`。
    unsafe { bytes_repr(ptr, instance) }
}

/// `bytes()`：空、`bytes(<整数>)`（该长度的零字节）、`bytes(<bytes>)`／`bytes(<可迭代的整数>)`、
/// `bytes(<str>, <编码>)`（第一刀只认 UTF-8；其余编码如实报 `LookupError`，消息照实测）。
///
/// 每一条消息都来自 `tests/fixture-bytes-3.14.json`（`tools/gen_bytes_fixture.py` 实测导出）。
pub unsafe fn bytes_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let build = |value: Vec<u8>| {
        instance
            .alloc(BytesObject::new(class, value))
            .into_raw()
            .cast::<Header>()
    };
    match args {
        [] => Ok(build(Vec::new())),
        [only] => {
            if let Some(count) = instance.int_of(*only).and_then(|value| value.to_i64()) {
                if count < 0 {
                    return Err(instance.raise_builtin_error("ValueError", "negative count"));
                }
                let Ok(length) = usize::try_from(count) else {
                    return Err(crate::ExecError::Unsupported {
                        opcode: 0,
                        what: "bytes(计数)：计数超出 usize 的形态还没接线",
                    });
                };
                // 大计数要一大块内存：如实报 `MemoryError`（实测那句消息为空），不做静默截断
                if length > MAX_BYTES_LENGTH {
                    return Err(instance.raise_builtin_error("MemoryError", ""));
                }
                return Ok(build(vec![0u8; length]));
            }
            if let Some(bytes) = instance.bytes_value(*only) {
                return Ok(build(bytes.to_vec()));
            }
            if instance.text_value(*only).is_some() {
                // 实测：`bytes('abc')` ⇒ 少了编码参数
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "string argument without an encoding",
                ));
            }
            if instance.float_value(*only).is_some() {
                // 实测：`bytes(1.5)` ⇒ 这条
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "cannot convert 'float' object to bytes",
                ));
            }
            // 可迭代的整数
            let mut out: Vec<u8> = Vec::new();
            for item in instance.collect_iterable(*only)? {
                let Some(number) = instance.int_of(item).and_then(|value| value.to_i64()) else {
                    let name = instance.type_name(instance.type_of(item));
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        &format!("'{name}' object cannot be interpreted as an integer"),
                    ));
                };
                if !(0..=255).contains(&number) {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "bytes must be in range(0, 256)",
                    ));
                }
                out.push(number as u8);
            }
            Ok(build(out))
        }
        [only, encoding] => {
            let Some(text) = instance.text_value(*only) else {
                let name = instance.type_name(instance.type_of(*only));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("cannot convert '{name}' object to bytes"),
                ));
            };
            let Some(name) = instance.text_value(*encoding) else {
                return Err(instance.raise_builtin_error("TypeError", "encoding must be a string"));
            };
            match name.to_ascii_lowercase().replace('_', "-").as_str() {
                // 第一刀只接 UTF-8：Rust 字符串就是 UTF-8，`.into_bytes()` 即编码结果
                "utf-8" | "utf8" | "u8" => Ok(build(text.into_bytes())),
                other => Err(instance.raise_builtin_error(
                    "LookupError",
                    &format!("unknown encoding: {other}"),
                )),
            }
        }
        _ => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "bytes_new：三个以上实参的形态还没接线",
        }),
    }
}

/// **`bytes` 的方法面**（`P1-12`；按 oracle 逐批）。
///
/// 机制与生成器族同一个（`OM-11` 的 `getattr` 槽）：**现造**一个绑定方法对象交出去。
/// 每一条的行为与消息都来自 `tests/fixture-bytes-3.14.json` 的 `methods` 段（实测导出）。
pub unsafe fn bytes_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "hex" => bytes_hex_native,
        "decode" => bytes_decode_native,
        "startswith" => bytes_startswith_native,
        "endswith" => bytes_endswith_native,
        "find" => bytes_find_native,
        "count" => bytes_count_native,
        "replace" => bytes_replace_native,
        "upper" => bytes_upper_native,
        "lower" => bytes_lower_native,
        "strip" => bytes_strip_native,
        "split" => bytes_split_native,
        "join" => bytes_join_native,
        "rfind" => bytes_rfind_native,
        "index" => bytes_index_native,
        "rindex" => bytes_rindex_native,
        "removeprefix" => bytes_removeprefix_native,
        "removesuffix" => bytes_removesuffix_native,
        "lstrip" => bytes_lstrip_native,
        "rstrip" => bytes_rstrip_native,
        "zfill" => bytes_zfill_native,
        "splitlines" => bytes_splitlines_native,
        "isdigit" => bytes_isdigit_native,
        "isspace" => bytes_isspace_native,
        "__contains__" => bytes_contains_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(method_type, "bytes", Cell::new(handler)));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// 从绑定方法拿 `self` 的**字节载荷**（所有 `bytes` 方法的第一句）。
fn bytes_receiver(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<Vec<u8>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "bytes 的方法需要 self",
        });
    };
    Ok(instance
        .bytes_value(this)
        .map(<[u8]>::to_vec)
        .unwrap_or_default())
}

/// 取一个必须是 `bytes` 的实参；不是就按实测的 `TypeError` 报。
fn bytes_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    index: usize,
) -> Result<Vec<u8>, crate::ExecError> {
    let Some(argument) = args.get(index) else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "这个方法少给了实参（参数个数消息随后补）",
        });
    };
    let Some(value) = instance.bytes_value(*argument) else {
        let name = instance.type_name(instance.type_of(*argument));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("a bytes-like object is required, not '{name}'"),
        ));
    };
    Ok(value.to_vec())
}

/// `bytes.hex()`（实测 `b'abc'.hex() == '616263'`）。
fn bytes_hex_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let text: String = value.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(instance.new_str(&text))
}

/// `bytes.decode(encoding='utf-8')`：第一刀只认 UTF-8；其余编码按实测报 `LookupError`。
fn bytes_decode_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let encoding = match args.first() {
        None => "utf-8".to_owned(),
        Some(argument) => match instance.text_value(*argument) {
            Some(text) => text,
            None => {
                let name = instance.type_name(instance.type_of(*argument));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("decode() argument 'encoding' must be str, not {name}"),
                ));
            }
        },
    };
    match encoding.to_ascii_lowercase().replace('_', "-").as_str() {
        "utf-8" | "utf8" | "u8" => match String::from_utf8(value) {
            Ok(text) => Ok(instance.new_str(&text)),
            Err(_) => Err(instance.raise_builtin_error(
                "UnicodeDecodeError",
                "'utf-8' codec can't decode the given bytes",
            )),
        },
        other => Err(instance.raise_builtin_error(
            "LookupError",
            &format!("unknown encoding: {other}"),
        )),
    }
}

/// `bytes.startswith(prefix)`／`endswith(suffix)`（实测就是前后缀判断）。
fn starts_ends_with(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    ends: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let other = bytes_argument(instance, args, 0)?;
    let matched = if ends {
        value.ends_with(&other)
    } else {
        value.starts_with(&other)
    };
    Ok(instance.retain(instance.singletons().boolean(matched)))
}

fn str_find_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let needle = text_argument(instance, args, 0, "find")?;
    // 参照按**字符**给下标 ⇒ 用 `char_indices` 数出字符序号 ✓（本层口径 ✓）
    let found = text
        .find(&needle)
        .map(|byte| text[..byte].chars().count() as i64)
        .unwrap_or(-1);
    Ok(instance.new_int(found))
}

fn str_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let needle = text_argument(instance, args, 0, "count")?;
    let count = if needle.is_empty() {
        text.chars().count() as i64 + 1
    } else {
        text.matches(needle.as_str()).count() as i64
    };
    Ok(instance.new_int(count))
}

fn str_isdigit_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let found = !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
    Ok(instance.retain(instance.singletons().boolean(found)))
}

fn str_isalpha_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let found = !text.is_empty() && text.chars().all(|c| c.is_alphabetic());
    Ok(instance.retain(instance.singletons().boolean(found)))
}

fn str_zfill_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let width = args
        .first()
        .and_then(|value| instance.int_value(*value))
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "zfill() 要一个整数宽度"))?;
    let current = text.chars().count() as i64;
    if width <= current {
        return Ok(instance.new_str(&text));
    }
    let padded = format!("{}{}", "0".repeat((width - current) as usize), text);
    Ok(instance.new_str(&padded))
}

fn str_splitlines_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let parts: Vec<NonNull<Header>> = text
        .lines()
        .map(|line| instance.new_str(line))
        .collect();
    Ok(instance.new_list(parts))
}

fn str_removeprefix_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let prefix = text_argument(instance, args, 0, "removeprefix")?;
    match text.strip_prefix(prefix.as_str()) {
        Some(rest) => Ok(instance.new_str(rest)),
        None => Ok(instance.new_str(&text)),
    }
}

fn str_removesuffix_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let suffix = text_argument(instance, args, 0, "removesuffix")?;
    match text.strip_suffix(suffix.as_str()) {
        Some(rest) => Ok(instance.new_str(rest)),
        None => Ok(instance.new_str(&text)),
    }
}

/// **按下标插入**：参照里 `insert(i, x)` 的 `i` 会被**夹到 `[0, len]`** ✓（负数表从尾部数 ✓）。
fn list_insert_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "insert() takes exactly 2 arguments",
        ));
    }
    let length = instance.list_items(list).map(|items| items.len()).unwrap_or(0) as i64;
    let mut index = instance.int_value(args[0]).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "insert() 的下标要整数")
    })?;
    if index < 0 {
        index += length;
        if index < 0 {
            index = 0;
        }
    }
    let index = index.min(length) as usize;
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    instance.retain(args[1]);
    object.insert_at(index, args[1]);
    Ok(instance.retain(instance.singletons().none()))
}

/// `index(x)`：找不到 ⇒ `ValueError`（与参照同文 ✓）。
fn list_index_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(wanted) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "index() takes at least 1 argument",
        ));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    match object.position_where(|item| {
        crate::executor::values_equal_public(instance, item, *wanted)
    }) {
        Some(index) => Ok(instance.new_int(index as i64)),
        None => Err(instance.raise_builtin_error("ValueError", " is not in list")),
    }
}

/// `count(x)`：相等元素个数 ✓。
fn list_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(wanted) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "count() takes exactly one argument",
        ));
    };
    let items = instance.list_items(list).unwrap_or_default();
    let count = items
        .into_iter()
        .filter(|item| crate::executor::values_equal_public(instance, *item, *wanted))
        .count() as i64;
    Ok(instance.new_int(count))
}

/// **按值找键的下标** ✓（引擎统一比较口径 ✓）。
fn dict_position(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
) -> Option<usize> {
    let entries = instance.dict_entries(mapping)?;
    entries
        .iter()
        .position(|(candidate, _)| crate::executor::values_equal_public(instance, *candidate, key))
}

/// `update(other)`：逐对并入 ✓（**已有的键替换值** ✓，新键插入 ✓；引用规矩照 `insert_raw` ✓）。
fn dict_update_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let Some(other) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "update expected at most 1 argument, got 0",
        ));
    };
    let pairs = instance
        .dict_entries(*other)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "update() 目前只接字典"))?;
    for (key, value) in pairs {
        match dict_position(instance, mapping, key) {
            Some(index) => {
                // SAFETY: 上面刚确认是本实例的 dict。
                let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                instance.retain(value);
                if let Some(old) = object.set_value_at(index, value) {
                    // 旧值那份引用由本对象持有 ⇒ 归还引擎 ✓
                    unsafe { instance.release_object(old.as_ptr()) };
                }
            }
            None => {
                instance.retain(key);
                instance.retain(value);
                instance.dict_insert_raw(mapping, key, value);
            }
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// `setdefault(key[, default])`：有就返回**已有值** ✓（借来的引用要 `retain` ✓），没有就插入并返回默认 ✓。
fn dict_setdefault_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let Some(key) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "setdefault expected at least 1 argument, got 0",
        ));
    };
    if let Some(index) = dict_position(instance, mapping, *key) {
        let entries = instance.dict_entries(mapping).unwrap_or_default();
        return Ok(instance.retain(entries[index].1));
    }
    let default = match args.get(1) {
        Some(value) => *value,
        None => instance.retain(instance.singletons().none()),
    };
    instance.retain(*key);
    instance.retain(default);
    instance.dict_insert_raw(mapping, *key, default);
    Ok(default)
}

/// `pop(key[, default])`：摘掉一项并返回它的**值** ✓（键那份引用**归还引擎** ✓）。
fn dict_pop_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let Some(key) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "pop expected at least 1 argument, got 0",
        ));
    };
    if let Some(index) = dict_position(instance, mapping, *key) {
        // SAFETY: 上面刚确认是本实例的 dict。
        let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        if let Some((removed_key, value)) = object.remove_at(index) {
            unsafe { instance.release_object(removed_key.as_ptr()) };
            return Ok(value);
        }
    }
    match args.get(1) {
        Some(default) => Ok(instance.retain(*default)),
        None => Err(instance.raise_builtin_error("KeyError", "")),
    }
}

/// `reverse()`：就地反转 ✓（只动顺序，不碰引用 ✓）。
fn list_reverse_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    object.reverse_items();
    Ok(instance.retain(instance.singletons().none()))
}

/// `copy()`（第 154 轮）：**浅拷贝** ✓（`dict_entries` 是**借用** ⇒ 每项 `retain` ✓ —— 第 145 轮的教训 ✓）。
fn dict_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let copy = instance.new_dict();
    for (key, value) in instance.dict_entries(mapping).unwrap_or_default() {
        instance.retain(key);
        instance.retain(value);
        instance.dict_insert_raw(copy, key, value);
    }
    Ok(copy)
}

/// `clear()`（第 154 轮）：逐项摘掉并把两份引用**归还引擎** ✓。
fn dict_clear_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    loop {
        // SAFETY: 绑定的是本实例的 dict。
        let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        match object.remove_at(0) {
            Some((key, value)) => unsafe {
                instance.release_object(key.as_ptr());
                instance.release_object(value.as_ptr());
            },
            None => break,
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// `popitem()`（第 154 轮）：本层按**插入顺序**存 ✓ ⇒ 给**最后**一项 ✓（参照 3.7+ 也是"最后一项" ✓）。
fn dict_popitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let length = instance.dict_entries(mapping).map(|e| e.len()).unwrap_or(0);
    if length == 0 {
        return Err(instance.raise_builtin_error("KeyError", "popitem(): dictionary is empty"));
    }
    // SAFETY: 绑定的是本实例的 dict。
    let object = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    match object.remove_at(length - 1) {
        Some((key, value)) => Ok(instance.new_tuple(vec![key, value])),
        None => Err(instance.raise_builtin_error("KeyError", "popitem(): dictionary is empty")),
    }
}

/// `clear()`（第 154 轮）：逐项弹出并把那份引用**归还引擎** ✓。
fn list_clear_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    // SAFETY: 绑定的是本实例的 list。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    while let Some(item) = object.pop_last() {
        unsafe { instance.release_object(item.as_ptr()) };
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// `lstrip`／`rstrip`（第 154 轮）：**不给参数**时按空白 ✓（带参版随后补 ✗，如实登记 ✓）。
fn str_strip_side_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    from_left: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "带参数的 strip／lstrip／rstrip 尚未接线",
        ));
    }
    let trimmed = if from_left {
        text.trim_start().to_owned()
    } else {
        text.trim_end().to_owned()
    };
    Ok(instance.new_str(&trimmed))
}

fn str_lstrip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_strip_side_native(instance, bound, args, true)
}

fn str_rstrip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_strip_side_native(instance, bound, args, false)
}

/// `title()`（第 154 轮）：每个"词首"大写 ✓、其余**小写** ✓（参照口径 ✓，实测 `"aBc".title()` ⇒ `'Abc'` ✓）。
fn str_title_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut out = String::with_capacity(text.len());
    let mut at_word_start = true;
    for character in text.chars() {
        // 参照把"字母"当词字符 ✓（空白与标点都断开 ✓）
        if character.is_alphabetic() {
            if at_word_start {
                out.extend(character.to_uppercase());
            } else {
                out.extend(character.to_lowercase());
            }
            at_word_start = false;
        } else {
            out.push(character);
            at_word_start = true;
        }
    }
    Ok(instance.new_str(&out))
}

/// `capitalize()`（第 154 轮）：首字符大写 ✓、**其余全部小写** ✓（参照口径 ✓：`"aB"` ⇒ `'Ab'` ✓）。
fn str_capitalize_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let mut characters = text.chars();
    let capitalized = match characters.next() {
        Some(first) => {
            let mut out = String::new();
            out.extend(first.to_uppercase());
            out.extend(characters.flat_map(|character| character.to_lowercase()));
            out
        }
        None => String::new(),
    };
    Ok(instance.new_str(&capitalized))
}

/// `remove(x)`（第 154 轮）：按**值**找到第一项并摘掉 ✓（那份引用**归还引擎** ✓）；找不到报
/// `ValueError: list.remove(x): x not in list` ✓ 同文 ✓。
fn list_remove_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(target) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "remove() takes exactly one argument"));
    };
    // SAFETY: 绑定的是本实例的 list。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    let index = object.position_where(|item| crate::executor::values_equal_public(instance, item, *target));
    match index {
        Some(at) => {
            if let Some(removed) = object.remove_at(at) {
                unsafe { instance.release_object(removed.as_ptr()) };
            }
            Ok(instance.retain(instance.singletons().none()))
        }
        None => Err(instance.raise_builtin_error("ValueError", "list.remove(x): x not in list")),
    }
}

/// **`set` 的方法面**（第 146 轮）：`add`／`discard`／`update`／`copy` ✓ —— 与 `str`／`list`／`dict`
/// 同一套路 ✓（返回绑定的 `MethodObject` ✓）。相等性按 `values_equal`（引擎统一口径 ✓）。
pub unsafe fn set_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "add" => set_add_native,
        "discard" => set_discard_native,
        "update" => set_update_native,
        "copy" => set_copy_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "set",
        Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// 绑定 `self` 的 `set`（方法契约保证有 ✓）。
fn bound_set(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, crate::ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}

/// 集合里有没有与 `item` 相等的元素 ✓（引擎统一比较口径 ✓）。
fn set_contains(
    instance: &Instance,
    set: NonNull<Header>,
    item: NonNull<Header>,
) -> Option<usize> {
    // SAFETY: 调用方保证 set 是本实例的 `set`。
    let object = unsafe { &*set.as_ptr().cast::<SetObject>() };
    object.position_of(|candidate| crate::executor::values_equal_public(instance, candidate, item))
}

fn set_add_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let set = bound_set(instance, bound)?;
    let Some(item) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "add() takes exactly one argument (0 given)",
        ));
    };
    if set_contains(instance, set, *item).is_none() {
        instance.retain(*item);
        instance.set_insert_raw(set, *item);
    }
    Ok(instance.retain(instance.singletons().none()))
}

fn set_discard_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let set = bound_set(instance, bound)?;
    let Some(item) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "discard() takes exactly one argument (0 given)",
        ));
    };
    if let Some(index) = set_contains(instance, set, *item) {
        // SAFETY: 上面刚确认是本实例的 set。
        let object = unsafe { &*set.as_ptr().cast::<SetObject>() };
        if let Some(removed) = object.remove_at(index) {
            // 被移除的那份引用由本对象持有 ⇒ 归还引擎 ✓
            unsafe { instance.release_object(removed.as_ptr()) };
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}

fn set_update_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let set = bound_set(instance, bound)?;
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "update() takes exactly one argument (0 given)",
        ));
    };
    let items = match instance.iterable_items(*iterable) {
        Some(items) => items,
        None => {
            return Err(instance.raise_builtin_error("TypeError", "object is not iterable"))
        }
    };
    for item in items {
        if set_contains(instance, set, item).is_none() {
            instance.retain(item);
            instance.set_insert_raw(set, item);
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}

fn set_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let set = bound_set(instance, bound)?;
    // `set_items` 是**借用** ✓ ⇒ 每项先还一份交给新集合 ✓
    let items: Vec<NonNull<Header>> = instance
        .set_items(set)
        .unwrap_or_default()
        .into_iter()
        .map(|item| instance.retain(item))
        .collect();
    Ok(instance.new_set(items))
}

/// `rjust`／`ljust`／`center`（第 154 轮）：按宽度补空格 ✓（**不接填充字符** ✗，如实登记 ✓）。
fn str_pad_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    mode: u8,
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let width = args
        .first()
        .and_then(|value| instance.int_value(*value))
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "width 要整数"))?;
    let current = text.chars().count() as i64;
    if width <= current {
        return Ok(instance.new_str(&text));
    }
    let pad = (width - current) as usize;
    let padded = match mode {
        0 => format!("{}{}", " ".repeat(pad), text), // rjust
        1 => format!("{}{}", text, " ".repeat(pad)), // ljust
        _ => {
            // center 的**准确规则**照参照 ✓：`left = pad//2 + (pad & width & 1)`（CPython 的
            //   `str.center` 就是这么写的 ✓）—— 实测三个样例都对 ✓：`"a".center(2)` ⇒ `'a '` ✓、
            //   `"ab".center(5)` ⇒ `'  ab '` ✓、`"a".center(5)` ⇒ `'  a  '` ✓。
            //   （我先前先写成"多的一格在左"✗、又改成 ceil ✗，两次都不对 ⇒ 现在照公式 ✓。）
            let left = (pad / 2 + (pad & (width as usize) & 1)) as usize;
            format!("{}{}{}", " ".repeat(left), text, " ".repeat(pad - left))
        }
    };
    Ok(instance.new_str(&padded))
}

fn str_rjust_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_pad_native(instance, bound, args, 0)
}

fn str_ljust_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_pad_native(instance, bound, args, 1)
}

fn str_center_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    str_pad_native(instance, bound, args, 2)
}

/// `partition(sep)`（第 154 轮）：给三元组 ✓（找不到 ⇒ `(自身, "", "")` ✓ 与参照同义 ✓）。
fn str_partition_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let separator = text_argument(instance, args, 0, "partition")?;
    if separator.is_empty() {
        return Err(instance.raise_builtin_error("ValueError", "empty separator"));
    }
    let (head, sep, tail) = match text.find(&separator) {
        Some(at) => (
            text[..at].to_owned(),
            separator.clone(),
            text[at + separator.len()..].to_owned(),
        ),
        None => (text.clone(), String::new(), String::new()),
    };
    let parts = vec![
        instance.new_str(&head),
        instance.new_str(&sep),
        instance.new_str(&tail),
    ];
    Ok(instance.new_tuple(parts))
}

/// `rsplit(sep)`（第 154 轮）：**只接单参** ✓（无参的空白切分随后补 ✗）；`maxsplit` 未接 ✗。
fn str_rsplit_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let separator = text_argument(instance, args, 0, "rsplit")?;
    if separator.is_empty() {
        return Err(instance.raise_builtin_error("ValueError", "empty separator"));
    }
    // **Rust 的 `rsplit` 是逆序产出** ✗ ⇒ 要 `.rev()` 才与参照同序 ✓
    //   （参照里 `"a,b,c".rsplit(",")` 与 `split` **同序** ✓ ⇒ 空格子放最后 ✓；夹具/语料当场抓到 ✓）。
    // **Rust 的 `rsplit` 逆序产出** ✗，且 `&str` 的 `rsplit` **不支持 `.rev()`** ✗（`StrSearcher`
    //   不是双端 ✓）⇒ **先收进 Vec、再 `reverse()`** ✓，这样才与参照同序 ✓。
    let mut collected: Vec<String> = text
        .rsplit(separator.as_str())
        .map(|part| part.to_owned())
        .collect();
    collected.reverse();
    let parts: Vec<NonNull<Header>> = collected
        .iter()
        .map(|part| instance.new_str(part))
        .collect();
    Ok(instance.new_list(parts))
}

/// **`dict` 的方法面**（第 143／145 轮）：`get`／`keys`／`items`／`values` ✓ —— 与 `str`／`list`
/// 同一套路 ✓（返回绑定的 `MethodObject` ✓）。
///
/// **已知偏离**（如实登记 ✓）：`keys`／`items`／`values` 参照返回**视图对象** ✗，本层先返回
/// **列表** ✓（`len`／迭代／`list(...)` 这些常见用法一致 ✓；视图特有的集合运算未接 ✗）。
pub unsafe fn dict_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "get" => dict_get_native,
        "keys" => dict_keys_native,
        "values" => dict_values_native,
        "items" => dict_items_native,
        "update" => dict_update_native,
        "setdefault" => dict_setdefault_native,
        "pop" => dict_pop_native,
        "copy" => dict_copy_native,
        "clear" => dict_clear_native,
        "popitem" => dict_popitem_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "dict",
        Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// 绑定 `self` 的 `dict`（方法契约保证有 ✓）。
fn bound_dict(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, crate::ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}

/// **按键取值**（本层口径 ✓）：`str` 走名字通道 ✓；`int` 走线性比较 ✓（其余键型随后补 ✗）。
fn dict_lookup(
    instance: &Instance,
    mapping: NonNull<Header>,
    key: NonNull<Header>,
) -> Option<NonNull<Header>> {
    if let Some(name) = instance.text_of(key) {
        return instance.dict_get(mapping, name);
    }
    if let Some(wanted) = instance.int_value(key) {
        for (candidate, value) in instance.dict_entries(mapping)? {
            if instance.int_value(candidate) == Some(wanted) {
                return Some(value);
            }
        }
    }
    None
}

fn dict_get_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let Some(key) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "get expected at least 1 argument, got 0",
        ));
    };
    if let Some(found) = dict_lookup(instance, mapping, *key) {
        // **借来的引用要还一份**（第 145 轮：`dict_get`／`dict_entries` 都是**借用** ✓；
        //   不 retain ⇒ 调用方释放后 double free ✗ —— 这一族在第 131 轮的 `type()` 上已经栽过 ✓）。
        return Ok(instance.retain(found));
    }
    match args.get(1) {
        Some(default) => Ok(instance.retain(*default)),
        None => Ok(instance.retain(instance.singletons().none())),
    }
}

fn dict_keys_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let entries = instance
        .dict_entries(mapping)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "not a dict"))?;
    let keys: Vec<NonNull<Header>> = entries
        .into_iter()
        // `entries()` 给的是**借用** ✓，而 `new_list` 会**接管** ⇒ 每项先还一份 ✓
        .map(|(key, _)| instance.retain(key))
        .collect();
    Ok(instance.new_list(keys))
}

fn dict_values_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let entries = instance
        .dict_entries(mapping)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "not a dict"))?;
    let values: Vec<NonNull<Header>> = entries
        .into_iter()
        .map(|(_, value)| instance.retain(value))
        .collect();
    Ok(instance.new_list(values))
}

fn dict_items_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let mapping = bound_dict(instance, bound)?;
    let entries = instance
        .dict_entries(mapping)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "not a dict"))?;
    let pairs: Vec<NonNull<Header>> = entries
        .into_iter()
        .map(|(key, value)| instance.new_tuple(vec![instance.retain(key), instance.retain(value)]))
        .collect();
    Ok(instance.new_list(pairs))
}

/// **`list` 的方法面**（第 143 轮）：照 `str_getattr` 同一套路 ✓（返回绑定的 `MethodObject` ✓）。
pub unsafe fn list_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "append" => list_append_native,
        "extend" => list_extend_native,
        "pop" => list_pop_native,
        "insert" => list_insert_native,
        "index" => list_index_native,
        "count" => list_count_native,
        "reverse" => list_reverse_native,
        "clear" => list_clear_native,
        "remove" => list_remove_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "list",
        Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// 绑定 `self` 的 `list`（方法契约保证有 ✓）。
fn bound_list(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, crate::ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}

fn list_append_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(item) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "append() takes exactly one argument (0 given)",
        ));
    };
    // `append` **接管**一份引用 ⇒ 这里先还一份 ✓（实参是借来的 ✓）
    instance.retain(*item);
    instance.list_append(list, *item);
    Ok(instance.retain(instance.singletons().none()))
}

fn list_extend_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "extend() takes exactly one argument (0 given)",
        ));
    };
    let items = match instance.iterable_items(*iterable) {
        Some(items) => items,
        None => {
            return Err(instance.raise_builtin_error("TypeError", "object is not iterable"))
        }
    };
    for item in items {
        instance.retain(item);
        instance.list_append(list, item);
    }
    Ok(instance.retain(instance.singletons().none()))
}

fn list_pop_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let list = bound_list(instance, bound)?;
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "pop() 目前只接无参（带下标随后补）",
        ));
    }
    // SAFETY: 绑定的是本类型的存活对象。
    let object = unsafe { &*list.as_ptr().cast::<ListObject>() };
    match object.pop_last() {
        Some(item) => Ok(item),
        None => Err(instance.raise_builtin_error("IndexError", "pop from empty list")),
    }
}

/// **`slice` 的属性面**（第 151 轮）：`start`／`stop`／`step` ✓（省略的那段给 `None` ✓ —— 与参照
/// 同义 ✓）。注册在 `slice` 类型的 `getattr` 槽上 ✓（**一处真相** ✓）。
pub unsafe fn slice_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<SliceObject>() };
    let value = match name {
        "start" => object.start(),
        "stop" => object.stop(),
        "step" => object.step(),
        _ => return None,
    };
    Some(match value {
        Some(number) => instance.new_int(number),
        None => instance.retain(instance.singletons().none()),
    })
}

/// **`str` 的方法面**（第 143 轮）：照 `bytes_getattr` 的同一套路 ✓（返回**绑定**的
/// `builtin_function_or_method` ✓，`self` 就是那个字符串 ✓）。
pub unsafe fn str_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "upper" => str_upper_native,
        "lower" => str_lower_native,
        "strip" => str_strip_native,
        "startswith" => str_startswith_native,
        "endswith" => str_endswith_native,
        "join" => str_join_native,
        "split" => str_split_native,
        "replace" => str_replace_native,
        "find" => str_find_native,
        "count" => str_count_native,
        "isdigit" => str_isdigit_native,
        "isalpha" => str_isalpha_native,
        "zfill" => str_zfill_native,
        "splitlines" => str_splitlines_native,
        "removeprefix" => str_removeprefix_native,
        "removesuffix" => str_removesuffix_native,
        "rjust" => str_rjust_native,
        "ljust" => str_ljust_native,
        "center" => str_center_native,
        "partition" => str_partition_native,
        "rsplit" => str_rsplit_native,
        "lstrip" => str_lstrip_native,
        "rstrip" => str_rstrip_native,
        "title" => str_title_native,
        "capitalize" => str_capitalize_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "str",
        Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// 取绑定 `self` 的字符串（方法契约保证有 ✓）。
fn bound_text(instance: &Instance, bound: Option<NonNull<Header>>) -> Result<String, crate::ExecError> {
    let Some(bound) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor needs an argument"));
    };
    // SAFETY: 绑定的是本类型的存活对象。
    Ok(unsafe { &*bound.as_ptr().cast::<StrObject>() }.value().to_owned())
}

/// 取一个**字符串实参**（不是 str ⇒ 与参照同形的 `TypeError` ✓）。
fn text_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    index: usize,
    what: &str,
) -> Result<String, crate::ExecError> {
    let Some(value) = args.get(index) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{what}() takes at least {} argument", index + 1),
        ));
    };
    match instance.text_of(*value) {
        Some(text) => Ok(text.to_owned()),
        None => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{what}(): expected str"),
        )),
    }
}

fn str_upper_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_str(&text.to_uppercase()))
}

fn str_lower_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_str(&text.to_lowercase()))
}

fn str_strip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    Ok(instance.new_str(text.trim()))
}

fn str_startswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let prefix = text_argument(instance, args, 0, "startswith")?;
    let found = text.starts_with(&prefix);
    Ok(instance.retain(instance.singletons().boolean(found)))
}

fn str_endswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let suffix = text_argument(instance, args, 0, "endswith")?;
    let found = text.ends_with(&suffix);
    Ok(instance.retain(instance.singletons().boolean(found)))
}

fn str_join_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let separator = bound_text(instance, bound)?;
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error("TypeError", "join() takes exactly one argument"));
    };
    let items = match instance.iterable_items(*iterable) {
        Some(items) => items,
        None => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "can only join an iterable",
            ))
        }
    };
    let mut parts: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        match instance.text_of(item) {
            Some(text) => parts.push(text.to_owned()),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "sequence item: expected str instance",
                ))
            }
        }
    }
    Ok(instance.new_str(&parts.join(&separator)))
}

fn str_split_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    // 无参 ⇒ 与参照同义的"按空白切、丢弃空段" ✓；有参 ⇒ 按该分隔符切 ✓
    let parts: Vec<NonNull<Header>> = match args.first() {
        Some(separator) => {
            let separator = match instance.text_of(*separator) {
                Some(text) => text.to_owned(),
                None => {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "must be str or None, not the given type",
                    ))
                }
            };
            text.split(separator.as_str())
                .map(|part| instance.new_str(part))
                .collect()
        }
        None => text
            .split_whitespace()
            .map(|part| instance.new_str(part))
            .collect(),
    };
    Ok(instance.new_list(parts))
}

fn str_replace_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let text = bound_text(instance, bound)?;
    let from = text_argument(instance, args, 0, "replace")?;
    let to = text_argument(instance, args, 1, "replace")?;
    Ok(instance.new_str(&text.replace(&from, &to)))
}

fn bytes_startswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    starts_ends_with(instance, bound, args, false)
}

fn bytes_endswith_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    starts_ends_with(instance, bound, args, true)
}

/// `bytes.find(sub)`：找到给下标、找不到给 `-1`（实测）。
fn bytes_find_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    let found = if needle.is_empty() {
        Some(0)
    } else {
        value
            .windows(needle.len())
            .position(|window| window == needle.as_slice())
    };
    Ok(instance.new_int(found.map_or(-1, |position| position as i64)))
}

/// `bytes.count(sub)`：**不重叠**计数（实测 `b'aaa'.count(b'aa') == 1`）。
fn bytes_count_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    if needle.is_empty() {
        return Ok(instance.new_int(value.len() as i64 + 1));
    }
    let mut count = 0i64;
    let mut at = 0usize;
    while at + needle.len() <= value.len() {
        if value[at..at + needle.len()] == needle[..] {
            count += 1;
            at += needle.len();
        } else {
            at += 1;
        }
    }
    Ok(instance.new_int(count))
}

/// `bytes.replace(old, new)`：全部替换。
fn bytes_replace_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let old = bytes_argument(instance, args, 0)?;
    let new = bytes_argument(instance, args, 1)?;
    if old.is_empty() {
        // 实测：`b'abc'.replace(b'', b'x') == b'xaxbxcx'`（每字节之间插一遍，两端也插）
        let mut out: Vec<u8> = Vec::with_capacity(value.len() * (new.len() + 1) + new.len());
        out.extend_from_slice(&new);
        for byte in &value {
            out.push(*byte);
            out.extend_from_slice(&new);
        }
        return Ok(instance.new_bytes(&out));
    }
    let mut out: Vec<u8> = Vec::with_capacity(value.len());
    let mut at = 0usize;
    while at < value.len() {
        if at + old.len() <= value.len() && value[at..at + old.len()] == old[..] {
            out.extend_from_slice(&new);
            at += old.len();
        } else {
            out.push(value[at]);
            at += 1;
        }
    }
    Ok(instance.new_bytes(&out))
}

/// `bytes.upper()`／`lower()`：**只动 ASCII 字母**（实测）。
fn bytes_case_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    upper: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let mapped: Vec<u8> = value
        .into_iter()
        .map(|byte| if upper { byte.to_ascii_uppercase() } else { byte.to_ascii_lowercase() })
        .collect();
    Ok(instance.new_bytes(&mapped))
}

fn bytes_upper_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_case_native(instance, bound, true)
}

fn bytes_lower_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_case_native(instance, bound, false)
}

/// `bytes.strip()`：去掉两端的 **ASCII 空白**（实测 `b'  ab  '.strip() == b'ab'`）。
fn bytes_strip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    // 不带实参 ⇒ 去 ASCII 空白；带实参 ⇒ 那个**字节集合**（实测 `b'  ab  '.strip(b'a')`
    // 原样返回——空白不在集合里）
    let cut: Option<Vec<u8>> = match args.first() {
        None => None,
        Some(_) => Some(bytes_argument(instance, args, 0)?),
    };
    let is_cut = |byte: u8| match &cut {
        None => byte.is_ascii_whitespace(),
        Some(set) => set.contains(&byte),
    };
    let start = value.iter().position(|byte| !is_cut(*byte)).unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|byte| !is_cut(*byte))
        .map_or(start, |position| position + 1);
    Ok(instance.new_bytes(&value[start..end]))
}

/// `bytes.split(sep)`：按分隔符切开，给 `list[bytes]`。
fn bytes_split_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let separator = bytes_argument(instance, args, 0)?;
    if separator.is_empty() {
        // 实测：`b'abc'.split(b'')` ⇒ `ValueError: empty separator`
        return Err(instance.raise_builtin_error("ValueError", "empty separator"));
    }
    let mut parts: Vec<NonNull<Header>> = Vec::new();
    let mut start = 0usize;
    let mut at = 0usize;
    while at + separator.len() <= value.len() {
        if value[at..at + separator.len()] == separator[..] {
            parts.push(instance.new_bytes(&value[start..at]));
            at += separator.len();
            start = at;
        } else {
            at += 1;
        }
    }
    parts.push(instance.new_bytes(&value[start..]));
    Ok(instance.new_list(parts))
}

/// `bytes.join(iterable)`：把一串 `bytes` 用自己接起来。
fn bytes_join_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let separator = bytes_receiver(instance, bound)?;
    let Some(iterable) = args.first() else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "bytes.join 少给了实参",
        });
    };
    let items = instance.collect_iterable(*iterable)?;
    let mut out: Vec<u8> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.extend_from_slice(&separator);
        }
        let Some(value) = instance.bytes_value(*item) else {
            let name = instance.type_name(instance.type_of(*item));
            // 实测：`b','.join([1])` ⇒ `sequence item 0: expected a bytes-like object, int found`
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("sequence item {index}: expected a bytes-like object, {name} found"),
            ));
        };
        out.extend_from_slice(value);
    }
    Ok(instance.new_bytes(&out))
}

py_object! {
    /// `slice` 的实例（`P1-12` 点名的"索引／切片"要它）。
    ///
    /// **第一刀只接线整数与 `None`**：参照允许任意对象（靠 `__index__`），那要属性通道，
    /// 随后补——非整数字段如实报实测的那条 `TypeError`，不猜。
    pub struct SliceObject {
        /// `start`（`None` ＝ 省略）。
        start: Option<i64>,
        /// `stop`。
        stop: Option<i64>,
        /// `step`。
        step: Option<i64>,
    }
}

impl SliceObject {
    /// **三段访问器**（第 151 轮，`slice.start`／`stop`／`step` 用 ✓）。
    pub fn start(&self) -> Option<i64> {
        self.start
    }
    pub fn stop(&self) -> Option<i64> {
        self.stop
    }
    pub fn step(&self) -> Option<i64> {
        self.step
    }
    /// 见 [`TupleObject::slots`]：载荷是三个 `Option<i64>`，不持有对象引用。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
    }
}

/// `slice` 的 `repr`：`slice(1, 2, 3)`／`slice(None, None, None)`（实测）。
pub unsafe fn slice_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<SliceObject>() };
    let show = |value: Option<i64>| match value {
        Some(number) => number.to_string(),
        None => "None".to_owned(),
    };
    Ok(format!(
        "slice({}, {}, {})",
        show(object.start),
        show(object.stop),
        show(object.step)
    ))
}

/// `slice(...)`：`slice(stop)`／`slice(start, stop[, step])`（实测的三种形态）。
pub unsafe fn classmethod_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(function) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "classmethod expected 1 argument, got 0",
        ));
    };
    instance.retain(*function);
    let ty = instance
        .type_named("classmethod")
        .expect("引导期已登记 classmethod 类型");
    // **走宏生成的 `new`** ✓（它接收字段作参数 ✓）：这样既符合规范 ✓，也消掉「never used」警告 ✓
    //（第 158 轮的教训 ✓：直接写字面量会绕过它 ✗）。
    Ok(instance
        .alloc_payload(ClassMethodObject::new(ty, *function))
        .cast::<Header>())
}

pub unsafe fn slice_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let field = |argument: &NonNull<Header>| -> Result<Option<i64>, crate::ExecError> {
        if Some(instance.type_of(*argument)) == instance.type_named("NoneType") {
            return Ok(None);
        }
        match instance.int_of(*argument).and_then(|value| value.to_i64()) {
            Some(value) => Ok(Some(value)),
            None => Err(instance.raise_builtin_error(
                "TypeError",
                // 实测：`b'abc'[slice("a")]` ⇒ 这条
                "slice indices must be integers or None or have an __index__ method",
            )),
        }
    };
    let (start, stop, step) = match args {
        [] => {
            return Err(crate::ExecError::Unsupported {
                opcode: 0,
                what: "slice() 至少要一个实参",
            })
        }
        [stop] => (None, field(stop)?, None),
        [start, stop] => (field(start)?, field(stop)?, None),
        [start, stop, step] => (field(start)?, field(stop)?, field(step)?),
        _ => {
            return Err(crate::ExecError::Unsupported {
                opcode: 0,
                what: "slice() 最多三个实参",
            })
        }
    };
    // 步长为 0：实测在**切片求值**时报 `ValueError: slice step cannot be zero`
    // （`slice(1, 2, 0)` 本身可以构造）⇒ 这条留给 `subscript_get` 的切片路径报
    Ok(instance
        .alloc(SliceObject::new(class, start, stop, step))
        .into_raw()
        .cast::<Header>())
}

/// 找子串的**位置表**（`find`／`rfind` 共用；空针返回 `0`／`len`）。
fn bytes_occurrences(value: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() {
        return vec![0];
    }
    value
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(index, _)| index)
        .collect()
}

/// `bytes.rfind(sub)`：**最后一个**位置，找不到 `-1`。
fn bytes_rfind_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    let found = bytes_occurrences(&value, &needle).last().copied();
    Ok(instance.new_int(found.map_or(-1, |position| position as i64)))
}

/// `bytes.index(sub)`／`rindex(sub)`：与 `find`／`rfind` 同，但找不到报
/// 实测的 `ValueError: subsection not found`。
fn bytes_index_like(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    from_end: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    let occurrences = bytes_occurrences(&value, &needle);
    let found = if from_end { occurrences.last() } else { occurrences.first() };
    match found {
        Some(position) => Ok(instance.new_int(*position as i64)),
        None => Err(instance.raise_builtin_error("ValueError", "subsection not found")),
    }
}

fn bytes_index_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_index_like(instance, bound, args, false)
}

fn bytes_rindex_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_index_like(instance, bound, args, true)
}

/// `bytes.removeprefix(p)`／`removesuffix(s)`（实测：没有该前后缀时**原样返回**）。
fn bytes_remove_affix(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    suffix: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let affix = bytes_argument(instance, args, 0)?;
    let trimmed = if suffix {
        value
            .strip_suffix(affix.as_slice())
            .map(<[u8]>::to_vec)
            .unwrap_or(value)
    } else {
        value
            .strip_prefix(affix.as_slice())
            .map(<[u8]>::to_vec)
            .unwrap_or(value)
    };
    Ok(instance.new_bytes(&trimmed))
}

fn bytes_removeprefix_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_remove_affix(instance, bound, args, false)
}

fn bytes_removesuffix_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_remove_affix(instance, bound, args, true)
}

/// `bytes.lstrip()`／`rstrip()`（与 `strip` 同一套"无实参 ⇒ ASCII 空白，有实参 ⇒ 字节集合"）。
fn bytes_strip_side(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    left: bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let cut: Option<Vec<u8>> = match args.first() {
        None => None,
        Some(_) => Some(bytes_argument(instance, args, 0)?),
    };
    let is_cut = |byte: u8| match &cut {
        None => byte.is_ascii_whitespace(),
        Some(set) => set.contains(&byte),
    };
    let kept = if left {
        let start = value.iter().position(|byte| !is_cut(*byte)).unwrap_or(value.len());
        &value[start..]
    } else {
        let end = value
            .iter()
            .rposition(|byte| !is_cut(*byte))
            .map_or(0, |position| position + 1);
        &value[..end]
    };
    Ok(instance.new_bytes(kept))
}

fn bytes_lstrip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_strip_side(instance, bound, args, true)
}

fn bytes_rstrip_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_strip_side(instance, bound, args, false)
}

/// `bytes.zfill(width)`：左边补 `0`（有符号时符号在最前，实测 `b'-12'.zfill(5) == b'-0012'`）。
fn bytes_zfill_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let Some(width) = args.first().and_then(|arg| instance.int_of(*arg)) else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "bytes.zfill 少给了宽度的实参",
        });
    };
    let Some(width) = width.to_i64().filter(|width| *width > 0) else {
        return Ok(instance.new_bytes(&value));
    };
    let width = width as usize;
    if value.len() >= width {
        return Ok(instance.new_bytes(&value));
    }
    let missing = width - value.len();
    let (sign, digits) = match value.first() {
        Some(b'+') | Some(b'-') => (Some(value[0]), &value[1..]),
        _ => (None, &value[..]),
    };
    let mut out: Vec<u8> = Vec::with_capacity(width);
    if let Some(sign) = sign {
        out.push(sign);
    }
    out.extend(core::iter::repeat_n(b'0', missing));
    out.extend_from_slice(digits);
    Ok(instance.new_bytes(&out))
}

/// `bytes.splitlines()`：按 `\n`／`\r\n`／`\r` 切（不保留行尾）。
fn bytes_splitlines_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let mut parts: Vec<NonNull<Header>> = Vec::new();
    let mut start = 0usize;
    let mut at = 0usize;
    while at < value.len() {
        match value[at] {
            b'\n' => {
                parts.push(instance.new_bytes(&value[start..at]));
                at += 1;
                start = at;
            }
            b'\r' => {
                parts.push(instance.new_bytes(&value[start..at]));
                at += if value.get(at + 1) == Some(&b'\n') { 2 } else { 1 };
                start = at;
            }
            _ => at += 1,
        }
    }
    // 末尾没有换行符时还有一段
    if start < value.len() {
        parts.push(instance.new_bytes(&value[start..]));
    }
    Ok(instance.new_list(parts))
}

/// `bytes.isdigit()`／`isspace()`：**整串非空且全为**对应字符（实测）。
fn bytes_all_are(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    predicate: fn(u8) -> bool,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let matched = !value.is_empty() && value.iter().all(|byte| predicate(*byte));
    Ok(instance.retain(instance.singletons().boolean(matched)))
}

fn bytes_isdigit_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_all_are(instance, bound, |byte| byte.is_ascii_digit())
}

fn bytes_isspace_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    bytes_all_are(instance, bound, |byte| byte.is_ascii_whitespace())
}

/// `bytes.__contains__`（`in`）：子串查找；左操作数不是 bytes 时报实测的消息。
fn bytes_contains_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = bytes_receiver(instance, bound)?;
    let needle = bytes_argument(instance, args, 0)?;
    let matched = if needle.is_empty() {
        true
    } else {
        value.windows(needle.len()).any(|window| window == needle.as_slice())
    };
    Ok(instance.retain(instance.singletons().boolean(matched)))
}

/// **`bytes` 的长度上限**（实现上限，写进规格的"未定"栏）：`1 << 30` ＝ 1 GiB。
///
/// 与位移那处同一个道理：Rust 的分配失败是**中止进程**，不能拿它当错误通道，
/// 所以先自设一条线，超线报实测同款的 `MemoryError`（消息为空）。
const MAX_BYTES_LENGTH: usize = 1 << 30;

impl BuiltinFunctionObject {
    /// 见 [`TupleObject::slots`]：本身不持有对象引用（名字是静态串）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc).with_repr(builtin_function_repr)
    }

    /// 名字。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Rust 实现。
    pub fn function(&self) -> NativeFn {
        self.function.get()
    }
}

/// 原生可调用对象的 `repr`：`<built-in function len>`（实测）。
pub unsafe fn builtin_function_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<BuiltinFunctionObject>() };
    Ok(format!("<built-in function {}>", object.name()))
}

/// **`AB-58`**：定长宿主布局的 `dealloc` 槽——载荷**存储**由 VM 释放。
///
/// 宿主在载荷里自己持有的东西由宿主的 `dealloc`（`OM-14`）放掉，那是**另一个**槽位，
/// 见 `crates/pyawa-abi` 的宿主对象槽实现；这里只负责把 VM 分的那块内存还回去。
///
/// # Safety
///
/// 由 `Instance` 在计数归零、`clear` 跑过之后调用（`OM-20` ③）。
pub unsafe fn free_fixed_layout(ptr: *mut Header) {
    // SAFETY: 调用方保证 ptr 是本实例的定长宿主对象。
    let ty = unsafe { &*ptr }.ty();
    // SAFETY: ty 由注册表持有。
    let size = unsafe { &*ty.as_ptr() }.instance_size;
    let layout = core::alloc::Layout::from_size_align(size, core::mem::align_of::<Header>())
        .expect("宿主载荷尺寸溢出");
    // SAFETY: 这块内存正是 `alloc_host_object` 按同一 layout 分配的，且计数已归零。
    unsafe { std::alloc::dealloc(ptr.cast::<u8>(), layout) };
}

/// **Python 级终结器**（`OM-20` ①／`OM-14`）：按 `TS-44` 走属性通道找 `__del__` 并调用。
///
/// 布局无关：用户类实例（`AttributeObject`）与**宿主类型**及其 Python 子类都用它
/// （`AB-37`／`OM-14`：`tp_dealloc` 得能被 Python 覆写）。
///
/// 覆写里抛出的异常在参照实现里是"**被吞掉并报告**"（`OM-33` 的弱引用那条同理）；
/// 本层暂**吞掉**（报告机制要 `sys.unraisablehook`，随后补——清单里记着）。
pub unsafe fn python_level_finalize(ptr: *mut Header, instance: &Instance) {
    let object = NonNull::new(ptr).expect("调用方保证非空");
    match crate::executor::call_object_method(instance, object, "__del__", &[]) {
        Ok(Some(result)) => {
            // SAFETY: result 是新引用。
            unsafe { instance.release_object(result.as_ptr()) };
        }
        Ok(None) => {}
        Err(crate::ExecError::Raised { exception }) => {
            // 吞掉并记在实例上（`pending_exception`），供宿主随后查看
            let _ = instance.set_pending_exception(Some(exception));
        }
        Err(_) => {}
    }
}

py_object! {
    /// **`classmethod`**（第 158 轮）：包一个可调用对象 ✓（**本对象持有一份引用**）。
    ///
    /// **已接线**：类型对象本身 ＋ `classmethod(f)` 构造 ✓ —— 这样 `Lib/abc.py:28` 的
    /// `class abstractclassmethod(classmethod):` 就能过 ✓（它需要一个**类型**做基类 ✓）。
    /// **未接线** ✗：描述符协议（`__get__` 绑定 `cls` ✓）⇒ 包好的方法还**不能真正绑定** ✓（如实登记 ✓）。
    pub struct ClassMethodObject {
        /// 被包起来的可调用对象（**本对象持有一份引用**）。
        function: NonNull<Header>,
    }
}

impl ClassMethodObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(classmethod_traverse)
            .with_clear(classmethod_clear)
    }

    /// 被包起来的对象（**借用**）。
    pub fn function(&self) -> NonNull<Header> {
        self.function
    }
}

py_object! {
    /// **`_weakref.ref`**（第 173 轮）：给 `Lib/_weakrefset.py` 用的**最小面子** ✓。
    ///
    /// **如实登记的偏差** ✗：本层**没有真正的弱引用**（GC 不支持 ✓）⇒ 这里存的是**强引用** ✓
    /// ⇒ 目标永远不会被回收 ✓（`WeakSet` 因而**不会自动清理** ✓）。对"能把 `abc.py`／`os.py` 跑起来"
    /// 这一步够用 ✓；真正的弱语义留待专门一轮 ✓。
    pub struct WeakRefObject {
        /// 目标对象（**本对象持有一份引用**）。
        target: NonNull<Header>,
    }
}

impl WeakRefObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(weakref_traverse)
            .with_clear(weakref_clear)
            .with_call(weakref_call)
    }

    /// 目标（**借用**）。
    pub fn target(&self) -> NonNull<Header> {
        self.target
    }
}

// **手写** ✓（第 158／160／161 轮的教训 ✓：机械改名会留下错误强转 ✗，而 GC 静态检查**只查结构** ✓）。
unsafe fn weakref_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<WeakRefObject>() };
    visit(object.target().as_ptr());
}

unsafe fn weakref_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<WeakRefObject>() };
    // SAFETY: 这一份引用由本对象持有。
    unsafe { instance.release_object(object.target().as_ptr()) };
}

/// **调用 `ref(x)`** ✓：给回目标本身 ✓（强引用 ⇒ 一定还在 ✓；真正弱语义随后补 ✗）。
unsafe fn weakref_call(
    ptr: *mut Header,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<WeakRefObject>() };
    Ok(instance.retain(object.target()))
}

/// 造一个 `ref`（`_weakref` 模组的 `ref` ✓）：接 1 或 2 个实参 ✓（回调**忽略** ✗，已登记 ✓）。
/// **调用元类型**（第 183 轮重放）：`type(x)` ⇒ 取类型 ✓；无参 ⇒ 参照原话报错 ✓。
///
/// **重要** ✓：调用**一个类**时（`C(...)` ✓），`bound` 是那个**类对象** ✓ ⇒ 这时**不能**走这里 ✗，
/// 得交回**正常的实例化路径** ✓（元类型一旦挂了 call 槽，就会**接管**所有"调用类"的场合 ✓）。
/// `dict.fromkeys(iterable, value=None)`（第 184 轮：**真实实现** ✓，替掉第 182 轮的占位 ✗）。
///
/// 支持的**可迭代对象**：`list`／`tuple`／`set`／`frozenset`／`dict`（取键 ✓）。
/// **尚未接线** ✗：字符串（要字符对象 ✓）、生成器／迭代器（要走迭代协议 ✓）⇒ 如实报未接线 ✓。
// **`safe fn`** ✓（第 184 轮：stdlib 有 `#![forbid(unsafe_code)]` ✗ ⇒ 跨 crate 的面必须是安全的 ✓；
// 它自己的内部照旧用 `unsafe {}` 分块 ✓）。
pub fn dict_fromkeys_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(source) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "fromkeys expected at least 1 argument, got 0",
        ));
    };
    let value = match args.get(1) {
        Some(given) => {
            // SAFETY: given 是存活对象；下面交给字典时要多一份引用 ✓。
            unsafe { instance.incref_object(given.as_ptr()) };
            *given
        }
        None => instance.retain(instance.singletons().none()),
    };
    let source_ty = instance.type_name(instance.type_of(*source));
    let keys: Vec<NonNull<Header>> = match source_ty.as_str() {
        "list" => unsafe { &*source.as_ptr().cast::<ListObject>() }.items().to_vec(),
        "tuple" => unsafe { &*source.as_ptr().cast::<TupleObject>() }.items().to_vec(),
        "set" | "frozenset" => unsafe { &*source.as_ptr().cast::<SetObject>() }.items().to_vec(),
        "dict" => unsafe { &*source.as_ptr().cast::<DictObject>() }
            .entries()
            .into_iter()
            .map(|(key, _)| key)
            .collect::<Vec<NonNull<Header>>>(),
        _ => {
            unsafe { instance.release_object(value.as_ptr()) };
            return Err(crate::ExecError::Unsupported {
                opcode: 0,
                what: "dict.fromkeys：这个可迭代对象的形态随后补",
            });
        }
    };
    let mapping = instance.new_dict();
    for key in keys {
        // SAFETY: key 由源容器持有，存活。
        unsafe { instance.incref_object(key.as_ptr()) };
        instance.dict_insert_raw(mapping, key, value);
    }
    // 每个键都接管了一份 value ✓ ⇒ 这里还掉最初那一份 ✓。
    unsafe { instance.release_object(value.as_ptr()) };
    Ok(mapping)
}

pub unsafe fn type_call(
    _ptr: *mut Header,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    if bound.is_some() {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "type_call：绑定形态（调用类）应交回实例化路径 —— 本槽不该被走到",
        });
    }
    match args.len() {
        1 => Ok(instance.retain(instance.type_of(args[0]).cast())),
        0 => Err(instance.raise_builtin_error(
            "TypeError",
            "cannot create 'type' instances",
        )),
        _ => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "type(name, bases, ns)：三参形态随后补",
        }),
    }
}

pub unsafe fn weakref_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(target) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "ref expected at least 1 argument, got 0",
        ));
    };
    instance.retain(*target);
    let ty = instance
        .type_named("weakref")
        .expect("引导期已登记 weakref 类型");
    Ok(instance
        .alloc_payload(WeakRefObject::new(ty, *target))
        .cast::<Header>())
}

py_object! {
    /// **`property`**（第 161 轮）：与 `classmethod`／`staticmethod` 同一模式 ✓。
    ///
    /// **已接线**：类型对象 ＋ `property(fget)` 构造 ✓（`Lib/abc.py` 的 `class abstractproperty(property)` 要它 ✓）。
    /// **未接线** ✗：`fset`／`fdel`／`doc`（本层只存 `fget` ✓）与**描述符协议**（`__get__` ✓）⇒ 真正当装饰器用还不行 ✓。
    pub struct PropertyObject {
        /// `fget`（**本对象持有一份引用**）。
        fget: NonNull<Header>,
    }
}

impl PropertyObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(property_traverse)
            .with_clear(property_clear)
    }

    /// `fget`（**借用**）。
    pub fn fget(&self) -> NonNull<Header> {
        self.fget
    }
}

// **手写** ✓（第 158／160／161 轮的教训 ✓：机械改名会留下错误强转 ✗，GC 静态检查只查结构 ✓ 查不出 ✓）。
unsafe fn property_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<PropertyObject>() };
    visit(object.fget().as_ptr());
}

unsafe fn property_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<PropertyObject>() };
    // SAFETY: 这一份引用由本对象持有。
    unsafe { instance.release_object(object.fget().as_ptr()) };
}

pub unsafe fn property_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(fget) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "property expected at least 1 argument, got 0",
        ));
    };
    instance.retain(*fget);
    let ty = instance
        .type_named("property")
        .expect("引导期已登记 property 类型");
    Ok(instance
        .alloc_payload(PropertyObject::new(ty, *fget))
        .cast::<Header>())
}

py_object! {
    /// **`staticmethod`**（第 161 轮）：与 `classmethod` 同一模式 ✓（包一个可调用对象 ✓）。
    ///
    /// **已接线**：类型对象 ＋ `staticmethod(f)` 构造 ✓（`Lib/abc.py` 的 `class abstractstaticmethod(staticmethod)` 要它 ✓）。
    /// **未接线** ✗：描述符协议（`__get__` ✓）⇒ 包好的函数还**不能真正绑定** ✓（如实登记 ✓）。
    pub struct StaticMethodObject {
        /// 被包起来的可调用对象（**本对象持有一份引用**）。
        function: NonNull<Header>,
    }
}

impl StaticMethodObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(staticmethod_traverse)
            .with_clear(staticmethod_clear)
    }

    /// 被包起来的对象（**借用**）。
    pub fn function(&self) -> NonNull<Header> {
        self.function
    }
}

// **手写** ✓（第 158／160／161 轮的教训 ✓：机械改名会留下错误强转 ✗，而 GC 静态检查**只查结构** ✓ 查不出 ✓）。
unsafe fn staticmethod_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StaticMethodObject>() };
    visit(object.function().as_ptr());
}

unsafe fn staticmethod_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StaticMethodObject>() };
    // SAFETY: 这一份引用由本对象持有。
    unsafe { instance.release_object(object.function().as_ptr()) };
}

pub unsafe fn staticmethod_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(function) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "staticmethod expected 1 argument, got 0",
        ));
    };
    instance.retain(*function);
    let ty = instance
        .type_named("staticmethod")
        .expect("引导期已登记 staticmethod 类型");
    // **走宏生成的 `new`** ✓（它接收字段作参数 ✓）：这样既符合规范 ✓，也消掉「never used」警告 ✓
    //（第 158 轮的教训 ✓：直接写字面量会绕过它 ✗）。
    Ok(instance
        .alloc_payload(StaticMethodObject::new(ty, *function))
        .cast::<Header>())
}

unsafe fn classmethod_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    // **一律手写** ✓（第 158／160 轮的教训 ✓：机械改名会留下错误强转 ✗，GC 静态检查查不出 ✓）。
    let object = unsafe { &*ptr.cast::<ClassMethodObject>() };
    visit(object.function().as_ptr());
}

unsafe fn classmethod_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ClassMethodObject>() };
    // SAFETY: 这一份引用由本对象持有。
    unsafe { instance.release_object(object.function().as_ptr()) };
}

impl MethodObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(method_traverse)
            .with_clear(method_clear)
    }

    /// 函数（**借用**）。
    pub fn function(&self) -> NonNull<Header> {
        self.function
    }

    /// 绑定的实例（**借用**）。
    pub fn this(&self) -> NonNull<Header> {
        self.this
    }
}

/// `OM-40`：列出绑定方法持有的引用。
unsafe fn method_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    visit(object.function().as_ptr());
    visit(object.this().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出两份引用。
unsafe fn method_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    // SAFETY: 这两份引用由本对象持有。
    unsafe {
        instance.release_object(object.function().as_ptr());
        instance.release_object(object.this().as_ptr());
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
unsafe fn asend_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<AsendObject>() };
    visit(object.generator().as_ptr());
    if let Some(sent) = object.sent() {
        visit(sent.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出 asend 持有的引用。
unsafe fn asend_clear(ptr: *mut Header, instance: &Instance) {
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
unsafe fn asend_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
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

/// `§10` 生成器／协程族的**方法**：`send`／`throw`／`close`（生成器还有 `__next__`）。
///
/// 槽位交出的必须是**绑定方法对象**：`LOAD_ATTR` 在"取方法"形态下会给 `(值, NULL)` 两格
/// （见执行器的 `LOAD_ATTR`），所以已经绑好 self 的方法正好被 `CALL` 按"无 self"调用。
/// **`f.__annotations__`**（PEP 649 的惰性求值 ＋ **缓存**）。
///
/// 实测（3.14.4）：同一函数的 `f.__annotations__` 是**同一对象**，且 `__annotate__` 只被调用
/// **一次**（`format = 1`）；**没有注解**的函数（`__annotate__` 是 `None`）给 `{}`，也照样缓存。
///
/// ⚠ **未接**：参照里这个属性**可写**（赋值会换掉注解），本层是只读的计算属性。
pub fn function_annotations(
    instance: &Instance,
    ptr: *mut Header,
) -> Result<NonNull<Header>, crate::ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    if let Some(cached) = object.annotations_cache() {
        // SAFETY: 缓存由本对象持有，调用方要自己那份。
        unsafe { instance.incref_object(cached.as_ptr()) };
        return Ok(cached);
    }
    let computed = match object.annotate() {
        Some(callable) => {
            let format = instance.new_int(1);
            // `call_value` 只**借用**实参 ⇒ 用完归还
            let result = crate::executor::call_value(instance, callable, &[format], &[]);
            // SAFETY: format 是上面刚造的那份引用。
            unsafe { instance.release_object(format.as_ptr()) };
            result?
        }
        // 没有注解 ⇒ `{}`（实测 `plain.__annotations__ == {}`）
        None => instance.new_dict(),
    };
    // 缓存自己持一份
    // SAFETY: computed 是新引用，存活。
    unsafe { instance.incref_object(computed.as_ptr()) };
    if let Some(old) = object.set_annotations_cache(Some(computed)) {
        // SAFETY: old 是旧缓存持有的那份。
        unsafe { instance.release_object(old.as_ptr()) };
    }
    Ok(computed)
}

/// **函数对象**的属性通道（`OM-11` 的 `getattr` 槽）。
///
/// 暴露的名字与**语义**照参照实测（3.14.4）：
/// `__name__`／`__qualname__`／`__code__`／`__defaults__`（无默认值 ⇒ `None`）／
/// `__kwdefaults__`（同上）／`__globals__`／`__annotate__`（PEP 649；**无注解 ⇒ `None`**）。
///
/// **未接**：`__doc__`（本层不解析文档字符串 ⇒ 一律 `None`，与"没有文档字符串"的情形一致）、
/// `__annotations__`（要调用 `__annotate__` 并**缓存**，另一笔——实测它的 `is` 稳定）。
/// 返回值一律**新引用**（调用方按 `OM-16` 接手）。
pub unsafe fn function_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    match name {
        "__name__" | "__qualname__" => {
            let code = object.code();
            // SAFETY: code 由函数持有，存活。
            let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
            let text = if name == "__name__" {
                code.name()
            } else {
                code.qualname()
            };
            Some(instance.new_str(text))
        }
        "__doc__" => {
            let code = object.code();
            // SAFETY: code 由函数持有，存活。
            let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
            // **实测**：只有 `co_flags` 的 `0x4000000`（"有文档串"）置位时，常量 0 才是文档串
            // ——`def f(): return "x"` 的常量 0 是 `'x'`，但 `f.__doc__` 是 `None`。
            if code.flags() & 0x400_0000 == 0 {
                return Some(instance.retain(instance.singletons().none()));
            }
            match code.constant(0) {
                Some(value) => {
                    // SAFETY: 值由 code 持有，调用方要自己那份。
                    unsafe { instance.incref_object(value.as_ptr()) };
                    Some(value)
                }
                None => Some(instance.retain(instance.singletons().none())),
            }
        }
        "__code__" => {
            // 新增一份（调用方接手）
            // SAFETY: code 由函数持有，存活。
            unsafe { instance.incref_object(object.code().as_ptr()) };
            Some(object.code())
        }
        "__closure__" => {
            // **闭包**（第 183 轮）：`Lib/types.py` 要 `f.__closure__[0]` ✓ —— `CellObject` 本来就有 ✓。
            // 没有闭包时参照给 `None` ✓；有闭包给**元组** ✓（元组持每个单元一份新引用 ✓）。
            let cells = object.closure();
            if cells.is_empty() {
                return Some(instance.retain(instance.singletons().none()));
            }
            let items: Vec<NonNull<Header>> = cells
                .iter()
                .map(|cell| {
                    // SAFETY: cell 由函数持有，存活；这里再取一份交给元组。
                    unsafe { instance.incref_object(cell.as_ptr()) };
                    *cell
                })
                .collect();
            Some(instance.new_tuple(items))
        }
        "__defaults__" => {
            let defaults = object.defaults();
            if defaults.is_empty() {
                return Some(instance.retain(instance.singletons().none()));
            }
            let mut items = Vec::with_capacity(defaults.len());
            for default in defaults {
                // SAFETY: 默认值由函数持有，存活。
                unsafe { instance.incref_object(default.as_ptr()) };
                items.push(*default);
            }
            Some(instance.new_tuple(items))
        }
        "__kwdefaults__" => match object.kwdefaults() {
            Some(mapping) => {
                // SAFETY: mapping 由函数持有，存活。
                unsafe { instance.incref_object(mapping.as_ptr()) };
                Some(mapping)
            }
            None => Some(instance.retain(instance.singletons().none())),
        },
        "__globals__" => match object.globals() {
            Some(mapping) => {
                // SAFETY: mapping 由函数持有，存活。
                unsafe { instance.incref_object(mapping.as_ptr()) };
                Some(mapping)
            }
            None => Some(instance.retain(instance.singletons().none())),
        },
        "__annotate__" => match object.annotate() {
            Some(callable) => {
                // SAFETY: callable 由函数持有，存活。
                unsafe { instance.incref_object(callable.as_ptr()) };
                Some(callable)
            }
            // 实测：**没有注解**的函数，`f.__annotate__` 就是 `None`（不是缺属性）
            None => Some(instance.retain(instance.singletons().none())),
        },
        _ => None,
    }
}

pub unsafe fn generator_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // **协程不是迭代器**（实测它没有 `__next__`，也没有 `__iter__`）；生成器两者都有。
    // SAFETY: ptr 是本类型的存活对象（槽位契约）。
    let is_generator = unsafe {
        instance
            .type_name(instance.type_of(NonNull::new_unchecked(ptr)))
            == "generator"
    };
    // SAFETY: ptr 是本类型的存活对象（槽位契约）。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let type_name = instance.type_name(instance.type_of(owner));
    if type_name == "async_generator" {
        match name {
            // `__aiter__()` 返回自己（实测异步生成器就是它自己的 async iterator）
            "__aiter__" => {
                // SAFETY: ptr 由槽位契约保证存活，这里新增一份交给调用方。
                unsafe { instance.incref_object(ptr) };
                return Some(owner);
            }
            // `__anext__()` 交出 awaitable（我们的 `AsendObject`）；`asend(v)` 同形、带值
            "__anext__" | "asend" => {
                let method_type = instance
                    .type_named("builtin_function_or_method")
                    .expect("引导期已登记");
                let handler: NativeFn = if name == "asend" {
                    async_generator_asend_native
                } else {
                    async_generator_anext_native
                };
                let native = instance.alloc(BuiltinFunctionObject::new(
                    method_type,
                    "async_generator",
                    Cell::new(handler),
                ));
                let native_raw = native.into_raw().cast::<Header>();
                // SAFETY: ptr 由槽位契约保证存活，方法对象要自己那份 self。
                unsafe { instance.incref_object(ptr) };
                let bound = instance.alloc(MethodObject::new(
                    instance.type_named("method").expect("method 已登记"),
                    native_raw,
                    owner,
                ));
                return Some(bound.into_raw().cast::<Header>());
            }
            _ => {}
        }
    }
    let handler: NativeFn = match name {
        "send" => generator_send_native,
        "__next__" if is_generator => generator_next_native,
        "throw" => generator_throw_native,
        "close" => generator_close_native,
        _ => return None,
    };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    // 方法对象要自己持有函数与 self 各一份引用
    let native = instance.alloc(BuiltinFunctionObject::new(method_type, "generator", Cell::new(handler)));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: ptr 是本类型的存活对象（槽位契约）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        // SAFETY: ptr 由槽位契约保证非空（是本类型的存活对象）。
        unsafe { NonNull::new_unchecked(ptr) },
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// `async_generator.__anext__()`：交出一个 awaitable（`AsendObject`）。
unsafe fn async_generator_anext_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("__anext__ 是绑定方法，必须有 self");
    make_asend(instance, generator, None)
}

/// `async_generator.asend(value)`：同上，但把值带进去。
unsafe fn async_generator_asend_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("asend 是绑定方法，必须有 self");
    make_asend(instance, generator, args.first().copied())
}

/// 造一个 `AsendObject`（**新引用**）。
fn make_asend(
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

/// `send(value)`：把值送进生成器，返回**下一个让出值**；跑完则抛 `StopIteration(返回值)`。
unsafe fn generator_send_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("send 是绑定方法，必须有 self");
    if !kwargs.is_empty() {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "生成器的 send 不接受关键字实参（参照实现同）",
        });
    }
    let sent = args.first().copied();
    // SAFETY: 本函数本身是槽位回调，调用方保证 generator 存活。
    unsafe { resume_with_sent(instance, generator, sent) }
}

/// `__next__()`：等价于 `send(None)`。
unsafe fn generator_next_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("__next__ 是绑定方法，必须有 self");
    if !args.is_empty() || !kwargs.is_empty() {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "生成器的 __next__ 不接受实参",
        });
    }
    // SAFETY: 同上。
    unsafe { resume_with_sent(instance, generator, None) }
}

/// `throw(类型[, 值[, traceback]])`：把异常**抛在挂起点**。
///
/// 实测口径：生成器**已经跑完**或**从未启动**时，异常抛在**调用处**（不进去）；类会自动实例化
/// （`throw(ValueError, 'msg')` ⇒ `ValueError: msg`）；实例再带值 ⇒
/// `TypeError: instance exception may not have a separate value`；实参超过 3 个 ⇒
/// `TypeError: throw expected at most 3 arguments, got N`；不是异常 ⇒ 见
/// [`thrown_exception`] 里那条实测消息。
unsafe fn generator_throw_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("throw 是绑定方法，必须有 self");
    if !kwargs.is_empty() {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "生成器的 throw 不接受关键字实参",
        });
    }
    if args.is_empty() {
        return Err(crate::executor::raise_builtin(
            instance,
            "TypeError",
            "throw expected at least 1 argument, got 0",
        ));
    }
    if args.len() > 3 {
        return Err(crate::executor::raise_builtin(
            instance,
            "TypeError",
            &format!("throw expected at most 3 arguments, got {}", args.len()),
        ));
    }
    let exception = thrown_exception(instance, args[0], args.get(1).copied())?;
    // SAFETY: 槽位契约保证这是本实例里存活的生成器。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    if object.finished() || !object.started() {
        // 还没进去（或已经结束）⇒ 抛在调用处
        return Err(ExecError::Raised { exception });
    }
    match crate::executor::resume_generator_with_raise(instance, generator, exception)? {
        crate::executor::GeneratorOutcome::Yielded(value) => Ok(value),
        crate::executor::GeneratorOutcome::Returned(value) => {
            Err(crate::builtin_objects::stop_iteration(instance, Some(value)))
        }
    }
}

/// `close()`：往生成器里抛 `GeneratorExit`。实测口径——
/// 已经跑完或从未启动 ⇒ `None`（**不跑函数体**）；被关闭时又让出 ⇒
/// `RuntimeError: generator ignored GeneratorExit`；正常收尾/捕获后返回 ⇒ 交回**返回值**
/// （所以 `return 99` 的生成器 `close()` 得 `99`，普通情况是 `None`）；
/// 抛出别的异常 ⇒ 照原样往外抛。
unsafe fn generator_close_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let generator = bound.expect("close 是绑定方法，必须有 self");
    if !args.is_empty() || !kwargs.is_empty() {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "生成器的 close 不接受实参",
        });
    }
    let none = || instance.new_none();
    // SAFETY: 同上。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    if object.finished() || !object.started() {
        object.mark_finished();
        return Ok(none());
    }
    object.mark_finished();
    let exit = exception_instance(instance, "GeneratorExit", Vec::new());
    let outcome = crate::executor::resume_generator_with_raise(instance, generator, exit);
    match outcome {
        Ok(crate::executor::GeneratorOutcome::Yielded(value)) => {
            // 让出的值交回一份引用（异常要抛出去，值用不上了）
            // SAFETY: value 是新引用。
            unsafe { instance.release_object(value.as_ptr()) };
            Err(crate::executor::raise_builtin(
                instance,
                "RuntimeError",
                "generator ignored GeneratorExit",
            ))
        }
        Ok(crate::executor::GeneratorOutcome::Returned(value)) => Ok(value),
        Err(ExecError::Raised { exception }) => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            let generator_exit = instance
                .type_named("GeneratorExit")
                .expect("异常层次在引导期已登记");
            if instance.is_subtype(ty, generator_exit) {
                // SAFETY: 这一份由本函数持有。
                unsafe { instance.release_object(exception.as_ptr()) };
                Ok(none())
            } else if instance.type_name(ty) == "StopIteration" {
                // 生成器内部抛 `StopIteration` ⇒ `close` 交出它的返回值（有就取第一个实参）
                // SAFETY: 类型身份已确认。
                let exception_object = unsafe { &*exception.as_ptr().cast::<ExceptionObject>() };
                let value = exception_object.args().first().copied();
                match value {
                    Some(value) => {
                        // SAFETY: 值由异常对象持有，这里新增一份交给调用方。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        // SAFETY: 异常那份由本函数持有。
                        unsafe { instance.release_object(exception.as_ptr()) };
                        Ok(value)
                    }
                    None => {
                        // SAFETY: 同上。
                        unsafe { instance.release_object(exception.as_ptr()) };
                        Ok(none())
                    }
                }
            } else {
                Err(ExecError::Raised { exception })
            }
        }
        Err(other) => Err(other),
    }
}

/// 把一个不是异常的东西变成异常（`throw` 的入口）：类 ⇒ 实例化（可带值）；
/// 异常实例 ⇒ 直接用（再带值 ⇒ 实测 `TypeError`）；其余 ⇒ 实测 `TypeError`。
fn thrown_exception(
    instance: &Instance,
    value: NonNull<Header>,
    extra: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: value 是存活对象。
    let ty = unsafe { value.as_ref() }.ty();
    let base_exception = instance
        .type_named("BaseException")
        .expect("异常层次在引导期已登记");
    if let Some(class) = instance.as_type(value) {
        if !instance.is_subtype(class, base_exception) {
            return Err(crate::executor::raise_builtin(
                instance,
                "TypeError",
                &format!(
                    "exceptions must be classes or instances deriving from BaseException, not {}",
                    instance.type_name(class)
                ),
            ));
        }
        let mut arguments: Vec<NonNull<Header>> = Vec::new();
        if let Some(extra) = extra {
            // SAFETY: extra 由调用方保证存活。
            unsafe { instance.incref_object(extra.as_ptr()) };
            arguments.push(extra);
        }
        return crate::executor::call_callable(instance, value, None, arguments, Vec::new(), 0);
    }
    if instance.is_subtype(ty, base_exception) {
        if extra.is_some() {
            return Err(crate::executor::raise_builtin(
                instance,
                "TypeError",
                "instance exception may not have a separate value",
            ));
        }
        // SAFETY: value 由调用方保证存活，这里新增一份交给抛出方。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    // SAFETY: value 是存活对象。
    let name = unsafe { ty.as_ref() }.name();
    Err(crate::executor::raise_builtin(
        instance,
        "TypeError",
        &format!("exceptions must be classes or instances deriving from BaseException, not {name}"),
    ))
}

/// 按名字造一个异常实例（不带实参）。
pub(crate) fn exception_instance(
    instance: &Instance,
    class: &str,
    args: Vec<NonNull<Header>>,
) -> NonNull<Header> {
    let ty = instance
        .type_named(class)
        .unwrap_or_else(|| instance.type_named("Exception").expect("异常层次已登记"));
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(args),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
        RefCell::new(None),
    ));
    object.into_raw().cast::<Header>()
}

/// 生成器方法共用的入口：`send` 对"刚创建"的生成器只接受 `None`。
unsafe fn resume_with_sent(
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
                return Err(crate::executor::raise_builtin(
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

/// 造一个 `StopIteration`（可选带返回值——参照实现把它放进 `.value`）。
pub(crate) fn stop_iteration(instance: &Instance, value: Option<NonNull<Header>>) -> ExecError {
    let ty = instance
        .type_named("StopIteration")
        .expect("异常层次在引导期已登记");
    let args: Vec<NonNull<Header>> = match value {
        Some(value) => vec![value],
        None => Vec::new(),
    };
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(args),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
        RefCell::new(None),
    ));
    ExecError::Raised {
        exception: object.into_raw().cast::<Header>(),
    }
}

/// `OM-40`：列出生成器持有的引用。
unsafe fn generator_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    visit(object.frame().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出帧。
unsafe fn generator_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.frame().as_ptr()) };
}

impl ExceptionObject {
    /// 见 [`TupleObject::slots`]：链上可能指回异常自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_getattr(exception_getattr)
            .with_traverse(exception_traverse)
            .with_clear(exception_clear)
    }

    /// 构造实参（**借用**的副本）。
    ///
    /// **读不到就退回空表** ✓（`try_borrow` ✓，第 162／163 轮）：本层有「**可变借用横跨回调**」的
    /// 真 bug ✗（同一个 panic 位点已登记 ✓）⇒ 若这里用 `borrow()`，`Lib/abc.py`／`os.py` 这类
    /// 深一点的导入会**直接 panic** ✗。`args()` 是**只读**语义 ✓ ⇒ 退回空表只是**消息少一段** ✓，
    /// 绝不让整台 VM 崩掉 ✓。**根因仍未修** ✗（继续登记 ✓）。
    pub fn args(&self) -> Vec<NonNull<Header>> {
        self.args
            .try_borrow()
            .map(|slot| slot.clone())
            .unwrap_or_default()
    }

    /// `e.args` 缓存下来的那个 `tuple`（**借用**）。
    pub fn args_tuple(&self) -> Option<NonNull<Header>> {
        *self.args_tuple.borrow()
    }

    /// 记下 `e.args` 的 `tuple`（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_args_tuple(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.args_tuple.replace(value)
    }

    /// 设置构造实参（**新引用**，由本对象接手）。
    pub fn set_args(&self, args: Vec<NonNull<Header>>) {
        *self.args.borrow_mut() = args;
    }

    /// `__cause__`。
    pub fn cause(&self) -> Option<NonNull<Header>> {
        *self.cause.borrow()
    }

    /// 设置 `__cause__`（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_cause(&self, cause: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut *self.cause.borrow_mut(), cause)
    }

    /// `__context__`。
    pub fn context(&self) -> Option<NonNull<Header>> {
        *self.context.borrow()
    }

    /// 设置 `__context__`（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_context(&self, context: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut *self.context.borrow_mut(), context)
    }

    /// `__suppress_context__`。
    pub fn suppress_context(&self) -> bool {
        self.suppress_context.get()
    }

    /// 置 `__suppress_context__`。
    pub fn set_suppress_context(&self, value: bool) {
        self.suppress_context.set(value);
    }

    /// 消息文本：第一个实参是 `str` 时取它的内容（`str(e)` 的最小形态）。
    ///
    /// 需要实例是为了找 `str` 类型——载荷里不存实例指针（`OM-40`：载荷只放裸引用）。
    pub fn message_with(&self, instance: &Instance) -> Option<String> {
        let first = self.args().first().copied()?;
        // SAFETY: first 由本对象持有。
        let ty = unsafe { first.as_ref() }.ty();
        if ty != instance.singletons().str_type() {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*first.as_ptr().cast::<StrObject>() }.value().to_owned())
    }
}

/// `OM-40`：列出异常持有的引用。
/// **`OM-11` 的 `getattr` 槽**（异常实例）：`args`／`__cause__`／`__context__`／
/// `__suppress_context__`／`__traceback__`。
///
/// 形状逐条实测（`tests/exceptions.rs`）：
/// - `e.args` 是 `tuple`，且**按实例缓存**（`e.args is e.args` 为真）
/// - `__cause__`／`__context__` 没有就是 `None`；`__suppress_context__` 是布尔
/// - `__traceback__` 在**未抛**时是 `None`；抛过之后参照实现给 `traceback` 对象，
///   本层还没有 traceback 对象（清单里记着）⇒ 一律 `None`
/// - 其它名字返回 `None`，让调用方继续走类型字典／实例字典，最终报参照实现那句
///   `'X' object has no attribute 'Y'`
///
/// # Safety
///
/// 契约见 `GetAttrFn`：返回**新引用**或 `None`。
pub unsafe fn exception_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    match name {
        "args" => {
            if let Some(cached) = object.args_tuple() {
                // SAFETY: 缓存由本对象持有，存活。
                unsafe { instance.incref_object(cached.as_ptr()) };
                return Some(cached);
            }
            let items: Vec<NonNull<Header>> = object.args();
            for item in &items {
                // SAFETY: 元组要自己那份。
                unsafe { instance.incref_object(item.as_ptr()) };
            }
            let tuple = instance.new_tuple(items);
            if let Some(old) = object.set_args_tuple(Some(tuple)) {
                // SAFETY: 旧缓存由本对象持有。
                unsafe { instance.release_object(old.as_ptr()) };
            }
            Some(tuple)
        }
        "__cause__" => Some(match object.cause() {
            Some(value) => {
                // SAFETY: 由本对象持有。
                unsafe { instance.incref_object(value.as_ptr()) };
                value
            }
            None => instance.new_none(),
        }),
        "__context__" => Some(match object.context() {
            Some(value) => {
                // SAFETY: 同上。
                unsafe { instance.incref_object(value.as_ptr()) };
                value
            }
            None => instance.new_none(),
        }),
        "__suppress_context__" => Some(instance.new_bool(object.suppress_context())),
        // 未抛时参照实现就是 `None`；抛过之后的 `traceback` 对象本层还没有（清单里记着）
        "__traceback__" => Some(instance.new_none()),
        _ => None,
    }
}

unsafe fn exception_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    for value in object.args() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.cause() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.context() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.args_tuple() {
        visit(value.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出异常持有的引用。
unsafe fn exception_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    // **先取出、后释放**（第 148 轮）：可变借用**不能**横跨 `release_object` ✗ —— 释放可能触发
    // 析构／GC，而那条路会**回头共享借用**同一份 `args` ⇒ `RefCell already mutably borrowed` ✗
    // （实测：`try: assert False except AssertionError: pass` 直接崩 ✓）。
    let taken = core::mem::take(&mut *object.args.borrow_mut());
    for value in taken {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_cause(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_context(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_args_tuple(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
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
unsafe fn iterator_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IteratorObject>() };
    visit(object.target().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出被迭代对象。
unsafe fn iterator_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IteratorObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.target().as_ptr()) };
}

impl AttributeObject {
    /// 见 [`TupleObject::slots`]：属性字典里的值可能指回对象自己。
    ///
    /// 终结器按 **`TS-44`** 走**属性通道**找 `__del__`（`OM-20` ①）——用户类的实例走这条。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(attribute_traverse)
            .with_clear(attribute_clear)
            .with_finalize(python_level_finalize)
    }

    /// 属性字典（**借用**；还没建就是 `None`）。
    pub fn attributes(&self) -> Option<NonNull<Header>> {
        *self.attributes.borrow()
    }

    /// 设置属性字典（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_attributes(&self, mapping: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut *self.attributes.borrow_mut(), mapping)
    }
}

/// `OM-40`：列出属性字典。
unsafe fn attribute_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<AttributeObject>() };
    if let Some(mapping) = object.attributes() {
        visit(mapping.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出属性字典。
unsafe fn attribute_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<AttributeObject>() };
    if let Some(mapping) = object.set_attributes(None) {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(mapping.as_ptr()) };
    }
}

impl FunctionObject {
    /// 见 [`TupleObject::slots`]：默认值可能指向别的对象（甚至函数自己）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(function_traverse)
            .with_clear(function_clear)
    }

    /// code object（**借用**的裸引用）。
    pub fn code(&self) -> NonNull<Header> {
        self.code
    }

    /// 位置参数默认值（**借用**）。
    pub fn defaults(&self) -> &[NonNull<Header>] {
        &self.defaults
    }

    /// 设置位置参数默认值（**新引用**，由本对象接手）。
    pub fn set_defaults(&mut self, defaults: Vec<NonNull<Header>>) {
        self.defaults = defaults;
    }

    /// 闭包（**借用**的 cell 列表；建帧时装进自由槽）。
    pub fn closure(&self) -> Vec<NonNull<Header>> {
        self.closure.borrow().clone()
    }

    /// 设置闭包（**新引用**，由本对象接手；返回旧的，调用方负责释放）。
    pub fn set_closure(&self, items: Vec<NonNull<Header>>) -> Vec<NonNull<Header>> {
        core::mem::replace(&mut *self.closure.borrow_mut(), items)
    }

    /// 仅关键字参数默认值（**借用**的 `dict`）。
    pub fn kwdefaults(&self) -> Option<NonNull<Header>> {
        self.kwdefaults
    }

    /// 设置仅关键字默认值（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_kwdefaults(&mut self, kwdefaults: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut self.kwdefaults, kwdefaults)
    }

    /// **`__globals__`**（**借用**；`MAKE_FUNCTION` 之后就有）。
    pub fn globals(&self) -> Option<NonNull<Header>> {
        *self.globals.borrow()
    }

    /// 设置 `__globals__`（**新引用**，由本对象接手；返回被顶下来的旧值）。
    /// `__annotate__`（**借用**；没有就是 `None`）。
    pub fn annotate(&self) -> Option<NonNull<Header>> {
        *self.annotate.borrow()
    }

    /// 换 `__annotate__`，返回旧值（**调用方负责归还**）。
    pub fn set_annotate(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.annotate.replace(value)
    }

    /// `__annotations__` 的缓存（**借用**；没算过就是 `None`）。
    pub fn annotations_cache(&self) -> Option<NonNull<Header>> {
        *self.annotations_cache.borrow()
    }

    /// 换 `__annotations__` 缓存，返回旧值（**调用方负责归还**）。
    pub fn set_annotations_cache(
        &self,
        value: Option<NonNull<Header>>,
    ) -> Option<NonNull<Header>> {
        self.annotations_cache.replace(value)
    }

    pub fn set_globals(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.globals.replace(value)
    }
}

/// `OM-40`：列出函数持有的引用。
unsafe fn function_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    visit(object.code().as_ptr());
    for value in object.defaults() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.kwdefaults() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.globals() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.annotate() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.annotations_cache() {
        visit(value.as_ptr());
    }
    for cell in object.closure() {
        visit(cell.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出函数持有的引用。
unsafe fn function_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &mut *ptr.cast::<FunctionObject>() };
    // SAFETY: 这些引用由本对象持有。
    unsafe { instance.release_object(object.code().as_ptr()) };
    for cell in object.set_closure(Vec::new()) {
        // SAFETY: 同上。
        unsafe { instance.release_object(cell.as_ptr()) };
    }
    for value in core::mem::take(&mut object.defaults) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_kwdefaults(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_globals(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    // `annotate`（`SET_FUNCTION_ATTRIBUTE` 的 bit4）与 `__annotations__` 缓存
    // ——**此前漏在 traverse／clear 之外**，这里补齐（否则 GC 看不到、也不释放）
    if let Some(value) = object.set_annotate(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_annotations_cache(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

py_object! {
    /// **内部哨兵**：`CALL` 的"没有 self"槽位（参照实现在栈上放 `NULL` 指针）。
    ///
    /// *内部*：它**不**进 `TS-41` 的内建类型表，也**禁止**暴露给 Python 代码——
    /// Python 侧看到的永远是 `None`。
    pub struct NullObject {}
}

impl FloatObject {
    /// 数值。
    pub fn value(&self) -> f64 {
        self.value
    }
}

impl StrObject {
    /// 内容。
    pub fn value(&self) -> &str {
        &self.value
    }

    /// 是否为空串（`OM-23` 的空串单例就是它）。
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

// --------------------------------------------------------------------------- #
// 容器（`BC-49` 的容器与解包族；载荷布局按 `TS-43` 由实现自选）
// --------------------------------------------------------------------------- #

py_object! {
    /// `tuple` 的实例：**不可变**——造好之后不再改（载荷只在构造期填）。
    pub struct TupleObject {
        items: Vec<NonNull<Header>>,
    }
}

py_object! {
    /// `list` 的实例。
    pub struct ListObject {
        items: RefCell<Vec<NonNull<Header>>>,
    }


}

py_object! {
    /// `set` 的实例。
    ///
    /// *已知偏离*：本层按**插入顺序**存（关联表），参照实现的迭代顺序由哈希决定。
    /// 集合的文档契约是"无序"，故这一条属于**实现观测面**，落地 `MS-19` 时要如实登记。
    pub struct SetObject {
        items: RefCell<Vec<NonNull<Header>>>,
    }
}

py_object! {
    /// `dict` 的实例。
    ///
    /// *临时*：关联表 ＋ 线性查找（查找走"值相等"而不是 `__hash__`／`__eq__` 槽位——
    /// 那两个槽位随类型系统接线）。**插入顺序**与参照实现一致。
    pub struct DictObject {
        entries: RefCell<Vec<(NonNull<Header>, NonNull<Header>)>>,
    }
}

impl TupleObject {
    /// 注册这个类型时的槽位表（持有引用 ⇒ 要 `traverse`／`clear`，`OM-12`）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(tuple_traverse)
            .with_clear(tuple_clear)
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 按位置取元素（**借用**的裸引用）。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.get(index).copied()
    }

    /// 全部元素（**借用**）。
    pub fn items(&self) -> &[NonNull<Header>] {
        &self.items
    }
}

impl ListObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(list_traverse)
            .with_clear(list_clear)
    }

    /// **按下标摘掉一项**（第 154 轮，`list.remove()` 用 ✓）：那份引用**转交**调用方 ✓。
    pub fn remove_at(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }

    /// **就地反转**（第 147 轮，`list.reverse()` 用 ✓）——只动顺序，**不碰引用** ✓。
    pub fn reverse_items(&self) {
        self.items.borrow_mut().reverse();
    }

    /// **按下标插入**（第 146 轮，`list.insert()` 用 ✓；`index` 越界按参照**夹到两端** ✓）。
    pub fn insert_at(&self, index: usize, item: NonNull<Header>) {
        let mut items = self.items.borrow_mut();
        let at = index.min(items.len());
        items.insert(at, item);
    }

    /// **按下标取**（判等由调用方做 ✓）。
    pub fn position_where(
        &self,
        predicate: impl Fn(NonNull<Header>) -> bool,
    ) -> Option<usize> {
        self.items.borrow().iter().position(|item| predicate(*item))
    }

    /// **弹出末项**（第 143 轮，`list.pop()` 用 ✓）：返回那一项（**那份引用交给调用方** ✓）。
    /// 注意：**必须并进这个 impl** ✗ —— 静态检查 `gc_field_coverage` 只读**该类型的第一个
    /// `impl` 块** ✓；我先前另立一个更靠前的 `impl ListObject` ⇒ 它看不到 `with_traverse`／
    /// `with_clear` ⇒ 判红 ✓（根因就是这 ✓）。
    pub fn pop_last(&self) -> Option<NonNull<Header>> {
        self.items.borrow_mut().pop()
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// 按位置取元素（**借用**）。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.borrow().get(index).copied()
    }

    /// 追加一个**新引用**（引用由本对象接管）。
    pub fn append(&self, value: NonNull<Header>) {
        self.items.borrow_mut().push(value);
    }

    /// 追加一批**新引用**。
    pub fn extend(&self, values: impl IntoIterator<Item = NonNull<Header>>) {
        self.items.borrow_mut().extend(values);
    }

    /// 全部元素（**借用**的副本）。
    pub fn items(&self) -> Vec<NonNull<Header>> {
        self.items.borrow().clone()
    }

    /// 替换第 `index` 项（**新引用**），返回旧值（调用方负责释放）。
    /// 在 `index` 处插入（`a[i:j] = …` 的**切片写**要用；`index` 必须 `<= len`）。
    pub fn insert(&self, index: usize, value: NonNull<Header>) {
        self.items.borrow_mut().insert(index, value);
    }

    pub fn replace(&self, index: usize, value: NonNull<Header>) -> Option<NonNull<Header>> {
        self.items
            .borrow_mut()
            .get_mut(index)
            .map(|slot| core::mem::replace(slot, value))
    }

    /// 删除第 `index` 项，返回被删的那份引用（调用方负责释放）。
    pub fn remove(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }
}

impl SetObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(set_traverse)
            .with_clear(set_clear)
    }

    /// **去掉一个元素**（第 146 轮，`set.discard()` 用 ✓）：找到就交给调用方 ✓（那份引用
    /// **转交**出去 ✓），没找到给 `None` ✓。本层 `set` 按**插入顺序**存 ✓（`SetObject` 的
    /// 既有口径 ✓），相等性由调用方按 `values_equal` 判定后传入**下标** ✓。
    pub fn remove_at(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }

    /// **判等用的线性查找**（本层口径 ✓）：返回第一个与 `wanted` 相等的下标 ✓。
    pub fn position_of(
        &self,
        predicate: impl Fn(NonNull<Header>) -> bool,
    ) -> Option<usize> {
        self.items.borrow().iter().position(|item| predicate(*item))
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// 按位置取元素（**借用**）——顺序是插入顺序，见类型文档的*已知偏离*。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.borrow().get(index).copied()
    }

    /// 全部元素（**借用**的副本）。
    pub fn items(&self) -> Vec<NonNull<Header>> {
        self.items.borrow().clone()
    }

    /// 追加一个**新引用**（不去重；调用方负责先查重）。
    pub fn insert_raw(&self, value: NonNull<Header>) {
        self.items.borrow_mut().push(value);
    }

    /// 删除第 `index` 项，返回被删的那份引用（调用方负责释放）。
    pub fn remove(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }

    /// 是否已含某个元素（按指针）。
    pub fn contains_ptr(&self, value: NonNull<Header>) -> bool {
        self.items.borrow().iter().any(|entry| *entry == value)
    }
}

impl DictObject {
    /// **替换某一项的值**（第 147 轮，`dict.update`／`setdefault` 用 ✓）：返回**旧值** ✓，
    /// 由调用方归还引擎 ✓（本对象不再持有它 ✓）。
    pub fn set_value_at(&self, index: usize, value: NonNull<Header>) -> Option<NonNull<Header>> {
        let mut entries = self.entries.borrow_mut();
        let entry = entries.get_mut(index)?;
        Some(core::mem::replace(&mut entry.1, value))
    }

    /// **摘掉某一项**（第 147 轮，`dict.pop` 用 ✓）：键与值**各一份引用转交**调用方 ✓。
    pub fn remove_at(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        let mut entries = self.entries.borrow_mut();
        if index >= entries.len() {
            return None;
        }
        Some(entries.remove(index))
    }

    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(dict_traverse)
            .with_clear(dict_clear)
    }

    /// 条目数。
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    /// 按位置取条目（**借用**）——顺序是插入顺序。
    pub fn entry(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        self.entries.borrow().get(index).copied()
    }

    /// 全部条目（**借用**的副本）。
    pub fn entries(&self) -> Vec<(NonNull<Header>, NonNull<Header>)> {
        self.entries.borrow().clone()
    }

    /// 找某个键的位置（按指针；值相等的查找在 `executor` 里做）。
    pub fn position_of_ptr(&self, key: NonNull<Header>) -> Option<usize> {
        self.entries
            .borrow()
            .iter()
            .position(|(candidate, _)| *candidate == key)
    }

    /// 追加一个**新引用**的键值对（不查重；调用方先用"值相等"查过）。
    pub fn insert_raw(&self, key: NonNull<Header>, value: NonNull<Header>) {
        self.entries.borrow_mut().push((key, value));
    }

    /// 替换第 `index` 项的值（**新引用**），返回旧值（调用方负责释放）。
    pub fn replace_value(
        &self,
        index: usize,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        self.entries
            .borrow_mut()
            .get_mut(index)
            .map(|slot| core::mem::replace(&mut slot.1, value))
    }

    /// 删除第 `index` 项，返回 `(键, 值)`（调用方负责释放这两份引用）。
    pub fn remove(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        let mut entries = self.entries.borrow_mut();
        if index >= entries.len() {
            return None;
        }
        Some(entries.remove(index))
    }

    /// 写入一个**新引用**的键值对；键已存在时替换值并交出旧值（调用方负责释放）。
    pub fn insert(
        &self,
        key: NonNull<Header>,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        let mut entries = self.entries.borrow_mut();
        if let Some(slot) = entries
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
            .map(|slot| &mut slot.1)
        {
            return Some(core::mem::replace(slot, value));
        }
        entries.push((key, value));
        None
    }
}

/// `OM-40`：列出元组元素。
unsafe fn tuple_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TupleObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出元组元素。
unsafe fn tuple_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TupleObject>() };
    for value in object.items() {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

/// 见 [`tuple_traverse`]。
pub(crate) unsafe fn list_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}

/// 见 [`tuple_clear`]。
pub(crate) unsafe fn list_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    for value in object.items() {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

/// 见 [`tuple_traverse`]。
unsafe fn set_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}

/// 见 [`tuple_clear`]。
unsafe fn set_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    for value in object.items() {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

/// 见 [`tuple_traverse`]（键与值都要列）。
unsafe fn dict_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    for (key, value) in object.entries() {
        visit(key.as_ptr());
        visit(value.as_ptr());
    }
}

/// 见 [`tuple_clear`]（键与值都要交出）。
unsafe fn dict_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    for (key, value) in object.entries() {
        // SAFETY: 这些引用由本对象持有。
        unsafe {
            instance.release_object(key.as_ptr());
            instance.release_object(value.as_ptr());
        }
    }
}

// ---- `OM-11` 的 `new` 槽：类型被调用时的实例化（`T` 的构造）----

/// `object()`：无属性的裸实例。
pub unsafe fn plain_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "object() takes no arguments"));
    }
    Ok(instance.alloc(PlainObject::new(class)).into_raw().cast::<Header>())
}

/// 用户类（载荷是 [`AttributeObject`]）：空实例，字典惰性建立（`OM-14`）。
///
/// **实参不在这里处理**——参照实现里它们归 `__init__`（调用方拿到实例后再调它），
/// 所以这个槽**必须**收下任意实参、不因"有实参"而拒绝。
pub unsafe fn attribute_new(
    class: NonNull<crate::TypeObject>,
    _args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    Ok(
        instance
            .alloc(AttributeObject::new(class, core::cell::RefCell::new(None)))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `int()`：0（零参形态）、整数、十进制串、**浮点**（向零截断）。
pub unsafe fn int_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **实测口径**（`tools/gen_constructors_fixture.py` 的 12 条里那两条 `int`）：
    //   `int('a')` ⇒ `ValueError: invalid literal for int() with base 10: 'a'`
    //   `int([])`  ⇒ `TypeError: int() argument must be a string, a bytes-like object or a real number, not 'list'`
    // 另实测：`' 12 '`／`'+12'`／`'-12'`／`'1_2'` 都接受；`'0x10'`（base 10）与 `'12.5'` 报 `ValueError`。
    // **未接线**：`base` 参数形态、非 ASCII 数字（`int('１２')` 参照**接受** ⇒ 我们不假装报 `ValueError`
    // ✗，而是如实报未实现）。
    // 任意精度本身**已落地**（`P1-11` 第一刀之后：不再有"超出 i64"这一说）。
    match args {
        [] => Ok(instance.new_int(0)),
        [only] => {
            if let Some(value) = instance.int_of(*only) {
                // `int(5)` ⇒ 5；`int(True)` ⇒ 1（`bool` 的载荷就是整数）；大整数原样再交回
                return Ok(instance.new_int_value(value));
            }
            if let Some(number) = instance.float_value(*only) {
                // `int(浮点)`：**向零截断**；`inf`／`nan` 各按参照实测的消息报错
                if number.is_nan() {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        "cannot convert float NaN to integer",
                    ));
                }
                if number.is_infinite() {
                    return Err(instance.raise_builtin_error(
                        "OverflowError",
                        "cannot convert float infinity to integer",
                    ));
                }
                return Ok(instance.new_int_value(IntValue::from_big(
                    crate::bigint::BigInt::from_f64_truncated(number),
                )));
            }
            let Some(text) = instance.text_value(*only) else {
                let name = instance.type_name(instance.type_of(*only));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!(
                        "int() argument must be a string, a bytes-like object or a real number, not '{name}'"
                    ),
                ));
            };
            // **`TS-45` ①**：`str` → `int` 的位数上限（参照实测：**前导零也计入**，
            // 符号与下划线不计；`0` ＝ 不限）。消息带实际位数，照实测原文拼。
            let limit = instance.int_max_str_digits();
            if limit != 0 {
                let digits = text.chars().filter(|character| character.is_ascii_digit()).count();
                if digits > limit as usize {
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        &format!(
                            "Exceeds the limit ({limit} digits) for integer string conversion: \
                             value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
                        ),
                    ));
                }
            }
            match parse_decimal(&text) {
                Decimal::Value(value) => Ok(instance.new_int_value(IntValue::from_big(value))),
                Decimal::NotALiteral => Err(instance.raise_builtin_error(
                    "ValueError",
                    &format!("invalid literal for int() with base 10: '{text}'"),
                )),
                Decimal::NotWired => Err(crate::ExecError::Unsupported {
                    opcode: 0,
                    what: "int_new：非 ASCII 数字／超出 i64 的写法还没接线（TS-45 的任意精度是 P1-11）",
                }),
            }
        }
        _ => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "int_new：`base` 等实参形态还没接线",
        }),
    }
}

/// `int(<字符串>)` 的十进制解析（**只做实测确认过的那一档**；数值本身是任意精度）。
enum Decimal {
    /// 解析成功。
    Value(BigInt),
    /// 参照会报 `ValueError`（非法字面量）。
    NotALiteral,
    /// 参照**接受**但本层没接线（非 ASCII 数字）⇒ 必须如实报未实现，**不许**冒充 `ValueError`。
    NotWired,
}

fn parse_decimal(text: &str) -> Decimal {
    let trimmed = text.trim_matches(|c: char| c.is_ascii_whitespace());
    let (negative, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    if digits.is_empty() {
        return Decimal::NotALiteral;
    }
    let mut cleaned = String::with_capacity(digits.len() + 1);
    if negative {
        cleaned.push('-');
    }
    let mut seen_digit = false;
    for character in digits.chars() {
        if character == '_' {
            continue; // 实测：`'1_2'` 参照接受
        }
        match character.to_digit(10) {
            Some(digit) if character.is_ascii() => {
                seen_digit = true;
                cleaned.push(char::from(b'0' + digit as u8));
            }
            // 非 ASCII 数字（参照接受）⇒ 未接线；真正的非法字符 ⇒ 参照报 ValueError
            Some(_) => return Decimal::NotWired,
            None => return Decimal::NotALiteral,
        }
    }
    if !seen_digit {
        return Decimal::NotALiteral;
    }
    match BigInt::from_decimal(&cleaned) {
        Some(value) => Decimal::Value(value),
        None => Decimal::NotALiteral,
    }
}

/// 取 `bool` **单例**并给调用方一份引用（`OM-23`）。
fn singleton_bool(instance: &Instance, value: bool) -> NonNull<Header> {
    let flag = instance.singletons().boolean(value);
    // SAFETY: 单例由实例持有。
    unsafe { instance.incref_object(flag.as_ptr()) };
    flag
}

/// `bool()`：零参 ⇒ `False`；一个实参 ⇒ 真值；多参 ⇒ 照实测报 `TypeError`。
pub unsafe fn bool_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // 实测：`bool()` ⇒ `False`；`bool(1, 2)` ⇒ `TypeError: bool expected at most 1 argument, got 2`；
    // `bool(x)` ⇒ `x` 的真值 —— 走核心**同一份**真值判定（`truthiness_public`），不另写一套 ✓
    match args {
        [] => Ok(singleton_bool(instance, false)),
        [only] => {
            let truth = crate::executor::truthiness_public(instance, *only, 0)?;
            Ok(singleton_bool(instance, truth))
        }
        _ => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("bool expected at most 1 argument, got {}", args.len()),
        )),
    }
}

/// `float()`：`0.0`；`float(<整数>)`：**正确舍入**到最近的 double（溢出报 `OverflowError`，
/// 消息照实测 `int too large to convert to float`）；`float(<浮点>)`：原值。
///
/// **未接线**：`float('<串>')`（参照会解析十进制／`inf`／`nan`）与多实参形态——都如实报未实现，
/// **不手写**参照的消息（那条消息得先实测，归构造函数的夹具）。
pub unsafe fn float_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let value = match args {
        [] => 0.0,
        [only] => {
            if let Some(integer) = instance.int_of(*only) {
                let wide = integer.to_bigint();
                let number = wide.to_f64();
                if number.is_infinite() && !wide.is_zero() {
                    return Err(instance.raise_builtin_error(
                        "OverflowError",
                        "int too large to convert to float",
                    ));
                }
                number
            } else if let Some(number) = instance.float_value(*only) {
                number
            } else {
                return Err(crate::ExecError::Unsupported {
                    opcode: 0,
                    what: "float_new：这个实参形态还没接线（字符串解析等）",
                });
            }
        }
        _ => {
            return Err(crate::ExecError::Unsupported {
                opcode: 0,
                what: "float_new：多实参形态还没接线",
            })
        }
    };
    Ok(
        instance
            .alloc(FloatObject::new(class, value))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `list()`：空列表。
pub unsafe fn list_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **`list(...)`**（第 184 轮：替掉"只接无参"的形态 ✗ —— `list` 这个名字改成**类型对象**之后，
    // `list(可迭代)` 就走到这里了 ✓）。
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let mut borrowed = true;
    if let Some(source) = args.first() {
        let source_ty = instance.type_name(instance.type_of(*source));
        match source_ty.as_str() {
            "list" => items = unsafe { &*source.as_ptr().cast::<ListObject>() }.items().to_vec(),
            "tuple" => items = unsafe { &*source.as_ptr().cast::<TupleObject>() }.items().to_vec(),
            "set" | "frozenset" => {
                items = unsafe { &*source.as_ptr().cast::<SetObject>() }.items().to_vec()
            }
            "dict" => {
                items = unsafe { &*source.as_ptr().cast::<DictObject>() }
                    .entries()
                    .into_iter()
                    .map(|(key, _)| key)
                    .collect()
            }
            _ => {
                // **任何可迭代对象** ✓：走**一处真相**的 `iter_object` ＋ `advance_iterator` ✓
                // （`list(迭代器)`／`list(range(…))` 等全靠它 ✓）；这条路给的是**新引用** ✓。
                borrowed = false;
                let iterator = instance.iter_object(*source)?;
                loop {
                    match instance.advance_iterator(iterator)? {
                        Some(item) => items.push(item),
                        None => break,
                    }
                }
                unsafe { instance.release_object(iterator.as_ptr()) };
            }
        }
    }
    if borrowed {
        // 容器那几条支路给的是**借用** ⇒ 逐个取一份新引用交给新列表 ✓。
        for item in &items {
            unsafe { instance.incref_object(item.as_ptr()) };
        }
    }
    Ok(
        instance
            .alloc(ListObject::new(class, core::cell::RefCell::new(items)))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `dict()`：空字典。
pub unsafe fn dict_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    if !args.is_empty() {
        return Err(crate::ExecError::Unsupported { opcode: 0, what: "dict_new：这个实参形态还没接线" });
    }
    Ok(
        instance
            .alloc(DictObject::new(class, core::cell::RefCell::new(Vec::new())))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `set()`：空集合。
pub unsafe fn set_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    if !args.is_empty() {
        return Err(crate::ExecError::Unsupported { opcode: 0, what: "set_new：这个实参形态还没接线" });
    }
    Ok(
        instance
            .alloc(SetObject::new(class, core::cell::RefCell::new(Vec::new())))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `tuple()`：空元组。
pub unsafe fn tuple_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let _ = class;
    // **`tuple()`**：**OM-23** 要求给**空元组单例**（`tuple() is ()` 必须为真 ✓）。
    let Some(source) = args.first() else {
        return Ok(instance.new_tuple(Vec::new()));
    };
    // **`tuple(可迭代)`**（第 184 轮，与 `list_new` 同款 ✓）：容器走快路 ✓，其余走
    // **`iter_object` ＋ `advance_iterator`**（**一处真相** ✓）。
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let mut borrowed = true;
    let source_ty = instance.type_name(instance.type_of(*source));
    match source_ty.as_str() {
        "list" => items = unsafe { &*source.as_ptr().cast::<ListObject>() }.items().to_vec(),
        "tuple" => items = unsafe { &*source.as_ptr().cast::<TupleObject>() }.items().to_vec(),
        "set" | "frozenset" => {
            items = unsafe { &*source.as_ptr().cast::<SetObject>() }.items().to_vec()
        }
        "dict" => {
            items = unsafe { &*source.as_ptr().cast::<DictObject>() }
                .entries()
                .into_iter()
                .map(|(key, _)| key)
                .collect()
        }
        _ => {
            borrowed = false;
            let iterator = instance.iter_object(*source)?;
            loop {
                match instance.advance_iterator(iterator)? {
                    Some(item) => items.push(item),
                    None => break,
                }
            }
            unsafe { instance.release_object(iterator.as_ptr()) };
        }
    }
    if borrowed {
        for item in &items {
            unsafe { instance.incref_object(item.as_ptr()) };
        }
    }
    Ok(instance.new_tuple(items))
}

/// `str()`：空串（走 `OM-23` 的单例）。
pub unsafe fn str_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **`str(x)` 的参照口径**（第 185 轮实测 ✓）：`str()` ⇒ `''` ✓；本来就是 `str` ⇒ **原样给回** ✓（并 `retain` ✓）；
    // 其余走 `str()` 那一套 ✓（`str([1, 2])` ⇒ `'[1, 2]'` ✓、`str(123)` ⇒ `'123'` ✓、`str(None)` ⇒ `'None'` ✓）。
    let Some(value) = args.first() else {
        return Ok(instance.new_str(""));
    };
    if instance.type_of(*value) == instance.singletons().str_type() {
        return Ok(instance.retain(*value));
    }
    let text = instance.object_repr(*value)?;
    Ok(instance.new_str(&text))
}

/// 异常类：`ValueError("x")` —— **实参进 `args`**（借用视图，这里自己 incref）。
///
/// 这是 `raise ValueError("x")` 能跑通的那一半：编译器发的是"调用类 ＋ `RAISE_VARARGS 1`"。
pub unsafe fn exception_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let mut stored: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        stored.push(*argument);
    }
    let object = instance.alloc(ExceptionObject::new(
        class,
        core::cell::RefCell::new(stored),
        core::cell::RefCell::new(None),
        core::cell::RefCell::new(None),
        core::cell::Cell::new(false),
        core::cell::RefCell::new(None),
    ));
    Ok(object.into_raw().cast::<Header>())
}

// ---- `OM-11` 的 `repr`／`str` 槽（形状**逐条实测**，见 tests/repr.rs 的文件头）----

/// 浮点的 `repr`：Rust 的 `{:?}` 已是"最短往返"，但指数写法与特殊值要和参照实现对齐
/// （实测：`1e+16`、`inf`、`nan`）。
fn float_repr_text(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    let text = format!("{value:?}");
    // Rust：`1e16`；参照实现：`1e+16`（指数带符号）
    match text.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with('-') && !exponent.starts_with('+') => {
            format!("{mantissa}e+{exponent}")
        }
        _ => text,
    }
}

/// `int` 的 `repr`：十进制（大整数走 `BigInt::to_decimal`）。
///
/// **`TS-45` ①的输出方向**：位数超过 `sys.get_int_max_str_digits()`（`0` ＝ 不限）⇒
/// `ValueError`（消息照参照**实测**，与输入方向那句不同：这句不带 `value has N digits`）。
pub unsafe fn int_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IntObject>() };
    let text = object.value.to_decimal();
    let limit = instance.int_max_str_digits();
    if limit != 0 && text.trim_start_matches('-').len() > limit as usize {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &digit_limit_message(limit),
        ));
    }
    Ok(text)
}

/// `bool` 的 `repr`／`str`：`True`／`False`。
pub unsafe fn bool_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<BoolObject>() };
    Ok(if object.value { "True" } else { "False" }.to_owned())
}

/// `NoneType` 的 `repr`：`None`。
pub unsafe fn none_repr(_ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    Ok("None".to_owned())
}

/// `float` 的 `repr`。
pub unsafe fn float_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FloatObject>() };
    Ok(float_repr_text(object.value))
}

/// `str` 的 `repr`：按参照实现的引号与转义规则（实测：能用单引号就用单引号）。
pub unsafe fn str_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StrObject>() };
    Ok(crate::instance::quote_str(object.value(), false))
}

/// `str` 的 `str`：内容本身。
pub unsafe fn str_str(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StrObject>() };
    Ok(object.value().to_owned())
}

/// `list` 的 `repr`：`[a, b]`；自引用给 `[...]`（实测）。
pub unsafe fn list_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Ok("[...]".to_owned());
    }
    let items = object.items();
    let mut text = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(instance, *item)?);
    }
    text.push(']');
    instance.leave_repr(ptr as usize);
    Ok(text)
}

/// `tuple` 的 `repr`：空是 `()`、单个是 `(x,)`（实测）。
pub unsafe fn tuple_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TupleObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Ok("(...)".to_owned());
    }
    let mut text = String::from("(");
    for index in 0..object.len() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(
            instance,
            object.item(index).expect("下标在范围内"),
        )?);
    }
    if object.len() == 1 {
        text.push(',');
    }
    text.push(')');
    instance.leave_repr(ptr as usize);
    Ok(text)
}

/// `dict` 的 `repr`：`{k: v}`；自引用给 `{'k': {...}}`（实测）。
pub unsafe fn dict_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Ok("{...}".to_owned());
    }
    let mut text = String::from("{");
    for (index, (key, value)) in object.entries().into_iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(instance, key)?);
        text.push_str(": ");
        text.push_str(&crate::executor::element_repr(instance, value)?);
    }
    text.push('}');
    instance.leave_repr(ptr as usize);
    Ok(text)
}

/// `set` 的 `repr`：空是 `set()`、否则 `{a, b}`（实测）。
pub unsafe fn set_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    let items = object.items();
    if items.is_empty() {
        return Ok("set()".to_owned());
    }
    let mut text = String::from("{");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(instance, *item)?);
    }
    text.push('}');
    Ok(text)
}

/// 类型对象的 `repr`：`<class 'int'>`（实测）。
pub unsafe fn type_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<crate::TypeObject>() };
    Ok(format!("<class '{}'>", object.name()))
}

/// 生成器的 `repr`：`<generator object gen at 0x…>`（实测）。
pub unsafe fn generator_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    unsafe { generator_repr_named(ptr, instance, "generator") }
}

/// 生成器／协程共用的 `repr`：词不同（实测 `<generator object f at 0x…>`／`<coroutine object f at 0x…>`）。
unsafe fn generator_repr_named(
    ptr: *mut Header,
    _instance: &Instance,
    word: &str,
) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    let frame = object.frame();
    // SAFETY: 帧由生成器持有，存活。
    let frame_ref = unsafe { &*frame.as_ptr().cast::<crate::Frame>() };
    let name = match frame_ref.code() {
        // SAFETY: code 由帧持有，存活。
        Some(code) => unsafe { code.cast::<crate::CodeObject>().as_ref() }.name(),
        None => "?",
    };
    Ok(format!("<{word} object {name} at {ptr:p}>"))
}

/// 函数的 `repr`：`<function demo at 0x…>`（实测）。
pub unsafe fn function_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    // SAFETY: 函数持有 code object 的一份引用。
    let code = object.code();
    // SAFETY: 同上。
    let name = unsafe { code.cast::<crate::CodeObject>().as_ref() }.name();
    Ok(format!("<function {name} at {ptr:p}>"))
}

/// code object 的 `repr`：`<code object demo at 0x…, file "…", line 1>`（实测）。
pub unsafe fn code_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<crate::CodeObject>() };
    Ok(format!(
        "<code object {} at {ptr:p}, file \"{}\", line {}>",
        object.name(),
        object.filename(),
        object.firstlineno()
    ))
}

/// 绑定方法的 `repr`：`<bound method m of <C object at 0x…>>`。
///
/// 实测的形状是 `<bound method C.m of …>`：名字取 **`BC-4` 的 `co_qualname`**
/// （编译器已产出它；类体方法那个 `C.m` 由**类创建钩子**在建类时补写——已落地）。
pub unsafe fn method_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    let function = object.function();
    // SAFETY: 方法对象持有函数的一份引用。
    let function_ref = unsafe { &*function.as_ptr().cast::<FunctionObject>() };
    // SAFETY: 函数持有 code object 的一份引用。
    let code = function_ref.code();
    // SAFETY: 同上。
    let qualname = unsafe { code.cast::<crate::CodeObject>().as_ref() }.qualname();
    Ok(format!(
        "<bound method {qualname} of {}>",
        instance.object_repr(object.this())?
    ))
}

/// 异常实例的 `repr`／`str`：`ValueError('x')`（实测：无参是 `ValueError()`）。
/// **`OM-11` 的 `str` 槽**（异常实例）——**与 `repr` 不同**，形状实测：
///
/// - 没有实参 ⇒ 空串（`str(ValueError())` == `''`）
/// - **一个**实参 ⇒ 那个实参的 `str`（`str(ValueError('x'))` == `'x'`）
/// - 多个实参 ⇒ 实参元组的 `repr`（`str(ValueError('a','b'))` == `"('a', 'b')"`）
///
/// 这条差异是 `T-BC-22` 的行为夹具抓出来的：此前 `str` 槽直接挂了 `exception_repr`，
/// 于是 `str(e)` 与 `repr(e)` 一模一样（`str(ValueError('x'))` 给的是 `"ValueError('x')"`）。
///
/// # Safety
///
/// 契约见 `StrFn`。
pub unsafe fn exception_str(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let header = unsafe { &*ptr };
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    let args = object.args();
    // `KeyError` 是**唯一**在单实参上改口径的：`str(KeyError('k'))` == `"'k'"`（实参的 repr）。
    // 实测：0 个实参仍是 `''`、≥2 个仍是实参元组的 repr（与基类同）⇒ 只改单实参那一格。
    // 子类继承（参照实现也继承）。
    let key_error_style = match instance.type_named("KeyError") {
        // SAFETY: 类型对象由注册表持有。
        Some(key_error) => instance.is_subtype(header.ty(), key_error),
        None => false,
    };
    match args.len() {
        0 => Ok(String::new()),
        1 if key_error_style => instance.object_repr(args[0]),
        1 => instance.object_str(args[0]),
        _ => {
            let mut rendered = Vec::with_capacity(args.len());
            for argument in args {
                rendered.push(instance.object_repr(argument)?);
            }
            Ok(format!("({})", rendered.join(", ")))
        }
    }
}

/// **`OM-11` 的 `repr` 槽**（异常实例）：`ValueError('x')`／`ValueError()`（实测）。
///
/// # Safety
///
/// 契约见 `ReprFn`。
pub unsafe fn exception_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let header = unsafe { &*ptr };
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    // SAFETY: 类型名由注册表持有。
    let name = unsafe { header.ty().as_ref() }.name();
    let args = object.args();
    let mut rendered = Vec::with_capacity(args.len());
    for argument in args {
        rendered.push(instance.object_repr(argument)?);
    }
    Ok(format!("{name}({})", rendered.join(", ")))
}

// ---- `__format__` 的原生实现（`TS-44`：**没有** `format` 槽，内建类型在**类型字典**里
// ---- 放**原生可调用对象**；默认行为以参照实现为准：空规格 ⇒ `str(x)`，非空且类型未覆写 ⇒ TypeError）

use crate::format::{self, SpecError};

/// 从原生调用的实参里取格式规格（`__format__(self, spec)` 的 `spec`）。
///
/// **先验类型**再读载荷（`TS-43`：布局自选，故禁止跨类型硬转），返回 **owned** 文本
/// （免得把借用传来传去）。
fn spec_argument(instance: &Instance, args: &[NonNull<Header>]) -> String {
    let Some(first) = args.first().copied() else {
        return String::new();
    };
    // SAFETY: first 由调用方保证存活。
    if unsafe { first.as_ref() }.ty() != instance.singletons().str_type() {
        return String::new();
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*first.as_ptr().cast::<StrObject>() }.value().to_owned()
}

/// 把 `format.rs` 的结果折成"原生返回值或实测消息的异常"。
fn format_outcome(
    instance: &Instance,
    result: Result<String, SpecError>,
    class_name: &str,
    max_str_digits: u32,
) -> Result<NonNull<Header>, crate::ExecError> {
    match result {
        Ok(text) => Ok(instance.new_str(&text)),
        Err(SpecError::NegativeZero) => Err(crate::executor::raise_builtin(
            instance,
            "ValueError",
            format::NEGATIVE_ZERO_MESSAGE,
        )),
        Err(SpecError::UnknownCode(code)) => {
            let message = format!("Unknown format code '{code}' for object of type '{class_name}'");
            Err(crate::executor::raise_builtin(instance, "ValueError", &message))
        }
        Err(SpecError::FloatOverflow) => Err(crate::executor::raise_builtin(
            instance,
            "OverflowError",
            "int too large to convert to float",
        )),
        Err(SpecError::CharTooLarge) => Err(crate::executor::raise_builtin(
            instance,
            "OverflowError",
            "Python int too large to convert to C long",
        )),
        Err(SpecError::CharOutOfRange) => Err(crate::executor::raise_builtin(
            instance,
            "OverflowError",
            "%c arg not in range(0x110000)",
        )),
        Err(SpecError::DigitLimit) => Err(instance.raise_builtin_error(
            "ValueError",
            &digit_limit_message(max_str_digits),
        )),
        Err(SpecError::NotImplemented) => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "这条格式化规格本层还没实现（迷你语言的其余部分）",
        }),
    }
}

/// `TS-45` ①的**输出方向**消息（`repr`／`str`／`format` 三处共用一条真相）。
fn digit_limit_message(limit: u32) -> String {
    format!(
        "Exceeds the limit ({limit} digits) for integer string conversion; \
         use sys.set_int_max_str_digits() to increase the limit"
    )
}

/// `object.__format__`（默认）：空规格 ⇒ `str(x)`；非空 ⇒ TypeError（消息实测）。
pub unsafe fn native_format_object(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec = spec_argument(instance, args);
    if !spec.is_empty() {
        // SAFETY: this 由调用方保证存活。
        let class_name = unsafe { this.as_ref().ty().as_ref() }.name();
        let message = format!("unsupported format string passed to {class_name}.__format__");
        return Err(crate::executor::raise_builtin(instance, "TypeError", &message));
    }
    Ok(instance.new_str(&instance.object_str(this)?))
}

/// `int.__format__`：空规格 ⇒ `str(self)`（于是 `bool` 走 `True`／`False`），否则数值规格。
pub unsafe fn native_format_int(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    if spec_text.is_empty() {
        return Ok(instance.new_str(&instance.object_str(this)?));
    }
    // `bool` 继承 `int.__format__`（实测 `bool.__dict__` 里**没有** `__format__`），
    // 所以这里必须按**实际类型**读载荷：`BoolObject` 与 `IntObject` 是两个布局。
    // SAFETY: this 是存活对象。
    let this_header = unsafe { this.as_ref() };
    let type_name = unsafe { this_header.ty().as_ref() }.name();
    let payload = if type_name == "bool" {
        // SAFETY: 类型身份已确认。
        IntValue::Small(i64::from(unsafe { &*this.as_ptr().cast::<BoolObject>() }.value))
    } else {
        // SAFETY: 同上。
        unsafe { &*this.as_ptr().cast::<IntObject>() }.value.clone()
    };
    let limit = instance.int_max_str_digits();
    match format::parse(&spec_text) {
        Ok(spec) => {
            let outcome = format::format_big_int(&payload.to_bigint(), &spec, limit);
            format_outcome(instance, outcome, type_name, limit)
        }
        Err(error) => format_outcome(instance, Err(error), type_name, limit),
    }
}

/// `float.__format__`：空规格 ⇒ `str(self)`，否则浮点规格。
pub unsafe fn native_format_float(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    if spec_text.is_empty() {
        return Ok(instance.new_str(&instance.object_str(this)?));
    }
    // SAFETY: 契约上 this 是 float 实例。
    let value = unsafe { &*this.as_ptr().cast::<FloatObject>() }.value;
    match format::parse(&spec_text) {
        Ok(spec) => {
            let outcome = format::format_float(value, &spec);
            format_outcome(instance, outcome, "float", instance.int_max_str_digits())
        }
        Err(error) => format_outcome(instance, Err(error), "float", instance.int_max_str_digits()),
    }
}

/// `str.__format__`：空规格 ⇒ 内容本身；否则只认对齐／宽度／精度。
pub unsafe fn native_format_str(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    // SAFETY: 契约上 this 是 str 实例。
    let text = unsafe { &*this.as_ptr().cast::<StrObject>() }.value().to_owned();
    let spec = match format::parse(&spec_text) {
        Ok(spec) => spec,
        Err(error) => return format_outcome(instance, Err(error), "str", instance.int_max_str_digits()),
    };
    if let Some(code) = spec.ty {
        if code != 's' {
            return format_outcome(instance, Err(SpecError::UnknownCode(code)), "str", instance.int_max_str_digits());
        }
    }
    format_outcome(
        instance,
        format::format_str(&text, &spec),
        "str",
        instance.int_max_str_digits(),
    )
}
