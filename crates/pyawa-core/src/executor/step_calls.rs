//! `executor` 的 `step_calls` 域：调用、内建内联调用与函数对象。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_calls<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "DICT_MERGE" | "DICT_UPDATE" => {
                    // 实测净 −1：把 TOS 那个字典并进 TOS1，然后弹掉 TOS。
                    // `DICT_UPDATE` 覆盖同名键；`DICT_MERGE` 遇到同名键要报错——那条消息在参照实现里
                    // 带着**函数的 qualname**（实测：`__main__.demo() got multiple values for keyword
                    // argument 'a'`），而此刻调用者还在栈下好几层，本层取不到，所以如实报未接线。
                    let source = frame.get().pop()?;
                    let destination = frame.get().peek()?;
                    // SAFETY: 两个都在帧值栈上，存活。
                    let source_type = unsafe { source.as_ref() }.ty();
                    let destination_type = unsafe { destination.as_ref() }.ty();
                    if source_type != builtin_type(instance, "dict")
                        || destination_type != builtin_type(instance, "dict")
                    {
                        release(instance, source);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "DICT_MERGE／DICT_UPDATE 只接线了 dict（映射协议随后补）",
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let source_entries = unsafe { &*source.as_ptr().cast::<DictObject>() }.entries();
                    // SAFETY: 同上。
                    let destination_dict = unsafe { &*destination.as_ptr().cast::<DictObject>() };
                    let is_merge = name == "DICT_MERGE";
                    for (key, value) in source_entries {
                        let position = destination_dict
                            .entries()
                            .iter()
                            .position(|(existing, _)| values_equal(instance, *existing, key));
                        if let Some(existing_position) = position {
                            if is_merge {
                                release(instance, source);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "DICT_MERGE 的同名键错误要函数的 qualname（参照实现的消息带它）",
                                });
                            }
                            let (old_key, old_value) = destination_dict
                                .remove(existing_position)
                                .expect("刚查到的位置");
                            release(instance, old_key);
                            release(instance, old_value);
                        }
                        // SAFETY: 键值由源字典持有，这里各新增一份引用交给目标字典。
                        unsafe {
                            instance.incref_object(key.as_ptr());
                            instance.incref_object(value.as_ptr());
                        }
                        destination_dict.insert_raw(key, value);
                    }
                    release(instance, source);
                }
                "CALL_FUNCTION_EX" => {
                    // 实测净 −3；栈自下而上是 `[可调用, self|NULL, 实参 tuple, 关键字 dict|NULL]`
                    // （`f(*a)` 的发射里第二个 `PUSH_NULL` 就是"没有关键字"那一格）。
                    let keyword_source = frame.get().pop()?;
                    let argument_source = frame.get().pop()?;
                    let self_or_null = frame.get().pop()?;
                    let callable = frame.get().pop()?;
                    // SAFETY: 都在帧值栈上（刚出栈），存活。
                    let null = instance.singletons().null();
                    let bound_self = if self_or_null == null {
                        None
                    } else {
                        // SAFETY: 同上。
                        unsafe { instance.incref_object(self_or_null.as_ptr()) };
                        Some(self_or_null)
                    };
                    release(instance, self_or_null);

                    // SAFETY: 类型身份检查在下面。
                    let argument_type = unsafe { argument_source.as_ref() }.ty();
                    // **实参可以是任意可迭代** ✓（第 149 轮实测参照：`f(*[1, 2])`／`f(*(i for i in (1, 2)))`
                    //   都行 ✓）⇒ `tuple` 走快路 ✓，其余按**迭代协议**摊开 ✓（`iterable_items` 是
                    //   **一处真相** ✓，且它返回的是**借用** ⇒ 每项先 `retain` ✓ —— 第 145 轮的教训 ✓）。
                    let mut args: Vec<NonNull<Header>> = Vec::new();
                    // **位置实参为空时参照传的是 `NULL`**（第 149 轮：`f(1, **kw)` ⇒ `LOAD f; LOAD_CONST 1;
                    //   BUILD_MAP 1; …; CALL_FUNCTION_EX` ✓，其中实参那格是 `NULL` ✗ 不是空元组 ✓）
                    // ⇒ 先认下这一种，再走后面的通用路 ✓。
                    if argument_source == null {
                        // 空实参 ⇒ 什么也不用做 ✓（`args` 已是空表 ✓）
                    } else if argument_type == builtin_type(instance, "tuple") {
                        // SAFETY: 类型身份已确认。
                        let arguments = unsafe { &*argument_source.as_ptr().cast::<TupleObject>() };
                        args.reserve(arguments.len());
                        for index in 0..arguments.len() {
                            let value = arguments.item(index).expect("下标在范围内");
                            // SAFETY: 元素由元组持有。
                            unsafe { instance.incref_object(value.as_ptr()) };
                            args.push(value);
                        }
                    } else if let Some(items) = instance.iterable_items(argument_source) {
                        args.reserve(items.len());
                        for item in items {
                            instance.retain(item);
                            args.push(item);
                        }
                    } else {
                        release(instance, callable);
                        release(instance, argument_source);
                        release(instance, keyword_source);
                        if let Some(bound) = bound_self {
                            release(instance, bound);
                        }
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "argument after * must be an iterable",
                        });
                    }
                    release(instance, argument_source);

                    let mut kwargs: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::new();
                    if keyword_source != null {
                        // SAFETY: 类型身份检查在下面。
                        let keyword_type = unsafe { keyword_source.as_ref() }.ty();
                        if keyword_type != builtin_type(instance, "dict") {
                            for value in args {
                                release(instance, value);
                            }
                            release(instance, callable);
                            release(instance, keyword_source);
                            if let Some(bound) = bound_self {
                                release(instance, bound);
                            }
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "CALL_FUNCTION_EX 的关键字必须是 dict（编译器保证）",
                            });
                        }
                        // SAFETY: 类型身份已确认。
                        let mapping = unsafe { &*keyword_source.as_ptr().cast::<DictObject>() };
                        for (key, value) in mapping.entries() {
                            // SAFETY: 键值由字典持有。
                            unsafe {
                                instance.incref_object(key.as_ptr());
                                instance.incref_object(value.as_ptr());
                            }
                            kwargs.push((key, value));
                        }
                    }
                    release(instance, keyword_source);

                    let result =
                        call_callable(instance, callable, bound_self, args, kwargs, opcode_number)?;
                    frame.get().push(result)?;
                }
                "CALL_INTRINSIC_1" => {
                    // 实测净 0（就地把 TOS 换掉）；oparg 是**内建表的编号**，按名字分派（`BC-50`）。
                    let intrinsic = crate::opcode_metadata::INTRINSIC1_DESCS
                        .get(oparg)
                        .copied()
                        .unwrap_or("INTRINSIC_1_INVALID");
                    match intrinsic {
                        "INTRINSIC_UNARY_POSITIVE" => {
                            // `+x`：本层只接整数／布尔（真协议 `__pos__` 随后补）——原地不动即可
                            let value = frame.get().peek()?;
                            // SAFETY: value 在帧值栈上，存活。
                            let ty = unsafe { value.as_ref() }.ty();
                            // **浮点也要过**（第 318 轮）：`+1.5` 参照给 `1.5` ✓ —— 先前只接
                            // 整数／布尔 ✗ ⇒ `f(+1.5)` 报"只接线了整数／布尔" ✗。
                            if ty != instance.singletons().int_type()
                                && ty != instance.singletons().bool_type()
                                && ty != builtin_type(instance, "float")
                            {
                                // **实例走 `__pos__`**（第 220 轮；`MS-19`：能力缺口必须修 ✓）：
                                // 参照对**没有** `__pos__` 的对象报
                                // `TypeError: bad operand type for unary +: 'C'` ✓；先前这里一律报
                                // "只接线了整数／布尔／浮点"（`Unsupported` ✗）—— 连错误面都对不上 ✗。
                                // 交给 `unary_public` 的同一条 dunder 路 ✓（第 219 轮已经铺好 ✓）。
                                let operand = frame.get().pop()?;
                                let outcome = unary_public(instance, operand, "+", opcode_number);
                                release(instance, operand);
                                let result = outcome?;
                                push(instance, frame.get(), result)?;
                                release(instance, result);
                            }
                        }
                        "INTRINSIC_IMPORT_STAR" => {
                            // `from <模块> import *`：把模块的**公开**名字写进当前命名空间 ✓
                            // **参照口径**（第 413 轮，task-26 ✓）：模块有 `__all__` ⇒ **只**按它导（序列）✓；
                            // 没有 ⇒ 只取不以下划线开头的名字 ✓。
                            // 先前**只**走后者 ✗ ⇒ `from .subprocess import *` 把 `asyncio.subprocess` 里
                            // `import subprocess` 的**stdlib subprocess 模块**也搬进了 `asyncio` 的命名空间 ✗
                            // ⇒ `asyncio/__init__.py` 的 `base_events.__all__ + … + subprocess.__all__`
                            // 变成 **tuple + list** ✗ ⇒ `import asyncio` 一进门就 `TypeError` ✗。
                            // **不动栈**：参照里随后的 `POP_TOP` 才把模块弹掉 ✓（`IMPORT_STAR` 净 0 ✓）
                            let module = frame.get().peek()?;
                            let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "`import *` 需要命名空间帧（模块／类体）",
                            })?;
                            let Some(attributes) = mounted_instance_dict(instance, module) else {
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "`import *` 的对象没有属性字典",
                                });
                            };
                            let mut names: Vec<String> = Vec::new();
                            let exported = instance.dict_get(attributes, "__all__");
                            let mut use_all = false;
                            if let Some(exported) = exported {
                                let exported_type = instance.type_of(exported);
                                let tuple_type = builtin_type(instance, "tuple");
                                let list_type = builtin_type(instance, "list");
                                // **元组与列表分开取** ✗（第 413 轮）：先前把两种类型塞进同一个
                                // `if`，再**只**试 `tuple_items` ✗ ⇒ list 形态（`__all__ = ["x"]` ✓）
                                // 取不到 ⇒ 退回"全部公开名"✗（`from moda import *` 把 `y`／`os` 也搬进来 ✓）。
                                let exported_items: Option<Vec<NonNull<Header>>> = if exported_type == tuple_type {
                                    instance.tuple_items(exported)
                                } else if exported_type == list_type {
                                    instance.list_items(exported)
                                } else {
                                    None
                                };
                                if let Some(items) = exported_items {
                                    for item in items {
                                        match instance.text_of(item) {
                                            Some(text) => names.push(text.to_owned()),
                                            None => {
                                                let kind = instance.type_name(instance.type_of(item));
                                                return Err(instance.raise_builtin_error(
                                                    "TypeError",
                                                    &format!("__all__ must be str, not {kind}"),
                                                ));
                                            }
                                        }
                                    }
                                    use_all = true;
                                }
                            }
                            if !use_all {
                                // SAFETY: attributes 由模块对象持有，存活。
                                let entries =
                                    unsafe { &*attributes.as_ptr().cast::<DictObject>() }.entries();
                                for (key, _) in entries {
                                    let Some(name) = instance.text_of(key).map(|text| text.to_owned())
                                    else {
                                        continue;
                                    };
                                    if name.starts_with('_') {
                                        continue;
                                    }
                                    names.push(name);
                                }
                            }
                            for name in names {
                                let Some(value) = instance.dict_get(attributes, &name) else {
                                    let module_name =
                                        crate::executor::module_text(instance, module, "__name__")
                                            .unwrap_or_else(|| "?".to_owned());
                                    return Err(instance.raise_builtin_error(
                                        "AttributeError",
                                        &format!("module '{module_name}' has no attribute '{name}'"),
                                    ));
                                };
                                // **交一份新引用**：`dict_set` 会接管它（`OM-16` ✓）
                                // SAFETY: value 由模块的属性字典持有，存活。
                                unsafe { instance.incref_object(value.as_ptr()) };
                                instance.dict_set(namespace, &name, value);
                            }
                        }
                        "INTRINSIC_LIST_TO_TUPLE" => {
                            // `(*[1, 2],)`：把 TOS 的列表换成元组（元素各持一份引用）
                            let value = frame.get().pop()?;
                            // SAFETY: value 是刚出栈的存活对象。
                            let ty = unsafe { value.as_ref() }.ty();
                            if ty != builtin_type(instance, "list") {
                                release(instance, value);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "INTRINSIC_LIST_TO_TUPLE 的操作数必须是 list",
                                });
                            }
                            // SAFETY: 类型身份已确认。
                            let list = unsafe { &*value.as_ptr().cast::<ListObject>() };
                            let items = list.items();
                            let mut moved: Vec<NonNull<Header>> = Vec::with_capacity(items.len());
                            for item in items {
                                // SAFETY: 元素由列表持有，这里各新增一份引用交给元组。
                                unsafe { instance.incref_object(item.as_ptr()) };
                                moved.push(item);
                            }
                            release(instance, value);
                            let tuple = instance.new_tuple(moved);
                            frame.get().push(tuple)?;
                        }
                        // **PEP 695 的三件** ✓（task-24 轮 1 ✓；轮 2 的编译器外壳就发它们 ✓）：
                        // - `INTRINSIC_TYPEVAR`（7）：栈顶是**名字串** ✓ ⇒ 换成 `_typing.TypeVar` 实例 ✓；
                        // - `INTRINSIC_SUBSCRIPT_GENERIC`（10）：栈顶是**类型参数元组** ✓ ⇒ 换成
                        //   `Generic[params]` 这个**基类**表达（复用已有 `GenericAlias` ✓ —— 它的
                        //   `__mro_entries__` 已接线 ✓ ⇒ 类会多一个 `typing.Generic` 基 ✓）；
                        // - `INTRINSIC_TYPEALIAS`（11）：**只注册编号** ✓（轮 2 落语义 ✓，如实报未接线 ✓）。
                        "INTRINSIC_TYPEVAR" => {
                            let name = frame.get().pop()?;
                            let Some(typevar_type) = instance.type_named("TypeVar") else {
                                release(instance, name);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "INTRINSIC_TYPEVAR：`_typing.TypeVar` 还没登记",
                                });
                            };
                            let outcome = crate::builtin::typing::typevar_new(typevar_type, &[name], instance);
                            release(instance, name);
                            let value = outcome?;
                            push(instance, frame.get(), value)?;
                            release(instance, value);
                        }
                        "INTRINSIC_SUBSCRIPT_GENERIC" => {
                            let params = frame.get().pop()?;
                            let generic = instance
                                .type_named("Generic")
                                .unwrap_or_else(|| instance.new_attribute_type("Generic"));
                            let base = instance.new_generic_alias(generic.cast(), params);
                            release(instance, params);
                            push(instance, frame.get(), base)?;
                            release(instance, base);
                        }
                        "INTRINSIC_TYPEALIAS" => {
                            // **`type X = …`**（task-24 收口 ③ ✓）：TOS 是三元组
                            // `(名字, 类型参数元组|None, 值 thunk)` ✓（参照 `dis` 实测 ✓）⇒ 造一个
                            // `TypeAliasType` 实例 ✓（`__value__` 惰性 ✓）。
                            let tuple = frame.get().pop()?;
                            // SAFETY: tuple 是帧值栈上的存活对象。
                            let is_tuple = unsafe { tuple.as_ref() }.ty() == builtin_type(instance, "tuple");
                            if !is_tuple {
                                release(instance, tuple);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "INTRINSIC_TYPEALIAS 的实参必须是三元组",
                                });
                            }
                            // SAFETY: 类型身份已确认。
                            let items = unsafe { &*tuple.as_ptr().cast::<TupleObject>() };
                            if items.len() != 3 {
                                release(instance, tuple);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "INTRINSIC_TYPEALIAS 的三元组长度必须是 3",
                                });
                            }
                            // SAFETY: 三项由元组持有，存活（借用视图 ✓）。
                            let (Some(name), Some(params), Some(thunk)) =
                                (items.item(0), items.item(1), items.item(2))
                            else {
                                release(instance, tuple);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "INTRINSIC_TYPEALIAS 的三元组取项失败",
                                });
                            };
                            let outcome =
                                crate::builtin::typing::typealias_from_parts(instance, name, params, thunk);
                            release(instance, tuple);
                            let alias = outcome?;
                            push(instance, frame.get(), alias)?;
                            release(instance, alias);
                        }
                        "INTRINSIC_STOPITERATION_ERROR" => {
                            // 生成器里漏出来的 `StopIteration` 要转成 `RuntimeError`
                            // （实测原话：`generator raised StopIteration`）——用于生成器异常表那条收尾路径。
                            let value = frame.get().peek()?;
                            // SAFETY: value 在帧值栈上，存活。
                            let ty = unsafe { value.as_ref() }.ty();
                            let stop_iteration = instance
                                .type_named("StopIteration")
                                .expect("StopIteration 在异常层次里");
                            if instance.is_subtype(ty, stop_iteration) {
                                release(instance, frame.get().pop()?);
                                let exception = new_exception(
                                    instance,
                                    exception_type(instance, "RuntimeError"),
                                    "generator raised StopIteration",
                                );
                                frame.get().push(exception)?;
                            }
                            // 不是 `StopIteration` 就原样留着（净 0）
                        }
                        "INTRINSIC_1_INVALID" => {
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "CALL_INTRINSIC_1 的 oparg 越界（表里没有这一号）",
                            });
                        }
                        other => {
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: other,
                            });
                        }
                    }
                }
                "CALL_INTRINSIC_2" => {
                    // **PEP 695 的第四件** ✓（task-24 轮 1 ✓）：`INTRINSIC_SET_FUNCTION_TYPE_PARAMS`
                    // （oparg 4 ✓）—— 栈是 `(函数, 类型参数元组)` ✓ ⇒ 给函数挂 `__type_params__` ✓
                    // 再把函数压回去 ✓（参照的 `def f[T]` 外壳正是这么收尾的 ✓）。
                    let intrinsic = crate::opcode_metadata::INTRINSIC2_DESCS
                        .get(oparg)
                        .copied()
                        .unwrap_or("INTRINSIC_2_INVALID");
                    match intrinsic {
                        "INTRINSIC_SET_FUNCTION_TYPE_PARAMS" => {
                            let params = frame.get().pop()?;
                            let function = frame.get().pop()?;
                            // 挂不上就**原样放过** ✓（轮 2 的外壳走的是函数对象 ✓）。
                            let _ = instance.set_attribute_value(function, "__type_params__", params);
                            release(instance, params);
                            push(instance, frame.get(), function)?;
                            release(instance, function);
                        }
                        // **PEP 695 的 bound**（task-24 收口 ①）：栈 `(名字, thunk)` ⇒ 造 TypeVar 并挂惰性 bound ✓。
                        "INTRINSIC_TYPEVAR_WITH_BOUND" => {
                            let thunk = frame.get().pop()?;
                            let name = frame.get().pop()?;
                            let Some(typevar_type) = instance.type_named("TypeVar") else {
                                release(instance, thunk);
                                release(instance, name);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "INTRINSIC_TYPEVAR_WITH_BOUND：`_typing.TypeVar` 还没登记",
                                });
                            };
                            let outcome = crate::builtin::typing::typevar_with_bound(
                                instance, typevar_type, name, thunk,
                            );
                            release(instance, thunk);
                            release(instance, name);
                            let value = outcome?;
                            push(instance, frame.get(), value)?;
                            release(instance, value);
                        }
                        // **PEP 695 的 default**（同上）：栈 `(类型参数, thunk)` ⇒ 挂惰性 default，
                        // 并把那个类型参数原样交回 ✓（参照紧跟 `COPY 1` ✓）。
                        "INTRINSIC_SET_TYPEPARAM_DEFAULT" => {
                            let thunk = frame.get().pop()?;
                            let typevar = frame.get().pop()?;
                            let outcome =
                                crate::builtin::typing::typevar_set_default(instance, typevar, thunk);
                            release(instance, thunk);
                            let value = outcome?;
                            release(instance, typevar);
                            push(instance, frame.get(), value)?;
                            release(instance, value);
                        }
                        other => {
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: other,
                            });
                        }
                    }
                }
                "LOAD_BUILD_CLASS" => {
                    // 实测：`LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <类体>; MAKE_FUNCTION; …`
                    let Some(build_class) = instance.build_class() else {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "本实例没有 __build_class__（引导期未建？）",
                        });
                    };
                    push(instance, frame.get(), build_class)?;
                }
                "LOAD_SPECIAL" => {
                    // **`with` 协议的第一步**（3.14 的发射骨架实测）：
                    //   `LOAD_FAST_BORROW ctx; COPY; LOAD_SPECIAL __exit__; SWAP 2; SWAP 3;
                    //    LOAD_SPECIAL __enter__; CALL 0; …`
                    // 净栈效应 **+1**：**弹出对象、压入 (可调用, self)**，`self` 在 TOS
                    // ——`CALL` 一贯的栈形状是 `[可调用, NULL|self, 实参…]`（`PUSH_NULL` 排在可调用
                    // **之后**），所以这里必须"可调用在下、self 在上"，紧随其后的 `CALL` 才取得对
                    // （`__exit__` 那一份留在栈上，给正常出口与异常出口各用一次）。
                    let name = crate::opcode::get_special_method_names()
                        .get(oparg as usize)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "LOAD_SPECIAL 的下标不在特殊方法表里",
                        })?;
                    let object = frame.get().pop()?;
                    match attribute_lookup(instance, object, name) {
                        Ok(Attribute::Method { function, this }) => {
                            // 两者都是**借用**：压栈会各自 incref
                            push(instance, frame.get(), function)?;
                            push(instance, frame.get(), this)?;
                            release(instance, object);
                        }
                        Ok(Attribute::Value(value)) => {
                            push(instance, frame.get(), value)?;
                            push(instance, frame.get(), object)?;
                            release(instance, object);
                        }
                        Ok(Attribute::Owned(value)) => {
                            push(instance, frame.get(), value)?;
                            push(instance, frame.get(), object)?;
                            release(instance, object);
                            release(instance, value);
                        }
                        Err(error) => {
                            release(instance, object);
                            return Err(error);
                        }
                    }
                }
                "PUSH_NULL" => {
                    // CALL 的"没有 self"槽位（参照实现在栈上放 NULL 指针，这里放内部哨兵）
                    push(instance, frame.get(), instance.singletons().null())?;
                }
                "MAKE_FUNCTION" => {
                    // 实测：MAKE_FUNCTION **只**吃 code 对象；默认值随后由 SET_FUNCTION_ATTRIBUTE 挂
                    let code_header = frame.get().pop()?;
                    // SAFETY: code_header 是本实例的存活对象（由常量表持有）。
                    let code_type_ok = unsafe { code_header.as_ref() }.ty();
                    let code_object_type = instance
                        .type_named("CodeObject")
                        .map(|ty| ty)
                        .unwrap_or_else(|| builtin_type(instance, "object"));
                    if code_type_ok != code_object_type {
                        release(instance, code_header);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "MAKE_FUNCTION 只接受 code object（闭包与注解随后补）",
                        });
                    }
                    // **`__globals__`**：函数"记住"定义处的全局映射（`BC-57` 的 `LOAD_GLOBAL` 要它）。
                    // 模块体没有单独的一层 ⇒ 取命名空间（`effective_globals`）。
                    let captured = frame.get().effective_globals();
                    if let Some(mapping) = captured {
                        // SAFETY: 映射由帧持有，函数要自己那份。
                        unsafe { instance.incref_object(mapping.as_ptr()) };
                    }
                    let object = instance.alloc(FunctionObject::new(
                        builtin_type(instance, "function"),
                        code_header,
                        Vec::new(),
                        None,
                        RefCell::new(captured),
                        RefCell::new(Vec::new()),
                        RefCell::new(None),
                core::cell::RefCell::new(None)));
                    frame.get().push(object.into_raw().cast::<Header>())?;
                }
                "SET_FUNCTION_ATTRIBUTE" => {
                    // 实测：栈是 [属性值, 函数]，**函数在 TOS**；挂完把函数留在栈上
                    let function = frame.get().pop()?;
                    let attribute = frame.get().pop()?;
                    // SAFETY: function 是帧值栈上的存活对象。
                    if unsafe { function.as_ref() }.ty() != builtin_type(instance, "function") {
                        release(instance, attribute);
                        release(instance, function);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "SET_FUNCTION_ATTRIBUTE 只接线了函数对象",
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let object = unsafe { &mut *function.as_ptr().cast::<FunctionObject>() };
                    match oparg {
                        1 => {
                            // defaults：一个 tuple（实测）
                            let items = sequence_items(instance, attribute, opcode_number);
                            release(instance, attribute);
                            object.set_defaults(items?);
                        }
                        2 => {
                            // kwdefaults：一个 dict（实测）
                            if unsafe { attribute.as_ref() }.ty() != builtin_type(instance, "dict") {
                                release(instance, attribute);
                                release(instance, function);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "kwdefaults 必须是 dict",
                                });
                            }
                            if let Some(old) = object.set_kwdefaults(Some(attribute)) {
                                release(instance, old);
                            }
                        }
                        16 => {
                            // **bit4 `annotate`**（3.14 的延迟注解协议，`SPEC-bytecode.md` 的属性位表）：
                            // 值是那个"按 `format` 产出注解字典"的**可调用对象**，挂在函数上
                            if let Some(old) = object.set_annotate(Some(attribute)) {
                                release(instance, old);
                            }
                        }
                        8 => {
                            // **bit3 `closure`**：值是 **cell 元组**（`BUILD_TUPLE n` 造的），
                            // 建帧时装进自由槽（`SPEC-bytecode.md` 的属性位表）
                            let items = sequence_items(instance, attribute, opcode_number);
                            release(instance, attribute);
                            match items {
                                Ok(items) => {
                                    for value in object.set_closure(items) {
                                        release(instance, value);
                                    }
                                }
                                Err(error) => {
                                    release(instance, function);
                                    return Err(error);
                                }
                            }
                        }
                        _ => {
                            release(instance, attribute);
                            release(instance, function);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "SET_FUNCTION_ATTRIBUTE 只接线了 defaults(1)／kwdefaults(2)／closure(8)／annotate(16)",
                            });
                        }
                    }
                    frame.get().push(function)?;
                }
                "CALL" | "CALL_KW" => {
                    // 实测：`[可调用, NULL|self, 位置实参…]`；`CALL_KW` 另把**关键字名元组**放在 TOS
                    let names = if name == "CALL_KW" {
                        Some(frame.get().pop()?)
                    } else {
                        None
                    };
                    let keyword_count = match names {
                        Some(names) => {
                            // SAFETY: names 是帧值栈上的存活对象。
                            if unsafe { names.as_ref() }.ty() != builtin_type(instance, "tuple") {
                                release(instance, names);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "CALL_KW 的关键字名表必须是 tuple",
                                });
                            }
                            // SAFETY: 类型身份已确认。
                            unsafe { &*names.as_ptr().cast::<TupleObject>() }.len()
                        }
                        None => 0,
                    };

                    let mut keywords = Vec::with_capacity(keyword_count);
                    for _ in 0..keyword_count {
                        keywords.push(frame.get().pop()?);
                    }
                    keywords.reverse();

                    let positional_count = oparg.saturating_sub(keyword_count);
                    let mut args = Vec::with_capacity(positional_count);
                    for _ in 0..positional_count {
                        args.push(frame.get().pop()?);
                    }
                    args.reverse();

                    let self_or_null = frame.get().pop()?;
                    let callable = frame.get().pop()?;

                    let bound_self = if self_or_null == instance.singletons().null() {
                        release(instance, self_or_null);
                        None
                    } else {
                        Some(self_or_null)
                    };

                    let mut kwargs = Vec::with_capacity(keyword_count);
                    if let Some(names) = names {
                        // SAFETY: 上面确认过它是 tuple。
                        let table = unsafe { &*names.as_ptr().cast::<TupleObject>() };
                        for (index, value) in keywords.into_iter().enumerate() {
                            let key = match table.item(index) {
                                Some(key) => key,
                                None => {
                                    release(instance, value);
                                    release(instance, names);
                                    release(instance, callable);
                                    for value in args {
                                        release(instance, value);
                                    }
                                    if let Some(self_object) = bound_self {
                                        release(instance, self_object);
                                    }
                                    for (key, value) in kwargs {
                                        release(instance, key);
                                        release(instance, value);
                                    }
                                    return Err(ExecError::Unsupported {
                                        opcode: opcode_number,
                                        what: "CALL_KW 的关键字个数与名表长度不符",
                                    });
                                }
                            };
                            // SAFETY: key 由元组持有，存活。
                            unsafe { instance.incref_object(key.as_ptr()) };
                            kwargs.push((key, value));
                        }
                        release(instance, names);
                    }

                    // **装饰器形态的 `self_or_null` 槽**（`@p.setter`／`@p.deleter`、`@classmethod` 一族 ✓）：
                    // `@dec def f` 编译成 `[dec, 被装饰函数] CALL 0` ✓ —— 参照语义是"这一格并入**位置实参表**" ✓。
                    // 而本层 `call_callable` 对**绑定方法**用的是它**自己的** `this` 并**忽略**这一格 ✗
                    // （那是内部调用的绑定约定 ✓，不能全局改 ✗ —— 实测会导致 `import re`／`import json` 回归 ✗）。
                    // ⇒ 只在 `CALL` 这一处、只对 `method` 并入 ✓（内部 `call_callable` 调用点不受影响 ✓）。
                    let mut args = args;
                    let mut bound_self = bound_self;
                    if let Some(self_object) = bound_self {
                        // SAFETY: callable 是帧值栈上的存活对象。
                        if unsafe { callable.as_ref() }.ty() == builtin_type(instance, "method") {
                            args.insert(0, self_object);
                            bound_self = None;
                        }
                    }
                    let result = call_callable(
                        instance,
                        callable,
                        bound_self,
                        args,
                        kwargs,
                        opcode_number,
                    );
                    release(instance, callable);
                    frame.get().push(result?)?;
                }
        _ => unreachable!("step_calls 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
