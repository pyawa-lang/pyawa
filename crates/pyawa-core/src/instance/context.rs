//! `Instance` 的上下文/异常/回收域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

/// **`OM-11`**：`repr` 递归守卫的进入结果（见 [`Instance::enter_repr`]）。
///
/// 分三支（`P3-41` 第 1 轮 ✓：**崩溃是硬要求**——不能再让深嵌套 `repr` 顶穿原生栈 ✗）：
/// * `Entered`：进入成功；离开由 [`ReprScope`] 的 `Drop` 负责（**含任何提前 `?` 返回** ✓）；
/// * `Cycle`：同一个对象已在生成中 ⇒ 容器按参照给 `[...]`／`{...}` ✓；
/// * `TooDeep`：嵌套深度达到 [`Instance::recursion_limit`] ⇒ 按参照报 `RecursionError` ✓
///   （参照里 `repr` 的 C 递归与 Python 调用共用同一个额度 ⇒ 这里同口径 ✓）。
pub enum ReprEntry<'a> {
    /// 进入成功（离开时由作用域守卫摘除）。
    Entered(ReprScope<'a>),
    /// 自引用（同一个对象已在生成中）。
    Cycle,
    /// 嵌套太深 ⇒ 必须报 `RecursionError`，不得继续吃原生栈。
    TooDeep,
}

/// [`ReprEntry::Entered`] 的守卫：`Drop` 时从 `repr_guard` 里摘掉自己那一条。
pub struct ReprScope<'a> {
    instance: &'a Instance,
    address: usize,
}

impl Drop for ReprScope<'_> {
    fn drop(&mut self) {
        self.instance.leave_repr(self.address);
    }
}

impl Instance {
    /// 把**内建容器**摊成元素表（`bytes(<可迭代>)` 用）。
    ///
    /// 接 `list`／`tuple`／`set`／`frozenset` ✓（元素都是**借用** ✓）；其余可迭代对象
    /// （`bytearray`／`range`／生成器…）如实报未实现（其中多数类型本层还没有，见 `TS-42` 的阶梯）。
    pub fn collect_iterable(
        &self,
        object: NonNull<Header>,
    ) -> Result<Vec<NonNull<Header>>, ExecError> {
        let ty = self.type_of(object);
        if Some(ty) == self.type_named("list") {
            // SAFETY: 类型身份已确认。
            return Ok(unsafe { &*object.as_ptr().cast::<ListObject>() }.items().to_vec());
        }
        if Some(ty) == self.type_named("tuple") {
            // SAFETY: 同上。
            let tuple = unsafe { &*object.as_ptr().cast::<TupleObject>() };
            return Ok((0..tuple.len())
                .filter_map(|index| tuple.item(index))
                .collect());
        }
        // **`set`／`frozenset` 也收** ✓（第 722 轮 ✓）：`Lib/urllib/parse.py:878` 的
        // `_ALWAYS_SAFE_BYTES = bytes(_ALWAYS_SAFE)`（`_ALWAYS_SAFE` 是 `frozenset` ✓）
        // 在**导入期**就调它 ✗ ⇒ 先前那条"只接线了 list／tuple"把 `urllib.parse`
        // ⇒ `email.utils` 一串压在下面 ✓。元素是**借用** ✓（与上面两支同口径 ✓）。
        if Some(ty) == self.type_named("set") || Some(ty) == self.type_named("frozenset") {
            // SAFETY: 类型身份已确认。
            return Ok(unsafe { &*object.as_ptr().cast::<SetObject>() }.items().to_vec());
        }
        Err(ExecError::Unsupported {
            opcode: 0,
            what: "bytes(<可迭代>)：只接线了 list／tuple／set／frozenset（其余走迭代器协议，随后补）",
        })
    }

