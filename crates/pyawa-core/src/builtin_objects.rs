//! 内建类型的**载荷**（`docs/SPEC-type-system.md` 的 `TS-43`：布局由实现自选，不进 ABI）。
//!
//! **TS-41** 的表（`crate::builtin_types`）只记"类型存在、层次正确"；本文件才是它们的表示。
//! 只有 `OM-23` 点名的那几个才做单例（`None`／`True`／`False`／小整数／空串），
//! 其余类型"每次造一个新对象"——`is` 语义因此与参照实现一致（`OM-39`）。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use crate::header::Header;
use crate::instance::Instance;
use crate::py_object;
use crate::type_object::Slots;

py_object! {
    /// `None` 的单例载体。*占位*：Python 层类型名与协议随后补。
    pub struct NoneObject {}
}

py_object! {
    /// `True`／`False` 的单例载体。
    pub struct BoolObject {
        /// 真假。
        value: bool,
    }
}

py_object! {
    /// 小整数的单例载体。
    pub struct IntObject {
        /// 数值；一定落在 `SMALL_INT_MIN..=SMALL_INT_MAX`。
        value: i64,
    }
}

py_object! {
    /// `object` 的实例。*占位*：`object()` 不携带状态。
    pub struct PlainObject {}
}

/// 原生（Rust 实现）可调用的签名（`AB-24` 的宿主函数最终也走这条）。
///
/// 实参是**借用视图**——要留住的必须自己 incref；返回值是**新引用**。
/// 出错时返回 [`crate::ExecError`]（脚本异常经它冒泡）。
pub type NativeFn = unsafe fn(
    &Instance,
    Option<NonNull<Header>>,
    &[NonNull<Header>],
    &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError>;

py_object! {
    /// **原生可调用对象**（`builtin_function_or_method`）：Rust 函数 ＋ 一个名字。
    ///
    /// 它是 `AB-24`／`AB-25` 的宿主函数、`__build_class__` 一类内建函数的落点；
    /// 绑定了 `self` 的形态（`[].append`）只是多带一个 `self`（本层用 `MethodObject` 表达绑定，
    /// 故这里只存函数与名字）。
    pub struct BuiltinFunctionObject {
        /// 名字（`repr` 用；最终应当是 `str` 对象）。
        name: &'static str,
        /// Rust 实现。
        function: Cell<NativeFn>,
    }
}

py_object! {
    /// **绑定方法**：函数 ＋ 要绑上去的 `self`（两者都持有一份引用）。
    ///
    /// `OM-11` 的 `getattr` 在类型字典里查到函数时产出它：`obj.method`（**不调用**）拿到的是
    /// 这个对象，而 `obj.method()`（编译器的取方法位）仍然走"函数 ＋ `self`"的栈形态。
    /// 有了它，`OM-14` 的子类分派（`__init__`／`__new__`／`__del__` 的覆写）与生成器方法
    /// （`send`／`throw`／`close`）才有落点。
    pub struct MethodObject {
        /// 函数对象（**本对象持有一份引用**）。
        function: NonNull<Header>,
        /// 绑定的实例（**本对象持有一份引用**）。
        this: NonNull<Header>,
    }
}

py_object! {
    /// 生成器：一个**挂起的帧** ＋ 是否已跑完。
    ///
    /// 挂起时值栈在帧自己的恢复点里（`BC-47`），故这里只持有帧的引用。
    pub struct GeneratorObject {
        /// 生成器的帧（**本对象持有一份引用**）。
        frame: NonNull<Header>,
        /// 已经跑完（之后的 `FOR_ITER` 直接走耗尽路径）。
        finished: Cell<bool>,
    }
}

py_object! {
    /// 异常实例：`args` ＋ 链（`__cause__`／`__context__`）＋ 抑制标志。
    ///
    /// 一个 Rust 载荷支撑表里那整棵 `BaseException` 树（`TS-43`：布局自选）。
    /// `BC-60` ② 要求"当前异常状态按实例存"，那条状态在 [`crate::Instance`] 上，不在这里。
    pub struct ExceptionObject {
        /// 构造实参（`e.args`）。
        args: RefCell<Vec<NonNull<Header>>>,
        /// `raise X from Y` 里的 `Y`（**本对象持有一份引用**）。
        cause: RefCell<Option<NonNull<Header>>>,
        /// 隐式上下文（正在处理的那个异常）。
        context: RefCell<Option<NonNull<Header>>>,
        /// `raise X from None` 会把上下文抑制掉（参照实现里 `__suppress_context__`）。
        suppress_context: Cell<bool>,
        /// `e.args` 读出来的那个 `tuple`（**本对象持有一份引用**）。
        ///
        /// 实测：`e.args is e.args` 为真（每个实例缓一个），而两个不同实例的 `args`
        /// 不是同一对象 ⇒ 必须按实例缓存，不能每次现造。
        args_tuple: RefCell<Option<NonNull<Header>>>,
    }
}

py_object! {
    /// 迭代器：一个被迭代的对象 ＋ 游标。
    ///
    /// *临时*：一个 Rust 载荷支撑表里那几个迭代器类型（`tuple_iterator`／`list_iterator`／
    /// `str_ascii_iterator`／`dict_keyiterator`／`set_iterator`）——`TS-43` 允许布局自选；
    /// 「下一个」按**被迭代对象的类型**分派，不另存 kind。
    pub struct IteratorObject {
        /// 被迭代的对象（**本对象持有一份引用**）。
        target: NonNull<Header>,
        /// 游标（下一个要取的下标）。
        index: Cell<usize>,
    }
}

py_object! {
    /// **用户定义的类**的实例载荷：带一个属性字典（`STORE_ATTR` 写这里）。
    ///
    /// `TS-43`：载荷布局由实现自选。**为什么单独一个类型**：参照实现里 `object()` **没有**
    /// `__dict__`，而用户类的实例有——把字典挂在 `PlainObject` 上就会把 `object()` 也带偏。
    pub struct AttributeObject {
        /// 属性字典（惰性创建）。
        attributes: RefCell<Option<NonNull<Header>>>,
    }
}

py_object! {
    /// `float` 的实例（C `double`，与参照实现一致）。
    pub struct FloatObject {
        /// 数值；`inf`／`nan` 照旧。
        value: f64,
    }
}

py_object! {
    /// `function` 的实例：一个 code object ＋ 默认值。
    ///
    /// *临时*：签名里还没有注解、闭包与 `__qualname__`；它们随 `SET_FUNCTION_ATTRIBUTE`
    /// 的其余标志位（实测 8 ＝ closure、16 ＝ annotate）与属性族补齐。
    pub struct FunctionObject {
        /// 被执行的 code object（**本对象持有一份引用**）。
        code: NonNull<Header>,
        /// 位置参数默认值（对齐到**尾部**若干位置参数，与参照实现一致）。
        defaults: Vec<NonNull<Header>>,
        /// 仅关键字参数默认值（`dict`，可为空）。
        kwdefaults: Option<NonNull<Header>>,
    }
}

py_object! {
    /// `str` 的实例。*临时*：载荷是 Rust 字符串；字符层面的一致性随 `CM-13` 的 Unicode 数据补。
    pub struct StrObject {
        /// 内容（UTF-8）。
        value: String,
    }
}

impl BuiltinFunctionObject {
    /// 见 [`TupleObject::slots`]：本身不持有对象引用（名字是静态串）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc).with_repr(builtin_function_repr)
    }

    /// 名字。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Rust 实现。
    pub fn function(&self) -> NativeFn {
        self.function.get()
    }
}

