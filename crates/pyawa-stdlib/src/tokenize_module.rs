//! `_tokenize` 模块（task-19 ✓）：**真词法器** ✓ —— `Lib/tokenize.py:37` 的裸 `import _tokenize` ✓、
//! `generate_tokens`／`tokenize` 全走它的 `TokenizerIter` ✓（3.14 已无纯 Python 回退 ✓）。
//!
//! **一处真相** ✓：扫描**不另写** ✗ —— 底层就是 `pyawa_core::python_tokens` ✓（即编译器的词法器 ✓），
//! 本模块只做**参照 `tokenize` 的投影** ✓：类别 ⇒ `token` 数字 ✓、物理行层（`COMMENT`／`NL` ✓）、
//! `NEWLINE` 的**一位宽尾** ✓、`INDENT` 的空白文本 ✓、`line` 字段 ✓、五元组打包 ✓。
//!
//! **如实范围** ✗（随后补 ✓）：**f-string 整段按 `STRING` 出** ✗（参照给
//! `FSTRING_START`／`FSTRING_MIDDLE`／`FSTRING_END` ✓）；**制表符缩进**照底层如实报未接线 ✓；
//! `encoding` 走 `decode_with_encoding` 的四档 ✓。

use core::cell::Cell;
use core::ptr::NonNull;

use pyawa_core::{
    BuiltinFunctionObject, ExecError, Header, Instance, NativeFn, PythonTokenKind,
};

/// 模块名（`_tokenize`）。
pub const NAME: &str = "_tokenize";

/// 类名（`type(it).__name__` 照参照 ✓）。
const ITER_CLASS: &str = "_tokenize.TokenizerIter";

/// `Lib/token.py` 里的数字（**以 `token.py` 为准** ✓ —— 那是 Python 侧的一处真相 ✓）。
mod tok {
    pub const ENDMARKER: i64 = 0;
    pub const NAME: i64 = 1;
    pub const NUMBER: i64 = 2;
    pub const STRING: i64 = 3;
    pub const NEWLINE: i64 = 4;
    pub const INDENT: i64 = 5;
    pub const DEDENT: i64 = 6;
    pub const OP: i64 = 55;
    pub const FSTRING_START: i64 = 59;
    pub const FSTRING_MIDDLE: i64 = 60;
    pub const FSTRING_END: i64 = 61;
    pub const COMMENT: i64 = 65;
    pub const NL: i64 = 66;
}

/// 一个待产出的参照 token（**五元组**的原料 ✓）。
struct Row {
    kind: i64,
    text: String,
    start: (i64, i64),
    end: (i64, i64),
    line: String,
}

/// 造一个原生可调用对象（**新引用** ✓）。
fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 物理行切分（**行含结尾 `\n`** ✓；最后一行可能没有 ✓）。
fn split_physical_lines(source: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            lines.push(&source[start..index + 1]);
            start = index + 1;
        }
    }
    if start < source.len() {
        lines.push(&source[start..]);
    }
    lines
}

/// 某一物理行的 `NL`（行尾有 `\n` 才发 ✓，与参照同口径 ✓）。
fn push_nl(out: &mut Vec<Row>, lines: &[&str], row: usize) {
    let Some(text) = lines.get(row - 1).copied() else {
        return;
    };
    if !text.ends_with('\n') {
        return;
    }
    let body_len = text.strip_suffix('\n').unwrap_or(text).chars().count();
    out.push(Row {
        kind: tok::NL,
        text: "\n".to_owned(),
        start: (row as i64, body_len as i64),
        end: (row as i64, body_len as i64 + 1),
        line: text.to_owned(),
    });
}

