//! `parser` 的 `patterns` 域：`match` 的模式解析。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

/// **`match` 语句**（第 290 轮）：`match`／`case` 是**软关键字** ✓ ⇒ 先试解析主语与 `:`，
/// 试不中给 `Ok(None)`（调用方走普通那条路 ✓，例如 `match(x)` 是调用 ✓）。
pub(crate) fn try_parse_match(
    lexed: &Lexed,
    cursor: usize,
    depth: usize,
    in_function: bool,
) -> Result<Option<(Statement, usize)>, CompileError> {
    let Ok((subject, after_subject)) = parse_expression(lexed, cursor + 1) else {
        return Ok(None);
    };
    if lexed.lexemes.get(after_subject) != Some(&Lexeme::Colon) {
        return Ok(None);
    }
    // 到这里**确定**是 `match` 语句 ✓：后面的错都**如实报** ✓
    let keyword_span = lexed.spans[cursor];
    let mut at = after_subject + 1;
    if lexed.lexemes.get(at) != Some(&Lexeme::Newline) {
        return Err(CompileError::Syntax("`match` 的冒号后面要换行".to_owned()));
    }
    at += 1;
    if lexed.lexemes.get(at) != Some(&Lexeme::Indent) {
        return Err(CompileError::Syntax("`match` 的体要缩进".to_owned()));
    }
    at += 1;
    let mut cases: Vec<MatchCase> = Vec::new();
    let mut last_end = keyword_span;
    loop {
        let case_span = match lexed.lexemes.get(at) {
            Some(Lexeme::Name(word)) if word == "case" => lexed.spans[at],
            _ => break,
        };
        at += 1;
        let (pattern, next) = parse_pattern(lexed, at)?;
        at = next;
        let mut guard: Option<Expression> = None;
        // `if` 在词法层是**关键字词素** ✓（不是名字 ✓）
        if lexed.lexemes.get(at) == Some(&Lexeme::If) {
            let (expression, next) = parse_expression(lexed, at + 1)?;
            guard = Some(expression);
            at = next;
        }
        let (body, next) = parse_suite(lexed, at, depth, in_function)?;
        at = next;
        last_end = statements_last_end(&body).unwrap_or(case_span);
        cases.push(MatchCase {
            pattern,
            guard,
            body,
            span: case_span,
        });
    }
    if cases.is_empty() {
        return Err(CompileError::Syntax(
            "`match` 至少要有一条 `case`".to_owned(),
        ));
    }
    if lexed.lexemes.get(at) != Some(&Lexeme::Dedent) {
        return Err(CompileError::Syntax("`match` 的体没有正常收尾".to_owned()));
    }
    at += 1;
    Ok(Some((
        Statement::Match {
            span: keyword_span.to(last_end),
            subject,
            cases,
        },
        at,
    )))
}

/// **一个模式**（第 290 轮，最小面）：`或` 在最外层 ✓（`case "a" | "b":`）。
pub(crate) fn parse_pattern(lexed: &Lexed, cursor: usize) -> Result<(Pattern, usize), CompileError> {
    let (first, mut at) = parse_pattern_primary(lexed, cursor)?;
    let mut alternatives: Vec<Pattern> = vec![first];
    while lexed.lexemes.get(at) == Some(&Lexeme::Pipe) {
        let (next_pattern, next) = parse_pattern_primary(lexed, at + 1)?;
        alternatives.push(next_pattern);
        at = next;
    }
    let pattern = if alternatives.len() == 1 {
        alternatives.remove(0)
    } else {
        // 参照：`case a | b:` 里**不许有捕获**（`SyntaxError: name capture … makes remaining patterns unreachable` ✓）
        if alternatives
            .iter()
            .any(|item| matches!(item, Pattern::Capture(_, _)))
        {
            return Err(CompileError::Syntax(
                "`match` 的或模式里不许有捕获（参照同样是语法错）".to_owned(),
            ));
        }
        let start = pattern_span(&alternatives[0]);
        let end = pattern_span(alternatives.last().expect("刚判过非空"));
        Pattern::Or(alternatives, start.to(end))
    };
    // **`as` 模式**（task-45 ✓）：`case A.y as z:`／`case [1] as z:`／`case (0 | 1) as z:`
    // —— 官方 `test_patma.py` 一族；先前在 `as` 上报"这里要冒号" ✗。`as` 只吃一个名字 ✓。
    if let Some(Lexeme::Name(word)) = lexed.lexemes.get(at) {
        if word == "as" {
            let Some(Lexeme::Name(binding)) = lexed.lexemes.get(at + 1) else {
                return Err(CompileError::Syntax("`as` 后面要一个名字".to_owned()));
            };
            let binding_span = lexed.spans[at + 1];
            let span = pattern_span(&pattern).to(binding_span);
            return Ok((Pattern::As(Box::new(pattern), binding.clone(), span), at + 2));
        }
    }
    Ok((pattern, at))
}

