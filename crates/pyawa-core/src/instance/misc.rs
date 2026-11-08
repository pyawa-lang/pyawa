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
                "[gc] index={} live={} live_after={} freed={} ms={}",
                self.gc_collections.get(),
                live_before,
                self.live_objects(),
                freed,
                started.elapsed().as_millis()
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
