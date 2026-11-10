//! `Instance` 的杂项域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

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

    /// 本实例中尚未释放的普通对象数（类型对象不计）。
    pub fn live_objects(&self) -> usize {
        self.live.borrow().len()
    }

    /// **全部存活对象地址快照**（round 121 诊断：反查"谁持有某个对象"时要用到**未跟踪**对象 ✓）。
    pub(crate) fn live_addresses(&self) -> Vec<usize> {
        self.live.borrow().iter().copied().collect()
    }

    /// 登记 **AST 解析回调**（round 86，由 `_ast` 模块在引导期调用一次 ✓）。
    pub fn set_ast_parser(&self, parser: crate::AstParserFn) {
        self.ast_parser.set(Some(parser));
    }

    /// 取 AST 解析回调（未登记 ⇒ `None` ✓）。
    pub fn ast_parser(&self) -> Option<crate::AstParserFn> {
        self.ast_parser.get()
    }

    /// 登记一个类型的 `__slots__` 槽名表（round 147）。
    pub(crate) fn register_type_slots(&self, ty: NonNull<crate::TypeObject>, names: Vec<String>) {
        if !names.is_empty() {
            self.type_slots
                .borrow_mut()
                .insert(ty.as_ptr() as usize, names.into_iter().collect());
        }
    }

    /// 名字在该类型的**MRO 上的槽名表**里吗？（`None` ＝整条 MRO 都没有 `__slots__` ⇒ 不限制 ✓）
    ///
    /// **必须沿 MRO 找**（round 147 真 bug 修 ✗）：`Lib/_pydatetime.py` 的
    /// `class datetime(date): __slots__ = time.__slots__` 只列了 time 的槽，而 `datetime.__new__`
    /// 会写 `self._year`（**date 的槽** ✓）——只看精确类型会误报 ✗（对拍 22 条 lib3_* 当场红 ✓）。
    pub(crate) fn type_slot_contains(&self, ty: NonNull<crate::TypeObject>, name: &str) -> Option<bool> {
        let table = self.type_slots.borrow();
        // **没有登记过任何 `__slots__` ⇒ 直接放行**（bootstrap 期一律走这条 ✓）：既快，也避免在
        // 运行时引导阶段对"还没成型"的类型对象解引用 `mro()`（round 147 实测：引导期 `_io`
        // 建流对象时 set_attribute 会崩 ✗）。
        if table.is_empty() {
            return None;
        }
        // SAFETY: 类型由注册表持有、MRO 条目都存活。
        let mro = unsafe { ty.as_ref() }.mro();
        let object_type = self.type_named("object");
        let mut contains = false;
        for entry in mro {
            let entry_ty = entry.cast::<crate::TypeObject>();
            // **`object` 自身不授予实例字典** ✓。
            if Some(entry_ty) == object_type {
                continue;
            }
            match table.get(&(entry.as_ptr() as usize)) {
                Some(slots) => {
                    // **私有名改写兼容**（round 150 真 bug 修 ✗）：源码里 `__slots__` 可写
                    // 原文名（`'__num'`）或已改写名（`'_Rat__num'`）；而本层**编译器未做**类内
                    // `self.__num` 的名字改写（属性名原样到达）⇒ 两种形态都要认 ✓
                    //（`test_binop.Rat`：`__slots__ = ['_Rat__num']`、写 `self.__num`，先前 8 格 error ✓）．
                    // SAFETY: 类型由注册表持有。
                    let class_name = unsafe { entry.as_ref() }.name().to_owned();
                    let stripped = class_name.trim_start_matches('_').to_owned();
                    let mangled = if !stripped.is_empty()
                        && name.starts_with("__")
                        && !name.ends_with("__")
                        && name.len() > 2
                    {
                        Some(format!("_{stripped}{name}"))
                    } else {
                        None
                    };
                    if slots.contains(name)
                        || mangled.as_ref().is_some_and(|mangled| slots.contains(mangled))
                        || (name.starts_with('_') && name.contains("__") && !name.ends_with("__") && {
                            let raw = name.split_once("__").map(|(_, rest)| format!("__{rest}"));
                            raw.as_ref().is_some_and(|raw| slots.contains(raw))
                        })
                    {
                        contains = true;
                    }
                }
                None => {
                    // **用户类**没声明 `__slots__` ⇒ 参照给它（及其子类）实例字典 ⇒ 不限制 ✓
                    // （`Lib/typing.py` 的 `_SpecialGenericAlias` 继承无 `__slots__` 的
                    // `_BaseGenericAlias` 正是这种 ✓）。
                    // **内建基类**（`dict`／`list` 等）没有槽表也**不授予**字典 ⇒ 继续走 ✓
                    //（`class defaultdict2(dict): __slots__ = [...]` 必须受限 ✓，tut2 ✓）。
                    if self.is_user_type_object(entry_ty) {
                        return None;
                    }
                }
            }
        }
        // 走到这里 ⇒ 没有"授予字典"的用户基类 ⇒ 限制生效 ✓。
        Some(contains)
    }

    /// 取"被迭代源类型 → 布局种类"缓存（round 146 性能，见 `iter.rs::source_kind`）。
    pub(crate) fn iterable_source_kind(&self, ty: NonNull<crate::TypeObject>) -> Option<u8> {
        self.iterable_source_kinds
            .borrow()
            .get(&(ty.as_ptr() as usize))
            .copied()
    }

    /// 写"被迭代源类型 → 布局种类"缓存（同上）。
    pub(crate) fn set_iterable_source_kind(&self, ty: NonNull<crate::TypeObject>, kind: u8) {
        self.iterable_source_kinds
            .borrow_mut()
            .insert(ty.as_ptr() as usize, kind);
    }

    /// 这个迭代器类型**有没有专用推进分支**（round 145 性能）：`advance_iterator` 的 20+ 个
    /// 内建名比较只对链表里的类型有意义；普通迭代器（`list_iterator`／`range_iterator` 等）一次
    /// 哈希查即可跳到通用路径 ✓。集合按 `types_generation` 缓存（惰性类型后建 ⇒ 自动重建 ✓）。
    pub(crate) fn is_dedicated_iterator_type(&self, ty: NonNull<crate::TypeObject>) -> bool {
        const DEDICATED: &[&str] = &[
            "list_reverseiterator",
            "reversed",
            "count",
            "repeat",
            "islice",
            "range_iterator",
            "longrange_iterator",
            "product",
            "permutations",
            "compress",
            "combinations",
            "combinations_with_replacement",
            "zip",
            "map",
            "filter",
            "zip_longest",
            "pairwise",
            "batched",
            "cycle",
            "accumulate",
            "starmap",
            "takewhile",
            "dropwhile",
            "filterfalse",
            "chain",
        ];
        let generation = self.types_generation.get();
        let mut cache = self.dedicated_iterator_types.borrow_mut();
        let stale = match cache.as_ref() {
            Some((cached, _)) => *cached != generation,
            None => true,
        };
        if stale {
            let mut set = std::collections::HashSet::with_capacity(DEDICATED.len());
            for name in DEDICATED {
                if let Some(found) = self.type_named(name) {
                    set.insert(found.as_ptr() as usize);
                }
            }
            *cache = Some((generation, set));
        }
        cache
            .as_ref()
            .is_some_and(|(_, set)| set.contains(&(ty.as_ptr() as usize)))
    }

    /// 是否是**用户定义类**的类型对象（round 135：迭代器协议的错误口径按它分档 ✓）。
    pub(crate) fn is_user_type(&self, ty: NonNull<Header>) -> bool {
        self.user_type_set.borrow().contains(&(ty.as_ptr() as usize))
    }

    /// 同上，入参是类型对象指针的便捷版（round 135）。
    pub(crate) fn is_user_type_object(&self, ty: NonNull<TypeObject>) -> bool {
        self.is_user_type(ty.cast::<Header>())
    }

    /// **登记动态（用户）类类型**（round 99）：参与"孤儿类型回收" ✓。
    pub(crate) fn register_user_type(&self, ty: NonNull<Header>) {
        let mut list = self.user_types.borrow_mut();
        if !list.iter().any(|entry| entry.as_ptr() == ty.as_ptr()) {
            list.push(ty);
            self.user_type_set
                .borrow_mut()
                .insert(ty.as_ptr() as usize);
        }
    }

    /// **登记 / 注销用户类实例**（round 119，`adopt`／`release_one`／`free_garbage` 调 ✓）。
    pub(crate) fn register_user_instance(&self, ty: NonNull<TypeObject>, object: NonNull<Header>) {
        if !self.user_type_set.borrow().contains(&(ty.as_ptr() as usize)) {
            return;
        }
        self.user_instances
            .borrow_mut()
            .entry(ty.as_ptr() as usize)
            .or_default()
            .insert(object.as_ptr() as usize);
    }

    pub(crate) fn unregister_user_instance(&self, ty: NonNull<TypeObject>, object: NonNull<Header>) {
        if self.user_type_set.borrow().is_empty() {
            return;
        }
        if let Some(entries) = self
            .user_instances
            .borrow_mut()
            .get_mut(&(ty.as_ptr() as usize))
        {
            entries.remove(&(object.as_ptr() as usize));
        }
    }

    /// 某个用户类型的活实例地址快照（孤儿回收用 ✓）。
    pub(crate) fn user_instances_of(&self, ty: NonNull<Header>) -> Vec<NonNull<Header>> {
        self.user_instances
            .borrow()
            .get(&(ty.as_ptr() as usize))
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|address| NonNull::new(*address as *mut Header))
                    .collect()
            })
            .unwrap_or_default()
    }


    /// 动态类类型的快照（GC 用 ✓）。
    pub(crate) fn user_type_headers(&self) -> Vec<NonNull<Header>> {
        self.user_types.borrow().clone()
    }

    /// **`__getattribute__` 派发守卫**（round 87）。
    pub fn getattribute_dispatching(&self) -> bool {
        self.getattribute_dispatching.get()
    }

    /// 设置 `__getattribute__` 派发守卫（round 87）。
    pub fn set_getattribute_dispatching(&self, value: bool) {
        self.getattribute_dispatching.set(value);
    }

    /// **本地追踪回调执行中**（round 181）。
    pub fn trace_dispatching(&self) -> bool {
        self.trace_dispatching.get()
    }

    /// 设置追踪回调执行标志。
    pub fn set_trace_dispatching(&self, value: bool) {
        self.trace_dispatching.set(value);
    }

    /// **全局追踪函数**（借用 → 新引用；`None` ⇒ 无）。
    pub fn global_trace(&self) -> Option<NonNull<Header>> {
        match *self.global_trace.borrow() {
            Some(value) => {
                // SAFETY: 实例持有这份引用；借用视图要新引用 ✓。
                unsafe { self.incref_object(value.as_ptr()) };
                Some(value)
            }
            None => None,
        }
    }

    /// 设置全局追踪函数（**接管** `value` 的新引用；`None` ⇒ 清掉 ✓）。
    pub fn set_global_trace(&self, value: Option<NonNull<Header>>) {
        let previous = self.global_trace.replace(value);
        if let Some(previous) = previous {
            // SAFETY: 旧值那份由实例持有。
            unsafe { self.release_object(previous.as_ptr()) };
        }
    }

    /// **全局性能钩子**（同 `global_trace` 口径）。
    pub fn global_profile(&self) -> Option<NonNull<Header>> {
        match *self.global_profile.borrow() {
            Some(value) => {
                // SAFETY: 实例持有这份引用；借用视图要新引用 ✓。
                unsafe { self.incref_object(value.as_ptr()) };
                Some(value)
            }
            None => None,
        }
    }

    /// 设置全局性能钩子（同 `set_global_trace` 口径）。
    pub fn set_global_profile(&self, value: Option<NonNull<Header>>) {
        let previous = self.global_profile.replace(value);
        if let Some(previous) = previous {
            // SAFETY: 旧值那份由实例持有。
            unsafe { self.release_object(previous.as_ptr()) };
        }
    }

    /// **类型对象 `__text_signature__` 旁表**（round 87）。
    pub fn set_type_text_signature(&self, ty: NonNull<Header>, signature: &'static str) {
        self.type_text_signatures
            .borrow_mut()
            .insert(ty.as_ptr() as usize, signature);
    }

    /// 取类型对象的文本签名（`None` ⇒ 没有 ✓）。
    pub fn type_text_signature(&self, ty: NonNull<Header>) -> Option<&'static str> {
        self.type_text_signatures
            .borrow()
            .get(&(ty.as_ptr() as usize))
            .copied()
    }

    /// **内建函数 `__text_signature__` 旁表**（round 86）。
    pub fn set_builtin_text_signature(&self, function: NonNull<Header>, signature: &'static str) {
        self.builtin_text_signatures
            .borrow_mut()
            .insert(function.as_ptr() as usize, signature);
    }

    /// 取内建函数的 `__text_signature__`（借用，`None` ⇒ 没有）✓。
    pub fn builtin_text_signature(&self, function: NonNull<Header>) -> Option<&'static str> {
        self.builtin_text_signatures
            .borrow()
            .get(&(function.as_ptr() as usize))
            .copied()
    }

    /// **Python 级终结器旁表**（round 82）：是否已跑过 `__del__` ✓。
    pub(crate) fn python_finalized_contains(&self, pointer: usize) -> bool {
        self.python_finalized.borrow().contains(&pointer)
    }

    /// 记一次 Python 级终结（round 82）。
    pub(crate) fn mark_python_finalized(&self, pointer: usize) {
        self.python_finalized.borrow_mut().insert(pointer);
    }

    /// 对象释放时摘掉旁表条目（round 82，地址复用安全 ✓）。
    pub(crate) fn forget_python_finalized(&self, pointer: usize) {
        self.python_finalized.borrow_mut().remove(&pointer);
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

    /// **OM-25**…**OM-30**：跑一次标记-清除，返回本次释放的对象数。
    ///
    /// 顺序按 **OM-27** 固定：① 求不可达集合 → ② 先清弱引用 → ③ 调终结器 → ④ 释放。
    /// 回收范围仅限 `GC_TRACKED` 对象（**OM-25**）；不可达但尚未释放的对象**禁止**暴露（**OM-30**）。
    /// **显式** `gc.collect()`：与自动回收区分（round 110）——孤儿用户类回收必须在显式回收时必跑 ✓
    /// （`test_free_after_iterating` 一类判据都显式 `gc.collect()` ✓），自动回收则节流（每 8 次一次）✓。
    pub fn collect_explicit(&self) -> usize {
        self.collect_explicit_generation(true)
    }

    /// **请求下一次自动回收做全量**（round 157）。
    pub fn request_full_collect(&self) {
        self.gc_force_full.set(true);
    }

    /// **自动回收**（round 157）：默认年轻代；每 16 轮一次全量 ✓。
    pub fn collect_auto(&self) -> usize {
        if self.gc_force_full.replace(false) {
            return self.collect();
        }
        let tick = self.gc_auto_ticks.get().wrapping_add(1);
        self.gc_auto_ticks.set(tick);
        if tick % 16 == 0 {
            self.collect()
        } else {
            self.collect_explicit_generation(false)
        }
    }

    /// **记异常分配位点**（round 165 诊断）。
    pub fn record_exception_site(&self, exception: NonNull<Header>, site: String) {
        let mut sites = self.exception_sites.borrow_mut();
        if sites.len() > 100_000 {
            return;
        }
        sites.insert(exception.as_ptr() as usize, site);
    }

    /// 读异常分配位点（round 165 诊断）。
    pub fn exception_site_of(&self, exception: NonNull<Header>) -> Option<String> {
        self.exception_sites
            .borrow()
            .get(&(exception.as_ptr() as usize))
            .cloned()
    }

    /// **按代显式回收**（round 148）：`full=false`（`gc.collect(0)`／`collect(1)`）时跳过孤儿类型回收 ✓。
    pub fn collect_explicit_generation(&self, full: bool) -> usize {
        let was_explicit = self.gc_explicit.get();
        let was_skip = self.gc_skip_reclaim.get();
        self.gc_explicit.set(true);
        self.gc_skip_reclaim.set(!full);
        let freed = self.collect();
        self.gc_skip_reclaim.set(was_skip);
        self.gc_explicit.set(was_explicit);
        freed
    }

    pub fn collect(&self) -> usize {
        if self.gc_running.get() {
            // 终结器／clear 里又触发了一次回收：本次让路，交给外层那次。
            return 0;
        }
        self.gc_running.set(true);
        self.gc_collections.set(self.gc_collections.get() + 1);
        let started = std::time::Instant::now();
        let live_before = self.live_objects();
        let freed = self.collect_inner();
        if std::env::var_os("PYAWA_EXC_STATE_DEBUG").is_some() {
            let pending = self.pending_exception();
            let pending_name = pending.map(|value| {
                // SAFETY: 槽里的对象由实例保活。
                let ty = unsafe { value.as_ref() }.ty();
                unsafe { ty.as_ref() }.name().to_owned()
            });
            eprintln!(
                "[exc-state] pending={:?} stack={}",
                pending_name,
                self.exception_depth()
            );
        }
        // **年轻代边界要在清扫之后取**（round 157 真 bug 修 ✗）：清扫前取的头指针可能是马上要释放的
        // 垃圾 ⇒ 边界悬垂 ⇒ 下一次"年轻段"走到链尾都碰不到它 ⇒ 把**整链**当年轻代（实测 378k ✗）。
        self.gc_young_mark.set(self.gc_head.get());
        self.gc_running.set(false);
        // 弱引用回调在回收循环**之后**统一调（round 59 ✓；回调里可安全跑 Python ✓）。
        self.drain_weakref_callbacks();
        // **诊断**（`PYAWA_GC_LOG=1` 才打）：回收前后存活数 ＋ 耗时（默认零输出 ✓）。
        // `PYAWA_GC_LOG=2` 另打**类型直方图 top 8**（查"GC 收不掉谁"用 ✓）。
        if crate::diag::flag("PYAWA_GC_LOG") && std::env::var("PYAWA_GC_LOG").as_deref() == Ok("2") {
            let headers = self.gc_headers();
            let mut counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for header in &headers {
                // SAFETY: 链上的对象都存活。
                let ty = unsafe { header.as_ref() }.ty();
                let name = unsafe { ty.as_ref() }.name().to_owned();
                *counts.entry(name).or_insert(0) += 1;
            }
            let mut top: Vec<(String, usize)> = counts.into_iter().collect();
            top.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            let summary: Vec<String> = top
                .iter()
                .take(8)
                .map(|(name, count)| format!("{name}={count}"))
                .collect();
            eprintln!("[gc-types] {}", summary.join(" "));
        }
        // `PYAWA_GC_LOG=3`：给几种"疑似泄漏"的对象**反查持有者链**（最多 5 跳；一次性诊断 ✓）。
        if crate::diag::flag("PYAWA_GC_LOG") && std::env::var("PYAWA_GC_LOG").as_deref() == Ok("3") {
            let headers = self.gc_headers();
            let mut referrers: std::collections::HashMap<usize, Vec<NonNull<Header>>> =
                std::collections::HashMap::new();
            for header in &headers {
                for child in self.children_of(*header) {
                    referrers.entry(child as usize).or_default().push(*header);
                }
            }
            for kind in ["AttributeError", "DummyKey", "DummyValue", "weakref", "dict"] {
                let Some(start) = headers.iter().find(|header| {
                    // SAFETY: 链上对象都存活。
                    let ty = unsafe { header.as_ref() }.ty();
                    unsafe { ty.as_ref() }.name() == kind
                }) else {
                    continue;
                };
                let mut cursor = *start;
                let mut chain: Vec<String> = Vec::new();
                for _ in 0..5 {
                    let header = unsafe { cursor.as_ref() };
                    let ty = header.ty();
                    chain.push(format!(
                        "{}(rc={})",
                        unsafe { ty.as_ref() }.name(),
                        header.refcount()
                    ));
                    let Some(parents) = referrers.get(&(cursor.as_ptr() as usize)) else {
                        break;
                    };
                    let Some(parent) = parents
                        .iter()
                        .find(|parent| parent.as_ptr() as usize != cursor.as_ptr() as usize)
                    else {
                        break;
                    };
                    cursor = *parent;
                }
                eprintln!("[gc-hold] {} => {}", kind, chain.join(" <- "));
            }
        }
        if crate::diag::flag("PYAWA_GC_LOG") {
            eprintln!(
                "[gc] index={} live={} live_after={} freed={} ms={} tb_created={} tb_freed={}",
                self.gc_collections.get(),
                live_before,
                self.live_objects(),
                freed,
                started.elapsed().as_millis(),
                self.traceback_created.get(),
                self.traceback_freed.get()
            );
        }
        freed
    }

    /// **已执行的回收次数**（诊断；`P3-41` 慢因修复的回归测试用）。
    pub fn gc_collections(&self) -> usize {
        self.gc_collections.get()
    }
}