/// **模式里的数字字面量链**（task-45 ✓）：`case 0 + 0j:`／`case -1 - 2j:` —— 参照把这些
/// **常量折叠**成一个复数常量（`dis` 里就是一条 `LOAD_CONST`）。只收"实部 ± 虚部"这一形态
/// （与表达式那边的 `fold_complex_binary` 同口径；更复杂的表达式模式参照本来也不允许）。
fn pattern_number_tail(lexed: &Lexed, mut value: Constant, mut at: usize) -> (Constant, usize) {
    loop {
        let negative = match lexed.lexemes.get(at) {
            Some(Lexeme::Plus) => false,
            Some(Lexeme::Minus) => true,
            _ => return (value, at),
        };
        let Some(Lexeme::Imaginary(bits)) = lexed.lexemes.get(at + 1) else {
            return (value, at);
        };
        let imag = f64::from_bits(*bits);
        let real = match value {
            Constant::Int(integer) => integer as f64,
            Constant::Float(bits) => f64::from_bits(bits),
            Constant::BigInt(ref text) => match text.parse::<f64>() {
                Ok(number) => number,
                Err(_) => return (value, at),
            },
            _ => return (value, at),
        };
        value = Constant::Complex {
            real_bits: real.to_bits(),
            imag_bits: (if negative { -imag } else { imag }).to_bits(),
        };
        at += 2;
    }
}

