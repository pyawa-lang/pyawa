//! `_weakref` 模块（第 173 轮）：**最小面** = `ref` ✓。
//!
//! **如实登记的偏差** ✗：本层**没有真正的弱引用**（GC 不支持 ✓）⇒ `ref(x)` 存的是**强引用** ✓
//! ⇒ 目标不会被回收 ✓、`ref(x, 回调)` 的**回调被忽略** ✓。对「把 `Lib/abc.py`／`os.py` 跑起来」这一步够用 ✓。
//!
//! `CX-4`：本 crate 不碰平台 ⇒ 这里不 import 任何平台接口 ✓。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};

/// 模块名（`_weakref`）。
pub const NAME: &str = "_weakref";


/// 建 `_weakref` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // **`ref` 就是那个类型对象** ✓（构造器在它的 `new` 槽里 ✓）—— 与 `slice`／`object` 同一手法 ✓。
    // **先 `retain` 再交给字典** ✓（`dict_set` 接管一份引用 ✓ —— 第 161 轮的堆损坏就是这么来的 ✗）。
    if let Some(weakref_type) = instance.type_named("weakref") {
        instance.retain(weakref_type.cast());
        instance.dict_set(namespace, "ref", weakref_type.cast());
    }
    let exports = instance.new_list(vec![instance.new_str("ref")]);
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
