//! `format(value, spec)` 的**受测子集**：`FORMAT_WITH_SPEC` 的默认行为。
//!
//! 语法（照参照实现）：`[[fill]align][sign][z][#][0][width][grouping][.precision][type]`。
//! 消息与形状都是**实测**的（见 `tests/format_spec.rs` 的文件头）：
//!
//! ```text
//! format(42, '>5')      = '   42'
//! format(42, '05')      = '00042'        format(42, '#x')  = '0x2a'
//! format(42, '_b')      = '10_1010'      format(42, 'c')   = '*'
//! format(42, '=+8')     = '+     42'
//! format(3.14159, '.2f')= '3.14'         format(3.14159, 'e') = '3.141590e+00'
//! format(3.14159, '%')  = '314.159000%'  format(3.0, 'g')  = '3'
//! format('ab', '.1')    = 'a'            format('ab', '5') = 'ab   '
//! format(True, 'd')     = '1'            format(True, '')  = 'True'
//! format(None, '')      = 'None'
//! ```
//!
//! **未实现的**（如实报"未实现"，不猜）：`z`（负零强制的报错照实测给出）、
//! `n` 的本地化（按 `d` 处理）、`=` 之外的数值填充细节、`c` 之外的字符码。

/// 解析后的格式规格。
#[derive(Clone, Copy, Debug, Default)]
pub struct Spec {
    /// 填充字符（默认空格）。
    pub fill: char,
    /// 对齐（`<`／`>`／`^`／`=`）。
    pub align: Option<char>,
    /// 符号（`+`／`-`／空格）。
    pub sign: Option<char>,
    /// `#`：进制前缀。
    pub alternate: bool,
    /// `0`：用零填充（等价于 `fill='0'` ＋ `align='='`）。
    pub zero: bool,
    /// 最小宽度。
    pub width: Option<usize>,
    /// 分组符（`,` 或 `_`）。
    pub grouping: Option<char>,
    /// 精度（`.N`）。
    pub precision: Option<usize>,
    /// 类型码（最后一个字符）。
    pub ty: Option<char>,
}

/// 规格里带 `z`（负零强制）时的**实测**报错。
pub const NEGATIVE_ZERO_MESSAGE: &str =
    "Negative zero coercion (z) not allowed in integer format specifier";

/// 格式化失败：调用方按类别给出**实测**的消息。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecError {
    /// 规格里出现了 `z`（负零强制）：参照实现在整数上有专门报错。
    NegativeZero,
    /// 类型码不在本层实现的范围内（调用方报 `Unknown format code 'c' for object of type 'int'`）。
    UnknownCode(char),
    /// 这个写法本层还没实现（调用方如实报未实现，不猜语义）。
    NotImplemented,
}

/// 解析规格字符串。
pub fn parse(text: &str) -> Result<Spec, SpecError> {
    let characters: Vec<char> = text.chars().collect();
    let mut index = 0usize;
    let mut spec = Spec {
        fill: ' ',
        ..Spec::default()
    };
    // `[[fill]align]`：fill 必须紧跟一个对齐字符
    if characters.len() >= 2 && matches!(characters[1], '<' | '>' | '^' | '=') {
        spec.fill = characters[0];
        spec.align = Some(characters[1]);
        index += 2;
    } else if let Some(first) = characters.first() {
        if matches!(first, '<' | '>' | '^' | '=') {
            spec.align = Some(*first);
            index += 1;
        }
    }
    // `[sign]`
    if let Some(character) = characters.get(index) {
        if matches!(character, '+' | '-' | ' ') {
            spec.sign = Some(*character);
            index += 1;
        }
    }
    // `[z]`（参照实现的负零强制；本层照实测报错）
    if characters.get(index) == Some(&'z') {
        return Err(SpecError::NegativeZero);
    }
    // `[#]`
    if characters.get(index) == Some(&'#') {
        spec.alternate = true;
        index += 1;
    }
    // `[0]`
    if characters.get(index) == Some(&'0') {
        spec.zero = true;
        index += 1;
    }
    // `[width]`
    let width_start = index;
    while characters.get(index).is_some_and(|c| c.is_ascii_digit()) {
        index += 1;
    }
    if index > width_start {
        spec.width = text[width_start..index].parse::<usize>().ok();
        // `0` 已经被当作零填充吃掉了；`05` 的 `5` 在这里
    }
    // `[grouping]`
    if let Some(character) = characters.get(index) {
        if *character == ',' || *character == '_' {
            spec.grouping = Some(*character);
            index += 1;
        }
    }
    // `[.precision]`
    if characters.get(index) == Some(&'.') {
        index += 1;
        let precision_start = index;
        while characters.get(index).is_some_and(|c| c.is_ascii_digit()) {
            index += 1;
        }
        spec.precision = text[precision_start..index].parse::<usize>().ok().or(Some(0));
    }
    // `[type]`
    if let Some(character) = characters.get(index) {
        spec.ty = Some(*character);
        index += 1;
    }
    if index < characters.len() {
        // 剩下的部分本层不认：如实报"未实现"（不猜语义）
        return Err(SpecError::NotImplemented);
    }
    Ok(spec)
}

