//! `Instance` 的引用计数与释放域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓（模块级自由函数仍留在 instance.rs ✓）。

use super::*;

/// **refcount 毒值**（round 107 确定性诊断）：对象真正释放前把 `refcount` 写成这个值；之后任何
/// `incref`／`release` 再碰它 ⇒ 当场 panic 并带现场＋Rust 回溯（比 glibc 的 `malloc`/`double-linked`
/// 崩溃更早、且能直接指出**第二次释放**来自哪条 Rust 路径 ✓）。环境门控：`PYAWA_REFCOUNT_CANARY=1`。
pub(crate) const REFCOUNT_POISON: u32 = 0xDEAD_BEEF;

/// canary 开关（`OnceLock` 缓存 env ✓）。
pub(crate) fn refcount_canary() -> bool {
    static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("PYAWA_REFCOUNT_CANARY").is_some())
}

/// 释放前写毒值（`free_garbage` 调用；只在 canary 模式 ✓）。
pub(crate) fn poison_refcount(header: NonNull<Header>) {
    if refcount_canary() {
        // SAFETY: 调用方保证 header 仍可写（dealloc 之前）。
        unsafe { header.as_ref() }.set_refcount(REFCOUNT_POISON);
    }
}

/// **形参所有权**（round 98 起为**默认模型**）：帧拥有前 `argcount` 个局部槽
/// （`bind_arguments` 把实参**所有权**填进去 ⇒ 帧释放时归还 ✓）。
///
/// 历史：round 43/81/83/84/89-97 反复试验"帧拥有形参"；round 95 做成运行期开关、round 96-97 修掉
/// 两处"清空操作数"UAF、round 97 实测 `PYAWA_FRAME_OWN=1` 全量套件 **ok 68 持平、无 ok 回退**、
/// 十项闸门全绿 ⇒ round 98 **翻成默认** ✓。应急回退：`PYAWA_FRAME_OWN=0` 走旧模型（帧跳过形参槽）。
pub(crate) fn frame_own_params() -> bool {
    static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FLAG.get_or_init(|| {
        !matches!(
            std::env::var("PYAWA_FRAME_OWN").ok().as_deref(),
            Some("0") | Some("false") | Some("no")
        )
    })
}

/// 被跟踪对象的地址集合（round 92；只在 `PYAWA_REF_TRACE` 开启时填充）。
static REF_TRACKED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<usize>>> =
    std::sync::OnceLock::new();

impl Instance {
    /// 归还一份引用（[`Self::retain`] 的配对）。
    ///
    /// 与 `retain` 一样是**安全函数**：契约（"这份引用确实是你持有的"）由调用方保证——
    /// `#![forbid(unsafe_code)]` 的 stdlib 要管理中途丢弃的中间数量，必须有这条配对。
    pub fn release(&self, object: NonNull<Header>) {
        // SAFETY: 调用方保证这份引用归它所有（见本函数的契约）。
        unsafe { self.release_object(object.as_ptr()) };
    }

