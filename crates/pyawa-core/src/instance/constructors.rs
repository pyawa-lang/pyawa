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
