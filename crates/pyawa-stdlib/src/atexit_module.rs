//! **`atexit` 最小面** ✓（task-22 ✓）：参照里它是 **C 模块** ✗（上游没有 `Lib/atexit.py` ✓）
//! ⇒ 本层按 Rust 侧模块落地 ✓（与 `sys`／`_sre` 同款装配 ✓）。
//!
//! 面：`register(func, *args, **kwargs)` ✓、`unregister(func)` ✓、`_run_exitfuncs()` ✓、`_clear()` ✓。
//! **一处真相**：注册表存在**模块命名空间的私有键** `_handlers` 里 ✓（**按实例** ✓ —— 不用
//! `thread_local` ✗，免得同进程多实例互相串 ✓）；那份表是**元组** ✓（不可变 ⇒ 每次整体换新 ✓ ——
//! 本 crate `#![forbid(unsafe_code)]` ✗，元组读写全走 core 的**安全** API ✓，不用裸载荷指针 ✓）。
//!
//! **如实范围** ✗：`_run_exitfuncs` **没有**接到解释器退出 ✓（组合根还没有退出钩子 ✓）——
//! 目前只有**显式调用**才会跑 ✓；注册／注销的语义与参照一致 ✓（`Lib/logging/__init__.py` 的
//! `atexit.register(shutdown)` 与 `Lib/concurrent/futures/thread.py` 同款都只要"注册"不炸 ✓）。

use core::ptr::NonNull;
use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`sys.modules` 里的键 ✓）。
pub const NAME: &str = "atexit";

/// 私有键：注册表（`tuple` of `(callable, args, kwargs)` ✓）。
const HANDLERS: &str = "_handlers";

/// 元组里第 `index` 项（**借用** ✓）。
fn tuple_at(instance: &Instance, tuple: NonNull<Header>, index: usize) -> Option<NonNull<Header>> {
    instance
        .tuple_items(tuple)
        .and_then(|items| items.get(index).copied())
}

fn make_native(instance: &Instance, name: &'static str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        name,
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 取本模块的命名空间（**借用** ✓；模块对象由 `sys.modules` 持着 ✓）。
fn namespace_of(instance: &Instance) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    pyawa_core::mounted_instance_dict(instance, module)
}

/// 注册表那一串条目（**借用** ✓）。
fn entries_of(instance: &Instance) -> Option<Vec<NonNull<Header>>> {
    let handlers = instance.dict_get(namespace_of(instance)?, HANDLERS)?;
    instance.tuple_items(handlers)
}

/// 换一份新的注册表 ✓（元组不可变 ⇒ 整体换 ✓）。
fn store_entries(instance: &Instance, entries: Vec<NonNull<Header>>) {
    let Some(namespace) = namespace_of(instance) else {
        return;
    };
    // `new_tuple` 要**新引用** ✓（entries 里的是借用 ⇒ 各留一份 ✓）。
    let owned: Vec<NonNull<Header>> = entries
        .into_iter()
        .map(|value| instance.retain(value))
        .collect();
    let handlers = instance.new_tuple(owned);
    instance.dict_set(namespace, HANDLERS, handlers);
    instance.release(handlers);
}

/// **`atexit.register(func, *args, **kwargs)`** ✓：登记一项 ⇒ 回 `func` ✓（参照同 ✓）。
fn register_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(callable) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "register() takes at least 1 argument (0 given)",
        ));
    };
    let Some(mut entries) = entries_of(instance) else {
        return Err(instance.raise_builtin_error("RuntimeError", "atexit 模块没装好"));
    };
    // 参数那一份**拷贝**（元组接手传入的引用 ✓）。
    let positionals: Vec<NonNull<Header>> = args[1..]
        .iter()
        .map(|value| instance.retain(*value))
        .collect();
    let positional_tuple = instance.new_tuple(positionals);
    let keyword_dict = instance.new_dict();
    for (key, value) in kwargs {
        if let Some(text) = instance.text_of(*key) {
            let text = text.to_owned();
            instance.dict_set(keyword_dict, &text, *value);
        }
    }
    let entry = instance.new_tuple(vec![
        instance.retain(callable),
        positional_tuple,
        keyword_dict,
    ]);
    entries.push(entry);
    store_entries(instance, entries);
    Ok(callable)
}

