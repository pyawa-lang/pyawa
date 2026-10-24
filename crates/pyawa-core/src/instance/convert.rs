//! `Instance` 的迭代/取值/转换域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// **迭代协议**的公开入口：把对象摊成一批**新引用**（`min`／`max`／`sorted` 要用）。
    ///
    /// 认：`list`／`tuple`／`str`（逐字符）／`dict`（逐**键**）／`set`。
    /// 不认识就给 `None`——**不猜**：调用方据此报参照实现那条
    /// `TypeError: 'int' object is not iterable`（实测）。
    pub fn iterable_items(&self, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        let owned = |value: NonNull<Header>| {
            // SAFETY: value 由容器持有，存活；调用方要自己那份。
            unsafe { self.incref_object(value.as_ptr()) };
            value
        };
        if self.type_named("list") == Some(ty) {
            // SAFETY: 类型身份已确认。
            let list = unsafe { &*object.as_ptr().cast::<ListObject>() };
            return Some(list.items().into_iter().map(owned).collect());
        }
        if self.type_named("tuple") == Some(ty) {
            // SAFETY: 同上。
            let tuple = unsafe { &*object.as_ptr().cast::<TupleObject>() };
            return Some(tuple.items().iter().copied().map(owned).collect());
        }
        if ty == self.singletons().str_type() {
            // SAFETY: 同上。
            let text = unsafe { &*object.as_ptr().cast::<StrObject>() }
                .value()
                .to_owned();
            return Some(
                text.chars()
                    .map(|character| self.new_str(&character.to_string()))
                    .collect(),
            );
        }
        if ty == self.type_named("dict")? {
            // SAFETY: 同上。
            let dict = unsafe { &*object.as_ptr().cast::<DictObject>() };
            return Some(dict.entries().into_iter().map(|(key, _)| owned(key)).collect());
        }
        // **`frozenset` 与 `set` 同载荷** ✓（第 236 轮）⇒ 迭代这条也一并认 ✓。
        if ty == self.type_named("set")? || Some(ty) == self.type_named("frozenset") {
            // SAFETY: 同上。
            let set = unsafe { &*object.as_ptr().cast::<SetObject>() };
            return Some(set.items().into_iter().map(owned).collect());
        }
        // **迭代器对象**（第 137 轮）：`list(itertools.repeat(5, 3))`／`list(x for x in y)` 这类
        // 都要能消费 ✓ ⇒ 复用执行器那份 `advance`（内建迭代器 ＋ `__next__` 协议 ✓ **一处真相** ✓）；
        // 既不是迭代器也不是可迭代 ⇒ `None`（调用方照常报"不是可迭代" ✓）。
        // **先走迭代协议** ✓（第 627 轮真 bug 修 ✗）：`iter()` 认所有带 `__iter__` 的类型 ✓ ——
        // 包括由类型 **`getattr` 槽**动态给 `__iter__` 的 `range` ✓；先前直接拿
        // `runtime::advance(self, object)` ✗（只认内建迭代器 ✓）⇒ `sum(range(4))` 报
        // `TypeError: 'range' object is not iterable` ✗（本轮 `range_builtin` 对拍实测 ✓）。
        if let Ok(iterator) = crate::executor::iter_value(self, object) {
            let mut items = Vec::new();
            loop {
                match crate::executor::runtime::advance(self, iterator) {
                    Ok(Some(item)) => items.push(item),
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
            // SAFETY: iterator 是本函数刚拿到的**新引用** ✓。
            unsafe { self.release_object(iterator.as_ptr()) };
            return Some(items);
        }
        None
    }

    /// 是不是 `bool`（`True`／`False` 是 `int` 的子类，别的地方要分开判）。
    /// `bool` 的**值**（不是 `bool` 就给 `None`）。
    /// **迭代推进**（第 142 轮）：直接复用执行器那份（`executor::runtime::advance` ✓ **一处真相** ✓）——
    /// 内建 `next()` 要的就是它 ✓。
    pub fn advance_iterator(
        &self,
        object: NonNull<Header>,
    ) -> Result<Option<NonNull<Header>>, ExecError> {
        crate::executor::runtime::advance(self, object)
    }

    /// **取迭代器**（第 142 轮）：直接复用执行器那份（`executor::iter::iter_value` ✓ **一处真相** ✓）——
    /// 内建 `iter()` 要的就是它 ✓（`iter(迭代器) is 它自己` ✓ 由那份实现保证 ✓）。
    pub fn iter_object(&self, object: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
        crate::executor::iter::iter_value(self, object)
    }

    /// 把对象当**类型对象**看（是就给 `Some`，否则 `None`）。
    pub fn as_type(&self, object: NonNull<Header>) -> Option<NonNull<TypeObject>> {
        if !self.is_type_object(object) {
            return None;
        }
        // SAFETY: 对象就是类型对象（类型身份已确认）。
        Some(unsafe { NonNull::new_unchecked(object.as_ptr().cast::<TypeObject>()) })
    }

    pub fn bool_value(&self, object: NonNull<Header>) -> Option<bool> {
        if !self.is_bool(object) {
            return None;
        }
        // SAFETY: 类型身份已确认。
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*object.as_ptr().cast::<BoolObject>() }.value)
    }

    pub fn index_value(&self, object: NonNull<Header>) -> Result<Option<i64>, ExecError> {
        if let Some(value) = self.int_value(object) {
            return Ok(Some(value));
        }
        let method = match crate::executor::attribute::attribute_optional(self, object, "__index__") {
            Ok(Some(method)) => method,
            // 没有这个方法、或取属性出错 ⇒ 如实"不是整数" ✓（由调用方报 TypeError ✓）
            Ok(None) | Err(_) => return Ok(None),
        };
        let result = crate::executor::call::call_value(self, method, &[], &[]);
        // SAFETY: method 是新引用。
        unsafe { self.release_object(method.as_ptr()) };
        match result {
            Ok(value) => {
                let number = self.int_value(value);
                // SAFETY: value 是新引用。
                unsafe { self.release_object(value.as_ptr()) };
                Ok(number)
            }
            Err(_) => Ok(None),
        }
    }

    /// 读浮点载荷（`float` 才算；别的给 `None`）。
    pub fn float_value(&self, object: NonNull<Header>) -> Option<f64> {
        if self.type_of(object) == self
            .type_named("float")
            .expect("float 已登记")
        {
            // SAFETY: 类型身份已确认。
            return Some(unsafe { &*object.as_ptr().cast::<FloatObject>() }.value);
        }
        None
    }

    /// **复数载荷**（task-30）：`(real, imag)`；不是**恰好** `complex` 类型就 `None` ✓。
    ///
    /// 与 [`Self::float_value`]／[`Self::int_of`] 同一口径（恰类型；Python 子类的载荷面另说 ✓）。
    pub fn complex_value(&self, object: NonNull<Header>) -> Option<(f64, f64)> {
        if self.type_of(object) == self.type_named("complex").expect("complex 已登记") {
            // SAFETY: 类型身份已确认 ⇒ 载荷是 `ComplexObject`（`TS-43` 布局自选 ✓）。
            let value = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::ComplexObject>() };
            return Some((value.real(), value.imag()));
        }
        None
    }

    /// 两个值的**序**（`min`／`max`／`sorted` 要用）：数值塔按数比、两个 `str` 按字典序。
    ///
    /// 其余给 `None`——调用方据此报参照实现那条
    /// `TypeError: '<' not supported between instances of 'str' and 'int'`（实测）。
    pub fn order_of(
        &self,
        left: NonNull<Header>,
        right: NonNull<Header>,
    ) -> Option<core::cmp::Ordering> {
        // **整数优先按整数比** ✓（第 632 轮真 bug 修 ✗）：先前一律转 `f64` ✗ ⇒ 大整数（`int_value`
        // 只认小整数 ✓ ⇒ 给 `None` ✗）落进"比不了" ✗ ⇒ `min(1 << 100, 5)` 报
        // `'<' not supported between instances of 'int' and 'int'` ✗（`Lib/re` 就断在这 ✓）；
        // 顺带也免掉"大整数转 f64 丢精度" ✗。用的是现成的 `IntValue::cmp` ✓（`bigint.rs:106` ✓）。
        let integer = |object: NonNull<Header>| -> Option<crate::bigint::IntValue> {
            let ty = self.type_of(object);
            if ty == self.singletons().bool_type()
                || self.is_subtype(ty, self.singletons().int_type())
            {
                return self.int_of(object);
            }
            None
        };
        if let (Some(left_integer), Some(right_integer)) = (integer(left), integer(right)) {
            return Some(left_integer.cmp(&right_integer));
        }
        let number = |object: NonNull<Header>| -> Option<f64> {
            // SAFETY: object 是存活对象。
            let ty = unsafe { object.as_ref() }.ty();
            if let Some(value) = self.int_value(object) {
                if ty == self.singletons().bool_type() || ty == self.singletons().int_type() {
                    return Some(value as f64);
                }
            }
            if let Some(value) = self.float_value(object) {
                if ty == self.type_named("float")? {
                    return Some(value);
                }
            }
            None
        };
        if let (Some(left_number), Some(right_number)) = (number(left), number(right)) {
            return left_number.partial_cmp(&right_number);
        }
        // **序列字典序** ✓（task-23 ✓）：`tuple` 之间、`list` 之间（各自子类也算同族 ✓）逐元素比 ✓，
        // 首个不等元素定序（递归走本函数 ✓）、一方是前缀 ⇒ 短的更小 ✓。`sorted`／`list.sort` 走的
        // 就是这里 ✓（`Lib/heapq.py` 的 `nsmallest`／`nlargest` 立据 ✓）。**混合**（tuple vs list）
        // 与"元素比不了"都回 `None` ✓ ⇒ 由调用方报 `'<' not supported between instances of …` ✓。
        let left_is_tuple = self.tuple_items(left).is_some();
        let right_is_tuple = self.tuple_items(right).is_some();
        if left_is_tuple == right_is_tuple {
            let left_sequence = if left_is_tuple { self.tuple_items(left) } else { self.list_items(left) };
            let right_sequence = if right_is_tuple { self.tuple_items(right) } else { self.list_items(right) };
            if let (Some(left_items), Some(right_items)) = (left_sequence, right_sequence) {
                for (first, second) in left_items.iter().zip(right_items.iter()) {
                    if crate::executor::values_equal_public(self, *first, *second) {
                        continue;
                    }
                    return self.order_of(*first, *second);
                }
                return Some(left_items.len().cmp(&right_items.len()));
            }
        }
        // SAFETY: 两者都是存活对象。
        let left_type = unsafe { left.as_ref() }.ty();
        let right_type = unsafe { right.as_ref() }.ty();
        let text_type = self.singletons().str_type();
        if left_type == text_type && right_type == text_type {
            // SAFETY: 类型身份已确认。
            let left_text = unsafe { &*left.as_ptr().cast::<StrObject>() }.value().to_owned();
            // SAFETY: 同上。
            let right_text = unsafe { &*right.as_ptr().cast::<StrObject>() }.value().to_owned();
            return Some(left_text.cmp(&right_text));
        }
        // **富比较协议**（task-12 ✓）：内建快速路径都判不出时，照 `COMPARE_OP` 的**同一处真相**
        // （`executor::compare_public`：正向方法 → `NotImplemented` 走反向 → 再判不出报 `TypeError`）
        // 判 `<`／`>` —— `sorted`／`list.sort`／`heapq` 排**用户对象**要靠它 ✓（真 `unittest` 的
        // `functools.cmp_to_key` 把方法名包成 `KeyWrapper` 再 `sorted`，先前就撞在这条上 ✗）。
        // `compare_public` 报错（两边都没有可比方法）⇒ 仍回 `None`：调用方照旧报
        // `'<' not supported between instances of …` ✓（文案不变）。
        if crate::executor::compare_public(self, left, right, "<", 0).unwrap_or(false) {
            return Some(core::cmp::Ordering::Less);
        }
        if crate::executor::compare_public(self, left, right, ">", 0).unwrap_or(false) {
            return Some(core::cmp::Ordering::Greater);
        }
        None
    }

    /// **取属性（可选）**（第 148 轮）：直接复用执行器那条属性通道 ✓（**一处真相** ✓）——
    /// 内建 `getattr`／`hasattr` 要的就是它 ✓。**必须走它** ✗：早先我直接拿对象当 `dict` 查
    /// （`dict_get` ✗ 会把指针强转成 `DictObject` 读 ⇒ **UB** ✓，实测触发 abort ✓）。
    pub fn attribute_optional_of(
        &self,
        object: NonNull<Header>,
        name: &str,
    ) -> Result<Option<NonNull<Header>>, ExecError> {
        crate::executor::attribute::attribute_optional(self, object, name)
    }
}

