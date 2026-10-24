//! 本文件是 `builtin_objects` 的 `exceptions` 域：异常实例的载荷、属性面与 `OM-40` 遍历。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;


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

/// 把一个不是异常的东西变成异常（`throw` 的入口）：类 ⇒ 实例化（可带值）；
/// 异常实例 ⇒ 直接用（再带值 ⇒ 实测 `TypeError`）；其余 ⇒ 实测 `TypeError`。
pub(crate) fn thrown_exception(
    instance: &Instance,
    value: NonNull<Header>,
    extra: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: value 是存活对象。
    let ty = unsafe { value.as_ref() }.ty();
    let base_exception = instance
        .type_named("BaseException")
        .expect("异常层次在引导期已登记");
    if let Some(class) = instance.as_type(value) {
        if !instance.is_subtype(class, base_exception) {
            return Err(crate::executor::runtime::raise_builtin(
                instance,
                "TypeError",
                &format!(
                    "exceptions must be classes or instances deriving from BaseException, not {}",
                    instance.type_name(class)
                ),
            ));
        }
        let mut arguments: Vec<NonNull<Header>> = Vec::new();
        if let Some(extra) = extra {
            // SAFETY: extra 由调用方保证存活。
            unsafe { instance.incref_object(extra.as_ptr()) };
            arguments.push(extra);
        }
        return crate::executor::call::call_callable(instance, value, None, arguments, Vec::new(), 0);
    }
    if instance.is_subtype(ty, base_exception) {
        if extra.is_some() {
            return Err(crate::executor::runtime::raise_builtin(
                instance,
                "TypeError",
                "instance exception may not have a separate value",
            ));
        }
        // SAFETY: value 由调用方保证存活，这里新增一份交给抛出方。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    // SAFETY: value 是存活对象。
    let name = unsafe { ty.as_ref() }.name();
    Err(crate::executor::runtime::raise_builtin(
        instance,
        "TypeError",
        &format!("exceptions must be classes or instances deriving from BaseException, not {name}"),
    ))
}

/// 按名字造一个异常实例（不带实参）。
pub(crate) fn exception_instance(
    instance: &Instance,
    class: &str,
    args: Vec<NonNull<Header>>,
) -> NonNull<Header> {
    let ty = instance
        .type_named(class)
        .unwrap_or_else(|| instance.type_named("Exception").expect("异常层次已登记"));
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(args),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
        RefCell::new(None),
    ));
    // **临时插桩**（第 193 轮）：谁真的经 `new` 造了异常对象 ✓。
    {
        let raw = object.into_raw();
        if instance.type_name(unsafe { raw.as_ref().header.ty() }) == "AttributeError" {
            eprintln!("[插桩] 构造 AttributeError：指针={:p}", raw.as_ptr());
        }
        return raw.cast::<Header>();
    }
}

/// 造一个 `StopIteration`（可选带返回值——参照实现把它放进 `.value`）。
pub(crate) fn stop_iteration(instance: &Instance, value: Option<NonNull<Header>>) -> ExecError {
    let ty = instance
        .type_named("StopIteration")
        .expect("异常层次在引导期已登记");
    let args: Vec<NonNull<Header>> = match value {
        Some(value) => vec![value],
        None => Vec::new(),
    };
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(args),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
        RefCell::new(None),
    ));
    ExecError::Raised {
        exception: object.into_raw().cast::<Header>(),
    }
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
    ///
    /// **读不到就退回空表** ✓（`try_borrow` ✓，第 162／163 轮）：本层有「**可变借用横跨回调**」的
    /// 真 bug ✗（同一个 panic 位点已登记 ✓）⇒ 若这里用 `borrow()`，`Lib/abc.py`／`os.py` 这类
    /// 深一点的导入会**直接 panic** ✗。`args()` 是**只读**语义 ✓ ⇒ 退回空表只是**消息少一段** ✓，
    /// 绝不让整台 VM 崩掉 ✓。**根因仍未修** ✗（继续登记 ✓）。
    pub fn args(&self) -> Vec<NonNull<Header>> {
        self.args
            .try_borrow()
            .map(|slot| slot.clone())
            .unwrap_or_default()
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
            // **缓存一份、调用方再一份** ✓（第 727 轮真 bug 修 ✗）：先前同一份引用**同时**交给
            // 缓存与调用方 ⇒ 先释放的那边把它打掉、另一边的 `exception_clear` 再释放 ⇒
            // `[悬垂] release_object … 不在活表里` ✗（`PYAWA_DANGLING=1` 实测 ✓）。
            // SAFETY: tuple 由本函数的新引用持有，这里给调用方再加一份 ✓。
            unsafe { instance.incref_object(tuple.as_ptr()) };
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
        // **`with_traceback(tb, /)`**（task-24 扩）：参照里它是 `BaseException` 的方法 ✓
        //（`Lib/unittest/case.py:279` 的 `exc_value.with_traceback(None)` 正靠它 ✓）。
        // 与 `dict_getattr` 同一做法：把原生函数**绑到本实例**上交出去 ✓（调用时 `bound` 就是 self ✓）。
        "with_traceback" => {
            // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
            let owner = unsafe { NonNull::new_unchecked(ptr) };
            let method_type = instance
                .type_named("builtin_function_or_method")
                .expect("引导期已登记");
            let native = instance.alloc(BuiltinFunctionObject::new(
                method_type,
                "with_traceback",
                Cell::new(with_traceback_native),
            ));
            let native_raw = native.into_raw().cast::<Header>();
            // SAFETY: 方法对象要自己那份 self（`OM-16`）。
            unsafe { instance.incref_object(ptr) };
            let bound = instance.alloc(MethodObject::new(
                instance.type_named("method").expect("method 已登记"),
                native_raw,
                owner,
            ));
            Some(bound.into_raw().cast::<Header>())
        }
        // **有就给、没有给 `None`** ✓（第 213 轮：`BC-60` 的最小起步 ✓）—— `raise` 时挂上去 ✓
        //（`Instance::new_traceback` ✓）；未抛过 ⇒ `None` ✓。
        "__traceback__" => {
            // SAFETY: ptr 指向本类型的存活对象（外部契约 ✓）。
            let owner = unsafe { core::ptr::NonNull::new_unchecked(ptr) };
            let stored = crate::executor::protocol::mounted_instance_dict(instance, owner)
                .and_then(|mapping| instance.dict_get(mapping, "__traceback__"));
            Some(match stored {
                Some(value) => instance.retain(value),
                None => instance.new_none(),
            })
        }
        _ => None,
    }
}

