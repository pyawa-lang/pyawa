//! `parser` 的 `targets` 域：赋值／`for`／`with`／推导式的**目标**解析。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

/// **逗号目标尾巴**（第 722 轮抽出）：从**已经吃到第一个目标**的位置起，收 `, <目标>` 序列 ✓。
///
/// 覆盖三种既有口径 ✓：`*目标`（星号只允许一个 ✓，在发射期核 ✓）、**尾随逗号**
/// （`x, = [7]` ⇒ `last_span` 取逗号自身 ✓，第 188 轮实测的跨度口径 ✓）、以及普通元素 ✓。
/// 返回时 `cursor` **停在 `=` 上** ✓（调用方自己核 ✓）。
pub(crate) fn parse_tuple_target_tail(
    lexed: &Lexed,
    cursor: &mut usize,
    targets: &mut Vec<(Expression, bool)>,
    last_span: &mut Span,
) -> Result<(), CompileError> {
    let tokens = &lexed.lexemes;
    while tokens.get(*cursor) == Some(&Lexeme::Comma) {
        *cursor += 1;
        // **尾随逗号** ✓（第 188 轮真 bug 修复 ✗）：`x, = [7]` ✓ 与 `isabs, = {…}` ✓
        // 是**单元素元组目标** ✓ ⇒ 吃了逗号后若**紧跟 `=`** ⇒ 就此收尾 ✓
        //（先前无条件再解析一个元素 ✗ ⇒ 在 `=` 上炸出"表达式里出现 Some(Assign)"✗）。
        if tokens.get(*cursor) == Some(&Lexeme::Assign) {
            // **位置也要把逗号算进去** ✓（第 188 轮夹具实测 ✓）：参照给 `x, = [7]` 里
            // 元组目标的跨度是**列 0–2**（含尾随逗号 ✓），先前只到 `x`（0–1 ✗）⇒
            // `co_positions()` 差一处 ✓ ⇒ 这里把"最后一段"取成**逗号自身**的跨度 ✓。
            *last_span = lexed.spans[*cursor - 1];
            break;
        }
        // `*目标`（星号只允许一个 ✓，在发射期核）
        let starred = tokens.get(*cursor) == Some(&Lexeme::Star);
        if starred {
            *cursor += 1;
        }
        let (item, next) = parse_expression(lexed, *cursor)?;
        *cursor = next;
        *last_span = item.span();
        targets.push((item, starred));
    }
    Ok(())
}

/// 把**已经解析出的首个目标表达式**摊成一层目标表（第 405 轮，嵌套目标）：括号／方括号
/// 元组显示摊成它自己的项（项里的 `*目标` 拆成星标记 ✓），单个名字／属性／下标就是一项。
///
/// 这样 `(a, b) = x`／`(a, b), (c, d) = x` 与 `a, (b, c) = x` 走**同一条**尾巴 ✓
/// （后者的嵌套组在尾巴里由 `parse_expression` 自然给出元组显示 ✓）。
pub(crate) fn tuple_targets_from_expression(
    expression: Expression,
) -> Result<Vec<(Expression, bool)>, CompileError> {
    match expression {
        Expression::TupleLiteral(items, _) | Expression::List(items, _) => Ok(items
            .into_iter()
            .map(|item| match item {
                Expression::Starred(inner, _) => (*inner, true),
                other => (other, false),
            })
            .collect()),
        name @ (Expression::Name(..)
        | Expression::Attribute(..)
        | Expression::Subscript(..)) => Ok(vec![(name, false)]),
        other => {
            let span = other.span();
            Err(CompileError::Unsupported(format!(
                "括号目标只接线了元组与单个名字／属性／下标（第 {} 行）",
                span.line_start
            )))
        }
    }
}

/// **`(`／`[` 起头的赋值目标**（第 405 轮）：首段已按表达式解析成 `first`。
///
/// 三种结果：
/// - `ParenTarget::Single(目标)`：`(a) = x`／`(a.b) = x`（括号只是**分组** ⇒ 走普通赋值 ✓，**不**解包）
/// - `ParenTarget::Tuple(目标表, 目标跨度)`：`(a, b) = x`／`[a, b] = x`／`(a, b), (c, d) = x`
///   （首层摊开，尾巴的项由 `parse_tuple_target_tail` ＋ `parse_expression` 自然收下 ✓，
///   嵌套组照旧是元组显示 ✓）
/// - `ParenTarget::NotTarget(表达式)`：不是赋值（调用方按表达式语句收 ✓）
pub(crate) enum ParenTarget {
    Single(Expression),
    Tuple(Vec<(Expression, bool)>, Span),
    /// 不是赋值 ⇒ 调用方按表达式语句收 ✓（把原表达式**还回去** ✓）
    NotTarget(Expression),
}

