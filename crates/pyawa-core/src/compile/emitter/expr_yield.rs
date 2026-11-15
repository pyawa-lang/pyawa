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
                // **`await <表达式>`**（round 220 起，round 229/230 修形状）：照参照 3.14 的
                // `<值>; GET_AWAITABLE 1; LOAD_CONST None; L_send: SEND <出>; YIELD_VALUE 1; RESUME 3;
                //  JUMP_BACKWARD_NO_INTERRUPT L_send; <出> END_SEND` ✓ ——
                // **循环标签在 `SEND` 上**（恢复后重送 ✓）、**中间没有 `POP_TOP`**（否则跳回时把接收者弹掉 ✗）✓。
                Expression::Await(value, span) => {
                    // **只剩 `except` 处理块体内那一格还是"透明"回退**（round 230 记录 ✗）：
                    // `try` **体**里的 `await` 已经真接线并通过（`IgnoreGeneratorExit().aclose()` 的
                    // `try: await self.athrow(...)` ⇒ `test_collections` **全绿** ✓）；处理块体内再发
                    // 等待循环的异常表组合还没验证 ⇒ 先保持回退（不影响本轮判据 ✓）。
                    if self.handler_depth > 0 {
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
                    // **循环标签要在 `SEND` 上**（round 229 真 bug 修 ✗）：参照 3.14 的
                    // `await` 形状是 `GET_AWAITABLE; LOAD_CONST None; L_send: SEND L_done;
                    // YIELD_VALUE 1; RESUME 3; JUMP_BACKWARD_NO_INTERRUPT L_send; L_done: END_SEND`
                    // ✓ —— 先前标签打在 `YIELD_VALUE` 上、跳回 `YIELD_VALUE` ✗ ⇒ 恢复后**重复 yield**
                    // （而不是把送进去的值重送给 `SEND`）⇒ 驱动方收到多余的值／`StackUnderflow`
                    // ✗（`test_collections.TestOneTrickPonyABCs.test_AsyncGenerator` 的
                    // `IgnoreGeneratorExit().aclose()` ✓）。
                    self.mark_label(await_loop);
                    self.emit_directed_jump(
                        *span,
                        opcode::opcode("SEND").expect("SEND 在表里"),
                        await_done,
                        false,
                    );
                    self.emit_at(*span, opcode::opcode("YIELD_VALUE").expect("YIELD_VALUE 在表里"), 1);
                    self.emit_at(*span, opcode::opcode("RESUME").expect("RESUME 在表里"), 3);
                    // **这里不要再 `POP_TOP`**（round 230 真 bug 修 ✗）：参照的 await 循环是
                    // `SEND; YIELD_VALUE 1; RESUME 3; JUMP_BACKWARD_NO_INTERRUPT L_send` —— **没有**
                    // `POP_TOP`（异步生成器挂起的那条路也不该弹）✓。先前多弹一格 ✗ ⇒ 跳回 `SEND` 时
                    // 栈上只剩 sent、**接收者被弹掉** ⇒ `StackUnderflow` ✗
                    //（`IgnoreGeneratorExit().aclose()` 的第二次 `send` ✓）。
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
