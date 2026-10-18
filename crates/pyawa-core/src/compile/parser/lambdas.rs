//! `parser` 的 `lambdas` 域：`lambda` 的表达式／语句体解析。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

/// **`lambda 形参表: 表达式`**（3.14 实测形态）：形参语法与 `def` 同族但没有注解、没有 `/`；
/// 体是**一条表达式**（发射时就是"求值再 `RETURN_VALUE`"）。
pub(crate) fn parse_lambda(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let tokens = &lexed.lexemes;
    let keyword_span = lexed.spans[cursor];
    let mut cursor = cursor + 1;
    let mut parameters: Vec<Parameter> = Vec::new();
    let mut kwonly: Vec<Parameter> = Vec::new();
    let mut varargs: Option<String> = None;
    let mut varkw: Option<String> = None;
    let mut after_star = false;
    while tokens.get(cursor) != Some(&Lexeme::Colon) {
        match tokens.get(cursor) {
            Some(Lexeme::Star) => {
                cursor += 1;
                after_star = true;
                if let Some(Lexeme::Name(name)) = tokens.get(cursor) {
                    varargs = Some(name.clone());
                    cursor += 1;
                }
            }
            Some(Lexeme::DoubleStar) => {
                cursor += 1;
                match tokens.get(cursor) {
                    Some(Lexeme::Name(name)) => {
                        varkw = Some(name.clone());
                        cursor += 1;
                    }
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "`**` 后面要一个名字，实际 {other:?}"
                        )))
                    }
                }
            }
            Some(Lexeme::Name(name)) => {
                let name = name.clone();
                cursor += 1;
                let default = if tokens.get(cursor) == Some(&Lexeme::Assign) {
                    let (expression, next) = parse_expression(lexed, cursor + 1)?;
                    cursor = next;
                    Some(expression)
                } else {
                    None
                };
                if varkw.is_some() {
                    return Err(CompileError::Syntax("`**kw` 之后不能再有形参".to_owned()));
                }
                let parameter = Parameter {
                    name,
                    posonly: false,
                    annotation: None,
                    annotation_span: None,
                    default,
                };
                if after_star {
                    kwonly.push(parameter);
                } else {
                    parameters.push(parameter);
                }
            }
            other => {
                return Err(CompileError::Syntax(format!(
                    "`lambda` 的形参表里出现 {other:?}"
                )))
            }
        }
        match tokens.get(cursor) {
            Some(Lexeme::Comma) => cursor += 1,
            Some(Lexeme::Colon) => break,
            other => {
                return Err(CompileError::Syntax(format!(
                    "`lambda` 的形参表里要 `,` 或 `:`，实际 {other:?}"
                )))
            }
        }
    }
    if tokens.get(cursor) != Some(&Lexeme::Colon) {
        return Err(CompileError::Syntax("`lambda` 的形参表后面要冒号".to_owned()));
    }
    cursor += 1;
    // **`BC-62` 多语句 body**（**只在扩展模式**）：`( 语句… 末表达式 )`。
    // 单行表达式形态**行为完全不变** ✗ —— 只有"括号内出现语句分隔／赋值／顶层物理换行"时才走
    // 新形态；否则仍走原来的 `parse_expression`（产物逐字节不变）。纯 Python 模式（`.py`）
    // 根本不进这一支 ⇒ 同形态照旧在括号解析处报 `Syntax`（`BC-15`）。
    let (body, next) = if lexed.mode == Mode::Extension
        && lexed.lexemes.get(cursor) == Some(&Lexeme::LeftParen)
        && lambda_body_looks_like_statements(lexed, cursor)
    {
        let (statements, after) = parse_lambda_statements(lexed, cursor)?;
        // 空体与"单个表达式语句"都**回落到表达式通道**（`()` 仍是空元组、`(x + 1)` 的产物
        // 与改动前逐字节相同）—— 新形态只在**真的多语句／赋值**时启用。
        let body = match statements.as_slice() {
            [] => LambdaBody::Expression(Box::new(parse_expression(lexed, cursor)?.0)),
            [Statement::Expression(expression, _)] => {
                LambdaBody::Expression(Box::new(expression.clone()))
            }
            _ => LambdaBody::Statements(statements),
        };
        (body, after)
    } else {
        let (body, next) = parse_expression(lexed, cursor)?;
        (LambdaBody::Expression(Box::new(body)), next)
    };
    let span = keyword_span.to(body.span());
    Ok((
        Expression::Lambda {
            parameters,
            kwonly,
            varargs,
            varkw,
            body,
            span,
        },
        next,
    ))
}