/// 对齐后的最终文本。
///
/// `0`（`spec.zero`）在没有显式对齐时等价于"填充 `0` ＋ 符号感知对齐"（实测
/// `format(42, '05') = '00042'`、`format(42, '=+8') = '+     42'`）。
fn pad(body: String, spec: &Spec, numeric: bool) -> String {
    let width = spec.width.unwrap_or(0);
    let length = body.chars().count();
    if length >= width {
        return body;
    }
    let missing = width - length;
    let zero_mode = spec.zero && spec.align.is_none();
    let fill = if zero_mode { '0' } else { spec.fill };
    let align = spec.align.unwrap_or(if zero_mode {
        '='
    } else if numeric {
        '>'
    } else {
        '<'
    });
    match align {
        '<' => format!("{body}{}", fill.to_string().repeat(missing)),
        '>' => format!("{}{body}", fill.to_string().repeat(missing)),
        '^' => {
            let left = missing / 2;
            let right = missing - left;
            format!(
                "{}{body}{}",
                fill.to_string().repeat(left),
                fill.to_string().repeat(right)
            )
        }
        // `=`：填充插在符号／前缀之后（整数与浮点的"零填充"就是这个）
        _ => {
            let split = body
                .char_indices()
                .position(|(_, character)| {
                    !matches!(character, '+' | '-' | ' ' | '#' | '0' | 'x' | 'X' | 'o' | 'b')
                })
                .map(|position| position)
                .unwrap_or(body.len());
            let (head, tail) = body.split_at(split);
            format!("{head}{}{tail}", fill.to_string().repeat(missing))
        }
    }
}

/// `str` 的格式化（实测：默认左对齐，`.N` 截断）。
pub fn format_str(text: &str, spec: &Spec) -> Result<String, SpecError> {
    let mut body = match spec.precision {
        Some(precision) => text.chars().take(precision).collect::<String>(),
        None => text.to_owned(),
    };
    if spec.zero {
        body = pad(body, spec, false);
    }
    Ok(pad(body, spec, false))
}

/// 整数分组（`,` 每三位；`_` 在二进制／八进制／十六进制里每四位，十进制每三位——实测 `10_1010`）。
fn group(digits: &str, spec: &Spec, base: u32) -> String {
    let Some(separator) = spec.grouping else {
        return digits.to_owned();
    };
    if separator == ',' && base != 10 {
        return digits.to_owned();
    }
    let size = if base == 10 { 3 } else { 4 };
    let characters: Vec<char> = digits.chars().collect();
    let mut out: Vec<char> = Vec::new();
    for (index, character) in characters.iter().enumerate() {
        if index > 0 && (characters.len() - index) % size == 0 {
            out.push(separator);
        }
        out.push(*character);
    }
    out.into_iter().collect()
}

/// 浮点文本的**千分位／下划线分组**（实测 `format(1234.5, ',.2f')` ⇒ `1,234.50`）。
///
/// 只分组**整数部分**：先按指数切（`e`／`E` 的尾数也可能有整数部分），再按小数点切。
/// 浮点的 `_` 也是每 3 位（与 `int` 的十六进制/二进制每 4 位不同），所以 base 一律取 10。
fn group_float(body: &str, spec: &Spec) -> String {
    if spec.grouping.is_none() {
        return body.to_owned();
    }
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(index) => (&body[..index], &body[index..]),
        None => (body, ""),
    };
    let (sign, digits) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa),
    };
    let (integer, fraction) = match digits.find('.') {
        Some(index) => (&digits[..index], &digits[index..]),
        None => (digits, ""),
    };
    let grouped = group(integer, spec, 10);
    format!("{sign}{grouped}{fraction}{exponent}")
}

