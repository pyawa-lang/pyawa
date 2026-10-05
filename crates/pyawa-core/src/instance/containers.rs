//! `Instance` 的容器访问器域（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
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

    /// 元组的元素（**借用视图**；不是元组给 `None`）。
    pub fn tuple_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        if !self.type_named("tuple").is_some_and(|base| self.is_subtype(self.type_of(object), base)) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        let tuple = unsafe { &*object.as_ptr().cast::<TupleObject>() };
        Some((0..tuple.len()).map(|index| tuple.item(index).expect("下标在范围内")).collect())
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

    /// 摊开一个 `list` 的元素（**借用**一份拷贝；不是 `list` 给 `None`）。
    pub fn list_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        if !self.type_named("list").is_some_and(|base| self.is_subtype(self.type_of(object), base)) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<ListObject>() }.items().to_vec())
    }

    /// 摊开一个 `dict` 的键值对。
    pub fn dict_entries(
        &self,
        object: NonNull<Header>,
    ) -> Option<Vec<(NonNull<Header>, NonNull<Header>)>> {
        if !self.type_named("dict").is_some_and(|base| self.is_subtype(self.type_of(object), base)) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<DictObject>() }.entries())
    }

    /// 摊开一个 `set` 的元素。
    pub fn set_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        // **`frozenset` 也算** ✓（第 236 轮）。
        let _ty = self.type_of(object);
        if !["set", "frozenset"].iter().any(|name| {
            self.type_named(name)
                .is_some_and(|base| self.is_subtype(self.type_of(object), base))
        }) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<SetObject>() }.items().to_vec())
    }

    /// 往 `list` 追加一项（**接管** `item` 的那份引用）。
    pub fn list_append(&self, list: NonNull<Header>, item: NonNull<Header>) {
        // SAFETY: 调用方保证 list 是本实例的 `list`（名字与类型都在契约里）。
        unsafe { &*list.as_ptr().cast::<ListObject>() }.append(item);
    }

    /// 往 `dict` 写入一对（**接管** key／value 各一份引用；不查重）。
    pub fn dict_insert_raw(
        &self,
        dict: NonNull<Header>,
        key: NonNull<Header>,
        value: NonNull<Header>,
    ) {
        // SAFETY: 同上。
        unsafe { &*dict.as_ptr().cast::<DictObject>() }.insert_raw(key, value);
    }

    /// 往 `set` 写入一项（**接管**一份引用；不查重）。
    pub fn set_insert_raw(&self, set: NonNull<Header>, item: NonNull<Header>) {
        // SAFETY: 同上。
        unsafe { &*set.as_ptr().cast::<SetObject>() }.insert_raw(item);
    }

    /// 读整数载荷（`int` 与 `bool` 都算；别的给 `None`）。
    ///
    /// **这是 `i64` 快路径**：大整数（`TS-45`）在这里给 `None`——那**不代表"不是整数"**。
    /// 要按类型分派的地方用 [`Instance::int_of`]。
    /// **`__index__` 感知的取整** ✓（第 228 轮）：先按整数读 ✓，读不出再走 **`__index__` 协议** ✓。
    ///
    /// 参照的 `range()`／下标／切片／`bin()` 一族都认它 ✓ —— 本层先前**只认整数** ✗
    /// （实测上游 `Lib/os.py` 那条链就是被它挡住的 ✓）。
    /// **把存活对象的类型改指** ✓（第 228 轮）：给"**同一份载荷、两个类型名**"那种情形用 ✓
    /// （`range()` 的大整数上限 ⇒ 迭代器要叫 `longrange_iterator` ✓，参照也分两个名字 ✓）。
    pub fn set_type_of(&self, object: NonNull<Header>, ty: NonNull<TypeObject>) {
        // SAFETY: object 存活（由调用方保证）；ty 是注册表里的类型 ✓。
        unsafe { object.as_ref() }.set_ty(ty);
    }

    /// 容器／字符串长度（`str` 按**字节**数；别的给 `None`）。
    pub fn length_of(&self, object: NonNull<Header>) -> Option<usize> {
        let ty = self.type_of(object);
        if self
            .type_named("str")
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<StrObject>() }.value().len());
        }
        if self
            .type_named("bytes")
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<BytesObject>() }.value().len());
        }
        if self
            .type_named("deque")
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<crate::builtin_objects::DequeObject>() }.len());
        }
        // **按子类型判**（第 99 轮真 bug 修 ✗）：先前是**精确类型**比较 ✗ ⇒ `class D(dict)` 这种
        // **内建类型的子类**一律落空 ✓（两行复现：`class D(dict)` ⇒ `len(D())` 报
        // `TypeError: object of type 'D' has no len()` ✗，参照给 0 ✓）。
        if self
            .type_named("dict")
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<DictObject>() }.entries().len());
        }
        // **`set`／`frozenset` 按自己的载荷读** ✓（第 236 轮顺手修 ✗）：先前这里把 `set` **当 `DictObject`** 读 ✗
        // ⇒ 长度靠"两种载荷碰巧同布局"歪打正着 ✓；现在明写 ✓。
        if ["set", "frozenset"].iter().any(|name| {
            self.type_named(name)
                .is_some_and(|base| self.is_subtype(ty, base))
        }) {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<SetObject>() }.items().len());
        }
        if self
            .type_named("list")
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<ListObject>() }.len());
        }
        if self
            .type_named("tuple")
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<TupleObject>() }.len());
        }
        None
    }

    /// **`bytes` 的载荷**（**借用**；不是 `bytes` 给 `None`）。
    pub fn bytes_value(&self, object: NonNull<Header>) -> Option<&[u8]> {
        if Some(self.type_of(object)) == self.type_named("bytes") {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<BytesObject>() }.value());
        }
        None
    }

    pub fn dict_set(&self, mapping: NonNull<Header>, key: &str, value: NonNull<Header>) {
        self.zombie_probe(mapping);
        // **借用 ⇒ 自己加一份** ✓（`OM-` 口径统一 ✓）。
        unsafe { self.incref_object(value.as_ptr()) };
        // **接管前的"欠计数"检测** ✓（第 275 轮，`PYAWA_DANGLING=1`）：`dict_set` **接管**一份引用 ✓
        // ⇒ 交来的值若**引用计数已是 0** ✗ ⇒ 调用方给的是**借来的**（或已死的）那份 ✓ ⇒ 字典从此持有一份
        // **不存在的**引用 ✓ ⇒ 迟早悬垂 ✓。报出**键名** ✓ ⇒ 一次把这类站点逐个点出来 ✓。
        if dangling_mode() {
            let header = unsafe { value.as_ref() };
            if !header.is_immortal() && header.refcount() == 0 {
                panic!("[欠计数] dict_set(`{key}`) 接管的值**计数已是 0** ✗ ⇒ 调用方交的是**借来的**引用 ✓");
            }
        }
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        // 查重用一个**临时键**（借用视图）：查完立刻归还，字典自己另存一份
        let probe = self.new_str(key);
        let position = dict
            .entries()
            .iter()
            .position(|(existing, _)| crate::executor::values::values_equal_public(self, *existing, probe));
        // SAFETY: probe 是新引用，比较完即归还。
        unsafe { self.release_object(probe.as_ptr()) };
        if let Some(position) = position {
            if let Some((old_key, old_value)) = dict.remove(position) {
            // **同值重存 ＋ 计数只有 1** ✗（第 275 轮）：`dict_set` 会**先释放旧值** ✓ ⇒ 若新旧是**同一个**
            // 对象、且它的计数已只剩 1 ✓ ⇒ 这一释放**当场把它打死** ✗ ⇒ 字典随即存进**悬垂指针** ✓。
            // ⇒ 报出**键名** ✓（调用方该在存之前 `retain` ✓）。
            if dangling_mode() && old_value.as_ptr() == value.as_ptr() {
                let header = unsafe { value.as_ref() };
                if !header.is_immortal() && header.refcount() <= 1 {
                    panic!("[同值重存] dict_set(`{key}`) 的旧值与新值同一个对象、计数只有 {} ✗ ⇒ 替换时会被打死 ✓", header.refcount());
                }
            }
                // SAFETY: 旧键值由字典持有。
                // **先查活表** ✓（第 274 轮诊断）：把**键名**带进哨兵 ⇒ 一眼看出是哪个条目 ✓。
                if dangling_mode() {
                    self.assert_live(old_key, &format!("dict_set 旧键（新键 `{key}`）"));
                    self.assert_live(old_value, &format!("dict_set 旧值（新键 `{key}`）"));
                }
                unsafe {
                    self.release_object(old_key.as_ptr());
                    self.release_object(old_value.as_ptr());
                }
            }
        }
        let stored_key = self.new_str(key);
        dict.insert_raw(stored_key, value);
    }

    /// 往 `dict` 里按**整数**键写一个值（**接管** `value`；`errorcode` 这类用）。
    pub fn dict_set_int(&self, mapping: NonNull<Header>, key: i64, value: NonNull<Header>) {
        self.zombie_probe(mapping);
        // **借用口径同 [`Self::dict_set`]** ✓（第 275 轮 ✓）。
        unsafe { self.incref_object(value.as_ptr()) };
        // SAFETY: 调用方保证 mapping 是本实例里存活的 dict。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let probe = self.new_int(key);
        let position = dict
            .entries()
            .iter()
            .position(|(existing, _)| crate::executor::values::values_equal_public(self, *existing, probe));
        // SAFETY: probe 是新引用，比较完就归还。
        unsafe { self.release_object(probe.as_ptr()) };
        if let Some(position) = position {
            if let Some((old_key, old_value)) = dict.remove(position) {
                // SAFETY: 旧键值由字典持有。
                unsafe {
                    self.release_object(old_key.as_ptr());
                    self.release_object(old_value.as_ptr());
                }
            }
        }
        let stored_key = self.new_int(key);
        dict.insert_raw(stored_key, value);
    }

    /// 按**字符串**键读 `dict`（**借用**；不存在给 `None`）。
    pub fn dict_get(&self, mapping: NonNull<Header>, key: &str) -> Option<NonNull<Header>> {
        // SAFETY: 调用方保证 mapping 存活。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let probe = self.new_str(key);
        let found = dict
            .entries()
            .iter()
            .position(|(existing, _)| crate::executor::values::values_equal_public(self, *existing, probe));
        // SAFETY: probe 是新引用，比较完就归还。
        unsafe { self.release_object(probe.as_ptr()) };
        found.and_then(|position| dict.entry(position).map(|(_, value)| value))
    }

    /// **`DESIGN.md` §9 第 20 条**：注入平台相关**只读常量**（`errno` 一类）。
    ///
    /// 由 `pyawa-runtime` 在启动时调用；**按名字**查、数字随平台（`CM-20`）。
    /// 注入的是一份**拷贝**，并按名字排序以便二分查找；**不新增能力域**（`REQUIREMENTS.md`）。
    pub fn set_platform_constants(&self, constants: &[(&'static str, i64)]) {
        let mut table: Vec<(&'static str, i64)> = constants.to_vec();
        table.sort_unstable_by_key(|(name, _)| *name);
        *self.platform_constants.borrow_mut() = table;
    }

    /// 设置位数上限（**只存值**；"0 或 ≥ 阈值"的规则由 `sys.set_int_max_str_digits` 把关，
    /// 与参照一致——那条规则报的是 `ValueError`，属脚本可见语义）。
    pub fn set_int_max_str_digits(&self, value: u32) {
        self.int_max_str_digits.set(value);
    }

    /// 记下最近一次抛出的异常（**新引用**，由实例接手；旧的那份交出去由调用方释放）。
    pub fn set_pending_exception(
        &self,
        exception: Option<NonNull<Header>>,
    ) -> Option<NonNull<Header>> {
        self.pending_exception.replace(exception)
    }

    /// **OM-3**：本实例当前占用的字节数（能力接口不承担预算，见 `CP-8`）。
    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated.get()
    }

    /// **OM-26**：设置阈值三元组。`t0` **0 会被拒绝**——那等于每次分配都回收；
    /// `t1`／`t2` 只存不生效（单代，**临时**）。
    pub fn set_gc_threshold(&self, threshold: (usize, usize, usize)) {
        assert!(threshold.0 > 0, "OM-26：阈值必须可配置且不为 0");
        self.gc_threshold.set(threshold);
        // **配额计数一并归零**（第 309 轮）：阈值是"**从设定那一刻**起再过多少次分配就回收" ✓ ——
        // 归零前，引导期（含本轮给 `object` 挂 `__hash__` 那两个原生对象 ✓）的分配会把计数顶到
        // 阈值之上 ✗ ⇒ 判据一旦设定就**立刻**满足 ⇒ 自回收的时机变得不可预期 ✗
        // （`object_model.rs` 的 `auto_collection_triggers_at_threshold` 当场变红 ✓）。
        self.gc_alloc_count.set(0);
    }
}
