//! 本文件是 `builtin_objects` 的 `callables` 域：可调用对象与描述符（原生函数／`function`／`method`／`classmethod`／`staticmethod`／`property`／`attribute`／`weakref`）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;


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
    /// `function` 的实例：一个 code object ＋ 默认值。
    ///
    /// **注解与 `__qualname__` 都已就位**（`SET_FUNCTION_ATTRIBUTE` 的 16／2 与 `code_with_qualname`）；
    /// 仍缺的是**闭包**（`COPY_FREE_VARS` 一族）；下面这行是历史沿革
    /// 的其余标志位（实测 8 ＝ closure、16 ＝ annotate）与属性族补齐。
    pub struct FunctionObject {
        /// 被执行的 code object（**本对象持有一份引用**）。
        code: NonNull<Header>,
        /// 位置参数默认值（对齐到**尾部**若干位置参数，与参照实现一致）。
        defaults: Vec<NonNull<Header>>,
        /// 仅关键字参数默认值（`dict`，可为空）。
        kwdefaults: Option<NonNull<Header>>,
        /// **`__globals__`**：函数**定义处**的全局映射（**本对象持有一份引用**）。
        ///
        /// `MAKE_FUNCTION` 时从当前帧取（`BC-57` 的 `LOAD_GLOBAL` 要它）；
        /// 模块体的帧没有单独的全局表，此时取它的**命名空间**。
        globals: RefCell<Option<NonNull<Header>>>,
        /// **闭包**（`SET_FUNCTION_ATTRIBUTE` 的 bit3 `closure(8)`；第 82 轮接线）：
        /// 值是 **cell 元组**，建帧时装进自由槽（`CPython` 3.11+ 在建帧阶段做）。
        closure: RefCell<Vec<NonNull<Header>>>,
        /// **`__annotate__`**（`SET_FUNCTION_ATTRIBUTE` 的 bit4；3.14 的**延迟注解**协议，
        /// `SPEC-bytecode.md` §… 的表与 `SPEC-type-system.md` 都要求它存在）。
        ///
        /// 值是那个"按 `format` 参数产出注解字典"的**可调用对象**（本对象持一份引用）；
        /// `__annotations__` 已落地（惰性调用 ＋ 缓存）；`__annotate_func__`／
        /// `__annotations_cache__` 在参照实现里**不是**函数属性（那是 `typing` 自己对象上的名字）。
        annotate: RefCell<Option<NonNull<Header>>>,
        /// **`__annotations__` 的缓存**（实测：同一函数的 `f.__annotations__` 是**同一对象**，
        /// 且 `__annotate__` 只被调用**一次**）。
        annotations_cache: RefCell<Option<NonNull<Header>>>,
        // **可写属性的覆盖值组**（`py_object!` 第二臂：`new` 不收它们 ✓）：
        // 参照 3.14 实测：`f.__name__ = …`／`f.__qualname__ = …`／`f.__doc__ = …`／
        // `f.__module__ = …` **不**写进实例字典 ✓（`f.__dict__` 仍是 `{}` ✓），也不碰
        // code object —— 是函数对象自己的槽 ✓。`None` ＝ 用 code object／`__globals__`
        // 的默认值；`Some(None 单例)` ＝ 显式覆盖成 `None`（`del f.__doc__` 之后就是它 ✓）。
        {
            /// `__name__` 的覆盖值。
            name_override: RefCell<Option<NonNull<Header>>>,
            /// `__qualname__` 的覆盖值。
            qualname_override: RefCell<Option<NonNull<Header>>>,
            /// `__doc__` 的覆盖值（`del` 之后是 `Some(None 单例)` ✓）。
            doc_override: RefCell<Option<NonNull<Header>>>,
            /// `__module__` 的覆盖值（`del` 之后是 `Some(None 单例)` ✓）。
            module_override: RefCell<Option<NonNull<Header>>>,
        }
    }
}

/// `slice(...)`：`slice(stop)`／`slice(start, stop[, step])`（实测的三种形态）。
pub unsafe fn classmethod_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **无参也合法** ✓（第 186 轮实测：参照的 `classmethod()` 默认 `f=None` ✓ ——
    // `Lib/importlib/_bootstrap.py` 正是这么用的 ✓）。
    let function = match args.first() {
        Some(given) => *given,
        None => instance.singletons().none(),
    };
    instance.retain(function);
    // **用调用方给的那个类** ✓（同 `staticmethod_new` ✓；`Lib/abc.py` 的
    // `class abstractclassmethod(classmethod)` 就靠这条 ✓）。
    let ty = class;
    // **走宏生成的 `new`** ✓（它接收字段作参数 ✓）：这样既符合规范 ✓，也消掉「never used」警告 ✓
    //（第 158 轮的教训 ✓：直接写字面量会绕过它 ✗）。
    Ok(instance
        .alloc_payload(ClassMethodObject::new(ty, function))
        .cast::<Header>())
}

