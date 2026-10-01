//! 类型对象（`docs/SPEC-object-model.md` §6，**OM-9**…**OM-15**）。
//!
//! 本层是 M1 的**最小骨架**：字段与槽位位置就位、注册表按实例存放（**OM-15**）；
//! 基类数组／MRO／类型字典目前是占位容器，C3 线性化（**OM-13**）与属性协议待接线。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use crate::header::{Header, PyObject};
use crate::instance::Instance;
use crate::py_object;

/// **OM-11**：类型对象的槽位表（Rust 函数指针；命名与布局自定，语义等价于 CPython 的 `tp_*`）。
///
/// 本层先落地对象模型自己需要的四个槽位：
///
/// | 槽位 | 语义 | 出处 |
/// |---|---|---|
/// | `dealloc` | 释放内存（第 ③ 步） | **OM-20** ③ |
/// | `finalize` | 终结器（`__del__`，可复活） | **OM-20** ① |
/// | `traverse` | 列出直接引用的对象 | **OM-12**／**OM-29**／**OM-36** |
/// | `clear` | 清空持有的引用（第 ② 步） | **OM-12**／**OM-20** ②／**OM-21** |
///
/// **尚未接线**：`call`、`hash`、`richcompare`、`iter`、`repr`、`str`
/// —— 它们的签名更依赖值表示与协议（`SPEC-type-system.md`），现在定会替那份规格做主。
/// （`getattr`／`setattr` 已按 `SPEC-bytecode.md` §10 的注"签名由实现自选"定下形状。）

/// 属性读槽（`OM-11` 的 `getattr`）：`name` 是属性名，返回**新引用**或 `None`（表示"没有"）。
///
/// 返回 `None` 时调用方继续走类型字典／实例字典，最后才报 `AttributeError`——
/// **内建类型不许旁路属性通道**，这条路就是那条通道。
pub type GetAttrFn = unsafe fn(*mut Header, &str, &crate::Instance) -> Option<NonNull<Header>>;

/// 实例化槽（`OM-11`／`OM-14` 的 `new`）：给类型与（**借用**的）位置实参，返回**新引用**。
///
/// 返回 `None` ＝ 这个类型不能这样实例化（调用方报 `TypeError`，消息照参照实现）。
/// 实参是借用视图——需要在实例里存下它们的槽位（异常类型就是）必须自己 incref。
pub type NewFn = unsafe fn(
    NonNull<TypeObject>,
    &[NonNull<Header>],
    &crate::Instance,
) -> Option<NonNull<Header>>;

/// `OM-11` 的 `repr` 槽：返回**调试表示**的文本（`None` ＝ 这个类型没实现）。
///
/// `SPEC-type-system.md` §8：省略时"由类型对象给默认形式"（`<X object at 0x…>`）。
/// 形状自选（`OM-38`）；返回 Rust 文本而不是 `str` 对象，接线 Python 级 `__repr__`
/// 覆写时再改成对象形态。
pub type ReprFn = unsafe fn(*mut Header, &crate::Instance) -> Option<String>;

/// `OM-11` 的 `str` 槽：`SPEC-type-system.md` §8 规定**省略时回退到 `repr`**。
pub type StrFn = unsafe fn(*mut Header, &crate::Instance) -> Option<String>;

/// `__format__` 的结果（`OM-11` 的槽位表是"**至少**含"，`format` 是本层加的一个）。
#[derive(Clone, Debug)]
pub enum FormatOutcome {
    /// 格式化后的文本。
    Text(String),
    /// 这个类型不认这条规格（调用方报
    /// `TypeError: unsupported format string passed to X.__format__`）。
    Unsupported,
    /// 类型码不认（调用方报 `ValueError: Unknown format code 'c' for object of type 'int'`）。
    UnknownCode(char),
    /// 规格里出现了 `z`（负零强制）：调用方报**实测**的那条 `ValueError`。
    NegativeZero,
    /// 本层还没实现这种写法。
    NotImplemented,
}

/// `OM-11` 的 `format` 槽（本层新增；`SPEC-type-system.md` §8 的槽位表是"至少含"）。
pub type FormatFn = unsafe fn(*mut Header, &str, &crate::Instance) -> FormatOutcome;

