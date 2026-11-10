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
/// **可调用对象的 `__call__` 类型字典入口**（round 215）：`collections.abc.Callable` 的
/// `__subclasshook__` 用 `_check_methods(C, "__call__")`（查 `C.__dict__` ✓）⇒ 只放 call 槽不够 ✗
/// （`isinstance(lambda: 0, Callable)` 判 False ✗）。值与真实调用等价（转 `call_callable` ✓）。
pub(crate) fn dunder_call_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = match bound {
        Some(receiver) => (receiver, args),
        None => match args.split_first() {
            Some((receiver, rest)) => (*receiver, rest),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "__call__ 需要一个接收者",
                ))
            }
        },
    };
    crate::executor::call::call_callable(instance, receiver, None, rest.to_vec(), kwargs.to_vec(), 0)
}

/// **内建容器的 `__getitem__` 类型字典入口**（round 214）：`tuple.__getitem__`／`list.__getitem__` 一类
/// 在类型上取要是**同一个对象**（`namedtuple` 的 `Point.__getitem__ == tuple.__getitem__` ✓）；
/// 绑定／未绑定两种形态都认 ✓（未绑定：接收者在 `args[0]` ✓）。
pub(crate) fn container_getitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (container, rest) = match bound {
        Some(container) => (container, args),
        None => match args.split_first() {
            Some((container, rest)) => (*container, rest),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "descriptor '__getitem__' needs an argument",
                ))
            }
        },
    };
    let Some(key) = rest.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__getitem__ expected 1 argument, got 0",
        ));
    };
    crate::executor::subscript::subscript_get_inner(instance, container, *key, 0)
}

/// **`dict.__reversed__`**（round 214）：`collections.abc.Reversible` 的 `_check_methods` 要看
/// `dict.__dict__['__reversed__']` ✓（`Counter()`／`dict()` 都是 Reversible ✓）。返回键的反向迭代器 ✓
///（值/项视图的反向由各自类型再补 ✓）。
pub(crate) fn dict_reversed_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (container, _) = match bound {
        Some(container) => (container, args),
        None => match args.split_first() {
            Some((container, rest)) => (*container, rest),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "descriptor '__reversed__' needs an argument",
                ))
            }
        },
    };
    let entries = instance.dict_entries(container).unwrap_or_default();
    let mut keys: Vec<NonNull<Header>> = Vec::with_capacity(entries.len());
    for (key, _) in entries.into_iter().rev() {
        keys.push(instance.retain(key));
    }
    let list = instance.new_list(keys);
    let iterator = crate::executor::iter_value(instance, list);
    instance.release(list);
    iterator
}

/// **`dict.__contains__` 的"内建语义"入口**（round 212）：`dict` 子类覆写里会写
/// `dict.__contains__(self, k)` ✓ —— 若这里再走 `contains()` 的协议回退会**递归**到覆写本身 ✗
/// （`test_collections` 的 `lowerdict` 实测 `RecursionError` ✗）⇒ 这里**只走内建哈希查键** ✓。
/// **`dict.__getitem__` 的"内建语义"入口**（round 212）：`dict` 子类覆写里写
/// `dict.__getitem__(self, k)` ✓ —— 若走一般下标协议会**递归**到覆写本身 ✗ ⇒ 这里只做**内建查键** ✓。
pub(crate) fn dict_getitem_builtin_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (container, rest) = match bound {
        Some(container) => (container, args),
        None => match args.split_first() {
            Some((container, rest)) => (*container, rest),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "descriptor '__getitem__' needs an argument",
                ))
            }
        },
    };
    let Some(key) = rest.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__getitem__ expected 1 argument, got 0",
        ));
    };
    crate::builtin::dict::reject_unhashable_key(instance, *key)?;
    let position = match instance.dict_lookup_key_checked(container, *key)? {
        Some(position) => position,
        None => {
            instance.retain(*key);
            let exception = crate::executor::new_exception_with_args(
                instance,
                crate::executor::exception_type(instance, "KeyError"),
                vec![*key],
            );
            return Err(crate::executor::runtime::raise(instance, exception));
        }
    };
    // SAFETY: container 是活的 dict，position 刚查到。
    let dict = unsafe { &*container.as_ptr().cast::<crate::builtin_objects::DictObject>() };
    let (_, value) = dict.entry(position).expect("刚查到的位置");
    instance.retain(value);
    Ok(value)
}

