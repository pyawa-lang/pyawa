//! `parser` 的 `expressions` 域：表达式与优先级链（含比较、布尔、二元层级）。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

/// 解析一条 `if`／`elif` 链（`elif` 与"`else:` 里套 `if`"**同形**，参照实测逐字节相同）。
///
/// `cursor` 指着 `if` **或** `elif`（后者是 `Name("elif")`：关键字表里没有它）。
/// **显示里的一项**：`*表达式` 包成 `Expression::Starred` ✓，否则就是普通表达式 ✓。
pub(crate) fn parse_star_or_expression(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    if lexed.lexemes.get(cursor) == Some(&Lexeme::Star) {
        let star_span = lexed.spans[cursor];
        let (value, next) = parse_expression(lexed, cursor + 1)?;
        return Ok((
            Expression::Starred(Box::new(value.clone()), star_span.to(value.span())),
            next,
        ));
    }
    // **显示里的裸海象**（第 316 轮，参照口径）：`[z := 7]`／`(a := 1, b)` 都合法 ✓ ——
    // 与位置实参同一条口径（`parse_condition` ✓）；`*` 那一支照旧 ✓。
    parse_condition(lexed, cursor)
}

/// 游标处是不是「目标链 ＋ `=`」（**链式赋值**的判断；**不跨行** ✓）。
///
/// 目标链的形态与 `del`／元组目标同一口径：`名字` ＋ 任意串 `[键]`／`.名字` ✓。
pub(crate) fn looks_like_target_then_assign(lexed: &Lexed, cursor: usize) -> bool {
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
pub(crate) fn parse_value_expression(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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
        // **尾随逗号也算进元组显示的跨度** ✓（参照实测：`t = 1,` 的 `LOAD_CONST` 位点是
        // `1,` 那一段 `(4,6)` ✓ —— 逗号在里面 ✓）
        let comma_span = lexed.spans[next];
        next += 1;
        // 末尾逗号：`t = 1,`
        if matches!(
            lexed.lexemes.get(next),
            Some(Lexeme::Newline) | Some(Lexeme::End) | Some(Lexeme::Dedent) | None
        ) {
            end = comma_span;
            break;
        }
        // **元组显示项可以是 `*表达式`** ✓（`a, b = 1, *rest` ✓，第 405 轮）
        let (item, after) = parse_star_or_expression(lexed, next)?;
        end = item.span();
        items.push(item);
        next = after;
    }
    let span = items[0].span().to(end);
    Ok((Expression::TupleLiteral(items, span), next))
}

