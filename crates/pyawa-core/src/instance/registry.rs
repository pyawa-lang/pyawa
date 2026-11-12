//! `Instance` 的类型注册表与引导域（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// **OM-13**／**OM-14**：登记基类，MRO 由 C3 算出并写入；不一致时返回 `None`。
    pub fn register_bases(
        &self,
        ty: NonNull<TypeObject>,
        bases: Vec<NonNull<TypeObject>>,
    ) -> Option<Vec<NonNull<TypeObject>>> {
        let mro = self.linearize(ty, &bases)?;
        // SAFETY: ty 由本实例的注册表持有。
        let type_object = unsafe { ty.as_ref() };
        // **round 248 性能（A2-3）**：基类可能命中"用户覆写 dunder" ⇒ 子类也可命中（继承这个能力位 ✓）。
        if bases
            .iter()
            .any(|base| unsafe { base.as_ref() }.is_dunder_capable())
        {
            type_object.mark_dunder_capable();
        }
        type_object.set_bases(bases, mro.clone());
        Some(mro)
    }

    /// 类型对象的**名字**（安全读取；给诊断消息与 stdlib 用）。
    pub fn type_name(&self, ty: NonNull<TypeObject>) -> String {
        // SAFETY: 类型对象由注册表持有。
        unsafe { ty.as_ref() }.name().to_owned()
    }

    /// 注册表里的**全部类型**（克隆一份借用视图 ✓）—— 给 `type.__subclasses__()` 这类
    /// "按基类**反查**"的入口用 ✓（第 722 轮 ✓）。
    pub fn registered_types(&self) -> Vec<NonNull<TypeObject>> {
        self.types.borrow().clone()
    }

    /// 对象的类型（**借用**）。
    pub fn type_of(&self, object: NonNull<Header>) -> NonNull<TypeObject> {
        // SAFETY: 调用方保证 object 存活。
        unsafe { object.as_ref() }.ty()
    }

    /// 按名字在注册表里找一个类型。
    ///
    /// 这是**内部**查询（`TS-41` 的对拍与引导期要用）；Python 可见的属性访问**必须**走
    /// `OM-11` 的 `getattr` 槽位，**禁止**用这个函数旁路属性通道。
    pub fn type_named(&self, name: &str) -> Option<NonNull<TypeObject>> {
        // **O(1) 名字索引**（round 142 性能修 ✓）：先查索引；缺失时退回线性扫（兼容"索引之前的登记"✓
        // 并顺手补索引 ✓）。
        if let Some(found) = self.types_by_name.borrow().get(name) {
            return Some(*found);
        }
        // **负缓存**（round 248 性能，A2-3）：**未命中的名字也 O(1)** —— 先前每次 miss 都要
        // **线性扫全表** ✗（`HotTypes` 重建时每个未登记名字都扫一遍；`Iter<NonNull<TypeObject>>`
        // 那条热点正是这里）。按 `types_generation` 失效：登记新类型后清空 ⇒ 后补的名字仍能查到 ✓。
        {
            let generation = self.types_generation.get();
            let mut missing = self.types_missing.borrow_mut();
            if missing.0 != generation {
                *missing = (generation, std::collections::HashSet::new());
            }
            if missing.1.contains(name) {
                return None;
            }
        }
        let found = self.types.borrow().iter().copied().find(|ty| {
            // SAFETY: 注册表里的类型都存活。
            unsafe { ty.as_ref() }.name() == name
        });
        let Some(found) = found else {
            // 记进负缓存（同一代数内不再扫表 ✓）；`types_generation` 变化时整体失效 ✓。
            let generation = self.types_generation.get();
            let mut missing = self.types_missing.borrow_mut();
            if missing.0 != generation {
                *missing = (generation, std::collections::HashSet::new());
            }
            missing.1.insert(name.to_owned());
            return None;
        };
        self.types_by_name
            .borrow_mut()
            .entry(name.to_owned())
            .or_insert(found);
        Some(found)
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

    /// **round 248 性能（A2-3）**：这个类型的命名空间可能命中"用户覆写 dunder"吗？（一次位测 ✓）
    pub(crate) fn dunder_capable(&self, ty: NonNull<TypeObject>) -> bool {
        // SAFETY: 类型对象由注册表持有、实例存活期间一直有效。
        unsafe { ty.as_ref() }.is_dunder_capable()
    }

    /// **round 248 性能（A2-3）**：把"命名空间里挂了 `__module__`"上报成类型标志（原生类型用 ✓）。
    pub fn mark_dunder_capable(&self, ty: NonNull<TypeObject>) {
        // SAFETY: 类型对象由注册表持有、实例存活期间一直有效。
        unsafe { ty.as_ref() }.mark_dunder_capable();
    }

    /// **OM-23**：本实例的单例表。
    pub fn singletons(&self) -> &Singletons {
        self.singletons
            .get()
            .expect("单例表在 Instance::new 中引导，必然存在")
    }
}
