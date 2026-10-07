//! `Instance` 的 VM 状态字段 setter（从 `instance/containers.rs` 正名搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// 装/取**模块表**（与 `sys.modules` 同一份 ✓；返回旧的，调用方负责释放）。
    pub fn set_modules(&self, mapping: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        // **本方法自己 `retain` 新的那一份** ✓（第 199 轮**真 bug 修复** ✗）：模块表有**两处**持有者 ✓
        // —— 本实例 ✓ 与 `sys.modules` 里那一项 ✓ ⇒ 先前只留一份引用 ✗ ⇒ `find_unreachable` 把"本实例的这份"
        // 当成**候选内部引用**减掉 ✓ ⇒ external 归零 ✗ ⇒ **模块表被判不可达** ✗ ⇒ 整个模块表连同所有模块被 `free` ✗
        // ⇒ 活对象被释放 ⇒ 堆损坏 ✓（实测：修前 `原始 refcount=1`／`external=0` ✗，修后 `2`／`1` ✓）。
        if let Some(new) = mapping {
            // SAFETY: new 由调用方保证存活。
            unsafe { self.incref_object(new.as_ptr()) };
        }
        core::mem::replace(&mut *self.modules.borrow_mut(), mapping)
    }

    /// **注册一个能力域**（`AB-33`／`AB-34`）：`classification` 缺失 ⇒ 注册**失败** ✓
    /// （`CP-25`：禁止落默认值）。返回是否注册成功。
    pub fn set_capability(
        &self,
        domain: usize,
        implementation: *const core::ffi::c_void,
        classification: Option<i32>,
    ) -> bool {
        if classification.is_none() {
            return false;
        }
        let mut slots = self.capabilities.borrow_mut();
        match slots.get_mut(domain) {
            Some(slot) => {
                slot.implementation = implementation;
                slot.classification = classification;
                true
            }
            None => false,
        }
    }

    /// **`OM-10`**：往类型字典里写一项（**新引用**，由字典接手；返回被顶下来的旧值）。
    ///
    /// 字典惰性创建。**禁止**用这个函数给内建类型旁路属性通道——
    /// Python 可见属性一律走 `OM-11` 的 `getattr` 槽位。
    pub fn set_type_attribute(
        &self,
        ty: NonNull<TypeObject>,
        name: &str,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        // SAFETY: ty 由注册表持有。
        let type_object = unsafe { ty.as_ref() };
        let mapping = match type_object.dict() {
            Some(mapping) => mapping,
            None => {
                let mapping = self
                    .adopt(DictObject::new(
                        self.type_named("dict").expect("dict 已在引导期登记"),
                        RefCell::new(Vec::new()),
                    ))
                    .cast::<Header>();
                type_object.set_dict(Some(mapping));
                mapping
            }
        };
        // SAFETY: mapping 由类型对象持有。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let key = self
            .adopt(StrObject::new(self.singletons().str_type(), name.to_owned()))
            .cast::<Header>();
        let position = dict
            .entries()
            .into_iter()
            .position(|(existing, _)| str_matches(self, existing, name));
        match position {
            Some(slot) => {
                // 键已在表里：新键那份引用交回去
                // SAFETY: key 是刚 adopt 的对象，只有这一份引用。
                unsafe { self.release_object(key.as_ptr()) };
                dict.replace_value(slot, value)
            }
            None => {
                dict.insert_raw(key, value);
                None
            }
        }
    }

    /// 装一个内建名字空间（**新引用**，由实例接手；返回被顶下来的旧值）。
    pub fn set_builtins(&self, mapping: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.builtins.replace(mapping)
    }

    /// 挂上／恢复当前帧对象（第 230 轮）；**只给执行器的 RAII 守卫用** ✓。
    pub fn set_current_frame(&self, frame: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.current_frame.replace(frame)
    }

    /// 挂上／恢复当前帧的全局映射（第 156 轮）；**只给执行器的 RAII 守卫用** ✓。
    pub fn set_current_globals(&self, globals: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.current_globals.replace(globals)
    }

    /// **存属性**（第 148 轮）：复用 `STORE_ATTR` 那条路 ✓（`opcode` 只用于错误消息 ⇒ 给 0 ✓）。
    /// **`setattr(obj, 名, 值)` 的协议口径** ✓（第 706 轮）：类型上有 `__setattr__` 就交给它 ✓，
    /// 只有走不通才落到默认的实例字典 ✓（参照里 `setattr` 是**协议调用** ✓，不是"直接写字典" ✗）。
    /// 上游 `enum.py:373` 的 `setattr(self, '_generate_next_value', _gnv)` 正是靠它 ✓
    /// （先前直接写实例字典 ✗ ⇒ 读回来 `AttributeError: 'EnumDict' object has no attribute
    /// '_generate_next_value'` ✓ ⇒ "换回上游 `enum.py`"卡住 ✓）。
    pub fn set_attribute_with_protocol(
        &self,
        object: NonNull<Header>,
        name: &str,
        value: NonNull<Header>,
    ) -> Result<(), ExecError> {
        if let Ok(crate::executor::Attribute::Method { function, this }) =
            crate::executor::attribute_lookup(self, object, "__setattr__")
        {
            // SAFETY: 实参是帧值栈上的存活对象，交给调用方前各添一份新引用。
            unsafe { self.incref_object(value.as_ptr()) };
            let name_object = self.new_str(name);
            crate::executor::call::call_callable(
                self,
                function,
                Some(this),
                vec![name_object, value],
                Vec::new(),
                0,
            )?;
            return Ok(());
        }
        self.set_attribute_value(object, name, value)
    }

    pub fn set_attribute_value(
        &self,
        object: NonNull<Header>,
        name: &str,
        value: NonNull<Header>,
    ) -> Result<(), ExecError> {
        // **`value` 是调用方借用的** ✓（`setattr` 那条路传的是 `args[2]` ✓）——而
        // `instance_attribute_set` 是**接管语义** ✓ ⇒ 这里必须**先给自己那份** ✗
        //（第 209 轮真 bug 修复 ✗：先前没加 ⇒ 属性表里的指针**没有计数** ✗ ⇒ 值被提前释放 ⇒
        // 属性表／函数字典**释放后重用** ⇒ 堆损坏 ✓。口径与 `attribute_write` 完全一致 ✓ = 一处真相 ✓）。
        // SAFETY: 调用方保证 value 存活；属性表要自己那份。
        unsafe { self.incref_object(value.as_ptr()) };
        crate::executor::protocol::instance_attribute_set(self, object, name, value, 0)
    }
}
