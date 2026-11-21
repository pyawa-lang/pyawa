//! **`AB-75`／`AB-73` 的宿主对象协议钩子（core 侧）**：类型／实例／模块统一、**每对象一份**。
//!
//! 设计（`PLAN §12.12` 步 3；`SPEC-c-abi` §17「语义唯一」）：
//! - **存储**：`Instance` 内一张「对象地址 → 回调」表（类型对象也是对象 ⇒ 类型态与实例态同一张表，
//!   互不覆盖：键不同）；
//! - **派发**：属性读/写**普通规则全未命中**时，先查**对象自身**的条目，再查**其类型对象**的条目
//!   （类型态是"该类型所有实例的默认"，实例态覆盖之）；模块对象也走同一条（模块是普通对象）；
//! - **生命周期**：对象释放（`release_one`）时摘条目；实例销毁时整表随实例释放；
//! - **不透明 `token`**：core 不解释，只原样传给 ABI 闭包（ABI 用它找回 `pa_type_ops` 副本）。
//!
//! core **不**知道宿主回调的语义：闭包的签名就是 core 的 `HostObjectGetAttr`／`HostObjectSetAttr`，
//! 这样"只有一条属性回退路径"（`AB-73`/`AB-75` 的同一性）。

use core::ptr::NonNull;

use crate::header::Header;
use crate::instance::Instance;
use crate::ExecError;

/// 宿主对象**属性回退读**回调（`AB-73` 的 `getattr_fn` 的 core 形态）。
///
/// `Ok(None)` ⇒ 宿主"未处理"，VM 继续默认（最终 `AttributeError`）；`Err` ⇒ 原样上抛（宿主错误
/// 不得静默回退，`AB-22`）。`token` 由 [`HostObjectOps`] 的同一份记录给出（core 不解释）。
pub type HostObjectGetAttr =
    unsafe fn(*mut Header, &str, &Instance, usize) -> Result<Option<NonNull<Header>>, ExecError>;

/// 宿主对象**属性回退写**回调（`AB-73` 的 `setattr_fn` 的 core 形态）。
///
/// `Ok(true)` ⇒ 宿主已处理（VM 负责释放交出的那份值）；`Ok(false)` ⇒ 未处理；`Err` ⇒ 原样上抛。
pub type HostObjectSetAttr =
    unsafe fn(*mut Header, &str, Option<NonNull<Header>>, &Instance, usize) -> Result<bool, ExecError>;

/// 宿主对象 **await 开始** 回调（`AB-88`）：返回 provider token；`Ok(None)` 表示未处理。
pub type HostObjectAwait =
    unsafe fn(*mut Header, &Instance, usize) -> Result<Option<u64>, ExecError>;

/// 宿主对象 **await 取消** 回调（`AB-89`）：reason 0=INTERRUPT／1=DESTROY／2=THROW。
pub type HostObjectAwaitCancel =
    unsafe fn(*mut Header, &Instance, usize, i32) -> Result<(), ExecError>;

/// 一个对象的宿主协议钩子（**拷贝**语义；记录本身由 ABI 层持有）。
#[derive(Clone, Copy)]
pub struct HostObjectOps {
    /// 属性回退读（`None` ⇒ 不派发）。
    pub getattr: Option<HostObjectGetAttr>,
    /// 属性回退写（`None` ⇒ 不派发）。
    pub setattr: Option<HostObjectSetAttr>,
    /// 宿主 await 开始（`AB-88`；`None` ⇒ 不派发）。
    pub await_fn: Option<HostObjectAwait>,
    /// 宿主 await 取消（`AB-89`；可选）。
    pub await_cancel: Option<HostObjectAwaitCancel>,
    /// 透传给回调的不透明字（ABI 侧记录指针；core 只存/传）。
    pub token: usize,
}

impl Instance {
    /// **`AB-75`**：给任意对象（类型／实例／模块）装/清宿主协议钩子（**每对象一份**）。
    ///
    /// 替换时旧条目由本函数丢弃（`HostObjectOps` 是 `Copy`，不需要额外释放）；`None` ⇒ 清除。
    pub fn set_object_ops(&self, object: NonNull<Header>, ops: Option<HostObjectOps>) {
        let key = object.as_ptr() as usize;
        let mut table = self.object_ops.borrow_mut();
        match ops {
            Some(ops) => {
                table.insert(key, ops);
            }
            None => {
                table.remove(&key);
            }
        }
    }

    /// 对象**自身**的条目（不含类型继承）。
    pub(crate) fn object_ops(&self, object: NonNull<Header>) -> Option<HostObjectOps> {
        self.object_ops
            .borrow()
            .get(&(object.as_ptr() as usize))
            .copied()
    }

    /// 对象**类型对象**的条目（类型态；实例没有自身条目时的默认）。
    pub(crate) fn type_object_ops(&self, object: NonNull<Header>) -> Option<HostObjectOps> {
        let ty = self.type_of(object);
        self.object_ops
            .borrow()
            .get(&(ty.as_ptr() as usize))
            .copied()
    }

    /// 该对象是否可能有宿主协议钩子（自身或类型态）——`iter`/`arithmetic` 的快路预筛用。
    pub fn has_host_object_ops(&self, object: NonNull<Header>) -> bool {
        // **一次借用**（避免 `||` 两侧各借一次 ⇒ `RefCell` 已借用 panic）。
        let table = self.object_ops.borrow();
        if table.contains_key(&(object.as_ptr() as usize)) {
            return true;
        }
        let ty = self.type_of(object);
        table.contains_key(&(ty.as_ptr() as usize))
    }

