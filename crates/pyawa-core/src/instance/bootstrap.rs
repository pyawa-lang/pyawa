//! `Instance` 的引导与注册表访问域（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    pub fn new() -> Self {
        let this = Self {
            bytes_allocated: Cell::new(0),
            current_frame: core::cell::Cell::new(None),
            not_implemented_singleton: core::cell::Cell::new(None),
            live: RefCell::new(HashSet::new()),
            watch: Cell::new(0),
            freed_sites: RefCell::new(std::collections::HashMap::new()),
            zombie_trace: Cell::new(flag("PYAWA_ZOMBIE_TRACE")),
            quarantine: RefCell::new(Vec::new()),
            types: RefCell::new(Vec::new()),
            metatype: Cell::new(None),
            capabilities: RefCell::new([CapabilityEntry::default(); pyawa_capabilities::DOMAIN_COUNT]),
            capability_table: RefCell::new(super::capability_table::CapabilityTable::new()),
            path_cwd: RefCell::new(None),
            modules: RefCell::new(None),
            singletons: OnceCell::new(),
            current_globals: Cell::new(None),
            exception_state: RefCell::new(Vec::new()),
            build_class: Cell::new(None),
            pending_exception: Cell::new(None),
            builtins: Cell::new(None),
            platform_constants: RefCell::new(Vec::new()),
            int_max_str_digits: Cell::new(INT_MAX_STR_DIGITS_DEFAULT),
            pending: RefCell::new(Vec::new()),
            draining: Cell::new(false),
            gc_head: Cell::new(ptr::null_mut()),
            gc_count: Cell::new(0),
            call_depth: Cell::new(0),
            recursion_limit: Cell::new(crate::instance::MAX_CALL_DEPTH),
            gc_threshold: Cell::new(DEFAULT_GC_THRESHOLD),
            gc_alloc_count: Cell::new(0),
            gc_frozen: RefCell::new(HashSet::new()),
            gc_running: Cell::new(false),
            repr_guard: RefCell::new(Vec::new()),
            interrupted: core::sync::atomic::AtomicBool::new(false),
        };

        // 元类型自指：类型对象的类型就是它自己（与 CPython 的 `PyType_Type` 同理）。
        // 此处 `ty` 先落在 `NonNull::dangling()` 上，写完自指后立即成为正常类型对象。
        let metatype = this.alloc_type_raw(
            "type",
            core::mem::size_of::<TypeObject>(),
            // **注意** ✗（第 240 轮查证后**保留原样** ✓）：类型对象由 `alloc_type_raw` 用
            // **`Box::leak(Box::new(TypeObject))`** 分配 ✓ ⇒ 释放就该走宏生成的 `Box::from_raw` ✓
            //（**同一 layout** ✓）。我一度把它改成按 `instance_size` 释放 ✗ ⇒ 反而**制造**了不一致 ✗。
            // 结论 ✓：**这对本来就是对的** ✓ —— tcache 那条另有其因 ✓，继续查 ✓。
            Slots::new(TypeObject::dealloc)
                .with_repr(crate::builtin_objects::type_repr)
                .with_call(crate::builtin_objects::type_call)
                // **类型对象自己的遍历** ✓（第 198 轮**真 bug 修复** ✗：先前元类型**没有** T/C ⇒
                // 类型的**命名空间字典**不被遍历 ⇒ 只在类体里被引用的函数／类被判不可达 ⇒ 被 `free` ✗
                // ⇒ 活对象的内存被后续分配重写 ⇒ glibc 迟到地报 `corrupted double-linked list` ✗）。
                .with_traverse(crate::type_object::type_traverse)
                .with_clear(crate::type_object::type_clear),
        );
        // SAFETY: metatype 刚分配、尚未交给任何其他代码；写入自指后它才被引用。
        unsafe { metatype.as_ref().header.set_ty(metatype) };
        this.metatype.set(Some(metatype));

        this.bootstrap_builtin_types();
        this
    }

    /// 内建名字空间（**借用**；没装就是 `None`）。
    pub fn builtins(&self) -> Option<NonNull<Header>> {
        self.builtins.get()
    }

    /// 模块表（**借用**）。
    pub fn modules(&self) -> Option<NonNull<Header>> {
        *self.modules.borrow()
    }

    /// 元类型：类型对象自身的类型。
    pub fn metatype(&self) -> NonNull<TypeObject> {
        self.metatype
            .get()
            .expect("元类型在 Instance::new 中引导，必然存在")
    }

    /// **`LOAD_BUILD_CLASS`** 压的那个内建（`__build_class__`；**借用**）。
    ///
    /// 参照实现从 `builtins` 取它；Pyawa 还没有 `builtins` 模块（`P3-14`），故先按实例存一个。
    pub fn build_class(&self) -> Option<NonNull<Header>> {
        self.build_class.get()
    }

    /// 某个域的**不透明实现指针**（`AB-32`：本层只存不解释）。
    pub fn capability(&self, domain: usize) -> Option<*const core::ffi::c_void> {
        let slots = self.capabilities.borrow();
        let slot = slots.get(domain)?;
        (!slot.implementation.is_null()).then_some(slot.implementation)
    }

    /// **`BC-4`**：造一份与 `code` 同内容、但 `co_qualname` 换掉的 **code 副本**（**新引用**）。
    ///
    /// 用途：参照实现里方法的 `co_qualname`（`C.m`）是**编译器**写死的；本层编译器还没有类体，
    /// 所以由**类创建钩子**在建类时把 `m` 改成 `C.m`——那时就得换一份 code（原 code 可能与别处共享，
    /// 不能就地改）。常量表**逐项新增引用**（新 code 自己持有一份）。
    pub fn code_with_qualname(
        &self,
        code: &crate::CodeObject,
        qualname: String,
    ) -> NonNull<Header> {
        let code_type = self
            .type_named("CodeObject")
            .expect("CodeObject 在引导期已登记");
        let mut names = Vec::new();
        let mut index = 0usize;
        while let Some(name) = code.name_at(index) {
            names.push(name.to_owned());
            index += 1;
        }
        let varnames: Vec<String> = (0..code.nlocals())
            .filter_map(|slot| code.varname(slot).map(str::to_owned))
            .collect();
        let mut consts = Vec::with_capacity(code.const_count());
        for position in 0..code.const_count() {
            if let Some(constant) = code.constant(position) {
                // 新 code 的常量表要自己那份引用（它的 clear／dealloc 会释放）
                // SAFETY: 常量由原 code 持有，存活。
                unsafe { self.incref_object(constant.as_ptr()) };
                consts.push(Some(constant));
            } else {
                consts.push(None);
            }
        }
        self.alloc(crate::CodeObject::new(
            code_type,
            code.name(),
            qualname,
            code.filename().to_owned(),
            code.firstlineno(),
            code.stacksize(),
            code.nlocals(),
            code.argcount(),
            code.posonlyargcount(),
            code.kwonlyargcount(),
            code.flags(),
            varnames,
            names,
            code.cellvars().to_vec(),
            code.freevars().to_vec(),
            code.code().to_vec(),
            code.exceptiontable().to_vec(),
            consts,
            code.positions().to_vec(),
        ))
        .into_raw()
        .cast::<Header>()
    }
}

impl Instance {
    /// **OM-23**：按实例创建单例（`None`／`True`／`False`／小整数）。
    ///
    /// 引导期还不能借出 `&Instance` 造 `Owned` 守卫，所以走 [`Instance::adopt`]：
    /// 引用由实例自己持有，随实例销毁一起释放（`OM-2`）。
    /// 按 `TS-41` 的**探测表**登记一个类型的基类（MRO 由 C3 算）。
    ///
    /// 层次**不手写**：基类名字取自 `crate::builtin_types`。基类必须**先**注册（表里的顺序
    /// 是参照实现的顺序，但引导期按依赖手工排序）。
    pub(crate) fn register_from_table(&self, ty: NonNull<TypeObject>) {
        // SAFETY: ty 由本实例的注册表持有。
        let name = unsafe { ty.as_ref() }.name();
        let entry = crate::builtin_types::builtin_type(name)
            .unwrap_or_else(|| panic!("TS-41：{name} 必须在探测表里"));
        let bases: Vec<NonNull<TypeObject>> = entry
            .bases
            .iter()
            .map(|base| {
                self.type_named(base).unwrap_or_else(|| {
                    panic!("TS-41：{name} 的基类 {base} 必须先于它注册")
                })
            })
            .collect();
        assert!(
            self.register_bases(ty, bases).is_some(),
            "OM-13：{name} 的 MRO 应当可线性化"
        );
    }

