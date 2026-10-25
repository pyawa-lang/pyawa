//! `compile` 的子模块（拆分自单文件时期，见 `AGENTS.md`）。

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Lexeme {
    Name(String),
    Int(i64),
    /// **装不进 `i64` 的整数字面量**（第 285 轮，`P1-11` 的剩余面）：存**十进制**串 ✓
    /// （常量池那一格也是十进制串 ✓ ⇒ 词法期只做一次进制换算 ✓）。
    BigInt(String),
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
    /// **浮点字面量**（第 127 轮）：值是 **IEEE-754 位模式** ✓
    Float(u64),
    /// **虚部字面量**（task-29 ✓）：`1j`／`1.5j`／`1e3j` —— 值＝**虚部**的 IEEE-754 位模式
    /// （实部恒为 `0.0`；`Constant::Complex` 在编译期补实部 ✓）。
    Imaginary(u64),
    /// `:=`（**海象**／赋值表达式；与 `:` 分开 ✓）
    Walrus,
    LeftParen,
    RightParen,
    Comma,
    /// **`;`**（简单语句分隔符，第 405 轮）：与 `Newline` **分开** ✓ —— 行内 `if` 的
    /// 分号语句表后面还能跟**次行的 `else:`** ✓；先前一律折成 `Newline` ✗ ⇒ 行内体在
    /// 第一个 `;` 处就"收尾" ✗ ⇒ 下一个分句被当成新语句、随后的 `else` 成孤儿 ✗。
    Semicolon,
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
    /// **`?`**（`TS-46` 的可空后缀；**只在扩展模式**下才会产生 —— 纯 Python 模式仍在下面
    /// 的"不认识的字符"臂报 `Syntax` ⇒ `BC-15` 直接成立 ✓）。
    Question,
}

