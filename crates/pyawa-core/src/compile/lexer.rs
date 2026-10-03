//! `compile` 的子模块（拆分自单文件时期，见 `AGENTS.md`）。

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Lexeme {
    Name(String),
    Int(i64),
    Str(String),
    /// **f-string 的原文**（`f'…'`／`rf'…'`；花括号留给 `parse_fstring` 切片）。
    /// `offset` 是**内容**在源码里的起始列（插值里的表达式要按它平移跨度）；
    /// `contents` 是**原文**（转义**不解码**——第 251 轮起由切段时按源下标解码 ⇒ 位点保留源偏移），
    /// `raw` 表示原始字符串（`r`／`rf` 前缀 ⇒ 反斜杠不解码）。
    FStr {
        contents: String,
        offset: u32,
        raw: bool,
    },
    /// **`bytes` 字面量**（`P1-12`／`b'…'`）：转义在**词法**这一层就解成字节。
    Bytes(Vec<u8>),
    Assign,
    /// `+=` 一族（增强赋值）。
    AugAssign(AugOperator),
    /// `@`（矩阵乘；与 `@=` 分开）。
    At,
    Plus,
    /// `.`（属性访问）
    Dot,
    Colon,
    /// `:=`（**海象**／赋值表达式；与 `:` 分开 ✓）
    Walrus,
    LeftParen,
    RightParen,
    Comma,
    Newline,
    Indent,
    Dedent,
    Return,
    Def,
    /// `class`
    Class,
    /// `nonlocal`
    Nonlocal,
    /// `global`
    Global,
    /// `yield`
    Yield,
    /// `raise`
    Raise,
    If,
    Else,
    While,
    For,
    In,
    Star,
    DoubleStar,
    Arrow,
    LeftBracket,
    LeftBrace,
    RightBracket,
    RightBrace,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    EqualEqual,
    NotEqual,
    End,
    /// `/`（形参表里的仅位置分隔符；除法未接线）
    Slash,
    /// `-`（减号／负号；`->` 是单独的 `Arrow`）
    Minus,
    /// `//`（整除）
    DoubleSlash,
    /// `%`
    Percent,
    /// `&`
    Ampersand,
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `~`
    Tilde,
    /// `<<`
    LeftShift,
    /// `>>`
    RightShift,
}

/// 词法结果：单元 ＋ 与它**一一对应**的跨度。
pub(super) struct Lexed {
    pub(super) lexemes: Vec<Lexeme>,
    pub(super) spans: Vec<Span>,
}

/// 解一个 `bytes` 字面量里的转义（`characters` 从**反斜杠**那一位开始）。
///
/// 回 `(字节, 吃掉几个字符)`。支持集＝参照实测里出现过的那批：`\n \t \r \\ \' \" \a \b \f \v`、
/// `\xNN`（**两位**十六进制）、`\ooo`（一至三位八进制）。其余如实报**未实现**——`\u`／`\U`／
/// `\N{}` 在 bytes 里的口径没实测过，不猜。
/// 解码**一处**字符串转义（`position` 指在反斜杠上）⇒ `(替换文本, 消费的字符数)`。
///
/// 行继续返回空串（消费掉反斜杠与换行）。`\\N{…}` 具名转义要 Unicode 名字表 ⇒ 如实报未实现；
/// 认不出的转义按 CPython 原样留下（反斜杠 ＋ 那个字符）。
pub(super) fn lex_string_escape(characters: &[char], position: usize) -> Result<(String, usize), CompileError> {
    let Some(next) = characters.get(position + 1).copied() else {
        return Err(CompileError::Syntax("反斜杠后面没有字符".to_owned()));
    };
    Ok(match next {
        'n' => ("\n".to_owned(), 2),
        't' => ("\t".to_owned(), 2),
        'r' => ("\r".to_owned(), 2),
        '\\' => ("\\".to_owned(), 2),
        '\'' => ("\'".to_owned(), 2),
        '"' => ("\"".to_owned(), 2),
        'a' => ("\u{7}".to_owned(), 2),
        'b' => ("\u{8}".to_owned(), 2),
        'f' => ("\u{c}".to_owned(), 2),
        'v' => ("\u{b}".to_owned(), 2),
        '\n' => (String::new(), 2),
        '\r' => (
            String::new(),
            if characters.get(position + 2) == Some(&'\n') { 3 } else { 2 },
        ),
        'x' => (lex_hex_escape(characters, position + 2, 2)?.to_string(), 4),
        'u' => (lex_hex_escape(characters, position + 2, 4)?.to_string(), 6),
        'U' => (lex_hex_escape(characters, position + 2, 8)?.to_string(), 10),
        digit @ '0'..='7' => {
            let (value, consumed) = lex_octal_escape(characters, position + 1, digit)?;
            (value.to_string(), 1 + consumed)
        }
        'N' => {
            return Err(CompileError::Unsupported(
                "`\\N{…}` 具名转义要整张 Unicode 名字表（与 M3 的 `Lib/`／数据面绑定）".to_owned(),
            ))
        }
        other => (format!("\\{other}"), 2),
    })
}