/// **`staticmethod.__get__`／`classmethod.__get__` 的实参口径**（`P3-17`）
///
/// 参照 3.14.4 实测：
/// - 实参个数 1 或 2 ✓（0 ⇒ `TypeError: __get__ expected at least 1 argument, got 0`；
///   3+ ⇒ `TypeError: __get__ expected at most 2 arguments, got N` ✓）；
/// - `obj is None` **且** `owner` 缺省或 `None` ⇒ `TypeError: __get__(None, None) is invalid` ✓；
/// - `owner` 显式 `None` 与"缺省"同义 ✓。

/// **内建"包装对象"的载荷布局核对**（防止把 Python 子类实例当内建载荷读 ⇒ UB／崩溃 ✗）。
///
/// 本层 `class X(classmethod)` 这类**内建类型的 Python 子类**目前落在**通用 `AttributeObject`
/// 布局**上（`classes.rs` 的 `AB-58` 布局表只纳了容器／数值族 ✗）⇒ 载荷里**没有**被包的
/// 函数 ✗。描述符 `__get__` 若照 `ClassMethodObject` 强转就会读到 NULL（实测 abort ✗）。
/// 判据：**类型同名**（正常路径 ✓），或**实例尺寸与该内建类型一致**（将来的"布局沿用"档 ✓）。
/// 两者都不满足 ⇒ 回一句可定位的 `TypeError`（**不崩** ✓，也不伪装成差异 ✓）。
pub(crate) fn builtin_wrapper_layout_ok(
    instance: &Instance,
    wrapper: NonNull<Header>,
    base_name: &str,
    expected_size: usize,
) -> bool {
    let wrapper_type = instance.type_of(wrapper);
    if instance.type_name(wrapper_type) == base_name {
        return true;
    }
    // SAFETY: wrapper_type 由注册表持有。
    unsafe { wrapper_type.as_ref() }.instance_size() == expected_size
}

/// "内建包装对象的 Python 子类还没有布局支持" 的统一错误（**如实报未接线** ✓）。
pub(crate) fn wrapper_layout_unsupported(
    instance: &Instance,
    wrapper: NonNull<Header>,
    base_name: &str,
) -> crate::ExecError {
    let actual = instance.type_name(instance.type_of(wrapper));
    instance.raise_builtin_error(
        "TypeError",
        &format!(
            "'{actual}' 是 '{base_name}' 的 Python 子类，本层还没有它的布局支持 \
             （{base_name} 的载荷不可用）"
        ),
    )
}

pub(crate) fn descriptor_get_target(
    instance: &Instance,
    args: &[NonNull<Header>],
) -> Result<(Option<NonNull<Header>>, Option<NonNull<Header>>), crate::ExecError> {
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__get__ expected at least 1 argument, got 0",
        ));
    }
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("__get__ expected at most 2 arguments, got {}", args.len()),
        ));
    }
    let none = instance.singletons().none();
    let obj = args.first().copied().filter(|value| *value != none);
    let owner = args.get(1).copied().filter(|value| *value != none);
    if obj.is_none() && owner.is_none() {
        return Err(instance.raise_builtin_error("TypeError", "__get__(None, None) is invalid"));
    }
    Ok((obj, owner))
}

/// **`staticmethod.__get__(obj, owner=None)`** ✓（`P3-17`）：交回**被包的函数本身** ✓
/// （类访问与实例访问一样 ✓；参照实测 `S.__dict__['s'].__get__(None, S)` 就是那个函数 ✓）。
pub unsafe fn staticmethod_descriptor_get(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(wrapper) = bound else {
        // 类级取用（`staticmethod.__get__(wrapper, obj, owner)`）本层没接线 ⇒ 如实报缺 self。
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__get__' for 'staticmethod' objects needs an argument",
        ));
    };
    // 实参校验照参照（含 `__get__(None, None) is invalid` ✓）。
    let _ = descriptor_get_target(instance, args)?;
    // **布局核对**（Python 子类实例可能是通用布局 ⇒ 不能当 `StaticMethodObject` 读 ✓）。
    if !builtin_wrapper_layout_ok(
        instance,
        wrapper,
        "staticmethod",
        core::mem::size_of::<StaticMethodObject>(),
    ) {
        return Err(wrapper_layout_unsupported(instance, wrapper, "staticmethod"));
    }
    // SAFETY: 布局核对过（类型同名或实例尺寸一致）⇒ 可以按 `StaticMethodObject` 读载荷。
    let inner = unsafe { &*wrapper.as_ptr().cast::<StaticMethodObject>() }.function();
    Ok(instance.retain(inner))
}

