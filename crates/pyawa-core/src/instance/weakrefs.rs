//! **弱引用登记与失效**（round 59；`OM-31` 的弱引用面）。
//!
//! 目标**不**被弱引用持有 ⇒ 目标释放时必须「置死 ＋ 调回调」。登记表放在 `Instance` 上
//! （`weakrefs: 目标地址 ⇒ 弱引用对象指针的表`；弱引用对象本身由用户／容器持有，这里**不持有**
//! 引用 ⇒ 弱引用对象释放时要**摘除**登记 ✓）。
//!
//! 口径照参照（`_weakref` 的 `tp_dealloc` 顺序）：目标计数归零后、真正释放前，逐个
//! `mark_dead()`，然后以**弱引用对象为唯一实参**调用回调 ✓（每个回调只调一次 ✓）。
//! GC 回收路径（`free_garbage`）里同样要失效 —— 见 `Instance::collect_inner` 的"回调队列"。

use super::*;

impl Instance {
    /// 这个对象**能不能**建弱引用（round 59 的近似面：照参照的"有 `__weakref__` 槽"）。
    ///
    /// **如实说** ✗：本层没有 `tp_weaklistoffset` 概念 ⇒ 这里按**类型名**给一份近似表
    /// （标量／不可变容器／迭代器一律不可弱引用；用户类、函数、方法、模块、类型、生成器等可 ✓）。
    pub fn is_weakrefable(&self, object: NonNull<Header>) -> bool {
        let name = self.type_name(self.type_of(object));
        if name.ends_with("iterator") {
            return false;
        }
        !matches!(
            name.as_str(),
            "int"
                | "bool"
                | "float"
                | "complex"
                | "str"
                | "bytes"
                | "bytearray"
                | "tuple"
                | "list"
                | "dict"
                | "range"
                | "slice"
                | "NoneType"
                | "ellipsis"
                | "object"
                | "property"
                | "staticmethod"
                | "classmethod"
                | "super"
                | "frame"
                | "mappingproxy"
                | "enumerate"
                | "zip"
                | "map"
                | "filter"
                | "reversed"
                | "itertools"
        )
    }

    /// 登记一个弱引用（**不持有**弱引用对象的引用 ✓）。
    pub fn register_weakref(&self, target: NonNull<Header>, weakref: NonNull<Header>) {
        // **OM-7 bit 2**（round 250 性能，A2-5）：目标"有弱引用"的事实记在**对象头**里 ⇒
        // 释放路径一次位测就能早退（`take_weakref_callbacks` 每次释放都调 ✗）。
        // SAFETY: target 由调用方保证存活。
        unsafe { target.as_ref() }.set_flag(crate::flags::HAS_WEAKREFS);
        self.weakrefs
            .borrow_mut()
            .entry(target.as_ptr() as usize)
            .or_default()
            .push(weakref);
    }

    /// 摘除一个弱引用（弱引用对象释放时；按**地址**比较，不解引用 ✓）。
    pub fn unregister_weakref(&self, target_address: usize, weakref: NonNull<Header>) {
        let mut table = self.weakrefs.borrow_mut();
        let Some(list) = table.get_mut(&target_address) else {
            return;
        };
        list.retain(|candidate| *candidate != weakref);
        if list.is_empty() {
            table.remove(&target_address);
            drop(table);
            // **位清零**（round 250 性能，A2-5）：目标还在活表里才可安全解引用（目标已死时
            // 这条登记早被 `take_weakref_callbacks` 整批摘走了 ✓）。
            if self.live.borrow().contains(&target_address) {
                // SAFETY: 刚确认目标地址在活表里。
                unsafe {
                    NonNull::new(target_address as *mut Header)
                        .map(|target| target.as_ref().clear_flag(crate::flags::HAS_WEAKREFS))
                };
            }
        }
    }

    /// 目标地址 ⇒ 指向它的弱引用数（`getweakrefcount`）。
    pub fn weakref_count(&self, target: NonNull<Header>) -> usize {
        self.weakrefs
            .borrow()
            .get(&(target.as_ptr() as usize))
            .map_or(0, Vec::len)
    }

    /// 目标地址 ⇒ 指向它的弱引用（**借用副本**；调用方要自己 retain ✓）。
    pub fn weakrefs_of(&self, target: NonNull<Header>) -> Vec<NonNull<Header>> {
        self.weakrefs
            .borrow()
            .get(&(target.as_ptr() as usize))
            .cloned()
            .unwrap_or_default()
    }

