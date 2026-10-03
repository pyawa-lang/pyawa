//! `compile` 的子模块（拆分自单文件时期，见 `AGENTS.md`）。

use super::*;

pub(super) fn parse_module(lexed: &Lexed) -> Result<Vec<Statement>, CompileError> {
    let mut cursor = 0usize;
    let statements = parse_statements(lexed, &mut cursor, 0, false)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::End) {
        return Err(CompileError::Syntax(format!(
            "模块结尾多出了 {:?}",
            lexed.lexemes.get(cursor)
        )));
    }
    Ok(statements)
}

/// 解析一条 `if`／`elif` 链（`elif` 与"`else:` 里套 `if`"**同形**，参照实测逐字节相同）。
///
/// `cursor` 指着 `if` **或** `elif`（后者是 `Name("elif")`：关键字表里没有它）。
/// **显示里的一项**：`*表达式` 包成 `Expression::Starred` ✓，否则就是普通表达式 ✓。
fn parse_star_or_expression(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    if lexed.lexemes.get(cursor) == Some(&Lexeme::Star) {
        let star_span = lexed.spans[cursor];
        let (value, next) = parse_expression(lexed, cursor + 1)?;
        return Ok((
            Expression::Starred(Box::new(value.clone()), star_span.to(value.span())),
            next,
        ));
    }
    parse_expression(lexed, cursor)
}

/// 游标处是不是「目标链 ＋ `=`」（**链式赋值**的判断；**不跨行** ✓）。
///
/// 目标链的形态与 `del`／元组目标同一口径：`名字` ＋ 任意串 `[键]`／`.名字` ✓。
fn looks_like_target_then_assign(lexed: &Lexed, cursor: usize) -> bool {
    let tokens = &lexed.lexemes;
    let mut index = cursor;
    if !matches!(tokens.get(index), Some(Lexeme::Name(_))) {
        return false;
    }
    index += 1;
    loop {
        match tokens.get(index) {
            Some(Lexeme::Dot) => {
                if !matches!(tokens.get(index + 1), Some(Lexeme::Name(_))) {
                    return false;
                }
                index += 2;
            }
            Some(Lexeme::LeftBracket) => {
                let mut depth = 0usize;
                loop {
                    match tokens.get(index) {
                        Some(Lexeme::LeftBracket) => {
                            depth += 1;
                            index += 1;
                        }
                        Some(Lexeme::RightBracket) => {
                            depth -= 1;
                            index += 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        Some(_) => index += 1,
                        None => return false,
                    }
                }
            }
            Some(Lexeme::Assign) => return true,
            _ => return false,
        }
    }
}

/// **条件位置的表达式**：允许**不带括号的海象**（实测参照允许 `if x := f():` ✓）。
///
/// `_bootstrap.py:547` 就是这种形态 ✗（我们此前只接了括号形式的 `(x := …)` ✓）。
/// **值位置的表达式**：允许**元组显示**（`a, b = 1, 2` ✓ —— 没有括号的逗号列表 ✓）。
fn parse_value_expression(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (first, mut next) = parse_expression(lexed, cursor)?;
    if lexed.lexemes.get(next) != Some(&Lexeme::Comma) {
        return Ok((first, next));
    }
    let mut items = vec![first];
    let mut end = items[0].span();
    loop {
        if lexed.lexemes.get(next) != Some(&Lexeme::Comma) {
            break;
        }
        next += 1;
        // 末尾逗号：`t = 1,`
        if matches!(
            lexed.lexemes.get(next),
            Some(Lexeme::Newline) | Some(Lexeme::End) | Some(Lexeme::Dedent) | None
        ) {
            break;
        }
        let (item, after) = parse_expression(lexed, next)?;
        end = item.span();
        items.push(item);
        next = after;
    }
    let span = items[0].span().to(end);
    Ok((Expression::TupleLiteral(items, span), next))
}

fn parse_condition(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    if let (Some(Lexeme::Name(name)), Some(Lexeme::Walrus)) =
        (lexed.lexemes.get(cursor), lexed.lexemes.get(cursor + 1))
    {
        let target = name.clone();
        let target_span = lexed.spans[cursor];
        let (value, next) = parse_expression(lexed, cursor + 2)?;
        let span = target_span.to(value.span());
        return Ok((
            Expression::Walrus {
                target,
                target_span,
                value: Box::new(value),
                span,
            },
            next,
        ));
    }
    parse_expression(lexed, cursor)
}

pub(super) fn parse_if_chain(
    lexed: &Lexed,
    cursor: usize,
    depth: usize,
    in_function: bool,
) -> Result<(Statement, usize), CompileError> {
    let tokens = &lexed.lexemes;
    let mut cursor_value = cursor;
    let cursor = &mut cursor_value;
    {

                    let keyword_span = lexed.spans[*cursor];
                    *cursor += 1;
                    let (condition, next) = parse_condition(lexed, *cursor)?;
                    *cursor = next;
                    if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                        return Err(CompileError::Syntax(format!("`if` 后面要冒号（第 {} 行，实际 {:?}）", lexed.spans[*cursor].line_start, tokens.get(*cursor)).to_owned()));
                    }
                    *cursor += 1;
                    if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                        return Err(CompileError::Syntax("`if` 的冒号后面要换行".to_owned()));
                    }
                    *cursor += 1;
                    if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                        return Err(CompileError::Syntax("`if` 的体要缩进".to_owned()));
                    }
                    *cursor += 1;
                    let then_body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                    if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                        return Err(CompileError::Syntax("`if` 的体没有正常收尾".to_owned()));
                    }
                    *cursor += 1;
                    // **`elif`**：参照实测与"`else:` 里套一个 `if`"**完全同形**（字节码逐条相同）
                    // ⇒ 按那个形状解析：递归再入 `if` 分支，产物放进 `else_body`
                    // （`elif` 在关键字表里没有 ⇒ 是 `Name("elif")`）
                    if tokens.get(*cursor) == Some(&Lexeme::Name("elif".to_owned())) {
                        let (nested, next) = parse_if_chain(lexed, *cursor, depth, in_function)?;
                        *cursor = next;
                        let else_body = vec![nested];
                        let body_end = statements_last_end(&else_body).unwrap_or(keyword_span);
                        return Ok((
                            Statement::If {
                                span: keyword_span.to(body_end),
                                condition,
                                then_body,
                                else_body,
                            },
                            *cursor,
                        ));
                    }
                    // `else` 可选：`else` `:` NEWLINE INDENT … DEDENT
                    let mut else_body: Vec<Statement> = Vec::new();
                    if tokens.get(*cursor) == Some(&Lexeme::Else) {
                        *cursor += 1;
                        if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                            return Err(CompileError::Syntax("`else` 后面要冒号".to_owned()));
                        }
                        *cursor += 1;
                        if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                            return Err(CompileError::Syntax("`else` 的冒号后面要换行".to_owned()));
                        }
                        *cursor += 1;
                        if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                            return Err(CompileError::Syntax("`else` 的体要缩进".to_owned()));
                        }
                        *cursor += 1;
                        else_body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                        if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                            return Err(CompileError::Syntax("`else` 的体没有正常收尾".to_owned()));
                        }
                        *cursor += 1;
                    }
                    let body_end = if else_body.is_empty() {
                        statements_last_end(&then_body)
                    } else {
                        statements_last_end(&else_body)
                    }
                    .unwrap_or(keyword_span);
                    return Ok((
                        Statement::If {
                            span: keyword_span.to(body_end),
                            condition,
                            then_body,
                            else_body,
                        },
                        *cursor,
                    ));
            
    }
}