/// **`cp437` 表**（第 726 轮 ✓）：由参照本机 `bytes(range(256)).decode('cp437')` 生成 ✓
///（`Lib/zipfile` 用它解文件名 ✓；`encodings/cp437.py` 那条路要 namedtuple，见「未修 bug」✓）。
pub(crate) const CP437: [char; 256] = ['\u{0}', '\u{1}', '\u{2}', '\u{3}', '\u{4}', '\u{5}', '\u{6}', '\u{7}', '\u{8}', '\u{9}', '\u{a}', '\u{b}', '\u{c}', '\u{d}', '\u{e}', '\u{f}', '\u{10}', '\u{11}', '\u{12}', '\u{13}', '\u{14}', '\u{15}', '\u{16}', '\u{17}', '\u{18}', '\u{19}', '\u{1a}', '\u{1b}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}', '\u{20}', '\u{21}', '\u{22}', '\u{23}', '\u{24}', '\u{25}', '\u{26}', '\u{27}', '\u{28}', '\u{29}', '\u{2a}', '\u{2b}', '\u{2c}', '\u{2d}', '\u{2e}', '\u{2f}', '\u{30}', '\u{31}', '\u{32}', '\u{33}', '\u{34}', '\u{35}', '\u{36}', '\u{37}', '\u{38}', '\u{39}', '\u{3a}', '\u{3b}', '\u{3c}', '\u{3d}', '\u{3e}', '\u{3f}', '\u{40}', '\u{41}', '\u{42}', '\u{43}', '\u{44}', '\u{45}', '\u{46}', '\u{47}', '\u{48}', '\u{49}', '\u{4a}', '\u{4b}', '\u{4c}', '\u{4d}', '\u{4e}', '\u{4f}', '\u{50}', '\u{51}', '\u{52}', '\u{53}', '\u{54}', '\u{55}', '\u{56}', '\u{57}', '\u{58}', '\u{59}', '\u{5a}', '\u{5b}', '\u{5c}', '\u{5d}', '\u{5e}', '\u{5f}', '\u{60}', '\u{61}', '\u{62}', '\u{63}', '\u{64}', '\u{65}', '\u{66}', '\u{67}', '\u{68}', '\u{69}', '\u{6a}', '\u{6b}', '\u{6c}', '\u{6d}', '\u{6e}', '\u{6f}', '\u{70}', '\u{71}', '\u{72}', '\u{73}', '\u{74}', '\u{75}', '\u{76}', '\u{77}', '\u{78}', '\u{79}', '\u{7a}', '\u{7b}', '\u{7c}', '\u{7d}', '\u{7e}', '\u{7f}', '\u{c7}', '\u{fc}', '\u{e9}', '\u{e2}', '\u{e4}', '\u{e0}', '\u{e5}', '\u{e7}', '\u{ea}', '\u{eb}', '\u{e8}', '\u{ef}', '\u{ee}', '\u{ec}', '\u{c4}', '\u{c5}', '\u{c9}', '\u{e6}', '\u{c6}', '\u{f4}', '\u{f6}', '\u{f2}', '\u{fb}', '\u{f9}', '\u{ff}', '\u{d6}', '\u{dc}', '\u{a2}', '\u{a3}', '\u{a5}', '\u{20a7}', '\u{192}', '\u{e1}', '\u{ed}', '\u{f3}', '\u{fa}', '\u{f1}', '\u{d1}', '\u{aa}', '\u{ba}', '\u{bf}', '\u{2310}', '\u{ac}', '\u{bd}', '\u{bc}', '\u{a1}', '\u{ab}', '\u{bb}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}', '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{255c}', '\u{255b}', '\u{2510}', '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}', '\u{253c}', '\u{255e}', '\u{255f}', '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{2567}', '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256b}', '\u{256a}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{258c}', '\u{2590}', '\u{2580}', '\u{3b1}', '\u{df}', '\u{393}', '\u{3c0}', '\u{3a3}', '\u{3c3}', '\u{b5}', '\u{3c4}', '\u{3a6}', '\u{398}', '\u{3a9}', '\u{3b4}', '\u{221e}', '\u{3c6}', '\u{3b5}', '\u{2229}', '\u{2261}', '\u{b1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{f7}', '\u{2248}', '\u{b0}', '\u{2219}', '\u{b7}', '\u{221a}', '\u{207f}', '\u{b2}', '\u{25a0}', '\u{a0}'];