/// 原生可调用对象的 `repr`：`<built-in function len>`（实测）。
pub unsafe fn builtin_function_repr(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<BuiltinFunctionObject>() };
    Some(format!("<built-in function {}>", object.name()))
}

/// **`AB-58`**：定长宿主布局的 `dealloc` 槽——载荷**存储**由 VM 释放。
///
/// 宿主在载荷里自己持有的东西由宿主的 `dealloc`（`OM-14`）放掉，那是**另一个**槽位，
/// 见 `crates/pyawa-abi` 的宿主对象槽实现；这里只负责把 VM 分的那块内存还回去。
///
/// # Safety
///
/// 由 `Instance` 在计数归零、`clear` 跑过之后调用（`OM-20` ③）。
pub unsafe fn free_fixed_layout(ptr: *mut Header) {
    // SAFETY: 调用方保证 ptr 是本实例的定长宿主对象。
    let ty = unsafe { &*ptr }.ty();
    // SAFETY: ty 由注册表持有。
    let size = unsafe { &*ty.as_ptr() }.instance_size;
    let layout = core::alloc::Layout::from_size_align(size, core::mem::align_of::<Header>())
        .expect("宿主载荷尺寸溢出");
    // SAFETY: 这块内存正是 `alloc_host_object` 按同一 layout 分配的，且计数已归零。
    unsafe { std::alloc::dealloc(ptr.cast::<u8>(), layout) };
}

/// **Python 级终结器**（`OM-20` ①／`OM-14`）：按 `TS-44` 走属性通道找 `__del__` 并调用。
///
/// 布局无关：用户类实例（`AttributeObject`）与**宿主类型**及其 Python 子类都用它
/// （`AB-37`／`OM-14`：`tp_dealloc` 得能被 Python 覆写）。
///
/// 覆写里抛出的异常在参照实现里是"**被吞掉并报告**"（`OM-33` 的弱引用那条同理）；
/// 本层暂**吞掉**（报告机制要 `sys.unraisablehook`，随后补——清单里记着）。
pub unsafe fn python_level_finalize(ptr: *mut Header, instance: &Instance) {
    let object = NonNull::new(ptr).expect("调用方保证非空");
    match crate::executor::call_object_method(instance, object, "__del__", &[]) {
        Ok(Some(result)) => {
            // SAFETY: result 是新引用。
            unsafe { instance.release_object(result.as_ptr()) };
        }
        Ok(None) => {}
        Err(crate::ExecError::Raised { exception }) => {
            // 吞掉并记在实例上（`pending_exception`），供宿主随后查看
            let _ = instance.set_pending_exception(Some(exception));
        }
        Err(_) => {}
    }
}

impl MethodObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(method_traverse)
            .with_clear(method_clear)
    }

    /// 函数（**借用**）。
    pub fn function(&self) -> NonNull<Header> {
        self.function
    }

    /// 绑定的实例（**借用**）。
    pub fn this(&self) -> NonNull<Header> {
        self.this
    }
}

/// `OM-40`：列出绑定方法持有的引用。
unsafe fn method_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    visit(object.function().as_ptr());
    visit(object.this().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出两份引用。
unsafe fn method_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    // SAFETY: 这两份引用由本对象持有。
    unsafe {
        instance.release_object(object.function().as_ptr());
        instance.release_object(object.this().as_ptr());
    }
}