pub(super) fn parse_statements(
    lexed: &Lexed,
    cursor: &mut usize,
    depth: usize,
    in_function: bool,
) -> Result<Vec<Statement>, CompileError> {
    let tokens = &lexed.lexemes;
    let mut statements = Vec::new();
    loop {
        while matches!(tokens.get(*cursor), Some(Lexeme::Newline)) {
            *cursor += 1;
        }
        // **装饰器**（`@<表达式>`，可叠）：先按源码序收集，随后只允许挂在 `def` 上 ✓
        // （`class` 的装饰器本轮未接 ✗ ⇒ 如实报错，不静默忽略）
        let mut decorators: Vec<Expression> = Vec::new();
        let mut decorators_first_line: Option<u32> = None;
        while matches!(tokens.get(*cursor), Some(Lexeme::At)) {
            if decorators_first_line.is_none() {
                decorators_first_line = Some(lexed.spans[*cursor].line_start);
            }
            *cursor += 1;
            let (expression, next) = parse_expression(lexed, *cursor)?;
            *cursor = next;
            decorators.push(expression);
            expect_statement_end(tokens, cursor)?;
            // **一行一条装饰器**：行尾换行要自己跳过（实测 `expect_statement_end` 之后游标仍停在
            // `Newline` 上 ✗ ⇒ `@a.b` 那种"属性表达式"会把它留给守卫，被误判成"不是 def" ✓）
            while matches!(tokens.get(*cursor), Some(Lexeme::Newline)) {
                *cursor += 1;
            }
        }
        if !decorators.is_empty() && !matches!(tokens.get(*cursor), Some(Lexeme::Def)) {
            return Err(CompileError::Unsupported(
                "装饰器只接线了 `def`（`class` 的装饰器随后补）".to_owned(),
            ));
        }
        match tokens.get(*cursor) {
            Some(Lexeme::End) => break,
            Some(Lexeme::Dedent) => {
                if depth == 0 {
                    return Err(CompileError::Syntax("多余的缩进收尾".to_owned()));
                }
                break;
            }
            // `class <名字> [(<基类…>)]: <体>`
            Some(Lexeme::Class) => {
                let class_span = lexed.spans[*cursor];
                let first_line = class_span.line_start;
                *cursor += 1;
                let name = match tokens.get(*cursor) {
                    Some(Lexeme::Name(name)) => name.clone(),
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "`class` 后面要名字，实际 {other:?}"
                        )))
                    }
                };
                *cursor += 1;
                // 基类（可省略括号；实测基类用 `LOAD_NAME` 压栈）
                let mut bases: Vec<Expression> = Vec::new();
                if tokens.get(*cursor) == Some(&Lexeme::LeftParen) {
                    *cursor += 1;
                    loop {
                        match tokens.get(*cursor) {
                            Some(Lexeme::RightParen) => {
                                *cursor += 1;
                                break;
                            }
                            Some(Lexeme::Comma) => {
                                *cursor += 1;
                            }
                            _ => {
                                let (expression, next) =
                                    parse_expression(lexed, *cursor)?;
                                *cursor = next;
                                bases.push(expression);
                            }
                        }
                    }
                }
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`class` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`class` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`class` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`class` 的体没有正常收尾".to_owned()));
                }
                let body_end = statements_last_end(&body).unwrap_or(class_span);
                let span = class_span.to(body_end);
                *cursor += 1;
                statements.push(Statement::Class {
                    name,
                    span,
                    first_line,
                    bases,
                    body,
                });
            }
            Some(Lexeme::Def) => {
                // **函数里嵌套 `def` 已接线**（第 278 轮）：解析不再拦；闭包（引用外层局部）
                // 由发射期如实报错。
                let def_span = lexed.spans[*cursor];
                let first_line = def_span.line_start;
                *cursor += 1;
                let name = match tokens.get(*cursor) {
                    Some(Lexeme::Name(name)) => name.clone(),
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "`def` 后面要名字，实际 {other:?}"
                        )))
                    }
                };
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::LeftParen) {
                    return Err(CompileError::Unsupported(
                        "`def` 只接线了带括号的形参表".to_owned(),
                    ));
                }
                *cursor += 1;
                // 形参表：`名字 [":" 注解] ["=" 默认值]`，逗号分隔
                // `*args`／`**kw` 已支持；裸 `*` 之后的**仅关键字形参**与 `**kw` 后面的形参仍未接
                let mut parameters: Vec<Parameter> = Vec::new();
                let mut kwonly: Vec<Parameter> = Vec::new();
                let mut varargs: Option<String> = None;
                let mut varkw: Option<String> = None;
                // 裸 `*`（或 `*args`）之后就是**仅关键字**形参
                let mut after_star = false;
                let mut expect_parameter = true;
                loop {
                    match tokens.get(*cursor) {
                        Some(Lexeme::RightParen) => {
                            *cursor += 1;
                            break;
                        }
                        // `*args`／`**kw`（`BC-56` 的签名元数据；`varnames` 排在最后两位）
                        Some(Lexeme::Star) => {
                            *cursor += 1;
                            // `*名字` ⇒ `*args`；裸 `*`（后面是逗号或 `)`）⇒ 只是引出仅关键字形参
                            if let Some(Lexeme::Name(name)) = tokens.get(*cursor) {
                                varargs = Some(name.clone());
                                *cursor += 1;
                            }
                            after_star = true;
                            expect_parameter = false;
                        }
                        // `/`：把**它前面**那些位置形参标成"仅位置"
                        Some(Lexeme::Slash) => {
                            *cursor += 1;
                            if after_star || parameters.iter().any(|item| item.posonly) {
                                return Err(CompileError::Syntax(
                                    "`/` 只能出现在位置形参之后、且只能出现一次".to_owned(),
                                ));
                            }
                            for parameter in parameters.iter_mut() {
                                parameter.posonly = true;
                            }
                            expect_parameter = false;
                        }
                        Some(Lexeme::DoubleStar) => {
                            *cursor += 1;
                            match tokens.get(*cursor) {
                                Some(Lexeme::Name(name)) => {
                                    varkw = Some(name.clone());
                                    *cursor += 1;
                                }
                                other => {
                                    return Err(CompileError::Syntax(format!(
                                        "`**` 后面要一个名字，实际 {other:?}"
                                    )))
                                }
                            }
                            expect_parameter = false;
                        }
                        Some(Lexeme::Name(parameter)) if expect_parameter => {
                            let name = parameter.clone();
                            *cursor += 1;
                            let mut annotation_span = None;
                            let annotation = if tokens.get(*cursor) == Some(&Lexeme::Colon) {
                                let begin = lexed.spans[*cursor + 1];
                                let (label, next) = parse_type_at(lexed, *cursor + 1)?;
                                annotation_span = Some(begin.to(lexed.spans[next - 1]));
                                *cursor = next;
                                Some(label)
                            } else {
                                None
                            };
                            // 默认值：`= <表达式>`（`BC-*`：默认值在 **def 那一刻**求值）
                            let default = if tokens.get(*cursor) == Some(&Lexeme::Assign) {
                                let (expression, next) = parse_expression(lexed, *cursor + 1)?;
                                *cursor = next;
                                Some(expression)
                            } else {
                                None
                            };
                            if varkw.is_some() {
                                return Err(CompileError::Syntax(
                                    "`**kw` 之后不能再有形参".to_owned(),
                                ));
                            }
                            let parameter = Parameter {
                                name,
                                posonly: false,
                                annotation,
                                annotation_span,
                                default,
                            };
                            if after_star {
                                kwonly.push(parameter);
                            } else {
                                parameters.push(parameter);
                            }
                            expect_parameter = false;
                        }
                        Some(Lexeme::Comma) if !expect_parameter => {
                            *cursor += 1;
                            expect_parameter = true;
                        }
                        other => {
                            return Err(CompileError::Syntax(format!("形参表里出现 {other:?}")))
                        }
                    }
                }
                // 返回注解：`-> 类型`
                let mut returns_span = None;
                let returns = if tokens.get(*cursor) == Some(&Lexeme::Arrow) {
                    let begin = lexed.spans[*cursor + 1];
                    let (label, next) = parse_type_at(lexed, *cursor + 1)?;
                    returns_span = Some(begin.to(lexed.spans[next - 1]));
                    *cursor = next;
                    Some(label)
                } else {
                    None
                };
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`def` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`def` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`def` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let body = parse_statements(lexed, cursor, depth + 1, true)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`def` 的体没有正常收尾".to_owned()));
                }
                // `def` 的整段：从 `def` 关键字到**体最后一行的行尾**（实测 `(1, 2, 0, 12)`）
                let body_end = statements_last_end(&body).unwrap_or(def_span);
                let span = def_span.to(body_end);
                *cursor += 1;
                // **有装饰器时，`first_line` 取第一条装饰器那一行**（实测：内层 code object 的
                // 行表首项是 `(1,1)`＝`@dec` 那行 ✓，而不是 `def` 那行的 `(2,2)` ✗）
                let first_line = decorators_first_line.unwrap_or(first_line);
                statements.push(Statement::Def {
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
                });
            }
            Some(Lexeme::For) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let (target, target_span) = match tokens.get(*cursor) {
                    Some(Lexeme::Name(name)) => (name.clone(), lexed.spans[*cursor]),
                    other => {
                        let span = lexed.spans[*cursor];
                        return Err(CompileError::Syntax(format!(
                            "`for` 后面要一个名字，实际 {other:?}（第 {} 行）",
                            span.line_start
                        )));
                    }
                };
                let first_target_span = target_span;
                *cursor += 1;
                // **元组目标**（第 118 轮）：`for n, line in …` ✓ —— 实测 `LOAD x; GET_ITER; FOR_ITER;
                //   UNPACK_SEQUENCE 2`（位点＝**整段目标** `n, line` ✓）⇒ 随后按目标序存 ✓。
                let mut tuple_targets: Vec<(String, Span)> = Vec::new();
                let mut last_target_span = first_target_span;
                if tokens.get(*cursor) == Some(&Lexeme::Comma) {
                    tuple_targets.push((target.clone(), first_target_span));
                    while tokens.get(*cursor) == Some(&Lexeme::Comma) {
                        *cursor += 1;
                        // 允许尾逗号（`for a, in …`）
                        if tokens.get(*cursor) == Some(&Lexeme::In) {
                            break;
                        }
                        match tokens.get(*cursor) {
                            Some(Lexeme::Name(name)) => {
                                let span = lexed.spans[*cursor];
                                tuple_targets.push((name.clone(), span));
                                last_target_span = span;
                                *cursor += 1;
                            }
                            other => {
                                let span = lexed.spans[*cursor];
                                return Err(CompileError::Syntax(format!(
                                    "`for` 的元组目标后面要名字，实际 {other:?}（第 {} 行）",
                                    span.line_start
                                )));
                            }
                        }
                    }
                }
                if tokens.get(*cursor) != Some(&Lexeme::In) {
                    let span = lexed.spans[*cursor];
                    return Err(CompileError::Syntax(format!(
                        "`for` 的名字后面要 `in`，实际 {:?}（第 {} 行）",
                        tokens.get(*cursor),
                        span.line_start
                    )));
                }
                let target_span = if tuple_targets.is_empty() {
                    first_target_span
                } else {
                    first_target_span.to(last_target_span)
                };
                *cursor += 1;
                let (iterable, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`for` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`for` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`for` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`for` 的体没有正常收尾".to_owned()));
                }
                *cursor += 1;
                let mut for_else: Vec<Statement> = Vec::new();
                if tokens.get(*cursor) == Some(&Lexeme::Else) {
                    let (parsed, next) = parse_else_block(lexed, *cursor, depth, in_function)?;
                    for_else = parsed;
                    *cursor = next;
                }
                let body_end = if for_else.is_empty() {
                    statements_last_end(&body)
                } else {
                    statements_last_end(&for_else)
                }
                .unwrap_or(keyword_span);
                statements.push(Statement::For {
                    tuple_targets,
                    span: keyword_span.to(body_end),
                    target,
                    target_span,
                    iterable,
                    body,
                    else_body: for_else,
                });
            }
            Some(Lexeme::While) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                // 与 `if` 同一条口径：条件位置**允许裸海象** ✓（一处真相 ✓）
                let (condition, next) = parse_condition(lexed, *cursor)?;
                *cursor = next;
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax(format!(
                        "`while` 后面要冒号（第 {} 行，实际 {:?}）",
                        lexed.spans[*cursor].line_start,
                        tokens.get(*cursor)
                    )));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`while` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`while` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`while` 的体没有正常收尾".to_owned()));
                }
                *cursor += 1;
                let mut while_else: Vec<Statement> = Vec::new();
                if tokens.get(*cursor) == Some(&Lexeme::Else) {
                    let (parsed, next) = parse_else_block(lexed, *cursor, depth, in_function)?;
                    while_else = parsed;
                    *cursor = next;
                }
                let body_end = if while_else.is_empty() {
                    statements_last_end(&body)
                } else {
                    statements_last_end(&while_else)
                }
                .unwrap_or(keyword_span);
                statements.push(Statement::While {
                    span: keyword_span.to(body_end),
                    condition,
                    body,
                    else_body: while_else,
                });
            }
            Some(Lexeme::Global) => {
                // `global a, b`（**不发任何指令** ✓；实测模块层与函数层都一样 ⇒ 效果落在
                //   **存储形态**上：`global a` 后 `a = 1` 发 `STORE_GLOBAL` ✓）
                let keyword_span = lexed.spans[*cursor];
                let mut names = Vec::new();
                let mut at = *cursor + 1;
                loop {
                    match lexed.lexemes.get(at) {
                        Some(Lexeme::Name(text)) => {
                            names.push(text.clone());
                            at += 1;
                        }
                        _ => break,
                    }
                    if lexed.lexemes.get(at) == Some(&Lexeme::Comma) {
                        at += 1;
                    } else {
                        break;
                    }
                }
                let last_span = lexed
                    .spans
                    .get(at.saturating_sub(1))
                    .copied()
                    .unwrap_or(keyword_span);
                statements.push(Statement::Global(names, keyword_span.to(last_span)));
                *cursor = at;
            }
            Some(Lexeme::Nonlocal) => {
                // `nonlocal a, b`（**不发任何指令** ✓；声明的作用在分析层承担）
                let keyword_span = lexed.spans[*cursor];
                let mut names = Vec::new();
                let mut at = *cursor + 1;
                loop {
                    match lexed.lexemes.get(at) {
                        Some(Lexeme::Name(text)) => {
                            names.push(text.clone());
                            at += 1;
                        }
                        _ => break,
                    }
                    if lexed.lexemes.get(at) == Some(&Lexeme::Comma) {
                        at += 1;
                    } else {
                        break;
                    }
                }
                let last_span = lexed.spans.get(at.saturating_sub(1)).copied().unwrap_or(keyword_span);
                statements.push(Statement::NonLocal(names, keyword_span.to(last_span)));
                *cursor = at;
            }
            Some(Lexeme::If) => {
                let (statement, next) = parse_if_chain(lexed, *cursor, depth, in_function)?;
                *cursor = next;
                statements.push(statement);
            }
            Some(Lexeme::Raise) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let bare = matches!(tokens.get(*cursor), Some(Lexeme::Newline) | None);
                let (value, cause) = if bare {
                    (None, None)
                } else {
                    let (value, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    let is_from = matches!(tokens.get(*cursor), Some(Lexeme::Name(name)) if name == "from");
                    let cause = if is_from {
                        let (cause, next) = parse_expression(lexed, *cursor + 1)?;
                        *cursor = next;
                        Some(cause)
                    } else {
                        None
                    };
                    (Some(value), cause)
                };
                let end = cause
                    .as_ref()
                    .map(|cause| cause.span())
                    .or_else(|| value.as_ref().map(|value| value.span()))
                    .unwrap_or(keyword_span);
                statements.push(Statement::Raise { value, cause, span: keyword_span.to(end) });
                expect_statement_end(tokens, cursor)?;
            }
            Some(Lexeme::Yield) => {
                // `yield` / `yield 表达式`（第 124 轮）：形态见发射臂 ✓
                let statement_span = lexed.spans[*cursor];
                *cursor += 1;
                let value = if matches!(
                    tokens.get(*cursor),
                    Some(Lexeme::Newline) | Some(Lexeme::End) | Some(Lexeme::Dedent) | None | Some(Lexeme::RightParen)
                ) {
                    None
                } else {
                    let (value, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    Some(value)
                };
                let end = value.as_ref().map(|item| item.span()).unwrap_or(statement_span);
                statements.push(Statement::Yield(value, statement_span.to(end)));
            }
            Some(Lexeme::Return) => {
                if !in_function {
                    return Err(CompileError::Syntax(
                        "模块级的 `return`（参照实现也是 SyntaxError）".to_owned(),
                    ));
                }
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                // **裸 `return`**（实测：`LOAD_CONST None; RETURN_VALUE`，两条**位点＝`return` 关键字** ✓）
                let (value, next) = if matches!(
                    tokens.get(*cursor),
                    Some(Lexeme::Newline) | Some(Lexeme::End) | Some(Lexeme::Dedent) | None
                ) {
                    (Expression::Constant(Constant::None, keyword_span), *cursor)
                } else {
                    parse_expression_list(lexed, *cursor)?
                };
                *cursor = next;
                // 实测：整条 `return …` 的位置从 `return` 起到表达式末尾
                let span = keyword_span.to(value.span());
                statements.push(Statement::Return(value, span));
                expect_statement_end(tokens, cursor)?;
            }
            // **`pass`**：实测**不产生任何指令**（连 `NOP` 都没有）⇒ 解析掉就行
            Some(Lexeme::Name(name)) if name == "break" => {
                let position = lexed.spans[*cursor];
                *cursor += 1;
                statements.push(Statement::Break(position));
                expect_statement_end(tokens, cursor)?;
            }
            Some(Lexeme::Name(name)) if name == "continue" => {
                let position = lexed.spans[*cursor];
                *cursor += 1;
                statements.push(Statement::Continue(position));
                expect_statement_end(tokens, cursor)?;
            }
            // **`del <目标> (, <目标>)*`**：目标用表达式解析（`Name`／`Attribute`／`Subscript` ✓），
            // 形状在**发射期**校验（别的形状如实报未接线 ✓）
            Some(Lexeme::Name(name)) if name == "del" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let mut targets = Vec::new();
                loop {
                    let (target, next) = parse_expression(lexed, *cursor)?;
                    targets.push(target);
                    *cursor = next;
                    if lexed.lexemes.get(*cursor) == Some(&Lexeme::Comma) {
                        *cursor += 1;
                        continue;
                    }
                    break;
                }
                let end = targets
                    .last()
                    .map(|target| target.span())
                    .unwrap_or(keyword_span);
                statements.push(Statement::Delete {
                    targets,
                    span: keyword_span.to(end),
                });
                expect_statement_end(tokens, cursor)?;
            }
            // **`assert <测试> [, <消息>]`**：3.14 实测形态见发射臂
            Some(Lexeme::Name(name)) if name == "assert" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let (test, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                let message = if matches!(tokens.get(*cursor), Some(Lexeme::Comma)) {
                    let (message, next) = parse_expression(lexed, *cursor + 1)?;
                    *cursor = next;
                    Some(message)
                } else {
                    None
                };
                let end = message
                    .as_ref()
                    .map(|message| message.span())
                    .unwrap_or_else(|| test.span());
                statements.push(Statement::Assert {
                    test,
                    message,
                    span: keyword_span.to(end),
                });
                expect_statement_end(tokens, cursor)?;
            }
            Some(Lexeme::Name(name)) if name == "pass" => {
                let position = lexed.spans[*cursor];
                *cursor += 1;
                statements.push(Statement::Pass(position));
                expect_statement_end(tokens, cursor)?;
            }
            // **`import <模块> [as <名字>] (, …)*`**（3.14 实测形态见发射臂）
            Some(Lexeme::Name(name)) if name == "import" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let mut items: Vec<(String, Option<String>)> = Vec::new();
                // 延迟初始化：内层循环第一轮就赋（跨度取**最后消费的那个 token**）
                let mut end;
                loop {
                    let mut module = String::new();
                    loop {
                        match tokens.get(*cursor) {
                            Some(Lexeme::Name(part)) => {
                                module.push_str(part);
                                end = lexed.spans[*cursor];
                                *cursor += 1;
                            }
                            other => {
                                return Err(CompileError::Syntax(format!(
                                    "`import` 后面要模块名，实际 {other:?}"
                                )))
                            }
                        }
                        if tokens.get(*cursor) == Some(&Lexeme::Dot) {
                            module.push('.');
                            *cursor += 1;
                        } else {
                            break;
                        }
                    }
                    let alias = if matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "as")
                    {
                        let Some(Lexeme::Name(alias)) = tokens.get(*cursor + 1) else {
                            return Err(CompileError::Syntax("`as` 后面要一个名字".to_owned()));
                        };
                        let alias = alias.clone();
                        end = lexed.spans[*cursor + 1];
                        *cursor += 2;
                        Some(alias)
                    } else {
                        None
                    };
                    items.push((module, alias));
                    if tokens.get(*cursor) == Some(&Lexeme::Comma) {
                        *cursor += 1;
                        continue;
                    }
                    break;
                }
                expect_statement_end(tokens, cursor)?;
                statements.push(Statement::Import {
                    items,
                    span: keyword_span.to(end),
                });
            }
            // **`from <点*><模块> import <名字表> | *`**
            Some(Lexeme::Name(name)) if name == "from" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let mut level = 0u8;
                while tokens.get(*cursor) == Some(&Lexeme::Dot) {
                    level += 1;
                    *cursor += 1;
                }
                let mut module = String::new();
                let mut end;
                loop {
                    match tokens.get(*cursor) {
                        // `import` 是**关键字**，不能当成模块名吃进来（`from . import b`）
                        Some(Lexeme::Name(part)) if part != "import" => {
                            module.push_str(part);
                            *cursor += 1;
                        }
                        _ => break,
                    }
                    if tokens.get(*cursor) == Some(&Lexeme::Dot) {
                        module.push('.');
                        *cursor += 1;
                    } else {
                        break;
                    }
                }
                if !matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "import") {
                    return Err(CompileError::Syntax("`from …` 后面要 `import`".to_owned()));
                }
                *cursor += 1;
                let parenthesized = tokens.get(*cursor) == Some(&Lexeme::LeftParen);
                if parenthesized {
                    *cursor += 1;
                }
                let star = tokens.get(*cursor) == Some(&Lexeme::Star);
                let mut names: Vec<(String, Option<String>)> = Vec::new();
                if star {
                    end = lexed.spans[*cursor];
                    *cursor += 1;
                } else {
                    loop {
                        let Some(Lexeme::Name(item)) = tokens.get(*cursor) else {
                            return Err(CompileError::Syntax(format!(
                                "`from … import` 后面要名字，实际 {:?}",
                                tokens.get(*cursor)
                            )));
                        };
                        let item = item.clone();
                        end = lexed.spans[*cursor];
                        *cursor += 1;
                        let alias =
                            if matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "as")
                            {
                                let Some(Lexeme::Name(alias)) = tokens.get(*cursor + 1) else {
                                    return Err(CompileError::Syntax(
                                        "`as` 后面要一个名字".to_owned(),
                                    ));
                                };
                                let alias = alias.clone();
                                end = lexed.spans[*cursor + 1];
                                *cursor += 2;
                                Some(alias)
                            } else {
                                None
                            };
                        names.push((item, alias));
                        if tokens.get(*cursor) == Some(&Lexeme::Comma) {
                            *cursor += 1;
                            continue;
                        }
                        break;
                    }
                }
                if parenthesized {
                    if tokens.get(*cursor) != Some(&Lexeme::RightParen) {
                        return Err(CompileError::Syntax(
                            "`from … import (…)` 少了 `)`".to_owned(),
                        ));
                    }
                    end = lexed.spans[*cursor];
                    *cursor += 1;
                }
                expect_statement_end(tokens, cursor)?;
                statements.push(Statement::ImportFrom {
                    module,
                    level,
                    names,
                    star,
                    span: keyword_span.to(end),
                });
            }
            Some(Lexeme::Name(name)) if name == "with" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let mut items = Vec::new();
                loop {
                    let (context, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    let target = if matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "as")
                    {
                        let Some(Lexeme::Name(identifier)) = tokens.get(*cursor + 1) else {
                            return Err(CompileError::Syntax(
                                "`as` 后面要一个目标名".to_owned(),
                            ));
                        };
                        let identifier = identifier.clone();
                        let identifier_span = lexed.spans[*cursor + 1];
                        *cursor += 2;
                        Some((identifier, identifier_span))
                    } else {
                        None
                    };
                    items.push((context, target));
                    if tokens.get(*cursor) == Some(&Lexeme::Comma) {
                        *cursor += 1;
                        continue;
                    }
                    break;
                }
                let (body, next) = parse_suite(lexed, *cursor, depth, in_function)?;
                *cursor = next;
                let body_end = statements_last_end(&body).unwrap_or(keyword_span);
                statements.push(Statement::With {
                    items,
                    body,
                    span: keyword_span.to(body_end),
                });
            }
            Some(Lexeme::Name(name)) if name == "try" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let (body, next) = parse_suite(lexed, *cursor, depth, in_function)?;
                *cursor = next;
                let mut handlers = Vec::new();
                while tokens.get(*cursor) == Some(&Lexeme::Name("except".to_owned())) {
                    let handler_span = lexed.spans[*cursor];
                    *cursor += 1;
                    let type_ = if tokens.get(*cursor) == Some(&Lexeme::Colon) {
                        None
                    } else {
                        let (first, first_next) = parse_expression(lexed, *cursor)?;
                        let first_span = first.span();
                        let mut types = vec![first];
                        let mut next = first_next;
                        // **逗号分隔的多异常**（第 122 轮）：`except A, B:` ✓ —— 3.x 仍接受，
                        //   语义同 `except (A, B):` ✓（实测上游 `site.py:601` 就是它）。
                        while tokens.get(next) == Some(&Lexeme::Comma) {
                            let (item, after) = parse_expression(lexed, next + 1)?;
                            types.push(item);
                            next = after;
                        }
                        *cursor = next;
                        if types.len() == 1 {
                            types.pop()
                        } else {
                            let last_span = types
                                .last()
                                .map(|item| item.span())
                                .unwrap_or(first_span);
                            Some(Expression::TupleLiteral(types, first_span.to(last_span)))
                        }
                    };
                    let name = if matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "as") {
                        let Some(Lexeme::Name(identifier)) = tokens.get(*cursor + 1) else {
                            return Err(CompileError::Syntax(
                                "`as` 后面要一个名字".to_owned(),
                            ));
                        };
                        let identifier = identifier.clone();
                        *cursor += 2;
                        Some(identifier)
                    } else {
                        None
                    };
                    let (handler_body, next) = parse_suite(lexed, *cursor, depth, in_function)?;
                    *cursor = next;
                    // 裸 `except:` **必须最后一条**（参照也是 `SyntaxError`）——
                    // 只在**本条 `try` 的处理块列表**内看下一条是不是 `except`（不是扫整个文件）
                    if type_.is_none()
                        && tokens.get(*cursor) == Some(&Lexeme::Name("except".to_owned()))
                    {
                        return Err(CompileError::Syntax(
                            "默认的 `except:` 必须是最后一条".to_owned(),
                        ));
                    }
                    let body_end = statements_last_end(&handler_body).unwrap_or(handler_span);
                    handlers.push(Handler {
                        type_,
                        name,
                        body: handler_body,
                        span: handler_span.to(body_end),
                    });
                }
                // `else:`（只有先有 `except` 才合法）
                let mut else_body: Vec<Statement> = Vec::new();
                if tokens.get(*cursor) == Some(&Lexeme::Else) {
                    if handlers.is_empty() {
                        return Err(CompileError::Syntax(
                            "`try` 的 `else` 前面必须有 `except`".to_owned(),
                        ));
                    }
                    *cursor += 1;
                    let (suite, next) = parse_suite(lexed, *cursor, depth, in_function)?;
                    *cursor = next;
                    else_body = suite;
                }
                // `finally:`（`try/finally` 允许**没有** `except`）
                let mut finally_body: Vec<Statement> = Vec::new();
                if matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "finally") {
                    *cursor += 1;
                    let (suite, next) = parse_suite(lexed, *cursor, depth, in_function)?;
                    *cursor = next;
                    finally_body = suite;
                }
                if handlers.is_empty() && finally_body.is_empty() {
                    return Err(CompileError::Syntax(
                        "`try` 后面至少要有一条 `except` 或 `finally`".to_owned(),
                    ));
                }
                // 跨度：从 `try` 到**最后一个套件**的末尾
                let mut body_end = statements_last_end(&body).unwrap_or(keyword_span);
                if let Some(handler) = handlers.last() {
                    body_end = handler.span;
                }
                if let Some(end) = statements_last_end(&else_body) {
                    body_end = end;
                }
                if let Some(end) = statements_last_end(&finally_body) {
                    body_end = end;
                }
                statements.push(Statement::Try {
                    body,
                    handlers,
                    else_body,
                    finally_body,
                    span: keyword_span.to(body_end),
                });
            }
            Some(Lexeme::Name(target)) => {
                let target = target.clone();
                let target_span = lexed.spans[*cursor];
                let statement_start = *cursor;
                // 先看是不是**调用**（表达式语句）：`f()`／`f(1)`
                if matches!(tokens.get(*cursor + 1), Some(Lexeme::LeftParen)) {
                    let (expression, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    let span = expression.span();
                    statements.push(Statement::Expression(expression, span));
                    expect_statement_end(tokens, cursor)?;
                    continue;
                }
                *cursor += 1;
                // **目标链**（第 222 轮统一）：`名字` 后接**任意串**的 `[键]` / `.名字`
                // （实测 `a[0].b = v`：值先压、再求目标链 `a[0]`、最后按**最后一跳**选
                //  `STORE_ATTR`／`STORE_SUBSCR`；增强赋值同理，中间多一次"取旧值"）
                let mut chain = Expression::Name(target.clone(), target_span);
                loop {
                    match tokens.get(*cursor) {
                        Some(Lexeme::Dot) => {
                            let name = match tokens.get(*cursor + 1) {
                                Some(Lexeme::Name(name)) => name.clone(),
                                other => {
                                    return Err(CompileError::Syntax(format!(
                                        "`.` 后面要名字，实际 {other:?}"
                                    )))
                                }
                            };
                            let span = chain.span().to(lexed.spans[*cursor + 1]);
                            chain = Expression::Attribute(Box::new(chain), name, span);
                            *cursor += 2;
                        }
                        Some(Lexeme::LeftBracket) => {
                            let begin = chain.span();
                            let (key, next) = parse_subscript_item(lexed, *cursor + 1)?;
                            if tokens.get(next) != Some(&Lexeme::RightBracket) {
                                return Err(CompileError::Syntax(format!(
                                    "`[` 之后要 `]`，实际 {:?}",
                                    tokens.get(next)
                                )));
                            }
                            let span = begin.to(lexed.spans[next]);
                            chain = Expression::Subscript(Box::new(chain), Box::new(key), span);
                            *cursor = next + 1;
                        }
                        _ => break,
                    }
                }
                // **元组解包赋值**（第 107 轮；实测形态见发射臂 ✓）：`a, b = x`／`a[0], b = x`／
                // `a, *b, c = x`。目标用**表达式**解析（与 `del` 同一口径 ✓），`*` 单独记一位 ✓。
                if tokens.get(*cursor) == Some(&Lexeme::Comma) {
                    let first_chain = chain.clone();
                    let mut targets: Vec<(Expression, bool)> = vec![(chain, false)];
                    let mut last_span = first_chain.span();
                    while tokens.get(*cursor) == Some(&Lexeme::Comma) {
                        *cursor += 1;
                        // `*目标`（星号只允许一个 ✓，在发射期核）
                        let starred = tokens.get(*cursor) == Some(&Lexeme::Star);
                        if starred {
                            *cursor += 1;
                        }
                        let (item, next) = parse_expression(lexed, *cursor)?;
                        *cursor = next;
                        last_span = item.span();
                        targets.push((item, starred));
                    }
                    if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                        let span = lexed.spans.get(*cursor).copied();
                        return Err(CompileError::Syntax(format!(
                            "元组目标之后要 `=`，实际 {:?}（第 {} 行）",
                            tokens.get(*cursor),
                            span.map(|span| span.line_start).unwrap_or(0)
                        )));
                    }
                    *cursor += 1;
                    // 值位置允许**元组显示**（`a, b = 1, 2` ✓）
                    let (value, next) = parse_value_expression(lexed, *cursor)?;
                    *cursor = next;
                    statements.push(Statement::AssignTuple {
                        targets,
                        value: value.clone(),
                        target_span: target_span.to(last_span),
                        span: target_span.to(value.span()),
                    });
                    expect_statement_end(tokens, cursor)?;
                    continue;
                }
                // **增强赋值**：三种目标各一套栈序（见 `Statement::AugAssign`）
                if let Some(Lexeme::AugAssign(operator)) = tokens.get(*cursor) {
                    let operator = *operator;
                    *cursor += 1;
                    let (value, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    let span = target_span.to(value.span());
                    let target = match chain {
                        Expression::Attribute(object, name, attribute_span) => {
                            AugTarget::Attribute {
                                object: *object,
                                name,
                                span: attribute_span,
                            }
                        }
                        Expression::Subscript(container, key, subscript_span) => {
                            AugTarget::Subscript {
                                container: *container,
                                key: *key,
                                target_span: subscript_span,
                                span,
                            }
                        }
                        Expression::Name(name, name_span) => AugTarget::Name(name, name_span),
                        _ => {
                            return Err(CompileError::Unsupported(
                                "这个形态还不支持增强赋值".to_owned(),
                            ))
                        }
                    };
                    statements.push(Statement::AugAssign {
                        target,
                        operator,
                        value,
                        span,
                    });
                    expect_statement_end(tokens, cursor)?;
                    continue;
                }
                // 目标链之后接 `(` ⇒ **方法调用的表达式语句**（`obj.method(…)`，实测常见）
                // ⇒ 整句按表达式重解析（此前只接线了 `名字(…)`，这种会误报"未接线"）
                if tokens.get(*cursor) == Some(&Lexeme::LeftParen) {
                    let (expression, next) = parse_expression(lexed, statement_start)?;
                    *cursor = next;
                    let span = expression.span();
                    statements.push(Statement::Expression(expression, span));
                    expect_statement_end(tokens, cursor)?;
                    continue;
                }
                if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                    // **裸名字当表达式语句**（第 118 轮）：目标链走完却没有 `=` ⇒
                    //   整句按表达式重解析 ✓（与上面"方法调用"那条同一手法 ✓）；
                    //   真解析不出来才往下报错 ✓。
                    if let Ok((expression, next)) = parse_expression(lexed, statement_start) {
                        if next > statement_start {
                            *cursor = next;
                            let span = expression.span();
                            statements.push(Statement::Expression(expression, span));
                            expect_statement_end(tokens, cursor)?;
                            continue;
                        }
                    }
                    // **报错自带位置与 token**（第 104 轮：第 87／102 轮同一课的第三次 ✓）
                    let span = lexed.spans[*cursor];
                    return Err(CompileError::Unsupported(format!(
                        "只接线了 `名字 = 表达式`（含目标链）／`名字 += …` 与 `return`；\
                         这里遇到 {:?}（第 {} 行，列 {}-{}）",
                        tokens.get(*cursor),
                        span.line_start,
                        span.col_start,
                        span.col_end
                    )));
                }
                *cursor += 1;
                // **链式赋值**（第 112 轮；形态见发射臂 ✓）：`=` 之后若还是「目标链 ＋ `=`」⇒ 收成一串
                if looks_like_target_then_assign(lexed, *cursor) {
                    let mut targets = vec![chain.clone()];
                    loop {
                        let (next_target, after) = parse_expression(lexed, *cursor)?;
                        *cursor = after;
                        targets.push(next_target);
                        if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                            break;
                        }
                        *cursor += 1;
                        if !looks_like_target_then_assign(lexed, *cursor) {
                            break;
                        }
                    }
                    let (value, after) = parse_value_expression(lexed, *cursor)?;
                    *cursor = after;
                    let span = target_span.to(value.span());
                    statements.push(Statement::AssignChained {
                        targets,
                        value,
                        span,
                    });
                    expect_statement_end(tokens, cursor)?;
                    continue;
                }
                let (value, next) = parse_expression_list(lexed, *cursor)?;
                *cursor = next;
                let span = target_span.to(value.span());
                // 目标链自身的跨度（要在 `match chain` 之前取，避免部分移动）
                let chain_span = chain.span();
                match chain {
                    Expression::Attribute(object, name, _) => {
                        // **含下标的链**取目标链那段的跨度（`a[0].b = v` ⇒ `(0,6)`），
                        // **纯属性链**取整条语句（`self.x = 1` ⇒ `(3,3,8,18)`）——第 241 轮实测
                        statements.push(Statement::AssignAttr {
                            object: *object,
                            name,
                            value,
                            // `span` 留整条语句（AST 层要用），发射走 `target_span`
                            span,
                            target_span: chain_span,
                        });
                    }
                    Expression::Subscript(container, key, subscript_span) => {
                        statements.push(Statement::AssignSubscript {
                            container: *container,
                            key: *key,
                            value,
                            target_span: subscript_span,
                            span,
                        });
                    }
                    _ => {
                        statements.push(Statement::Assign {
                            target,
                            target_span,
                            value,
                            span,
                        });
                    }
                }
                expect_statement_end(tokens, cursor)?;
            }
            // 字符串字面量单独成句：**文档字符串**那一条（作用域首句才当文档串；
            // 其余位置的常量表达式语句，参照实现也会**丢掉**——实测 `def f(): x = 1; "s"; return x`
            // 的 `co_consts` 里没有那个 `"s"`）
            // 字面量单独成句（`Str` 是文档串那条；`Int`／`Bytes` 是**裸表达式语句**，
            // 实测参照对纯常量表达式语句**不产生指令**——`def f(): x = 1; "s"; return x` 的
            // `co_consts` 里没有那个 `"s"`。第 281 轮把后两种也放进来（此前报"不认识的语句开头"）
            // **`(` 起头的语句**（第 111 轮）：`(a, b) = x`（带括号的元组目标 ✓）或普通的
            //   括号表达式语句 ✓。实测：`(a, b) = x` ⇒ `UNPACK_SEQUENCE 2`，**目标跨度含括号** ✓；
            //   `(a) = x` ⇒ 退化成普通赋值 ✓；`(a, b) = 1, 2` ⇒ 等长窥孔 ✓。
            Some(Lexeme::LeftParen) => {
                let (target_expression, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                if tokens.get(*cursor) == Some(&Lexeme::Assign) {
                    *cursor += 1;
                    let (value, after) = parse_value_expression(lexed, *cursor)?;
                    *cursor = after;
                    match target_expression {
                        Expression::TupleLiteral(items, paren_span) => {
                            let targets: Vec<(Expression, bool)> =
                                items.into_iter().map(|item| (item, false)).collect();
                            statements.push(Statement::AssignTuple {
                                targets,
                                value: value.clone(),
                                target_span: paren_span,
                                span: paren_span.to(value.span()),
                            });
                        }
                        Expression::Name(name, name_span) => {
                            statements.push(Statement::Assign {
                                target: name,
                                target_span: name_span,
                                value: value.clone(),
                                span: name_span.to(value.span()),
                            });
                        }
                        other => {
                            let span = other.span();
                            return Err(CompileError::Unsupported(format!(
                                "括号目标只接线了元组与单个名字（第 {} 行）",
                                span.line_start
                            )));
                        }
                    }
                    expect_statement_end(tokens, cursor)?;
                } else {
                    // 普通括号表达式语句 ✓
                    let span = target_expression.span();
                    statements.push(Statement::Expression(target_expression, span));
                    expect_statement_end(tokens, cursor)?;
                }
            }
            Some(Lexeme::Str(_)) | Some(Lexeme::Int(_)) | Some(Lexeme::Bytes(_)) => {
                let (expression, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                let span = expression.span();
                statements.push(Statement::Expression(expression, span));
                expect_statement_end(tokens, cursor)?;
            }
            other => {
                return Err({
                    let span = lexed.spans[*cursor];
                    CompileError::Syntax(format!(
                        "不认识的语句开头 {other:?}（第 {} 行，列 {}-{}）",
                        span.line_start, span.col_start, span.col_end
                    ))
                });
            }
        }
    }
    Ok(statements)
}