/// **模式的基本项**：字面量／捕获／通配 ✓；其余如实报**未接线** ✓（`CM-6`）。
pub(crate) fn parse_pattern_primary(lexed: &Lexed, cursor: usize) -> Result<(Pattern, usize), CompileError> {
    let span = lexed
        .spans
        .get(cursor)
        .copied()
        .unwrap_or(Span::new(1, 1, 0, 0));
    match lexed.lexemes.get(cursor) {
        Some(Lexeme::Name(name)) if name == "_" => Ok((Pattern::Wildcard(span), cursor + 1)),
        Some(Lexeme::Name(name)) if name == "None" => {
            Ok((Pattern::Literal(Constant::None, span), cursor + 1))
        }
        Some(Lexeme::Name(name)) if name == "True" => {
            Ok((Pattern::Literal(Constant::Bool(true), span), cursor + 1))
        }
        Some(Lexeme::Name(name)) if name == "False" => {
            Ok((Pattern::Literal(Constant::Bool(false), span), cursor + 1))
        }
        Some(Lexeme::Name(name)) => {
            // **点号链 ＋ `(`** ⇒ 类模式**（`case ast.Call()` ✓）；只点号 ⇒ **值模式**
            // （`case Color.RED` ✓）；都不是 ⇒ 捕获（`case x` ✓）。
            let mut at = cursor;
            let mut expression = Expression::Name(name.clone(), span);
            while lexed.lexemes.get(at + 1) == Some(&Lexeme::Dot) {
                let Some(Lexeme::Name(attribute)) = lexed.lexemes.get(at + 2) else {
                    return Err(CompileError::Syntax(
                        "值模式的 `.` 后面要一个属性名".to_owned(),
                    ));
                };
                let attribute_span = lexed.spans[at + 2];
                expression =
                    Expression::Attribute(Box::new(expression), attribute.clone(), span.to(attribute_span));
                at += 2;
            }
            if lexed.lexemes.get(at + 1) == Some(&Lexeme::LeftParen) {
                return parse_class_pattern(lexed, expression, at + 2, span);
            }
            if at > cursor {
                return Ok((Pattern::Value(expression, span), at + 1));
            }
            Ok((Pattern::Capture(name.clone(), span), cursor + 1))
        }
        // **带符号的数字字面量模式**（task-45 ✓）：`case -0:`／`case -1.5:`／`case -1j:`
        // （官方 `test_patma.py:1013`）—— 参照 `dis` 里直接是 `LOAD_CONST -1` 那种常量 ✓。
        Some(Lexeme::Minus) | Some(Lexeme::Plus) => {
            let negative = lexed.lexemes.get(cursor) == Some(&Lexeme::Minus);
            let literal_span = span.to(lexed.spans.get(cursor + 1).copied().unwrap_or(span));
            let constant = match lexed.lexemes.get(cursor + 1) {
                Some(Lexeme::Int(value)) => Constant::Int(if negative {
                    value.checked_neg().ok_or_else(|| {
                        CompileError::Unsupported("模式里的整数取负溢出".to_owned())
                    })?
                } else {
                    *value
                }),
                Some(Lexeme::BigInt(text)) => Constant::BigInt(if negative {
                    format!("-{text}")
                } else {
                    text.clone()
                }),
                Some(Lexeme::Float(bits)) => Constant::Float(if negative {
                    (-f64::from_bits(*bits)).to_bits()
                } else {
                    *bits
                }),
                Some(Lexeme::Imaginary(bits)) => {
                    let value = f64::from_bits(*bits);
                    Constant::Complex {
                        real_bits: if negative { (-0.0f64).to_bits() } else { 0f64.to_bits() },
                        imag_bits: if negative { (-value).to_bits() } else { *bits },
                    }
                }
                other => {
                    return Err(CompileError::Syntax(format!(
                        "模式里的 `+`／`-` 后面要数字字面量，实际 {other:?}"
                    )))
                }
            };
            let (constant, at) = pattern_number_tail(lexed, constant, cursor + 2);
            Ok((Pattern::Literal(constant, literal_span), at))
        }
        Some(Lexeme::Int(value)) => {
            let (constant, at) = pattern_number_tail(lexed, Constant::Int(*value), cursor + 1);
            Ok((Pattern::Literal(constant, span), at))
        }
        Some(Lexeme::BigInt(text)) => {
            let (constant, at) =
                pattern_number_tail(lexed, Constant::BigInt(text.clone()), cursor + 1);
            Ok((Pattern::Literal(constant, span), at))
        }
        Some(Lexeme::Str(text)) => Ok((
            Pattern::Literal(Constant::Str(text.clone()), span),
            cursor + 1,
        )),
        // **含孤立代理的字符串模式**（P3-38 S2b ✓）
        Some(Lexeme::StrWtf8(bytes)) => Ok((
            Pattern::Literal(Constant::StrWtf8(bytes.clone()), span),
            cursor + 1,
        )),
        Some(Lexeme::Bytes(value)) => Ok((
            Pattern::Literal(Constant::Bytes(value.clone()), span),
            cursor + 1,
        )),
        Some(Lexeme::Float(bits)) => {
            let (constant, at) = pattern_number_tail(lexed, Constant::Float(*bits), cursor + 1);
            Ok((Pattern::Literal(constant, span), at))
        }
        // **虚部字面量**（task-29 ✓）：`case 1j:` 的常量模式（实部补 `0.0` ✓）
        Some(Lexeme::Imaginary(bits)) => {
            let (constant, at) = pattern_number_tail(
                lexed,
                Constant::Complex {
                    real_bits: 0f64.to_bits(),
                    imag_bits: *bits,
                },
                cursor + 1,
            );
            Ok((Pattern::Literal(constant, span), at))
        }
        // **序列模式**（task-37）：`[a, b, *rest]` ✓
        Some(Lexeme::LeftBracket) => parse_sequence_pattern(lexed, cursor, Lexeme::RightBracket, span),
        // **映射模式**（task-37）：`{"k": v}`／`{}` ✓
        Some(Lexeme::LeftBrace) => parse_mapping_pattern(lexed, cursor, span),
        // **括号**：组模式 `(p)` 或括号元组模式 `(a, b)`／`()` ✓
        Some(Lexeme::LeftParen) => parse_paren_pattern(lexed, cursor, span),
        other => Err(CompileError::Syntax(format!(
            "`match` 的模式里遇到 {other:?}（第 {} 行）",
            span.line_start
        ))),
    }
}

