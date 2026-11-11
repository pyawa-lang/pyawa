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

use super::*;
use crate::executor::ITERATOR_TYPE_NAMES;
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
    // 迭代器协议（含 `itertools` 的专用推进分支）
    Iterator => "iterator",
    CallableIterator => "callable_iterator",
    DictKeyIterator => "dict_keyiterator",
    DictValueIterator => "dict_valueiterator",
    DictItemIterator => "dict_itemiterator",
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

/// 一份**按 `types_generation` 重建**的类型指针缓存。
pub(crate) struct HotTypes {
    /// 建立这份缓存时的 [`Instance::types_generation`]。
    generation: usize,
    /// `HotId` 下标 → 类型指针（名字当时未登记 ⇒ `None`，下次重建再试）。
    pointers: [Option<NonNull<TypeObject>>; HotId::COUNT],
    /// `ITERATOR_TYPE_NAMES` 下标 → 类型指针（同上）。
    iterator_pointers: [Option<NonNull<TypeObject>>; ITERATOR_TYPE_NAMES.len()],
}

impl HotTypes {
    /// 重建：已命中的名字**不重查**（内建类型的首个登记不会变 ✓）；只补还缺的那些 ✓。
    fn build(instance: &Instance, generation: usize, previous: Option<HotTypes>) -> Self {
        let previous = previous.as_ref();
        let mut pointers = [None; HotId::COUNT];
        for (index, id) in HotId::ALL.iter().enumerate() {
            pointers[index] = previous
                .and_then(|cache| cache.pointers[index])
                .or_else(|| instance.type_named(id.name()));
        }
        let mut iterator_pointers = [None; ITERATOR_TYPE_NAMES.len()];
        for (index, name) in ITERATOR_TYPE_NAMES.iter().enumerate() {
            iterator_pointers[index] = previous
                .and_then(|cache| cache.iterator_pointers[index])
                .or_else(|| instance.type_named(name));
        }
        Self {
            generation,
            pointers,
            iterator_pointers,
        }
    }
}

impl Instance {
    /// 缓存过期（按 [`Instance::types_generation`]）就重建；否即是空操作。
    fn ensure_hot_types(&self) {
        let generation = self.types_generation.get();
        let fresh = {
            let cache = self.hot_types.borrow();
            cache
                .as_ref()
                .is_some_and(|cache| cache.generation == generation)
        };
        if fresh {
            return;
        }
        // **先取出旧缓存再重建**（`type_named` 里不碰本缓存 ⇒ 不会重入 ✓）。
        let previous = self.hot_types.borrow_mut().take();
        let rebuilt = HotTypes::build(self, generation, previous);
        *self.hot_types.borrow_mut() = Some(rebuilt);
    }

    /// 内建类型的**缓存指针**（热路径分派用；未登记 ⇒ `None`）。
    ///
    /// 只在**边界**（缓存重建）用字符串名；调用方拿它做指针比较 ✓。
    pub(crate) fn hot_type(&self, id: HotId) -> Option<NonNull<TypeObject>> {
        self.ensure_hot_types();
        self.hot_types
            .borrow()
            .as_ref()
            .and_then(|cache| cache.pointers[id as usize])
    }

    /// `ty` 是不是 `id` 指的那个内建类型（**指针比较**，热路径 ✓）。
    pub(crate) fn hot_type_is(&self, id: HotId, ty: NonNull<TypeObject>) -> bool {
        self.hot_type(id) == Some(ty)
    }

    /// 这个类型是不是本层接线的**迭代器类型**（`FOR_ITER` 每轮一次：指针比较，不再查 36 次名字 ✓）。
    pub(crate) fn hot_is_iterator_type(&self, ty: NonNull<TypeObject>) -> bool {
        self.ensure_hot_types();
        self.hot_types.borrow().as_ref().is_some_and(|cache| {
            cache
                .iterator_pointers
                .iter()
                .flatten()
                .any(|candidate| *candidate == ty)
        })
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