/// 词法结果：单元 ＋ 与它**一一对应**的跨度。
pub(super) struct Lexed {
    pub(super) lexemes: Vec<Lexeme>,
    pub(super) spans: Vec<Span>,
    /// **编译模式**（`BC-14` 的显式入参，词法期就带进来）：决定扩展词素（`?`）认不认；
    /// 语法层要判"扩展语法"时直接读它（免得给整张 parser 加参数）。
    pub(super) mode: Mode,
    /// **括号内的物理换行**（`BC-62` 的多语句 `lambda` body 用）：值是"换行发生在第几个词素
    /// **之前**"（即 `line_breaks` 里出现 `i` ⇒ 词素 `i-1` 与 `i` 之间有一个物理换行）。
    ///
    /// 为什么必须有它 ✗：括号内是**隐式续行**，参照（与本次词法器）**不发** `Newline` 词素
    /// （`(...)` 里的多行表达式不能当语句边界 ✗）。但 `BC-62` 的 `( 语句… 末表达式 )` 要
    /// **用换行分隔语句** ⇒ 只有把"哪儿有物理换行"记下来，`lambda` 体那一处才能区分
    /// "多行表达式"（`(a +\n b)`）与"多条语句"（`(x = 1\n x)`）。
    ///
    /// **只在括号深度 > 0 时记**（深度 0 的换行本来就是 `Newline` 词素 ✓）；纯 Python 模式
    /// 也照记，但只有扩展模式的 `lambda` 体读它 ⇒ `.py` 的解析与产物**逐字节不变**（`TS-5`）。
    pub(super) line_breaks: Vec<usize>,
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

/// **标识符继续字符**（PEP 3131 的 XID_Continue 近似，task-45 ✓）：字母数字／下划线之外，
/// 还包括**组合记号**与**变体选择符**那批（官方 `test_unicode_identifiers.py:10` 的
/// `x` 后跟 U+E0100 正是变体选择符 —— Rust 的 `char::is_alphanumeric` 不含它 ✗ ⇒ 先前
/// 在词法期报"不认识的字符" ✓）。这里按**常用区间**收：覆盖组合用记号（Mn/Mc/Me 的主要块）
/// 与变体选择符；XID_Start 仍用 `is_alphabetic`（字母面足够）。
fn is_identifier_continue(character: char) -> bool {
    if character == '_' || character.is_alphanumeric() {
        return true;
    }
    matches!(
        character as u32,
        0x0300..=0x036F      // Combining Diacritical Marks
            | 0x0483..=0x0489 // Cyrillic combining
            | 0x0591..=0x05BD | 0x05BF | 0x05C1..=0x05C2 | 0x05C4..=0x05C5 | 0x05C7
            | 0x0610..=0x061A | 0x064B..=0x065F | 0x0670
            | 0x06D6..=0x06DC | 0x06DF..=0x06E4 | 0x06E7..=0x06E8 | 0x06EA..=0x06ED
            | 0x0900..=0x0903 | 0x093A..=0x093C | 0x093E..=0x094F | 0x0951..=0x0957
            | 0x0962..=0x0963 | 0x0981..=0x0983 | 0x09BC | 0x09BE..=0x09CD
            | 0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E
            | 0x1AB0..=0x1AFF // Combining Diacritical Marks Extended
            | 0x1DC0..=0x1DFF // Combining Diacritical Marks Supplement
            | 0x20D0..=0x20F0 // Combining Diacritical Marks for Symbols
            | 0x302A..=0x302F | 0x3099..=0x309A
            | 0xFE00..=0xFE0F // Variation Selectors
            | 0xFE20..=0xFE2F // Combining Half Marks
            | 0xE0100..=0xE01EF // Variation Selectors Supplement
    )
}

pub(super) fn lex(source: &str, mode: Mode) -> Result<Lexed, CompileError> {
    let characters: Vec<char> = source.chars().collect();
    let mut index = 0usize;
    let mut lexemes = Vec::new();
    let mut spans = Vec::new();
    // **括号内的物理换行**（`BC-62`；见 [`Lexed::line_breaks`]）：记录"换行发生在第几个词素之前"。
    let mut line_breaks: Vec<usize> = Vec::new();
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
            // **缩进宽度**：空格 ＋ 制表符（tabstop＝8，与参照的词法器同口径 ✓）。
            // **制表符缩进的代码行**按归一后的宽度判块 ✓（`\t` 与 8 空格等价 ✓）；
            // **列号仍是字符下标** ✓（参照的 `co_positions` 把 `\t` 算 1 列 ✓，实测）。
            let mut width = 0usize;
            while let Some(character) = characters.get(index) {
                match character {
                    ' ' => {
                        width += 1;
                        index += 1;
                    }
                    '\t' => {
                        width = (width / 8 + 1) * 8;
                        index += 1;
                    }
                    _ => break,
                }
            }
            if matches!(characters.get(index), Some('\n') | None) {
                if let Some('\n') = characters.get(index) {
                    index += 1;
                    line += 1;
                    line_start_index = index;
                }
                continue;
            }
            // **只有注释的行**（第 286 轮）：参照的词法器把它当**空行** ✓ ⇒ 既不判缩进、
            // 也不发 `Indent`／`Dedent` ✗。先前只跳"纯空白行" ✗ ⇒ 缩进写的注释行当场发 `Indent` ✗
            // —— 实测最小复现：`x = 0` 之后跟一行"缩进的注释"，下一句就报
            // "不认识的语句开头 Some(Indent)" ✗（`Lib/test/support/__init__.py:190` 那种
            // "续行注释"正是这样把那一族 **26** 个模块挡住的 ✓）。
            if matches!(characters.get(index), Some('#')) {
                while !matches!(characters.get(index), Some('\n') | None) {
                    index += 1;
                }
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
            // **制表符当空白**（行内）：缩进判定在上面的 `at_line_start` 已按 tabstop 归一 ✓
            // ⇒ 这里照空格处理 ✓（注释行／续行里也会出现它 ✓）。
            ' ' | '\r' | '\t' => index += 1,
            '\n' => {
                if depth > 0 {
                    // 隐式续行里的物理换行不进词法流（CPython 的 `NL` ✓）——但**记一笔**，
                    // `BC-62` 的多语句 lambda body 要据此切语句（见 `Lexed::line_breaks`）。
                    line_breaks.push(lexemes.len());
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
            // **前导点浮点**（第 405 轮）：`.05`／`.5e3` —— 参照的 `co_positions` 把常量位点
            // 定成**从 `.` 起** ✓（实测 `.05` ⇒ 列 11-14 ✓）。`.数字` 在 Python 里只能是浮点
            // （属性名不能以数字开头 ✓、`...` 是三个点 ✓）⇒ 无歧义 ✓。
            '.' if characters
                .get(index + 1)
                .is_some_and(|character| character.is_ascii_digit()) =>
            {
                let start = column!(index);
                let mut scan = index + 1;
                while scan < characters.len()
                    && (characters[scan].is_ascii_digit() || characters[scan] == '_')
                {
                    scan += 1;
                }
                if matches!(characters.get(scan), Some('e' | 'E')) {
                    let mut look = scan + 1;
                    if matches!(characters.get(look), Some('+' | '-')) {
                        look += 1;
                    }
                    if characters.get(look).is_some_and(|item| item.is_ascii_digit()) {
                        scan = look;
                        while scan < characters.len()
                            && (characters[scan].is_ascii_digit() || characters[scan] == '_')
                        {
                            scan += 1;
                        }
                    }
                }
                // **前导点虚部**（task-42 ✓）：`.01j`（官方 `test_format.py:608` 的
                // `f"{.01j:z.1f}"` ✗ —— 先前 `.01` 收成 Float、`j` 成名字 ⇒ f-string 解析失败）。
                if matches!(characters.get(scan), Some('j' | 'J')) {
                    let after = characters.get(scan + 1).copied();
                    if after.is_some_and(|item| item.is_ascii_alphanumeric() || item == '_') {
                        return Err(CompileError::Syntax(
                            "invalid imaginary literal".to_owned(),
                        ));
                    }
                    let digits: String = characters[index + 1..scan]
                        .iter()
                        .filter(|item| **item != '_')
                        .collect();
                    let text = format!("0.{digits}");
                    let value: f64 = text.parse().map_err(|_| {
                        CompileError::Unsupported(format!("虚部 {text} 解析不了"))
                    })?;
                    index = scan + 1;
                    lexemes.push(Lexeme::Imaginary(value.to_bits()));
                    spans.push(Span::new(line, line, start, column!(index)));
                    continue;
                }
                let digits: String = characters[index + 1..scan]
                    .iter()
                    .filter(|item| **item != '_')
                    .collect();
                // 补 `0` 前缀：Rust 的 `f64` 解析不收 `.5` 这种写法 ✓
                let text = format!("0.{digits}");
                let value: f64 = text.parse().map_err(|_| {
                    CompileError::Unsupported(format!("浮点 {text} 解析不了"))
                })?;
                index = scan;
                lexemes.push(Lexeme::Float(value.to_bits()));
                spans.push(Span::new(line, line, start, column!(index)));
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
                    ';' => Lexeme::Semicolon,
                    '%' => Lexeme::Percent,
                    '&' => Lexeme::Ampersand,
                    '|' => Lexeme::Pipe,
                    '^' => Lexeme::Caret,
                    '~' => Lexeme::Tilde,
                    '@' => Lexeme::At,
                    _ => unreachable!("这一臂列出的字符都有映射"),
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
                            return Err(CompileError::Syntax(format!("字符串没有收尾引号（第 {line} 行，列 {}）", column!(index))))
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
                // **浮点**（第 127 轮）：`1.5`／`2.`／`1e3`／`1.5e-3` ✓（十进制才有小数点／指数）
                if radix == 10 {
                    let mut scan = digits_start;
                    while scan < characters.len()
                        && (characters[scan].is_ascii_digit() || characters[scan] == '_')
                    {
                        scan += 1;
                    }
                    let mut is_float = false;
                    if characters.get(scan) == Some(&'.') {
                        is_float = true;
                        scan += 1;
                        while scan < characters.len()
                            && (characters[scan].is_ascii_digit() || characters[scan] == '_')
                        {
                            scan += 1;
                        }
                    }
                    if matches!(characters.get(scan), Some('e' | 'E')) {
                        let mut look = scan + 1;
                        if matches!(characters.get(look), Some('+' | '-')) {
                            look += 1;
                        }
                        if characters.get(look).is_some_and(|item| item.is_ascii_digit()) {
                            is_float = true;
                            scan = look;
                            while scan < characters.len()
                                && (characters[scan].is_ascii_digit() || characters[scan] == '_')
                            {
                                scan += 1;
                            }
                        }
                    }
                    // **虚部字面量**（task-29 ✓）：十进制整数／浮点／指数形式后接 `j`／`J`
                    // （`1j`／`1.5j`／`1e3j`／`0j`／`1_0j` ✓；`0x1j` 这类走下面 radix≠10 的报错 ✓）。
                    // 值＝虚部（f64 位模式）；实部由编译期补 `0.0` ⇒ `Constant::Complex` ✓。
                    if matches!(characters.get(scan), Some('j' | 'J')) {
                        let after = characters.get(scan + 1).copied();
                        if after.is_some_and(|item| item.is_ascii_alphanumeric() || item == '_') {
                            return Err(CompileError::Syntax(
                                "invalid imaginary literal".to_owned(),
                            ));
                        }
                        let text: String = characters[digits_start..scan]
                            .iter()
                            .filter(|item| **item != '_')
                            .collect();
                        let value: f64 = text.parse().map_err(|_| {
                            CompileError::Unsupported(format!("虚部 {text} 解析不了"))
                        })?;
                        index = scan + 1;
                        lexemes.push(Lexeme::Imaginary(value.to_bits()));
                        spans.push(Span::new(line, line, start, column!(index)));
                        continue;
                    }
                    if is_float {
                        let text: String = characters[digits_start..scan]
                            .iter()
                            .filter(|item| **item != '_')
                            .collect();
                        let value: f64 = text.parse().map_err(|_| {
                            CompileError::Unsupported(format!("浮点 {text} 解析不了"))
                        })?;
                        index = scan;
                        lexemes.push(Lexeme::Float(value.to_bits()));
                        spans.push(Span::new(line, line, start, column!(index)));
                        continue;
                    }
                }
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
                let value = match i64::from_str_radix(&digits, radix) {
                    Ok(small) => Lexeme::Int(small),
                    Err(_) => {
                        // **超出 `i64` 的字面量**（第 285 轮）：参照把大整数直接放进 `co_consts` ✓
                        // ⇒ 本层同一形态（折成十进制串 ✓，`Constant::BigInt` ✓）。
                        // 先前这里当场报"超出本层范围" ✗ —— `Lib/test/support` 一族 **26** 个模块压在它上面 ✓。
                        let magnitude = crate::bigint::BigInt::from_str_radix(&digits, radix)
                            .ok_or_else(|| {
                                // **非法进制字面量**（task-29 ✓）：`0x1j` 一类照参照报
                                // `invalid hexadecimal literal` 的语义（本层给带原文的语法错 ✓）。
                                let what = match radix {
                                    16 => "invalid hexadecimal literal",
                                    8 => "invalid octal literal",
                                    2 => "invalid binary literal",
                                    _ => "invalid decimal literal",
                                };
                                CompileError::Syntax(format!("{what}：{digits}"))
                            })?;
                        Lexeme::BigInt(magnitude.to_decimal())
                    }
                };
                index = end;
                lexemes.push(value);
                spans.push(Span::new(line, line, start, column!(index)));
            }
            character if character.is_alphabetic() || character == '_' => {
                // **`f`／`r` 前缀**（第 238 轮）：`f'…'`／`rf'…'`／`fr'…'` 收成 `FStr`（原文），
                // 裸 `r'…'` 与普通字符串同形（本层不处理转义）。必须**先于**标识符分支判断。
                // **`t` 前缀（PEP 750）**（第 401 轮）：`t'…'` 走**与 f-string 同一条**通道 ✓
                // **如实偏差**：本层**没有** `string.templatelib.Template` ⇒ `t"…"` 的求值结果是 `str` ✓
                // （参照给 `Template` ✓）；只保证**能解析／能 import** ✓，不保证 Template 语义 ✓。
                if matches!(character, 'f' | 'F' | 'r' | 'R' | 't' | 'T') {
                    let single = matches!(characters.get(index + 1), Some('\'') | Some('"'));
                    let doubled = matches!(
                        (character, characters.get(index + 1), characters.get(index + 2)),
                        ('f' | 'F', Some('r' | 'R'), Some('\'') | Some('"'))
                            | ('r' | 'R', Some('f' | 'F'), Some('\'') | Some('"'))
                            | ('t' | 'T', Some('r' | 'R'), Some('\'') | Some('"'))
                            | ('r' | 'R', Some('t' | 'T'), Some('\'') | Some('"'))
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
                        let prefix_has_f = matches!(character, 'f' | 'F' | 't' | 'T')
                            || matches!(
                                characters.get(index - 2),
                                Some('f') | Some('F') | Some('t') | Some('T')
                            );
                        // `r` 前缀（含 `rf`／`fr`）⇒ **原始字符串**：反斜杠原样留下
                        // **task-19 第 3 轮真 bug** ✗：`index - 3` 在**单字符前缀**（`f'…'` ⇒ index＝2 ✓）
                        // 会**减法溢出 panic** ✓（实测 `Lib/dataclasses.py` 的 f-string 子词法当场炸 ✓）
                        // ⇒ 用 `checked_sub` ✓：位次不够就**没有**那个字符 ✓（语义不变 ✓）。
                        let prefix_has_r = matches!(character, 'r' | 'R')
                            || matches!(characters.get(index.wrapping_sub(2)), Some('r') | Some('R'))
                            || matches!(
                                index
                                    .checked_sub(3)
                                    .and_then(|position| characters.get(position)),
                                Some('r') | Some('R')
                            );
                        let mut contents = String::new();
                        // **插值深度**（第 305 轮，PEP 701 ✓）：`f'{g(1, '__notes__', repr)}'` 是合法的 ✓
                        // —— 同种引号出现在 `{…}` **里面**时**不当收尾** ✗。先前一律"见引号就收尾" ✗
                        // ⇒ 正文在 `'` 处被截断 ⇒ 报「f-string: expecting '}'」✗
                        //（`Lib/traceback.py:1072` 那一族 15 个模块 ✓）。
                        let mut brace_depth = 0usize;
                        loop {
                            match characters.get(index) {
                                Some(current)
                                    if *current == quote
                                        && brace_depth == 0
                                        && (!triple
                                            || (characters.get(index + 1) == Some(&quote)
                                                && characters.get(index + 2) == Some(&quote))) =>
                                {
                                    index += if triple { 3 } else { 1 };
                                    break;
                                }
                                // **转义花括号**：在插值**之外**成对出现时是字面量 ✓（不改变深度 ✓）
                                Some('{') if prefix_has_f && brace_depth == 0 && characters.get(index + 1) == Some(&'{') => {
                                    contents.push('{');
                                    contents.push('{');
                                    index += 2;
                                }
                                Some('}') if prefix_has_f && brace_depth == 0 && characters.get(index + 1) == Some(&'}') => {
                                    contents.push('}');
                                    contents.push('}');
                                    index += 2;
                                }
                                Some('{') => {
                                    // **普通字符串里的 `{`／`}` 是字面量**（task-45 ✓）：
                                    // 花括号/引号嵌套那套只对 **f-string** 成立；先前不分前缀 ⇒
                                    // 普通串里的 `{` 把深度顶成 1 ⇒ 收尾引号认不出 ⇒
                                    // "字符串没有收尾引号"（官方 test_fstring.py 的测试数据串 ✗）。
                                    if prefix_has_f {
                                        brace_depth += 1;
                                    }
                                    contents.push('{');
                                    index += 1;
                                }
                                Some('}') => {
                                    if prefix_has_f {
                                        brace_depth = brace_depth.saturating_sub(1);
                                    }
                                    contents.push('}');
                                    index += 1;
                                }
                                // **插值表达式里的注释**（task-45 ✓，PEP 701）：`# 到行尾`，
                                // 注释里的引号**不是**字符串起始 ✓。动因：官方 test_fstring.py:770
                                // 的三引号 f-string 里 `{ # the following operation it's \n3 * 2}`
                                // —— 先前把注释里的 `'` 当内层字符串开头 ⇒ 一路吞到 109 行之后的
                                // 反斜杠 ⇒ 报"不认识的字符" ✗。
                                // **启发式**：`#` 后紧跟空白/换行才算注释（格式说明里的 `#` 是**格式
                                // 标志**：`f'{x:#x}'`／`f'{10:#{1}0x}'` ⇒ `#` 后不是空白 ⇒ 不当注释 ✓）。
                                Some('#')
                                    if prefix_has_f && brace_depth > 0
                                        && characters
                                            .get(index + 1)
                                            .is_some_and(|next| next.is_whitespace()) =>
                                {
                                    while let Some(current) = characters.get(index) {
                                        contents.push(*current);
                                        index += 1;
                                        if *current == '\n' {
                                            line += 1;
                                            line_start_index = index;
                                            break;
                                        }
                                    }
                                }
                                // **插值里的嵌套字符串**：原样吞到它的收尾引号 ✓
                                Some(current)
                                    if prefix_has_f && brace_depth > 0 && (*current == '\'' || *current == '"') =>
                                {
                                    let inner_quote = *current;
                                    contents.push(*current);
                                    index += 1;
                                    loop {
                                        match characters.get(index) {
                                            None => break,
                                            Some('\\') => {
                                                contents.push('\\');
                                                index += 1;
                                                if let Some(next) = characters.get(index) {
                                                    contents.push(*next);
                                                    index += 1;
                                                }
                                            }
                                            Some(inner) if *inner == inner_quote => {
                                                contents.push(*inner);
                                                index += 1;
                                                break;
                                            }
                                            Some(inner) => {
                                                if *inner == '\n' {
                                                    line += 1;
                                                    line_start_index = index + 1;
                                                }
                                                contents.push(*inner);
                                                index += 1;
                                            }
                                        }
                                    }
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
                                    return Err(CompileError::Syntax(format!(
                                        "字符串没有收尾引号（第 {line} 行，列 {}）",
                                        column!(index)
                                    )))
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
                // **`br'…'`／`rb'…'`**（第 291 轮）：**原始** bytes —— 反斜杠原样留下、不解码 ✓。
                // `Lib/glob.py:283` 的 `br'[\1]'` 就是它 ✗：先前 `br` 被当成**名字** ✗
                // ⇒ 随后那个字符串落进实参表 ⇒ 报"实参表里出现 `Some(Str(…))`" ✗（`glob` 那一族 15 个模块 ✓）。
                let raw_bytes_prefix = (matches!(character, 'b' | 'B')
                    && matches!(characters.get(index + 1), Some('r' | 'R'))
                    && matches!(characters.get(index + 2), Some('\'') | Some('"')))
                    || (matches!(character, 'r' | 'R')
                        && matches!(characters.get(index + 1), Some('b' | 'B'))
                        && matches!(characters.get(index + 2), Some('\'') | Some('"')));
                if raw_bytes_prefix {
                    let start = column!(index);
                    let quote = characters[index + 2];
                    let triple = characters.get(index + 3) == Some(&quote)
                        && characters.get(index + 4) == Some(&quote);
                    index += if triple { 5 } else { 3 };
                    let mut value: Vec<u8> = Vec::new();
                    loop {
                        let at_quote = characters.get(index) == Some(&quote)
                            && (!triple
                                || (characters.get(index + 1) == Some(&quote)
                                    && characters.get(index + 2) == Some(&quote)));
                        if at_quote {
                            index += if triple { 3 } else { 1 };
                            break;
                        }
                        match characters.get(index) {
                            // **原始** ⇒ 反斜杠原样是两个字节 ✓（不解码 ✓）
                            Some(current) if current.is_ascii() => {
                                value.push(*current as u8);
                                // 三引号跨行：行号与行首索引都要跟上（否则后续词元位点整体漂移 ✗）
                                if triple && *current == '\n' {
                                    line += 1;
                                    line_start_index = index + 1;
                                }
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
                // `b'…'`／`B"…"`：**先**看前缀（否则会先被当成名字 `b`）。
                // 转义在这一层就解成**字节**；非 ASCII 字符照参照报 `SyntaxError`。
                if matches!(character, 'b' | 'B')
                    && matches!(characters.get(index + 1), Some('\'') | Some('"'))
                {
                    let start = column!(index);
                    let start_line = line;
                    let quote = characters[index + 1];
                    // **三引号 bytes** ✓（task-25 ✓）：`b"""…"""`（可跨行）—— 先前只认单引号 ✗
                    // ⇒ `b"""…"""` 被拆成 `Bytes(空)` ＋ 一堆词元 ⇒ 实参表报"实参表里出现 …" ✗
                    //（官方 `test_exceptions` 的 tokenizer 用例正是它 ✓）。
                    let triple = characters.get(index + 2) == Some(&quote)
                        && characters.get(index + 3) == Some(&quote);
                    index += if triple { 4 } else { 2 };
                    // 转义报错里的 `position` 是**相对字面量内容**的下标（实测口径）
                    let content_start = index;
                    let mut value: Vec<u8> = Vec::new();
                    loop {
                        let at_quote = characters.get(index) == Some(&quote)
                            && (!triple
                                || (characters.get(index + 1) == Some(&quote)
                                    && characters.get(index + 2) == Some(&quote)));
                        if at_quote {
                            index += if triple { 3 } else { 1 };
                            break;
                        }
                        match characters.get(index) {
                            Some('\\') => {
                                let (byte, width) =
                                    lex_bytes_escape(&characters[index..], index - content_start)?;
                                value.push(byte);
                                index += width;
                            }
                            Some(current) if current.is_ascii() => {
                                value.push(*current as u8);
                                // 三引号跨行：行号与行首索引都要跟上（后续词元位点不漂移 ✓）
                                if triple && *current == '\n' {
                                    line += 1;
                                    line_start_index = index + 1;
                                }
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
                    spans.push(Span::new(start_line, line, start, column!(index)));
                    continue;
                }
                let start = column!(index);
                let start_index = index;
                while index < characters.len()
                    && is_identifier_continue(characters[index])
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
            '\\' => {
                // **显式行连接** ✓（第 196 轮）：`\` 紧跟换行 ⇒ **两者一起吃掉** ✓、逻辑行**不断** ✓
                //（`Lib/_sitebuiltins.py:95` 的 `…help, " \` 就是它 ✓）。行号要 +1 ✓、行首下标要跟 ✓。
                if characters.get(index + 1) == Some(&'\n') {
                    index += 2;
                    line += 1;
                    line_start_index = index;
                    continue;
                }
                return Err(CompileError::Syntax(format!(
                    "不认识的字符 '\\\\'（第 {line} 行，列 {}）",
                    column!(index)
                )));
            }
            // **`?`**（`TS-46`）：**只**在扩展模式下是词素；纯 Python 模式落到下面的
            // `other` 臂当场报 `Syntax`（`BC-15` 的拒绝面就靠这一条，不另设分支）。
            '?' if mode == Mode::Extension => {
                let start = column!(index);
                lexemes.push(Lexeme::Question);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            other => {
                return Err(CompileError::Syntax(format!(
                    "不认识的字符 {other:?}（第 {line} 行，列 {}）",
                    column!(index)
                )))
            }
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
    Ok(Lexed { lexemes, spans, mode, line_breaks })
}

// ---- 语法 ----


// ==== 公开的 Python 词法视图（task-19：`_tokenize` 的**唯一**底层 ✓）==============================
//
// **一处真相** ✓：`_tokenize.TokenizerIter` 不另写扫描器 ✗ —— 它直接消费这里的
// [`python_tokens`] ✓（底层就是编译器的 [`lex`] ✓，字符串／数字／f-string／转义全走同一份实现 ✓）。
// 本层只把**词法单元**投影成 (类别 ＋ 起止位置) ✓；**原文**由调用方按 span 从源码切 ✓
// （编译器的 `Str`／`Int` 等已解码 ✗ —— tokenize 要的是**源文本** ✓，故不在这里给文本 ✓）。

/// 公开的**词法类别** ✓（对 `token` 模块的数字映射由 `_tokenize` 那一侧做 ✓）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PythonTokenKind {
    /// 名字**与关键字**（`tokenize` 把关键字也叫 `NAME` ✓）。
    Name,
    Number,
    /// `str` 字面量（含前后缀与三引号 ✓）。
    Str,
    /// `bytes` 字面量。
    Bytes,
    /// **f-string**（整段一个词法单元 ✓ —— 参照的 `FSTRING_*` 三段拆分在 `_tokenize` 那侧做 ✓）。
    FString,
    /// 运算符／分隔符（原文由 span 取 ✓）。
    Op,
    /// 逻辑行结束（`NEWLINE` ✓）。
    Newline,
    Indent,
    Dedent,
    /// 文件尾（`ENDMARKER` ✓）。
    End,
}

/// 一个公开词法单元：类别 ＋ `(行, 列)` 起止 ✓（**行从 1 起、列从 0 起** ✓，与 `tokenize` 同口径 ✓）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PythonToken {
    pub kind: PythonTokenKind,
    pub start: (u32, u32),
    pub end: (u32, u32),
}

/// 词法单元 ⇒ 公开类别 ✓（关键字统一成 `Name` ✓ —— 与参照一致 ✓）。
fn public_kind(lexeme: &Lexeme) -> PythonTokenKind {
    match lexeme {
        Lexeme::Name(_) => PythonTokenKind::Name,
        Lexeme::Int(_) | Lexeme::BigInt(_) | Lexeme::Float(_) | Lexeme::Imaginary(_) => {
            PythonTokenKind::Number
        }
        Lexeme::Str(_) => PythonTokenKind::Str,
        Lexeme::Bytes(_) => PythonTokenKind::Bytes,
        Lexeme::FStr { .. } => PythonTokenKind::FString,
        Lexeme::Newline => PythonTokenKind::Newline,
        Lexeme::Indent => PythonTokenKind::Indent,
        Lexeme::Dedent => PythonTokenKind::Dedent,
        Lexeme::End => PythonTokenKind::End,
        // 关键字（`def`／`class`／…）与一切运算符／分隔符 ⇒ `Name`／`Op` ✓（原文从 span 切 ✓）。
        Lexeme::Def | Lexeme::Class | Lexeme::Return | Lexeme::Nonlocal | Lexeme::Global
        | Lexeme::Yield | Lexeme::Raise | Lexeme::If | Lexeme::Else | Lexeme::While
        | Lexeme::For | Lexeme::In => PythonTokenKind::Name,
        _ => PythonTokenKind::Op,
    }
}

/// **公开的词法入口** ✓：源码（`str` ✓）⇒ 词法单元流 ✓；词法错按**消息**回 ✓
/// （调用方按参照把 `SyntaxError` 报出去 ✓）。
pub fn python_tokens(source: &str) -> Result<Vec<PythonToken>, String> {
    // `_tokenize` 是**纯 Python** 的视图（`.py` 语料）⇒ 固定纯模式（`?` 不是合法 token ✓）。
    let lexed = lex(source, Mode::PurePython).map_err(|error| format!("{error:?}"))?;
    Ok(lexed
        .lexemes
        .iter()
        .zip(lexed.spans.iter())
        .map(|(lexeme, span)| PythonToken {
            kind: public_kind(lexeme),
            start: (span.line_start, span.col_start),
            end: (span.line_end, span.col_end),
        })
        .collect())
}
