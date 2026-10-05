//! 内建类型的方法面，**按类型族分文件** ✓（第 123 轮起 ✓）：先从 `builtin_objects.rs` 搬 `str` 族 ✓。
//!
//! 取舍 ✓：**派发表**（`str_method_native` 一类）留在 `builtin_objects.rs` ✓ —— 它们是族之间的粘合层 ✓。

pub(crate) mod str;
pub(crate) mod bytes;
pub(crate) mod dict;
pub(crate) mod deque;
pub(crate) mod list;