/// **`classmethod.__get__(obj, owner=None)`** ✓（`P3-17`）：交回**绑到类上的方法** ✓。
///
/// 参照实测：`obj is None` ⇒ 绑到 `owner` ✓；`obj` 是实例 ⇒ 绑到 `owner`（缺省则 `type(obj)` ✓）；
/// `owner` 不要求是类型（`classmethod.__get__(inst, 42)` 就给 `42` 绑上 ✓）。
/// 这条是 `Lib/abc.py` 的 `class abstractclassmethod(classmethod)` 之所以能工作的基础 ✓。
pub unsafe fn classmethod_descriptor_get(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(wrapper) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__get__' for 'classmethod' objects needs an argument",
        ));
    };
    let (obj, owner) = descriptor_get_target(instance, args)?;
    // **布局核对**（同 `staticmethod.__get__` ✓）：子类实例不该被当 `ClassMethodObject` 读 ✓。
    if !builtin_wrapper_layout_ok(
        instance,
        wrapper,
        "classmethod",
        core::mem::size_of::<ClassMethodObject>(),
    ) {
        return Err(wrapper_layout_unsupported(instance, wrapper, "classmethod"));
    }
    // `owner` 缺省时：`obj is None` 那条已被 `descriptor_get_target` 挡掉 ⇒ 这里 obj 一定在 ✓。
    let this = match owner {
        Some(owner) => owner,
        None => instance
            .type_of(obj.expect("上面保证 obj 存在"))
            .cast::<Header>(),
    };
    // SAFETY: 绑定契约保证 `wrapper` 是存活的 classmethod 对象。
    let inner = unsafe { &*wrapper.as_ptr().cast::<ClassMethodObject>() }.function();
    // 绑定方法对象要各持一份引用（`OM-16`）。
    instance.retain(inner);
    instance.retain(this);
    Ok(instance
        .alloc(MethodObject::new(
            instance.type_named("method").expect("method 已登记"),
            inner,
            this,
        ))
        .into_raw()
        .cast::<Header>())
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

/// **原生可调用对象的名字类属性**（第 406 轮 ✓）：`fractions.py` 在**导入期**读
/// `fallback_operator.__name__`（`fallback_operator` 是 `operator.add` 这种**内建函数** ✓）
/// 来拼 `forward.__name__ = '__' + … + '__'` ✓ —— 先前 `builtin_function_or_method` 类型
/// **没有 getattr 槽** ✗ ⇒ 一律 `AttributeError` ✓。
///
/// 面：`__name__`／`__qualname__`（都回名字 ✓）、`__doc__`（**如实给空串** ✗：本层原生
/// 函数没有逐函数文档串；参照给的是各函数的 C 文档 ✓）。
pub unsafe fn builtin_function_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<BuiltinFunctionObject>() };
    let text = match name {
        "__name__" | "__qualname__" => object.name(),
        "__doc__" => "",
        _ => return None,
    };
    Some(instance.new_str(text))
}

/// 原生可调用对象的 `repr`：`<built-in function len>`（实测）。
pub unsafe fn builtin_function_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<BuiltinFunctionObject>() };
    Ok(format!("<built-in function {}>", object.name()))
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
    // **特殊查找**（`_PyObject_LookupSpecial` ✓，第 578 轮）：参照取 `__del__` **只扫 `type(self)` 的 MRO** ✓
    //（不碰实例字典 ✗、不走 `__getattribute__`/'__getattr__' ✗）。MRO 里没有 ⇒ 参照**根本不调** ✗ ⇒
    // 这里直接返回 ✓ —— 同时掐掉 `super` 对象那条绕行 ✓：旧写法走完整 `getattr` ✗ ⇒ 对 `super` 会绕进
    // `super_lookup` ✓ 读它自己正在释放的属性字典 ✓（第 578 轮实测：**每个** `super()` 临时对象都走这条 ✓，
    // 崩溃的读者正是它 ✓）。
    if instance
        .type_lookup(instance.type_of(object), "__del__")
        .is_none()
    {
        return;
    }
    match crate::executor::call::call_object_method(instance, object, "__del__", &[]) {
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

py_object! {
    /// **`classmethod`**（第 158 轮）：包一个可调用对象 ✓（**本对象持有一份引用**）。
    ///
    /// **已接线**：类型对象本身 ＋ `classmethod(f)` 构造 ✓ —— 这样 `Lib/abc.py:28` 的
    /// `class abstractclassmethod(classmethod):` 就能过 ✓（它需要一个**类型**做基类 ✓）。
    /// **未接线** ✗：描述符协议（`__get__` 绑定 `cls` ✓）⇒ 包好的方法还**不能真正绑定** ✓（如实登记 ✓）。
    pub struct ClassMethodObject {
        /// 被包起来的可调用对象（**本对象持有一份引用**）。
        function: NonNull<Header>,
    }
}

/// **`classmethod`／`staticmethod` 的属性面**（第 346 轮）：`__func__` 与 `__wrapped__` ✓。
///
/// 实测原形 ✓：上限榜上 `AttributeError: 'classmethod' object has no attribute '__func__'` × **39** 个模块
/// （`Lib/` 里到处是 `cls.__func__`／`f.__func__` 的用法 ✓ —— 例如 `abc.py`／`functools.py` 一带 ✓）。
pub unsafe fn classmethod_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ClassMethodObject>() };
    match name {
        "__func__" | "__wrapped__" => Some(instance.retain(object.function())),
        _ => None,
    }
}


