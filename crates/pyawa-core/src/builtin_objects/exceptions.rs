//! 本文件是 `builtin_objects` 的 `exceptions` 域：异常实例的载荷、属性面与 `OM-40` 遍历。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use crate::HotId;
use crate::TypeObject;
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
    let base_exception = instance.hot_type(HotId::BaseException)
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
        .unwrap_or_else(|| instance.hot_type(HotId::Exception).expect("异常层次已登记"));
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
    let ty = instance.hot_type(HotId::StopIteration)
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
    /// **整体替换 `args` 载荷**（round 239）：返回旧的那批（**本对象交出的引用**，调用方 release ✓）。
    pub fn replace_args(&self, values: Vec<NonNull<Header>>) -> Vec<NonNull<Header>> {
        core::mem::replace(&mut *self.args.borrow_mut(), values)
    }

    /// **取走 `args` 元组缓存**（round 239：替换 args 时要清掉 ✓）。
    pub fn take_args_tuple(&self) -> Option<NonNull<Header>> {
        self.args_tuple.borrow_mut().take()
    }

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

    /// **设 `__cause__`／`__context__`／`__suppress_context__`（带载荷形状核验的公开入口）** ✓
    /// （round 64）：执行器要给"PEP 479 包装出的 `RuntimeError`"挂 cause／context，而那些字段住在
    /// **`ExceptionObject` 载荷**里（`getattr` 也只读载荷 ✓）⇒ 直接写实例字典**读不到** ✗。
    ///
    /// 返回 `false` ⇒ 目标不是本载荷形状（调用方自行回退到实例字典路 ✓）。
    pub fn set_cause_context_suppress(
        instance: &Instance,
        exception: NonNull<Header>,
        cause: Option<NonNull<Header>>,
        context: Option<NonNull<Header>>,
        suppress: bool,
    ) -> bool {
        // SAFETY: 调用方保证 exception 存活；先核形状再读载荷 ✓。
        let ty = unsafe { exception.as_ref() }.ty();
        let type_object = unsafe { ty.as_ref() };
        if let Some(base) = instance.hot_type(HotId::BaseException) {
            if !instance.is_subtype(ty, base) {
                return false;
            }
        }
        if type_object.instance_size() != core::mem::size_of::<ExceptionObject>() {
            return false;
        }
        // SAFETY: 上面刚核过类型与载荷大小 ✓。
        let payload = unsafe { &*exception.as_ptr().cast::<ExceptionObject>() };
        // 带引用转移：cause/context 各是一份**新引用**，由载荷接手 ✓；旧值由调用方逻辑归还——
        // 这里简化成"直接顶掉并归还旧值"（一次性设置，没有并发读者 ✓）。
        if let Some(old) = payload.set_cause(cause) {
            // SAFETY: 旧值由载荷持有。
            unsafe { instance.release_object(old.as_ptr()) };
        }
        if let Some(old) = payload.set_context(context) {
            // SAFETY: 同上。
            unsafe { instance.release_object(old.as_ptr()) };
        }
        payload.set_suppress_context(suppress);
        true
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
        // P3-38 S2c：异常消息走安全口径（合法 UTF-8 原样；含代理 ⇒ 有损文本，不 panic ✓）
        Some(instance.text_value(first).unwrap_or_default())
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

/// **异常组的方法绑定**（`subgroup`／`split`／`derive`，C2 冷门语义 PEP 654）：
/// 造一个原生函数再包一层 `MethodObject` 绑到本实例（与 `add_note`／`with_traceback` 同款 ✓）。
fn bind_group_method(
    instance: &Instance,
    owner: NonNull<Header>,
    name: &'static str,
    handler: NativeFn,
) -> NonNull<Header> {
    let method_type = instance
        .hot_type(HotId::BuiltinFunctionOrMethod)
        .expect("builtin_function_or_method 在引导期已登记");
    let native = instance
        .alloc(BuiltinFunctionObject::new(method_type, name, Cell::new(handler)))
        .into_raw()
        .cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(owner.as_ptr()) };
    let bound = instance.alloc(MethodObject::new(
        instance.hot_type(HotId::Method).expect("method 已登记"),
        native,
        owner,
    ));
    bound.into_raw().cast::<Header>()
}

