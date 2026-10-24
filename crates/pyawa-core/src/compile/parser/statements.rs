//! `parser` 的 `statements` 域：语句序列／块／`if` 链与语句收尾。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

pub(crate) fn parse_module(lexed: &Lexed) -> Result<Vec<Statement>, CompileError> {
    let mut cursor = 0usize;
    let statements = parse_statements(lexed, &mut cursor, 0, false, false)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::End) {
        return Err(CompileError::Syntax(format!(
            "模块结尾多出了 {:?}",
            lexed.lexemes.get(cursor)
        )));
    }
    Ok(statements)
}

pub(crate) fn parse_if_chain(
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
                    // **行内体也收** ✓（第 188 轮）。
                    let then_body = parse_body_after_colon(lexed, cursor, depth, in_function)?;
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
                        // **行内体也收** ✓（第 188 轮）：助手**已经**吃掉收尾的 `Dedent` ✓
                        // ⇒ 这里**不能**再查一次 ✗（先前替换时留了这三行 ⇒ 正常 `if/else` 全被带坏 ✗）。
                        else_body = parse_body_after_colon(lexed, cursor, depth, in_function)?;
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

pub(crate) fn parse_statements(
    lexed: &Lexed,
    cursor: &mut usize,
    depth: usize,
    in_function: bool,
    // **到行尾即止** ✓（第 174 轮）：单行体（`def f(): pass` ✓）用 ✓ —— 走到 `Newline` 就交给调用方 ✓。
    stop_at_newline: bool,
) -> Result<Vec<Statement>, CompileError> {
    let tokens = &lexed.lexemes;
    let mut statements = Vec::new();
    loop {
        // **EOF 正常收尾** ✓（真 bug 修 ✗）：源文件**末尾没有换行**时，游标会停在 `tokens.len()`
        // 上（词法层给了 `End`，但**行内体**那几处会把它当"行尾换行"多前移一格 ✗）——
        // 先前这里没有这一支 ⇒ 落进下面 `other` 臂的**裸索引** `lexed.spans[*cursor]` ⇒
        // **Rust panic**（`index out of bounds: the len is 13 but the index is 13` ✓）。
        // 实测原形 ✓：`printf 'for i in range(2): print(i)' > p.py && pyawa p.py`（参照输出 0／1 ✓）。
        // 到 EOF 就是"语句已收完" ✓ ⇒ 与 `Newline`／`End` 同等对待 ✓（返回已收的 statements ✓）。
        if tokens.get(*cursor).is_none() {
            break;
        }
        if stop_at_newline && matches!(tokens.get(*cursor), Some(Lexeme::Newline)) {
            break;
        }
        if stop_at_newline {
            // **行内体**：分号是"同一行内下一句"的分隔符 ✓ —— 只吃分号、**不吃**换行 ✗
            //（先前 `;` 在词法层折成 `Newline` ✗ ⇒ 行内体在第一个分号处就收尾 ⇒ 次行 `else`
            // 成孤儿 ✓；第 405 轮分开之后这里按两类分别收 ✓）。
            while matches!(tokens.get(*cursor), Some(Lexeme::Semicolon)) {
                *cursor += 1;
            }
            if matches!(tokens.get(*cursor), Some(Lexeme::Newline)) {
                break;
            }
        } else {
            // 缩进块里：分号与换行都只是"语句之间"的间隔 ✓
            while matches!(tokens.get(*cursor), Some(Lexeme::Newline) | Some(Lexeme::Semicolon)) {
                *cursor += 1;
            }
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
            expect_statement_end(lexed, cursor)?;
            // **一行一条装饰器**：行尾换行要自己跳过（实测 `expect_statement_end` 之后游标仍停在
            // `Newline` 上 ✗ ⇒ `@a.b` 那种"属性表达式"会把它留给守卫，被误判成"不是 def" ✓）
            while matches!(tokens.get(*cursor), Some(Lexeme::Newline)) {
                *cursor += 1;
            }
        }
        // 装饰器后面**只能**跟 `def`／`class`／`async def` ✓（`async` 在本层是**当名字**读的 ✓）。
        // **必须精确** ✓：早先写成"任何 `Name` 都放行" ✗ ⇒ `@deco` 后面跟普通语句时装饰器会被**静默丢掉** ✗。
        let decorator_target_ok = match tokens.get(*cursor) {
            Some(Lexeme::Def) | Some(Lexeme::Class) => true,
            Some(Lexeme::Name(name)) => name == "async",
            _ => false,
        };
        if !decorators.is_empty() && !decorator_target_ok {
            // **带上行号** ✓（第 220 轮）：不然只看到"装饰器"两个字，定位全靠猜 ✗。
            let line = decorators_first_line.unwrap_or(0);
            return Err(CompileError::Unsupported(format!(
                "装饰器只接线了 `def`／`class`（第 {line} 行那个装饰器后面跟的不是它们）"
            )));
        }
        // **`async def`**（第 175 轮）：把 `async` 当**透明修饰符** ✓（只接这一种形态 ✓）。
        //   **已登记的近似** ✗：本层没有协程 ✓ ⇒ 异步函数会被当**普通函数** ✓ —— 只为让
        //   `Lib/types.py` 里那种**只定义、不调用**的代码能过 ✓；真正的协程留待专门一轮 ✓。
        // **`async def`**（第 185 轮改口径 ✓）：记下"这是异步 def" ✓，稍后给它的体补一条 `yield` ✓
        // ⇒ 编成**生成器** ✓ —— 调用得到**未启动**的生成器对象 ✓（CPython 给 coroutine ✓ ⇒ **已登记的偏差** ✗），
        // 但 `close()`／`__iter__` 因此可用 ✓ —— `Lib/types.py` 与 `abc.py` 正卡 `None.close()` ✗。
        let mut async_def = false;
        // **本轮的 `async for` 标记**（第 306 轮）：进 `For` 那一支时带过去 ✓。
        let mut async_for = false;
        // **本轮的 `async with` 标记**（第 307 轮）。
        let mut async_with = false;
        if matches!(tokens.get(*cursor), Some(Lexeme::Name(name)) if name == "async") {
            if matches!(tokens.get(*cursor + 1), Some(Lexeme::Def)) {
                *cursor += 1;
                async_def = true;
            } else if matches!(tokens.get(*cursor + 1), Some(Lexeme::Name(word)) if word == "with") {
                // **`async with`**（第 307 轮）：同样只是把 `async` 吃掉 ✓。
                *cursor += 1;
                async_with = true;
            } else if matches!(tokens.get(*cursor + 1), Some(Lexeme::For)) {
                // **`async for`**（第 306 轮）：只是把 `async` 吃掉 ✓ —— 循环骨架由
                // `Statement::For` 的 `is_async` 那一支发射 ✓（`async with` 仍如实报未接线 ✓）。
                *cursor += 1;
                async_for = true;
            } else {
                return Err(CompileError::Unsupported(
                    "`async with` 尚未接线（`async def`／`async for` 已接）".to_owned(),
                ));
            }
        }
        // **`type X = …`**（PEP 695 别名语句，task-24 收口 ③ ✓）：`type` 是**软关键字** ✓ ——
        // 只有 `type NAME` 后面跟 `=` 或 `[` 时才当语句 ✓（`type(x)`／`type = 1` 照旧走表达式 ✓）。
        if matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "type")
            && matches!(tokens.get(*cursor + 1), Some(Lexeme::Name(_)))
            && matches!(
                tokens.get(*cursor + 2),
                Some(Lexeme::Assign) | Some(Lexeme::LeftBracket)
            )
        {
            let keyword_span = lexed.spans[*cursor];
            *cursor += 1;
            let name = match tokens.get(*cursor) {
                Some(Lexeme::Name(name)) => name.clone(),
                other => {
                    return Err(CompileError::Syntax(format!(
                        "`type` 后面要名字，实际 {other:?}"
                    )))
                }
            };
            let name_span = lexed.spans[*cursor];
            *cursor += 1;
            let type_params = parse_type_params(lexed, cursor)?;
            if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                return Err(CompileError::Syntax("`type` 别名语句里要 `=`".to_owned()));
            }
            *cursor += 1;
            let (value, after) = parse_value_expression(lexed, *cursor)?;
            *cursor = after;
            expect_statement_end(lexed, cursor)?;
            let span = keyword_span.to(value.span());
            statements.push(Statement::TypeAlias {
                name,
                name_span,
                type_params,
                value,
                span,
            });
            continue;
        }
        let mut flow = StatementFlow::Continue;
        match tokens.get(*cursor) {
            Some(Lexeme::End) => break,
            Some(Lexeme::Dedent) => {
                flow = parse_statement_dedent(lexed, cursor, depth)?;
            }
            // `class <名字> [(<基类…>)]: <体>`
            Some(Lexeme::Class) => {
                flow = parse_statement_class(lexed, cursor, depth, in_function, decorators.clone(), &mut statements)?;
            }
            Some(Lexeme::Def) => {
                flow = parse_statement_def(lexed, cursor, depth, decorators.clone(), decorators_first_line, async_def, &mut statements)?;
            }
            Some(Lexeme::For) => {
                flow = parse_statement_for_loop(lexed, cursor, depth, in_function, async_for, &mut statements)?;
            }
            Some(Lexeme::While) => {
                flow = parse_statement_while_loop(lexed, cursor, depth, in_function, &mut statements)?;
            }
            Some(Lexeme::Global) => {
                flow = parse_statement_global(lexed, cursor, &mut statements)?;
            }
            Some(Lexeme::Nonlocal) => {
                flow = parse_statement_nonlocal(lexed, cursor, &mut statements)?;
            }
            Some(Lexeme::If) => {
                flow = parse_statement_if_stmt(lexed, cursor, depth, in_function, &mut statements)?;
            }
            Some(Lexeme::Raise) => {
                flow = parse_statement_raise(lexed, cursor, &mut statements)?;
            }
            Some(Lexeme::Yield) => {
                flow = parse_statement_yield(lexed, cursor, &mut statements)?;
            }
            Some(Lexeme::Return) => {
                flow = parse_statement_return(lexed, cursor, in_function, &mut statements)?;
            }
            // **`pass`**：实测**不产生任何指令**（连 `NOP` 都没有）⇒ 解析掉就行
            Some(Lexeme::Name(name)) if name == "break" => {
                flow = parse_statement_kw_break(lexed, cursor, &mut statements)?;
            }
            Some(Lexeme::Name(name)) if name == "continue" => {
                flow = parse_statement_kw_continue(lexed, cursor, &mut statements)?;
            }
            // **`del <目标> (, <目标>)*`**：目标用表达式解析（`Name`／`Attribute`／`Subscript` ✓），
            // 形状在**发射期**校验（别的形状如实报未接线 ✓）
            Some(Lexeme::Name(name)) if name == "del" => {
                flow = parse_statement_kw_del(lexed, cursor, &mut statements)?;
            }
            // **`assert <测试> [, <消息>]`**：3.14 实测形态见发射臂
            Some(Lexeme::Name(name)) if name == "assert" => {
                flow = parse_statement_kw_assert(lexed, cursor, &mut statements)?;
            }
            Some(Lexeme::Name(name)) if name == "pass" => {
                flow = parse_statement_kw_pass(lexed, cursor, &mut statements)?;
            }
            // **`import <模块> [as <名字>] (, …)*`**（3.14 实测形态见发射臂）
            Some(Lexeme::Name(name)) if name == "import" => {
                flow = parse_statement_kw_import(lexed, cursor, &mut statements)?;
            }
            // **`from <点*><模块> import <名字表> | *`**
            Some(Lexeme::Name(name)) if name == "from" => {
                flow = parse_statement_kw_from(lexed, cursor, &mut statements)?;
            }
            Some(Lexeme::Name(name)) if name == "with" => {
                flow = parse_statement_kw_with(lexed, cursor, depth, in_function, async_with, &mut statements)?;
            }
            Some(Lexeme::Name(name)) if name == "try" => {
                flow = parse_statement_kw_try(lexed, cursor, depth, in_function, &mut statements)?;
            }
            Some(Lexeme::Name(_)) => {
                flow = parse_statement_name_target(lexed, cursor, depth, in_function, &mut statements)?;
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
                flow = parse_statement_left_paren(lexed, cursor, &mut statements)?;
            }
            // **`[` 起头的语句**（第 722 轮）：`[a, b] = x`（**列表目标**解包 ✓）或列表显示的
            //   表达式语句 ✓。实测 `[dd, mm, yy, tm, tz] = data` ⇒ `UNPACK_SEQUENCE 5`，
            //   目标跨度＝**整个列表显示**（含方括号 ✓），与 `(a, b) = x` 那条同一口径 ✓。
            //   动因：`Lib/email/_parseaddr.py:97` 正是它 ✗ —— 先前报"不认识的语句开头
            //   Some(LeftBracket)"✗，把 email 那一族压在下面 ✓。
            Some(Lexeme::LeftBracket) => {
                flow = parse_statement_left_bracket(lexed, cursor, &mut statements)?;
            }
            // **`*` 起头的语句**（第 722 轮）：`*a, b = x`（**首个目标带星**的解包 ✓）。
            //   实测 `Lib/email/utils.py:319` 的 `*dtuple, tz = parsed_date_tz` 正是它 ✗ ——
            //   先前报"不认识的语句开头 Some(Star)"✓。名字打头那支只覆盖 `a, *b, c = x`
            //   （星号不在首位 ✗）⇒ 这里补首星那一路 ✓，其余交给同一个尾巴函数 ✓。
            Some(Lexeme::Star) => {
                flow = parse_statement_star(lexed, cursor, &mut statements)?;
            }
            // `Lexeme::Dot` 也收 ✓（第 177 轮）：`class C: ...` 这类**表达式语句** ✓（单个 `.` 会由
            // 表达式解析器如实报错 ✓）。
            Some(Lexeme::Str(_))
            | Some(Lexeme::Int(_))
            | Some(Lexeme::BigInt(_))
            | Some(Lexeme::Float(_))
            | Some(Lexeme::Bytes(_))
            // **字面量表达式语句** ✓（task-25 ✓）：`{1: 2}`／`{1, 2}` 单独成句也合法
            // （官方 `test_dict.py`／`test_setcomps.py` 一族用它做"字面量表达式"用例 ✓）——
            // 先前报"不认识的语句开头 Some(LeftBrace)" ✗。
            | Some(Lexeme::LeftBrace)
            | Some(Lexeme::Dot) => {
                // 尾逗号也算元组显示（`1,`／`{1: 2},` ✓）
                let (expression, next) = parse_expression_list(lexed, *cursor)?;
                *cursor = next;
                let span = expression.span();
                statements.push(Statement::Expression(expression, span));
                expect_statement_end(lexed, cursor)?;
            }
            other => {
                return Err({
                    // **不许裸索引** ✗（同上真 bug）：`other` 可以是 EOF（`None` ✓）之外的任何词法
                    // 单元 ✓；跨度取不到就给**空跨度** ✓（游标已在 `End` 之后时不再 panic ✓）。
                    let span = lexed.spans.get(*cursor).copied();
                    CompileError::Syntax(format!(
                        "不认识的语句开头 {other:?}（第 {} 行，列 {}-{}）",
                        span.map(|span| span.line_start).unwrap_or(0),
                        span.map(|span| span.col_start).unwrap_or(0),
                        span.map(|span| span.col_end).unwrap_or(0)
                    ))
                });
            }
        }
        match flow {
            StatementFlow::Break => break,
            StatementFlow::Continue => {}
        }
    }
    Ok(statements)
}