    /// **TS-41**／**TS-42** 的第一阶梯＋**容器的 M2 起步**：`object`／`type`／`NoneType`／
    /// `bool`／`int`／`float`／`str`／`tuple`／`list`／`dict`／`set`。
    ///
    /// 层次**不手写**：基类关系取自 `crate::builtin_types` 的探测表（`TS-41`），MRO 由
    /// **C3**（`OM-13`）算出。**禁止**为了省事直接写一份 MRO。
    pub(crate) fn bootstrap_builtin_types(&self) {
        self.bootstrap_core_scalars();
        self.bootstrap_containers_iters();
        self.bootstrap_context_thread();
        self.bootstrap_function_family();
        self.bootstrap_errors_cells();
        self.bootstrap_frame_singletons();
        self.bootstrap_iterator_registration();
        self.bootstrap_type_natives();
        self.bootstrap_build_class();
    }

    /// 引导第 1 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_core_scalars(&self) {
        let object_type = self.alloc_type_raw(
            "object",
            core::mem::size_of::<PlainObject>(),
            Slots::new(PlainObject::dealloc).with_new(crate::builtin_objects::plain_new),
        );
        self.register_from_table(object_type);

        // **`object.__hash__`**（第 309 轮）：本层先前**没有**它 ✗ ⇒ `C.__hash__`／`ref.__hash__`
        // 这类"在**类型对象**上取 dunder"的属性访问当场报 `AttributeError: 'type' object has no
        // attribute '__hash__'` ✗（`Lib/weakref.py:89` 的 `__hash__ = ref.__hash__` 正栽在这 ✓，
        // 那一族 **31** 个模块 ✓）。这里按"**身份哈希**"接一个 ✓（同一对象在同一进程里恒定 ✓；
        // 与参照各类型的哈希值**不一致** ✗ —— 如实登记 ✓：本层的字典查键走 `values_equal` ✓，
        // 不靠这个值 ✓）。
        // 元类型（`type`）也是对象；表里 `type` 的基类就是 `object`
        let metatype = self.metatype.get().expect("元类型在 Instance::new 里已引导");
        self.register_from_table(metatype);

        let _none_type = self.alloc_type_raw(
            "NoneType",
            core::mem::size_of::<NoneObject>(),
            Slots::new(NoneObject::dealloc)
                .with_repr(crate::builtin_objects::none_repr)
                .with_str(crate::builtin_objects::none_repr),
        );

