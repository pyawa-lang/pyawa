//! **`function` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `function_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：（无） ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::HotId;
use crate::builtin_objects::{FunctionObject, TupleObject};
use crate::executor::ExecError;
use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;


/// **`function.__code__` 的访问器形态** ✓（第 214 轮）：在**类型**上取得它 ✓
/// （`FunctionType.__code__` ✓ —— `Lib/types.py` 要 `type(...)` ✓），
/// 也支持绑定／非绑定两种调用 ✓（`f.__code__` ✓、`F.__code__(f)` ✓）。
pub fn function_code_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let target = bound.or_else(|| args.first().copied()).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "descriptor '__code__' needs an argument")
    })?;
    if instance.type_name(instance.type_of(target)) != "function" {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__code__' for 'function' objects doesn't apply to a different type",
        ));
    }
    // SAFETY: 类型身份刚确认。
    let function = unsafe { &*target.as_ptr().cast::<FunctionObject>() };
    Ok(instance.retain(function.code()))
}

/// **`function.__get__`**（第 410 轮，task-21 ✓）：函数的**描述符**入口 ✓。
///
/// 为什么必须要它 ✓：`enum.py` 的 `_is_descriptor(obj)` 判据是
/// `hasattr(obj, '__get__') or hasattr(obj, '__set__') or hasattr(obj, '__delete__')` ✓ ——
/// 类体里的**普通函数**（如 `Flag._get_value` ✓）若没有 `__get__` 就会被 `EnumDict.__setitem__`
/// 当成**枚举成员** ✗（实测 `Flag._member_names_ == ['_get_value']` ⇒ 随后创建 `IntFlag` 时
/// `_check_for_existing_members_` 报 `TypeError: <enum 'IntFlag'> cannot extend <flag 'Flag'>` ✗）。
///
/// 语义照参照：`f.__get__(None, owner)` ⇒ **f 本身** ✓；`f.__get__(obj, owner)` ⇒ 绑定方法 ✓。
/// **也接未绑定形态** `FunctionType.__get__(f, obj, owner)` ✓（`type(f).__get__` 取到的是
/// **未绑定**的原生方法；`Lib/inspect.py` 的 `_descriptor_get` 正是
/// `get = getattr(type(descriptor), '__get__'); get(descriptor, obj, type(obj))` ✓ ——
/// `getattr_static` 修好后 `inspect.signature(用户类)` 第一次走到这条 ✓ ⇒ 必须支持 ✓）。
pub fn function_descriptor_get(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // **绑定形态**（`f.__get__(…)`：`bound` 是 f）与**未绑定形态**（`FunctionType.__get__(f, …)`：
    // 第一个实参是 f）都要认 ✓。
    let (function, rest) = match bound {
        Some(function) => (function, args),
        None => match args.split_first() {
            Some((first, remaining)) => (*first, remaining),
            None => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "descriptor '__get__' of 'function' object needs an argument",
                ))
            }
        },
    };
    // SAFETY: 槽位契约保证 function 是存活对象。
    if instance.type_name(instance.type_of(function)) != "function" {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__get__' for 'function' objects doesn't apply to a different type",
        ));
    }
    let Some(this) = rest.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "function.__get__() takes 2 arguments (1 given)",
        ));
    };
    // `f.__get__(None, owner)` ⇒ 未绑定（就是 f 本身 ✓）
    if Some(instance.type_of(this)) == instance.hot_type(HotId::NoneType) {
        // 调用方要一份新引用
        // SAFETY: function 由调用方借用，存活。
        unsafe { instance.incref_object(function.as_ptr()) };
        return Ok(function);
    }
    // 其余 ⇒ 绑成方法 ✓（与属性通道给的那种对象同形 ✓）
    let method_type = instance.hot_type(HotId::Method)
        .expect("method 在引导期已登记");
    // SAFETY: function（借用）与 this（实参）都存活；方法对象要各持一份。
    unsafe {
        instance.incref_object(function.as_ptr());
        instance.incref_object(this.as_ptr());
    }
    Ok(instance
        .alloc(crate::MethodObject::new(method_type, function, this))
        .into_raw()
        .cast::<Header>())
}