impl GeneratorObject {
    /// 见 [`TupleObject::slots`]：帧里的局部槽可能指回生成器自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(generator_traverse)
            .with_clear(generator_clear)
    }

    /// 生成器的帧（**借用**）。
    pub fn frame(&self) -> NonNull<Header> {
        self.frame
    }

    /// 是否已跑完。
    pub fn finished(&self) -> bool {
        self.finished.get()
    }

    /// 置"已跑完"。
    pub fn mark_finished(&self) {
        self.finished.set(true);
    }
}

/// `OM-40`：列出生成器持有的引用。
unsafe fn generator_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    visit(object.frame().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出帧。
unsafe fn generator_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.frame().as_ptr()) };
}

impl ExceptionObject {
    /// 见 [`TupleObject::slots`]：链上可能指回异常自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_getattr(exception_getattr)
            .with_traverse(exception_traverse)
            .with_clear(exception_clear)
    }

    /// 构造实参（**借用**的副本）。
    pub fn args(&self) -> Vec<NonNull<Header>> {
        self.args.borrow().clone()
    }

    /// `e.args` 缓存下来的那个 `tuple`（**借用**）。
    pub fn args_tuple(&self) -> Option<NonNull<Header>> {
        *self.args_tuple.borrow()
    }

    /// 记下 `e.args` 的 `tuple`（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_args_tuple(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.args_tuple.replace(value)
    }

    /// 设置构造实参（**新引用**，由本对象接手）。
    pub fn set_args(&self, args: Vec<NonNull<Header>>) {
        *self.args.borrow_mut() = args;
    }

    /// `__cause__`。
    pub fn cause(&self) -> Option<NonNull<Header>> {
        *self.cause.borrow()
    }

    /// 设置 `__cause__`（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_cause(&self, cause: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut *self.cause.borrow_mut(), cause)
    }

    /// `__context__`。
    pub fn context(&self) -> Option<NonNull<Header>> {
        *self.context.borrow()
    }

    /// 设置 `__context__`（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_context(&self, context: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut *self.context.borrow_mut(), context)
    }

    /// `__suppress_context__`。
    pub fn suppress_context(&self) -> bool {
        self.suppress_context.get()
    }

    /// 置 `__suppress_context__`。
    pub fn set_suppress_context(&self, value: bool) {
        self.suppress_context.set(value);
    }

    /// 消息文本：第一个实参是 `str` 时取它的内容（`str(e)` 的最小形态）。
    ///
    /// 需要实例是为了找 `str` 类型——载荷里不存实例指针（`OM-40`：载荷只放裸引用）。
    pub fn message_with(&self, instance: &Instance) -> Option<String> {
        let first = self.args().first().copied()?;
        // SAFETY: first 由本对象持有。
        let ty = unsafe { first.as_ref() }.ty();
        if ty != instance.singletons().str_type() {
            return None;
        }
        // SAFETY: 类型身份已确认。
        Some(unsafe { &*first.as_ptr().cast::<StrObject>() }.value().to_owned())
    }
}

/// `OM-40`：列出异常持有的引用。
/// **`OM-11` 的 `getattr` 槽**（异常实例）：`args`／`__cause__`／`__context__`／
/// `__suppress_context__`／`__traceback__`。
///
/// 形状逐条实测（`tests/exceptions.rs`）：
/// - `e.args` 是 `tuple`，且**按实例缓存**（`e.args is e.args` 为真）
/// - `__cause__`／`__context__` 没有就是 `None`；`__suppress_context__` 是布尔
/// - `__traceback__` 在**未抛**时是 `None`；抛过之后参照实现给 `traceback` 对象，
///   本层还没有 traceback 对象（清单里记着）⇒ 一律 `None`
/// - 其它名字返回 `None`，让调用方继续走类型字典／实例字典，最终报参照实现那句
///   `'X' object has no attribute 'Y'`
///
/// # Safety
///
/// 契约见 `GetAttrFn`：返回**新引用**或 `None`。
pub unsafe fn exception_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    match name {
        "args" => {
            if let Some(cached) = object.args_tuple() {
                // SAFETY: 缓存由本对象持有，存活。
                unsafe { instance.incref_object(cached.as_ptr()) };
                return Some(cached);
            }
            let items: Vec<NonNull<Header>> = object.args();
            for item in &items {
                // SAFETY: 元组要自己那份。
                unsafe { instance.incref_object(item.as_ptr()) };
            }
            let tuple = instance.new_tuple(items);
            if let Some(old) = object.set_args_tuple(Some(tuple)) {
                // SAFETY: 旧缓存由本对象持有。
                unsafe { instance.release_object(old.as_ptr()) };
            }
            Some(tuple)
        }
        "__cause__" => Some(match object.cause() {
            Some(value) => {
                // SAFETY: 由本对象持有。
                unsafe { instance.incref_object(value.as_ptr()) };
                value
            }
            None => instance.new_none(),
        }),
        "__context__" => Some(match object.context() {
            Some(value) => {
                // SAFETY: 同上。
                unsafe { instance.incref_object(value.as_ptr()) };
                value
            }
            None => instance.new_none(),
        }),
        "__suppress_context__" => Some(instance.new_bool(object.suppress_context())),
        // 未抛时参照实现就是 `None`；抛过之后的 `traceback` 对象本层还没有（清单里记着）
        "__traceback__" => Some(instance.new_none()),
        _ => None,
    }
}