/// `int` 的格式化。
pub fn format_int(value: i64, spec: &Spec) -> Result<String, SpecError> {
    // 浮点类型的码转给浮点那条（实测：`format(42, '.2f') = '42.00'`）
    match spec.ty {
        Some('f') | Some('F') | Some('e') | Some('E') | Some('g') | Some('G') | Some('%') => {
            return format_float(value as f64, spec);
        }
        _ => {}
    }
    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    let (digits, prefix) = match spec.ty {
        None | Some('d') | Some('n') => (magnitude.to_string(), String::new()),
        Some('b') => (
            format!("{magnitude:b}"),
            if spec.alternate { "0b".to_owned() } else { String::new() },
        ),
        Some('o') => (
            format!("{magnitude:o}"),
            if spec.alternate { "0o".to_owned() } else { String::new() },
        ),
        Some('x') => (
            format!("{magnitude:x}"),
            if spec.alternate { "0x".to_owned() } else { String::new() },
        ),
        Some('X') => (
            format!("{magnitude:X}"),
            if spec.alternate { "0X".to_owned() } else { String::new() },
        ),
        Some('c') => (
            // 实测：`format(42, 'c') = '*'`
            char::from_u32(magnitude as u32)
                .map(|character| character.to_string())
                .unwrap_or_default(),
            String::new(),
        ),
        Some(other) => return Err(SpecError::UnknownCode(other)),
    };
    let base = match spec.ty {
        Some('b') => 2,
        Some('o') => 8,
        Some('x') | Some('X') => 16,
        _ => 10,
    };
    let digits = group(&digits, spec, base);
    let sign = if negative {
        "-"
    } else {
        match spec.sign {
            Some('+') => "+",
            Some(' ') => " ",
            _ => "",
        }
    };
    Ok(pad(format!("{sign}{prefix}{digits}"), spec, true))
}

/// 浮点的指数写法：参照实现补足两位指数并带符号（实测 `3.141590e+00`）。
fn exponent_text(text: &str, upper: bool) -> String {
    let marker = if upper { 'E' } else { 'e' };
    match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => {
            let (sign, digits) = if let Some(rest) = exponent.strip_prefix('-') {
                ("-", rest)
            } else {
                ("+", exponent)
            };
            let padded = if digits.len() < 2 {
                format!("0{digits}")
            } else {
                digits.to_owned()
            };
            format!("{mantissa}{marker}{sign}{padded}")
        }
        None => text.to_owned(),
    }
}

/// `float` 的格式化。
pub fn format_float(value: f64, spec: &Spec) -> Result<String, SpecError> {
    let precision = spec.precision;
    let upper = matches!(spec.ty, Some('F') | Some('E') | Some('G'));
    let mut suffix = String::new();
    let body = match spec.ty {
        None => {
            // 没有类型码：最短往返（`repr` 的形态），精度在场时按 `g` 处理
            match precision {
                Some(digits) => format!("{value:.digits$}"),
                None => repr_float(value),
            }
        }
        Some('f') | Some('F') => {
            let digits = precision.unwrap_or(6);
            format!("{value:.digits$}")
        }
        Some('e') | Some('E') => {
            let digits = precision.unwrap_or(6);
            let text = format!("{value:.digits$e}");
            exponent_text(&text, upper)
        }
        Some('g') | Some('G') => {
            let digits = precision.unwrap_or(6).max(1);
            let text = format!("{value:.digits$}");
            let trimmed = text.trim_end_matches('0').trim_end_matches('.').to_owned();
            if trimmed.is_empty() || trimmed == "-" {
                "0".to_owned()
            } else {
                trimmed
            }
        }
        Some('%') => {
            let digits = precision.unwrap_or(6);
            let text = format!("{:.digits$}", value * 100.0);
            suffix.push('%');
            text
        }
        Some('n') => {
            let digits = precision.unwrap_or(6);
            format!("{value:.digits$}")
        }
        Some(other) => return Err(SpecError::UnknownCode(other)),
    };
    // 符号
    let needs_sign = spec.sign.is_some() && !body.starts_with('-');
    let sign = if body.starts_with('-') {
        ""
    } else {
        match spec.sign {
            Some('+') => "+",
            Some(' ') => " ",
            _ => "",
        }
    };
    let _ = needs_sign;
    let body = group_float(&body, spec);
    Ok(pad(format!("{sign}{body}{suffix}"), spec, true))
}

/// 没有类型码时的浮点文本（`repr` 的形态，实测 `3.0` → `3.0`、`3.14159` → `3.14159`）。
pub fn repr_float(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    let text = format!("{value:?}");
    match text.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with(['-', '+']) => {
            format!("{mantissa}e+{exponent}")
        }
        _ => text,
    }
}
