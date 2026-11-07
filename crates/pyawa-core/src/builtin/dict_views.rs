//! **`dict` 视图**（round 115）：`dict.keys()`／`values()`／`items()` 交**视图对象** ✓
//! （先前交普通 `list` ✗ ⇒ `d.keys() | {...}` 全部 `unsupported operand type(s) for ^: 'list' and 'list'` ✗
//! —— `test_dict` 的 ~100 个错误 ✓）。
//!
//! 口径（照参照可观察行为）：
//! * `dict_keys`／`dict_items` 是**集合式**视图（`| & - ^` 与 `==` 交 `set` ✓）；
//! * `dict_values` 是**序列式**（只有迭代／len／contains／repr ✓）；
//! * 三者都有 `.mapping` 属性（交 `mappingproxy` ✓）；`repr` 给 `dict_keys([...])` ✓。
//!
//! **如实登记** ✗：视图是**快照**式实现（每次操作按当前字典内容重建 ✓；引用映射对象 ✓，
//! 但迭代器本身不检测字典变动 ✓ —— 官方用例不查这一档）。

use core::cell::Cell;
use crate::py_object;
use core::ptr::NonNull;

use crate::builtin_objects::{BuiltinFunctionObject, NativeFn};
use crate::executor::ExecError;
use crate::header::Header;
use crate::instance::Instance;
use crate::TypeObject;

py_object! {
    /// 字典视图：**映射对象**（本对象持一份引用 ✓）＋ 种类（0 keys／1 items／2 values ✓）。
    pub struct DictViewObject {
        mapping: NonNull<Header>,
        kind: Cell<u8>,
    }
}

impl DictViewObject {
    /// 见 [`TupleObject::slots`]：视图引用的字典可能指回视图（自引用 repr ✓）。
    pub fn slots() -> crate::type_object::Slots {
        crate::Slots::new(Self::dealloc)
            .with_traverse(view_traverse)
            .with_clear(view_clear)
            .with_repr(view_repr)
            .with_getattr(view_getattr)
    }

    /// 映射对象（借用）。
    pub fn mapping(&self) -> NonNull<Header> {
        self.mapping
    }

    /// 种类（0 keys／1 items／2 values）。
    pub fn kind(&self) -> u8 {
        self.kind.get()
    }
}

/// `OM-40`：视图持有映射那一份引用。
pub(crate) unsafe fn view_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<DictViewObject>() };
    visit(object.mapping().as_ptr());
}

/// `OM-20` ②：交还映射那一份引用。
pub(crate) unsafe fn view_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<DictViewObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.mapping().as_ptr()) };
}

fn make_native(instance: &Instance, name: &'static str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    instance
        .alloc(BuiltinFunctionObject::new(ty, name, Cell::new(handler)))
        .into_raw()
        .cast::<Header>()
}

/// 取（或惰性造）视图类型：`dict_keys`／`dict_items`／`dict_values` ✓。
fn view_type(instance: &Instance, kind: u8) -> Option<NonNull<TypeObject>> {
    let name = match kind {
        0 => "dict_keys",
        1 => "dict_items",
        _ => "dict_values",
    };
    if let Some(existing) = instance.type_named(name) {
        return Some(existing);
    }
    let ty = instance.alloc_type_raw(
        name,
        core::mem::size_of::<DictViewObject>(),
        DictViewObject::slots(),
    );
    if let Some(object_type) = instance.type_named("object") {
        let _ = instance.register_bases(ty, vec![object_type]);
    }
    let namespace = instance.type_namespace(ty.cast())?;
    let module = instance.new_str("builtins");
    instance.dict_set(namespace, "__module__", module);
    instance.release(module);
    let mut methods: Vec<(&'static str, NativeFn)> = vec![
        ("__len__", view_len_native),
        ("__iter__", view_iter_native),
        ("__contains__", view_contains_native),
        ("__repr__", view_repr_native_for_python),
        ("__eq__", view_eq_native),
    ];
    if kind != 2 {
        // **集合式运算**（`dict_values` 不做 ✓，与参照一致）。
        let binary: [(&'static str, NativeFn); 8] = [
            ("__or__", view_or_native),
            ("__ror__", view_ror_native),
            ("__and__", view_and_native),
            ("__rand__", view_rand_native),
            ("__sub__", view_sub_native),
            ("__rsub__", view_rsub_native),
            ("__xor__", view_xor_native),
            ("__rxor__", view_rxor_native),
        ];
        methods.extend(binary);
    }
    for (name, handler) in methods {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
        instance.release(function);
    }
    Some(ty)
}

/// **造一个视图**（`dict.keys()` 一族用 ✓；新引用）。
pub(crate) fn make_view(
    instance: &Instance,
    mapping: NonNull<Header>,
    kind: u8,
) -> Result<NonNull<Header>, ExecError> {
    let Some(ty) = view_type(instance, kind) else {
        return Err(instance.raise_builtin_error("RuntimeError", "dict 视图类型建不出来"));
    };
    // 视图要自己那一份映射引用 ✓。
    // SAFETY: mapping 由调用方保证存活。
    unsafe { instance.incref_object(mapping.as_ptr()) };
    Ok(instance
        .alloc(DictViewObject::new(ty, mapping, Cell::new(kind)))
        .into_raw()
        .cast::<Header>())
}

/// 视图的元素（**新引用**）：keys ⇒ 键；items ⇒ `(键, 值)` 元组；values ⇒ 值 ✓。
fn view_elements(instance: &Instance, view: NonNull<Header>) -> Result<Vec<NonNull<Header>>, ExecError> {
    // SAFETY: 调用方保证 view 是本类型的存活对象。
    let object = unsafe { &*view.as_ptr().cast::<DictViewObject>() };
    let entries = instance
        .dict_entries(object.mapping())
        .unwrap_or_default();
    Ok(entries
        .into_iter()
        .map(|(key, value)| match object.kind() {
            0 => instance.retain(key),
            1 => instance.new_tuple(vec![instance.retain(key), instance.retain(value)]),
            _ => instance.retain(value),
        })
        .collect())
}

fn receiver(bound: Option<NonNull<Header>>, args: &[NonNull<Header>]) -> Option<NonNull<Header>> {
    bound.or_else(|| args.first().copied())
}

fn view_len_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(view) = receiver(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "len() 需要 dict 视图"));
    };
    // SAFETY: 同上。
    let object = unsafe { &*view.as_ptr().cast::<DictViewObject>() };
    let length = instance
        .dict_entries(object.mapping())
        .map(|entries| entries.len())
        .unwrap_or(0);
    Ok(instance.new_int(length as i64))
}