pub(crate) fn dict_contains_builtin_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (container, rest) = match bound {
        Some(container) => (container, args),
        None => match args.split_first() {
            Some((container, rest)) => (*container, rest),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "descriptor '__contains__' needs an argument",
                ))
            }
        },
    };
    let Some(item) = rest.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__contains__ expected 1 argument, got 0",
        ));
    };
    crate::builtin::dict::reject_unhashable_key(instance, *item)?;
    let found = instance.dict_lookup_key_checked(container, *item)?.is_some();
    Ok(instance.new_bool(found))
}

pub(crate) fn container_contains_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // **两种形态都认**（round 212）：实例上取是绑定（`bound` ✓）；类型上取是**未绑定**
    // （`dict.__contains__(self, k)` ⇒ 接收者在 `args[0]` ✓，与 `dict.__getitem__` 同口径 ✓）。
    let (container, rest) = match bound {
        Some(container) => (container, args),
        None => match args.split_first() {
            Some((container, rest)) => (*container, rest),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "descriptor '__contains__' needs an argument",
                ))
            }
        },
    };
    let Some(item) = rest.first() else {
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
        {
            /// **查找索引**（round 70；round 88 改为**地址表**）：元素哈希 → **元素地址**（`usize`）；
            /// `None` ⇒ 还没建／已作废 ✓（插入顺序仍是 `items` 一处真相 ✓）。
            /// 为什么存地址不存下标 ✗：`discard`／`pop` 会让后面元素**下标整体前移** ⇒ 索引要重建／
            /// 每次 O(n)（`test_collections` 的 `set` 删除后 `in` 交替 ⇒ O(n²) 超时 ✗）；存地址后
            /// **删除只需从桶里摘掉那一个地址** ✓、下标无关 ✓。
            /// 用 `usize` 而非 `NonNull` 存 ✓：索引里的条目是**非持有引用**（`items` 才是所有权所在 ✓）
            /// ⇒ 不应被 `traverse` 当成子引用计数（`OM-40` 的 GC 字段覆盖检查也会误报 ✗）。
            index: RefCell<Option<std::collections::HashMap<u64, Vec<usize>>>>,
        }
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
    /// **插入顺序**与参照实现一致（`entries` 是唯一真相）；`index` 是**惰性构造的查找索引**
    /// （键哈希 → 条目下标），由 [`Instance::dict_lookup_key`] 维护/使用（`P3-41` round 42 修
    /// `test_dict` 的 O(n²)）。任何改键的操作（`insert_raw`／`remove`／`insert`）都会**作废**它；
    /// 改值（`set_value_at`／`replace_value`）不影响 ✓。
    pub struct DictObject {
        entries: RefCell<Vec<(NonNull<Header>, NonNull<Header>)>>,
        {
            /// 查找索引；`None` ⇒ 还没建／已作废。
            index: RefCell<Option<std::collections::HashMap<u64, Vec<usize>>>>,
            /// **键集版本号**（`P3-41` round 42）：改键（插入／删除）时 +1 ⇒ 迭代器据此报
            /// `RuntimeError: dictionary changed size during iteration` ✓（只改值不算 ✓，照参照）。
            version: Cell<u64>,
            /// 墓碑计数（round 156 调试）
            deleted: Cell<usize>,
            /// **逐项缓存键哈希**（round 121）：参照只算一次 `__hash__` ✓ —— 先前"哈希索引"重建时
            /// 把**已存键重新哈希** ✗ ⇒ `test_dict.test_setdefault_atomic` 的 `hash_count == 1`
            /// 当场 2 ✗。`u64::MAX` 是"未知"哨兵（走 `insert_raw` 的键）✓，索引重建时才现算 ✓。
            hashes: RefCell<Vec<u64>>,
        }
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

    /// **摘出全部元素**（引用**转交**调用方 ✓；round 105：`list *= n` 的就地重复用 ✓）。
    pub fn take_items(&self) -> Vec<NonNull<Header>> {
        core::mem::take(&mut *self.items.borrow_mut())
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
        // **作废查找索引**（round 86 真 bug 修 ✗）：见上面 `remove` 的同一注释 ✓。
        *self.index.borrow_mut() = None;
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

    /// 追加一个**新引用**（不去重；调用方负责先查重）。**作废查找索引** ✓。
    pub fn insert_raw(&self, value: NonNull<Header>) {
        *self.index.borrow_mut() = None;
        self.items.borrow_mut().push(value);
    }

    /// **建／用查找索引**（round 70）：`hash_of` 给元素的哈希（不可哈希给 `None` ⇒ 跳过）。
    pub fn ensure_index(&self, hash_of: impl Fn(NonNull<Header>) -> Option<u64>) {
        if self.index.borrow().is_some() {
            return;
        }
        let mut built: std::collections::HashMap<u64, Vec<usize>> =
            std::collections::HashMap::new();
        {
            let items = self.items.borrow();
            for item in items.iter() {
                if let Some(hash) = hash_of(*item) {
                    built.entry(hash).or_default().push(item.as_ptr() as usize);
                }
            }
        }
        *self.index.borrow_mut() = Some(built);
    }

    /// 同一哈希的候选**元素指针**（**借用后复制**，与 `DictObject::candidates` 同口径 ✓）。
    pub fn candidates(&self, hash: u64) -> Vec<NonNull<Header>> {
        self.index
            .borrow()
            .as_ref()
            .and_then(|map| map.get(&hash).cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|address| NonNull::new(address as *mut Header))
            .collect()
    }

    /// **按哈希插入**（增量维护索引 ✓；调用方已确认不重复 ✓）——**接管** `value` 那份引用 ✓。
    pub fn insert_hashed(&self, value: NonNull<Header>, hash: u64) {
        {
            let mut items = self.items.borrow_mut();
            items.push(value);
        }
        let mut index = self.index.borrow_mut();
        if let Some(map) = index.as_mut() {
            map.entry(hash).or_default().push(value.as_ptr() as usize);
        }
    }

    /// **按指针从索引里摘掉并删除元素**（round 88）：返回被删的那份引用（调用方负责释放）✓。
    /// 桶里只摘**那一个指针** ✓，其余元素与顺序不受影响 ✓（这也是 `discard` 的 O(1) 索引维护路径 ✓）。
    pub fn remove_ptr(&self, item: NonNull<Header>, hash: u64) -> Option<NonNull<Header>> {
        {
            let mut index = self.index.borrow_mut();
            if let Some(map) = index.as_mut() {
                if let Some(bucket) = map.get_mut(&hash) {
                    let address = item.as_ptr() as usize;
                    if let Some(position) = bucket.iter().position(|entry| *entry == address) {
                        bucket.remove(position);
                    }
                    if bucket.is_empty() {
                        map.remove(&hash);
                    }
                }
            }
        }
        let mut items = self.items.borrow_mut();
        let position = items.iter().position(|entry| *entry == item)?;
        Some(items.remove(position))
    }

    /// 删除第 `index` 项，返回被删的那份引用（调用方负责释放）。
    /// **作废查找索引**（round 86 真 bug 修 ✗）：删除会让后面元素下标前移 ⇒ 索引里的旧下标错位 ✓；
    /// 先前 `discard`／`pop`／`remove` 都不作废 ⇒ **删过元素后 `in` 会漏判**（实测
    /// `test_baseexception` 里 `exc_set.discard(...)` 之后 `assertIn('BaseExceptionGroup', …)` 失败 ✗，
    /// 而 `repr` 里明明有它 ✓）。`DictObject::remove_at` 一直有这一步 ✓，此处补齐 ✓。
    pub fn remove(&self, index: usize) -> Option<NonNull<Header>> {
        *self.index.borrow_mut() = None;
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
    pub fn tombstone() -> (NonNull<Header>, NonNull<Header>) {
        let dangling = NonNull::<Header>::dangling();
        (dangling, dangling)
    }

    pub fn is_tombstone(key: NonNull<Header>) -> bool {
        key == NonNull::<Header>::dangling()
    }

    pub fn next_live_entry(
        &self,
        from: usize,
    ) -> Option<(usize, (NonNull<Header>, NonNull<Header>))> {
        let entries = self.entries.borrow();
        for position in from..entries.len() {
            let entry = entries[position];
            if !Self::is_tombstone(entry.0) {
                return Some((position, entry));
            }
        }
        None
    }

    /// 最后一个活条目（跳过墓碑；给 `popitem`／`clear` 一类"从尾部取"的调用 ✓）。
    pub fn last_live_entry(&self) -> Option<(usize, (NonNull<Header>, NonNull<Header>))> {
        let entries = self.entries.borrow();
        for position in (0..entries.len()).rev() {
            let entry = entries[position];
            if !Self::is_tombstone(entry.0) {
                return Some((position, entry));
            }
        }
        None
    }

    pub fn find_live_position(
        &self,
        predicate: impl Fn(NonNull<Header>) -> bool,
    ) -> Option<usize> {
        let entries = self.entries.borrow();
        for (position, (key, _)) in entries.iter().enumerate() {
            if Self::is_tombstone(*key) {
                continue;
            }
            if predicate(*key) {
                return Some(position);
            }
        }
        None
    }

    fn compact_if_needed(&self) {
        let deleted = self.deleted.get();
        if deleted < 32 || deleted * 2 < self.entries.borrow().len() {
            return;
        }
        let mut entries = self.entries.borrow_mut();
        let mut hashes = self.hashes.borrow_mut();
        // **容量用饱和减法**（round 213 真 panic 修 ✗）：有的 dict 是"直接搬 entries"造出来的
        // （`mappingproxy_new` 一类 ✓，`hashes` 为空 ✓）⇒ `hashes.len() - deleted` 会**下溢**成
        // 天文数字 ⇒ `Vec::with_capacity` 报 `capacity overflow` panic ✗（`test_collections` 的
        // `TestNamedTuple.test_name_conflicts` 实测 ✓）。饱和减法 + 逐位置取 hash 兜底即可 ✓。
        let mut kept_entries: Vec<(NonNull<Header>, NonNull<Header>)> =
            Vec::with_capacity(entries.len().saturating_sub(deleted));
        let mut kept_hashes: Vec<u64> =
            Vec::with_capacity(hashes.len().saturating_sub(deleted));
        for (position, entry) in entries.iter().enumerate() {
            if Self::is_tombstone(entry.0) {
                continue;
            }
            kept_entries.push(*entry);
            kept_hashes.push(hashes.get(position).copied().unwrap_or(u64::MAX));
        }
        *entries = kept_entries;
        *hashes = kept_hashes;
        self.deleted.set(0);
        self.invalidate_index();
    }

    /// **替换某一项的值**（第 147 轮，`dict.update`／`setdefault` 用 ✓）：返回**旧值** ✓，
    /// 由调用方归还引擎 ✓（本对象不再持有它 ✓）。
    pub fn set_value_at(&self, index: usize, value: NonNull<Header>) -> Option<NonNull<Header>> {
        let mut entries = self.entries.borrow_mut();
        let entry = entries.get_mut(index)?;
        Some(core::mem::replace(&mut entry.1, value))
    }

    /// **键集版本号**（迭代器用它检测"迭代中改字典" ✓）。
    pub fn version(&self) -> u64 {
        self.version.get()
    }

    /// 记一次**键集变化**（插入／删除；改值不算 ✓）。
    fn bump_version(&self) {
        self.version.set(self.version.get().wrapping_add(1));
    }

    /// **作废查找索引**（改键的操作用；下一轮查找再惰性重建 ✓）。
    fn invalidate_index(&self) {
        *self.index.borrow_mut() = None;
    }

    /// **确保查找索引已建**（`P3-41` round 42）：`hash_of` 由 `Instance` 给（core 里唯一的
    /// 哈希口径 `values::key_hash` ✓ —— 与 `values_equal` 同源）。
    pub fn ensure_index(&self, hash_of: impl Fn(NonNull<Header>) -> Option<u64>) {
        if self.index.borrow().is_some() {
            return;
        }
        let built = {
            let entries = self.entries.borrow();
            let hashes = self.hashes.borrow();
            let mut map: std::collections::HashMap<u64, Vec<usize>> =
                std::collections::HashMap::new();
            for (position, (key, _)) in entries.iter().enumerate() {
                if Self::is_tombstone(*key) {
                    continue;
                }
                // **优先用缓存哈希**（参照只算一次 `__hash__` ✓）；哨兵才现算 ✓。
                let cached = hashes.get(position).copied().filter(|hash| *hash != u64::MAX);
                if let Some(hash) = cached.or_else(|| hash_of(*key)) {
                    map.entry(hash).or_default().push(position);
                }
            }
            map
        };
        *self.index.borrow_mut() = Some(built);
    }

    /// 某个哈希桶里的候选条目下标（**拷贝**；桶通常只有一条 ✓）。
    pub fn candidates(&self, hash: u64) -> Vec<usize> {
        self.index
            .borrow()
            .as_ref()
            .and_then(|map| map.get(&hash).cloned())
            .unwrap_or_default()
    }

    /// **带哈希插入**（调用方已算好键哈希；索引存在时**增量更新** ✓ —— 这样"边插边查"的
    /// 造表过程也是 O(1)／次，而不是每次作废重建 ✓）。
    pub fn insert_hashed(
        &self,
        key: NonNull<Header>,
        value: NonNull<Header>,
        hash: u64,
    ) -> usize {
        self.bump_version();
        let position = {
            let mut entries = self.entries.borrow_mut();
            let position = entries.len();
            entries.push((key, value));
            self.hashes.borrow_mut().push(hash);
            position
        };
        if let Some(index) = self.index.borrow_mut().as_mut() {
            index.entry(hash).or_default().push(position);
        }
        position
    }

    /// **摘掉某一项**（第 147 轮，`dict.pop` 用 ✓）：键与值**各一份引用转交**调用方 ✓。
    pub fn remove_at(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        self.bump_version();
        // **索引增量更新而不是整体作废**（round 156 性能）：墓碑保住"位置不变" ✓ ⇒ 删一项只需把
        // 该位置从它那个哈希桶里摘掉 ✓。先前每次删除都 `invalidate_index` ⇒ 下一次查找重建整张索引
        // 是 O(n) ⇒ `WeakKeyDictionary` 的 20k 次删除 O(n²)（实测 17 s ✓）。
        let hash = {
            let hashes = self.hashes.borrow();
            hashes.get(index).copied().unwrap_or(u64::MAX)
        };
        let removed = {
            let mut entries = self.entries.borrow_mut();
            let slot = entries.get_mut(index)?;
            if Self::is_tombstone(slot.0) {
                return None;
            }
            core::mem::replace(slot, Self::tombstone())
        };
        {
            let mut hashes = self.hashes.borrow_mut();
            if let Some(slot) = hashes.get_mut(index) {
                *slot = u64::MAX;
            }
        }
        if hash != u64::MAX {
            if let Some(index_map) = self.index.borrow_mut().as_mut() {
                if let Some(bucket) = index_map.get_mut(&hash) {
                    bucket.retain(|position| *position != index);
                }
            }
        }
        self.deleted.set(self.deleted.get() + 1);
        self.compact_if_needed();
        Some(removed)
    }

    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(dict_traverse)
            .with_clear(dict_clear)
    }

    pub fn len(&self) -> usize {
        self.entries.borrow().len().saturating_sub(self.deleted.get())
    }

    pub fn raw_len(&self) -> usize {
        self.entries.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entry(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        let entry = self.entries.borrow().get(index).copied()?;
        if Self::is_tombstone(entry.0) {
            None
        } else {
            Some(entry)
        }
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
        self.entries
            .borrow()
            .iter()
            .copied()
            .filter(|(key, _)| !Self::is_tombstone(*key))
            .collect()
    }

    /// 找某个键的位置（按指针；值相等的查找在 `executor` 里做）。
    pub fn position_of_ptr(&self, key: NonNull<Header>) -> Option<usize> {
        self.entries
            .borrow()
            .iter()
            .position(|(candidate, _)| !Self::is_tombstone(*candidate) && *candidate == key)
    }

    /// 追加一个**新引用**的键值对（不查重；调用方先用"值相等"查过）。
    ///
    /// **作废查找索引**（改键了 ✓）；走 [`Self::insert_hashed`] 的调用方可以保住索引 ✓。
    pub fn insert_raw(&self, key: NonNull<Header>, value: NonNull<Header>) {
        self.bump_version();
        self.invalidate_index();
        self.entries.borrow_mut().push((key, value));
        self.hashes.borrow_mut().push(u64::MAX);
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
        self.remove_at(index)
    }

    /// 写入一个**新引用**的键值对；键已存在时替换值并交出旧值（调用方负责释放）。
    pub fn insert(
        &self,
        key: NonNull<Header>,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        self.invalidate_index();
        let mut entries = self.entries.borrow_mut();
        if let Some(slot) = entries
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
            .map(|slot| &mut slot.1)
        {
            return Some(core::mem::replace(slot, value));
        }
        self.bump_version();
        entries.push((key, value));
        self.hashes.borrow_mut().push(u64::MAX);
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

    /// **按位置取元素**（**借用**；越界给 `None`）——`deque[i]` 的下标读走它 ✓。
    pub fn item(&self, position: usize) -> Option<NonNull<Header>> {
        self.items.borrow().get(position).copied()
    }

    /// **按位置换元素**（**接管**新引用，返回被换下的旧值由调用方释放）——`deque[i] = v` 走它 ✓。
    pub fn replace(&self, position: usize, value: NonNull<Header>) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        let slot = items.get_mut(position)?;
        let old = *slot;
        *slot = value;
        Some(old)
    }

    /// `maxlen` 的**原始**值（`usize::MAX` ＝ 不限 ✓）。
    pub fn raw_maxlen(&self) -> usize {
        self.maxlen.get()
    }

    /// **设置 `maxlen` 并按需裁掉头部**（round 114）：`deque([1,2,3], maxlen=2)` 的关键字
    /// 在 `__init__` 里才拿到（`NewFn` 没有关键字一路 ✗）⇒ 设完要把超出的头部丢掉 ✓。
    pub fn set_raw_maxlen(&self, instance: &Instance, maxlen: usize) {
        self.maxlen.set(maxlen);
        let mut items = self.items.borrow_mut();
        while items.len() > maxlen {
            let dropped = items.remove(0);
            // SAFETY: dropped 由本对象持有；这份引用交还实例 ✓。
            unsafe { instance.release_object(dropped.as_ptr()) };
        }
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
    for (key, value) in object.entries() {
        unsafe {
            instance.release_object(key.as_ptr());
            instance.release_object(value.as_ptr());
        }
    }
    object.entries.borrow_mut().clear();
    object.hashes.borrow_mut().clear();
    object.deleted.set(0);
}
