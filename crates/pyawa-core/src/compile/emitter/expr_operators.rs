//! `emitter` 的 `expr_operators` 域：运算符与真假/条件表达式（比较、布尔、二元、一元、空安全链）。

use super::*;

impl Emitter {
    pub(crate) fn emit_expression_operators(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
                Expression::NullCoalesce(left, right, span) => {
                    let end = self.new_label();
                    let take_left = self.new_label();
                    self.emit_expression(left)?;
                    // `COPY 1` 复制一份来判，判完**不弹原值**（不是 None 时它就是结果）
                    self.emit_named(*span, "COPY", 1);
                    self.emit_jump(
                        *span,
                        opcode::opcode("POP_JUMP_IF_NOT_NONE").expect("条件跳转在表里"),
                        take_left,
                    );
                    // `None` 分支：丢掉左值、求右值
                    self.emit_named(*span, "POP_TOP", 0);
                    self.emit_expression(right)?;
                    self.emit_jump(*span, opcode::opcode("JUMP_FORWARD").expect("跳转在表里"), end);
                    self.mark_label(take_left);
                    self.mark_label(end);
                    Ok(())
                }
                // **`BC-63` 空安全后缀链**：`receiver?.…`／`receiver?[…]`。
                // 骨架：`<receiver>; COPY 1; POP_JUMP_IF_NOT_NONE 有值; POP_TOP; LOAD_CONST None;
                //        JUMP_FORWARD 结束; 有值: <rest（基底＝栈顶那个值）>; 结束:`。
                // `rest` 里的 [`Expression::ChainSubject`] 是**零发射占位**（值已经压在栈顶）。
                Expression::SafeChain {
                    receiver,
                    rest,
                    span,
                } => {
                    let end = self.new_label();
                    let some = self.new_label();
                    self.emit_expression(receiver)?;
                    self.emit_named(*span, "COPY", 1);
                    self.emit_jump(
                        *span,
                        opcode::opcode("POP_JUMP_IF_NOT_NONE").expect("条件跳转在表里"),
                        some,
                    );
                    // `None` ⇒ 整段后缀链短路：丢掉接收者、结果 `None`、不再求值（含实参）
                    self.emit_named(*span, "POP_TOP", 0);
                    let index = self.intern_constant(Constant::None);
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    self.emit_jump(*span, opcode::opcode("JUMP_FORWARD").expect("跳转在表里"), end);
                    self.mark_label(some);
                    self.emit_expression(rest)?;
                    self.mark_label(end);
                    Ok(())
                }
                // 空安全链的**基底占位**：外层 `SafeChain` 已经把值压在栈顶 ⇒ 这里**什么都不发**。
                Expression::ChainedCompare {
                    operands,
                    operators,
                    span,
                } => {
                    self.emit_expression(&operands[0])?;
                    let takeover = self.chain_takeover.take();
                    let failed = match &takeover {
                        Some((label, _, _, _)) => *label,
                        None => self.new_label(),
                    };
                    for (index, operator) in operators.iter().enumerate() {
                        self.emit_expression(&operands[index + 1])?;
                        let last = index + 1 == operators.len();
                        if !last {
                            // 保住**中间操作数**：`SWAP 2; COPY 2`（实测实参都是 2）
                            self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                            self.emit_at(*span, opcode::opcode("COPY").expect("COPY 在表里"), 2);
                        }
                        // **`is`／`in` 两族在链式比较里也走 `IS_OP`／`CONTAINS_OP`**（第 327 轮）：
                        // 它们**没有** `COMPARE_OP` 的 oparg ✗ ⇒ 先前这里 `.expect(...)` 直接 panic ✗
                        //（实测 `if a == b is c:` ✓ —— 正是上限诊断里 29 个模块那一族的真身 ✓）。
                        // 与 [`Emitter::emit_compare`] 的约定一致：`is`⇒`IS_OP 0`／`is not`⇒`IS_OP 1`／
                        // `in`⇒`CONTAINS_OP 0`／`not in`⇒`CONTAINS_OP 1` ✓。
                        let is_in_family = matches!(
                            operator,
                            crate::compile::CompareOperator::Is
                                | crate::compile::CompareOperator::IsNot
                                | crate::compile::CompareOperator::In
                                | crate::compile::CompareOperator::NotIn
                        );
                        if is_in_family {
                            let (name, oparg) = match operator {
                                crate::compile::CompareOperator::Is => ("IS_OP", 0u8),
                                crate::compile::CompareOperator::IsNot => ("IS_OP", 1),
                                crate::compile::CompareOperator::In => ("CONTAINS_OP", 0),
                                _ => ("CONTAINS_OP", 1),
                            };
                            self.emit_at(*span, opcode::opcode(name).expect("比较指令在表里"), oparg);
                        } else {
                            let base = operator.oparg().expect("六个 `COMPARE_OP` 运算符之一");
                            // 非末段的结果立刻转布尔 ⇒ 不带 `|16`；末段按上下文（实测量到 2 / 18）
                            let oparg = if last && self.in_condition { base | 16 } else { base };
                            self.emit_at(
                                *span,
                                opcode::opcode("COMPARE_OP").expect("COMPARE_OP 在表里"),
                                oparg,
                            );
                        }
                        if !last {
                            self.emit_at(*span, opcode::opcode("COPY").expect("COPY 在表里"), 1);
                            self.emit_at(*span, opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"), 0);
                            self.emit_jump(
                                *span,
                                opcode::opcode("POP_JUMP_IF_FALSE").expect("条件跳转在表里"),
                                failed,
                            );
                            self.emit_at(
                                *span,
                                opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                                0,
                            );
                            self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                        }
                    }
                    // **失败路径**的清理（`SWAP 2; POP_TOP`：丢掉左操作数、留下 `False`）——参照把它
                    // **外提**到语句之后，本层就地在表达式尾发出，因此**成功路径必须跳过它**
                    // （否则成功时栈上只有一个结果，`SWAP 2` 会 `StackUnderflow`；第 262 轮的语料
                    // `chained_compare` 正是这么抓出来的）。
                    // **赋值臂接管时**（第 85 轮）：失败块由 `pending_chain_copies` 在**收尾之后**发出
                    // ⇒ 成功路径直接续下去，**不发** `JUMP_FORWARD`、也不就地发 `SWAP/POP_TOP` ✓
                    if takeover.is_some() {
                        return Ok(());
                    }
                    let done = self.new_label();
                    self.emit_jump(
                        *span,
                        opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                        done,
                    );
                    self.mark_label(failed);
                    self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                    self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                    self.mark_label(done);
                    Ok(())
                }
                // **三元表达式**：`<条件>; TO_BOOL; POP_JUMP_IF_FALSE → else; NOT_TAKEN;
                // <then>; JUMP_FORWARD → end; else: <else>; end:`（语义等价；参照把余部复制进两分支）
                Expression::Conditional {
                    condition,
                    then_value,
                    else_value,
                    span: _,
                } => {
                    let condition_span = condition.span();
                    let else_label = self.new_label();
                    self.emit_expression(condition)?;
                    self.emit_at(
                        condition_span,
                        opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"),
                        0,
                    );
                    self.emit_jump(
                        condition_span,
                        opcode::opcode("POP_JUMP_IF_FALSE").expect("条件跳转在表里"),
                        else_label,
                    );
                    self.emit_at(
                        condition_span,
                        opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                        0,
                    );
                    self.emit_expression(then_value)?;
                    let end_label = self.new_label();
                    self.emit_jump(
                        condition_span,
                        opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                        end_label,
                    );
                    self.mark_label(else_label);
                    self.emit_expression(else_value)?;
                    self.mark_label(end_label);
                    Ok(())
                }
                Expression::Compare(left, operator, right, span) => {
                    self.emit_compare(left, operator, right, *span)?;
                    Ok(())
                }

                Expression::BoolOp {
                    conjunction,
                    values,
                    ..
                } => {
                    // **值上下文**（实测模板，3.14 用 `COPY`／`TO_BOOL`／`POP_JUMP_IF_*`／`NOT_TAKEN`／`POP_TOP`）：
                    //   非末操作数：`值; COPY 1; TO_BOOL; POP_JUMP_IF_<短路方向>; NOT_TAKEN; POP_TOP`
                    //   末操作数  ：当作**值**求（不再测真值）
                    // 嵌套时**融合**：内层非末操作数跳到内层末操作数 `NOT_TAKEN` 之后的落点（实测
                    // `x = (a or b) and c`），内层末操作数按**外层继承的条件**跳（实测 `x = a and b or c`）。
                    // 折叠：常量短路（实测 `1 and 2` ⇒ `2`、`0 and 3` ⇒ `0`），且**加载位置取
                    // 「决定结果的那个操作数」**（`x = 0 and 3` 的 `LOAD_SMALL_INT` 位置是 `0` 那段，
                    // 不是整段表达式）
                    let folded = {
                        let mut deciding: Option<Span> = None;
                        let mut last_span = expression.span();
                        let mut result: Option<Constant> = None;
                        for value in values {
                            let Some(constant) = fold_constant(value)? else {
                                result = None;
                                break;
                            };
                            last_span = value.span();
                            let Some(truth) = truthiness(&constant) else {
                                result = None;
                                break;
                            };
                            let short_circuit = if *conjunction { !truth } else { truth };
                            result = Some(constant);
                            if short_circuit {
                                deciding = Some(value.span());
                                break;
                            }
                        }
                        result.map(|constant| (constant, deciding.unwrap_or(last_span)))
                    };
                    if let Some((folded, span)) = folded {
                        if let Some(leaf) = leftmost_literal(expression) {
                            self.intern_literal(leaf);
                        }
                        match folded {
                            Constant::Int(value) if (0..=255).contains(&value) => {
                                self.emit_at(
                                    span,
                                    opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                                    value as u8,
                                );
                            }
                            other => {
                                self.emit_constant(span, other);
                            }
                        }
                        return Ok(());
                    }
                    // 条件上下文（`if`／`while`）：不保留值 ⇒ 由 `emit_condition_jump_to` 走另一条路；
                    // 这里只处理"值上下文"
                    let end = self.new_label();
                    // 骨架指令取**整个布尔表达式**的跨度（实测；操作数的 `LOAD` 仍各自取）
                    let saved_scaffold = self.boolop_scaffold_span;
                    self.boolop_scaffold_span = Some(expression.span());
                    for value in &values[..values.len() - 1] {
                        self.emit_test_value(value, !*conjunction, end, None)?;
                    }
                    self.boolop_scaffold_span = saved_scaffold;
                    self.emit_operand(values.last().expect("`and`／`or` 至少一个操作数"))?;
                    self.mark_label(end);
                    Ok(())
                }
                Expression::Not(_, span) => {
                    // **`not` 的三种下场**（逐条实测）：
                    //   `not <名字等>`       ⇒ `TO_BOOL; UNARY_NOT`
                    //   `not (a is b)`／`in` ⇒ **翻转比较**（`IS_OP 1`／`CONTAINS_OP 1`，不"产出布尔再取反"）
                    //   `not (a < b)`        ⇒ 比较带 `bool(...)` 位（`|16`）＋ `UNARY_NOT`
                    //   双重 `not` **抵消**（偶数个：只留 `TO_BOOL`／只留 `bool(...)` 位，无 `UNARY_NOT`）
                    if let Some(folded) = fold_constant(expression)? {
                        if let Some(leaf) = leftmost_literal(expression) {
                            self.intern_literal(leaf);
                        }
                        match folded {
                            Constant::Int(value) if (0..=255).contains(&value) => {
                                self.emit_at(
                                    *span,
                                    opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                                    value as u8,
                                );
                            }
                            other => {
                                self.emit_constant(*span, other);
                            }
                        }
                        return Ok(());
                    }
                    // 数 `not` 的个数并剥掉（偶数个相互抵消）
                    let mut depth = 0usize;
                    let mut operand: &Expression = expression;
                    let mut deepest_span = *span;
                    while let Expression::Not(inner, inner_span) = operand {
                        depth += 1;
                        deepest_span = *inner_span;
                        operand = inner;
                    }
                    let odd = depth % 2 == 1;
                    // **跨度**（实测）：奇数个 `not` ⇒ 取**最外层**那段的；偶数个（相互抵消）⇒ 取
                    // **最内层** `not` 那段的（`x = not not a` 的 `TO_BOOL` 是 `(1,1,8,13)`）
                    let not_span = if odd { *span } else { deepest_span };
                    match operand {
                        Expression::Compare(left, operator, right, _compare_span) => {
                            let flipped = match operator {
                                CompareOperator::Is => CompareOperator::IsNot,
                                CompareOperator::IsNot => CompareOperator::Is,
                                CompareOperator::In => CompareOperator::NotIn,
                                CompareOperator::NotIn => CompareOperator::In,
                                other => *other,
                            };
                            let identity = matches!(
                                operator,
                                CompareOperator::Is
                                    | CompareOperator::IsNot
                                    | CompareOperator::In
                                    | CompareOperator::NotIn
                            );
                            // 推进之后，比较**整段**是那个 `not` 表达式（实测 `x = not a is b`
                            // 的 `IS_OP` 位置是 `(1,1,4,14)`＝整个 `not a is b`）
                            if identity {
                                // `is`／`in` 族：奇数翻参数、偶数原样；**都不"产出布尔再取反"**
                                let chosen = if odd { flipped } else { *operator };
                                // `is`／`in` 族**一律取最外层**那段的跨度（实测 `x = not not a is b`
                                // 的 `IS_OP` 是 `(1,1,4,18)`）
                                self.emit_compare(left, &chosen, right, *span)?;
                            } else {
                                // `COMPARE_OP` 族：一律带 `bool(...)` 位；奇数再补 `UNARY_NOT`
                                self.emit_compare_with_bool(left, operator, right, not_span)?;
                                if odd {
                                    self.emit_at(
                                        not_span,
                                        opcode::opcode("UNARY_NOT").expect("UNARY_NOT 在表里"),
                                        0,
                                    );
                                }
                            }
                        }
                        _ => {
                            self.emit_expression(operand)?;
                            self.emit_at(not_span, opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"), 0);
                            if odd {
                                self.emit_at(
                                    not_span,
                                    opcode::opcode("UNARY_NOT").expect("UNARY_NOT 在表里"),
                                    0,
                                );
                            }
                        }
                    }
                    Ok(())
                }
                Expression::Binary(operator, left, right, span) => {
                    // **常量折叠**（实测三条规则）：只有最左叶子进常量表；结果是小整数走
                    // `LOAD_SMALL_INT` 不进表；否则该常量在收尾之后才登记。**折叠出的加载位置是
                    // 整段表达式**（实测 `x = 1 + 2` ⇒ `(1,1,4,9)`）。
                    if let Some(folded) = fold_constant(expression)? {
                        if let Some(leaf) = leftmost_literal(expression) {
                            self.intern_literal(leaf);
                        }
                        match folded {
                            Constant::Int(value) if (0..=255).contains(&value) => {
                                self.emit_at(
                                    *span,
                                    opcode::opcode("LOAD_SMALL_INT")
                                        .expect("LOAD_SMALL_INT 在表里"),
                                    value as u8,
                                );
                            }
                            other => {
                                self.emit_constant(*span, other);
                            }
                        }
                        return Ok(());
                    }
                    self.emit_two_operands(left, right)?;
                    let symbol = operator.symbol();
                    let index = crate::opcode::get_nb_ops()
                        .iter()
                        .position(|entry| entry.1 == symbol)
                        .unwrap_or_else(|| panic!("nb_ops 里应当有 {symbol}"))
                        as u8;
                    // 实测：`BINARY_OP` 的位置是**整段 `a op b`**
                    self.emit_at(
                        *span,
                        opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
                        index,
                    );
                    Ok(())
                }
                Expression::Unary(operator, operand, span) => {
                    // 常量折叠：`x = -5` ⇒ `LOAD_CONST -5`（折叠规则同二元：最左叶子也进表）
                    if let Some(folded) = fold_constant(expression)? {
                        if let Some(leaf) = leftmost_literal(expression) {
                            self.intern_literal(leaf);
                        }
                        match folded {
                            Constant::Int(value) if (0..=255).contains(&value) => {
                                self.emit_at(
                                    *span,
                                    opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                                    value as u8,
                                );
                            }
                            other => {
                                self.emit_constant(*span, other);
                            }
                        }
                        return Ok(());
                    }
                    self.emit_expression(operand)?;
                    // `+x` 在 3.14 里**不是** `UNARY_POSITIVE`（那条约 3.12 就没了）：
                    // 实测是 `CALL_INTRINSIC_1 INTRINSIC_UNARY_POSITIVE`（下表按**名字**取，`BC-39`）
                    let opcode_number = match operator {
                        UnaryOperator::Positive => opcode::opcode("CALL_INTRINSIC_1")
                            .expect("CALL_INTRINSIC_1 在表里"),
                        UnaryOperator::Negative => {
                            opcode::opcode("UNARY_NEGATIVE").expect("UNARY_NEGATIVE 在表里")
                        }
                        UnaryOperator::Invert => {
                            opcode::opcode("UNARY_INVERT").expect("UNARY_INVERT 在表里")
                        }
                    };
                    let argument = match operator {
                        UnaryOperator::Positive => crate::opcode::get_intrinsic1_descs()
                            .iter()
                            .position(|name| *name == "INTRINSIC_UNARY_POSITIVE")
                            .expect("intrinsic1 表里应当有 INTRINSIC_UNARY_POSITIVE")
                            as u8,
                        _ => 0,
                    };
                    self.emit_at(*span, opcode_number, argument);
                    Ok(())
                }
            _ => unreachable!("emit_expression_operators 只接收 expr_operators 族的 Expression"),
        }
    }
}