/// **序列模式**（task-37）：`[` 之后、`]`／`)` 之前的项表（调用方给开括号位点 ✓）。
///
/// 项＝子模式或星号项 `*名字`／`*_`（参照：星号只收捕获／通配 ✓）。
fn parse_sequence_items(
    lexed: &Lexed,
    mut at: usize,
    close: &Lexeme,
    open_span: Span,
    mut items: Vec<Pattern>,
) -> Result<(Vec<Pattern>, usize, Span), CompileError> {
    let close_span: Span;
    loop {
        if lexed.lexemes.get(at) == Some(close) {
            close_span = lexed.spans[at];
            at += 1;
            break;
        }
        if lexed.lexemes.get(at) == Some(&Lexeme::Star) {
            let star_span = lexed.spans[at];
            let (inner, next) = parse_pattern(lexed, at + 1)?;
            if !matches!(inner, Pattern::Capture(_, _) | Pattern::Wildcard(_)) {
                return Err(CompileError::Syntax(
                    "序列模式的 `*` 后面只能是名字或 `_`".to_owned(),
                ));
            }
            items.push(Pattern::Star(Box::new(inner), star_span));
            at = next;
        } else {
            let (item, next) = parse_pattern(lexed, at)?;
            items.push(item);
            at = next;
        }
        match lexed.lexemes.get(at) {
            Some(Lexeme::Comma) => at += 1,
            Some(token) if token == close => {
                close_span = lexed.spans[at];
                at += 1;
                break;
            }
            other => {
                return Err(CompileError::Syntax(format!(
                    "序列模式里期望 `,` 或收尾括号，遇到 {other:?}（第 {} 行）",
                    lexed.spans.get(at).map(|span| span.line_start).unwrap_or(0)
                )))
            }
        }
    }
    Ok((items, at, open_span.to(close_span)))
}

/// `[a, b, *rest]` 的整条模式 ✓。
fn parse_sequence_pattern(
    lexed: &Lexed,
    cursor: usize,
    close: Lexeme,
    open_span: Span,
) -> Result<(Pattern, usize), CompileError> {
    let (items, at, span) = parse_sequence_items(lexed, cursor + 1, &close, open_span, Vec::new())?;
    Ok((Pattern::Sequence { items, span }, at))
}

/// 括号形态（task-37）：`()` 空序列／`(a, b)` 括号元组序列／`(p)` 组模式（直接交出子模式）✓。
fn parse_paren_pattern(
    lexed: &Lexed,
    cursor: usize,
    open_span: Span,
) -> Result<(Pattern, usize), CompileError> {
    if lexed.lexemes.get(cursor + 1) == Some(&Lexeme::RightParen) {
        return Ok((
            Pattern::Sequence {
                items: Vec::new(),
                span: open_span.to(lexed.spans[cursor + 1]),
            },
            cursor + 2,
        ));
    }
    let (first, at) = parse_pattern(lexed, cursor + 1)?;
    match lexed.lexemes.get(at) {
        // 组模式：`(p)` ⇒ 语义上就是 `p` 本身 ✓（捕获／通配照旧触发"不可达"那条规则 ✓）。
        Some(Lexeme::RightParen) => Ok((first, at + 1)),
        Some(Lexeme::Comma) => {
            let (items, at, span) = parse_sequence_items(
                lexed,
                at + 1,
                &Lexeme::RightParen,
                open_span,
                vec![first],
            )?;
            Ok((Pattern::Sequence { items, span }, at))
        }
        other => Err(CompileError::Syntax(format!(
            "括号模式里期望 `,` 或 `)`，遇到 {other:?}"
        ))),
    }
}