unsafe fn exception_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    for value in object.args() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.cause() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.context() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.args_tuple() {
        visit(value.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出异常持有的引用。
unsafe fn exception_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    for value in core::mem::take(&mut *object.args.borrow_mut()) {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_cause(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_context(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_args_tuple(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

impl IteratorObject {
    /// 见 [`TupleObject::slots`]：被迭代的对象可能指回迭代器自己。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(iterator_traverse)
            .with_clear(iterator_clear)
    }

    /// 被迭代的对象（**借用**）。
    pub fn target(&self) -> NonNull<Header> {
        self.target
    }

    /// 游标。
    pub fn index(&self) -> usize {
        self.index.get()
    }

    /// 推进游标。
    pub fn advance(&self) {
        self.index.set(self.index.get() + 1);
    }
}

/// `OM-40`：列出迭代器持有的引用。
unsafe fn iterator_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IteratorObject>() };
    visit(object.target().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出被迭代对象。
unsafe fn iterator_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IteratorObject>() };
    // SAFETY: 该引用由本对象持有。
    unsafe { instance.release_object(object.target().as_ptr()) };
}

impl AttributeObject {
    /// 见 [`TupleObject::slots`]：属性字典里的值可能指回对象自己。
    ///
    /// 终结器按 **`TS-44`** 走**属性通道**找 `__del__`（`OM-20` ①）——用户类的实例走这条。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(attribute_traverse)
            .with_clear(attribute_clear)
            .with_finalize(python_level_finalize)
    }

    /// 属性字典（**借用**；还没建就是 `None`）。
    pub fn attributes(&self) -> Option<NonNull<Header>> {
        *self.attributes.borrow()
    }

    /// 设置属性字典（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_attributes(&self, mapping: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut *self.attributes.borrow_mut(), mapping)
    }
}

/// `OM-40`：列出属性字典。
unsafe fn attribute_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<AttributeObject>() };
    if let Some(mapping) = object.attributes() {
        visit(mapping.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出属性字典。
unsafe fn attribute_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<AttributeObject>() };
    if let Some(mapping) = object.set_attributes(None) {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(mapping.as_ptr()) };
    }
}

impl FunctionObject {
    /// 见 [`TupleObject::slots`]：默认值可能指向别的对象（甚至函数自己）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(function_traverse)
            .with_clear(function_clear)
    }

    /// code object（**借用**的裸引用）。
    pub fn code(&self) -> NonNull<Header> {
        self.code
    }

    /// 位置参数默认值（**借用**）。
    pub fn defaults(&self) -> &[NonNull<Header>] {
        &self.defaults
    }

    /// 设置位置参数默认值（**新引用**，由本对象接手）。
    pub fn set_defaults(&mut self, defaults: Vec<NonNull<Header>>) {
        self.defaults = defaults;
    }

    /// 仅关键字参数默认值（**借用**的 `dict`）。
    pub fn kwdefaults(&self) -> Option<NonNull<Header>> {
        self.kwdefaults
    }

    /// 设置仅关键字默认值（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_kwdefaults(&mut self, kwdefaults: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut self.kwdefaults, kwdefaults)
    }
}

/// `OM-40`：列出函数持有的引用。
unsafe fn function_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    visit(object.code().as_ptr());
    for value in object.defaults() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.kwdefaults() {
        visit(value.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出函数持有的引用。
unsafe fn function_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &mut *ptr.cast::<FunctionObject>() };
    // SAFETY: 这些引用由本对象持有。
    unsafe { instance.release_object(object.code().as_ptr()) };
    for value in core::mem::take(&mut object.defaults) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_kwdefaults(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

py_object! {
    /// **内部哨兵**：`CALL` 的"没有 self"槽位（参照实现在栈上放 `NULL` 指针）。
    ///
    /// *内部*：它**不**进 `TS-41` 的内建类型表，也**禁止**暴露给 Python 代码——
    /// Python 侧看到的永远是 `None`。
    pub struct NullObject {}
}

impl FloatObject {
    /// 数值。
    pub fn value(&self) -> f64 {
        self.value
    }
}

impl StrObject {
    /// 内容。
    pub fn value(&self) -> &str {
        &self.value
    }

    /// 是否为空串（`OM-23` 的空串单例就是它）。
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

// --------------------------------------------------------------------------- #
// 容器（`BC-49` 的容器与解包族；载荷布局按 `TS-43` 由实现自选）
// --------------------------------------------------------------------------- #

py_object! {
    /// `tuple` 的实例：**不可变**——造好之后不再改（载荷只在构造期填）。
    pub struct TupleObject {
        items: Vec<NonNull<Header>>,
    }
}

py_object! {
    /// `list` 的实例。
    pub struct ListObject {
        items: RefCell<Vec<NonNull<Header>>>,
    }
}

py_object! {
    /// `set` 的实例。
    ///
    /// *已知偏离*：本层按**插入顺序**存（关联表），参照实现的迭代顺序由哈希决定。
    /// 集合的文档契约是"无序"，故这一条属于**实现观测面**，落地 `MS-19` 时要如实登记。
    pub struct SetObject {
        items: RefCell<Vec<NonNull<Header>>>,
    }
}

py_object! {
    /// `dict` 的实例。
    ///
    /// *临时*：关联表 ＋ 线性查找（查找走"值相等"而不是 `__hash__`／`__eq__` 槽位——
    /// 那两个槽位随类型系统接线）。**插入顺序**与参照实现一致。
    pub struct DictObject {
        entries: RefCell<Vec<(NonNull<Header>, NonNull<Header>)>>,
    }
}

impl TupleObject {
    /// 注册这个类型时的槽位表（持有引用 ⇒ 要 `traverse`／`clear`，`OM-12`）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(tuple_traverse)
            .with_clear(tuple_clear)
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 按位置取元素（**借用**的裸引用）。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.get(index).copied()
    }

    /// 全部元素（**借用**）。
    pub fn items(&self) -> &[NonNull<Header>] {
        &self.items
    }
}

impl ListObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(list_traverse)
            .with_clear(list_clear)
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// 按位置取元素（**借用**）。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.borrow().get(index).copied()
    }

    /// 追加一个**新引用**（引用由本对象接管）。
    pub fn append(&self, value: NonNull<Header>) {
        self.items.borrow_mut().push(value);
    }

    /// 追加一批**新引用**。
    pub fn extend(&self, values: impl IntoIterator<Item = NonNull<Header>>) {
        self.items.borrow_mut().extend(values);
    }

    /// 全部元素（**借用**的副本）。
    pub fn items(&self) -> Vec<NonNull<Header>> {
        self.items.borrow().clone()
    }

    /// 替换第 `index` 项（**新引用**），返回旧值（调用方负责释放）。
    pub fn replace(&self, index: usize, value: NonNull<Header>) -> Option<NonNull<Header>> {
        self.items
            .borrow_mut()
            .get_mut(index)
            .map(|slot| core::mem::replace(slot, value))
    }

    /// 删除第 `index` 项，返回被删的那份引用（调用方负责释放）。
    pub fn remove(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }
}

