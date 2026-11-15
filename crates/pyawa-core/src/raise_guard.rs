//! **RAII 异常引用守卫**（阶段 2／A，2026-11-14）：把"`Err(Raised)` 那份引用被静默丢掉"
//! 从"靠纪律"变成"类型上不可能"。
//!
//! ## 背景（为什么需要）
//!
//! 本层是引用计数 VM：抛出异常时 `raise()` 建**两份所有权**——一份进实例的
//! `pending_exception` 槽（保活／链式上下文用），一份随 `ExecError::Raised` 交出去给派发器。
//! 后者是**裸 `NonNull<Header>`**：调用方"丢弃错误"时忘了还它，就漏"异常＋回溯链＋整帧局部"。
//! 这类 bug 在同一错误模式下反复出现（`PLAN` §12.6 记录了 6 处，结构上还有更多）——
//! 根因不是"分支写错"，而是**所有权协议没有类型表达**（借用/转移混用、Drop 不兜底）。
//!
//! ## 形状
//!
//! - [`RaiseRef`]：`Err(Raised)` 里那份所有权的**守卫**。`Drop` 时**只归还 `Err` 那份**、
//!   **不动 `pending` 槽**（槽是实例的 Python 可见异常状态：调用方/测试要读它，下一次
//!   `raise`/执行收尾自会替换清理）。⇒ 任何"忘记处理"都不再泄漏 `Err` 引用。
//!   要"连槽一起丢"（吞异常/宿主已取消息）必须**显式** [`RaiseRef::discard`]（幂等）。
//! - [`PendingRef`]：`Instance::replace_pending` 交回的**旧 pending** 所有权守卫，`Drop` 自动
//!   释放（取代旧 API "交回裸指针、由调用方记得 release"——终结器那条泄漏正是忘了还）。
//! - 显式消费只有两个入口：`RaiseRef::into_pending()`（把 `Err` 那份移交回 `pending` 槽）与
//!   `RaiseRef::discard()`（提前丢弃；等价于默认 Drop，只是让意图可读）。两者都是幂等的
//!   （内部 `consumed` 位保证不会二次归还）。
//!
//! ## 不变量（如实的边界）
//!
//! `RaiseRef` 里存 `*const Instance`：本类型**只在同一实例的执行栈内传播／消费**（与
//! `ExecError` 的既有生命周期一致：ABI 侧在 `pa_state` 存活期内、执行器侧在 `execute` 调用内）。
//! 把错误保存到实例销毁之后再用会破坏该不变量——与改动前"裸指针 + 实例存活"是同一契约，
//! 只是现在释放时机由 Drop 保证。
//!
//! ## 与诊断账本的关系
//!
//! `Drop`／`discard` 记 [`crate::error_ledger::record_consumed`]；`into_pending` 记
//! [`crate::error_ledger::record_pending_transfer`]。`raise()` 侧的"生成／移交 pending"账不变
//! ⇒ `PYAWA_GC_LOG=4` 的恒等式在 RAII 之后仍成立（`generated == 消费＋移交＋…`）。

use core::cell::Cell;
use core::ptr::NonNull;

use crate::header::Header;
use crate::instance::Instance;

/// `Err(ExecError::Raised)` 里那份异常引用的**所有权守卫**（`Drop` 自动归还）。
///
/// 用 `#[repr(transparent)]` 不必要；本类型只在异常路径出现，多一个 `consumed` bool 可接受。
pub struct RaiseRef {
    /// 持有的那一份引用（`raise()` 里为 `Err` 侧新增的那份）。
    exception: NonNull<Header>,
    /// 建它时的实例（不变量见模块文档：错误只在本实例执行栈内传播/消费）。
    instance: *const Instance,
    /// 是否已消费（`discard()`／`into_pending()`）；保证 Drop 不再二次归还。
    consumed: Cell<bool>,
}

impl RaiseRef {
    /// **接收一份 owned 引用**（调用方必须刚为它 `incref`／新建；本函数接管，不再 incref）。
    pub fn from_owned(instance: &Instance, exception: NonNull<Header>) -> Self {
        Self {
            exception,
            instance,
            consumed: Cell::new(false),
        }
    }

    /// 借用异常对象（**不转移所有权**）。
    pub fn get(&self) -> NonNull<Header> {
        self.exception
    }

    fn instance_ref(&self) -> &Instance {
        // SAFETY: 不变量（模块文档）：错误只在本实例执行栈内传播/消费，实例存活。
        unsafe { &*self.instance }
    }