fn view_iter_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(view) = receiver(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "iter() 需要 dict 视图"));
    };
    // SAFETY: 同上。
    let object = unsafe { &*view.as_ptr().cast::<DictViewObject>() };
    if object.kind() == 0 {
        // 键：直接用字典键迭代器 ✓（活体 ✓）。
        return instance.iter_object(object.mapping());
    }
    let elements = view_elements(instance, view)?;
    let snapshot = instance.new_list(elements);
    let iterator = instance.iter_object(snapshot);
    instance.release(snapshot);
    iterator
}

fn view_contains_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(view) = receiver(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "in 需要 dict 视图"));
    };
    let Some(item) = args.last().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "in 需要 1 个实参"));
    };
    // SAFETY: 同上。
    let object = unsafe { &*view.as_ptr().cast::<DictViewObject>() };
    if object.kind() == 0 {
        return Ok(instance.new_bool(crate::executor::iter::contains(
            instance,
            object.mapping(),
            item,
            0,
        )?));
    }
    let elements = view_elements(instance, view)?;
    let mut found = false;
    for element in elements {
        let equal = if object.kind() == 1 {
            crate::executor::protocol::equality_public(instance, item, element)?
        } else {
            crate::executor::values_equal_public(instance, item, element)
        };
        instance.release(element);
        if equal {
            found = true;
            break;
        }
    }
    Ok(instance.new_bool(found))
}

/// 把任何操作数转成 `set`（视图／集合／可迭代 ✓；返回**新引用**）。
fn operand_to_set(instance: &Instance, value: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    let ty = instance.type_of(value);
    let is_set = instance
        .type_named("set")
        .is_some_and(|base| instance.is_subtype(ty, base))
        || Some(ty) == instance.type_named("frozenset");
    if is_set {
        return Ok(instance.retain(value));
    }
    if instance
        .type_named("dict_keys")
        .is_some_and(|base| instance.is_subtype(ty, base))
        || instance
            .type_named("dict_items")
            .is_some_and(|base| instance.is_subtype(ty, base))
    {
        let elements = view_elements(instance, value)?;
        return Ok(instance.new_set(elements));
    }
    // 其余：交给 `set(可迭代)` 构造 ✓（类型对象当可调用；`call_value` 借用实参 ✓）。
    let Some(set_type) = instance.type_named("set") else {
        return Err(instance.raise_builtin_error("RuntimeError", "set 未登记"));
    };
    let result = pyawa_core_call_value(instance, set_type.cast::<Header>(), &[value])?;
    Ok(result)
}

/// 本文件内的小包装（避免到处写长路径）。
fn pyawa_core_call_value(
    instance: &Instance,
    callable: NonNull<Header>,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    crate::executor::call::call_value(instance, callable, args, &[])
}

fn view_set(instance: &Instance, view: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    let elements = view_elements(instance, view)?;
    Ok(instance.new_set(elements))
}