/// `f.__call__(*args, **kwargs)`（round 97）：Python 函数的 `__call__` 方法-wrapper ✓。
///
/// 动因：参照里每个函数都有 `__call__` ✓；`Lib/unittest/mock.py` 的 `_callable(obj)` 判据是
/// `getattr(obj, '__call__', None) is not None` ✗ ⇒ 我们的函数没有这个属性 ⇒ 被当成**不可调用**
/// ⇒ `side_effect` 走 `next(effect)` ⇒ `TypeError/未实现`（官方 `test_tempfile` 的
/// `test_unexpected_error` 直接状态 5 中止 ✗；`test_mock` 一族同理 ✓）。
/// 实现：绑定一个原生转发器，内部按**转移语义**把实参交给 `call_callable` ✓。
pub fn function_call_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "__call__() 缺少 self"));
    };
    let mut owned_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活；`call_callable` 是转移语义 ⇒ 这里各留一份。
        unsafe { instance.incref_object(argument.as_ptr()) };
        owned_args.push(*argument);
    }
    let mut owned_kwargs: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::with_capacity(kwargs.len());
    for (key, value) in kwargs {
        // SAFETY: 同上。
        unsafe {
            instance.incref_object(key.as_ptr());
            instance.incref_object(value.as_ptr());
        }
        owned_kwargs.push((*key, *value));
    }
    crate::executor::call::call_callable(instance, this, None, owned_args, owned_kwargs, 0)
}

/// 把一个原生转发器**绑**到 `target` 上交出去（`method` 对象；各持一份引用 ✓）。
fn bind_native_to(
    instance: &Instance,
    name: &'static str,
    handler: crate::NativeFn,
    target: NonNull<Header>,
) -> NonNull<Header> {
    let method_type = instance.hot_type(HotId::BuiltinFunctionOrMethod)
        .expect("builtin_function_or_method 在引导期已登记");
    let native = instance.alloc(crate::BuiltinFunctionObject::new(
        method_type,
        name,
        core::cell::Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self。
    unsafe { instance.incref_object(target.as_ptr()) };
    instance
        .alloc(crate::MethodObject::new(
            instance.hot_type(HotId::Method).expect("method 已登记"),
            native_raw,
            target,
        ))
        .into_raw()
        .cast::<Header>()
}

/// **`function.__globals__` 的访问器形态** ✓（同 `__code__` ✓）。`__globals__` 可能为空 ⇒ 给 `None` ✓。
pub fn function_globals_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let target = bound.or_else(|| args.first().copied()).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "descriptor '__globals__' needs an argument")
    })?;
    if instance.type_name(instance.type_of(target)) != "function" {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__globals__' for 'function' objects doesn't apply to a different type",
        ));
    }
    // SAFETY: 类型身份刚确认。
    let function = unsafe { &*target.as_ptr().cast::<FunctionObject>() };
    Ok(match function.globals() {
        Some(value) => instance.retain(value),
        None => instance.new_none(),
    })
}

/// `§10` 生成器／协程族的**方法**：`send`／`throw`／`close`（生成器还有 `__next__`）。
///
/// 槽位交出的必须是**绑定方法对象**：`LOAD_ATTR` 在"取方法"形态下会给 `(值, NULL)` 两格
/// （见执行器的 `LOAD_ATTR`），所以已经绑好 self 的方法正好被 `CALL` 按"无 self"调用。
/// **`f.__annotations__`**（PEP 649 的惰性求值 ＋ **缓存**）。
///
/// 实测（3.14.4）：同一函数的 `f.__annotations__` 是**同一对象**，且 `__annotate__` 只被调用
/// **一次**（`format = 1`）；**没有注解**的函数（`__annotate__` 是 `None`）给 `{}`，也照样缓存。
///
/// ⚠ **未接**：参照里这个属性**可写**（赋值会换掉注解），本层是只读的计算属性。
pub fn function_annotations(
    instance: &Instance,
    ptr: *mut Header,
) -> Result<NonNull<Header>, crate::ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    if let Some(cached) = object.annotations_cache() {
        // SAFETY: 缓存由本对象持有，调用方要自己那份。
        unsafe { instance.incref_object(cached.as_ptr()) };
        return Ok(cached);
    }
    let computed = match object.annotate() {
        Some(callable) => {
            let format = instance.new_int(1);
            // `call_value` 只**借用**实参 ⇒ 用完归还
            let result = crate::executor::call::call_value(instance, callable, &[format], &[]);
            // SAFETY: format 是上面刚造的那份引用。
            unsafe { instance.release_object(format.as_ptr()) };
            result?
        }
        // 没有注解 ⇒ `{}`（实测 `plain.__annotations__ == {}`）
        None => instance.new_dict(),
    };
    // 缓存自己持一份
    // SAFETY: computed 是新引用，存活。
    unsafe { instance.incref_object(computed.as_ptr()) };
    if let Some(old) = object.set_annotations_cache(Some(computed)) {
        // SAFETY: old 是旧缓存持有的那份。
        unsafe { instance.release_object(old.as_ptr()) };
    }
    Ok(computed)
}

