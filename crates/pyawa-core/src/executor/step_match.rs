//! `executor` 的 `step_match` 域：`match` 的模式匹配指令。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_match<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "MATCH_SEQUENCE" => {
                    // 净 +1：压"是不是序列"，被测对象留着。实测 `str`／`dict` **不算**序列
                    let subject = frame.get().peek()?;
                    // SAFETY: subject 在帧值栈上，存活。
                    let ty = unsafe { subject.as_ref() }.ty();
                    let truth = ty == builtin_type(instance, "list")
                        || ty == builtin_type(instance, "tuple");
                    let raw = instance.singletons().boolean(truth);
                    push(instance, frame.get(), raw)?;
                }
                "MATCH_MAPPING" => {
                    // 净 +1：本层只有 `dict` 算映射
                    let subject = frame.get().peek()?;
                    // SAFETY: subject 在帧值栈上，存活。
                    let truth = unsafe { subject.as_ref() }.ty() == builtin_type(instance, "dict");
                    let raw = instance.singletons().boolean(truth);
                    push(instance, frame.get(), raw)?;
                }
                "MATCH_KEYS" => {
                    // 净 +1：栈是 `[被测映射, 键的 tuple]`——**两者都留着**，再压"值的 tuple"；
                    // 任一键缺失就压 `None`（实测：缺键 ⇒ 这个 case 不匹配）
                    let keys = frame.get().peek()?;
                    let subject = frame.get().peek_from_top(2)?;
                    // SAFETY: 两个都在帧值栈上，存活。
                    let keys_type = unsafe { keys.as_ref() }.ty();
                    if keys_type != builtin_type(instance, "tuple") {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "MATCH_KEYS 的键必须是 tuple（编译器保证）",
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let key_items = unsafe { &*keys.as_ptr().cast::<TupleObject>() };
                    // SAFETY: subject 是存活对象。
                    let subject_type = unsafe { subject.as_ref() }.ty();
                    if subject_type != builtin_type(instance, "dict") {
                        let none = instance.singletons().none();
                        push(instance, frame.get(), none)?;
                        return Ok(Step::Continue);
                    }
                    // SAFETY: 类型身份已确认。
                    let mapping = unsafe { &*subject.as_ptr().cast::<DictObject>() };
                    let mut values: Vec<NonNull<Header>> = Vec::with_capacity(key_items.len());
                    let mut missing = false;
                    for index in 0..key_items.len() {
                        let key = key_items.item(index).expect("下标在范围内");
                        match mapping
                            .entries()
                            .iter()
                            .position(|(existing, _)| values_equal(instance, *existing, key))
                        {
                            Some(position) => {
                                let (_, value) = mapping.entry(position).expect("刚查到的位置");
                                // SAFETY: value 由字典持有，存活。
                                unsafe { instance.incref_object(value.as_ptr()) };
                                values.push(value);
                            }
                            None => {
                                missing = true;
                                break;
                            }
                        }
                    }
                    if missing {
                        for value in values {
                            release(instance, value);
                        }
                        let none = instance.singletons().none();
                        push(instance, frame.get(), none)?;
                    } else {
                        let tuple = instance.new_tuple(values);
                        frame.get().push(tuple)?;
                    }
                }
                "MATCH_CLASS" => {
                    // 净 −2：栈是 `[被测对象, 类, 关键字名 tuple]`——**被测对象也被吃掉**，
                    // 命中就压"取出的属性 tuple"，不命中就压 `None`（实测形状：
                    // `MATCH_CLASS n; COPY 1; POP_JUMP_IF_NONE L; UNPACK_SEQUENCE …`）。
                    let names = frame.get().pop()?;
                    let class_object = frame.get().pop()?;
                    let subject = frame.get().pop()?;
                    if oparg != 0 {
                        release(instance, names);
                        release(instance, class_object);
                        release(instance, subject);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "MATCH_CLASS 只接线了关键字形参（位置形参随后补）",
                        });
                    }
                    // SAFETY: class_object 是刚出栈的存活对象。
                    let class_type = unsafe { class_object.as_ref() }.ty();
                    if class_type != builtin_type(instance, "type") {
                        release(instance, names);
                        release(instance, class_object);
                        release(instance, subject);
                        return Err(raise_builtin(
                            instance,
                            "TypeError",
                            "called match pattern must be a type",
                        ));
                    }
                    let class = class_object.cast::<TypeObject>();
                    // SAFETY: subject 是存活对象。
                    let subject_type = unsafe { subject.as_ref() }.ty();
                    let matched = instance.is_subtype(subject_type, class);
                    release(instance, class_object);
                    if !matched {
                        release(instance, names);
                        release(instance, subject);
                        let none = instance.singletons().none();
                        push(instance, frame.get(), none)?;
                        return Ok(Step::Continue);
                    }
                    // SAFETY: names 是 tuple（编译器保证）。
                    let name_items = unsafe { &*names.as_ptr().cast::<TupleObject>() };
                    let mut values: Vec<NonNull<Header>> = Vec::with_capacity(name_items.len());
                    for index in 0..name_items.len() {
                        let name = name_items.item(index).expect("下标在范围内");
                        // SAFETY: name 由 tuple 持有，存活。
                        let name_type = unsafe { name.as_ref() }.ty();
                        if name_type != instance.singletons().str_type() {
                            for value in values {
                                release(instance, value);
                            }
                            release(instance, names);
                            release(instance, subject);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "MATCH_CLASS 的关键字名必须是 str",
                            });
                        }
                        // SAFETY: 类型身份已确认。
                        let text = unsafe { &*name.as_ptr().cast::<StrObject>() }.value().to_owned();
                        match attribute_lookup(instance, subject, &text) {
                            Ok(Attribute::Owned(raw)) => values.push(raw),
                            Ok(Attribute::Value(raw)) => {
                                // SAFETY: raw 由类型／实例字典持有，存活。
                                unsafe { instance.incref_object(raw.as_ptr()) };
                                values.push(raw);
                            }
                            Ok(Attribute::Method { .. }) => {
                                // 取到的是**方法**（函数 ＋ self 绑定），本层还没有"绑定方法"对象
                                for value in values {
                                    release(instance, value);
                                }
                                release(instance, names);
                                release(instance, subject);
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "MATCH_CLASS 的属性是方法时要「绑定方法」对象（随后补）",
                                });
                            }
                            Err(error) => {
                                release(instance, names);
                                release(instance, subject);
                                return Err(error);
                            }
                        }
                    }
                    release(instance, names);
                    release(instance, subject);
                    let tuple = instance.new_tuple(values);
                    frame.get().push(tuple)?;
                }
        _ => unreachable!("step_match 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