    pub fn refcount_of(&self, object: NonNull<Header>) -> u32 {
        // SAFETY: 调用方按 `OM-16` 保证 object 是本实例里的存活对象。
        unsafe { object.as_ref() }.refcount()
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

    /// 释放一个**新引用**（**OM-16**）；计数归零时按 **OM-20** 的顺序处理：
    /// ① 终结器（可复活）→ ② `clear` → ③ 释放。清空走 **OM-21** 的待处理栈，不朴素递归。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中**存活**的对象，且调用方交出的是一份**新引用**。
    /// **悬垂哨兵** ✓（第 273 轮诊断）：见 [`dangling_mode`] ✓。
    pub fn assert_live(&self, ptr: NonNull<Header>, site: &str) {
        if !dangling_mode() {
            return;
        }
        let address = ptr.as_ptr() as usize;
        if self.live.borrow().contains(&address) {
            return;
        }
        // **类型对象不记活表** ✓（`live_objects()` 的口径："普通对象数，类型对象不计" ✓）——
        // 判据必须用**注册表**（`self.types` ✓ 所有类型对象都在那里 ✓）而**不能解引用** ✗：
        // 哨兵手里的指针可能**真的已经死了** ✓，读它的 `ty()` 会当场段错误（第 284 轮实测：
        // `PYAWA_DANGLING=1` 下 `import_posixpath_surface` 直接 SIGSEGV、连 panic 都没来得及打 ✗）。
        if self
            .types
            .borrow()
            .iter()
            .any(|ty| ty.as_ptr() as usize == address)
        {
            return;
        }
        // **已回收类型也认**（round 130）：本体仍在（`Box::leak`），裸 `ty` 与残余计数引用都可能回来 ✓。
        if self.retired_types.borrow().contains(&address) {
            return;
        }
        {
            // 诊断增强（round 59）：开了 `PYAWA_ZOMBIE_TRACE=1` 时附上**最早那次释放**的
            // 类型与现场（"谁把它放掉的" ✓）。
            let recorded = self
                .freed_sites
                .borrow()
                .get(&address)
                .cloned();
            match recorded {
                Some((name, freed_at)) => panic!(
                    "[悬垂] {site} 要碰 {address:#x}，但它**不在活表里** ✗ ⇒ 这个指针**已经被释放过** ✓\
                     （原类型 {name}；释放于 {freed_at}）"
                ),
                None => panic!(
                    "[悬垂] {site} 要碰 {address:#x}，但它**不在活表里** ✗ ⇒ 这个指针**已经被释放过** ✓"
                ),
            }
        }
    }
}

impl Instance {
    /// **引用事件跟踪**（round 90，诊断；`PYAWA_REF_TRACE=<类型名[,类型名]>`）：给该类型的对象
    /// 每次 incref／decref 打一行（**现场** = 当前 Python 站点）。`PYAWA_REF_TRACE_CLASS=<类名>`
    /// （round 99）另按**类型对象自己的名字**跟踪（盯"类对象本身"而不是它的实例 ✓）。
    /// 默认零输出（`OnceLock` 缓存环境变量 ✓）。
    pub(crate) fn ref_trace_matches(&self, ptr: *mut Header) -> bool {
        // **快路**（round 249 性能，A2-4）：两个跟踪开关都没开时，一次原子读直接返回 ✓
        //（本函数**每次 incref／release 都调**；先前要读两个 `OnceLock` 再判空 ✗）。
        static ANY_ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !*ANY_ENABLED.get_or_init(|| {
            std::env::var_os("PYAWA_REF_TRACE").is_some()
                || std::env::var_os("PYAWA_REF_TRACE_CLASS").is_some()
        }) {
            return false;
        }
        static TARGET: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
        static CLASS_TARGET: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
        // **按类自己的名字**：目标是注册表里的类型对象 ⇒ 用 `TypeObject::name` ✓。
        if let Some(class_targets) = CLASS_TARGET.get_or_init(|| {
            std::env::var("PYAWA_REF_TRACE_CLASS").ok().map(|text| {
                text.split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
        }) {
            let is_type = self
                .types
                .borrow()
                .iter()
                .any(|entry| entry.as_ptr() as usize == ptr as usize);
            if is_type {
                // SAFETY: 刚确认 ptr 是注册表里的类型对象（存活）。
                let type_name = unsafe { &*ptr.cast::<crate::TypeObject>() }.name();
                if class_targets.iter().any(|target| target == "*" || target == type_name) {
                    return true;
                }
            }
        }
        let Some(targets) = TARGET.get_or_init(|| {
            std::env::var("PYAWA_REF_TRACE")
                .ok()
                .map(|text| text.split(',').map(str::trim).filter(|part| !part.is_empty()).map(str::to_owned).collect())
        }) else {
            return false;
        };
        // SAFETY: 调用方保证 ptr 是存活对象。
        let ty = unsafe { &*ptr }.ty();
        // SAFETY: ty 由注册表持有。
        let name = unsafe { ty.as_ref() }.name();
        targets.iter().any(|target| target == "*" || target == name)
    }

    /// **引用事件跟踪（安全版）**：`release_object` 可能在**已释放**指针上调用 ⇒ 不能解引用头部
    /// （读 `ty` 会段错误 ✓）。这里只查"曾被跟踪器记录过的地址集合"（round 92）。
    fn ref_trace_tracked(ptr: *mut Header) -> bool {
        let Some(set) = REF_TRACKED.get() else {
            return false;
        };
        let Ok(set) = set.lock() else {
            return false;
        };
        set.contains(&(ptr as usize))
    }

    /// 记录被跟踪对象的地址（分配／incref 时调用；只在跟踪开启时有意义）。
    pub(crate) fn ref_trace_record(ptr: *mut Header) {
        const _: () = ();
        if REF_TRACKED.get().is_none()
            && std::env::var_os("PYAWA_REF_TRACE").is_none()
            && std::env::var_os("PYAWA_REF_TRACE_CLASS").is_none()
        {
            return;
        }
        let set = REF_TRACKED.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
        if let Ok(mut set) = set.lock() {
            set.insert(ptr as usize);
        }
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
        if refcount_canary() && unsafe { &*ptr }.refcount() == REFCOUNT_POISON {
            panic!(
                "[canary] incref 碰到**已释放**对象 {:#x}（refcount=毒值）⇒ 现场={}\n{}",
                ptr as usize,
                self.current_site(),
                std::backtrace::Backtrace::force_capture()
            );
        }
        // **对已释放对象的 incref 是 UAF**（round 94 诊断）：`PYAWA_ZOMBIE_TRACE=1` 时当场报出**现场** ✓
        // （先于后续 release 崩溃；用来点名"悬垂引用从哪来"）。判据用**释放登记**（`freed_sites`）：
        // 引导期那些"从不进活表"的单例不会被误报 ✓。
        if self.zombie_trace.get()
            && !self.live.borrow().contains(&(ptr as usize))
            && !self.is_registered_type(unsafe { NonNull::new_unchecked(ptr) })
            && self.freed_sites.borrow().contains_key(&(ptr as usize))
        {
            let site = self.current_site();
            panic!(
                "[悬垂-incref] incref 要碰 {:#x}，但它**不在活表里** ✗（已被释放过）⇒ 悬垂引用来自：{site}\n{}",
                ptr as usize,
                std::backtrace::Backtrace::force_capture()
            );
        }
        if self.ref_trace_matches(ptr) {
            Self::ref_trace_record(ptr);
            eprintln!(
                "[ref] +1 ptr={ptr:p} rc={} 现场={}",
                unsafe { &*ptr }.refcount() + 1,
                self.current_site()
            );
            if std::env::var_os("PYAWA_REF_TRACE_INCREF_BT").is_some() {
                eprintln!("{}", std::backtrace::Backtrace::force_capture());
            }
        }
        if self.watch.get() == ptr as usize {
            eprintln!(
                "[watch] incref {ptr:p} → rc={} 现场={}",
                unsafe { &*ptr }.refcount() + 1,
                self.current_site()
            );
        }
        // **盯帧诊断**（round 190）：`PYAWA_WATCH_FRAME=1` ＋ `PYAWA_WATCH_FRAME_CODE=<co_name>`
        // ⇒ 该 code 的帧被 incref／decref 时打现场＋回溯（用来找"谁给它加了引用却没记成 GC 边"）。
        static WATCH_FRAME: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if crate::diag::flag_cached("PYAWA_WATCH_FRAME", &WATCH_FRAME) {
            // SAFETY: ptr 由调用方保证存活。
            let header = unsafe { &*ptr };
            let ty = header.ty();
            // SAFETY: ty 由注册表持有。
            if unsafe { ty.as_ref() }.name() == "frame" {
                // SAFETY: ptr 是帧（类型名刚核过）。
                let frame = unsafe { &*ptr.cast::<crate::Frame>() };
                let code_name = frame
                    .code()
                    .map(|code| unsafe { &*code.as_ptr().cast::<crate::CodeObject>() })
                    .map(|code| code.name().to_owned())
                    .unwrap_or_default();
                let wanted = std::env::var("PYAWA_WATCH_FRAME_CODE").unwrap_or_default();
                if wanted.is_empty() || wanted == code_name {
                    eprintln!(
                        "[watch-frame] +1 {ptr:p} code={code_name} rc={} 现场={}\n{}",
                        header.refcount() + 1,
                        self.current_site(),
                        std::backtrace::Backtrace::force_capture()
                    );
                }
            }
        }
        // SAFETY: 由调用方保证 ptr 有效。
        unsafe { &*ptr }.incref();
    }

    /// **释放登记查询**（round 96 诊断）：这个地址是否已被释放过（`PYAWA_ZOMBIE_TRACE` 才记录 ✓）。
    pub(crate) fn was_freed(&self, ptr: NonNull<Header>) -> bool {
        self.freed_sites
            .borrow()
            .contains_key(&(ptr.as_ptr() as usize))
    }

    /// **注册表中类型对象**（round 96 诊断）：类型不进活表、由 `types` 注册表持有 ⇒ `incref` 的
    /// "不在活表"断言对它们要放行（`release_object` 对注册类型的 guard 会提前返回、不真释放 ✓）。
    pub(crate) fn is_registered_type(&self, ptr: NonNull<Header>) -> bool {
        self.types
            .borrow()
            .iter()
            .any(|ty| ty.as_ptr() as usize == ptr.as_ptr() as usize)
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

    /// **round 82**：待处理栈重放前的活表检查（死地址跳过 ✓）。
    pub(crate) fn is_live_address(&self, ptr: NonNull<Header>) -> bool {
        self.live.borrow().contains(&(ptr.as_ptr() as usize))
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
        // **盯帧诊断**（round 190/191）：释放侧对称打点 ✓。
        static WATCH_FRAME: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if crate::diag::flag_cached("PYAWA_WATCH_FRAME", &WATCH_FRAME) {
            // SAFETY: 调用方保证 ptr 存活。
            let header = unsafe { &*ptr };
            let ty = header.ty();
            // SAFETY: ty 由注册表持有。
            if unsafe { ty.as_ref() }.name() == "frame" {
                // SAFETY: ptr 是帧（类型名刚核过）。
                let frame = unsafe { &*ptr.cast::<crate::Frame>() };
                let code_name = frame
                    .code()
                    .map(|code| unsafe { &*code.as_ptr().cast::<crate::CodeObject>() })
                    .map(|code| code.name().to_owned())
                    .unwrap_or_default();
                let wanted = std::env::var("PYAWA_WATCH_FRAME_CODE").unwrap_or_default();
                if wanted.is_empty() || wanted == code_name {
                    eprintln!(
                        "[watch-frame] -1 {ptr:p} code={code_name} rc={} 现场={}",
                        header.refcount().saturating_sub(0),
                        self.current_site(),
                    );
                }
            }
        }
        // **按判据盯**（第 115 轮，`PYAWA_WATCH_DICT=1`）：任何 `dict` **掉到 0** 都报现场 ✓
        // —— 第 114 轮查明"死在 @540 的是**另一个类**的命名空间" ✓ ⇒ 盯一个地址不够 ✓，
        // 要用**判据**（类型＝`dict` ✓）把**第一个被打到 0 的那个**逼出来 ✓。
        if crate::diag::flag_site!("PYAWA_WATCH_DICT") && unsafe { &*ptr }.refcount() == 1 {
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
        if Self::ref_trace_tracked(ptr) {
            // **放在 `assert_live` 之前**（round 91）：双放现场要先被看见 ✓；**不解引用**（round 92：
            // 死指针读头部会段错误 ⇒ 释放行只在 `live=true` 时读计数 ✓）。
            let live_here = self.live.borrow().contains(&(ptr as usize));
            if live_here {
                let remaining = unsafe { &*ptr }.refcount().saturating_sub(1);
                eprintln!(
                    "[ref] -1 ptr={ptr:p} rc={remaining} live=true 现场={}",
                    self.current_site()
                );
                // **首次释放的 Rust 路径**（round 93 诊断）：`PYAWA_REF_TRACE_BT=1` 时打回溯；
                // `PYAWA_REF_TRACE_BT_ALL=1` 时**每次** -1 都打（round 104 诊断）。
                if (remaining == 0 && std::env::var_os("PYAWA_REF_TRACE_BT").is_some())
                    || std::env::var_os("PYAWA_REF_TRACE_BT_ALL").is_some()
                {
                    eprintln!("{}", std::backtrace::Backtrace::force_capture());
                }
            } else {
                // 类型对象不进活表但**仍存活**（注册表持有）⇒ 能安全读计数，打出来便于对账 ✓。
                let readable = self.is_registered_type(unsafe { NonNull::new_unchecked(ptr) });
                if readable {
                    eprintln!(
                        "[ref] -1 ptr={ptr:p} rc={} live=false(注册类型) 现场={}",
                        unsafe { &*ptr }.refcount().saturating_sub(1),
                        self.current_site()
                    );
                } else {
                    eprintln!("[ref] -1 ptr={ptr:p} live=false 现场={}", self.current_site());
                    if std::env::var_os("PYAWA_REF_TRACE_DEAD_BT").is_some() {
                        eprintln!("{}", std::backtrace::Backtrace::force_capture());
                    }
                }
            }
        }
        if refcount_canary() && unsafe { &*ptr }.refcount() == REFCOUNT_POISON {
            panic!(
                "[canary] release 碰到**已释放**对象 {:#x}（refcount=毒值）⇒ 现场={}\n{}",
                ptr as usize,
                self.current_site(),
                std::backtrace::Backtrace::force_capture()
            );
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
        // **round 246 性能**：判定改成升序地址表的二分（先前是**每次释放线性扫**一遍类型表 ✗）。
        if self.is_registered_type_address(ptr as usize) {
            // **诊断（round 98，`PYAWA_TYPE_REL_TRACE=1`）**：把"注册类型被放到 0"的**首次**现场打出来
            // （这是欠计数/多放的源头；guard 只挡住症状 ✓）。
            if crate::diag::flag_site!("PYAWA_TYPE_REL_TRACE") {
                // SAFETY: ptr 是注册表里的类型对象（存活）。
                let name = unsafe { &*ptr.cast::<crate::TypeObject>() }.name();
                eprintln!(
                    "[type-rel] 注册类型 {name} 被放到 0：现场={}\n{}",
                    self.current_site(),
                    std::backtrace::Backtrace::force_capture()
                );
            }
            return;
        }

        // 回收进行中：不可达对象由本次 `collect` 统一释放，这里只减计数（OM-27 ④）。
        {
            // **一次借用**（round 249 性能，A2-4）：先前 `is_empty()` 与 `contains()` 各借一次 ✗。
            let frozen = self.gc_frozen.borrow();
            if !frozen.is_empty() && frozen.contains(&(ptr as usize)) {
                return;
            }
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
            // **重复入队的是"终结延迟判定"的对象**（round 82）：它可能已在更早的一轮里被释放
            // （`FINALIZING` 保持 → 形参释放把计数打到 0 → 正常释放）⇒ 这里跳过死地址 ✓。
            if !self.is_live_address(object) {
                continue;
            }
            self.release_one(object);
        }
        self.draining.set(false);
    }

}