/// **函数对象**的属性通道（`OM-11` 的 `getattr` 槽）。
///
/// 暴露的名字与**语义**照参照实测（3.14.4）：
/// `__name__`／`__qualname__`／`__code__`／`__defaults__`（无默认值 ⇒ `None`）／
/// `__kwdefaults__`（同上）／`__globals__`／`__annotate__`（PEP 649；**无注解 ⇒ `None`**）。
///
/// **可写属性**（`__name__`／`__qualname__`／`__doc__`／`__module__`）：先看函数对象上的
/// **覆盖字段**（`functools.wraps` 的 `setattr` 就走这条 ✓；那四个字段由
/// `function.__dict__` 里的 `function_attribute_descriptor` 写 ✓ —— 见本文件末尾的
/// `function_attribute_descriptor_set`）；没覆盖才用 code object／`__globals__` 的默认值 ✓。
/// **注意**：这条槽在实例字典之前跑 ✓ ⇒ 覆盖值不会被 `f.__dict__` 里同名的项遮住 ✓
/// （与参照的**数据描述符优先于实例字典**同效 ✓）。
/// 返回值一律**新引用**（调用方按 `OM-16` 接手）。
pub unsafe fn function_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    match name {
        // **`__call__`**（round 97）：见 `function_call_native` 的注释 ✓。
        "__call__" => {
            let owner = unsafe { NonNull::new_unchecked(ptr) };
            return Some(bind_native_to(
                instance,
                "__call__",
                function_call_native as crate::NativeFn,
                owner,
            ));
        }
        "__name__" => {
            if let Some(override_value) = object.name_override() {
                // SAFETY: 覆盖值由函数持有，调用方要自己那份。
                unsafe { instance.incref_object(override_value.as_ptr()) };
                return Some(override_value);
            }
            // **默认值也缓存** ✓（task-22）：同一函数的 `f.__name__` 必须是**同一对象** ✓
            // —— 官方 `test_decorators` 的 `assertIs(getattr(wrapper, …), getattr(func, …))` 靠它 ✓。
            if let Some(cached) = *object.name_cache.borrow() {
                // SAFETY: 缓存值由函数持有，调用方要自己那份。
                unsafe { instance.incref_object(cached.as_ptr()) };
                return Some(cached);
            }
            let code = object.code();
            // SAFETY: code 由函数持有，存活。
            let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
            let name = instance.new_str(code.name());
            // 缓存持一份、返回值是另一份 ✓（`new_str` 给的是新引用，各拿各的）。
            // SAFETY: name 是新引用，缓存接手一份。
            unsafe { instance.incref_object(name.as_ptr()) };
            *object.name_cache.borrow_mut() = Some(name);
            Some(name)
        }
        "__qualname__" => {
            if let Some(override_value) = object.qualname_override() {
                // SAFETY: 同上。
                unsafe { instance.incref_object(override_value.as_ptr()) };
                return Some(override_value);
            }
            // **默认值也缓存** ✓（同上）。
            if let Some(cached) = *object.qualname_cache.borrow() {
                // SAFETY: 缓存值由函数持有，调用方要自己那份。
                unsafe { instance.incref_object(cached.as_ptr()) };
                return Some(cached);
            }
            let code = object.code();
            // SAFETY: code 由函数持有，存活。
            let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
            let qualname = instance.new_str(code.qualname());
            // SAFETY: qualname 是新引用，缓存接手一份。
            unsafe { instance.incref_object(qualname.as_ptr()) };
            *object.qualname_cache.borrow_mut() = Some(qualname);
            Some(qualname)
        }
        "__doc__" => {
            // **覆盖值优先** ✓：`del f.__doc__` 之后参照给 `None`（不是 code 的文档串 ✓）。
            if let Some(override_value) = object.doc_override() {
                // SAFETY: 覆盖值由函数持有，调用方要自己那份。
                unsafe { instance.incref_object(override_value.as_ptr()) };
                return Some(override_value);
            }
            let code = object.code();
            // SAFETY: code 由函数持有，存活。
            let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
            // **实测**：只有 `co_flags` 的 `0x4000000`（"有文档串"）置位时，常量 0 才是文档串
            // ——`def f(): return "x"` 的常量 0 是 `'x'`，但 `f.__doc__` 是 `None`。
            if code.flags() & 0x400_0000 == 0 {
                return Some(instance.retain(instance.singletons().none()));
            }
            match code.constant(0) {
                Some(value) => {
                    // SAFETY: 值由 code 持有，调用方要自己那份。
                    unsafe { instance.incref_object(value.as_ptr()) };
                    Some(value)
                }
                None => Some(instance.retain(instance.singletons().none())),
            }
        }
        "__code__" => {
            // 新增一份（调用方接手）
            // SAFETY: code 由函数持有，存活。
            unsafe { instance.incref_object(object.code().as_ptr()) };
            Some(object.code())
        }
        "__closure__" => {
            // **闭包**（第 183 轮）：`Lib/types.py` 要 `f.__closure__[0]` ✓ —— `CellObject` 本来就有 ✓。
            // 没有闭包时参照给 `None` ✓；有闭包给**元组** ✓（元组持每个单元一份新引用 ✓）。
            let cells = object.closure();
            if cells.is_empty() {
                return Some(instance.retain(instance.singletons().none()));
            }
            let items: Vec<NonNull<Header>> = cells
                .iter()
                .map(|cell| {
                    // SAFETY: cell 由函数持有，存活；这里再取一份交给元组。
                    unsafe { instance.incref_object(cell.as_ptr()) };
                    *cell
                })
                .collect();
            Some(instance.new_tuple(items))
        }
        "__defaults__" => {
            let defaults = object.defaults();
            if defaults.is_empty() {
                return Some(instance.retain(instance.singletons().none()));
            }
            let mut items = Vec::with_capacity(defaults.len());
            for default in defaults {
                // SAFETY: 默认值由函数持有，存活。
                unsafe { instance.incref_object(default.as_ptr()) };
                items.push(*default);
            }
            Some(instance.new_tuple(items))
        }
        "__kwdefaults__" => match object.kwdefaults() {
            Some(mapping) => {
                // SAFETY: mapping 由函数持有，存活。
                unsafe { instance.incref_object(mapping.as_ptr()) };
                Some(mapping)
            }
            None => Some(instance.retain(instance.singletons().none())),
        },
        // **`__module__`**（第 312 轮）：参照在**定义时**把它写死成当时那个模块的 `__name__` ✓；
        // 本层从函数的 `__globals__` 里取同名的那一个 ✓ —— 对模块级函数与嵌套函数结果一致 ✓
        // （抓不到就退到 `builtins` ✓，与参照给内建函数的取值同形 ✓）。
        // 动因：上限榜上 `object has no attribute '__module__'` × **67** 个模块 ✓ ——
        // `Lib/_collections_abc.py` 一族用 `getattr(x, "__module__")` 探 typing 别名 ✓。
        "__module__" => {
            // **覆盖值优先** ✓（`functools.wraps` 的 `setattr(wrapper, "__module__", …)` ✓）。
            if let Some(override_value) = object.module_override() {
                // SAFETY: 覆盖值由函数持有，调用方要自己那份。
                unsafe { instance.incref_object(override_value.as_ptr()) };
                return Some(override_value);
            }
            // **默认值也缓存** ✓（task-22）：同一函数的 `f.__module__` 必须是**同一对象** ✓。
            if let Some(cached) = *object.module_cache.borrow() {
                // SAFETY: 缓存值由函数持有，调用方要自己那份。
                unsafe { instance.incref_object(cached.as_ptr()) };
                return Some(cached);
            }
            // 注意：`globals()` 给的是**命名空间字典本身** ⇒ 直接查 `__name__` ✓
            // （`module_text` 要的是**模块对象** ✗，第一版传错了 ⇒ 一律退回 `builtins` ✗）。
            let module = object
                .globals()
                .and_then(|globals| instance.dict_get(globals, "__name__"))
                .and_then(|value| instance.text_of(value).map(str::to_owned))
                .unwrap_or_else(|| "builtins".to_owned());
            let module = instance.new_str(&module);
            // SAFETY: module 是新引用，缓存接手一份。
            unsafe { instance.incref_object(module.as_ptr()) };
            *object.module_cache.borrow_mut() = Some(module);
            Some(module)
        }
        // **`__class__`**（同上）：函数对象的类型就是 `function` ✓（参照给的就是那个类 ✓）。
        "__class__" => {
            let ty = instance.type_of(unsafe { NonNull::new_unchecked(ptr) });
            Some(instance.retain(ty.cast()))
        }
        // **`function.__builtins__`**（round 216）：参照里它是**只读属性**，值取
        // `__globals__["__builtins__"]`（没有 ⇒ 空 dict 一类）；`namedtuple` 的 `__new__`
        // 模板就是 `__builtins__ == {}`（`test_collections.TestNamedTuple.test_new_builtins_issue_43102` ✓）。
        "__builtins__" => match object.globals() {
            Some(mapping) => {
                if let Some(found) = instance.dict_get(mapping, "__builtins__") {
                    // SAFETY: found 由映射持有，交调用方一份。
                    unsafe { instance.incref_object(found.as_ptr()) };
                    Some(found)
                } else {
                    Some(instance.new_dict())
                }
            }
            None => Some(instance.new_dict()),
        },
        "__globals__" => match object.globals() {
            Some(mapping) => {
                // SAFETY: mapping 由函数持有，存活。
                unsafe { instance.incref_object(mapping.as_ptr()) };
                Some(mapping)
            }
            None => Some(instance.retain(instance.singletons().none())),
        },
        "__annotate__" => match object.annotate() {
            Some(callable) => {
                // SAFETY: callable 由函数持有，存活。
                unsafe { instance.incref_object(callable.as_ptr()) };
                Some(callable)
            }
            // 实测：**没有注解**的函数，`f.__annotate__` 就是 `None`（不是缺属性）
            None => Some(instance.retain(instance.singletons().none())),
        },
        _ => None,
    }
}

