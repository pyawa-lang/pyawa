//! 实例级内存、释放协议与循环回收（`docs/SPEC-object-model.md` §4、§7、§9）。
//!
//! 一个 [`Instance`] 就是 VM 侧一切可变状态的宿主（`DESIGN.md` §3 不变量 2）：
//! 对象堆、字节记账、类型注册表、回收链表与待处理栈都挂在它上面，**没有进程级全局状态**。

use core::cell::{Cell, OnceCell, RefCell};
use crate::diag::{dangling_mode, flag, leak_mode, quarantine_mode};
use core::ptr;
use core::ptr::NonNull;
use std::collections::{HashMap, HashSet};

use crate::flags;

/// **只漏不放** 的实验开关 ✓（第 238 轮，仅供对照实验 ✓）：`PYAWA_LEAK_MODE` **只读一次** ✓。
mod fs;

mod refcount;

mod gc;
mod context;
mod accessors;
mod constructors;
mod query;
mod platform;
mod registry;
mod alloc;
mod containers;
mod state;

fn ruler_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| flag("PYAWA_RULER"))
}

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

/// **Python 层调用深度上限**（第 319 轮）。
///
/// 取值依据（**实测**，不是照抄参照的 1000 ✗）：本层每次 Python 调用要吃好几层 Rust 栈 ✓，
/// 而对拍的 `pyawa_side_runner` 在**测试线程**上跑（栈比主线程小 ✓）。实测给它
/// `RUST_MIN_STACK=67108864` 后 `import collections` 不再崩 ✓ ⇒ 崩溃点落在默认栈上；
/// **取值 64 是量出来的** ✗：本层每层 Python 调用要吃掉相当一块原生栈 ✓（实测上限 200 时，
/// 主线程 8 MiB 栈在**触发守卫之前**就已经顶穿 ✗）⇒ 取 64 让守卫来得比栈崩**早** ✓；
/// 而 `Lib/` 里真实模块的调用深度都在几十层以内 ✓（导入链不是嵌套调用 ✓，不计深度 ✓）。
/// **如实登记**：参照默认 1000 且可用 `sys.setrecursionlimit` 调 ✗ —— 本层上限更低、且该接口还没接 ✗。
pub const MAX_CALL_DEPTH: usize = 64;

pub struct Instance {
    /// **当前帧对象**（第 230 轮；`sys._getframe()` ✓）。
    current_frame: core::cell::Cell<Option<NonNull<Header>>>,
    /// **`NotImplemented` 单例**（第 215 轮）。
    not_implemented_singleton: core::cell::Cell<Option<NonNull<Header>>>,
    /// **OM-3**：每实例字节计数器（预算职责留在 VM 侧，禁止下放给能力接口）。
    bytes_allocated: Cell<usize>,
    /// 本实例分配、尚未释放的普通对象（`usize` = 头部地址；**O(1)** 增删）。
    live: RefCell<HashSet<usize>>,
    /// **盯住的地址**（第 113 轮，`PYAWA_NS_DEBUG` 下由建类那处设 ✓）：0 ＝ 关 ✓。
    /// 对它**每一次 incref／decref 都报现场与计数** ✓ —— 不看地址归因 ✗、只看**计数与现场** ✓，
    /// 用来解释"`rc=2` 进去、内部归零" ✓（上限榜那一族的内存缺陷 ✓，见第 107～112 轮台账 ✓）。
    watch: Cell<usize>,
    /// **释放登记**（第 88 轮，按需开启 ✓）：`地址 → (类型名, 释放于哪个 Python 现场)` ✓ ——
    /// 给"写入点查一下这个对象是不是**已经释放过**"用 ✓（僵尸写 ✓：旧主人还在写已释放的对象 ✓）。
    freed_sites: RefCell<std::collections::HashMap<usize, (String, String)>>,
    /// 是否开启（`PYAWA_ZOMBIE_TRACE=1` ✓；关着零开销 ✓）。
    zombie_trace: Cell<bool>,
    /// **毒化隔离区** ✓（第 272 轮诊断；只在 `PYAWA_QUARANTINE=1` 时填 ✓）：
    /// `(头部地址, 载荷字节数, 类型名, 释放现场)` ✓ —— 第 297 轮补的第四格是**释放点**
    /// （`<帧 qualname>@<指令指针>`）：事后"多放一份"报出来时，两头都要有。
    quarantine: RefCell<Vec<(usize, usize, String, String)>>,
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
    /// **调用深度**（第 319 轮）：每次进入一个 Python 可调用就 +1 ✓ —— 上限见 [`MAX_CALL_DEPTH`] ✓。
    ///
    /// 动因：本层的"调用"是靠 **Rust 递归**（`call_callable` → `execute` → …）实现的 ✓ ⇒
    /// Python 层的深递归会直接吃**原生栈** ✗。上限诊断里那族 `子进程退出码 -11`（**29** 个模块 ✓）
    /// 就是这么来的：对拍的 `pyawa_side_runner` 跑在**测试线程**上（栈小得多 ✓，实测给它
    /// `RUST_MIN_STACK=67108864` 就不再崩 ✓）⇒ `import collections` 那种**不算深的**递归也能顶穿 ✗。
    /// **正确做法**是像参照一样在 **Python 层**设限并报 `RecursionError` ✓，而不是让原生栈崩掉 ✗。
    call_depth: Cell<usize>,
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
    /// **`NotImplemented` 单例** ✓（第 215 轮）：`Lib/types.py` 要 `type(NotImplemented)` ✓
    /// （`NotImplementedType` ✓）。参照里它是**单例** ✓ ⇒ 每次给**同一个**对象 ✓。
    pub fn not_implemented(&self) -> NonNull<Header> {
        if let Some(cached) = self.not_implemented_singleton.get() {
            return cached;
        }
        let ty = self
            .type_named("NotImplementedType")
            .unwrap_or_else(|| self.new_attribute_type("NotImplementedType"));
        let object = self
            .alloc(crate::builtin_objects::AttributeObject::new(
                ty,
                core::cell::RefCell::new(None),
            ))
            .into_raw()
            .cast::<Header>();
        self.not_implemented_singleton.set(Some(object));
        object
    }

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