/// 属性写槽（`OM-11` 的 `setattr`）：`None` ＝ 删除；返回是否受理。
pub type SetAttrFn =
    unsafe fn(*mut Header, &str, Option<NonNull<Header>>, &crate::Instance) -> bool;

#[derive(Clone, Copy)]
pub struct Slots {
    pub(crate) dealloc: unsafe fn(*mut Header),
    pub(crate) finalize: Option<unsafe fn(*mut Header, &Instance)>,
    pub(crate) traverse: Option<unsafe fn(*mut Header, &mut dyn FnMut(*mut Header))>,
    pub(crate) clear: Option<unsafe fn(*mut Header, &Instance)>,
    /// 属性读槽（`OM-11` 的 `getattr`）——**内建类型的属性通道**，不许另开旁路。
    pub(crate) getattr: Option<GetAttrFn>,
    /// 属性写槽（`OM-11` 的 `setattr`）。
    pub(crate) setattr: Option<SetAttrFn>,
    /// 实例化槽（`OM-11` 的 `new`）。
    pub(crate) new: Option<NewFn>,
    /// `OM-11` 的 `repr` 槽。
    pub(crate) repr: Option<ReprFn>,
    /// `OM-11` 的 `str` 槽。
    pub(crate) str: Option<StrFn>,
    /// `format` 槽（本层新增）。
    pub(crate) format: Option<FormatFn>,
}

impl Slots {
    /// **OM-11** 的 `dealloc` 是必填项：没有它，计数归零后无人释放内存。
    pub fn new(dealloc: unsafe fn(*mut Header)) -> Self {
        Self {
            dealloc,
            finalize: None,
            traverse: None,
            clear: None,
            getattr: None,
            setattr: None,
            new: None,
            repr: None,
            str: None,
            format: None,
        }
    }

    /// `format` 槽（本层新增）。
    pub fn with_format(mut self, format: FormatFn) -> Self {
        self.format = Some(format);
        self
    }

    /// `OM-11` 的 `repr` 槽。
    pub fn with_repr(mut self, repr: ReprFn) -> Self {
        self.repr = Some(repr);
        self
    }

    /// `OM-11` 的 `str` 槽。
    pub fn with_str(mut self, str: StrFn) -> Self {
        self.str = Some(str);
        self
    }

    /// 实例化槽（`OM-11` 的 `new`）。
    pub fn with_new(mut self, new: NewFn) -> Self {
        self.new = Some(new);
        self
    }

    /// 属性读槽（`OM-11` 的 `getattr`）。
    pub fn with_getattr(mut self, getattr: GetAttrFn) -> Self {
        self.getattr = Some(getattr);
        self
    }

    /// 属性写槽（`OM-11` 的 `setattr`）。
    pub fn with_setattr(mut self, setattr: SetAttrFn) -> Self {
        self.setattr = Some(setattr);
        self
    }

    /// 终结器（**OM-20** ①）。实现可以"复活"对象：把计数改回大于 0。
    pub fn with_finalize(mut self, finalize: unsafe fn(*mut Header, &Instance)) -> Self {
        self.finalize = Some(finalize);
        self
    }

    /// 遍历回调（**OM-12**）：漏报泄漏、虚报误回收（**OM-36**），实现必须完整。
    pub fn with_traverse(
        mut self,
        traverse: unsafe fn(*mut Header, &mut dyn FnMut(*mut Header)),
    ) -> Self {
        self.traverse = Some(traverse);
        self
    }

    /// 清空回调（**OM-20** ②）：释放自己持有的全部引用；**OM-21** 要求不得朴素递归。
    pub fn with_clear(mut self, clear: unsafe fn(*mut Header, &Instance)) -> Self {
        self.clear = Some(clear);
        self
    }
}

/// **内部**类型标志：这个类型的实例带属性字典（`STORE_ATTR` 写进它）。
///
/// `OM-10` 的 `type_flags` 位分配还没规格化，这条只在本层内部用；不进 ABI。
pub const HAS_INSTANCE_DICT: u32 = 1 << 0;

/// **内部**类型标志：这个类型的实例把属性字典**内联在载荷里**（载荷是 [`crate::AttributeObject`]）。
///
/// 用户类的实例走这条；宿主／固定布局的实例（例如 `list` 的子类）**没有**这一位，
/// 它们的字典按 **OM-14** 挂在 [`crate::Header`] 的那一格上。
pub const INLINE_INSTANCE_DICT: u32 = 1 << 1;

