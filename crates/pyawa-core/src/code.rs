//! code object —— 帧与执行器需要的元信息（`docs/SPEC-bytecode.md` §2.4、§9）。
//!
//! **当前是最小面**：带"尺寸、字节串与常量表"。`BC-4` 要求的完整 `co_*` 表面（`co_names`／
//! `co_positions()`／`_varname_from_oparg()`… 见 `SPEC-bytecode.md` §2.4）随编译管线一起补——
//! 那需要值表示与容器，现在写会替 `SPEC-object-model.md` §12 做主。

use core::ptr::NonNull;

use crate::header::Header;
use crate::instance::Instance;
use crate::py_object;
use crate::type_object::Slots;

py_object! {
    /// `BC-42`：帧持有它；`BC-4` 的 Python 层可见属性随后补齐。
    pub struct CodeObject {
        /// *占位*：最终是 `str` 对象（`co_name`）。
        name: &'static str,
        /// `BC-43`：值栈上界（`co_stacksize`）。
        stacksize: usize,
        /// `BC-42`／`BC-44`：局部槽数（`co_nlocals`）。
        nlocals: usize,
        /// `BC-45`：cell 槽数（`co_cellvars` 的条数）。
        ncellvars: usize,
        /// `BC-45`：free 槽数（`co_freevars` 的条数）。
        nfreevars: usize,
        /// `BC-33`：码元字节串，**每码元 2 字节**（`opcode: u8` ＋ `oparg: u8`）。
        code: Vec<u8>,
        /// `BC-54`：异常表字节串（base-64 varint，`dis._parse_exception_table` 会原样解析它）。
        /// 本层只**保存**；解析与游标推进随执行器补。
        exceptiontable: Vec<u8>,
        /// `BC-4`：常量表（`co_consts`）。**裸引用**（`OM-40`）：只在 `clear`／`traverse` 里释放。
        consts: Vec<Option<NonNull<Header>>>,
    }
}

impl CodeObject {
    /// 注册这个类型时的槽位表：常量表会持有对象引用，因此要 `traverse`／`clear`
    /// （`OM-12`：code object 经常量表可能与别的对象成环）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(code_traverse)
            .with_clear(code_clear)
    }

    /// `co_name` 的占位。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// `BC-43`：值栈上界。
    pub fn stacksize(&self) -> usize {
        self.stacksize
    }

    /// `BC-44`：局部槽数。
    pub fn nlocals(&self) -> usize {
        self.nlocals
    }

    /// `BC-45`：cell 槽数。
    pub fn ncellvars(&self) -> usize {
        self.ncellvars
    }

    /// `BC-45`：free 槽数。
    pub fn nfreevars(&self) -> usize {
        self.nfreevars
    }

    /// `BC-33`：码元字节串（每码元 2 字节）。
    pub fn code(&self) -> &[u8] {
        &self.code
    }

    /// `BC-54`：异常表字节串（本层不解析）。
    pub fn exceptiontable(&self) -> &[u8] {
        &self.exceptiontable
    }

    /// `BC-33`：码元数（每码元 2 字节）。
    pub fn instruction_count(&self) -> usize {
        self.code.len() / 2
    }

    /// `BC-4`：常量表条数（`co_consts` 的长度）。
    pub fn const_count(&self) -> usize {
        self.consts.len()
    }

    /// `BC-4`：取常量表里的一项（**借用**的裸引用；要持有请用 [`Instance::own`]）。
    pub fn constant(&self, index: usize) -> Option<NonNull<Header>> {
        self.consts.get(index).copied().flatten()
    }

    /// 写入一项常量：`value` 是**新引用**，返回被顶下来的旧引用（调用方负责释放）。
    pub fn set_constant(
        &mut self,
        index: usize,
        value: Option<NonNull<Header>>,
    ) -> Option<NonNull<Header>> {
        core::mem::replace(&mut self.consts[index], value)
    }
}

/// `OM-40`：列出常量表里的引用。
unsafe fn code_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let code = unsafe { &*ptr.cast::<CodeObject>() };
    for slot in &code.consts {
        if let Some(value) = slot {
            visit(value.as_ptr());
        }
    }
}

/// `OM-40`／`OM-20` ②：交出并释放常量表里的引用。
unsafe fn code_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let code = unsafe { &*ptr.cast::<CodeObject>() };
    for slot in &code.consts {
        if let Some(value) = slot {
            // SAFETY: 该引用由本对象持有，这里交还一份。
            unsafe { instance.release_object(value.as_ptr()) };
        }
    }
}
