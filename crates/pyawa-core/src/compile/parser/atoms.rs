//! `parser` 的 `atoms` 域：原子表达式与后缀链（属性／调用／下标／切片）。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

/// 解析**下标里的一项**：普通表达式，或者切片（`a[b:c]`／`a[b:c:d]`）。
///
/// 界全是常量（含缺省）时直接给 `Constant::Slice` —— 参照实测把它放进**常量池**
/// （`x = a[1:2]` ⇒ `LOAD_CONST slice(1, 2, None)`，且入表在 `None` **之前**）。
/// 解析下标里的**项列表** ✓（第 283 轮）：
/// `a[i]`／`a[i, j]`／`a[i,]` —— 逗号多于一项就折成**元组键** ✓（参照里 `a[i, j]` 等价于 `a[(i, j)]` ✓）。
/// 先前只认单一项 ✗ ⇒ `re` 的 `_cache2[type(pattern), pattern, flags]` 报
/// "`[` 之后要 `]`，实际 Some(Comma)" ✗（`re` 那一族 **28** 个模块压在它上面 ✓）。
pub(crate) fn parse_subscript_key(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(Expression, usize), CompileError> {
    let (first, mut cursor) = parse_subscript_item(lexed, cursor)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::Comma) {
        return Ok((first, cursor));
    }
    let start = first.span();
    let mut items = vec![first];
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Comma) {
        cursor += 1;
        // **尾随逗号**（`a[i,]` ⇒ 一项的元组 ✓）
        if lexed.lexemes.get(cursor) == Some(&Lexeme::RightBracket) {
            break;
        }
        let (item, next) = parse_subscript_item(lexed, cursor)?;
        items.push(item);
        cursor = next;
    }
    let end = items
        .last()
        .map(|item| item.span())
        .unwrap_or(start);
    Ok((Expression::TupleLiteral(items, start.to(end)), cursor))
}

