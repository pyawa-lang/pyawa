//! `Instance` 的对象分配与新建域（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// 造一个 `None`（**新引用**）。
    pub fn new_none(&self) -> NonNull<Header> {
        let none = self.singletons().none();
        // SAFETY: 单例由实例持有；这里新增一份给调用方。
        unsafe { self.incref_object(none.as_ptr()) };
        none
    }

    /// 造一个布尔（**新引用**）。
    pub fn new_bool(&self, value: bool) -> NonNull<Header> {
        let flag = self.singletons().boolean(value);
        // SAFETY: 同上。
        unsafe { self.incref_object(flag.as_ptr()) };
        flag
    }

    /// 造一个浮点（**新引用**）。
    pub fn new_float(&self, value: f64) -> NonNull<Header> {
        let float_type = self
            .type_named("float")
            .expect("float 在引导期已登记（OM-13）");
        self.alloc(FloatObject::new(float_type, value))
            .into_raw()
            .cast::<Header>()
    }

    /// 造一个 `list`（**接手**一批新引用，`CM-4` 的 stdlib 要用）。
    pub fn new_list(&self, items: Vec<NonNull<Header>>) -> NonNull<Header> {
        let object = self.alloc(ListObject::new(
            self.type_named("list").expect("list 在引导期已登记"),
            core::cell::RefCell::new(items),
        ));
        object.into_raw().cast::<Header>()
    }

    /// 造一个 `set`（**新引用**）。
    pub fn new_set(&self, items: Vec<NonNull<Header>>) -> NonNull<Header> {
        let object = self.alloc(SetObject::new(
            self.type_named("set").expect("set 在引导期已登记"),
            core::cell::RefCell::new(items),
        ));
        object.into_raw().cast::<Header>()
    }

    /// 造一个 `slice`（**新引用**）——给切片路径与测试用。
    pub fn new_slice(&self, start: Option<i64>, stop: Option<i64>, step: Option<i64>) -> NonNull<Header> {
        let slice_type = self.type_named("slice").expect("slice 在引导期已登记");
        self.alloc(SliceObject::new(slice_type, start, stop, step))
            .into_raw()
            .cast::<Header>()
    }

    /// 造一个 `bytes`（**新引用**）——给编译产物的常量池（`P1-12`）与构造路径用。
    pub fn new_bytes(&self, value: &[u8]) -> NonNull<Header> {
        let bytes_type = self
            .type_named("bytes")
            .expect("bytes 在引导期已登记");
        self.alloc(BytesObject::new(bytes_type, value.to_vec()))
            .into_raw()
            .cast::<Header>()
    }

    /// 建一个空 `dict`（**新引用**）——给 stdlib 模块建命名空间用（`CM-4` 的 Python 面）。
    pub fn new_dict(&self) -> NonNull<Header> {
        let dict_type = self
            .type_named("dict")
            .expect("dict 在引导期已登记（OM-13）");
        self.alloc(DictObject::new(dict_type, RefCell::new(Vec::new())))
            .into_raw()
            .cast::<Header>()
    }

    /// **`AB-58`／`OM-14`**：按**定长宿主布局**分配一个实例（头部 ＋ `payload_size` 字节载荷）。
    ///
    /// 返回 `(对象, 载荷指针)`；载荷为 `None` 表示 `payload_size == 0`（`AB-58`）。
    /// 载荷由 VM 分配、归 VM 所有（随对象释放），**宿主禁止** `free`／`realloc`；
    /// 宿主必须在**对象对脚本可见之前**把它填完。载荷区域已清零。
    pub fn alloc_host_object(
        &self,
        ty: NonNull<TypeObject>,
    ) -> (NonNull<Header>, Option<NonNull<u8>>) {
        // SAFETY: ty 由本实例的注册表持有。
        let info = unsafe { ty.as_ref() };
        let size = info.instance_size;
        assert!(
            size >= crate::header::HEADER_SIZE_BYTES,
            "OM-5：定长宿主布局至少要装得下头部"
        );
        let layout = core::alloc::Layout::from_size_align(
            size,
            core::mem::align_of::<crate::Header>(),
        )
        .expect("宿主载荷尺寸溢出");
        // SAFETY: layout 非零尺寸（≥ 头部）。
        let raw = unsafe { std::alloc::alloc_zeroed(layout) };
        let Some(raw) = NonNull::new(raw) else {
            std::alloc::handle_alloc_error(layout);
        };
        let header = raw.cast::<crate::Header>();
        // SAFETY: 刚分配、已清零，且还没有别的地方引用它。
        unsafe { header.as_ptr().write(crate::Header::new(ty)) };

        let tracked = info.slots.traverse.is_some();
        if tracked {
            // `OM-12`：位在**对象头部**上（`flags::GC_TRACKED`），**不是**类型标志——
            // 类型标志的 `1 << 1` 是 `INLINE_INSTANCE_DICT`，写错会把宿主对象当成"内联字典"
            // 去读载荷（症状：属性查找读出垃圾字典指针，`misaligned pointer dereference`）。
            // SAFETY: header 刚写好、还没有别的地方引用它。
            unsafe { header.as_ref() }.set_flag(crate::flags::GC_TRACKED);
        }
        if self.zombie_trace.get() {
            // **分配时清掉释放登记** ✓（第 92 轮纠错 ✓）：登记表按**地址**记 ✓，而地址会被复用 ✗
            // ⇒ 不清就会**假阳性** ✓（实测：刚造好的字典被指认为"已释放" ✗，而它的 `rc` 明明是 1 ✓）。
            // 清掉之后，命中就只剩一种含义：**释放之后没再分配过** ⇒ 可靠 ✓。
            self.freed_sites
                .borrow_mut()
                .remove(&(header.as_ptr() as usize));
        }
        self.live.borrow_mut().insert(header.as_ptr() as usize);
        self.bytes_allocated.set(self.bytes_allocated.get() + size);
        if tracked {
            self.link_gc(header);
        }

        let payload = if size > crate::header::HEADER_SIZE_BYTES {
            // SAFETY: 分配了 size 字节，偏移在范围内。
            Some(unsafe {
                NonNull::new_unchecked(raw.as_ptr().add(crate::header::HEADER_SIZE_BYTES))
            })
        } else {
            None
        };
        (header, payload)
    }

    /// 造一个整数（落在单例区间就用那个单例）——**新引用**。
    ///
    /// 给**对象类型自己的槽位实现**用（`getattr` 一类要在 crate 内造可见对象）。
    pub fn new_int(&self, value: i64) -> NonNull<Header> {
        if let Some(singleton) = self.singletons().small_int(value) {
            // SAFETY: 单例由实例持有，存活。
            unsafe { self.incref_object(singleton.as_ptr()) };
            return singleton;
        }
        self.alloc_int(IntValue::Small(value))
    }

    /// **`traceback` 对象** ✓（第 213 轮，**`BC-60` 的最小起步** ✓）：`tb_frame` ＝ 抛出处的帧 ✓，
    /// `tb_next`／`tb_lineno`／`tb_lasti` 先给 `None`／`0`／`0` ✓ —— **如实说** ✗：行号与链式 `tb_next`
    /// **尚未接线** ✓（`DIV-6` 仍留着 ✓，等 `BC-60` 的完整面 ✓）。
    pub fn new_traceback(&self, frame: NonNull<Header>) -> NonNull<Header> {
        let traceback_type = self
            .type_named("traceback")
            .unwrap_or_else(|| self.new_attribute_type("traceback"));
        let object = self
            .alloc(crate::builtin_objects::AttributeObject::new(
                traceback_type,
                core::cell::RefCell::new(Some(self.new_dict())),
            ))
            .into_raw()
            .cast::<Header>();
        // `set_attribute_value` 收的是**借用** ✓ ⇒ 这里每项自己那份用完即还 ✓（口径见第 209 轮 ✓）。
        // **尽力而为** ✓（异常／追踪对象没有实例字典时如实不挂 ✓）。
        let _ = self.set_attribute_value(object, "tb_frame", frame);
        let none = self.new_none();
        // **尽力而为** ✓（异常／追踪对象没有实例字典时如实不挂 ✓）。
        let _ = self.set_attribute_value(object, "tb_next", none);
        unsafe { self.release_object(none.as_ptr()) };
        let lineno = self.new_int(0);
        // **尽力而为** ✓（异常／追踪对象没有实例字典时如实不挂 ✓）。
        let _ = self.set_attribute_value(object, "tb_lineno", lineno);
        unsafe { self.release_object(lineno.as_ptr()) };
        let lasti = self.new_int(0);
        // **尽力而为** ✓（异常／追踪对象没有实例字典时如实不挂 ✓）。
        let _ = self.set_attribute_value(object, "tb_lasti", lasti);
        unsafe { self.release_object(lasti.as_ptr()) };
        object
    }

    pub fn new_str(&self, text: &str) -> NonNull<Header> {
        if text.is_empty() {
            let empty = self.singletons().empty_str();
            // SAFETY: 单例由实例持有，存活。
            unsafe { self.incref_object(empty.as_ptr()) };
            return empty;
        }
        self.alloc(StrObject::new(
            self.singletons().str_type(),
            text.to_owned(),
        ))
        .into_raw()
        .cast::<Header>()
    }

    pub fn new_tuple(&self, items: Vec<NonNull<Header>>) -> NonNull<Header> {
        if items.is_empty() {
            // **OM-23**：空元组是**单例**（`() is ()` 为真）——调用方按"新引用"接收，
            // 所以这里要多给一份。
            return self.retain(self.singletons().empty_tuple());
        }
        let tuple_type = self
            .type_named("tuple")
            .expect("tuple 在引导期已登记");
        self.alloc(TupleObject::new(tuple_type, items))
            .into_raw()
            .cast::<Header>()
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
        let ty = self.alloc_type_raw(name, instance_size, slots);
        // CPython 里"没写基类"的类继承 `object`；MRO 仍由 C3 算（OM-13）
        let object_type = self
            .type_named("object")
            .expect("object 在 Instance::new 的引导期就已登记");
        assert!(
            self.register_bases(ty, vec![object_type]).is_some(),
            "新类型的 MRO 应当总能算出来"
        );
        ty
    }
}
