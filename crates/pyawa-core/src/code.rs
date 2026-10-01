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
        /// `BC-4`：`co_qualname`（*临时*：Rust 字符串，Python 可见的是 `str`）。
        qualname: String,
        /// `BC-4`：`co_filename`（*临时*：同上）。
        filename: String,
        /// `BC-4`：`co_firstlineno`。
        firstlineno: usize,
        /// `BC-43`：值栈上界（`co_stacksize`）。
        stacksize: usize,
        /// `BC-42`／`BC-44`：局部槽数（`co_nlocals`）。
        nlocals: usize,
        /// `BC-4`：位置参数个数（`co_argcount`，含仅位置参数）。
        argcount: usize,
        /// `BC-4`：仅位置参数个数（`co_posonlyargcount`）。
        posonlyargcount: usize,
        /// `BC-4`：仅关键字参数个数（`co_kwonlyargcount`）。
        kwonlyargcount: usize,
        /// `BC-4`：标志位（`co_flags`）。**只**用到位 4／8（`CO_VARARGS`／`CO_VARKEYWORDS`，实测值 4／8）。
        flags: u32,
        /// `BC-4`：局部名表（`co_varnames`）。*临时*：内部用 Rust 字符串，
        /// Python 可见的 `tuple[str]` 随 `getattr` 槽位再接。
        varnames: Vec<String>,
        /// `BC-4`：全局／属性名表（`co_names`）。`LOAD_ATTR` 一族的 oparg 是它的下标。
        names: Vec<String>,
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
    /// （`OM-12`：code object 经常量表可能与别的对象成环）；
    /// 属性走 `OM-11` 的 `getattr` 槽（`BC-4` 的 `co_*` 是**计算型**属性，不是字典常量）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(code_traverse)
            .with_clear(code_clear)
            .with_getattr(code_getattr)
            .with_repr(crate::builtin_objects::code_repr)
    }

    /// `co_name` 的占位。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// `BC-4`：`co_qualname`。
    pub fn qualname(&self) -> &str {
        &self.qualname
    }

    /// `BC-4`：`co_filename`。
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// `BC-4`：`co_firstlineno`。
    pub fn firstlineno(&self) -> usize {
        self.firstlineno
    }

    /// `BC-43`：值栈上界。
    pub fn stacksize(&self) -> usize {
        self.stacksize
    }

    /// `BC-44`：局部槽数。
    pub fn nlocals(&self) -> usize {
        self.nlocals
    }

    /// `BC-4`：位置参数个数（含仅位置参数）。
    pub fn argcount(&self) -> usize {
        self.argcount
    }

    /// `BC-4`：仅位置参数个数。
    pub fn posonlyargcount(&self) -> usize {
        self.posonlyargcount
    }

    /// `BC-4`：仅关键字参数个数。
    pub fn kwonlyargcount(&self) -> usize {
        self.kwonlyargcount
    }

    /// `BC-4`：标志位。
    pub fn flags(&self) -> u32 {
        self.flags
    }

    /// `BC-4`：第 `index` 个名字表项（`co_names`）；`LOAD_ATTR` 一族按它找属性名。
    pub fn name_at(&self, index: usize) -> Option<&str> {
        self.names.get(index).map(String::as_str)
    }

    /// `BC-4`／**BC-56**：第 `slot` 个局部槽的名字（参数绑定按名字匹配关键字实参）。
    pub fn varname(&self, slot: usize) -> Option<&str> {
        self.varnames.get(slot).map(String::as_str)
    }

    /// `BC-56`：是否收多余位置实参（`CO_VARARGS`，实测 4）。
    pub fn has_varargs(&self) -> bool {
        self.flags & 0x04 != 0
    }

    /// `BC-56`：是否收未知关键字（`CO_VARKEYWORDS`，实测 8）。
    pub fn has_varkeywords(&self) -> bool {
        self.flags & 0x08 != 0
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

/// `BC-4` 的 `co_*` 属性（`OM-11` 的 `getattr` 槽）：**计算型**属性，返回**新引用**。
///
/// 已接线：`co_name`／`co_qualname`／`co_filename`／`co_firstlineno`／`co_argcount`／`co_posonlyargcount`／`co_kwonlyargcount`／`co_nlocals`／
/// `co_stacksize`／`co_flags`／`co_ncellvars`／`co_nfreevars`／`co_varnames`／`co_names`／`co_consts`。
/// **未接线**：`co_code`／`co_exceptiontable`（要 `bytes` 类型，`TS-42` 排在 M3+）、
/// `co_positions()`／`co_lines()`（要方法调用与 tuple 迭代，且要行号表）。
pub unsafe fn code_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let code = unsafe { &*ptr.cast::<CodeObject>() };
    let integer = |value: usize| Some(instance.new_int(value as i64));
    match name {
        "co_name" => Some(instance.new_str(code.name())),
        "co_qualname" => Some(instance.new_str(code.qualname())),
        "co_filename" => Some(instance.new_str(code.filename())),
        "co_firstlineno" => integer(code.firstlineno()),
        "co_argcount" => integer(code.argcount()),
        "co_posonlyargcount" => integer(code.posonlyargcount()),
        "co_kwonlyargcount" => integer(code.kwonlyargcount()),
        "co_nlocals" => integer(code.nlocals()),
        "co_stacksize" => integer(code.stacksize()),
        "co_flags" => integer(code.flags() as usize),
        "co_ncellvars" => integer(code.ncellvars()),
        "co_nfreevars" => integer(code.nfreevars()),
        "co_varnames" => {
            let items: Vec<NonNull<Header>> = (0..code.nlocals())
                .map(|slot| instance.new_str(code.varname(slot).unwrap_or("<unknown>")))
                .collect();
            Some(instance.new_tuple(items))
        }
        "co_names" => {
            let mut items: Vec<NonNull<Header>> = Vec::new();
            let mut index = 0;
            while let Some(entry) = code.name_at(index) {
                items.push(instance.new_str(entry));
                index += 1;
            }
            Some(instance.new_tuple(items))
        }
        "co_consts" => {
            let mut items: Vec<NonNull<Header>> = Vec::new();
            for index in 0..code.const_count() {
                match code.constant(index) {
                    Some(value) => {
                        // SAFETY: 常量由本对象持有，存活。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        items.push(value);
                    }
                    None => {
                        let none = instance.singletons().none();
                        // SAFETY: 单例由实例持有。
                        unsafe { instance.incref_object(none.as_ptr()) };
                        items.push(none);
                    }
                }
            }
            Some(instance.new_tuple(items))
        }
        _ => None,
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
