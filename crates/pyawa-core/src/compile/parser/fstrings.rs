//! `parser` 的 `fstrings` 域：f-string 的分段解析与相邻字面量合并。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

/// **f-string**：把原文切成"字面段／插值段"；插值里的表达式按**内容起始列**平移跨度后重新词法解析。
pub(crate) fn parse_fstring(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let Some(Lexeme::FStr {
        contents,
        offset,
        raw,
    }) = lexed.lexemes.get(cursor)
    else {
        unreachable!("只由 `FStr` 词法进入");
    };
    let span = lexed.spans[cursor];
    let parts = parse_fstring_parts(contents, *raw, span.line_start, *offset, lexed.mode)?;
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

/// **相邻字面段合并** ✓（`f"a{x} " f"b{x}"` ⇒ `' b'` **一段** ✓）：合并段跨度取两段首尾 ✓
/// （与 `"a" "b" f"c{d}"` 那条口径一致 ✓）。
pub(crate) fn merge_adjacent_literal_parts(parts: Vec<FStringPart>) -> Vec<FStringPart> {
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
    merged_parts
}

/// **继续吃后续相邻的字面量** ✓（`FStr` 与 `Str` 都吃 ✓）—— 「一处真相」：
/// `f"a{x}" f"b"`／`"p" f"a" f"b"`／`f"a" "b"` 三段路都走它 ✓。
///
/// **task-19 第 3 轮真 bug** ✗：`Str` 臂先前只吃**一枚**后随 f-string ✗ ⇒
/// `('Field(' f'name={…!r},' f'type={…!r},' ')')` 里**第二枚**留在原地 ⇒ 括号那一组报
/// 「括号没有闭合，实际 Some(FStr …)」✗（`Lib/dataclasses.py:314` 正是这个形状 ✓，
/// 它把 `_colorize`／`traceback`／`logging` 一族全挡在上面 ✓）。
pub(crate) fn consume_adjacent_string_parts(
    lexed: &Lexed,
    mut after: usize,
    mut parts: Vec<FStringPart>,
    mut tail_span: Span,
) -> Result<(Vec<FStringPart>, usize, Span), CompileError> {
    loop {
        match lexed.lexemes.get(after) {
            Some(Lexeme::FStr { .. }) => {
                // 整体跨度的**尾**取**词素跨度**（实测 `f"a{x}" f"b"` 的 `BUILD_STRING` 位点是
                // 第二个字面量的整体范围 ✓，不是降级后 `Str` 的内容范围 ✗）。
                let literal_span = lexed.spans[after];
                let (next, next_after) = parse_fstring(lexed, after)?;
                tail_span = literal_span;
                match next {
                    Expression::FString { parts: more, .. } => parts.extend(more),
                    Expression::Str(text, span) => parts.push(FStringPart::Literal { text, span }),
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
    Ok((parts, after, tail_span))
}

/// **f-string 后面紧跟普通字符串**（`f"a{c}" "b"`）：并进它的**最后一段**（第 285 轮）。
/// 参照实测：后接串并入最后一段字面量；`"a" f"b{c}"` 则是并入**第一段**（那一半在 `Str` 分支做）。
pub(crate) fn merge_fstring_tail(mut value: Expression, mut cursor: usize, lexed: &Lexed) -> (Expression, usize) {
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
/// `mode` 跟着穿到底（插值里的表达式要按**同一个模式**重新词法，`TS-46` 的 `?` 才认得出 ✓）。
pub(crate) fn parse_fstring_parts(
    contents: &str,
    raw: bool,
    line: u32,
    offset: u32,
    mode: Mode,
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
                        // **所有括号都要计深** ✓（第 223 轮真 bug 修复 ✗）：只数 `{}` 时，
                        // `f"{a[:-1]}"` 里的**切片冒号**会被当成格式分隔符 ✗ ⇒ 扫描冲出字符串 ⇒
                        // 报「表达式里出现 `Some(End)`」✗（上游 `_collections_abc.py:490` 正是这个形状 ✓）。
                        '{' | '[' | '(' => depth += 1,
                        '}' | ']' | ')' => {
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
                    // **带上位点**（第 305 轮）：先前只有一句"expecting '}'" ✗ ⇒ 定不了是哪一条
                    // f-string（`Lib/traceback.py` 那一族就是被它挡着 ✓）。位点取**这个插值的起点** ✓。
                    return Err(CompileError::Syntax(format!(
                        "f-string: expecting '}}'（插值起始于第 {} 行，列 {}）",
                        line, col_now
                    )));
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
                            return Err(CompileError::Syntax(format!(
                                "f-string: expecting '}}'（转换说明之后，第 {} 行，列 {}）",
                                line, col_now
                            )));
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
                let body_text: String = characters[index + 1..expression_end].iter().collect();
                // **调试形态 `f"{表达式=}"`**（第 286 轮；参照逐条实测 ✓）：正文是**段内原文**
                //（`{ x = }` ⇒ 字面量 ` x = ` ✓，`=` 前后空白都留着 ✓），表达式是 `=` 之前那段
                // **去掉首尾空白** ✓；没写转换时**默认 `!r`** ✓（`f"{x=}"` ⇒ `x=5`、
                // `f"{s=!s}"` ⇒ `s=hi` ✓）。判据：**去掉尾部空白后**最后一个 `=` 且它前面**不是**
                // `=!<>:` 之一（那样它是 `==`／`!=`／`<=`／`>=`／`:=` 的一部分 ✓）。
                let trimmed = body_text.trim_end();
                let debug_split = if let Some(without_equal) = trimmed.strip_suffix('=') {
                    let previous = without_equal.chars().last();
                    if matches!(previous, Some('=' | '!' | '<' | '>' | ':')) {
                        None
                    } else {
                        Some(without_equal.len())
                    }
                } else {
                    None
                };
                let (expression_text, debug_literal) = match debug_split {
                    Some(split) => {
                        let text = body_text
                            .chars()
                            .take(split)
                            .collect::<String>()
                            .trim()
                            .to_owned();
                        (text, Some(body_text.clone()))
                    }
                    None => (body_text.clone(), None),
                };
                let expression =
                    parse_fstring_expression(&expression_text, expression_line, expression_col, mode)?;
                if let Some(literal_text) = debug_literal {
                    // 字面量那一段的位点：从 `{` 之后到表达式段末尾（与参照同形即可 ✓）
                    parts.push(FStringPart::Literal {
                        text: literal_text,
                        span: Span::new(open_line, line_now, open_col, col_now),
                    });
                }
                let conversion = conversion.or(if debug_split.is_some() { Some(2) } else { None });
                let spec = match spec_text {
                    None => None,
                    Some(text) if text.is_empty() => Some(Vec::new()),
                    Some(text) => {
                        let (spec_line, spec_col) =
                            position_at(open_line, open_col, index, spec_offset);
                        Some(parse_fstring_parts(&text, raw, spec_line, spec_col, mode)?)
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
pub(crate) fn parse_fstring_expression(
    text: &str,
    line: u32,
    column: u32,
    mode: Mode,
) -> Result<Expression, CompileError> {
    // **抹掉两端空白**（第 305 轮修）：`f"{ w }"` 是合法的 ✓ —— 先前把片段**原样**再词法化 ✗
    // ⇒ 前导空格被当成**缩进** ⇒ 词素里出现 `Indent` ⇒ 报"表达式里出现 `Some(Indent)`" ✗
    //（`Lib/traceback.py` 那一族 **15** 个模块就卡在这一格 ✓）。抹掉的**字符数要补回列号** ✓，
    // 否则片段里所有位点都会偏 ✗。
    let leading = text.chars().take_while(|character| character.is_whitespace()).count();
    let column = column + leading as u32;
    let text = text.trim();
    if text.contains('\n') {
        return Err(CompileError::Unsupported(
            "f-string 里跨行的表达式尚未接线".to_owned(),
        ));
    }
    let mut lexed = lex(text, mode)?;
    for span in &mut lexed.spans {
        *span = Span::new(
            span.line_start + line - 1,
            span.line_end + line - 1,
            span.col_start + column,
            span.col_end + column,
        );
    }
    // **元组表达式**（task-42 ✓）：`f"{w1, w2, w3}"` 是合法的（官方 `test_minidom.py:208`）；
    // 先前用 `parse_expression` ⇒ 在 `,` 上报"没能整段解析" ✗。
    let (expression, next) = parse_expression_list(&lexed, 0)?;
    for lexeme in &lexed.lexemes[next..] {
        if !matches!(lexeme, Lexeme::Newline | Lexeme::Dedent | Lexeme::Indent | Lexeme::End) {
            return Err(CompileError::Syntax(format!(
                "f-string 里的表达式 {text:?} 没能整段解析（多出 {lexeme:?}，已读到第 {next} 个）"
            )));
        }
    }
    Ok(expression)
}