/// **绑定 `method` 的属性面**（第 259 轮 ✓）：`__self__` ⇒ 绑定的实例 ✓、
/// `__func__`／`__wrapped__` ⇒ 函数 ✓ —— 模板是第 346 轮给 `classmethod`／`staticmethod` 补的那一对 ✓
/// （那一轮是 39 个模块的卡点 ✓；绑定方法的这两格此前**完全没有** ✗ ⇒ 实测
/// `m.__self__` 报 `AttributeError: 'method' object has no attribute '__self__'` ✗）。
pub unsafe fn method_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    match name {
        "__self__" => Some(instance.retain(object.this())),
        "__func__" | "__wrapped__" => Some(instance.retain(object.function())),
        // **函数面代理**（第 276 轮 ✓）：参照里绑定方法把 `__qualname__`／`__name__` 等转发给被绑函数 ✓
        //（实测 `d.__setitem__.__qualname__ == 'D.__setitem__'` ✓）。先前没有这几格 ✗ ⇒ AttributeError ✗。
        // 取法：先查**函数类型字典**里的现成条目 ✓；没有再看它是不是 Python 函数 ✓（是则用 `code()`
        // 合成 `__name__`／`__qualname__` ✓ —— 与 `method_repr`（第 269 轮 ✓）同一口径 ✓）。
        "__qualname__" | "__name__" | "__doc__" => {
            let function = object.function();
            // SAFETY: function 由方法对象持有。
            let function_type = unsafe { function.as_ref() }.ty();
            if Some(function_type) == instance.type_named("function") {
                // **`P3-17` 之后函数类型字典里也有了这几格（数据描述符对象 ✓）** ⇒ 不能像先前那样
                // `type_lookup` 直接交出去（会把**描述符对象**当值交出去 ✗）⇒ 走 `function_getattr`
                // 的**值**面（覆盖字段优先 ✓ —— 与 `f.__name__` 同一处真相 ✓）。
                // SAFETY: 刚确认是 Python 函数。
                return unsafe {
                    crate::builtin::function::function_getattr(function.as_ptr(), name, instance)
                };
            }
            // 非 Python 函数（原生绑定方法一类）：退回类型字典里的现成条目 ✓。
            instance
                .type_lookup(function_type, name)
                .map(|found| instance.retain(found))
        }
        "__module__" | "__annotations__" | "__defaults__" | "__kwdefaults__" | "__code__"
        | "__globals__" | "__dict__" | "__type_params__" => {
            let function = object.function();
            // SAFETY: function 由方法对象持有。
            let function_type = unsafe { function.as_ref() }.ty();
            if Some(function_type) == instance.type_named("function") {
                // SAFETY: 刚确认是 Python 函数。
                let from_getattr = unsafe {
                    crate::builtin::function::function_getattr(function.as_ptr(), name, instance)
                };
                if from_getattr.is_some() {
                    return from_getattr;
                }
            }
            instance.type_lookup(function_type, name).map(|found| instance.retain(found))
        }
        _ => None,
    }
}

/// **`staticmethod` 的属性面**（第 346 轮）：`__func__`／`__wrapped__` ✓（与 `classmethod` 同形 ✓）。
pub unsafe fn staticmethod_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象（`staticmethod` 与 `classmethod` 同载荷 ✓）。
    let object = unsafe { &*ptr.cast::<ClassMethodObject>() };
    match name {
        "__func__" | "__wrapped__" => Some(instance.retain(object.function())),
        _ => None,
    }
}

impl ClassMethodObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(classmethod_traverse)
            .with_clear(classmethod_clear)
    }

    /// 被包起来的对象（**借用**）。
    pub fn function(&self) -> NonNull<Header> {
        self.function
    }
}

py_object! {
    /// **`_weakref.ref`**（第 173 轮）：给 `Lib/_weakrefset.py` 用的**最小面子** ✓。
    ///
    /// **如实登记的偏差** ✗：本层**没有真正的弱引用**（GC 不支持 ✓）⇒ 这里存的是**强引用** ✓
    /// ⇒ 目标永远不会被回收 ✓（`WeakSet` 因而**不会自动清理** ✓）。对"能把 `abc.py`／`os.py` 跑起来"
    /// 这一步够用 ✓；真正的弱语义留待专门一轮 ✓。
    pub struct WeakRefObject {
        /// 目标对象（**本对象持有一份引用**）。
        target: NonNull<Header>,
    }
}