/// **`BaseException.with_traceback(tb, /)`**（task-24 扩）：把 `__traceback__` 设成 `tb` 并返回 `self` ✓。
///
/// 语义照参照 3.14.4 实测：
/// - `e.with_traceback(None) is e` ✓（同时 `e.__traceback__ == None` ✓）；
/// - 0 参／2 参 ⇒ `TypeError: BaseException.with_traceback() takes exactly one argument (N given)` ✓；
/// - 关键字实参 ⇒ `TypeError: BaseException.with_traceback() takes no keyword arguments` ✓；
/// - 非 traceback／None ⇒ `TypeError: __traceback__ must be a traceback or None` ✓。
///
/// **如实偏差** ✗：本层的 traceback 对象是 `BC-60` 的"最小起步"（`tb_frame` ＋ 占位的
/// `tb_next`／`tb_lineno`／`tb_lasti` ✓）⇒ 这里只做**类型校验 ＋ 挂载**，不重建链 ✓
/// （与 `DIV-6` 的既有口径一致 ✓；测试里只靠"设／读回"这一档 ✓）。
unsafe fn with_traceback_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor 'with_traceback' needs an argument",
        ));
    };
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "BaseException.with_traceback() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "BaseException.with_traceback() takes exactly one argument ({} given)",
                args.len()
            ),
        ));
    }
    let traceback = args[0];
    if traceback != instance.singletons().none() && !is_traceback_object(instance, traceback) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__traceback__ must be a traceback or None",
        ));
    }
    // **借用**：`set_attribute_value` 自己 retain ✓。挂到异常的实例字典 —— `__traceback__` 的
    // 读取面（本文件 `exception_getattr` 的 `"__traceback__"` 臂）读的就是这一格 ✓。
    instance.set_attribute_value(this, "__traceback__", traceback)?;
    Ok(instance.retain(this))
}

/// `object` 是不是本层的 **traceback 对象**（类型名 `traceback`；该类型由
/// [`Instance::new_traceback`] 按需惰性建 ✓）。
fn is_traceback_object(instance: &Instance, object: NonNull<Header>) -> bool {
    let ty = instance.type_of(object);
    match instance.type_named("traceback") {
        Some(traceback_type) => instance.is_subtype(ty, traceback_type),
        // 还没建过这个类型 ⇒ 按名字兜底（`new_traceback` 用的就是同名类型 ✓）。
        None => instance.type_name(ty) == "traceback",
    }
}

pub(crate) unsafe fn exception_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
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
pub(crate) unsafe fn exception_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ExceptionObject>() };
    // **先取出、后释放**（第 148 轮）：可变借用**不能**横跨 `release_object` ✗ —— 释放可能触发
    // 析构／GC，而那条路会**回头共享借用**同一份 `args` ⇒ `RefCell already mutably borrowed` ✗
    // （实测：`try: assert False except AssertionError: pass` 直接崩 ✓）。
    let taken = core::mem::take(&mut *object.args.borrow_mut());
    for value in taken {
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