/// **`atexit.unregister(func)`** ✓：按**身份**摘掉登记项 ⇒ 回 `None` ✓。
fn unregister_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(callable) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "unregister() takes at least 1 argument (0 given)",
        ));
    };
    let Some(entries) = entries_of(instance) else {
        return Err(instance.raise_builtin_error("RuntimeError", "atexit 模块没装好"));
    };
    let kept: Vec<NonNull<Header>> = entries
        .into_iter()
        .filter(|entry| tuple_at(instance, *entry, 0) != Some(callable))
        .collect();
    store_entries(instance, kept);
    Ok(instance.retain(instance.singletons().none()))
}

/// **`atexit._clear()`** ✓：清空注册表 ⇒ 回 `None` ✓。
fn clear_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if entries_of(instance).is_none() {
        return Err(instance.raise_builtin_error("RuntimeError", "atexit 模块没装好"));
    }
    store_entries(instance, Vec::new());
    Ok(instance.retain(instance.singletons().none()))
}

/// **`atexit._run_exitfuncs()`** ✓：**后进先出**跑一遍并清空 ✓（参照同 ✓）；某项抛异常 ⇒ 原样传出 ✓。
fn run_exitfuncs_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    loop {
        let Some(mut entries) = entries_of(instance) else {
            return Err(instance.raise_builtin_error("RuntimeError", "atexit 模块没装好"));
        };
        let Some(entry) = entries.pop() else {
            return Ok(instance.retain(instance.singletons().none()));
        };
        store_entries(instance, entries);
        let callable = tuple_at(instance, entry, 0);
        let positionals = tuple_at(instance, entry, 1)
            .map(|value| {
                instance
                    .tuple_items(value)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|item| instance.retain(item))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let keywords: Vec<(NonNull<Header>, NonNull<Header>)> = tuple_at(instance, entry, 2)
            .map(|value| {
                instance
                    .dict_entries(value)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(key, value)| (instance.retain(key), instance.retain(value)))
                    .collect()
            })
            .unwrap_or_default();
        let Some(callable) = callable else {
            continue;
        };
        let outcome =
            pyawa_core::executor::call::call_value(instance, callable, &positionals, &keywords);
        for value in positionals {
            instance.release(value);
        }
        for (key, value) in keywords {
            instance.release(key);
            instance.release(value);
        }
        // 返回值丢掉（参照同 ✓）。
        if let Ok(value) = outcome {
            instance.release(value);
        }
        outcome?;
    }
}

/// 装出 `atexit` 的命名空间 ✓（**新建 dict** ✓；由组合根包成模块对象 ✓）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // 模块对象要有 `__name__` ✓（与 `errno`／`weakref` 同款 ✓ —— 少了它
    // `Lib/logging/__init__.py` 一族的 `__name__` 取用会炸 ✓）。
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    instance.release(module_name);
    let doc = instance.new_str("atexit 最小面（task-22）：register／unregister／_run_exitfuncs／_clear。");
    instance.dict_set(namespace, "__doc__", doc);
    instance.release(doc);
    let handlers = instance.new_tuple(Vec::new());
    instance.dict_set(namespace, HANDLERS, handlers);
    instance.release(handlers);
    for (name, handler) in [
        ("register", register_native as NativeFn),
        ("unregister", unregister_native as NativeFn),
        ("_clear", clear_native as NativeFn),
        ("_run_exitfuncs", run_exitfuncs_native as NativeFn),
    ] {
        let native = make_native(instance, name, handler);
        instance.dict_set(namespace, name, native);
        instance.release(native);
    }
    namespace
}