/// **内置快速路径的编码名归一**（`utf-8`／`ascii`／`latin-1`／`cp437` 四档 —— **一处真相** ✓）。
///
/// 两个旧实现各有一张白名单（`str.encode` 在 `builtin/str.rs`、`bytes.decode` 在这份文件 ✗）——
/// task-9 把它们合到这里：内置四档走**快路径**（下面的 `encode_builtin`／`decode_builtin`），
/// **其余一律委派** `_codecs.lookup`（见 [`Instance::encode_with_encoding_and_errors`]）。
///
/// 别名取参照 `_codecs.lookup` 与两张旧表的**并集**；不在表里的名字交给注册表（`encodings`
/// 一族的别名更全，例如 `us`／`cp65001` 之外的 `ansi_x3.4_1986` 等 ✓）。
pub(crate) fn builtin_encoding(name: &str) -> Option<&'static str> {
    let lowered = name.to_ascii_lowercase().replace(['_', ' '], "-");
    match lowered.as_str() {
        "utf-8" | "utf8" | "u8" | "utf" | "cp65001" => Some("utf-8"),
        "ascii" | "us-ascii" | "646" | "ansi-x3.4-1968" => Some("ascii"),
        "latin-1" | "latin1" | "latin" | "iso-8859-1" | "iso8859-1" | "8859" | "cp819" | "l1" => {
            Some("latin-1")
        }
        "cp437" | "437" | "ibm437" | "csibm437" => Some("cp437"),
        _ => None,
    }
}

