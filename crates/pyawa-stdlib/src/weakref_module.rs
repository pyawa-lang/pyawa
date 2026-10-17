//! `_weakref` 模块（第 173 轮）：**最小面** = `ref` ✓。
//!
//! **如实登记的偏差** ✗：本层**没有真正的弱引用**（GC 不支持 ✓）⇒ `ref(x)` 存的是**强引用** ✓
//! ⇒ 目标不会被回收 ✓、`ref(x, 回调)` 的**回调被忽略** ✓。对「把 `Lib/abc.py`／`os.py` 跑起来」这一步够用 ✓。
//!
//! `CX-4`：本 crate 不碰平台 ⇒ 这里不 import 任何平台接口 ✓。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`_weakref`）。
pub const NAME: &str = "_weakref";


/// 造一个原生可调用对象（**新引用**；与 `weakref`／`operator` 两个模块同一做法）。
fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// **`proxy(obj, callback=None)`**（第 309 轮）：本层**没有真正的弱引用**（GC 不支持 ✓）⇒
/// 直接**交回目标对象本身** ✓（代理本就"转发一切" ✓ ⇒ 属性访问、调用、下标都与目标一致 ✓）。
///
/// **如实登记的偏差** ✗：`proxy(obj) is obj` 在参照里是 `False` ✓、本层是 `True` ✓；
/// `type(proxy(obj))` 在参照里是 `ProxyType` ✓、本层是目标的类型 ✓。与
/// `ref(x)` 存**强引用**那条偏差同源 ✓，都为"把 `Lib/` 跑起来"这一步服务 ✓。
/// 动因：上限诊断里 `ImportError: cannot import name 'proxy' from '_weakref'` × **31** 个模块 ✓。
fn proxy_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(target) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "proxy expected at least 1 argument, got 0",
        ));
    };
    Ok(instance.retain(*target))
}

/// `getweakrefcount(obj)`：本层没有真正的弱引用 ⇒ 恒 **0** ✓（如实 ✓）。
fn getweakrefcount_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.new_int(0))
}

/// `getweakrefs(obj)`：同上 ⇒ 恒**空列表** ✓。
fn getweakrefs_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.new_list(Vec::new()))
}

/// `_remove_dead_weakref(dict, key)`：本层没有死引用 ⇒ **什么都不做** ✓（返回 `None` ✓）。
fn remove_dead_weakref_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}

/// 建 `_weakref` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // **`ref` 就是那个类型对象** ✓（构造器在它的 `new` 槽里 ✓）—— 与 `slice`／`object` 同一手法 ✓。
    // **先 `retain` 再交给字典** ✓（`dict_set` 接管一份引用 ✓ —— 第 161 轮的堆损坏就是这么来的 ✗）。
    if let Some(weakref_type) = instance.type_named("weakref") {
        instance.retain(weakref_type.cast());
        instance.dict_set(namespace, "ref", weakref_type.cast());
    }
    // **`ProxyType`／`CallableProxyType`／`ReferenceType`**：本层没有真正的代理对象 ⇒ 一律给
    // **`weakref` 类型** ✓（`weakref.py` 只把它们放成元组做 `isinstance` 一类的事 ✓；如实登记 ✓）。
    for name in ["ProxyType", "CallableProxyType", "ReferenceType"] {
        if let Some(weakref_type) = instance.type_named("weakref") {
            instance.retain(weakref_type.cast());
            instance.dict_set(namespace, name, weakref_type.cast());
        }
    }
    // **四个函数**（第 309 轮）：`proxy` 是上限榜上 31 个模块的第一卡点 ✓；其余三个按"没有弱引用"
    // 那套近似给常量结果 ✓（如实 ✓）。
    for (name, handler) in [
        ("proxy", proxy_native as NativeFn),
        ("getweakrefcount", getweakrefcount_native as NativeFn),
        ("getweakrefs", getweakrefs_native as NativeFn),
        ("_remove_dead_weakref", remove_dead_weakref_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    let exports = instance.new_list(vec![
        instance.new_str("ref"),
        instance.new_str("proxy"),
        instance.new_str("getweakrefcount"),
        instance.new_str("getweakrefs"),
        instance.new_str("ProxyType"),
        instance.new_str("CallableProxyType"),
        instance.new_str("ReferenceType"),
    ]);
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