/// 字符串转义的十六进制段（`\x` 2 位、`\u` 4 位、`\U` 8 位）⇒ 一个码点。
pub(super) fn lex_hex_escape(
    characters: &[char],
    position: usize,
    digits: usize,
) -> Result<char, CompileError> {
    let mut value: u32 = 0;
    for offset in 0..digits {
        let Some(character) = characters.get(position + offset) else {
            return Err(CompileError::Syntax("十六进制转义不完整".to_owned()));
        };
        let Some(digit) = character.to_digit(16) else {
            return Err(CompileError::Syntax(format!(
                "(value error) invalid \\x escape at position {position}"
            )));
        };
        value = value * 16 + digit;
    }
    char::from_u32(value)
        .ok_or_else(|| CompileError::Syntax(format!("invalid unicode escape \\U{value:08x}")))
}

/// 字符串转义的八进制段（最多 3 位、`0o400` 以上越界）。
pub(super) fn lex_octal_escape(
    characters: &[char],
    position: usize,
    first: char,
) -> Result<(char, usize), CompileError> {
    let mut value = first.to_digit(8).expect("首字符已判过");
    let mut consumed = 1;
    while consumed < 3 {
        match characters.get(position + consumed).and_then(|item| item.to_digit(8)) {
            Some(digit) => {
                value = value * 8 + digit;
                consumed += 1;
            }
            None => break,
        }
    }
    if value > 0xFF {
        return Err(CompileError::Syntax("octal escape out of range".to_owned()));
    }
    char::from_u32(value)
        .map(|character| (character, consumed))
        .ok_or_else(|| CompileError::Syntax("invalid octal escape".to_owned()))
}

pub(super) fn lex_bytes_escape(characters: &[char], position: usize) -> Result<(u8, usize), CompileError> {
    let simple = |byte: u8| Ok((byte, 2));
    match characters.get(1) {
        Some('n') => simple(b'\n'),
        Some('t') => simple(b'\t'),
        Some('r') => simple(b'\r'),
        Some('\\') => simple(b'\\'),
        Some('\'') => simple(b'\''),
        Some('"') => simple(b'"'),
        Some('a') => simple(0x07),
        Some('b') => simple(0x08),
        Some('f') => simple(0x0c),
        Some('v') => simple(0x0b),
        Some('x') => {
            let digits: String = characters
                .iter()
                .skip(2)
                .take(2)
                .take_while(|character| character.is_ascii_hexdigit())
                .collect();
            if digits.len() != 2 {
                // 实测：`b'\x1'` ⇒ `(value error) invalid \x escape at position 0`
                // （位置是反斜杠相对**字面量内容**起点的下标；`b'a\x1'` ⇒ 1）
                return Err(CompileError::Syntax(format!(
                    "(value error) invalid \\x escape at position {position}"
                )));
            }
            let byte = u8::from_str_radix(&digits, 16)
                .map_err(|_| CompileError::Syntax("invalid \\x escape".to_owned()))?;
            Ok((byte, 4))
        }
        Some(first) if first.is_digit(8) => {
            let digits: String = characters
                .iter()
                .skip(1)
                .take(3)
                .take_while(|character| character.is_digit(8))
                .collect();
            let code = u32::from_str_radix(&digits, 8)
                .map_err(|_| CompileError::Syntax("invalid octal escape".to_owned()))?;
            if code > 0xFF {
                return Err(CompileError::Syntax("octal escape out of range".to_owned()));
            }
            Ok((code as u8, 1 + digits.chars().count()))
        }
        Some(other) => Err(CompileError::Unsupported(format!(
            "bytes 字面量里的转义 \\{other} 尚未接线"
        ))),
        None => Err(CompileError::Syntax(
            "unterminated string literal (detected at line 1)".to_owned(),
        )),
    }
}

