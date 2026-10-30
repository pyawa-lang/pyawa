//! `emitter` 的 `expr_fstrings` 域：

use super::*;

impl Emitter {
    pub(crate) fn emit_expression_fstrings(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
                Expression::FString { parts, span } => {
                    for part in parts {
                        self.emit_fstring_part(part, *span)?;
                    }
                    if parts.len() > 1 {
                        self.emit_indexed(*span, "BUILD_STRING", parts.len());
                    }
                    Ok(())
                }
                // **`lambda`**（实测）：嵌套单元名／qualname 都是 `<lambda>`（函数里是
                // `<f>.<locals>.<lambda>`）；体 ＝ 那条表达式的 `Return`；随后与 `def` 共用
                // "造函数对象"（默认值 → `LOAD_CONST <code>` → `MAKE_FUNCTION` → 挂属性）
                // **`yield` 当表达式** ✓（第 219 轮）：与语句版同源 ✓，只是**不丢**那个值 ✓
                // —— `YIELD_VALUE` 之后 `RESUME 5` 把"送进来的值"留在栈上 ✓（语句版随后 `POP_TOP` 丢掉 ✓）。
            _ => unreachable!("emit_expression_fstrings 只接收 expr_fstrings 族的 Expression"),
        }
    }
}