pub(crate) fn parse_subscript_item(
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
pub(crate) fn constant_slice(
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

pub(crate) fn parse_atom(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let span = lexed
        .spans
        .get(cursor)
        .copied()
        .unwrap_or(Span::new(1, 1, 0, 0));
    let (term, cursor) = { match lexed.lexemes.get(cursor) {
        Some(Lexeme::Int(value)) => (Expression::Int(*value, span), cursor + 1),
        // **大整数字面量**（第 285 轮）：常量池那条路 ✓（`Constant::BigInt` ✓）。
        Some(Lexeme::BigInt(text)) => (Expression::BigInt(text.clone(), span), cursor + 1),
        // **浮点字面量**（第 127 轮）：值与位点都来自词素 ✓
        Some(Lexeme::Float(bits)) => (Expression::Float(*bits, span), cursor + 1),
        // **虚部字面量**（task-29 ✓）：`1j`／`1.5j`／`1e3j` —— 词素里是虚部位模式 ✓
        Some(Lexeme::Imaginary(bits)) => (Expression::Imaginary(*bits, span), cursor + 1),
        // **bytes 的隐式拼接**（第 314 轮）：与字符串同一套口径 ✓ —— 相邻 `b"…"` **合成一个常量** ✓。
        // 动因：`Lib/base64.py:437` 的
        // `_b85alphabet = (b"0123456789…" \n b"abcdef…")` 跨行相邻 ⇒ 先前**不并** ✗ ⇒ 括号那一组
        // 见到第二个 bytes 字面量 ⇒ 报"括号没有闭合，实际 Some(Bytes([…]))" ✗（那一族 **10** 个模块 ✓）。
        Some(Lexeme::Bytes(_)) => parse_atom_bytes(lexed, cursor, span)?,
        Some(Lexeme::Str(_)) => parse_atom_str(lexed, cursor, span)?,
        // **f-string**（第 238 轮）：切片成「字面段／插值段」，插值里的表达式按**相对列偏移**重新词法。
        // **相邻字面量拼接**（第 113 轮）：后随 **f-string**（`f"a{x}" f"b"` ✓）或**普通串**
        // （`f"a{x}" "b"` ✓）都要并进来 —— 实测上游 `_bootstrap.py:1426`／`site.py:210` 就卡在
        // 「相邻 **f-string ＋ f-string**」✗（此前后随 f-string 那条路只接在 `Str` 臂里 ✗）。
        Some(Lexeme::FStr { .. }) => parse_atom_fstr(lexed, cursor)?,
        // **`None` 是常量**（实测：`x = None` ⇒ 常量表 `['None']`、`LOAD_CONST 0`）；
        // `True`／`False` 要等 `Constant::Bool`（下一轮）
        // **推导式**：`[<元素> for <目标> in <可迭代> [if <条件>]*]`
        Some(Lexeme::LeftBrace) => parse_atom_leftbrace(lexed, cursor)?,
        Some(Lexeme::LeftParen) => parse_atom_leftparen(lexed, cursor)?,
        Some(Lexeme::LeftBracket) => parse_atom_leftbracket(lexed, cursor)?,
        // **`lambda`**（3.14 实测）：`lambda 形参表: 表达式`；返回一个函数对象
        Some(Lexeme::Name(name)) if name == "lambda" => parse_lambda(lexed, cursor)?,
        Some(Lexeme::Name(name)) if name == "None" => {
            (Expression::Constant(Constant::None, span), cursor + 1)
        }
        // **`...`**（第 177 轮）：**主表达式位置**的三个连续 `Dot` ✓ —— 属性链上的单个 `.` 不受影响 ✓
        // （这里只在"语句/表达式开头"这一支 ✓）。
        Some(Lexeme::Dot)
            if matches!(lexed.lexemes.get(cursor + 1), Some(Lexeme::Dot))
                && matches!(lexed.lexemes.get(cursor + 2), Some(Lexeme::Dot)) =>
        {
            // **跨度要盖住三个点** ✓（第 177 轮实证：`class C: ...` 的列是 `(0, 12)` ✓，不是起点的 10 ✗）。
            let dot_span = lexed.spans[cursor].to(lexed.spans[cursor + 2]);
            (Expression::Constant(Constant::Ellipsis, dot_span), cursor + 3)
        }
        // `True`／`False` 同样是**常量**（实测：`x = True` ⇒ 常量表 `['True', 'None']`）
        Some(Lexeme::Name(name)) if name == "True" => {
            (Expression::Constant(Constant::Bool(true), span), cursor + 1)
        }
        Some(Lexeme::Name(name)) if name == "False" => {
            (Expression::Constant(Constant::Bool(false), span), cursor + 1)
        }
        // **`yield` 当表达式** ✓（第 219 轮）：`(lambda: (yield))` ✓、`x = (yield)` ✓ 一类 ✓。
        // 无值的情形＝后面紧跟收尾记号 ✓（列表与语句版同口径 ✓，另加括起来的那些 ✓）。
        Some(Lexeme::Yield) => {
            let keyword_span = lexed.spans[cursor];
            let mut next = cursor + 1;
            // **`yield from <表达式>` 的表达式形态**（第 315 轮）。
            if matches!(lexed.lexemes.get(next), Some(Lexeme::Name(name)) if name == "from") {
                next += 1;
                let (delegated, after) = parse_expression_list(lexed, next)?;
                let end = delegated.span();
                return Ok((
                    Expression::YieldFrom(Box::new(delegated), keyword_span.to(end)),
                    after,
                ));
            }
            let value = if matches!(
                lexed.lexemes.get(next),
                None | Some(Lexeme::Newline)
                    | Some(Lexeme::Semicolon)
                    | Some(Lexeme::End)
                    | Some(Lexeme::Dedent)
                    | Some(Lexeme::RightParen)
                    | Some(Lexeme::RightBracket)
                    | Some(Lexeme::RightBrace)
                    | Some(Lexeme::Comma)
                    | Some(Lexeme::Colon)
            ) {
                None
            } else {
                let (value, after) = parse_expression_list(lexed, next)?;
                next = after;
                Some(Box::new(value))
            };
            let end = value.as_ref().map(|item| item.span()).unwrap_or(keyword_span);
            (Expression::Yield(value, keyword_span.to(end)), next)
        }
        // **`await <表达式>`** ✓（第 221 轮）：本层**没有协程** ✗ ⇒ 近似成"**就是那个表达式**" ✓
        //（与 `async def` 编成生成器是**同一族近似** ✓，口径已随它一起登记 ✓）。
        // 只吃**原子**那一半 ✓ ⇒ 后面的 `.meth(...)`／下标由**后缀层**接着处理 ✓
        //（`await self.asend(None)` ⇒ 正是这个形状 ✓）。
        Some(Lexeme::Name(name)) if name == "await" => parse_atom(lexed, cursor + 1)?,
        Some(Lexeme::Name(name)) => (Expression::Name(name.clone(), span), cursor + 1),
        other => {
            let span = lexed.spans[cursor];
            return Err(CompileError::Syntax(format!(
                "表达式里出现 {other:?}（第 {} 行，列 {}-{}）",
                span.line_start, span.col_start, span.col_end
            )));
        }
 } };
    // **统一后缀链**（第 221 轮；`BC-63` 起还吃扩展的 `?.`／`?[`）：`.`／`(`／`[`／`?` 按任意顺序串。
    parse_postfix_chain(lexed, term, cursor)
}

/// **统一后缀链**（`.`／`(`／`[`；扩展模式再加 `BC-63` 的空安全后缀 `?.`／`?[`）。
///
/// `term` 是链的**基底**（已经解析好的表达式）；返回"基底串完所有后缀"的结果与游标。
///
/// **`?.`／`?[` 的接法**：遇到 `?` 时把**当时的 `term`** 当 `SafeChain` 的接收者，`rest` 用
/// [`Expression::ChainSubject`] 作基底、**递归继续**解析剩余后缀 —— 这样 `a?.b.c` 的 `.c`
/// 落在**守卫之内**（`a` 为 `None` 时整段不执行、结果是 `None`，`BC-63`）；`a?.b?.c` 的第二个
/// 安全点再由递归里的下一层 `SafeChain` 表达 ✓。
pub(crate) fn parse_postfix_chain(
    lexed: &Lexed,
    mut term: Expression,
    mut cursor: usize,
) -> Result<(Expression, usize), CompileError> {
    loop {
        // **`BC-63` 的空安全后缀**（只在扩展模式；纯 Python 模式的 `?` 在词法期就报 `Syntax`）。
        if lexed.mode == Mode::Extension && lexed.lexemes.get(cursor) == Some(&Lexeme::Question) {
            // `??` 是二元空合并（同一 token 的另一种用法）⇒ 后缀链到此为止，交给上层。
            if lexed.lexemes.get(cursor + 1) == Some(&Lexeme::Question) {
                break;
            }
            let question_span = lexed.spans[cursor];
            match lexed.lexemes.get(cursor + 1) {
                Some(Lexeme::Dot) => {
                    let name = match lexed.lexemes.get(cursor + 2) {
                        Some(Lexeme::Name(name)) => name.clone(),
                        other => {
                            return Err(CompileError::Syntax(format!(
                                "`?.` 后面要属性名，实际 {other:?}"
                            )))
                        }
                    };
                    let subject = Expression::ChainSubject(question_span);
                    let attribute_span = question_span.to(lexed.spans[cursor + 2]);
                    let rest = Expression::Attribute(Box::new(subject), name, attribute_span);
                    let (rest, after) = parse_postfix_chain(lexed, rest, cursor + 3)?;
                    let span = term.span().to(rest.span());
                    return Ok((
                        Expression::SafeChain {
                            receiver: Box::new(term),
                            rest: Box::new(rest),
                            span,
                        },
                        after,
                    ));
                }
                Some(Lexeme::LeftBracket) => {
                    let (key, next) = parse_subscript_key(lexed, cursor + 2)?;
                    if lexed.lexemes.get(next) != Some(&Lexeme::RightBracket) {
                        let site = lexed.spans.get(next).copied().unwrap_or(question_span);
                        return Err(CompileError::Syntax(format!(
                            "`?[` 之后要 `]`，实际 {:?}（第 {} 行，列 {}-{}）",
                            lexed.lexemes.get(next),
                            site.line_start,
                            site.col_start,
                            site.col_end
                        )));
                    }
                    let subject_span = question_span;
                    let subscript_span = question_span.to(lexed.spans[next]);
                    let rest = Expression::Subscript(
                        Box::new(Expression::ChainSubject(subject_span)),
                        Box::new(key),
                        subscript_span,
                    );
                    let (rest, after) = parse_postfix_chain(lexed, rest, next + 1)?;
                    let span = term.span().to(rest.span());
                    return Ok((
                        Expression::SafeChain {
                            receiver: Box::new(term),
                            rest: Box::new(rest),
                            span,
                        },
                        after,
                    ));
                }
                other => {
                    let site = lexed.spans.get(cursor).copied().unwrap_or(Span::new(1, 1, 0, 0));
                    return Err(CompileError::Syntax(format!(
                        "`?` 后面要 `.` 或 `[`（空安全操作符；`??` 是空合并），实际 {other:?}\
                         （第 {} 行，列 {}-{}）",
                        site.line_start, site.col_start, site.col_end
                    )));
                }
            }
        }
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
            let (call, next) = parse_call_suffix(lexed, term, cursor)?;
            term = call;
            cursor = next;
            continue;
        }
        if lexed.lexemes.get(cursor) == Some(&Lexeme::LeftBracket) {
            let (subscript, next) = parse_subscript_suffix(lexed, term, cursor)?;
            term = subscript;
            cursor = next;
            continue;
        }
        // 后缀都不是 ⇒ 链到头了
        break;
    }
    Ok((term, cursor))
}

/// 一个**调用后缀**（`f(args…)`／`obj.m(args…)`；`cursor` 指着 `(`）。
///
/// 从 `parse_atom` 的统一后缀链里**原样抽出**（`BC-63` 要为 `?.`／`?[` 递归复用同一段后缀逻辑，
/// 不抽出来就得复制整段实参表解析）。行为与抽出前逐字节相同。
pub(crate) fn parse_call_suffix(
    lexed: &Lexed,
    function: Expression,
    cursor: usize,
) -> Result<(Expression, usize), CompileError> {
    let callee_span = function.span();
    // 左括号的位点（第 140 轮）：实参里出现生成器表达式时，参照给它的跨度是**这对括号之间**
    // （实测 `sum(i for i in g if i > 0)` ⇒ (14,37) ✓）⇒ 这里记下来备用 ✓。
    let open_paren_span = lexed.spans[cursor];
    let mut cursor = cursor + 1;
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
        // `*表达式`：**关键字实参之后也合法** ✓（第 405 轮，`pickle.py:615` 的
        // `self.save_reduce(obj=obj, *rv)` ✓）。参照的求值次序实测是**所有 `*` 先、
        // 关键字值后**（与发射器的"位置／星号在前、关键字 map 在后"同构 ✓）；
        // **`**` 之后不行** ✗（参照也是语法错："iterable argument unpacking follows
        // keyword argument unpacking" ✓）。
        if lexed.lexemes.get(cursor) == Some(&Lexeme::Star) {
            if !dict_arguments.is_empty() {
                return Err(CompileError::Syntax(
                    "`*` 不能出现在 `**` 实参之后".to_owned(),
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
                    let span = lexed.spans[cursor];
                    return Err(CompileError::Syntax(format!(
                        "实参表里出现 {other:?}（第 {} 行，列 {}-{}）",
                        span.line_start, span.col_start, span.col_end
                    )));
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
                    let span = lexed.spans[cursor];
                    return Err(CompileError::Syntax(format!(
                        "实参表里出现 {other:?}（第 {} 行，列 {}-{}）",
                        span.line_start, span.col_start, span.col_end
                    )));
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
            // **`+`／`-` 开头的实参是合法的一元式**（第 318 轮）：`f(-1.5)`／`f(x, +2)` ——
            // 参照都接受 ✓。先前一律报"实参表里出现运算符" ✗ ⇒ 只留 `<`／`>` 那条（真正的
            // 语法错形态 ✓：缺逗号之类）。
            if matches!(
                lexed.lexemes.get(cursor),
                Some(Lexeme::Less) | Some(Lexeme::Greater)
            ) {
                return Err(CompileError::Syntax("实参表里出现运算符".to_owned()));
            }
            // **位置实参允许裸海象**（第 316 轮，参照口径 ✓）：`f(x := 5)` ✓ ——
            // 先前一律走 `parse_expression` ✗ ⇒ 它不吃裸海象 ⇒ 下一个词素是 `Walrus` ⇒
            // 报"实参表里出现 Some(Walrus)" ✗（`Lib/_py_warnings.py:436` 的
            // `_is_internal_filename(filename := frame.f_code.co_filename)` 正卡它 ✓）。
            // 关键字实参那一路**照旧**不吃裸海象 ✓（参照里 `f(a=x := 1)` 本身是语法错 ✓）。
            let (argument, next) = parse_condition(lexed, cursor)?;
            // **实参位置的生成器表达式**（第 125 轮）：`f(x for x in y)` ✓ ——
            //   参照允许它**只作为唯一实参**（否则要加括号 ✓）⇒ 这里如实要求唯一 ✓。
            if { reject_async_comprehension(lexed, next)?; matches!(lexed.lexemes.get(next), Some(Lexeme::For)) } {
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
                // **跨度＝从元素到最后一个生成器**（第 140 轮实测：`sum(i for i in g if i > 0)`
                //   里生成器表达式的跨度是 `(14,37)` ✓，而不是元素自身 `(15,16)` ✗）
                // 末尾取**闭括号那个词素的起点**（第 140 轮：`.to()` 取的是对方起点 ✓ ⇒
                //   `(14,37)` 里的 37 正是 `)` 的列 ✓）
                let span = open_paren_span.to(lexed.spans[after]);
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
                let span = lexed.spans[cursor];
                return Err(CompileError::Syntax(format!(
                    "实参表里出现 {other:?}（第 {} 行，列 {}-{}）",
                    span.line_start, span.col_start, span.col_end
                )));
            }
        }
    }
    let closing = lexed
        .spans
        .get(cursor.saturating_sub(1))
        .copied()
        .unwrap_or(callee_span);
    Ok((
        Expression::Call {
            function: Box::new(function),
            arguments,
            star_arguments,
            keywords,
            dict_arguments,
            callee_span,
            span: callee_span.to(closing),
        },
        cursor,
    ))
}

/// 一个**下标后缀**（`a[键]`；`cursor` 指着 `[`）。从统一后缀链里原样抽出（同 [`parse_call_suffix`]）。
pub(crate) fn parse_subscript_suffix(
    lexed: &Lexed,
    container: Expression,
    cursor: usize,
) -> Result<(Expression, usize), CompileError> {
    let start = container.span();
    let (key, next) = parse_subscript_key(lexed, cursor + 1)?;
    if lexed.lexemes.get(next) != Some(&Lexeme::RightBracket) {
        // **带上位点**（第 315 轮）：先前的消息定不了是哪一行（`traceback` 那族 15 个模块卡在这）。
        let site = lexed.spans.get(next).copied().unwrap_or(start);
        return Err(CompileError::Syntax(format!(
            "`[` 之后要 `]`，实际 {:?}（第 {} 行，列 {}-{}）",
            lexed.lexemes.get(next),
            site.line_start,
            site.col_start,
            site.col_end
        )));
    }
    let span = start.to(lexed.spans[next]);
    Ok((
        Expression::Subscript(Box::new(container), Box::new(key), span),
        next + 1,
    ))
}


// ---- 把编译产物装成真的 `CodeObject`（`P1-10` 与执行器／属性面的接缝） ----

/// `parse_atom` 的 `Bytes` 分支（原样搬入，`break 'atom` 语义不变）。
fn parse_atom_bytes(lexed: &Lexed, cursor: usize, span: Span) -> Result<(Expression, usize), CompileError> {
    let (term, cursor) = {
        match lexed.lexemes.get(cursor) {
            Some(Lexeme::Bytes(bytes)) => {
            let mut merged = bytes.clone();
            let mut end_span = span;
            let mut cursor = cursor + 1;
            while let Some(Lexeme::Bytes(next)) = lexed.lexemes.get(cursor) {
                merged.extend_from_slice(next);
                if let Some(next_span) = lexed.spans.get(cursor) {
                    end_span = *next_span;
                }
                cursor += 1;
            }
            let _ = end_span;
            (Expression::Bytes(merged, span), cursor)
            }
            _ => unreachable!("parse_atom_bytes 只接收 Bytes"),
        }
    };
    Ok((term, cursor))
}

/// `parse_atom` 的 `Str` 分支（原样搬入，`break 'atom` 语义不变）。
fn parse_atom_str(lexed: &Lexed, cursor: usize, span: Span) -> Result<(Expression, usize), CompileError> {
    let (term, cursor) = {
        match lexed.lexemes.get(cursor) {
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
                let fspan = fstring.span();
                let mut parts = match fstring {
                    Expression::FString { parts, .. } => parts,
                    Expression::Str(text, mspan) => vec![FStringPart::Literal { text, span: mspan }],
                    other => return Ok((other, after)),
                };
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
                // **后面若还有相邻字面量（含**第二枚** f-string ✓）继续吃** ✓ ——
                // task-19 第 3 轮真 bug：先前只吃一枚后随 f-string ✗ ⇒
                // `('Field(' f'name={…!r},' f'type={…!r},' …)` 报「括号没有闭合」✗。
                let (parts, after, tail_span) =
                    consume_adjacent_string_parts(lexed, after, parts, fspan)?;
                let parts = merge_adjacent_literal_parts(parts);
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
                    return Ok((Expression::Str(joined, span.to(tail_span)), after));
                }
                return Ok((Expression::FString { parts, span: span.to(tail_span) }, after));
            }
            (Expression::Str(merged, span.to(end_span)), cursor)
            }
            _ => unreachable!("parse_atom_str 只接收 Str"),
        }
    };
    Ok((term, cursor))
}