/// 把 `(covered, up_to]` 之间**没有代码 token** 的物理行补成 `NL`／`COMMENT` ✓。
fn synth_lines(out: &mut Vec<Row>, covered: &mut usize, lines: &[&str], up_to: usize) {
    while *covered < up_to {
        let row = *covered + 1;
        *covered = row;
        let Some(text) = lines.get(row - 1).copied() else {
            continue;
        };
        let body = text.strip_suffix('\n').unwrap_or(text);
        let body_len = body.chars().count();
        let hash = body.chars().position(|ch| ch == '#');
        match hash {
            Some(column) if body.chars().take(column).all(|ch| ch == ' ' || ch == '\t' || ch == '\r') => {
                // 整行注释 ⇒ `COMMENT` ＋ `NL` ✓
                out.push(Row {
                    kind: tok::COMMENT,
                    text: body.chars().skip(column).collect(),
                    start: (row as i64, column as i64),
                    end: (row as i64, body_len as i64),
                    line: text.to_owned(),
                });
                if text.ends_with('\n') {
                    out.push(Row {
                        kind: tok::NL,
                        text: "\n".to_owned(),
                        start: (row as i64, body_len as i64),
                        end: (row as i64, body_len as i64 + 1),
                        line: text.to_owned(),
                    });
                }
            }
            _ if body.trim().is_empty() => {
                // 空行（含只有空白的行）⇒ `NL` ✓
                if text.ends_with('\n') {
                    out.push(Row {
                        kind: tok::NL,
                        text: "\n".to_owned(),
                        start: (row as i64, body_len as i64),
                        end: (row as i64, body_len as i64 + 1),
                        line: text.to_owned(),
                    });
                }
            }
            _ => {
                // **续行**（括号里换行 ✓，底层不发 `Newline` ✓）⇒ `NL` ✓
                if text.ends_with('\n') && !out.is_empty() {
                    out.push(Row {
                        kind: tok::NL,
                        text: "\n".to_owned(),
                        start: (row as i64, body_len as i64),
                        end: (row as i64, body_len as i64 + 1),
                        line: text.to_owned(),
                    });
                }
            }
        }
    }
}

/// 取 span 的**源文本** ✓（`(行,列)` 都按**字符**下标 ✓）。
fn slice_span(chars: &[Vec<char>], start: (u32, u32), end: (u32, u32)) -> String {
    let (start_row, start_col) = (start.0 as usize, start.1 as usize);
    let (end_row, end_col) = (end.0 as usize, end.1 as usize);
    let empty = Vec::new();
    if start_row == end_row {
        let line = chars.get(start_row.saturating_sub(1)).unwrap_or(&empty);
        return line.iter().skip(start_col).take(end_col.saturating_sub(start_col)).collect();
    }
    let mut pieces: Vec<String> = Vec::new();
    for row in start_row..=end_row {
        let line = chars.get(row.saturating_sub(1)).unwrap_or(&empty);
        let from = if row == start_row { start_col } else { 0 };
        let to = if row == end_row { end_col } else { line.len() };
        pieces.push(line.iter().skip(from).take(to.saturating_sub(from)).collect());
    }
    pieces.join("")
}

/// 行尾注释：`token_end` 之后到行尾之间第一个 `#` ✓（**本行**才算 ✓）。
fn trailing_comment(chars: &[Vec<char>], token_end: (u32, u32), lines: &[&str]) -> Option<Row> {
    let row = token_end.0 as usize;
    if row == 0 {
        return None;
    }
    let empty = Vec::new();
    let line = chars.get(row - 1).unwrap_or(&empty);
    let from = token_end.1 as usize;
    let hash = line.iter().skip(from).position(|ch| *ch == '#')? + from;
    // **注释文本不含行尾** ✓（参照：`COMMENT '# trailing'` (2,23)-(2,33) ✓ —— 先前把 `\n` 也收进去了 ✗）。
    let text: String = line.iter().skip(hash).collect::<String>();
    let text = text.trim_end_matches(['\n', '\r', '\u{2028}', '\u{2029}']).to_owned();
    if text.trim_end().is_empty() {
        return None;
    }
    let end_column = hash + text.chars().count();
    Some(Row {
        kind: tok::COMMENT,
        text,
        start: (row as i64, hash as i64),
        end: (row as i64, end_column as i64),
        line: lines.get(row - 1).copied().unwrap_or("").to_owned(),
    })
}

