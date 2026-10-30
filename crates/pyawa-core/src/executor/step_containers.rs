//! `executor` 的 `step_containers` 域：容器构造、解包与下标写入。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_containers<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, code: &CodeObject, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "BUILD_TUPLE" | "BUILD_LIST" | "BUILD_SET" => {
                    // oparg 是元素个数；压栈顺序就是元素顺序（先压的在前）
                    let mut items = Vec::with_capacity(oparg);
                    for _ in 0..oparg {
                        items.push(frame.get().pop()?);
                    }
                    items.reverse();

                    // **句首诊断** ✓（第 670 轮，门控 `PYAWA_TUPLE_DEBUG=1`）：建出来的元组**前两项**
                    // 是什么、在哪个代码对象的哪个站点 ✓ —— 用来逮"栈上残留"型错误 ✓
                    //（实测 `re` 里 `(min, max, item)` 建成了 `(len, len, item)` ✗ ⇒ 前两项被上一条
                    // `len(this)` 的残留顶掉 ✓ 的形状 ✓）。
                    if crate::diag::flag("PYAWA_TUPLE_DEBUG") {
                        let show = |item: &NonNull<Header>| {
                            let ty = instance.type_name(instance.type_of(*item));
                            if ty == "builtin_function_or_method" {
                                instance.object_repr(*item).unwrap_or_else(|_| ty.clone())
                            } else {
                                ty
                            }
                        };
                        let head: Vec<String> = items.iter().take(2).map(show).collect();
                        eprintln!(
                            "[tuple] name={} 码元={} 站点={} 个数={} 前两项={:?}",
                            code.name(),
                            frame.get().instruction_pointer() / 2,
                            instance.current_site(),
                            oparg,
                            head
                        );
                    }

                    match name {
                        "BUILD_TUPLE" => {
                            // **OM-23**：空元组是单例 ⇒ 必须走 `new_tuple`（`items` 为空时它给单例）
                            let tuple = instance.new_tuple(items);
                            frame.get().push(tuple)?;
                        }
                        "BUILD_LIST" => push_container(
                            instance,
                            frame.get(),
                            ListObject::new(builtin_type(instance, "list"), RefCell::new(items)),
                        )?,
                        _ => {
                            // set：按**值相等**查重，保留**先出现**的那个（与参照实现一致）
                            let set = instance.alloc(SetObject::new(
                                builtin_type(instance, "set"),
                                RefCell::new(Vec::new()),
                            ));
                            for item in items {
                                let duplicate = set
                                    .get()
                                    .items()
                                    .iter()
                                    .any(|existing| values_equal(instance, *existing, item));
                                if duplicate {
                                    release(instance, item);
                                } else {
                                    set.get().insert_raw(item);
                                }
                            }
                            frame.get().push(set.into_raw().cast::<Header>())?;
                        }
                    }
                }
                "BUILD_SLICE" => {
                    // `a[b:c:d]`（三段，至少一段非常量时参照发这条）：栈序是 lower, upper[, step]
                    if !(2..=3).contains(&oparg) {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "BUILD_SLICE 的 oparg 只能是 2 或 3（参照实测）",
                        });
                    }
                    let mut arguments = Vec::with_capacity(oparg);
                    for _ in 0..oparg {
                        arguments.push(frame.get().pop()?);
                    }
                    arguments.reverse();
                    let slice = build_slice(instance, &arguments, opcode_number)?;
                    frame.get().push(slice)?;
                }
                "BINARY_SLICE" => {
                    // `a[b:c]`（两段，至少一段非常量时参照发这条）：栈序是 container, lower, upper
                    let upper = frame.get().pop()?;
                    let lower = frame.get().pop()?;
                    let container = frame.get().pop()?;
                    let slice = build_slice(instance, &[lower, upper], opcode_number)?;
                    let result = subscript_get(instance, container, slice, opcode_number);
                    release(instance, container);
                    release(instance, slice);
                    frame.get().push(result?)?;
                }
                "BUILD_MAP" => {
                    // 压栈顺序是 key1 value1 key2 value2 …（实测），弹出后反转成对
                    let mut items = Vec::with_capacity(oparg * 2);
                    for _ in 0..oparg * 2 {
                        items.push(frame.get().pop()?);
                    }
                    items.reverse();

                    let dict = instance.alloc(DictObject::new(
                        builtin_type(instance, "dict"),
                        RefCell::new(Vec::new()),
                    ));
                    for pair in items.chunks(2) {
                        let (key, value) = (pair[0], pair[1]);
                        // 键按**值相等**查重：命中则**保留先出现的键**、替换值
                        let position = dict
                            .get()
                            .entries()
                            .iter()
                            .position(|(existing, _)| values_equal(instance, *existing, key));
                        match position {
                            Some(slot) => {
                                if let Some(old) = dict.get().replace_value(slot, value) {
                                    release(instance, old);
                                }
                                release(instance, key);
                            }
                            None => dict.get().insert_raw(key, value),
                        }
                    }
                    frame.get().push(dict.into_raw().cast::<Header>())?;
                }
                "BUILD_STRING" => {
                    let mut parts = Vec::with_capacity(oparg);
                    for _ in 0..oparg {
                        parts.push(frame.get().pop()?);
                    }
                    parts.reverse();

                    let str_type = instance.singletons().str_type();
                    // P3-38 S2a-2 批 2：f-string 的拼接在 **WTF-8 字节层**做 ✓（合法 UTF-8 输出
                    // 与旧口径逐字节一致；孤立代理段也能拼 ✓）。
                    let mut bytes: Vec<u8> = Vec::new();
                    let mut wrong_type = false;
                    for part in parts {
                        // SAFETY: part 是刚出栈的存活对象。
                        if unsafe { part.as_ref() }.ty() == str_type {
                            // SAFETY: 类型身份已确认。
                            bytes.extend_from_slice(
                                unsafe { &*part.as_ptr().cast::<StrObject>() }.wtf8_bytes(),
                            );
                        } else {
                            wrong_type = true;
                        }
                        release(instance, part);
                    }
                    if wrong_type {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "BUILD_STRING 只接线了 str",
                        });
                    }
                    if bytes.is_empty() {
                        // OM-23：空串走单例
                        push(instance, frame.get(), instance.singletons().empty_str())?;
                    } else {
                        push_container(
                            instance,
                            frame.get(),
                            StrObject::new_wtf8(str_type, bytes)
                                .expect("f-string 各段都是合法 WTF-8"),
                        )?;
                    }
                }
                "UNPACK_SEQUENCE" | "UNPACK_EX" => {
                    let raw = frame.get().pop()?;
                    if crate::diag::flag("PYAWA_UNPACK_DEBUG") {
                        // SAFETY: raw 由本帧值栈持有，存活。
                        let tn = unsafe { (&*raw.as_ptr()).ty().as_ref() }.name().to_owned();
                        eprintln!("[unpack_site] opcode={opcode_number} raw_type={tn} site={}", instance.current_site());
                    }
                    let items = sequence_items(instance, raw, opcode_number);
                    release(instance, raw);
                    let items = items?;

                    if name == "UNPACK_SEQUENCE" {
                        if items.len() != oparg {
                            for item in items.iter().copied() {
                                release(instance, item);
                            }
                            let message = if items.len() < oparg {
                                format!(
                                    "not enough values to unpack (expected {oparg}, got {})",
                                    items.len()
                                )
                            } else {
                                format!(
                                    "too many values to unpack (expected {oparg})"
                                )
                            };
                            return Err(raise_builtin(instance, "ValueError", &message));
                        }
                        // 参照实现把元素**从右往左**压栈 ⇒ 最左边的目标拿到 TOS
                        for item in items.into_iter().rev() {
                            frame.get().push(item)?;
                        }
                    } else {
                        // BC-38：UNPACK_EX 的 oparg ＝ 前者个数 ｜ 后者个数 << 8
                        let before = oparg & 0xFF;
                        let after = oparg >> 8;
                        if items.len() < before + after {
                            for item in items.iter().copied() {
                                release(instance, item);
                            }
                            let message = format!(
                                "not enough values to unpack (expected at least {}, got {})",
                                before + after,
                                items.len()
                            );
                            return Err(raise_builtin(instance, "ValueError", &message));
                        }
                        let total = items.len();
                        let middle = items[before..total - after].to_vec();
                        let middle_list = instance.alloc(ListObject::new(
                            builtin_type(instance, "list"),
                            RefCell::new(middle),
                        ));
                        let middle_raw = middle_list.into_raw().cast::<Header>();

                        for item in items[total - after..].iter().rev() {
                            frame.get().push(*item)?;
                        }
                        frame.get().push(middle_raw)?;
                        for item in items[..before].iter().rev() {
                            frame.get().push(*item)?;
                        }
                    }
                }
                "LIST_APPEND" | "SET_ADD" => {
                    // **第 235 轮实测修正**：真正的栈里 `FOR_ITER` **不弹迭代器**，迭代器夹在容器与
                    // 元素之间 ⇒ `[保存值, 容器, 迭代器, 元素]`。参照的 `PEEK(oparg)` 是"从新栈顶数
                    // 第 oparg 个"（`PEEK(1)` 才是 TOS）⇒ 弹出值之后容器在 `peek_from_top(oparg)`
                    //（此前写成 `oparg - 1`，取到的是迭代器 ⇒ 报"容器在栈上的位置或类型不符"）
                    if oparg == 0 {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "LIST_APPEND／SET_ADD 的 oparg 至少为 1",
                        });
                    }
                    let value = frame.get().pop()?;
                    let container = frame.get().peek_from_top(oparg)?;
                    // SAFETY: container 在帧的值栈上，存活。
                    let ty = unsafe { container.as_ref() }.ty();
                    if name == "LIST_APPEND" && ty == builtin_type(instance, "list") {
                        // SAFETY: 类型身份已确认。
                        unsafe { &*container.as_ptr().cast::<ListObject>() }.append(value);
                    } else if name == "SET_ADD" && ty == builtin_type(instance, "set") {
                        // SAFETY: 同上。
                        let set = unsafe { &*container.as_ptr().cast::<SetObject>() };
                        let duplicate = set
                            .items()
                            .iter()
                            .any(|existing| values_equal(instance, *existing, value));
                        if duplicate {
                            release(instance, value);
                        } else {
                            set.insert_raw(value);
                        }
                    } else {
                        release(instance, value);
                        // **诊断**：把真实栈形状带进报错（此前只报"位置或类型不符"，看不出取错了哪一格）
                        let mut kinds: Vec<String> = Vec::new();
                        for index in 1..=5 {
                            kinds.push(match frame.get().peek_from_top(index) {
                                Ok(raw) => {
                                    let raw_ty = unsafe { raw.as_ref() }.ty();
                                    let mut label = "其它".to_owned();
                                    for candidate in ["list", "tuple", "set", "dict", "int", "str"] {
                                        if raw_ty == builtin_type(instance, candidate) {
                                            label = candidate.to_owned();
                                        }
                                    }
                                    if raw == instance.singletons().none() {
                                        label = "None".to_owned();
                                    }
                                    if raw == instance.singletons().null() {
                                        label = "NULL".to_owned();
                                    }
                                    label
                                }
                                Err(_) => "越界".to_owned(),
                            });
                        }
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: Box::leak(
                                format!(
                                    "{name} 的容器位置／类型不符（oparg {oparg}，栈顶往下 {kinds:?}）"
                                )
                                .into_boxed_str(),
                            ),
                        });
                    }
                }
                "MAP_ADD" => {
                    // **第 235 轮实测修正**：同 `LIST_APPEND`——`[保存值, 容器, 迭代器, 键, 值]`，
                    // 地址从新栈顶数：弹出键值之后容器在 `peek_from_top(oparg)`（此前写成 `oparg - 1`）。
                    if oparg == 0 {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "MAP_ADD 的 oparg 至少为 1",
                        });
                    }
                    let value = frame.get().pop()?;
                    let key = frame.get().pop()?;
                    let container = frame.get().peek_from_top(oparg)?;
                    // SAFETY: container 在帧的值栈上，存活。
                    let ty = unsafe { container.as_ref() }.ty();
                    if ty != builtin_type(instance, "dict") {
                        release(instance, key);
                        release(instance, value);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: Box::leak(
                                format!("MAP_ADD 的容器在栈上的位置或类型不符（opcode {opcode_number}）")
                                    .into_boxed_str(),
                            ),
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let dict = unsafe { &*container.as_ptr().cast::<DictObject>() };
                    let position = dict
                        .entries()
                        .iter()
                        .position(|(existing, _)| values_equal(instance, *existing, key));
                    match position {
                        Some(slot) => {
                            if let Some(old) = dict.replace_value(slot, value) {
                                release(instance, old);
                            }
                            release(instance, key);
                        }
                        None => dict.insert_raw(key, value),
                    }
                }
                "LIST_EXTEND" | "SET_UPDATE" => {
                    // 实测（`[*a, *b]`）：容器在 PEEK(oparg + 1)、源是 TOS ⇒ 弹出源之后容器在 PEEK(oparg)
                    let source = frame.get().pop()?;
                    let container = frame.get().peek_from_top(oparg)?;
                    let items = sequence_items(instance, source, opcode_number);
                    release(instance, source);
                    let items = items?;

                    // SAFETY: container 在帧的值栈上，存活。
                    let ty = unsafe { container.as_ref() }.ty();
                    if name == "LIST_EXTEND" && ty == builtin_type(instance, "list") {
                        // SAFETY: 类型身份已确认。
                        unsafe { &*container.as_ptr().cast::<ListObject>() }.extend(items);
                    } else if name == "SET_UPDATE" && ty == builtin_type(instance, "set") {
                        // SAFETY: 同上。
                        let set = unsafe { &*container.as_ptr().cast::<SetObject>() };
                        for item in items {
                            let duplicate = set
                                .items()
                                .iter()
                                .any(|existing| values_equal(instance, *existing, item));
                            if duplicate {
                                release(instance, item);
                            } else {
                                set.insert_raw(item);
                            }
                        }
                    } else {
                        for item in items {
                            release(instance, item);
                        }
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "容器在栈上的位置或类型不符（指令名见下）",
                        });
                    }
                }
                "GET_LEN" => {
                    // 实测：+1（不弹原对象）
                    let raw = frame.get().peek()?;
                    let length = iterable_length(instance, raw, opcode_number)?;
                    push_int(instance, frame.get(), length as i64)?;
                }
                "STORE_SUBSCR" => {
                    // 实测：`[值, 容器, 键]`，键在 TOS
                    let key = frame.get().pop()?;
                    let container = frame.get().pop()?;
                    let value = frame.get().pop()?;
                    let outcome = subscript_set(instance, container, key, value, opcode_number);
                    release(instance, container);
                    release(instance, key);
                    outcome?;
                }
                "STORE_SLICE" => {
                    // **`x[a:b] = 值`** ✓（第 686 轮接线 ✗；`BC-…` 的净 −4 ✓）：栈序实测为
                    // `[值, 容器, 开始, 结束]`（TOS＝结束 ✓，与 `STORE_SUBSCR` 同族 ✓）。
                    // `textwrap`／`json` 一族都靠它 ✓（先前报"指令 37 尚未接线" ✗）。
                    let stop = frame.get().pop()?;
                    let start = frame.get().pop()?;
                    let container = frame.get().pop()?;
                    let value = frame.get().pop()?;
                    let slice = instance.new_slice(
                        instance.int_value(start),
                        instance.int_value(stop),
                        None,
                    );
                    let outcome = subscript_set(instance, container, slice, value, opcode_number);
                    release(instance, container);
                    release(instance, slice);
                    release(instance, start);
                    release(instance, stop);
                    outcome?;
                }
                "DELETE_SUBSCR" => {
                    // 实测：`[容器, 键]`
                    let key = frame.get().pop()?;
                    let container = frame.get().pop()?;
                    let outcome = subscript_del(instance, container, key, opcode_number);
                    release(instance, container);
                    release(instance, key);
                    outcome?;
                }
        _ => unreachable!("step_containers 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
