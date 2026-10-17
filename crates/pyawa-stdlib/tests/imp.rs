//! `_imp` 的契约测试（`SPEC-c-modules.md` §5.2.4）。
//!
//! 两件已落地的：`pyc_magic_number_token`（**自定值**，与参照**必须不同**）与 `is_builtin`
//! （并入模块表之前一律 `0`）。其余逐条列在 §5.2.4 的"未落地"里。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};
use pyawa_stdlib::imp_module;

#[path = "fixtures/imp.rs"]
mod fixture;

use fixture::{
    REFERENCE_DOC, REFERENCE_IS_BUILTIN_SYS, REFERENCE_IS_BUILTIN_UNKNOWN,
    REFERENCE_MAGIC_LOW_16, REFERENCE_NAMES, REFERENCE_PYC_MAGIC_NUMBER_TOKEN,
};

/// 取命名空间里的一项（新引用在调用方）。
fn attribute(instance: &Instance, namespace: NonNull<Header>, name: &str) -> NonNull<Header> {
    instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("§5.2.4：`_imp.{name}` 必须存在"))
}

/// 调一个原生函数（与 `sys.rs` 的写法一致：直接取 handler）。
fn call(
    instance: &Instance,
    function: NonNull<Header>,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    // SAFETY: function 是本实例里存活的原生可调用对象。
    let handler = unsafe {
        (*function
            .as_ptr()
            .cast::<pyawa_core::BuiltinFunctionObject>())
        .function()
    };
    // SAFETY: 实参都是调用方持有的引用（handler 只借用）。
    unsafe { handler(instance, None, args, &[]) }
}

#[test]
fn the_magic_token_is_ours_not_the_references() {
    // `SPEC-c-modules.md` §6：**必须**提供 `pyc_magic_number_token`，**其值由 Pyawa 自定**
    let instance = Instance::new();
    let namespace = imp_module::build(&instance);
    let token = attribute(&instance, namespace, "pyc_magic_number_token");
    let value = instance.int_value(token).expect("必须是整数");
    assert_eq!(value, imp_module::PYC_MAGIC_NUMBER_TOKEN);
    assert_ne!(
        value, REFERENCE_PYC_MAGIC_NUMBER_TOKEN,
        "不得冒用参照实现的 token"
    );
    // 低 16 位稳定（`_bootstrap_external` 用它算 `MAGIC_NUMBER`）——自定值也该有稳定低位
    assert_eq!(value & 0xFFFF, imp_module::PYC_MAGIC_NUMBER_TOKEN & 0xFFFF);
    assert_ne!(value & 0xFFFF, REFERENCE_MAGIC_LOW_16);
    // 代码段里那套自定标识：`PYAW` 四个字节按小端读
    assert_eq!(value, i64::from(u32::from_le_bytes(*b"PYAW")));

    assert_eq!(instance.text_value(attribute(&instance, namespace, "__name__")).as_deref(), Some("_imp"));
    assert_eq!(
        instance.text_value(attribute(&instance, namespace, "__doc__")).as_deref(),
        Some(imp_module::DOC)
    );
    // 与参照的 `__doc__` 同源（同一条句子）
    assert_eq!(imp_module::DOC, REFERENCE_DOC);
}

#[test]
fn is_builtin_follows_the_module_table() {
    // ① 模块表**没装配**（裸 `Instance::new()`）⇒ 照"未提供"口径给 `0` ✓
    let bare = Instance::new();
    let namespace = imp_module::build(&bare);
    let is_builtin = attribute(&bare, namespace, "is_builtin");
    let sys_name = bare.new_str("sys");
    let result = call(&bare, is_builtin, &[sys_name]).expect("一个 str 实参应当成功");
    assert_eq!(bare.int_value(result), Some(0), "表没装配 ⇒ 0");

    // ② 表装配之后 ⇒ **照表**给 `-1`／`0`（第 280 轮；§5.2.4 原文要求"表落地后照表给" ✓）
    let instance = Instance::new();
    pyawa_stdlib::install(&instance, "[test]", &[]);
    let namespace = imp_module::build(&instance);
    let is_builtin = attribute(&instance, namespace, "is_builtin");
    for name in ["sys", "_imp", "_thread", "_warnings", "_weakref", "_io", "posix"] {
        let argument = instance.new_str(name);
        let result = call(&instance, is_builtin, &[argument]).expect("一个 str 实参应当成功");
        assert_eq!(
            instance.int_value(result),
            Some(REFERENCE_IS_BUILTIN_SYS),
            "`{name}` 在表里、也列在 `sys.builtin_module_names` ⇒ `-1`"
        );
    }
    let unknown = instance.new_str("nope");
    let result = call(&instance, is_builtin, &[unknown]).expect("未知名字也成功");
    assert_eq!(
        instance.int_value(result),
        Some(REFERENCE_IS_BUILTIN_UNKNOWN),
        "未知名字与参照一致：0"
    );
}

#[test]
fn is_frozen_is_false_and_extension_suffixes_is_empty() {
    // 第 280 轮：这两条是 `importlib` 的引导路径**进门就要**的（`_setup` 与 `_bootstrap_external:233`）
    let instance = Instance::new();
    pyawa_stdlib::install(&instance, "[test]", &[]);
    let namespace = imp_module::build(&instance);

    let is_frozen = attribute(&instance, namespace, "is_frozen");
    let name = instance.new_str("importlib._bootstrap");
    let result = call(&instance, is_frozen, &[name]).expect("一个 str 实参应当成功");
    assert_eq!(instance.int_value(result), Some(0), "本层没有冻结模块 ⇒ False");

    let suffixes = attribute(&instance, namespace, "extension_suffixes");
    let result = call(&instance, suffixes, &[]).expect("无参调用应当成功");
    let items = instance.tuple_items(result).or_else(|| instance.list_items(result));
    assert_eq!(items.map(|items| items.len()), Some(0), "不支持扩展模块 ⇒ 空表");
}

#[test]
fn is_builtin_rejects_non_strings_with_the_measured_message() {
    let instance = Instance::new();
    let namespace = imp_module::build(&instance);
    let is_builtin = attribute(&instance, namespace, "is_builtin");
    let number = instance.new_int(1);
    let error = call(&instance, is_builtin, &[number]).expect_err("非 str 要报错");
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(&instance)
                .unwrap_or_default();
            assert_eq!(message, "is_builtin() argument must be str, not int");
        }
        other => panic!("应当是脚本异常，实际 {other:?}"),
    }
}

#[test]
fn the_reference_surface_is_recorded() {
    // 夹具记下参照的**公开名字清单**：§5.2.4 的"未落地"逐条对着它写的。
    assert!(REFERENCE_NAMES.contains(&"pyc_magic_number_token"));
    assert!(REFERENCE_NAMES.contains(&"is_builtin"));
    assert!(REFERENCE_NAMES.len() >= 20, "参照的公开面不止两三个");
}
