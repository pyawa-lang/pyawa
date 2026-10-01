//! 帧（`docs/SPEC-bytecode.md` §9，**BC-42**…**BC-48**）。
//!
//! **BC-48**：帧必须是对象，且将来能在 Python 层观察（`BC-7`）。载荷按 `OM-40` 存**裸引用**，
//! 只在 `clear`／`traverse`／`dealloc` 里释放——所以帧本身带 `traverse`／`clear`、被标 `GC_TRACKED`
//! （`OM-12`：帧引用闭包 cell，而 cell 引用帧，是典型的环）。
//!
//! 本层只做**布局与托管**：值栈的进出、局部槽与 cell 槽、指令指针与异常表游标、可挂起状态。
//! 指令的**语义**由执行器（下一步）按 `BC-49` 的起步指令集实现。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use crate::code::CodeObject;
use crate::header::Header;
use crate::instance::Instance;
use crate::py_object;
use crate::refcount::Owned;
use crate::type_object::{Slots, TypeObject};

/// 帧操作的失败形态。**BC-43**：越界**必须**报错，**禁止** UB 或静默扩容。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// 值栈已达 `co_stacksize`。失败时调用方**仍持有**那个引用。
    StackOverflow { capacity: usize },
    /// 值栈为空时弹出。
    StackUnderflow,
    /// 局部槽／cell 槽下标越界。
    SlotOutOfRange { slot: usize, count: usize },
    /// 已经挂起／尚未挂起时做了相反的操作。
    WrongSuspendState { suspended: bool },
}

/// **BC-47**：可挂起帧的恢复点——指令指针 ＋ 值栈镜像 ＋ 异常表游标。
///
/// 值栈是**搬进来**的（不是复制）：每一项的引用归属不变，因此不会重复计数。
/// 公开是为了让 `Frame` 的字段可见性与 `py_object!` 生成物一致（字段都是 `pub`）。
pub struct ResumePoint {
    /// 恢复时回到的码元偏移（`BC-33`）。
    pub instruction_pointer: usize,
    /// 恢复时的异常表游标（字节偏移，`BC-54`）。
    pub exception_cursor: usize,
    /// 值栈镜像（**持有**其中每一项的引用）。
    pub stack: Vec<NonNull<Header>>,
}

py_object! {
    /// **BC-42** 要求的最小字段集。
    pub struct Frame {
        /// 本帧执行的 code object：**帧持有它的一份引用**（`OM-16`）。
        code: RefCell<Option<NonNull<Header>>>,
        /// **BC-42**／**BC-44**：局部槽数组，长度 = `co_nlocals`。
        locals: RefCell<Vec<Option<NonNull<Header>>>>,
        /// **`LOAD_NAME`／`STORE_NAME` 的落点**：类体／模块帧的"局部变量"是一个**映射**
        /// （`dict`），而不是槽数组——`__build_class__` 把类命名空间交给类体帧。
        namespace: RefCell<Option<NonNull<Header>>>,
        /// **`LOAD_GLOBAL`** 用的全局映射（`BC-57`）：函数帧取函数的 `__globals__`，
        /// 类体帧取定义处那一层，模块体没有单独的一层（此时就是它的命名空间）。
        globals: RefCell<Option<NonNull<Header>>>,
        /// **BC-42**／**BC-43**：值栈，深度上界 = `co_stacksize`。
        stack: RefCell<Vec<NonNull<Header>>>,
        /// **BC-45**：cell 槽数组，**独立于** `locals`。
        cells: RefCell<Vec<Option<NonNull<Header>>>>,
        /// **BC-42**：指令指针，单位是**码元**（`BC-33`：每码元 2 字节）。
        instruction_pointer: Cell<usize>,
        /// **BC-42**：异常表游标（字节偏移，`BC-54`）。
        exception_cursor: Cell<usize>,
        /// **BC-47**：挂起时的恢复点；未挂起为 `None`。
        resume: RefCell<Option<ResumePoint>>,
        /// **恢复时要先抛的异常**（生成器的 `throw`／`close` 用）。
        ///
        /// 参照实现里"抛在挂起点"就是这条路径：恢复前把异常放这儿，`execute` 一恢复就按
        /// **本帧自己的**异常表派发它（所以生成器体里的 `try/except` 能接住它）。
        pending_raise: RefCell<Option<NonNull<Header>>>,
        /// 值栈上界（从 code object 抄一份，避免每次入栈都借 `code`）。
        stacksize: usize,
        /// 是否处于挂起状态（`BC-47`）。
        suspended: Cell<bool>,
    }
}