/// `parse_atom` 的 `FStr` 分支（原样搬入，`break 'atom` 语义不变）。
fn parse_atom_fstr(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (term, cursor) = {
        match lexed.lexemes.get(cursor) {
            Some(Lexeme::FStr { .. }) => {
            let (first, after) = parse_fstring(lexed, cursor)?;
            let first_span = first.span();
            let parts = match first {
                Expression::FString { parts, .. } => parts,
                // **无插值的 f-string 会被降级成 `Str`** ✓ ⇒ **照样要继续拼** ✓
                //（第 222 轮真 bug 修复 ✗：先前这里直接 `return` ⇒ `f"a" f"b"` 只吃下第一个 ✗
                // ⇒ 下一个 f-string 留在原地 ⇒ 报 "实参表里出现 Some(FStr …)" ✗，
                // 上游 `Lib/_collections_abc.py:481` 正是这个形状 ✓）。
                Expression::Str(text, span) => vec![FStringPart::Literal { text, span }],
                other => return Ok((other, after)),
            };
            // **后续相邻字面量**（`FStr`／`Str` 都吃 ✓）走同一处 ✓
            let (parts, after, tail_span) =
                consume_adjacent_string_parts(lexed, after, parts, first_span)?;
            // **边界处的相邻字面段要合并**（实测 `f"a{x} " f"b{x}"` ⇒ 段是 `'a'`／插值／
            //   `' b'`／插值 ✓，不是 `' '` 与 `'b'` 两段 ✗）；合并段的跨度取**两段首尾** ✓
            //   （与 `"a" "b" f"c{d}"` 那条口径一致 ✓）。
            let merged_parts = merge_adjacent_literal_parts(parts);
            (
                Expression::FString {
                    parts: merged_parts,
                    span: first_span.to(tail_span),
                },
                after,
            )
            }
            _ => unreachable!("parse_atom_fstr 只接收 FStr"),
        }
    };
    Ok((term, cursor))
}

