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
/// 形参所有权实验开关（round 95；见 `refcount::frame_own_params`）。
pub(crate) use refcount::frame_own_params;
/// refcount 毒值诊断（round 107；见 `refcount::poison_refcount`）。
pub(crate) use refcount::poison_refcount;
mod gc;
mod context;
// `P3-41`：`repr` 递归守卫的三支结果（`OM-11`）。
pub(crate) use context::ReprEntry;
mod accessors;
mod constructors;
mod query;
mod platform;
mod registry;
mod alloc;
mod containers;
mod state;
mod capability_table;
mod weakrefs;
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
    /// **Python 级终结器旁表**（round 82，照 `OM-20` 的参照语义 ＝ at-most-once）：
    /// 地址 → 已跑过 `__del__` ✓。`header` 的 bit 4–7 是**禁止占用**的保留位（`OM-7`）⇒ 不能加标志
    /// ⇒ 用旁表 ✓；对象释放时同步摘除（地址复用不会串味 ✓）。
    /// 只对**用户类的 `__del__`**（`python_level_finalize`）生效；宿主终结器槽保持 `OM-20` 的
    /// "复活后再死可再终结"语义（`object_model` 的用例钉住 ✓）。
    python_finalized: RefCell<std::collections::HashSet<usize>>,
    /// **AST 解析回调**（round 86）：`compile(..., PyCF_ONLY_AST)` 的落点。core 不能依赖
    /// stdlib（分层）⇒ 由 `_ast` 模块在引导期登记一个函数指针 ✓（未登记时 compile 照旧报未接线 ✓）。
    ast_parser: Cell<Option<crate::AstParserFn>>,
    /// **已回收类型的地址登记**（round 130）：孤儿回收把类型从 `types`/`user_types` 摘掉后，**类型本体**
    /// 仍被 `Box::leak` 保留（裸 `ty` 指针仍有效 ✓）；登记在这里让 `assert_live` 认得它 ⇒ 仍在别处的
    /// 计数引用将来释放它时不会被当野指针 ✓（round 129 曾因此悬垂）。
    retired_types: RefCell<std::collections::HashSet<usize>>,
    /// **类型名 → 类型对象索引**（round 142 真性能修 ✗）：`type_named` 先前是**对 `types` 线性扫**
    /// （每次 `builtin_type(instance, …)` 都扫一遍 ⇒ 套件里类型一多就**每次 Python 调用**吃几微秒 ✓）；
    /// 现在 O(1) 哈希查找 ✓（`or_insert` 保持"先登记者优先"，与线性扫的旧口径一致 ✓）。
    types_by_name: RefCell<std::collections::HashMap<String, NonNull<TypeObject>>>,
    /// **类型注册代数**（round 145）：每登记一个类型 +1 ⇒ 依据"全部内建名"算出来的缓存（如
    /// [`Instance::is_dedicated_iterator_type`] 的集合）在代数变化时**重建** ✓（惰性建的迭代器
    /// 类型在首次推进之后才出现时，缓存不能过期 ✗）。
    types_generation: Cell<usize>,
    /// **"有专用推进分支"的迭代器类型地址集合**（round 145 性能）：`advance_iterator` 先前对
    /// **每个**步进都比 20+ 个内建名 ✗（`for` 循环每轮一次 ⇒ 热路径大头）；这里按代数缓存一份
    /// 集合，普通迭代器（`list_iterator`／`range_iterator` 等）**一次哈希查**就跳过整条链 ✓。
    dedicated_iterator_types: RefCell<Option<(usize, std::collections::HashSet<usize>)>>,
    /// **被迭代源类型 → 布局种类缓存**（round 146 性能，`iter.rs::source_kind` 用）：避免每个元素都做
    /// 7 次 `type_is_subtype_of`（`type_named` ＋ MRO 线性扫 ✗）。类型布局与实例无关 ⇒ 只存 `u8` 小码 ✓。
    iterable_source_kinds: RefCell<std::collections::HashMap<usize, u8>>,
    /// **用户类型地址集合**（round 119）：`adopt` 里 O(1) 判断"这个对象是不是用户类实例"✓。
    user_type_set: RefCell<std::collections::HashSet<usize>>,
    /// **用户类实例登记表**（round 119）：类型地址 → 活实例地址集合 ✓ —— 孤儿回收**必须能枚举活实例**
    /// 才能"先跑终结器、再清类型字典"（round 118 的教训：只扫 `gc_headers` 会漏非跟踪布局的实例 ✗）。
    user_instances: RefCell<std::collections::HashMap<usize, std::collections::HashSet<usize>>>,
    /// **动态（用户）类类型的"孤儿回收"名单**（round 99）：`class`/`type()` 造出的类型 —— 参照里类型是
    /// GC 对象；本层**不释放类型 Box**（`alloc_type_raw` 的 `Box::leak` 与 `AttributeObject::slots().dealloc`
    /// 不同构 ⇒ 真释放会 invalid free ✗），改为**保守孤儿回收**：`rc == 1`（只剩注册表）× 没有活对象以它
    /// 为 `ty` × 没有其它类型以它为 bases/mro/元类 ⇒ 摘注册表＋清字典（环随之由引用计数释放 ✓；类型本体留着，
    /// 任何裸 `ty` 指针仍有效 ✓）。可收 `test_dict`/`test_list` 的 `test_free_after_iterating` ✓。
    user_types: RefCell<Vec<NonNull<Header>>>,
    /// **`__getattribute__` 派发守卫**（round 87）：用户覆写走钩子期间置位，防止默认实现递归 ✓。
    getattribute_dispatching: Cell<bool>,
    /// **类型对象的 `__text_signature__` 旁表**（round 87）：`staticmethod`/`classmethod`/`property`
    /// 一类内建类在参照里有非空签名（`(function, /)` 等）——`inspect.signature(<类>)` 的类分支
    /// 会先读"MRO 里第一个非空 `base.__text_signature__`" ✓。
    type_text_signatures: RefCell<std::collections::HashMap<usize, &'static str>>,
    /// **内建函数的 `__text_signature__` 旁表**（round 86）：`inspect.signature` 解析内建可调用
    /// 对象（`object.__init__`／`__new__`／`__init_subclass__`）的文本签名入口 ✓。
    builtin_text_signatures: RefCell<std::collections::HashMap<usize, &'static str>>,
    /// **`mappingproxy` 的活视图来源**（round 125）：键/值/项视图的 `.mapping` 要是**源字典的活视图** ✓
    /// ⇒ 按代理地址记源 dict 指针；等价比较等入口先同步快照 ✓（`test_dict.test_views_mapping` ✓）。
    mappingproxy_live: RefCell<std::collections::HashMap<usize, NonNull<Header>>>,
    /// **`mappingproxy` 的来源类型**（round 114）：`MappingProxyType(OrderedDict(...))` 的
    /// `.copy()`／`repr` 要还原**底层映射类型** ✓ ⇒ 按代理对象地址记一份来源类型 ✓
    /// （类型对象活到实例销毁 ⇒ 不会悬垂 ✓；条目随代理存活而累积，量小如实登记）。
    mappingproxy_sources: RefCell<std::collections::HashMap<usize, NonNull<TypeObject>>>,
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
    /// **弱引用登记**（round 59）：目标地址 ⇒ 指向它的弱引用对象（**不持有引用** ✓）。
    /// 目标释放时逐个置死并调用回调 ✓（`invalidate_weakrefs`）；弱引用对象自己释放时摘除 ✓。
    weakrefs: RefCell<std::collections::HashMap<usize, Vec<NonNull<Header>>>>,
    /// **`isinstance` 的 ABC 快存**（round 73）：`(类, 实例类型, _abc_invalidation_counter)` ⇒ 结果 ✓。
    ///
    /// 为什么 ✗：`collections.abc` 的 `__instancecheck__` 是纯 Python（两个 `WeakSet` 查缓存 ＋
    /// `__subclasshook__`），本层每次调用要 ~45 µs（`ref()` 9 µs＋两次集合查），而 `Lib/` 里
    /// `isinstance(x, Sequence)` 一族遍地都是（`UserString` 的每个方法都带一个）；实测
    /// `test_userstring` 因此从 round 44 的 42 s 涨到 ~90 s（survey 120 s 阈值上临界漂移 ✗）。
    /// **失效**：ABC 的注册表变更会令 `ABCMeta._abc_invalidation_counter` ＋1，键里带上它就天然失效 ✓；
    /// 只对**元类恰是 `ABCMeta`** 的类启用 ✓（用户自定义元类的 `__instancecheck__` 可能有副作用 ⇒ 不缓存 ✗）。
    isinstance_cache: RefCell<std::collections::HashMap<(usize, usize, u64), bool>>,
    /// **GC 回收路径延后执行的弱引用回调**（round 59）：`free_garbage` 阶段只登记，
    /// 等整个回收循环结束后统一调（那时 `gc_running` 已复位 ✓）。
    pending_weakref_callbacks: RefCell<Vec<(NonNull<Header>, NonNull<Header>)>>,
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
    /// 诊断（round 62，`PYAWA_EXC_INFO_DEBUG=1`）：与 `exception_state` 同形的**压入现场**表 ✓。
    exception_state_sites: RefCell<Vec<String>>,
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
    /// **`gc.get_debug()`／`gc.set_debug()` 的落点**（round 92）：按实例存（`CX-3` 禁止进程级全局 ✓）。
    ///
    /// 本层没有调试通道（`PYAWA_GC_LOG` 一类是宿主侧诊断）⇒ 这里只**忠实保存/回报**用户设的位，
    /// 不声称有 `DEBUG_SAVEALL` 等语义 ✓（参照里设了不生效也只是"没有可观测行为" ✓）。
    gc_debug: Cell<u64>,
    /// **下一次自动回收的分配计数水位**（`P3-41` 慢因修复）：
    /// 单代标记-清除的每次代价是 **O(存活对象数)** ⇒ 固定阈值（2000）会让"存活对象多"的
    /// 工作负载退化成 **O(n²)**（`test_frame` 的 15 万层嵌套 ≈ 26.9 s，实测见 round 39 台账）。
    /// ⇒ 每次回收后把水位调到"再分配 `max(阈值, 当前存活数)` 次"：每次回收的 O(存活) 摊到
    /// 至少同样多次分配上 ⇒ 摊还 O(1)。`sys.get_threshold()` 仍只报**配置**值（口径不变 ✓）。
    gc_next_collect: Cell<usize>,
    /// 自动／手动回收的**实际执行次数**（诊断面；`gc.get_stats` 的口径底座）。
    gc_collections: Cell<usize>,
    /// 回收进行中：这些对象只减计数、由本次回收统一释放（见 [`Instance::collect`]）。
    gc_frozen: RefCell<HashSet<usize>>,
    /// 回收是否正在进行：终结器／`clear` 里再触发回收时不得嵌套（否则会动到外层手里的指针）。
    gc_running: Cell<bool>,
    /// **显式** `gc.collect()` 正在跑（round 110）：孤儿用户类回收只在显式回收或每 N 次自动回收时做 ✓
    /// （否则重负载套件每次自动回收都做 `user_types × gc_headers` 扫描 ⇒ 4–5× 变慢 ✗）。
    gc_explicit: Cell<bool>,
    /// **类型级 dunder 的"每类型一份"包装器**（round 103）：参照里每个内建类型在**自己的类型字典**
    /// 里有自己的 slot wrapper（`list.__repr__ is not dict.__repr__` ✓）—— 本层这些 dunder 由
    /// `object` 的同一个 native 兜底 ✗ ⇒ 在**类型对象**上取到的都是**同一个对象** ✗ ⇒
    /// `pprint` 的 `_dispatch[type(obj).__repr__]`（以类型属性作 dict 键）全部落到最后写入的那一项 ✗
    /// （实测：任意 list 被当成 deque 交给 `_pprint_deque` ⇒ `'list' object has no attribute
    /// 'maxlen'` ✗，官方 `test_pkg`／`test_rlcompleter` 的收尾都卡在这 ✓）。
    /// 缓存键 `(类型对象地址, 名字)` ✓；值由本实例持有（随实例释放 ✓）。
    type_dunders: RefCell<std::collections::HashMap<(usize, &'static str), NonNull<Header>>>,
    /// **小元组复用池**（round 96）：参照的 `tuple` freelist ✓ —— 生成器表达式每轮产的
    /// `(i, i)` 如果**立刻被丢弃**，下一轮会拿到**同一地址** ⇒ 官方 `test_genexps` 的
    /// 「Verify re-use of tuples」doctest 要求 `max(id)-min(id)==0` ✓（先前每次新分配 ⇒ 非 0 ✗）。
    ///
    /// **只放已 clear 过、内存还在的 `tuple` 对象**（≤20 个，参照的 freelist 上限同量级 ✓）；
    /// 复用时要**重新登记**（活表／GC 链／字节计数／`GC_TRACKED` ✓，见 `Instance::reuse_tuple`）。
    /// `PYAWA_QUARANTINE`／`PYAWA_NO_GC` 的实测模式**不进池** ✓（保持诊断语义 ✓）。
    tuple_pool: RefCell<Vec<NonNull<Header>>>,
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

pub(crate) use quote::{quote_bytes, quote_wtf8};

impl Instance {
    /// **`isinstance` 的 ABC 快存：读**（round 73）——键含 ABC 失效计数 ✓。
    pub fn isinstance_cache_get(&self, key: (usize, usize, u64)) -> Option<bool> {
        self.isinstance_cache.borrow().get(&key).copied()
    }

    /// **`isinstance` 的 ABC 快存：写**（round 73）；超过 8192 条就整体清掉 ✓（防无界增长）。
    pub fn isinstance_cache_put(&self, key: (usize, usize, u64), value: bool) {
        let mut cache = self.isinstance_cache.borrow_mut();
        if cache.len() > 8192 {
            cache.clear();
        }
        cache.insert(key, value);
    }
}