/// **映射模式**（task-37）：`{}`／`{"k": v, ...}` ✓。
///
/// 键只收**字面量／值模式**（参照同规 ✓）；`**rest` 如实报**未接线**（本轮不做 ✓）。
fn parse_mapping_pattern(
    lexed: &Lexed,
    cursor: usize,
    open_span: Span,
) -> Result<(Pattern, usize), CompileError> {
    let mut at = cursor + 1;
    let mut items: Vec<(Pattern, Pattern)> = Vec::new();
    let close_span: Span;
    loop {
        if lexed.lexemes.get(at) == Some(&Lexeme::RightBrace) {
            close_span = lexed.spans[at];
            at += 1;
            break;
        }
        if lexed.lexemes.get(at) == Some(&Lexeme::DoubleStar) {
            return Err(CompileError::Unsupported(
                "映射模式的 `**rest` 剩余项捕获尚未接线".to_owned(),
            ));
        }
        let (key, next) = parse_pattern_primary(lexed, at)?;
        if !matches!(key, Pattern::Literal(_, _) | Pattern::Value(_, _)) {
            return Err(CompileError::Syntax(
                "映射模式的键只能是字面量或值模式（参照同规）".to_owned(),
            ));
        }
        if lexed.lexemes.get(next) != Some(&Lexeme::Colon) {
            return Err(CompileError::Syntax(
                "映射模式的键后面要 `:`".to_owned(),
            ));
        }
        let (value, next) = parse_pattern(lexed, next + 1)?;
        items.push((key, value));
        at = next;
        match lexed.lexemes.get(at) {
            Some(Lexeme::Comma) => at += 1,
            Some(Lexeme::RightBrace) => {
                close_span = lexed.spans[at];
                at += 1;
                break;
            }
            other => {
                return Err(CompileError::Syntax(format!(
                    "映射模式里期望 `,` 或 `}}`，遇到 {other:?}"
                )))
            }
        }
    }
    Ok((
        Pattern::Mapping {
            items,
            span: open_span.to(close_span),
        },
        at,
    ))
}

/// **类模式的实参表**（第 300 轮）：`[名字 = 子模式 | 子模式] ("," …)* ")"` ✓。
/// 类表达式由调用方解析好（`str` ✓、`ast.Call` ✓）。
pub(crate) fn parse_class_pattern(
    lexed: &Lexed,
    class: Expression,
    cursor: usize,
    span: Span,
) -> Result<(Pattern, usize), CompileError> {
    let mut at = cursor;
    let mut positional: Vec<Pattern> = Vec::new();
    let mut keywords: Vec<(String, Pattern)> = Vec::new();
    loop {
        if lexed.lexemes.get(at) == Some(&Lexeme::RightParen) {
            at += 1;
            break;
        }
        if let (Some(Lexeme::Name(key)), Some(Lexeme::Assign)) =
            (lexed.lexemes.get(at), lexed.lexemes.get(at + 1))
        {
            let key = key.clone();
            let (sub, next) = parse_pattern(lexed, at + 2)?;
            keywords.push((key, sub));
            at = next;
        } else {
            let (sub, next) = parse_pattern(lexed, at)?;
            positional.push(sub);
            at = next;
        }
        match lexed.lexemes.get(at) {
            Some(Lexeme::Comma) => at += 1,
            Some(Lexeme::RightParen) => {
                at += 1;
                break;
            }
            other => {
                return Err(CompileError::Syntax(format!(
                    "类模式的实参表里出现 {other:?}"
                )))
            }
        }
    }
    Ok((
        Pattern::Class {
            class,
            positional,
            keywords,
            span,
        },
        at,
    ))
}

/// 一个模式的位点（四种形态各取自己的 ✓）。
pub(crate) fn pattern_span(pattern: &Pattern) -> Span {
    match pattern {
        Pattern::Literal(_, span)
        | Pattern::Capture(_, span)
        | Pattern::Wildcard(span)
        | Pattern::Value(_, span)
        | Pattern::Or(_, span)
        | Pattern::As(_, _, span) => *span,
        Pattern::Class { span, .. }
        | Pattern::Sequence { span, .. }
        | Pattern::Mapping { span, .. }
        | Pattern::Star(_, span) => *span,
    }
}
