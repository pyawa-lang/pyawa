//! `Instance` 的分配后处理/追踪/GC 邻接域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// **新增一份引用**并交回同一对象（给"按原样返回实参"的原生函数用，`OM-16`）。
    pub fn retain(&self, object: NonNull<Header>) -> NonNull<Header> {
        // SAFETY: 调用方保证 object 存活。
        unsafe { self.incref_object(object.as_ptr()) };
        object
    }

    /// **对象真假**（第 131 轮）：直接复用执行器那份判定 ✓（**一处真相** ✓）——
    /// 内建 `bool()` 要的就是它（`bool_value` 只覆盖 bool／None ✗ ⇒ `bool(0)` 会错 ✗）。
    pub fn truthiness_of(&self, object: NonNull<Header>) -> Result<bool, ExecError> {
        crate::executor::iter::truthiness(self, object, 0)
    }

    /// **真值**（`TO_BOOL` 的同一处真相：`all`／`any` 要用）。
    ///
    /// 假：`None`／`False`／数值零／空串／空容器；其余真（没有 `__bool__`／`__len__` 的对象
    /// 按参照实现是**真**）。
    pub fn truth_of(&self, object: NonNull<Header>) -> bool {
        let ty = self.type_of(object);
        if ty == self.singletons().none_type() {
            return false;
        }
        if let Some(flag) = self.bool_value(object) {
            return flag;
        }
        if ty == self.singletons().int_type() {
            // 大整数不能看 `i64` 那个快路径（`int_value` 对它给 `None` ⇒ 会被当成 0＝假）
            return self.int_of(object).map(|value| !value.is_zero()).unwrap_or(false);
        }
        if self.type_named("float") == Some(ty) {
            return self.float_value(object).unwrap_or(0.0) != 0.0;
        }
        if ty == self.singletons().str_type() {
            // SAFETY: 类型身份已确认。
            return !unsafe { &*object.as_ptr().cast::<StrObject>() }.value().is_empty();
        }
        if let Some(length) = self.length_of(object) {
            return length != 0;
        }
        true
    }

    /// **`OM-22`**：对象的**引用计数**（**安全**读取）。
    ///
    /// 给 stdlib 的 `sys.getrefcount` 用——那个 crate 是 `#![forbid(unsafe_code)]`，
    /// 不能自己去 `as_ref()`。
    /// **开始盯住某个地址**（第 113 轮诊断用 ✓）。
    pub fn watch_address(&self, object: NonNull<Header>) {
        self.watch.set(object.as_ptr() as usize);
    }

    /// **OM-25**：当前参与循环回收（`GC_TRACKED`）的对象数。
    pub fn tracked_objects(&self) -> usize {
        self.gc_count.get()
    }
}