pub(crate) fn parse_condition(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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

/// 比较层（在 `+` 之上）：本层只接线**一次**比较，链式（`a < b < c`）如实报未接线。
/// 比较运算符的识别 ＋ 连带几个词之后要吃掉的**词数**（`is not`／`not in` 是两词）。
pub(crate) fn comparison_operator(lexed: &Lexed, cursor: usize) -> Option<(CompareOperator, usize)> {
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
pub(crate) fn parse_comparison(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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
pub(crate) fn parse_not_test(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    if lexed.lexemes.get(cursor) == Some(&Lexeme::Name("not".to_owned())) {
        let start = lexed.spans.get(cursor).copied().unwrap_or(Span::new(1, 1, 0, 0));
        let (operand, next) = parse_not_test(lexed, cursor + 1)?;
        let span = start.to(operand.span());
        return Ok((Expression::Not(Box::new(operand), span), next));
    }
    parse_comparison(lexed, cursor)
}

/// **`and` 层**（Python 的 `and_test`）。
pub(crate) fn parse_and_test(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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
pub(crate) fn parse_or_test(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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

/// 表达式入口（`or` 层；`BC-63` 的 `??` 在它**之下**）。
pub(crate) fn parse_expression(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (value, cursor) = parse_or_test(lexed, cursor)?;
    // **`BC-63` 空合并 `??`**：**低于 `or`／`and`**、**右结合**（`a ?? b ?? c` ＝ `a ?? (b ?? c)`）。
    // 「右结合」由"右侧递归回 `parse_expression`"实现；词法上是同一个 token 连写两枚 `?`。
    // `??=` 是**语句**（`??` 后面紧跟 `=`）—— 这里**不**吃，留给语句层（否则右侧会从 `=` 开始解析）。
    if lexed.mode == Mode::Extension
        && lexed.lexemes.get(cursor) == Some(&Lexeme::Question)
        && lexed.lexemes.get(cursor + 1) == Some(&Lexeme::Question)
        && lexed.lexemes.get(cursor + 2) != Some(&Lexeme::Assign)
    {
        let (right, next) = parse_expression(lexed, cursor + 2)?;
        let span = value.span().to(right.span());
        return Ok((
            Expression::NullCoalesce(Box::new(value), Box::new(right), span),
            next,
        ));
    }
    // **三元表达式** `a if b else c`（右结合 ⇒ `a if b else c if d else e` 的 else 分支再递归）
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

/// **表达式列表**：逗号分隔 ⇒ 元组字面量（实测 `x = 1, 2` 与 `x = (1, 2)` 同形）。
///
/// 只给**语句层**用（赋值右值、`return`）——调用实参有自己的解析（那里的逗号是分隔符）。
pub(crate) fn parse_expression_list(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(Expression, usize), CompileError> {
    // **首项也可以是 `*表达式`** ✓（第 405 轮）：`t = *a,`／`return *a,` 参照都合法 ✓
    //（参照 `dis`：`LOAD a; BUILD_LIST 0; LIST_EXTEND 1; CALL_INTRINSIC_1 6` ✓）。
    let (first, mut cursor) = parse_star_or_expression(lexed, cursor)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::Comma) {
        return Ok((first, cursor));
    }
    let mut items = vec![first];
    let mut end = items[0].span();
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Comma) {
        // **尾随逗号也算进跨度** ✓（同 `parse_value_expression` 那条口径）
        let comma_span = lexed.spans[cursor];
        cursor += 1;
        if matches!(
            lexed.lexemes.get(cursor),
            None | Some(Lexeme::Newline)
                | Some(Lexeme::RightParen)
                | Some(Lexeme::RightBracket)
                | Some(Lexeme::RightBrace)
        ) {
            end = comma_span;
            break;
        }
        let (item, next) = parse_star_or_expression(lexed, cursor)?;
        end = item.span();
        items.push(item);
        cursor = next;
    }
    let span = items
        .first()
        .expect("至少一项")
        .span()
        .to(end);
    Ok((Expression::TupleLiteral(items, span), cursor))
}

/// 一层通用的**左结合**二元运算（`|`／`^`／`&`／`<<`／`>>`／`+`／`-`／`*`… 都走它）。
pub(crate) fn parse_binary_level(
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
pub(crate) fn parse_bitwise_or(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_bitwise_xor,
        &[(Lexeme::Pipe, BinaryOperator::BitOr)],
    )
}

/// `^`。
pub(crate) fn parse_bitwise_xor(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_bitwise_and,
        &[(Lexeme::Caret, BinaryOperator::BitXor)],
    )
}

/// `&`。
pub(crate) fn parse_bitwise_and(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    parse_binary_level(
        lexed,
        cursor,
        parse_shift,
        &[(Lexeme::Ampersand, BinaryOperator::BitAnd)],
    )
}

/// `<<`／`>>`。
pub(crate) fn parse_shift(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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
pub(crate) fn parse_sum(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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
pub(crate) fn parse_term(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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
pub(crate) fn parse_factor(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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
pub(crate) fn parse_power(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
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

/// 解析推导式的**一层或多层生成器**（游标指向第一个 `for`）：
/// `for <目标> in <可迭代> [if <条件>]*` 重复出现就依次收下，返回停在收尾括号上的游标。
pub(crate) fn parse_comprehension_generators(
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

