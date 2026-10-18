//! **`_codecs_jp`**（CJK 多字节编解码族的第一族）：`Lib/encodings/cp932.py` 一族的 `getcodec` 底座。
//!
//! 契约见 `docs/SPEC-c-modules.md` §5.2.18（`_codecs` 一族）。导出面**刻意收窄** ✓ —— 参照的
//! `_codecs_jp` 只有 **`getcodec`** 一个名字（`dir(_codecs_jp)` 实测 ✓）：
//!
//! ```text
//! _codecs_jp.getcodec('cp932') ⇒ MultibyteCodec 实例（encode／decode 见 _multibytecodec）
//! ```
//!
//! 本族的 **7** 个 `Lib/encodings/*.py` 模块（逐字同步、不手改 ✓）全靠它：
//! `cp932`／`euc_jp`／`euc_jis_2004`／`euc_jisx0213`／`shift_jis`／`shift_jis_2004`／
//! `shift_jisx0213`。表与引擎的**一处真相**在 [`crate::multibyte_module`]（含生成路径与缺口）。
//!
//! 其余三族（`_codecs_cn`／`_codecs_kr`／`_codecs_tw`）**尚未落地** ⇒ 对应
//! `import encodings.gbk` 一族仍报 `No module named '_codecs_cn'`（如实缺口，不造空壳 ✗）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, NativeFn};

/// 模块名（`_codecs_jp`）。
pub const NAME: &str = "_codecs_jp";

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

/// 建 `_codecs_jp` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let function = make_native(instance, "getcodec", crate::multibyte_module::getcodec_native);
    instance.dict_set(namespace, "getcodec", function);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str("Codec registry for the Japanese multibyte encodings (Pyawa built-in).");
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