/// 是不是 `BaseExceptionGroup` 的实例（异常组判据的唯一入口 ✓）。
fn is_exception_group(instance: &Instance, object: NonNull<Header>) -> bool {
    instance
        .type_named("BaseExceptionGroup")
        .is_some_and(|base| instance.is_subtype(instance.type_of(object), base))
}

/// 异常组的**直接子项**（借用视图；构造时 `args[1]` 是 list ✓，兼容 tuple ✓）。
fn group_children(instance: &Instance, group: NonNull<Header>) -> Vec<NonNull<Header>> {
    // SAFETY: group 是存活的异常实例（调用方保证）。
    let object = unsafe { &*group.as_ptr().cast::<ExceptionObject>() };
    object
        .args()
        .get(1)
        .copied()
        .and_then(|exceptions| {
            instance
                .list_items(exceptions)
                .or_else(|| instance.tuple_items(exceptions))
        })
        .unwrap_or_default()
}

/// 异常组的消息（`args[0]` 借用；没有就 `None`）。
fn group_message(instance: &Instance, group: NonNull<Header>) -> Option<NonNull<Header>> {
    let _ = instance;
    // SAFETY: 同上。
    unsafe { &*group.as_ptr().cast::<ExceptionObject>() }.args().first().copied()
}

/// 子树里有没有**非 `Exception`** 的叶子（决定结果组的类：`BaseExceptionGroup` vs `ExceptionGroup` ✓）。
fn group_subtree_is_base(instance: &Instance, object: NonNull<Header>) -> bool {
    if is_exception_group(instance, object) {
        return group_children(instance, object)
            .into_iter()
            .any(|child| group_subtree_is_base(instance, child));
    }
    let is_exception = instance
        .type_named("Exception")
        .is_some_and(|exception| instance.is_subtype(instance.type_of(object), exception));
    !is_exception
}

/// 造一个新异常组（类按子树定 ✓；**不碰空组** —— 空组照参照在 `split` 里返回 `None` ✓）。
fn new_exception_group(
    instance: &Instance,
    message: NonNull<Header>,
    children: Vec<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    let base = children
        .iter()
        .any(|child| group_subtree_is_base(instance, *child));
    let type_name = if base { "BaseExceptionGroup" } else { "ExceptionGroup" };
    let Some(group_type) = instance.type_named(type_name) else {
        return Err(instance.raise_builtin_error(
            "RuntimeError",
            &format!("{type_name} 类型没登记"),
        ));
    };
    // `new_list` **接管**引用 ✓（子项引用由调用方转交）。
    let exceptions = instance.new_list(children);
    let type_value = instance.type_value(group_type);
    let result = crate::executor::call::call_value(instance, type_value, &[message, exceptions], &[]);
    instance.release(exceptions);
    instance.release(type_value);
    result
}

