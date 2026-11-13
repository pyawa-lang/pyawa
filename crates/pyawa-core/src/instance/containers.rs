//! `Instance` 的容器访问器域（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use crate::HotId;
use super::*;

impl Instance {
    /// 元组的元素（**借用视图**；不是元组给 `None`）。
    pub fn tuple_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        if !self.hot_type(HotId::Tuple).is_some_and(|base| self.is_subtype(self.type_of(object), base)) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        let tuple = unsafe { &*object.as_ptr().cast::<TupleObject>() };
        Some((0..tuple.len()).map(|index| tuple.item(index).expect("下标在范围内")).collect())
    }

    /// 摊开一个 `list` 的元素（**借用**一份拷贝；不是 `list` 给 `None`）。
    pub fn list_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        if !self.hot_type(HotId::List).is_some_and(|base| self.is_subtype(self.type_of(object), base)) {
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
        if !self.hot_type(HotId::Dict).is_some_and(|base| self.is_subtype(self.type_of(object), base)) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<DictObject>() }.entries())
    }

    /// 摊开一个 `set` 的元素。
    pub fn set_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        // **`frozenset` 也算** ✓（第 236 轮）。
        let ty = self.type_of(object);
        if !self.hot_subtype_of_any(ty, &[HotId::Set, HotId::FrozenSet]) {
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
    /// **`len()` 的实例协议** ✓（第 626 轮）：内建那几种（`str`／`bytes`／`dict`／`list`… ✓）走
    /// [`length_of`](Self::length_of) ✓；其余按参照**只在类型上**查 `__len__` ✓（特殊方法不查实例字典 ✓）
    /// 并调它 ✓ —— 这一格先前**整块没有** ✗ ⇒ `Lib/re/_parser.py:164` 的 `SubPattern.__len__` 不被认 ✓
    /// ⇒ `import re` 报 `TypeError: object of type 'SubPattern' has no len()` ✗（本轮实测 ✓）。
    /// 返回非整数 ⇒ 照参照报 `TypeError: '<类型>' object cannot be interpreted as an integer` ✓；
    /// 负数 ⇒ `ValueError: __len__() should return >= 0` ✓。
    pub fn length_with_protocol(
        &self,
        object: NonNull<Header>,
    ) -> Result<Option<usize>, ExecError> {
        if let Some(length) = self.length_of(object) {
            return Ok(Some(length));
        }
        // **走属性通道** ✓（第 627 轮修 ✗）：`__len__` 可能由类型的 **`getattr` 槽**动态给出 ✓
        //（`range` 就是 ✓）—— 只查类型字典 ✗ 会漏掉它 ⇒ `len(range(5))` 报
        // `object of type 'range' has no len()` ✗（本轮实测 ✓）。这与 `__call__`／切片那条路一致 ✓。
        let (method, this) = match crate::executor::attribute_lookup(self, object, "__len__") {
            Ok(crate::executor::Attribute::Method { function, this }) => (function, Some(this)),
            Ok(crate::executor::Attribute::Value(value))
            | Ok(crate::executor::Attribute::Owned(value)) => (value, Some(object)),
            Err(error) => {
                // **探针丢异常要还引用**（round 198 真泄漏修 ✗）。
                crate::executor::runtime::discard_error(self, &error);
                return Ok(None);
            }
        };
        let result = crate::executor::call::call_callable(
            self,
            method,
            this,
            Vec::new(),
            Vec::new(),
            0,
        )?;
        let index = self.index_value(result);
        // SAFETY: result 是刚调用得到的新引用，这里消费掉。
        unsafe { self.release_object(result.as_ptr()) };
        let Some(value) = index? else {
            let message = format!(
                "'{}' object cannot be interpreted as an integer",
                self.type_name(self.type_of(result))
            );
            return Err(self.raise_builtin_error("TypeError", &message));
        };
        if value < 0 {
            return Err(self.raise_builtin_error("ValueError", "__len__() should return >= 0"));
        }
        Ok(Some(value as usize))
    }

    pub fn length_of(&self, object: NonNull<Header>) -> Option<usize> {
        let ty = self.type_of(object);
        if self.hot_type(HotId::Str)
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // **按码点数** ✓（task-19 第 2 轮真 bug ✗ 修的就是"UTF-8 字节数"；P3-38 S2a-2 批 2 起
            // 统一走 `str_code_point_len`：UTF-8 表示等价于 `chars().count()`，WTF-8 表示的
            // **孤立代理也各算 1** ✓，且不再经 `value()`（那对 WTF-8 表示会守卫 panic）✓。
            return self.str_code_point_len(object);
        }
        if self.hot_type(HotId::ByteArray).is_some_and(|base| {
            self.type_of(object) == base || self.is_subtype(self.type_of(object), base)
        }) {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<crate::builtin_objects::BytearrayObject>() }
                .value()
                .len());
        }
        if self.hot_type(HotId::Bytes)
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<BytesObject>() }.value().len());
        }
        if self.hot_type(HotId::Deque)
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<crate::builtin_objects::DequeObject>() }.len());
        }
        // **按子类型判**（第 99 轮真 bug 修 ✗）：先前是**精确类型**比较 ✗ ⇒ `class D(dict)` 这种
        // **内建类型的子类**一律落空 ✓（两行复现：`class D(dict)` ⇒ `len(D())` 报
        // `TypeError: object of type 'D' has no len()` ✗，参照给 0 ✓）。
        if self.hot_type(HotId::Dict)
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<DictObject>() }.entries().len());
        }
        // **`set`／`frozenset` 按自己的载荷读** ✓（第 236 轮顺手修 ✗）：先前这里把 `set` **当 `DictObject`** 读 ✗
        // ⇒ 长度靠"两种载荷碰巧同布局"歪打正着 ✓；现在明写 ✓。
        if self.hot_subtype_of_any(ty, &[HotId::Set, HotId::FrozenSet]) {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<SetObject>() }.items().len());
        }
        if self.hot_type(HotId::List)
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<ListObject>() }.len());
        }
        if self.hot_type(HotId::Tuple)
            .is_some_and(|base| self.is_subtype(ty, base))
        {
            // SAFETY: 同上。
            return Some(unsafe { &*object.as_ptr().cast::<TupleObject>() }.len());
        }
        None
    }

    /// **`bytes` 的载荷**（**借用**；不是 `bytes` 给 `None`）。
    pub fn bytes_value(&self, object: NonNull<Header>) -> Option<&[u8]> {
        if Some(self.type_of(object)) == self.hot_type(HotId::Bytes) {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<BytesObject>() }.value());
        }
        None
    }

    /// **`bytes`／`bytearray`（含子类）的载荷**（round 100；借用 ✓）。
    ///
    /// 动因：比较族与等值族都只认精确 `bytes` ✗ ⇒ `bytearray(b"a") < bytearray(b"b")` 报
    /// `TypeError`、`bytearray(b"ab") == b"ab"` 为 False ✗（参照都按**字节内容** ✓）。
    pub fn bytes_like_value(&self, object: NonNull<Header>) -> Option<Vec<u8>> {
        // **零分配的类型判据**（round 100 性能修 ✗）：热路径是 `==`／`<`，不能每次造 `String`
        // （`type_name` 会 ✓）。用类型指针＋`is_subtype`（MRO 指针扫 ✓）判 `bytes`／`bytearray` 族 ✓。
        let ty = unsafe { object.as_ref() }.ty();
        let bytes_type = self.hot_type(HotId::Bytes)?;
        let bytearray_type = self.hot_type(HotId::ByteArray)?;
        let family = |candidate: NonNull<crate::TypeObject>| {
            candidate == bytes_type
                || candidate == bytearray_type
                || self.is_subtype(candidate, bytes_type)
                || self.is_subtype(candidate, bytearray_type)
        };
        if !family(ty) {
            return None;
        }
        if ty == bytes_type || self.is_subtype(ty, bytes_type) {
            // SAFETY: 类型身份已确认（`bytes` 与子类布局同形 ✓）。
            return Some(unsafe { &*object.as_ptr().cast::<BytesObject>() }.value().to_vec());
        }
        // SAFETY: 类型身份已确认（`bytearray` 与子类布局同形 ✓）。
        Some(
            unsafe { &*object.as_ptr().cast::<crate::builtin_objects::BytearrayObject>() }
                .value()
                .to_vec(),
        )
    }

    /// **往 `bytearray` 载荷里写一段**（`_struct.pack_into` 用 ✓；第 725 轮）。
    ///
    /// 返回 `false` ＝ **不是 `bytearray`** ✓ 或**越界** ✓ —— 调用方据此如实报
    /// `TypeError`／`ValueError` ✓（与参照的"只收可写 buffer"口径同向 ✓）。
    pub fn write_bytearray_at(&self, object: NonNull<Header>, offset: usize, bytes: &[u8]) -> bool {
        if Some(self.type_of(object)) != self.hot_type(HotId::ByteArray) {
            return false;
        }
        // SAFETY: 类型身份刚确认（`bytearray` 的载荷就是 `BytearrayObject` ✓）。
        let payload = unsafe {
            &*object.as_ptr().cast::<crate::builtin_objects::BytearrayObject>()
        };
        let mut value = payload.value();
        if offset + bytes.len() > value.len() {
            return false;
        }
        value[offset..offset + bytes.len()].copy_from_slice(bytes);
        true
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
        // **按哈希索引查**（`P3-41` round 42）：`hash_text(key)` 与 `key_hash` 同族 ⇒ 命中直接
        // `replace_value`（**保住插入顺序** ✓，参照也如此；先前 remove 再 insert 会把键挪到末尾 ✗）。
        let hash = crate::executor::values::hash_text(key);
        if let Some(position) = self.dict_lookup_hashed(mapping, hash, |existing| {
            self.text_of(existing) == Some(key)
        }) {
            // SAFETY: 调用方保证 mapping 是本实例里存活的 dict。
            let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
            let old_value = dict.replace_value(position, value).expect("position 刚由索引给出");
            // **同值重存 ＋ 计数只有 1** ✗（第 275 轮）：替换前要判"新值是不是就是旧值" ✓。
            if dangling_mode() && old_value.as_ptr() == value.as_ptr() {
                let header = unsafe { value.as_ref() };
                if !header.is_immortal() && header.refcount() <= 1 {
                    panic!("[同值重存] dict_set(`{key}`) 的旧值与新值同一个对象、计数只有 {} ✗ ⇒ 替换时会被打死 ✓", header.refcount());
                }
            }
            if dangling_mode() {
                self.assert_live(old_value, &format!("dict_set 旧值（新键 `{key}`）"));
            }
            // SAFETY: 旧值由字典持有，替换后交回本函数。
            unsafe { self.release_object(old_value.as_ptr()) };
            return;
        }
        let stored_key = self.new_str(key);
        // SAFETY: 调用方保证 mapping 存活。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        dict.insert_hashed(stored_key, value, hash);
    }

    /// 往 `dict` 里按**整数**键写一个值（**接管** `value`；`errorcode` 这类用）。
    pub fn dict_set_int(&self, mapping: NonNull<Header>, key: i64, value: NonNull<Header>) {
        self.zombie_probe(mapping);
        // **借用口径同 [`Self::dict_set`]** ✓（第 275 轮 ✓）。
        unsafe { self.incref_object(value.as_ptr()) };
        let hash = crate::executor::values::hash_i64(key);
        // 先造一个探针键：命中时它只用于比较（比较完归还）；未命中时**直接当存储键**（省一次分配 ✓）。
        let probe = self.new_int(key);
        if let Some(position) = self.dict_lookup_hashed(mapping, hash, |existing| {
            crate::executor::values_equal_public(self, existing, probe)
        }) {
            // SAFETY: 调用方保证 mapping 是本实例里存活的 dict。
            let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
            // SAFETY: probe 是新引用，比较完即归还。
            unsafe { self.release_object(probe.as_ptr()) };
            if let Some(old_value) = dict.replace_value(position, value) {
                // SAFETY: 旧值由字典持有。
                unsafe { self.release_object(old_value.as_ptr()) };
            }
            return;
        }
        // SAFETY: 调用方保证 mapping 存活。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        dict.insert_hashed(probe, value, hash);
    }

    /// **按"已算好的哈希 ＋ 匹配谓词"查字典**（`P3-41` round 42 的内部口）：命中给条目下标 ✓。
    ///
    /// 索引惰性建（第一问时 O(n)，之后 O(1)；改键的操作会作废它 ✓）。
    /// 按名字定位（**哈希索引** ✓）：`dict_get`／属性读写／全局名查都走它。
    fn dict_position_by_name(
        &self,
        mapping: NonNull<Header>,
        name: &str,
        hash: u64,
    ) -> Option<usize> {
        self.dict_lookup_hashed(mapping, hash, |existing| {
            self.text_of(existing) == Some(name)
        })
    }

    fn dict_lookup_hashed(
        &self,
        mapping: NonNull<Header>,
        hash: u64,
        matches: impl Fn(NonNull<Header>) -> bool,
    ) -> Option<usize> {
        // SAFETY: 调用方保证 mapping 存活。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        dict.ensure_index(|candidate| crate::executor::values::key_hash(self, candidate));
        for position in dict.candidates(hash) {
            if let Some((existing, _)) = dict.entry(position) {
                if matches(existing) {
                    return Some(position);
                }
            }
        }
        None
    }

    /// **按键对象查字典**（`values_equal` 口径 ✓；不可哈希的键退回线性扫 ✓）。
    /// **按对象键删一项**（round 59，给 `_remove_dead_weakref` 用）：找到就删并归还键值引用 ✓。
    /// 返回是否删掉 ✓。
    pub fn dict_remove_key(&self, mapping: NonNull<Header>, key: NonNull<Header>) -> bool {
        // SAFETY: mapping 由调用方保证是本实例的 dict（`dict_entries` 口径 ✓）。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        // **按哈希索引删**（round 256 性能真 bug 修 ✗）：先前这里走 `find_live_position`
        // （**线性扫**）✗ —— `_weakref._remove_dead_weakref` 对**每一个死引用**调一次 ⇒
        // `WeakValueDictionary` 批量失效是 O(n²)（实测 70k 条目的 copy 测试 **154 s** ✗）。
        // 有哈希就查索引（与 `dict_lookup_key` 同口径 ✓），取不到哈希（不可哈希的怪键）才回退线性 ✓。
        let position = match crate::executor::values::key_hash(self, key) {
            Some(hash) => self.dict_lookup_hashed(mapping, hash, |candidate| {
                crate::executor::values::values_equal_public(self, candidate, key)
            }),
            None => dict.find_live_position(|candidate| {
                crate::executor::values::values_equal_public(self, candidate, key)
            }),
        };
        let Some(position) = position else {
            return false;
        };
        // SAFETY: `mapping` 由调用方保证是本实例的 dict（`dict_entries` 刚确认过 ✓）。
        let Some((old_key, old_value)) =
            (unsafe { &*mapping.as_ptr().cast::<DictObject>() }).remove(position)
        else {
            return false;
        };
        // SAFETY: 这两份引用由字典交出。
        unsafe {
            self.release_object(old_key.as_ptr());
            self.release_object(old_value.as_ptr());
        }
        true
    }

    pub fn dict_lookup_key(
        &self,
        mapping: NonNull<Header>,
        key: NonNull<Header>,
    ) -> Option<usize> {
        if let Some(hash) = crate::executor::values::key_hash(self, key) {
            return self.dict_lookup_hashed(mapping, hash, |existing| {
                // **键查找按 `==` 语义**（round 116 真 bug 修 ✗）：先前只比"结构相等" ✗ ⇒
                // 只覆写 `__hash__`／`__eq__` 的用户键（`test_dict.test_str_nonstr` 的 `Key3` ✓）
                // 明明哈希相同却查不到 ✗。先走结构快路（内建标量／字符串 ✓），不中再走**完整
                // `==` 协议**（用户 `__eq__`／反射 ✓）；异常按"不相等"处理（查找面不抛 ✓）。
                if crate::executor::values_equal_public(self, existing, key) {
                    return true;
                }
                crate::executor::protocol::equality_public(self, existing, key).unwrap_or(false)
            });
        }
        // 不可哈希／没覆盖的键 ⇒ 线性兜底（正常路径：设置键时已被拒 ✓）。
        // SAFETY: 调用方保证 mapping 存活。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        dict.find_live_position(|existing| crate::executor::values_equal_public(self, existing, key))
    }

    /// **可失败的 `dict_lookup_key`**（round 120）：键的 `__hash__`／`__eq__` 抛异常时**原样上抛** ✓
    /// （参照口径：`d[x]`／`x in d` 等遇到 `BadEq`／`HashError` 就抛原异常 ✓ ——
    /// `test_dict.test_bad_key`／`test_unhashable_key` ✓）。入口函数用它；内部索引建构仍用吞错版 ✓。
    pub fn dict_lookup_key_checked(
        &self,
        mapping: NonNull<Header>,
        key: NonNull<Header>,
    ) -> Result<Option<usize>, crate::ExecError> {
        match crate::executor::values::key_hash_checked(self, key)? {
            Some(hash) => {
                // 闭包是 `Fn`（不是 `FnMut`）⇒ 用 `RefCell` 记异常 ✓。
                let error: core::cell::RefCell<Option<crate::ExecError>> =
                    core::cell::RefCell::new(None);
                let found = self.dict_lookup_hashed(mapping, hash, |existing| {
                    match crate::executor::protocol::equality_public(self, existing, key) {
                        Ok(same) => same,
                        Err(raised) => {
                            let mut slot = error.borrow_mut();
                            if slot.is_none() {
                                *slot = Some(raised);
                            }
                            false
                        }
                    }
                });
                if let Some(raised) = error.into_inner() {
                    return Err(raised);
                }
                Ok(found)
            }
            None => {
                // 不可哈希 ⇒ 线性兜底（设置键时已被拒 ✓）。
                // SAFETY: 调用方保证 mapping 存活。
                let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                Ok(dict
                    .find_live_position(|existing| crate::executor::values_equal_public(self, existing, key)))
            }
        }
    }

    /// 按**字符串**键读 `dict`（**借用**；不存在给 `None`）。
    pub fn dict_get(&self, mapping: NonNull<Header>, key: &str) -> Option<NonNull<Header>> {
        self.dict_get_hashed(mapping, key, crate::executor::values::hash_text(key))
    }

    /// 按**已算好的哈希**读字符串键（round 250 性能，A2-5）：调用方手里有 `co_names` 的预计算
    /// 哈希时（`LOAD_ATTR`／`LOAD_GLOBAL` 一族）不必再每轮 `hash_text` 一遍 ✓。
    pub fn dict_get_hashed(
        &self,
        mapping: NonNull<Header>,
        key: &str,
        hash: u64,
    ) -> Option<NonNull<Header>> {
        let position = self.dict_position_by_name(mapping, key, hash)?;
        // SAFETY: 调用方保证 mapping 存活。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        dict.entry(position).map(|(_, value)| value)
    }

    /// 按**名字**写 `dict`（**接管** `value` 那份引用；已存在 ⇒ 原位替换并释放旧值 ✓）。
    ///
    /// round 250 性能（A2-5）：先前属性／命名空间写走 `find_live_position` **线性扫**（每次写
    /// 都要遍历整个命名空间 ✗）⇒ 这里走哈希索引；新键用 `insert_hashed`（**增量维护索引** ✓，
    /// 不再 `insert_raw` 把索引整体作废 ✗）。插入顺序语义与旧口径一致（新键在末尾 ✓）。
    pub fn dict_set_owned_by_name(&self, mapping: NonNull<Header>, name: &str, value: NonNull<Header>) {
        self.zombie_probe(mapping);
        // **小字典线性、大字典哈希**（round 250 性能，A2-5）：实例属性字典常只有几个键 ⇒
        // 哈希路径的"建索引＋桶 `Vec` 分配"反而更贵（attr_write 档实测回退 ✗）；命名空间/全局表
        // 很大 ⇒ 哈希索引收益明显 ✓。小字典路径**不算哈希** ✓。
        const SMALL_DICT: usize = 8;
        // SAFETY: 调用方保证 mapping 是本实例的 dict。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        if dict.len() < SMALL_DICT {
            if let Some(position) =
                dict.find_live_position(|existing| self.text_of(existing) == Some(name))
            {
                if let Some(old) = dict.replace_value(position, value) {
                    // SAFETY: 旧值由字典持有，替换后交回本函数。
                    unsafe { self.release_object(old.as_ptr()) };
                }
                return;
            }
            let key = self.new_str(name);
            dict.insert_raw(key, value);
            return;
        }
        let hash = crate::executor::values::hash_text(name);
        if let Some(position) = self.dict_position_by_name(mapping, name, hash) {
            if let Some(old) = dict.replace_value(position, value) {
                // SAFETY: 旧值由字典持有，替换后交回本函数。
                unsafe { self.release_object(old.as_ptr()) };
            }
            return;
        }
        let key = self.new_str(name);
        dict.insert_hashed(key, value, hash);
    }

    /// 按**名字**从 `dict` 里摘一项（哈希索引 ✓；返回被摘下的键值引用，调用方负责释放）。
    pub fn dict_remove_by_name(
        &self,
        mapping: NonNull<Header>,
        name: &str,
    ) -> Option<(NonNull<Header>, NonNull<Header>)> {
        /// 小字典线性找（不建索引、不算哈希 ✓）。
        const SMALL_DICT: usize = 8;
        // SAFETY: 调用方保证 mapping 是本实例的 dict。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let position = if dict.len() < SMALL_DICT {
            dict.find_live_position(|existing| self.text_of(existing) == Some(name))?
        } else {
            self.dict_position_by_name(mapping, name, crate::executor::values::hash_text(name))?
        };
        dict.remove(position)
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

    /// **OM-3 扩**：设置每实例内存配额（字节；`0` ＝ 不限）。
    ///
    /// 语义：**活着的**分配字节数（[`Self::bytes_allocated`]，释放会减）超过 `limit` ⇒ 分配器置
    /// `memory_hit`，执行器在**下一条指令边界**抛**可捕获的** `MemoryError`（不 abort／不 panic ✓）。
    /// 已用量超过新上限时立即置位（下一次指令边界就抛 ✓）。宿主经 ABI `pa_set_memory_limit`
    /// 调用（ABI-adjacent，发布前定 ✓）。
    pub fn set_memory_limit(&self, limit: usize) {
        self.memory_limit.set(limit);
        self.memory_ceiling.set(limit);
        self.memory_hit.set(limit != 0 && self.bytes_allocated.get() > limit);
    }

    /// 当前每实例内存配额（`0` ＝ 不限）。
    pub fn memory_limit(&self) -> usize {
        self.memory_limit.get()
    }

    /// 配额命中标志（分配器置位；取走后清掉 —— 处理块里还能继续跑，直到再次超限 ✓）。
    pub fn take_memory_hit(&self) -> bool {
        self.memory_hit.replace(false)
    }

    /// **分配记账**（`OM-3` 的唯一入口）：累积字节数并按需置配额命中位 ✓
    /// （判据是 `memory_ceiling`：初始＝上限，每次抛错后抬高一档 `slack` ⇒ 处理块能跑 ✓）。
    pub(crate) fn account_allocation(&self, size: usize) {
        let next = self.bytes_allocated.get().saturating_add(size);
        self.bytes_allocated.set(next);
        if self.memory_limit.get() != 0 && next > self.memory_ceiling.get() {
            self.memory_hit.set(true);
        }
    }

    /// **对象载荷字节**（`OM-3` 扩的释放侧口径）：`str`／`bytes` 的载荷按实际字节算；
    /// 其它类型暂按 0（容器指针数组等由对象头大小近似 —— 如实记，后续按需补 ✓）。
    pub(crate) fn object_payload_bytes(
        &self,
        ty: NonNull<crate::TypeObject>,
        object: NonNull<crate::Header>,
    ) -> usize {
        if self.is_subtype(ty, self.singletons().str_type()) {
            // SAFETY: 类型身份刚确认 ⇒ 载荷是 `StrObject`。
            return unsafe { &*object.as_ptr().cast::<crate::builtin_objects::StrObject>() }
                .wtf8_bytes()
                .len();
        }
        if self
            .hot_type(crate::HotId::Bytes)
            .is_some_and(|bytes| self.is_subtype(ty, bytes))
        {
            // SAFETY: 类型身份刚确认 ⇒ 载荷是 `BytesObject`。
            return unsafe { &*object.as_ptr().cast::<crate::builtin_objects::BytesObject>() }
                .value()
                .len();
        }
        0
    }

    /// **释放记账**（`OM-3` 扩）：结构体字节 ＋ 载荷字节一起减回 ✓（`saturating_sub` 防口径漂移
    /// 下的负数 ✗）。
    pub(crate) fn release_accounting(
        &self,
        ty: NonNull<crate::TypeObject>,
        object: NonNull<crate::Header>,
    ) {
        // SAFETY: ty 由注册表持有。
        let size = unsafe { ty.as_ref() }.instance_size;
        let payload = self.object_payload_bytes(ty, object);
        self.bytes_allocated
            .set(self.bytes_allocated.get().saturating_sub(size + payload));
    }

    /// 每次抛 `MemoryError` 时抬高的余量：`max(64 KiB, limit/16)`（宿主占用上界 ≈ `limit+slack`）。
    fn memory_slack(&self) -> usize {
        let limit = self.memory_limit.get();
        limit.div_ceil(16).max(64 * 1024)
    }

    /// 构造一次配额异常并**抬高水位**（处理块能继续分配／清理；继续增长则再次抛 ✓）。
    pub fn memory_budget_error(&self) -> crate::ExecError {
        let ceiling = self
            .bytes_allocated
            .get()
            .saturating_add(self.memory_slack());
        self.memory_ceiling.set(ceiling);
        self.memory_hit.set(false);
        self.raise_builtin_error(
            "MemoryError",
            &format!(
                "超出每实例内存配额（上限 {} 字节；已用 {} 字节）",
                self.memory_limit.get(),
                self.bytes_allocated.get()
            ),
        )
    }

    /// **大块分配前的预算检查**（`OM-3` 扩）：明知要再要 `additional` 字节时先问一声 ⇒ 超限直接
    /// 抛 `MemoryError`（不先分配一大块再抛 ✗）。返回的 `Err` 就是**可捕获的** Python 异常 ✓。
    pub fn ensure_memory_budget(&self, additional: usize) -> Result<(), crate::ExecError> {
        if self.memory_limit.get() != 0
            && self.bytes_allocated.get().saturating_add(additional) > self.memory_ceiling.get()
        {
            return Err(self.memory_budget_error());
        }
        Ok(())
    }

    /// 配额已命中 ⇒ 抛 `MemoryError`（取走命中位；执行器边界与大块分配循环共用 ✓）。
    pub fn enforce_memory_quota(&self) -> Result<(), crate::ExecError> {
        if self.take_memory_hit() {
            return Err(self.memory_budget_error());
        }
        Ok(())
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
        self.gc_next_collect.set(threshold.0);
    }
}
