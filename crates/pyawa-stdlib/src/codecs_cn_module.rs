//! **`_codecs_cn`**（CJK 多字节编解码族的第二族，round 11）：`Lib/encodings/gb2312.py`／`gbk.py` 的底座。
//!
//! 契约见 `docs/SPEC-c-modules.md` §5.2.18（`_codecs` 一族）。导出面**刻意收窄** ✓ —— 参照的
//! `_codecs_cn` 只有 **`getcodec`** 一个名字（`dir(_codecs_cn)` 实测 ✓）：
//!
//! ```text
//! _codecs_cn.getcodec('gbk') ⇒ MultibyteCodec 实例（encode／decode 见 _multibytecodec）
//! ```
//!
//! 本族已落的 `Lib/encodings/*.py` 模块（逐字同步、不手改 ✓）：`gb2312`／`gbk`
//! （`cp936` 是 `gbk` 的**别名**，`Lib/encodings/` 里没有 `cp936.py` ✓）。
//! 表与引擎的**一处真相**在 [`crate::multibyte_module`]（含生成路径与缺口）。
//!
//! 同族的 `gb18030`（4 字节区间 ≈ 110 万码位，需要专用线性表示）与 `hz`（有状态 7-bit）**未落地**
//! ⇒ `import encodings.gb18030`／`encodings.hz` 会停在模块级
//! `_codecs_cn.getcodec(...)` 的 `LookupError: no such codec is supported.`（如实缺口，不造空壳 ✗）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, NativeFn};

/// 模块名（`_codecs_cn`）。
pub const NAME: &str = "_codecs_cn";

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

/// 建 `_codecs_cn` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let function = make_native(instance, "getcodec", crate::multibyte_module::getcodec_native);
    instance.dict_set(namespace, "getcodec", function);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str("Codec registry for the Chinese multibyte encodings (Pyawa built-in).");
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