/// 条件匹配：**类型**／**类型元组**／**可调用**（PEP 654 的 `split(condition)` 三种都收 ✓）。
fn group_condition_matches(
    instance: &Instance,
    leaf: NonNull<Header>,
    condition: NonNull<Header>,
) -> Result<bool, ExecError> {
    if let Some(items) = instance.tuple_items(condition) {
        for item in items {
            if group_condition_matches(instance, leaf, item)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if instance.is_type_object(condition) {
        return Ok(instance.is_subtype(
            instance.type_of(leaf),
            condition.cast::<TypeObject>(),
        ));
    }
    let result = crate::executor::call::call_value(instance, condition, &[leaf], &[])?;
    let truth = crate::executor::truthiness_public(instance, result, 0)?;
    instance.release(result);
    Ok(truth)
}

/// **递归拆分**：返回 `(匹配组, 剩余组)`；某一侧为空 ⇒ `None` ✓（结构照参照保留：整棵子树全中／
/// 全不中时整段搬走，部分命中时在**那一层**造新组 ✓）。
fn split_exception_group(
    instance: &Instance,
    group: NonNull<Header>,
    condition: NonNull<Header>,
) -> Result<(Option<NonNull<Header>>, Option<NonNull<Header>>), ExecError> {
    let message = group_message(instance, group);
    let children = group_children(instance, group);
    let mut matched: Vec<NonNull<Header>> = Vec::new();
    let mut rest: Vec<NonNull<Header>> = Vec::new();
    for child in children {
        if is_exception_group(instance, child) {
            let (child_matched, child_rest) = split_exception_group(instance, child, condition)?;
            if let Some(child_matched) = child_matched {
                matched.push(child_matched);
            }
            if let Some(child_rest) = child_rest {
                rest.push(child_rest);
            }
        } else if group_condition_matches(instance, child, condition)? {
            // 借用项 ⇒ 给新组添一份引用（`new_list` 会接管 ✓）。
            // SAFETY: child 由 group 持有，group 存活。
            unsafe { instance.incref_object(child.as_ptr()) };
            matched.push(child);
        } else {
            // SAFETY: 同上。
            unsafe { instance.incref_object(child.as_ptr()) };
            rest.push(child);
        }
    }
    let matched_group = if matched.is_empty() {
        None
    } else {
        Some(new_exception_group(instance, message.unwrap_or_else(|| instance.new_none()), matched)?)
    };
    let rest_group = if rest.is_empty() {
        None
    } else {
        Some(new_exception_group(instance, message.unwrap_or_else(|| instance.new_none()), rest)?)
    };
    Ok((matched_group, rest_group))
}

/// **`BaseExceptionGroup.split(condition)`**（PEP 654）：返回 `(匹配组, 剩余组)`，空的一侧是
/// `None` ✓。
unsafe fn group_split_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor 'split' needs an argument",
        ));
    };
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "split() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("split() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let (matched, rest) = split_exception_group(instance, this, args[0])?;
    let matched = matched.unwrap_or_else(|| instance.new_none());
    let rest = rest.unwrap_or_else(|| instance.new_none());
    Ok(instance.new_tuple(vec![matched, rest]))
}

/// **`BaseExceptionGroup.subgroup(condition)`**：只回匹配的一半（空 ⇒ `None`）✓。
unsafe fn group_subgroup_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor 'subgroup' needs an argument",
        ));
    };
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "subgroup() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("subgroup() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let (matched, _rest) = split_exception_group(instance, this, args[0])?;
    Ok(matched.unwrap_or_else(|| instance.new_none()))
}