fn view_binary(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    method: &str,
    reverse: bool,
) -> Result<NonNull<Header>, ExecError> {
    let Some(view) = receiver(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "集合运算需要 dict 视图"));
    };
    let Some(other) = args.last().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "集合运算需要 1 个实参"));
    };
    let (left, right) = if reverse {
        (operand_to_set(instance, other)?, view_set(instance, view)?)
    } else {
        (view_set(instance, view)?, operand_to_set(instance, other)?)
    };
    let result = set_binary(instance, left, right, method);
    instance.release(left);
    instance.release(right);
    result
}

/// 两个 `set` 的运算（**新引用**）：直接调 `set` 族的 native ✓（`union`／`intersection`／
/// `difference` 现成 ✓；`symmetric_difference` 本层没有 ⇒ 这里按集合成员关系算 ✓）。
fn set_binary(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    method: &str,
) -> Result<NonNull<Header>, ExecError> {
    if method == "symmetric_difference" {
        let mut items: Vec<NonNull<Header>> = Vec::new();
        for (source, exclude) in [(left, right), (right, left)] {
            let Some(entries) = instance.set_items(source) else {
                return Err(instance.raise_builtin_error("TypeError", "集合运算需要 set"));
            };
            for item in entries {
                let present = crate::executor::iter::contains(instance, exclude, item, 0)?;
                if !present {
                    items.push(instance.retain(item));
                }
            }
        }
        return Ok(instance.new_set(items));
    }
    let handler: NativeFn = match method {
        "union" => crate::builtin::set::set_union_native,
        "intersection" => crate::builtin::set::set_intersection_native,
        "difference" => crate::builtin::set::set_difference_native,
        _ => return Err(instance.raise_builtin_error("TypeError", "未知集合运算")),
    };
    let native = make_native(instance, "set_op", handler);
    // 实参所有权**转移**给调用 ✓；可调用对象借用 ✓。
    // SAFETY: right 由调用方保证存活。
    unsafe { instance.incref_object(right.as_ptr()) };
    let result = crate::executor::call::call_callable(
        instance,
        native,
        Some(left),
        vec![right],
        Vec::new(),
        0,
    );
    instance.release(native);
    result
}

fn view_or_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    view_binary(instance, bound, args, "union", false)
}
fn view_ror_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    view_binary(instance, bound, args, "union", true)
}
fn view_and_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    view_binary(instance, bound, args, "intersection", false)
}
fn view_rand_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    view_binary(instance, bound, args, "intersection", true)
}
fn view_sub_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    view_binary(instance, bound, args, "difference", false)
}
fn view_rsub_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    view_binary(instance, bound, args, "difference", true)
}
fn view_xor_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    view_binary(instance, bound, args, "symmetric_difference", false)
}
fn view_rxor_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    view_binary(instance, bound, args, "symmetric_difference", true)
}

fn view_eq_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(view) = receiver(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "== 需要 dict 视图"));
    };
    let Some(other) = args.last().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "== 需要 1 个实参"));
    };
    let left = view_set(instance, view)?;
    let right = operand_to_set(instance, other)?;
    let equal = crate::executor::protocol::equality_public(instance, left, right);
    instance.release(left);
    instance.release(right);
    equal.map(|value| instance.new_bool(value))
}

/// `view.__repr__()`（Python 面）：与槽位同文本 ✓。
fn view_repr_native_for_python(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(view) = receiver(bound, args) else {
        return Err(instance.raise_builtin_error("TypeError", "repr() 需要 dict 视图"));
    };
    // SAFETY: 同上。
    let text = unsafe { view_repr(view.as_ptr(), instance) }?;
    Ok(instance.new_str(&text))
}

/// `repr(d.keys())` ⇒ `dict_keys([...])` ✓（items 的元组照 repr ✓）。
pub(crate) unsafe fn view_repr(
    ptr: *mut Header,
    instance: &Instance,
) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型族的存活对象。
    let view = unsafe { NonNull::new_unchecked(ptr) };
    let type_name = instance.type_name(instance.type_of(view));
    let elements = view_elements(instance, view)?;
    let mut rendered: Vec<String> = Vec::with_capacity(elements.len());
    for element in elements {
        rendered.push(instance.object_repr(element)?);
        instance.release(element);
    }
    Ok(format!("{type_name}([{}])", rendered.join(", ")))
}

/// **属性面**：`mapping` 属性交 `mappingproxy` ✓（参照的 `.mapping` ✓；`test_views_mapping` ✓）。
pub(crate) unsafe fn view_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if name != "mapping" {
        return None;
    }
    // SAFETY: 契约同其它 getattr 槽。
    let object = unsafe { &*ptr.cast::<DictViewObject>() };
    instance.mappingproxy_of(object.mapping())
}