/// 扫描 `lambda` 体那对括号（`open` 指着 `(`），判断里面**是不是语句序列**（`BC-62`）。
///
/// 判据（任一条命中即走新形态）：括号内**相对深度 0** 处出现
/// ① `;`（语句分隔符）；② `=`（赋值；`==`／关键字实参的 `=` 在更深的括号里，不算）；
/// ③ 一个**顶层物理换行**（[`Lexed::line_breaks`]；括号内换行不产生 `Newline` 词素，故单记）。
///
/// 只是"看起来像"：真正是不是语句序列由 [`parse_lambda_statements`] 定；它失败时**不回退**
/// （用户写的就是想用新形态 ⇒ 报真实语法错，比静默按表达式解析更好定位）。
pub(crate) fn lambda_body_looks_like_statements(lexed: &Lexed, open: usize) -> bool {
    let tokens = &lexed.lexemes;
    let mut depth = 0usize;
    let mut cursor = open + 1;
    while let Some(token) = tokens.get(cursor) {
        match token {
            Lexeme::LeftParen | Lexeme::LeftBracket | Lexeme::LeftBrace => depth += 1,
            Lexeme::RightParen | Lexeme::RightBracket | Lexeme::RightBrace => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Lexeme::Semicolon if depth == 0 => return true,
            Lexeme::Assign if depth == 0 => return true,
            Lexeme::AugAssign(_) if depth == 0 => return true,
            // `pass`（软关键字：词法器给 `Name("pass")`）也只能是语句 —— 单行 `(pass)` 要按
            // "没有末表达式 ⇒ None" 处理，不能当名字表达式解析。
            Lexeme::Name(name) if name == "pass" && depth == 0 => return true,
            _ => {}
        }
        if depth == 0 && lexed.line_breaks.contains(&cursor) && cursor > open + 1 {
            return true;
        }
        cursor += 1;
    }
    false
}

/// 解析 `lambda` 体的 `( 语句… 末表达式 )`（`BC-62`；`open` 指着 `(`）。
///
/// 括号内是**简单语句**序列（`;`／顶层物理换行分隔）；**不支持**复合语句（见模块文档的
/// 如实边界）。返回语句表与**闭括号之后**的游标。
pub(crate) fn parse_lambda_statements(
    lexed: &Lexed,
    open: usize,
) -> Result<(Vec<Statement>, usize), CompileError> {
    let tokens = &lexed.lexemes;
    let mut cursor = open + 1;
    let mut statements = Vec::new();
    loop {
        // 分隔符：`;` 随便几个；物理换行只是"边界信息"，不产生词素。
        while matches!(tokens.get(cursor), Some(Lexeme::Semicolon)) {
            cursor += 1;
        }
        match tokens.get(cursor) {
            Some(Lexeme::RightParen) => return Ok((statements, cursor + 1)),
            None | Some(Lexeme::End) => {
                return Err(CompileError::Syntax(
                    "`lambda` 的多语句体没有闭合的 `)`".to_owned(),
                ))
            }
            _ => {}
        }
        let (statement, next) = parse_lambda_statement(lexed, cursor)?;
        statements.push(statement);
        cursor = next;
        // 语句之间**必须**有分隔（`;`、物理换行、或直接到闭括号）。
        let mut semicolon_separated = false;
        while matches!(tokens.get(cursor), Some(Lexeme::Semicolon)) {
            cursor += 1;
            semicolon_separated = true;
        }
        if matches!(tokens.get(cursor), Some(Lexeme::RightParen)) {
            continue;
        }
        // `;` 分隔就够了（同一行能连写）；否则**必须**有一个物理换行（括号内换行没词素）。
        if !semicolon_separated && !lexed.line_breaks.contains(&cursor) {
            return Err(CompileError::Syntax(format!(
                "`lambda` 多语句体里两条语句之间要 `;` 或换行，实际 {:?}（第 {} 行）",
                tokens.get(cursor),
                lexed
                    .spans
                    .get(cursor)
                    .map(|span| span.line_start)
                    .unwrap_or(0)
            )));
        }
    }
}