/// **f-string 三段拆分** ✓（task-19 第 2 轮 ✓）：一个 `Lexeme::FStr` ⇒ `FSTRING_START` ＋
/// `FSTRING_MIDDLE`／字段 token ＋ `FSTRING_END` ✓（与参照逐 token 对齐 ✓）。
///
/// **口径**（本机 `python3` 3.14 实测 ✓）：
/// - `START` ＝**前缀字母 ＋ 开引号** ✓（`f"` ✓／`rf"` ✓／三引号 ✓）；
/// - 字面文本 ⇒ `MIDDLE` ✓；**`{{`／`}}` 只算一个花括号** ✓（第二个**不占任何 span** ✓：
///   `f"a{{b}}c"` ⇒ `MIDDLE 'a{'` (6,8) ✓、`MIDDLE 'b}'` (9,11) ✓、`MIDDLE 'c'` (12,13) ✓）；
/// - 字段 ⇒ `OP '{'` ＋ **表达式逐 token**（走同一个 `python_tokens` ✓、位置按偏移平移 ✓）
///   ＋（可选）`OP '='`／`OP '!'`＋`NAME <转换字符>`／`OP ':'` ＋ **格式说明**（`MIDDLE` 段 ＋ 嵌套字段 ✓，
///   **说明段结束处一定再发一个 `MIDDLE`** ✓ —— 空串也算 ✓：`f"{x:{w}}"` ⇒ `MIDDLE ''` (12,12) ✓）
///   ＋ `OP '}'` ✓；
/// - 收尾 `END` ＝**闭引号** ✓。
struct FStringScanner<'a> {
    /// 内容字符（开引号之后、闭引号之前 ✓）。
    content: Vec<char>,
    /// 每个内容字符的**绝对位置** ✓（行 1 起、列 0 起 ✓）。
    positions: Vec<(i64, i64)>,
    /// 物理行（`line` 字段用 ✓）。
    lines: &'a [&'a str],
    /// 产出 ✓。
    out: Vec<Row>,
    /// 当前 `MIDDLE` 缓冲 ✓（起位置 ＋ 文本 ✓）。
    middle: Option<(i64, i64, String)>,
    /// 光标 ✓。
    cursor: usize,
}

impl<'a> FStringScanner<'a> {
    fn line_of(&self, row: i64) -> String {
        self.lines
            .get((row - 1).max(0) as usize)
            .copied()
            .unwrap_or("")
            .to_owned()
    }

    fn flush_middle(&mut self) {
        if let Some((row, column, text)) = self.middle.take() {
            let end_column = column + text.chars().count() as i64;
            self.out.push(Row {
                kind: tok::FSTRING_MIDDLE,
                text,
                start: (row, column),
                end: (row, end_column),
                line: self.line_of(row),
            });
        }
    }

    /// **空 `MIDDLE`** ✓（格式说明段收尾那一条 ✓）。
    fn push_empty_middle(&mut self) {
        let (row, column) = self
            .positions
            .get(self.cursor)
            .copied()
            .or_else(|| self.positions.last().map(|(row, column)| (*row, column + 1)))
            .unwrap_or((1, 0));
        self.out.push(Row {
            kind: tok::FSTRING_MIDDLE,
            text: String::new(),
            start: (row, column),
            end: (row, column),
            line: self.line_of(row),
        });
    }

    fn push_text(&mut self, character: char) {
        match &mut self.middle {
            Some((_, _, text)) => text.push(character),
            None => {
                let (row, column) = self.positions[self.cursor];
                self.middle = Some((row, column, character.to_string()));
            }
        }
    }

    /// 吃一个普通字面字符 ✓。
    fn take_literal(&mut self) {
        let character = self.content[self.cursor];
        self.push_text(character);
        self.cursor += 1;
    }

    /// **花括号转义** ✓（`{{`／`}}`）：只把一个花括号写进 `MIDDLE` ✓、第二个**不占 span** ✓、
    /// 并且**把当前 `MIDDLE` 段断开** ✓（参照实测：`f"a{{b}}c"` ⇒ 三段 `'a{'`／`'b}'`／`'c'` ✓）。
    fn take_escape(&mut self, character: char) {
        self.push_text(character);
        self.cursor += 2;
        self.flush_middle();
    }

    /// 位置按子源偏移平移的 token ✓（子源第 1 行从 `col0` 起算 ✓，其余行从 0 ✓）。
    fn push_shifted(
        &mut self,
        kind: i64,
        text: String,
        start: (u32, u32),
        end: (u32, u32),
        row0: i64,
        col0: i64,
    ) {
        let shift = |(row, column): (u32, u32)| -> (i64, i64) {
            let absolute_row = row0 + row as i64 - 1;
            let absolute_column = if row == 1 { col0 + column as i64 } else { column as i64 };
            (absolute_row, absolute_column)
        };
        let start = shift(start);
        let end = shift(end);
        self.out.push(Row {
            kind,
            text,
            start,
            end,
            line: self.line_of(start.0),
        });
    }

