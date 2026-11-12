//! **热路径类型指针缓存**（round 246 性能，A2-1）：把"按类型**名字**查表"从执行器热路径拿掉。
//!
//! 背景（`target/hotprofile.txt` 实测）：`Instance::type_named` 每次都是 SipHash 查表 ⇒
//! `is_iterator_type` 每个 `FOR_ITER` 做 36 次、`release_object` 每次释放**线性扫**一遍类型表、
//! `truthiness`／`arithmetic_public`／`inplace_add` 每条指令各做几次 —— 基线里"按名字查类型"
//! ≈ **37%**、"类型表线性扫" ≈ **6%** ✗。
//!
//! 口径（2026-11-12 用户口径）：**字符串名只留边界** —— 本文件**重建缓存**时每个名字查一次；
//! 热路径只做 [`NonNull<TypeObject>`](core::ptr::NonNull) 的**指针比较** ✓。
//!
//! 语义等价依据：`type_named` 是"**先登记者优先**" ⇒ 凡本表列出的名字都是内建类型名，
//! 它们的首个类型在引导期定死、且**不会被退休**（退休只发生在用户动态类上）⇒ 缓存命中后
//! 不必再回查 ✓。惰性创建的内建类型（如 `range_iterator`）出现在 `types_generation` 变化之后
//! ⇒ 缓存重建时自动补上 ✓（已经命中的名字不重查，重建成本与"缺多少"成正比 ✓）。
//!
//! **round 249（A2-4）**：槽位改成 `[Cell<Option<NonNull<TypeObject>>>]`（每查一次只读一次
//! `Cell`，不再 `RefCell` 借用 ✗）；"迭代器类型／专用迭代器类型"改**类型能力位**（建类型时定死），
//! 不再维护指针集合 ✓。

use super::*;
use crate::type_object::TypeObject;

/// 热路径分派用的类型名（**枚举值 = 缓存下标**；`name` 只在重建缓存时出现 ✓）。
macro_rules! hot_ids {
    ($($variant:ident => $name:literal),* $(,)?) => {
        /// 热路径上按**类型指针**分派的内建类型（名字 → 稳定下标，重建时各查一次）。
        #[derive(Clone, Copy)]
        #[repr(usize)]
        pub(crate) enum HotId {
            $($variant),*
        }

        impl HotId {
            /// 全部编号（重建缓存时逐项解析名字）。
            pub(crate) const ALL: &'static [HotId] = &[$(HotId::$variant),*];

            /// 编号个数（缓存数组长度）。
            pub(crate) const COUNT: usize = HotId::ALL.len();

            /// 这个编号对应的**类型名**（只在重建缓存时用；热路径不碰字符串 ✓）。
            pub(crate) fn name(self) -> &'static str {
                match self {
                    $(HotId::$variant => $name),*
                }
            }
        }
    };
}

hot_ids! {
    // 对象模型与内建标量／容器
    Object => "object",
    Type => "type",
    Int => "int",
    Float => "float",
    Complex => "complex",
    Bool => "bool",
    Str => "str",
    Bytes => "bytes",
    ByteArray => "bytearray",
    List => "list",
    Tuple => "tuple",
    Dict => "dict",
    Set => "set",
    FrozenSet => "frozenset",
    Range => "range",
    Deque => "deque",
    Slice => "slice",
    Cell => "cell",
    WeakRef => "weakref",
    MappingProxy => "mappingproxy",
    NotImplementedType => "NotImplementedType",
    NoneType => "NoneType",
    Ellipsis => "ellipsis",
    MemoryView => "memoryview",
    GetSetDescriptor => "getset_descriptor",
    MemberDescriptor => "member_descriptor",
    // 视图／映射／容器族（`dict` 的 keys/items/values 视图类型是按需建的）
    DictKeys => "dict_keys",
    DictItems => "dict_items",
    DictValues => "dict_values",
    DefaultDict => "defaultdict",
    SetIterator => "set_iterator",
    // 线程／上下文／typing 面
    Lock => "lock",
    ThreadHandle => "_ThreadHandle",
    TypeVar => "TypeVar",
    Generic => "Generic",
    UnionType => "UnionType",
    GenericAlias => "GenericAlias",
    Token => "Token",
    ContextVar => "ContextVar",
    Context => "Context",
    FrameLocalsProxy => "FrameLocalsProxy",
    SimpleNamespace => "SimpleNamespace",
    NoDefaultType => "NoDefaultType",
    TypeAliasType => "TypeAliasType",
    Asend => "async_generator_asend",
    ABCMeta => "ABCMeta",
    // 调用／帧／生成器面
    Function => "function",
    BuiltinFunctionOrMethod => "builtin_function_or_method",
    Method => "method",
    CodeObject => "CodeObject",
    Module => "module",
    Frame => "frame",
    Traceback => "traceback",
    Generator => "generator",
    Coroutine => "coroutine",
    AsyncGenerator => "async_generator",
    StaticMethod => "staticmethod",
    ClassMethod => "classmethod",
    Property => "property",
    Super => "super",
    // 异常族（raise／匹配／对拍热路径）
    BaseException => "BaseException",
    Exception => "Exception",
    TypeError => "TypeError",
    ValueError => "ValueError",
    AttributeError => "AttributeError",
    KeyError => "KeyError",
    IndexError => "IndexError",
    StopIteration => "StopIteration",
    StopAsyncIteration => "StopAsyncIteration",
    GeneratorExit => "GeneratorExit",
    SystemExit => "SystemExit",
    RuntimeError => "RuntimeError",
    ZeroDivisionError => "ZeroDivisionError",
    OverflowError => "OverflowError",
    NotImplementedError => "NotImplementedError",
    NameError => "NameError",
    ImportError => "ImportError",
    SyntaxError => "SyntaxError",
    OSError => "OSError",
    UnicodeError => "UnicodeError",
    DeprecationWarning => "DeprecationWarning",
    AssertionError => "AssertionError",
    // 迭代器协议（含 `itertools` 的专用推进分支）
    Iterator => "iterator",
    CallableIterator => "callable_iterator",
    DictKeyIterator => "dict_keyiterator",
    DictValueIterator => "dict_valueiterator",
    DictItemIterator => "dict_itemiterator",
    ListIterator => "list_iterator",
    TupleIterator => "tuple_iterator",
    ListReverseIterator => "list_reverseiterator",
    Reversed => "reversed",
    Count => "count",
    Repeat => "repeat",
    Islice => "islice",
    RangeIterator => "range_iterator",
    LongRangeIterator => "longrange_iterator",
    Product => "product",
    Permutations => "permutations",
    Compress => "compress",
    Combinations => "combinations",
    CombinationsWithReplacement => "combinations_with_replacement",
    Zip => "zip",
    Map => "map",
    Filter => "filter",
    ZipLongest => "zip_longest",
    Pairwise => "pairwise",
    Batched => "batched",
    Cycle => "cycle",
    Accumulate => "accumulate",
    Starmap => "starmap",
    Takewhile => "takewhile",
    Dropwhile => "dropwhile",
    Filterfalse => "filterfalse",
    Chain => "chain",
}

