//! `_collections` 模块（`SPEC-c-modules.md`；第 331 轮）。
//!
//! **本段落地**：`deque` ✓ —— 它是 `Lib/collections/__init__.py` 的
//! `from _collections import deque, ...` 那一行要的名字 ✓，也是上限榜上
//! `ImportError: cannot import name 'deque' from 'collections'` × **18** 个模块的卡点 ✓。
//!
//! 类型本身（载荷、方法面、`repr`、`len`）都在**核心** ✓（要看容器内部 ✓ —— 与 `list`／`set` 同一口径 ✓），
//! 本模块只把类型对象**导出**成 `deque` 这个名字 ✓（`CX-4`：stdlib 不碰平台 ✓）。
//!
//! **如实登记的未接面** ✗：`deque` 的迭代协议／下标／`__contains__`／`__eq__`／`reverse`（随后补 ✓）；
//! `_collections` 里 `collections/__init__.py` 还要的 `_count_elements`／`_tuplegetter` 等名字 ✗
//! —— 那些等真正撞上再补 ✓（当前先让 `deque` 这一行能过 ✓）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};

/// 模块名（`_collections`）。
pub const NAME: &str = "_collections";

/// 建 `_collections` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // **`deque` 就是那个类型对象** ✓（构造器在它的 `new` 槽里 ✓ —— 与 `slice`／`weakref` 同一手法 ✓）。
    if let Some(deque_type) = instance.type_named("deque") {
        instance.retain(deque_type.cast());
        instance.dict_set(namespace, "deque", deque_type.cast());
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