impl SetObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(set_traverse)
            .with_clear(set_clear)
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// 按位置取元素（**借用**）——顺序是插入顺序，见类型文档的*已知偏离*。
    pub fn item(&self, index: usize) -> Option<NonNull<Header>> {
        self.items.borrow().get(index).copied()
    }

    /// 全部元素（**借用**的副本）。
    pub fn items(&self) -> Vec<NonNull<Header>> {
        self.items.borrow().clone()
    }

    /// 追加一个**新引用**（不去重；调用方负责先查重）。
    pub fn insert_raw(&self, value: NonNull<Header>) {
        self.items.borrow_mut().push(value);
    }

    /// 删除第 `index` 项，返回被删的那份引用（调用方负责释放）。
    pub fn remove(&self, index: usize) -> Option<NonNull<Header>> {
        let mut items = self.items.borrow_mut();
        if index >= items.len() {
            return None;
        }
        Some(items.remove(index))
    }

    /// 是否已含某个元素（按指针）。
    pub fn contains_ptr(&self, value: NonNull<Header>) -> bool {
        self.items.borrow().iter().any(|entry| *entry == value)
    }
}

impl DictObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(dict_traverse)
            .with_clear(dict_clear)
    }

    /// 条目数。
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    /// 按位置取条目（**借用**）——顺序是插入顺序。
    pub fn entry(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        self.entries.borrow().get(index).copied()
    }

    /// 全部条目（**借用**的副本）。
    pub fn entries(&self) -> Vec<(NonNull<Header>, NonNull<Header>)> {
        self.entries.borrow().clone()
    }

    /// 找某个键的位置（按指针；值相等的查找在 `executor` 里做）。
    pub fn position_of_ptr(&self, key: NonNull<Header>) -> Option<usize> {
        self.entries
            .borrow()
            .iter()
            .position(|(candidate, _)| *candidate == key)
    }

    /// 追加一个**新引用**的键值对（不查重；调用方先用"值相等"查过）。
    pub fn insert_raw(&self, key: NonNull<Header>, value: NonNull<Header>) {
        self.entries.borrow_mut().push((key, value));
    }

    /// 替换第 `index` 项的值（**新引用**），返回旧值（调用方负责释放）。
    pub fn replace_value(
        &self,
        index: usize,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        self.entries
            .borrow_mut()
            .get_mut(index)
            .map(|slot| core::mem::replace(&mut slot.1, value))
    }

    /// 删除第 `index` 项，返回 `(键, 值)`（调用方负责释放这两份引用）。
    pub fn remove(&self, index: usize) -> Option<(NonNull<Header>, NonNull<Header>)> {
        let mut entries = self.entries.borrow_mut();
        if index >= entries.len() {
            return None;
        }
        Some(entries.remove(index))
    }

    /// 写入一个**新引用**的键值对；键已存在时替换值并交出旧值（调用方负责释放）。
    pub fn insert(
        &self,
        key: NonNull<Header>,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        let mut entries = self.entries.borrow_mut();
        if let Some(slot) = entries
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
            .map(|slot| &mut slot.1)
        {
            return Some(core::mem::replace(slot, value));
        }
        entries.push((key, value));
        None
    }
}

/// `OM-40`：列出元组元素。
unsafe fn tuple_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TupleObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出元组元素。
unsafe fn tuple_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TupleObject>() };
    for value in object.items() {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

/// 见 [`tuple_traverse`]。
unsafe fn list_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}

/// 见 [`tuple_clear`]。
unsafe fn list_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    for value in object.items() {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

/// 见 [`tuple_traverse`]。
unsafe fn set_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    for value in object.items() {
        visit(value.as_ptr());
    }
}

/// 见 [`tuple_clear`]。
unsafe fn set_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    for value in object.items() {
        // SAFETY: 该引用由本对象持有。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

/// 见 [`tuple_traverse`]（键与值都要列）。
unsafe fn dict_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    for (key, value) in object.entries() {
        visit(key.as_ptr());
        visit(value.as_ptr());
    }
}

/// 见 [`tuple_clear`]（键与值都要交出）。
unsafe fn dict_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    for (key, value) in object.entries() {
        // SAFETY: 这些引用由本对象持有。
        unsafe {
            instance.release_object(key.as_ptr());
            instance.release_object(value.as_ptr());
        }
    }
}