pub(crate) fn parse_paren_target(
    lexed: &Lexed,
    cursor: &mut usize,
    first: Expression,
    start_span: Span,
) -> Result<ParenTarget, CompileError> {
    let is_group = matches!(
        first,
        Expression::TupleLiteral(..) | Expression::List(..)
    );
    let next = lexed.lexemes.get(*cursor);
    if !is_group {
        if next == Some(&Lexeme::Assign) {
            return Ok(ParenTarget::Single(first));
        }
        if next != Some(&Lexeme::Comma) {
            return Ok(ParenTarget::NotTarget(first));
        }
    } else if next != Some(&Lexeme::Assign) && next != Some(&Lexeme::Comma) {
        return Ok(ParenTarget::NotTarget(first));
    }
    // **后面还有逗号** ⇒ 首段是**顶层目标表里的一项**（`(a, b), (c, d) = x` 的两项都是组 ✓）：
    // 不能把它的内容摊到顶层 ✗（否则顶层 `UNPACK_SEQUENCE` 的个数多算 ✓）。
    // **紧跟 `=`** ⇒ 首段那个括号组**就是整个目标表** ⇒ 摊成它自己的项 ✓（`(a, b) = x` 一条解包 ✓）。
    let first_span = first.span();
    let mut targets = if next == Some(&Lexeme::Comma) {
        vec![(first, false)]
    } else {
        tuple_targets_from_expression(first)?
    };
    // 最后一段先取**首段自身的跨度**（`(a,\n b,\n) = x` 的括号组跨多行 ⇒ `UNPACK_SEQUENCE`
    // 的位点要盖到**闭括号**为止 ✓；先前取语句起点的单字符跨度 ✗，被逐字节夹具当场抓住 ✓）。
    let mut last_span = first_span;
    parse_tuple_target_tail(lexed, cursor, &mut targets, &mut last_span)?;
    if lexed.lexemes.get(*cursor) != Some(&Lexeme::Assign) {
        let span = lexed.spans.get(*cursor).copied();
        return Err(CompileError::Syntax(format!(
            "元组目标之后要 `=`，实际 {:?}（第 {} 行）",
            lexed.lexemes.get(*cursor),
            span.map(|span| span.line_start).unwrap_or(0)
        )));
    }
    Ok(ParenTarget::Tuple(targets, start_span.to(last_span)))
}

/// **`with … as <目标>` 的名字／括号元组目标**（第 405 轮，复用 `for` 目标那套解析 ✓）。
pub(crate) fn parse_with_target_item(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(ForTarget, usize), CompileError> {
    match lexed.lexemes.get(cursor) {
        // **属性／下标目标** ✓（task-25 ✓）：参照的 `as` 后面是任意 `star_target`
        // ⇒ `with ct as self.foo:`（官方 `test_with.py:231`）／`with a as b[0]:` 都合法 ✓。
        // **属性／下标目标（可含调用）** ✓（task-25 ✓）：参照的 `as` 后面是任意 `star_target`
        // ⇒ `with … as list(targets.values())[0][1]:`（官方 `test_with.py:672`）／`self.foo`／
        // `targets[1][0]` 都合法 ✓ —— 先前只认名字／括号组 ⇒ 在 `(` 上报"这里要冒号" ✗。
        Some(Lexeme::Name(_))
            if matches!(
                lexed.lexemes.get(cursor + 1),
                Some(Lexeme::Dot) | Some(Lexeme::LeftBracket) | Some(Lexeme::LeftParen)
            ) =>
        {
            let (expression, next) = parse_expression(lexed, cursor)?;
            Ok((for_target_from_expression(expression)?, next))
        }
        Some(Lexeme::Name(name)) => Ok((ForTarget::Name(name.clone(), lexed.spans[cursor]), cursor + 1)),
        Some(Lexeme::LeftParen) | Some(Lexeme::LeftBracket) => parse_for_target_group(lexed, cursor),
        other => {
            let span = lexed.spans.get(cursor).copied();
            Err(CompileError::Unsupported(format!(
                "`with … as` 只接线了名字／属性／下标与括号元组目标，实际 {other:?}（第 {} 行）",
                span.map(|span| span.line_start).unwrap_or(0)
            )))
        }
    }
}

/// 解析推导式的**目标**：名字、**括号／方括号组**（可嵌 ✓），以及**裸目标表**（`a, b`／`a, (b, c)`）。
///
/// 第 281 轮补过 `for (f, i) in …` ✓；第 405 轮把它改成**递归组** ✓ ⇒ 模式目标
/// `for [off] in …`（`_pyrepl/terminfo.py:405` ✓）与嵌套组都收 ✓。
/// 尾随逗号照旧算"组"（`(a,)`／`a,` 要拆包 ✓，而 `(a)` 是名字 ✓ —— 实测出来的分界 ✓）。
pub(crate) fn parse_comprehension_target(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(ComprehensionTarget, usize), CompileError> {
    let (first, mut cursor) = parse_comprehension_target_item(lexed, cursor)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::Comma) {
        return Ok((first, cursor));
    }
    // **裸目标表**：`a, b`／`(a, b), c` —— 首项之后按逗号收 ✓（尾随逗号 ⇒ 仍是一层组 ✓）
    let start = comprehension_target_span(&first);
    let mut items = vec![first];
    let mut last = start;
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Comma) {
        cursor += 1;
        if matches!(lexed.lexemes.get(cursor), Some(Lexeme::In) | None) {
            break;
        }
        let (item, next) = parse_comprehension_target_item(lexed, cursor)?;
        last = comprehension_target_span(&item);
        items.push(item);
        cursor = next;
    }
    Ok((ComprehensionTarget::Group(items, start.to(last)), cursor))
}