/// `parse_atom` 的 `LeftBrace` 分支（原样搬入，`break 'atom` 语义不变）。
fn parse_atom_leftbrace(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (term, cursor) = 'atom: {
        match lexed.lexemes.get(cursor) {
            Some(Lexeme::LeftBrace) => {
            let start = lexed.spans[cursor];
            let mut cursor = cursor + 1;
            let mut pairs: Vec<MapItem> = Vec::new();
            loop {
                if lexed.lexemes.get(cursor) == Some(&Lexeme::RightBrace) {
                    cursor += 1;
                    break;
                }
                // **`**映射`**（第 293 轮）：字典显示里的解包项 ✓ —— 照参照 `dis` 实测，
                // 它发 `LOAD <映射>; DICT_UPDATE 1` ✓（`{**a, **b}`／`{1: 2, **a, 3: 4}` ✓）。
                if lexed.lexemes.get(cursor) == Some(&Lexeme::DoubleStar) {
                    let (value, next) = parse_expression(lexed, cursor + 1)?;
                    cursor = next;
                    pairs.push(MapItem::Unpack(value));
                    match lexed.lexemes.get(cursor) {
                        Some(Lexeme::Comma) => {
                            cursor += 1;
                            continue;
                        }
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
                let (key, next) = parse_expression(lexed, cursor)?;
                cursor = next;
                // **集合推导式**：`{<元素> for <目标> in <可迭代> [if <条件>]*}`
                if { reject_async_comprehension(lexed, cursor)?; matches!(lexed.lexemes.get(cursor), Some(Lexeme::For)) } {
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
                    break 'atom (Expression::Comprehension {
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
                        cursor + 1,);
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
                    // **不能提前 `return Ok`** ✗（第 412 轮，task-24 ✓）：那会**绕过统一后缀链** ✓
                    // ⇒ `{1, 2}.__contains__(1)` 报「实参表里出现 Some(Dot)」✗（`wsgiref/handlers.py:24`
                    // 的 `_is_request = {…}.__contains__` 正是这形状 ⇒ `wsgiref.simple_server` 卡住 ✓）。
                    // 与字典／列表字面量同口径：交回 `'atom` 让后缀链接着处理 ✓。
                    break 'atom (Expression::SetLiteral(items, span), cursor);
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
                if { reject_async_comprehension(lexed, cursor)?; matches!(lexed.lexemes.get(cursor), Some(Lexeme::For)) } {
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
                    break 'atom (Expression::Comprehension {
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
                        cursor + 1,);
                }
                pairs.push(MapItem::Pair(key, value));
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
            _ => unreachable!("parse_atom_leftbrace 只接收 LeftBrace"),
        }
    };
    Ok((term, cursor))
}

/// `parse_atom` 的 `LeftParen` 分支（原样搬入，`break 'atom` 语义不变）。
fn parse_atom_leftparen(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (term, cursor) = 'atom: {
        match lexed.lexemes.get(cursor) {
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
                    if { reject_async_comprehension(lexed, after_element)?; matches!(lexed.lexemes.get(after_element), Some(Lexeme::For)) } {
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
                        break 'atom (Expression::Comprehension {
                                kind: ComprehensionKind::Generator,
                                element: Box::new(element),
                                value: None,
                                generators,
                                span,
                            },
                            after + 1,);
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
                // **不能在这里 `return`**（第 304 轮修）：`parse_atom` 的**统一后缀链**
                // （`.`／`(`／`[`，在 `match` 之后）会被绕过 ⇒ `(ch := …).isspace()`
                // 报「括号没有闭合，实际 `Some(Dot)`」（`Lib/traceback.py:923`，那一族 15 个模块）。
                // 这一臂照常**返回元组**，后缀链就能接上。
                (
                    Expression::Walrus {
                        target,
                        target_span,
                        value: Box::new(value),
                        span,
                    },
                    next + 1,
                )
            } else {
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
            // **括号结果也要走后缀链**（第 127 轮）：`(expr).attr` / `(expr)(args)` ✓
            //（此前这里提前 return ✗ ⇒ `(int(x) & 0xFFFFFFFF).to_bytes(...)` 报「语句结尾多出 .」✗）
            (expression, cursor + 1)
            }
            }
            _ => unreachable!("parse_atom_leftparen 只接收 LeftParen"),
        }
    };
    Ok((term, cursor))
}

