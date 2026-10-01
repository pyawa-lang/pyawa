//! code object —— 帧与执行器需要的元信息（`docs/SPEC-bytecode.md` §2.4、§9）。
//!
//! **当前是最小面**：只带"尺寸与字节串"。`BC-4` 要求的完整 `co_*` 表面（`co_consts`／`co_names`／
//! `co_positions()`／`_varname_from_oparg()`… 见 `SPEC-bytecode.md` §2.4）随编译管线一起补——
//! 那需要值表示与容器，现在写会替 `SPEC-object-model.md` §12 做主。

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
    }
}

impl CodeObject {
    /// 注册这个类型时的槽位表：code object 不持有任何对象引用，只要 `dealloc`。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
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
}