    /// **找字段表达式的终点** ✓：字面文本里**先扫**出 `!`／`:`／`=`／`}` ✓
    /// （不能把带 `!` 的整段丢给词法器 ✗ —— `!` 不是合法算符 ⇒ 词法直接失败 ✓）。
    /// 跳过字符串字面量与括号内部 ✓（`f"{d['}']}"` 那种 ✓）。
    fn scan_expression(&self) -> (usize, Option<char>) {
        let mut index = self.cursor;
        let mut depth: i64 = 0;
        while index < self.content.len() {
            let character = self.content[index];
            match character {
                '\'' | '"' => {
                    let quote = character;
                    let triple = self.content.get(index + 1) == Some(&quote)
                        && self.content.get(index + 2) == Some(&quote);
                    let mut step = if triple { 3 } else { 1 };
                    index += step;
                    while index < self.content.len() {
                        let inner = self.content[index];
                        if inner == '\\' {
                            index += 2;
                            continue;
                        }
                        if inner == quote {
                            if triple {
                                if self.content.get(index + 1) == Some(&quote)
                                    && self.content.get(index + 2) == Some(&quote)
                                {
                                    index += 3;
                                    break;
                                }
                                index += 1;
                                continue;
                            }
                            index += 1;
                            break;
                        }
                        if inner == '\n' && !triple {
                            break;
                        }
                        index += 1;
                    }
                    let _ = step;
                    step = 0;
                    let _ = step;
                    continue;
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    index += 1;
                }
                ')' | ']' => {
                    depth -= 1;
                    index += 1;
                }
                '}' => {
                    if depth == 0 {
                        return (index, Some('}'));
                    }
                    depth -= 1;
                    index += 1;
                }
                '!' => {
                    if depth == 0 && self.content.get(index + 1) != Some(&'=') {
                        return (index, Some('!'));
                    }
                    index += 1;
                }
                ':' => {
                    if depth == 0 && self.content.get(index + 1) != Some(&'=') {
                        return (index, Some(':'));
                    }
                    index += 1;
                }
                '=' => {
                    let previous = if index > self.cursor { self.content.get(index - 1).copied() } else { None };
                    let compound = matches!(
                        previous,
                        Some('+' | '-' | '*' | '/' | '%' | '&' | '|' | '^' | '<' | '>' | '@' | '!' | ':')
                    );
                    if depth == 0 && !compound && self.content.get(index + 1) != Some(&'=') {
                        return (index, Some('='));
                    }
                    index += 1;
                }
                _ => index += 1,
            }
        }
        (index, None)
    }

    /// 一个**替换字段** ✓：光标在 `{` 上 ✓。
    fn take_field(&mut self) {
        self.flush_middle();
        let (row, column) = self.positions[self.cursor];
        self.out.push(Row {
            kind: tok::OP,
            text: "{".to_owned(),
            start: (row, column),
            end: (row, column + 1),
            line: self.line_of(row),
        });
        self.cursor += 1;
        let (expression_end, mut terminator) = self.scan_expression();
        // 表达式：**只把这一段**交给词法器 ✓（一处真相 ✓），位置按偏移平移 ✓
        if expression_end > self.cursor {
            let body: Vec<char> = self.content[self.cursor..expression_end].to_vec();
            let (row0, col0) = self.positions[self.cursor];
            let body_text: String = body.iter().collect();
            if let Ok(tokens) = pyawa_core::python_tokens(&body_text) {
                for token in tokens {
                    // **虚拟 token 不进字段** ✓（子源只是一段表达式 ✓ —— 行首的 `Indent`／文尾的
                    // `Dedent` 都是词法器按"独立源码"补的 ✓，参照的字段里没有它们 ✗）。
                    if matches!(
                        token.kind,
                        PythonTokenKind::End
                            | PythonTokenKind::Newline
                            | PythonTokenKind::Indent
                            | PythonTokenKind::Dedent
                    ) {
                        continue;
                    }
                    let text = slice_span(&[body.clone()], token.start, token.end);
                    if token.kind == PythonTokenKind::FString {
                        emit_fstring(&mut self.out, &[body.clone()], self.lines, token.start, token.end);
                        continue;
                    }
                    let kind = match token.kind {
                        PythonTokenKind::Name => tok::NAME,
                        PythonTokenKind::Number => tok::NUMBER,
                        PythonTokenKind::Str | PythonTokenKind::Bytes => tok::STRING,
                        _ => tok::OP,
                    };
                    self.push_shifted(kind, text, token.start, token.end, row0, col0);
                }
            }
        }
        self.cursor = expression_end;
        // 终止符：`=`／`!`／`:`／`}` ✓
        loop {
            let Some(character) = (if terminator.is_some() {
                terminator
            } else {
                self.content.get(self.cursor).copied()
            }) else {
                return;
            };
            match character {
                '=' => {
                    let (row, column) = self.positions[self.cursor];
                    self.out.push(Row {
                        kind: tok::OP,
                        text: "=".to_owned(),
                        start: (row, column),
                        end: (row, column + 1),
                        line: self.line_of(row),
                    });
                    self.cursor += 1;
                    terminator = self.content.get(self.cursor).copied();
                }
                '!' => {
                    let (row, column) = self.positions[self.cursor];
                    self.out.push(Row {
                        kind: tok::OP,
                        text: "!".to_owned(),
                        start: (row, column),
                        end: (row, column + 1),
                        line: self.line_of(row),
                    });
                    self.cursor += 1;
                    if self.cursor < self.content.len() {
                        let conversion = self.content[self.cursor];
                        let (row2, column2) = self.positions[self.cursor];
                        self.out.push(Row {
                            kind: tok::NAME,
                            text: conversion.to_string(),
                            start: (row2, column2),
                            end: (row2, column2 + 1),
                            line: self.line_of(row2),
                        });
                        self.cursor += 1;
                    }
                    terminator = self.content.get(self.cursor).copied();
                }
                ':' => {
                    let (row, column) = self.positions[self.cursor];
                    self.out.push(Row {
                        kind: tok::OP,
                        text: ":".to_owned(),
                        start: (row, column),
                        end: (row, column + 1),
                        line: self.line_of(row),
                    });
                    self.cursor += 1;
                    self.take_format_spec();
                    // **说明段收尾一定再发一个 `MIDDLE`** ✓（空串也算 ✓）
                    if self.middle.is_some() {
                        self.flush_middle();
                    } else {
                        self.push_empty_middle();
                    }
                    terminator = self.content.get(self.cursor).copied();
                }
                '}' => {
                    let (row, column) = self.positions[self.cursor];
                    self.out.push(Row {
                        kind: tok::OP,
                        text: "}".to_owned(),
                        start: (row, column),
                        end: (row, column + 1),
                        line: self.line_of(row),
                    });
                    self.cursor += 1;
                    return;
                }
                _ => return,
            }
        }
    }

