//! `_collections` 模块（`SPEC-c-modules.md`；第 331 轮）。
//!
//! **本段落地**：`deque` ✓ —— 它是 `Lib/collections/__init__.py` 的
//! `from _collections import deque, ...` 那一行要的名字 ✓，也是上限榜上
//! `ImportError: cannot import name 'deque' from 'collections'` × **18** 个模块的卡点 ✓。
//!
//! 类型本身（载荷、方法面、`repr`、`len`）都在**核心** ✓（要看容器内部 ✓ —— 与 `list`／`set` 同一口径 ✓），
//! 本模块只把类型对象**导出**成 `deque` 这个名字 ✓（`CX-4`：stdlib 不碰平台 ✓）。
//!
//! **如实登记的未接面** ✗：`deque` 的迭代协议／下标／`__contains__`／`__eq__`／`reverse`（随后补 ✓）；
//! `_collections` 里 `collections/__init__.py` 还要的 `_count_elements`／`_tuplegetter` 等名字 ✗
//! —— 那些等真正撞上再补 ✓（当前先让 `deque` 这一行能过 ✓）。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use pyawa_core::{AttributeObject, BuiltinFunctionObject, ExecError, Header, Instance, NativeFn};

/// 模块名（`_collections`）。
pub const NAME: &str = "_collections";

/// 类型名（`_collections._tuplegetter` ✓ —— `type(namedtuple 字段).__name__` 照参照 ✓）。
const TUPLEGETTER: &str = "_tuplegetter";

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法 ✓）。
fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 取／建 `_collections._tuplegetter` 类型 ✓（幂等 ✓）—— 属性类型 ＋ 一条 `__get__` ✓。
fn ensure_tuplegetter_type(instance: &Instance) -> Option<NonNull<pyawa_core::TypeObject>> {
    let ty = match instance.type_named(TUPLEGETTER) {
        Some(existing) => existing,
        None => instance.new_attribute_type(TUPLEGETTER),
    };
    if let Some(namespace) = instance.type_namespace(ty.cast()) {
        if instance.dict_get(namespace, "__get__").is_none() {
            let function = make_native(instance, "__get__", tuplegetter_get as NativeFn);
            instance.dict_set(namespace, "__get__", function);
        }
    }
    Some(ty)
}

/// **`_tuplegetter(index, doc='')`** ✓（task-14 ✓；`Lib/collections/__init__.py:357` 的裸 import ✓）。
///
/// 参照里它是 `tuple` 的**只读下标描述符** ✓：`Point.x` ⇒ 描述符本身 ✓、`Point(1,2).x` ⇒ `obj[index]` ✓。
/// 这里**不新增载荷类型** ✗：`index`／`__doc__` 放实例字典 ✓（`AttributeObject` ✓，与 `_io` 占位类型同法 ✓）。
fn tuplegetter_new(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(index) = args.first().and_then(|value| instance.int_value(*value)) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_tuplegetter 的第一个实参要是整数下标",
        ));
    };
    let doc = args
        .get(1)
        .and_then(|value| instance.text_of(*value))
        .unwrap_or("")
        .to_owned();
    let ty = ensure_tuplegetter_type(instance)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "_tuplegetter 建类失败"))?;
    let attributes = instance.new_dict();
    let index_value = instance.new_int(index);
    instance.dict_set(attributes, "_index", index_value);
    let doc_value = instance.new_str(&doc);
    instance.dict_set(attributes, "__doc__", doc_value);
    let object = instance.alloc(AttributeObject::new(ty, RefCell::new(Some(attributes))));
    Ok(object.into_raw().cast::<Header>())
}

/// **`_tuplegetter.__get__(obj, type)`** ✓：`obj` 是 `None` ⇒ 交回**描述符本身** ✓；
/// 否则按 `obj[index]` 取 ✓（与 `obj[key]` **同一处实现** ✓：`subscript_read` ✓）。
fn tuplegetter_get(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(descriptor) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "_tuplegetter.__get__ 缺少 self"));
    };
    let target = args.first().copied();
    let class_level = match target {
        Some(value) => value == instance.singletons().none(),
        None => true,
    };
    if class_level {
        return Ok(instance.retain(descriptor));
    }
    let index = instance
        .attribute_optional_of(descriptor, "_index")?
        .and_then(|value| instance.int_value(value))
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "_tuplegetter 的下标丢了"))?;
    let target = target.expect("上面判过 `obj` 不是 `None`");
    let key = instance.new_int(index);
    let result = pyawa_core::executor::subscript::subscript_read(instance, target, key);
    instance.release(key);
    result
}

/// 建 `_collections` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // **`deque` 就是那个类型对象** ✓（构造器在它的 `new` 槽里 ✓ —— 与 `slice`／`weakref` 同一手法 ✓）。
    if let Some(deque_type) = instance.type_named("deque") {
        instance.retain(deque_type.cast());
        instance.dict_set(namespace, "deque", deque_type.cast());
    }
    // **`_tuplegetter`** ✓（task-14）：`collections.namedtuple` 的字段描述符 ✓（裸 `from _collections import` ✓）。
    let getter = make_native(instance, "_tuplegetter", tuplegetter_new as NativeFn);
    instance.dict_set(namespace, "_tuplegetter", getter);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
