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
) -> Result<NonNull<Header>, crate::ExecError>;

/// `OM-11` 的 `repr` 槽：返回**调试表示**的文本（`None` ＝ 这个类型没实现）。
///
/// `SPEC-type-system.md` §8：省略时"由类型对象给默认形式"（`<X object at 0x…>`）。
/// 形状自选（`OM-38`）；返回 Rust 文本而不是 `str` 对象，接线 Python 级 `__repr__`
/// 覆写时再改成对象形态。
/// `OM-11` 的 `repr` 槽：`repr` 一段文本；**失败必须能表达**（`OM-11` 扩：槽位签名要能带
/// 异常——`TS-45` ①的**输出方向**就靠它：超出位数上限要抛 `ValueError`）。
pub type ReprFn = unsafe fn(*mut Header, &crate::Instance) -> Result<String, crate::ExecError>;

/// `OM-11` 的 `str` 槽：`SPEC-type-system.md` §8 规定**省略时回退到 `repr`**。
pub type StrFn = unsafe fn(*mut Header, &crate::Instance) -> Result<String, crate::ExecError>;

/// `OM-11` 的 `call` 槽：调用这个类型的实例。返回**新引用**；失败抛 `TypeError` 一类
/// （`SPEC-type-system.md` §8：失败抛 `TypeError`，含实参不匹配）。
///
/// 实参是**借用视图**（要留住的自己 incref）；`bound_self` 亦是借用。
pub type CallFn = unsafe fn(
    *mut Header,
    Option<NonNull<Header>>,
    &[NonNull<Header>],
    &[(NonNull<Header>, NonNull<Header>)],
    &crate::Instance,
) -> Result<NonNull<Header>, crate::ExecError>;

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
    /// `OM-11` 的 `call` 槽。
    pub(crate) call: Option<CallFn>,
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
            call: None,
        }
    }

    /// `OM-11` 的 `call` 槽。
    pub fn with_call(mut self, call: CallFn) -> Self {
        self.call = Some(call);
        self
    }

    /// **`AB-58`／`AB-37`**：从**定长宿主布局**的基类继承槽位——只带走与布局／生命周期
    /// 有关的那几个（`dealloc`／`traverse`／`finalize`），**不带** `new`（宿主类型没有默认
    /// 构造：实例由宿主经 `pa_newhandle` 建，`AB-58`），也不带属性通道的槽位
    /// （那些由 Python 层的类字典决定）。
    pub fn inherit_host_layout(&self) -> Self {
        Self {
            dealloc: self.dealloc,
            finalize: self.finalize,
            traverse: self.traverse,
            clear: self.clear,
            ..Self::new(self.dealloc)
        }
    }

    /// **连 `new` 一起继承**（第 99 轮真 bug 修 ✗）：内建类型的**子类**（`class D(dict)` ✓）实例化时
    /// 必须走**基类的 `tp_new`** ✓ —— 载荷就是在它里面建起来的（`dict_new` 里那格
    /// `RefCell<Vec<…>>` ✓）；`inherit_host_layout` 把 `new` 置空 ✗ ⇒ 子类实例走通用分配
    /// ⇒ 载荷是**未初始化内存** ✓ ⇒ 读"长度／容量／指针"读到的是那块内存里别的东西 ✓。
    pub fn inherit_host_layout_with_new(&self) -> Self {
        let mut slots = self.inherit_host_layout();
        slots.new = self.new;
        slots
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

/// **内部**类型标志：`AB-37` 的"本类型**不可**被继承"（`pa_sig.flags` 的 `PA_TYPE_FINAL`）。
pub const FINAL_TYPE: u32 = 1 << 3;

/// **内部**类型标志：这个类型的分配走**通用 Python 对象路径**（`attribute_new`）。
///
/// 有了它，执行器才能不加函数指针比较（`rustc` 明说函数地址不保证唯一）就判定
/// "带实参创建但没有 `__init__`"该报参照实现那句 `X() takes no arguments` ——
/// 内建类型（`ValueError('x')` 一类）的 `new` 槽是自己实现的，**不**该吃这条规则。
pub const GENERIC_ALLOCATION: u32 = 1 << 2;

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
        /// **`OM-14`** 宿主类型：宿主提供的 `dealloc`（放掉载荷**内部**它自己的资源）。
        ///
        /// 载荷**存储**归 VM（`AB-58`），所以这里只释放"宿主在载荷里持有的东西"，
        /// **禁止**在这里 `free` 载荷本身。
        host_dealloc: Cell<Option<HostDealloc>>,
        /// **`OM-14`** 宿主类型：宿主提供的 `traverse`（`OM-36`：列出全部直接引用）。
        host_traverse: Cell<Option<HostTraverse>>,
    }
}

/// **`OM-14`／`OM-34`**：宿主类型的 `dealloc` 槽形状（载荷**内部**资源的释放；载荷存储归 VM）。
pub type HostDealloc = unsafe extern "C" fn(*mut core::ffi::c_void);