impl WeakRefObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(weakref_traverse)
            .with_clear(weakref_clear)
            .with_call(weakref_call)
    }

    /// 目标（**借用**）。
    pub fn target(&self) -> NonNull<Header> {
        self.target
    }
}

// **手写** ✓（第 158／160／161 轮的教训 ✓：机械改名会留下错误强转 ✗，而 GC 静态检查**只查结构** ✓）。
pub(crate) unsafe fn weakref_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<WeakRefObject>() };
    visit(object.target().as_ptr());
}

pub(crate) unsafe fn weakref_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<WeakRefObject>() };
    // SAFETY: 这一份引用由本对象持有。
    unsafe { instance.release_object(object.target().as_ptr()) };
}

/// **调用 `ref(x)`** ✓：给回目标本身 ✓（强引用 ⇒ 一定还在 ✓；真正弱语义随后补 ✗）。
pub(crate) unsafe fn weakref_call(
    ptr: *mut Header,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<WeakRefObject>() };
    Ok(instance.retain(object.target()))
}

/// 造一个 `ref`（`_weakref` 模组的 `ref` ✓）：接 1 或 2 个实参 ✓（回调**忽略** ✗，已登记 ✓）。
/// **调用元类型**（第 183 轮重放）：`type(x)` ⇒ 取类型 ✓；无参 ⇒ 参照原话报错 ✓。
///
/// **重要** ✓：调用**一个类**时（`C(...)` ✓），`bound` 是那个**类对象** ✓ ⇒ 这时**不能**走这里 ✗，
/// 得交回**正常的实例化路径** ✓（元类型一旦挂了 call 槽，就会**接管**所有"调用类"的场合 ✓）。
/// `dict.fromkeys(iterable, value=None)`（第 184 轮：**真实实现** ✓，替掉第 182 轮的占位 ✗）。
///
/// 支持的**可迭代对象**：`list`／`tuple`／`set`／`frozenset`／`dict`（取键 ✓）。
/// **尚未接线** ✗：字符串（要字符对象 ✓）、生成器／迭代器（要走迭代协议 ✓）⇒ 如实报未接线 ✓。
// **`safe fn`** ✓（第 184 轮：stdlib 有 `#![forbid(unsafe_code)]` ✗ ⇒ 跨 crate 的面必须是安全的 ✓；
// 它自己的内部照旧用 `unsafe {}` 分块 ✓）。


pub unsafe fn type_call(
    _ptr: *mut Header,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    if bound.is_some() {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "type_call：绑定形态（调用类）应交回实例化路径 —— 本槽不该被走到",
        });
    }
    match args.len() {
        1 => Ok(instance.retain(instance.type_of(args[0]).cast())),
        0 => Err(instance.raise_builtin_error(
            "TypeError",
            "cannot create 'type' instances",
        )),
        // **三参形态**（`type(name, bases, ns)` ✓，第 512 轮接线 ✗）：直接交给 `type_new_native` ✓
        // （类创建那一处真相 ✓；`__prepare__`／元类／`build_class_from_parts` 都不重复实现 ✓）。
        // 队列依据：`tools/next_work.py` 显示 `functools`／`contextlib`／`_threading_local`／`threading`
        // 都被这条"随后补"挡住 ✓。
        3 => crate::builtin_objects::type_new_native(instance, None, args, &[]),
        _ => Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "type(...)：只接 1 参（取类型）与 3 参（建类）",
        }),
    }
}

/// 造一个 `ref`（`_weakref` 模组的 `ref` ✓）：接 1 或 2 个实参 ✓（回调**忽略** ✗，已登记 ✓）。
///
/// **`class` 用调用方请求的类** ✓（task-7）：`class KeyedRef(weakref.ref)` 的 `__new__` 要造出
/// **子类**实例 ✓（载荷是同一个 `WeakRefObject` ✓ —— `classes.rs` 的"内建布局族"把 `weakref`
/// 纳入了 ⇒ 子类沿用同一布局 ✓）；先前**硬编码**成 `weakref` 类型 ✗ ⇒ 子类实例的 `type()`
/// 与 `isinstance()` 都会错 ✗。
pub unsafe fn weakref_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(target) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "ref expected at least 1 argument, got 0",
        ));
    };
    instance.retain(*target);
    // SAFETY: `class` 由注册表持有；`target` 的那份引用由新对象接手（上面刚 retain ✓）。
    Ok(instance
        .alloc_payload(WeakRefObject::new(class, *target))
        .cast::<Header>())
}