/// **`async` 推导式**（`[i async for i in aiter]` ✓ 合法 Python）⇒ 本层**如实报未接线** ✓。
///
/// 为什么要这一步 ✗：`async` 是硬关键字但本层词法器给的是 `Name("async")` ⇒ 列表／集合／字典／
/// 生成器表达式在元素后面看到它时按"字面量下一项"收 ⇒ 报**语法错**"列表字面量里出现
/// Some(Name(\"async\"))" ✗ —— 那是**把合法 Python 拒了** ✓（`MS-19` 口径不许）。
/// 真正接线要 `GET_AITER`／`GET_ANEXT`／`END_ASYNC_FOR` 那一套（不在本批范围）⇒ 这里**明确**
/// 报 `Unsupported`（状态 5＝未实现，不是语法错 ✓），别静默编错语义 ✓。
fn reject_async_comprehension(lexed: &Lexed, cursor: usize) -> Result<(), CompileError> {
    if matches!(
        lexed.lexemes.get(cursor),
        Some(Lexeme::Name(name)) if name == "async"
    ) && matches!(lexed.lexemes.get(cursor + 1), Some(Lexeme::For))
    {
        return Err(CompileError::Unsupported(
            "`async` 推导式尚未接线（GET_AITER／GET_ANEXT 那一套）".to_owned(),
        ));
    }
    Ok(())
}

