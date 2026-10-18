//! `executor` 的 `step_imports` 域：导入。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_imports<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, code: &CodeObject, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "IMPORT_NAME" => {
                    // 栈：`[level, fromlist]`（编译器先压两项 ✓）；`level > 0` 是**相对导入**（第 278 轮 ✓）。
                    let fromlist = frame.get().pop()?;
                    let level = frame.get().pop()?;
                    let level_value = instance.int_value(level);
                    release(instance, level);
                    // `fromlist` **留到装载之后再放** ✓（第 279 轮：`from . import 子模块` 那一步要用它 ✓）
                    let raw = code
                        .name_at(oparg as usize)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    // **相对导入** ✓（第 278 轮接线）：`level > 0` 时按当前模块的**包上下文**把名字解析成
                    // **绝对名** ✓ ⇒ 之后**只走下面这一条路** ✓（不复制第二套查找逻辑 ✓）。
                    let full = if level_value.unwrap_or(0) > 0 {
                        resolve_relative_import(
                            instance,
                            frame.get(),
                            &raw,
                            level_value.unwrap_or(0) as usize,
                            opcode_number,
                        )?
                    } else {
                        raw
                    };
                    // `import a.b.c` 交出的是**顶层模块**（随后 `STORE_NAME a` ✓，照参照实测）
                    let top = full.split('.').next().unwrap_or(full.as_str()).to_owned();
                    let modules = instance.modules().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "模块表未装配（import 的加载器未接：`P3-12`）",
                    })?;
                    // **`sys.modules` 里已有"整条带点名字"⇒ 直接用它** ✓（第 203 轮真 bug 修复 ✗）：
                    // `Lib/os.py:103` 正是 `sys.modules['os.path'] = path` ✓、104 再 `from os.path import …` ✓
                    // ⇒ 先前**先**去加载顶层 `os` ✗ —— 而 `os` 是**模块**不是包 ✗ ⇒ 报 `No module named 'os'`-ish ✗
                    //（实测最小复现：`sys.modules['demo.sub'] = itertools` 之后 `from demo.sub import count` ✗，
                    //  参照成功 ✓）。⇒ 与 CPython 同序 ✓：**先查模块表** ✓。
                    // **`fromlist` 非空 ⇒ 交叶子；空 ⇒ 交顶层** ✓（参照 `__import__` 的语义 ✓）——
                    // 这一判据**缓存命中那条路也要走** ✓（第 722 轮真 bug 修 ✗）：先前 `full` 在模块表里
                    // 就直接把**叶子**交出去 ✗ ⇒ `import email.quoprimime`（此时它已在表里 ✓）把局部名
                    // `email` 绑成 **quoprimime 模块本身** ✗ ⇒ `Lib/email/header.py:56` 的
                    // `email.quoprimime._max_append` 报 `'module' object has no attribute 'quoprimime'` ✗，
                    // `Lib/email/contentmanager.py:229` 的 `email.message.Message` 同型 ✗ ——
                    // email 一族十来个模块都压在这一条上 ✓。
                    let wants_leaf = full != top
                        && instance.type_name(instance.type_of(fromlist)) == "tuple"
                        && instance.tuple_items(fromlist).is_some_and(|items| !items.is_empty());
                    let module = match instance.dict_get(modules, &full) {
                        Some(found) => {
                            // 缓存命中：`fromlist` 空 ⇒ 交**顶层**（`sys.modules['os.path'] = …` 那种
                            // "父包不在表里"的写法**不能**凭空报错 ✗ ⇒ 取不到顶层就退回叶子 ✓）。
                            if !wants_leaf && full != top {
                                instance.dict_get(modules, &top).unwrap_or(found)
                            } else {
                                found
                            }
                        }
                        None => {
                            let loaded = match instance.dict_get(modules, &top) {
                                Some(found) => found,
                                // **加载器**（`IM-` 最小面）：按 `sys.path` 经 `fs` 域读 `<dir>/<名字>.py` ✓
                                None => load_module(instance, modules, &top, opcode_number)?,
                            };
                            // **带点名字要把整条链都导入**（第 135 轮）：`import a.b` 之后 `a.b` 必须可见 ✓
                            // （参照语义 ✓；`load_module` 会在父包的 `__path__` 里找子模块并挂成属性 ✓）。
                            // 顶层仍然交出（随后 `STORE_NAME a` ✓，与参照实测一致 ✓）。
                            if full != top && instance.dict_get(modules, &full).is_none() {
                                load_module(instance, modules, &full, opcode_number)?;
                            }
                            if wants_leaf {
                                instance.dict_get(modules, &full).unwrap_or(loaded)
                            } else {
                                loaded
                            }
                        }
                    };
                    // **`fromlist`：把"名字"当子模块载入** ✓（第 279 轮；参照的 `_handle_fromlist` ✓）
                    // —— `from . import _bootstrap` 就是靠这一步让 `importlib` 一族 4 个文件过线的 ✓。
                    // 模块名取**模块自己的 `__name__`** ✓（照参照 ✓；`sys.modules['os.path'] = posixpath`
                    // 那一类里，`full` 与模块真名**可以不同** ✗）。
                    let own_name =
                        module_text(instance, module, "__name__").unwrap_or_else(|| full.clone());
                    let handled = handle_fromlist(
                        instance,
                        modules,
                        module,
                        fromlist,
                        &own_name,
                        opcode_number,
                    );
                    release(instance, fromlist);
                    handled?;
                    // 交出一份**新引用**（`dict_get` 是借出 ✓）
                    // SAFETY: module 由模块表持有，活到实例销毁。
                    unsafe { instance.incref_object(module.as_ptr()) };
                    frame.get().push(module)?;
                }
                "IMPORT_FROM" => {
                    let name = code
                        .name_at(oparg as usize)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let module = frame.get().pop()?;
                    let found = attribute_lookup(instance, module, &name);
                    match found {
                        Ok(Attribute::Owned(value)) => {
                            frame.get().push(module)?;
                            frame.get().push(value)?;
                        }
                        Ok(Attribute::Value(value)) => {
                            frame.get().push(module)?;
                            push(instance, frame.get(), value)?;
                        }
                        Ok(Attribute::Method { function, this }) => {
                            // 模块属性理论上不会是方法 ✗，但**不吞错**：照 `LOAD_ATTR` 的取方法形态绑 ✓
                            // SAFETY: function／this 都还活着（类型字典与调用方持有）。
                            unsafe {
                                instance.incref_object(function.as_ptr());
                                instance.incref_object(this.as_ptr());
                            }
                            let bound = instance.alloc(crate::builtin_objects::MethodObject::new(
                                instance
                                    .type_named("method")
                                    .expect("`method` 类型已登记"),
                                function,
                                this,
                            ));
                            frame.get().push(module)?;
                            frame.get().push(bound.into_raw().cast::<Header>())?;
                        }
                        Err(error) => {
                            // **`from M import 缺名` 要报 `ImportError`** ✓（第 288 轮）：照参照实测
                            // `cannot import name 'x' from 'm'` ✓ —— 上游 `Lib/io.py:93` 的
                            // `try: from _io import _WindowsConsoleIO / except ImportError: pass` 正是靠它 ✓；
                            // 先前直接抛 `AttributeError` ✗ ⇒ 那个 `try` **接不住** ✗ ⇒ 整个 `io` 导入失败 ✓。
                            let attribute_error = builtin_type(instance, "AttributeError");
                            let is_missing = match &error {
                                ExecError::Raised { exception } => {
                                    // SAFETY: 抛出的异常由实例持有，存活。
                                    unsafe { exception.as_ref() }.ty() == attribute_error
                                }
                                _ => false,
                            };
                            if is_missing {
                                // **PEP 562**：先问**模块自己的** `__getattr__` ✓（`typing.Match` 那条路 ✓）；
                                // 它也没有 ⇒ **回退查 `sys.modules[<module.__name__>.<name>]`** ✓
                                //（第 408 轮，task-16 ①：CPython issue #17636 —— 循环相对导入
                                // `pkg/__init__ → a → b → from . import a` 里，`a` 已进 `sys.modules`
                                // 但还没挂成 `pkg` 的属性 ⇒ 只查属性会误报 `cannot import name` ✗；
                                // 参照在 `import_from` 里属性失败后查这条回退 ✓）。
                                if let Some(value) = module_getattr_missing(instance, module, &name) {
                                    frame.get().push(module)?;
                                    frame.get().push(value)?;
                                    return Ok(Step::Continue);
                                }
                                if let Some(modules) = instance.modules() {
                                    if let Some(module_name) = module_text(instance, module, "__name__") {
                                        let full = format!("{module_name}.{name}");
                                        if let Some(value) = instance.dict_get(modules, &full) {
                                            // 值来自模块表（**借用**）⇒ `push` 会自己留一份 ✓
                                            frame.get().push(module)?;
                                            push(instance, frame.get(), value)?;
                                            return Ok(Step::Continue);
                                        }
                                    }
                                }
                                // 两条回退都没有 ⇒ 照旧报 `ImportError` ✓。
                                // 模块名：从模块命名空间里借读 `__name__` ✓（借用 ⇒ 不还引用 ✓）
                                let module_name = crate::executor::module_text(instance, module, "__name__")
                                    .unwrap_or_else(|| "?".to_owned());
                                let converted = raise_builtin(
                                    instance,
                                    "ImportError",
                                    &format!("cannot import name '{name}' from '{module_name}'"),
                                );
                                release(instance, module);
                                return Err(converted);
                            }
                            release(instance, module);
                            return Err(error);
                        }
                    }
                }
        _ => unreachable!("step_imports 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
