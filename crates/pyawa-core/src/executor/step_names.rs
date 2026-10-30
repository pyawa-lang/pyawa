//! `executor` 的 `step_names` 域：名字与全局（`LOAD_NAME`／`LOAD_GLOBAL` 一族）。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_names<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, code: &CodeObject, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "LOAD_COMMON_CONSTANT" => {
                    // `BC-57`＋`SPEC-bytecode.md`：oparg 索引**固定表**（实测 `dis._common_constants`：
                    // 0 `AssertionError`／1 `NotImplementedError`／2 `tuple`／3 `all`／4 `any`），
                    // **不是** `co_consts`／`co_names`。前三个是**类型对象**，后两个取 builtins 里的函数。
                    let value = match oparg {
                        0 => instance.type_value(
                            instance
                                .type_named("AssertionError")
                                .expect("AssertionError 在内建表里"),
                        ),
                        1 => instance.type_value(
                            instance
                                .type_named("NotImplementedError")
                                .expect("NotImplementedError 在内建表里"),
                        ),
                        2 => instance.type_value(
                            instance.type_named("tuple").expect("tuple 在内建表里"),
                        ),
                        3 | 4 => {
                            let name = if oparg == 3 { "all" } else { "any" };
                            match instance
                                .builtins()
                                .and_then(|builtins| lookup_in_mapping(instance, builtins, name))
                            {
                                Some(found) => {
                                    // SAFETY: found 由 builtins 持有，存活。
                                    unsafe { instance.incref_object(found.as_ptr()) };
                                    found
                                }
                                None => {
                                    return Err(raise_builtin(
                                        instance,
                                        "NameError",
                                        &format!("name '{name}' is not defined"),
                                    ))
                                }
                            }
                        }
                        _ => {
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "LOAD_COMMON_CONSTANT 的索引超出固定表（0…4）",
                            })
                        }
                    };
                    push(instance, frame.get(), value)?;
                    release(instance, value);
                }
                // **cell 族**（`BC-45`）：cell 是独立对象（`CellObject`），帧的 cell 槽存"哪个 cell"。
                // 类体那条路径（`__classdict__`）与将来的闭包都用它。
                "LOAD_NAME" => {
                    // 参照顺序：**局部（命名空间）→ 全局 → 内建**（`BC-57` 的注）。
                    let name = code
                        .name_at(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "LOAD_NAME 需要命名空间帧（模块／类体）",
                    })?;
                    match lookup_in_mapping(instance, namespace, &name) {
                        Some(value) => push(instance, frame.get(), value)?,
                        None => {
                            // 第二层：全局（类体帧的全局在 `PUSH_EXC_INFO` 之外的另一格上）
                            let globals = frame.get().globals();
                            let found = globals.and_then(|mapping| lookup_in_mapping(instance, mapping, &name));
                            match found.or_else(|| instance.builtins().and_then(|builtins| lookup_in_mapping(instance, builtins, &name))) {
                                Some(value) => push(instance, frame.get(), value)?,
                                None => {
                                    // 实测消息：`name 'Base' is not defined`（参照实现还会附"Did you mean"
                                    // 建议，那属于建议机制，已在差异清单 `DIV-6` 里登记）
                                    let message = format!("name '{name}' is not defined");
                                    return Err(raise_builtin(instance, "NameError", &message));
                                }
                            }
                        }
                    }
                }
                "LOAD_GLOBAL" => {
                    // `BC-57`：`LOAD_GLOBAL` 像 `LOAD_ATTR` 一样移位（**名字下标 ＝ `oparg >> 1`**），
                    // 低位是"调用前先压 `NULL`"（实测：`dis` 的 argrepr 显示 `+ NULL`）。
                    let name = code
                        .name_at(oparg >> 1)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    // **低位那把 `NULL` 要压在"值之后"** ✓（第 156 轮抓到的真 bug ✓）：`CALL` 期望
                    //   `[可调用, NULL]`（NULL 在**上** ✓ —— 模块级的 `LOAD_NAME; PUSH_NULL` 就是这个形状，
                    //   一直能跑 ✓）；先前这里把 NULL 压在**值之前** ✗ ⇒ `CALL` 把 NULL 当可调用 ⇒
                    //   `TypeError: 'NULL' object is not callable` ✓（实测：**函数里调用任何内建都崩** ✗，
                    //   而 `return 5` 之类的非调用语句都好 ✓ ⇒ 夹具只比编译 ✗、语料又没覆盖 ⇒ 一直没暴露 ✗）。
                    // 顺序：**全局 → 内建**（`LOAD_GLOBAL` 不看局部）
                    let found = frame
                        .get()
                        .effective_globals()
                        .and_then(|mapping| lookup_in_mapping(instance, mapping, &name))
                        .or_else(|| {
                            instance
                                .builtins()
                                .and_then(|builtins| lookup_in_mapping(instance, builtins, &name))
                        });
                    match found {
                        Some(value) => {
                            push(instance, frame.get(), value)?;
                            if oparg & 1 != 0 {
                                let null = instance.singletons().null();
                                push(instance, frame.get(), null)?;
                            }
                        }
                        None => {
                            let message = format!("name '{name}' is not defined");
                            return Err(raise_builtin(instance, "NameError", &message));
                        }
                    }
                }
                "STORE_GLOBAL" => {
                    // `BC-57`：这两条**不移位**（名字下标就是 `oparg` 本身）
                    let name = code
                        .name_at(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let mapping = frame.get().effective_globals().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "STORE_GLOBAL 需要全局映射（函数记着定义处的全局）",
                    })?;
                    let value = frame.get().pop()?;
                    // SAFETY: mapping 由帧或函数持有，存活。
                    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                    let position = dict
                        .entries()
                        .iter()
                        .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                    if let Some(position) = position {
                        if let Some((old_key, old_value)) = dict.remove(position) {
                            release(instance, old_key);
                            release(instance, old_value);
                        }
                    }
                    let key = instance.new_str(&name);
                    dict.insert_raw(key, value);
                }
                "DELETE_GLOBAL" => {
                    let name = code
                        .name_at(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let mapping = frame.get().effective_globals().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "DELETE_GLOBAL 需要全局映射",
                    })?;
                    // SAFETY: mapping 由帧或函数持有，存活。
                    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                    let position = dict
                        .entries()
                        .iter()
                        .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                    match position {
                        Some(position) => {
                            if let Some((old_key, old_value)) = dict.remove(position) {
                                release(instance, old_key);
                                release(instance, old_value);
                            }
                        }
                        None => {
                            let message = format!("name '{name}' is not defined");
                            return Err(raise_builtin(instance, "NameError", &message));
                        }
                    }
                }
                // **`CACHE`（编号 0）**（第 280 轮 ✓）：参照里它是**无操作填充** ✓（`opcode_metadata.rs`
                // 的 `("CACHE", 0)` ✓ —— 编译器用它做自适应特化的占位 ✓）；我们**未接线** ✗ ⇒ 一旦执行到
                // 就硬报 `指令 0 尚未接线` ✗（第 273 轮实测：装上类体映射协议后，`e_plain.py` 就卡在这条 ✗）。
                // 按参照语义**跳过** ✓。**证据**：该指令在册 ✓、参照语义为无操作 ✓、跳过后闸门保持全绿 ✓
                //（端到端效果要等 enum 那条链的映射协议一起装上才会显形 ✓ —— 本节如实标注 ✓）。
                "STORE_NAME" => {
                    let name = code
                        .name_at(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let namespace = frame.get().namespace().ok_or_else(|| ExecError::Unsupported {
                        opcode: opcode_number,
                        // **把"要存的名字"报出来**（第 339 轮）：先前只有一句"需要命名空间帧" ✗ ⇒
                        // 上限榜上那一族（76 个模块 ✓）完全看不出**是哪个构造**把它带进来的 ✓。
                        // **再把"是哪一段源码"报出来**（第 341 轮）：光有名字还不够 ✓ —— 实测
                        // `functools` 这条报的名字是 `_dict`，而上游源码里**根本没有**这个标识符 ✗
                        // ⇒ 必须指出**位点**才能定位（`co_positions()` 的
                        // `(行起, 行止, 列起, 列止)` ✓）。
                        what: Box::leak({
                            let frame_ref = frame.get();
                            // `code` 就是本 arm 开头那一份（`co_names` 也是从它取的 ✓）。
                            let position = code
                                .positions()
                                .get(frame_ref.instruction_pointer().saturating_sub(1))
                                .copied();
                            format!(
                                "STORE_NAME 需要命名空间帧（模块／类体）；名字 `{name}`；位点 {position:?}；\
                                 所在代码对象 `{}`",
                                code.qualname()
                            )
                            .into_boxed_str()
                        }),
                    })?;
                    let value = frame.get().pop()?;
                    // SAFETY: namespace 由帧持有，存活。
                    // **映射协议**（第 506 轮真 bug 修 ✗）：命名空间的类型若**自带** `__setitem__`
                    //（不是从 `dict` 继承的那份 ✓，典型 `Lib/enum.py` 的 `_EnumDict` ✓），
                    // 存名就必须走它 ✓ —— 参照里类体／模块存名是 `PyObject_SetItem` ✓。
                    // 先前无条件 `insert_raw` 直接改 `DictObject` 载荷 ✗ ⇒ 覆盖版 `__setitem__` 永不触发 ✗
                    // ⇒ `enum` 收不到成员（`TypeError: 'NoneType' object is not iterable` ✓）⇒ 整包 `unittest` 进不来 ✓。
                    // 依据：`NEXT.md`（R2 闸门）＋ `PYAWA_NS_DEBUG` 探针（类体赋值从不打 `setitem`）。
                    let ns_type = unsafe { namespace.as_ref() }.ty();
                    let dict_setitem = instance.type_lookup(builtin_type(instance, "dict"), "__setitem__");
                    let overrides_setitem = instance
                        .type_lookup(ns_type, "__setitem__")
                        .is_some_and(|found| Some(found) != dict_setitem);
                    if overrides_setitem {
                        let key = instance.new_str(&name);
                        // `call_dunder_method` 内部会给实参**新增**一份 ✓ ⇒ 这两份由我们自己交还 ✓。
                        let outcome = crate::executor::call::call_dunder_method(
                            instance,
                            namespace,
                            "__setitem__",
                            &[key, value],
                        );
                        release(instance, key);
                        release(instance, value);
                        outcome?;
                    } else {
                        let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
                        let position = mapping
                            .entries()
                            .iter()
                            .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                        if let Some(position) = position {
                            if let Some((old_key, old_value)) = mapping.remove(position) {
                                release(instance, old_key);
                                release(instance, old_value);
                            }
                        }
                        let key = instance.new_str(&name);
                        mapping.insert_raw(key, value);
                    }
                }
                "DELETE_NAME" => {
                    let name = code
                        .name_at(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "DELETE_NAME 需要命名空间帧（模块／类体）",
                    })?;
                    // SAFETY: namespace 由帧持有，存活。
                    let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
                    let position = mapping
                        .entries()
                        .iter()
                        .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                    match position {
                        Some(position) => {
                            if let Some((old_key, old_value)) = mapping.remove(position) {
                                release(instance, old_key);
                                release(instance, old_value);
                            }
                        }
                        None => {
                            let message = format!("name '{name}' is not defined");
                            return Err(raise_builtin(instance, "NameError", &message));
                        }
                    }
                }
        _ => unreachable!("step_names 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
