//! `Instance` 的中断与异常栈域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// **`AB-5`①**：请求中断本实例（幂等）。
    pub fn request_interrupt(&self) {
        self.interrupted.set(true);
    }

    /// 本实例是否被请求中断（执行器每条指令看它）。
    pub fn interrupted(&self) -> bool {
        self.interrupted.get()
    }

    /// 清掉中断请求（宿主重新开始执行前用；`pa_interrupt` 的配套）。
    pub fn clear_interrupt(&self) {
        self.interrupted.set(false);
    }

    /// **BC-60** ②：压入一个正在处理的异常（**新引用**，由实例接手）。
    pub fn push_exception(&self, exception: NonNull<Header>) {
        self.exception_state.borrow_mut().push(exception);
    }

    /// **BC-60** ②：弹出当前异常（交出一份**新引用**）。
    pub fn pop_exception(&self) -> Option<NonNull<Header>> {
        self.exception_state.borrow_mut().pop()
    }

    /// 最近一次抛出的异常（**借用**；`ExecError::Raised` 借它保活）。
    pub fn pending_exception(&self) -> Option<NonNull<Header>> {
        self.pending_exception.get()
    }

    /// 抛一个内建异常（按名字），返回可直接上抛的执行错误。
    ///
    /// 给**槽位实现**用（宿主函数一类要在 core 之外抛 Python 异常）。
    pub fn raise_builtin_error(&self, name: &str, message: &str) -> crate::ExecError {
        crate::executor::runtime::raise_builtin(self, name, message)
    }
}