    /// 归还 `Err` 那份引用；若 `pending_exception` 槽仍指向同一对象，**一并清槽并归还槽里那份**。
    ///
    /// 与改动前的 `discard_raised` 同口径（宿主/调用方已拿到消息、异常不再需要）。
    /// 幂等：二次调用是空操作。
    ///
    /// **与 Drop 的区别**：Drop 只还 `Err` 那份、保留槽；本函数连槽一起清（槽若仍指向它）。
    pub fn discard(&self) {
        if self.consumed.replace(true) {
            return;
        }
        let instance = self.instance_ref();
        let pointer = self.exception.as_ptr() as usize;
        // SAFETY: 这份引用由本守卫持有，归还一次。
        unsafe { instance.release_object(self.exception.as_ptr()) };
        if instance.pending_exception().map(|value| value.as_ptr() as usize) == Some(pointer) {
            // **旧 pending 由 `PendingRef` 守卫在 Drop 时归还**（同旧 `discard_raised`）。
            let _previous = instance.replace_pending(None);
        }
        crate::error_ledger::record_consumed(instance, self.exception);
    }

    /// **只归还 `Err` 那份，保留 `pending` 槽**（槽里另有自己那份引用保活）。
    ///
    /// 用于"宿主之后还要读 `pending`"的路径（ABI 的 `SystemExit`：CLI 要按它决定退出码）。
    /// 与 `discard` 的区别：**不清槽**；对象因槽里那份引用继续存活。
    pub fn release_keeping_pending(&self) {
        if self.consumed.replace(true) {
            return;
        }
        let instance = self.instance_ref();
        // SAFETY: 这份引用由本守卫持有，归还一次。
        unsafe { instance.release_object(self.exception.as_ptr()) };
        crate::error_ledger::record_consumed(instance, self.exception);
    }

    /// **把 `Err` 那份所有权移交回 `pending` 槽**（不释放对象），交回被顶下来的旧 pending 守卫。
    ///
    /// 语义与旧 `set_pending_exception(Some(..))` 的"接管"一致：旧槽那份由返回的
    /// [`PendingRef`] 在 Drop 时归还。
    pub fn into_pending(self) -> PendingRef {
        let instance = self.instance_ref();
        self.consumed.set(true);
        let pointer = self.exception;
        // SAFETY: 所有权随 pending 槽接手，本守卫不再归还（consumed 已置位）。
        let previous = instance.replace_pending(Some(pointer));
        crate::error_ledger::record_pending_transfer(instance, pointer);
        previous
    }
}

impl Drop for RaiseRef {
    fn drop(&mut self) {
        // **默认只归还 `Err` 那份**（这正是结构性修复的核心：忘记处理也不会漏 `Err` 引用），
        // **不动 `pending` 槽**——它是实例的 Python 可见异常状态（调用方/测试可能要读它；
        // 下一次 `raise`/执行收尾自会替换/清理）。要"连槽一起丢"必须显式 [`RaiseRef::discard`]
        // （吞异常/宿主已取消息的那些路径这么用，与旧 `discard_raised` 同口径）。
        self.release_keeping_pending();
    }
}

impl core::fmt::Debug for RaiseRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RaiseRef")
            .field("exception", &self.exception)
            .field("consumed", &self.consumed.get())
            .finish()
    }
}

impl PartialEq for RaiseRef {
    fn eq(&self, other: &Self) -> bool {
        self.exception == other.exception
    }
}

impl Eq for RaiseRef {}

/// `Instance::replace_pending` 交回的**旧 pending 所有权守卫**（`Drop` 自动释放）。
#[derive(Debug)]
pub struct PendingRef {
    exception: Option<NonNull<Header>>,
    instance: *const Instance,
}

impl PendingRef {
    /// **接管一份 owned 引用**（如"临时保住旧 pending"时手工 incref 的那份）。
    pub fn from_owned(instance: &Instance, exception: NonNull<Header>) -> Self {
        Self {
            exception: Some(exception),
            instance,
        }
    }

    pub(crate) fn from_old(instance: &Instance, exception: Option<NonNull<Header>>) -> Self {
        Self {
            exception,
            instance,
        }
    }

    /// 旧 pending 的裸指针（**借用**；守卫仍负责释放）。
    pub fn get(&self) -> Option<NonNull<Header>> {
        self.exception
    }

    /// 取走所有权（调用方接手释放；本守卫不再处理）。
    pub fn into_inner(mut self) -> Option<NonNull<Header>> {
        let taken = self.exception.take();
        // 已取走 ⇒ Drop 是空操作（Option 已 None）。
        taken
    }
}

impl Drop for PendingRef {
    fn drop(&mut self) {
        let Some(previous) = self.exception.take() else {
            return;
        };
        // SAFETY: 不变量（模块文档）：实例存活；这份引用由本守卫持有。
        let instance = unsafe { &*self.instance };
        unsafe { instance.release_object(previous.as_ptr()) };
    }
}