    /// 属性读的回退入口（`AB-73`/`AB-75`）：自身条目优先，其次类型态；都没有 ⇒ `Ok(None)`。
    pub(crate) fn host_object_getattr(
        &self,
        object: NonNull<Header>,
        name: &str,
    ) -> Result<Option<NonNull<Header>>, ExecError> {
        let ops = self
            .object_ops(object)
            .or_else(|| self.type_object_ops(object));
        match ops.and_then(|ops| ops.getattr.map(|getattr| (getattr, ops.token))) {
            Some((getattr, token)) => {
                // SAFETY: 闭包由 ABI 层注册，契约见 `HostObjectGetAttr`。
                unsafe { getattr(object.as_ptr(), name, self, token) }
            }
            None => Ok(None),
        }
    }

    /// 属性写的回退入口：自身条目优先，其次类型态；都没有 ⇒ `Ok(None)`（未处理）。
    pub(crate) fn host_object_setattr(
        &self,
        object: NonNull<Header>,
        name: &str,
        value: Option<NonNull<Header>>,
    ) -> Result<Option<bool>, ExecError> {
        let ops = self
            .object_ops(object)
            .or_else(|| self.type_object_ops(object));
        match ops.and_then(|ops| ops.setattr.map(|setattr| (setattr, ops.token))) {
            Some((setattr, token)) => {
                // SAFETY: 闭包由 ABI 层注册，契约见 `HostObjectSetAttr`。
                Ok(Some(unsafe { setattr(object.as_ptr(), name, value, self, token) }?))
            }
            None => Ok(None),
        }
    }

    /// 对象释放时摘掉它的条目（`AB-75` 的生命周期：随对象释放）。
    pub(crate) fn forget_object_ops(&self, object: NonNull<Header>) {
        self.object_ops
            .borrow_mut()
            .remove(&(object.as_ptr() as usize));
    }
}

impl Instance {
    /// **协议派发统一入口（宿主优先）**：供 stdlib（`format`／`hash`／`abs` 等宿主内建）复用。
    /// 宿主 `PA_ERR_NOTFOUND` 才落回类型/实例属性通道（`call_special_method`）。
    pub fn call_protocol_method(
        &self,
        object: NonNull<Header>,
        name: &str,
        args: &[NonNull<Header>],
    ) -> Result<Option<NonNull<Header>>, ExecError> {
        crate::executor::protocol::call_protocol_method(self, object, name, args)
    }
}

/// **`AB-88`**：宿主 await 开始的 core 入口；`Ok(None)` ⇒ 不是宿主 awaitable。
impl Instance {
    pub(crate) fn host_object_await(
        &self,
        object: NonNull<Header>,
    ) -> Result<Option<u64>, ExecError> {
        let ops = self
            .object_ops(object)
            .or_else(|| self.type_object_ops(object));
        match ops.and_then(|ops| ops.await_fn.map(|f| (f, ops.token))) {
            Some((await_fn, token)) => {
                // SAFETY: 闭包由 ABI 层注册，契约见 `HostObjectAwait`。
                unsafe { await_fn(object.as_ptr(), self, token) }
            }
            None => Ok(None),
        }
    }

    /// **`AB-89`**：通知宿主 await 被取消/销毁/注入异常。
    #[allow(dead_code)] // S3.1：取消/销毁/throw 的派发路径；接口先落地。
    pub(crate) fn host_object_await_cancel(
        &self,
        object: NonNull<Header>,
        reason: i32,
    ) -> Result<(), ExecError> {
        let ops = self
            .object_ops(object)
            .or_else(|| self.type_object_ops(object));
        match ops.and_then(|ops| ops.await_cancel.map(|f| (f, ops.token))) {
            Some((cancel, token)) => {
                // SAFETY: 同上。
                unsafe { cancel(object.as_ptr(), self, token, reason) }
            }
            None => Ok(()),
        }
    }

    /// 记录“某对象正处于宿主 await 中”（token 由宿主解释；VM 只用于 SEND 派发）。
    pub(crate) fn begin_host_await(&self, object: NonNull<Header>, token: u64) {
        self.host_awaiting
            .borrow_mut()
            .insert(object.as_ptr() as usize, (token, false));
    }

    /// 取某对象的 await 状态：`(token, 是否已经 yield 过 token)`。
    pub(crate) fn host_await_state(&self, object: NonNull<Header>) -> Option<(u64, bool)> {
        self.host_awaiting
            .borrow()
            .get(&(object.as_ptr() as usize))
            .copied()
    }

    pub(crate) fn mark_host_await_yielded(&self, object: NonNull<Header>) {
        if let Some(state) = self
            .host_awaiting
            .borrow_mut()
            .get_mut(&(object.as_ptr() as usize))
        {
            state.1 = true;
        }
    }

    pub(crate) fn finish_host_await(&self, object: NonNull<Header>) {
        self.host_awaiting
            .borrow_mut()
            .remove(&(object.as_ptr() as usize));
    }

    pub(crate) fn forget_host_await(&self, object: NonNull<Header>) {
        self.host_awaiting
            .borrow_mut()
            .remove(&(object.as_ptr() as usize));
    }
}