py_object! {
    /// **OM-9**：类型对象自身也是对象（有 [`Header`]）。
    pub struct TypeObject {
        /// **OM-10** 名字。*占位*：最终必须是 `str` 对象（值表示落地后换掉）。
        name: &'static str,
        /// **OM-10** 基类数组。*占位*：最终是 `tuple`。
        bases: RefCell<Vec<NonNull<TypeObject>>>,
        /// **OM-10**／**OM-13** MRO。*占位*：C3 线性化待接线。
        mro: RefCell<Vec<NonNull<TypeObject>>>,
        /// **OM-10** 类型标志。位分配待 `SPEC-type-system.md` 与 §13-1（宿主类型是否可继承）。
        type_flags: Cell<u32>,
        /// **OM-11** 槽位表。
        slots: Slots,
        /// **OM-10** 类型字典。*占位*：最终是 `dict` 对象。
        dict: RefCell<Option<NonNull<Header>>>,
        /// 实例字节数：供 `Instance` 记账与断言（**OM-3**、**OM-5** 的布局一致性）。
        instance_size: usize,
    }
}

impl TypeObject {
    /// **OM-10** 名字。*占位*语义见字段注释。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// 实例字节数（与 `size_of::<T>()` 必须一致，`Instance::alloc` 会断言）。
    pub fn instance_size(&self) -> usize {
        self.instance_size
    }

    /// 槽位表（只读副本；槽位表在类型创建后不再改动）。
    pub fn slots(&self) -> Slots {
        self.slots
    }

    /// **OM-10**：基类数组（*占位*：最终是 `tuple`）。
    pub fn bases(&self) -> Vec<NonNull<TypeObject>> {
        self.bases.borrow().clone()
    }

    /// **OM-10**／**OM-13**：MRO（*占位*：C3 线性化待接线）。
    pub fn mro(&self) -> Vec<NonNull<TypeObject>> {
        self.mro.borrow().clone()
    }

    /// 登记基类与 MRO。
    ///
    /// *临时*：内建类型层次**手工登记**（`TS-40` 要求 `bool ⊂ int`）；完整的 C3 线性化、
    /// `__mro_entries__` 与 `__init_subclass__` 由 **OM-13**／**OM-14** 落地后接管。
    pub fn set_bases(&self, bases: Vec<NonNull<TypeObject>>, mro: Vec<NonNull<TypeObject>>) {
        *self.bases.borrow_mut() = bases;
        *self.mro.borrow_mut() = mro;
    }

    /// **OM-10**：类型字典（*占位*：最终是 `dict` 对象；可能还没建）。
    pub fn dict(&self) -> Option<NonNull<Header>> {
        *self.dict.borrow()
    }

    /// 设置类型字典（**新引用**，由类型对象接手）。
    pub fn set_dict(&self, mapping: Option<NonNull<Header>>) {
        *self.dict.borrow_mut() = mapping;
    }

    /// **OM-10**：类型标志（本层只用 [`HAS_INSTANCE_DICT`]）。
    pub fn type_flags(&self) -> u32 {
        self.type_flags.get()
    }

    /// 置上 [`HAS_INSTANCE_DICT`] ＋ [`INLINE_INSTANCE_DICT`]（载荷本身就是 [`crate::AttributeObject`]）。
    pub fn mark_has_instance_dict(&self) {
        self.type_flags
            .set(self.type_flags.get() | HAS_INSTANCE_DICT | INLINE_INSTANCE_DICT);
    }

    /// **OM-14**：置上 [`HAS_INSTANCE_DICT`]，但字典**另行挂载**在头部那一格上
    /// （宿主类型与"布局固定"的子类走这条）。
    pub fn mark_external_instance_dict(&self) {
        self.type_flags.set(self.type_flags.get() | HAS_INSTANCE_DICT);
    }

    /// 实例字典是不是内联在载荷里。
    pub fn has_inline_instance_dict(&self) -> bool {
        self.type_flags.get() & INLINE_INSTANCE_DICT != 0
    }

    /// **OM-12**：是否参与循环回收。
    pub fn is_gc_tracked(&self) -> bool {
        self.header().has_flag(crate::flags::GC_TRACKED)
    }
}
