//! **`_typing` 模块装配**（task-24 轮 1 ✓）：参照里它是 **C 模块** ✗（`Lib/_typing.py` 不存在 ✓）
//! ⇒ 类型与语义在 **core**（`builtin/typing.rs` ✓，那边能用 `alloc_type_raw` 造带槽的类型 ✓），
//! 这里只做**命名空间装配**（与 `sys`／`atexit` 同款 ✓）。
//!
//! 名字表照参照 `dir(_typing)` ✓：`Generic`／`NoDefault`／`ParamSpec`／`ParamSpecArgs`／
//! `ParamSpecKwargs`／`TypeAliasType`／`TypeVar`／`TypeVarTuple`／`Union`／`_idfunc` ✓。
//! **如实范围** ✗：除 `TypeVar`／`NoDefault`／`NoDefaultType`／`_idfunc` 外，其余名字这一轮只**立住**
//! （普通类型占位 ✓），语义在轮 3 的 `Lib/typing.py` 那条链上补 ✓。

use core::ptr::NonNull;
use pyawa_core::{Header, Instance};

/// 模块名（`sys.modules` 里的键 ✓）。
pub const NAME: &str = "_typing";

pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    instance.release(module_name);
    let doc = instance.new_str("_typing 最小面（task-24 轮 1）：TypeVar／NoDefault／_idfunc ＋ 占位名。");
    instance.dict_set(namespace, "__doc__", doc);
    instance.release(doc);
    // **两个真类型** ✓（引导期带槽建出来的 ✓；类型对象由注册表持有 ⇒ 直接放、不另加引用 ✓）。
    if let Some(typevar) = instance.type_named("TypeVar") {
        instance.dict_set(namespace, "TypeVar", typevar.cast());
    }
    // **`NoDefault` 单例** ✓（新引用 ⇒ 放完归还 ✓）。**注意**：参照**不**导出 `NoDefaultType` ✗
    // （`dir(_typing)` 里没有它 ✓）⇒ 这里也不导出 ✓；类型仍可经 `type(NoDefault)` 拿到 ✓。
    let marker = pyawa_core::no_default_singleton(instance);
    instance.dict_set(namespace, "NoDefault", marker);
    instance.release(marker);
    // **`Generic`** ✓：普通类 ✓（`Cannot inherit from plain Generic` 那条在 `typing.py` ✓，轮 3 ✓）。
    let generic = instance.new_attribute_type("Generic");
    instance.dict_set(namespace, "Generic", generic.cast());
    // **占位名** ✓（照参照 `dir(_typing)` ✓）。
    for name in pyawa_core::PLACEHOLDER_NAMES {
        let placeholder = pyawa_core::placeholder_type(instance, name);
        instance.dict_set(namespace, name, placeholder.cast());
    }
    // **`_idfunc`** ✓（恒等 ✓）。
    let idfunc = pyawa_core::make_function(instance, "_idfunc", pyawa_core::idfunc_native);
    instance.dict_set(namespace, "_idfunc", idfunc);
    instance.release(idfunc);
    namespace
}