/// 一段语句的最后一条的末尾跨度（`def` 的整段要用它收尾）。
pub(super) fn statements_last_end(statements: &[Statement]) -> Option<Span> {
    statements.last().map(|statement| match statement {
        Statement::Assign { span, .. }
        | Statement::Return(_, span)
        | Statement::NonLocal(_, span)
        | Statement::Global(_, span)
        | Statement::Yield(_, span)
        | Statement::Expression(_, span)
        | Statement::Def { span, .. }
        | Statement::Class { span, .. }
        | Statement::Pass(span)
        | Statement::Assert { span, .. }
        | Statement::Delete { span, .. }
        | Statement::AssignTuple { span, .. }
        | Statement::AssignChained { span, .. }
        | Statement::Import { span, .. }
        | Statement::ImportFrom { span, .. }
        | Statement::With { span, .. }
        | Statement::Try { span, .. }
        | Statement::Break(span)
        | Statement::Continue(span)
        | Statement::AugAssign { span, .. }
        | Statement::AssignSubscript { span, .. }
        | Statement::AssignAttr { span, .. }
        | Statement::Raise { span, .. }
        | Statement::If { span, .. }
        | Statement::While { span, .. }
        | Statement::For { span, .. } => *span,
    })
}

/// 解析一个**缩进体**（`:` 换行 缩进 体 去缩进）；`cursor` 指着冒号。
pub(super) fn parse_suite(
    lexed: &Lexed,
    cursor: usize,
    depth: usize,
    in_function: bool,
) -> Result<(Vec<Statement>, usize), CompileError> {
    let tokens = &lexed.lexemes;
    let mut cursor = cursor;
    if tokens.get(cursor) != Some(&Lexeme::Colon) {
        return Err(CompileError::Syntax(format!(
            "这里要冒号，实际 {:?}（第 {} 行，列 {}-{}）",
            tokens.get(cursor),
            lexed.spans[cursor].line_start,
            lexed.spans[cursor].col_start,
            lexed.spans[cursor].col_end
        )));
    }
    cursor += 1;
    if tokens.get(cursor) != Some(&Lexeme::Newline) {
        return Err(CompileError::Syntax("冒号后面要换行".to_owned()));
    }
    cursor += 1;
    if tokens.get(cursor) != Some(&Lexeme::Indent) {
        return Err(CompileError::Syntax("体要缩进".to_owned()));
    }
    cursor += 1;
    let body = parse_statements(lexed, &mut cursor, depth + 1, in_function)?;
    if tokens.get(cursor) != Some(&Lexeme::Dedent) {
        return Err(CompileError::Syntax("体没有正常收尾".to_owned()));
    }
    Ok((body, cursor + 1))
}