    /// **`OM-11`**：`repr` 的递归守卫——已经在生成中的对象给 [`ReprEntry::Cycle`]
    /// （容器据此给出 `[...]`／`{...}`，与参照实现一致）；
    /// 嵌套深度超过有效递归上限给 [`ReprEntry::TooDeep`]（调用方报 `RecursionError`）。
    ///
    /// **为什么深度要设限**（`P3-41` ✗✗）：本层 `repr` 是**原生递归** ⇒ 深嵌套容器（如
    /// `test_frame` 的 15 万层嵌套 `dict`）会在守卫来得及之前顶穿 8 MiB 原生栈，
    /// 进程以 `SIGABRT` 退出 ✗。参照的 C 递归守卫与 Python 调用的额度**同一个**
    /// （`Py_EnterRecursiveCall`）⇒ 这里也照 [`Instance::recursion_limit`] 算 ✓。
    pub fn enter_repr(&self, address: usize) -> ReprEntry<'_> {
        let mut guard = self.repr_guard.borrow_mut();
        if guard.contains(&address) {
            return ReprEntry::Cycle;
        }
        if guard.len() >= self.recursion_limit.get() {
            return ReprEntry::TooDeep;
        }
        guard.push(address);
        ReprEntry::Entered(ReprScope {
            instance: self,
            address,
        })
    }

    /// [`ReprEntry::TooDeep`] 对应的 `RecursionError`（消息照参照口径）。
    pub fn repr_recursion_error(&self) -> ExecError {
        self.raise_builtin_error(
            "RecursionError",
            "maximum recursion depth exceeded while getting the repr of an object",
        )
    }

    /// 退出 `repr` 的递归守卫。
    pub fn leave_repr(&self, address: usize) {
        let mut guard = self.repr_guard.borrow_mut();
        if let Some(position) = guard.iter().rposition(|entry| *entry == address) {
            guard.remove(position);
        }
    }

    /// **BC-60** ②：当前异常状态的层数（诊断用——`PUSH_EXC_INFO`／`POP_EXCEPT` 配对着用）。
    pub fn exception_depth(&self) -> usize {
        self.exception_state.borrow().len()
    }

    /// **进入一次 Python 调用**（第 319 轮）：超过 [`MAX_CALL_DEPTH`] ⇒ 返回 `Err` ✓
    /// （调用方据此报 `RecursionError` ✓）。
    ///
    /// 调用方**必须**在返回前配对调用 [`Instance::leave_call`]（含出错路径 ✓）——
    /// 见 `executor` 里 `call_callable` 的用户函数那一支 ✓。
    pub fn enter_call(&self) -> Result<(), ()> {
        let depth = self.call_depth.get();
        if depth >= self.recursion_limit.get() {
            return Err(());
        }
        self.call_depth.set(depth + 1);
        Ok(())
    }

    /// **`sys.getrecursionlimit()`** ✓（task-20 第 3 轮 ✓）：有效上限（默认 `MAX_CALL_DEPTH` ✓）。
    pub fn recursion_limit(&self) -> usize {
        self.recursion_limit.get()
    }

    /// **`sys.setrecursionlimit(n)`** ✓：**只降不升** ✓（本层的 Python 调用吃原生栈 ✗ ——
    /// 见 [`MAX_CALL_DEPTH`] 那段 ✓；调高会顶穿 ⇒ 如实夹住 ✓）。
    pub fn set_recursion_limit(&self, value: usize) -> usize {
        let clamped = value.min(crate::instance::MAX_CALL_DEPTH).max(1);
        self.recursion_limit.set(clamped);
        clamped
    }

    /// **退出一次 Python 调用**（与 [`Instance::enter_call`] 配对 ✓）。
    pub fn leave_call(&self) {
        let depth = self.call_depth.get();
        self.call_depth.set(depth.saturating_sub(1));
    }

    /// **异常的消息文本** ✓（第 193 轮：**先核形状，再读载荷** ✓ —— ABI 与诊断都走这里 ✓，**一处真相** ✓）。
    ///
    /// **为什么必须核** ✗：类型名是异常却**不是** `ExceptionObject` 载荷的对象确实会出现 ✓
    /// （第 192 轮两次插桩都因此**当场段错误** ✗，退出码 139 ✓）⇒ 核两条：① 类型是 `BaseException` 的子类型 ✓；
    /// ② **载荷大小**与 `ExceptionObject` 一致 ✓。对不上就返回 `None` ✓（**绝不**硬读 ✗）。
    pub fn exception_message_of(&self, object: NonNull<Header>) -> Option<String> {
        // SAFETY: object 由调用方保证存活。
        let ty = unsafe { object.as_ref() }.ty();
        // SAFETY: ty 由注册表持有。
        let type_object = unsafe { ty.as_ref() };
        if let Some(base) = self.type_named("BaseException") {
            if !self.is_subtype(ty, base) {
                return None;
            }
        }
        if type_object.instance_size() != core::mem::size_of::<crate::builtin_objects::ExceptionObject>()
        {
            return None;
        }
        // SAFETY: 上面刚核过类型与载荷大小 ✓。
        let payload = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::ExceptionObject>() };
        payload.message_with(self)
    }
}