/// 错误处理器名字的分型（内置四档的**错误**路径，照参照）：
/// `strict`／`ignore`／`replace` 在调用处各接一条；其余名字——参照的内建三条未接线的报
/// `NotImplementedError`（如实，不伪造输出），**没注册过的**照参照报 `LookupError` ✓。
fn builtin_error_handler_failure(instance: &Instance, codec: &str, handler: &str) -> ExecError {
    if matches!(handler, "xmlcharrefreplace" | "backslashreplace" | "namereplace") {
        instance.raise_builtin_error(
            "NotImplementedError",
            &format!(
                "`{codec}` codec 的 `{handler}` 错误处理器尚未接线（只认 strict／ignore／replace）"
            ),
        )
    } else {
        instance.raise_builtin_error(
            "LookupError",
            &format!("unknown error handler name '{handler}'"),
        )
    }
}

/// 字符字面量（照参照的转义形态 ✓）：≤ `0xFF` 用 `\xNN`、BMP 用 `\uNNNN`、增补面用 `\UNNNNNNNN`。
fn character_literal(codepoint: u32) -> String {
    if codepoint <= 0xFF {
        format!("\\x{codepoint:02x}")
    } else if codepoint <= 0xFFFF {
        format!("\\u{codepoint:04x}")
    } else {
        format!("\\U{codepoint:08x}")
    }
}

