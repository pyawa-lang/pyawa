//! **`_codecs_kr`**（CJK 多字节编解码族的第四族，round 12）：`Lib/encodings/euc_kr.py`／`cp949.py`／
//! `johab.py` 的底座。
//!
//! 契约见 `docs/SPEC-c-modules.md` §5.2.18（`_codecs` 一族）。导出面**刻意收窄** ✓ —— 参照的
//! `_codecs_kr` 只有 **`getcodec`** 一个名字（`dir(_codecs_kr)` 实测 ✓）：
//!
//! ```text
//! _codecs_kr.getcodec('euc_kr') ⇒ MultibyteCodec 实例（encode／decode 见 _multibytecodec）
//! ```
//!
//! 本族已落的 `Lib/encodings/*.py` 模块（逐字同步、不手改 ✓）：`euc_kr`／`cp949`／`johab`。
//! 表与引擎的**一处真相**在 [`crate::multibyte_module`]（含生成路径与缺口）。
//!
//! **`euc_kr` 的特例**（照参照实测 ✓）：谚文会分解成 **8 字节**定长序列（4 个 jamo 双字节码），
//! 解码器在 `a4d4` 之后对**任何**字节都还报 `incomplete` ⇒ 生成器把它标成**吸收态长前缀**
//! （`L` 行），引擎一步跳到 8 字节再查表；截断报 `incomplete`（`bytes in position 0-6`）、
//! 凑满 8 字节但不匹配报 `illegal`（`byte 0xa4 in position 0`）——均与参照逐行对拍 ✓。
//!
//! 韩文的 ISO-2022 变体（`iso2022_kr`）在参照里挂在 `_codecs_iso2022`（另一族，未落地）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, NativeFn};

/// 模块名（`_codecs_kr`）。
pub const NAME: &str = "_codecs_kr";

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

/// 建 `_codecs_kr` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let function = make_native(instance, "getcodec", crate::multibyte_module::getcodec_kr_native);
    instance.dict_set(namespace, "getcodec", function);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str("Codec registry for the Korean multibyte encodings (Pyawa built-in).");
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