    /// **格式说明** ✓：到匹配的外层 `}` 之前 ✓（`MIDDLE` 段 ＋ 嵌套字段 ✓，`{{`／`}}` 只算一个 ✓）。
    fn take_format_spec(&mut self) {
        while self.cursor < self.content.len() {
            let character = self.content[self.cursor];
            match character {
                '{' => {
                    if self.content.get(self.cursor + 1) == Some(&'{') {
                        self.take_escape('{');
                    } else {
                        self.take_field();
                    }
                }
                '}' => {
                    if self.content.get(self.cursor + 1) == Some(&'}') {
                        self.take_escape('}');
                    } else {
                        break;
                    }
                }
                _ => self.take_literal(),
            }
        }
    }
}

/// **一个 f-string 词法单元 ⇒ 参照的三段／字段 token** ✓。
fn emit_fstring(out: &mut Vec<Row>, chars: &[Vec<char>], lines: &[&str], start: (u32, u32), end: (u32, u32)) {
    let text = slice_span(chars, start, end);
    let characters: Vec<char> = text.chars().collect();
    // 前缀字母 ＋ 引号 ✓
    let mut prefix_len = 0usize;
    while matches!(characters.get(prefix_len), Some(ch) if ch.is_ascii_alphabetic()) {
        prefix_len += 1;
    }
    let Some(quote) = characters.get(prefix_len).copied() else {
        return;
    };
    if quote != '\'' && quote != '"' {
        return;
    }
    let triple = characters.get(prefix_len + 1) == Some(&quote)
        && characters.get(prefix_len + 2) == Some(&quote);
    let quote_len = if triple { 3 } else { 1 };
    let opener_len = prefix_len + quote_len;
    let start_row = start.0 as i64;
    let start_column = start.1 as i64;
    out.push(Row {
        kind: tok::FSTRING_START,
        text: characters[..opener_len].iter().collect(),
        start: (start_row, start_column),
        end: (start_row, start_column + opener_len as i64),
        line: lines.get(start.0 as usize - 1).copied().unwrap_or("").to_owned(),
    });
    let content: Vec<char> = characters[opener_len..characters.len().saturating_sub(quote_len)].to_vec();
    // 内容每个字符的绝对位置 ✓
    let mut positions: Vec<(i64, i64)> = Vec::with_capacity(content.len());
    let mut row = start_row;
    let mut column = start_column + opener_len as i64;
    for character in &content {
        positions.push((row, column));
        if *character == '\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    let mut scanner = FStringScanner {
        content,
        positions,
        lines,
        out: Vec::new(),
        middle: None,
        cursor: 0,
    };
    while scanner.cursor < scanner.content.len() {
        let character = scanner.content[scanner.cursor];
        match character {
            '{' => {
                if scanner.content.get(scanner.cursor + 1) == Some(&'{') {
                    scanner.take_escape('{');
                } else {
                    scanner.take_field();
                }
            }
            '}' => {
                if scanner.content.get(scanner.cursor + 1) == Some(&'}') {
                    scanner.take_escape('}');
                } else {
                    scanner.take_literal();
                }
            }
            _ => scanner.take_literal(),
        }
    }
    scanner.flush_middle();
    out.extend(scanner.out);
    // **闭引号** ✓
    let end_row = end.0 as i64;
    let end_column = end.1 as i64;
    out.push(Row {
        kind: tok::FSTRING_END,
        text: characters[characters.len() - quote_len..].iter().collect(),
        start: (end_row, end_column - quote_len as i64),
        end: (end_row, end_column),
        line: lines.get(end.0 as usize - 1).copied().unwrap_or("").to_owned(),
    });
}

