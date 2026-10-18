//! `emitter` 的 `stmt_defs` 域：定义语句（`def`／`class`／`type`）的发射。

use super::*;

impl Emitter {
    pub(crate) fn emit_statement_defs(&mut self, statement: &Statement, _rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
                Statement::Class {
                    name,
                    span,
                    first_line,
                    bases,
                    keywords,
                    body,
                    decorators,
                    type_params,
                } => self.emit_class_statement(
                    name, span, first_line, decorators, keywords, bases, body, type_params,
                ),
                Statement::Def {
                    name,
                    decorators,
                    span,
                    first_line,
                    parameters,
                    kwonly,
                    returns,
                    returns_span,
                    varargs,
                    varkw,
                    body,
                    type_params,
                } => self.emit_def_statement(
                    name, decorators, span, first_line, parameters, kwonly, varargs, varkw, returns,
                    returns_span, body, type_params,
                ),
                // **`type X = <值>`**（PEP 695 别名语句 ✓，task-24 收口 ③ ✓）
                Statement::TypeAlias {
                    name,
                    name_span,
                    type_params,
                    value,
                    span,
                } => self.emit_type_alias_statement(name, name_span, type_params, value, span),
            _ => unreachable!("emit_statement_defs 只接收 stmt_defs 族的 Statement"),
        }
    }
}
