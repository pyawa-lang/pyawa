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
                // **`BC-63` 空合并 `a ?? b`**：`a` 求值一次、**只判 `None`**；不是 `None` ⇒ 取 `a`，
                // 否则取 `b`（`b` 惰性求值）。`or`／`and` 的"看真假"与它无关（`0`／`''` 都算有值）。
            _ => unreachable!("emit_expression_yield 只接收 expr_yield 族的 Expression"),
        }
    }
}