/// `lambda` 多语句体里的**一条简单语句**（`BC-62`；返回语句与下一游标）。
///
/// 接线：表达式语句、`名字 = 表达式`／属性／下标目标的赋值、增强赋值、`pass`、`名字 ??= 表达式`。
/// **不接**：复合语句（`if`／`for`／`try`…）、`return`、`del`、带注解赋值 —— 各自报明确错误
/// （不是静默忽略）。
pub(crate) fn parse_lambda_statement(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(Statement, usize), CompileError> {
    let tokens = &lexed.lexemes;
    let start_span = lexed.spans.get(cursor).copied().unwrap_or(Span::new(1, 1, 0, 0));
    // `pass`（软：名字形态）——空操作，但保留跨度给收尾。
    if let Some(Lexeme::Name(name)) = tokens.get(cursor) {
        if name == "pass" {
            return Ok((Statement::Pass(start_span), cursor + 1));
        }
        if name == "lambda" {
            // `lambda` 开头的表达式语句（`lambda: x` 自身）走下面通用路
        }
    }
    if matches!(
        tokens.get(cursor),
        Some(Lexeme::If)
            | Some(Lexeme::While)
            | Some(Lexeme::For)
            | Some(Lexeme::Return)
            | Some(Lexeme::Raise)
            | Some(Lexeme::Yield)
            | Some(Lexeme::Nonlocal)
            | Some(Lexeme::Global)
    ) {
        return Err(CompileError::Unsupported(format!(
            "`BC-62` 的 `lambda` 多语句体只接简单语句；这个开头（第 {} 行）属复合语句／控制流 ⇒ \
             请改用单行 `def` 或在括号外写",
            start_span.line_start
        )));
    }
    let (expression, next) = parse_expression(lexed, cursor)?;
    // `name ??= value`（空合并赋值；语句形态）
    if lexed.mode == Mode::Extension {
        let is_null_assign = tokens.get(next) == Some(&Lexeme::Question)
            && tokens.get(next + 1) == Some(&Lexeme::Question)
            && tokens.get(next + 2) == Some(&Lexeme::Assign);
        if is_null_assign {
            let Expression::Name(name, name_span) = &expression else {
                return Err(CompileError::Unsupported(format!(
                    "`??=` 只接线名字目标（第 {} 行）",
                    start_span.line_start
                )));
            };
            let (value, after) = parse_expression(lexed, next + 3)?;
            let span = name_span.to(value.span());
            return Ok((
                Statement::NullCoalesceAssign {
                    name: name.clone(),
                    name_span: *name_span,
                    value,
                    span,
                },
                after,
            ));
        }
    }
    // 增强赋值（`+=` 一族）
    if let Some(Lexeme::AugAssign(operator)) = tokens.get(next) {
        let operator = *operator;
        let (value, after) = parse_expression(lexed, next + 1)?;
        let span = start_span.to(value.span());
        let target = match expression {
            Expression::Name(name, name_span) => AugTarget::Name(name, name_span),
            Expression::Attribute(object, name, attribute_span) => AugTarget::Attribute {
                object: *object,
                name,
                span: attribute_span,
            },
            Expression::Subscript(container, key, subscript_span) => AugTarget::Subscript {
                container: *container,
                key: *key,
                target_span: subscript_span,
                span,
            },
            other => {
                return Err(CompileError::Unsupported(format!(
                    "这个形态还不支持增强赋值（第 {} 行）",
                    other.span().line_start
                )))
            }
        };
        return Ok((
            Statement::AugAssign {
                target,
                operator,
                value,
                span,
            },
            after,
        ));
    }
    // 普通赋值（只接名字／属性／下标目标；元组目标等不在 `BC-62` 的简单语句面里）
    if tokens.get(next) == Some(&Lexeme::Assign) {
        let (value, after) = parse_expression(lexed, next + 1)?;
        let span = start_span.to(value.span());
        let target_span = expression.span();
        let statement = match expression {
            Expression::Name(name, name_span) => Statement::Assign {
                target: name,
                target_span: name_span,
                value,
                span,
            },
            Expression::Attribute(object, name, attribute_span) => Statement::AssignAttr {
                object: *object,
                name,
                value,
                span,
                target_span: attribute_span,
            },
            Expression::Subscript(container, key, subscript_span) => Statement::AssignSubscript {
                container: *container,
                key: *key,
                value,
                target_span: subscript_span,
                span,
            },
            _ => {
                return Err(CompileError::Unsupported(format!(
                    "`BC-62` 多语句体只接名字／属性／下标目标的赋值（第 {} 行）",
                    target_span.line_start
                )))
            }
        };
        return Ok((statement, after));
    }
    // 表达式语句（末一条若是它就是返回值）
    let span = expression.span();
    Ok((Statement::Expression(expression, span), next))
}

