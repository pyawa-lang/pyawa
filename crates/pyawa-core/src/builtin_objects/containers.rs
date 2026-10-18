//! 本文件是 `builtin_objects` 的 `containers` 域：容器载荷与方法面（`tuple`／`list`／`set`／`dict`／`deque`）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;


/// 绑定 `self` 的 `set`（方法契约保证有 ✓）。
pub(crate) fn bound_set(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, crate::ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}

/// **`dict` 的方法面**（第 143／145 轮）：`get`／`keys`／`items`／`values` ✓ —— 与 `str`／`list`
/// 同一套路 ✓（返回绑定的 `MethodObject` ✓）。
///
/// **已知偏离**（如实登记 ✓）：`keys`／`items`／`values` 参照返回**视图对象** ✗，本层先返回
/// **列表** ✓（`len`／迭代／`list(...)` 这些常见用法一致 ✓；视图特有的集合运算未接 ✗）。
/// **容器方法面的 `__len__`** ✓（第 639 轮）：内建容器被**取属性** `__len__` 时要给出方法 ✓
/// （`Lib/functools.py` 的 `cache_len = cache.__len__` ✓ 与 `cache.get` 一族同路 ✓）。先前
/// `dict_getattr`／`list_getattr` 都没有这一格 ✗ ⇒ `AttributeError: 'dict' object has no attribute
/// '__len__'` ✓（`import re` 就断在这 ✓）。长度本身复用 `length_with_protocol` ✓（**一处真相** ✓）。
pub(crate) fn container_len_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(object) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__len__` 没有绑定实例",
        });
    };
    // **不能走 `length_with_protocol`** ✗（第 642 轮）：那条路会再查 `__len__` ✓ ⇒ 我们的
    // `__len__` 又调回本函数 ⇒ **无限递归** ✓（实测直接栈溢出 ✗）⇒ 这里只认**直接长度** ✓。
    match instance.length_of(object) {
        Some(length) => Ok(instance.new_int(length as i64)),
        None => Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "object of type '{}' has no len()",
                instance.type_name(instance.type_of(object))
            ),
        )),
    }
}

pub unsafe fn dict_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "__getitem__" => dict_getitem_native,
        "__setitem__" => dict_setitem_native,
        "__delitem__" => dict_delitem_native,
        "__eq__" => dict_eq_native,
        "__len__" => container_len_native,
        "get" => dict_get_native,
        "__contains__" => container_contains_native,
        "keys" => dict_keys_native,
        "values" => dict_values_native,
        "items" => dict_items_native,
        "update" => dict_update_native,
        "setdefault" => dict_setdefault_native,
        "pop" => dict_pop_native,
        "copy" => dict_copy_native,
        "clear" => dict_clear_native,
        "popitem" => dict_popitem_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(
        method_type,
        "dict",
        Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// 绑定 `self` 的 `dict`（方法契约保证有 ✓）。
pub(crate) fn bound_dict(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, crate::ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}


/// **`x.__contains__(y)`**（第 303 轮修 `P3-25`）：容器通用 —— 语义与 `y in x` **同一处**实现
/// （`executor::iter::contains` ✓）⇒ 不另写一遍 ✓。
///
/// 动因：上限榜上 `AttributeError: 'frozenset' object has no attribute '__contains__'` 那一族
/// **12** 个模块 ✓ —— 本层的 `in` 是**指令内联**的 ✓，但 `x.__contains__(y)` 这种**取属性**的路
/// 先前只有 `bytes` 接了一个 ✓（`str_getattr` 一带 ✓）。
pub(crate) fn container_contains_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(container) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__contains__' needs an argument",
        ));
    };
    let Some(item) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__contains__ expected 1 argument, got 0",
        ));
    };
    let found = crate::executor::protocol::contains_public(instance, container, *item, 0)?;
    Ok(instance.new_bool(found))
}


/// **`dict.__getitem__`／`dict.__setitem__`**（第 310 轮）：这两个 dunder **在类型上取**时是
/// **未绑定**的 ✓ ⇒ 接收者在 `args[0]` ✓；在**实例上取**时我们已给绑定形态 ✓ ⇒ 接收者在 `bound` ✓。
/// 两种都认 ✓（`Lib/collections/__init__.py:120` 的 `dict_setitem=dict.__setitem__` 正是前者 ✓，
/// 那一族 **31** 个模块 ✓）。
pub(crate) fn container_receiver(
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
) -> (Option<NonNull<Header>>, &[NonNull<Header>]) {
    match bound {
        Some(receiver) => (Some(receiver), args),
        None => match args.split_first() {
            Some((first, rest)) => (Some(*first), rest),
            None => (None, args),
        },
    }
}


/// 绑定 `self` 的 `list`（方法契约保证有 ✓）。
pub(crate) fn bound_list(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, crate::ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}

// 容器（`BC-49` 的容器与解包族；载荷布局按 `TS-43` 由实现自选）

py_object! {
    /// `tuple` 的实例：**不可变**——造好之后不再改（载荷只在构造期填）。
    pub struct TupleObject {
        items: Vec<NonNull<Header>>,
    }
}

py_object! {
    /// `list` 的实例。
    pub struct ListObject {
        items: RefCell<Vec<NonNull<Header>>>,
    }


}

py_object! {
    /// `set` 的实例。
    ///
    /// *已知偏离*：本层按**插入顺序**存（关联表），参照实现的迭代顺序由哈希决定。
    /// 集合的文档契约是"无序"，故这一条属于**实现观测面**，落地 `MS-19` 时要如实登记。
    pub struct SetObject {
        items: RefCell<Vec<NonNull<Header>>>,
    }
}

py_object! {
    /// **`collections.deque` 的实例**（第 331 轮）。
    ///
    /// 载荷就是一个 `Vec` ＋ `maxlen` ✓ —— `deque` 的语义（两端进出、按 `maxlen` 丢另一端）
    /// 在**方法面**里实现 ✓；本层不做环形缓冲（`Vec` 的头部操作是 O(n) ✗，但语义正确 ✓，
    /// 属于**实现观测面**，`MS-19` 落地时如实登记 ✓）。
    ///
    /// **本段落地**：`append`／`appendleft`／`pop`／`popleft`／`extend`／`extendleft`／`clear`／
    /// `rotate`／`count`／`remove`／`index`／`insert`／`copy` ＋ `maxlen` ＋ `len()` ＋ `repr` ✓。
    /// **如实登记的未接面** ✗：迭代协议（`for x in deque(...)` 要一个迭代器类型 ✓，随后补 ✓）、
    /// `__getitem__`／`__setitem__`／`__delitem__`／`__contains__`／`__eq__`／`reverse` ✓。
    pub struct DequeObject {
        items: RefCell<Vec<NonNull<Header>>>,
        /// `maxlen`（`None` ⇒ 不限 ✓；用 `usize::MAX` 当"不限"的哨兵 ✓ —— 参照的 `maxlen`
        /// 属性在"不限"时给 `None` ✓，`deque_getattr` 会翻回来 ✓）。
        maxlen: Cell<usize>,
    }
}

py_object! {
    /// `dict` 的实例。
    ///
    /// *临时*：关联表 ＋ 线性查找（查找走"值相等"而不是 `__hash__`／`__eq__` 槽位——
    /// 那两个槽位随类型系统接线）。**插入顺序**与参照实现一致。
    pub struct DictObject {
        entries: RefCell<Vec<(NonNull<Header>, NonNull<Header>)>>,
    }
}

impl TupleObject {
    /// 注册这个类型时的槽位表（持有引用 ⇒ 要 `traverse`／`clear`，`OM-12`）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(tuple_traverse)
            .with_clear(tuple_clear)
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 按位置取元素（**借用**的裸引用）。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.get(index).copied()
    }

    /// 全部元素（**借用**）。
    pub fn items(&self) -> &[NonNull<Header>] {
        &self.items
    }
}

impl ListObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(list_traverse)
            .with_clear(list_clear)
    }

    /// **按下标摘掉一项**（第 154 轮，`list.remove()` 用 ✓）：那份引用**转交**调用方 ✓。
    pub fn remove_at(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }

    /// **就地反转**（第 147 轮，`list.reverse()` 用 ✓）——只动顺序，**不碰引用** ✓。
    pub fn reverse_items(&self) {
        self.items.borrow_mut().reverse();
    }

    /// **按下标插入**（第 146 轮，`list.insert()` 用 ✓；`index` 越界按参照**夹到两端** ✓）。
    pub fn insert_at(&self, index: usize, item: NonNull<Header>) {
        let mut items = self.items.borrow_mut();
        let at = index.min(items.len());
        items.insert(at, item);
    }

    /// **按下标取**（判等由调用方做 ✓）。
    pub fn position_where(
        &self,
        predicate: impl Fn(NonNull<Header>) -> bool,
    ) -> Option<usize> {
        self.items.borrow().iter().position(|item| predicate(*item))
    }

    /// **弹出末项**（第 143 轮，`list.pop()` 用 ✓）：返回那一项（**那份引用交给调用方** ✓）。
    /// 注意：**必须并进这个 impl** ✗ —— 静态检查 `gc_field_coverage` 只读**该类型的第一个
    /// `impl` 块** ✓；我先前另立一个更靠前的 `impl ListObject` ⇒ 它看不到 `with_traverse`／
    /// `with_clear` ⇒ 判红 ✓（根因就是这 ✓）。
    pub fn pop_last(&self) -> Option<NonNull<Header>> {
        self.items.borrow_mut().pop()
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// 按位置取元素（**借用**）。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.borrow().get(index).copied()
    }

    /// 追加一个**新引用**（引用由本对象接管）。
    pub fn append(&self, value: NonNull<Header>) {
        self.items.borrow_mut().push(value);
    }

    /// 追加一批**新引用**。
    pub fn extend(&self, values: impl IntoIterator<Item = NonNull<Header>>) {
        self.items.borrow_mut().extend(values);
    }

    /// 全部元素（**借用**的副本）。
    pub fn items(&self) -> Vec<NonNull<Header>> {
        self.items.borrow().clone()
    }

    /// 替换第 `index` 项（**新引用**），返回旧值（调用方负责释放）。
    /// 在 `index` 处插入（`a[i:j] = …` 的**切片写**要用；`index` 必须 `<= len`）。
    pub fn insert(&self, index: usize, value: NonNull<Header>) {
        self.items.borrow_mut().insert(index, value);
    }

    pub fn replace(&self, index: usize, value: NonNull<Header>) -> Option<NonNull<Header>> {
        self.items
            .borrow_mut()
            .get_mut(index)
            .map(|slot| core::mem::replace(slot, value))
    }

    /// 删除第 `index` 项，返回被删的那份引用（调用方负责释放）。
    pub fn remove(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }
}

impl SetObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(set_traverse)
            .with_clear(set_clear)
    }

    /// **去掉一个元素**（第 146 轮，`set.discard()` 用 ✓）：找到就交给调用方 ✓（那份引用
    /// **转交**出去 ✓），没找到给 `None` ✓。本层 `set` 按**插入顺序**存 ✓（`SetObject` 的
    /// 既有口径 ✓），相等性由调用方按 `values_equal` 判定后传入**下标** ✓。
    pub fn remove_at(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }

    /// **判等用的线性查找**（本层口径 ✓）：返回第一个与 `wanted` 相等的下标 ✓。
    pub fn position_of(
        &self,
        predicate: impl Fn(NonNull<Header>) -> bool,
    ) -> Option<usize> {
        self.items.borrow().iter().position(|item| predicate(*item))
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// 按位置取元素（**借用**）——顺序是插入顺序，见类型文档的*已知偏离*。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.borrow().get(index).copied()
    }

    /// 全部元素（**借用**的副本）。
    pub fn items(&self) -> Vec<NonNull<Header>> {
        self.items.borrow().clone()
    }

    /// 追加一个**新引用**（不去重；调用方负责先查重）。
    pub fn insert_raw(&self, value: NonNull<Header>) {
        self.items.borrow_mut().push(value);
    }

    /// 删除第 `index` 项，返回被删的那份引用（调用方负责释放）。
    pub fn remove(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }

    /// 是否已含某个元素（按指针）。
    pub fn contains_ptr(&self, value: NonNull<Header>) -> bool {
        self.items.borrow().iter().any(|entry| *entry == value)
    }
}

impl DictObject {
    /// **替换某一项的值**（第 147 轮，`dict.update`／`setdefault` 用 ✓）：返回**旧值** ✓，
    /// 由调用方归还引擎 ✓（本对象不再持有它 ✓）。
    pub fn set_value_at(&self, index: usize, value: NonNull<Header>) -> Option<NonNull<Header>> {
        let mut entries = self.entries.borrow_mut();
        let entry = entries.get_mut(index)?;
        Some(core::mem::replace(&mut entry.1, value))
    }

    /// **摘掉某一项**（第 147 轮，`dict.pop` 用 ✓）：键与值**各一份引用转交**调用方 ✓。
    pub fn remove_at(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        let mut entries = self.entries.borrow_mut();
        if index >= entries.len() {
            return None;
        }
        Some(entries.remove(index))
    }

    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(dict_traverse)
            .with_clear(dict_clear)
    }

    /// 条目数。
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    /// 按位置取条目（**借用**）——顺序是插入顺序。
    pub fn entry(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        self.entries.borrow().get(index).copied()
    }

    /// 全部条目（**借用**的副本）。
    pub fn entries(&self) -> Vec<(NonNull<Header>, NonNull<Header>)> {
        // **这里是上限榜 `-6`（SIGABRT）族的落点** ✓（第 82 轮用 `RUST_BACKTRACE=1` 抓到 ✓）：
        //   `core::ptr::copy_nonoverlapping::<(NonNull<Header>, NonNull<Header>)>`
        //     ← `[…].to_vec` ← `Vec<…>::clone` ← **本函数**
        //     ← `executor::protocol::lookup_in_mapping` ← `execute` ← `run_class_body` ← `build_class_native` ✓
        // ⇒ std 的 `copy_nonoverlapping` **前置条件被违反** ⇒ **非展开 panic** ⇒ **abort**（拿不到回溯 ✗）。
        // 本函数只是 `RefCell<Vec<…>>::borrow().clone()` ✓ ⇒ 唯一解释是**这个 `DictObject` 已被释放**、
        // 内存被别的东西复用（实测是字符串：那些"长度"的十六进制里含 `__cod__`／`name` ✓）
        // ⇒ **释放后使用** ✓。与 `Lib/re/__init__.py` 顶层就能触发 ✓、两次运行值不同 ✓、以及
        // `PYAWA_QUARANTINE=1`／`PYAWA_DANGLING=1` 下症状相同 ✓ 完全相符 ✓。
        // **试过的诊断** ✗：在本函数里查 `Vec` 自诉的 `len`／`capacity`／指针自洽性 —— **不响** ✓
        //（对象整体悬垂时，读到的字段可能"自洽" ✗），而且本函数在**查找热路径**上 ✓ ⇒ 白付开销 ✗
        // ⇒ 撤掉守卫、只留这段注记 ✓。**下一步**：从 `run_class_body`／`build_class_native` 这一侧
        // 核**命名空间字典的所有权**（谁提前把它释放了 ✓），而不是在这里加检查 ✓。
        self.entries.borrow().clone()
    }

    /// 找某个键的位置（按指针；值相等的查找在 `executor` 里做）。
    pub fn position_of_ptr(&self, key: NonNull<Header>) -> Option<usize> {
        self.entries
            .borrow()
            .iter()
            .position(|(candidate, _)| *candidate == key)
    }

    /// 追加一个**新引用**的键值对（不查重；调用方先用"值相等"查过）。
    pub fn insert_raw(&self, key: NonNull<Header>, value: NonNull<Header>) {
        self.entries.borrow_mut().push((key, value));
    }

    /// 替换第 `index` 项的值（**新引用**），返回旧值（调用方负责释放）。
    pub fn replace_value(
        &self,
        index: usize,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        self.entries
            .borrow_mut()
            .get_mut(index)
            .map(|slot| core::mem::replace(&mut slot.1, value))
    }

    /// 删除第 `index` 项，返回 `(键, 值)`（调用方负责释放这两份引用）。
    pub fn remove(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        let mut entries = self.entries.borrow_mut();
        if index >= entries.len() {
            return None;
        }
        Some(entries.remove(index))
    }

    /// 写入一个**新引用**的键值对；键已存在时替换值并交出旧值（调用方负责释放）。
    pub fn insert(
        &self,
        key: NonNull<Header>,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        let mut entries = self.entries.borrow_mut();
        if let Some(slot) = entries
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
            .map(|slot| &mut slot.1)
        {
            return Some(core::mem::replace(slot, value));
        }
        entries.push((key, value));
        None
    }
}

/// `OM-40`：列出元组元素。
pub(crate) unsafe fn tuple_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TupleObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出元组元素。
pub(crate) unsafe fn tuple_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    // **元组同样要腾空** ✗（第 206 轮修复 ✓；`items` 是普通 `Vec` ⇒ 可变借用取走 ✓）。
    let object = unsafe { &mut *ptr.cast::<TupleObject>() };
    for value in core::mem::take(&mut object.items) {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

impl DequeObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(deque_traverse)
            .with_clear(deque_clear)
            .with_repr(deque_repr)
    }

    /// 元素（**借用**）。
    pub fn items(&self) -> Vec<NonNull<Header>> {
        self.items.borrow().clone()
    }

    /// 长度。
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// `maxlen` 的**原始**值（`usize::MAX` ＝ 不限 ✓）。
    pub fn raw_maxlen(&self) -> usize {
        self.maxlen.get()
    }

    /// 末尾压入（**接管**一份引用 ✓）；超过 `maxlen` 时从另一端丢一个 ✓（丢了就释放 ✓）。
    pub fn push_back(&self, instance: &Instance, value: NonNull<Header>) {
        let mut items = self.items.borrow_mut();
        items.push(value);
        if items.len() > self.maxlen.get() {
            let dropped = items.remove(0);
            // SAFETY: dropped 由本对象持有；这份引用交还实例 ✓。
            unsafe { instance.release_object(dropped.as_ptr()) };
        }
    }

    /// 头部压入（同上 ✓）。
    pub fn push_front(&self, instance: &Instance, value: NonNull<Header>) {
        let mut items = self.items.borrow_mut();
        items.insert(0, value);
        if items.len() > self.maxlen.get() {
            let dropped = items.pop().expect("刚插过，非空");
            // SAFETY: 同上。
            unsafe { instance.release_object(dropped.as_ptr()) };
        }
    }
}


/// 取 `deque` 的 `self`（方法契约保证有 ✓）**与新引用**的实参表。
macro_rules! deque_self {
    ($instance:expr, $bound:expr, $args:expr, $name:literal) => {{
        let Some(owner) = $bound else {
            return Err($instance.raise_builtin_error("TypeError", concat!($name, " 缺少 self")));
        };
        // SAFETY: owner 由方法对象持有，存活。
        let object = unsafe { &*owner.as_ptr().cast::<DequeObject>() };
        (object, owner)
    }};
}
pub(crate) use deque_self;

/// 见 [`tuple_traverse`]（键与值都要列）。
pub(crate) unsafe fn dict_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    for (key, value) in object.entries() {
        visit(key.as_ptr());
        visit(value.as_ptr());
    }
}

/// 见 [`tuple_clear`]（键与值都要交出）。
pub(crate) unsafe fn dict_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    // **同 `list_clear`** ✗（第 206 轮修复 ✓）。
    for (key, value) in core::mem::take(&mut *object.entries.borrow_mut()) {
        // SAFETY: 这些引用由本对象持有。
        unsafe {
            instance.release_object(key.as_ptr());
            instance.release_object(value.as_ptr());
        }
    }
}