/// 解析 `else: <换行> <缩进体>`（`if`／`for`／`while` 共用）；`cursor` 指着 `else`。
pub(super) fn parse_else_block(
    lexed: &Lexed,
    cursor: usize,
    depth: usize,
    in_function: bool,
) -> Result<(Vec<Statement>, usize), CompileError> {
    let tokens = &lexed.lexemes;
    let mut cursor = cursor + 1;
    if tokens.get(cursor) != Some(&Lexeme::Colon) {
        return Err(CompileError::Syntax("`else` 后面要冒号".to_owned()));
    }
    cursor += 1;
    if tokens.get(cursor) != Some(&Lexeme::Newline) {
        return Err(CompileError::Syntax("`else` 的冒号后面要换行".to_owned()));
    }
    cursor += 1;
    if tokens.get(cursor) != Some(&Lexeme::Indent) {
        return Err(CompileError::Syntax("`else` 的体要缩进".to_owned()));
    }
    cursor += 1;
    let body = parse_statements(lexed, &mut cursor, depth + 1, in_function)?;
    if tokens.get(cursor) != Some(&Lexeme::Dedent) {
        return Err(CompileError::Syntax("`else` 的体没有正常收尾".to_owned()));
    }
    Ok((body, cursor + 1))
}

/// 解析一个**注解类型**（`TS-31` 的边界标签；`BC-24` 的签名条目就是它）。
///
/// - 名字 ⇒ 类型标签（`Constant::Type`）：`int`／`str`／`list` 一类
/// - `Any` ⇒ 执行器认识的 `"Any"` 标签（`TS-28` 的双向相容）
/// - `None` ⇒ `NoneType`（`-> None` 的值就是 `None`；`NoneType` 在内建表里）
/// - `名字[内层]` ⇒ 复合标签 `(外类型, 内标签)`（深层档位按它递归，`TS-30` 的不变性落在外类型上）
pub(super) fn parse_type_at(lexed: &Lexed, cursor: usize) -> Result<(Constant, usize), CompileError> {
    let name = match lexed.lexemes.get(cursor) {
        Some(Lexeme::Name(name)) => name.clone(),
        other => {
            return Err(CompileError::Syntax(format!(
                "注解里要一个类型名，实际 {other:?}"
            )))
        }
    };
    let mut cursor = cursor + 1;
    let base = match name.as_str() {
        "Any" => Constant::Str("Any".to_owned()),
        "None" => Constant::Type("NoneType".to_owned()),
        _ => Constant::Type(name),
    };
    if lexed.lexemes.get(cursor) == Some(&Lexeme::LeftBracket) {
        let (inner, next) = parse_type_at(lexed, cursor + 1)?;
        cursor = next;
        if lexed.lexemes.get(cursor) != Some(&Lexeme::RightBracket) {
            return Err(CompileError::Syntax("注解的 `[` 没有收尾 `]`".to_owned()));
        }
        return Ok((Constant::Tuple(vec![base, inner]), cursor + 1));
    }
    Ok((base, cursor))
}

pub(super) fn expect_statement_end(tokens: &[Lexeme], cursor: &mut usize) -> Result<(), CompileError> {
    match tokens.get(*cursor) {
        Some(Lexeme::Newline) | Some(Lexeme::End) | Some(Lexeme::Dedent) => Ok(()),
        other => Err(CompileError::Syntax(format!("语句结尾多出了 {other:?}"))),
    }
}