// ---- `OM-11` 的 `new` 槽：类型被调用时的实例化（`T` 的构造）----

/// `object()`：无属性的裸实例。
pub unsafe fn plain_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    Some(instance.alloc(PlainObject::new(class)).into_raw().cast::<Header>())
}

/// 用户类（载荷是 [`AttributeObject`]）：空实例，字典惰性建立（`OM-14`）。
///
/// **实参不在这里处理**——参照实现里它们归 `__init__`（调用方拿到实例后再调它），
/// 所以这个槽**必须**收下任意实参、不因"有实参"而拒绝。
pub unsafe fn attribute_new(
    class: NonNull<crate::TypeObject>,
    _args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    Some(
        instance
            .alloc(AttributeObject::new(class, core::cell::RefCell::new(None)))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `int()`：0（零参形态；从字符串／其它类型构造随后补）。
pub unsafe fn int_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    Some(instance.new_int(0))
}

/// `bool()`：`False`（零参形态）。
pub unsafe fn bool_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    let flag = instance.singletons().boolean(false);
    // SAFETY: 单例由实例持有。
    unsafe { instance.incref_object(flag.as_ptr()) };
    Some(flag)
}

/// `float()`：0.0（零参形态）。
pub unsafe fn float_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    Some(
        instance
            .alloc(FloatObject::new(class, 0.0))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `list()`：空列表。
pub unsafe fn list_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    Some(
        instance
            .alloc(ListObject::new(class, core::cell::RefCell::new(Vec::new())))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `dict()`：空字典。
pub unsafe fn dict_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    Some(
        instance
            .alloc(DictObject::new(class, core::cell::RefCell::new(Vec::new())))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `set()`：空集合。
pub unsafe fn set_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    Some(
        instance
            .alloc(SetObject::new(class, core::cell::RefCell::new(Vec::new())))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `tuple()`：空元组。
pub unsafe fn tuple_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    Some(
        instance
            .alloc(TupleObject::new(class, Vec::new()))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `str()`：空串（走 `OM-23` 的单例）。
pub unsafe fn str_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    if !args.is_empty() {
        return None;
    }
    Some(instance.new_str(""))
}

/// 异常类：`ValueError("x")` —— **实参进 `args`**（借用视图，这里自己 incref）。
///
/// 这是 `raise ValueError("x")` 能跑通的那一半：编译器发的是"调用类 ＋ `RAISE_VARARGS 1`"。
pub unsafe fn exception_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let mut stored: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        stored.push(*argument);
    }
    let object = instance.alloc(ExceptionObject::new(
        class,
        core::cell::RefCell::new(stored),
        core::cell::RefCell::new(None),
        core::cell::RefCell::new(None),
        core::cell::Cell::new(false),
        core::cell::RefCell::new(None),
    ));
    Some(object.into_raw().cast::<Header>())
}

// ---- `OM-11` 的 `repr`／`str` 槽（形状**逐条实测**，见 tests/repr.rs 的文件头）----

/// 浮点的 `repr`：Rust 的 `{:?}` 已是"最短往返"，但指数写法与特殊值要和参照实现对齐
/// （实测：`1e+16`、`inf`、`nan`）。
fn float_repr_text(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    let text = format!("{value:?}");
    // Rust：`1e16`；参照实现：`1e+16`（指数带符号）
    match text.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with('-') && !exponent.starts_with('+') => {
            format!("{mantissa}e+{exponent}")
        }
        _ => text,
    }
}

/// `int` 的 `repr`：十进制。
pub unsafe fn int_repr(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<IntObject>() };
    Some(object.value.to_string())
}

/// `bool` 的 `repr`／`str`：`True`／`False`。
pub unsafe fn bool_repr(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<BoolObject>() };
    Some(if object.value { "True" } else { "False" }.to_owned())
}

/// `NoneType` 的 `repr`：`None`。
pub unsafe fn none_repr(_ptr: *mut Header, _instance: &Instance) -> Option<String> {
    Some("None".to_owned())
}

/// `float` 的 `repr`。
pub unsafe fn float_repr(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FloatObject>() };
    Some(float_repr_text(object.value))
}

/// `str` 的 `repr`：按参照实现的引号与转义规则（实测：能用单引号就用单引号）。
pub unsafe fn str_repr(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StrObject>() };
    Some(crate::instance::quote_str(object.value(), false))
}

/// `str` 的 `str`：内容本身。
pub unsafe fn str_str(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StrObject>() };
    Some(object.value().to_owned())
}

/// `list` 的 `repr`：`[a, b]`；自引用给 `[...]`（实测）。
pub unsafe fn list_repr(ptr: *mut Header, instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ListObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Some("[...]".to_owned());
    }
    let items = object.items();
    let mut text = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(instance, *item));
    }
    text.push(']');
    instance.leave_repr(ptr as usize);
    Some(text)
}

/// `tuple` 的 `repr`：空是 `()`、单个是 `(x,)`（实测）。
pub unsafe fn tuple_repr(ptr: *mut Header, instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<TupleObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Some("(...)".to_owned());
    }
    let mut text = String::from("(");
    for index in 0..object.len() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(
            instance,
            object.item(index).expect("下标在范围内"),
        ));
    }
    if object.len() == 1 {
        text.push(',');
    }
    text.push(')');
    instance.leave_repr(ptr as usize);
    Some(text)
}

