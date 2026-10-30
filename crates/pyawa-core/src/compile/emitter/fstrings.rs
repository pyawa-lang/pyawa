//! `emitter` 的 `fstrings` 域：f-string 分段（[`Emitter::emit_fstring_part`]）。

use super::*;

impl Emitter {
    /// 发 f-string 的**一段**（字面量／插值；格式规格本身也是若干段）。
    pub(crate) fn emit_fstring_part(&mut self, part: &FStringPart, span: Span) -> Result<(), CompileError> {
        match part {
            FStringPart::Literal { text, span: literal_span } => {
                let index = self.intern_constant(Constant::Str(text.clone()));
                self.emit_indexed(*literal_span, "LOAD_CONST", index);
                Ok(())
            }
            // **含孤立代理的字面段**（P3-38 S2b ✓）：与普通段同形，只是常量走 WTF-8 表示 ✓。
            FStringPart::LiteralWtf8 {
                bytes,
                span: literal_span,
            } => {
                let index = self.intern_constant(Constant::StrWtf8(bytes.clone()));
                self.emit_indexed(*literal_span, "LOAD_CONST", index);
                Ok(())
            }
            FStringPart::Formatted {
                expression,
                conversion,
                spec,
                spec_span,
                span: part_span,
            } => {
                self.emit_expression(expression)?;
                if let Some(conversion) = conversion {
                    self.emit_at(
                        *part_span,
                        opcode::opcode("CONVERT_VALUE").expect("CONVERT_VALUE 在表里"),
                        *conversion,
                    );
                }
                match spec {
                    None => {
                        self.emit_at(
                            *part_span,
                            opcode::opcode("FORMAT_SIMPLE").expect("FORMAT_SIMPLE 在表里"),
                            0,
                        );
                    }
                    Some(parts) => {
                        if parts.is_empty() {
                            // **空规格**（`f"{x:}"`／`f"{x!r:}"`）—— 第 286 轮真 bug 修复 ✗：
                            // 参照这里是 `LOAD_CONST ''` ＋ `FORMAT_WITH_SPEC` ✓（逐条 `dis` 实测 ✓）；
                            // 先前**什么都不发** ✗ ⇒ 栈顶的**值本身**被当成规格 ✗ ⇒
                            // `TypeError: format spec must be a str` ✗（`f"{x=:}"` 也就走不通 ✓）。
                            let index = self.intern_constant(Constant::Str(String::new()));
                            self.emit_indexed(*part_span, "LOAD_CONST", index);
                        }
                        for part in parts {
                            self.emit_fstring_part(part, span)?;
                        }
                        if parts.len() > 1 {
                            // 规格内部的 `BUILD_STRING` 取**规格那一段**的跨度（实测）
                            self.emit_indexed(spec_span.unwrap_or(*part_span), "BUILD_STRING", parts.len());
                        }
                        self.emit_at(
                            *part_span,
                            opcode::opcode("FORMAT_WITH_SPEC").expect("FORMAT_WITH_SPEC 在表里"),
                            0,
                        );
                    }
                }
                Ok(())
            }
        }
    }

}