/// 一段语句的最后一条的末尾跨度（`def` 的整段要用它收尾）。
pub(crate) fn statements_last_end(statements: &[Statement]) -> Option<Span> {
    statements.last().map(|statement| match statement {
        // **`match`**（第 290 轮）：跨度取**最后一条 `case` 的体**末尾 ✓（没有体就取整段 ✓）
        Statement::Match { cases, span, .. } => cases
            .iter()
            .rev()
            .find_map(|case| statements_last_end(&case.body))
            .unwrap_or(*span),
        Statement::Annotation { span, .. }
        | Statement::TypeAlias { span, .. }
        | Statement::Assign { span, .. }
        | Statement::YieldFrom(_, span)
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
        | Statement::NullCoalesceAssign { span, .. }
        | Statement::Raise { span, .. }
        | Statement::If { span, .. }
        | Statement::While { span, .. }
        | Statement::For { span, .. } => *span,
    })
}

/// 解析一个**缩进体**（`:` 换行 缩进 体 去缩进）；`cursor` 指着冒号。
/// **冒号之后的"体"** ✓（第 188 轮）：既可能是**缩进块** ✓，也可能是**同一行的简单语句** ✓
///（`if not m: return \'\'` ✓ —— `Lib/genericpath.py:107` 正是它 ✓；`def` 那边第 174 轮已接 ✓）。
///
/// 返回体，并把 `cursor` 停在**换行符之后** ✓。
pub(crate) fn parse_body_after_colon(
    lexed: &Lexed,
    cursor: &mut usize,
    depth: usize,
    in_function: bool,
) -> Result<Vec<Statement>, CompileError> {
    let tokens = &lexed.lexemes;
    if tokens.get(*cursor) != Some(&Lexeme::Newline) {
        // **行内体** ✓：简单语句，到行尾即止 ✓（`stop_at_newline` ✓）。
        let body = parse_statements(lexed, cursor, depth + 1, in_function, true)?;
        // **行内空体必须报错** ✓（`if True:` 冒号后直接 EOF ⇒ 参照 `IndentationError` ✓，
        // 不能静默编成空体 ✗ —— 与 for／while／def／class 同一条口径 ✓）。
        if body.is_empty() {
            return Err(CompileError::Syntax("体要缩进".to_owned()));
        }
        if tokens.get(*cursor) == Some(&Lexeme::Newline) {
            *cursor += 1;
        }
        return Ok(body);
    }
    *cursor += 1;
    if tokens.get(*cursor) != Some(&Lexeme::Indent) {
        return Err(CompileError::Syntax("体要缩进".to_owned()));
    }
    *cursor += 1;
    let body = parse_statements(lexed, cursor, depth + 1, in_function, false)?;
    if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
        return Err(CompileError::Syntax("体没有正常收尾".to_owned()));
    }
    *cursor += 1;
    Ok(body)
}