/// **源码 ⇒ 参照的 token 流** ✓（`generate_tokens` 的口径 ✓）。
fn tokenize_source(source: &str) -> Result<Vec<Row>, String> {
    let lines: Vec<&str> = split_physical_lines(source);
    // 每行的**字符**数组（列＝字符下标 ✓，与参照同口径 ✓ —— 全角也按 1 列 ✓）。
    let chars: Vec<Vec<char>> = lines.iter().map(|line| line.chars().collect()).collect();
    let core = pyawa_core::python_tokens(source)?;
    let mut out: Vec<Row> = Vec::new();
    // 上一条**已覆盖**的物理行（补 `NL`／`COMMENT` 用 ✓，1 起 ✓）。
    let mut covered: usize = 0;
    // **本行还没发的行尾 `NL`** ✓（隐式续行 ✓ —— 底层对括号里的换行不发 `Newline` ✓）：
    // 下一条 token 落到更后面的行时补 ✓。
    let mut pending_nl: Option<usize> = None;
    for (index, token) in core.iter().enumerate() {
        let next = core.get(index + 1).copied();
        let token_row = token.start.0.max(1) as usize;
        if let Some(pending) = pending_nl {
            if token_row > pending {
                push_nl(&mut out, &lines, pending);
                covered = covered.max(pending);
                pending_nl = None;
            }
        }
        match token.kind {
            PythonTokenKind::End => {
                synth_lines(&mut out, &mut covered, &lines, lines.len());
                out.push(Row {
                    kind: tok::ENDMARKER,
                    text: String::new(),
                    start: (token.start.0 as i64, token.start.1 as i64),
                    end: (token.end.0 as i64, token.end.1 as i64),
                    line: String::new(),
                });
            }
            PythonTokenKind::Indent => {
                let row = token.start.0.max(1) as usize;
                synth_lines(&mut out, &mut covered, &lines, row - 1);
                let text = lines.get(row - 1).copied().unwrap_or("");
                let width = text.chars().take_while(|ch| *ch == ' ').count();
                out.push(Row {
                    kind: tok::INDENT,
                    text: text.chars().take(width).collect(),
                    start: (row as i64, 0),
                    end: (row as i64, width as i64),
                    line: text.to_owned(),
                });
                covered = covered.max(row - 1);
            }
            PythonTokenKind::Dedent => {
                // 行首缩进回退：**先把这一行之前空行／注释行的 `NL` 补上** ✓（参照的 `NL` 在 `DEDENT`
                // 之前 ✓：第 6 行空行的 `NL` 先出、第 7 行的两条 `DEDENT` 后出 ✓）。
                synth_lines(&mut out, &mut covered, &lines, token_row.saturating_sub(1));
                // **位置＝触发行的缩进宽度** ✓、`line` ＝那一行 ✓（参照实测：`DEDENT '' (5,8) (5,8)
                // '        return -x\n'` ✓、两条 `(7,0)` ✓）；文件尾没有对应物理行时给 `(row,0)` ＋空串 ✓。
                let line_text = lines.get(token_row.saturating_sub(1)).copied().unwrap_or("");
                let width = line_text.chars().take_while(|ch| *ch == ' ').count();
                let (column, row_for_line) = if line_text.is_empty() {
                    (0usize, None)
                } else {
                    (width, Some(token_row))
                };
                out.push(Row {
                    kind: tok::DEDENT,
                    text: String::new(),
                    start: (token.start.0 as i64, column as i64),
                    end: (token.end.0 as i64, column as i64),
                    line: row_for_line
                        .map(|row| lines.get(row - 1).copied().unwrap_or("").to_owned())
                        .unwrap_or_default(),
                });
                covered = covered.max(token_row.saturating_sub(1));
            }
            PythonTokenKind::Newline => {
                let row = token.start.0.max(1) as usize;
                synth_lines(&mut out, &mut covered, &lines, row - 1);
                out.push(Row {
                    kind: tok::NEWLINE,
                    text: "\n".to_owned(),
                    start: (row as i64, token.start.1 as i64),
                    end: (row as i64, token.start.1 as i64 + 1),
                    line: lines.get(row - 1).copied().unwrap_or("").to_owned(),
                });
                covered = covered.max(row);
            }
            _ => {
                let (row, column) = token.start;
                let (end_row, end_col) = token.end;
                synth_lines(&mut out, &mut covered, &lines, row.max(1) as usize - 1);
                // **f-string ⇒ 三段拆分** ✓（task-19 第 2 轮 ✓）
                if token.kind == PythonTokenKind::FString {
                    emit_fstring(&mut out, &chars, &lines, token.start, token.end);
                    covered = covered.max(end_row.max(row) as usize);
                    continue;
                }
                let kind = match token.kind {
                    PythonTokenKind::Name => tok::NAME,
                    PythonTokenKind::Number => tok::NUMBER,
                    PythonTokenKind::Str | PythonTokenKind::Bytes => tok::STRING,
                    _ => tok::OP,
                };
                let text = slice_span(&chars, token.start, token.end);
                // **`line` 字段＝这个 token 自己吃掉的物理行** ✓（多行字符串 ⇒ 逐行接起来 ✓ ——
                // 参照实测：`STRING '"""multi\nline"""'` 的 `line` 是
                // `'    text = """multi\n' ＋ 'line"""\n'` ✓，而同行的 `NAME 'text'` 只给**第一**物理行 ✓）。
                let line_text = if end_row > row {
                    lines
                        .get(row as usize - 1..end_row as usize)
                        .map(|slice| slice.concat())
                        .unwrap_or_default()
                } else {
                    lines.get(row as usize - 1).copied().unwrap_or("").to_owned()
                };
                out.push(Row {
                    kind,
                    text,
                    start: (row as i64, column as i64),
                    end: (end_row as i64, end_col as i64),
                    line: line_text,
                });
                covered = covered.max(end_row.max(row) as usize);
                // 行尾注释：**本行最后一段代码之后**才看 ✓（否则会把注释挤到代码前面 ✗）
                let next_is_code_same_row = next.is_some_and(|next| {
                    matches!(
                        next.kind,
                        PythonTokenKind::Name
                            | PythonTokenKind::Number
                            | PythonTokenKind::Str
                            | PythonTokenKind::Bytes
                            | PythonTokenKind::FString
                            | PythonTokenKind::Op
                    ) && next.start.0 == end_row
                });
                if !next_is_code_same_row {
                    if let Some(comment) = trailing_comment(&chars, token.end, &lines) {
                        out.push(comment);
                    }
                    // 下一条**不是**同一行的 `Newline` ⇒ 本行的行尾还没发 ✓（续行 ✓）
                    let newline_follows = next.is_some_and(|next| {
                        next.kind == PythonTokenKind::Newline
                            && next.start.0 == end_row.max(row)
                    });
                    if !newline_follows {
                        pending_nl = Some(end_row.max(row) as usize);
                    }
                }
            }
        }
    }
    Ok(out)
}