pub(super) fn lex(source: &str) -> Result<Lexed, CompileError> {
    let characters: Vec<char> = source.chars().collect();
    let mut index = 0usize;
    let mut lexemes = Vec::new();
    let mut spans = Vec::new();
    let mut indents: Vec<usize> = vec![0];
    let mut at_line_start = true;
    let mut line: u32 = 1;
    let mut line_start_index = 0usize;
    // 注意：这里**不能**用闭包借 `line_start_index`（后面要改它）⇒ 写成宏式的内联表达式
    macro_rules! column {
        ($index:expr) => {
            ($index - line_start_index) as u32
        };
    }
    // **括号深度**（`(...)`／`[...]`／`{...}`）：> 0 时按 CPython 的 `NL` 处理 —— **不发**
    // `Newline`、也不做缩进块判定 ✓（隐式续行）。第 101 轮实测：不跟踪它，上游
    // `importlib/_bootstrap.py`／`_bootstrap_external.py` 这类多行调用会被当成新逻辑行
    // ⇒ 报「缩进对不齐」✗。深度只在**字符串/注释之外**变（它们各自被整段吃掉 ✓）。
    let mut depth: usize = 0;
    while index < characters.len() {
        if at_line_start {
            let mut width = 0usize;
            while matches!(characters.get(index), Some(' ')) {
                width += 1;
                index += 1;
            }
            if matches!(characters.get(index), Some('\n') | None) {
                if let Some('\n') = characters.get(index) {
                    index += 1;
                    line += 1;
                    line_start_index = index;
                }
                continue;
            }
            let current = *indents.last().expect("至少有一个");
            let zero = Span::new(line, line, 0, 0);
            let _ = &zero;
            if depth > 0 {
                // 隐式续行：行首空白照跳，**不**判缩进块 ✓
                at_line_start = false;
                let character = *characters.get(index).expect("上面已确认不是文件尾");
                if character == '\n' {
                    index += 1;
                    line += 1;
                    line_start_index = index;
                    at_line_start = true;
                }
                continue;
            }
            if width > current {
                indents.push(width);
                lexemes.push(Lexeme::Indent);
                spans.push(zero);
            } else if width < current {
                while *indents.last().expect("至少有一个") > width {
                    indents.pop();
                    lexemes.push(Lexeme::Dedent);
                    spans.push(zero);
                }
                if *indents.last().expect("至少有一个") != width {
                    return Err(CompileError::Syntax(format!(
                        "缩进对不齐：宽度 {width}（第 {line} 行，列 {}）",
                        column!(index)
                    )));
                }
            }
            at_line_start = false;
        }
        let character = characters[index];
        // `:=`（海象）：两字符，必须**先于** `:` 的单字符臂看 ✓（第 105 轮；`_bootstrap.py:119` 就是它 ✗）
        if character == ':' && characters.get(index + 1) == Some(&'=') {
            lexemes.push(Lexeme::Walrus);
            spans.push(Span::new(line, line, column!(index), column!(index + 2)));
            index += 2;
            continue;
        }
        // 深度：字符串／注释各自整段消费 ⇒ 这里看到的括号一定在**代码位置** ✓
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        match character {
            ' ' | '\r' => index += 1,
            '\t' => return Err(CompileError::Unsupported("制表符缩进尚未接线".to_owned())),
            '\n' => {
                if depth > 0 {
                    // 隐式续行里的物理换行不进词法流（CPython 的 `NL` ✓）
                    index += 1;
                    line += 1;
                    line_start_index = index;
                    at_line_start = true;
                    continue;
                }
                lexemes.push(Lexeme::Newline);
                spans.push(Span::new(line, line, column!(index), column!(index)));
                index += 1;
                line += 1;
                line_start_index = index;
                at_line_start = true;
            }
            '<' | '>' | '=' | '!' => {
                // 两字符比较要从**当前**位置看下一个字符
                let next = characters.get(index + 1).copied();
                let (lexeme, width) = match (character, next) {
                    ('<', Some('=')) => (Lexeme::LessEqual, 2),
                    ('>', Some('=')) => (Lexeme::GreaterEqual, 2),
                    // 位移与其增强赋值：**必须先于**单字符 `<`／`>` 匹配
                    ('<', Some('<')) if characters.get(index + 2) == Some(&'=') => {
                        (Lexeme::AugAssign(AugOperator::LeftShift), 3)
                    }
                    ('>', Some('>')) if characters.get(index + 2) == Some(&'=') => {
                        (Lexeme::AugAssign(AugOperator::RightShift), 3)
                    }
                    ('<', Some('<')) => (Lexeme::LeftShift, 2),
                    ('>', Some('>')) => (Lexeme::RightShift, 2),
                    ('=', Some('=')) => (Lexeme::EqualEqual, 2),
                    ('!', Some('=')) => (Lexeme::NotEqual, 2),
                    ('<', _) => (Lexeme::Less, 1),
                    ('>', _) => (Lexeme::Greater, 1),
                    ('=', _) => (Lexeme::Assign, 1),
                    _ => return Err(CompileError::Syntax("孤立的 `!`".to_owned())),
                };
                let start = column!(index);
                lexemes.push(lexeme);
                spans.push(Span::new(line, line, start, start + width as u32));
                index += width;
            }
            '/' => {
                // `/`：仅位置形参分隔符 ＋ 真除法；`//` 整除；`/=`／`//=` 增强赋值
                let start = column!(index);
                let second = characters.get(index + 1).copied();
                let (lexeme, width) = match second {
                    Some('/') if characters.get(index + 2) == Some(&'=') => {
                        (Lexeme::AugAssign(AugOperator::FloorDivide), 3)
                    }
                    Some('=') => (Lexeme::AugAssign(AugOperator::TrueDivide), 2),
                    Some('/') => (Lexeme::DoubleSlash, 2),
                    _ => (Lexeme::Slash, 1),
                };
                lexemes.push(lexeme);
                spans.push(Span::new(line, line, start, start + width as u32));
                index += width;
            }
            '-' => {
                // `->`（返回注解）／`-=`（增强赋值）／`-`（减号与负号）
                let start = column!(index);
                let (lexeme, width) = match characters.get(index + 1).copied() {
                    Some('>') => (Lexeme::Arrow, 2),
                    Some('=') => (Lexeme::AugAssign(AugOperator::Subtract), 2),
                    _ => (Lexeme::Minus, 1),
                };
                lexemes.push(lexeme);
                spans.push(Span::new(line, line, start, start + width as u32));
                index += width;
            }
            '{' => {
                let start = column!(index);
                lexemes.push(Lexeme::LeftBrace);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '}' => {
                let start = column!(index);
                lexemes.push(Lexeme::RightBrace);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '[' => {
                let start = column!(index);
                lexemes.push(Lexeme::LeftBracket);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            ']' => {
                let start = column!(index);
                lexemes.push(Lexeme::RightBracket);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '*' => {
                let start = column!(index);
                let second = characters.get(index + 1).copied();
                let (lexeme, width) = match second {
                    // `**=`（先于 `**` 判）
                    Some('*') if characters.get(index + 2) == Some(&'=') => {
                        (Lexeme::AugAssign(AugOperator::Power), 3)
                    }
                    Some('*') => (Lexeme::DoubleStar, 2),
                    Some('=') => (Lexeme::AugAssign(AugOperator::Multiply), 2),
                    _ => (Lexeme::Star, 1),
                };
                lexemes.push(lexeme);
                spans.push(Span::new(line, line, start, start + width as u32));
                index += width;
            }
            '.' | '+' | ':' | '(' | ')' | ',' | ';' | '%' | '&' | '|' | '^' | '~' | '@' => {
                let start = column!(index);
                // 增强赋值：`+=`／`-=`／`%=`／`&=`／`|=`／`^=`／`@=`（`==` 已在上面分流）
                let augmented = characters.get(index + 1) == Some(&'=');
                let operator = match (character, augmented) {
                    ('+', true) => Some(AugOperator::Add),
                    ('%', true) => Some(AugOperator::Remainder),
                    ('&', true) => Some(AugOperator::BitAnd),
                    ('|', true) => Some(AugOperator::BitOr),
                    ('^', true) => Some(AugOperator::BitXor),
                    ('@', true) => Some(AugOperator::MatrixMultiply),
                    _ => None,
                };
                if let Some(operator) = operator {
                    lexemes.push(Lexeme::AugAssign(operator));
                    spans.push(Span::new(line, line, start, start + 2));
                    index += 2;
                    continue;
                }
                lexemes.push(match character {
                    '=' => Lexeme::Assign,
                    '.' => Lexeme::Dot,
                    '+' => Lexeme::Plus,
                    ':' => Lexeme::Colon,
                    '(' => Lexeme::LeftParen,
                    ')' => Lexeme::RightParen,
                    ',' => Lexeme::Comma,
                    '%' => Lexeme::Percent,
                    '&' => Lexeme::Ampersand,
                    '|' => Lexeme::Pipe,
                    '^' => Lexeme::Caret,
                    '~' => Lexeme::Tilde,
                    '@' => Lexeme::At,
                    _ => Lexeme::Newline,
                });
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '#' => {
                while !matches!(characters.get(index), Some('\n') | None) {
                    index += 1;
                }
            }
            // 单双引号**等价**（实测的语料里两种都有；转义仍未接线）
            '\'' | '"' => {
                let quote = characters[index];
                let start = column!(index);
                let start_line = line;
                // **三引号**（`'''`／`"""`）：收尾要连着三个同种引号
                let triple = characters.get(index + 1) == Some(&quote)
                    && characters.get(index + 2) == Some(&quote);
                index += if triple { 3 } else { 1 };
                let mut text = String::new();
                loop {
                    match characters.get(index) {
                        Some(character)
                            if *character == quote
                                && (!triple
                                    || (characters.get(index + 1) == Some(&quote)
                                        && characters.get(index + 2) == Some(&quote))) =>
                        {
                            index += if triple { 3 } else { 1 };
                            break;
                        }
                        // **字符串转义**（解码逻辑在 `lex_string_escape`，一处真相）
                        Some('\\') => {
                            let (decoded, consumed) = lex_string_escape(&characters, index)?;
                            text.push_str(&decoded);
                            // 消费掉的换行（行继续／`\r\n`）⇒ 行号与行首索引要跟上
                            for offset in 0..consumed {
                                if characters.get(index + offset) == Some(&'\n') {
                                    line += 1;
                                    line_start_index = index + offset + 1;
                                }
                            }
                            index += consumed;
                        }
                        Some(character) => {
                            // 字面量里的真换行（三引号／行继续后的续行）⇒ 行号与**行首索引**都要跟上
                            if *character == '\n' {
                                line += 1;
                                line_start_index = index + 1;
                            }
                            text.push(*character);
                            index += 1;
                        }
                        None => {
                            return Err(CompileError::Syntax("字符串没有收尾引号".to_owned()))
                        }
                    }
                }
                lexemes.push(Lexeme::Str(text));
                // 跨度**跨行**（实测 `x = "a\<换行>b"` 的常量位点是 `(1, 2, …)`）
                spans.push(Span::new(start_line, line, start, column!(index)));
            }
            character if character.is_ascii_digit() => {
                let start = column!(index);
                // **进制前缀与下划线**（第 126 轮实测）：`0xFF`／`0o17`／`0b1010` 与 `1_000` ✓
                //（参照里它们与十进制**同形** ⇒ 都是 `LOAD_SMALL_INT`／`LOAD_CONST`，位点取字面量自身 ✓）
                let (radix, digits_start) = match (characters.get(index), characters.get(index + 1)) {
                    (Some('0'), Some('x' | 'X')) => (16u32, index + 2),
                    (Some('0'), Some('o' | 'O')) => (8, index + 2),
                    (Some('0'), Some('b' | 'B')) => (2, index + 2),
                    _ => (10, index),
                };
                let mut end = digits_start;
                while end < characters.len()
                    && (characters[end].is_ascii_alphanumeric() || characters[end] == '_')
                {
                    end += 1;
                }
                let digits: String = characters[digits_start..end]
                    .iter()
                    .filter(|item| **item != '_')
                    .collect();
                let value = i64::from_str_radix(&digits, radix).map_err(|_| {
                    CompileError::Unsupported(format!("整数 {digits}（进制 {radix}）超出本层范围"))
                })?;
                index = end;
                lexemes.push(Lexeme::Int(value));
                spans.push(Span::new(line, line, start, column!(index)));
            }
            character if character.is_alphabetic() || character == '_' => {
                // **`f`／`r` 前缀**（第 238 轮）：`f'…'`／`rf'…'`／`fr'…'` 收成 `FStr`（原文），
                // 裸 `r'…'` 与普通字符串同形（本层不处理转义）。必须**先于**标识符分支判断。
                if matches!(character, 'f' | 'F' | 'r' | 'R') {
                    let single = matches!(characters.get(index + 1), Some('\'') | Some('"'));
                    let doubled = matches!(
                        (character, characters.get(index + 1), characters.get(index + 2)),
                        (
                            'f' | 'F' | 'r' | 'R',
                            Some('r' | 'R' | 'f' | 'F'),
                            Some('\'') | Some('"')
                        )
                    );
                    if single || doubled {
                        let start = column!(index);
                        let prefix_start_index = index;
                        let quote_index = if single { index + 1 } else { index + 2 };
                        let quote = characters[quote_index];
                        // **三引号**（`f"""…"""`／`rf'''…'''`）同样要认
                        let triple = characters.get(quote_index + 1) == Some(&quote)
                            && characters.get(quote_index + 2) == Some(&quote);
                        let start_line = line;
                        index = quote_index + if triple { 3 } else { 1 };
                        let prefix_has_f = matches!(character, 'f' | 'F')
                            || matches!(characters.get(index - 2), Some('f') | Some('F'));
                        // `r` 前缀（含 `rf`／`fr`）⇒ **原始字符串**：反斜杠原样留下
                        let prefix_has_r = matches!(character, 'r' | 'R')
                            || matches!(characters.get(index - 2), Some('r') | Some('R'))
                            || matches!(characters.get(index - 3), Some('r') | Some('R'));
                        let mut contents = String::new();
                        loop {
                            match characters.get(index) {
                                Some(current)
                                    if *current == quote
                                        && (!triple
                                            || (characters.get(index + 1) == Some(&quote)
                                                && characters.get(index + 2) == Some(&quote))) =>
                                {
                                    index += if triple { 3 } else { 1 };
                                    break;
                                }
                                // **反斜杠与其后一个字符原样进正文**（`r` 串不解码；非 `r` 串留给
                                // 切段时解码 ⇒ 字面段的位点天然按**源偏移**算）
                                Some('\\') => {
                                    contents.push('\\');
                                    index += 1;
                                    if let Some(current) = characters.get(index) {
                                        if *current == '\n' {
                                            line += 1;
                                            line_start_index = index + 1;
                                        }
                                        contents.push(*current);
                                        index += 1;
                                    }
                                }
                                Some(current) => {
                                    // 字面量里的真换行（三引号跨行）⇒ 行号与行首索引都要跟上
                                    if *current == '\n' {
                                        line += 1;
                                        line_start_index = index + 1;
                                    }
                                    contents.push(*current);
                                    index += 1;
                                }
                                None => {
                                    return Err(CompileError::Syntax(
                                        "字符串没有收尾引号".to_owned(),
                                    ))
                                }
                            }
                        }
                        // 跨度**跨行**（与普通字符串同一条规则）
                        let span = Span::new(start_line, line, start, column!(index));
                        if prefix_has_f {
                            // 三引号时正文从**第三个引号之后**开始（先算好再传：宏展开不带括号）
                            let content_index = quote_index + if triple { 3 } else { 1 };
                            lexemes.push(Lexeme::FStr {
                                contents,
                                // 正文列 ＝ **前缀所在列** ＋ 词内偏移（跨行后不能再用"当前行首"）
                                offset: start + (content_index - prefix_start_index) as u32,
                                raw: prefix_has_r,
                            });
                        } else {
                            lexemes.push(Lexeme::Str(contents));
                        }
                        spans.push(span);
                        continue;
                    }
                }
                // `b'…'`／`B"…"`：**先**看前缀（否则会先被当成名字 `b`）。
                // 转义在这一层就解成**字节**；非 ASCII 字符照参照报 `SyntaxError`。
                if matches!(character, 'b' | 'B')
                    && matches!(characters.get(index + 1), Some('\'') | Some('"'))
                {
                    let start = column!(index);
                    let quote = characters[index + 1];
                    index += 2;
                    // 转义报错里的 `position` 是**相对字面量内容**的下标（实测口径）
                    let content_start = index;
                    let mut value: Vec<u8> = Vec::new();
                    loop {
                        match characters.get(index) {
                            Some(current) if *current == quote => {
                                index += 1;
                                break;
                            }
                            Some('\\') => {
                                let (byte, width) =
                                    lex_bytes_escape(&characters[index..], index - content_start)?;
                                value.push(byte);
                                index += width;
                            }
                            Some(current) if current.is_ascii() => {
                                value.push(*current as u8);
                                index += 1;
                            }
                            Some(_) => {
                                return Err(CompileError::Syntax(
                                    "bytes can only contain ASCII literal characters".to_owned(),
                                ))
                            }
                            None => {
                                return Err(CompileError::Syntax(
                                    "unterminated string literal (detected at line 1)".to_owned(),
                                ))
                            }
                        }
                    }
                    lexemes.push(Lexeme::Bytes(value));
                    spans.push(Span::new(line, line, start, column!(index)));
                    continue;
                }
                let start = column!(index);
                let start_index = index;
                while index < characters.len()
                    && (characters[index].is_alphanumeric() || characters[index] == '_')
                {
                    index += 1;
                }
                let text: String = characters[start_index..index].iter().collect();
                let end = column!(index);
                lexemes.push(match text.as_str() {
                    "return" => Lexeme::Return,
                    "raise" => Lexeme::Raise,
                    "def" => Lexeme::Def,
                    "class" => Lexeme::Class,
                    "nonlocal" => Lexeme::Nonlocal,
                    "global" => Lexeme::Global,
                    "yield" => Lexeme::Yield,
                    "if" => Lexeme::If,
                    "while" => Lexeme::While,
                    "for" => Lexeme::For,
                    "in" => Lexeme::In,
                    "else" => Lexeme::Else,
                    _ => Lexeme::Name(text),
                });
                spans.push(Span::new(line, line, start, end));
            }
            other => return Err(CompileError::Syntax(format!("不认识的字符 {other:?}"))),
        }
    }
    let end_span = Span::new(line, line, column!(index), column!(index));
    while indents.len() > 1 {
        indents.pop();
        lexemes.push(Lexeme::Dedent);
        spans.push(end_span);
    }
    lexemes.push(Lexeme::End);
    spans.push(end_span);
    Ok(Lexed { lexemes, spans })
}

// ---- 语法 ----

