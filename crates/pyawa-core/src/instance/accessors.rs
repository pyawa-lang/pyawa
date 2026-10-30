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


    /// **最小 `eval`**（第 616 轮 ✓）：把源码**当表达式**编译、在给定命名空间里求值 ✓、回值 ✓。
    ///
    /// 为什么这么写 ✗：`eval` 拿不到"调用者的帧"（`sys._getframe(1)` 我们只接了 depth 0 ✓）⇒ 只能支持
    /// **`eval(源码, 命名空间)`** 这一形态 ✓（自带 globals ✓）。`functools` 的 `_CacheInfo` 正是这么调的 ✓
    ///（实测：`eval("lambda _cls, hits, misses, maxsize, currsize: _tuple_new(_cls, (hits, misses, maxsize, currsize))", ns)` ✓）。
    pub fn eval_source(
        &self,
        source: &str,
        namespace: NonNull<Header>,
    ) -> Result<NonNull<Header>, ExecError> {
        // 包装成"赋值一个内部名字" ⇒ 既能编译**表达式** ✓、又能把结果留在命名空间里取回 ✓。
        let wrapped = format!("__pyawa_eval_result__ = (\n{source}\n)\n");
        let unit = crate::compile::compile(
            &wrapped,
            "<eval>",
            crate::compile::Mode::PurePython,
            crate::compile::CheckTier::Shallow,
            0,
        )
        .map_err(|error| {
            self.raise_builtin_error("SyntaxError", &format!("eval: {error:?}"))
        })?;
        let code = crate::compile::instantiate(self, &unit);
        let frame_type = self.type_named("frame").ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "eval：`frame` 类型未登记",
        })?;
        // 帧要接手的是一份**新引用** ✓ ⇒ 给调用方那份借用再加一份 ✓（`for_code_with_namespace` 的口径照
        // `executor/import.rs` ✓）。
        // SAFETY: namespace 由调用方保证存活。
        unsafe { self.incref_object(namespace.as_ptr()) };
        let frame = crate::frame::Frame::for_code_with_namespace(frame_type, &code, namespace);
        let frame = self.alloc(frame);
        crate::execute(self, &frame)?;
        self.dict_get(namespace, "__pyawa_eval_result__")
            .ok_or_else(|| self.raise_builtin_error("RuntimeError", "eval: 没有拿到结果"))
    }

    /// **`exec(源码, globals, locals)`** ✓（task-20 第 3 轮 ✓）：`Lib/dataclasses.py` 的
    /// `_create_fn` 一族就是「拼源码 ⇒ `exec` 跑 ⇒ 从命名空间取名」✓ —— 与 [`Self::eval_source`]
    /// **同一处真相**（都走 `compile` ⇒ `instantiate` ⇒ `Frame::for_code_with_namespace` ⇒ `execute` ✓）。
    ///
    /// `locals` 是**命名空间**（帧接手一份新引用 ✓）；`globals` 给了且与 `locals` 不同就当作
    /// **全局查找表** ✓（`frame.set_globals` 也吃新引用 ✓）。回 `None`（新引用 ✓）。
    pub fn exec_source(
        &self,
        source: &str,
        globals: Option<NonNull<Header>>,
        locals: NonNull<Header>,
    ) -> Result<NonNull<Header>, ExecError> {
        let unit = crate::compile::compile(
            source,
            "<string>",
            crate::compile::Mode::PurePython,
            crate::compile::CheckTier::Shallow,
            0,
        )
        .map_err(|error| {
            self.raise_builtin_error("SyntaxError", &format!("exec: {error:?}"))
        })?;
        let code = crate::compile::instantiate(self, &unit);
        let frame_type = self.type_named("frame").ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "exec：`frame` 类型未登记",
        })?;
        // 帧接手**新引用** ✓（`for_code_with_namespace` 的口径 ✓）。
        // SAFETY: locals 由调用方保证存活。
        unsafe { self.incref_object(locals.as_ptr()) };
        let frame = crate::frame::Frame::for_code_with_namespace(frame_type, &code, locals);
        if let Some(globals) = globals {
            if globals != locals {
                // SAFETY: globals 由调用方保证存活；`set_globals` 吃新引用 ✓。
                unsafe { self.incref_object(globals.as_ptr()) };
                frame.set_globals(globals);
            }
        }
        let frame = self.alloc(frame);
        crate::execute(self, &frame)?;
        let none = self.singletons().none();
        // SAFETY: none 是单例，这里交出新的一份。
        unsafe { self.incref_object(none.as_ptr()) };
        Ok(none)
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
        // **`int` 的子类也算整数** ✓（第 610 轮 ✓）：`class N(int)` 的实例载荷布局同 `int` ✓；
        // 先前只认"类型恰好是 `int`" ✗ ⇒ 子类实例（`type(n) is N` ✓）一造出来就"不再是整数" ✗。
        if self.is_subtype(ty, self.singletons().int_type()) {
            // SAFETY: 布局由基类 `int` 决定，子类同布局。
            return Some(unsafe { &*object.as_ptr().cast::<IntObject>() }.value.clone());
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
        //
        // **P3-38 S2a 唯一允许的行为改动点**：先前直接 `value()`（对 WTF-8 表示会 panic）⇒
        // 现在走 `as_utf8()`：WTF-8 表示（含孤立代理）**如实给 `None`** ✓。合法 UTF-8 路径的
        // 结果**一个字节都不变** ✓（S2a 的生产路径也不会构造 WTF-8 表示 ⇒ 实际无可观测变化）。
        unsafe { &*object.as_ptr().cast::<crate::StrObject>() }.as_utf8()
    }

    /// **`str` 对象的 WTF-8 字节**（P3-38 S2a；不是 `str` ⇒ `None`）。
    ///
    /// 与 [`Instance::text_of`] 的分工：`text_of` 是"合法 UTF-8 视图"（WTF-8 ⇒ `None`），
    /// 本函数**总是**给字节（UTF-8 表示的字节就是合法 WTF-8 ✓）——S2b 起语义层用它 ✓。
    pub fn wtf8_of(&self, object: NonNull<Header>) -> Option<&[u8]> {
        // **`str` 的子类沿用 str 布局**（`AB-58` 族）⇒ 子类实例也给字节 ✓（S3a：`str.encode`
        // 对 `class S(str)` 同样成立；先前只认精确 `str` ✗）。
        if !self.is_str_object(object) {
            return None;
        }
        // SAFETY: 类型身份刚确认是 `str`（或其子类，布局同 str）✓。
        Some(unsafe { &*object.as_ptr().cast::<crate::StrObject>() }.wtf8_bytes())
    }

    /// `object` 是不是 **`str` 或其子类**（公开版；`str` 的值语义对子类同样成立 ✓）。
    pub fn is_str_object(&self, object: NonNull<Header>) -> bool {
        self.is_str_like(object)
    }

    /// **`sys.getsizeof` 的底层口径**（P3-38 S3e-3，task-67）：**本实现**的对象占用字节数
    /// ＝ 类型载荷尺寸 ＋ 容器的指针数组／字符串字节（**不含**元素对象自身；与 CPython 的
    /// 具体数字不同，这是实现自定量，不冒充 CPython 数值 ✓）。
    ///
    /// `dict` 的估算含每项 16 字节（键值指针）＋ 32 字节表头；`str` 含结尾 NUL 一字节。
    pub fn object_sizeof(&self, object: NonNull<Header>) -> usize {
        let ty = self.type_of(object);
        // SAFETY: 类型身份由 `type_of` 给出，注册表持有。
        let base = unsafe { ty.as_ref() }.instance_size();
        let extra = match self.type_name(ty).as_str() {
            "str" => self.wtf8_of(object).map(|bytes| bytes.len() + 1).unwrap_or(0),
            "bytes" => self.bytes_value(object).map(<[u8]>::len).unwrap_or(0),
            "list" => 8 * self.list_items(object).map(|items| items.len()).unwrap_or(0),
            "tuple" => 8 * self.tuple_items(object).map(|items| items.len()).unwrap_or(0),
            "dict" => self
                .dict_entries(object)
                .map(|entries| 32 + 16 * entries.len())
                .unwrap_or(0),
            "set" | "frozenset" => 8 * self.set_items(object).map(|items| items.len()).unwrap_or(0),
            _ => 0,
        };
        base + extra
    }

    /// **`str` 的码点表**（P3-38 S2a-2 批 2）：UTF-8 ⇒ `chars()`；WTF-8 ⇒ 逐码点解码
    /// （孤立代理各算一个码点 ✓）；不是 `str` ⇒ `None`。
    pub fn str_code_point_vec(&self, object: NonNull<Header>) -> Option<Vec<u32>> {
        if !self.is_str_like(object) {
            return None;
        }
        // SAFETY: 类型身份已确认是 `str`（载荷同布局）。
        let object = unsafe { &*object.as_ptr().cast::<crate::StrObject>() };
        match object.as_utf8() {
            Some(text) => Some(text.chars().map(u32::from).collect()),
            None => {
                let mut points = Vec::new();
                for point in crate::wtf8::code_points(object.wtf8_bytes()) {
                    points.push(point.ok()?);
                }
                Some(points)
            }
        }
    }

    /// **`str` 的第 `index` 个码点**（0 起；越界／非 str ⇒ `None`）。
    pub fn str_code_point_at(&self, object: NonNull<Header>, index: usize) -> Option<u32> {
        match self.str_code_point_vec(object) {
            Some(points) => points.get(index).copied(),
            None => None,
        }
    }

    /// 用**一串码点**造一个 `str`（新引用；P3-38 S2a-2 批 2 的"单字符／切片结果"统一入口 ✓）。
    ///
    /// 码点必须来自已校验的 WTF-8（本层内部保证 ✓）——编码失败属内部不变量被破坏，如实当场炸。
    /// 结果走 [`Instance::new_str_wtf8`] ⇒ 合法 UTF-8 仍存 UTF-8 表示（合法字符串零行为改动 ✓）。
    pub fn new_str_code_points(&self, points: &[u32]) -> NonNull<Header> {
        let mut bytes = Vec::with_capacity(points.len());
        for &point in points {
            crate::wtf8::encode_code_point(point, &mut bytes)
                .expect("码点必须来自已校验的 WTF-8");
        }
        self.new_str_wtf8(&bytes)
            .expect("由合法码点拼出的 WTF-8 必合法")
    }

    /// **单个码点 ⇒ 单字符 `str`**（新引用）：标量走 UTF-8 表示、孤立代理走 WTF-8 表示 ✓。
    pub fn new_str_code_point(&self, point: u32) -> NonNull<Header> {
        self.new_str_code_points(&[point])
    }

    /// **`str` 的字节层重复**（`s * n` 的载荷一半；P3-38 S2a-2 批 2）。
    ///
    /// 在 **WTF-8 字节层**重复 ✓（合法 UTF-8 结果存 Utf8、旧行为不变；含孤立代理的串也能重复 ✓）；
    /// 不是 `str` ⇒ `None`。**上限检查**（`MemoryError`）由调用方做（它要能抛异常 ✓）。
    pub fn str_repeat(&self, object: NonNull<Header>, count: usize) -> Option<NonNull<Header>> {
        let bytes = self.str_wtf8_bytes(object)?;
        let mut repeated = Vec::with_capacity(bytes.len().saturating_mul(count));
        for _ in 0..count {
            repeated.extend_from_slice(bytes);
        }
        self.new_str_wtf8(&repeated)
    }

    /// **子串判定**（`in` 的安全口径，P3-38 S2a-2 批 2）：两边都是 `str` ⇒ 按 **WTF-8 字节**
    /// 子串匹配 ✓（`needle` 为空 ⇒ `true` ✓）。
    ///
    /// **为什么字节子串对 WTF-8 安全**：WTF-8 是**自同步**编码——首字节（ASCII 或 `>=0xC2`）不会
    /// 出现在任何多字节序列的续字节位上（续字节恒 `0x80..=0xBF`），且代理的 3 字节形式也是自同步的
    /// ⇒ 字节子串匹配 ⟺ **码点子序列**匹配 ✓（不会在码点中间"错位命中"）。
    /// 不是 `str` ⇒ `None`（由调用方报参照的 TypeError ✓）。
    pub fn str_contains(
        &self,
        container: NonNull<Header>,
        needle: NonNull<Header>,
    ) -> Option<bool> {
        if !self.is_str_like(container) || !self.is_str_like(needle) {
            return None;
        }
        // SAFETY: 两边类型身份都已确认是 `str`（载荷同布局）。
        let haystack = unsafe { &*container.as_ptr().cast::<crate::StrObject>() }.wtf8_bytes();
        let needle = unsafe { &*needle.as_ptr().cast::<crate::StrObject>() }.wtf8_bytes();
        if needle.is_empty() {
            return Some(true);
        }
        Some(haystack.windows(needle.len()).any(|window| window == needle))
    }

    /// `object` 是不是 **`str` 或其子类**（`str` 的值语义对子类同样成立 ✓）。
    fn is_str_like(&self, object: NonNull<Header>) -> bool {
        let ty = self.type_of(object);
        self.type_named("str")
            .is_some_and(|base| ty == base || self.is_subtype(ty, base))
    }

    /// **`str` 的 WTF-8 字节**（P3-38 S2a-2；`wtf8_of` 的语义化别名 ✓）。
    pub fn str_wtf8_bytes(&self, object: NonNull<Header>) -> Option<&[u8]> {
        self.wtf8_of(object)
    }

    /// **`str` 的码点长度**（`len()` 的口径：**代理各算 1 个码点** ✓；不是 `str` ⇒ `None`）。
    ///
    /// UTF-8 表示 ⇒ `chars().count()`（与旧口径一致 ✓）；WTF-8 表示 ⇒ 逐码点计数（含孤立代理 ✓）。
    pub fn str_code_point_len(&self, object: NonNull<Header>) -> Option<usize> {
        if !self.is_str_like(object) {
            return None;
        }
        // SAFETY: 类型身份已确认是 `str`（或其子类，载荷同布局）。
        let object = unsafe { &*object.as_ptr().cast::<crate::StrObject>() };
        match object.as_utf8() {
            Some(text) => Some(text.chars().count()),
            None => {
                let mut count = 0usize;
                for point in crate::wtf8::code_points(object.wtf8_bytes()) {
                    // 已校验的 WTF-8 ⇒ 不该出现 Err；真出现就如实 `None`（不吞、不猜 ✓）。
                    point.ok()?;
                    count += 1;
                }
                Some(count)
            }
        }
    }

    /// **两个 `str` 的相等**（`==` 的值语义）：按 **WTF-8 字节**逐字节相等 ✓
    /// （合法 UTF-8 的字节相等与旧口径一致 ✓；含代理的串也能正确比较 ✓）。
    /// 任一不是 `str` ⇒ `None`（由调用方走别的分支 ✓）。
    pub fn str_equals(&self, left: NonNull<Header>, right: NonNull<Header>) -> Option<bool> {
        if !self.is_str_like(left) || !self.is_str_like(right) {
            return None;
        }
        // SAFETY: 两个类型身份都已确认是 `str`（载荷同布局）。
        let left_bytes = unsafe { &*left.as_ptr().cast::<crate::StrObject>() }.wtf8_bytes();
        let right_bytes = unsafe { &*right.as_ptr().cast::<crate::StrObject>() }.wtf8_bytes();
        Some(left_bytes == right_bytes)
    }

    /// **`str` 是否为空**（真值口径）：两分支都看字节/字符是否为空 ✓；不是 `str` ⇒ `None`。
    pub fn str_is_empty(&self, object: NonNull<Header>) -> Option<bool> {
        if !self.is_str_like(object) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<crate::StrObject>() }.is_empty())
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
        // 类对象：`str(cls)` 与 `repr(cls)` 同形 ✓（参照实测 ✓）。
        if self.is_type_object(object) {
            return Ok(crate::executor::protocol::class_object_repr(self, object));
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
        // **类对象**：`<class '模块.限定名'>`（第 410 轮，task-21 G4 ✓）——先前落到默认
        // `<X object at 0x…>` ✗（带自定义元类的类被 `repr` 时更糟：会误调**类自己的**
        // `__repr__` 实例方法 ⇒ `Enum` 一族当场炸 ✓，详见 `protocol::call_special_method`）。
        if self.is_type_object(object) {
            return Ok(crate::executor::protocol::class_object_repr(self, object));
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
        // 默认形式：`<模块.类名 object at 0x…>`（第 599 轮 ✓）：参照给**模块限定** ✓
        //（`<__main__.D object at 0x…>` ✓）；内建类型**不带**前缀 ✓ ⇒ `__module__` 缺失或为
        // `builtins` 时只用类名 ✓（原先这里写的是"模块／qualname 随类创建钩子接线后补" ✗，本轮补上模块那半 ✓）。
        // SAFETY: 同上。
        let name = unsafe { ty.as_ref() }.name().to_owned();
        let qualified = match self
            .type_lookup(ty, "__module__")
            .and_then(|value| self.text_of(value).map(|text| text.to_owned()))
        {
            Some(module) if module != "builtins" => format!("{module}.{name}"),
            _ => name,
        };
        Ok(format!("<{qualified} object at {:p}>", object.as_ptr()))
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

impl Instance {
    /// **`__annotations__` 的 3.14 口径** ✓（task-20 起 ✓，task-21 补 PEP 649 ✓）：
    /// - **自有 `__annotations__`** ⇒ 直接给 ✓（用户显式赋过 ✓）；
    /// - **有 `__annotate_func__`**（类体的惰性注解 ✓）⇒ 调 `__annotate__(1)`（`Format.VALUE` ✓）
    ///   取回字典 ✓、**不缓存** ✓（3.14 实测 `'__annotations__' in C.__dict__` 仍为 `False` ✓）；
    /// - **类对象**没有注解 ⇒ 给**新的空 `dict`** ✓、**不缓存** ✓（同 3.14 ✓）；
    /// - **模块对象**没有 ⇒ 建空 `dict` **并缓存**进模块字典 ✓（3.14 实测模块是缓存的 ✓）。
    ///
    /// 回**新引用** ✓（`Owned`／原生返回值的契约 ✓）。
    pub fn annotations_of(&self, target: NonNull<Header>) -> Option<NonNull<Header>> {
        let is_module = self
            .type_named("module")
            .is_some_and(|module| self.is_subtype(self.type_of(target), module));
        let namespace = if self.is_type_object(target) {
            self.type_namespace(target)
        } else if is_module {
            crate::mounted_instance_dict(self, target)
        } else {
            None
        }?;
        if let Some(existing) = self.dict_get(namespace, "__annotations__") {
            // SAFETY: existing 由命名空间持有，存活；这里交出新的一份。
            unsafe { self.incref_object(existing.as_ptr()) };
            return Some(existing);
        }
        // **类对象的缓存槽 `__annotations_cache__`** ✓（3.14 实测 ✓）：`C.__annotations__ is C.__annotations__`
        // 为真 ✓，而 `C.__dict__['__annotations__']` 仍 `KeyError` ✓（缓存**不**写进 `__annotations__` ✓）。
        if !is_module {
            if let Some(cached) = self.dict_get(namespace, "__annotations_cache__") {
                // SAFETY: cached 由命名空间持有，存活；这里交出新的一份。
                unsafe { self.incref_object(cached.as_ptr()) };
                return Some(cached);
            }
        }
        // **PEP 649 惰性注解** ✓：类体把注解函数存在 `__annotate_func__` ✓ ⇒ 调 `(1)` ✓。
        let computed = if let Some(annotate) = self.dict_get(namespace, "__annotate_func__") {
            let one = self.new_int(1);
            let outcome = crate::executor::call::call_value(self, annotate, &[one], &[]);
            self.release(one);
            match outcome {
                Ok(value) => value,
                Err(_) => return None,
            }
        } else {
            self.new_dict()
        };
        // **模块**缓存进 `__annotations__` ✓；**类对象**缓存进 `__annotations_cache__` ✓（3.14 实测 ✓）。
        if is_module {
            self.dict_set(namespace, "__annotations__", computed);
        } else {
            self.dict_set(namespace, "__annotations_cache__", computed);
        }
        Some(computed)
    }
}