/// 逐行读 `readline`（`str` 或 `bytes` ✓）拼成源码 ✓。
fn read_source(instance: &Instance, reader: NonNull<Header>, encoding: &str) -> Result<String, ExecError> {
    let mut text = String::new();
    loop {
        let line = match pyawa_core::executor::call::call_value(instance, reader, &[], &[]) {
            Ok(value) => value,
            Err(ExecError::Raised { exception }) => {
                let raised = instance.type_of(exception);
                let stop = instance.type_named("StopIteration");
                if stop.is_some_and(|base| raised == base || instance.is_subtype(raised, base)) {
                    break;
                }
                return Err(ExecError::Raised { exception });
            }
            Err(other) => return Err(other),
        };
        if let Some(raw) = instance.bytes_value(line) {
            if raw.is_empty() {
                break;
            }
            let decoded = instance.decode_with_encoding(raw, encoding)?;
            if let Some(piece) = instance.text_of(decoded) {
                text.push_str(piece);
            }
        } else if let Some(piece) = instance.text_of(line) {
            if piece.is_empty() {
                break;
            }
            text.push_str(piece);
        } else {
            return Err(instance.raise_builtin_error("TypeError", "readline 要回 str 或 bytes"));
        }
    }
    Ok(text)
}

/// 把 token 行装成 `list[tuple]` ✓（**五元组** ✓：`(type, string, start, end, line)` ✓）。
fn rows_to_list(instance: &Instance, rows: Vec<Row>) -> NonNull<Header> {
    let mut items: Vec<NonNull<Header>> = Vec::with_capacity(rows.len());
    for row in rows {
        let kind = instance.new_int(row.kind);
        let text = instance.new_str(&row.text);
        let start = instance.new_tuple(vec![instance.new_int(row.start.0), instance.new_int(row.start.1)]);
        let end = instance.new_tuple(vec![instance.new_int(row.end.0), instance.new_int(row.end.1)]);
        let line = instance.new_str(&row.line);
        items.push(instance.new_tuple(vec![kind, text, start, end, line]));
    }
    instance.new_list(items)
}

