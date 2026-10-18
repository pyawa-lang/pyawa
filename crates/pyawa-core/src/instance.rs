//! 实例级内存、释放协议与循环回收（`docs/SPEC-object-model.md` §4、§7、§9）。
//!
//! 一个 [`Instance`] 就是 VM 侧一切可变状态的宿主（`DESIGN.md` §3 不变量 2）：
//! 对象堆、字节记账、类型注册表、回收链表与待处理栈都挂在它上面，**没有进程级全局状态**。

use core::cell::{Cell, OnceCell, RefCell};
use util::{ruler_on, str_matches};
use crate::diag::{dangling_mode, flag, leak_mode, quarantine_mode};
use core::ptr;
use core::ptr::NonNull;
use std::collections::{HashMap, HashSet};

use crate::flags;

/// **只漏不放** 的实验开关 ✓（第 238 轮，仅供对照实验 ✓）：`PYAWA_LEAK_MODE` **只读一次** ✓。
mod fs;

mod net;

mod path;

mod refcount;

mod util;
mod misc;
mod bootstrap;
mod interrupt;
mod convert;
// **编码表与归一函数**（第 726 轮）：`_codecs`／`bytes.decode`／`str.encode` 的**一处真相** ✓。
pub(crate) use convert::CP437;
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
mod capability_table;
mod wait;

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
    /// **`CP-31`**：每实例的 `fd → 能力对象` 表（`fd` ＝ 表下标）。
    ///
    /// 为什么在 `Instance` 上：`fd` 的分配／释放／复制（含 `dup2`）**必须**由 VM 侧完成
    /// （`CP-31`），provider 只给不透明句柄；实例销毁由 `Drop` 兜底释放全部条目（`CP-19`）。
    /// 实现与语义（别名计数、`select` 分流）见 `instance/capability_table.rs`。
    capability_table: RefCell<capability_table::CapabilityTable>,
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
    /// `pa_create`（`pyawa-abi`）创建实例时注入生成表，**按名字**查（数字随平台）。**不新增能力域**
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
    /// **`sys.getrecursionlimit()` 的口径** ✓（task-20 第 3 轮 ✓）：有效上限就是它 ✓，
    /// 默认 [`MAX_CALL_DEPTH`] ✓；`sys.setrecursionlimit` **只降不升** ✓（理由见 `MAX_CALL_DEPTH`
    /// 那段：本层的 Python 调用吃**原生栈** ✗ ⇒ 调高会顶穿 ✓ —— 比参照小是**如实口径** ✓）。
    recursion_limit: Cell<usize>,
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
    /// **`CP-22`：实例 cwd**（`SPEC-capabilities.md`；`os.chdir` 的落点）。
    ///
    /// guest 绝对形态（`/` ＝ 根能力的根）；`None` ＝ 尚未设置 ⇒ 首次解析时取宿主进程 cwd
    /// 作为初值（**过渡口径**：`CP-24` 的残余权威，正路是宿主创建实例时显式给，见
    /// `instance/path.rs` 的模块文档）。
    path_cwd: RefCell<Option<Vec<u8>>>,
    /// **`AB-5`①／`CX-3`**：本实例被请求中断（`pa_interrupt` 的落点）。
    ///
    /// **按实例**存——`CX-3` 禁止进程级共享；执行器每条指令检查一次，
    /// 于是"执行类函数随即返回"（`PA_ERR_INTERRUPT`）成立。
    /// **`AB-5`①**：中断请求标志。
    ///
    /// **原子**（round 12）：`pa_interrupt` 的契约是宿主在执行中从**另一线程**请求中断
    /// （DESIGN §7.2 的"VM 线程等待能力结果时宿主必须能中断"）⇒ 普通 `Cell<bool>` 跨线程
    /// 读写是数据竞争；用 `AtomicBool` 让"执行中另一线程 `pa_interrupt`"有明确定义。
    interrupted: core::sync::atomic::AtomicBool,
}

mod quarantine;
mod drop;
mod quote;

pub(crate) use quote::{quote_bytes, quote_str};