    /// **目标即将释放**：把所有指向它的弱引用置死并取走回调 ✓。
    ///
    /// 返回 `(弱引用, 回调)`：弱引用**已新增一份引用**（防止回调链里被释放 ✓）；
    /// 回调是**字段里那份的所有权**（字段已清空；调用方负责释放 ✓）。
    pub(crate) fn take_weakref_callbacks(
        &self,
        target: NonNull<Header>,
    ) -> Vec<(NonNull<Header>, NonNull<Header>)> {
        // **快路**（round 250 性能，A2-5）：对象头没有 `HAS_WEAKREFS` ⇒ 一定没有登记表项 ✓
        //（本函数**每次释放都调**；先前要 `borrow_mut()`＋对地址算一遍哈希再查空表 ✗）。
        // SAFETY: target 由调用方保证存活（释放路径在 dealloc 之前调用 ✓）。
        let header = unsafe { target.as_ref() };
        if !header.has_flag(crate::flags::HAS_WEAKREFS) {
            return Vec::new();
        }
        let removed = self
            .weakrefs
            .borrow_mut()
            .remove(&(target.as_ptr() as usize))
            .unwrap_or_default();
        // 本目标名下的弱引用已被**整批**摘走 ⇒ 位清零 ✓（之后再登记会重新置位 ✓）。
        if !removed.is_empty() {
            header.clear_flag(crate::flags::HAS_WEAKREFS);
        }
        let mut callbacks = Vec::new();
        for weakref in removed {
            // SAFETY: 登记表里的弱引用对象若还活着才取回调；对象可能已在 GC 清除阶段被清掉
            // （那时 `callback` 字段已被 `weakref_clear` 取走 ⇒ 这里拿到 `None` ✓）。这里只用
            // 指针比较／读字段，不做 `assert_live`（GC 的冻结窗口里对象状态特殊）。
            let alive = self.live.borrow().contains(&(weakref.as_ptr() as usize));
            if !alive {
                continue;
            }
            // SAFETY: 刚确认存活。
            let object = unsafe { &*weakref.as_ptr().cast::<crate::builtin_objects::WeakRefObject>() };
            object.mark_dead();
            if let Some(callback) = object.take_callback() {
                // 弱引用本体在回调期间要活着（回调可能把它从容器里删掉 ✓）。
                // SAFETY: 刚确认存活。
                unsafe { self.incref_object(weakref.as_ptr()) };
                callbacks.push((weakref, callback));
            }
        }
        callbacks
    }

    /// **调用一组弱引用回调**（`(弱引用, 回调)`；弱引用那份引用由本函数归还 ✓）。
    pub(crate) fn run_weakref_callbacks(&self, mut callbacks: Vec<(NonNull<Header>, NonNull<Header>)>) {
        for (weakref, callback) in callbacks.drain(..) {
            let outcome = crate::executor::call::call_value(
                self,
                callback,
                &[weakref],
                &[],
            );
            match outcome {
                Ok(value) => {
                    self.release(value);
                }
                Err(error) => {
                    if crate::diag::flag_site!("PYAWA_WEAKREF_DEBUG") {
                        eprintln!("[weakref] 回调失败：{error:?}");
                    }
                }
            }
            // 回调那份引用已在 `call_value` 里按借用视图归还（实参表由调用方持有）⇒ 这里还
            // 弱引用与回调各一份 ✓。
            self.release(weakref);
            self.release(callback);
        }
    }

    /// **GC 路径**：把回调排队（整个回收循环结束后再统一调 ✓）。
    pub(crate) fn queue_weakref_callbacks(
        &self,
        callbacks: Vec<(NonNull<Header>, NonNull<Header>)>,
    ) {
        if !callbacks.is_empty() {
            self.pending_weakref_callbacks.borrow_mut().extend(callbacks);
        }
    }

    /// **GC 路径**：调掉排队的回调（`collect` 收尾处调用 ✓）。
    pub(crate) fn drain_weakref_callbacks(&self) {
        let callbacks = core::mem::take(&mut *self.pending_weakref_callbacks.borrow_mut());
        if !callbacks.is_empty() {
            self.run_weakref_callbacks(callbacks);
        }
    }

    /// **弱引用对象自己将死**：从目标的登记表里摘掉自己（按地址比较，不解引用目标 ✓）。
    pub(crate) fn forget_weakref_object(&self, object: NonNull<Header>) {
        // **快路**（round 250 性能，A2-5）：整张弱引用表空着 ⇒ 不可能是某个目标的弱引用 ✓
        //（本函数每次释放都调 ✗）。
        if self.weakrefs.borrow().is_empty() {
            return;
        }
        let Some(state) = crate::builtin_objects::weakref_state(self, object) else {
            return;
        };
        if let Some(target) = state {
            self.unregister_weakref(target.as_ptr() as usize, object);
        }
    }

    /// 目标释放前的**一步到位**（正常引用计数路径）：置死 ＋ 立即调回调 ✓。
    pub(crate) fn invalidate_weakrefs(&self, target: NonNull<Header>) {
        let callbacks = self.take_weakref_callbacks(target);
        if !callbacks.is_empty() {
            self.run_weakref_callbacks(callbacks);
        }
    }
}
