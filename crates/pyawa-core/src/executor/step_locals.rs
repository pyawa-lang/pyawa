//! `executor` 的 `step_locals` 域：局部／cell／自由变量的读写与 fast 合并形态。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_locals<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, code: &CodeObject, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "COPY_FREE_VARS" => {
                    // **3.11+ 是 no-op**：自由变量在建帧阶段由函数的闭包装入（`Frame::install_closure`）。
                    // 保留这条 arm 是为了**不报 Unsupported**（如实表达"语义已由建帧承担"）。
                    let _ = oparg;
                }
                "MAKE_CELL" => {
                    // 净 0：把 **cell 槽**第 `oparg` 格换成一个新 cell；初值取**同号局部槽**（若有）
                    let slot = oparg as usize;
                    // **把同号局部槽的值"搬"进新 cell**（第 334 轮真 bug 修 ✗）：先前用 `raw_local`
                    // **借用** ✗ ⇒ 同一份引用**两边都算持有** ✓（局部数组一份、cell 一份 ✓）⇒
                    // 帧收尾释放局部那份 ＋ `cell_clear` 释放 cell 那份 ⇒ **同一份引用被减两次** ✗
                    // —— 实测：`import threading` ⇒ `对已释放对象 decref：类型 list` ✓，
                    // 回溯落点正是 `Header::decref ← cell::cell_clear` ✓（上限榜上那一族 73 个模块 ✓）。
                    // 现在把局部那份**取走**（`set_local(slot, None)` 返回旧值 ✓）⇒ 所有权只剩一份 ✓。
                    // 类体的 `nlocals` 是 0 ⇒ 这一步给 `None` ✓（与先前一致 ✓）。
                    let initial = frame.get().set_local(slot, None).unwrap_or(None);
                    let cell_type = instance
                        .type_named("cell")
                        .expect("引导期已登记 cell 类型");
                    let cell = instance
                        .alloc(crate::cell::CellObject::new(cell_type, RefCell::new(initial)))
                        .into_raw()
                        .cast::<Header>();
                    match frame.get().set_cell(slot, Some(cell)) {
                        Ok(Some(old)) => release(instance, old),
                        Ok(None) => {}
                        Err(error) => {
                            release(instance, cell);
                            return Err(ExecError::Frame(error));
                        }
                    }
                }
                "LOAD_LOCALS" => {
                    // 净 +1：压**本帧的命名空间映射**（类体的 `LOAD_LOCALS` 就是取那个 dict）。
                    // 函数帧没有独立命名空间 ⇒ 如实报未接线。
                    match frame.get().namespace() {
                        Some(mapping) => {
                            // SAFETY: mapping 由帧持有，存活。
                            unsafe { instance.incref_object(mapping.as_ptr()) };
                            push(instance, frame.get(), mapping)?;
                            release(instance, mapping);
                        }
                        None => {
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "`LOAD_LOCALS` 只接线了带命名空间的帧（类体）",
                            })
                        }
                    }
                }
                "STORE_DEREF" => {
                    // 净 −1：把 TOS 存进 cell 槽第 `oparg` 格那个 cell（cell 接手一份引用）
                    let value = frame.get().pop()?;
                    let slot = oparg as usize;
                    let cell = match frame.get().cell(slot) {
                        Ok(cell) => cell,
                        Err(error) => {
                            release(instance, value);
                            return Err(ExecError::Frame(error));
                        }
                    };
                    let Some(cell) = cell else {
                        release(instance, value);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "`STORE_DEREF` 的 cell 槽是空的（`MAKE_CELL` 没跑过）",
                        });
                    };
                    // SAFETY: cell 由帧的 cell 槽持有，存活。
                    let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                    // **`replace` 接管 `value` 那份引用** ✓（见 `CellObject::replace` 的契约 ✓）——
                    // 这里**不得**再释放一次 ✗（第 208 轮真 bug 修复 ✗：先前多放一次 ⇒ cell 里留着
                    // **没被记账的指针** ✗ ⇒ 值被提前释放 ⇒ cell 还指着它 ⇒ **释放后重用** ⇒ 堆损坏 ✓）。
                    // 线索来自"**每次释放前扫全图看还有谁指着它**"那把尺子 ✓（引用者全是 `cell` ✓）。
                    if let Some(old) = object.replace(Some(value)) {
                        release(instance, old);
                    }
                }
                "LOAD_DEREF" => {
                    // 净 +1：压 cell 槽第 `oparg` 格那个 cell 的值
                    let value = load_deref_value(instance, frame.get(), code, oparg)?;
                    push(instance, frame.get(), value)?;
                    release(instance, value);
                }
                // **PEP 695 的"类字典优先"两条**（`BC-45` 扩；task-24 外壳）：
                // `LOAD_FROM_DICT_OR_DEREF` —— TOS 是一个**映射**（`LOAD_LOCALS` 压的类命名空间 ✓），
                // 先按 **localsplus 第 `oparg` 格那个 cell 的名字**在里面找 ✓，找不到就取那个 cell 的值 ✓。
                // 参照用它给类体写 `__type_params__`（`.type_params` 是类体的自由变量 ✓，
                // 名字取自 localsplus ✓ —— `dis` 的 argrepr 就是 localsplus 名 ✓）。
                "LOAD_FROM_DICT_OR_DEREF" => {
                    let mapping = frame.get().pop()?;
                    let name = localsplus_name(code, oparg);
                    let found = lookup_in_mapping(instance, mapping, &name);
                    release(instance, mapping);
                    match found {
                        Some(value) => push(instance, frame.get(), value)?,
                        None => {
                            let value =
                                load_deref_value(instance, frame.get(), code, oparg)?;
                            push(instance, frame.get(), value)?;
                            release(instance, value);
                        }
                    }
                }
                // `LOAD_FROM_DICT_OR_GLOBALS`：TOS 是一个**映射**，先在里面按 `co_names[oparg]` 找 ✓，
                // 再退**全局** ✓、再退**内建** ✓（参照的类体注解函数用它做"类字典优先"查找 ✓；
                // 实测注解里的 `int` 是内建 ⇒ 必须退到内建 ✓）。
                "LOAD_FROM_DICT_OR_GLOBALS" => {
                    let mapping = frame.get().pop()?;
                    let name = code
                        .name_at(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "co_names 下标越界",
                        })?
                        .to_owned();
                    let found = lookup_in_mapping(instance, mapping, &name);
                    release(instance, mapping);
                    let found = found
                        .or_else(|| {
                            frame
                                .get()
                                .effective_globals()
                                .and_then(|globals| lookup_in_mapping(instance, globals, &name))
                        })
                        .or_else(|| {
                            instance
                                .builtins()
                                .and_then(|builtins| lookup_in_mapping(instance, builtins, &name))
                        });
                    match found {
                        Some(value) => push(instance, frame.get(), value)?,
                        None => {
                            let message = format!("name '{name}' is not defined");
                            return Err(raise_builtin(instance, "NameError", &message));
                        }
                    }
                }
                "LOAD_CONST" => {
                    let raw = code
                        .constant(oparg)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "常量表下标越界",
                        })?;
                    push(instance, frame.get(), raw)?;
                }
                // BC-51：LOAD_FAST 假定槽位已绑定（编译器保证）；这里宁可报错也不读垃圾
                // `LOAD_FAST_BORROW` 是 3.14 的借用形态：语义与 `LOAD_FAST` 相同（栈上不留新引用）。
                // 本层的值栈一律持有引用，故照常新增一份——**可观察语义一致**，只是少了那点优化。
                "LOAD_FAST" | "LOAD_FAST_CHECK" | "LOAD_FAST_BORROW" => {
                    slot_trace(instance, &code, oparg as usize, "读");                                match frame.get().local(oparg) {
                        Ok(Some(raw)) => push(instance, frame.get(), raw)?,
                        Ok(None) => return Err(unbound_local_error(instance, frame.get(), oparg)),
                        // **cell 在参照实现里也是"快速局部槽"**：类体的 `__classdict__` 只有 cell 槽
                        // （`nlocals` 是 0），而参照收尾用的是 `LOAD_FAST_BORROW 0` 读那个 cell
                        // ⇒ 局部槽越界时回落到**同号 cell 槽**（`BC-45` 的独立 cell 槽模型下的兼容）。
                        Err(crate::FrameError::SlotOutOfRange { .. }) => {
                            let cell = match frame.get().cell(oparg) {
                                Ok(Some(cell)) => cell,
                                _ => return Err(unbound_local_error(instance, frame.get(), oparg)),
                            };
                            // SAFETY: cell 由帧的 cell 槽持有，存活。
                            let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                            let Some(value) = object.value() else {
                                return Err(unbound_local_error(instance, frame.get(), oparg));
                            };
                            // SAFETY: 值由 cell 持有，存活。
                            unsafe { instance.incref_object(value.as_ptr()) };
                            push(instance, frame.get(), value)?;
                            release(instance, value);
                        }
                        Err(error) => return Err(ExecError::Frame(error)),
                    }
                }
                "STORE_FAST" => {
                    slot_trace(instance, &code, oparg as usize, "写");
                    let value = frame.get().pop()?;
                    // **NULL 哨兵＝"未绑定"**（第 234 轮）：推导式的 `LOAD_FAST_AND_CLEAR` 在外层同名局部
                    // **本来就没有**时会压那个哨兵，收尾的 `STORE_FAST` 要把它还原成"清空槽"而不是存一个
                    // NULL 对象（否则推导式之后那个名字会变成 NULL 而不是 `NameError`）
                    let restored = if value == instance.singletons().null() {
                        None
                    } else {
                        Some(value)
                    };
                                    if let Some(old) = frame.get().set_local(oparg, restored)? {
                        release(instance, old);
                    }
                    // **`STORE_FAST` 的槽号诊断** ✓（第 305 轮，门控 `PYAWA_STORE_FAST_DEBUG=1`）：
                    // 与融合加载读到的槽号对照 ✓ ⇒ 定"写入槽 ≠ 读取槽"✓（第 304 轮的铁证 ✓）。
                    if crate::diag::flag("PYAWA_STORE_FAST_DEBUG") {
                        if let Some(v) = restored {
                            // SAFETY: v 由本帧刚写入的槽持有，存活。
                            let tn = unsafe { (&*v.as_ptr()).ty().as_ref() }.name().to_owned();
                            eprintln!("[store_fast] slot={oparg} type={tn}");
                        }
                    }
                }
                "DELETE_FAST" => {
                    match frame.get().set_local(oparg, None)? {
                    Some(old) => release(instance, old),
                    None => return Err(unbound_local_error(instance, frame.get(), oparg)),
                    }
                }
                "LOAD_FAST_AND_CLEAR" => {
                    // **推导式的"变量不外泄"**（第 234 轮实测）：把该局部**原值**压栈（本来没绑定就压
                    // NULL 哨兵），随即**清空**这个槽；推导式收尾的 `STORE_FAST` 再把它还原
                    let saved = frame.get().local(oparg)?;
                    match saved {
                        Some(raw) => push(instance, frame.get(), raw)?,
                        None => push(instance, frame.get(), instance.singletons().null())?,
                    }
                    if let Some(old) = frame.get().set_local(oparg, None)? {
                        release(instance, old);
                    }
                }
                "STORE_FAST_LOAD_FAST" => {
                    // 净 0：`oparg` 打包两个局部槽——**高 4 位收 TOS**、低 4 位**再压回**（实测
                    // `STORE_FAST_LOAD_FAST 0 (x, x)`：先存 `x` 再把同一个槽压回来）
                    let value = frame.get().pop()?;
                    let store_slot = oparg >> 4;
                    let load_slot = oparg & 0x0F;
                    slot_trace(instance, &code, store_slot, "写(融合113高)");
                    slot_trace(instance, &code, load_slot, "读(融合113低)");
                    if frame.get().set_local(store_slot, Some(value)).is_err() {
                        release(instance, value);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "STORE_FAST_LOAD_FAST 的槽位越界",
                        });
                    }
                    let loaded = frame.get().local(load_slot)?.ok_or(ExecError::UnboundLocal {
                        slot: load_slot,
                    })?;
                    push(instance, frame.get(), loaded)?;
                }
                "LOAD_FAST_LOAD_FAST" | "LOAD_FAST_BORROW_LOAD_FAST_BORROW" => {
                    // 实测净 +2：`oparg` 打包两个局部槽，**高 4 位先压**（`dis` 的 argrepr 就是
                    // "(第一个, 第二个)"；`LOAD_FAST_BORROW_LOAD_FAST_BORROW 1 (a, b)` 里 a＝0、b＝1）
                    let first = oparg >> 4;
                    let second = oparg & 0x0F;
                    if crate::diag::flag("PYAWA_EMIT87_DEBUG") {
                        eprintln!(
                            "[run87] name={} 码元={} arg={} 高={} 低={}",
                            code.name(),
                            frame.get().instruction_pointer() / 2,
                            oparg,
                            first,
                            second
                        );
                    }
                    let left = frame.get().local(first)?.ok_or(ExecError::UnboundLocal {
                        slot: first,
                    })?;
                    push(instance, frame.get(), left)?;
                    let right = frame.get().local(second)?.ok_or(ExecError::UnboundLocal {
                        slot: second,
                    })?;
                    push(instance, frame.get(), right)?;
                    // **融合加载的槽号诊断** ✓（第 304 轮，门控 `PYAWA_FUSED_LOAD_DEBUG=1`）：第 302 轮
                    // 已证"错值是这一句自己压的" ✗ ⇒ 这里看它**到底从哪两个槽取的** ✓。
                    if crate::diag::flag("PYAWA_FUSED_LOAD_DEBUG") {
                        // SAFETY: 两个值分别由对应帧槽持有，存活。
                        let lt = unsafe { (&*left.as_ptr()).ty().as_ref() }.name().to_owned();
                        let rt = unsafe { (&*right.as_ptr()).ty().as_ref() }.name().to_owned();
                        eprintln!(
                            "[fused_load] oparg={oparg} first={first} second={second} left={lt} right={rt}"
                        );
                    }
                }
                "STORE_FAST_STORE_FAST" => {
                    // 净 −2：`oparg` 打包两个局部槽——**高 4 位收 TOS**、低 4 位收 TOS1
                    // （实测 `STORE_FAST_STORE_FAST 18 (a, b)` 里 a 是 1、b 是 2，而解包把**第一个**元素压在栈顶）
                    let first = frame.get().pop()?;
                    let second = frame.get().pop()?;
                    let low = oparg & 0x0F;
                    let high = oparg >> 4;
                    if frame.get().set_local(high, Some(first)).is_err()
                        || frame.get().set_local(low, Some(second)).is_err()
                    {
                        release(instance, first);
                        release(instance, second);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "STORE_FAST_STORE_FAST 的槽位越界",
                        });
                    }
                }
        _ => unreachable!("step_locals 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