pub(crate) fn parse_suite(
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
    let body = parse_statements(lexed, &mut cursor, depth + 1, in_function, false)?;
    if tokens.get(cursor) != Some(&Lexeme::Dedent) {
        return Err(CompileError::Syntax("体没有正常收尾".to_owned()));
    }
    Ok((body, cursor + 1))
}

/// **冒号之后的套件**（task-25 ✓）：缩进块或**行内简单语句**，`cursor` 指着冒号。
///
/// `parse_suite` 只接缩进块 ✗ ⇒ `try: x = 1`／`with a: pass`／`else: self.fail(…)` 这类
/// **合法行内套件**报"冒号后面要换行" ✗（官方套件 8 个测试 ＋ 135 个签名档在这一格）。
/// 统一到已有的 `parse_body_after_colon`（`if`／`def` 那一族早就在用 ✓）。
/// 返回后 `cursor` 停在**换行之后** ✓（与 `parse_suite` 的收尾口径一致 ✓）。
pub(crate) fn parse_colon_suite(
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
    let body = parse_body_after_colon(lexed, &mut cursor, depth, in_function)?;
    Ok((body, cursor))
}

/// 解析 `else: <换行> <缩进体>`（`if`／`for`／`while` 共用）；`cursor` 指着 `else`。
pub(crate) fn parse_else_block(
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
    // **行内也收** ✓（task-25 ✓）：`for …: … / else: self.fail(…)` —— 官方套件里就是它 ✗。
    let body = parse_body_after_colon(lexed, &mut cursor, depth, in_function)?;
    Ok((body, cursor))
}

/// 一条语句之后必须**到此为止**（换行／EOF／退回缩进 ✓）。
///
/// **带位置** ✓（第 283 轮）：先前只报词元 ✗ ⇒ 对着 `Lib/` 里几千行的文件**无从下手** ✓ ——
/// 上游那几个"语句结尾多出了 `Some(…)`"的模块（`asyncio`／`multiprocessing.context` 一族 ✓）
/// 就是靠这行位置定到**具体哪一句**的 ✓。
pub(crate) fn expect_statement_end(lexed: &Lexed, cursor: &mut usize) -> Result<(), CompileError> {
    match lexed.lexemes.get(*cursor) {
        // **`;` 也是语句结尾** ✓（简单语句分隔符；调用方的循环会把它吃掉 ✓）
        Some(Lexeme::Newline) | Some(Lexeme::End) | Some(Lexeme::Dedent) | Some(Lexeme::Semicolon) => {
            Ok(())
        }
        other => {
            let span = lexed.spans[*cursor];
            Err(CompileError::Syntax(format!(
                "语句结尾多出了 {other:?}（第 {} 行，列 {}-{}）",
                span.line_start, span.col_start, span.col_end
            )))
        }
    }
}

/// `parse_statements` 的语句族：拆出的每个分支返回 [`StatementFlow`]（Continue ≈ 原 fallthrough）。
enum StatementFlow {
    Continue,
    Break,
}

/// 从 `parse_statements` 的 `Some(Lexeme::Dedent)` 分支原样搬入。
fn parse_statement_dedent(lexed: &Lexed, cursor: &mut usize, depth: usize) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Dedent) => {
                if depth == 0 {
                    return Err(CompileError::Syntax("多余的缩进收尾".to_owned()));
                }
                return Ok(StatementFlow::Break);
        }
        _ => unreachable!("parse_statement_dedent 只接收对应分支"),
    }
}