impl Frame {
    /// 注册这个类型时的槽位表：`dealloc` ＋ `traverse`／`clear`（`OM-12`：帧可成环）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(frame_traverse)
            .with_clear(frame_clear)
    }

    /// 按 code object 的尺寸建帧，并**复制一份对 code 的引用**由帧持有（`BC-42`）。
    pub fn for_code(ty: NonNull<TypeObject>, code: &Owned<'_, CodeObject>) -> Self {
        let info = code.get();
        let code_reference = code.clone().into_raw().cast::<Header>();
        Self {
            header: Header::new(ty),
            code: RefCell::new(Some(code_reference)),
            locals: RefCell::new(vec![None; info.nlocals()]),
            namespace: RefCell::new(None),
            globals: RefCell::new(None),
            stack: RefCell::new(Vec::with_capacity(info.stacksize())),
            cells: RefCell::new(vec![None; info.ncellvars() + info.nfreevars()]),
            instruction_pointer: Cell::new(0),
            exception_cursor: Cell::new(0),
            pending_raise: RefCell::new(None),
            resume: RefCell::new(None),
            stacksize: info.stacksize(),
            suspended: Cell::new(false),
        }
    }

    /// **`__build_class__`**：造一个把局部变量放在**映射**里的帧（类体／模块级代码用）。
    ///
    /// `namespace` 是**新引用**，由帧接手（`traverse`／`clear` 会释放它）。
    pub fn for_code_with_namespace(
        ty: NonNull<TypeObject>,
        code: &Owned<'_, CodeObject>,
        namespace: NonNull<Header>,
    ) -> Self {
        let frame = Self::for_code(ty, code);
        *frame.namespace.borrow_mut() = Some(namespace);
        frame
    }

    /// 本帧的命名空间映射（**借用**；不是映射帧则为 `None`）。
    pub fn namespace(&self) -> Option<NonNull<Header>> {
        *self.namespace.borrow()
    }

    /// **`BC-57`**：本帧的全局映射（**借用**；没有就是 `None`）。
    ///
    /// 模块体没有单独的一层 ⇒ 调用方按"命名空间即全局"处理（`effective_globals`）。
    pub fn globals(&self) -> Option<NonNull<Header>> {
        *self.globals.borrow()
    }

    /// 设置全局映射（**新引用**，由帧接手）。
    pub fn set_globals(&self, mapping: NonNull<Header>) {
        *self.globals.borrow_mut() = Some(mapping);
    }

    /// 本帧的**有效全局映射**：显式的全局表，没有就是命名空间（模块体）。
    pub fn effective_globals(&self) -> Option<NonNull<Header>> {
        self.globals().or_else(|| self.namespace())
    }

    /// 帧持有的 code object 裸引用（**借用**）。
    pub fn code(&self) -> Option<NonNull<Header>> {
        *self.code.borrow()
    }

    /// **BC-43**：值栈上界。
    pub fn stacksize(&self) -> usize {
        self.stacksize
    }

    /// 值栈当前深度。
    pub fn depth(&self) -> usize {
        self.stack.borrow().len()
    }

    /// **BC-43**：入栈。`value` 是**新引用**（`OM-16`），成功后由帧托管；
    /// 失败（栈满）时**引用仍归调用方**。
    pub fn push(&self, value: NonNull<Header>) -> Result<(), FrameError> {
        let mut stack = self.stack.borrow_mut();
        if stack.len() >= self.stacksize {
            return Err(FrameError::StackOverflow { capacity: self.stacksize });
        }
        stack.push(value);
        Ok(())
    }

    /// 交换栈顶与"从栈顶往下第 `index` 项"（**1 起数**；参照实现的 `SWAP(i)`）。
    ///
    /// `index < 2` 是空操作（自己换自己）；越界报 `StackUnderflow`。
    pub fn swap_from_top(&self, index: usize) -> Result<(), FrameError> {
        let mut stack = self.stack.borrow_mut();
        let length = stack.len();
        if index == 0 || index > length {
            return Err(FrameError::StackUnderflow);
        }
        stack.swap(length - 1, length - index);
        Ok(())
    }

    /// **BC-43**／**BC-46**：出栈并**交出**那份引用；调用方随后必须按 `OM-20` 处理它。
    pub fn pop(&self) -> Result<NonNull<Header>, FrameError> {
        self.stack.borrow_mut().pop().ok_or(FrameError::StackUnderflow)
    }

    /// 只看栈顶（**借用**，不转移所有权）。
    pub fn peek(&self) -> Result<NonNull<Header>, FrameError> {
        self.stack
            .borrow()
            .last()
            .copied()
            .ok_or(FrameError::StackUnderflow)
    }

    /// 从栈顶往下第 `index` 项（**1 起数**，`1` 就是栈顶；**借用**）。
    ///
    /// 与参照实现的 `PEEK(n)` 同一约定——`LIST_APPEND` 一类指令的 oparg 就是它。
    pub fn peek_from_top(&self, index: usize) -> Result<NonNull<Header>, FrameError> {
        let stack = self.stack.borrow();
        if index == 0 || index > stack.len() {
            return Err(FrameError::StackUnderflow);
        }
        Ok(stack[stack.len() - index])
    }

    /// **BC-44**：局部槽数。
    pub fn local_count(&self) -> usize {
        self.locals.borrow().len()
    }

    /// 读局部槽（**借用**）。
    pub fn local(&self, slot: usize) -> Result<Option<NonNull<Header>>, FrameError> {
        let locals = self.locals.borrow();
        locals
            .get(slot)
            .copied()
            .ok_or(FrameError::SlotOutOfRange { slot, count: locals.len() })
    }

    /// 写局部槽：`value` 是**新引用**；返回被顶下来的旧引用，**调用方负责释放**。
    pub fn set_local(
        &self,
        slot: usize,
        value: Option<NonNull<Header>>,
    ) -> Result<Option<NonNull<Header>>, FrameError> {
        let mut locals = self.locals.borrow_mut();
        let count = locals.len();
        match locals.get_mut(slot) {
            Some(slot) => Ok(core::mem::replace(slot, value)),
            None => Err(FrameError::SlotOutOfRange { slot, count }),
        }
    }

    /// **BC-45**：cell／free 槽数。
    pub fn cell_count(&self) -> usize {
        self.cells.borrow().len()
    }

    /// 读 cell 槽（**借用**）。
    pub fn cell(&self, slot: usize) -> Result<Option<NonNull<Header>>, FrameError> {
        let cells = self.cells.borrow();
        cells
            .get(slot)
            .copied()
            .ok_or(FrameError::SlotOutOfRange { slot, count: cells.len() })
    }

    /// 写 cell 槽：`value` 是**新引用**；返回旧引用，**调用方负责释放**。
    pub fn set_cell(
        &self,
        slot: usize,
        value: Option<NonNull<Header>>,
    ) -> Result<Option<NonNull<Header>>, FrameError> {
        let mut cells = self.cells.borrow_mut();
        let count = cells.len();
        match cells.get_mut(slot) {
            Some(slot) => Ok(core::mem::replace(slot, value)),
            None => Err(FrameError::SlotOutOfRange { slot, count }),
        }
    }

    /// **BC-42**：指令指针（**码元**单位）。
    pub fn instruction_pointer(&self) -> usize {
        self.instruction_pointer.get()
    }

    /// 设置指令指针（跳转／推进）。
    pub fn set_instruction_pointer(&self, offset: usize) {
        self.instruction_pointer.set(offset);
    }

    /// **BC-42**：异常表游标（字节偏移）。
    pub fn exception_cursor(&self) -> usize {
        self.exception_cursor.get()
    }

    /// 设置异常表游标。
    pub fn set_exception_cursor(&self, offset: usize) {
        self.exception_cursor.set(offset);
    }

    /// 是否挂起（`BC-47`）。
    pub fn is_suspended(&self) -> bool {
        self.suspended.get()
    }

    /// **BC-47**：挂起——把值栈**整体搬进**恢复点，并记下指令指针与异常表游标。
    /// 值栈项的所有权不变（只是换了存放位置），因此不会重复计数。
    pub fn suspend(&self) -> Result<(), FrameError> {
        if self.suspended.get() {
            return Err(FrameError::WrongSuspendState { suspended: true });
        }
        let stack = core::mem::take(&mut *self.stack.borrow_mut());
        *self.resume.borrow_mut() = Some(ResumePoint {
            instruction_pointer: self.instruction_pointer.get(),
            exception_cursor: self.exception_cursor.get(),
            stack,
        });
        self.suspended.set(true);
        Ok(())
    }

    /// **BC-47**：从恢复点继续——指令指针、值栈镜像、异常表游标一并还原。
    /// 放一个"恢复时要先抛"的异常（**新引用**，由帧接手；返回被顶下来的旧值）。
    pub fn set_pending_raise(
        &self,
        exception: Option<NonNull<Header>>,
    ) -> Option<NonNull<Header>> {
        self.pending_raise.replace(exception)
    }

    /// 取走"恢复时要先抛"的异常（**交出引用**，调用方按 `OM-20` 处理）。
    pub fn take_pending_raise(&self) -> Option<NonNull<Header>> {
        self.pending_raise.borrow_mut().take()
    }

    pub fn resume(&self) -> Result<(), FrameError> {
        if !self.suspended.get() {
            return Err(FrameError::WrongSuspendState { suspended: false });
        }
        if let Some(point) = self.resume.borrow_mut().take() {
            self.instruction_pointer.set(point.instruction_pointer);
            self.exception_cursor.set(point.exception_cursor);
            *self.stack.borrow_mut() = point.stack;
        }
        self.suspended.set(false);
        Ok(())
    }

    /// 供测试与调试：挂起时恢复点的（指令指针，异常表游标，栈深）；未挂起为 `None`。
    pub fn resume_point(&self) -> Option<(usize, usize, usize)> {
        self.resume
            .borrow()
            .as_ref()
            .map(|point| (point.instruction_pointer, point.exception_cursor, point.stack.len()))
    }
}