/// **内置四档的编码快路径**（不碰注册表；语义与旧 `str_encode_native` 逐条一致 ✓）。
fn encode_builtin(
    instance: &Instance,
    text: &str,
    codec: &'static str,
    errors: &str,
) -> Result<Vec<u8>, ExecError> {
    match codec {
        "utf-8" => Ok(text.as_bytes().to_vec()),
        "latin-1" => {
            let mut bytes: Vec<u8> = Vec::with_capacity(text.len());
            for (position, character) in text.chars().enumerate() {
                let code = character as u32;
                if code <= 0xFF {
                    bytes.push(code as u8);
                    continue;
                }
                match errors {
                    "ignore" => {}
                    "replace" => bytes.push(b'?'),
                    "strict" => {
                        return Err(instance.raise_builtin_error(
                            "UnicodeEncodeError",
                            &format!(
                                "'latin-1' codec can't encode character '{}' in position {position}: ordinal not in range(256)",
                                character_literal(code)
                            ),
                        ))
                    }
                    other => return Err(builtin_error_handler_failure(instance, codec, other)),
                }
            }
            Ok(bytes)
        }
        "ascii" => {
            let mut bytes: Vec<u8> = Vec::with_capacity(text.len());
            for (position, character) in text.chars().enumerate() {
                let code = character as u32;
                if code < 0x80 {
                    bytes.push(code as u8);
                    continue;
                }
                match errors {
                    "ignore" => {}
                    "replace" => bytes.push(b'?'),
                    "strict" => {
                        return Err(instance.raise_builtin_error(
                            "UnicodeEncodeError",
                            &format!(
                                "'ascii' codec can't encode character '{}' in position {position}: ordinal not in range(128)",
                                character_literal(code)
                            ),
                        ))
                    }
                    other => return Err(builtin_error_handler_failure(instance, codec, other)),
                }
            }
            Ok(bytes)
        }
        "cp437" => {
            let table = crate::instance::CP437;
            let mut bytes: Vec<u8> = Vec::with_capacity(text.len());
            for (position, character) in text.chars().enumerate() {
                match table.iter().position(|candidate| *candidate == character) {
                    Some(byte) => bytes.push(byte as u8),
                    None => match errors {
                        "ignore" => {}
                        "replace" => bytes.push(b'?'),
                        "strict" => {
                            return Err(instance.raise_builtin_error(
                                "UnicodeEncodeError",
                                &format!(
                                    // 参照的 cp437 走 **charmap** 编解码器 ⇒ 文案是 `'charmap' codec` ＋
                                    // `character maps to <undefined>`（实测 ✓），不是 `'cp437' … no mapping`。
                                    "'charmap' codec can't encode character '\\u{:04x}' in position {position}: character maps to <undefined>",
                                    character as u32
                                ),
                            ))
                        }
                        other => return Err(builtin_error_handler_failure(instance, codec, other)),
                    },
                }
            }
            Ok(bytes)
        }
        _ => unreachable!("builtin_encoding 只回四档"),
    }
}

