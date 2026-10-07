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

    /// 两个值的**序**（`min`／`max`／`sorted` 要用）：数值塔按数比、两个 `str` 按字典序。
    ///
    /// 其余给 `None`——调用方据此报参照实现那条
    /// `TypeError: '<' not supported between instances of 'str' and 'int'`（实测）。
    pub fn order_of(
        &self,
        left: NonNull<Header>,
        right: NonNull<Header>,
    ) -> Option<core::cmp::Ordering> {
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
