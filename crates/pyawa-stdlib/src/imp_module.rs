//! `_imp` 模块（契约 `docs/SPEC-c-modules.md` §5.2.4；`CM-14` 顺序里紧接 `sys` 的那个）。
//!
//! 本层只落地**真正能落地**的两件（其余逐条记在 §5.2.4 的"未落地"里，**不伪造**）：
//!
//! - **`pyc_magic_number_token`**（`SPEC-c-modules.md` §6 的硬义务）：`_bootstrap_external.py`
//!   用它算 `MAGIC_NUMBER`（取**低 16 位**）。**值由 Pyawa 自定**——本层取 `.pyac` 自己的标识
//!   （`PYAWAC\0\0` 的 `PYAW` 部分按小端读成整数），**不冒用**参照实现的 `0xa0d0e2b`
//! - **`is_builtin(name)`**：并入模块表之前**一律 `0`**（还没有可导入的模块 ⇒ `CM-6` 的
//!   "未提供"口径）；参数不是 `str` 时照参照实测的消息报 `TypeError`
//!
//! 本模块**不碰**平台（`CX-4`：stdlib 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};

/// 模块名（`_imp`）。
pub const NAME: &str = "_imp";

/// 模块的 `__doc__`（与参照实现同源的一句话）。
pub const DOC: &str = "(Extremely) low-level import machinery bits as used by importlib.";

/// **自定**的 `pyc_magic_number_token`。
///
/// 取 `.pyac` 的 `MAGIC`（`pyawa-runtime` 的 `PYAWAC\0\0`）里 `PYAW` 那四个字节按小端读成整数：
/// `0x5741_5950`。低 16 位（`0x5950`）稳定 ⇒ 满足 `_bootstrap_external` 的用法；
/// 与参照实现的 `0xa0d0e2b` **必然不同**（不得冒用）。
pub const PYC_MAGIC_NUMBER_TOKEN: i64 = 0x5741_5950;

/// **`is_builtin(name)`**：并入模块表之前一律 `0`。
///
/// 参照实现是三态（`-1` ＝ 内建、`0` ＝ 不是、`1` ＝ 本应内建却不在表里）；本层还没有
/// 可导入的模块 ⇒ 全部按"不是"回答（`CM-6` 的"未提供"口径），等 `P3-12` 的模块表落地再照表给。
fn is_builtin_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("is_builtin() takes exactly one argument ({} given)", args.len()),
        ));
    }
    // 参数必须是 `str`：消息照参照**实测**（`is_builtin() argument must be str, not int`）
    if instance.text_value(args[0]).is_none() {
        let name = instance.type_name(instance.type_of(args[0]));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("is_builtin() argument must be str, not {name}"),
        ));
    }
    Ok(instance.new_int(0))
}

/// 造一个原生可调用对象（**新引用**；与 `builtins_module`／`sys_module` 同一做法）。
fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
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

/// 建 `_imp` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let token = instance.new_int(PYC_MAGIC_NUMBER_TOKEN);
    instance.dict_set(namespace, "pyc_magic_number_token", token);
    let is_builtin = make_native(instance, "is_builtin", is_builtin_native);
    instance.dict_set(namespace, "is_builtin", is_builtin);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