/// 取 `self` ✓：**绑定形态**（`bound` ✓）与**位置形态**（`args[0]` ✓）都要认 ——
/// 类调用协议在两条路上都会出现 ✓。回 `(self, 其余实参)` ✓。
fn split_self<'a>(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> Result<(NonNull<Header>, &'a [NonNull<Header>]), ExecError> {
    match bound {
        Some(this) => Ok((this, args)),
        None => match args.split_first() {
            Some((this, rest)) => Ok((*this, rest)),
            None => Err(instance.raise_builtin_error("TypeError", "TokenizerIter 缺少 self")),
        },
    }
}

/// **`TokenizerIter(source, encoding=None, extra_tokens=False)`** ✓：`source` 是**readline 可调用** ✓
/// （`generate_tokens` 给 `str` 行 ✓、`tokenize()` 给 `bytes` 行 ＋ `encoding` ✓）。
fn tokenizer_iter_init(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = split_self(instance, bound, args)?;
    let Some(reader) = rest.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "TokenizerIter 要一个 readline 可调用"));
    };
    let encoding = kwargs
        .iter()
        .find(|(key, _)| instance.text_of(*key) == Some("encoding"))
        .and_then(|(_, value)| instance.text_of(*value))
        .unwrap_or("utf-8")
        .to_owned();
    let source = read_source(instance, reader, &encoding)?;
    let rows = tokenize_source(&source)
        .map_err(|error| instance.raise_builtin_error("SyntaxError", &error))?;
    // **状态写在 `type.__call__` 造出来的那个实例上** ✓（`__init__` 不能再造一个 ✗ ——
    // 那样 `__next__` 拿到的还是原来那个空实例 ✓，实测就报"没有 token 表" ✗）。
    let tokens = rows_to_list(instance, rows);
    instance.set_attribute_value(this, "_tokens", tokens)?;
    let zero = instance.new_int(0);
    instance.set_attribute_value(this, "_index", zero)?;
    Ok(instance.retain(this))
}

/// `TokenizerIter.__iter__` ✓（迭代器就是自己 ✓）。
fn tokenizer_iter_self(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = split_self(instance, bound, args)?;
    Ok(instance.retain(this))
}

/// `TokenizerIter.__next__` ✓：给下一个五元组 ✓，完事按参照报 `StopIteration` ✓。
fn tokenizer_next(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = split_self(instance, bound, args)?;
    let index = instance
        .attribute_optional_of(this, "_index")?
        .and_then(|value| instance.int_value(value))
        .unwrap_or(0);
    let tokens = instance
        .attribute_optional_of(this, "_tokens")?
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "TokenizerIter 没有 token 表"))?;
    let items = instance
        .list_items(tokens)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "TokenizerIter 的 token 表不是 list"))?;
    let Some(item) = items.get(index as usize).copied() else {
        return Err(instance.raise_builtin_error("StopIteration", ""));
    };
    let next = instance.new_int(index + 1);
    instance.set_attribute_value(this, "_index", next)?;
    Ok(instance.retain(item))
}

/// 建 `_tokenize` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let base = instance.type_named("object").expect("object 已登记");
    let class_namespace = instance.new_dict();
    for (name, handler) in [
        ("__init__", tokenizer_iter_init as NativeFn),
        ("__iter__", tokenizer_iter_self as NativeFn),
        ("__next__", tokenizer_next as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    if let Ok(class) =
        pyawa_core::build_class_from_parts(instance, ITER_CLASS.to_owned(), vec![base.cast()], class_namespace, None)
    {
        instance.dict_set(namespace, "TokenizerIter", class);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
