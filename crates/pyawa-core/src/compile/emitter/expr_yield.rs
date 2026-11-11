//! `emitter` 的 `expr_yield` 域：表达式位置的 `yield`／`yield from`。

use super::*;

impl Emitter {
    pub(crate) fn emit_expression_yield(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
                Expression::Yield(value, span) => {
                    if let Some(value) = value {
                        self.emit_expression(value)?;
                    } else {
                        let index = self.intern_constant(Constant::None);
                        self.emit_indexed(*span, "LOAD_CONST", index);
                    }
                    self.emit_named(*span, "YIELD_VALUE", 0);
                    self.emit_named(*span, "RESUME", 5);
                    self.last_span = *span;
                    Ok(())
                }
                // **`yield from <表达式>`**（第 315 轮）：值**留着**（表达式形态）⇒ 末尾不补 `POP_TOP` ✓。
                Expression::YieldFrom(value, span) => {
                    self.emit_yield_from(value, *span, false)?;
                    self.last_span = *span;
                    Ok(())
                }
                // **`await <表达式>`**（round 220）：照参照的形状 ——
                // `<值>; GET_AWAITABLE 1; LOAD_CONST None; SEND <出>; YIELD_VALUE 1; RESUME 3;
                //  JUMP_BACKWARD_NO_INTERRUPT <回>; <出> END_SEND` ✓（与 `async with`／`async for` 同一套 ✓）。
                Expression::Await(value, span) => {
                    // **`try`／`with` 里的 `await` 先保持"透明"**（round 220 如实登记 ✗）：
                    // 真等待要在异常表 `depth` 里把这些合成指令（YIELD_VALUE／RESUME）算进去；
                    // 当前编译器在"处理块内发等待循环"时会算错 depth ⇒ 处理块展开后
                    // `StackUnderflow` ✗（`test_collections` 的 `MinimalAGen.aclose`
                    // ＝ `async def aclose(self): try: await self.athrow(...) ...` 实测 ✓）。
                    // ⇒ 有 `try` 处理块时退回旧口径（透明近似 ✓，与 round 219 之前一致 ✓）；
                    // 顶层 `await`（无处理块）走真等待 ✓。
                    if self.handler_depth > 0 || self.try_depth > 0 {
                        self.emit_expression(value)?;
                        self.last_span = *span;
                        return Ok(());
                    }
                    self.emit_expression(value)?;
                    let await_loop = self.new_label();
                    let await_done = self.new_label();
                    self.emit_at(*span, opcode::opcode("GET_AWAITABLE").expect("GET_AWAITABLE 在表里"), 1);
                    let none_index = self.intern_constant(Constant::None);
                    self.emit_indexed(*span, "LOAD_CONST", none_index);
                    self.emit_directed_jump(
                        *span,
                        opcode::opcode("SEND").expect("SEND 在表里"),
                        await_done,
                        false,
                    );
                    self.mark_label(await_loop);
                    self.emit_at(*span, opcode::opcode("YIELD_VALUE").expect("YIELD_VALUE 在表里"), 1);
                    self.emit_at(*span, opcode::opcode("RESUME").expect("RESUME 在表里"), 3);
                    self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                    self.emit_directed_jump(
                        *span,
                        opcode::opcode("JUMP_BACKWARD_NO_INTERRUPT")
                            .expect("JUMP_BACKWARD_NO_INTERRUPT 在表里"),
                        await_loop,
                        true,
                    );
                    self.mark_label(await_done);
                    self.emit_at(*span, opcode::opcode("END_SEND").expect("END_SEND 在表里"), 0);
                    self.last_span = *span;
                    Ok(())
                }
                // **`BC-63` 空合并 `a ?? b`**：`a` 求值一次、**只判 `None`**；不是 `None` ⇒ 取 `a`，
                // 否则取 `b`（`b` 惰性求值）。`or`／`and` 的"看真假"与它无关（`0`／`''` 都算有值）。
            _ => unreachable!("emit_expression_yield 只接收 expr_yield 族的 Expression"),
        }
    }
}