/// `dict` 的 `repr`：`{k: v}`；自引用给 `{'k': {...}}`（实测）。
pub unsafe fn dict_repr(ptr: *mut Header, instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<DictObject>() };
    if !instance.enter_repr(ptr as usize) {
        return Some("{...}".to_owned());
    }
    let mut text = String::from("{");
    for (index, (key, value)) in object.entries().into_iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(instance, key));
        text.push_str(": ");
        text.push_str(&crate::executor::element_repr(instance, value));
    }
    text.push('}');
    instance.leave_repr(ptr as usize);
    Some(text)
}

/// `set` 的 `repr`：空是 `set()`、否则 `{a, b}`（实测）。
pub unsafe fn set_repr(ptr: *mut Header, instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<SetObject>() };
    let items = object.items();
    if items.is_empty() {
        return Some("set()".to_owned());
    }
    let mut text = String::from("{");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&crate::executor::element_repr(instance, *item));
    }
    text.push('}');
    Some(text)
}

/// 类型对象的 `repr`：`<class 'int'>`（实测）。
pub unsafe fn type_repr(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<crate::TypeObject>() };
    Some(format!("<class '{}'>", object.name()))
}

/// 生成器的 `repr`：`<generator object gen at 0x…>`（实测）。
pub unsafe fn generator_repr(ptr: *mut Header, instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<GeneratorObject>() };
    let frame = object.frame();
    // SAFETY: 帧由生成器持有，存活。
    let frame_ref = unsafe { &*frame.as_ptr().cast::<crate::Frame>() };
    let name = match frame_ref.code() {
        // SAFETY: code 由帧持有，存活。
        Some(code) => unsafe { code.cast::<crate::CodeObject>().as_ref() }.name(),
        None => "?",
    };
    let _ = instance;
    Some(format!("<generator object {name} at {ptr:p}>"))
}

/// 函数的 `repr`：`<function demo at 0x…>`（实测）。
pub unsafe fn function_repr(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    // SAFETY: 函数持有 code object 的一份引用。
    let code = object.code();
    // SAFETY: 同上。
    let name = unsafe { code.cast::<crate::CodeObject>().as_ref() }.name();
    Some(format!("<function {name} at {ptr:p}>"))
}

/// code object 的 `repr`：`<code object demo at 0x…, file "…", line 1>`（实测）。
pub unsafe fn code_repr(ptr: *mut Header, _instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<crate::CodeObject>() };
    Some(format!(
        "<code object {} at {ptr:p}, file \"{}\", line {}>",
        object.name(),
        object.filename(),
        object.firstlineno()
    ))
}

/// 绑定方法的 `repr`：`<bound method m of <C object at 0x…>>`。
///
/// 实测的形状是 `<bound method C.m of …>`（带 qualname）；本层的函数只存了
/// `co_name`（qualname 随类创建钩子接线后补），故这里给 `<bound method m of …>`。
pub unsafe fn method_repr(ptr: *mut Header, instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    let function = object.function();
    // SAFETY: 方法对象持有函数的一份引用。
    let function_ref = unsafe { &*function.as_ptr().cast::<FunctionObject>() };
    // SAFETY: 函数持有 code object 的一份引用。
    let code = function_ref.code();
    // SAFETY: 同上。
    let name = unsafe { code.cast::<crate::CodeObject>().as_ref() }.name();
    Some(format!(
        "<bound method {name} of {}>",
        instance.object_repr(object.this())
    ))
}

/// 异常实例的 `repr`／`str`：`ValueError('x')`（实测：无参是 `ValueError()`）。
/// **`OM-11` 的 `str` 槽**（异常实例）——**与 `repr` 不同**，形状实测：
///
/// - 没有实参 ⇒ 空串（`str(ValueError())` == `''`）
/// - **一个**实参 ⇒ 那个实参的 `str`（`str(ValueError('x'))` == `'x'`）
/// - 多个实参 ⇒ 实参元组的 `repr`（`str(ValueError('a','b'))` == `"('a', 'b')"`）
///
/// 这条差异是 `T-BC-22` 的行为夹具抓出来的：此前 `str` 槽直接挂了 `exception_repr`，
/// 于是 `str(e)` 与 `repr(e)` 一模一样（`str(ValueError('x'))` 给的是 `"ValueError('x')"`）。
///
/// # Safety
///
/// 契约见 `StrFn`。
pub unsafe fn exception_str(ptr: *mut Header, instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let header = unsafe { &*ptr };
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    let args = object.args();
    // `KeyError` 是**唯一**在单实参上改口径的：`str(KeyError('k'))` == `"'k'"`（实参的 repr）。
    // 实测：0 个实参仍是 `''`、≥2 个仍是实参元组的 repr（与基类同）⇒ 只改单实参那一格。
    // 子类继承（参照实现也继承）。
    let key_error_style = match instance.type_named("KeyError") {
        // SAFETY: 类型对象由注册表持有。
        Some(key_error) => instance.is_subtype(header.ty(), key_error),
        None => false,
    };
    match args.len() {
        0 => Some(String::new()),
        1 if key_error_style => Some(instance.object_repr(args[0])),
        1 => Some(instance.object_str(args[0])),
        _ => {
            let rendered: Vec<String> = args
                .iter()
                .map(|argument| instance.object_repr(*argument))
                .collect();
            Some(format!("({})", rendered.join(", ")))
        }
    }
}

