//! `executor` 的 `step_operators` 域：运算符（一元／二元／比较／`in`）。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_operators<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "TO_BOOL" | "UNARY_NOT" => {
                    let value = frame.get().pop()?;
                    let truth = truthiness(instance, value, opcode_number)?;
                    release(instance, value);
                    let truth = if name == "UNARY_NOT" { !truth } else { truth };
                    let raw = instance.singletons().boolean(truth);
                    push(instance, frame.get(), raw)?;
                }
                "UNARY_NEGATIVE" | "UNARY_INVERT" => {
                    let value = frame.get().pop()?;
                    let symbol = if name == "UNARY_NEGATIVE" { "-" } else { "~" };
                    let result = unary_public(instance, value, symbol, opcode_number);
                    release(instance, value);
                    frame.get().push(result?)?;
                }
                "CONTAINS_OP" => {
                    // 实测：`x in c` 的栈是 `[x, c]`（容器在 TOS）；`oparg` 0 ＝ `in`、1 ＝ `not in`
                    // （`dis` 的 argrepr 就是这两个词）。
                    let container = frame.get().pop()?;
                    let item = frame.get().pop()?;
                    let found = contains(instance, container, item, opcode_number)?;
                    let truth = if oparg & 1 != 0 { !found } else { found };
                    release(instance, container);
                    release(instance, item);
                    let raw = instance.singletons().boolean(truth);
                    push(instance, frame.get(), raw)?;
                }
                "IS_OP" => {
                    let right = frame.get().pop()?;
                    let left = frame.get().pop()?;
                    // OM-39：`is` 就是对象身份——栈上放的是真对象，直接比指针
                    let identical = left == right;
                    release(instance, left);
                    release(instance, right);
                    let truth = if oparg == 0 { identical } else { !identical };
                    let raw = instance.singletons().boolean(truth);
                    push(instance, frame.get(), raw)?;
                }
                "COMPARE_OP" => {
                    let right = frame.get().pop()?;
                    let left = frame.get().pop()?;
                    // **通用比较**（`TS-40`）：`int`／`bool`／`str` 按值；其余报实测的 `TypeError`。
                    // 整数路径与从前一致（同一套 `partial_cmp`），只是不再把非整数当成"未接线"。

                    // `BC-39`／`BC-58`：cmp 下标 ＝ `oparg >> 5`；bit 4（`& 16`）是 `bool(...)` 标志，
                    // 低 4 位是参照实现的编译期信息（`dis` 不读、本层**不解释**但**必须容受**）
                    let operator = opcode::get_cmp_op().get(oparg >> 5).copied().ok_or(
                        ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "COMPARE_OP 的 cmp 下标（oparg >> 5）超出 cmp_op 的六元组（BC-39／BC-58）",
                        },
                    )?;
                    let truth = compare_public(instance, left, right, operator, opcode_number);
                    release(instance, left);
                    release(instance, right);
                    let raw = instance.singletons().boolean(truth?);
                    push(instance, frame.get(), raw)?;
                }
                "BINARY_OP" => {
                    let right = frame.get().pop()?;
                    let left = frame.get().pop()?;

                    // BC-39：oparg 对应 `get_nb_ops()` 的顺序；BC-50：名字从表里取，不写死
                    let name = opcode::get_nb_ops()
                        .get(oparg)
                        .map(|(name, _)| *name)
                        .ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "BINARY_OP 的 oparg 超出 get_nb_ops() 的范围（BC-39）",
                        })?;

                    if name == "NB_SUBSCR" {
                        // 下标读：`[容器, 键]`（实测）
                        let result = subscript_get(instance, left, right, opcode_number);
                        release(instance, left);
                        release(instance, right);
                        frame.get().push(result?)?;
                    } else {
                        // **一处真相**：二元运算走公开入口（`P1-11` 的任意精度核心 ＋ 参照实测的消息），
                        // 不再在这里用 `i64` ＋ "结果必须落在单例区间"那套旧假设
                        let result = match name {
                            // `+` 走 concat：`str`／`bytes`／`list`／`tuple` 拼接，其余落到算术
                            "NB_ADD" => concat_public(instance, left, right, opcode_number),
                            "NB_SUBTRACT" => arithmetic_public(instance, left, right, "-", opcode_number),
                            "NB_MULTIPLY" => arithmetic_public(instance, left, right, "*", opcode_number),
                            "NB_FLOOR_DIVIDE" => {
                                arithmetic_public(instance, left, right, "//", opcode_number)
                            }
                            "NB_REMAINDER" => arithmetic_public(instance, left, right, "%", opcode_number),
                            // 真除法：结果是 **float**（`arithmetic_public` 的 `/` 分支）
                            "NB_TRUE_DIVIDE" => {
                                arithmetic_public(instance, left, right, "/", opcode_number)
                            }
                            "NB_POWER" => arithmetic_public(instance, left, right, "**", opcode_number),
                            "NB_AND" => arithmetic_public(instance, left, right, "&", opcode_number),
                            // **`PEP 604`**：`X | None`／`None | X`／union 再 `|` ⇒ union（参照口径）；
                            // 不是类型/union 语境（`1 | 2`、`int | 5`）⇒ 落回算术（`TypeError` 消息照旧）。
                            "NB_OR" => match pep604_union_or(instance, left, right) {
                                Some(union) => Ok(union),
                                None => arithmetic_public(instance, left, right, "|", opcode_number),
                            },
                            "NB_XOR" => arithmetic_public(instance, left, right, "^", opcode_number),
                            // `@`（矩阵乘）：`arithmetic_public` 对 `@` 一律如实报参照实测的 `TypeError`
                            "NB_MATRIX_MULTIPLY" => {
                                arithmetic_public(instance, left, right, "@", opcode_number)
                            }
                            "NB_LSHIFT" => arithmetic_public(instance, left, right, "<<", opcode_number),
                            "NB_RSHIFT" => arithmetic_public(instance, left, right, ">>", opcode_number),
                            // **增强赋值**（`+=` 一族，`NB_INPLACE_*`）：不可变类型（`int`／`bool`／
                            // `float`／`str`／`bytes`／`tuple`）的"就地"就是基运算 + 重新绑定 ⇒ 直接
                            // 落基运算；**可变容器**里 `list` 的 `+=` 是**就地 extend**（别名可见，
                            // 实测）⇒ 单独走；`set`／`dict` 的就地运算**尚未接线**（如实报，不悄悄
                            // 换成"重新绑定"——那会与参照的可观察行为不同）
                            "NB_INPLACE_ADD" => inplace_add(instance, left, right, opcode_number),
                            "NB_INPLACE_SUBTRACT" => {
                                inplace_arithmetic(instance, left, right, "-", opcode_number)
                            }
                            "NB_INPLACE_MULTIPLY" => {
                                inplace_arithmetic(instance, left, right, "*", opcode_number)
                            }
                            "NB_INPLACE_TRUE_DIVIDE" => {
                                inplace_arithmetic(instance, left, right, "/", opcode_number)
                            }
                            "NB_INPLACE_FLOOR_DIVIDE" => {
                                inplace_arithmetic(instance, left, right, "//", opcode_number)
                            }
                            "NB_INPLACE_REMAINDER" => {
                                inplace_arithmetic(instance, left, right, "%", opcode_number)
                            }
                            "NB_INPLACE_POWER" => {
                                inplace_arithmetic(instance, left, right, "**", opcode_number)
                            }
                            "NB_INPLACE_LSHIFT" => {
                                inplace_arithmetic(instance, left, right, "<<", opcode_number)
                            }
                            "NB_INPLACE_RSHIFT" => {
                                inplace_arithmetic(instance, left, right, ">>", opcode_number)
                            }
                            "NB_INPLACE_AND" => {
                                inplace_arithmetic(instance, left, right, "&", opcode_number)
                            }
                            "NB_INPLACE_XOR" => {
                                inplace_arithmetic(instance, left, right, "^", opcode_number)
                            }
                            "NB_INPLACE_OR" => {
                                inplace_arithmetic(instance, left, right, "|", opcode_number)
                            }
                            "NB_INPLACE_MATRIX_MULTIPLY" => {
                                inplace_arithmetic(instance, left, right, "@", opcode_number)
                            }
                            _ => Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "该 NB_* 运算尚未接线（矩阵乘 `@`／`@=` 随后补）",
                            }),
                        };
                        release(instance, left);
                        release(instance, right);
                        frame.get().push(result?)?;
                    }
                }
        _ => unreachable!("step_operators 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