/// **`OM-14`／`OM-36`**：宿主 `traverse` 的访问回调（把子引用报给 VM）。
pub type HostVisit = unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void);

/// **`OM-14`／`OM-36`**：宿主类型的 `traverse` 槽形状（"上下文 ＋ 回调"形态）。
pub type HostTraverse =
    unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, HostVisit);

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

    /// 是不是**定长宿主布局**（`OM-14`／`AB-58`：载荷紧跟在头部之后、由 VM 分配）。
    pub fn is_host_layout(&self) -> bool {
        self.host_dealloc.get().is_some()
    }

    /// **`OM-14`**：记下宿主的 `dealloc`／`traverse`（注册宿主类型时一次性设置）。
    pub fn set_host_hooks(&self, dealloc: HostDealloc, traverse: HostTraverse) {
        self.host_dealloc.set(Some(dealloc));
        self.host_traverse.set(Some(traverse));
    }

    /// 宿主的 `dealloc`（借用）。
    pub fn host_dealloc(&self) -> Option<HostDealloc> {
        self.host_dealloc.get()
    }

    /// 宿主的 `traverse`（借用）。
    pub fn host_traverse(&self) -> Option<HostTraverse> {
        self.host_traverse.get()
    }

    /// **`AB-58`**：宿主载荷的**偏移**（头部之后；头部长 40 且对齐 8，故载荷天然 8 对齐）。
    ///
    /// 子类实例的载荷**在同一偏移**（`AB-37`：按同一尺寸由 VM 分配）。
    pub fn payload_offset(&self) -> usize {
        crate::header::HEADER_SIZE_BYTES
    }

    /// **`AB-58`**：宿主载荷的**字节数**（`instance_size` 去掉头部）。
    pub fn payload_size(&self) -> usize {
        self.instance_size.saturating_sub(crate::header::HEADER_SIZE_BYTES)
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
        self.type_flags.set(
            self.type_flags.get() | HAS_INSTANCE_DICT | INLINE_INSTANCE_DICT | GENERIC_ALLOCATION,
        );
    }

    /// **`AB-37`**：把这个类型标成**不可继承**（`PA_TYPE_FINAL`）。
    pub fn mark_final(&self) {
        self.type_flags.set(self.type_flags.get() | FINAL_TYPE);
    }

    /// **`AB-37`**：这个类型是不是**不可继承**。
    pub fn is_final(&self) -> bool {
        self.type_flags.get() & FINAL_TYPE != 0
    }

    /// 这个类型是不是走**通用 Python 对象分配**（[`GENERIC_ALLOCATION`]）。
    pub fn has_generic_allocation(&self) -> bool {
        self.type_flags.get() & GENERIC_ALLOCATION != 0
    }

    /// **OM-14**：置上 [`HAS_INSTANCE_DICT`]，但字典**另行挂载**在头部那一格上
    /// （宿主类型与"布局固定"的子类走这条）。
    pub fn mark_external_instance_dict(&self) {
        self.type_flags.set(self.type_flags.get() | HAS_INSTANCE_DICT);
    }

    /// **OM-11**：这个类型有没有 `call` 槽（有 ⇒ 它的实例可调用）。
    pub fn has_call_slot(&self) -> bool {
        self.slots.call.is_some()
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

/// **类型对象的引用遍历** ✓（第 198 轮：**这一处漏了 ⇒ GC 会回收活对象** ✗）。
///
/// **为什么致命** ✗：类型字典里放着的函数／类往往只有"这个类型"一个引用 ✓ ⇒ 不遍历它 ⇒
/// 那些对象在 `find_unreachable` 里被算成不可达 ✓ ⇒ 被 `free` ✗ ⇒ 它们的内存随后被别的分配**重写** ⇒
/// glibc 在**很久之后**才报 `corrupted double-linked list` ✗（第 133–135 轮的现场正是如此 ✓）。
pub(crate) unsafe fn type_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let type_object = unsafe { &*ptr.cast::<TypeObject>() };
    if let Some(dict) = *type_object.dict.borrow() {
        visit(dict.as_ptr());
    }
    for base in type_object.bases.borrow().iter() {
        visit(base.as_ptr().cast::<Header>());
    }
    for entry in type_object.mro.borrow().iter() {
        visit(entry.as_ptr().cast::<Header>());
    }
}

/// 与 [`type_traverse`] 对称的清理 ✓（三份引用都由类型对象持有 ✓）。
pub(crate) unsafe fn type_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let type_object = unsafe { &*ptr.cast::<TypeObject>() };
    if let Some(dict) = type_object.dict.borrow_mut().take() {
        // SAFETY: 这一份引用由本对象持有。
        unsafe { instance.release_object(dict.as_ptr()) };
    }
    for base in type_object.bases.borrow_mut().drain(..) {
        // SAFETY: 同上。
        unsafe { instance.release_object(base.as_ptr().cast::<Header>()) };
    }
    for entry in type_object.mro.borrow_mut().drain(..) {
        // SAFETY: 同上。
        unsafe { instance.release_object(entry.as_ptr().cast::<Header>()) };
    }
}
