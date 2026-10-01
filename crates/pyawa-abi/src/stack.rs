//! 虚拟栈（`AB-9`…`AB-13`）：宿主与能力函数用它收发值（`DESIGN.md` §8.1 的 Lua 风格线格式）。
//!
//! - **`AB-9`**：索引规则——**正索引自底（1 起）**、**负索引自顶（−1 是栈顶）**、`0` 非法
//! - **`AB-10`**：栈上每个槽位**持有**一个引用（`OM-16`／`OM-20`）；宿主按 §6 的转移规则归还
//! - **`AB-12`**：栈深上限存在，越界返回错误码（**禁止** UB）
//! - **`AB-13`**：栈**与实例绑定**（`OM-1` 的实例隔离）
//! - **`AB-15`**：借用与持有可区分——槽位带 `owned` 标记，`pa_retain` 把借来的转成持有
//!
//! 类型标签的取值由实现定（规格只说"类型标签"），写进 `include/pa.h` 与这里的
//! [`tag`] 模块，两者必须一致。

use core::ptr::NonNull;

use pyawa_core::{DictObject, FloatObject, Header, Instance, IntObject, ListObject, StrObject};

use crate::status::*;

/// `AB-9`：栈深上限（越界返回 `PA_ERR_INVALID`，**禁止** UB）。
pub const STACK_LIMIT: usize = 1_000_000;

/// 类型标签（`pa_type` 的返回值；取值是实现自选，与 `pa.h` 一致）。
pub mod tag {
    /// 没有值／`None`。
    pub const PA_TNIL: i32 = 0;
    /// 布尔。
    pub const PA_TBOOLEAN: i32 = 1;
    /// 整数。
    pub const PA_TINTEGER: i32 = 2;
    /// 浮点。
    pub const PA_TNUMBER: i32 = 3;
    /// 字符串。
    pub const PA_TSTRING: i32 = 4;
    /// 表（本层就是 `dict`）。
    pub const PA_TTABLE: i32 = 5;
    /// 可调用。
    pub const PA_TFUNCTION: i32 = 6;
    /// 宿主对象句柄（`OM-34`，尚未接线）。
    pub const PA_THANDLE: i32 = 7;
}

/// 一个栈槽：值 ＋ 是否**持有**一份引用（`AB-10`／`AB-15`）。
#[derive(Clone, Copy)]
pub struct Slot {
    /// 值（裸引用；`owned` 为真时本槽持有一份）。
    pub object: NonNull<Header>,
    /// 本槽是否持有引用。
    pub owned: bool,
}

/// 虚拟栈。
pub struct VirtualStack {
    slots: Vec<Slot>,
}

impl VirtualStack {
    /// 空栈。
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// `pa_gettop`：当前栈深。
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// 是否空栈。
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// `AB-9`：把外部索引换算成下标。`0` 或越界返回 `None`。
    pub fn index_of(&self, index: i32) -> Option<usize> {
        if index > 0 {
            let position = (index - 1) as usize;
            (position < self.slots.len()).then_some(position)
        } else if index < 0 {
            let back = (-index) as usize;
            (back <= self.slots.len()).then(|| self.slots.len() - back)
        } else {
            None
        }
    }

    /// 读一个槽（**借用**）。
    pub fn get(&self, index: i32) -> Option<Slot> {
        self.index_of(index).map(|position| self.slots[position])
    }

    /// 压入一个**已持有**的值（新引用；失败时引用仍归调用方）。
    pub fn push_owned(&mut self, object: NonNull<Header>) -> i32 {
        if self.slots.len() >= STACK_LIMIT {
            return PA_ERR_INVALID;
        }
        self.slots.push(Slot {
            object,
            owned: true,
        });
        PA_OK
    }

    /// 压入一个**借用**的值（不持有引用）。
    pub fn push_borrowed(&mut self, object: NonNull<Header>) -> i32 {
        if self.slots.len() >= STACK_LIMIT {
            return PA_ERR_INVALID;
        }
        self.slots.push(Slot {
            object,
            owned: false,
        });
        PA_OK
    }

    /// 弹出一个槽（交回槽位；调用方按 `owned` 归还）。
    pub fn pop_slot(&mut self) -> Option<Slot> {
        self.slots.pop()
    }

    /// 把某个槽转成**持有**（`AB-15` 的"借用 → 持有"）：返回是否需要 incref。
    pub fn mark_owned(&mut self, index: i32) -> Option<()> {
        let position = self.index_of(index)?;
        if !self.slots[position].owned {
            self.slots[position].owned = true;
            return Some(());
        }
        Some(())
    }