impl Instance {
    /// 热类型槽过期（按 [`Instance::types_generation`]）就重建；否即是空操作（只读一次 `Cell` ✓）。
    fn ensure_hot_types(&self) {
        let generation = self.types_generation.get();
        if self.hot_types_generation.get() == generation {
            return;
        }
        // **已命中的名字不重查**（内建名的首个登记不会变 ✓）；只补还缺的 ✓。
        for (index, id) in HotId::ALL.iter().enumerate() {
            if self.hot_type_slots[index].get().is_none() {
                self.hot_type_slots[index].set(self.type_named(id.name()));
            }
        }
        self.hot_types_generation.set(generation);
    }

    /// 内建类型的**缓存指针**（热路径分派用；未登记 ⇒ `None`）。
    ///
    /// 只在**边界**（缓存重建）用字符串名；热路径只读一次 `Cell` ✓。
    pub(crate) fn hot_type(&self, id: HotId) -> Option<NonNull<TypeObject>> {
        self.ensure_hot_types();
        self.hot_type_slots[id as usize].get()
    }

    /// `ty` 是不是 `id` 指的那个内建类型（**指针比较**，热路径 ✓）。
    pub(crate) fn hot_type_is(&self, id: HotId, ty: NonNull<TypeObject>) -> bool {
        self.hot_type(id) == Some(ty)
    }

    /// `ty` 是不是 `ids` 里**任一**内建类型（**精确**比较；热路径指针比较 ✓）。
    pub(crate) fn hot_type_is_any(&self, ty: NonNull<TypeObject>, ids: &[HotId]) -> bool {
        ids.iter().any(|id| self.hot_type_is(*id, ty))
    }

    /// `ty` 是不是 `ids` 里**任一**内建类型（**含子类**；指针表 + MRO 指针扫，无字符串名 ✓）。
    pub(crate) fn hot_subtype_of_any(&self, ty: NonNull<TypeObject>, ids: &[HotId]) -> bool {
        ids.iter()
            .any(|id| self.hot_type(*id).is_some_and(|base| self.is_subtype(ty, base)))
    }

    /// 这个类型是不是本层接线的**迭代器类型**（`FOR_ITER` 每轮一次：**一次能力位测** ✓；
    /// 能力位在建类型时按"首个登记"名字定死 —— 与旧口径（`type_named` 比 36 个名字）等价 ✓）。
    pub(crate) fn hot_is_iterator_type(&self, ty: NonNull<TypeObject>) -> bool {
        // SAFETY: 类型对象由注册表持有、在实例存活期间一直有效。
        unsafe { ty.as_ref() }.is_iterator_kind()
    }

    /// 地址是不是**注册表里的类型对象**（`release_object` 每次释放一次：O(log n)，不再线性扫 ✓）。
    pub(crate) fn is_registered_type_address(&self, address: usize) -> bool {
        self.type_addresses.borrow().binary_search(&address).is_ok()
    }

    /// 登记一个类型的地址（`alloc_type_raw` 里调用：保持 `type_addresses` 升序 ✓）。
    pub(crate) fn register_type_address(&self, address: usize) {
        let mut addresses = self.type_addresses.borrow_mut();
        if let Err(index) = addresses.binary_search(&address) {
            addresses.insert(index, address);
        }
    }

    /// 摘掉一个类型的地址（孤儿回收 retire 时调用 ✓）。
    pub(crate) fn unregister_type_address(&self, address: usize) {
        self.type_addresses
            .borrow_mut()
            .retain(|entry| *entry != address);
    }
}