/// UTF-8 解码错误的文案（照参照的分型：`invalid start byte`／`invalid continuation byte`／
/// `unexpected end of data`；单字节用 `byte 0x.. in position N`、多字节用 `bytes in position N-M` ✓）。
fn utf8_decode_error(instance: &Instance, value: &[u8], start: usize, error_len: Option<usize>) -> ExecError {
    let byte = value.get(start).copied().unwrap_or(0);
    let (reason, end) = match error_len {
        None => ("unexpected end of data", value.len()),
        Some(length) => {
            // 起始字节合法（0xC2..=0xF4）但续字节非法 ⇒ continuation；否则 start
            let reason = if (0xC2..=0xF4).contains(&byte) {
                "invalid continuation byte"
            } else {
                "invalid start byte"
            };
            (reason, (start + length).min(value.len()))
        }
    };
    let message = if end.saturating_sub(start) == 1 {
        format!("'utf-8' codec can't decode byte 0x{byte:02x} in position {start}: {reason}")
    } else {
        format!(
            "'utf-8' codec can't decode bytes in position {start}-{}: {reason}",
            end.saturating_sub(1)
        )
    };
    instance.raise_builtin_error("UnicodeDecodeError", &message)
}

/// **UTF-8 解码**（`errors` 三条：非法序列按**一个错误单元**跳过／替换成 `\uFFFD`；末尾截断序列
/// 整段算一个单元 —— 与参照的 `replace` 口径一致 ✓）。
fn decode_utf8_builtin(instance: &Instance, value: &[u8], errors: &str) -> Result<String, ExecError> {
    let mut out = String::with_capacity(value.len());
    let mut consumed = 0usize;
    while consumed < value.len() {
        match core::str::from_utf8(&value[consumed..]) {
            Ok(rest) => {
                out.push_str(rest);
                consumed = value.len();
            }
            Err(error) => {
                let valid = error.valid_up_to();
                if valid > 0 {
                    out.push_str(core::str::from_utf8(&value[consumed..consumed + valid]).unwrap_or(""));
                    consumed += valid;
                }
                let span = error.error_len().unwrap_or(value.len() - consumed);
                match errors {
                    "strict" => return Err(utf8_decode_error(instance, value, consumed, error.error_len())),
                    "ignore" => consumed += span,
                    "replace" => {
                        out.push('\u{fffd}');
                        consumed += span;
                    }
                    other => return Err(builtin_error_handler_failure(instance, "utf-8", other)),
                }
            }
        }
    }
    Ok(out)
}