/// `BC-46`／`OM-40`：列出帧直接持有的全部引用。
unsafe fn frame_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let frame = unsafe { &*ptr.cast::<Frame>() };
    if let Some(code) = frame.code() {
        visit(code.as_ptr());
    }
    if let Some(namespace) = frame.namespace() {
        visit(namespace.as_ptr());
    }
    if let Some(globals) = frame.globals() {
        visit(globals.as_ptr());
    }
    if let Some(exception) = *frame.pending_raise.borrow() {
        visit(exception.as_ptr());
    }
    for slot in frame.locals.borrow().iter() {
        if let Some(value) = slot {
            visit(value.as_ptr());
        }
    }
    for value in frame.stack.borrow().iter() {
        visit(value.as_ptr());
    }
    for slot in frame.cells.borrow().iter() {
        if let Some(value) = slot {
            visit(value.as_ptr());
        }
    }
    if let Some(point) = frame.resume.borrow().as_ref() {
        for value in &point.stack {
            visit(value.as_ptr());
        }
    }
}

/// `BC-46`／`OM-20` ②：交出并释放帧持有的全部引用（`OM-21`：逐个释放，不递归）。
unsafe fn frame_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let frame = unsafe { &*ptr.cast::<Frame>() };

    if let Some(code) = frame.code.borrow_mut().take() {
        // SAFETY: 该引用由本帧持有，这里交还一份。
        unsafe { instance.release_object(code.as_ptr()) };
    }
    if let Some(exception) = frame.pending_raise.borrow_mut().take() {
        // SAFETY: 这份引用由帧持有。
        unsafe { instance.release_object(exception.as_ptr()) };
    }
    if let Some(globals) = frame.globals.borrow_mut().take() {
        // SAFETY: 这份引用由帧持有。
        unsafe { instance.release_object(globals.as_ptr()) };
    }
    if let Some(namespace) = frame.namespace.borrow_mut().take() {
        // SAFETY: 同上。
        unsafe { instance.release_object(namespace.as_ptr()) };
    }
    for slot in frame.locals.borrow_mut().iter_mut() {
        if let Some(value) = slot.take() {
            // SAFETY: 同上。
            unsafe { instance.release_object(value.as_ptr()) };
        }
    }
    for value in core::mem::take(&mut *frame.stack.borrow_mut()) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    for slot in frame.cells.borrow_mut().iter_mut() {
        if let Some(value) = slot.take() {
            // SAFETY: 同上。
            unsafe { instance.release_object(value.as_ptr()) };
        }
    }
    if let Some(point) = frame.resume.borrow_mut().take() {
        for value in point.stack {
            // SAFETY: 同上。
            unsafe { instance.release_object(value.as_ptr()) };
        }
    }
}