/// 比较层（在 `+` 之上）：本层只接线**一次**比较，链式（`a < b < c`）如实报未接线。
/// 比较运算符的识别 ＋ 连带几个词之后要吃掉的**词数**（`is not`／`not in` 是两词）。
pub(super) fn comparison_operator(lexed: &Lexed, cursor: usize) -> Option<(CompareOperator, usize)> {
    match lexed.lexemes.get(cursor) {
        Some(Lexeme::Less) => Some((CompareOperator::Less, 1)),
        Some(Lexeme::LessEqual) => Some((CompareOperator::LessEqual, 1)),
        Some(Lexeme::EqualEqual) => Some((CompareOperator::Equal, 1)),
        Some(Lexeme::NotEqual) => Some((CompareOperator::NotEqual, 1)),
        Some(Lexeme::Greater) => Some((CompareOperator::Greater, 1)),
        Some(Lexeme::GreaterEqual) => Some((CompareOperator::GreaterEqual, 1)),
        // `is`／`is not`（实测：`IS_OP` 的 0／1）
        Some(Lexeme::Name(name)) if name == "is" => {
            if lexed.lexemes.get(cursor + 1) == Some(&Lexeme::Name("not".to_owned())) {
                Some((CompareOperator::IsNot, 2))
            } else {
                Some((CompareOperator::Is, 1))
            }
        }
        // `in`／`not in`（实测：`CONTAINS_OP` 的 0／1）；`in` 是**关键字单元**（`Lexeme::In`）
        Some(Lexeme::In) => Some((CompareOperator::In, 1)),
        // `not in`：`in` 是**关键字单元**（`Lexeme::In`）
        Some(Lexeme::Name(name))
            if name == "not" && lexed.lexemes.get(cursor + 1) == Some(&Lexeme::In) =>
        {
            Some((CompareOperator::NotIn, 2))
        }
        _ => None,
    }
}

/// 比较层（`<`／`<=`／`==`／`!=`／`>`／`>=`／`is`／`is not`／`in`／`not in`）。
pub(super) fn parse_comparison(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (left, cursor) = parse_bitwise_or(lexed, cursor)?;
    let Some((operator, width)) = comparison_operator(lexed, cursor) else {
        return Ok((left, cursor));
    };
    let (right, mut cursor) = parse_bitwise_or(lexed, cursor + width)?;
    // **链式比较**（`a < b < c`）：继续吃运算符，凑够两个以上就走 `ChainedCompare`
    let mut operands = vec![left, right];
    let mut operators = vec![operator];
    while let Some((next_operator, width)) = comparison_operator(lexed, cursor) {
        let (next_operand, next_cursor) = parse_bitwise_or(lexed, cursor + width)?;
        operators.push(next_operator);
        operands.push(next_operand);
        cursor = next_cursor;
    }
    if operands.len() == 2 {
        let span = operands[0].span().to(operands[1].span());
        let mut drain = operands.into_iter();
        let left = drain.next().expect("刚判过两个");
        let right = drain.next().expect("刚判过两个");
        return Ok((
            Expression::Compare(
                Box::new(left),
                operators.pop().expect("刚判过一个"),
                Box::new(right),
                span,
            ),
            cursor,
        ));
    }
    let span = operands
        .first()
        .expect("至少两个")
        .span()
        .to(operands.last().expect("至少两个").span());
    Ok((Expression::ChainedCompare { operands, operators, span }, cursor))
}

/// **`not` 层**（Python 的 `not_test`：`not` 比比较**松**、比 `and`／`or` **紧**）。
pub(super) fn parse_not_test(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    if lexed.lexemes.get(cursor) == Some(&Lexeme::Name("not".to_owned())) {
        let start = lexed.spans.get(cursor).copied().unwrap_or(Span::new(1, 1, 0, 0));
        let (operand, next) = parse_not_test(lexed, cursor + 1)?;
        let span = start.to(operand.span());
        return Ok((Expression::Not(Box::new(operand), span), next));
    }
    parse_comparison(lexed, cursor)
}

/// **`and` 层**（Python 的 `and_test`）。
pub(super) fn parse_and_test(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let start_span = lexed.spans[cursor];
    let (first, mut cursor) = parse_not_test(lexed, cursor)?;
    let mut values = vec![first];
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Name("and".to_owned())) {
        let (value, next) = parse_not_test(lexed, cursor + 1)?;
        values.push(value);
        cursor = next;
    }
    if values.len() == 1 {
        return Ok((values.pop().expect("刚判过长度"), cursor));
    }
    // **跨度取这次解析的 token 区间**（第 240 轮实测）：外层布尔链因此**含两端括号**
    // （`x = (a and b) or (c and d)` ⇒ `(4,26)`），而括号内的那层不含左括号
    // （内层 `a and b` ⇒ `(5,12)`）—— 用"操作数首尾"算会差一格
    let span = start_span.to(lexed.spans[cursor - 1]);
    Ok((
        Expression::BoolOp {
            conjunction: true,
            values,
            span,
        },
        cursor,
    ))
}

/// **`or` 层**（Python 的 `or_test`；表达式入口）。
pub(super) fn parse_or_test(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let start_span = lexed.spans[cursor];
    let (first, mut cursor) = parse_and_test(lexed, cursor)?;
    let mut values = vec![first];
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Name("or".to_owned())) {
        let (value, next) = parse_and_test(lexed, cursor + 1)?;
        values.push(value);
        cursor = next;
    }
    if values.len() == 1 {
        return Ok((values.pop().expect("刚判过长度"), cursor));
    }
    // **跨度取这次解析的 token 区间**（第 240 轮实测）：外层布尔链因此**含两端括号**
    // （`x = (a and b) or (c and d)` ⇒ `(4,26)`），而括号内的那层不含左括号
    // （内层 `a and b` ⇒ `(5,12)`）—— 用"操作数首尾"算会差一格
    let span = start_span.to(lexed.spans[cursor - 1]);
    Ok((
        Expression::BoolOp {
            conjunction: false,
            values,
            span,
        },
        cursor,
    ))
}

/// 表达式入口（`or` 层）。
pub(super) fn parse_expression(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    // **三元表达式** `a if b else c`（右结合 ⇒ `a if b else c if d else e` 的 else 分支再递归）
    let (value, cursor) = parse_or_test(lexed, cursor)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::If) {
        return Ok((value, cursor));
    }
    let (condition, cursor) = parse_or_test(lexed, cursor + 1)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::Else) {
        return Err(CompileError::Syntax(
            "三元表达式 `a if b else c` 缺 `else`".to_owned(),
        ));
    }
    let (else_value, cursor) = parse_expression(lexed, cursor + 1)?;
    let span = value.span().to(else_value.span());
    Ok((
        Expression::Conditional {
            condition: Box::new(condition),
            then_value: Box::new(value),
            else_value: Box::new(else_value),
            span,
        },
        cursor,
    ))
}

/// 解析**下标里的一项**：普通表达式，或者切片（`a[b:c]`／`a[b:c:d]`）。
///
/// 界全是常量（含缺省）时直接给 `Constant::Slice` —— 参照实测把它放进**常量池**
/// （`x = a[1:2]` ⇒ `LOAD_CONST slice(1, 2, None)`，且入表在 `None` **之前**）。
pub(super) fn parse_subscript_item(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(Expression, usize), CompileError> {
    let open = lexed
        .spans
        .get(cursor)
        .copied()
        .unwrap_or(Span::new(1, 1, 0, 0));
    let mut cursor = cursor;
    let mut lower = None;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::Colon) {
        let (expression, next) = parse_expression(lexed, cursor)?;
        lower = Some(Box::new(expression));
        cursor = next;
    }
    if lexed.lexemes.get(cursor) != Some(&Lexeme::Colon) {
        // 不是切片 ⇒ 必须是普通表达式
        let Some(lower) = lower else {
            return Err(CompileError::Syntax("下标里不能空着".to_owned()));
        };
        return Ok((*lower, cursor));
    }
    cursor += 1;
    let mut upper = None;
    if !matches!(
        lexed.lexemes.get(cursor),
        Some(&Lexeme::Colon) | Some(&Lexeme::RightBracket)
    ) {
        let (expression, next) = parse_expression(lexed, cursor)?;
        upper = Some(Box::new(expression));
        cursor = next;
    }
    let mut step = None;
    if lexed.lexemes.get(cursor) == Some(&Lexeme::Colon) {
        cursor += 1;
        if lexed.lexemes.get(cursor) != Some(&Lexeme::RightBracket) {
            let (expression, next) = parse_expression(lexed, cursor)?;
            step = Some(Box::new(expression));
            cursor = next;
        }
    }
    let last = lexed
        .spans
        .get(cursor.saturating_sub(1))
        .copied()
        .unwrap_or(open);
    let span = open.to(last);
    if let Some(constant) = constant_slice(&lower, &upper, &step)? {
        return Ok((Expression::Constant(constant, span), cursor));
    }
    Ok((
        Expression::SliceLiteral {
            lower,
            upper,
            step,
            span,
        },
        cursor,
    ))
}

/// 下标当"复合表达式"看吗？（影响存入与收尾的跨度，逐形态实测）
///
/// **不是**复合的只有一种：**两段非常量切片**（`a[:c]`／`a[b:c]`，参照发 `BINARY_SLICE`）

/// 三个界都是常量（或缺省）⇒ 给 `Constant::Slice`；只要有一段是**非常量**就给 `None`。
pub(super) fn constant_slice(
    lower: &Option<Box<Expression>>,
    upper: &Option<Box<Expression>>,
    step: &Option<Box<Expression>>,
) -> Result<Option<Constant>, CompileError> {
    let mut fields: [Option<i64>; 3] = [None, None, None];
    for (index, part) in [lower, upper, step].into_iter().enumerate() {
        let Some(expression) = part else {
            continue;
        };
        match fold_constant(expression)? {
            Some(Constant::Int(value)) => fields[index] = Some(value),
            // `a[None:2]` 那种：`None` 就是"缺"
            Some(Constant::None) => fields[index] = None,
            _ => return Ok(None),
        }
    }
    Ok(Some(Constant::Slice {
        start: fields[0],
        stop: fields[1],
        step: fields[2],
    }))
}

/// **表达式列表**：逗号分隔 ⇒ 元组字面量（实测 `x = 1, 2` 与 `x = (1, 2)` 同形）。
///
/// 只给**语句层**用（赋值右值、`return`）——调用实参有自己的解析（那里的逗号是分隔符）。
pub(super) fn parse_expression_list(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(Expression, usize), CompileError> {
    let (first, mut cursor) = parse_expression(lexed, cursor)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::Comma) {
        return Ok((first, cursor));
    }
    let mut items = vec![first];
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Comma) {
        cursor += 1;
        if matches!(
            lexed.lexemes.get(cursor),
            None | Some(Lexeme::Newline)
                | Some(Lexeme::RightParen)
                | Some(Lexeme::RightBracket)
                | Some(Lexeme::RightBrace)
        ) {
            break;
        }
        let (item, next) = parse_expression(lexed, cursor)?;
        items.push(item);
        cursor = next;
    }
    let span = items
        .first()
        .expect("至少一项")
        .span()
        .to(items.last().expect("至少一项").span());
    Ok((Expression::TupleLiteral(items, span), cursor))
}

/// 一层通用的**左结合**二元运算（`|`／`^`／`&`／`<<`／`>>`／`+`／`-`／`*`… 都走它）。
pub(super) fn parse_binary_level(
    lexed: &Lexed,
    cursor: usize,
    next_level: fn(&Lexed, usize) -> Result<(Expression, usize), CompileError>,
    operators: &[(Lexeme, BinaryOperator)],
) -> Result<(Expression, usize), CompileError> {
    let (mut left, mut cursor) = next_level(lexed, cursor)?;
    loop {
        let Some(lexeme) = lexed.lexemes.get(cursor) else {
            break;
        };
        let Some((_, operator)) = operators.iter().find(|(unit, _)| unit == lexeme) else {
            break;
        };
        let (right, next) = next_level(lexed, cursor + 1)?;
        let span = left.span().to(right.span());
        left = Expression::Binary(*operator, Box::new(left), Box::new(right), span);
        cursor = next;
    }
    Ok((left, cursor))
}

/// `|`（最低的算术位运算层）。
pub(super) fn parse_bitwise_or(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_bitwise_xor,
        &[(Lexeme::Pipe, BinaryOperator::BitOr)],
    )
}

/// `^`。
pub(super) fn parse_bitwise_xor(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_bitwise_and,
        &[(Lexeme::Caret, BinaryOperator::BitXor)],
    )
}

/// `&`。
pub(super) fn parse_bitwise_and(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_shift,
        &[(Lexeme::Ampersand, BinaryOperator::BitAnd)],
    )
}

/// `<<`／`>>`。
pub(super) fn parse_shift(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_sum,
        &[
            (Lexeme::LeftShift, BinaryOperator::LeftShift),
            (Lexeme::RightShift, BinaryOperator::RightShift),
        ],
    )
}

/// `+`／`-`（算术加减）。
pub(super) fn parse_sum(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_term,
        &[
            (Lexeme::Plus, BinaryOperator::Add),
            (Lexeme::Minus, BinaryOperator::Subtract),
        ],
    )
}

/// `*`／`/`／`//`／`%`（乘除族）。
pub(super) fn parse_term(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_factor,
        &[
            (Lexeme::Star, BinaryOperator::Multiply),
            (Lexeme::Slash, BinaryOperator::TrueDivide),
            (Lexeme::DoubleSlash, BinaryOperator::FloorDivide),
            (Lexeme::Percent, BinaryOperator::Remainder),
            (Lexeme::At, BinaryOperator::MatrixMultiply),
        ],
    )
}

/// 一元 `+`／`-`／`~`（Python 的 `factor`；它在 `**` **之上** ⇒ `-2 ** 2 == -4`）。
pub(super) fn parse_factor(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let operator = match lexed.lexemes.get(cursor) {
        Some(Lexeme::Minus) => UnaryOperator::Negative,
        Some(Lexeme::Plus) => UnaryOperator::Positive,
        Some(Lexeme::Tilde) => UnaryOperator::Invert,
        _ => return parse_power(lexed, cursor),
    };
    let (operand, next) = parse_factor(lexed, cursor + 1)?;
    let span = lexed
        .spans
        .get(cursor)
        .copied()
        .unwrap_or_else(|| operand.span())
        .to(operand.span());
    Ok((Expression::Unary(operator, Box::new(operand), span), next))
}