/// `OM-40`：列出函数持有的引用。
pub(crate) unsafe fn function_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    visit(object.code().as_ptr());
    for value in object.defaults() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.kwdefaults() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.globals() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.annotate() {
        visit(value.as_ptr());
    }
    if let Some(value) = object.annotations_cache() {
        visit(value.as_ptr());
    }
    // **可写属性的覆盖值**（`py_object!` 第二臂的四个默认字段 ✓）：持引用 ⇒ 必须走 GC ✓。
    // ＋ **默认值缓存**（task-22：`name_cache`／`qualname_cache`／`module_cache` 也是本对象持有的引用 ✓）。
    for value in [
        object.name_override(),
        object.qualname_override(),
        object.doc_override(),
        object.module_override(),
        object.name_cache(),
        object.qualname_cache(),
        object.module_cache(),
    ]
    .into_iter()
    .flatten()
    {
        visit(value.as_ptr());
    }
    for cell in object.closure() {
        visit(cell.as_ptr());
    }
}

/// `OM-40`／`OM-20` ②：交出函数持有的引用。
pub(crate) unsafe fn function_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &mut *ptr.cast::<FunctionObject>() };
    // SAFETY: 这些引用由本对象持有。
    unsafe { instance.release_object(object.code().as_ptr()) };
    for cell in object.set_closure(Vec::new()) {
        // SAFETY: 同上。
        unsafe { instance.release_object(cell.as_ptr()) };
    }
    for value in core::mem::take(&mut object.defaults) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_kwdefaults(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_globals(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    // `annotate`（`SET_FUNCTION_ATTRIBUTE` 的 bit4）与 `__annotations__` 缓存
    // ——**此前漏在 traverse／clear 之外**，这里补齐（否则 GC 看不到、也不释放）
    if let Some(value) = object.set_annotate(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    if let Some(value) = object.set_annotations_cache(None) {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
    // **可写属性的覆盖值**（四个默认字段；`OM-12` 的"持引用字段必须被 clear 覆盖" ✓）。
    // ＋ **默认值缓存**（task-22：同上，持引用 ⇒ 必须被 clear 覆盖并归还 ✓）。
    for value in [
        object.set_name_override(None),
        object.set_qualname_override(None),
        object.set_doc_override(None),
        object.set_module_override(None),
        object.name_cache.borrow_mut().take(),
        object.qualname_cache.borrow_mut().take(),
        object.module_cache.borrow_mut().take(),
    ]
    .into_iter()
    .flatten()
    {
        // SAFETY: 同上。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

/// 函数的 `repr`：`<function demo at 0x…>`（实测）。
pub unsafe fn function_repr(ptr: *mut Header, instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    // **`__qualname__` 覆盖值优先** ✓（参照的 `repr` 用的就是 `__qualname__` ✓：
    // `functools.wraps` 之后 `repr(f)` 与 `repr(g)` 同名 ✓）。
    if let Some(override_value) = object.qualname_override() {
        if let Some(text) = instance.text_of(override_value) {
            return Ok(format!("<function {text} at {ptr:p}>"));
        }
    }
    // SAFETY: 函数持有 code object 的一份引用。
    let code = object.code();
    // SAFETY: 同上。
    let name = unsafe { code.cast::<crate::CodeObject>().as_ref() }.name();
    Ok(format!("<function {name} at {ptr:p}>"))
}

// --------------------------------------------------------------------------- #
// **函数可写属性** ＝ **可写 getset 数据描述符**（`P3-17` 同族 ＋ `functools.wraps.__name__` 的正路 ✓）
// --------------------------------------------------------------------------- #
//
// 参照 3.14.4 实测（逐条对拍，脚本见 `crates/pyawa-runtime/tests/function_attributes.rs`）：
// `f.__name__`／`f.__qualname__`／`f.__doc__`／`f.__module__` 在 `function.__dict__` 里是
// **数据描述符**（getset／member descriptor ✓）⇒ 赋值走 `__set__`、**不**进实例字典 ✓；
// 读走 `__get__`，且**优先于实例字典** ✓。`__name__`／`__qualname__` 的 `__set__` 只收
// `str`（子类也收 ✓），`del` 也走 `__set__(NULL)` ⇒ `TypeError: … must be set to a string object` ✓；
// `__doc__`／`__module__` 不收类型限制、可以 `del`（`del` 之后读到 `None` ✓）。
//
// 本层实现：值存在 [`FunctionObject`] 的四个**覆盖字段**里（`py_object!` 第二臂 ✓，
// 不进实例字典 ✓），描述符对象由 `executor::protocol::install_builtin_descriptors` 装进
// `function` 的类型字典 ✓。**如实登记**：描述符**类型名**是自建的
// `function_attribute_descriptor`（参照是 `getset_descriptor`／`member_descriptor`）——
// 类别偏差按既有"方案 A"口径如实标 ✓，语义面逐条一致 ✓。

/// 可写属性的种类（描述符实例的 `_kind` 字符串 ⇒ 这里的第一维 ✓）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionAttributeKind {
    Name,
    Qualname,
    Doc,
    Module,
    /// `f.__defaults__`（位置默认值元组／`None`；round 111 接线可写）
    Defaults,
    /// `f.__kwdefaults__`（仅关键字默认值 dict／`None`；round 111 接线可写）
    Kwdefaults,
}

impl FunctionAttributeKind {
    /// 从描述符实例的 `_kind` 取种类（**借用视图** ⇒ 立刻转成枚举 ✓）。
    fn of(instance: &Instance, descriptor: NonNull<Header>) -> Option<Self> {
        let kind = instance
            .attribute_optional_of(descriptor, "_kind")
            .ok()
            .flatten()?;
        match instance.text_of(kind)? {
            "name" => Some(Self::Name),
            "qualname" => Some(Self::Qualname),
            "doc" => Some(Self::Doc),
            "module" => Some(Self::Module),
            "defaults" => Some(Self::Defaults),
            "kwdefaults" => Some(Self::Kwdefaults),
            _ => None,
        }
    }

    /// Python 侧的属性名（错误消息与 `function_getattr` 的默认值都用它 ✓）。
    fn attribute_name(self) -> &'static str {
        match self {
            Self::Name => "__name__",
            Self::Qualname => "__qualname__",
            Self::Doc => "__doc__",
            Self::Module => "__module__",
            Self::Defaults => "__defaults__",
            Self::Kwdefaults => "__kwdefaults__",
        }
    }
}

/// 取函数对象；不是 `function` ⇒ 参照那句 `descriptor '…' for 'function' objects doesn't apply to a '…' object`。
fn function_target(
    instance: &Instance,
    kind: FunctionAttributeKind,
    target: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    if instance.type_name(instance.type_of(target)) != "function" {
        let actual = instance.type_name(instance.type_of(target));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "descriptor '{}' for 'function' objects doesn't apply to a '{actual}' object",
                kind.attribute_name()
            ),
        ));
    }
    Ok(target)
}