/// `parse_atom` 的 `LeftBracket` 分支（原样搬入，`break 'atom` 语义不变）。
fn parse_atom_leftbracket(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (term, cursor) = 'atom: {
        match lexed.lexemes.get(cursor) {
            Some(Lexeme::LeftBracket) => {
            let start = lexed.spans[cursor];
            // 这个位置的 `cursor` 是**不可变参数**（外层要到 match 之后才 `let (mut term, mut cursor)`）
            // ⇒ 这里遮蔽一个本地可变的
            let mut cursor = cursor + 1;
            let mut items = Vec::new();
            if lexed.lexemes.get(cursor) != Some(&Lexeme::RightBracket) {
                // **首项也可能是星号** ✓（第 224 轮）：`[*a, b]` ✓ —— 先前这里用 `parse_expression` ✗
                // ⇒ 星号当场报「表达式里出现 `Some(Star)`」✗（上游 `_collections_abc.py:479` 的
                // `(*t_args, t_result)`／`[*…]` 一族正是这个形状 ✓）。星号项不会被当成推导式元素 ✓
                //（后面跟的不是 `for` ✓）⇒ 照旧落到下面那条"逐项"的路上 ✓。
                let (first, next) = parse_star_or_expression(lexed, cursor)?;
                cursor = next;
                // **清单推导式**（3.12+ 内联；实测骨架见发射臂）：`[<元素> for <目标> in <可迭代>
                // [if <条件>]*]`。多重 `for` 的融合指令选择属优化器细节 ⇒ 暂如实报未接线
                if { reject_async_comprehension(lexed, cursor)?; matches!(lexed.lexemes.get(cursor), Some(Lexeme::For)) } {
                    let (target, after_target) = parse_comprehension_target(lexed, cursor + 1)?;
                    // **看 `after_target`，不是 `cursor + 2`** ✗（第 281 轮真 bug 修 ✓）：
                    // 先前这里写死"目标只有一个词元" ✗ ⇒ `[a for a, b in …]`（元组目标 ✓）
                    // 与 `[a for (a, b) in …]`（带括号 ✓）**全部**误报"后面要 `in`" ✗ ——
                    // 而集合／字典推导式那两条用的是 `after_target` ✓ ⇒ 同一形状在 `{…}` 里能跑、
                    // 在 `[…]` 里不能跑 ✓（实测 ✓）。
                    if !matches!(lexed.lexemes.get(after_target), Some(Lexeme::In))
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
                    break 'atom (Expression::Comprehension {
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
                        cursor + 1,);
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
            _ => unreachable!("parse_atom_leftbracket 只接收 LeftBracket"),
        }
    };
    Ok((term, cursor))
}

