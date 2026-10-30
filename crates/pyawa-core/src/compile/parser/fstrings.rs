//! `parser` 的 `fstrings` 域：f-string 的分段解析与相邻字面量合并。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

/// 用**WTF-8 字节**造一个字面段：合法 UTF-8 仍是旧的 [`FStringPart::Literal`] ✓，
/// 含孤立代理才走新的 [`FStringPart::LiteralWtf8`]（P3-38 S2b ✓）。
pub(crate) fn literal_part_from_bytes(bytes: Vec<u8>, span: Span) -> FStringPart {
    match String::from_utf8(bytes) {
        Ok(text) => FStringPart::Literal { text, span },
        Err(error) => FStringPart::LiteralWtf8 {
            bytes: error.into_bytes(),
            span,
        },
    }
}

/// 字面段的 WTF-8 字节视图（两种形态都行 ✓）。
pub(crate) fn literal_part_bytes(part: &FStringPart) -> Option<&[u8]> {
    match part {
        FStringPart::Literal { text, .. } => Some(text.as_bytes()),
        FStringPart::LiteralWtf8 { bytes, .. } => Some(bytes),
        _ => None,
    }
}

/// 字面段的跨度。
pub(crate) fn literal_part_span(part: &FStringPart) -> Option<Span> {
    match part {
        FStringPart::Literal { span, .. } | FStringPart::LiteralWtf8 { span, .. } => Some(*span),
        _ => None,
    }
}

/// 用**WTF-8 字节**造一个字符串表达式（合法 UTF-8 ⇒ 旧的 `Expression::Str` ✓，代理 ⇒ `StrWtf8`）。
pub(crate) fn string_expression_from_bytes(bytes: Vec<u8>, span: Span) -> Expression {
    match String::from_utf8(bytes) {
        Ok(text) => Expression::Str(text, span),
        Err(error) => Expression::StrWtf8(error.into_bytes(), span),
    }
}

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
    if parts.iter().all(|part| {
        matches!(
            part,
            FStringPart::Literal { .. } | FStringPart::LiteralWtf8 { .. }
        )
    }) {
        let mut joined: Vec<u8> = Vec::new();
        for part in &parts {
            joined.extend_from_slice(literal_part_bytes(part).expect("刚判过全是字面段"));
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
        return Ok(merge_fstring_tail(
            string_expression_from_bytes(joined, lowered_span),
            cursor + 1,
            lexed,
        ));
    }
    Ok(merge_fstring_tail(
        Expression::FString { parts, span },
        cursor + 1,
        lexed,
    ))
}

/// **相邻字面段合并** ✓（`f"a{x} " f"b{x}"` ⇒ `' b'` **一段** ✓）：合并段跨度取两段首尾 ✓
/// （与 `"a" "b" f"c{d}"` 那条口径一致 ✓）。P3-38 S2b：按 **WTF-8 字节**合并
/// （合法 UTF-8 结果仍产出旧同形的 `Literal` ✓；混入代理后改走 `LiteralWtf8` ✓）。
pub(crate) fn merge_adjacent_literal_parts(parts: Vec<FStringPart>) -> Vec<FStringPart> {
    let mut merged_parts: Vec<FStringPart> = Vec::with_capacity(parts.len());
    for part in parts {
        if let (Some(head), Some(head_span)) = (
            merged_parts.last().and_then(literal_part_bytes).map(<[u8]>::to_vec),
            merged_parts.last().and_then(literal_part_span),
        ) {
            if let (Some(tail), Some(tail_span)) = (literal_part_bytes(&part), literal_part_span(&part))
            {
                let mut bytes = head;
                bytes.extend_from_slice(tail);
                let merged_span = head_span.to(tail_span);
                *merged_parts.last_mut().expect("刚判过非空") =
                    literal_part_from_bytes(bytes, merged_span);
                continue;
            }
        }
        merged_parts.push(part);
    }
    merged_parts
}

/// **继续吃后续相邻的字面量** ✓（`FStr`／`Str`／`StrWtf8` 都吃 ✓）—— 「一处真相」：
/// `f"a{x}" f"b"`／`"p" f"a" f"b"`／`f"a" "b"` 三段路都走它 ✓。
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
                    Expression::StrWtf8(bytes, span) => {
                        parts.push(FStringPart::LiteralWtf8 { bytes, span })
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
            Some(Lexeme::StrWtf8(bytes)) => {
                let bytes = bytes.clone();
                let span = lexed.spans[after];
                tail_span = span;
                parts.push(FStringPart::LiteralWtf8 { bytes, span });
                after += 1;
            }
            _ => break,
        }
    }
    Ok((parts, after, tail_span))
}

