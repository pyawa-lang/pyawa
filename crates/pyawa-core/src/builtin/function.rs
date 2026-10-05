//! **`function` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `function_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：（无） ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::builtin_objects::{FunctionObject};
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
/// **未接**：`__doc__`（本层不解析文档字符串 ⇒ 一律 `None`，与"没有文档字符串"的情形一致）、
/// `__annotations__`（要调用 `__annotate__` 并**缓存**，另一笔——实测它的 `is` 稳定）。
/// 返回值一律**新引用**（调用方按 `OM-16` 接手）。
pub unsafe fn function_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    match name {
        "__name__" | "__qualname__" => {
            let code = object.code();
            // SAFETY: code 由函数持有，存活。
            let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
            let text = if name == "__name__" {
                code.name()
            } else {
                code.qualname()
            };
            Some(instance.new_str(text))
        }
        "__doc__" => {
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
            // 注意：`globals()` 给的是**命名空间字典本身** ⇒ 直接查 `__name__` ✓
            // （`module_text` 要的是**模块对象** ✗，第一版传错了 ⇒ 一律退回 `builtins` ✗）。
            let module = object
                .globals()
                .and_then(|globals| instance.dict_get(globals, "__name__"))
                .and_then(|value| instance.text_of(value).map(str::to_owned))
                .unwrap_or_else(|| "builtins".to_owned());
            Some(instance.new_str(&module))
        }
        // **`__class__`**（同上）：函数对象的类型就是 `function` ✓（参照给的就是那个类 ✓）。
        "__class__" => {
            let ty = instance.type_of(unsafe { NonNull::new_unchecked(ptr) });
            Some(instance.retain(ty.cast()))
        }
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
}

/// 函数的 `repr`：`<function demo at 0x…>`（实测）。
pub unsafe fn function_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FunctionObject>() };
    // SAFETY: 函数持有 code object 的一份引用。
    let code = object.code();
    // SAFETY: 同上。
    let name = unsafe { code.cast::<crate::CodeObject>().as_ref() }.name();
    Ok(format!("<function {name} at {ptr:p}>"))
}