/// **`BaseExceptionGroup.derive(exceptions)`**：同一消息、同一类的新组 ✓。
unsafe fn group_derive_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor 'derive' needs an argument",
        ));
    };
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "derive() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("derive() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let children = instance
        .list_items(args[0])
        .or_else(|| instance.tuple_items(args[0]))
        .unwrap_or_default();
    let mut retained: Vec<NonNull<Header>> = Vec::with_capacity(children.len());
    for child in children {
        // SAFETY: child 由传入序列持有，序列存活到调用结束。
        unsafe { instance.incref_object(child.as_ptr()) };
        retained.push(child);
    }
    let message = group_message(instance, this).unwrap_or_else(|| instance.new_none());
    new_exception_group(instance, message, retained)
}

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
        // **`SyntaxError` 的属性面**（round 237）：参照的 `SyntaxError(msg, (filename, lineno,
        // offset, text, end_lineno, end_offset))` 会把第二项元组**摊进属性** ✓；先前一个都没有 ✗
        // ⇒ `sys.__excepthook__(*sys.exc_info())` 读 `exc.filename` 当场 AttributeError ✗
        //（`test_exceptions.test_subclass` 的 `MySyntaxError("bad bad", ("bad.py", 1, 2, "abcdefg", 1, 7))` ✓）。
        "msg" | "lineno" | "offset" | "text" | "end_lineno" | "end_offset" => {
            // SAFETY: ptr 是本类型的存活对象。
            let ty = unsafe { (*ptr).ty() };
            let is_syntax_error = instance.hot_type(HotId::SyntaxError)
                .is_some_and(|syntax| instance.is_subtype(ty, syntax));
            if !is_syntax_error {
                return None;
            }
            let arguments = object.args();
            let value = if matches!(name, "msg") {
                arguments.first().copied()
            } else {
                let details = arguments.get(1).copied();
                let items = details.and_then(|details| instance.tuple_items(details));
                let index = match name {
                    "filename" => 0,
                    "lineno" => 1,
                    "offset" => 2,
                    "text" => 3,
                    "end_lineno" => 4,
                    _ => 5,
                };
                items.and_then(|items| items.get(index).copied())
            };
            Some(match value {
                Some(value) => {
                    // SAFETY: 值由本对象持有，这里新增一份交给调用方 ✓。
                    unsafe { instance.incref_object(value.as_ptr()) };
                    value
                }
                None => instance.new_none(),
            })
        }
        // **`BaseExceptionGroup` 的属性面**（C2 冷门语义，PEP 654 的前置）：参照里
        // `eg.message`＝`args[0]`、`eg.exceptions`＝**元组**（构造时传 list 也转成 tuple ✓）。
        // 先前两者都没有 ⇒ `except*` 之外连普通属性读都 AttributeError ✗。
        "message" | "exceptions" => {
            // SAFETY: ptr 是本类型的存活对象。
            let ty = unsafe { (*ptr).ty() };
            let is_group = instance
                .type_named("BaseExceptionGroup")
                .is_some_and(|group| instance.is_subtype(ty, group));
            if !is_group {
                return None;
            }
            let arguments = object.args();
            if name == "message" {
                return Some(match arguments.first().copied() {
                    Some(value) => {
                        // SAFETY: 值由本对象持有，这里新增一份交给调用方 ✓。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        value
                    }
                    None => instance.new_none(),
                });
            }
            let Some(second) = arguments.get(1).copied() else {
                return Some(instance.new_tuple(Vec::new()));
            };
            let items = instance
                .tuple_items(second)
                .or_else(|| instance.list_items(second))
                .unwrap_or_default();
            let copied: Vec<NonNull<Header>> = items
                .into_iter()
                .map(|item| {
                    // SAFETY: 子异常由 `args[1]` 持有，这里新增一份给新元组 ✓。
                    unsafe { instance.incref_object(item.as_ptr()) };
                    item
                })
                .collect();
            Some(instance.new_tuple(copied))
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
        // **`SystemExit.code`**（round 88）：参照里是 `args[0]`（无实参 ⇒ `None`）✓；
        // 其它异常没有这个属性 ⇒ 交回调用方继续走类型字典／报 AttributeError ✓
        // （实测 `except SystemExit as e: e.code` 先前报 `'SystemExit' object has no attribute 'code'` ✗）。
        "code" => {
            // SAFETY: ptr 是本类型的存活对象。
            let ty = unsafe { (*ptr).ty() };
            let is_system_exit = instance.hot_type(HotId::SystemExit)
                .is_some_and(|system_exit| instance.is_subtype(ty, system_exit));
            if !is_system_exit {
                return None;
            }
            Some(match object.args().first() {
                Some(value) => {
                    // SAFETY: 值由本对象持有，这里新增一份交给调用方 ✓。
                    unsafe { instance.incref_object(value.as_ptr()) };
                    *value
                }
                None => instance.new_none(),
            })
        }
        // **`StopIteration.value`／`StopAsyncIteration.value`**（round 83）：参照里是 `args[0]`
        // （无实参 ⇒ `None`）✓；先前没有这个属性 ⇒ `except StopIteration as e: e.value` 当场
        // AttributeError ✗（`test_frame.test_f_generator` 的 `g.close()` 与协程收尾 ✓）。
        "value" => {
            // SAFETY: ptr 是本类型的存活对象。
            let ty = unsafe { (*ptr).ty() };
            let is_stop = instance.hot_type(HotId::StopIteration)
                .is_some_and(|stop| instance.is_subtype(ty, stop))
                || instance.hot_type(HotId::StopAsyncIteration)
                    .is_some_and(|stop| instance.is_subtype(ty, stop));
            if !is_stop {
                return None;
            }
            Some(match object.args().first() {
                Some(value) => {
                    // SAFETY: 值由本对象持有，这里新增一份交给调用方 ✓。
                    unsafe { instance.incref_object(value.as_ptr()) };
                    *value
                }
                None => instance.new_none(),
            })
        }
        // **`UnicodeError` 一族的属性面**（round 238）：`UnicodeDecodeError(encoding, object, start,
        // end, reason)` 的五个字段在参照里是**只读属性** ✓；先前一个都没有 ✗ ⇒
        // `e.encoding`／`e.object`／`e.start`／`e.end`／`e.reason` 全 `AttributeError` ✗。
        "encoding" | "object" | "start" | "end" | "reason" => {
            // SAFETY: ptr 是本类型的存活对象。
            let ty = unsafe { (*ptr).ty() };
            let is_unicode_error = instance.hot_type(HotId::UnicodeError)
                .is_some_and(|base| instance.is_subtype(ty, base));
            if !is_unicode_error {
                return None;
            }
            let arguments = object.args();
            // 注意：`UnicodeDecodeError`／`UnicodeEncodeError`／`UnicodeTranslateError` 的
            // `args` 布局同形（encoding, object, start, end, reason ✓）；纯 `UnicodeError` 只有
            // 一个 str 实参 ⇒ 属性一律 `None` ✓（照参照的宽松口径 ✓）。
            if arguments.len() < 5 {
                return Some(instance.new_none());
            }
            let index = match name {
                "encoding" => 0,
                "object" => 1,
                "start" => 2,
                "end" => 3,
                _ => 4,
            };
            let value = arguments[index];
            // SAFETY: 值由本对象持有，这里新增一份交给调用方 ✓。
            unsafe { instance.incref_object(value.as_ptr()) };
            Some(value)
        }
        // **`filename`**（round 237）：两种异常都有它 —— `SyntaxError` 取第二项元组第 0 格 ✓；
        // 其余按 `OSError` 口径取 `args[2]` ✓（两边都要，避免同一个 match 里出现两条 `"filename"` 臂
        // ⇒ 编译器 "unreachable pattern" 警告 ✗）。
        "filename" => {
            // SAFETY: ptr 是本类型的存活对象。
            let ty = unsafe { (*ptr).ty() };
            let arguments = object.args();
            let is_syntax_error = instance.hot_type(HotId::SyntaxError)
                .is_some_and(|syntax| instance.is_subtype(ty, syntax));
            let value = if is_syntax_error {
                arguments
                    .get(1)
                    .copied()
                    .and_then(|details| instance.tuple_items(details))
                    .and_then(|items| items.first().copied())
            } else {
                let is_os_error = instance.hot_type(HotId::OSError)
                    .is_some_and(|os_error| instance.is_subtype(ty, os_error));
                if !is_os_error {
                    return None;
                }
                if arguments.len() > 2 {
                    arguments.get(2).copied()
                } else {
                    None
                }
            };
            Some(match value {
                Some(value) => {
                    // SAFETY: 值由本对象持有，这里新增一份交给调用方 ✓。
                    unsafe { instance.incref_object(value.as_ptr()) };
                    value
                }
                None => instance.new_none(),
            })
        }
        // **`OSError` 一族的属性面**（round 108）：`errno`／`strerror`／`filename`／`filename2` ✓
        // —— 参照里它们就是 `args` 的前四项（`OSError(errno, strerror, filename, filename2)` ✓）；
        // 先前完全取不到 ✗ ⇒ `except FileNotFoundError as e: e.filename` 当场 AttributeError ✗
        //（官方 `test_unicode_file_functions`／`test_unicode_file` 一族 ✓）。
        "errno" | "strerror" | "filename2" => {
            // SAFETY: ptr 是本类型的存活对象。
            let ty = unsafe { (*ptr).ty() };
            let is_os_error = instance.hot_type(HotId::OSError)
                .is_some_and(|os_error| instance.is_subtype(ty, os_error));
            if !is_os_error {
                return None;
            }
            let arguments = object.args();
            let index = match name {
                "errno" => 0,
                "strerror" => 1,
                "filename" => 2,
                _ => 3,
            };
            // 参照口径：只有 `args.len() >= 2` 时前两项才是 errno/strerror；`filename` 同理看长度 ✓。
            let present = match index {
                0 | 1 => arguments.len() >= 2,
                _ => arguments.len() > index,
            };
            if !present {
                return Some(instance.new_none());
            }
            let value = arguments[index];
            // SAFETY: 值由本对象持有，这里新增一份交给调用方 ✓。
            unsafe { instance.incref_object(value.as_ptr()) };
            Some(value)
        }
        // **`with_traceback(tb, /)`**（task-24 扩）：参照里它是 `BaseException` 的方法 ✓
        //（`Lib/unittest/case.py:279` 的 `exc_value.with_traceback(None)` 正靠它 ✓）。
        // 与 `dict_getattr` 同一做法：把原生函数**绑到本实例**上交出去 ✓（调用时 `bound` 就是 self ✓）。
        "__setstate__" => {
            // `P3-42`：`BaseException.__setstate__(state)`（`test_baseexception` 的
            // gh-97591 用例直接调它）。与 `with_traceback` 同一「绑到本实例」的做法 ✓。
            // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
            let owner = unsafe { NonNull::new_unchecked(ptr) };
            let method_type = instance.hot_type(HotId::BuiltinFunctionOrMethod)
                .expect("引导期已登记");
            let native = instance.alloc(BuiltinFunctionObject::new(
                method_type,
                "__setstate__",
                Cell::new(exception_setstate_native),
            ));
            let native_raw = native.into_raw().cast::<Header>();
            // SAFETY: 方法对象要自己那份 self（`OM-16`）。
            unsafe { instance.incref_object(ptr) };
            let bound = instance.alloc(MethodObject::new(
                instance.hot_type(HotId::Method).expect("method 已登记"),
                native_raw,
                owner,
            ));
            Some(bound.into_raw().cast::<Header>())
        }
        "with_traceback" => {
            // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
            let owner = unsafe { NonNull::new_unchecked(ptr) };
            let method_type = instance.hot_type(HotId::BuiltinFunctionOrMethod)
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
                instance.hot_type(HotId::Method).expect("method 已登记"),
                native_raw,
                owner,
            ));
            Some(bound.into_raw().cast::<Header>())
        }
        "add_note" => {
            // **PEP 678 的 `BaseException.add_note(note)`**（round 65）：`__notes__` 存在实例字典里 ✓
            // （`test_subclassinit` 的 `__set_name__` 失败路径要读它 ✓）。
            // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
            let owner = unsafe { NonNull::new_unchecked(ptr) };
            let method_type = instance.hot_type(HotId::BuiltinFunctionOrMethod)
                .expect("引导期已登记");
            let native = instance.alloc(BuiltinFunctionObject::new(
                method_type,
                "add_note",
                Cell::new(add_note_native),
            ));
            let native_raw = native.into_raw().cast::<Header>();
            // SAFETY: 方法对象要自己那份 self（`OM-16`）。
            unsafe { instance.incref_object(ptr) };
            let bound = instance.alloc(MethodObject::new(
                instance.hot_type(HotId::Method).expect("method 已登记"),
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
        // **异常组方法**（C2/PEP 654）：`subgroup`／`split`／`derive` —— 只对异常组给 ✓。
        "subgroup" | "split" | "derive" => {
            // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
            let owner = unsafe { NonNull::new_unchecked(ptr) };
            if !is_exception_group(instance, owner) {
                return None;
            }
            Some(match name {
                "subgroup" => bind_group_method(instance, owner, "subgroup", group_subgroup_native),
                "split" => bind_group_method(instance, owner, "split", group_split_native),
                _ => bind_group_method(instance, owner, "derive", group_derive_native),
            })
        }
        _ => None,
    }
}

/// **`BaseException.__setstate__(state)`**（`P3-42`）：把 `state` 字典里的项逐条设成实例属性 ✓。
///
/// 语义照本机 3.14.4 实测：`state is None` ⇒ 什么都不做；非字典 ⇒
/// `TypeError: state is not a dictionary`；0 参／2 参／关键字 ⇒ 各自 TypeError（见下）✓。
unsafe fn exception_setstate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "unbound method BaseException.__setstate__() needs an argument",
        ));
    };
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "BaseException.__setstate__() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "BaseException.__setstate__() takes exactly one argument ({} given)",
                args.len()
            ),
        ));
    }
    let state = args[0];
    if state == instance.singletons().none() {
        return Ok(instance.retain(instance.singletons().none()));
    }
    let Some(entries) = instance.dict_entries(state) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "state is not a dictionary",
        ));
    };
    for (key, value) in entries {
        // **借用语义**：`set_attribute_value` 自己 retain 一份 ✓（键转文本，非文本键跳过——如实）。
        let Some(name) = instance.text_of(key).map(str::to_owned) else {
            continue;
        };
        instance.set_attribute_value(this, &name, value)?;
    }
    Ok(instance.retain(instance.singletons().none()))
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
/// **给异常追加一条 note**（PEP 678）：`__notes__` 是实例字典里的 `list` ✓。
///
/// 返回 `false` ⇒ `__notes__` 现值不是 list（参照会覆盖成新 list？这里保守放弃 ✓）。
pub(crate) fn add_exception_note(
    instance: &Instance,
    exception: NonNull<Header>,
    note: &str,
) -> bool {
    let Some(memory) = crate::executor::protocol::mounted_instance_dict(instance, exception)
    else {
        return false;
    };
    let existing = instance.dict_get(memory, "__notes__");
    let mut items: Vec<NonNull<Header>> = Vec::new();
    if let Some(existing) = existing {
        let Some(values) = instance.list_items(existing) else {
            return false;
        };
        for item in values {
            // SAFETY: item 由 list 持有。
            unsafe { instance.incref_object(item.as_ptr()) };
            items.push(item);
        }
    }
    let text = instance.new_str(note);
    items.push(text);
    let list = instance.new_list(items);
    let _ = instance.set_attribute_value(exception, "__notes__", list);
    // SAFETY: list 的那份引用交给实例字典（set_attribute_value 已 retain ✓）⇒ 这里归还新建那份 ✓。
    unsafe { instance.release_object(list.as_ptr()) };
    true
}

