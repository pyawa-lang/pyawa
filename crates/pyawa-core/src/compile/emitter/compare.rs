//! `emitter` 的 `compare` 域：比较与二元运算的发射。

use super::*;

impl Emitter {
    /// 发一条**比较**：`COMPARE_OP`（六个）或 `IS_OP`／`CONTAINS_OP`（`is`／`in` 两族）。
    pub(crate) fn emit_compare(
        &mut self,
        left: &Expression,
        operator: &CompareOperator,
        right: &Expression,
        span: Span,
    ) -> Result<(), CompileError> {
        if matches!(
            operator,
            CompareOperator::Is
                | CompareOperator::IsNot
                | CompareOperator::In
                | CompareOperator::NotIn
        ) {
            self.emit_two_operands(left, right)?;
            let (name, oparg) = match operator {
                CompareOperator::Is => ("IS_OP", 0),
                CompareOperator::IsNot => ("IS_OP", 1),
                CompareOperator::In => ("CONTAINS_OP", 0),
                _ => ("CONTAINS_OP", 1),
            };
            self.emit_at(span, opcode::opcode(name).expect("比较指令在表里"), oparg);
            return Ok(());
        }
        self.emit_compare_plain(left, operator, right, span)
    }

    /// `COMPARE_OP`，并按当前上下文决定是否带 `bool(...)` 位（`|16`）。
    pub(crate) fn emit_compare_plain(
        &mut self,
        left: &Expression,
        operator: &CompareOperator,
        right: &Expression,
        span: Span,
    ) -> Result<(), CompileError> {
        self.emit_two_operands(left, right)?;
        let base = operator
            .oparg()
            .expect("`is`／`in` 一族不走 `COMPARE_OP`（调用方已分流）");
        let oparg = if self.in_condition { base | 16 } else { base };
        self.emit_at(
            span,
            opcode::opcode("COMPARE_OP").expect("COMPARE_OP 在表里"),
            oparg,
        );
        Ok(())
    }

    /// `COMPARE_OP` **强制带** `bool(...)` 位（`not` 推进比较时用：参照实测 `not a < b` ⇒ 18）。
    pub(crate) fn emit_compare_with_bool(
        &mut self,
        left: &Expression,
        operator: &CompareOperator,
        right: &Expression,
        span: Span,
    ) -> Result<(), CompileError> {
        self.emit_two_operands(left, right)?;
        let base = operator
            .oparg()
            .expect("`is`／`in` 一族不走 `COMPARE_OP`（调用方已分流）");
        self.emit_at(
            span,
            opcode::opcode("COMPARE_OP").expect("COMPARE_OP 在表里"),
            base | 16,
        );
        Ok(())
    }

    /// 发一条 `BINARY_OP`（`NB_SUBSCR`，即 `[]`）。
    pub(crate) fn emit_binary_op_subscript(&mut self, span: Span) {
        let index = crate::opcode::get_nb_ops()
            .iter()
            .position(|entry| entry.1 == "[]")
            .expect("nb_ops 里应当有 []") as u8;
        self.emit_at(
            span,
            opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
            index,
        );
    }

}