/// 推导式目标的一项：名字，或括号／方括号组（递归 ✓）。
pub(crate) fn parse_comprehension_target_item(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(ComprehensionTarget, usize), CompileError> {
    match lexed.lexemes.get(cursor) {
        Some(Lexeme::Name(name)) => Ok((
            ComprehensionTarget::Name(name.clone(), lexed.spans[cursor]),
            cursor + 1,
        )),
        Some(Lexeme::LeftParen) | Some(Lexeme::LeftBracket) => {
            parse_comprehension_target_group(lexed, cursor)
        }
        other => {
            let span = lexed.spans.get(cursor).copied();
            Err(CompileError::Syntax(format!(
                "推导式的 `for` 后面要一个目标（名字或括号组），实际 {other:?}（第 {} 行）",
                span.map(|span| span.line_start).unwrap_or(0)
            )))
        }
    }
}

/// 推导式的**括号／方括号组目标**（`(a, b)`／`[off]`，可再嵌 ✓）。
///
/// 单元素且**没有逗号**的括号只是**分组** ✓（`for (a) in …` 是名字、`for ((a, b)) in …`
/// 退化到内层组 ✓，与表达式那套口径一致 ✓）。
pub(crate) fn parse_comprehension_target_group(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(ComprehensionTarget, usize), CompileError> {
    let open_span = lexed.spans[cursor];
    let closing = if lexed.lexemes.get(cursor) == Some(&Lexeme::LeftParen) {
        Lexeme::RightParen
    } else {
        Lexeme::RightBracket
    };
    let mut at = cursor + 1;
    let mut items: Vec<ComprehensionTarget> = Vec::new();
    let mut saw_comma = false;
    while lexed.lexemes.get(at) != Some(&closing) {
        let (item, next) = parse_comprehension_target_item(lexed, at)?;
        items.push(item);
        at = next;
        if lexed.lexemes.get(at) == Some(&Lexeme::Comma) {
            saw_comma = true;
            at += 1;
            continue;
        }
        break;
    }
    if lexed.lexemes.get(at) != Some(&closing) {
        let span = lexed.spans.get(at).copied();
        return Err(CompileError::Syntax(format!(
            "推导式的括号目标没有收尾，实际 {:?}（第 {} 行）",
            lexed.lexemes.get(at),
            span.map(|span| span.line_start).unwrap_or(0)
        )));
    }
    let span = open_span.to(lexed.spans[at]);
    // **圆括号**的单元素无逗号只是分组 ✓（`(a)` ⇒ 名字；`((a, b))` ⇒ 内层组 ✓）。
    // **方括号不是分组** ✗：`[off]` 是模式／解包目标 ⇒ 要一层 `UNPACK_SEQUENCE 1` ✓
    //（`_pyrepl/terminfo.py:405` 的 `for [off] in …` 正是它 ✓ —— 先前在这里收成名字 ⇒ 少解一层 ✓）。
    if closing == Lexeme::RightParen && !saw_comma && items.len() == 1 {
        return Ok((items.pop().expect("刚判过只有一个"), at + 1));
    }
    Ok((ComprehensionTarget::Group(items, span), at + 1))
}

/// 一项推导式目标的跨度（`UNPACK_SEQUENCE` 那些位点用）。
pub(crate) fn comprehension_target_span(target: &ComprehensionTarget) -> Span {
    match target {
        ComprehensionTarget::Name(_, span) | ComprehensionTarget::Group(_, span) => *span,
    }
}

/// 把"属性／下标"表达式转成 `ForTarget`（`for`／`with … as` 的目标那一族 ✓，task-25 ✓）。
fn for_target_from_expression(expression: Expression) -> Result<ForTarget, CompileError> {
    match expression {
        Expression::Attribute(object, name, span) => {
            Ok(ForTarget::Attribute(object, name, span))
        }
        Expression::Subscript(object, key, span) => {
            Ok(ForTarget::Subscript(object, key, span))
        }
        other => {
            let span = other.span();
            Err(CompileError::Syntax(format!(
                "`for`／`with … as` 的目标只接线了名字／属性／下标／括号组（第 {} 行）",
                span.line_start
            )))
        }
    }
}

/// **`for` 目标的一项**（第 289 轮）：名字、属性／下标，或**括号／方括号元组**（可再嵌 ✓）；
/// `*` 项包成 `Starred` ✓（task-25 ✓）。
pub(crate) fn parse_for_target_item(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(ForTarget, usize), CompileError> {
    match lexed.lexemes.get(cursor) {
        // **属性／下标目标** ✓（task-25 ✓）：参照语法里 `for`／`with … as` 的目标是任意
        // `star_target` ⇒ `for d[k] in …`／`with C() as (targets[1][0], x)`（官方
        // `test_with.py:689` 一族）都合法 ✓ —— 先前只认名字 ⇒ 括号目标里报"没有收尾" ✗。
        Some(Lexeme::Name(_))
            if matches!(
                lexed.lexemes.get(cursor + 1),
                Some(Lexeme::Dot) | Some(Lexeme::LeftBracket) | Some(Lexeme::LeftParen)
            ) =>
        {
            let (expression, next) = parse_expression(lexed, cursor)?;
            Ok((for_target_from_expression(expression)?, next))
        }
        Some(Lexeme::Name(name)) => Ok((ForTarget::Name(name.clone(), lexed.spans[cursor]), cursor + 1)),
        Some(Lexeme::LeftParen) | Some(Lexeme::LeftBracket) => parse_for_target_group(lexed, cursor),
        // **星号项** ✓（task-25 ✓）：`for full_name, *_ in error_list:`（官方 `libregrtest/result.py:198`）
        // —— 先前这里报"`for` 的目标要名字或括号元组，实际 Some(Star)" ✗。
        Some(Lexeme::Star) => {
            let star_span = lexed.spans[cursor];
            let (inner, next) = parse_for_target_item(lexed, cursor + 1)?;
            let span = star_span.to(item_span(&inner));
            Ok((ForTarget::Starred(Box::new(inner), span), next))
        }
        other => Err(CompileError::Syntax(format!(
            "`for` 的目标要名字或括号元组，实际 {other:?}（第 {} 行）",
            lexed.spans.get(cursor).map(|span| span.line_start).unwrap_or(0)
        ))),
    }
}

/// **括号／方括号元组目标**（`(a, b)`／`[a, b]`，可再嵌 ✓）—— 位点取**括号那一段** ✓。
pub(crate) fn parse_for_target_group(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(ForTarget, usize), CompileError> {
    let open_span = lexed.spans[cursor];
    let closing = if lexed.lexemes.get(cursor) == Some(&Lexeme::LeftParen) {
        Lexeme::RightParen
    } else {
        Lexeme::RightBracket
    };
    let mut items: Vec<ForTarget> = Vec::new();
    let mut at = cursor + 1;
    loop {
        if lexed.lexemes.get(at) == Some(&closing) {
            break;
        }
        let (item, next) = parse_for_target_item(lexed, at)?;
        items.push(item);
        at = next;
        match lexed.lexemes.get(at) {
            Some(Lexeme::Comma) => at += 1,
            _ => break,
        }
    }
    if lexed.lexemes.get(at) != Some(&closing) {
        return Err(CompileError::Syntax(
            "`for` 的括号目标没有收尾".to_owned(),
        ));
    }
    let span = open_span.to(lexed.spans[at]);
    Ok((ForTarget::Group(items, span), at + 1))
}

/// 一项目标的位点（`ForTarget` 两种形态各取自己的 ✓）。
pub(crate) fn item_span(target: &ForTarget) -> Span {
    match target {
        ForTarget::Name(_, span)
        | ForTarget::Group(_, span)
        | ForTarget::Starred(_, span)
        | ForTarget::Attribute(_, _, span)
        | ForTarget::Subscript(_, _, span) => *span,
    }
}

