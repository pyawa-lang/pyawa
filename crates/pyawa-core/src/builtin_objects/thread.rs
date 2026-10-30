//! 本文件是 `builtin_objects` 的 `thread` 域：`_thread` 的锁与线程句柄（`ThreadLockObject`／`ThreadHandleObject`）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;



/// **本层"当前线程"的 ident**。
///
/// 依据：`DESIGN.md` §5 的挂起是**协作式**的，每个实例只有一条执行流 ⇒ 本层**每实例单线程** ✓。
/// 参照的 `get_ident()` 给的是**平台相关**的大整数（本机实测 `127797024846336` 一类）⇒ 取值属
/// `MS-17` 的**实现观测面** ✓ ⇒ 本层给一个**稳定的小整数** ✓（跨线程代码本来就没有可跑的地基 ✓）。
pub const MAIN_THREAD_IDENT: i64 = 1;

py_object! {
    /// **`_thread` 的锁**（第 280 轮）：`_thread.lock` 与 `_thread.RLock` **共用这一份载荷** ✓。
    ///
    /// **为什么归 VM** ✓（用户裁定 A）：`SPEC-capabilities.md` §9.9 的 `ipc` 行自己写着
    /// "`thread_*` …**线程语义归 VM 侧**"（且不可异步化）⇒ 单线程下锁就是 VM 内的记账，
    /// **不碰外部世界权威** ✓（`CM-8` 管的是"需外部世界权威"的模块 ✓）。
    ///
    /// 语义：
    /// - `RLock`：同一"线程"**可重入** ✓（`depth` 记重入深度 ✓ —— `importlib` 的 `_ModuleLock` 靠它 ✓）；
    /// - 普通锁：未持 ⇒ 取得 ✓；**已持** ⇒ 参照会**阻塞** ✓ —— 而本层单线程、挂起是协作式的
    ///   ⇒ 那个持有者**永远跑不到 `release`** ⇒ 那是一处**死锁** ✗ ⇒ 按 `CM-6` **如实报未实现** ✓
    ///   （**不**静默假装成功 ✗）。非阻塞形态 `acquire(False)` 照参照给 `False` ✓。
    pub struct ThreadLockObject {
        /// 重入深度（普通锁只用 0／1）。
        depth: Cell<usize>,
        /// 持有者的 ident（未持时 `0`）。
        owner: Cell<i64>,
    }
}

impl ThreadLockObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_new(thread_lock_new)
            .with_getattr(thread_lock_getattr)
    }

    /// 重入深度。
    pub fn depth(&self) -> usize {
        self.depth.get()
    }

    /// 持有者 ident。
    pub fn owner(&self) -> i64 {
        self.owner.get()
    }
}

/// 这个锁是不是**可重入**的那一个（两个类型共用载荷 ⇒ 判据只能取**类型名** ✓）。
pub(crate) fn lock_is_recursive(instance: &Instance, lock: NonNull<Header>) -> bool {
    instance.type_name(instance.type_of(lock)) == "RLock"
}

/// 把绑定形态的 `self` 取出来（方法契约保证有 ✓）。
pub(crate) fn bound_lock(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, crate::ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}

py_object! {
    /// **`_thread._ThreadHandle`**（第 333 轮）：线程句柄的**类型占位** ✓。
    ///
    /// `Lib/threading.py` 在模块级就做 `_ThreadHandle = _thread._ThreadHandle` ✗ ⇒ 这个名字
    /// **必须存在**才 import 得动 ✓（上限榜上 43 个模块卡在 `_thread` ✓）。
    /// **如实说明** ✗：本层**没有真线程**（每实例单线程 ✓）⇒ 句柄里没有任何真实状态 ✓，
    /// 不是"伪造一个能用的句柄" ✓ —— `Thread.start()` 那一类仍走 `NotImplementedError` ✓。
    pub struct ThreadHandleObject {
        /// 占位（**不是引用** ⇒ 不需要 traverse ✓，`gc_field_coverage` 只查持引用字段 ✓）。
        started: Cell<bool>,
    }
}

impl ThreadHandleObject {
    /// 见 [`TupleObject::slots`]（**不持引用** ⇒ 只有 `dealloc` ✓）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
    }
}