/// `**`（**右结合**，右侧可以是 `factor` ⇒ `2 ** -1` 也解析得动）。
pub(super) fn parse_power(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (left, cursor) = parse_atom(lexed, cursor)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::DoubleStar) {
        return Ok((left, cursor));
    }
    let (right, next) = parse_factor(lexed, cursor + 1)?;
    let span = left.span().to(right.span());
    Ok((
        Expression::Binary(BinaryOperator::Power, Box::new(left), Box::new(right), span),
        next,
    ))
}

/// **f-string**：把原文切成"字面段／插值段"；插值里的表达式按**内容起始列**平移跨度后重新词法解析。
pub(super) fn parse_fstring(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let Some(Lexeme::FStr {
        contents,
        offset,
        raw,
    }) = lexed.lexemes.get(cursor)
    else {
        unreachable!("只由 `FStr` 词法进入");
    };
    let span = lexed.spans[cursor];
    let parts = parse_fstring_parts(contents, *raw, span.line_start, *offset)?;
    if parts
        .iter()
        .all(|part| matches!(part, FStringPart::Literal { .. }))
    {
        let mut joined = String::new();
        for part in &parts {
            if let FStringPart::Literal { text, .. } = part {
                joined.push_str(text);
            }
        }
        // **纯字面量**的位点：取**内容**那一段（实测 `f"a"` ⇒ `(6,7)`、`f"{{}}"` ⇒ `(6,10)`）；
        // 内容为空（`f""`）才取整条字面量（`(4,7)`）
        let lowered_span = if contents.is_empty() {
            span
        } else {
            Span::new(
                span.line_start,
                span.line_start,
                *offset,
                *offset + contents.chars().count() as u32,
            )
        };
        return Ok(merge_fstring_tail(Expression::Str(joined, lowered_span), cursor + 1, lexed));
    }
    Ok(merge_fstring_tail(
        Expression::FString { parts, span },
        cursor + 1,
        lexed,
    ))
}

/// **f-string 后面紧跟普通字符串**（`f"a{c}" "b"`）：并进它的**最后一段**（第 285 轮）。
/// 参照实测：后接串并入最后一段字面量；`"a" f"b{c}"` 则是并入**第一段**（那一半在 `Str` 分支做）。
pub(super) fn merge_fstring_tail(mut value: Expression, mut cursor: usize, lexed: &Lexed) -> (Expression, usize) {
    let mut tail = String::new();
    let mut end_span = value.span();
    while let Some(Lexeme::Str(next_text)) = lexed.lexemes.get(cursor) {
        tail.push_str(next_text);
        if let Some(next_span) = lexed.spans.get(cursor) {
            end_span = *next_span;
        }
        cursor += 1;
    }
    if tail.is_empty() {
        return (value, cursor);
    }
    match &mut value {
        Expression::FString { parts, span } => {
            match parts.last_mut() {
                Some(FStringPart::Literal { text, span }) => {
                    // 后接那半对称：跨度延到后接串的终点
                    *span = span.to(end_span);
                    text.push_str(&tail);
                }
                _ => parts.push(FStringPart::Literal { text: tail, span: end_span }),
            }
            *span = span.to(end_span);
        }
        Expression::Str(text, span) => {
            text.push_str(&tail);
            *span = span.to(end_span);
        }
        _ => {}
    }
    (value, cursor)
}

/// 把 f-string 的**原文**切成段。`line`／`offset` 是原文所在行与**内容起始列**（平移跨度用）。
pub(super) fn parse_fstring_parts(
    contents: &str,
    raw: bool,
    line: u32,
    offset: u32,
) -> Result<Vec<FStringPart>, CompileError> {
    let characters: Vec<char> = contents.chars().collect();
    let mut index = 0usize;
    // **逐字符跟踪行列**（第 252 轮）：跨行 f-string 的每段位点都落到它真正所在的行列
    let mut line_now = line;
    let mut col_now = offset;
    // 把 `index` 推到 `$to`（不含），沿途更新行列
    macro_rules! walk {
        ($to:expr) => {{
            let to = $to;
            while index < to {
                if characters.get(index) == Some(&'\n') {
                    line_now += 1;
                    col_now = 0;
                } else {
                    col_now += 1;
                }
                index += 1;
            }
        }};
    }
    // `from`（含）到 `to`（不含）之后的行列（不动 `index`）
    let position_at = |mut at_line: u32, mut at_col: u32, from: usize, to: usize| {
        for at in from..to {
            if characters.get(at) == Some(&'\n') {
                at_line += 1;
                at_col = 0;
            } else {
                at_col += 1;
            }
        }
        (at_line, at_col)
    };
    let mut parts: Vec<FStringPart> = Vec::new();
    let mut literal = String::new();
    let mut literal_line = line;
    let mut literal_col = offset;
    while index < characters.len() {
        match characters[index] {
            '{' if characters.get(index + 1) == Some(&'{') => {
                if literal.is_empty() {
                    literal_line = line_now;
                    literal_col = col_now;
                }
                literal.push('{');
                walk!(index + 2);
            }
            '}' if characters.get(index + 1) == Some(&'}') => {
                if literal.is_empty() {
                    literal_line = line_now;
                    literal_col = col_now;
                }
                literal.push('}');
                walk!(index + 2);
            }
            '}' => {
                return Err(CompileError::Syntax(
                    "f-string: single '}' is not allowed".to_owned(),
                ))
            }
            '{' => {
                if !literal.is_empty() {
                    parts.push(FStringPart::Literal {
                        text: core::mem::take(&mut literal),
                        span: Span::new(literal_line, line_now, literal_col, col_now),
                    });
                }
                let open_line = line_now;
                let open_col = col_now;
                let mut depth = 1usize;
                let mut scan = index + 1;
                let mut separator: Option<usize> = None;
                while scan < characters.len() {
                    match characters[scan] {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        '!' | ':' if depth == 1 && separator.is_none() => separator = Some(scan),
                        _ => {}
                    }
                    scan += 1;
                }
                if scan >= characters.len() {
                    return Err(CompileError::Syntax("f-string: expecting '}'".to_owned()));
                }
                let (expression_end, conversion, spec_text, spec_offset) = match separator {
                    None => (scan, None, None, 0usize),
                    Some(at) if characters[at] == '!' => {
                        let Some(mark) = characters.get(at + 1) else {
                            return Err(CompileError::Syntax(
                                "f-string: missing conversion character".to_owned(),
                            ));
                        };
                        let conversion = match mark {
                            's' => 1u8,
                            'r' => 2,
                            'a' => 3,
                            other => {
                                return Err(CompileError::Syntax(format!(
                                    "f-string: invalid conversion character {other:?}"
                                )))
                            }
                        };
                        let mut after = at + 2;
                        let spec = if characters.get(after) == Some(&':') {
                            after += 1;
                            Some(characters[after..scan].iter().collect::<String>())
                        } else {
                            None
                        };
                        if after != scan && characters.get(after.wrapping_sub(1)) != Some(&':') {
                            return Err(CompileError::Syntax("f-string: expecting '}'".to_owned()));
                        }
                        (at, Some(conversion), spec, after)
                    }
                    Some(at) => {
                        let text: String = characters[at + 1..scan].iter().collect();
                        (at, None, Some(text), at + 1)
                    }
                };
                let (expression_line, expression_col) =
                    position_at(open_line, open_col, index, index + 1);
                let expression_text: String =
                    characters[index + 1..expression_end].iter().collect();
                let expression =
                    parse_fstring_expression(&expression_text, expression_line, expression_col)?;
                let spec = match spec_text {
                    None => None,
                    Some(text) if text.is_empty() => Some(Vec::new()),
                    Some(text) => {
                        let (spec_line, spec_col) =
                            position_at(open_line, open_col, index, spec_offset);
                        Some(parse_fstring_parts(&text, raw, spec_line, spec_col)?)
                    }
                };
                let spec_span = spec.as_ref().map(|_| {
                    // `spec_span` 的起点是**冒号那一列**（旧口径 `spec_offset - 1`）
                    let (start_line, start_col) =
                        position_at(open_line, open_col, index, spec_offset.saturating_sub(1));
                    let (end_line, end_col) = position_at(open_line, open_col, index, scan);
                    Span::new(start_line, end_line, start_col, end_col)
                });
                walk!(scan + 1);
                parts.push(FStringPart::Formatted {
                    expression,
                    conversion,
                    spec,
                    spec_span,
                    span: Span::new(open_line, line_now, open_col, col_now),
                });
                literal_line = line_now;
                literal_col = col_now;
            }
            // **转义**（非原始串）：解码进正文，跨度仍按**源**下标算（这就是"源偏移映射"）
            '\\' if !raw => {
                if literal.is_empty() {
                    literal_line = line_now;
                    literal_col = col_now;
                }
                let (decoded, consumed) = lex_string_escape(&characters, index)?;
                literal.push_str(&decoded);
                walk!(index + consumed);
            }
            other => {
                if literal.is_empty() {
                    literal_line = line_now;
                    literal_col = col_now;
                }
                literal.push(other);
                walk!(index + 1);
            }
        }
    }
    if !literal.is_empty() {
        parts.push(FStringPart::Literal {
            text: literal,
            span: Span::new(literal_line, line_now, literal_col, col_now),
        });
    }
    Ok(parts)
}

/// 解析 f-string 插值里的**表达式**：把那段文字重新词法，并把跨度按 `line`／`column` 平移。
pub(super) fn parse_fstring_expression(
    text: &str,
    line: u32,
    column: u32,
) -> Result<Expression, CompileError> {
    if text.contains('\n') {
        return Err(CompileError::Unsupported(
            "f-string 里跨行的表达式尚未接线".to_owned(),
        ));
    }
    let mut lexed = lex(text)?;
    for span in &mut lexed.spans {
        *span = Span::new(
            span.line_start + line - 1,
            span.line_end + line - 1,
            span.col_start + column,
            span.col_end + column,
        );
    }
    let (expression, next) = parse_expression(&lexed, 0)?;
    for lexeme in &lexed.lexemes[next..] {
        if !matches!(lexeme, Lexeme::Newline | Lexeme::Dedent | Lexeme::Indent | Lexeme::End) {
            return Err(CompileError::Syntax(format!(
                "f-string 里的表达式 {text:?} 没能整段解析（多出 {lexeme:?}，已读到第 {next} 个）"
            )));
        }
    }
    Ok(expression)
}