/// **内置四档的解码快路径**（不碰注册表；语义与旧实现逐条一致 ＋ 补上 `errors` 三态 ✓）。
fn decode_builtin(
    instance: &Instance,
    value: &[u8],
    codec: &'static str,
    errors: &str,
) -> Result<String, ExecError> {
    match codec {
        "utf-8" => decode_utf8_builtin(instance, value, errors),
        "ascii" => {
            let mut out = String::with_capacity(value.len());
            for (index, byte) in value.iter().enumerate() {
                if *byte < 0x80 {
                    out.push(*byte as char);
                    continue;
                }
                match errors {
                    "ignore" => {}
                    "replace" => out.push('\u{fffd}'),
                    "strict" => {
                        return Err(instance.raise_builtin_error(
                            "UnicodeDecodeError",
                            &format!(
                                "'ascii' codec can't decode byte 0x{byte:02x} in position {index}: ordinal not in range(128)"
                            ),
                        ))
                    }
                    other => return Err(builtin_error_handler_failure(instance, codec, other)),
                }
            }
            Ok(out)
        }
        "latin-1" => Ok(value.iter().map(|byte| *byte as char).collect()),
        "cp437" => Ok(value
            .iter()
            .map(|byte| crate::instance::CP437[*byte as usize])
            .collect()),
        _ => unreachable!("builtin_encoding 只回四档"),
    }
}

impl Instance {
    /// 调 `_codecs.lookup(encoding)`（**一处委派**：内置四档之外的名字全走它 ✓）。
    ///
    /// `_codecs.lookup` 自己会（①）查内建表、（②）问已注册的 search function、（③）**懒导入**
    /// `encodings` 再问 —— 因此这里不需要知道任何编码清单 ✓（白名单只有 [`builtin_encoding`] 一张）。
    fn codec_registry_lookup(&self, encoding: &str) -> Result<NonNull<Header>, ExecError> {
        let unavailable = || {
            self.raise_builtin_error("LookupError", &format!("unknown encoding: {encoding}"))
        };
        let Some(modules) = self.modules() else {
            return Err(unavailable());
        };
        let Some(module) = self.dict_get(modules, "_codecs") else {
            return Err(unavailable());
        };
        let Some(namespace) = crate::executor::protocol::mounted_instance_dict(self, module) else {
            return Err(unavailable());
        };
        let Some(lookup) = self.dict_get(namespace, "lookup") else {
            return Err(unavailable());
        };
        let name = self.new_str(encoding);
        let result = crate::executor::call::call_value(self, lookup, &[name], &[]);
        // SAFETY: `name` 是本函数刚拿到的**新引用** ✓。
        unsafe { self.release_object(name.as_ptr()) };
        result
    }

    /// 编码器返回值归一：注册表里的编解码器照参照交 `(bytes, 消耗数)` 二元组；本层的内建四档
    /// `CodecInfo` 交裸 `bytes` ⇒ 两种形态都认 ✓（返回**新引用**）。
    fn codec_output_value(&self, output: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
        let Some(items) = self.tuple_items(output) else {
            return Ok(output);
        };
        let Some(first) = items.first().copied() else {
            return Ok(output);
        };
        let retained = self.retain(first);
        // SAFETY: `output` 是调用方交来的**新引用** ⇒ 取完第一项就还掉 ✓。
        unsafe { self.release_object(output.as_ptr()) };
        Ok(retained)
    }

    /// 这个 `CodecInfo` 是不是**文本编码器**（`_is_text_encoding`；缺省 `True` ✓）。
    ///
    /// `rot13`／`base64` 一类是"任意对象编解码器"，`str.encode`／`bytes.decode` 碰到它们照参照报
    /// `LookupError: 'x' is not a text encoding; use codecs.encode() to handle arbitrary codecs` ✓
    /// （**不是** `TypeError`，实测 ✓；名字用**调用方给的原文**，`'ROT13'` 就报 `'ROT13'` ✓）。
    fn codec_is_text_encoding(&self, info: NonNull<Header>) -> Result<bool, ExecError> {
        let Some(flag) = self.attribute_optional_of(info, "_is_text_encoding")? else {
            return Ok(true);
        };
        self.truthiness_of(flag)
    }