/// 按种类换**覆盖字段**，返回旧值（**调用方负责归还**）。
fn replace_override(
    object: &FunctionObject,
    kind: FunctionAttributeKind,
    value: Option<NonNull<Header>>,
) -> Option<NonNull<Header>> {
    match kind {
        FunctionAttributeKind::Name => object.set_name_override(value),
        FunctionAttributeKind::Qualname => object.set_qualname_override(value),
        FunctionAttributeKind::Doc => object.set_doc_override(value),
        FunctionAttributeKind::Module => object.set_module_override(value),
        // **`__defaults__`／`__kwdefaults__` 不走"覆盖值"面**：它们直接改函数字段 ✓（见 `set` 分支）。
        FunctionAttributeKind::Defaults | FunctionAttributeKind::Kwdefaults => {
            value.map(|_| value.unwrap())
        }
    }
}

/// **`function_attribute_descriptor.__get__(obj, owner=None)`** ✓。
pub(crate) unsafe fn function_attribute_descriptor_get(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(descriptor) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__get__' needs an argument",
        ));
    };
    let Some(kind) = FunctionAttributeKind::of(instance, descriptor) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__get__' 的 _kind 不合法",
        ));
    };
    // **实参个数照参照** ✓（3.14.4 实测：0 ⇒ `__get__ expected at least 1 argument, got 0`；
    // 3+ ⇒ `… at most 2 arguments, got N` ✓）。
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
    let target = args[0];
    // `descr.__get__(None, owner)` ⇒ 交回**描述符自己** ✓（参照 getset 的口径 ✓）；
    // `owner` 缺省或也是 `None` ⇒ `TypeError: __get__(None, None) is invalid` ✓。
    if target == none {
        let owner_missing = args.len() < 2 || args[1] == none;
        if owner_missing {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "__get__(None, None) is invalid",
            ));
        }
        return Ok(instance.retain(descriptor));
    }
    let target = function_target(instance, kind, target)?;
    // **一处真相**：默认值（code object／`__globals__`）由 `function_getattr` 给 ✓，
    // 覆盖值的检查也在它里面（且优先）✓。
    // SAFETY: 上面已确认 target 是函数对象。
    unsafe { function_getattr(target.as_ptr(), kind.attribute_name(), instance) }.ok_or_else(|| {
        instance.raise_builtin_error(
            "AttributeError",
            &format!("'function' object has no attribute {}", kind.attribute_name()),
        )
    })
}

