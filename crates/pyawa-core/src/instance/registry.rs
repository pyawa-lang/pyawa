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
        unsafe { ty.as_ref() }.set_bases(bases, mro.clone());
        Some(mro)
    }

    /// 类型对象的**名字**（安全读取；给诊断消息与 stdlib 用）。
    pub fn type_name(&self, ty: NonNull<TypeObject>) -> String {
        // SAFETY: 类型对象由注册表持有。
        unsafe { ty.as_ref() }.name().to_owned()
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
        self.types
            .borrow()
            .iter()
            .copied()
            .find(|ty| {
                // SAFETY: 注册表里的类型都存活。
                unsafe { ty.as_ref() }.name() == name
            })
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

    /// **OM-23**：本实例的单例表。
    pub fn singletons(&self) -> &Singletons {
        self.singletons
            .get()
            .expect("单例表在 Instance::new 中引导，必然存在")
    }
}
