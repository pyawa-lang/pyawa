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

    /// **复数对象** ✓（task-30／P3-34）：`real` ＋ `imag`（**一份新引用** ✓）。
    ///
    /// 类型是登记表里那个**真正的 `complex`** ✓（`type(1j) is complex` 面成立 ✓），
    /// 载荷为 [`crate::builtin_objects::ComplexObject`]。`task-29` 的 `LOAD_CONST` 实例化
    /// （字面量／常量）与 `task-31` 的构造器都调这里 ✓。
    pub fn new_complex(&self, real: f64, imag: f64) -> NonNull<Header> {
        let complex_type = self
            .type_named("complex")
            .expect("complex 在引导期已登记（OM-13）");
        self.alloc(crate::builtin_objects::ComplexObject::new(
            complex_type,
            real,
            imag,
        ))
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
        // **P3-38 S3e-3**：`tb_lineno`／`tb_lasti` 从**抛出处的帧**算真值（先前恒 0 ✗ ——
        // `traceback` 格式化与官方 `test_raise`／`test_dis` 都读它们）。`tb_lasti` 是
        // **字节偏移**（码元 × 2，参照实测 `tb_lasti == 14` 对 7 码元 ✓）。
        let (lineno_value, lasti_value) = {
            // SAFETY: `new_traceback` 的契约是 frame 为存活帧对象。
            let frame_ref = unsafe { &*frame.as_ptr().cast::<crate::frame::Frame>() };
            let instruction_pointer = frame_ref.instruction_pointer();
            // SAFETY: code 由帧持有，存活。
            let code_ref = frame_ref
                .code()
                .map(|code| unsafe { &*code.as_ptr().cast::<crate::CodeObject>() });
            let lineno = code_ref
                .and_then(|code| crate::frame::line_at(code, instruction_pointer))
                .unwrap_or(0) as i64;
            // **口径说明（P3-38 S3e-3）**：本层位置表按**每条指令**一条（`BC-18` 夹具），
            // 参照 `co_positions()` 按**每码元**一条（含 `CACHE`）。`traceback` 用
            // `tb_lasti // 2` 当下标 ⇒ 落在 `CACHE` 槽时本层表里没有对应项；此时给 **0**
            // （**如实**表示"位置表口径没有这一槽"，不伪造位置），保证格式化不越界。
            // 编译器口径统一后这里自然恢复真偏移 ✓。
            let lasti = match code_ref {
                Some(code) if instruction_pointer < code.positions().len() => {
                    (instruction_pointer * 2) as i64
                }
                _ => 0,
            };
            (lineno, lasti)
        };
        let lineno = self.new_int(lineno_value);
        // **尽力而为** ✓（异常／追踪对象没有实例字典时如实不挂 ✓）。
        let _ = self.set_attribute_value(object, "tb_lineno", lineno);
        unsafe { self.release_object(lineno.as_ptr()) };
        let lasti = self.new_int(lasti_value);
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

    /// 造一个 **WTF-8** 形态的 `str`（P3-38 S2a；`None` ＝ 非法 WTF-8）——**新引用**。
    ///
    /// - `bytes` 必须是合法 WTF-8（[`crate::wtf8::validate`] ✓）；非法 ⇒ `None`
    ///   （**不**按有损转换静默收下 ✓）。
    /// - **合法 UTF-8** 的字节走 UTF-8 表示（零行为改动：`value()` 视图仍可用 ✓）；
    ///   只有含孤立代理这类才进 WTF-8 表示 ✓（`StrObject::new_wtf8` 里分派）。
    /// - 空串照 `new_str` 走 **`OM-23` 单例** ✓。
    pub fn new_str_wtf8(&self, bytes: &[u8]) -> Option<NonNull<Header>> {
        if bytes.is_empty() {
            let empty = self.singletons().empty_str();
            // SAFETY: 单例由实例持有，存活。
            unsafe { self.incref_object(empty.as_ptr()) };
            return Some(empty);
        }
        let object = StrObject::new_wtf8(self.singletons().str_type(), bytes.to_vec())?;
        Some(self.alloc(object).into_raw().cast::<Header>())
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

impl Instance {
    /// **OM-14**：把**自定义载荷**（宿主类型）放进本实例成为对象——**新引用**。
    ///
    /// 载荷类型必须满足：`size_of::<T>()` 与该类型的 `instance_size` 一致（**OM-5**：头部在
    /// 第一个字段），且它自己的 `dealloc` 槽要与这里的布局相配。宿主类型走这条。
    pub fn alloc_payload<T: crate::PyObject>(&self, payload: T) -> NonNull<T> {
        self.alloc(payload).into_raw()
    }

    // ---- 容器载荷的**安全**面（`pyawa-stdlib` 是 `forbid(unsafe_code)`，它只能走这些）----

    /// 往 `dict` 里按**字符串**键写一个值（**接管** `value` 的引用，`OM-16`）。
    ///
    /// 键已存在则替换（旧值由这里释放）。给 stdlib 建模块用。
    /// **口径（第 275 轮按用户裁定改为"借用" ✓）**：`dict_set` **自己**为字典那一份 `incref` ✓
    /// —— 与 CPython 的 `PyDict_SetItem` 一致 ✓。**调用方不必**先 `retain` ✓，也**不必**交出所有权 ✓
    /// （先前是"接管一份引用" ✗ ⇒ 每个"把查找结果直接交给字典"的站点都得自己记得 `retain` ✗
    ///  ⇒ 实测同类站点 100+ 处、已漏出至少两处 ✗ ⇒ `MS-25` 的悬垂条目就是这么来的 ✓）。
    /// **僵尸写探测**（第 88 轮）：要写的对象**已经在释放登记里** ⇒ 旧主人还在写 ✓ ⇒ 当场报出
    /// **是谁释放的、哪个 Python 现场** ✓（`PYAWA_ZOMBIE_TRACE=1` 才开 ✓）。
    pub(crate) fn zombie_probe(&self, object: NonNull<Header>) {
        if !self.zombie_trace.get() {
            return;
        }
        if let Some((name, site)) = self
            .freed_sites
            .borrow()
            .get(&(object.as_ptr() as usize))
            .cloned()
        {
            panic!(
                // **连 Rust 回溯一起打** ✓（第 91 轮）：光有 Python 现场还不足以指认**谁把 `str`
                // 传进来** ✗（类型检查也没用 ✗ —— 那块内存的表头自己已经烂了 ✓，`type_of` 读不出真类型 ✓）。
                // 回溯里就会出现"哪条 Rust 路径调了 `dict_set`" ✓。
                "[僵尸写] 正在写一个**已释放**的对象 {:#x}（{name}；释放于 {site}）✗；写入方：{}\n{}",
                object.as_ptr() as usize,
                self.current_site(),
                std::backtrace::Backtrace::force_capture()
            );
        }
    }

    /// 直接分配一个 `int` 对象（**不查单例表**）。
    ///
    /// `new_int` 与 `new_int_value` **不能互相调**（非单例值会来回递归到爆栈——
    /// 第 200 轮实测踩过：`300` 一路 `new_int` ↔ `new_int_value`）。
    pub(crate) fn alloc_int(&self, value: IntValue) -> NonNull<Header> {
        let int_type = self.singletons().int_type();
        self.alloc(IntObject::new(int_type, value))
            .into_raw()
            .cast::<Header>()
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
            // **开关**（第 93 轮，`PYAWA_NO_GC=1` 才跳过 ✓）：用来判定"刚造好就是垃圾"是不是
            // **这次回收**干的 ✓ —— 若关掉之后崩就没了 ✓，那病灶就在"回收与刚分配对象"的时序上 ✓。
            if crate::diag::flag_off("PYAWA_NO_GC") {
                self.collect();
            }
        }

        Owned::new(ptr, self)
    }

    /// 把一个已构造好的对象交给本实例托管（记账 ＋ 入链 ＋ 标 `GC_TRACKED`），
    /// 返回它的指针；**引用由实例自己持有**。
    ///
    /// 引导期（`Instance::new` 造单例时）用不了 `Owned`——那需要先借出 `&Instance`。
    pub(crate) fn adopt<T: PyObject>(&self, value: T) -> NonNull<T> {
        let ty = value.header().ty();
        let size = core::mem::size_of::<T>();
        debug_assert_eq!(
            size,
            // SAFETY: ty 由某个实例的类型注册表持有（OM-15），在实例存活期间有效；
            // 这里在 debug 下用它校验类型元数据与 Rust 布局一致。
            unsafe { ty.as_ref() }.instance_size,
            "类型的 instance_size 与 Rust 布局不一致"
        );

        // **哨兵方案已撤** ✗（第 241 轮）：`adopt` 与"宏生成的 `dealloc`"**不是一一对应**的 ✗ ——
        // `Box` 分配出来的**类型对象**也走同一个宏 ✓ ⇒ 在那里读"尾部魔数"读的是**邻居内存** ✗ ⇒ **假阳性** ✓
        //（实测：`type` 对象 size=248 时报越界 ✗，回溯直指 `TypeObject::dealloc` ✓）。
        // 要真做，得先让每块内存**带"有没有哨兵"的出处位** ✓ ⇒ 随后再做 ✓。
        let ptr = NonNull::from(Box::leak(Box::new(value)));
        let header = ptr.cast::<Header>();

        // **OM-12**：可成环的类型必须标记 GC_TRACKED。本层用"是否提供 traverse 槽位"判定；
        // 类型对象自身也会成环（bases／dict），但它的 traverse／clear 待接线后再补标记。
        let tracked = unsafe { ty.as_ref() }.slots.traverse.is_some();
        if tracked {
            // SAFETY: header 指向刚刚分配、尚未交给其他代码的对象。
            unsafe { header.as_ref() }.set_flag(flags::GC_TRACKED);
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
        ptr
    }

    pub(crate) fn alloc_type_raw(
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
            Cell::new(None),
            Cell::new(None),
        );
        let ptr = NonNull::from(Box::leak(Box::new(object)));
        // 类型对象也走同一本账（OM-3），但由注册表持有：不进 `live`，销毁时统一释放（OM-2）。
        self.bytes_allocated
            .set(self.bytes_allocated.get() + core::mem::size_of::<TypeObject>());
        self.types.borrow_mut().push(ptr);
        ptr
    }
}