/// **`float.__getformat__`**（task-23 ✓；参照 3.14.4 实测）：`test/support/__init__.py:522`
/// 在**导入期**做 `float.__getformat__('double')` ✓ ⇒ 类型字典里要这个名字 ✓。
///
/// 纯函数（参照类别就是 `builtin_function_or_method` ✓ —— 方案 A 下与参照同类 ✓）：
/// `'double'`／`'float'` ⇒ `'IEEE, little-endian'`（本机 x86 实测；**端序取宿主** ✓）；
/// 其它串 ⇒ `ValueError: __getformat__() argument 1 must be 'double' or 'float'`；
/// 非 str ⇒ `TypeError: __getformat__() argument must be str, not <类型>`；
/// 参数个数／关键字文案照参照 ✓。
pub(crate) fn float_getformat_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "float.__getformat__() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "float.__getformat__() takes exactly one argument ({} given)",
                args.len()
            ),
        ));
    }
    let Some(text) = instance.text_of(args[0]) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "__getformat__() argument must be str, not {}",
                instance.type_name(instance.type_of(args[0]))
            ),
        ));
    };
    let format = match text {
        "double" | "float" => {
            if cfg!(target_endian = "little") {
                "IEEE, little-endian"
            } else {
                "IEEE, big-endian"
            }
        }
        _ => {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "__getformat__() argument 1 must be 'double' or 'float'",
            ))
        }
    };
    Ok(instance.new_str(format))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`P3-41` 慢因修复的回归**：自动回收的水位必须随**存活对象数**放大。
    ///
    /// 固定阈值（2000）下，单代标记-清除每 2000 次分配就做一次 **O(存活)** 的全堆扫描 ⇒
    /// "存活对象多"的工作负载退化成 O(n²)（`test_frame` 的 15 万层嵌套：26.9 s）。
    /// 这里撑起 256 个可达 dict ＋ 引导期的固定对象，然后做 6400 次分配：
    /// 固定阈值会收 ~100 次，放大水位后应收得少得多。
    #[test]
    fn auto_collection_waterline_scales_with_live_objects() {
        let instance = Instance::new();
        instance.set_gc_threshold((64, 10, 0));
        let root = instance.new_dict();
        let mut held = Vec::new();
        for index in 0..256 {
            let child = instance.new_dict();
            instance.dict_set(root, &format!("k{index}"), child);
            held.push(child);
        }
        let baseline = instance.gc_collections();
        for index in 0..6_400 {
            let temporary = instance.new_dict();
            let value = instance.new_int(index as i64);
            instance.dict_set_int(temporary, index, value);
            let _ = temporary;
        }
        let collections = instance.gc_collections() - baseline;
        assert!(collections >= 1, "阈值仍然生效：6400 次分配里必须真的回收过");
        assert!(
            collections < 40,
            "水位应随存活放大（固定阈值下约 100 次），实测 {collections} 次"
        );
        let _ = (&root, &held);
    }
}