/// **`function_attribute_descriptor.__set__(obj, value)`** ✓（写覆盖字段，**不**动实例字典 ✓）。
pub(crate) unsafe fn function_attribute_descriptor_set(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(descriptor) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__set__' needs an argument",
        ));
    };
    let Some(kind) = FunctionAttributeKind::of(instance, descriptor) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__set__' 的 _kind 不合法",
        ));
    };
    // **实参个数照参照** ✓（实测：`__set__ expected 2 arguments, got N` ✓）。
    if args.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("__set__ expected 2 arguments, got {}", args.len()),
        ));
    }
    let target = args[0];
    let value = args[1];
    let target = function_target(instance, kind, target)?;
    // 参照实测：`__name__`／`__qualname__` 只收 `str`（`str` 子类也收 ✓）；
    // `__doc__`／`__module__` 什么对象都收 ✓。
    match kind {
        FunctionAttributeKind::Name | FunctionAttributeKind::Qualname => {
            if instance.text_of(value).is_none() {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("{} must be set to a string object", kind.attribute_name()),
                ));
            }
        }
        // 参照实测：`__doc__`／`__module__` 的 `__set__` 不收类型限制 ✓。
        FunctionAttributeKind::Doc | FunctionAttributeKind::Module => {}
        // **`__defaults__`**：`None` 或 tuple ✓（参照：`TypeError: __defaults__ must be set to a
        // tuple or None` ✓）。tuple ⇒ 每项各留一份交给函数字段 ✓。
        FunctionAttributeKind::Defaults => {
            let is_none = value == instance.singletons().none();
            let is_tuple = instance.hot_type(HotId::Tuple)
                .is_some_and(|tuple_ty| instance.type_of(value) == tuple_ty);
            if !is_none && !is_tuple {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "__defaults__ must be set to a tuple or None",
                ));
            }
            // SAFETY: 上面已确认 target 是函数对象。
            let object = unsafe { &*target.as_ptr().cast::<FunctionObject>() };
            let old: Vec<NonNull<Header>> = object.defaults().to_vec();
            let fresh: Vec<NonNull<Header>> = if is_none {
                Vec::new()
            } else {
                // SAFETY: is_tuple 已确认。
                let tuple = unsafe { &*value.as_ptr().cast::<TupleObject>() };
                let mut items = Vec::with_capacity(tuple.len());
                for index in 0..tuple.len() {
                    let item = tuple.item(index).expect("下标在范围内");
                    // SAFETY: 元素由元组持有。
                    unsafe { instance.incref_object(item.as_ptr()) };
                    items.push(item);
                }
                items
            };
            // SAFETY: 仍是同一函数对象（函数字段可写）。
            unsafe { &mut *target.as_ptr().cast::<FunctionObject>() }.set_defaults(fresh);
            for old_item in old {
                // SAFETY: 旧默认值那份由函数字段持有 ⇒ 归还。
                unsafe { instance.release_object(old_item.as_ptr()) };
            }
            return Ok(instance.retain(instance.singletons().none()));
        }
        // **`__kwdefaults__`**：`None` 或 dict ✓（参照：`TypeError: __kwdefaults__ must be set to a
        // dict or None` ✓）。只换字段持有那份 ✓。
        FunctionAttributeKind::Kwdefaults => {
            let is_none = value == instance.singletons().none();
            let is_dict = instance.hot_type(HotId::Dict)
                .is_some_and(|dict_ty| instance.is_subtype(instance.type_of(value), dict_ty));
            if !is_none && !is_dict {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "__kwdefaults__ must be set to a dict or None",
                ));
            }
            // SAFETY: 上面已确认 target 是函数对象。
            let object = unsafe { &*target.as_ptr().cast::<FunctionObject>() };
            let old = object.kwdefaults();
            let fresh = if is_none { None } else { Some(value) };
            if let Some(mapping) = fresh {
                // 字段接手一份新引用。
                instance.retain(mapping);
            }
            // SAFETY: 仍是同一函数对象（函数字段可写）。
            unsafe { &mut *target.as_ptr().cast::<FunctionObject>() }.set_kwdefaults(fresh);
            if let Some(old) = old {
                // SAFETY: 旧 dict 那份由函数字段持有 ⇒ 归还。
                unsafe { instance.release_object(old.as_ptr()) };
            }
            return Ok(instance.retain(instance.singletons().none()));
        }
    }
    // SAFETY: 上面已确认 target 是函数对象。
    let object = unsafe { &*target.as_ptr().cast::<FunctionObject>() };
    // **本函数接手一份新引用**（字段持有它 ✓）。
    instance.retain(value);
    let previous = replace_override(object, kind, Some(value));
    if let Some(previous) = previous {
        // SAFETY: previous 是旧覆盖值，由本函数负责归还。
        unsafe { instance.release_object(previous.as_ptr()) };
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`function_attribute_descriptor.__delete__(obj)`** ✓。
///
/// 参照实测：`__name__`／`__qualname__` 抛 `TypeError: … must be set to a string object` ✓；
/// `__doc__`／`__module__` **可以删**，删完读到 `None` ✓（⇒ 存一个显式的 `None` 覆盖 ✓，
/// 不是"回到 code object 的默认值" ✗）。
pub(crate) unsafe fn function_attribute_descriptor_delete(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(descriptor) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__delete__' needs an argument",
        ));
    };
    let Some(kind) = FunctionAttributeKind::of(instance, descriptor) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__delete__' 的 _kind 不合法",
        ));
    };
    // **实参个数照参照** ✓（实测 0 ⇒ `TypeError: expected 1 argument, got 0` ✓）。
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("expected 1 argument, got {}", args.len()),
        ));
    }
    let target = args[0];
    let target = function_target(instance, kind, target)?;
    match kind {
        // 参照的 `__set__` 收到 `NULL` 时就是这句（它把 `NULL` 当"不是 str" ✓）。
        FunctionAttributeKind::Name | FunctionAttributeKind::Qualname => {
            Err(instance.raise_builtin_error(
                "TypeError",
                &format!("{} must be set to a string object", kind.attribute_name()),
            ))
        }
        // `del f.__defaults__`／`del f.__kwdefaults__` ⇒ 清空字段（读回 `None` ✓）。
        FunctionAttributeKind::Defaults => {
            // SAFETY: 上面已确认 target 是函数对象。
            let object = unsafe { &*target.as_ptr().cast::<FunctionObject>() };
            let old: Vec<NonNull<Header>> = object.defaults().to_vec();
            // SAFETY: 同一函数对象；字段可写。
            unsafe { &mut *target.as_ptr().cast::<FunctionObject>() }.set_defaults(Vec::new());
            for item in old {
                // SAFETY: 旧默认值那份由函数字段持有 ⇒ 归还。
                unsafe { instance.release_object(item.as_ptr()) };
            }
            Ok(instance.retain(instance.singletons().none()))
        }
        FunctionAttributeKind::Kwdefaults => {
            // SAFETY: 上面已确认 target 是函数对象。
            let object = unsafe { &*target.as_ptr().cast::<FunctionObject>() };
            let old = object.kwdefaults();
            // SAFETY: 同一函数对象；字段可写。
            unsafe { &mut *target.as_ptr().cast::<FunctionObject>() }.set_kwdefaults(None);
            if let Some(old) = old {
                // SAFETY: 旧 dict 那份由函数字段持有 ⇒ 归还。
                unsafe { instance.release_object(old.as_ptr()) };
            }
            Ok(instance.retain(instance.singletons().none()))
        }
        FunctionAttributeKind::Doc | FunctionAttributeKind::Module => {
            // SAFETY: 上面已确认 target 是函数对象。
            let object = unsafe { &*target.as_ptr().cast::<FunctionObject>() };
            let none = instance.retain(instance.singletons().none());
            if let Some(previous) = replace_override(object, kind, Some(none)) {
                // SAFETY: previous 是旧覆盖值，由本函数负责归还。
                unsafe { instance.release_object(previous.as_ptr()) };
            }
            Ok(instance.retain(instance.singletons().none()))
        }
    }
}
