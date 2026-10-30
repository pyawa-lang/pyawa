//! 本文件是 `builtin_objects` 的 `contextvars` 域：`_contextvars` 的载荷与方法面（`ContextVar`／`Token`／`Context`）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;


py_object! {
    /// **`_contextvars.ContextVar`**（第 332 轮）。
    ///
    /// **如实登记的偏差** ✗：本层**没有真正的上下文隔离**（任务／线程局部状态尚未接线 ✓）——
    /// 值就存在**变量自己**身上 ✓（与 `weakref` 存强引用同源的"近似" ✓）。对"把 `Lib/` 跑起来"
    /// 这一步够用 ✓：`get`／`set`／`reset` 的**单上下文**语义与参照一致 ✓。
    pub struct ContextVarObject {
        /// 名字（`str`；**本对象持有一份引用** ✓）。
        name: NonNull<Header>,
        /// 默认值（`None` 单例表示"没有默认值" ✓ —— 参照用 `Token.MISSING` 哨兵，本层随后补 ✓）。
        default: NonNull<Header>,
        /// 当前值栈（`set` 往里压 ✓、`reset` 弹回 ✓）。
        values: RefCell<Vec<NonNull<Header>>>,
    }
}

py_object! {
    /// **`_contextvars.Token`**：`set` 的返回值 ✓，`reset(token)` 用它回滚 ✓。
    pub struct TokenObject {
        /// 产生它的变量（**持有引用** ✓）。
        var: NonNull<Header>,
        /// 旧值（**持有引用** ✓）。
        old_value: NonNull<Header>,
    }
}

py_object! {
    /// **`_contextvars.Context`**：`contextvars.py` 会把它 `register` 成 `Mapping` ✓ ⇒
    /// 必须是个**类型对象** ✓。**如实登记的偏差** ✗：本层的 `Context` 不承载独立状态
    /// （`run` 直接在全局上跑 ✓），只有"能用、能 isinstance"这一层 ✓。
    pub struct ContextObject {
        /// 占位（`dict`；随后接真正的上下文映射 ✓）。
        mapping: NonNull<Header>,
    }
}


impl ContextVarObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(context_var_traverse)
            .with_clear(context_var_clear)
    }

    /// 名字（**借用**）。
    pub fn name(&self) -> NonNull<Header> {
        self.name
    }

    /// 默认值（**借用**）。
    pub fn default(&self) -> NonNull<Header> {
        self.default
    }

    /// 当前值（**借用**）；没设过 ⇒ `None` ✓。
    pub fn current(&self) -> Option<NonNull<Header>> {
        self.values.borrow().last().copied()
    }

    /// 值栈的一份拷贝（**访问器** ✓：`gc_field_coverage` 那条守卫按"字段名出现在 traverse 体内"
    /// 查 ✓，走访问器才与 `set_traverse` 同一手法 ✓）。
    pub fn values(&self) -> Vec<NonNull<Header>> {
        self.values.borrow().clone()
    }

    /// 压入一个值（**接管**一份引用 ✓）。
    pub fn push_value(&self, value: NonNull<Header>) {
        self.values.borrow_mut().push(value);
    }

    /// 弹回上一个值（**交出**一份引用 ✓ —— 调用方负责释放 ✓）。
    pub fn pop_value(&self) -> Option<NonNull<Header>> {
        self.values.borrow_mut().pop()
    }
}

impl TokenObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(token_traverse)
            .with_clear(token_clear)
    }

    /// 变量（**借用**）。
    pub fn var(&self) -> NonNull<Header> {
        self.var
    }

    /// 旧值（**借用**）。
    pub fn old_value(&self) -> NonNull<Header> {
        self.old_value
    }
}

pub(crate) unsafe fn token_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TokenObject>() };
    visit(object.var().as_ptr());
    visit(object.old_value().as_ptr());
}

pub(crate) unsafe fn token_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<TokenObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.var().as_ptr()) };
    // SAFETY: 同上。
    unsafe { instance.release_object(object.old_value().as_ptr()) };
}

impl ContextObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(context_traverse)
            .with_clear(context_clear)
    }

    /// 映射（**借用**）。
    pub fn mapping(&self) -> NonNull<Header> {
        self.mapping
    }
}


/// `Token` 的属性面 ✓（`var`／`old_value` ✓）。
pub unsafe fn token_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TokenObject>() };
    match name {
        "var" => Some(instance.retain(object.var())),
        "old_value" => Some(instance.retain(object.old_value())),
        _ => None,
    }
}
// **下一轮接线用**：这四个方法的第一版实现会崩（单跑静默、合并跑段错误 ✓）⇒ 第 332 轮先把
// `Context` 的方法面改成如实报 `NotImplementedError` ✓（不把崩溃留在树里 ✗）。实现体**留着** ✓
// —— 它们是"接线时要走的路" ✓ ⇒ 这里显式放行"暂时没人调用" ✓，闸门要求 0 警告 ✓。
// **下一轮接线用**：这四个方法的第一版实现会崩（单跑静默、合并跑段错误 ✓）⇒ 第 332 轮先把
// `Context` 的方法面改成如实报 `NotImplementedError` ✓（不把崩溃留在树里 ✗）。实现体**留着** ✓
// —— 它们是"接线时要走的路" ✓ ⇒ 这里显式放行"暂时没人调用" ✓，闸门要求 0 警告 ✓。
// **下一轮接线用**：这四个方法的第一版实现会崩（单跑静默、合并跑段错误 ✓）⇒ 第 332 轮先把
// `Context` 的方法面改成如实报 `NotImplementedError` ✓（不把崩溃留在树里 ✗）。实现体**留着** ✓
// —— 它们是"接线时要走的路" ✓ ⇒ 这里显式放行"暂时没人调用" ✓，闸门要求 0 警告 ✓。
// **下一轮接线用**：这四个方法的第一版实现会崩（单跑静默、合并跑段错误 ✓）⇒ 第 332 轮先把
// `Context` 的方法面改成如实报 `NotImplementedError` ✓（不把崩溃留在树里 ✗）。实现体**留着** ✓
// —— 它们是"接线时要走的路" ✓ ⇒ 这里显式放行"暂时没人调用" ✓，闸门要求 0 警告 ✓。

/// `copy_context()` ✓。
pub fn copy_context_value(instance: &Instance) -> NonNull<Header> {
    let context_type = instance
        .type_named("Context")
        .expect("Context 在引导期已登记");
    let mapping = instance.new_dict();
    let object = instance.alloc(ContextObject::new(context_type, mapping));
    object.into_raw().cast::<Header>()
}
