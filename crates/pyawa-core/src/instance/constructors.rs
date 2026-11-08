//! `Instance` 的构造器域方法（迭代器构造器为主 + 类型构造器；从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 第 169 轮正名：原 `setup.rs` 名字偏宽，内容以 `new_*_iterator` 一族为主。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// **数值加法**（`sum` 要用）：数值塔内给 `Some`（新引用），其余 `None`。
    pub fn add_values(&self, left: NonNull<Header>, right: NonNull<Header>) -> Option<NonNull<Header>> {
        let as_number = |object: NonNull<Header>| -> Option<(bool, f64)> {
            let ty = self.type_of(object);
            if let Some(value) = self.int_value(object) {
                if ty == self.singletons().int_type() || ty == self.singletons().bool_type() {
                    return Some((true, value as f64));
                }
            }
            if self.type_named("float") == Some(ty) {
                return Some((false, self.float_value(object)?));
            }
            None
        };
        // **`bytearray` 的拼接** ✓（第 643 轮）：任一操作数是 `bytearray` ⇒ 结果给 **`bytearray`** ✓
        //（参照口径 ✓）；另一侧要是 `bytes`／`bytearray` ✓。`re._compiler` 的 `data += chunk` 正需要它 ✓
        //（先前报 `unsupported operand type(s) for +: 'bytearray' and 'bytes'` ✗）。**只加这一支** ✗，
        // `bytes`／数值塔的既有行为一律不动 ✓。
        let name_of = |object: NonNull<Header>| self.type_name(self.type_of(object));
        if name_of(left) == "bytearray" || name_of(right) == "bytearray" {
            let mut bytes: Vec<u8> = Vec::new();
            for side in [left, right] {
                match name_of(side).as_str() {
                    "bytearray" => {
                        // SAFETY: 类型身份已确认，载荷就是 `BytearrayObject`。
                        let data = unsafe { &*side.as_ptr().cast::<crate::builtin_objects::BytearrayObject>() };
                        bytes.extend(data.value().iter().copied());
                    }
                    "bytes" => {
                        // SAFETY: 类型身份已确认。
                        let data = unsafe { &*side.as_ptr().cast::<crate::builtin_objects::BytesObject>() };
                        bytes.extend(data.value().iter().copied());
                    }
                    _ => return None,
                }
            }
            let ty = self.type_named("bytearray")?;
            return Some(
                self.alloc(crate::builtin_objects::BytearrayObject::new(
                    ty,
                    std::cell::RefCell::new(bytes),
                ))
                .into_raw()
                .cast::<Header>(),
            );
        }
        let (left_is_int, left_number) = as_number(left)?;
        let (right_is_int, right_number) = as_number(right)?;
        if left_is_int && right_is_int {
            return Some(self.new_int(left_number as i64 + right_number as i64));
        }
        Some(self.new_float(left_number + right_number))
    }

    /// 造一个 `int`（**任意精度载荷**，`TS-45`）——**新引用**。
    ///
    /// 装得下 `i64` 的走 [`Instance::new_int`]（于是 `OM-23` 的小整数单例照旧生效）；
    /// 大整数**不进单例表**（单例只覆盖 `-5..=256`）。
    pub fn new_int_value(&self, value: IntValue) -> NonNull<Header> {
        if let IntValue::Small(small) = value {
            return self.new_int(small);
        }
        self.alloc_int(value)
    }

    /// **造一个"空"的内部类型** ✓（第 288 轮）：给 stdlib 当**占位基类／占位类型**用 ✓
    /// （`_io` 的 `_IOBase` 一族与 `FileIO` 一族 ✓）。
    ///
    /// 为什么这个助手在**核心**：`CX-22` 说 stdlib **不许碰载荷布局** ✓ ⇒ 类型创建留在核心 ✓，
    /// stdlib 只拿"名字 ＋ 类型对象" ✓（`OM-11` 的 `dealloc` 是必填项 ⇒ 借 `PlainObject` 那一份 ✓）。
    ///
    /// **如实说** ✗：这类类型**没有** `new` 槽 ✓ ⇒ 实例化时按参照的"不能创建实例"报错 ✓、
    /// 方法面为空 ✓ —— 真正的 I/O 要走 `fs` 能力域 ✓（`P3-14` 的续 ✓）。
    pub fn new_bare_type(&self, name: &str) -> NonNull<TypeObject> {
        let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
        self.alloc_type_raw(
            leaked,
            core::mem::size_of::<PlainObject>(),
            Slots::new(PlainObject::dealloc),
        )
    }

    /// **从十进制串造一个 `int`**（第 285 轮，常量池的大整数字面量那条路 ✓）——**新引用**。
    ///
    /// 装得下 `i64` 的**降级**走 [`Instance::new_int`] ✓（小整数单例照旧 ✓）；
    /// 串解析不了给 `None` ✓（常量池里的串由编译器保证合法 ✓，这里只是防御 ✓）。
    pub fn new_int_from_decimal(&self, text: &str) -> Option<NonNull<Header>> {
        let value = IntValue::from_decimal(text)?;
        Some(self.new_int_value(value))
    }

    /// **类型下标的结果** ✓（第 214 轮）：`list[int]` ✓ —— CPython 给 `types.GenericAlias` ✓。
    ///
    /// 用既有那一档（`AttributeObject` ＋ 实例字典 ✓）惰性建出同名类型 ✓，装两个字段：
    /// `__origin__`＝被下标的类型 ✓、`__args__`＝下标 ✓（`Lib/types.py` 只取 `type(...)` ✓）。
    /// **如实说** ✗：别名目前只是"**装得下**" ✓ —— 不参与 `isinstance`／参数检查 ✓（随 `P3-*` 再接 ✓）。
    /// **类型 `|` 的结果** ✓（第 214 轮）：`int | str` ✓ —— CPython 给 `types.UnionType` ✓。
    ///
    /// 同样用"`AttributeObject` ＋ 实例字典"那一档惰性建出 ✓，装 `__args__`＝两元的 `tuple` ✓。
    /// **如实说** ✗：联合目前只是"**装得下**" ✓（不参与 `isinstance`／匹配 ✓，随 `P3-*` 再接 ✓）。
    pub fn new_union_type(&self, left: NonNull<Header>, right: NonNull<Header>) -> NonNull<Header> {
        let union_type = self
            .type_named("UnionType")
            .unwrap_or_else(|| self.new_attribute_type("UnionType"));
        let object = self
            .alloc(crate::builtin_objects::AttributeObject::new(
                union_type,
                core::cell::RefCell::new(Some(self.new_dict())),
            ))
            .into_raw()
            .cast::<Header>();
        // 两元 `tuple` ✓（`new_tuple` 接管传入的引用 ✓）。
        let args = self.new_tuple(vec![left, right]);
        let _ = self.set_attribute_value(object, "__args__", args);
        unsafe { self.release_object(args.as_ptr()) };
        object
    }

    pub fn new_generic_alias(&self, origin: NonNull<Header>, args: NonNull<Header>) -> NonNull<Header> {
        let alias_type = self
            .type_named("GenericAlias")
            .unwrap_or_else(|| self.new_attribute_type("GenericAlias"));
        // **`__mro_entries__` 随第一次造别名装上** ✓（task-20 第 2 轮 ✓）：引导期那遍可能还没建
        // `GenericAlias` 类型 ✗ ⇒ 这里**幂等**补 ✓（`class T(Mapping[str, str])` 靠它换基类 ✓）。
        if let Some(namespace) = self.type_namespace(alias_type.cast()) {
            if self.dict_get(namespace, "__mro_entries__").is_none() {
                let function_type = self
                    .type_named("builtin_function_or_method")
                    .expect("builtin_function_or_method 在引导期已登记");
                let function = self.alloc(crate::builtin_objects::BuiltinFunctionObject::new(
                    function_type,
                    "__mro_entries__",
                    core::cell::Cell::new(
                        crate::builtin_objects::generic_alias_mro_entries as crate::NativeFn,
                    ),
                ));
                self.dict_set(namespace, "__mro_entries__", function.into_raw().cast::<Header>());
            }
            // **`__getattr__` 也随第一次造别名装上** ✓（task-24 尾件 ✓）：别名缺属性转给
            // `__origin__` ✓（参照 `_GenericAlias` ✓）—— `Lib/typing.py` 的 `_ProtocolMeta.__new__`
            // 读 `base.__name__` 靠它 ✓。
            if self.dict_get(namespace, "__getattr__").is_none() {
                let function_type = self
                    .type_named("builtin_function_or_method")
                    .expect("builtin_function_or_method 在引导期已登记");
                let function = self.alloc(crate::builtin_objects::BuiltinFunctionObject::new(
                    function_type,
                    "__getattr__",
                    core::cell::Cell::new(
                        crate::builtin_objects::generic_alias_getattr as crate::NativeFn,
                    ),
                ));
                self.dict_set(namespace, "__getattr__", function.into_raw().cast::<Header>());
            }
        }
        let object = self
            .alloc(crate::builtin_objects::AttributeObject::new(
                alias_type,
                core::cell::RefCell::new(Some(self.new_dict())),
            ))
            .into_raw()
            .cast::<Header>();
        // `set_attribute_value` 收**借用** ✓ ⇒ 不额外加减 ✓。
        let _ = self.set_attribute_value(object, "__origin__", origin);
        let _ = self.set_attribute_value(object, "__args__", args);
        object
    }

    /// 造一个 `tuple`（元素是**新引用**，由元组接手）——**新引用**。
    /// 造一个 `itertools.repeat` 迭代器（**新引用**）。
    ///
    /// `value` 是**借用**入参——构造器自己新增一份引用（stdlib 侧是 `forbid(unsafe_code)`，
    /// 不能自己 `incref`）。`remaining < 0` 表示无限。
    pub fn new_repeat_iterator(&self, value: NonNull<Header>, remaining: i64) -> NonNull<Header> {
        // SAFETY: 调用方按 `OM-16` 保证 value 存活。
        unsafe { self.incref_object(value.as_ptr()) };
        let ty = self.type_named("repeat").expect("引导期已登记 repeat 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Repeat {
                value,
                remaining,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.islice` 迭代器（**新引用**）。
    ///
    /// **借用**入参（构造器自己加一份引用）——三个 `itertools` 构造器统一这条约定，
    /// 调用方始终保留自己那份（stdlib 侧用安全的 `Instance::release` 还）。
    /// `inner` 必须是本层认的迭代器（`executor::iter::iter_value` 交出来的就是）。
    pub fn new_islice_iterator(
        &self,
        inner: NonNull<Header>,
        start: i64,
        stop: i64,
        step: i64,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证 inner 存活。
        unsafe { self.incref_object(inner.as_ptr()) };
        let ty = self.type_named("islice").expect("引导期已登记 islice 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Islice {
                inner,
                start,
                position: 0,
                stop,
                step,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.takewhile`／`dropwhile`／`filterfalse` 迭代器（**新引用**）。
    ///
    /// `inner` 与 `predicate` 都是**借用**入参（构造器各加一份引用）。
    pub fn new_filter_like_iterator(
        &self,
        mode: u8,
        inner: NonNull<Header>,
        predicate: NonNull<Header>,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证两者存活。
        unsafe {
            self.incref_object(inner.as_ptr());
            self.incref_object(predicate.as_ptr());
        }
        let name = match mode {
            0 => "takewhile",
            1 => "dropwhile",
            _ => "filterfalse",
        };
        let ty = self
            .type_named(name)
            .unwrap_or_else(|| panic!("引导期已登记 {name} 类型"));
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::FilterLike {
                inner,
                predicate,
                mode,
                state: false,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.compress` 迭代器（**新引用**；两个入参都**借用**）。
    pub fn new_compress_iterator(
        &self,
        data: NonNull<Header>,
        selectors: NonNull<Header>,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证两者存活。
        unsafe {
            self.incref_object(data.as_ptr());
            self.incref_object(selectors.as_ptr());
        }
        let ty = self
            .type_named("compress")
            .expect("引导期已登记 compress 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Compress {
                data,
                selectors,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.combinations` 迭代器（**新引用**；两个入参都**借用**）。
    pub fn new_combinations_iterator(
        &self,
        pool: NonNull<Header>,
        r: i64,
        replace: bool,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证 pool 存活。
        unsafe { self.incref_object(pool.as_ptr()) };
        let indices = self.new_list(Vec::new());
        let name = if replace {
            "combinations_with_replacement"
        } else {
            "combinations"
        };
        let ty = self
            .type_named(name)
            .unwrap_or_else(|| panic!("引导期已登记 {name} 类型"));
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Combinations {
                pool,
                r,
                indices,
                started: false,
                done: false,
                replace,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.product` 迭代器（**新引用**；`pools` **借用**）。
    pub fn new_product_iterator(&self, pools: NonNull<Header>) -> NonNull<Header> {
        // SAFETY: 调用方保证 pools 存活。
        unsafe { self.incref_object(pools.as_ptr()) };
        let indices = self.new_list(Vec::new());
        let ty = self.type_named("product").expect("引导期已登记 product 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Product {
                pools,
                indices,
                started: false,
                done: false,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.permutations` 迭代器（**新引用**；`pool` **借用**）。
    pub fn new_permutations_iterator(&self, pool: NonNull<Header>, r: i64) -> NonNull<Header> {
        // SAFETY: 调用方保证 pool 存活。
        unsafe { self.incref_object(pool.as_ptr()) };
        let indices = self.new_list(Vec::new());
        let ty = self
            .type_named("permutations")
            .expect("引导期已登记 permutations 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Permutations {
                pool,
                r,
                indices,
                started: false,
                done: false,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.zip_longest` 迭代器（**新引用**；两个入参都**借用**）。
    ///
    /// `iterators` 是一个 `list`，元素都是迭代器（模块面先用 `iter_value` 造好）。
    /// 造一个 `zip` 迭代器 ✓（第 229 轮；**新引用** ✓；`iterators` **借用** ✓）。
    pub fn new_zip_iterator(&self, iterators: NonNull<Header>) -> NonNull<Header> {
        // SAFETY: 调用方保证 iterators 存活。
        unsafe { self.incref_object(iterators.as_ptr()) };
        let ty = self.type_named("zip").expect("引导期已登记 zip 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Zip { iterators }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个内建 **`map`** 迭代器（`P3-41` round 41；**新引用**；两个入参都**借用**）。
    ///
    /// `iterators` 是一个 `list`，元素都是迭代器（模块面先用 `iter_object` 造好 ✓）。
    pub fn new_map_iterator(
        &self,
        function: NonNull<Header>,
        iterators: NonNull<Header>,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证两者存活。
        unsafe {
            self.incref_object(function.as_ptr());
            self.incref_object(iterators.as_ptr());
        }
        let ty = self.type_named("map").expect("引导期已登记 map 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Map {
                function,
                iterators,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个内建 **`filter`** 迭代器（**新引用**；两个入参都**借用**；`predicate` 可以是 `None`）。
    pub fn new_filter_iterator(
        &self,
        predicate: NonNull<Header>,
        inner: NonNull<Header>,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证两者存活。
        unsafe {
            self.incref_object(predicate.as_ptr());
            self.incref_object(inner.as_ptr());
        }
        let ty = self.type_named("filter").expect("引导期已登记 filter 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Filter {
                predicate,
                inner,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    pub fn new_zip_longest_iterator(
        &self,
        iterators: NonNull<Header>,
        fillvalue: NonNull<Header>,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证两者存活。
        unsafe {
            self.incref_object(iterators.as_ptr());
            self.incref_object(fillvalue.as_ptr());
        }
        let ty = self
            .type_named("zip_longest")
            .expect("引导期已登记 zip_longest 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::ZipLongest {
                iterators,
                fillvalue,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.pairwise` 迭代器（**新引用**；`inner` **借用**）。
    pub fn new_pairwise_iterator(&self, inner: NonNull<Header>) -> NonNull<Header> {
        // SAFETY: 调用方保证 inner 存活。
        unsafe { self.incref_object(inner.as_ptr()) };
        let ty = self
            .type_named("pairwise")
            .expect("引导期已登记 pairwise 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Pairwise {
                inner,
                previous: None,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.batched` 迭代器（**新引用**；`inner` **借用**）。
    pub fn new_batched_iterator(&self, inner: NonNull<Header>, size: i64) -> NonNull<Header> {
        // SAFETY: 调用方保证 inner 存活。
        unsafe { self.incref_object(inner.as_ptr()) };
        let ty = self
            .type_named("batched")
            .expect("引导期已登记 batched 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Batched {
                inner,
                size,
                done: false,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.cycle` 迭代器（**新引用**；`inner` **借用**）。
    pub fn new_cycle_iterator(&self, inner: NonNull<Header>) -> NonNull<Header> {
        // SAFETY: 调用方保证 inner 存活。
        unsafe { self.incref_object(inner.as_ptr()) };
        let cache = self.new_list(Vec::new());
        let ty = self.type_named("cycle").expect("引导期已登记 cycle 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Cycle {
                inner,
                cache,
                filling: true,
                index: 0,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.accumulate` 迭代器（**新引用**；两个入参都**借用**）。
    pub fn new_accumulate_iterator(
        &self,
        inner: NonNull<Header>,
        function: Option<NonNull<Header>>,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证它们存活。
        unsafe {
            self.incref_object(inner.as_ptr());
            if let Some(value) = function {
                self.incref_object(value.as_ptr());
            }
        }
        let ty = self
            .type_named("accumulate")
            .expect("引导期已登记 accumulate 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Accumulate {
                inner,
                function,
                total: None,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.starmap` 迭代器（**新引用**；两个入参都**借用**）。
    pub fn new_starmap_iterator(
        &self,
        inner: NonNull<Header>,
        function: NonNull<Header>,
    ) -> NonNull<Header> {
        // SAFETY: 调用方保证两者存活。
        unsafe {
            self.incref_object(inner.as_ptr());
            self.incref_object(function.as_ptr());
        }
        let ty = self
            .type_named("starmap")
            .expect("引导期已登记 starmap 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Starmap {
                inner,
                function,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.chain` 迭代器（**新引用**）。
    ///
    /// `outer` 是**借用**入参（构造器自己加一份引用）：一个"逐个吐可迭代对象"的迭代器
    /// （`chain(*args)` 由模块面把参数收进 list 再 `iter_value` 得到它）。
    pub fn new_chain_iterator(&self, outer: NonNull<Header>) -> NonNull<Header> {
        // SAFETY: 调用方保证 outer 存活。
        unsafe { self.incref_object(outer.as_ptr()) };
        let ty = self.type_named("chain").expect("引导期已登记 chain 类型");
        self.alloc(crate::builtin_objects::ItStateObject::new(
            ty,
            core::cell::Cell::new(crate::builtin_objects::ItStateKind::Chain {
                outer,
                current: None,
            }),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个 `itertools.count` 迭代器（**新引用**；`SPEC-c-modules.md` §5.2.6）。
    ///
    /// `current` 是**下一个**要吐的值。只含整数 ⇒ 不持对象引用。
    pub fn new_count_iterator(&self, current: i64, step: i64) -> NonNull<Header> {
        let ty = self.type_named("count").expect("引导期已登记 count 类型");
        self.alloc(crate::builtin_objects::CountIteratorObject::new(
            ty,
            core::cell::Cell::new(current),
            core::cell::Cell::new(step),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// 造一个**实例带属性字典**的类型（用户类的实例就是这样）。
    ///
    /// 载荷用 [`AttributeObject`]（`TS-43`：布局自选），并置 [`crate::HAS_INSTANCE_DICT`]；
    /// 执行器据此决定 `STORE_ATTR` 往哪写。`object()` 自己**不**带字典——与参照实现一致。
    pub fn new_attribute_type(&self, name: &'static str) -> NonNull<TypeObject> {
        let ty = self.new_type(
            name,
            core::mem::size_of::<AttributeObject>(),
            AttributeObject::slots().with_new(crate::builtin_objects::attribute_new),
        );
        // SAFETY: ty 由注册表持有。
        unsafe { ty.as_ref() }.mark_has_instance_dict();
        ty
    }
}

/// **`mappingproxy` 的构造**（round 112）：`types.MappingProxyType = type(type.__dict__)` ✓ ——
/// 参照的 `type.__dict__` 是 **mappingproxy** 对象 ✓；本层先前直接交命名空间 dict ✗ ⇒
/// `type(type.__dict__)` 得到 **dict** ✗ ⇒ `pprint._dispatch` 里 `mappingproxy.__repr__` 覆盖了
/// `dict.__repr__` 的条目 ⇒ **每个 dict 都走 mappingproxy 的打印分支** ⇒ 无限递归 ✗
/// （`test_pprint` 一团 RecursionError ✓）。
///
/// 口径：载荷**照样是 `DictObject`**（MRO 里把 `dict` 作为基类 ⇒ 方法面／下标／`len` 全可用 ✓）；
/// 构造时**复制**一份条目（参照是**只读视图**；本层如实登记为"快照"，写操作不会回写底层 ✓）。
#[allow(unsafe_code)]
pub unsafe fn mappingproxy_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(source) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "mappingproxy() takes exactly one argument",
        ));
    };
    let Some(source_entries) = instance.dict_entries(source) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "mappingproxy() argument must be a mapping",
        ));
    };
    let entries: Vec<(NonNull<Header>, NonNull<Header>)> = source_entries
        .into_iter()
        .map(|(key, value)| {
            // SAFETY: 借用视图 ⇒ 新字典各持一份。
            unsafe {
                instance.incref_object(key.as_ptr());
                instance.incref_object(value.as_ptr());
            }
            (key, value)
        })
        .collect();
    let proxy = instance
        .alloc(DictObject::new(class, core::cell::RefCell::new(entries)))
        .into_raw()
        .cast::<Header>();
    // **记来源类型**（round 114）：`copy()`／`repr` 要还原（`OrderedDict`／`dict`）✓。
    let source_type = instance.type_of(source);
    if Some(source_type) != instance.type_named("dict") {
        instance.register_mappingproxy_source(proxy, source_type);
    }
    Ok(proxy)
}

impl Instance {
    /// **记一份 `mappingproxy` 的来源类型**（round 114）。
    pub fn register_mappingproxy_source(
        &self,
        proxy: NonNull<Header>,
        source_type: NonNull<crate::TypeObject>,
    ) {
        self.mappingproxy_sources
            .borrow_mut()
            .insert(proxy.as_ptr() as usize, source_type);
    }

    /// 取 `mappingproxy` 的来源类型（没有 ⇒ `None`：来源就是普通 dict ✓）。
    pub fn mappingproxy_source(&self, proxy: NonNull<Header>) -> Option<NonNull<crate::TypeObject>> {
        self.mappingproxy_sources
            .borrow()
            .get(&(proxy.as_ptr() as usize))
            .copied()
    }

    /// **登记一个"活视图" `mappingproxy`**（round 125）：`.mapping` 用 ✓。
    pub fn register_mappingproxy_live(&self, proxy: NonNull<Header>, source: NonNull<Header>) {
        self.mappingproxy_live
            .borrow_mut()
            .insert(proxy.as_ptr() as usize, source);
    }

    /// 取 `mappingproxy` 的活来源（没有 ⇒ `None`）。
    pub fn mappingproxy_live_source(&self, proxy: NonNull<Header>) -> Option<NonNull<Header>> {
        self.mappingproxy_live
            .borrow()
            .get(&(proxy.as_ptr() as usize))
            .copied()
    }

    /// **把活视图代理的快照与源字典同步**（round 125）：`m == d` 一类入口先调 ✓。
    /// 旧条目逐项归还（借用视图 ⇒ 需要自己 retain 新的）✓。
    pub fn sync_mappingproxy(&self, proxy: NonNull<Header>) {
        let Some(source) = self.mappingproxy_live_source(proxy) else {
            return;
        };
        let Some(entries) = self.dict_entries(source) else {
            return;
        };
        // SAFETY: proxy 是 mappingproxy（DictObject 同构布局）✓。
        let dict = unsafe { &*proxy.as_ptr().cast::<crate::builtin_objects::DictObject>() };
        // 清掉旧快照（键值各一份引用要归还 ✓）。
        let old_entries = core::mem::take(&mut *dict.entries.borrow_mut());
        for (key, value) in old_entries {
            // SAFETY: 旧引用由代理持有。
            unsafe {
                self.release_object(key.as_ptr());
                self.release_object(value.as_ptr());
            }
        }
        for (key, value) in entries {
            // SAFETY: 键值由源 dict 持有；代理要自己那两份。
            unsafe {
                self.incref_object(key.as_ptr());
                self.incref_object(value.as_ptr());
            }
            self.dict_insert_raw(proxy, key, value);
        }
    }

    /// **按来源类型"复制"一份映射**（round 114）：`mappingproxy.copy()` 用 ✓ ——
    /// 来源是 `OrderedDict` 一类 ⇒ 构造它的新实例 ✓（pprint 的 `mappingproxy(OrderedDict([...]))` ✓）；
    /// 来源是普通 dict ⇒ 新 dict ✓。
    pub fn mappingproxy_copy_like(&self, proxy: NonNull<Header>) -> Option<NonNull<Header>> {
        let entries = self.dict_entries(proxy)?;
        let dict = self.new_dict();
        for (key, value) in entries {
            self.dict_insert_raw(dict, self.retain(key), self.retain(value));
        }
        match self.mappingproxy_source(proxy) {
            None => Some(dict),
            Some(source_type) => {
                // 调来源类型的构造函数（`OrderedDict(dict)` ✓）；实参所有权**转移**给调用 ✓。
                let result = crate::executor::call::call_callable(
                    self,
                    source_type.cast::<Header>(),
                    None,
                    vec![dict],
                    Vec::new(),
                    0,
                );
                match result {
                    Ok(value) => Some(value),
                    Err(_) => {
                        // 构造失败 ⇒ 退普通 dict（如实降级 ✓；正常类不会到这里）。
                        self.release(dict);
                        None
                    }
                }
            }
        }
    }

    /// **把一份 dict 包成 `mappingproxy`**（round 112；`type.__dict__` 那条读法用 ✓）：
    /// 载荷是**复制**的条目（本层如实登记为快照 ✓）。
    pub fn mappingproxy_of(&self, source: NonNull<Header>) -> Option<NonNull<Header>> {
        let class = self.type_named("mappingproxy")?;
        let entries = self.dict_entries(source)?;
        let copied: Vec<(NonNull<Header>, NonNull<Header>)> = entries
            .into_iter()
            .map(|(key, value)| {
                // SAFETY: 借用视图 ⇒ 新字典各持一份。
                unsafe {
                    self.incref_object(key.as_ptr());
                    self.incref_object(value.as_ptr());
                }
                (key, value)
            })
            .collect();
        Some(
            self.alloc(DictObject::new(class, core::cell::RefCell::new(copied)))
                .into_raw()
                .cast::<Header>(),
        )
    }
}

/// `mappingproxy` 的 `repr`：`mappingproxy({...})` ✓（内层照 dict 的 repr）。
pub unsafe fn mappingproxy_repr(
    ptr: *mut Header,
    instance: &Instance,
) -> Result<String, crate::ExecError> {
    let object = NonNull::new(ptr).expect("调用方保证非空");
    // **有来源类型**（`OrderedDict` 等）⇒ 内层照它的 repr ✓（`mappingproxy(OrderedDict([...]))` ✓）。
    if instance.mappingproxy_source(object).is_some() {
        if let Some(copied) = instance.mappingproxy_copy_like(object) {
            // SAFETY: copied 是新引用。
            let text = instance.object_repr(copied);
            unsafe { instance.release_object(copied.as_ptr()) };
            return Ok(format!("mappingproxy({})", text?));
        }
    }
    // SAFETY: 载荷与 dict 同构。
    let inner = unsafe { crate::builtin_objects::dict_repr(ptr, instance) }?;
    Ok(format!("mappingproxy({inner})"))
}

/// **`mappingproxy.copy()`**（round 114）：交**按来源类型复制**的映射 ✓（见
/// [`Instance::mappingproxy_copy_like`]）。
pub(crate) fn mappingproxy_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, _) = crate::builtin_objects::container_receiver(bound, args);
    let Some(proxy) = receiver else {
        return Err(instance.raise_builtin_error("TypeError", "copy() 需要 mappingproxy"));
    };
    instance
        .mappingproxy_copy_like(proxy)
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "mappingproxy 的载荷不是映射"))
}

/// **`mappingproxy` 的属性面**：`copy` 走自己的实现 ✓，其余委托 `dict_getattr` ✓。
pub unsafe fn mappingproxy_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if name == "copy" {
        let method_type = instance
            .type_named("builtin_function_or_method")
            .expect("builtin_function_or_method 已登记");
        let native = instance.alloc(BuiltinFunctionObject::new(
            method_type,
            "copy",
            core::cell::Cell::new(mappingproxy_copy_native),
        ));
        let native_raw = native.into_raw().cast::<Header>();
        // SAFETY: 方法对象要自己那份 self。
        unsafe { instance.incref_object(ptr) };
        let bound = instance.alloc(crate::MethodObject::new(
            instance.type_named("method").expect("method 已登记"),
            native_raw,
            unsafe { NonNull::new_unchecked(ptr) },
        ));
        return Some(bound.into_raw().cast::<Header>());
    }
    // SAFETY: 契约同上（槽位保证 ptr 是本类型族存活对象）。
    unsafe { crate::builtin_objects::dict_getattr(ptr, name, instance) }
}