    /// **委派编码**：`_codecs.lookup(encoding).encode(text, errors)`（返回 bytes）。
    fn delegate_encode(
        &self,
        text: &str,
        encoding: &str,
        errors: &str,
    ) -> Result<NonNull<Header>, ExecError> {
        let info = self.codec_registry_lookup(encoding)?;
        if !self.codec_is_text_encoding(info)? {
            return Err(self.raise_builtin_error(
                "LookupError",
                &format!(
                    "'{encoding}' is not a text encoding; use codecs.encode() to handle arbitrary codecs"
                ),
            ));
        }
        let Some(method) = self.attribute_optional_of(info, "encode")? else {
            return Err(self.raise_builtin_error(
                "LookupError",
                &format!("unknown encoding: {encoding}"),
            ));
        };
        let text_value = self.new_str(text);
        let errors_value = self.new_str(errors);
        let result = crate::executor::call::call_value(self, method, &[text_value, errors_value], &[]);
        // SAFETY: 两个实参是本函数的**新引用**（`call_value` 自己也 retain 过一份）⇒ 用完归还 ✓。
        unsafe {
            self.release_object(text_value.as_ptr());
            self.release_object(errors_value.as_ptr());
        }
        self.codec_output_value(result?)
    }

    /// **委派解码**：`_codecs.lookup(encoding).decode(bytes, errors)`（返回 str）。
    fn delegate_decode(
        &self,
        value: &[u8],
        encoding: &str,
        errors: &str,
    ) -> Result<NonNull<Header>, ExecError> {
        let info = self.codec_registry_lookup(encoding)?;
        if !self.codec_is_text_encoding(info)? {
            return Err(self.raise_builtin_error(
                "LookupError",
                &format!(
                    "'{encoding}' is not a text encoding; use codecs.decode() to handle arbitrary codecs"
                ),
            ));
        }
        let Some(method) = self.attribute_optional_of(info, "decode")? else {
            return Err(self.raise_builtin_error(
                "LookupError",
                &format!("unknown encoding: {encoding}"),
            ));
        };
        let bytes_value = self.new_bytes(value);
        let errors_value = self.new_str(errors);
        let result = crate::executor::call::call_value(self, method, &[bytes_value, errors_value], &[]);
        // SAFETY: 同上 —— 两个实参是本函数的新引用 ✓。
        unsafe {
            self.release_object(bytes_value.as_ptr());
            self.release_object(errors_value.as_ptr());
        }
        self.codec_output_value(result?)
    }

    /// **按名字解字节** ✓（`bytes.decode`／`str(bytes, …)`／`_io` 的**一处真相** ✓）：
    /// 内置四档走快路径，其余**委派 `_codecs.lookup` 注册表**（⇒ `Lib/encodings/*` 全族可用 ✓）。
    pub fn decode_with_encoding(
        &self,
        value: &[u8],
        encoding: &str,
    ) -> Result<NonNull<Header>, ExecError> {
        self.decode_with_encoding_and_errors(value, encoding, "strict")
    }

    /// [`Self::decode_with_encoding`] ＋ `errors`（照参照的 strict／ignore／replace ✓）。
    pub fn decode_with_encoding_and_errors(
        &self,
        value: &[u8],
        encoding: &str,
        errors: &str,
    ) -> Result<NonNull<Header>, ExecError> {
        if let Some(codec) = builtin_encoding(encoding) {
            let text = decode_builtin(self, value, codec, errors)?;
            return Ok(self.new_str(&text));
        }
        self.delegate_decode(value, encoding, errors)
    }

    /// **按名字编文本** ✓（`str.encode`／`_io` 的**一处真相** ✓）：内置四档走快路径，
    /// 其余委派 `_codecs.lookup` 注册表（返回 **bytes** ✓）。
    pub fn encode_with_encoding(
        &self,
        text: &str,
        encoding: &str,
    ) -> Result<NonNull<Header>, ExecError> {
        self.encode_with_encoding_and_errors(text, encoding, "strict")
    }

    /// [`Self::encode_with_encoding`] ＋ `errors`（照参照的 strict／ignore／replace ✓）。
    pub fn encode_with_encoding_and_errors(
        &self,
        text: &str,
        encoding: &str,
        errors: &str,
    ) -> Result<NonNull<Header>, ExecError> {
        if let Some(codec) = builtin_encoding(encoding) {
            let bytes = encode_builtin(self, text, codec, errors)?;
            return Ok(self.new_bytes(&bytes));
        }
        self.delegate_encode(text, encoding, errors)
    }
}
