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

/// **`is_builtin(name)`**：三态 —— `-1` ＝ 是内建模块、`0` ＝ 不是、`1` ＝ "本应内建却不在表里" ✓。
///
/// **第 280 轮据实接线** ✓（`SPEC-c-modules.md` §5.2.4 原文就写着"等 `P3-12` 的模块表落地后再照表给
/// `-1`／`1`" ✓）：并入模块表之后按表回答 ✓ ——
/// ① 不在模块表里 ⇒ `0`；② 在表里、也列在 `sys.builtin_module_names` ⇒ `-1`；③ 在表里、没列 ⇒ `1`。
/// （本层目前**没有**第三态：表就是那张名字清单 ✓ —— 留着这条分支是为了表与清单将来能分开 ✓。）
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
    let Some(name) = instance.text_value(args[0]) else {
        let name = instance.type_name(instance.type_of(args[0]));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("is_builtin() argument must be str, not {name}"),
        ));
    };
    let Some(modules) = instance.modules() else {
        // 模块表还没装配（裸 `Instance::new()`）⇒ 照"未提供"口径给 `0` ✓
        return Ok(instance.new_int(0));
    };
    if instance.dict_get(modules, &name).is_none() {
        return Ok(instance.new_int(0));
    }
    let verdict = if builtin_module_names_contains(instance, modules, &name) {
        -1
    } else {
        1
    };
    Ok(instance.new_int(verdict))
}

/// `name` 在不在 `sys.builtin_module_names` 里（读的就是 `sys` 模块**那一份**元组 ✓，一处真相 ✓）。
fn builtin_module_names_contains(
    instance: &Instance,
    modules: NonNull<Header>,
    name: &str,
) -> bool {
    let Some(sys_module) = instance.dict_get(modules, "sys") else {
        return false;
    };
    let Some(namespace) = pyawa_core::mounted_instance_dict(instance, sys_module) else {
        return false;
    };
    let Some(listed) = instance.dict_get(namespace, "builtin_module_names") else {
        return false;
    };
    if instance.type_name(instance.type_of(listed)) != "tuple" {
        return false;
    }
    let items = instance.tuple_items(listed).unwrap_or_default();
    items
        .into_iter()
        .any(|item| instance.text_value(item).as_deref() == Some(name))
}

/// **`is_frozen(name)`**（第 280 轮）：本层**没有**冻结模块 ⇒ 一律 `0`（`False`）✓。
///
/// 为什么这是**诚实**的而不是"未实现" ✗：冻结表（`IM-27`／`IM-33`／`IM-34`）还没做 ✓，而"我们一个
/// 冻结模块也没有"是**确定的事实** ✓ ⇒ 参照在同样情形也返回 `False` ✓（`_bootstrap._setup` 就是靠
/// 它把"非内建、非冻结"的模块 `continue` 掉 ✓）。
fn is_frozen_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("is_frozen() takes exactly one argument ({} given)", args.len()),
        ));
    }
    if instance.text_value(args[0]).is_none() {
        let name = instance.type_name(instance.type_of(args[0]));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("is_frozen() argument must be str, not {name}"),
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

/// **`extension_suffixes()`**（第 280 轮）：**空列表** ✓。
///
/// 为什么是**空**而不是"未实现" ✗：本层**不支持原生的扩展模块**（动态加载的 `.so` 一类要平台，
/// 而 Pyawa 的产物是 `.pyac` ✓）⇒ "一个扩展后缀也没有"是**确定的事实** ✓ ⇒ 空列表正是它的表示 ✓。
/// 参照实现给四个后缀（`['.cpython-314-….so', '.abi3.so', '.abi3-….so', '.so']` ✓）—— 那个**必须不同** ✓
/// （我们本来就不该认那些文件 ✓）。`_bootstrap_external` 用它算 `EXTENSION_SUFFIXES` ✓，
/// 空表只是让"扩展加载器"那一族为空 ✓（`CM-6`：没有的东西不冒充 ✓）。
fn extension_suffixes_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.new_list(Vec::new()))
}

/// 建 `_imp` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let token = instance.new_int(PYC_MAGIC_NUMBER_TOKEN);
    instance.dict_set(namespace, "pyc_magic_number_token", token);
    let is_builtin = make_native(instance, "is_builtin", is_builtin_native);
    instance.dict_set(namespace, "is_builtin", is_builtin);
    // **`is_frozen`**（第 280 轮）：`_bootstrap._setup` 会对每个非内建模块调它 ✓ —— 少了它，
    // `importlib` 一进门就 `AttributeError: module 没有 is_frozen` ✗。
    let is_frozen = make_native(instance, "is_frozen", is_frozen_native);
    instance.dict_set(namespace, "is_frozen", is_frozen);
    // **`extension_suffixes`**（第 280 轮）：`_bootstrap_external.py:233` 在**模块级**就调它 ✓
    // ⇒ 少了它，`import importlib` 停在 `'module' object has no attribute 'extension_suffixes'` ✗。
    let extension_suffixes = make_native(
        instance,
        "extension_suffixes",
        extension_suffixes_native,
    );
    instance.dict_set(namespace, "extension_suffixes", extension_suffixes);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
