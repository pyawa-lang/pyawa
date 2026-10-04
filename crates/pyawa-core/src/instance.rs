//! 实例级内存、释放协议与循环回收（`docs/SPEC-object-model.md` §4、§7、§9）。
//!
//! 一个 [`Instance`] 就是 VM 侧一切可变状态的宿主（`DESIGN.md` §3 不变量 2）：
//! 对象堆、字节记账、类型注册表、回收链表与待处理栈都挂在它上面，**没有进程级全局状态**。

use core::cell::{Cell, OnceCell, RefCell};
use core::ptr;
use core::ptr::NonNull;
use std::collections::{HashMap, HashSet};

use crate::flags;
use crate::bigint::IntValue;
use crate::executor::ExecError;
use crate::header::{Header, PyObject};
use crate::refcount::{Owned, PyRef};
use crate::frame::Frame;
use crate::builtin_objects::{
    AsendObject, AttributeObject, BoolObject, BuiltinFunctionObject, BytesObject, DictObject,
    ExceptionObject, FloatObject, FunctionObject, GeneratorObject, IntObject, IteratorObject,
    ListObject, MethodObject, NoneObject, NullObject, PlainObject, SetObject, SliceObject,
    StrObject, TupleObject,
};
use crate::singleton::{Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
use crate::type_object::{Slots, TypeObject};

/// 一个 `str` 对象的内容是否等于给定的 Rust 字符串（属性名比较用）。
fn str_matches(instance: &Instance, raw: NonNull<Header>, expected: &str) -> bool {
    // SAFETY: 调用方保证 raw 是存活对象。
    if unsafe { raw.as_ref() }.ty() != instance.singletons().str_type() {
        return false;
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value() == expected
}

/// **`TS-45` ①**：`int`→`str`／`str`→`int` 的位数上限**默认值**（参照 3.14.4 实测）。
pub const INT_MAX_STR_DIGITS_DEFAULT: u32 = 4300;

/// **`TS-45` ①**：`set_int_max_str_digits` 允许的最小非零值（参照实测
/// `sys.int_info.str_digits_check_threshold == 640`）。
pub const INT_MAX_STR_DIGITS_THRESHOLD: u32 = 640;

/// **OM-26**：回收阈值，**三元组**形态。
///
/// 参照实现（本机 CPython 3.14.4 实测）：`gc.get_threshold() == (2000, 10, 0)`。
/// 本层**单代**：只有 `.0` 生效，后两位**存而不生效**——这一"存而不生效"是**临时**的，
/// 等真分代落地（`SPEC-object-model.md` 的 `OM-26`、`DESIGN.md` §13-18）。
pub const DEFAULT_GC_THRESHOLD: (usize, usize, usize) = (2000, 10, 0);

/// **OM-1**／**OM-3**／**OM-4**：一个实例的对象堆与记账。
/// 一个能力域的注册状态（形状与 `pyawa-abi` 的 `CapabilitySlot` 对应；`AB-32`：本层只存 ✓）。
#[derive(Clone, Copy, Debug, Default)]
pub struct CapabilityEntry {
    /// 宿主给的 vtable 指针（不透明）。
    pub implementation: *const core::ffi::c_void,
    /// **`CP-25`**：异步分类；`None` ＝ 尚未声明（那时**禁止**注册实现 ✓）。
    pub classification: Option<i32>,
}

/// 一次能力调用的三种结局（`CP-3`：成功／机器错误／未实现；**禁止**用 `errno` 表示"未实现" ✓）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityCallError {
    /// 该域**未注册**（`CP-2`：调用时报"未实现" ✓）。
    NotRegistered,
    /// 槽位**未实现**（`CP-5`）。
    NotImplemented,
    /// 机器错误；`errno` 原样带回。
    Machine(i32),
}

pub struct Instance {
    /// **OM-3**：每实例字节计数器（预算职责留在 VM 侧，禁止下放给能力接口）。
    bytes_allocated: Cell<usize>,
    /// 本实例分配、尚未释放的普通对象（`usize` = 头部地址；**O(1)** 增删）。
    live: RefCell<HashSet<usize>>,
    /// **OM-15**：类型注册表按实例存放；注册表持有每个类型对象的一份引用。
    types: RefCell<Vec<NonNull<TypeObject>>>,
    /// 元类型（类型对象的类型，自指）。
    metatype: Cell<Option<NonNull<TypeObject>>>,
    /// **OM-23**：本实例的单例表（引导期填好，之后只读）。
    singletons: OnceCell<Singletons>,
    /// **正在执行的那一帧的全局映射**（第 156 轮）：给内建 `globals()` 用 ✓。
    ///
    /// `Frame` 本来就带 `globals` 那一格（`BC-57`：函数帧取函数的 `__globals__` ✓）⇒ 只需把**最内层**
    /// 的那一格挂到实例上 ✓；执行器用 **RAII 守卫**（`Drop` 恢复 ✓）挂／摘 ✓，这样 `execute` 里
    /// **任何**提前返回（含 `?`）都不会留下悬空指针 ✓。
    current_globals: Cell<Option<NonNull<Header>>>,
    /// **能力域槽位**（`AB-33`：按域注册；`AB-34`／`CP-25`：必须带异步分类，缺失即注册失败 ✓）。
    ///
    /// 存的是**不透明指针**（`AB-32`：本层只存不解释 ✓）；`fs` 域的形状解释见
    /// [`Instance::fs_vtable`]（`CP-12`：形状来自 `pyawa-capabilities` ✓）。
    capabilities: RefCell<[CapabilityEntry; pyawa_capabilities::DOMAIN_COUNT]>,
    /// **模块表**（`IM-`：`import` 查的就是这一份 ✓）。与 `sys.modules` 是**同一个 dict**
    /// （一处真相 ✓，由组合根装 ✓）；未装 ⇒ `import` 报"加载器未接" ✓。
    modules: RefCell<Option<NonNull<Header>>>,
    /// **BC-60** ②：**本实例**的当前异常状态（正在处理的异常）——**禁止**进程级全局。
    exception_state: RefCell<Vec<NonNull<Header>>>,
    /// `__build_class__`（引导期建好；见 [`Instance::build_class`]）。
    build_class: Cell<Option<NonNull<Header>>>,
    /// 最近一次抛出的异常（**本实例持有一份引用**）：`ExecError::Raised` 借它保活。
    pending_exception: Cell<Option<NonNull<Header>>>,
    /// **内建名字空间**（`builtins`）：`LOAD_NAME`／`LOAD_GLOBAL` 的最后回退层。
    ///
    /// 现在是**可选**的（核心引导期只装 `__build_class__` 一个可调用对象，还不是映射）；
    /// 由组合根／stdlib 装一个真的映射进来（那时模块级代码才看得到内建）。**未装 ⇒ 回退层为空**。
    builtins: Cell<Option<NonNull<Header>>>,
    /// **`DESIGN.md` §9 第 20 条**：平台相关**只读常量**（`errno` 一类）——由
    /// `pyawa-runtime` 在启动时注入，**按名字**查（数字随平台）。**不新增能力域**
    /// （`CM-20`：映射按名字匹配）。存在实例上 ⇒ 不引入任何进程级状态（`CX-3`）。
    platform_constants: RefCell<Vec<(&'static str, i64)>>,
    /// **`TS-45` ①**：`sys.get_int_max_str_digits()`／`set_int_max_str_digits()` 的落点
    /// （按实例存，`CX-3`；`0` ＝ 不限）。默认与阈值都是**参照实测**（见两个常量）。
    int_max_str_digits: Cell<u32>,
    /// **OM-21**：待处理栈——计数归零的对象在这里排队，由最外层调用逐个清空（禁止朴素递归）。
    pending: RefCell<Vec<NonNull<Header>>>,
    /// 是否正在清空待处理栈（重入检测）。
    draining: Cell<bool>,
    /// **OM-25**：`GC_TRACKED` 对象的侵入式链表头（借头部的 `gc_prev`／`gc_next`）。
    gc_head: Cell<*mut Header>,
    /// 链表中当前的跟踪对象数。
    gc_count: Cell<usize>,
    /// **OM-26**：阈值可配置。
    gc_threshold: Cell<(usize, usize, usize)>,
    /// 自上次回收以来的分配计数。
    gc_alloc_count: Cell<usize>,
    /// 回收进行中：这些对象只减计数、由本次回收统一释放（见 [`Instance::collect`]）。
    gc_frozen: RefCell<HashSet<usize>>,
    /// 回收是否正在进行：终结器／`clear` 里再触发回收时不得嵌套（否则会动到外层手里的指针）。
    gc_running: Cell<bool>,
    /// **`OM-11` 的 `repr` 递归守卫**（`CX-3`：按实例存）：正在生成 `repr` 的对象地址。
    repr_guard: RefCell<Vec<usize>>,
    /// **`AB-5`①／`CX-3`**：本实例被请求中断（`pa_interrupt` 的落点）。
    ///
    /// **按实例**存——`CX-3` 禁止进程级共享；执行器每条指令检查一次，
    /// 于是"执行类函数随即返回"（`PA_ERR_INTERRUPT`）成立。
    interrupted: Cell<bool>,
}

impl Instance {
    /// 创建一个实例，并引导它的**元类型**。
    ///
    /// **OM-1**：每个实例有自己的堆与单例表；本函数不触碰任何进程级状态。
    pub fn new() -> Self {
        let this = Self {
            bytes_allocated: Cell::new(0),
            live: RefCell::new(HashSet::new()),
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

    /// 装/取**模块表**（与 `sys.modules` 同一份 ✓；返回旧的，调用方负责释放）。
    pub fn set_modules(&self, mapping: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        // **本方法自己 `retain` 新的那一份** ✓（第 199 轮**真 bug 修复** ✗）：模块表有**两处**持有者 ✓
        // —— 本实例 ✓ 与 `sys.modules` 里那一项 ✓ ⇒ 先前只留一份引用 ✗ ⇒ `find_unreachable` 把"本实例的这份"
        // 当成**候选内部引用**减掉 ✓ ⇒ external 归零 ✗ ⇒ **模块表被判不可达** ✗ ⇒ 整个模块表连同所有模块被 `free` ✗
        // ⇒ 活对象被释放 ⇒ 堆损坏 ✓（实测：修前 `原始 refcount=1`／`external=0` ✗，修后 `2`／`1` ✓）。
        if let Some(new) = mapping {
            // SAFETY: new 由调用方保证存活。
            unsafe { self.incref_object(new.as_ptr()) };
        }
        core::mem::replace(&mut *self.modules.borrow_mut(), mapping)
    }

    /// 模块表（**借用**）。
    pub fn modules(&self) -> Option<NonNull<Header>> {
        *self.modules.borrow()
    }

    /// **注册一个能力域**（`AB-33`／`AB-34`）：`classification` 缺失 ⇒ 注册**失败** ✓
    /// （`CP-25`：禁止落默认值）。返回是否注册成功。
    pub fn set_capability(
        &self,
        domain: usize,
        implementation: *const core::ffi::c_void,
        classification: Option<i32>,
    ) -> bool {
        if classification.is_none() {
            return false;
        }
        let mut slots = self.capabilities.borrow_mut();
        match slots.get_mut(domain) {
            Some(slot) => {
                slot.implementation = implementation;
                slot.classification = classification;
                true
            }
            None => false,
        }
    }

    /// 某个域的**不透明实现指针**（`AB-32`：本层只存不解释）。
    pub fn capability(&self, domain: usize) -> Option<*const core::ffi::c_void> {
        let slots = self.capabilities.borrow();
        let slot = slots.get(domain)?;
        (!slot.implementation.is_null()).then_some(slot.implementation)
    }

    /// **经 `fs` 域写一段字节**（`CP-2`／`CP-3`／`CP-5` 的三态在这里落成结果 ✓）。
    ///
    /// 调用方（stdlib 的 `_io`）只管文本层与编码；**平台**在提供者那边 ✓（`CX-4`）。
    pub fn fs_write(&self, handle: u64, bytes: &[u8]) -> Result<usize, CapabilityCallError> {
        let Some(table) = self.fs_vtable() else {
            return Err(CapabilityCallError::NotRegistered);
        };
        let Some(write) = table.write else {
            return Err(CapabilityCallError::NotImplemented);
        };
        let mut written = 0usize;
        let mut errno = 0i32;
        match write(
            table.state,
            pyawa_capabilities::fs::Handle(handle),
            bytes.as_ptr(),
            bytes.len(),
            &mut written,
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(written),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => {
                Err(CapabilityCallError::Machine(errno))
            }
        }
    }

    /// **经 `fs` 域打开文件**（`CP-2`／`CP-3`／`CP-5` 三态同上 ✓）。`path` 按**字节原样**交给提供者 ✓
    /// （平台侧怎么解释路径不归本层管 ✓）。
    pub fn fs_open(&self, path: &[u8], flags: i32, mode: u32) -> Result<u64, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let open = table.open.ok_or(CapabilityCallError::NotImplemented)?;
        let mut handle = pyawa_capabilities::fs::Handle(0);
        let mut errno = 0i32;
        match open(table.state, path.as_ptr(), path.len(), flags, mode, &mut handle, &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(handle.0),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域读**（把 `buffer` 填满到能读的为止 ✓）。返回实际读到的字节数 ✓。
    pub fn fs_read(&self, handle: u64, buffer: &mut [u8]) -> Result<usize, CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let read = table.read.ok_or(CapabilityCallError::NotImplemented)?;
        let mut read_bytes = 0usize;
        let mut errno = 0i32;
        match read(
            table.state,
            pyawa_capabilities::fs::Handle(handle),
            buffer.as_mut_ptr(),
            buffer.len(),
            buffer.len(),
            &mut read_bytes,
            &mut errno,
        ) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(read_bytes),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **经 `fs` 域关闭句柄**。
    pub fn fs_close(&self, handle: u64) -> Result<(), CapabilityCallError> {
        let table = self.fs_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let close = table.close.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match close(table.state, pyawa_capabilities::fs::Handle(handle), &mut errno) {
            pyawa_capabilities::fs::CapStatus::Ok => Ok(()),
            pyawa_capabilities::fs::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::fs::CapStatus::Machine => Err(CapabilityCallError::Machine(errno)),
        }
    }

    /// **`fs` 域的形状视图**（`SPEC-capabilities.md` §9.1）：把宿主注册的不透明指针按
    /// [`pyawa_capabilities::fs::CpFsVtable`] 解释 ✓。`None` ＝ 该域未提供（`CP-2` ✓）。
    ///
    /// # Safety
    ///
    /// 注册方（宿主）必须保证：该指针指向一个**在实例存活期间有效**的 `CpFsVtable` ✓
    /// （`AB-16`／`AB-17` 的借用纪律由调用方遵守 ✓）。
    pub fn fs_vtable(&self) -> Option<pyawa_capabilities::fs::CpFsVtable> {
        let pointer = self.capability(pyawa_capabilities::DOMAIN_FS)?;
        // SAFETY: 见函数文档——注册方保证指针有效且布局正确。
        Some(unsafe { *pointer.cast::<pyawa_capabilities::fs::CpFsVtable>() })
    }

    /// **OM-23**：按实例创建单例（`None`／`True`／`False`／小整数）。
    ///
    /// 引导期还不能借出 `&Instance` 造 `Owned` 守卫，所以走 [`Instance::adopt`]：
    /// 引用由实例自己持有，随实例销毁一起释放（`OM-2`）。
    /// 按 `TS-41` 的**探测表**登记一个类型的基类（MRO 由 C3 算）。
    ///
    /// 层次**不手写**：基类名字取自 `crate::builtin_types`。基类必须**先**注册（表里的顺序
    /// 是参照实现的顺序，但引导期按依赖手工排序）。
    fn register_from_table(&self, ty: NonNull<TypeObject>) {
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
    fn bootstrap_builtin_types(&self) {
        let object_type = self.alloc_type_raw(
            "object",
            core::mem::size_of::<PlainObject>(),
            Slots::new(PlainObject::dealloc).with_new(crate::builtin_objects::plain_new),
        );
        self.register_from_table(object_type);

        // 元类型（`type`）也是对象；表里 `type` 的基类就是 `object`
        let metatype = self.metatype.get().expect("元类型在 Instance::new 里已引导");
        self.register_from_table(metatype);

        let none_type = self.alloc_type_raw(
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
        let bool_type = self.alloc_type_raw(
            "bool",
            core::mem::size_of::<BoolObject>(),
            Slots::new(BoolObject::dealloc)
                .with_new(crate::builtin_objects::bool_new)
                .with_repr(crate::builtin_objects::bool_repr)
                .with_str(crate::builtin_objects::bool_repr),
        );
        let int_type = self.alloc_type_raw(
            "int",
            core::mem::size_of::<IntObject>(),
            Slots::new(IntObject::dealloc)
                .with_new(crate::builtin_objects::int_new)
                .with_repr(crate::builtin_objects::int_repr)
                .with_str(crate::builtin_objects::int_repr)
                // **方法面**（第 195 轮）：`to_bytes`／`bit_length` ✓。
                .with_getattr(crate::builtin_objects::int_getattr),
        );
        let float_type = self.alloc_type_raw(
            "float",
            core::mem::size_of::<FloatObject>(),
            Slots::new(FloatObject::dealloc)
                .with_new(crate::builtin_objects::float_new)
                .with_repr(crate::builtin_objects::float_repr)
                .with_str(crate::builtin_objects::float_repr),
        );
        let str_type = self.alloc_type_raw(
            "str",
            core::mem::size_of::<StrObject>(),
            Slots::new(StrObject::dealloc)
                .with_new(crate::builtin_objects::str_new)
                .with_repr(crate::builtin_objects::str_repr)
                .with_str(crate::builtin_objects::str_str)
                // **方法面**（第 143 轮）：`Lib/` 里每个文件都在用字符串方法 ✓
                .with_getattr(crate::builtin_objects::str_getattr),
        );

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



                .with_new(crate::builtin_objects::property_new)
                // **方法面**（第 186 轮）：`fget`／`fset`／`fdel` 取值 ＋ `getter`／`setter`／`deleter` ✓。
                .with_getattr(crate::builtin_objects::property_getattr),



        );



        assert!(



            self.register_bases(property_type, vec![object_type]).is_some(),



            "property 的基类是 object"



        );




        let _classmethod_type = self.alloc_type_raw(
            "classmethod",
            core::mem::size_of::<crate::builtin_objects::ClassMethodObject>(),
            crate::builtin_objects::ClassMethodObject::slots()
                .with_new(crate::builtin_objects::classmethod_new),
        );

        let slice_type = self.alloc_type_raw(
            "slice",
            core::mem::size_of::<SliceObject>(),
            crate::builtin_objects::SliceObject::slots()
                .with_new(crate::builtin_objects::slice_new)
                .with_repr(crate::builtin_objects::slice_repr)
                // **属性面**（第 151 轮）：`slice(1, 3).start` 等 ✓
                .with_getattr(crate::builtin_objects::slice_getattr),
        );

        // **`bytes`**（`P1-12`／`TS-42` 的"M2 之后、M3 之前"档：`marshal` 与 `co_code` 要它）
        let bytes_type = self.alloc_type_raw(
            "bytes",
            core::mem::size_of::<BytesObject>(),
            crate::builtin_objects::BytesObject::slots()
                .with_new(crate::builtin_objects::bytes_new)
                .with_repr(crate::builtin_objects::bytes_repr)
                .with_str(crate::builtin_objects::bytes_str)
                .with_getattr(crate::builtin_objects::bytes_getattr),
        );

        // 容器：`TS-42` 的 M2 起步（层次取自探测表）
        let tuple_type = self.alloc_type_raw(
            "tuple",
            core::mem::size_of::<TupleObject>(),
            TupleObject::slots()
                .with_new(crate::builtin_objects::tuple_new)
                .with_repr(crate::builtin_objects::tuple_repr),
        );
        let list_type = self.alloc_type_raw(
            "list",
            core::mem::size_of::<ListObject>(),
            ListObject::slots()
                .with_new(crate::builtin_objects::list_new)
                .with_repr(crate::builtin_objects::list_repr)
                // `traverse`／`clear` **不在这里挂** ✓ —— `ListObject::slots()`（`builtin_objects.rs`
                // 里那个 impl ✓）已经含了 ✓，**一处真相** ✓；且静态检查 `gc_field_coverage` 只读
                // 那个 impl 块 ✓（在这里重复挂一次会让真相分叉 ✗）。
                // **方法面**（第 143 轮）
                .with_getattr(crate::builtin_objects::list_getattr),
        );
        let dict_type = self.alloc_type_raw(
            "dict",
            core::mem::size_of::<DictObject>(),
            DictObject::slots()
                .with_new(crate::builtin_objects::dict_new)
                .with_repr(crate::builtin_objects::dict_repr)
                // **方法面**（第 145 轮）
                .with_getattr(crate::builtin_objects::dict_getattr),
        );
        let set_type = self.alloc_type_raw(
            "set",
            core::mem::size_of::<SetObject>(),
            SetObject::slots()
                .with_new(crate::builtin_objects::set_new)
                .with_repr(crate::builtin_objects::set_repr)
                // **方法面**（第 146 轮）
                .with_getattr(crate::builtin_objects::set_getattr),
        );

        // `function`：`TS-42` 的 M2（调用与返回族逼出来的）
        let function_type = self.alloc_type_raw(
            "function",
            core::mem::size_of::<FunctionObject>(),
            FunctionObject::slots()
                .with_repr(crate::builtin_objects::function_repr)
                .with_getattr(crate::builtin_objects::function_getattr),
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
        .map(|name| {
            self.alloc_type_raw(
                name,
                core::mem::size_of::<IteratorObject>(),
                IteratorObject::slots(),
            )
        })
        .collect();

        // 原生可调用对象（`AB-24` 的宿主函数、`__build_class__` 一类内建函数的落点）
        let builtin_function_type = self.alloc_type_raw(
            "builtin_function_or_method",
            core::mem::size_of::<BuiltinFunctionObject>(),
            BuiltinFunctionObject::slots(),
        );

        // 绑定方法（`OM-11` 的 `getattr` 查到函数时的产物）
        let method_type = self.alloc_type_raw(
            "method",
            core::mem::size_of::<MethodObject>(),
            MethodObject::slots().with_repr(crate::builtin_objects::method_repr),
        );

        // 生成器（`§10` 的生成器与协程族）：名字与基类照探测表
        let generator_type = self.alloc_type_raw(
            "generator",
            core::mem::size_of::<GeneratorObject>(),
            GeneratorObject::slots()
                .with_repr(crate::builtin_objects::generator_repr)
                .with_getattr(crate::builtin_objects::generator_getattr),
        );

        // 协程（`§10` 的生成器与协程族）：载荷与生成器同形（一个挂起的帧 ＋ 标志），
        // 名字与基类照探测表；`repr` 的词是 `coroutine`（实测 `<coroutine object f at 0x…>`）。
        let coroutine_type = self.alloc_type_raw(
            "coroutine",
            core::mem::size_of::<GeneratorObject>(),
            GeneratorObject::slots()
                .with_repr(crate::builtin_objects::coroutine_repr)
                .with_getattr(crate::builtin_objects::generator_getattr),
        );

        // 异步生成器（`CO_ASYNC_GENERATOR`，实测 `0x200`）：载荷同样与生成器同形，
        // 类型名与基类照探测表（实测 `async_generator` → `object`）。
        let async_generator_type = self.alloc_type_raw(
            "async_generator",
            core::mem::size_of::<GeneratorObject>(),
            GeneratorObject::slots()
                .with_repr(crate::builtin_objects::async_generator_repr)
                .with_getattr(crate::builtin_objects::generator_getattr),
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

        // **内部** Frame 类型：执行器要给被调函数建帧（不进 `TS-41` 的内建表）
        let frame_type = self.alloc_type_raw(
            "Frame",
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
                slice_type,
                tuple_type,
                list_type,
                dict_type,
                set_type,
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
        ] {
            let native = self.alloc(BuiltinFunctionObject::new(
                builtin_function_type,
                name,
                Cell::new(function),
            ));
            self.set_type_attribute(ty, name, native.into_raw().cast::<Header>());
        }

        // `LOAD_BUILD_CLASS` 要压的内建（`__build_class__`）：造一个原生可调用对象按实例存
        let build_class = self.alloc(BuiltinFunctionObject::new(
            builtin_function_type,
            "__build_class__",
            Cell::new(crate::classes::build_class_native as crate::NativeFn),
        ));
        self.build_class.set(Some(build_class.into_raw().cast::<Header>()));
    }

    /// **OM-13**：C3 线性化。基类顺序矛盾（没有可用候选）时返回 `None`。
    ///
    /// `L(C) = [C] + merge(L(B1), …, L(Bn), [B1, …, Bn])`；`merge` 每轮取"不出现在任何列表
    /// **尾部**"的第一个表头。**禁止**用"深度优先拼接"糊过去——那样 `__mro__` 与参照实现不一致。
    pub fn linearize(
        &self,
        ty: NonNull<TypeObject>,
        bases: &[NonNull<TypeObject>],
    ) -> Option<Vec<NonNull<TypeObject>>> {
        let mut sequences: Vec<Vec<NonNull<TypeObject>>> = Vec::new();
        for base in bases {
            // SAFETY: 基类由本实例的注册表持有（OM-15），在实例存活期间有效。
            sequences.push(unsafe { base.as_ref() }.mro());
        }
        sequences.push(bases.to_vec());

        let mut result = vec![ty];
        loop {
            sequences.retain(|sequence| !sequence.is_empty());
            if sequences.is_empty() {
                return Some(result);
            }
            let mut chosen = None;
            for sequence in &sequences {
                let candidate = sequence[0];
                let blocked = sequences
                    .iter()
                    .any(|other| other[1..].contains(&candidate));
                if !blocked {
                    chosen = Some(candidate);
                    break;
                }
            }
            let chosen = chosen?;
            result.push(chosen);
            for sequence in sequences.iter_mut() {
                sequence.retain(|entry| *entry != chosen);
            }
        }
    }

    /// **OM-13**／**OM-14**：登记基类，MRO 由 C3 算出并写入；不一致时返回 `None`。
    pub fn register_bases(
        &self,
        ty: NonNull<TypeObject>,
        bases: Vec<NonNull<TypeObject>>,
    ) -> Option<Vec<NonNull<TypeObject>>> {
        let mro = self.linearize(ty, &bases)?;
        // SAFETY: ty 由本实例的注册表持有。
        unsafe { ty.as_ref() }.set_bases(bases, mro.clone());
        Some(mro)
    }

    /// **`OM-10`**：沿 **MRO** 查类型字典（**借用**的裸引用；查不到返回 `None`）。
    ///
    /// 这是属性查找的"类型那一半"（`OM-11` 的 `getattr` 槽位随类型系统接线后接管分派）。
    pub fn type_lookup(&self, ty: NonNull<TypeObject>, name: &str) -> Option<NonNull<Header>> {
        self.type_lookup_owner(ty, name).map(|(_, value)| value)
    }

    /// **沿 MRO 查类型字典，并把"是哪个类型定义的"一起报出来** ✓（第 211 轮，**一处真相** ✓）。
    ///
    /// 为什么要它 ✗：`override_text` 需要区分"**用户／内建类型自己的** dunder"（真覆写 ✓）与
    /// "**`object` 上那条**属性面注册"（本层新挂的 `object.__str__`／`__repr__` ✓ ⇒ **不是**覆写 ✓）。
    pub fn type_lookup_owner(
        &self,
        ty: NonNull<TypeObject>,
        name: &str,
    ) -> Option<(NonNull<TypeObject>, NonNull<Header>)> {
        // SAFETY: ty 由注册表持有，MRO 里的类型同样存活。
        for entry in unsafe { ty.as_ref() }.mro() {
            // SAFETY: 同上。
            let mapping = unsafe { entry.as_ref() }.dict();
            let Some(mapping) = mapping else { continue };
            // SAFETY: mapping 由类型对象持有。
            let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
            let found = dict
                .entries()
                .into_iter()
                .find(|(key, _)| str_matches(self, *key, name));
            if let Some((_, value)) = found {
                return Some((entry, value));
            }
        }
        None
    }

    /// **`OM-10`**：往类型字典里写一项（**新引用**，由字典接手；返回被顶下来的旧值）。
    ///
    /// 字典惰性创建。**禁止**用这个函数给内建类型旁路属性通道——
    /// Python 可见属性一律走 `OM-11` 的 `getattr` 槽位。
    pub fn set_type_attribute(
        &self,
        ty: NonNull<TypeObject>,
        name: &str,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        // SAFETY: ty 由注册表持有。
        let type_object = unsafe { ty.as_ref() };
        let mapping = match type_object.dict() {
            Some(mapping) => mapping,
            None => {
                let mapping = self
                    .adopt(DictObject::new(
                        self.type_named("dict").expect("dict 已在引导期登记"),
                        RefCell::new(Vec::new()),
                    ))
                    .cast::<Header>();
                type_object.set_dict(Some(mapping));
                mapping
            }
        };
        // SAFETY: mapping 由类型对象持有。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let key = self
            .adopt(StrObject::new(self.singletons().str_type(), name.to_owned()))
            .cast::<Header>();
        let position = dict
            .entries()
            .into_iter()
            .position(|(existing, _)| str_matches(self, existing, name));
        match position {
            Some(slot) => {
                // 键已在表里：新键那份引用交回去
                // SAFETY: key 是刚 adopt 的对象，只有这一份引用。
                unsafe { self.release_object(key.as_ptr()) };
                dict.replace_value(slot, value)
            }
            None => {
                dict.insert_raw(key, value);
                None
            }
        }
    }

    /// **OM-14**：把**自定义载荷**（宿主类型）放进本实例成为对象——**新引用**。
    ///
    /// 载荷类型必须满足：`size_of::<T>()` 与该类型的 `instance_size` 一致（**OM-5**：头部在
    /// 第一个字段），且它自己的 `dealloc` 槽要与这里的布局相配。宿主类型走这条。
    pub fn alloc_payload<T: crate::PyObject>(&self, payload: T) -> NonNull<T> {
        self.alloc(payload).into_raw()
    }

    /// 抛一个内建异常（按名字），返回可直接上抛的执行错误。
    ///
    /// 给**槽位实现**用（宿主函数一类要在 core 之外抛 Python 异常）。
    pub fn raise_builtin_error(&self, name: &str, message: &str) -> crate::ExecError {
        crate::executor::raise_builtin(self, name, message)
    }

    /// 类型对象的**名字**（安全读取；给诊断消息与 stdlib 用）。
    pub fn type_name(&self, ty: NonNull<TypeObject>) -> String {
        // SAFETY: 类型对象由注册表持有。
        unsafe { ty.as_ref() }.name().to_owned()
    }

    /// 把一个类型对象当**值**用（**新引用**；给 `isinstance(x, T)` 这类传参）。
    pub fn type_value(&self, ty: NonNull<TypeObject>) -> NonNull<Header> {
        let header = ty.cast::<Header>();
        // SAFETY: 类型对象由注册表持有，存活。
        unsafe { self.incref_object(header.as_ptr()) };
        header
    }

    /// 对象是不是**类型对象**（`type` 的实例）——`isinstance`／`issubclass` 要用。
    pub fn is_type_object(&self, object: NonNull<Header>) -> bool {
        self.type_of(object) == self.metatype()
    }

    /// 把对象当**类型对象**看（是就给 `Some`，否则 `None`）。
    pub fn as_type(&self, object: NonNull<Header>) -> Option<NonNull<TypeObject>> {
        if !self.is_type_object(object) {
            return None;
        }
        // SAFETY: 对象就是类型对象（类型身份已确认）。
        Some(unsafe { NonNull::new_unchecked(object.as_ptr().cast::<TypeObject>()) })
    }

    /// **可调用判定**（`OM-11`）——**一处口径**：类型的 `call` 槽存在，**或**它是内建可调用
    /// 类型（`function`／`builtin_function_or_method`／`method`／元类型，这几个的调用语义写
    /// 在 `call_callable` 里）。
    ///
    /// 给 `callable()`、`pa_isfunction` 一类共用；两边各写一份就会漂。
    pub fn is_callable(&self, object: NonNull<Header>) -> bool {
        let ty = self.type_of(object);
        // SAFETY: 类型对象由注册表持有。
        if unsafe { ty.as_ref() }.has_call_slot() {
            return true;
        }
        ty == self.metatype()
            || Some(ty) == self.type_named("function")
            || Some(ty) == self.type_named("builtin_function_or_method")
            || Some(ty) == self.type_named("method")
    }

    /// 元组的元素（**借用视图**；不是元组给 `None`）。
    pub fn tuple_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        if Some(self.type_of(object)) != self.type_named("tuple") {
            return None;
        }
        // SAFETY: 类型身份已确认。
        let tuple = unsafe { &*object.as_ptr().cast::<TupleObject>() };
        Some((0..tuple.len()).map(|index| tuple.item(index).expect("下标在范围内")).collect())
    }

    /// 装一个内建名字空间（**新引用**，由实例接手；返回被顶下来的旧值）。
    pub fn set_builtins(&self, mapping: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.builtins.replace(mapping)
    }

    /// 内建名字空间（**借用**；没装就是 `None`）。
    pub fn builtins(&self) -> Option<NonNull<Header>> {
        self.builtins.get()
    }

    /// **迭代协议**的公开入口：把对象摊成一批**新引用**（`min`／`max`／`sorted` 要用）。
    ///
    /// 认：`list`／`tuple`／`str`（逐字符）／`dict`（逐**键**）／`set`。
    /// 不认识就给 `None`——**不猜**：调用方据此报参照实现那条
    /// `TypeError: 'int' object is not iterable`（实测）。
    pub fn iterable_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        let owned = |value: NonNull<Header>| {
            // SAFETY: value 由容器持有，存活；调用方要自己那份。
            unsafe { self.incref_object(value.as_ptr()) };
            value
        };
        if self.type_named("list") == Some(ty) {
            // SAFETY: 类型身份已确认。
            let list = unsafe { &*object.as_ptr().cast::<ListObject>() };
            return Some(list.items().into_iter().map(owned).collect());
        }
        if self.type_named("tuple") == Some(ty) {
            // SAFETY: 同上。
            let tuple = unsafe { &*object.as_ptr().cast::<TupleObject>() };
            return Some(tuple.items().iter().copied().map(owned).collect());
        }
        if ty == self.singletons().str_type() {
            // SAFETY: 同上。
            let text = unsafe { &*object.as_ptr().cast::<StrObject>() }
                .value()
                .to_owned();
            return Some(
                text.chars()
                    .map(|character| self.new_str(&character.to_string()))
                    .collect(),
            );
        }
        if ty == self.type_named("dict")? {
            // SAFETY: 同上。
            let dict = unsafe { &*object.as_ptr().cast::<DictObject>() };
            return Some(dict.entries().into_iter().map(|(key, _)| owned(key)).collect());
        }
        if ty == self.type_named("set")? {
            // SAFETY: 同上。
            let set = unsafe { &*object.as_ptr().cast::<SetObject>() };
            return Some(set.items().into_iter().map(owned).collect());
        }
        // **迭代器对象**（第 137 轮）：`list(itertools.repeat(5, 3))`／`list(x for x in y)` 这类
        // 都要能消费 ✓ ⇒ 复用执行器那份 `advance`（内建迭代器 ＋ `__next__` 协议 ✓ **一处真相** ✓）；
        // 既不是迭代器也不是可迭代 ⇒ `None`（调用方照常报"不是可迭代" ✓）。
        let mut items = Vec::new();
        loop {
            match crate::executor::advance(self, object) {
                Ok(Some(item)) => items.push(item),
                Ok(None) => return Some(items),
                Err(_) => return None,
            }
        }
    }

    /// 两个值的**序**（`min`／`max`／`sorted` 要用）：数值塔按数比、两个 `str` 按字典序。
    ///
    /// 其余给 `None`——调用方据此报参照实现那条
    /// `TypeError: '<' not supported between instances of 'str' and 'int'`（实测）。
    pub fn order_of(
        &self,
        left: NonNull<Header>,
        right: NonNull<Header>,
    ) -> Option<core::cmp::Ordering> {
        let number = |object: NonNull<Header>| -> Option<f64> {
            // SAFETY: object 是存活对象。
            let ty = unsafe { object.as_ref() }.ty();
            if let Some(value) = self.int_value(object) {
                if ty == self.singletons().bool_type() || ty == self.singletons().int_type() {
                    return Some(value as f64);
                }
            }
            if let Some(value) = self.float_value(object) {
                if ty == self.type_named("float")? {
                    return Some(value);
                }
            }
            None
        };
        if let (Some(left_number), Some(right_number)) = (number(left), number(right)) {
            return left_number.partial_cmp(&right_number);
        }
        // SAFETY: 两者都是存活对象。
        let left_type = unsafe { left.as_ref() }.ty();
        let right_type = unsafe { right.as_ref() }.ty();
        let text_type = self.singletons().str_type();
        if left_type == text_type && right_type == text_type {
            // SAFETY: 类型身份已确认。
            let left_text = unsafe { &*left.as_ptr().cast::<StrObject>() }.value().to_owned();
            // SAFETY: 同上。
            let right_text = unsafe { &*right.as_ptr().cast::<StrObject>() }.value().to_owned();
            return Some(left_text.cmp(&right_text));
        }
        None
    }

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

    /// **新增一份引用**并交回同一对象（给"按原样返回实参"的原生函数用，`OM-16`）。
    pub fn retain(&self, object: NonNull<Header>) -> NonNull<Header> {
        // SAFETY: 调用方保证 object 存活。
        unsafe { self.incref_object(object.as_ptr()) };
        object
    }

    /// 归还一份引用（[`Self::retain`] 的配对）。
    ///
    /// 与 `retain` 一样是**安全函数**：契约（"这份引用确实是你持有的"）由调用方保证——
    /// `#![forbid(unsafe_code)]` 的 stdlib 要管理中途丢弃的中间数量，必须有这条配对。
    pub fn release(&self, object: NonNull<Header>) {
        // SAFETY: 调用方保证这份引用归它所有（见本函数的契约）。
        unsafe { self.release_object(object.as_ptr()) };
    }

    /// 对象的类型（**借用**）。
    pub fn type_of(&self, object: NonNull<Header>) -> NonNull<TypeObject> {
        // SAFETY: 调用方保证 object 存活。
        unsafe { object.as_ref() }.ty()
    }

    /// 是不是 `bool`（`True`／`False` 是 `int` 的子类，别的地方要分开判）。
    /// `bool` 的**值**（不是 `bool` 就给 `None`）。
    /// **迭代推进**（第 142 轮）：直接复用执行器那份（`executor::advance` ✓ **一处真相** ✓）——
    /// 内建 `next()` 要的就是它 ✓。
    pub fn advance_iterator(
        &self,
        object: NonNull<Header>,
    ) -> Result<Option<NonNull<Header>>, ExecError> {
        crate::executor::advance(self, object)
    }

    /// **取迭代器**（第 142 轮）：直接复用执行器那份（`executor::iter_value` ✓ **一处真相** ✓）——
    /// 内建 `iter()` 要的就是它 ✓（`iter(迭代器) is 它自己` ✓ 由那份实现保证 ✓）。
    pub fn iter_object(&self, object: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
        crate::executor::iter_value(self, object)
    }

    /// **当前帧的全局映射**（第 156 轮，**借用**）：`globals()` 的取值口 ✓。
    pub fn current_globals(&self) -> Option<NonNull<Header>> {
        self.current_globals.get()
    }

    /// 挂上／恢复当前帧的全局映射（第 156 轮）；**只给执行器的 RAII 守卫用** ✓。
    pub fn set_current_globals(&self, globals: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.current_globals.replace(globals)
    }

    /// **取属性（可选）**（第 148 轮）：直接复用执行器那条属性通道 ✓（**一处真相** ✓）——
    /// 内建 `getattr`／`hasattr` 要的就是它 ✓。**必须走它** ✗：早先我直接拿对象当 `dict` 查
    /// （`dict_get` ✗ 会把指针强转成 `DictObject` 读 ⇒ **UB** ✓，实测触发 abort ✓）。
    pub fn attribute_optional_of(
        &self,
        object: NonNull<Header>,
        name: &str,
    ) -> Result<Option<NonNull<Header>>, ExecError> {
        crate::executor::attribute_optional(self, object, name)
    }

    /// **存属性**（第 148 轮）：复用 `STORE_ATTR` 那条路 ✓（`opcode` 只用于错误消息 ⇒ 给 0 ✓）。
    pub fn set_attribute_value(
        &self,
        object: NonNull<Header>,
        name: &str,
        value: NonNull<Header>,
    ) -> Result<(), ExecError> {
        // **`value` 是调用方借用的** ✓（`setattr` 那条路传的是 `args[2]` ✓）——而
        // `instance_attribute_set` 是**接管语义** ✓ ⇒ 这里必须**先给自己那份** ✗
        //（第 209 轮真 bug 修复 ✗：先前没加 ⇒ 属性表里的指针**没有计数** ✗ ⇒ 值被提前释放 ⇒
        // 属性表／函数字典**释放后重用** ⇒ 堆损坏 ✓。口径与 `attribute_write` 完全一致 ✓ = 一处真相 ✓）。
        // SAFETY: 调用方保证 value 存活；属性表要自己那份。
        unsafe { self.incref_object(value.as_ptr()) };
        crate::executor::instance_attribute_set(self, object, name, value, 0)
    }

    /// **对象真假**（第 131 轮）：直接复用执行器那份判定 ✓（**一处真相** ✓）——
    /// 内建 `bool()` 要的就是它（`bool_value` 只覆盖 bool／None ✗ ⇒ `bool(0)` 会错 ✗）。
    pub fn truthiness_of(&self, object: NonNull<Header>) -> Result<bool, ExecError> {
        crate::executor::truthiness(self, object, 0)
    }

    pub fn bool_value(&self, object: NonNull<Header>) -> Option<bool> {
        if !self.is_bool(object) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<BoolObject>() }.value)
    }

    /// **真值**（`TO_BOOL` 的同一处真相：`all`／`any` 要用）。
    ///
    /// 假：`None`／`False`／数值零／空串／空容器；其余真（没有 `__bool__`／`__len__` 的对象
    /// 按参照实现是**真**）。
    pub fn truth_of(&self, object: NonNull<Header>) -> bool {
        let ty = self.type_of(object);
        if ty == self.singletons().none_type() {
            return false;
        }
        if let Some(flag) = self.bool_value(object) {
            return flag;
        }
        if ty == self.singletons().int_type() {
            // 大整数不能看 `i64` 那个快路径（`int_value` 对它给 `None` ⇒ 会被当成 0＝假）
            return self.int_of(object).map(|value| !value.is_zero()).unwrap_or(false);
        }
        if self.type_named("float") == Some(ty) {
            return self.float_value(object).unwrap_or(0.0) != 0.0;
        }
        if ty == self.singletons().str_type() {
            // SAFETY: 类型身份已确认。
            return !unsafe { &*object.as_ptr().cast::<StrObject>() }.value().is_empty();
        }
        if let Some(length) = self.length_of(object) {
            return length != 0;
        }
        true
    }

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
        let (left_is_int, left_number) = as_number(left)?;
        let (right_is_int, right_number) = as_number(right)?;
        if left_is_int && right_is_int {
            return Some(self.new_int(left_number as i64 + right_number as i64));
        }
        Some(self.new_float(left_number + right_number))
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

    // ---- 容器载荷的**安全**面（`pyawa-stdlib` 是 `forbid(unsafe_code)`，它只能走这些）----

    /// 摊开一个 `list` 的元素（**借用**一份拷贝；不是 `list` 给 `None`）。
    pub fn list_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        if Some(self.type_of(object)) != self.type_named("list") {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<ListObject>() }.items().to_vec())
    }

    /// 摊开一个 `dict` 的键值对。
    pub fn dict_entries(
        &self,
        object: NonNull<Header>,
    ) -> Option<Vec<(NonNull<Header>, NonNull<Header>)>> {
        if Some(self.type_of(object)) != self.type_named("dict") {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<DictObject>() }.entries())
    }

    /// 摊开一个 `set` 的元素。
    pub fn set_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        if Some(self.type_of(object)) != self.type_named("set") {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<SetObject>() }.items().to_vec())
    }

    /// 往 `list` 追加一项（**接管** `item` 的那份引用）。
    pub fn list_append(&self, list: NonNull<Header>, item: NonNull<Header>) {
        // SAFETY: 调用方保证 list 是本实例的 `list`（名字与类型都在契约里）。
        unsafe { &*list.as_ptr().cast::<ListObject>() }.append(item);
    }

    /// 往 `dict` 写入一对（**接管** key／value 各一份引用；不查重）。
    pub fn dict_insert_raw(
        &self,
        dict: NonNull<Header>,
        key: NonNull<Header>,
        value: NonNull<Header>,
    ) {
        // SAFETY: 同上。
        unsafe { &*dict.as_ptr().cast::<DictObject>() }.insert_raw(key, value);
    }

    /// 往 `set` 写入一项（**接管**一份引用；不查重）。
    pub fn set_insert_raw(&self, set: NonNull<Header>, item: NonNull<Header>) {
        // SAFETY: 同上。
        unsafe { &*set.as_ptr().cast::<SetObject>() }.insert_raw(item);
    }

    pub fn is_bool(&self, object: NonNull<Header>) -> bool {
        self.type_of(object) == self.singletons().bool_type()
    }

    /// 读整数载荷（`int` 与 `bool` 都算；别的给 `None`）。
    ///
    /// **这是 `i64` 快路径**：大整数（`TS-45`）在这里给 `None`——那**不代表"不是整数"**。
    /// 要按类型分派的地方用 [`Instance::int_of`]。
    pub fn int_value(&self, object: NonNull<Header>) -> Option<i64> {
        self.int_of(object).and_then(|value| value.to_i64())
    }

    /// 读整数载荷（含**大整数**；`int` 与 `bool` 都算）。
    pub fn int_of(&self, object: NonNull<Header>) -> Option<IntValue> {
        let ty = self.type_of(object);
        if ty == self.singletons().int_type() {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<IntObject>() }.value.clone());
        }
        if ty == self.singletons().bool_type() {
            // SAFETY: 同上。
            return Some(IntValue::Small(i64::from(
                unsafe { &*object.as_ptr().cast::<BoolObject>() }.value,
            )));
        }
        None
    }

    /// 读浮点载荷（`float` 才算；别的给 `None`）。
    pub fn float_value(&self, object: NonNull<Header>) -> Option<f64> {
        if self.type_of(object) == self
            .type_named("float")
            .expect("float 已登记")
        {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<FloatObject>() }.value);
        }
        None
    }

    /// 读字符串内容（**复制**；不是 `str` 给 `None`）。
    pub fn text_value(&self, object: NonNull<Header>) -> Option<String> {
        if self.type_of(object) != self.singletons().str_type() {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<StrObject>() }.value().to_owned())
    }

    /// 容器／字符串长度（`str` 按**字节**数；别的给 `None`）。
    pub fn length_of(&self, object: NonNull<Header>) -> Option<usize> {
        let ty = self.type_of(object);
        if ty == self.singletons().str_type() {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<StrObject>() }.value().len());
        }
        if Some(ty) == self.type_named("bytes") {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<BytesObject>() }.value().len());
        }
        if Some(ty) == self.type_named("dict") || Some(ty) == self.type_named("set") {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<DictObject>() }.entries().len());
        }
        if Some(ty) == self.type_named("list") {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<ListObject>() }.len());
        }
        if Some(ty) == self.type_named("tuple") {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<TupleObject>() }.len());
        }
        None
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

    /// **`bytes` 的载荷**（**借用**；不是 `bytes` 给 `None`）。
    pub fn bytes_value(&self, object: NonNull<Header>) -> Option<&[u8]> {
        if Some(self.type_of(object)) == self.type_named("bytes") {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<BytesObject>() }.value());
        }
        None
    }

    /// 把**内建容器**摊成元素表（`bytes(<可迭代>)` 用）。
    ///
    /// 只接 `list`／`tuple`；其余可迭代对象（`bytearray`／`range`／生成器…）如实报未实现
    /// （其中多数类型本层还没有，见 `TS-42` 的阶梯）。
    pub fn collect_iterable(
        &self,
        object: NonNull<Header>,
    ) -> Result<Vec<NonNull<Header>>, ExecError> {
        let ty = self.type_of(object);
        if Some(ty) == self.type_named("list") {
            // SAFETY: 类型身份已确认。
            return Ok(unsafe { &*object.as_ptr().cast::<ListObject>() }.items().to_vec());
        }
        if Some(ty) == self.type_named("tuple") {
            // SAFETY: 同上。
            let tuple = unsafe { &*object.as_ptr().cast::<TupleObject>() };
            return Ok((0..tuple.len())
                .filter_map(|index| tuple.item(index))
                .collect());
        }
        Err(ExecError::Unsupported {
            opcode: 0,
            what: "bytes(<可迭代>)：只接线了 list／tuple（其余走迭代器协议，随后补）",
        })
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

    /// 往 `dict` 里按**字符串**键写一个值（**接管** `value` 的引用，`OM-16`）。
    ///
    /// 键已存在则替换（旧值由这里释放）。给 stdlib 建模块用。
    pub fn dict_set(&self, mapping: NonNull<Header>, key: &str, value: NonNull<Header>) {
        // SAFETY: 调用方保证 mapping 是本实例里存活的 dict。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        // 查重用一个**临时键**（借用视图）：查完立刻归还，字典自己另存一份
        let probe = self.new_str(key);
        let position = dict
            .entries()
            .iter()
            .position(|(existing, _)| crate::executor::values_equal_public(self, *existing, probe));
        // SAFETY: probe 是新引用，比较完即归还。
        unsafe { self.release_object(probe.as_ptr()) };
        if let Some(position) = position {
            if let Some((old_key, old_value)) = dict.remove(position) {
                // SAFETY: 旧键值由字典持有。
                unsafe {
                    self.release_object(old_key.as_ptr());
                    self.release_object(old_value.as_ptr());
                }
            }
        }
        let stored_key = self.new_str(key);
        dict.insert_raw(stored_key, value);
    }

    /// 往 `dict` 里按**整数**键写一个值（**接管** `value`；`errorcode` 这类用）。
    pub fn dict_set_int(&self, mapping: NonNull<Header>, key: i64, value: NonNull<Header>) {
        // SAFETY: 调用方保证 mapping 是本实例里存活的 dict。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let probe = self.new_int(key);
        let position = dict
            .entries()
            .iter()
            .position(|(existing, _)| crate::executor::values_equal_public(self, *existing, probe));
        // SAFETY: probe 是新引用，比较完就归还。
        unsafe { self.release_object(probe.as_ptr()) };
        if let Some(position) = position {
            if let Some((old_key, old_value)) = dict.remove(position) {
                // SAFETY: 旧键值由字典持有。
                unsafe {
                    self.release_object(old_key.as_ptr());
                    self.release_object(old_value.as_ptr());
                }
            }
        }
        let stored_key = self.new_int(key);
        dict.insert_raw(stored_key, value);
    }

    /// 按**字符串**键读 `dict`（**借用**；不存在给 `None`）。
    pub fn dict_get(&self, mapping: NonNull<Header>, key: &str) -> Option<NonNull<Header>> {
        // SAFETY: 调用方保证 mapping 存活。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let probe = self.new_str(key);
        let found = dict
            .entries()
            .iter()
            .position(|(existing, _)| crate::executor::values_equal_public(self, *existing, probe));
        // SAFETY: probe 是新引用，比较完就归还。
        unsafe { self.release_object(probe.as_ptr()) };
        found.and_then(|position| dict.entry(position).map(|(_, value)| value))
    }

    /// **`DESIGN.md` §9 第 20 条**：注入平台相关**只读常量**（`errno` 一类）。
    ///
    /// 由 `pyawa-runtime` 在启动时调用；**按名字**查、数字随平台（`CM-20`）。
    /// 注入的是一份**拷贝**，并按名字排序以便二分查找；**不新增能力域**（`REQUIREMENTS.md`）。
    pub fn set_platform_constants(&self, constants: &[(&'static str, i64)]) {
        let mut table: Vec<(&'static str, i64)> = constants.to_vec();
        table.sort_unstable_by_key(|(name, _)| *name);
        *self.platform_constants.borrow_mut() = table;
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

    /// 按**名字**取平台常量（`CM-20`：映射按名字匹配，**禁止**硬编码数字）。
    pub fn platform_constant(&self, name: &str) -> Option<i64> {
        let table = self.platform_constants.borrow();
        table
            .binary_search_by_key(&name, |(candidate, _)| *candidate)
            .ok()
            .map(|position| table[position].1)
    }

    /// **整张平台常量表**（第 134 轮）：`errno` 模块要按**整表**建名字空间 ✓
    /// （`platform_constant` 只按名查 ✗ ⇒ `errno_module::build` 收的是一张切片 ✓）。
    pub fn platform_constants(&self) -> Vec<(&'static str, i64)> {
        self.platform_constants.borrow().clone()
    }

    /// 平台常量条数（测试与诊断用）。
    pub fn platform_constants_len(&self) -> usize {
        self.platform_constants.borrow().len()
    }

    /// **`TS-45` ①**：当前 `int`↔`str` 的位数上限（`0` ＝ 不限）。
    pub fn int_max_str_digits(&self) -> u32 {
        self.int_max_str_digits.get()
    }

    /// 设置位数上限（**只存值**；"0 或 ≥ 阈值"的规则由 `sys.set_int_max_str_digits` 把关，
    /// 与参照一致——那条规则报的是 `ValueError`，属脚本可见语义）。
    pub fn set_int_max_str_digits(&self, value: u32) {
        self.int_max_str_digits.set(value);
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

    /// 直接分配一个 `int` 对象（**不查单例表**）。
    ///
    /// `new_int` 与 `new_int_value` **不能互相调**（非单例值会来回递归到爆栈——
    /// 第 200 轮实测踩过：`300` 一路 `new_int` ↔ `new_int_value`）。
    fn alloc_int(&self, value: IntValue) -> NonNull<Header> {
        let int_type = self.singletons().int_type();
        self.alloc(IntObject::new(int_type, value))
            .into_raw()
            .cast::<Header>()
    }

    /// 造一个 `str`（空串走 `OM-23` 的单例）——**新引用**。
    /// **安全**地取一个 `str` 对象的文本（`None` ＝ 不是 `str`）✓。
    ///
    /// stdlib 侧 `#![forbid(unsafe_code)]`（`CX-4` 的静态扫描范围 ✓）⇒ 这类"进对象"的出口
    /// 留在核心 ✓。
    pub fn text_of(&self, object: NonNull<Header>) -> Option<&str> {
        if self.type_of(object) != self.singletons().str_type() {
            return None;
        }
        // SAFETY: 类型身份刚确认是 `str` ✓。
        Some(unsafe { &*object.as_ptr().cast::<crate::StrObject>() }.value())
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

    /// **`OM-11` 的 `str` 槽**：`str(对象)`。
    ///
    /// `SPEC-type-system.md` §8：该槽**省略时回退到 `repr`**。失败经 `Result` 上抛
    /// （`OM-11` 扩：`TS-45` ①的输出方向要能抛 `ValueError`）。
    pub fn object_str(&self, object: NonNull<Header>) -> Result<String, ExecError> {
        // **`TS-44`**：先走**属性通道**（类型字典里的 `__str__` 覆写）—— `override_text` 会
        // **忽略 `object` 自己那条** ✓（那是第 210 轮新挂的**属性面** ✓、不是格式化覆写 ✓）⇒
        // 内建类型仍走各自的 `str` 槽 ✓（否则 `object.__str__` 在每个 MRO 命中 ⇒ `f"{x}"` 给出
        // `\'1\'` ✗，实测四条 f-string 语料会红 ✓）。
        if let Some(text) = crate::executor::override_text(self, object, "__str__")? {
            return Ok(text);
        }
        self.object_str_native(object)
    }

    /// `str(对象)` 的**槽位**路径（`TS-44`：不走属性通道）——给已经是"通道内层"的调用方用，
    /// 免得 `element_repr` 这类已经查过覆写的地方再查一次（那会自递归）。
    pub fn object_str_native(&self, object: NonNull<Header>) -> Result<String, ExecError> {
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        // SAFETY: ty 由注册表持有。
        if let Some(slot) = unsafe { ty.as_ref() }.slots().str {
            // SAFETY: 槽位契约见 `StrFn`。
            return unsafe { slot(object.as_ptr(), self) };
        }
        self.object_repr_native(object)
    }

    /// **`OM-11` 的 `repr` 槽**：`repr(对象)`；槽位省略时给默认形式（`SPEC-type-system.md` §8）。
    pub fn object_repr(&self, object: NonNull<Header>) -> Result<String, ExecError> {
        // **`TS-44`**：先走**属性通道**（类型字典里的 `__repr__` 覆写）——与
        // `repr([obj])` 里元素的口径一致（此前顶层 `repr(obj)` 会**忽略**覆写，那是不一致）。
        if let Some(text) = crate::executor::override_text(self, object, "__repr__")? {
            return Ok(text);
        }
        self.object_repr_native(object)
    }

    /// `repr(对象)` 的**槽位**路径（`TS-44`：不走属性通道）。
    pub fn object_repr_native(&self, object: NonNull<Header>) -> Result<String, ExecError> {
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        // SAFETY: ty 由注册表持有。
        if let Some(slot) = unsafe { ty.as_ref() }.slots().repr {
            // SAFETY: 槽位契约见 `ReprFn`。
            return unsafe { slot(object.as_ptr(), self) };
        }
        // 默认形式：`<X object at 0x…>`（类型名；模块／qualname 随类创建钩子接线后补）
        // SAFETY: 同上。
        Ok(format!(
            "<{} object at {:p}>",
            unsafe { ty.as_ref() }.name(),
            object.as_ptr()
        ))
    }

    /// `ascii(对象)`：`repr` 且非 ASCII 字符转义。
    pub fn object_ascii(&self, object: NonNull<Header>) -> Result<String, ExecError> {
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        if ty == self.singletons().str_type() {
            // SAFETY: 类型身份已确认。
            let text = unsafe { &*object.as_ptr().cast::<StrObject>() }.value().to_owned();
            return Ok(quote_str(&text, true));
        }
        self.object_repr(object)
    }

    /// **`OM-11`**：`repr` 的递归守卫——已经在生成中的对象返回 `false`
    /// （容器据此给出 `[...]`／`{...}`，与参照实现一致）。
    pub fn enter_repr(&self, address: usize) -> bool {
        let mut guard = self.repr_guard.borrow_mut();
        if guard.contains(&address) {
            return false;
        }
        guard.push(address);
        true
    }

    /// 退出 `repr` 的递归守卫。
    pub fn leave_repr(&self, address: usize) {
        let mut guard = self.repr_guard.borrow_mut();
        if let Some(position) = guard.iter().rposition(|entry| *entry == address) {
            guard.remove(position);
        }
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
    /// `inner` 必须是本层认的迭代器（`executor::iter_value` 交出来的就是）。
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

    /// **`OM-22`**：对象的**引用计数**（**安全**读取）。
    ///
    /// 给 stdlib 的 `sys.getrefcount` 用——那个 crate 是 `#![forbid(unsafe_code)]`，
    /// 不能自己去 `as_ref()`。
    pub fn refcount_of(&self, object: NonNull<Header>) -> u32 {
        // SAFETY: 调用方按 `OM-16` 保证 object 是本实例里的存活对象。
        unsafe { object.as_ref() }.refcount()
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

    /// 按名字在注册表里找一个类型。
    ///
    /// 这是**内部**查询（`TS-41` 的对拍与引导期要用）；Python 可见的属性访问**必须**走
    /// `OM-11` 的 `getattr` 槽位，**禁止**用这个函数旁路属性通道。
    pub fn type_named(&self, name: &str) -> Option<NonNull<TypeObject>> {
        self.types
            .borrow()
            .iter()
            .copied()
            .find(|ty| {
                // SAFETY: 注册表里的类型都存活。
                unsafe { ty.as_ref() }.name() == name
            })
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

    /// **TS-40**／**TS-29**：`subtype` 是不是 `supertype` 的子类型（含自身）。
    ///
    /// 走 **MRO**（**OM-13** 的 C3 产物）——所以 `bool ⊂ int`、任何类型 `⊂ object` 都自动成立。
    /// `__subclasshook__`／ABC 注册（`numbers.Integral` 一类）随后补。
    pub fn is_subtype(&self, subtype: NonNull<TypeObject>, supertype: NonNull<TypeObject>) -> bool {
        if subtype == supertype {
            return true;
        }
        // SAFETY: 两个类型都由本实例的注册表持有。
        unsafe { subtype.as_ref() }.mro().contains(&supertype)
    }

    /// **`AB-5`①**：请求中断本实例（幂等）。
    pub fn request_interrupt(&self) {
        self.interrupted.set(true);
    }

    /// 本实例是否被请求中断（执行器每条指令看它）。
    pub fn interrupted(&self) -> bool {
        self.interrupted.get()
    }

    /// 清掉中断请求（宿主重新开始执行前用；`pa_interrupt` 的配套）。
    pub fn clear_interrupt(&self) {
        self.interrupted.set(false);
    }

    /// **BC-60** ②：**本实例**当前正在处理的异常（**借用**）。
    pub fn current_exception(&self) -> Option<NonNull<Header>> {
        self.exception_state.borrow().last().copied()
    }

    /// **BC-60** ②：当前异常状态的层数（诊断用——`PUSH_EXC_INFO`／`POP_EXCEPT` 配对着用）。
    pub fn exception_depth(&self) -> usize {
        self.exception_state.borrow().len()
    }

    /// **BC-60** ②：压入一个正在处理的异常（**新引用**，由实例接手）。
    pub fn push_exception(&self, exception: NonNull<Header>) {
        self.exception_state.borrow_mut().push(exception);
    }

    /// **BC-60** ②：弹出当前异常（交出一份**新引用**）。
    pub fn pop_exception(&self) -> Option<NonNull<Header>> {
        self.exception_state.borrow_mut().pop()
    }

    /// 最近一次抛出的异常（**借用**；`ExecError::Raised` 借它保活）。
    pub fn pending_exception(&self) -> Option<NonNull<Header>> {
        self.pending_exception.get()
    }

    /// 记下最近一次抛出的异常（**新引用**，由实例接手；旧的那份交出去由调用方释放）。
    pub fn set_pending_exception(
        &self,
        exception: Option<NonNull<Header>>,
    ) -> Option<NonNull<Header>> {
        self.pending_exception.replace(exception)
    }

    /// **`LOAD_BUILD_CLASS`** 压的那个内建（`__build_class__`；**借用**）。
    ///
    /// 参照实现从 `builtins` 取它；Pyawa 还没有 `builtins` 模块（`P3-14`），故先按实例存一个。
    pub fn build_class(&self) -> Option<NonNull<Header>> {
        self.build_class.get()
    }

    /// **OM-23**：本实例的单例表。
    pub fn singletons(&self) -> &Singletons {
        self.singletons
            .get()
            .expect("单例表在 Instance::new 中引导，必然存在")
    }

    /// **OM-40**：从裸引用**现取**一个守卫（取得一份新引用），用完即还。
    ///
    /// 载荷里只能存裸引用；要真正使用它，必须经这个访问器借出守卫。
    pub fn own(&self, raw: NonNull<Header>) -> PyRef<'_> {
        // SAFETY: 调用方（载荷的 traverse／clear）保证 raw 指向本实例的存活对象；
        // 这里为它新增一份引用，交给守卫负责归还。
        unsafe { self.incref_object(raw.as_ptr()) };
        // SAFETY: 同上。
        unsafe { PyRef::from_raw(raw, self) }
    }

    /// 元类型：类型对象自身的类型。
    pub fn metatype(&self) -> NonNull<TypeObject> {
        self.metatype
            .get()
            .expect("元类型在 Instance::new 中引导，必然存在")
    }

    /// **OM-3**：本实例当前占用的字节数（能力接口不承担预算，见 `CP-8`）。
    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated.get()
    }

    /// 本实例中尚未释放的普通对象数（类型对象不计）。
    pub fn live_objects(&self) -> usize {
        self.live.borrow().len()
    }

    /// **OM-25**：当前参与循环回收（`GC_TRACKED`）的对象数。
    pub fn tracked_objects(&self) -> usize {
        self.gc_count.get()
    }

    /// **OM-15**：本实例注册的类型对象数。
    pub fn type_count(&self) -> usize {
        self.types.borrow().len()
    }

    /// **OM-26**：回收阈值三元组。默认值见 [`DEFAULT_GC_THRESHOLD`]。
    ///
    /// 后两位**存而不生效**（单代，**临时**）；它们照样要能读回来，纯 Python 层会解三元组。
    pub fn gc_threshold(&self) -> (usize, usize, usize) {
        self.gc_threshold.get()
    }

    /// **OM-26**：设置阈值三元组。`t0` **0 会被拒绝**——那等于每次分配都回收；
    /// `t1`／`t2` 只存不生效（单代，**临时**）。
    pub fn set_gc_threshold(&self, threshold: (usize, usize, usize)) {
        assert!(threshold.0 > 0, "OM-26：阈值必须可配置且不为 0");
        self.gc_threshold.set(threshold);
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
            self.collect();
        }

        Owned::new(ptr, self)
    }

    /// 把一个已构造好的对象交给本实例托管（记账 ＋ 入链 ＋ 标 `GC_TRACKED`），
    /// 返回它的指针；**引用由实例自己持有**。
    ///
    /// 引导期（`Instance::new` 造单例时）用不了 `Owned`——那需要先借出 `&Instance`。
    fn adopt<T: PyObject>(&self, value: T) -> NonNull<T> {
        let ty = value.header().ty();
        let size = core::mem::size_of::<T>();
        debug_assert_eq!(
            size,
            // SAFETY: ty 由某个实例的类型注册表持有（OM-15），在实例存活期间有效；
            // 这里在 debug 下用它校验类型元数据与 Rust 布局一致。
            unsafe { ty.as_ref() }.instance_size,
            "类型的 instance_size 与 Rust 布局不一致"
        );

        let ptr = NonNull::from(Box::leak(Box::new(value)));
        let header = ptr.cast::<Header>();

        // **OM-12**：可成环的类型必须标记 GC_TRACKED。本层用"是否提供 traverse 槽位"判定；
        // 类型对象自身也会成环（bases／dict），但它的 traverse／clear 待接线后再补标记。
        let tracked = unsafe { ty.as_ref() }.slots.traverse.is_some();
        if tracked {
            // SAFETY: header 指向刚刚分配、尚未交给其他代码的对象。
            unsafe { header.as_ref() }.set_flag(flags::GC_TRACKED);
        }

        self.live.borrow_mut().insert(header.as_ptr() as usize);
        self.bytes_allocated.set(self.bytes_allocated.get() + size);
        if tracked {
            self.link_gc(header);
        }
        ptr
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

    /// **OM-22**：`sys.getrefcount` 的可见语义——返回值**含参数借用**的那一份。
    pub fn getrefcount<T: PyObject>(&self, object: &T) -> u32 {
        object.header().refcount() + 1
    }

    /// 增加一个引用。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中**存活**的对象。
    pub unsafe fn incref_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        unsafe { &*ptr }.incref();
    }

    /// **OM-20** ①：把**正在终结**的对象复活——计数从 0 回到 1。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中一个正在执行终结器的对象（`FINALIZING` 已置位）。
    pub unsafe fn resurrect_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        let header = unsafe { &*ptr };
        debug_assert!(
            header.has_flag(flags::FINALIZING),
            "只有终结器执行中的对象可以被复活（OM-20）"
        );
        header.set_refcount(header.refcount() + 1);
    }

    /// 释放一个**新引用**（**OM-16**）；计数归零时按 **OM-20** 的顺序处理：
    /// ① 终结器（可复活）→ ② `clear` → ③ 释放。清空走 **OM-21** 的待处理栈，不朴素递归。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中**存活**的对象，且调用方交出的是一份**新引用**。
    pub unsafe fn release_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        let header = unsafe { &*ptr };
        // **OM-24**：M1 的 `IMMORTAL` 位恒为 0；这里只是防御，不承担语义。
        if header.is_immortal() {
            return;
        }
        if header.decref() != 0 {
            return;
        }

        // 回收进行中：不可达对象由本次 `collect` 统一释放，这里只减计数（OM-27 ④）。
        if !self.gc_frozen.borrow().is_empty() && self.gc_frozen.borrow().contains(&(ptr as usize)) {
            return;
        }

        // SAFETY: ptr 非空（调用方保证）。
        self.pending
            .borrow_mut()
            .push(unsafe { NonNull::new_unchecked(ptr) });

        if self.draining.get() {
            // 已经在清空栈里：交给最外层那次调用处理。
            return;
        }
        self.draining.set(true);
        loop {
            let next = self.pending.borrow_mut().pop();
            let Some(object) = next else { break };
            self.release_one(object);
        }
        self.draining.set(false);
    }

    /// **OM-25**…**OM-30**：跑一次标记-清除，返回本次释放的对象数。
    ///
    /// 顺序按 **OM-27** 固定：① 求不可达集合 → ② 先清弱引用 → ③ 调终结器 → ④ 释放。
    /// 回收范围仅限 `GC_TRACKED` 对象（**OM-25**）；不可达但尚未释放的对象**禁止**暴露（**OM-30**）。
    pub fn collect(&self) -> usize {
        if self.gc_running.get() {
            // 终结器／clear 里又触发了一次回收：本次让路，交给外层那次。
            return 0;
        }
        self.gc_running.set(true);
        let freed = self.collect_inner();
        self.gc_running.set(false);
        freed
    }

    /// [`Instance::collect`] 的主体；进入前 `gc_running` 已置位。
    fn collect_inner(&self) -> usize {
        let unreachable = self.find_unreachable();
        self.gc_alloc_count.set(0);
        if unreachable.is_empty() {
            return 0;
        }

        // ② 先清弱引用：§10 尚未接线（`HAS_WEAKREFS` 位也还没人置位），
        //    这里是顺序上的占位点——弱引用回调必须早于终结器（OM-27、PEP 442）。

        // ③ 终结器：对每个不可达对象至多调用一次；终结器可以复活对象（OM-20 ①）。
        //    终结期间同样"冻结"这批对象：终结器可能释放环内引用，提前释放会让我们
        //    手里的指针失效——OM-27 要求先全部终结、再统一释放。
        *self.gc_frozen.borrow_mut() = unreachable
            .iter()
            .map(|header| header.as_ptr() as usize)
            .collect();
        for header in &unreachable {
            let ty = unsafe { header.as_ref() }.ty();
            if let Some(finalize) = unsafe { ty.as_ref() }.slots.finalize {
                let header_ref = unsafe { header.as_ref() };
                if !header_ref.has_flag(flags::FINALIZING) {
                    header_ref.set_flag(flags::FINALIZING);
                    // SAFETY: header 是本实例的存活对象。
                    unsafe { finalize(header.as_ptr(), self) };
                }
            }
        }

        // 终结器可能复活对象、也可能让别的对象重新变可达（PEP 442）⇒ 重算不可达集合。
        let unreachable = self.find_unreachable();
        let garbage: HashSet<usize> = unreachable.iter().map(|h| h.as_ptr() as usize).collect();

        // 复活的对象要清掉 FINALIZING，之后它再次死亡时还能再终结一次（OM-20）。
        for header in self.gc_headers() {
            let header_ref = unsafe { header.as_ref() };
            if header_ref.has_flag(flags::FINALIZING) && !garbage.contains(&(header.as_ptr() as usize))
            {
                header_ref.clear_flag(flags::FINALIZING);
            }
        }

        if unreachable.is_empty() {
            self.gc_frozen.borrow_mut().clear();
            return 0;
        }

        // ④ 释放。先"冻结"这批对象：`clear` 之间的 decref 只减计数，不立即释放——
        //    否则同一环里的对象会被逐个提前释放，而本函数还持有它们的指针。
        *self.gc_frozen.borrow_mut() = garbage;
        for header in &unreachable {
            let ty = unsafe { header.as_ref() }.ty();
            if let Some(clear) = unsafe { ty.as_ref() }.slots.clear {
                // SAFETY: header 是本实例的存活对象，且尚未释放（刚被冻结）。
                unsafe { clear(header.as_ptr(), self) };
            }
            // **OM-14**：另行挂载的实例字典在同一阶段交出去（`OM-27` ④）
            if let Some(mapping) = unsafe { header.as_ref() }.take_instance_dict() {
                // SAFETY: 这份引用由该对象持有。
                unsafe { self.release_object(mapping.as_ptr()) };
            }
        }
        self.gc_frozen.borrow_mut().clear();

        for header in &unreachable {
            self.free_garbage(*header);
        }
        unreachable.len()
    }

    /// **OM-20**：单个对象的释放三步（正常引用计数路径）。
    fn release_one(&self, ptr: NonNull<Header>) {
        let ty = unsafe { ptr.as_ref() }.ty();

        // ① 终结器（`__del__`）：置 FINALIZING 防止重入；可以复活（把计数改回 > 0）。
        if let Some(finalize) = unsafe { ty.as_ref() }.slots.finalize {
            let header = unsafe { ptr.as_ref() };
            if !header.has_flag(flags::FINALIZING) {
                header.set_flag(flags::FINALIZING);
                // SAFETY: ptr 是本实例的存活对象，计数已归零且仍在待处理栈上。
                unsafe { finalize(ptr.as_ptr(), self) };
                let header = unsafe { ptr.as_ref() };
                if header.refcount() != 0 {
                    // 复活：清除 FINALIZING 并**放弃释放**（OM-20）。
                    header.clear_flag(flags::FINALIZING);
                    return;
                }
            }
        }

        // ② 清空持有的引用：释放它们（可能再入待处理栈）。
        if let Some(clear) = unsafe { ty.as_ref() }.slots.clear {
            // SAFETY: 同 ①。
            unsafe { clear(ptr.as_ptr(), self) };
        }
        // ②′ **OM-14**：另行挂载的实例字典也是本对象持有的一份引用
        if let Some(mapping) = unsafe { ptr.as_ref() }.take_instance_dict() {
            // SAFETY: 这份引用由本对象持有。
            unsafe { self.release_object(mapping.as_ptr()) };
        }

        // ③ 释放内存。
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(ptr);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        // SAFETY: 计数为 0，且 clear 已把持有的引用交出（OM-20 ③ 的前提）。
        unsafe { dealloc(ptr.as_ptr()) };
    }

    /// **OM-27** ④：释放一个不可达对象（`clear` 已经跑过，这里不再调终结器）。
    fn free_garbage(&self, header: NonNull<Header>) {
        let ty = unsafe { header.as_ref() }.ty();
        // **OM-14**：正常路径下 clear 阶段已经把这一格交出去了；万一还在，这里补一次释放
        if let Some(mapping) = unsafe { header.as_ref() }.take_instance_dict() {
            // SAFETY: 这份引用由该对象持有（或曾经持有）。
            unsafe { self.release_object(mapping.as_ptr()) };
        }
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(header);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        // SAFETY: 该对象已由可达性分析判为不可达，且 clear 已完成。
        unsafe { dealloc(header.as_ptr()) };
    }

    /// **OM-29**／**OM-30**：求不可达的跟踪对象。
    ///
    /// 两步：先按"引用计数 − 来自跟踪对象内部的引用数"找出根（外部引用 > 0），
    /// 再从根出发按 `traverse` 标记；没被标记的就是不可达集合。
    fn find_unreachable(&self) -> Vec<NonNull<Header>> {
        let candidates = self.gc_headers();
        if candidates.is_empty() {
            return Vec::new();
        }

        let index: HashMap<usize, usize> = candidates
            .iter()
            .enumerate()
            .map(|(position, candidate)| (candidate.as_ptr() as usize, position))
            .collect();

        let mut external: Vec<u32> = candidates
            .iter()
            // SAFETY: 候选都在本实例的回收链表上，即尚未释放。
            .map(|candidate| unsafe { candidate.as_ref() }.refcount())
            .collect();

        for candidate in &candidates {
            for child in self.children_of(*candidate) {
                if let Some(&position) = index.get(&(child as usize)) {
                    external[position] = external[position].saturating_sub(1);
                }
            }
        }

        let mut marked = vec![false; candidates.len()];
        let mut stack: Vec<usize> = (0..candidates.len()).filter(|i| external[*i] > 0).collect();
        while let Some(position) = stack.pop() {
            if marked[position] {
                continue;
            }
            marked[position] = true;
            for child in self.children_of(candidates[position]) {
                if let Some(&child_position) = index.get(&(child as usize)) {
                    if !marked[child_position] {
                        stack.push(child_position);
                    }
                }
            }
        }

        candidates
            .iter()
            .zip(marked)
            .filter(|(_, reached)| !*reached)
            .map(|(candidate, _)| *candidate)
            .collect()
    }

    /// 按 `traverse` 槽位取一个对象的直接引用（**OM-12**／**OM-29**／**OM-36**）。
    fn children_of(&self, header: NonNull<Header>) -> Vec<*mut Header> {
        let ty = unsafe { header.as_ref() }.ty();
        let mut children = Vec::new();
        // **OM-14**／**OM-36**：另行挂载的实例字典也算本对象的直接引用（漏报会永久泄漏）
        if let Some(mapping) = unsafe { header.as_ref() }.instance_dict() {
            children.push(mapping.as_ptr());
        }
        if let Some(traverse) = unsafe { ty.as_ref() }.slots.traverse {
            // SAFETY: header 是本实例的存活对象；回调只收集指针，不做解引用。
            unsafe { traverse(header.as_ptr(), &mut |child| children.push(child)) };
        }
        children
    }

    /// 回收链表上的全部对象（**OM-25**：只有 `GC_TRACKED` 入链）。
    fn gc_headers(&self) -> Vec<NonNull<Header>> {
        let mut result = Vec::with_capacity(self.gc_count.get());
        let mut cursor = self.gc_head.get();
        while !cursor.is_null() {
            // SAFETY: 链上的指针都由本实例分配且尚未释放。
            result.push(unsafe { NonNull::new_unchecked(cursor) });
            cursor = unsafe { (*cursor).gc_next() };
        }
        result
    }

    fn link_gc(&self, header: NonNull<Header>) {
        let head = self.gc_head.get();
        // SAFETY: header 刚分配；head 若非空则它是链上存活对象。
        unsafe {
            header.as_ref().set_gc_prev(ptr::null_mut());
            header.as_ref().set_gc_next(head);
            if !head.is_null() {
                (*head).set_gc_prev(header.as_ptr());
            }
        }
        self.gc_head.set(header.as_ptr());
        self.gc_count.set(self.gc_count.get() + 1);
    }

    fn unlink_gc(&self, header: NonNull<Header>) {
        // SAFETY: header 在本实例的回收链表上。
        let (prev, next) = unsafe {
            let header_ref = header.as_ref();
            (header_ref.gc_prev(), header_ref.gc_next())
        };
        if prev.is_null() {
            self.gc_head.set(next);
        } else {
            // SAFETY: prev 是链上存活对象。
            unsafe { (*prev).set_gc_next(next) };
        }
        if !next.is_null() {
            // SAFETY: next 是链上存活对象。
            unsafe { (*next).set_gc_prev(prev) };
        }
        // SAFETY: 同上。
        unsafe {
            header.as_ref().set_gc_prev(ptr::null_mut());
            header.as_ref().set_gc_next(ptr::null_mut());
        }
        self.gc_count.set(self.gc_count.get() - 1);
    }

    /// 从"存活集合"与回收链表上同时摘除。
    fn unlink(&self, header: NonNull<Header>) {
        self.live.borrow_mut().remove(&(header.as_ptr() as usize));
        // SAFETY: header 尚未释放。
        if unsafe { header.as_ref() }.has_flag(flags::GC_TRACKED) {
            self.unlink_gc(header);
        }
    }

    /// **异常的消息文本** ✓（第 193 轮：**先核形状，再读载荷** ✓ —— ABI 与诊断都走这里 ✓，**一处真相** ✓）。
    ///
    /// **为什么必须核** ✗：类型名是异常却**不是** `ExceptionObject` 载荷的对象确实会出现 ✓
    /// （第 192 轮两次插桩都因此**当场段错误** ✗，退出码 139 ✓）⇒ 核两条：① 类型是 `BaseException` 的子类型 ✓；
    /// ② **载荷大小**与 `ExceptionObject` 一致 ✓。对不上就返回 `None` ✓（**绝不**硬读 ✗）。
    pub fn exception_message_of(&self, object: NonNull<Header>) -> Option<String> {
        // SAFETY: object 由调用方保证存活。
        let ty = unsafe { object.as_ref() }.ty();
        // SAFETY: ty 由注册表持有。
        let type_object = unsafe { ty.as_ref() };
        if let Some(base) = self.type_named("BaseException") {
            if !self.is_subtype(ty, base) {
                return None;
            }
        }
        if type_object.instance_size() != core::mem::size_of::<crate::builtin_objects::ExceptionObject>()
        {
            return None;
        }
        // SAFETY: 上面刚核过类型与载荷大小 ✓。
        let payload = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::ExceptionObject>() };
        payload.message_with(self)
    }

    /// 取类型的**命名空间字典**；**没有就惰性挂一个空字典** ✓（第 182 轮抽出 ✓，**一处真相** ✓）。
    ///
    /// 为什么惰性：内建类型建在**引导期** ✗ —— 那时 `dict` 类型还没出生 ✓，挂不了字典 ✓。
    /// 于是改成"第一次要的时候再挂" ✓（`TypeObject::dict`／`set_dict`／`mark_has_instance_dict` ✓ 都在）。
    pub fn type_namespace(&self, ty: NonNull<Header>) -> Option<NonNull<Header>> {
        // SAFETY: 调用方保证 ty 是存活的类型对象。
        let type_object = unsafe { &*ty.as_ptr().cast::<crate::TypeObject>() };
        if let Some(existing) = type_object.dict() {
            return Some(existing);
        }
        let created = self.new_dict();
        type_object.set_dict(Some(created));
        // **外部**那一档 ✓：命名空间挂在 `TypeObject.dict` ✓，**不是**载荷里的内联 `AttributeObject` ✗
        //（第 201 轮真 bug：先前置了内联位 ⇒ 把 `TypeObject` 当 `AttributeObject` 读 ⇒ 垃圾指针 ⇒ 段错误 ✗）。
        type_object.mark_external_instance_dict();
        Some(created)
    }

    fn alloc_type_raw(
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

impl Default for Instance {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Instance {
    /// **OM-2**：实例销毁**必须**释放其堆内全部内存，**无论循环是否被回收过**——
    /// 不依赖回收器先跑完。
    fn drop(&mut self) {
        // 正常路径下 `live` 已经空了。仍有残留 ⇒ 计数环或未交出的引用，
        // 此时**强制释放**：不调终结器、不再 clear（环的语义已由 §9 的回收器负责，
        // 走到这里说明调用方没有 collect，而不是回收器做不到）。
        //
        // 本层能这样做，是因为生命周期把 `Owned` 钉在 `&Instance` 上：对象载荷里
        // **不可能**存着 `Owned` 守卫（那需要 `&'static Instance`），所以这里释放
        // 任何一个对象都不会回头去碰别的对象。
        let live: Vec<usize> = self.live.borrow().iter().copied().collect();
        for address in live {
            let header = unsafe { NonNull::new_unchecked(address as *mut Header) };
            // SAFETY: 每个地址都由本实例分配且尚未释放；类型对象在下一段之前一直存活。
            unsafe { Self::force_free(header) };
        }

        let types = core::mem::take(&mut *self.types.borrow_mut());
        for ty in types {
            // SAFETY: 类型对象由注册表持有，销毁时统一释放（OM-15）。它的类型就是元类型，
            // 可能已被释放，因此直接按 `TypeObject` 释放，不再读 `ty()`。
            unsafe { TypeObject::dealloc(ty.cast::<Header>().as_ptr()) };
        }

        self.gc_head.set(ptr::null_mut());
        self.gc_count.set(0);
        self.bytes_allocated.set(0);
    }
}

impl Instance {
    /// **OM-2** 的实例销毁路径：不调终结器、不做 clear。
    ///
    /// # Safety
    ///
    /// `header` 必须由本实例分配、尚未释放，且其载荷**不得**持有 `Owned` 守卫。
    unsafe fn force_free(header: NonNull<Header>) {
        // SAFETY: 调用方保证 header 有效；类型对象在本次销毁的第二段才释放。
        let ty = unsafe { header.as_ref() }.ty();
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        // SAFETY: 调用方保证。
        unsafe { dealloc(header.as_ptr()) };
    }
}

/// 字符串的引号形态（实测参照实现：能用单引号就用单引号；内容里有单引号而**没有**双引号时
/// 改用双引号）。`ascii` 为真时把非 ASCII 字符转义（`ascii()` 的语义）。
///
/// **临时**：`repr` 的完整规则属于类型自己的槽位（`OM-11`），接线后由那里说了算。
pub(crate) fn quote_str(text: &str, ascii: bool) -> String {
    let has_single = text.contains('\'');
    let has_double = text.contains('"');
    let quote = if has_single && !has_double { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if character == quote => {
                out.push('\\');
                out.push(character);
            }
            _ if ascii && !character.is_ascii() => {
                let code = character as u32;
                if code <= 0xFF {
                    out.push_str(&format!("\\x{code:02x}"));
                } else if code <= 0xFFFF {
                    out.push_str(&format!("\\u{code:04x}"));
                } else {
                    out.push_str(&format!("\\U{code:08x}"));
                }
            }
            _ => out.push(character),
        }
    }
    out.push(quote);
    out
}

/// **`bytes` 的 `repr` 引号与转义**（`P1-12`；规则与 `str` 同源但按**字节**判断，照参照实测）：
/// 能用单引号就用单引号（内容有 `'` 而无 `"` 时改用双引号）；`\t`／`\n`／`\r`／`\\` 用转义；
/// 可打印 ASCII（`0x20..=0x7e`）原样；**其余一律** `\xNN`（含 `0x7f` 与所有高位字节——
/// 实测 `repr(b'caf\xc3\xa9') == "b'caf\\xc3\\xa9'"`，即使那是合法的 UTF-8）。
pub(crate) fn quote_bytes(value: &[u8]) -> String {
    let has_single = value.contains(&b'\'');
    let has_double = value.contains(&b'"');
    let quote = if has_single && !has_double { b'"' } else { b'\'' };
    let mut out = String::from("b");
    out.push(char::from(quote));
    for byte in value {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            _ if *byte == quote => {
                out.push('\\');
                out.push(char::from(*byte));
            }
            _ if (0x20..=0x7e).contains(byte) => out.push(char::from(*byte)),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out.push(char::from(quote));
    out
}
