//! `Instance` 的查询/渲染/判定域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// 对象是不是**类型对象**（`type` 的实例）——`isinstance`／`issubclass` 要用。
    pub fn is_type_object(&self, object: NonNull<Header>) -> bool {
        // **判据是"元类型是 `type` 的子类"** ✓（第 231 轮真 bug 修复 ✗）：先前写的是"**恰为 `type`**" ✗
        // ⇒ 一旦某个类的元类型是**用户定义的元类**（`class M(type)` ＋ `metaclass=M` ✓），
        // 它就会被当成**普通对象** ⇒ 属性通道按 `AttributeObject` 读 ⇒ 读到 `0x4` ⇒ **段错误** ✗
        //（gdb 回溯：`build_class_native` → `call_dunder_method` → `type_of(0x4)` ✓）。
        self.is_subtype(self.type_of(object), self.metatype())
    }

    /// **可调用判定**（`OM-11`）——**一处口径**：类型的 `call` 槽存在，**或**它是内建可调用
    /// 类型（`function`／`builtin_function_or_method`／`method`／元类型，这几个的调用语义写
    /// 在 `call_callable` 里）。
    ///
    /// 给 `callable()`、`pa_isfunction` 一类共用；两边各写一份就会漂。
    pub fn is_callable(&self, object: NonNull<Header>) -> bool {
        let ty = self.type_of(object);
        // SAFETY: 类型对象由注册表持有。
        if unsafe { ty.as_ref() }.has_call_slot() {
            return true;
        }
        ty == self.metatype()
            || Some(ty) == self.type_named("function")
            || Some(ty) == self.type_named("builtin_function_or_method")
            || Some(ty) == self.type_named("method")
    }

    pub fn is_bool(&self, object: NonNull<Header>) -> bool {
        self.type_of(object) == self.singletons().bool_type()
    }
}