/// **`weakref.__new__(cls, ob, callback=None)`**（task-7）：`ref.__new__` 在参照里是 weakref 类型
/// **自己的** `tp_new` ✓ —— Python 子类（`class KeyedRef(weakref.ref)`）靠它拿到**请求的子类**
/// 与目标（`weakref.ref.__new__(type, ob, callback)` ✓）。
///
/// 派发形态与 [`data_new_native`](crate::builtin_objects::constructors::data_new_native) 一致 ✓：
/// 属性通道可能把**拥有者类型**放进 `bound` ✓，构造分派的 `__new__` 覆写支则把 `cls` 放进
/// `args[0]` ✓ —— 两种都要认 ✓（纯 `weakref` 自己的实例化还会走 `new` 槽 ✓，本函数只负责
/// "在类型对象上取到 `__new__` 再调用"这条 ✓）。
pub unsafe fn weakref_dunder_new(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (class_header, rest) = match (bound, args.split_first()) {
        (Some(this), Some((first, _remaining)))
            if instance.is_type_object(this) && !instance.is_type_object(*first) =>
        {
            (this, args)
        }
        (Some(this), None) if instance.is_type_object(this) => (this, &[][..]),
        (_, Some((first, remaining))) => (*first, remaining),
        (Some(this), None) => (this, &[][..]),
        (None, None) => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "ref.__new__(): not enough arguments",
            ))
        }
    };
    let Some(class) = instance.as_type(class_header) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "ref.__new__(X): X is not a type object",
        ));
    };
    // SAFETY: 槽位/实参都是借用视图，契约同 [`weakref_new`]。
    unsafe { weakref_new(class, rest, instance) }
}

py_object! {
    /// **`property`**（第 161 轮）：与 `classmethod`／`staticmethod` 同一模式 ✓。
    ///
    /// **已接线**：类型对象 ＋ `property(fget)` 构造 ✓（`Lib/abc.py` 的 `class abstractproperty(property)` 要它 ✓）。
    /// **未接线** ✗：`fset`／`fdel`／`doc`（本层只存 `fget` ✓）与**描述符协议**（`__get__` ✓）⇒ 真正当装饰器用还不行 ✓。
    pub struct PropertyObject {
        /// `fget`（**本对象持有一份引用**）。
        fget: NonNull<Header>,
        /// `fset`（第 186 轮：`setter` 要它 ✓；没有就是 `None` 单例 ✓）。
        fset: NonNull<Header>,
        /// `fdel`（同上 ✓）。
        fdel: NonNull<Header>,
    }
}

impl PropertyObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(property_traverse)
            .with_clear(property_clear)
    }

    /// `fget`（**借用**）。
    pub fn fget(&self) -> NonNull<Header> {
        self.fget
    }

    /// `fset`（**借用**）。
    pub fn fset(&self) -> NonNull<Header> {
        self.fset
    }

    /// `fdel`（**借用**）。
    pub fn fdel(&self) -> NonNull<Header> {
        self.fdel
    }
}

// **手写** ✓（第 158／160／161 轮的教训 ✓：机械改名会留下错误强转 ✗，GC 静态检查只查结构 ✓ 查不出 ✓）。

/// `property.getter`／`setter`／`deleter` 的**目标**（第 186 轮）：各返回**新** property ✓。
#[derive(Clone, Copy)]
pub(crate) enum PropertySlot {
    Getter,
    Setter,
    Deleter,
}

/// 取绑定的 property（方法契约保证有 ✓）。
pub(crate) fn bound_property(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, crate::ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}

py_object! {
    /// **`staticmethod`**（第 161 轮）：与 `classmethod` 同一模式 ✓（包一个可调用对象 ✓）。
    ///
    /// **已接线**：类型对象 ＋ `staticmethod(f)` 构造 ✓（`Lib/abc.py` 的 `class abstractstaticmethod(staticmethod)` 要它 ✓）。
    /// **未接线** ✗：描述符协议（`__get__` ✓）⇒ 包好的函数还**不能真正绑定** ✓（如实登记 ✓）。
    pub struct StaticMethodObject {
        /// 被包起来的可调用对象（**本对象持有一份引用**）。
        function: NonNull<Header>,
    }
}

impl StaticMethodObject {
    /// 见 [`TupleObject::slots`]。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_traverse(staticmethod_traverse)
            .with_clear(staticmethod_clear)
    }

    /// 被包起来的对象（**借用**）。
    pub fn function(&self) -> NonNull<Header> {
        self.function
    }
}

// **手写** ✓（第 158／160／161 轮的教训 ✓：机械改名会留下错误强转 ✗，而 GC 静态检查**只查结构** ✓ 查不出 ✓）。
pub(crate) unsafe fn staticmethod_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StaticMethodObject>() };
    visit(object.function().as_ptr());
}

pub(crate) unsafe fn staticmethod_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<StaticMethodObject>() };
    // SAFETY: 这一份引用由本对象持有。
    unsafe { instance.release_object(object.function().as_ptr()) };
}