/// 从 `parse_statements` 的 `Some(Lexeme::Class)` 分支原样搬入。
fn parse_statement_class(lexed: &Lexed, cursor: &mut usize, depth: usize, in_function: bool, decorators: Vec<Expression>, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
                // **PEP 695 类型参数表**（task-24 ✓）：`class C[T, U](…)` 的 `[T, U]` ✓
                let type_params = parse_type_params(lexed, cursor)?;
                // 基类（可省略括号；实测基类用 `LOAD_NAME` 压栈）
                let mut bases: Vec<Expression> = Vec::new();
                let mut class_keywords: Vec<(String, Expression)> = Vec::new();
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
                                // **类关键字**（第 157 轮）：`class C(B, metaclass=M):` ✓
                                //   本层只接 `metaclass`（其余**如实报未接线** ✗，不静默丢掉 ✓）。
                                if let (Some(Lexeme::Name(key)), Some(Lexeme::Assign)) =
                                    (tokens.get(*cursor), tokens.get(*cursor + 1))
                                {
                                    let key = key.clone();
                                    let (value, after) = parse_expression(lexed, *cursor + 2)?;
                                    *cursor = after;
                                    // **类关键字全收** ✓（第 292 轮）：`metaclass=` 之外的
                                    // 由运行期**原样转交**给元类的 `__new__`／`__init__` ✓
                                    //（参照口径 ✓；`Lib/enum.py:1400` 的 `class Flag(Enum, boundary=STRICT)`
                                    // 与 `Lib/typing.py` 的 `_root=` 都靠它 ✓）。
                                    class_keywords.push((key, value));
                                    continue;
                                }
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
                    // **单行体也接** ✓（第 174 轮）：`class …: pass` 这种 ✓ —— 冒号后不是换行就是单行体 ✓。
                    let inline = tokens.get(*cursor) != Some(&Lexeme::Newline);
                    if !inline {
                        *cursor += 1;
                        if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                            return Err(CompileError::Syntax("class 的体要缩进".to_owned()));
                        }
                        *cursor += 1;
                    }
                    let body = parse_statements(lexed, cursor, depth + 1, in_function, inline)?;
                // **行内空体必须报错** ✓（与缩进体的"要缩进"同一条 ✓）：`class C:` 冒号后直接
                // 文件尾 ⇒ 参照报 `IndentationError` ✓ ⇒ 我们给普通 `CompileError` ✓，**不静默接受** ✗。
                if inline && body.is_empty() {
                    return Err(CompileError::Syntax("class 的体要缩进".to_owned()));
                }
                let body_end = statements_last_end(&body).unwrap_or(class_span);
                let span = class_span.to(body_end);
                // **只在真停在收尾词上才前移** ✓：行内体停在 `Newline` ✓、缩进体停在 `Dedent` ✓；
                // 源末无换行时停在 `End` ✓ ⇒ **不吃** ✓（先前无条件 `+= 1` ⇒ 游标越过 `End` ⇒
                // `parse_module` 报"模块结尾多出了 None" ✗）。
                if matches!(tokens.get(*cursor), Some(Lexeme::Newline) | Some(Lexeme::Dedent)) {
                    *cursor += 1;
                }
                statements.push(Statement::Class {
                decorators: decorators.clone(),
                    name,
                    span,
                    first_line,
                    bases,
                    keywords: class_keywords,
                    body,
                    type_params,
                });
        }
        _ => unreachable!("parse_statement_class 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Def)` 分支原样搬入。
fn parse_statement_def(lexed: &Lexed, cursor: &mut usize, depth: usize, decorators: Vec<Expression>, decorators_first_line: Option<u32>, async_def: bool, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
                // **PEP 695 类型参数表**（task-24 ✓）：`def f[T](…)` 的 `[T]` ✓
                let type_params = parse_type_params(lexed, cursor)?;
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
                                // **`*args: 注解`**（第 299 轮修）：先前只认名字 ✗ ⇒
                                // `def f(*args: int, **kw: str)` 报"形参表里出现 Some(Colon)" ✗
                                //（`Lib/test/support/__init__.py` 那一族 **26** 个模块的首个卡点 ✓）。
                                //  注解本身**解析掉、不登记** ✓——与 `**kw` 同口径（本层 `varargs`／`varkw`
                                //  只留名字 ✓；函数 `__annotations__` 那面另记 ✓）。
                                if tokens.get(*cursor) == Some(&Lexeme::Colon) {
                                    let (_, next) = parse_type_at(lexed, *cursor + 1)?;
                                    *cursor = next;
                                }
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
                                    // **`**kw: 注解`**（同上 ✓）
                                    if tokens.get(*cursor) == Some(&Lexeme::Colon) {
                                        let (_, next) = parse_type_at(lexed, *cursor + 1)?;
                                        *cursor = next;
                                    }
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
                    // **单行体也接** ✓（第 174 轮）：`def …: pass` 这种 ✓ —— 冒号后不是换行就是单行体 ✓。
                    let inline = tokens.get(*cursor) != Some(&Lexeme::Newline);
                    if !inline {
                        *cursor += 1;
                        if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                            return Err(CompileError::Syntax("def 的体要缩进".to_owned()));
                        }
                        *cursor += 1;
                    }
                    let body = parse_statements(lexed, cursor, depth + 1, true, inline)?;
                // **行内空体必须报错** ✓（同 `class` 那条 ✓）：`def f():` 冒号后直接文件尾 ⇒
                // 参照 `IndentationError` ✓ ⇒ 普通 `CompileError` ✓。
                if inline && body.is_empty() {
                    return Err(CompileError::Syntax("def 的体要缩进".to_owned()));
                }
                // `def` 的整段：从 `def` 关键字到**体最后一行的行尾**（实测 `(1, 2, 0, 12)`）
                let body_end = statements_last_end(&body).unwrap_or(def_span);
                let span = def_span.to(body_end);
                // **只在真停在收尾词上才前移** ✓（行内 `Newline`／缩进 `Dedent`；`End` 不吃 ✓）。
                if matches!(tokens.get(*cursor), Some(Lexeme::Newline) | Some(Lexeme::Dedent)) {
                    *cursor += 1;
                }
                // **有装饰器时，`first_line` 取第一条装饰器那一行**（实测：内层 code object 的
                // 行表首项是 `(1,1)`＝`@dec` 那行 ✓，而不是 `def` 那行的 `(2,2)` ✗）
                let first_line = decorators_first_line.unwrap_or(first_line);
                // **`async def` 的体补一条 `yield`** ✓（第 185 轮）：这样它就是**生成器** ✓
                // （`statements_have_yield` 认它 ✓）⇒ 调用返回**未启动的生成器对象** ✓、`close()` 可用 ✓。
                let mut body = body;
                if async_def {
                    body.push(Statement::Yield(None, span));
                }
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
                    type_params,
                });
        }
        _ => unreachable!("parse_statement_def 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::For)` 分支原样搬入。
fn parse_statement_for_loop(lexed: &Lexed, cursor: &mut usize, depth: usize, in_function: bool, async_for: bool, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::For) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                // **第一项可以是名字，也可以是括号／方括号元组** ✓（第 289 轮：
                // `for (a, b) in …`／`for a, (b, c) in …` ✓）；不是这两种就照旧**如实报错** ✓。
                let (target, target_span) = match tokens.get(*cursor) {
                    // **属性／下标目标** ✓（task-25 ✓）：`for d[k] in …`（参照的 `star_target` ✓）
                    Some(Lexeme::Name(_))
                        if matches!(
                            tokens.get(*cursor + 1),
                            Some(Lexeme::Dot) | Some(Lexeme::LeftBracket)
                        ) =>
                    {
                        (String::new(), lexed.spans[*cursor])
                    }
                    Some(Lexeme::Name(name)) => (name.clone(), lexed.spans[*cursor]),
                    Some(Lexeme::LeftParen) | Some(Lexeme::LeftBracket) => {
                        // 括号开头：先记下空名字（下面按元组那条路走 ✓，`target` 不会被用 ✓）
                        (String::new(), lexed.spans[*cursor])
                    }
                    // **首项带星** ✓（task-25 ✓）：`for *a, b in …` —— 先不前进，交给下面的
                    // `parse_for_target_item`（它认识 `*` ✓）；单独一个 `*a` 会在后面报错 ✓。
                    Some(Lexeme::Star) => (String::new(), lexed.spans[*cursor]),
                    other => {
                        let span = lexed.spans[*cursor];
                        return Err(CompileError::Syntax(format!(
                            "`for` 后面要一个名字，实际 {other:?}（第 {} 行）",
                            span.line_start
                        )));
                    }
                };
                let first_target_span = target_span;
                let first_is_name_chain = matches!(
                    tokens.get(*cursor),
                    Some(Lexeme::Name(_))
                        if matches!(
                            tokens.get(*cursor + 1),
                            Some(Lexeme::Dot) | Some(Lexeme::LeftBracket)
                        )
                );
                if !first_is_name_chain
                    && !matches!(
                        tokens.get(*cursor),
                        Some(Lexeme::LeftParen) | Some(Lexeme::LeftBracket) | Some(Lexeme::Star)
                    )
                {
                    *cursor += 1;
                }
                // **目标表**（第 289 轮重写）：每一项可以是**名字**或**括号／方括号元组**（可再嵌 ✓）——
                // 照参照 `dis` 实测：`for a, (b, c) in x:` ⇒ `UNPACK_SEQUENCE 2`（整段目标）
                // ＋ `STORE a` ＋ `UNPACK_SEQUENCE 2`（`(b, c)` 那一段）＋ `STORE b` ＋ `STORE c` ✓。
                // 先前只认**名字** ✗ ⇒ `Lib/test/support/__init__.py:1887` 的
                // `for report_type, (old_mode, old_file) in …` 当场报"元组目标后面要名字" ✗。
                let mut tuple_targets: Vec<ForTarget> = Vec::new();
                let mut last_target_span = first_target_span;
                let first_needs_item = tokens.get(*cursor) == Some(&Lexeme::Star)
                    || matches!(
                        tokens.get(*cursor),
                        Some(Lexeme::Name(_))
                            if matches!(
                                tokens.get(*cursor + 1),
                                Some(Lexeme::Dot) | Some(Lexeme::LeftBracket)
                            )
                    );
                let first_item: ForTarget = if matches!(
                    tokens.get(*cursor),
                    Some(Lexeme::LeftParen) | Some(Lexeme::LeftBracket)
                ) {
                    let (item, next) = parse_for_target_group(lexed, *cursor)?;
                    last_target_span = item_span(&item);
                    *cursor = next;
                    item
                } else if first_needs_item {
                    // 首项带星／属性／下标：统一走 `parse_for_target_item`（那几臂 ✓）
                    let (item, next) = parse_for_target_item(lexed, *cursor)?;
                    last_target_span = item_span(&item);
                    *cursor = next;
                    item
                } else {
                    ForTarget::Name(target.clone(), first_target_span)
                };
                if tokens.get(*cursor) == Some(&Lexeme::Comma) {
                    tuple_targets.push(first_item);
                    while tokens.get(*cursor) == Some(&Lexeme::Comma) {
                        *cursor += 1;
                        // 允许尾逗号（`for a, in …`）
                        if tokens.get(*cursor) == Some(&Lexeme::In) {
                            break;
                        }
                        let (item, next) = parse_for_target_item(lexed, *cursor)?;
                        last_target_span = item_span(&item);
                        tuple_targets.push(item);
                        *cursor = next;
                    }
                } else {
                    match first_item {
                        // `for (a, b) in …`：**一层元组**（照参照要发 `UNPACK_SEQUENCE` ✓）
                        ForTarget::Group(items, span) => {
                            tuple_targets = items;
                            last_target_span = span;
                        }
                        // 单个**属性／下标**目标：一项目标、**不解包**（emitter 单目标路 ✓）
                        item @ (ForTarget::Attribute(..) | ForTarget::Subscript(..)) => {
                            last_target_span = item_span(&item);
                            tuple_targets = vec![item];
                        }
                        // `for *a in …`：参照给 `SyntaxError`（星号目标必须在列表／元组里 ✓）
                        ForTarget::Starred(..) => {
                            return Err(CompileError::Syntax(
                                "starred assignment target must be in a list or tuple".to_owned(),
                            ));
                        }
                        ForTarget::Name(..) => {}
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
                // **可迭代对象允许"元组显示"** ✓（第 292 轮）：`for x in a, b:` 与 `for x in (a, b):`
                // 等价 ✓（参照口径 ✓；`Lib/enum.py` 的 `for name in a, b:` 就是它 ✗ ——
                // 先前用只吃单个表达式的 `parse_expression` ✗ ⇒ 在逗号上报"`for` 后面要冒号" ✗）。
                let (iterable, next) = parse_value_expression(lexed, *cursor)?;
                *cursor = next;
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`for` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                // **单行体也接** ✓（第 723 轮）：`for x in y: stmt` ✓ —— 与 `def`／`class` 同口径 ✓
                //（`Lib/http/cookiejar.py:1760` 正是这一形态 ✓，先前因此进不了 `Lib/` ✗）。
                let inline = tokens.get(*cursor) != Some(&Lexeme::Newline);
                if !inline {
                    *cursor += 1;
                    if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                        return Err(CompileError::Syntax("`for` 的体要缩进".to_owned()));
                    }
                    *cursor += 1;
                }
                let body = parse_statements(lexed, cursor, depth + 1, in_function, inline)?;
                if inline {
                    // **行内空体必须报错** ✓（`for i in range(2):` 冒号后直接文件尾 ⇒ 参照
                    // `IndentationError` ✓）；**只吃真停在的那些** ✓（末尾无换行时停在 `End` ⇒ 不吃 ✓）。
                    if body.is_empty() {
                        return Err(CompileError::Syntax("`for` 的体要缩进".to_owned()));
                    }
                    if tokens.get(*cursor) == Some(&Lexeme::Newline) {
                        *cursor += 1;
                    }
                } else {
                    if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                        return Err(CompileError::Syntax("`for` 的体没有正常收尾".to_owned()));
                    }
                    *cursor += 1;
                }
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
                    is_async: async_for,
                    tuple_targets,
                    span: keyword_span.to(body_end),
                    target,
                    target_span,
                    iterable,
                    body,
                    else_body: for_else,
                });
        }
        _ => unreachable!("parse_statement_for_loop 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::While)` 分支原样搬入。
fn parse_statement_while_loop(lexed: &Lexed, cursor: &mut usize, depth: usize, in_function: bool, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
                // **单行体也接** ✓（第 723 轮）：`while x: stmt` ✓（`Lib/dis.py` 要它 ✓）。
                let inline = tokens.get(*cursor) != Some(&Lexeme::Newline);
                if !inline {
                    *cursor += 1;
                    if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                        return Err(CompileError::Syntax("`while` 的体要缩进".to_owned()));
                    }
                    *cursor += 1;
                }
                let body = parse_statements(lexed, cursor, depth + 1, in_function, inline)?;
                if inline {
                    // **行内空体必须报错** ✓ ＋ **只吃真停在的 `Newline`** ✓（同 `for` 那条 ✓）。
                    if body.is_empty() {
                        return Err(CompileError::Syntax("`while` 的体要缩进".to_owned()));
                    }
                    if tokens.get(*cursor) == Some(&Lexeme::Newline) {
                        *cursor += 1;
                    }
                } else {
                    if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                        return Err(CompileError::Syntax("`while` 的体没有正常收尾".to_owned()));
                    }
                    *cursor += 1;
                }
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
        _ => unreachable!("parse_statement_while_loop 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Global)` 分支原样搬入。
fn parse_statement_global(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
        _ => unreachable!("parse_statement_global 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Nonlocal)` 分支原样搬入。
fn parse_statement_nonlocal(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
        _ => unreachable!("parse_statement_nonlocal 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::If)` 分支原样搬入。
fn parse_statement_if_stmt(lexed: &Lexed, cursor: &mut usize, depth: usize, in_function: bool, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::If) => {
                let (statement, next) = parse_if_chain(lexed, *cursor, depth, in_function)?;
                *cursor = next;
                statements.push(statement);
        }
        _ => unreachable!("parse_statement_if_stmt 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Raise)` 分支原样搬入。
fn parse_statement_raise(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Raise) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let bare = matches!(
                    tokens.get(*cursor),
                    Some(Lexeme::Newline) | Some(Lexeme::Semicolon) | None
                );
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
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_raise 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Yield)` 分支原样搬入。
fn parse_statement_yield(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Yield) => {
                // `yield` / `yield 表达式`（第 124 轮）：形态见发射臂 ✓
                let statement_span = lexed.spans[*cursor];
                *cursor += 1;
                // **`yield from <表达式>`**（第 315 轮）：先认 `from` 这个词 —— 先前**根本没接** ✗
                // ⇒ `from` 被当**名字**、随后的 `[` 被当**下标** ⇒ 报"``[`` 之后要 `]`，实际
                // `Some(For)`" ✗（`Lib/traceback.py:1258` 的 `yield from [ … for l in … ]` 正卡它）。
                if matches!(tokens.get(*cursor), Some(Lexeme::Name(name)) if name == "from") {
                    *cursor += 1;
                    let (delegated, next) = parse_expression_list(lexed, *cursor)?;
                    *cursor = next;
                    let end = delegated.span();
                    statements.push(Statement::YieldFrom(Box::new(delegated), statement_span.to(end)));
                    return Ok(StatementFlow::Continue);
                }
                let value = if matches!(
                    tokens.get(*cursor),
                    Some(Lexeme::Newline)
                        | Some(Lexeme::Semicolon)
                        | Some(Lexeme::End)
                        | Some(Lexeme::Dedent)
                        | None
                        | Some(Lexeme::RightParen)
                ) {
                    None
                } else {
                    // **`yield a, b` 是元组**（第 138 轮实测：参照发 `BUILD_TUPLE 2`，位点取
                    // `a, b` 那段 ✓）⇒ 与 `return` 同一条路：`parse_expression_list` ✓。
                    let (value, next) = parse_expression_list(lexed, *cursor)?;
                    *cursor = next;
                    Some(value)
                };
                let end = value.as_ref().map(|item| item.span()).unwrap_or(statement_span);
                statements.push(Statement::Yield(value, statement_span.to(end)));
        }
        _ => unreachable!("parse_statement_yield 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Return)` 分支原样搬入。
fn parse_statement_return(lexed: &Lexed, cursor: &mut usize, in_function: bool, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
                    Some(Lexeme::Newline)
                        | Some(Lexeme::Semicolon)
                        | Some(Lexeme::End)
                        | Some(Lexeme::Dedent)
                        | None
                ) {
                    (Expression::Constant(Constant::None, keyword_span), *cursor)
                } else {
                    parse_expression_list(lexed, *cursor)?
                };
                *cursor = next;
                // 实测：整条 `return …` 的位置从 `return` 起到表达式末尾
                let span = keyword_span.to(value.span());
                statements.push(Statement::Return(value, span));
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_return 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "break"` 分支原样搬入。
fn parse_statement_kw_break(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Name(name)) if name == "break" => {
                let position = lexed.spans[*cursor];
                *cursor += 1;
                statements.push(Statement::Break(position));
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_kw_break 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "continue"` 分支原样搬入。
fn parse_statement_kw_continue(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Name(name)) if name == "continue" => {
                let position = lexed.spans[*cursor];
                *cursor += 1;
                statements.push(Statement::Continue(position));
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_kw_continue 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "del"` 分支原样搬入。
fn parse_statement_kw_del(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_kw_del 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "assert"` 分支原样搬入。
fn parse_statement_kw_assert(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_kw_assert 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "pass"` 分支原样搬入。
fn parse_statement_kw_pass(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Name(name)) if name == "pass" => {
                let position = lexed.spans[*cursor];
                *cursor += 1;
                statements.push(Statement::Pass(position));
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_kw_pass 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "import"` 分支原样搬入。
fn parse_statement_kw_import(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
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
                expect_statement_end(lexed, cursor)?;
                statements.push(Statement::Import {
                    items,
                    span: keyword_span.to(end),
                });
        }
        _ => unreachable!("parse_statement_kw_import 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "from"` 分支原样搬入。
fn parse_statement_kw_from(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Name(name)) if name == "from" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let mut level = 0u8;
                while tokens.get(*cursor) == Some(&Lexeme::Dot) {
                    level += 1;
                    *cursor += 1;
                }
                let mut module = String::new();
                // **初值只为编译器** ✓（第 213 轮）：放行尾逗号的那个 `break` 让 rustc 无法证明 `end` 已初始化 ✗；
                // 空列表（`import (,`）在参照里也不合法 ⇒ 这个初值走不到 ✓。
                let mut end = lexed.spans[*cursor];
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
                        // **括号里的尾逗号** ✓（第 213 轮）：`from x import (a, b,)` 在参照里**合法** ✓
                        // （`Lib/warnings.py` 就是这么写的 ✗ ⇒ 先前报"要名字、实际 RightParen" ✗）。
                        // **裸形式**（`from x import a,`）参照仍是语法错 ✗ ⇒ 只在 `parenthesized` 时放行 ✓。
                        if parenthesized && tokens.get(*cursor) == Some(&Lexeme::RightParen) {
                            break;
                        }
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
                expect_statement_end(lexed, cursor)?;
                statements.push(Statement::ImportFrom {
                    module,
                    level,
                    names,
                    star,
                    span: keyword_span.to(end),
                });
        }
        _ => unreachable!("parse_statement_kw_from 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// **链式赋值里的"目标（可为元组目标）＋ `=`"**（task-25 ✓）。
///
/// 参照里链式赋值的每个目标都是**赋值目标**，不限于名字链 ✓：
/// `chan = _, send = RecvChannel(cid), SendChannel(cid)`（官方 `test/support/channels.py:68`）
/// 的第二个目标是**元组目标** `_, send` ✓ —— 先前只按"名字链＋`=`"探测 ✗ ⇒ 整句按右值解析 ⇒
/// 在第二个 `=` 上报"语句结尾多出了 Some(Assign)" ✗。
///
/// 手法：先按表达式解析出首项（与语句级目标那条同一手法 ✓）；后面若是逗号 ⇒ 用
/// `parse_tuple_target_tail` 收成**目标表**（包成 `TupleLiteral`，星号项包 `Starred` ✓）。
/// 成功返回（目标表达式，`=` **之后**的位置）；解析失败／后面不是 `=` ⇒ `None`（调用方回退成右值 ✓）。
fn try_parse_chained_target(
    lexed: &Lexed,
    cursor: usize,
) -> Option<(Expression, usize)> {
    let tokens = &lexed.lexemes;
    let (first, after_first) = parse_expression(lexed, cursor).ok()?;
    let mut at = after_first;
    let first_span = first.span();
    let target = if tokens.get(at) == Some(&Lexeme::Comma) {
        let mut items: Vec<(Expression, bool)> = vec![(first, false)];
        let mut last_span = first_span;
        parse_tuple_target_tail(lexed, &mut at, &mut items, &mut last_span).ok()?;
        let mut expressions: Vec<Expression> = Vec::with_capacity(items.len());
        for (expression, starred) in items {
            if starred {
                let span = expression.span();
                expressions.push(Expression::Starred(Box::new(expression), span));
            } else {
                expressions.push(expression);
            }
        }
        Expression::TupleLiteral(expressions, first_span.to(last_span))
    } else {
        first
    };
    // 只认**赋值目标**形状 ✓（右值后面跟 `=` 的怪句要落回原来的报错 ✓）。
    if !matches!(
        target,
        Expression::Name(..)
            | Expression::Attribute(..)
            | Expression::Subscript(..)
            | Expression::TupleLiteral(..)
            | Expression::List(..)
    ) {
        return None;
    }
    if tokens.get(at) != Some(&Lexeme::Assign) {
        return None;
    }
    Some((target, at + 1))
}

/// **带注解的赋值／裸注解**（task-25 ✓ 抽出）：`cursor` 指着冒号，返回时停在语句尾之后。
///
/// 目标形状照参照：**名字**（`: ` 记进 `__annotations__`，本层如实当声明）／**属性／下标**
/// （注解**不求值** ✓，只发赋值那几条 ✓）。非名字目标还包括 `(pars): bool = True`
/// （括号名字，官方 `typinganndata/ann_module.py:21`）—— 括号只是分组，到这里已经是 `Name` ✓。
fn parse_annotated_target_assignment(
    lexed: &Lexed,
    cursor: &mut usize,
    chain: Expression,
    target_span: Span,
    statements: &mut Vec<Statement>,
) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    *cursor += 1;
    // `TS-47`：扩展模式下先把标注按**编译期类型标签**收（`T?` 以及嵌套在变量／类属性标注里的
    // `?`，如 `x: list[int?]`，在这一次收口）；判不出类型表达式时回退到表达式通道（`.pyawa` 仍
    // 接受 `.py` 能接受的任意注解表达式）。纯 Python 模式永远走表达式通道（`TS-5`）。
    let (annotation, after_annotation, annotation_span) =
        parse_annotation_source(lexed, *cursor)?;
    *cursor = after_annotation;
    // **注解声明**：名字目标才收 ✓（`x: int`／`x: int = 1` ✓）。
    if let Expression::Name(name, name_span) = &chain {
        statements.push(Statement::Annotation {
            name: name.clone(),
            name_span: *name_span,
            annotation,
            span: name_span.to(annotation_span),
        });
    }
    if tokens.get(*cursor) == Some(&Lexeme::Assign) {
        *cursor += 1;
        let (value, after_value) = parse_value_expression(lexed, *cursor)?;
        *cursor = after_value;
        // 目标链的跨度要先取（`STORE_ATTR`／`STORE_SUBSCR` 的位点取它 ✓，与**不带注解**那条路
        // 同一口径 ✓；参照 `self.x: int = 1` 的 `STORE_ATTR` 位点是 `self.x` 那一段 ✓）。
        let chain_span = chain.span();
        let span = target_span.to(value.span());
        match chain {
            Expression::Name(name, name_span) => {
                statements.push(Statement::Assign {
                    target: name,
                    target_span: name_span,
                    value: value.clone(),
                    span: name_span.to(value.span()),
                });
            }
            Expression::Attribute(object, name, _) => {
                statements.push(Statement::AssignAttr {
                    object: *object,
                    name,
                    value,
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
            other => {
                let span = other.span();
                return Err(CompileError::Unsupported(format!(
                    "带注解的赋值只接线了名字／属性／下标目标（第 {} 行）",
                    span.line_start
                )));
            }
        }
    } else {
        // 裸注解（`x: int`）⇒ 本层当**空操作** ✓（参照会记进 `__annotations__` ✗，如实登记 ✓）。
        // **非名字目标**参照会**求值目标的基对象再丢掉** ✓（`dis` 实测：`a.b: int` ⇒
        // `LOAD_NAME a; POP_TOP`）⇒ 这里照发一条表达式语句 ✓。
        match chain {
            Expression::Attribute(object, _, _) => {
                let span = object.span();
                statements.push(Statement::Expression(*object, span));
            }
            Expression::Subscript(container, _, _) => {
                let span = container.span();
                statements.push(Statement::Expression(*container, span));
            }
            _ => {}
        }
    }
    expect_statement_end(lexed, cursor)?;
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "with"` 分支原样搬入。
fn parse_statement_kw_with(lexed: &Lexed, cursor: &mut usize, depth: usize, in_function: bool, async_with: bool, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Name(name)) if name == "with" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let mut items = Vec::new();
                // **带括号的 `with`**（第 299 轮接）：`with (a as x, b, c as y):` —— 3.10 起合法 ✓，
                // `Lib/test/support/__init__.py:2943` 正是它 ✓（那一族 **26** 个模块 ✓）。
                // 括号只是**分组**：项还是照逗号分、`as` 还是照项挂 ✓（回填的那条 `JUMP` 不算括号里的 ✓）。
                let parenthesized = tokens.get(*cursor) == Some(&Lexeme::LeftParen);
                if parenthesized {
                    // 括号里允许**换行** ✓（实测参照就是这么写的 ✓）⇒ 吃掉括号后的换行词素 ✓。
                    *cursor += 1;
                    while tokens.get(*cursor) == Some(&Lexeme::Newline) {
                        *cursor += 1;
                    }
                }
                loop {
                    let (context, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    // **`as <目标>`**（第 405 轮）：名字 或 括号／方括号元组（可再嵌 ✓）——
                    // `with … as (write, encoding):`（`xml/etree/ElementTree.py:727` ✓）正是它 ✗：
                    // 先前只认单个名字 ⇒ 报"`as` 后面要一个目标名" ✓。目标复用 `for` 那套解析 ✓。
                    let target = if matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "as")
                    {
                        let (target, next) = parse_with_target_item(lexed, *cursor + 1)?;
                        *cursor = next;
                        Some(target)
                    } else {
                        None
                    };
                    items.push((context, target));
                    // 括号里：逗号（含尾随逗号 ✓）与换行都当分隔符跳掉 ✓
                    if parenthesized {
                        while matches!(
                            tokens.get(*cursor),
                            Some(Lexeme::Comma) | Some(Lexeme::Newline)
                        ) {
                            *cursor += 1;
                        }
                        if tokens.get(*cursor) == Some(&Lexeme::RightParen) {
                            *cursor += 1;
                            break;
                        }
                        continue;
                    }
                    if tokens.get(*cursor) == Some(&Lexeme::Comma) {
                        *cursor += 1;
                        continue;
                    }
                    break;
                }
                let (body, next) = parse_colon_suite(lexed, *cursor, depth, in_function)?;
                *cursor = next;
                let body_end = statements_last_end(&body).unwrap_or(keyword_span);
                statements.push(Statement::With {
                    is_async: async_with,
                    items,
                    body,
                    span: keyword_span.to(body_end),
                });
        }
        _ => unreachable!("parse_statement_kw_with 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(name)) if name == "try"` 分支原样搬入。
fn parse_statement_kw_try(lexed: &Lexed, cursor: &mut usize, depth: usize, in_function: bool, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Name(name)) if name == "try" => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let (body, next) = parse_colon_suite(lexed, *cursor, depth, in_function)?;
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
                    let (handler_body, next) = parse_colon_suite(lexed, *cursor, depth, in_function)?;
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
                    let (suite, next) = parse_colon_suite(lexed, *cursor, depth, in_function)?;
                    *cursor = next;
                    else_body = suite;
                }
                // `finally:`（`try/finally` 允许**没有** `except`）
                let mut finally_body: Vec<Statement> = Vec::new();
                if matches!(tokens.get(*cursor), Some(Lexeme::Name(word)) if word == "finally") {
                    *cursor += 1;
                    let (suite, next) = parse_colon_suite(lexed, *cursor, depth, in_function)?;
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
        _ => unreachable!("parse_statement_kw_try 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Name(target))` 分支原样搬入。
fn parse_statement_name_target(lexed: &Lexed, cursor: &mut usize, depth: usize, in_function: bool, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Name(target)) => {
                let target = target.clone();
                let target_span = lexed.spans[*cursor];
                let statement_start = *cursor;
                // **`match` 语句**（第 290 轮）：`match`／`case` 都是**软关键字** ✓
                //（`match(x)` 仍是调用 ✓）⇒ 先**试**解析"主语 ＋ `:`"，试不中就走普通那条路 ✓。
                if target == "match" {
                    if let Some((statement, next)) =
                        try_parse_match(lexed, *cursor, depth, in_function)?
                    {
                        statements.push(statement);
                        *cursor = next;
                        return Ok(StatementFlow::Continue);
                    }
                }
                // **调用开头的语句**（第 283 轮修 ✗）：`f()` 是表达式语句 ✓，但
                // `f()[k] = v`／`f().attr = v` 是**赋值** ✓ —— `multiprocessing/context.py:217` 的
                // `globals()['reduction'] = reduction` 正是它 ✗（先前一律当表达式语句 ⇒
                // 随后在 `=` 上报"语句结尾多出了 Some(Assign)"，那一族 **23** 个模块压在它上面 ✓）。
                // 手法：整段按**表达式**解析（调用／下标／属性后缀都在里面 ✓），再按"后面是不是赋值"
                // 分流 ✓ —— 是就把这个表达式当**目标链**交给下面既有那套（`=`／增强赋值／元组解包）✓。
                let mut chain;
                if matches!(tokens.get(*cursor + 1), Some(Lexeme::LeftParen)) {
                    let (expression, next) = parse_expression(lexed, *cursor)?;
                    // **带注解**（task-25 ✓）：`f().new_attr: object = object()` —— `:` 也要认
                    // （官方 `typinganndata/ann_module2.py:16`）⇒ 先把它当目标链收下，注解那一段
                    // 由下面统一的带注解赋值入口接 ✓。
                    let assignment_follows = matches!(
                        tokens.get(next),
                        Some(Lexeme::Assign)
                            | Some(Lexeme::AugAssign(_))
                            | Some(Lexeme::Colon)
                    );
                    if !assignment_follows
                        || !matches!(
                            expression,
                            Expression::Subscript(..) | Expression::Attribute(..)
                        )
                    {
                        // **表达式语句可以是元组显示** ✓（task-25 ✓）：`fn(a=0),` 末尾那个逗号
                        // 让它成了单元素元组 ✓（官方 `test_clinic`／`test_cprofile` 一族用它
                        // 触发"表达式语句求值" ✓）—— 先前报"语句结尾多出了 Some(Comma)" ✗。
                        let (expression, next) = parse_expression_list(lexed, statement_start)?;
                        *cursor = next;
                        let span = expression.span();
                        statements.push(Statement::Expression(expression, span));
                        expect_statement_end(lexed, cursor)?;
                        return Ok(StatementFlow::Continue);
                    }
                    *cursor = next;
                    chain = expression;
                } else {
                    *cursor += 1;
                    // **目标链**（第 222 轮统一）：`名字` 后接**任意串**的 `[键]` / `.名字`
                    // （实测 `a[0].b = v`：值先压、再求目标链 `a[0]`、最后按**最后一跳**选
                    //  `STORE_ATTR`／`STORE_SUBSCR`；增强赋值同理，中间多一次"取旧值"）
                    chain = Expression::Name(target.clone(), target_span);
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
                                let (key, next) = parse_subscript_key(lexed, *cursor + 1)?;
                                if tokens.get(next) != Some(&Lexeme::RightBracket) {
                                    // **带上位点**（第 315 轮）：先前的消息定不了是哪一行
                                    // （`traceback` 那族 15 个模块就卡在这）。
                                    let site = lexed.spans.get(next).copied().unwrap_or(begin);
                                    return Err(CompileError::Syntax(format!(
                                        "`[` 之后要 `]`，实际 {:?}（第 {} 行，列 {}-{}）",
                                        tokens.get(next),
                                        site.line_start,
                                        site.col_start,
                                        site.col_end
                                    )));
                                }
                                let span = begin.to(lexed.spans[next]);
                                chain = Expression::Subscript(Box::new(chain), Box::new(key), span);
                                *cursor = next + 1;
                            }
                            _ => break,
                        }
                    }
                }
                // **`BC-63` 空合并赋值**（`a ??= b`；扩展模式；**语句**）：只接名字目标
                // （属性／下标目标的"目标只求值一次"要多一层临时量 ⇒ 如实报未接线）。
                // 纯 Python 模式的四种写法在词法期就报 `Syntax`（`BC-15`），走不到这里。
                if lexed.mode == Mode::Extension
                    && tokens.get(*cursor) == Some(&Lexeme::Question)
                    && tokens.get(*cursor + 1) == Some(&Lexeme::Question)
                    && tokens.get(*cursor + 2) == Some(&Lexeme::Assign)
                {
                    let Expression::Name(name, name_span) = &chain else {
                        let line = lexed
                            .spans
                            .get(*cursor)
                            .map(|span| span.line_start)
                            .unwrap_or(0);
                        return Err(CompileError::Unsupported(format!(
                            "`??=` 只接线名字目标（第 {line} 行；属性／下标目标尚未接线）"
                        )));
                    };
                    let name = name.clone();
                    let name_span = *name_span;
                    *cursor += 3;
                    let (value, after) = parse_expression(lexed, *cursor)?;
                    *cursor = after;
                    let span = name_span.to(value.span());
                    statements.push(Statement::NullCoalesceAssign {
                        name,
                        name_span,
                        value,
                        span,
                    });
                    expect_statement_end(lexed, cursor)?;
                    return Ok(StatementFlow::Continue);
                }
                // **元组解包赋值**（第 107 轮；实测形态见发射臂 ✓）：`a, b = x`／`a[0], b = x`／
                // `a, *b, c = x`。目标用**表达式**解析（与 `del` 同一口径 ✓），`*` 单独记一位 ✓。
                if tokens.get(*cursor) == Some(&Lexeme::Comma) {
                    let first_chain = chain.clone();
                    let mut targets: Vec<(Expression, bool)> = vec![(chain, false)];
                    let mut last_span = first_chain.span();
                    // 逗号目标收成 `parse_tuple_target_tail` ✓（第 722 轮抽出）：首星那条
                    // `*a, b = x` 也要走**同一套**口径 ✓（此前只有"名字打头"这一支 ✗）。
                    parse_tuple_target_tail(lexed, cursor, &mut targets, &mut last_span)?;
                    if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                        let span = lexed.spans.get(*cursor).copied();
                        return Err(CompileError::Syntax(format!(
                            "元组目标之后要 `=`，实际 {:?}（第 {} 行）",
                            tokens.get(*cursor),
                            span.map(|span| span.line_start).unwrap_or(0)
                        )));
                    }
                    *cursor += 1;
                    // **链式赋值带元组目标** ✓（task-22 第 3 轮 ✓）：`a, b = s = expr` ✓
                    // （`Lib/heapq.py:373` 的 `value, order, next = s = h[0]` ✓）—— 先前到 `=`
                    // 就交给 `parse_value_expression` ✗ ⇒ 值位置冒出 `s = h[0]` ⇒
                    // "语句结尾多出了 Some(Assign)" ✗。参照语义：**右值求一次、从右往左赋** ✓。
                    let mut chain: Vec<Expression> = Vec::new();
                    while let Some((target, after)) = try_parse_chained_target(lexed, *cursor) {
                        *cursor = after;
                        chain.push(target);
                    }
                    let (value, next) = parse_value_expression(lexed, *cursor)?;
                    *cursor = next;
                    statements.push(Statement::AssignTuple {
                        targets,
                        value: value.clone(),
                        target_span: target_span.to(last_span),
                        span: target_span.to(value.span()),
                        chain,
                    });
                    expect_statement_end(lexed, cursor)?;
                    return Ok(StatementFlow::Continue);
                }
                // **带注解的赋值／裸注解**（第 340 轮；task-25 抽成统一入口 ✓）：
                // `x: int = 1`／`x: int`／`self.x: int = 1`／`(pars): bool = True` ✓。
                if tokens.get(*cursor) == Some(&Lexeme::Colon) {
                    return parse_annotated_target_assignment(
                        lexed, cursor, chain, target_span, statements,
                    );
                }
                // **增强赋值**：三种目标各一套栈序（见 `Statement::AugAssign`）
                if let Some(Lexeme::AugAssign(operator)) = tokens.get(*cursor) {
                    let operator = *operator;
                    *cursor += 1;
                    // 右值可以是元组显示（`x += 1,` ✓）；无逗号时行为与 `parse_expression` 一致 ✓
                    let (value, next) = parse_expression_list(lexed, *cursor)?;
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
                    expect_statement_end(lexed, cursor)?;
                    return Ok(StatementFlow::Continue);
                }
                // 目标链之后接 `(` ⇒ **方法调用的表达式语句**（`obj.method(…)`，实测常见）
                // ⇒ 整句按表达式重解析（此前只接线了 `名字(…)`，这种会误报"未接线"）。
                // **但也可能是赋值目标**（第 405 轮，`_pyrepl/keymap.py:204` 那族 ✓）：
                // `r.setdefault(first, {})[key[1:]] = value` —— 目标链里**夹着调用** ⇒ 先前按
                // 表达式语句收 ⇒ 在 `=` 上报"语句结尾多出了 Some(Assign)" ✗。整段表达式解析后
                // 再看后面是不是 `=`／增强赋值／逗号 ⇒ 是且形状是下标／属性就**当目标链**接着走 ✓
                //（`globlobals()['reduction'] = …` 那条“名字打头＋调用”的支路已有同一手法 ✓）。
                if tokens.get(*cursor) == Some(&Lexeme::LeftParen) {
                    let (expression, next) = parse_expression(lexed, statement_start)?;
                    // **带注解**（task-25 ✓）：`f().new_attr: object = object()`（官方
                    // `typinganndata/ann_module2.py:16`）—— 先前 `:` 不在候选里 ✗ ⇒ 按表达式语句收
                    // ⇒ 在 `:` 上报"语句结尾多出了 Some(Colon)" ✗。
                    let assignment_follows = matches!(
                        tokens.get(next),
                        Some(Lexeme::Assign)
                            | Some(Lexeme::AugAssign(_))
                            | Some(Lexeme::Comma)
                            | Some(Lexeme::Colon)
                    );
                    if assignment_follows
                        && matches!(
                            expression,
                            Expression::Subscript(..) | Expression::Attribute(..)
                        )
                    {
                        chain = expression;
                        *cursor = next;
                    } else {
                        // 同上：表达式语句可能是元组显示（尾逗号 ✓）
                        let (expression, next) = parse_expression_list(lexed, statement_start)?;
                        *cursor = next;
                        let span = expression.span();
                        statements.push(Statement::Expression(expression, span));
                        expect_statement_end(lexed, cursor)?;
                        return Ok(StatementFlow::Continue);
                    }
                }
                // 目标链（可能是"调用／属性"那条重新解析出来的 ✓）后接 `:` ⇒ 带注解的赋值 ✓。
                if tokens.get(*cursor) == Some(&Lexeme::Colon) {
                    return parse_annotated_target_assignment(
                        lexed, cursor, chain, target_span, statements,
                    );
                }
                if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                    // **裸名字当表达式语句**（第 118 轮）：目标链走完却没有 `=` ⇒
                    //   整句按表达式重解析 ✓（与上面"方法调用"那条同一手法 ✓）；
                    //   真解析不出来才往下报错 ✓。
                    if let Ok((expression, next)) = parse_expression_list(lexed, statement_start) {
                        if next > statement_start {
                            *cursor = next;
                            let span = expression.span();
                            statements.push(Statement::Expression(expression, span));
                            expect_statement_end(lexed, cursor)?;
                            return Ok(StatementFlow::Continue);
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
                // task-25：目标不再限于"名字链"——`try_parse_chained_target` 认识**元组目标**
                // （`chan = _, send = A(), B()` ✓）与属性／下标目标 ✓。
                let mut chained_targets: Vec<Expression> = Vec::new();
                while let Some((next_target, after)) = try_parse_chained_target(lexed, *cursor) {
                    *cursor = after;
                    chained_targets.push(next_target);
                }
                if !chained_targets.is_empty() {
                    let mut targets = vec![chain.clone()];
                    targets.extend(chained_targets);
                    let (value, after) = parse_value_expression(lexed, *cursor)?;
                    *cursor = after;
                    let span = target_span.to(value.span());
                    statements.push(Statement::AssignChained {
                        targets,
                        value,
                        span,
                    });
                    expect_statement_end(lexed, cursor)?;
                    return Ok(StatementFlow::Continue);
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
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_name_target 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::LeftParen)` 分支原样搬入。
fn parse_statement_left_paren(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::LeftParen) => {
                let start_span = lexed.spans[*cursor];
                let (first, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                match parse_paren_target(lexed, cursor, first, start_span)? {
                    // **单层分组目标**：`(a) = x` ⇒ 普通赋值 ✓（参照不退化成解包 ✓）
                    ParenTarget::Single(target) => {
                        *cursor += 1;
                        let (value, after) = parse_value_expression(lexed, *cursor)?;
                        *cursor = after;
                        let chain_span = target.span();
                        let span = start_span.to(value.span());
                        match target {
                            Expression::Name(name, name_span) => {
                                statements.push(Statement::Assign {
                                    target: name,
                                    target_span: name_span,
                                    value: value.clone(),
                                    span: name_span.to(value.span()),
                                });
                            }
                            Expression::Attribute(object, name, _) => {
                                statements.push(Statement::AssignAttr {
                                    object: *object,
                                    name,
                                    value,
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
                            other => {
                                let span = other.span();
                                return Err(CompileError::Unsupported(format!(
                                    "括号目标只接线了名字／属性／下标与元组（第 {} 行）",
                                    span.line_start
                                )));
                            }
                        }
                        expect_statement_end(lexed, cursor)?;
                    }
                    ParenTarget::Tuple(targets, target_span) => {
                        *cursor += 1;
                        let (value, after) = parse_value_expression(lexed, *cursor)?;
                        *cursor = after;
                        statements.push(Statement::AssignTuple {
                            targets,
                            value: value.clone(),
                            target_span,
                            span: target_span.to(value.span()),
                            chain: Vec::new(),
                        });
                        expect_statement_end(lexed, cursor)?;
                    }
                    ParenTarget::NotTarget(expression) => {
                        // **带注解的目标** ✓（task-25 ✓）：`(pars): bool = True`（官方
                        // `typinganndata/ann_module.py:21`）—— 括号只是分组，目标形状与
                        // `pars: bool = True` 同 ✓；先前这里按表达式语句收 ⇒ 在 `:` 上报
                        // "语句结尾多出了 Some(Colon)" ✗。
                        if tokens.get(*cursor) == Some(&Lexeme::Colon)
                            && matches!(
                                expression,
                                Expression::Name(..)
                                    | Expression::Attribute(..)
                                    | Expression::Subscript(..)
                            )
                        {
                            return parse_annotated_target_assignment(
                                lexed,
                                cursor,
                                expression,
                                start_span,
                                statements,
                            );
                        }
                        // 普通括号表达式语句 ✓
                        let span = expression.span();
                        statements.push(Statement::Expression(expression, span));
                        expect_statement_end(lexed, cursor)?;
                    }
                }
        }
        _ => unreachable!("parse_statement_left_paren 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::LeftBracket)` 分支原样搬入。
fn parse_statement_left_bracket(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::LeftBracket) => {
                let start_span = lexed.spans[*cursor];
                let statement_start = *cursor;
                let (first, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                match parse_paren_target(lexed, cursor, first, start_span)? {
                    ParenTarget::Single(target) => {
                        // 列表显示本身一定是元组目标（`[a] = x` 也要 `UNPACK_SEQUENCE 1`），
                        // 走不到这里；沿用同一条收法以防将来形状变化 ✓。
                        let span = target.span();
                        statements.push(Statement::Expression(target, span));
                        expect_statement_end(lexed, cursor)?;
                    }
                    ParenTarget::Tuple(targets, target_span) => {
                        *cursor += 1;
                        let (value, after) = parse_value_expression(lexed, *cursor)?;
                        *cursor = after;
                        statements.push(Statement::AssignTuple {
                            targets,
                            value: value.clone(),
                            target_span,
                            span: target_span.to(value.span()),
                            chain: Vec::new(),
                        });
                        expect_statement_end(lexed, cursor)?;
                    }
                    ParenTarget::NotTarget(expression) => {
                        // **带注解的目标** ✓（task-25 ✓）：`[list][0]: type`（官方 `test_grammar.py`
                        // 的注解目标用例 ✓）—— 下标目标带注解，与 `a[0]: int = 1` 同形状 ✓。
                        if tokens.get(*cursor) == Some(&Lexeme::Colon)
                            && matches!(
                                expression,
                                Expression::Attribute(..) | Expression::Subscript(..)
                            )
                        {
                            return parse_annotated_target_assignment(
                                lexed,
                                cursor,
                                expression,
                                start_span,
                                statements,
                            );
                        }
                        // 普通列表显示表达式语句 ✓（也可能是元组显示，尾逗号 ✓）
                        let (expression, next) = parse_expression_list(lexed, statement_start)?;
                        *cursor = next;
                        let span = expression.span();
                        statements.push(Statement::Expression(expression, span));
                        expect_statement_end(lexed, cursor)?;
                    }
                }
        }
        _ => unreachable!("parse_statement_left_bracket 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

/// 从 `parse_statements` 的 `Some(Lexeme::Star)` 分支原样搬入。
fn parse_statement_star(lexed: &Lexed, cursor: &mut usize, statements: &mut Vec<Statement>) -> Result<StatementFlow, CompileError> {
    let tokens = &lexed.lexemes;
    match tokens.get(*cursor) {
        Some(Lexeme::Star) => {
                let star_span = lexed.spans[*cursor];
                *cursor += 1;
                let (first, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                let first_span = star_span.to(first.span());
                let mut targets: Vec<(Expression, bool)> = vec![(first, true)];
                let mut last_span = first_span;
                parse_tuple_target_tail(lexed, cursor, &mut targets, &mut last_span)?;
                if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                    let span = lexed.spans.get(*cursor).copied();
                    return Err(CompileError::Syntax(format!(
                        "元组目标之后要 `=`，实际 {:?}（第 {} 行）",
                        tokens.get(*cursor),
                        span.map(|span| span.line_start).unwrap_or(0)
                    )));
                }
                *cursor += 1;
                let (value, next) = parse_value_expression(lexed, *cursor)?;
                *cursor = next;
                statements.push(Statement::AssignTuple {
                    targets,
                    value: value.clone(),
                    target_span: star_span.to(last_span),
                    span: star_span.to(value.span()),
                    chain: Vec::new(),
                });
                expect_statement_end(lexed, cursor)?;
        }
        _ => unreachable!("parse_statement_star 只接收对应分支"),
    }
    Ok(StatementFlow::Continue)
}

