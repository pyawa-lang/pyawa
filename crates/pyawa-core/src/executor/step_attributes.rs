//! `executor` 的 `step_attributes` 域：属性读写。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_attributes<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, code: &CodeObject, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "LOAD_ATTR" => {
                    // 实测：名字下标 ＝ `oparg >> 1`，**低位 ＝ 取方法**（`dis` 的 argrepr 显示 `+ NULL|self`）
                    let name = code
                        .name_at(oparg >> 1)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let method_flag = oparg & 1 != 0;
                    let object = frame.get().pop()?;
                    // **点号读也走 `__getattr__` 回退** ✓（第 712 轮 ✗ 修）：与 `getattr(obj, 名)` 同一条
                    // 协议 ✓（`attribute_lookup_with_getattr` 一处真相 ✓）。
                    let found = attribute_lookup_with_getattr(instance, object, &name);
                    match found {
                        Ok(Attribute::Owned(value)) => {
                            // 槽位交出的就是新引用 ⇒ 直接压栈，不再 incref
                            frame.get().push(value)?;
                            if method_flag {
                                push(instance, frame.get(), instance.singletons().null())?;
                            }
                        }
                        Ok(Attribute::Value(value)) => {
                            push(instance, frame.get(), value)?;
                            if method_flag {
                                // 取方法形态对非方法值也要补一个 NULL 槽，好让 CALL 统一处理
                                push(instance, frame.get(), instance.singletons().null())?;
                            }
                        }
                        Ok(Attribute::Method { function, this }) => {
                            if method_flag {
                                // 编译器的取方法位：栈上给"函数 ＋ self"，CALL 直接按 [可调用, self] 处理
                                push(instance, frame.get(), function)?;
                                push(instance, frame.get(), this)?;
                            } else {
                                // `obj.method`（**不调用**）：产出一个**绑定方法对象**
                                // SAFETY: function／this 都还活着（由类型字典与调用方持有）。
                                unsafe {
                                    instance.incref_object(function.as_ptr());
                                    instance.incref_object(this.as_ptr());
                                }
                                let bound = instance.alloc(MethodObject::new(
                                    builtin_type(instance, "method"),
                                    function,
                                    this,
                                ));
                                frame.get().push(bound.into_raw().cast::<Header>())?;
                            }
                        }
                        Err(error) => {
                            release(instance, object);
                            return Err(error);
                        }
                    }
                    release(instance, object);
                }
                "STORE_ATTR" => {
                    // `BC-57`：**只有** `LOAD_GLOBAL`／`LOAD_ATTR`／`LOAD_SUPER_ATTR` 移位——
                    // `STORE_ATTR` 的名字下标就是 `oparg` 本身（实测下标 4 的 `obj.epsilon` 给 4）。
                    // 栈是 `[值, 对象]`（**对象在 TOS**）
                    let name = code
                        .name_at(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    // 弹之前先记深度 ✓（用来看"哪条语句留了东西"✗）。
                    let depth_before = frame.get().stack.borrow().len();
                    // **弹之前的整栈转储** ✓（第 302 轮；独立成块、不动上面那两行 ✓）：看"要弹的两项" ✓。
                    if crate::diag::flag("PYAWA_STORE_ATTR_DEBUG") {
                        let items: Vec<String> = frame
                            .get()
                            .stack
                            .borrow()
                            .iter()
                            .map(|item| {
                                // SAFETY: 栈上每项都由本帧持有，存活。
                                unsafe { (&*item.as_ptr()).ty().as_ref() }.name().to_owned()
                            })
                            .collect();
                        eprintln!("[store_attr_stack] depth={} items=[{}]", depth_before, items.join(","));
                    }
                    let object = frame.get().pop()?;
                    let value = frame.get().pop()?;
                    // **`STORE_ATTR` 的操作数诊断** ✓（第 294 轮，门控 `PYAWA_STORE_ATTR_DEBUG=1`）：
                    // 打印名字与弹出值的**类型名** ✓ —— 用来定位"接收者变成 None/str"的那类问题 ✓（零开销 ✓）。
                    if crate::diag::flag("PYAWA_STORE_ATTR_DEBUG") {
                        // SAFETY: 两个值由本帧值栈持有，存活。
                        let oname = unsafe { (&*object.as_ptr()).ty().as_ref() }.name().to_owned();
                        let vname = unsafe { (&*value.as_ptr()).ty().as_ref() }.name().to_owned();
                        // 顺带把**整个值栈**摊开 ✓（第 300 轮：看"栈上到底堆着什么"✗）。
                        let dump: Vec<String> = frame
                            .get()
                            .stack
                            .borrow()
                            .iter()
                            .map(|item| {
                                // SAFETY: 栈上每项都由本帧持有，存活。
                                unsafe { (&*item.as_ptr()).ty().as_ref() }.name().to_owned()
                            })
                            .collect();
                        eprintln!(
                            "[store_attr] name={name} object_type={oname} value_type={vname} depth_before={depth_before} stack=[{}]",
                            dump.join(",")
                        );
                    }
                    // **点号赋值也走 `__setattr__` 协议** ✓（第 715 轮）：`obj.x = v` 与 `setattr(obj,'x',v)`
                    // 在参照里是**同一条协议** ✓。协议入口内部有道判据：**只有类型自己定义了 `__setattr__`**
                    // 才走协议 ✓，否则整条交给 `instance_attribute_set` ✓（描述符／类型对象／还账一处真相 ✓）
                    // —— 先前我把默认情形也拽进协议 ✗ ⇒ 引用计数多一格 ✗（`tests/attributes.rs` 实测 ✓）。
                    let outcome = instance.set_attribute_with_protocol(object, &name, value);
                    release(instance, object);
                    outcome?;
                }
                "DELETE_ATTR" => {
                    // 实测：名字下标 ＝ `oparg`（**不移位**）；栈是 `[对象]`
                    let name = code
                        .name_at(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let object = frame.get().pop()?;
                    let outcome = instance_attribute_delete(instance, object, &name);
                    release(instance, object);
                    outcome?;
                }
        _ => unreachable!("step_attributes 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