pub unsafe fn staticmethod_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **无参也合法** ✓（第 186 轮实测：参照的 `staticmethod()` 默认 `f=None` ✓ ——
    // `Lib/importlib/_bootstrap.py` 正是这么用的 ✓）。
    let function = match args.first() {
        Some(given) => *given,
        None => instance.singletons().none(),
    };
    instance.retain(function);
    // **用调用方给的那个类** ✓（`P3-17` 修 ✗）：`class X(staticmethod)` 的子类实例必须带着
    // **自己的类型**（参照 `type(X(f)) is X` ✓）；先前这里硬取 `type_named("staticmethod")` ✗
    // ⇒ 子类实例会报成基类类型 ✗（布局已由 `classes.rs` 的 AB-58 表沿用 ✓）。
    let ty = class;
    // **走宏生成的 `new`** ✓（它接收字段作参数 ✓）：这样既符合规范 ✓，也消掉「never used」警告 ✓
    //（第 158 轮的教训 ✓：直接写字面量会绕过它 ✗）。
    Ok(instance
        .alloc_payload(StaticMethodObject::new(ty, function))
        .cast::<Header>())
}

pub(crate) unsafe fn classmethod_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    // **一律手写** ✓（第 158／160 轮的教训 ✓：机械改名会留下错误强转 ✗，GC 静态检查查不出 ✓）。
    let object = unsafe { &*ptr.cast::<ClassMethodObject>() };
    visit(object.function().as_ptr());
}

pub(crate) unsafe fn classmethod_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ClassMethodObject>() };
    // SAFETY: 这一份引用由本对象持有。
    unsafe { instance.release_object(object.function().as_ptr()) };
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
pub(crate) unsafe fn method_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    visit(object.function().as_ptr());
    visit(object.this().as_ptr());
}

/// `OM-40`／`OM-20` ②：交出两份引用。
pub(crate) unsafe fn method_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<MethodObject>() };
    // SAFETY: 这两份引用由本对象持有。
    unsafe {
        instance.release_object(object.function().as_ptr());
        instance.release_object(object.this().as_ptr());
    }
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
        // **探针**（第 189 轮，`PYAWA_SETATTR_DEBUG=1`）：谁把"非普通 dict"（如类命名空间 `EnumDict`）
        // 放进某个对象的**内联属性字典** ✓ —— 盯**类型**（不盯地址 ✓，第 118 轮盯地址失败过 ✓）。
        if crate::diag::flag("PYAWA_SETATTR_DEBUG") {
            if let Some(mapping) = mapping {
                // SAFETY: mapping 由调用方保证存活。
                // SAFETY: mapping 由调用方保证存活；它的 ty 由注册表持有。
                let name = unsafe { (&*mapping.as_ptr()).ty().as_ref() }.name().to_owned();
                if name != "dict" {
                    eprintln!("[setattr] 属性字典 ← 类型={name}\n{}", std::backtrace::Backtrace::force_capture());
                }
            }
        }
        if crate::diag::flag("PYAWA_SETDICT_DEBUG") {
            if let Some(mapping) = mapping {
                eprintln!("[setdict] 内联属性字典 ← {:p}", mapping.as_ptr());
            }
        }
        core::mem::replace(&mut *self.attributes.borrow_mut(), mapping)
    }
}


/// **`new` 槽的 `__new__` 桥接**（第 607 轮探针版 ✓）：只给 `super_lookup` 用 ✓，不进类型字典 ✗。
pub(crate) fn new_slot_bridge_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(owner) = bound.filter(|this| instance.is_type_object(*this)) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__new__' of type object needs an argument",
        ));
    };
    let owner_type = owner.cast::<crate::TypeObject>();
    let (target, rest) = match args.first().copied() {
        Some(first) if instance.is_type_object(first) => {
            (first.cast::<crate::TypeObject>(), &args[1..])
        }
        _ => (owner_type, args),
    };
    // SAFETY: owner 是存活类对象，由注册表持有。
    let Some(slot) = (unsafe { owner_type.as_ref() }).slots().new else {
        return Err(instance.raise_builtin_error("TypeError", "cannot create instances"));
    };
    // SAFETY: 槽位契约见 `NewFn`。
    unsafe { slot(target, rest, instance) }
}

pub(crate) static NEW_BRIDGE_FUNCTION: std::sync::Mutex<Option<usize>> = std::sync::Mutex::new(None);

pub(crate) fn new_bridge_function(instance: &Instance) -> Option<NonNull<Header>> {
    if let Ok(guard) = NEW_BRIDGE_FUNCTION.lock() {
        if let Some(pointer) = *guard {
            return NonNull::new(pointer as *mut Header);
        }
    }
    let function_type = instance.type_named("builtin_function_or_method")?;
    let object = instance
        .alloc(BuiltinFunctionObject::new(
            function_type,
            "__new__",
            core::cell::Cell::new(new_slot_bridge_native),
        ))
        .into_raw()
        .cast::<Header>();
    if let Ok(mut guard) = NEW_BRIDGE_FUNCTION.lock() {
        *guard = Some(object.as_ptr() as usize);
    }
    Some(object)
}