/// 解析推导式的**一层或多层生成器**（游标指向第一个 `for`）：
/// `for <目标> in <可迭代> [if <条件>]*` 重复出现就依次收下，返回停在收尾括号上的游标。
pub(super) fn parse_comprehension_generators(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(Vec<Generator>, usize), CompileError> {
    let mut generators = Vec::new();
    let mut cursor = cursor;
    while matches!(lexed.lexemes.get(cursor), Some(Lexeme::For)) {
        let (target, after_target) = parse_comprehension_target(lexed, cursor + 1)?;
        if !matches!(lexed.lexemes.get(after_target), Some(Lexeme::In)) {
            return Err(CompileError::Syntax(
                "推导式的 `for <目标>` 后面要 `in`".to_owned(),
            ));
        }
        let (iterable, next) = parse_or_test(lexed, after_target + 1)?;
        cursor = next;
        let mut conditions = Vec::new();
        while matches!(lexed.lexemes.get(cursor), Some(Lexeme::If)) {
            let (condition, next) = parse_or_test(lexed, cursor + 1)?;
            cursor = next;
            conditions.push(condition);
        }
        generators.push(Generator {
            target,
            iterable,
            conditions,
        });
    }
    Ok((generators, cursor))
}

/// 解析推导式的**目标**：`名字` 或 `名字, 名字`（元组目标）。返回目标与游标（停在 `in` 上）。
pub(super) fn parse_comprehension_target(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(ComprehensionTarget, usize), CompileError> {
    let Some(Lexeme::Name(first)) = lexed.lexemes.get(cursor) else {
        return Err(CompileError::Syntax(
            "推导式的 `for` 后面要一个目标名".to_owned(),
        ));
    };
    let mut items: Vec<(String, Span)> = vec![(first.clone(), lexed.spans[cursor])];
    let mut cursor = cursor + 1;
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Comma) {
        let Some(Lexeme::Name(next)) = lexed.lexemes.get(cursor + 1) else {
            return Err(CompileError::Syntax(
                "推导式的元组目标里要一个名字".to_owned(),
            ));
        };
        items.push((next.clone(), lexed.spans[cursor + 1]));
        cursor += 2;
    }
    let target = if items.len() == 1 {
        let (name, span) = items.pop().expect("刚判断过只有一个");
        ComprehensionTarget::Name(name, span)
    } else {
        ComprehensionTarget::Tuple(items)
    };
    Ok((target, cursor))
}

/// **`lambda 形参表: 表达式`**（3.14 实测形态）：形参语法与 `def` 同族但没有注解、没有 `/`；
/// 体是**一条表达式**（发射时就是"求值再 `RETURN_VALUE`"）。
pub(super) fn parse_lambda(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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
    let (body, next) = parse_expression(lexed, cursor)?;
    let span = keyword_span.to(body.span());
    Ok((
        Expression::Lambda {
            parameters,
            kwonly,
            varargs,
            varkw,
            body: Box::new(body),
            span,
        },
        next,
    ))
}

pub(super) fn parse_atom(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let span = lexed
        .spans
        .get(cursor)
        .copied()
        .unwrap_or(Span::new(1, 1, 0, 0));
    let (mut term, mut cursor) = match lexed.lexemes.get(cursor) {
        Some(Lexeme::Int(value)) => (Expression::Int(*value, span), cursor + 1),
        Some(Lexeme::Str(text)) => {
            // **隐式字符串拼接**（第 283 轮）：相邻字符串字面量**合成一个常量**——
            // 实测 `y = "a" "b" "c"` ⇒ `co_consts` 只有 `'abc'`（不产生任何拼接指令）。
            // 跨度取**首尾**两段（位置要比的话按这个走）。与 f-string 混排是另一码事，未接线。
            let mut merged = text.clone();
            let mut end_span = span;
            let mut cursor = cursor + 1;
            while let Some(Lexeme::Str(next_text)) = lexed.lexemes.get(cursor) {
                merged.push_str(next_text);
                if let Some(next_span) = lexed.spans.get(cursor) {
                    end_span = *next_span;
                }
                cursor += 1;
            }
            // **与前导 f-string 相邻**（`"a" f"b{c}"`）：参照把前导串并进 f-string 的**第一段**
            // （实测 ⇒ `LOAD_CONST 'ab'; … FORMAT_SIMPLE; BUILD_STRING 2`）。纯字面量的 f-string
            // 已被 `parse_fstring` 降级成普通串 ⇒ 走上面那条合并即可。
            if let Some(Lexeme::FStr { .. }) = lexed.lexemes.get(cursor) {
                let (fstring, after) = parse_fstring(lexed, cursor)?;
                match fstring {
                    Expression::FString { mut parts, span: fspan } => {
                        // 首段**本来就是字面量** ⇒ 并进去（实测 `"a" "b" f"c{d}"` 的常量池只有
                        // `'abc'`，不是 `'ab'` ＋ `'c'` 两段 ✗）；否则插一段新的。
                        match parts.first_mut() {
                            Some(FStringPart::Literal { text, span: head_span }) => {
                                // 跨度取"**前导串起点 → 原段终点**"（实测 `"a" "b" f"c{d}"` 的
                                // 合并段是 `(1,1,4,15)` ✗ 不是原段的 `(14,15)`）
                                let merged_span = span.to(*head_span);
                                let mut head = merged.clone();
                                head.push_str(text);
                                *text = head;
                                *head_span = merged_span;
                            }
                            _ => parts.insert(
                                0,
                                FStringPart::Literal {
                                    text: merged.clone(),
                                    span: span.to(end_span),
                                },
                            ),
                        }
                        return Ok((Expression::FString { parts, span: span.to(fspan) }, after));
                    }
                    Expression::Str(more, mspan) => {
                        merged.push_str(&more);
                        return Ok((Expression::Str(merged, span.to(mspan)), after));
                    }
                    other => return Ok((other, after)),
                }
            }
            (Expression::Str(merged, span.to(end_span)), cursor)
        }
        // **f-string**（第 238 轮）：切片成「字面段／插值段」，插值里的表达式按**相对列偏移**重新词法。
        // **相邻字面量拼接**（第 113 轮）：后随 **f-string**（`f"a{x}" f"b"` ✓）或**普通串**
        // （`f"a{x}" "b"` ✓）都要并进来 —— 实测上游 `_bootstrap.py:1426`／`site.py:210` 就卡在
        // 「相邻 **f-string ＋ f-string**」✗（此前后随 f-string 那条路只接在 `Str` 臂里 ✗）。
        Some(Lexeme::FStr { .. }) => {
            let (first, mut after) = parse_fstring(lexed, cursor)?;
            let first_span = first.span();
            let mut parts = match first {
                Expression::FString { parts, .. } => parts,
                other => return Ok((other, after)),
            };
            let mut tail_span = first_span;
            loop {
                match lexed.lexemes.get(after) {
                    Some(Lexeme::FStr { .. }) => {
                        // 整体跨度的**尾**取**词素跨度**（实测 `f"a{x}" f"b"` 的 `BUILD_STRING`
                        //   位点是 `(2,2,4,16)` ✓ ＝ 第二个字面量 `f"b"` 的整体范围 ✓，不是降级后
                        //   `Str` 的内容范围 `(14,15)` ✗）
                        let literal_span = lexed.spans[after];
                        let (next, next_after) = parse_fstring(lexed, after)?;
                        tail_span = literal_span;
                        match next {
                            Expression::FString { parts: more, .. } => parts.extend(more),
                            Expression::Str(text, span) => {
                                parts.push(FStringPart::Literal { text, span })
                            }
                            _ => break,
                        }
                        after = next_after;
                    }
                    Some(Lexeme::Str(text)) => {
                        let text = text.clone();
                        let span = lexed.spans[after];
                        tail_span = span;
                        parts.push(FStringPart::Literal { text, span });
                        after += 1;
                    }
                    _ => break,
                }
            }
            // **边界处的相邻字面段要合并**（实测 `f"a{x} " f"b{x}"` ⇒ 段是 `'a'`／插值／
            //   `' b'`／插值 ✓，不是 `' '` 与 `'b'` 两段 ✗）；合并段的跨度取**两段首尾** ✓
            //   （与 `"a" "b" f"c{d}"` 那条口径一致 ✓）。
            let mut merged_parts: Vec<FStringPart> = Vec::with_capacity(parts.len());
            for part in parts {
                match (merged_parts.last_mut(), part) {
                    (
                        Some(FStringPart::Literal { text, span: head_span }),
                        FStringPart::Literal {
                            text: tail,
                            span: tail_span,
                        },
                    ) => {
                        text.push_str(&tail);
                        *head_span = head_span.to(tail_span);
                    }
                    (_, part) => merged_parts.push(part),
                }
            }
            (
                Expression::FString {
                    parts: merged_parts,
                    span: first_span.to(tail_span),
                },
                after,
            )
        }
        Some(Lexeme::Bytes(value)) => (Expression::Bytes(value.clone(), span), cursor + 1),
        // **`None` 是常量**（实测：`x = None` ⇒ 常量表 `['None']`、`LOAD_CONST 0`）；
        // `True`／`False` 要等 `Constant::Bool`（下一轮）
        // **推导式**：`[<元素> for <目标> in <可迭代> [if <条件>]*]`
        Some(Lexeme::LeftBrace) => {
            let start = lexed.spans[cursor];
            let mut cursor = cursor + 1;
            let mut pairs = Vec::new();
            loop {
                if lexed.lexemes.get(cursor) == Some(&Lexeme::RightBrace) {
                    cursor += 1;
                    break;
                }
                let (key, next) = parse_expression(lexed, cursor)?;
                cursor = next;
                // **集合推导式**：`{<元素> for <目标> in <可迭代> [if <条件>]*}`
                if matches!(lexed.lexemes.get(cursor), Some(Lexeme::For)) {
                    let (target, after_target) = parse_comprehension_target(lexed, cursor + 1)?;
                    if !matches!(lexed.lexemes.get(after_target), Some(Lexeme::In)) {
                        return Err(CompileError::Syntax(
                            "推导式的 `for <目标>` 后面要 `in`".to_owned(),
                        ));
                    }
                    let (iterable, mut cursor) = parse_or_test(lexed, after_target + 1)?;
                    let mut conditions = Vec::new();
                    while matches!(lexed.lexemes.get(cursor), Some(Lexeme::If)) {
                        let (condition, next) = parse_or_test(lexed, cursor + 1)?;
                        cursor = next;
                        conditions.push(condition);
                    }
                    // **多重 `for`**：继续收下后面的层
                    let (extra, after) = parse_comprehension_generators(lexed, cursor)?;
                    cursor = after;
                    if lexed.lexemes.get(cursor) != Some(&Lexeme::RightBrace) {
                        return Err(CompileError::Syntax("推导式要以 `}` 收尾".to_owned()));
                    }
                    let span = start.to(lexed.spans[cursor]);
                    return Ok((
                        Expression::Comprehension {
                            kind: ComprehensionKind::Set,
                            element: Box::new(key),
                            value: None,
                            generators: {
                                let mut list = vec![Generator {
                                    target,
                                    iterable,
                                    conditions,
                                }];
                                list.extend(extra);
                                list
                            },
                            span,
                        },
                        cursor + 1,
                    ));
                }
                // **集合字面量**（`{a, b}`／`{a}`）：既不是 `:`（字典）也不是 `for`（集合推导式）
                if matches!(
                    lexed.lexemes.get(cursor),
                    Some(Lexeme::Comma) | Some(Lexeme::RightBrace)
                ) {
                    let mut items = vec![key];
                    loop {
                        match lexed.lexemes.get(cursor) {
                            Some(Lexeme::Comma) => cursor += 1,
                            Some(Lexeme::RightBrace) => {
                                cursor += 1;
                                break;
                            }
                            other => {
                                return Err(CompileError::Syntax(format!(
                                    "集合字面量里出现 {other:?}"
                                )))
                            }
                        }
                        if lexed.lexemes.get(cursor) == Some(&Lexeme::RightBrace) {
                            cursor += 1;
                            break;
                        }
                        let (item, next) = parse_star_or_expression(lexed, cursor)?;
                        cursor = next;
                        items.push(item);
                    }
                    let span = start.to(lexed.spans[cursor - 1]);
                    return Ok((Expression::SetLiteral(items, span), cursor));
                }
                if lexed.lexemes.get(cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax(format!(
                        "字典字面量里键之后要 `:`，实际 {:?}",
                        lexed.lexemes.get(cursor)
                    )));
                }
                cursor += 1;
                let (value, next) = parse_expression(lexed, cursor)?;
                cursor = next;
                // **字典推导式**：`{<键>: <值> for <目标> in <可迭代> [if <条件>]*}`
                if matches!(lexed.lexemes.get(cursor), Some(Lexeme::For)) {
                    let (target, mut cursor) = parse_comprehension_target(lexed, cursor + 1)?;
                    if !matches!(lexed.lexemes.get(cursor), Some(Lexeme::In)) {
                        return Err(CompileError::Syntax(
                            "推导式的 `for <目标>` 后面要 `in`".to_owned(),
                        ));
                    }
                    let (iterable, next) = parse_or_test(lexed, cursor + 1)?;
                    cursor = next;
                    let mut conditions = Vec::new();
                    while matches!(lexed.lexemes.get(cursor), Some(Lexeme::If)) {
                        let (condition, next) = parse_or_test(lexed, cursor + 1)?;
                        cursor = next;
                        conditions.push(condition);
                    }
                    // **多重 `for`**：继续收下后面的层
                    let (extra, after) = parse_comprehension_generators(lexed, cursor)?;
                    cursor = after;
                    if lexed.lexemes.get(cursor) != Some(&Lexeme::RightBrace) {
                        return Err(CompileError::Syntax("推导式要以 `}` 收尾".to_owned()));
                    }
                    let span = start.to(lexed.spans[cursor]);
                    return Ok((
                        Expression::Comprehension {
                            kind: ComprehensionKind::Dict,
                            element: Box::new(key),
                            value: Some(Box::new(value)),
                            generators: {
                                let mut list = vec![Generator {
                                    target,
                                    iterable,
                                    conditions,
                                }];
                                list.extend(extra);
                                list
                            },
                            span,
                        },
                        cursor + 1,
                    ));
                }
                pairs.push((key, value));
                match lexed.lexemes.get(cursor) {
                    Some(Lexeme::Comma) => cursor += 1,
                    Some(Lexeme::RightBrace) => {
                        cursor += 1;
                        break;
                    }
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "字典字面量里出现 {other:?}"
                        )))
                    }
                }
            }
            let span = start.to(lexed.spans[cursor - 1]);
            (Expression::Map(pairs, span), cursor)
        }
        Some(Lexeme::LeftParen) => {
            // `(` 起头三种：`()` 空元组、`(a)` **分组**（参照不多发指令 ⇒ 不加节点）、
            // `(a, b)`／`(a,)` 元组字面量（实测全常量折成常量，否则 `BUILD_TUPLE`）
            let open = lexed.spans[cursor];
            let mut cursor = cursor + 1;
            if lexed.lexemes.get(cursor) == Some(&Lexeme::RightParen) {
                return Ok((
                    Expression::TupleLiteral(Vec::new(), open.to(lexed.spans[cursor])),
                    cursor + 1,
                ));
            }
            // **生成器表达式**（第 125 轮）：`(<元素> for <目标> in <可迭代> [if <条件>]*)` ✓
            //   （先试解析一个表达式：紧接着 `for` 就是它 ✓，否则原样回退到元组／分组那条路 ✓）
            {
                let saved = cursor;
                if let Ok((element, after_element)) = parse_expression(lexed, cursor) {
                    if matches!(lexed.lexemes.get(after_element), Some(Lexeme::For)) {
                        let (target, after_target) =
                            parse_comprehension_target(lexed, after_element + 1)?;
                        if !matches!(lexed.lexemes.get(after_target), Some(Lexeme::In)) {
                            return Err(CompileError::Syntax(
                                "生成器表达式的 `for <目标>` 后面要 `in`".to_owned(),
                            ));
                        }
                        let (iterable, mut at) = parse_or_test(lexed, after_target + 1)?;
                        let mut conditions = Vec::new();
                        while matches!(lexed.lexemes.get(at), Some(Lexeme::If)) {
                            let (condition, next) = parse_or_test(lexed, at + 1)?;
                            at = next;
                            conditions.push(condition);
                        }
                        let (extra, after) = parse_comprehension_generators(lexed, at)?;
                        if lexed.lexemes.get(after) != Some(&Lexeme::RightParen) {
                            return Err(CompileError::Syntax(
                                "生成器表达式要以 `)` 收尾".to_owned(),
                            ));
                        }
                        let span = open.to(lexed.spans[after]);
                        let mut generators = vec![Generator {
                            target,
                            iterable,
                            conditions,
                        }];
                        generators.extend(extra);
                        return Ok((
                            Expression::Comprehension {
                                kind: ComprehensionKind::Generator,
                                element: Box::new(element),
                                value: None,
                                generators,
                                span,
                            },
                            after + 1,
                        ));
                    }
                }
                cursor = saved;
            }
            // **海象**（`(名字 := 表达式)`）：参照形态见发射臂 ✓
            if let (Some(Lexeme::Name(name)), Some(Lexeme::Walrus)) =
                (lexed.lexemes.get(cursor), lexed.lexemes.get(cursor + 1))
            {
                let target = name.clone();
                let target_span = lexed.spans[cursor];
                let (value, next) = parse_expression(lexed, cursor + 2)?;
                if lexed.lexemes.get(next) != Some(&Lexeme::RightParen) {
                    let span = lexed.spans.get(next).copied();
                    return Err(CompileError::Syntax(format!(
                        "海象表达式要有闭合的 `)`，实际 {:?}（第 {} 行）",
                        lexed.lexemes.get(next),
                        span.map(|span| span.line_start).unwrap_or(0)
                    )));
                }
                // **跨度不含括号**（实测  的  位点是 ＝`y := 3`
                // 那一段 ✓，不是含 `(` 的 ✗）
                let _ = open;
                let span = target_span.to(value.span());
                return Ok((
                    Expression::Walrus {
                        target,
                        target_span,
                        value: Box::new(value),
                        span,
                    },
                    next + 1,
                ));
            }
            let mut items = Vec::new();
            let mut saw_comma = false;
            loop {
                let (item, next) = parse_star_or_expression(lexed, cursor)?;
                items.push(item);
                cursor = next;
                if lexed.lexemes.get(cursor) != Some(&Lexeme::Comma) {
                    break;
                }
                saw_comma = true;
                cursor += 1;
                if lexed.lexemes.get(cursor) == Some(&Lexeme::RightParen) {
                    break;
                }
            }
            if lexed.lexemes.get(cursor) != Some(&Lexeme::RightParen) {
                let span = lexed.spans.get(cursor).copied();
                return Err(CompileError::Syntax(format!(
                    "括号没有闭合，实际 {:?}（第 {} 行，列 {}-{}）",
                    lexed.lexemes.get(cursor),
                    span.map(|span| span.line_start).unwrap_or(0),
                    span.map(|span| span.col_start).unwrap_or(0),
                    span.map(|span| span.col_end).unwrap_or(0),
                )));
            }
            let close = lexed.spans[cursor];
            let expression = if items.len() == 1 && !saw_comma {
                items.pop().expect("刚判过长度")
            } else {
                Expression::TupleLiteral(items, open.to(close))
            };
            return Ok((expression, cursor + 1));
        }
        Some(Lexeme::LeftBracket) => {
            let start = lexed.spans[cursor];
            // 这个位置的 `cursor` 是**不可变参数**（外层要到 match 之后才 `let (mut term, mut cursor)`）
            // ⇒ 这里遮蔽一个本地可变的
            let mut cursor = cursor + 1;
            let mut items = Vec::new();
            if lexed.lexemes.get(cursor) != Some(&Lexeme::RightBracket) {
                let (first, next) = parse_expression(lexed, cursor)?;
                cursor = next;
                // **清单推导式**（3.12+ 内联；实测骨架见发射臂）：`[<元素> for <目标> in <可迭代>
                // [if <条件>]*]`。多重 `for` 的融合指令选择属优化器细节 ⇒ 暂如实报未接线
                if matches!(lexed.lexemes.get(cursor), Some(Lexeme::For)) {
                    let (target, after_target) = parse_comprehension_target(lexed, cursor + 1)?;
                    if !matches!(lexed.lexemes.get(cursor + 2), Some(Lexeme::In))
                    {
                        return Err(CompileError::Syntax(
                            "推导式的 `for <目标>` 后面要 `in`".to_owned(),
                        ));
                    }
                    let (iterable, next) = parse_or_test(lexed, after_target + 1)?;
                    cursor = next;
                    let mut conditions = Vec::new();
                    while matches!(lexed.lexemes.get(cursor), Some(Lexeme::If)) {
                        let (condition, next) = parse_or_test(lexed, cursor + 1)?;
                        cursor = next;
                        conditions.push(condition);
                    }
                    // **多重 `for`**：继续收下后面的层
                    let (extra, after) = parse_comprehension_generators(lexed, cursor)?;
                    cursor = after;
                    if lexed.lexemes.get(cursor) != Some(&Lexeme::RightBracket) {
                        return Err(CompileError::Syntax("推导式要以 `]` 收尾".to_owned()));
                    }
                    let span = start.to(lexed.spans[cursor]);
                    return Ok((
                        Expression::Comprehension {
                            kind: ComprehensionKind::List,
                            element: Box::new(first),
                            value: None,
                            generators: {
                                let mut list = vec![Generator {
                                    target,
                                    iterable,
                                    conditions,
                                }];
                                list.extend(extra);
                                list
                            },
                            span,
                        },
                        cursor + 1,
                    ));
                }
                items.push(first);
                loop {
                    match lexed.lexemes.get(cursor) {
                        Some(Lexeme::Comma) => cursor += 1,
                        Some(Lexeme::RightBracket) => {
                            cursor += 1;
                            break;
                        }
                        other => {
                            return Err(CompileError::Syntax(format!(
                                "列表字面量里出现 {other:?}"
                            )))
                        }
                    }
                    if lexed.lexemes.get(cursor) == Some(&Lexeme::RightBracket) {
                        cursor += 1;
                        break;
                    }
                    let (item, next) = parse_star_or_expression(lexed, cursor)?;
                    cursor = next;
                    items.push(item);
                }
            } else {
                cursor += 1;
            }
            // 整段的跨度：实测 `BUILD_LIST` 那条取**整个列表**（`[` 到 `]`），不是只取 `[`
            let span = start.to(lexed.spans[cursor - 1]);
            (Expression::List(items, span), cursor)
        }
        // **`lambda`**（3.14 实测）：`lambda 形参表: 表达式`；返回一个函数对象
        Some(Lexeme::Name(name)) if name == "lambda" => parse_lambda(lexed, cursor)?,
        Some(Lexeme::Name(name)) if name == "None" => {
            (Expression::Constant(Constant::None, span), cursor + 1)
        }
        // `True`／`False` 同样是**常量**（实测：`x = True` ⇒ 常量表 `['True', 'None']`）
        Some(Lexeme::Name(name)) if name == "True" => {
            (Expression::Constant(Constant::Bool(true), span), cursor + 1)
        }
        Some(Lexeme::Name(name)) if name == "False" => {
            (Expression::Constant(Constant::Bool(false), span), cursor + 1)
        }
        Some(Lexeme::Name(name)) => (Expression::Name(name.clone(), span), cursor + 1),
        other => {
            let span = lexed.spans[cursor];
            return Err(CompileError::Syntax(format!(
                "表达式里出现 {other:?}（第 {} 行，列 {}-{}）",
                span.line_start, span.col_start, span.col_end
            )));
        }
    };
    // **统一后缀链**（第 221 轮）：`.`／`(`／`[` 按**任意顺序**串（`a[0].b`、`f()[0].b`）
    loop {
        if lexed.lexemes.get(cursor) == Some(&Lexeme::Dot) {
        let name = match lexed.lexemes.get(cursor + 1) {
            Some(Lexeme::Name(name)) => name.clone(),
            other => {
                return Err(CompileError::Syntax(format!(
                    "`.` 后面要名字，实际 {other:?}"
                )))
            }
        };
        let span = term.span().to(lexed.spans[cursor + 1]);
        term = Expression::Attribute(Box::new(term), name, span);
        cursor += 2;
            continue;
        }
        if lexed.lexemes.get(cursor) == Some(&Lexeme::LeftParen) {
        let callee_span = term.span();
        cursor += 1;
        let mut arguments = Vec::new();
        let mut star_arguments: Vec<Expression> = Vec::new();
        let mut keywords: Vec<(String, Expression)> = Vec::new();
        let mut dict_arguments: Vec<Expression> = Vec::new();
        loop {
            match lexed.lexemes.get(cursor) {
                Some(Lexeme::RightParen) => {
                    cursor += 1;
                    break;
                }
                _ => {}
            }
            // `*表达式`：本层只接线"位置实参之后、关键字之前"这一个位置
            if lexed.lexemes.get(cursor) == Some(&Lexeme::Star) {
                if !keywords.is_empty() || !dict_arguments.is_empty() {
                    return Err(CompileError::Unsupported(
                        "`*` 出现在关键字实参之后尚未接线".to_owned(),
                    ));
                }
                cursor += 1;
                let (value, next) = parse_expression(lexed, cursor)?;
                star_arguments.push(value);
                cursor = next;
                match lexed.lexemes.get(cursor) {
                    Some(Lexeme::Comma) => cursor += 1,
                    Some(Lexeme::RightParen) => {}
                    other => {
                        {
                            let span = lexed.spans[cursor];
                            return Err(CompileError::Syntax(format!(
                                "实参表里出现 {other:?}（第 {} 行，列 {}-{}）",
                                span.line_start, span.col_start, span.col_end
                            )));
                        }
                    }
                }
                continue;
            }
            // `**表达式`
            if lexed.lexemes.get(cursor) == Some(&Lexeme::DoubleStar) {
                cursor += 1;
                let (value, next) = parse_expression(lexed, cursor)?;
                dict_arguments.push(value);
                cursor = next;
                match lexed.lexemes.get(cursor) {
                    Some(Lexeme::Comma) => cursor += 1,
                    Some(Lexeme::RightParen) => {}
                    other => {
                        {
                            let span = lexed.spans[cursor];
                            return Err(CompileError::Syntax(format!(
                                "实参表里出现 {other:?}（第 {} 行，列 {}-{}）",
                                span.line_start, span.col_start, span.col_end
                            )));
                        }
                    }
                }
                continue;
            }
            // 关键字实参：`名字 = 表达式`
            if let (Some(Lexeme::Name(name)), Some(Lexeme::Assign)) =
                (lexed.lexemes.get(cursor), lexed.lexemes.get(cursor + 1))
            {
                let name = name.clone();
                cursor += 2;
                let (value, next) = parse_expression(lexed, cursor)?;
                keywords.push((name, value));
                cursor = next;
            } else {
                if matches!(
                    lexed.lexemes.get(cursor),
                    Some(Lexeme::Plus) | Some(Lexeme::Less) | Some(Lexeme::Greater)
                ) {
                    return Err(CompileError::Syntax("实参表里出现运算符".to_owned()));
                }
                let (argument, next) = parse_expression(lexed, cursor)?;
                // **实参位置的生成器表达式**（第 125 轮）：`f(x for x in y)` ✓ ——
                //   参照允许它**只作为唯一实参**（否则要加括号 ✓）⇒ 这里如实要求唯一 ✓。
                if matches!(lexed.lexemes.get(next), Some(Lexeme::For)) {
                    if !arguments.is_empty() || !keywords.is_empty() || !star_arguments.is_empty() {
                        return Err(CompileError::Unsupported(
                            "生成器表达式作为实参时必须是**唯一**实参（否则要加括号 ✓）".to_owned(),
                        ));
                    }
                    let (target, after_target) = parse_comprehension_target(lexed, next + 1)?;
                    if !matches!(lexed.lexemes.get(after_target), Some(Lexeme::In)) {
                        return Err(CompileError::Syntax(
                            "生成器表达式的 `for <目标>` 后面要 `in`".to_owned(),
                        ));
                    }
                    let (iterable, mut at) = parse_or_test(lexed, after_target + 1)?;
                    let mut conditions = Vec::new();
                    while matches!(lexed.lexemes.get(at), Some(Lexeme::If)) {
                        let (condition, after_condition) = parse_or_test(lexed, at + 1)?;
                        at = after_condition;
                        conditions.push(condition);
                    }
                    let (extra, after) = parse_comprehension_generators(lexed, at)?;
                    let span = argument.span();
                    let mut generators = vec![Generator {
                        target,
                        iterable,
                        conditions,
                    }];
                    generators.extend(extra);
                    arguments.push(Expression::Comprehension {
                        kind: ComprehensionKind::Generator,
                        element: Box::new(argument),
                        value: None,
                        generators,
                        span,
                    });
                    cursor = after;
                } else {
                    arguments.push(argument);
                    cursor = next;
                }
            }
            match lexed.lexemes.get(cursor) {
                Some(Lexeme::Comma) => cursor += 1,
                Some(Lexeme::RightParen) => {}
                other => {
                    {
                            let span = lexed.spans[cursor];
                            return Err(CompileError::Syntax(format!(
                                "实参表里出现 {other:?}（第 {} 行，列 {}-{}）",
                                span.line_start, span.col_start, span.col_end
                            )));
                        }
                }
            }
        }
        let closing = lexed
            .spans
            .get(cursor.saturating_sub(1))
            .copied()
            .unwrap_or(callee_span);
        term = Expression::Call {
            function: Box::new(term),
            arguments,
            star_arguments,
            keywords,
            dict_arguments,
            callee_span,
            span: callee_span.to(closing),
        };
            continue;
        }
        if lexed.lexemes.get(cursor) == Some(&Lexeme::LeftBracket) {
        let start = term.span();
        let (key, next) = parse_subscript_item(lexed, cursor + 1)?;
        if lexed.lexemes.get(next) != Some(&Lexeme::RightBracket) {
            return Err(CompileError::Syntax(format!(
                "`[` 之后要 `]`，实际 {:?}",
                lexed.lexemes.get(next)
            )));
        }
        let span = start.to(lexed.spans[next]);
        term = Expression::Subscript(Box::new(term), Box::new(key), span);
        cursor = next + 1;
            continue;
        }
        // 三个后缀都不是 ⇒ 链到头了
        break;
    }
    Ok((term, cursor))
}

// ---- 把编译产物装成真的 `CodeObject`（`P1-10` 与执行器／属性面的接缝） ----