    /// 模块表（**借用**）。
    pub fn modules(&self) -> Option<NonNull<Header>> {
        *self.modules.borrow()
    }

    /// 某个域的**不透明实现指针**（`AB-32`：本层只存不解释）。
    pub fn capability(&self, domain: usize) -> Option<*const core::ffi::c_void> {
        let slots = self.capabilities.borrow();
        let slot = slots.get(domain)?;
        (!slot.implementation.is_null()).then_some(slot.implementation)
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

        // **`object.__hash__`**（第 309 轮）：本层先前**没有**它 ✗ ⇒ `C.__hash__`／`ref.__hash__`
        // 这类"在**类型对象**上取 dunder"的属性访问当场报 `AttributeError: 'type' object has no
        // attribute '__hash__'` ✗（`Lib/weakref.py:89` 的 `__hash__ = ref.__hash__` 正栽在这 ✓，
        // 那一族 **31** 个模块 ✓）。这里按"**身份哈希**"接一个 ✓（同一对象在同一进程里恒定 ✓；
        // 与参照各类型的哈希值**不一致** ✗ —— 如实登记 ✓：本层的字典查键走 `values_equal` ✓，
        // 不靠这个值 ✓）。
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
                .with_new(crate::builtin::int::int_new)
                .with_repr(crate::builtin::int::int_repr)
                .with_str(crate::builtin::int::int_repr)
                // **方法面**（第 195 轮）：`to_bytes`／`bit_length` ✓。
                .with_getattr(crate::builtin::int::int_getattr),
        );
        let float_type = self.alloc_type_raw(
            "float",
            core::mem::size_of::<FloatObject>(),
            Slots::new(FloatObject::dealloc)
                .with_new(crate::builtin::float::float_new)
                .with_repr(crate::builtin::float::float_repr)
                .with_str(crate::builtin::float::float_repr)
                // **方法面**（第 352 轮）：`is_integer`／`as_integer_ratio` ✓。
                .with_getattr(crate::builtin::float::float_getattr),
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

        // **`bytearray`** ✓（第 226 轮，**M3 的这件** ✓）：可调用 ✓、可迭代 ✓ —— 与 `bytes` **共用载荷与槽** ✓
        //（**类型对象**不同 ✓ ⇒ `type(iter(bytearray()))` 给 **`bytearray_iterator`** ✓，与 `bytes_iterator` **分开** ✓）。
        // **如实记** ✗：目前只做**空** `bytearray()` ✓（可变字节面随后接 ✓）。
        let bytearray_type = self.alloc_type_raw(
            "bytearray",
            core::mem::size_of::<BytesObject>(),
            crate::builtin_objects::BytesObject::slots()
                .with_new(crate::builtin_objects::bytes_new)
                .with_repr(crate::builtin_objects::bytes_repr)
                .with_str(crate::builtin_objects::bytes_str)
                .with_getattr(crate::builtin_objects::bytes_getattr),
        );

        // **`list_reverseiterator`** ✓（第 227 轮）：与其它内建迭代器同构 ✓（`IteratorObject` ＋ 那套槽 ✓）；
        // **方向由类型决定** ✓ ⇒ 不必给载荷加字段 ✓。
        let list_reverseiterator_type = self.alloc_type_raw(
            "list_reverseiterator",
            core::mem::size_of::<IteratorObject>(),
            IteratorObject::slots(),
        );

        // **`longrange_iterator`** ✓（第 228 轮）：`range()` 的**大整数上限**那一支；载荷与 `islice` 同构 ✓
        //（我们的 `range` 本来就是 `islice(count(…))` ✓），只是**类型不同** ✓ —— 参照也分成两个名字 ✓。
        let longrange_iterator_type = self.alloc_type_raw(
            "longrange_iterator",
            core::mem::size_of::<crate::builtin_objects::ItStateObject>(),
            crate::builtin_objects::ItStateObject::slots(),
        );

        // **`super`** ✓（第 233 轮）：**零参**形式 ✓ —— 载荷用 `AttributeObject`（存 `__thisclass__`／`__self__` ✓），
        // 查表走 `attribute_lookup` 里的**专用分支** ✓（要在那里才能造出"绑定方法" ✓）。
        let super_type = self.alloc_type_raw(
            "super",
            core::mem::size_of::<crate::builtin_objects::AttributeObject>(),
            crate::builtin_objects::AttributeObject::slots(),
        );

        // **`zip`** ✓（第 229 轮）：载荷与 `zip_longest` 同构 ✓ —— **取最短** ✓。
        // 表里**早有 `zip` 这个名字** ✓（`builtin_types.rs` ✓）⇒ 不必加表条目 ✓。
        let zip_type = self.alloc_type_raw(
            "zip",
            core::mem::size_of::<crate::builtin_objects::ItStateObject>(),
            crate::builtin_objects::ItStateObject::slots(),
        );

        // **`range_iterator`** ✓（第 228 轮）：`range()` 的**常规**那一支 ✓（参照的名字 ✓）。
        let range_iterator_type = self.alloc_type_raw(
            "range_iterator",
            core::mem::size_of::<crate::builtin_objects::ItStateObject>(),
            crate::builtin_objects::ItStateObject::slots(),
        );

        // 容器：`TS-42` 的 M2 起步（层次取自探测表）
        let tuple_type = self.alloc_type_raw(
            "tuple",
            core::mem::size_of::<TupleObject>(),
            TupleObject::slots()
                .with_new(crate::builtin_objects::tuple_new)
                .with_repr(crate::builtin_objects::tuple_repr)
        );
        let list_type = self.alloc_type_raw(
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
        let dict_type = self.alloc_type_raw(
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
        let memoryview_type = self.alloc_type_raw(
            "memoryview",
            core::mem::size_of::<crate::builtin_objects::BytesObject>(),
            memoryview_slots,
        );

        // **`range`** ✓（第 237 轮）：参照里它是**类型** ✓（`Range.register(range)` 一族 ✓）⇒ 本层补上它的**类型对象** ✓
        //（构造槽 `range_new` 见 core ✓）。**如实说** ✗：`range(n)` 给出的仍是**迭代器** ✓ ⇒ `type(range(n))` 现在
        // 给 `range_iterator` ✗（参照给 `range` ✓）—— 既有偏差 ✓，本轮**不动**它 ✓（只让**名字**成为类型 ✓）。
        let range_type = self.alloc_type_raw(
            "range",
            core::mem::size_of::<crate::builtin_objects::ItStateObject>(),
            crate::builtin_objects::ItStateObject::slots()
                .with_new(crate::builtin_objects::range_new),
        );

        // **`frozenset`** ✓（第 236 轮）：与 `set` **同载荷同槽** ✓（`set_new` 收 **class** ⇒ 直接复用 ✓），
        // 只是**另一个类型对象** ✓（`abc.py:180` 要 `frozenset(abstracts)` ✓、
        // `_collections_abc.py:687` 要 `Set.register(frozenset)` ✓）。
        // **如实说** ✗：本层没有"不可变"这层语义 ✓（`frozenset` 的实例目前**仍可改** ✓，随后补 ✓）。
        let frozenset_type = self.alloc_type_raw(
            "frozenset",
            core::mem::size_of::<SetObject>(),
            SetObject::slots()
                .with_new(crate::builtin::set::set_new)
                .with_repr(crate::builtin::set::set_repr)
                .with_getattr(crate::builtin::set::set_getattr),
        );

        let set_type = self.alloc_type_raw(
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
            crate::builtin_objects::ThreadHandleObject::slots(),
        );
        {
            let base = self.type_named("object").expect("object 已登记");
            assert!(
                self.register_bases(thread_handle_type, vec![base]).is_some(),
                "OM-13：ThreadHandle 的 MRO 应当可线性化"
            );
        }

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
                .with_repr(crate::builtin::generator::generator_repr)
                .with_getattr(crate::builtin::generator::generator_getattr),
        );

        // 协程（`§10` 的生成器与协程族）：载荷与生成器同形（一个挂起的帧 ＋ 标志），
        // 名字与基类照探测表；`repr` 的词是 `coroutine`（实测 `<coroutine object f at 0x…>`）。
        let coroutine_type = self.alloc_type_raw(
            "coroutine",
            core::mem::size_of::<GeneratorObject>(),
            GeneratorObject::slots()
                .with_repr(crate::builtin_objects::coroutine_repr)
                .with_getattr(crate::builtin::generator::generator_getattr),
        );

        // 异步生成器（`CO_ASYNC_GENERATOR`，实测 `0x200`）：载荷同样与生成器同形，
        // 类型名与基类照探测表（实测 `async_generator` → `object`）。
        let async_generator_type = self.alloc_type_raw(
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
        crate::executor::runtime::raise_builtin(self, name, message)
    }

    /// 把对象当**类型对象**看（是就给 `Some`，否则 `None`）。
    pub fn as_type(&self, object: NonNull<Header>) -> Option<NonNull<TypeObject>> {
        if !self.is_type_object(object) {
            return None;
        }
        // SAFETY: 对象就是类型对象（类型身份已确认）。
        Some(unsafe { NonNull::new_unchecked(object.as_ptr().cast::<TypeObject>()) })
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
        // **`frozenset` 与 `set` 同载荷** ✓（第 236 轮）⇒ 迭代这条也一并认 ✓。
        if ty == self.type_named("set")? || Some(ty) == self.type_named("frozenset") {
            // SAFETY: 同上。
            let set = unsafe { &*object.as_ptr().cast::<SetObject>() };
            return Some(set.items().into_iter().map(owned).collect());
        }
        // **迭代器对象**（第 137 轮）：`list(itertools.repeat(5, 3))`／`list(x for x in y)` 这类
        // 都要能消费 ✓ ⇒ 复用执行器那份 `advance`（内建迭代器 ＋ `__next__` 协议 ✓ **一处真相** ✓）；
        // 既不是迭代器也不是可迭代 ⇒ `None`（调用方照常报"不是可迭代" ✓）。
        let mut items = Vec::new();
        loop {
            match crate::executor::runtime::advance(self, object) {
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

    /// 是不是 `bool`（`True`／`False` 是 `int` 的子类，别的地方要分开判）。
    /// `bool` 的**值**（不是 `bool` 就给 `None`）。
    /// **迭代推进**（第 142 轮）：直接复用执行器那份（`executor::runtime::advance` ✓ **一处真相** ✓）——
    /// 内建 `next()` 要的就是它 ✓。
    pub fn advance_iterator(
        &self,
        object: NonNull<Header>,
    ) -> Result<Option<NonNull<Header>>, ExecError> {
        crate::executor::runtime::advance(self, object)
    }

    /// **取迭代器**（第 142 轮）：直接复用执行器那份（`executor::iter::iter_value` ✓ **一处真相** ✓）——
    /// 内建 `iter()` 要的就是它 ✓（`iter(迭代器) is 它自己` ✓ 由那份实现保证 ✓）。
    pub fn iter_object(&self, object: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
        crate::executor::iter::iter_value(self, object)
    }

    /// **取属性（可选）**（第 148 轮）：直接复用执行器那条属性通道 ✓（**一处真相** ✓）——
    /// 内建 `getattr`／`hasattr` 要的就是它 ✓。**必须走它** ✗：早先我直接拿对象当 `dict` 查
    /// （`dict_get` ✗ 会把指针强转成 `DictObject` 读 ⇒ **UB** ✓，实测触发 abort ✓）。
    pub fn attribute_optional_of(
        &self,
        object: NonNull<Header>,
        name: &str,
    ) -> Result<Option<NonNull<Header>>, ExecError> {
        crate::executor::attribute::attribute_optional(self, object, name)
    }

    pub fn bool_value(&self, object: NonNull<Header>) -> Option<bool> {
        if !self.is_bool(object) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<BoolObject>() }.value)
    }

    // ---- 容器载荷的**安全**面（`pyawa-stdlib` 是 `forbid(unsafe_code)`，它只能走这些）----

    pub fn index_value(&self, object: NonNull<Header>) -> Result<Option<i64>, ExecError> {
        if let Some(value) = self.int_value(object) {
            return Ok(Some(value));
        }
        let method = match crate::executor::attribute::attribute_optional(self, object, "__index__") {
            Ok(Some(method)) => method,
            // 没有这个方法、或取属性出错 ⇒ 如实"不是整数" ✓（由调用方报 TypeError ✓）
            Ok(None) | Err(_) => return Ok(None),
        };
        let result = crate::executor::call::call_value(self, method, &[], &[]);
        // SAFETY: method 是新引用。
        unsafe { self.release_object(method.as_ptr()) };
        match result {
            Ok(value) => {
                let number = self.int_value(value);
                // SAFETY: value 是新引用。
                unsafe { self.release_object(value.as_ptr()) };
                Ok(number)
            }
            Err(_) => Ok(None),
        }
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

    /// 往 `dict` 里按**字符串**键写一个值（**接管** `value` 的引用，`OM-16`）。
    ///
    /// 键已存在则替换（旧值由这里释放）。给 stdlib 建模块用。
    /// **口径（第 275 轮按用户裁定改为"借用" ✓）**：`dict_set` **自己**为字典那一份 `incref` ✓
    /// —— 与 CPython 的 `PyDict_SetItem` 一致 ✓。**调用方不必**先 `retain` ✓，也**不必**交出所有权 ✓
    /// （先前是"接管一份引用" ✗ ⇒ 每个"把查找结果直接交给字典"的站点都得自己记得 `retain` ✗
    ///  ⇒ 实测同类站点 100+ 处、已漏出至少两处 ✗ ⇒ `MS-25` 的悬垂条目就是这么来的 ✓）。
    /// **僵尸写探测**（第 88 轮）：要写的对象**已经在释放登记里** ⇒ 旧主人还在写 ✓ ⇒ 当场报出
    /// **是谁释放的、哪个 Python 现场** ✓（`PYAWA_ZOMBIE_TRACE=1` 才开 ✓）。
    fn zombie_probe(&self, object: NonNull<Header>) {
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
    fn alloc_int(&self, value: IntValue) -> NonNull<Header> {
        let int_type = self.singletons().int_type();
        self.alloc(IntObject::new(int_type, value))
            .into_raw()
            .cast::<Header>()
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

    /// **`LOAD_BUILD_CLASS`** 压的那个内建（`__build_class__`；**借用**）。
    ///
    /// 参照实现从 `builtins` 取它；Pyawa 还没有 `builtins` 模块（`P3-14`），故先按实例存一个。
    pub fn build_class(&self) -> Option<NonNull<Header>> {
        self.build_class.get()
    }

    /// 元类型：类型对象自身的类型。
    pub fn metatype(&self) -> NonNull<TypeObject> {
        self.metatype
            .get()
            .expect("元类型在 Instance::new 中引导，必然存在")
    }

    /// 本实例中尚未释放的普通对象数（类型对象不计）。
    pub fn live_objects(&self) -> usize {
        self.live.borrow().len()
    }

    /// **OM-26**：回收阈值三元组。默认值见 [`DEFAULT_GC_THRESHOLD`]。
    ///
    /// 后两位**存而不生效**（单代，**临时**）；它们照样要能读回来，纯 Python 层会解三元组。
    pub fn gc_threshold(&self) -> (usize, usize, usize) {
        self.gc_threshold.get()
    }

    /// 当前调用深度（诊断用 ✓）。
    pub fn call_depth(&self) -> usize {
        self.call_depth.get()
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
        // **诊断**（第 296 轮，只在 `PYAWA_QUARANTINE=1` 时花钱 ✓）：撞上隔离区里的对象时
        // 先把"它原来是什么类型、现在哪一帧在动它"报出来 ✓ —— 光一句"对已释放对象 incref" ✗
        // 查不动（第 295 轮就是靠这条栈才把 `P3-21` 定位到 `subscript_get` 的 ✓）。
        self.quarantine_report(ptr, "incref");
        if self.watch.get() == ptr as usize {
            eprintln!(
                "[watch] incref {ptr:p} → rc={} 现场={}",
                unsafe { &*ptr }.refcount() + 1,
                self.current_site()
            );
        }
        // SAFETY: 由调用方保证 ptr 有效。
        unsafe { &*ptr }.incref();
    }

    /// **诊断**：`ptr` 是不是隔离区里的**已释放对象**？是就报**原类型**与**当前帧的 qualname** ✓。
    ///
    /// 只在 `PYAWA_QUARANTINE=1` 时工作 ✓（其余时候第一句就返回 ✓）。
    fn quarantine_report(&self, ptr: *mut Header, what: &str) {
        if !quarantine_mode() {
            return;
        }
        let address = ptr as usize;
        let found = self
            .quarantine
            .borrow()
            .iter()
            .find(|(address_of, _, _, _)| *address_of == address)
            .cloned();
        let Some((_, size, name, site)) = found else {
            return;
        };
        let frame = self
            .current_frame()
            .and_then(|frame| {
                // SAFETY: 当前帧由执行器的守卫挂着，存活 ⇒ 指针指向一个 `Frame` 载荷。
                let code = unsafe { &*frame.as_ptr().cast::<crate::frame::Frame>() }.code()?;
                // SAFETY: 代码对象由函数对象持有，存活 ⇒ 指针指向一个 `CodeObject` 载荷。
                Some(
                    unsafe { &*code.as_ptr().cast::<crate::code::CodeObject>() }
                        .qualname()
                        .to_owned(),
                )
            })
            .unwrap_or_else(|| "<无当前帧>".to_owned());
        eprintln!(
            "[隔离区] {what} 撞上**已释放对象** {address:#x}（原类型 {name}，{size} 字节；释放于 {site}）⇒ 提前释放／多放一份 ✗；当前帧：{frame}"
        );
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

    pub unsafe fn release_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        // SAFETY: 调用方保证 ptr 有效；**先查活表** ✓（第 273 轮诊断）。
        if self.watch.get() == ptr as usize {
            eprintln!(
                "[watch] decref {ptr:p} → rc={} 现场={}",
                unsafe { &*ptr }.refcount() - 1,
                self.current_site()
            );
        }
        // **按判据盯**（第 115 轮，`PYAWA_WATCH_DICT=1`）：任何 `dict` **掉到 0** 都报现场 ✓
        // —— 第 114 轮查明"死在 @540 的是**另一个类**的命名空间" ✓ ⇒ 盯一个地址不够 ✓，
        // 要用**判据**（类型＝`dict` ✓）把**第一个被打到 0 的那个**逼出来 ✓。
        if flag("PYAWA_WATCH_DICT") && unsafe { &*ptr }.refcount() == 1 {
            let ty = unsafe { &*ptr }.ty();
            // SAFETY: ty 由注册表持有。
            let name = unsafe { ty.as_ref() }.name();
            if name == "dict" {
                eprintln!(
                    "[dict→0] {ptr:p} 掉到 0：现场={}\n{}",
                    self.current_site(),
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        self.assert_live(unsafe { NonNull::new_unchecked(ptr) }, "release_object");
        let header = unsafe { &*ptr };
        // **OM-24**：M1 的 `IMMORTAL` 位恒为 0；这里只是防御，不承担语义。
        if header.is_immortal() {
            return;
        }
        if header.decref() != 0 {
            return;
        }

        // **注册过的类型对象不在这里释放** ✓（第 284 轮）：类型对象**不记活表** ✓、由 `self.types`
        // 注册表持有、**销毁实例时统一释放** ✓（见 `Instance` 的销毁段 ✓）。先前没有这一道 ⇒
        // 一旦某处把**最后一份计数**放掉 ✗（实测：`abc.py` 的类体帧槽就持有 `ABC` 那个类对象 ✓）
        // ⇒ 通用释放路径**当场把注册表里的类型对象释放掉** ✗ ⇒ 之后谁再碰它谁段错误 ✓
        // （`PYAWA_DANGLING=1` 下 `import_posixpath` 的 SIGSEGV 就是这个 ✓）。
        // **如实说** ✗：这一道**挡住的是症状** ✓ —— 真正要查的是"**谁多放了一份**" ✓（欠计数 ✓），
        // 已记进 `P3-19` 与第 284 轮台账 ✓。
        if self
            .types
            .borrow()
            .iter()
            .any(|ty| ty.as_ptr() as usize == ptr as usize)
        {
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
        // **实验（第 238 轮）**：暂不真释放 ✓ ⇒ 崩溃消失即证明"释放后仍被用／写" ✗。
        if quarantine_mode() {
            self.quarantine_put(ptr);
        } else if !leak_mode() {
            // SAFETY: 计数为 0，且 clear 已把持有的引用交出（OM-20 ③ 的前提）。
            unsafe { dealloc(ptr.as_ptr()) };
        }
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
        if quarantine_mode() {
            self.quarantine_put(header);
        } else if !leak_mode() {
            // SAFETY: 该对象已由可达性分析判为不可达，且 clear 已完成。
            unsafe { dealloc(header.as_ptr()) };
        }
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

    /// **尺子** ✓（第 184 轮把当时那次**临时**手法**常驻**下来 ✓）：释放前问"**还有谁指着这个地址**" ✓。
    ///
    /// **与布局无关** ✓：靠每个类型的 `traverse` 槽（`OM-12` ✓）**扫全图** ✓；由 `PYAWA_RULER=1` 门控 ✓。
    /// **注意** ✗：成环的成员之间会**互相指** ✓ ⇒ 输出里出现环成员是**预期**的 ✓；
    /// 但若出现"**该对象已无人引用**却仍被某个类型指着" ✗，那就是**释放后用**的现场 ✓。
    fn who_points_at(&self, address: usize) -> Vec<(usize, String)> {
        let live: Vec<usize> = self.live.borrow().iter().copied().collect();
        let mut found = Vec::new();
        for item in live {
            let header = item as *mut Header;
            // SAFETY: `live` 里的地址都是存活对象 ✓。
            let ty = unsafe { &*header }.ty();
            // SAFETY: ty 由注册表持有 ✓。
            let Some(traverse) = (unsafe { ty.as_ref() }).slots.traverse else {
                continue;
            };
            let mut hit = false;
            // SAFETY: 槽位由类型提供，契约见 OM-12 ✓。
            unsafe {
                traverse(header, &mut |child| {
                    if child as usize == address {
                        hit = true;
                    }
                });
            }
            if hit {
                found.push((item, self.type_name(ty)));
            }
        }
        found
    }

    /// 从"存活集合"与回收链表上同时摘除。
    /// 隔离区：**毒化载荷** ＋ 记账 ✓（第 272 轮）。
    fn quarantine_put(&self, header: NonNull<Header>) {
        let ty = unsafe { header.as_ref() }.ty();
        let size = unsafe { ty.as_ref() }.instance_size;
        let name = self.type_name(ty);
        let site = self.current_site();
        let payload = header.as_ptr().cast::<u8>();
        let head = core::mem::size_of::<Header>();
        // SAFETY: 载荷大小来自类型元数据 ✓；对象已不在活表里、且不再交还分配器 ✓ ⇒ 本层独占 ✓。
        unsafe {
            core::ptr::write_bytes(payload.add(head), 0xDE, size.saturating_sub(head));
        }
        self.quarantine
            .borrow_mut()
            .push((header.as_ptr() as usize, size, name, site));
    }

    /// **当前执行现场**（第 297 轮诊断）：`<帧 qualname>@<指令指针>`；没有当前帧时给个占位。
    fn current_site(&self) -> String {
        let Some(frame) = self.current_frame() else {
            return "<无当前帧>".to_owned();
        };
        // SAFETY: 当前帧由执行器的守卫挂着，存活 ⇒ 指针指向 `Frame` 载荷。
        let frame = unsafe { &*frame.as_ptr().cast::<crate::frame::Frame>() };
        let pointer = frame.instruction_pointer();
        let name = frame
            .code()
            .map(|code| {
                // SAFETY: 代码对象由函数对象持有，存活 ⇒ 指针指向 `CodeObject` 载荷。
                unsafe { &*code.as_ptr().cast::<crate::code::CodeObject>() }
                    .qualname()
                    .to_owned()
            })
            .unwrap_or_else(|| "<无代码对象>".to_owned());
        format!("{name}@{pointer}")
    }

    /// 复核隔离区 ✓：毒化字节被改 ⇒ **释放后仍被写** ✓（use-after-free ✗）。
    fn quarantine_check(&self) {
        if !quarantine_mode() {
            return;
        }
        let head = core::mem::size_of::<Header>();
        let suspects: Vec<(usize, usize, String, String)> = self
            .quarantine
            .borrow()
            .iter()
            .filter(|(address, size, _, _)| {
                let payload = (*address as *mut u8).wrapping_add(head);
                let length = size.saturating_sub(head);
                // SAFETY: 隔离区的对象**没有**还给分配器 ⇒ 这段内存仍属本层 ✓。
                unsafe { (0..length).any(|index| *payload.add(index) != 0xDE) }
            })
            .cloned()
            .collect();
        if let Some((address, size, name, site)) = suspects.first() {
            eprintln!(
                "[隔离区] {address:#x}（{name}，{size} 字节；释放于 {site}）的载荷在**释放之后**被写过 ✗ ⇒ use-after-free ✓"
            );
            std::process::exit(3);
        }
    }

    fn unlink(&self, header: NonNull<Header>) {
        // **释放探针**（第 112 轮，`PYAWA_FREE_DEBUG=1`）：每次真正摘除一个对象都报
        // **地址 ＋ 类型 ＋ Python 现场 ＋ Rust 回溯** ✓ —— 用来分辨"同一条指令放了两次" ✗
        // 还是"重绑放一次、调用收尾又放一次" ✗（上限榜那一族的内存缺陷 ✓，见第 107～111 轮台账 ✓）。
        if flag("PYAWA_FREE_DEBUG") {
            // SAFETY: header 由调用方保证存活（正要摘除）。
            let name = unsafe { header.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            let name = unsafe { name.as_ref() }.name().to_owned();
            eprintln!(
                "[free 探针] {:#x} 类型={name} 现场={}\n{}",
                header.as_ptr() as usize,
                self.current_site(),
                std::backtrace::Backtrace::force_capture()
            );
        }
        if self.zombie_trace.get() {
            // SAFETY: header 由调用方保证存活（正要摘除）。
            let name = unsafe { header.as_ref() }.ty();
            // SAFETY: ty 由类型注册表持有。
            let name = unsafe { name.as_ref() }.name().to_owned();
            // **只记"第一次"释放** ✓（第 89 轮）：地址会被复用 ✓ ⇒ 后一次释放会把现场覆盖掉 ✗
            // ⇒ 那样只能看到"最后那个占着它的对象是谁" ✓（实测就是 `str` ✓，不是我们要抓的 ✓）。
            // 用 `or_insert` 留住**最早**那次释放的现场 ✓ —— 那才是"谁把这个 dict 放多了" ✓。
            self.freed_sites
                .borrow_mut()
                .insert(header.as_ptr() as usize, (name, self.current_site()));
        }
        self.quarantine_check();
        // **野释放检测** ✓（第 238 轮，**与布局无关** ✓、**先查后删** ✓）：要摘除的地址**必须在活表里** ✓。
        // 不在 ⇒ 三种可能：**从没分配过** ✗／**已经释放过** ✗（glibc 要到**进程退出**才报
        // `tcache_thread_shutdown(): unaligned tcache chunk detected` ✓）／**内部指针** ✗。
        // 注意：地址会被复用 ✓ ⇒ 所以判据是"**摘除时**在不在表里" ✓（不在 ⇒ 一定放多了 ✓），不会假阳性 ✓。
        if !self.live.borrow_mut().remove(&(header.as_ptr() as usize)) {
            eprintln!(
                "[野释放] {:#x} 不在活表里 ✗ —— 重复释放／内部指针／从未分配（见 PLAN 第 183 轮 ✓）",
                header.as_ptr() as usize
            );
            std::process::abort();
        }
        if ruler_on() {
            let refs = self.who_points_at(header.as_ptr() as usize);
            if !refs.is_empty() {
                eprintln!(
                    "[尺子] {:#x}（{}）仍被 {} 处指着 ✗：{:?}",
                    header.as_ptr() as usize,
                    self.type_name(unsafe { header.as_ref() }.ty()),
                    refs.len(),
                    refs
                );
            }
        }
        // SAFETY: header 尚未释放。
        if unsafe { header.as_ref() }.has_flag(flags::GC_TRACKED) {
            self.unlink_gc(header);
        }
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
        // **"只漏不放"也要盖住销毁这一段** ✓（第 240 轮）：进程马上要退出 ✓ ⇒ 漏掉不会有副作用 ✓，
        // 但能**判定**崩溃是否来自"销毁时那一大批释放" ✓。
        if leak_mode() {
            self.live.borrow_mut().clear();
            self.types.borrow_mut().clear();
            self.gc_head.set(ptr::null_mut());
            self.gc_count.set(0);
            self.bytes_allocated.set(0);
            return;
        }
        let live: Vec<usize> = self.live.borrow().iter().copied().collect();
        for address in live {
            let header = unsafe { NonNull::new_unchecked(address as *mut Header) };
            // SAFETY: 每个地址都由本实例分配且尚未释放；类型对象在下一段之前一直存活。
            unsafe { Self::force_free(header) };
        }

        let types = core::mem::take(&mut *self.types.borrow_mut());
        for ty in types {
            // **保留原样** ✓（理由同上 ✓）：类型对象是 `Box` 分配的 ✓ ⇒ 就按 `Box` 释放 ✓。
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
