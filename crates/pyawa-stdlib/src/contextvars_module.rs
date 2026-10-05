//! `_contextvars` 模块（`SPEC-c-modules.md`；第 332 轮）。
//!
//! **本段落地**：`ContextVar`／`Token`／`Context` 三个类型 ✓ ＋ `copy_context()` ✓ ——
//! `Lib/contextvars.py` 只做
//! `from _contextvars import Context, ContextVar, Token, copy_context` ＋
//! `_collections_abc.Mapping.register(Context)` ✓ ⇒ 这一组名字就够它 import ✓。
//! 上限榜上 `ModuleNotFoundError: No module named '_contextvars'` × **49** 个模块的卡点 ✓。
//!
//! 类型与语义都在**核心** ✓（要看对象内部 ✓ —— 与 `deque`／`set` 同一口径 ✓），
//! 本模块只把它们**导出** ✓（`CX-4`：stdlib 不碰平台 ✓）。
//!
//! **如实登记的偏差** ✗：本层**没有真正的上下文隔离**（任务／线程局部状态尚未接线 ✓）——
//! 值存在**变量自己**身上 ✓；`Context` 不承载独立状态（`run` 直接在全局上跑 ✓）。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`_contextvars`）。
pub const NAME: &str = "_contextvars";

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
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

/// `copy_context()` ✓。
fn copy_context_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(pyawa_core::copy_context_value(instance))
}

/// 建 `_contextvars` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // **三个类型对象** ✓（构造器在各自的 `new` 槽里 ✓ —— 与 `deque`／`slice` 同一手法 ✓）
    for name in ["ContextVar", "Token", "Context"] {
        if let Some(value) = instance.type_named(name) {
            instance.retain(value.cast());
            instance.dict_set(namespace, name, value.cast());
        }
    }
    let function = make_native(instance, "copy_context", copy_context_native as NativeFn);
    instance.dict_set(namespace, "copy_context", function);
    let exports = instance.new_list(vec![
        instance.new_str("Context"),
        instance.new_str("ContextVar"),
        instance.new_str("Token"),
        instance.new_str("copy_context"),
    ]);
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