/// **`OM-11` 的 `repr` 槽**（异常实例）：`ValueError('x')`／`ValueError()`（实测）。
///
/// # Safety
///
/// 契约见 `ReprFn`。
pub unsafe fn exception_repr(ptr: *mut Header, instance: &Instance) -> Option<String> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let header = unsafe { &*ptr };
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    // SAFETY: 类型名由注册表持有。
    let name = unsafe { header.ty().as_ref() }.name();
    let args = object.args();
    let rendered: Vec<String> = args
        .iter()
        .map(|argument| instance.object_repr(*argument))
        .collect();
    Some(format!("{name}({})", rendered.join(", ")))
}

// ---- `__format__` 的原生实现（`TS-44`：**没有** `format` 槽，内建类型在**类型字典**里
// ---- 放**原生可调用对象**；默认行为以参照实现为准：空规格 ⇒ `str(x)`，非空且类型未覆写 ⇒ TypeError）

use crate::format::{self, SpecError};

/// 从原生调用的实参里取格式规格（`__format__(self, spec)` 的 `spec`）。
///
/// **先验类型**再读载荷（`TS-43`：布局自选，故禁止跨类型硬转），返回 **owned** 文本
/// （免得把借用传来传去）。
fn spec_argument(instance: &Instance, args: &[NonNull<Header>]) -> String {
    let Some(first) = args.first().copied() else {
        return String::new();
    };
    // SAFETY: first 由调用方保证存活。
    if unsafe { first.as_ref() }.ty() != instance.singletons().str_type() {
        return String::new();
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*first.as_ptr().cast::<StrObject>() }.value().to_owned()
}

/// 把 `format.rs` 的结果折成"原生返回值或实测消息的异常"。
fn format_outcome(
    instance: &Instance,
    result: Result<String, SpecError>,
    class_name: &str,
) -> Result<NonNull<Header>, crate::ExecError> {
    match result {
        Ok(text) => Ok(instance.new_str(&text)),
        Err(SpecError::NegativeZero) => Err(crate::executor::raise_builtin(
            instance,
            "ValueError",
            format::NEGATIVE_ZERO_MESSAGE,
        )),
        Err(SpecError::UnknownCode(code)) => {
            let message = format!("Unknown format code '{code}' for object of type '{class_name}'");
            Err(crate::executor::raise_builtin(instance, "ValueError", &message))
        }
        Err(SpecError::NotImplemented) => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "这条格式化规格本层还没实现（迷你语言的其余部分）",
        }),
    }
}

/// `object.__format__`（默认）：空规格 ⇒ `str(x)`；非空 ⇒ TypeError（消息实测）。
pub unsafe fn native_format_object(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec = spec_argument(instance, args);
    if !spec.is_empty() {
        // SAFETY: this 由调用方保证存活。
        let class_name = unsafe { this.as_ref().ty().as_ref() }.name();
        let message = format!("unsupported format string passed to {class_name}.__format__");
        return Err(crate::executor::raise_builtin(instance, "TypeError", &message));
    }
    Ok(instance.new_str(&instance.object_str(this)))
}

/// `int.__format__`：空规格 ⇒ `str(self)`（于是 `bool` 走 `True`／`False`），否则数值规格。
pub unsafe fn native_format_int(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    if spec_text.is_empty() {
        return Ok(instance.new_str(&instance.object_str(this)));
    }
    // `bool` 继承 `int.__format__`（实测 `bool.__dict__` 里**没有** `__format__`），
    // 所以这里必须按**实际类型**读载荷：`BoolObject` 与 `IntObject` 是两个布局。
    // SAFETY: this 是存活对象。
    let this_header = unsafe { this.as_ref() };
    let type_name = unsafe { this_header.ty().as_ref() }.name();
    let value = if type_name == "bool" {
        // SAFETY: 类型身份已确认。
        i64::from(unsafe { &*this.as_ptr().cast::<BoolObject>() }.value)
    } else {
        // SAFETY: 同上。
        unsafe { &*this.as_ptr().cast::<IntObject>() }.value
    };
    match format::parse(&spec_text) {
        Ok(spec) => {
            let outcome = format::format_int(value, &spec);
            format_outcome(instance, outcome, type_name)
        }
        Err(error) => format_outcome(instance, Err(error), type_name),
    }
}

/// `float.__format__`：空规格 ⇒ `str(self)`，否则浮点规格。
pub unsafe fn native_format_float(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    if spec_text.is_empty() {
        return Ok(instance.new_str(&instance.object_str(this)));
    }
    // SAFETY: 契约上 this 是 float 实例。
    let value = unsafe { &*this.as_ptr().cast::<FloatObject>() }.value;
    match format::parse(&spec_text) {
        Ok(spec) => {
            let outcome = format::format_float(value, &spec);
            format_outcome(instance, outcome, "float")
        }
        Err(error) => format_outcome(instance, Err(error), "float"),
    }
}

/// `str.__format__`：空规格 ⇒ 内容本身；否则只认对齐／宽度／精度。
pub unsafe fn native_format_str(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "`__format__` 需要 self",
        });
    };
    let spec_text = spec_argument(instance, args);
    // SAFETY: 契约上 this 是 str 实例。
    let text = unsafe { &*this.as_ptr().cast::<StrObject>() }.value().to_owned();
    let spec = match format::parse(&spec_text) {
        Ok(spec) => spec,
        Err(error) => return format_outcome(instance, Err(error), "str"),
    };
    if let Some(code) = spec.ty {
        if code != 's' {
            return format_outcome(instance, Err(SpecError::UnknownCode(code)), "str");
        }
    }
    format_outcome(instance, format::format_str(&text, &spec), "str")
}
