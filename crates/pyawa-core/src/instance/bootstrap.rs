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
            gc_threshold: Cell::new(DEFAULT_GC_THRESHOLD),
            gc_alloc_count: Cell::new(0),
            gc_frozen: RefCell::new(HashSet::new()),
            gc_running: Cell::new(false),
            repr_guard: RefCell::new(Vec::new()),
            interrupted: Cell::new(false),
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
