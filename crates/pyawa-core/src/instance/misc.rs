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
        let freed = self.collect_inner();
        self.gc_running.set(false);
        freed
    }
}