    /// 把某个槽转成**借用**（"持有 → 释放"）：返回是否需要 release。
    pub fn mark_borrowed(&mut self, index: i32) -> Option<bool> {
        let position = self.index_of(index)?;
        let was_owned = self.slots[position].owned;
        self.slots[position].owned = false;
        Some(was_owned)
    }

    /// 截断到 `count` 项，返回被丢掉的槽（调用方负责归还持有的引用）。
    pub fn truncate(&mut self, count: usize) -> Vec<Slot> {
        if count >= self.slots.len() {
            return Vec::new();
        }
        self.slots.split_off(count)
    }

    /// 补齐到 `count` 项（用 `nil` 填）。
    pub fn grow_to(&mut self, count: usize, nil: NonNull<Header>) -> i32 {
        if count > STACK_LIMIT {
            return PA_ERR_INVALID;
        }
        while self.slots.len() < count {
            self.slots.push(Slot {
                object: nil,
                owned: false,
            });
        }
        PA_OK
    }

    /// 清空（调用方负责归还持有的引用）。
    pub fn drain(&mut self) -> Vec<Slot> {
        core::mem::take(&mut self.slots)
    }
}

/// **`AB-14`**：宿主可见的**值**——一个不透明句柄（不暴露头部与类型对象指针）。
///
/// 名字照 C 侧（`pa_value`），故显式关掉命名检查。
#[allow(non_camel_case_types)]
pub type pa_value = *mut core::ffi::c_void;

/// 类型标签（`pa_type`）：按**类型身份**判定，不猜结构。
pub fn tag_of(instance: &Instance, object: NonNull<Header>) -> i32 {
    // SAFETY: 调用方保证 object 存活。
    let ty = unsafe { object.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.none_type() {
        return tag::PA_TNIL;
    }
    if ty == singletons.bool_type() {
        return tag::PA_TBOOLEAN;
    }
    if ty == singletons.int_type() {
        return tag::PA_TINTEGER;
    }
    if Some(ty) == instance.type_named("float") {
        return tag::PA_TNUMBER;
    }
    if ty == singletons.str_type() {
        return tag::PA_TSTRING;
    }
    if Some(ty) == instance.type_named("dict") {
        return tag::PA_TTABLE;
    }
    // `OM-11`：有 `call` 槽的类型，其实例就是可调用的（宿主函数走的正是这条）
    // SAFETY: ty 由注册表持有。
    if unsafe { ty.as_ref() }.has_call_slot()
        || Some(ty) == instance.type_named("function")
        || Some(ty) == instance.type_named("builtin_function_or_method")
        || Some(ty) == instance.type_named("method")
        || ty == instance.metatype()
    {
        return tag::PA_TFUNCTION;
    }
    tag::PA_THANDLE
}

/// `pa_toboolean`：真值转换（`OM-11` 的 `__bool__` 槽位接线前，先按类型判）。
pub fn truthy(instance: &Instance, object: NonNull<Header>) -> bool {
    match tag_of(instance, object) {
        tag::PA_TNIL => false,
        tag::PA_TBOOLEAN => {
            // SAFETY: 调用方保证 object 存活，且类型身份已确认。
            unsafe { &*object.as_ptr().cast::<pyawa_core::BoolObject>() }.value
        }
        tag::PA_TINTEGER => {
            // SAFETY: 同上。
            unsafe { &*object.as_ptr().cast::<IntObject>() }.value != 0
        }
        tag::PA_TNUMBER => {
            // SAFETY: 同上。
            unsafe { &*object.as_ptr().cast::<FloatObject>() }.value != 0.0
        }
        tag::PA_TSTRING => {
            // SAFETY: 同上。
            !unsafe { &*object.as_ptr().cast::<StrObject>() }.value().is_empty()
        }
        tag::PA_TTABLE => {
            // SAFETY: 同上。
            !unsafe { &*object.as_ptr().cast::<DictObject>() }.entries().is_empty()
        }
        _ => true,
    }
}

/// 列表长度（`paL_len` 与 `pa_newlist` 用）。
pub fn list_len(instance: &Instance, object: NonNull<Header>) -> Option<usize> {
    // SAFETY: 调用方保证 object 存活。
    let ty = unsafe { object.as_ref() }.ty();
    if Some(ty) == instance.type_named("list") {
        // SAFETY: 类型身份已确认。
        return Some(unsafe { &*object.as_ptr().cast::<ListObject>() }.len());
    }
    None
}