/// **f-string 后面紧跟普通字符串**（`f"a{c}" "b"`）：并进它的**最后一段**（第 285 轮）。
/// 参照实测：后接串并入最后一段字面量；`"a" f"b{c}"` 则是并入**第一段**（那一半在 `Str` 分支做）。
/// P3-38 S2b：按 **WTF-8 字节**合并（`StrWtf8` 也认 ✓）。
pub(crate) fn merge_fstring_tail(
    value: Expression,
    mut cursor: usize,
    lexed: &Lexed,
) -> (Expression, usize) {
    let mut tail: Vec<u8> = Vec::new();
    let mut end_span = value.span();
    loop {
        match lexed.lexemes.get(cursor) {
            Some(Lexeme::Str(next_text)) => tail.extend_from_slice(next_text.as_bytes()),
            Some(Lexeme::StrWtf8(next_bytes)) => tail.extend_from_slice(next_bytes),
            _ => break,
        }
        if let Some(next_span) = lexed.spans.get(cursor) {
            end_span = *next_span;
        }
        cursor += 1;
    }
    if tail.is_empty() {
        return (value, cursor);
    }
    let value = match value {
        Expression::FString { mut parts, span } => {
            match parts.last().and_then(literal_part_span) {
                Some(last_span) => {
                    let mut bytes = literal_part_bytes(parts.last().expect("刚取到跨度"))
                        .expect("刚取到跨度")
                        .to_vec();
                    bytes.extend_from_slice(&tail);
                    let index = parts.len() - 1;
                    parts[index] = literal_part_from_bytes(bytes, last_span.to(end_span));
                }
                None => parts.push(literal_part_from_bytes(tail, end_span)),
            }
            Expression::FString {
                parts,
                span: span.to(end_span),
            }
        }
        Expression::Str(text, span) => {
            let mut bytes = text.into_bytes();
            bytes.extend_from_slice(&tail);
            string_expression_from_bytes(bytes, span.to(end_span))
        }
        Expression::StrWtf8(bytes, span) => {
            let mut bytes = bytes;
            bytes.extend_from_slice(&tail);
            string_expression_from_bytes(bytes, span.to(end_span))
        }
        other => other,
    };
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
    // P3-38 S2b：字面段按 **WTF-8 字节**累积（合法 UTF-8 段最终仍产出旧同形的 Literal ✓）
    let mut literal: Vec<u8> = Vec::new();
    let mut literal_line = line;
    let mut literal_col = offset;
    while index < characters.len() {
        match characters[index] {
            '{' if characters.get(index + 1) == Some(&'{') => {
                if literal.is_empty() {
                    literal_line = line_now;
                    literal_col = col_now;
                }
                literal.push(b'{');
                walk!(index + 2);
            }
            '}' if characters.get(index + 1) == Some(&'}') => {
                if literal.is_empty() {
                    literal_line = line_now;
                    literal_col = col_now;
                }
                literal.push(b'}');
                walk!(index + 2);
            }
            '}' => {
                return Err(CompileError::Syntax(
                    "f-string: single '}' is not allowed".to_owned(),
                ))
            }
            '{' => {
                if !literal.is_empty() {
                    parts.push(literal_part_from_bytes(
                        core::mem::take(&mut literal),
                        Span::new(literal_line, line_now, literal_col, col_now),
                    ));
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
                        // **插值里的嵌套字符串要整段跳过**（task-45 ✓）：官方
                        // `test_xml_etree.py:4661` 的 `f"{… ','.join(c.tag.split('}')[-1] …)}"`
                        // —— 先前把字符串里的 `}` 当**插值收尾** ✗ ⇒ 表达式被截断 ⇒
                        // "实参表里出现 Some(End)" ✗。按引号跳过（含三引号、反斜杠转义）。
                        '\'' | '"' => {
                            let quote = characters[scan];
                            let triple = characters.get(scan + 1) == Some(&quote)
                                && characters.get(scan + 2) == Some(&quote);
                            scan += if triple { 3 } else { 1 };
                            while scan < characters.len() {
                                if characters[scan] == '\\' {
                                    scan += 2;
                                    continue;
                                }
                                if characters[scan] == quote {
                                    if triple {
                                        if characters.get(scan + 1) == Some(&quote)
                                            && characters.get(scan + 2) == Some(&quote)
                                        {
                                            scan += 3;
                                            break;
                                        }
                                        scan += 1;
                                    } else {
                                        scan += 1;
                                        break;
                                    }
                                } else {
                                    scan += 1;
                                }
                            }
                            continue;
                        }
                        // **插值里的注释**（PEP 701）：跳过到行尾（不算括号/分隔符）。
                        // **只在表达式部分**（分隔符 `:` 之前）——格式说明里的 `#` 是**格式标志**
                        // （`f'{id(self):#x}'`），不是注释（task-45 ✓）。
                        '#' if separator.is_none() => {
                            while scan < characters.len() && characters[scan] != '\n' {
                                scan += 1;
                            }
                            continue;
                        }
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
                literal.extend_from_slice(&decoded);
                walk!(index + consumed);
            }
            other => {
                if literal.is_empty() {
                    literal_line = line_now;
                    literal_col = col_now;
                }
                let mut buffer = [0u8; 4];
                literal.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
                walk!(index + 1);
            }
        }
    }
    if !literal.is_empty() {
        parts.push(literal_part_from_bytes(
            literal,
            Span::new(literal_line, line_now, literal_col, col_now),
        ));
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