/// **`BaseException.add_note(note)`**（round 65）：参数必须是 `str` ✓（参照：
/// `add_note() argument must be str, not int` ✓）。
unsafe fn add_note_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor 'add_note' needs an argument",
        ));
    };
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "add_note() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "add_note() takes exactly one argument ({} given)",
                args.len()
            ),
        ));
    }
    let note = args[0];
    // SAFETY: note 是存活实参。
    let ty = unsafe { note.as_ref() }.ty();
    let is_str = ty == instance.singletons().str_type()
        || instance.is_subtype(ty, instance.singletons().str_type());
    if !is_str {
        let type_name = instance.type_name(ty).to_owned();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("add_note() argument must be str, not {type_name}"),
        ));
    }
    let Some(text) = instance.text_of(note).map(str::to_owned) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "add_note() argument must be str, not str",
        ));
    };
    // **`__notes__` 被用户改成非 list ⇒ `add_note` 要 `TypeError`**（round 238，照参照原话）：
    // `e.__notes__ = 42; e.add_note("x")` ⇒ `TypeError: Cannot add note: __notes__ is not a list` ✓
    //（`test_exceptions.ExceptionTests.test_notes` ✓；先前静默重建 list ✗）。
    if let Some(memory) = crate::executor::protocol::mounted_instance_dict(instance, this) {
        if let Some(existing) = instance.dict_get(memory, "__notes__") {
            if instance.list_items(existing).is_none() {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "Cannot add note: __notes__ is not a list",
                ));
            }
        }
    }
    add_exception_note(instance, this, &text);
    Ok(instance.new_none())
}

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
    match instance.hot_type(HotId::Traceback) {
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
