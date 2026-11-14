//! `Instance` 的中断与异常栈域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;


/// 诊断开关的**缓存版**（`PYAWA_EXC_INFO_DEBUG=1`）：异常是热路径（每个 `try/except` 都要
/// push／pop 一次）⇒ 不能每次都读环境变量 ✓。
fn exc_info_debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("PYAWA_EXC_INFO_DEBUG").is_some())
}

impl Instance {
    /// **`AB-5`①**：请求中断本实例（幂等）。
    ///
    /// **原子**（round 12）：宿主可以在执行中从**另一线程**调用（T-CP-10 的取消请求）
    /// ⇒ 标志是 `AtomicBool`，跨线程读写有定义；`SeqCst` 保证"置位后执行线程一定看得到"。
    pub fn request_interrupt(&self) {
        self.interrupted.store(true, core::sync::atomic::Ordering::SeqCst);
    }

    /// 本实例是否被请求中断。
    ///
    /// 执行器**每条指令**看一次；**可取消等待**（`instance/wait.rs`，`CP-27`）在**每个时间片**后
    /// 也看一次（`T-CP-10`）。
    pub fn interrupted(&self) -> bool {
        self.interrupted.load(core::sync::atomic::Ordering::SeqCst)
    }

    /// 清掉中断请求（宿主重新开始执行前用；`pa_interrupt` 的配套）。
    pub fn clear_interrupt(&self) {
        self.interrupted.store(false, core::sync::atomic::Ordering::SeqCst);
    }

    /// **BC-60** ②：压入一个正在处理的异常（**新引用**，由实例接手）。
    pub fn push_exception(&self, exception: NonNull<Header>) {
        if exc_info_debug() {
            let site = self.current_site_with_line();
            self.exception_state_sites.borrow_mut().push(site);
        }
        self.exception_state.borrow_mut().push(exception);
    }

    /// **BC-60** ②：弹出当前异常（交出一份**新引用**）。
    pub fn pop_exception(&self) -> Option<NonNull<Header>> {
        if exc_info_debug() {
            self.exception_state_sites.borrow_mut().pop();
        }
        self.exception_state.borrow_mut().pop()
    }

    /// **截断异常状态到 `depth`**（round 62）：**帧退出时**把本帧压入却没弹出的那些
    /// `PUSH_EXC_INFO` 收掉 ✓（参照里异常状态是**每帧**保存／恢复的；本层只有实例级一张栈
    /// ⇒ 必须在 `execute` 退出时还原到入口深度，否则 `sys.exc_info()` 会看到**上一个已结束函数**
    /// 残留的异常 —— 实测 `import enum` 后 `sys.exc_info()` 给 `KeyError('keep')` ✗）。
    /// 弹出的每一项都是**本实例持有的一份引用** ⇒ 这里逐个归还 ✓。
    pub(crate) fn truncate_exception_state(&self, depth: usize) {
        let mut removed: Vec<NonNull<Header>> = Vec::new();
        {
            let mut stack = self.exception_state.borrow_mut();
            while stack.len() > depth {
                if let Some(exception) = stack.pop() {
                    removed.push(exception);
                }
            }
        }
        for exception in removed {
            // SAFETY: 这份引用由异常状态栈持有（刚交出来）。
            unsafe { self.release_object(exception.as_ptr()) };
        }
        if exc_info_debug() {
            self.exception_state_sites.borrow_mut().truncate(depth);
        }
    }

    /// 诊断（round 62）：当前异常栈里每一项的**压入现场** ✓。
    pub fn exception_state_sites(&self) -> Vec<String> {
        self.exception_state_sites.borrow().clone()
    }

    /// 最近一次抛出的异常（**借用**；`ExecError::Raised` 借它保活）。
    pub fn pending_exception(&self) -> Option<NonNull<Header>> {
        self.pending_exception.get()
    }

    /// 抛一个内建异常（按名字），返回可直接上抛的执行错误。
    ///
    /// 给**槽位实现**用（宿主函数一类要在 core 之外抛 Python 异常）。
    #[track_caller]
    pub fn raise_builtin_error(&self, name: &str, message: &str) -> crate::ExecError {
        crate::executor::runtime::raise_builtin(self, name, message)
    }
}