/// `OM-40`：列出属性字典。
pub(crate) unsafe fn attribute_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<AttributeObject>() };
    if let Some(mapping) = object.attributes() {
        visit(mapping.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出属性字典。
pub(crate) unsafe fn attribute_clear(ptr: *mut Header, instance: &Instance) {
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

    /// 闭包（**借用**的 cell 列表；建帧时装进自由槽）。
    pub fn closure(&self) -> Vec<NonNull<Header>> {
        self.closure.borrow().clone()
    }

    /// 设置闭包（**新引用**，由本对象接手；返回旧的，调用方负责释放）。
    pub fn set_closure(&self, items: Vec<NonNull<Header>>) -> Vec<NonNull<Header>> {
        core::mem::replace(&mut *self.closure.borrow_mut(), items)
    }

    /// 仅关键字参数默认值（**借用**的 `dict`）。
    pub fn kwdefaults(&self) -> Option<NonNull<Header>> {
        self.kwdefaults
    }

    /// 设置仅关键字默认值（**新引用**，由本对象接手；返回被顶下来的旧值）。
    pub fn set_kwdefaults(&mut self, kwdefaults: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        core::mem::replace(&mut self.kwdefaults, kwdefaults)
    }

    /// **`__globals__`**（**借用**；`MAKE_FUNCTION` 之后就有）。
    pub fn globals(&self) -> Option<NonNull<Header>> {
        *self.globals.borrow()
    }

    /// 设置 `__globals__`（**新引用**，由本对象接手；返回被顶下来的旧值）。
    /// `__annotate__`（**借用**；没有就是 `None`）。
    pub fn annotate(&self) -> Option<NonNull<Header>> {
        *self.annotate.borrow()
    }

    /// 换 `__annotate__`，返回旧值（**调用方负责归还**）。
    pub fn set_annotate(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.annotate.replace(value)
    }

    /// `__annotations__` 的缓存（**借用**；没算过就是 `None`）。
    pub fn annotations_cache(&self) -> Option<NonNull<Header>> {
        *self.annotations_cache.borrow()
    }

    /// 换 `__annotations__` 缓存，返回旧值（**调用方负责归还**）。
    pub fn set_annotations_cache(
        &self,
        value: Option<NonNull<Header>>,
    ) -> Option<NonNull<Header>> {
        self.annotations_cache.replace(value)
    }

    pub fn set_globals(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.globals.replace(value)
    }

    /// **`__name__` 的覆盖值**（**借用**；`None` ⇒ 用 code object 的名字 ✓）。
    ///
    /// 四个覆盖值都由 `function.__dict__` 里的 `function_attribute_descriptor` 的
    /// `__set__`／`__delete__` 写（**数据描述符**语义 ✓，与参照的 getset／member descriptor 同形 ✓）。
    pub fn name_override(&self) -> Option<NonNull<Header>> {
        *self.name_override.borrow()
    }

    /// 换 `__name__` 覆盖值，返回旧值（**调用方负责归还**）。
    pub fn set_name_override(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.name_override.replace(value)
    }

    /// **`__qualname__` 的覆盖值**（**借用**；`None` ⇒ code object 的 qualname ✓）。
    pub fn qualname_override(&self) -> Option<NonNull<Header>> {
        *self.qualname_override.borrow()
    }

    /// 换 `__qualname__` 覆盖值，返回旧值（**调用方负责归还**）。
    pub fn set_qualname_override(
        &self,
        value: Option<NonNull<Header>>,
    ) -> Option<NonNull<Header>> {
        self.qualname_override.replace(value)
    }

    /// **`__doc__` 的覆盖值**（**借用**；`None` ⇒ code object 的文档串 ✓）。
    pub fn doc_override(&self) -> Option<NonNull<Header>> {
        *self.doc_override.borrow()
    }

    /// 换 `__doc__` 覆盖值，返回旧值（**调用方负责归还**）。
    pub fn set_doc_override(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.doc_override.replace(value)
    }

    /// **`__module__` 的覆盖值**（**借用**；`None` ⇒ 从 `__globals__` 取 ✓）。
    pub fn module_override(&self) -> Option<NonNull<Header>> {
        *self.module_override.borrow()
    }

    /// 换 `__module__` 覆盖值，返回旧值（**调用方负责归还**）。
    pub fn set_module_override(&self, value: Option<NonNull<Header>>) -> Option<NonNull<Header>> {
        self.module_override.replace(value)
    }
}

py_object! {
    /// **内部哨兵**：`CALL` 的"没有 self"槽位（参照实现在栈上放 `NULL` 指针）。
    ///
    /// *内部*：它**不**进 `TS-41` 的内建类型表，也**禁止**暴露给 Python 代码——
    /// Python 侧看到的永远是 `None`。
    pub struct NullObject {}
}