        // **`...` 的类型**（第 177 轮）：名字取自 `TS-41` 探测表 ✓（表里本来就有 `ellipsis` ✓）。
        // 载荷借 `NoneObject`（**空载荷** ✓）—— 只作单例载体 ✓，语义由类型名承担 ✓。
        let ellipsis_type = self.alloc_type_raw(
            "ellipsis",
            core::mem::size_of::<NoneObject>(),
            Slots::new(NoneObject::dealloc),
        );
        assert!(
            self.register_bases(ellipsis_type, vec![object_type]).is_some(),
            "ellipsis 的基类是 object"
        );
        let _bool_type = self.alloc_type_raw(
            "bool",
            core::mem::size_of::<BoolObject>(),
            Slots::new(BoolObject::dealloc)
                .with_new(crate::builtin_objects::bool_new)
                .with_repr(crate::builtin_objects::bool_repr)
                .with_str(crate::builtin_objects::bool_repr),
        );
        let _int_type = self.alloc_type_raw(
            "int",
            core::mem::size_of::<IntObject>(),
            Slots::new(IntObject::dealloc)
                .with_new(crate::builtin::int::int_new)
                .with_repr(crate::builtin::int::int_repr)
                .with_str(crate::builtin::int::int_repr)
                // **方法面**（第 195 轮）：`to_bytes`／`bit_length` ✓。
                .with_getattr(crate::builtin::int::int_getattr),
        );
        let _float_type = self.alloc_type_raw(
            "float",
            core::mem::size_of::<FloatObject>(),
            Slots::new(FloatObject::dealloc)
                .with_new(crate::builtin::float::float_new)
                .with_repr(crate::builtin::float::float_repr)
                .with_str(crate::builtin::float::float_repr)
                // **方法面**（第 352 轮）：`is_integer`／`as_integer_ratio` ✓。
                .with_getattr(crate::builtin::float::float_getattr),
        );
        let _str_type = self.alloc_type_raw(
            "str",
            core::mem::size_of::<StrObject>(),
            Slots::new(StrObject::dealloc)
                .with_new(crate::builtin_objects::str_new)
                .with_repr(crate::builtin_objects::str_repr)
                .with_str(crate::builtin_objects::str_str)
                // **方法面**（第 143 轮）：`Lib/` 里每个文件都在用字符串方法 ✓
                .with_getattr(crate::builtin_objects::str_getattr),
        );
    }

    /// 引导第 2 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_containers_iters(&self) {
        let object_type = self.type_named("object").expect("object 已登记");

        // **`slice`**（`P1-12` 的"索引／切片"；`TS-42` 把它排 M3+，但切片是这一档的判据）
        // **`classmethod`**（第 158 轮）：给 `Lib/abc.py` 的 `class abstractclassmethod(classmethod)` 用 ✓。
        // **`staticmethod`**（第 161 轮）：与 `classmethod` 同模式 ✓（`Lib/abc.py` 要它 ✓）。

        // **`_weakref` 的 `ref` 类型**（第 173 轮）：名字避开规格表 ✓（第 161 轮那种撞名教训 ✓）。


        let weakref_type = self.alloc_type_raw(


            "weakref",


            core::mem::size_of::<crate::builtin_objects::WeakRefObject>(),


            crate::builtin_objects::WeakRefObject::slots()
                // **构造器放 core** ✓（`builtin_objects` 是私有模块 ✗，stdlib 不能自己分配 ✓；
                // 与 `slice`／`classmethod` 同款：类型自身的 `new` 槽 ✓）。
                .with_new(crate::builtin_objects::weakref_new),


        );


        assert!(


            self.register_bases(weakref_type, vec![object_type]).is_some(),


            "weakref 的基类是 object"


        );



        let staticmethod_type = self.alloc_type_raw(

            "staticmethod",

            core::mem::size_of::<crate::builtin_objects::StaticMethodObject>(),

            crate::builtin_objects::StaticMethodObject::slots()
                .with_getattr(crate::builtin_objects::staticmethod_getattr)

                .with_new(crate::builtin_objects::staticmethod_new),

        );

        assert!(

            self.register_bases(staticmethod_type, vec![object_type]).is_some(),

            "staticmethod 的基类是 object"

        );


        // **`property`**（第 161 轮）：同模式第三份 ✓（`Lib/abc.py` 要它 ✓）。



        let property_type = self.alloc_type_raw(



            "property",



            core::mem::size_of::<crate::builtin_objects::PropertyObject>(),



            crate::builtin_objects::PropertyObject::slots()



                .with_new(crate::builtin::property::property_new)
                // **方法面**（第 186 轮）：`fget`／`fset`／`fdel` 取值 ＋ `getter`／`setter`／`deleter` ✓。
                .with_getattr(crate::builtin::property::property_getattr),



        );



        assert!(



            self.register_bases(property_type, vec![object_type]).is_some(),



            "property 的基类是 object"



        );




        let _classmethod_type = self.alloc_type_raw(
            "classmethod",
            core::mem::size_of::<crate::builtin_objects::ClassMethodObject>(),
            crate::builtin_objects::ClassMethodObject::slots()
                .with_new(crate::builtin_objects::classmethod_new)
                // **`__func__`／`__wrapped__`** ✓（第 346 轮）：上限榜上 39 个模块
                // `AttributeError: 'classmethod' object has no attribute '__func__'` ✓。
                .with_getattr(crate::builtin_objects::classmethod_getattr),
        );

        let _slice_type = self.alloc_type_raw(
            "slice",
            core::mem::size_of::<SliceObject>(),
            crate::builtin_objects::SliceObject::slots()
                .with_new(crate::builtin_objects::slice_new)
                .with_repr(crate::builtin_objects::slice_repr)
                // **属性面**（第 151 轮）：`slice(1, 3).start` 等 ✓
                .with_getattr(crate::builtin_objects::slice_getattr),
        );

        // **`bytes`**（`P1-12`／`TS-42` 的"M2 之后、M3 之前"档：`marshal` 与 `co_code` 要它）
        let _bytes_type = self.alloc_type_raw(
            "bytes",
            core::mem::size_of::<BytesObject>(),
            crate::builtin_objects::BytesObject::slots()
                .with_new(crate::builtin_objects::bytes_new)
                .with_repr(crate::builtin_objects::bytes_repr)
                .with_str(crate::builtin_objects::bytes_str)
                .with_getattr(crate::builtin_objects::bytes_getattr),
        );

        // **`bytearray`** ✓（第 226 轮，**M3 的这件** ✓）：可调用 ✓、可迭代 ✓ —— 与 `bytes` **共用载荷与槽** ✓
        //（**类型对象**不同 ✓ ⇒ `type(iter(bytearray()))` 给 **`bytearray_iterator`** ✓，与 `bytes_iterator` **分开** ✓）。
        // **如实记** ✗：目前只做**空** `bytearray()` ✓（可变字节面随后接 ✓）。
        // **`bytearray` 有自己的载荷** ✓（第 642 轮 ✗ 修）：`BytearrayObject`（`RefCell<Vec<u8>>` ✓ 可变的
        // 字节串 ✓）—— 先前借 `BytesObject`（`Vec<u8>` 不可变 ✗）⇒ `re._compiler` 的
        // `data[0:0] = …`／`mapping[i] = …` 全部报"下标赋值只接线了 list／dict" ✗。
        let _bytearray_type = self.alloc_type_raw(
            "bytearray",
            core::mem::size_of::<crate::builtin_objects::BytearrayObject>(),
            crate::builtin_objects::BytearrayObject::slots()
                .with_new(crate::builtin_objects::bytearray_new)
                .with_repr(crate::builtin_objects::bytearray_repr)
                .with_str(crate::builtin_objects::bytearray_repr)
                .with_getattr(crate::builtin_objects::bytearray_getattr),
        );

        // **`memoryview` 最小面** ✓（task-22 ✓）：构造 ＋ `cast`／`tobytes`／`tolist`／`len`／索引读写 ✓
        // （方法面由 `memoryview_new` 惰性挂 ✓）—— **载荷不新增** ✗：视图就是 `AttributeObject`
        // ＋ 实例字典（`_source`／`_format`／`_itemsize`／`_offset`／`_nbytes` ✓），**共享底层** ✓。
        // 与 `bytearray` **同款**：在 `builtin_types.rs` 里只是 `Ladder::Later` 占位 ✓，这里带槽建出来 ✓。
        let memoryview_type = self.alloc_type_raw(
            "memoryview",
            core::mem::size_of::<crate::builtin_objects::AttributeObject>(),
            crate::builtin_objects::AttributeObject::slots()
                .with_new(crate::builtin_objects::memoryview_call_new)
                .with_getattr(crate::builtin_objects::memoryview_getattr),
        );
        let _ = memoryview_type;

        // **`_typing` 轮 1 的两个类型** ✓（task-24 ✓）：`TypeVar`（`new` 槽 ＋ 属性面 ✓）与
        // `NoDefaultType`（回单例 ✓）—— 与 `memoryview`／`bytearray` 同款：`builtin_types.rs` 里
        // 只是 `Ladder::Later` 占位 ✓，这里带槽建出来 ✓。
        crate::builtin::typing::build_typing_types(self);
    }

    /// 引导第 3 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_context_thread(&self) {

        // **`list_reverseiterator`** ✓（第 227 轮）：与其它内建迭代器同构 ✓（`IteratorObject` ＋ 那套槽 ✓）；
        // **方向由类型决定** ✓ ⇒ 不必给载荷加字段 ✓。
        let _list_reverseiterator_type = self.alloc_type_raw(
            "list_reverseiterator",
            core::mem::size_of::<IteratorObject>(),
            IteratorObject::slots(),
        );

        // **`reversed`** ✓（第 727 轮）：`reversed(序列)` 的**通用惰性迭代器**（序列协议：
        // `__len__` ＋ `__getitem__` ✓）；`IteratorObject` 载荷 ＋ 类型区分方向 ✓。
        let _reversed_type = self.alloc_type_raw(
            "reversed",
            core::mem::size_of::<IteratorObject>(),
            IteratorObject::slots(),
        );

        // **`longrange_iterator`** ✓（第 228 轮）：`range()` 的**大整数上限**那一支；载荷与 `islice` 同构 ✓
        //（我们的 `range` 本来就是 `islice(count(…))` ✓），只是**类型不同** ✓ —— 参照也分成两个名字 ✓。
        let _longrange_iterator_type = self.alloc_type_raw(
            "longrange_iterator",
            core::mem::size_of::<crate::builtin_objects::ItStateObject>(),
            crate::builtin_objects::ItStateObject::slots(),
        );

        // **`super`** ✓（第 233 轮）：**零参**形式 ✓ —— 载荷用 `AttributeObject`（存 `__thisclass__`／`__self__` ✓），
        // 查表走 `attribute_lookup` 里的**专用分支** ✓（要在那里才能造出"绑定方法" ✓）。
        let _super_type = self.alloc_type_raw(
            "super",
            core::mem::size_of::<crate::builtin_objects::AttributeObject>(),
            crate::builtin_objects::AttributeObject::slots(),
        );

        // **`zip`** ✓（第 229 轮）：载荷与 `zip_longest` 同构 ✓ —— **取最短** ✓。
        // 表里**早有 `zip` 这个名字** ✓（`builtin_types.rs` ✓）⇒ 不必加表条目 ✓。
        let _zip_type = self.alloc_type_raw(
            "zip",
            core::mem::size_of::<crate::builtin_objects::ItStateObject>(),
            crate::builtin_objects::ItStateObject::slots(),
        );

        // **`range_iterator`** ✓（第 228 轮）：`range()` 的**常规**那一支 ✓（参照的名字 ✓）。
        let _range_iterator_type = self.alloc_type_raw(
            "range_iterator",
            core::mem::size_of::<crate::builtin_objects::ItStateObject>(),
            crate::builtin_objects::ItStateObject::slots(),
        );

        // 容器：`TS-42` 的 M2 起步（层次取自探测表）
        let _tuple_type = self.alloc_type_raw(
            "tuple",
            core::mem::size_of::<TupleObject>(),
            TupleObject::slots()
                .with_new(crate::builtin_objects::tuple_new)
                .with_repr(crate::builtin_objects::tuple_repr)
                // **方法面** ✓（task-26 ②）：`count`／`index`（实例路径绑 self ✓）。
                .with_getattr(crate::builtin_objects::tuple_getattr)
        );
        let _list_type = self.alloc_type_raw(
            "list",
            core::mem::size_of::<ListObject>(),
            ListObject::slots()
                .with_new(crate::builtin::list::list_new)
                .with_repr(crate::builtin::list::list_repr)
                // `traverse`／`clear` **不在这里挂** ✓ —— `ListObject::slots()`（`builtin_objects.rs`
                // 里那个 impl ✓）已经含了 ✓，**一处真相** ✓；且静态检查 `gc_field_coverage` 只读
                // 那个 impl 块 ✓（在这里重复挂一次会让真相分叉 ✗）。
                // **方法面**（第 143 轮）
                .with_getattr(crate::builtin::list::list_getattr),
        );
        let _dict_type = self.alloc_type_raw(
            "dict",
            core::mem::size_of::<DictObject>(),
            DictObject::slots()
                .with_new(crate::builtin_objects::dict_new)
                .with_repr(crate::builtin_objects::dict_repr)
                // **方法面**（第 145 轮）
                .with_getattr(crate::builtin_objects::dict_getattr),
        );
        // **`memoryview`** ✓（第 187 轮）：被 `_collections_abc.py:1062` 的 `Sequence.register(memoryview)` 用到 ✓
        // ⇒ 与 `range`／`frozenset` 同款：**名字必须是类型** ✓。
        // **如实说** ✗：本层**还没有**内存视图语义 ✓ ⇒ 载荷借 `BytesObject` ✓ 且**不挂构造槽** ✗
        //（照参照造出真正的 `memoryview` 随后补 ✓）。
        let mut memoryview_slots = crate::builtin_objects::BytesObject::slots();
        memoryview_slots.new = None;
        let _memoryview_type = self.alloc_type_raw(
            "memoryview",
            core::mem::size_of::<crate::builtin_objects::BytesObject>(),
            memoryview_slots,
        );

        // **`range`** ✓（第 237 轮）：参照里它是**类型** ✓（`Range.register(range)` 一族 ✓）⇒ 本层补上它的**类型对象** ✓
        //（构造槽 `range_new` 见 core ✓）。**如实说** ✗：`range(n)` 给出的仍是**迭代器** ✓ ⇒ `type(range(n))` 现在
        // 给 `range_iterator` ✗（参照给 `range` ✓）—— 既有偏差 ✓，本轮**不动**它 ✓（只让**名字**成为类型 ✓）。
        // **`range` 是真对象** ✓（第 627 轮 ✗ 修）：载荷 `RangeObject`（三整数 ＋ 大整数上限位 ✓），
        // 槽挂**构造** ✓／`repr` ✓／方法面（`__len__`／`__getitem__`／`__iter__` ✓）—— 先前 `range()`
        // 直接造 `islice(count(...))` 再改型成 `range_iterator` ✗ ⇒ `type(range(5))` 是迭代器 ✓、
        // `range(5)[1:3]` 报"切不了 range_iterator" ✗（`Lib/re` 就断在这 ✓）。
        let _range_type = self.alloc_type_raw(
            "range",
            core::mem::size_of::<crate::builtin_objects::RangeObject>(),
            crate::builtin_objects::RangeObject::slots()
                .with_new(crate::builtin_objects::range_new)
                .with_repr(crate::builtin_objects::range_repr)
                .with_getattr(crate::builtin_objects::range_getattr),
        );

        // **`complex`** ✓（task-30／P3-34）：真正的复数载荷（`ComplexObject{real, imag}` ✓）＋
        // 构造／表示／属性槽 ✓。第 640 轮的占位（借 `BytesObject`、无构造槽 ✗）已替换——
        // `task-29` 的 `LOAD_CONST` 实例化与 `task-31` 的 stdlib 构造器都按真类型走 ✓。
        let complex_type = self.alloc_type_raw(
            "complex",
            core::mem::size_of::<crate::builtin_objects::ComplexObject>(),
            crate::builtin_objects::ComplexObject::slots(),
        );
        // **层次要登记** ✓（`OM-13`／`TS-41`：MRO 由 C3 算，禁止手写 ✓）：先前占位类型漏了这一步 ✗
        // ⇒ `complex.__mro__` 为空、`issubclass(complex, object)` 假、类型字典里的方法也查不到 ✗。
        assert!(
            self.register_bases(
                complex_type,
                vec![self.type_named("object").expect("object 已登记")],
            )
            .is_some(),
            "OM-13：`complex` 的基类是 `object`"
        );

        // **`frozenset`** ✓（第 236 轮）：与 `set` **同载荷同槽** ✓（`set_new` 收 **class** ⇒ 直接复用 ✓），
        // 只是**另一个类型对象** ✓（`abc.py:180` 要 `frozenset(abstracts)` ✓、
        // `_collections_abc.py:687` 要 `Set.register(frozenset)` ✓）。
        // **如实说** ✗：本层没有"不可变"这层语义 ✓（`frozenset` 的实例目前**仍可改** ✓，随后补 ✓）。
        let _frozenset_type = self.alloc_type_raw(
            "frozenset",
            core::mem::size_of::<SetObject>(),
            SetObject::slots()
                .with_new(crate::builtin::set::set_new)
                .with_repr(crate::builtin::set::set_repr)
                .with_getattr(crate::builtin::set::set_getattr),
        );

        let _set_type = self.alloc_type_raw(
            "set",
            core::mem::size_of::<SetObject>(),
            SetObject::slots()
                .with_new(crate::builtin::set::set_new)
                .with_repr(crate::builtin::set::set_repr)
                // **方法面**（第 146 轮）
                .with_getattr(crate::builtin::set::set_getattr),
        );

        // **`deque`**（第 331 轮）：`collections.deque` —— 上限榜上
        // `ImportError: cannot import name 'deque' from 'collections'` × 18 个模块的卡点 ✓。
        // 方法面在 `builtin::deque::deque_getattr` ✓；`__repr__`／`traverse`／`clear` 一并给 ✓。
        // **如实登记的未接面** ✗：迭代协议（`for x in deque(...)`）、下标、`__contains__`／`__eq__`、
        // `reverse` —— 随后补 ✓。
        let deque_type = self.alloc_type_raw(
            "deque",
            core::mem::size_of::<crate::builtin_objects::DequeObject>(),
            crate::builtin_objects::DequeObject::slots()
                .with_new(crate::builtin::deque::deque_new)
                .with_getattr(crate::builtin::deque::deque_getattr),
        );
        // **基类只有 `object`** ✓：`deque` 不在探测表（`TS-41` 那份表是"参照里的事实" ✓）
        // ⇒ 不能走 `register_from_table` ✗（它会 panic ✓），直接按 C3 登记一条边 ✓（MRO 仍由核心算 ✓）。
        let deque_base = self.type_named("object").expect("object 已登记");
        assert!(
            self.register_bases(deque_type, vec![deque_base]).is_some(),
            "OM-13：deque 的 MRO 应当可线性化"
        );
    }

    /// 引导第 4 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_function_family(&self) {

        // **`_contextvars` 的三个类型**（第 332 轮）：`ContextVar`／`Token`／`Context` ——
        // `Lib/contextvars.py` 只做 `from _contextvars import Context, ContextVar, Token, copy_context`
        // ＋ `_collections_abc.Mapping.register(Context)` ✓ ⇒ `Context` 必须是**类型对象** ✓；
        // 上限榜上 `No module named '_contextvars'` 那一族（**49** 个模块）的卡点 ✓。
        // **如实登记的偏差** ✗：本层没有真正的上下文隔离（值存在变量自己身上 ✓），
        // `Context` 不承载独立状态 ✓ —— 单上下文的 `get`／`set`／`reset` 与参照一致 ✓。
        let context_var_type = self.alloc_type_raw(
            "ContextVar",
            core::mem::size_of::<crate::builtin_objects::ContextVarObject>(),
            crate::builtin_objects::ContextVarObject::slots()
                .with_new(crate::builtin::context::context_var_new)
                .with_getattr(crate::builtin::context::context_var_getattr),
        );
        let token_type = self.alloc_type_raw(
            "Token",
            core::mem::size_of::<crate::builtin_objects::TokenObject>(),
            crate::builtin_objects::TokenObject::slots()
                .with_getattr(crate::builtin_objects::token_getattr),
        );
        let context_type = self.alloc_type_raw(
            "Context",
            core::mem::size_of::<crate::builtin_objects::ContextObject>(),
            crate::builtin_objects::ContextObject::slots()
                .with_new(crate::builtin::context::context_new)
                .with_getattr(crate::builtin::context::context_getattr),
        );
        for (extra, label) in [
            (context_var_type, "ContextVar"),
            (token_type, "Token"),
            (context_type, "Context"),
        ] {
            let base = self.type_named("object").expect("object 已登记");
            assert!(
                self.register_bases(extra, vec![base]).is_some(),
                "OM-13：{label} 的 MRO 应当可线性化"
            );
        }

        // **`_thread._ThreadHandle`**（第 333 轮）：`Lib/threading.py` 在**模块级**就取这个名字 ✓
        // ⇒ 必须是个类型对象 ✓（本层没有真线程，句柄是个**类型占位** ✓，如实登记 ✓）。
        let thread_handle_type = self.alloc_type_raw(
            // 名字照参照 ✓（实测 `_thread._ThreadHandle.__name__ == '_ThreadHandle'` ✓）
            "_ThreadHandle",
            core::mem::size_of::<crate::builtin_objects::ThreadHandleObject>(),
            // **`_ThreadHandle()` 要能造** ✓（第 724 轮）：`Lib/threading.py` 的
            // `Thread.__init__` 里就有 `self._handle = _ThreadHandle()` ✓，而模块级 `_MainThread()`
            // 一跑就踩它 ✗（实测 `TypeError: cannot create '_ThreadHandle' instances` ✓）
            // ⇒ 补 `tp_new`（`plain_new` 按登记尺寸分配 ✓，与 `object`／`bool` 同一手法 ✓）。
            crate::builtin_objects::ThreadHandleObject::slots()
                .with_new(crate::builtin::thread::thread_handle_slot_new),
        );
        {
            let base = self.type_named("object").expect("object 已登记");
            assert!(
                self.register_bases(thread_handle_type, vec![base]).is_some(),
                "OM-13：ThreadHandle 的 MRO 应当可线性化"
            );
        }
    }

    /// 引导第 5 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_errors_cells(&self) {
        let object_type = self.type_named("object").expect("object 已登记");

        // `function`：`TS-42` 的 M2（调用与返回族逼出来的）
        let function_type = self.alloc_type_raw(
            "function",
            core::mem::size_of::<FunctionObject>(),
            FunctionObject::slots()
                .with_repr(crate::builtin::function::function_repr)
                .with_getattr(crate::builtin::function::function_getattr),
        );
        // **函数也有 `__dict__`** ✓（第 194 轮：CPython 里函数可挂任意属性 ✓ —— importlib 一带真的会设 ✓，
        // 第 193 轮那条 `'function' object has no attribute '__name__' and no __dict__ ...` 就是它缺 ✓）。
        // 用**外部字典**那一档 ✓：载荷保持 `FunctionObject` 不变 ✓（`mark_has_instance_dict` **不能用** ✗ ——
        // 它要求载荷本身就是 `AttributeObject` ✓），随后既有的**惰性挂载**路径自会生效 ✓。
        // SAFETY: function_type 是刚建好的类型对象，存活 ✓。
        unsafe { function_type.as_ref() }.mark_external_instance_dict();


        // 迭代器类型：名字**照探测表**取（`str` 的迭代器在这台机器上叫 `str_ascii_iterator`）
        // 后两个的**可迭代对象**（`bytes`／`bytearray`）本身排在 M3+，故它们现在只是类型存在
        // （`TS-42` 的 M2 要求"迭代器对象"齐备），不会被 `GET_ITER` 选中。
        let _iterator_types: Vec<NonNull<TypeObject>> = [
            "tuple_iterator",
            "list_iterator",
            "str_ascii_iterator",
            "dict_keyiterator",
            "set_iterator",
            "bytes_iterator",
            "bytearray_iterator",
        ]
        .iter()
        .map(|name| {
            self.alloc_type_raw(
                name,
                core::mem::size_of::<IteratorObject>(),
                IteratorObject::slots(),
            )
        })
        .collect();

        // 原生可调用对象（`AB-24` 的宿主函数、`__build_class__` 一类内建函数的落点）
        let _builtin_function_type = self.alloc_type_raw(
            "builtin_function_or_method",
            core::mem::size_of::<BuiltinFunctionObject>(),
            BuiltinFunctionObject::slots()
                // **名字类属性**（第 406 轮 ✓）：`fractions.py` 导入期读 `operator.add.__name__` ✓
                .with_getattr(crate::builtin_objects::builtin_function_getattr),
        );

        // 绑定方法（`OM-11` 的 `getattr` 查到函数时的产物）
        let _method_type = self.alloc_type_raw(
            "method",
            core::mem::size_of::<MethodObject>(),
            MethodObject::slots()
                .with_repr(crate::builtin_objects::method_repr)
                .with_getattr(crate::builtin_objects::method_getattr),
        );

        // 生成器（`§10` 的生成器与协程族）：名字与基类照探测表
        let _generator_type = self.alloc_type_raw(
            "generator",
            core::mem::size_of::<GeneratorObject>(),
            GeneratorObject::slots()
                .with_repr(crate::builtin::generator::generator_repr)
                .with_getattr(crate::builtin::generator::generator_getattr),
        );

        // 协程（`§10` 的生成器与协程族）：载荷与生成器同形（一个挂起的帧 ＋ 标志），
        // 名字与基类照探测表；`repr` 的词是 `coroutine`（实测 `<coroutine object f at 0x…>`）。
        let _coroutine_type = self.alloc_type_raw(
            "coroutine",
            core::mem::size_of::<GeneratorObject>(),
            GeneratorObject::slots()
                .with_repr(crate::builtin_objects::coroutine_repr)
                .with_getattr(crate::builtin::generator::generator_getattr),
        );

        // 异步生成器（`CO_ASYNC_GENERATOR`，实测 `0x200`）：载荷同样与生成器同形，
        // 类型名与基类照探测表（实测 `async_generator` → `object`）。
        let _async_generator_type = self.alloc_type_raw(
            "async_generator",
            core::mem::size_of::<GeneratorObject>(),
            GeneratorObject::slots()
                .with_repr(crate::builtin_objects::async_generator_repr)
                .with_getattr(crate::builtin::generator::generator_getattr),
        );

        // `async_generator.__anext__()` 交出的 awaitable（参照实现的 `async_generator_asend`）。
        // 它是本层的**内部类型**（探测表里没有），故不走 `register_from_table`。
        let asend_type = self.alloc_type_raw(
            "async_generator_asend",
            core::mem::size_of::<AsendObject>(),
            AsendObject::slots(),
        );
        assert!(
            self.register_bases(asend_type, vec![object_type]).is_some(),
            "OM-13：asend 的基类也是 object"
        );

        // 异常层次（`TS-42` 的 M2）：**名字与基类都来自探测表**，按"基类先注册"的顺序反复扫。
        // 一个 `ExceptionObject` 载荷撑起整棵树（`TS-43`：布局自选）。
        let exception_names: Vec<&'static str> = crate::builtin_types::BUILTIN_TYPES
            .iter()
            .filter(|entry| entry.name == "BaseException" || entry.mro.contains(&"BaseException"))
            .map(|entry| entry.name)
            .collect();
        let exception_types: Vec<(NonNull<TypeObject>, &'static str)> = exception_names
            .iter()
            .map(|name| {
                (
                    self.alloc_type_raw(
                        name,
                        core::mem::size_of::<ExceptionObject>(),
                        ExceptionObject::slots()
                            .with_new(crate::builtin_objects::exception_new)
                            .with_repr(crate::builtin_objects::exception_repr)
                            .with_str(crate::builtin_objects::exception_str),
                    ),
                    *name,
                )
            })
            .collect();

        // **异常对象要有实例字典** ✓（第 213 轮）：`__traceback__`（以及以后的 `__notes__` ✓）就挂在它上面 ✓
        // —— 这是 `BC-60` 的**最小起步** ✓（`DIV-6` 的完整面仍留着 ✓）。
        for (ty, _) in &exception_types {
            // SAFETY: ty 由注册表持有。
            unsafe { ty.as_ref() }.mark_external_instance_dict();
        }
        let mut registered: Vec<&'static str> = vec!["object"];
        loop {
            let mut progressed = false;
            for (ty, name) in exception_types.iter() {
                if registered.contains(name) {
                    continue;
                }
                let entry = crate::builtin_types::builtin_type(name)
                    .unwrap_or_else(|| panic!("TS-41：{name} 必须在探测表里"));
                if entry.bases.iter().all(|base| registered.contains(base)) {
                    self.register_from_table(*ty);
                    registered.push(name);
                    progressed = true;
                }
            }
            if exception_types
                .iter()
                .all(|(_, name)| registered.contains(name))
            {
                break;
            }
            assert!(progressed, "TS-41：异常层次里有环或基类缺失");
        }
    }

    /// 引导第 6 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_frame_singletons(&self) {
        let object_type = self.type_named("object").expect("object 已登记");

        // **Pyawa 专有**异常：`TypeBoundaryError`（`TS-12`：**必须**是 `TypeError` 的子类）。
        // 参照实现没有这个类 ⇒ 探测表（`TS-41`）里没有它，故与 `asend` 一样走
        // `alloc_type_raw` ＋ 显式登记基类，不混进"照表注册"那条路。
        let boundary_error_type = self.alloc_type_raw(
            "TypeBoundaryError",
            core::mem::size_of::<ExceptionObject>(),
            ExceptionObject::slots()
                .with_new(crate::builtin_objects::exception_new)
                .with_repr(crate::builtin_objects::exception_repr)
                .with_str(crate::builtin_objects::exception_str),
        );
        let type_error = self
            .type_named("TypeError")
            .expect("TS-41：TypeError 在内建表里");
        assert!(
            self.register_bases(boundary_error_type, vec![type_error]).is_some(),
            "TS-12：TypeBoundaryError 的基类是 TypeError"
        );

        // **Pyawa 专有**异常：`TypeAssignmentError`（`TS-47`／`BC-65`：**必须**是 `TypeError`
        // 的子类；与边界的 `TypeBoundaryError`（`TS-12`）**同族、分工不同**）。
        // 与上一条同路：`alloc_type_raw` ＋ 显式登记基类，不混进"照表注册"那条路。
        let assignment_error_type = self.alloc_type_raw(
            "TypeAssignmentError",
            core::mem::size_of::<ExceptionObject>(),
            ExceptionObject::slots()
                .with_new(crate::builtin_objects::exception_new)
                .with_repr(crate::builtin_objects::exception_repr)
                .with_str(crate::builtin_objects::exception_str),
        );
        assert!(
            self.register_bases(assignment_error_type, vec![type_error]).is_some(),
            "TS-47：TypeAssignmentError 的基类是 TypeError"
        );

        // **`cell`**（`BC-45`）：cell 是**独立对象**（`GC_TRACKED`，经 traverse／clear 入链），
        // 但它在 `TS-42` 的探测表里挂在 `Ladder::Later`（Python 层可见性排后面）⇒ 引导期
        // **不会**自动注册。帧的 cell 槽与 `MAKE_CELL`／`LOAD_DEREF` 一族都要它存在，故在这里
        // 显式建一个（与 `TypeBoundaryError` 同一条路：`alloc_type_raw` ＋ 基类 `object`）。
        let cell_type = self.alloc_type_raw(
            "cell",
            core::mem::size_of::<crate::cell::CellObject>(),
            crate::cell::CellObject::slots(),
        );
        assert!(
            self.register_bases(cell_type, vec![object_type]).is_some(),
            "BC-45：cell 的基类是 object"
        );

        // **Pyawa 专有**的 `itertools.count` 迭代器类型（`SPEC-c-modules.md` §5.2.6）。
        // 参照实现里 `type(itertools.count())` 的 `__name__` 是 `count`；类型表里没有它
        // ⇒ 与 `TypeBoundaryError` 一样走 `alloc_type_raw`，基类 `object`。
        let count_type = self.alloc_type_raw(
            "count",
            core::mem::size_of::<crate::builtin_objects::CountIteratorObject>(),
            crate::builtin_objects::CountIteratorObject::slots(),
        );
        assert!(
            self.register_bases(count_type, vec![object_type]).is_some(),
            "itertools.count 的基类是 object"
        );
        for name in [
            "repeat",
            "islice",
            "chain",
            "takewhile",
            "dropwhile",
            "filterfalse",
            "accumulate",
            "starmap",
            "cycle",
            "pairwise",
            "batched",
            "zip_longest",
            "compress",
            "combinations",
            "combinations_with_replacement",
            "permutations",
            "product",
        ] {
            let ty = self.alloc_type_raw(
                name,
                core::mem::size_of::<crate::builtin_objects::ItStateObject>(),
                crate::builtin_objects::ItStateObject::slots(),
            );
            assert!(
                self.register_bases(ty, vec![object_type]).is_some(),
                "itertools 的迭代器类型基类是 object"
            );
        }
    }

    /// 引导第 7 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_iterator_registration(&self) {
        let iterator_types: Vec<NonNull<TypeObject>> = [
            "tuple_iterator",
            "list_iterator",
            "str_ascii_iterator",
            "dict_keyiterator",
            "set_iterator",
            "bytes_iterator",
            "bytearray_iterator",
        ]
        .iter()
        .map(|name| self.type_named(name).expect("迭代器类型已登记"))
        .collect();
        let object_type = self.type_named("object").expect("object 已登记");
        let none_type = self.type_named("NoneType").expect("NoneType 已登记");
        let ellipsis_type = self.type_named("ellipsis").expect("ellipsis 已登记");
        let bool_type = self.type_named("bool").expect("bool 已登记");
        let int_type = self.type_named("int").expect("int 已登记");
        let float_type = self.type_named("float").expect("float 已登记");
        let str_type = self.type_named("str").expect("str 已登记");
        let slice_type = self.type_named("slice").expect("slice 已登记");
        let bytes_type = self.type_named("bytes").expect("bytes 已登记");
        let bytearray_type = self.type_named("bytearray").expect("bytearray 已登记");
        let memoryview_type = self.type_named("memoryview").expect("memoryview 已登记");
        let list_reverseiterator_type = self.type_named("list_reverseiterator").expect("list_reverseiterator 已登记");
        let reversed_type = self.type_named("reversed").expect("reversed 已登记");
        let longrange_iterator_type = self.type_named("longrange_iterator").expect("longrange_iterator 已登记");
        let super_type = self.type_named("super").expect("super 已登记");
        let zip_type = self.type_named("zip").expect("zip 已登记");
        let range_iterator_type = self.type_named("range_iterator").expect("range_iterator 已登记");
        let tuple_type = self.type_named("tuple").expect("tuple 已登记");
        let list_type = self.type_named("list").expect("list 已登记");
        let dict_type = self.type_named("dict").expect("dict 已登记");
        let range_type = self.type_named("range").expect("range 已登记");
        let frozenset_type = self.type_named("frozenset").expect("frozenset 已登记");
        let set_type = self.type_named("set").expect("set 已登记");
        let function_type = self.type_named("function").expect("function 已登记");
        let builtin_function_type = self.type_named("builtin_function_or_method").expect("builtin_function_or_method 已登记");
        let method_type = self.type_named("method").expect("method 已登记");
        let generator_type = self.type_named("generator").expect("generator 已登记");
        let coroutine_type = self.type_named("coroutine").expect("coroutine 已登记");
        let async_generator_type = self.type_named("async_generator").expect("async_generator 已登记");

        // **内部** Frame 类型：执行器要给被调函数建帧（不进 `TS-41` 的内建表）
        let frame_type = self.alloc_type_raw(
            "frame",
            core::mem::size_of::<Frame>(),
            Frame::slots(),
        );
        assert!(
            self.register_bases(frame_type, vec![object_type]).is_some(),
            "OM-13：内部 Frame 类型的基类也是 object"
        );

        // **内部** code 类型：`compile::instantiate` 与 `Instance::code_with_qualname` 都按
        // "引导期已登记"取它（两处的 `expect` 就是这么写的）⇒ 它必须与 `Frame` 一样在这里登记。
        // 少了这一格，那条 `expect` 只在测试自己登记过时才成立、生产路径会 panic。
        // 同样**不进** `TS-41` 的内建类型表。
        let code_type = self.alloc_type_raw(
            "CodeObject",
            core::mem::size_of::<crate::CodeObject>(),
            crate::CodeObject::slots(),
        );
        assert!(
            self.register_bases(code_type, vec![object_type]).is_some(),
            "OM-13：内部 code 类型的基类也是 object"
        );

        // **内部哨兵**：`CALL` 的 NULL 槽位。它**不**进 `TS-41` 的内建类型表，
        // 也不许暴露给 Python，故不走 `register_from_table`。
        let null_type = self.alloc_type_raw(
            "NULL",
            core::mem::size_of::<NullObject>(),
            Slots::new(NullObject::dealloc),
        );
        assert!(
            self.register_bases(null_type, vec![object_type]).is_some(),
            "OM-13：内部哨兵的基类也是 object"
        );

        // **内部** `_thread` 的两个锁类型（第 280 轮 ✓）：`lock` 与 `RLock` 共用同一份载荷
        // （`ThreadLockObject` ✓），差别只在**方法语义**（可重入靠**类型名**判 ✓）⇒ 都**不进**
        // `TS-41` 的内建类型表 ✓（与 `frame`／`CodeObject`／`NULL` 同款 ✓）。
        for lock_name in ["lock", "RLock"] {
            let lock_type = self.alloc_type_raw(
                lock_name,
                core::mem::size_of::<crate::builtin_objects::ThreadLockObject>(),
                crate::builtin_objects::ThreadLockObject::slots(),
            );
            assert!(
                self.register_bases(lock_type, vec![object_type]).is_some(),
                "OM-13：`_thread` 锁类型的基类也是 object"
            );
        }

        // 基类关系：`bool ⊂ int`（TS-40 点名），其余都是 `object` 的直接子类——全部查表
        for ty in iterator_types
            .iter()
            .copied()
            .chain([
                function_type,
                generator_type,
                coroutine_type,
                async_generator_type,
                builtin_function_type,
                method_type,
                none_type,
                int_type,
                bool_type,
                float_type,
                str_type,
                bytes_type,
                bytearray_type,
                list_reverseiterator_type,
                reversed_type,
                longrange_iterator_type,
                range_iterator_type,
                zip_type,
                super_type,
                slice_type,
                tuple_type,
                list_type,
                dict_type,
                set_type,
                frozenset_type,
                range_type,
                memoryview_type,
            ])
        {
            self.register_from_table(ty);
        }

        // **OM-23**：单例——`None`／`True`／`False`／小整数／**空串**
        let null = self.adopt(NullObject::new(null_type)).cast::<Header>();
        let none = self.adopt(NoneObject::new(none_type)).cast::<Header>();
        let ellipsis = self
            .adopt(NoneObject::new(ellipsis_type))
            .cast::<Header>();
        let true_ = self.adopt(BoolObject::new(bool_type, true)).cast::<Header>();
        let false_ = self.adopt(BoolObject::new(bool_type, false)).cast::<Header>();
        let empty_str = self
            .adopt(StrObject::new(str_type, String::new()))
            .cast::<Header>();
        // **OM-23**：空元组（与空串同类）——`() is ()` **必须**为真（`is` 可观测）
        let empty_tuple = self
            .adopt(TupleObject::new(tuple_type, Vec::new()))
            .cast::<Header>();

        let count = (SMALL_INT_MAX - SMALL_INT_MIN + 1) as usize;
        let mut small_ints = Vec::with_capacity(count);
        for value in SMALL_INT_MIN..=SMALL_INT_MAX {
            small_ints.push(self.adopt(IntObject::new(int_type, IntValue::Small(value))).cast::<Header>());
        }

        assert!(
            self.singletons
                .set(Singletons::new(
                    none_type,
                    bool_type,
                    int_type,
                    str_type,
                    null,
                    empty_str,
                    empty_tuple,
                    none,
                    ellipsis,
                    true_,
                    false_,
                    small_ints,
                ))
                .is_ok(),
            "单例表在 Instance::new 里只设一次"
        );
    }

    /// 引导第 8 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_type_natives(&self) {
        let object_type = self.type_named("object").expect("object 已登记");
        let int_type = self.type_named("int").expect("int 已登记");
        let float_type = self.type_named("float").expect("float 已登记");
        let str_type = self.type_named("str").expect("str 已登记");
        let bytes_type = self.type_named("bytes").expect("bytes 已登记");
        let builtin_function_type = self.type_named("builtin_function_or_method").expect("builtin_function_or_method 已登记");

        // `TS-44`：`__format__` **没有槽位** ⇒ 内建类型在**类型字典**里放**原生可调用对象**
        // （`object` 那一层给默认：空规格 ⇒ `str(x)`、非空 ⇒ TypeError，消息实测）
        for (ty, name, function) in [
            (
                object_type,
                "__format__",
                crate::builtin_objects::native_format_object as crate::NativeFn,
            ),
            (
                int_type,
                "__format__",
                crate::builtin_objects::native_format_int as crate::NativeFn,
            ),
            (
                float_type,
                "__format__",
                crate::builtin_objects::native_format_float as crate::NativeFn,
            ),
            (
                str_type,
                "__format__",
                crate::builtin_objects::native_format_str as crate::NativeFn,
            ),
            // **`object.__hash__`**（第 309 轮）：本层先前没有它 ⇒ `C.__hash__`／`ref.__hash__`
            // 这类"在**类型对象**上取 dunder"的属性访问当场报
            // `AttributeError: 'type' object has no attribute '__hash__'`（`Lib/weakref.py:89` 的
            // `__hash__ = ref.__hash__` 正栽在这，那一族 **31** 个模块）。按**身份哈希**接一个：
            // 同一对象恒定；与参照各类型的哈希值**不一致**（如实登记 —— 本层字典查键走
            // `values_equal`，不靠这个值）。
            (
                self.type_named("object").expect("object 已登记"),
                "__hash__",
                crate::builtin::object::object_hash_native as crate::NativeFn,
            ),
            // **`object` 那一族 dunder**（第 310 轮）：Lib 里"在类型上取 dunder"最多的就是它们
            // （`grep` 实测：`__new__` 31 次、`__setattr__` 28 次、`__getattribute__` 17 次、
            // `__repr__` 10 次、`__str__` 9 次、`__init__` 9 次、`__ne__` 4 次 …）。
            // 一律转调核心已有的**同一处实现**（`values_equal`／`attribute_read`／
            // `attribute_write`／`object_repr`）；接收者两种形态都认。
            (
                self.type_named("object").expect("object 已登记"),
                "__eq__",
                crate::builtin::object::object_eq_native as crate::NativeFn,
            ),
            (
                self.type_named("object").expect("object 已登记"),
                "__ne__",
                crate::builtin::object::object_ne_native as crate::NativeFn,
            ),
            (
                self.type_named("object").expect("object 已登记"),
                "__repr__",
                crate::builtin::object::object_repr_native as crate::NativeFn,
            ),
            (
                self.type_named("object").expect("object 已登记"),
                "__str__",
                crate::builtin::object::object_repr_native as crate::NativeFn,
            ),
            (
                self.type_named("object").expect("object 已登记"),
                "__reduce_ex__",
                crate::builtin::object::object_reduce_ex_native as crate::NativeFn,
            ),
            // **`NoneType` 要有 `__str__`** ✓（第 271 轮）：`str(None)` 参照给 `'None'` ✓；本层 `NoneType` 的
            // 查找链上取不到它 ✗ ⇒ 于是 `'NoneType' object has no attribute '__str__'` 卡住同一批 118 个模块 ✓
            // ⇒ 直接给 `NoneType` 的命名空间补一份（复用 `object.__str__` 的实现 ✓ = `object_repr_native` ✓）。
            (
                self.type_named("NoneType").expect("NoneType 已登记"),
                "__str__",
                crate::builtin::object::object_repr_native as crate::NativeFn,
            ),
            (
                self.type_named("object").expect("object 已登记"),
                "__setattr__",
                crate::builtin::object::object_setattr_native as crate::NativeFn,
            ),
            (
                self.type_named("object").expect("object 已登记"),
                "__getattribute__",
                crate::builtin::object::object_getattribute_native as crate::NativeFn,
            ),
            (
                self.type_named("object").expect("object 已登记"),
                "__init__",
                crate::builtin::object::object_init_native as crate::NativeFn,
            ),
            // **`object.__init_subclass__`** ✓（第 722 轮）：`Lib/random.py:225` 一族在
            // `super().__init_subclass__(**kwargs)` 里要默认实现 ✓（先前 `object` 上没有这个名字 ✗）。
            (
                self.type_named("object").expect("object 已登记"),
                "__init_subclass__",
                crate::builtin::object::object_init_subclass_native as crate::NativeFn,
            ),
            // **`complex` 的类型级方法面**（task-30）：参照里 `complex.__complex__(z)`／
            // `complex.conjugate(z)`／`complex.__hash__(z)`／`complex.__abs__(z)` 都可用 ✓
            //（实例面是 `getattr` 槽 ✓，这里补类型级入口 ✓；未绑定时接收者＝第一个位置实参 ✓）。
            (
                self.type_named("complex").expect("complex 已登记"),
                "__complex__",
                crate::builtin_objects::complex_dunder_native as crate::NativeFn,
            ),
            (
                self.type_named("complex").expect("complex 已登记"),
                "conjugate",
                crate::builtin_objects::complex_conjugate_native as crate::NativeFn,
            ),
            (
                self.type_named("complex").expect("complex 已登记"),
                "__hash__",
                crate::builtin_objects::complex_hash_native as crate::NativeFn,
            ),
            (
                self.type_named("complex").expect("complex 已登记"),
                "__abs__",
                crate::builtin_objects::complex_abs_native as crate::NativeFn,
            ),
            (
                self.type_named("complex").expect("complex 已登记"),
                "__bool__",
                crate::builtin_objects::complex_bool_native as crate::NativeFn,
            ),
            // **`tuple.__new__`** ✓（第 728 轮）：属性通道可达 ✓（namedtuple／`CodecInfo`／`dis`／`enum` 的前置 ✓）。
            (
                self.type_named("tuple").expect("tuple 已登记"),
                "__new__",
                crate::builtin_objects::tuple_new_native as crate::NativeFn,
            ),
            // **`tuple.count`／`tuple.index` 的类型级入口** ✓（task-26 ②；未绑定语义：
            // 接收者＝第一个位置实参 ✓ —— 与 `tuple_new_native` 的"两种形态都认"同款 ✓）。
            (
                self.type_named("tuple").expect("tuple 已登记"),
                "count",
                crate::builtin_objects::tuple_count_native as crate::NativeFn,
            ),
            (
                self.type_named("tuple").expect("tuple 已登记"),
                "index",
                crate::builtin_objects::tuple_index_native as crate::NativeFn,
            ),
            // **`object.__subclasshook__`** ✓（第 722 轮）：`Lib/_py_abc.py:123` 读它做 ABC 的
            // `__subclasscheck__` ✓（先前缺 ⇒ `isinstance(x, ABC)` 报 AttributeError ⇒
            // **虚拟子类注册整条失效** ✗）。
            (
                self.type_named("object").expect("object 已登记"),
                "__subclasshook__",
                crate::builtin::object::object_subclasshook_native as crate::NativeFn,
            ),
            // **`dict` 的两个下标 dunder 也要在**类型**上取得到**（第 310 轮）：`Lib/collections/
            // __init__.py:120` 的 `dict_setitem=dict.__setitem__` 正是"在**类型对象**上取 dunder"，
            // 这走的是类型自己的命名空间（不是实例那条 `dict_getattr` 的路）⇒ 挂在类型字典里 ✓
            // （未绑定 ✓ ⇒ 调用时接收者在第一个实参 ✓，两个原生都认那两种形态 ✓）。
            (
                self.type_named("dict").expect("dict 已登记"),
                "__getitem__",
                crate::builtin::dict::dict_getitem_native as crate::NativeFn,
            ),
            (
                self.type_named("dict").expect("dict 已登记"),
                "__setitem__",
                crate::builtin::dict::dict_setitem_native as crate::NativeFn,
            ),
            (
                self.type_named("dict").expect("dict 已登记"),
                "__delitem__",
                crate::builtin::dict::dict_delitem_native as crate::NativeFn,
            ),
            (
                self.type_named("dict").expect("dict 已登记"),
                "__eq__",
                crate::builtin::dict::dict_eq_native as crate::NativeFn,
            ),
            // **`str.maketrans`／`bytes.maketrans`**（第 313 轮）：`'type' object has no attribute
            // 'maketrans'` × **67** 个模块的卡点 ✓。两个都是**静态**用法（在类型对象上取 ⇒ 无接收者 ✓）。
            (
                str_type,
                "maketrans",
                crate::builtin::str::str_maketrans_native as crate::NativeFn,
            ),
            (
                bytes_type,
                "maketrans",
                crate::builtin::bytes::bytes_maketrans_native as crate::NativeFn,
            ),
            // **`int.from_bytes`**（第 722 轮 ✓）：`Lib/ipaddress.py` **导入期**就要它 ✓
            //（`'type' object has no attribute 'from_bytes'` 把 `ipaddress` ⇒ `urllib.parse`
            // ⇒ `email.utils` 这一串压在下面 ✓）。类型级静态用法（无接收者 ✓），与 `maketrans` 同路 ✓。
            (
                self.type_named("int").expect("int 已登记"),
                "from_bytes",
                crate::builtin::int::int_from_bytes_native as crate::NativeFn,
            ),
            // **`int.bit_length` 描述符**（第 406 轮 ✓）：`_pydecimal.py:5648` 导入期
            // `_nbits = int.bit_length` ✓（描述符语义：接收者是第一个实参 ✓）。
            (
                self.type_named("int").expect("int 已登记"),
                "bit_length",
                crate::builtin::int::int_bit_length_descriptor as crate::NativeFn,
            ),
            // **`float.fromhex`**（第 406 轮 ✓）：`_pylong.py:260` 导入期
            // `float.fromhex('0x1.a934f0979a371p-2')` ✓（类型级静态用法，与 `int.from_bytes` 同路 ✓）。
            (
                self.type_named("float").expect("float 已登记"),
                "fromhex",
                crate::builtin::float::float_fromhex_native as crate::NativeFn,
            ),
            // **`list.append` 的类型级描述符**（task-23 ✓）：`multiprocessing/reduction.py:219` 的
            // `register(type(list.append), …)` 是**导入期**取用 ✓（描述符语义：接收者＝第一个实参 ✓）。
            (
                self.type_named("list").expect("list 已登记"),
                "append",
                crate::builtin::list::list_append_descriptor as crate::NativeFn,
            ),
            // **`int.__add__` 的类型级描述符**（task-23 ✓）：同文件 `:220` 的 `type(int.__add__)` ✓。
            (
                self.type_named("int").expect("int 已登记"),
                "__add__",
                crate::executor::arithmetic::int_add_descriptor as crate::NativeFn,
            ),
            // **`float.__getformat__`**（task-23 ✓）：`test/support/__init__.py:522` **导入期**
            // `float.__getformat__('double')` ✓（静态用法，无接收者 ✓；纯函数真实现 ✓）。
            (
                self.type_named("float").expect("float 已登记"),
                "__getformat__",
                misc::float_getformat_native as crate::NativeFn,
            ),
        ] {
            let native = self.alloc(BuiltinFunctionObject::new(
                builtin_function_type,
                name,
                Cell::new(function),
            ));
            self.set_type_attribute(ty, name, native.into_raw().cast::<Header>());
        }

        // **`type.__annotations__` getset 描述符** ✓（task-20 ✓）：`annotationlib:1141` 读的就是
        // `type.__dict__["__annotations__"].__get__` ✓（类对象与模块对象两条路 ✓）。
        self.install_annotations_descriptor();
    }

    /// 引导第 9 段（同文件原顺序搬入；跨段引用经 `type_named` 重新取）。
    fn bootstrap_build_class(&self) {
        let builtin_function_type = self.type_named("builtin_function_or_method").expect("builtin_function_or_method 已登记");

        // `LOAD_BUILD_CLASS` 要压的内建（`__build_class__`）：造一个原生可调用对象按实例存
        let build_class = self.alloc(BuiltinFunctionObject::new(
            builtin_function_type,
            "__build_class__",
            Cell::new(crate::classes::build_class_native as crate::NativeFn),
        ));
        self.build_class.set(Some(build_class.into_raw().cast::<Header>()));
    }


    /// 装 **`type.__annotations__`** 描述符 ✓（task-20 ✓，**幂等** ✓）。
    ///
    /// 「描述符」＝ 一个**属性类型**的实例 ✓，它的类型命名空间里有一条 `__get__` ✓
    /// （与 stdlib 侧 `_tuplegetter` 同一手法 ✓）⇒ 既满足 `type.__dict__["__annotations__"].__get__` ✓，
    /// 也让 `getattr(C, "__annotations__")` 这条路能走描述符协议 ✓。
    pub(crate) fn install_annotations_descriptor(&self) {
        let Some(type_object) = self.type_named("type") else {
            return;
        };
        let descriptor_type = match self.type_named("getset_descriptor") {
            Some(existing) => existing,
            None => {
                let ty = self.new_attribute_type("getset_descriptor");
                if let Some(namespace) = self.type_namespace(ty.cast()) {
                    let function_type = self
                        .type_named("builtin_function_or_method")
                        .expect("builtin_function_or_method 在引导期已登记");
                    let function = self.alloc(BuiltinFunctionObject::new(
                        function_type,
                        "__get__",
                        Cell::new(
                            crate::builtin_objects::getset_descriptor_get as crate::NativeFn,
                        ),
                    ));
                    self.dict_set(namespace, "__get__", function.into_raw().cast::<Header>());
                }
                ty
            }
        };
        let annotations_kind = self.new_dict();
        let kind = self.new_str("annotations");
        self.dict_set(annotations_kind, "_kind", kind);
        let descriptor = self.alloc(crate::builtin_objects::AttributeObject::new(
            descriptor_type,
            core::cell::RefCell::new(Some(annotations_kind)),
        ));
        let descriptor = descriptor.into_raw().cast::<Header>();
        if let Some(namespace) = self.type_namespace(type_object.cast()) {
            self.dict_set(namespace, "__annotations__", descriptor);
        }
        // **`type.__dict__["__mro__"]`** ✓（task-20 第 2 轮）：`Lib/inspect.py:1673` 的
        // `_static_getmro = type.__dict__['__mro__'].__get__` 要它 ✓（同一类描述符 ✓）。
        let dict_kind = self.new_dict();
        let kind = self.new_str("dict");
        self.dict_set(dict_kind, "_kind", kind);
        let dict_descriptor = self.alloc(crate::builtin_objects::AttributeObject::new(
            descriptor_type,
            core::cell::RefCell::new(Some(dict_kind)),
        ));
        let dict_descriptor = dict_descriptor.into_raw().cast::<Header>();
        if let Some(namespace) = self.type_namespace(type_object.cast()) {
            self.dict_set(namespace, "__dict__", dict_descriptor);
        }
        // **`types.GenericAlias.__mro_entries__`** ✓（task-20 第 2 轮，PEP 560）：类型下标的结果当基类
        // （`class T(Mapping[str, str])` ✓）要靠它换出真正的基类 ✓。
        for alias_name in ["GenericAlias", "UnionType"] {
            if let Some(alias_type) = self.type_named(alias_name) {
                if let Some(namespace) = self.type_namespace(alias_type.cast()) {
                    if self.dict_get(namespace, "__mro_entries__").is_none() {
                        let function_type = self
                            .type_named("builtin_function_or_method")
                            .expect("builtin_function_or_method 在引导期已登记");
                        let function = self.alloc(BuiltinFunctionObject::new(
                            function_type,
                            "__mro_entries__",
                            Cell::new(
                                crate::builtin_objects::generic_alias_mro_entries as crate::NativeFn,
                            ),
                        ));
                        self.dict_set(namespace, "__mro_entries__", function.into_raw().cast::<Header>());
                    }
                }
            }
        }
        let mro_kind = self.new_dict();
        let kind = self.new_str("mro");
        self.dict_set(mro_kind, "_kind", kind);
        let mro_descriptor = self.alloc(crate::builtin_objects::AttributeObject::new(
            descriptor_type,
            core::cell::RefCell::new(Some(mro_kind)),
        ));
        let mro_descriptor = mro_descriptor.into_raw().cast::<Header>();
        if let Some(namespace) = self.type_namespace(type_object.cast()) {
            self.dict_set(namespace, "__mro__", mro_descriptor);
        }
    }

}
