//! `Instance` 的类型/对象/整数/文本取值与判定域（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
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
    /// **造一个带实例字典的子类**（第 322 轮改中性名 ✓；原为 `binascii.Error` 而加 ✓ —— 现在 `_multibytecodec` 的 5 个类也用它 ✓）：stdlib 是
    /// `#![forbid(unsafe_code)]` ✗（要解引用 raw 指针 ✓）⇒ 造类必须留在 core ✓。
    /// **通用分配**（`attribute_new` ✓）⇒ 实例类型＝子类 ✓（沿用基类 `new` ⇒ 类型是基类 ✗ ⇒ `except`
    /// 接不住 ✗，实测 ✓）；**自带一个只读实例属性的 `str` 槽** ✓ —— **不抄基类槽** ✗（基类槽假定基类
    /// 载荷 ⇒ 布局不符 ⇒ 内存中止 ✗，第 316 轮实测 ✓），也**不碰载荷** ✗ ⇒ 不会再崩 ✓。
    pub fn new_subclass_with_instance_dict(
        &self,
        name: &'static str,
        base_name: &str,
    ) -> Option<NonNull<Header>> {
        let base = self.type_named(base_name)?;
        let size = core::mem::size_of::<crate::builtin_objects::AttributeObject>();
        let ty = self.new_type(
            name,
            size,
            crate::builtin_objects::AttributeObject::slots()
                .with_new(crate::builtin_objects::attribute_new)
                .with_str(crate::builtin_objects::generic_exception_str),
        );
        self.register_bases(ty, vec![base]);
        // SAFETY: ty 刚由 new_type 注册，由注册表持有。
        unsafe { ty.as_ref() }.mark_has_instance_dict();
        Some(ty.cast::<Header>())
    }

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

    /// 把一个类型对象当**值**用（**新引用**；给 `isinstance(x, T)` 这类传参）。
    pub fn type_value(&self, ty: NonNull<TypeObject>) -> NonNull<Header> {
        let header = ty.cast::<Header>();
        // SAFETY: 类型对象由注册表持有，存活。
        unsafe { self.incref_object(header.as_ptr()) };
        header
    }

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

    /// 读字符串内容（**复制**；不是 `str` 给 `None`）。
    pub fn text_value(&self, object: NonNull<Header>) -> Option<String> {
        if self.type_of(object) != self.singletons().str_type() {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<StrObject>() }.value().to_owned())
    }

    /// **`TS-45` ①**：当前 `int`↔`str` 的位数上限（`0` ＝ 不限）。
    pub fn int_max_str_digits(&self) -> u32 {
        self.int_max_str_digits.get()
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

    /// **`OM-11` 的 `str` 槽**：`str(对象)`。
    ///
    /// `SPEC-type-system.md` §8：该槽**省略时回退到 `repr`**。失败经 `Result` 上抛
    /// （`OM-11` 扩：`TS-45` ①的输出方向要能抛 `ValueError`）。
    pub fn object_str(&self, object: NonNull<Header>) -> Result<String, ExecError> {
        // **`TS-44`**：先走**属性通道**（类型字典里的 `__str__` 覆写）—— `override_text` 会
        // **忽略 `object` 自己那条** ✓（那是第 210 轮新挂的**属性面** ✓、不是格式化覆写 ✓）⇒
        // 内建类型仍走各自的 `str` 槽 ✓（否则 `object.__str__` 在每个 MRO 命中 ⇒ `f"{x}"` 给出
        // `\'1\'` ✗，实测四条 f-string 语料会红 ✓）。
        if let Some(text) = crate::executor::protocol::override_text(self, object, "__str__")? {
            return Ok(text);
        }
        self.object_str_native(object)
    }

    /// `str(对象)` 的**槽位**路径（`TS-44`：不走属性通道）——给已经是"通道内层"的调用方用，
    /// 免得 `element_repr` 这类已经查过覆写的地方再查一次（那会自递归）。
    pub fn object_str_native(&self, object: NonNull<Header>) -> Result<String, ExecError> {
        // **`TS-44`**（第 597 轮真 bug 修 ✓）：先走**属性通道**（类字典里的 `__str__` 覆写 ✓）——
        // 与 `object_repr` 对 `__repr__` 的口径**一致** ✓。先前这里只认类型的 `str` **槽** ✗
        // ⇒ 用户类覆写 `__str__` 时 `print(a)` 走不到它 ✗（实测：`str(a)` ⇒ `A-str` ✓ 而
        // `print(a)` ⇒ `<A object at 0x…>` ✗，参照两处都是 `A-str` ✓）——面很宽 ✓（凡覆写 `__str__` 的类都错 ✓）。
        if let Some(text) = crate::executor::protocol::override_text(self, object, "__str__")? {
            return Ok(text);
        }
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        // SAFETY: ty 由注册表持有。
        if let Some(slot) = unsafe { ty.as_ref() }.slots().str {
            // SAFETY: 槽位契约见 `StrFn`。
            return unsafe { slot(object.as_ptr(), self) };
        }
        // **回落**：没有 `str` 槽时按参照走 **`__repr__`** ✓（`object.__str__` 就是回落 `__repr__` ✓）——
        // `object_repr` 自带"属性通道的 `__repr__` ⇒ 槽 ⇒ 默认形式"这条链 ✓（一处真相 ✓，
        // 先前这里直接 `object_repr_native` ✗ ⇒ **只有 `__repr__` 的类**的 `print` 会打默认形式 ✗）。
        self.object_repr(object)
    }

    /// **`OM-11` 的 `repr` 槽**：`repr(对象)`；槽位省略时给默认形式（`SPEC-type-system.md` §8）。
    pub fn object_repr(&self, object: NonNull<Header>) -> Result<String, ExecError> {
        // **`TS-44`**：先走**属性通道**（类型字典里的 `__repr__` 覆写）——与
        // `repr([obj])` 里元素的口径一致（此前顶层 `repr(obj)` 会**忽略**覆写，那是不一致）。
        if let Some(text) = crate::executor::protocol::override_text(self, object, "__repr__")? {
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

    /// **OM-15**：本实例注册的类型对象数。
    pub fn type_count(&self) -> usize {
        self.types.borrow().len()
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
}
