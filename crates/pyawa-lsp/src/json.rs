//! **手写 JSON**（LSP 帧体的极小子集）——本 crate **零第三方依赖**（`CX-27` 允许但不强求 ✓）。
//!
//! 只覆盖 LSP 客户端会发来的形状：object／array／string／number／bool／null；字符串支持
//! JSON 的转义（含 `\uXXXX` 与代理对）。**不做**重复键合并、数字精度之类的扩展（LSP 用不到）。
//! 序列化用于我们发回的通知／响应（键顺序固定、无美化）。

/// JSON 值（本模块的唯一表示）。
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `null`
    Null,
    /// `true`／`false`
    Bool(bool),
    /// 数字（统一按 `f64`；LSP 的 `id` 与行列都是小整数）
    Number(f64),
    /// 字符串
    String(String),
    /// 数组
    Array(Vec<Value>),
    /// 对象（保持插入顺序；查键取第一个命中）
    Object(Vec<(String, Value)>),
}

impl Value {
    /// 取对象键（非对象或没有 ⇒ `None`）。
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries
                .iter()
                .find(|(existing, _)| existing == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// 字符串视图。
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    /// 无符号 32 位整数视图（LSP 的 line／character／id 都是小整数）。
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Value::Number(number) if *number >= 0.0 && number.fract() == 0.0 => {
                Some(*number as u32)
            }
            _ => None,
        }
    }

    /// 布尔视图。
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// 序列化成 JSON 文本（紧凑、键顺序保持）。
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        write_value(&mut out, self);
        out
    }
}

/// 把字符串变成**带引号的** JSON 字符串（测试与内部拼装用）。
pub fn escape_quoted(text: &str) -> String {
    format!("\"{}\"", escape(text))
}

/// 把字符串转义成 JSON 字符串**内容**（不含首尾引号）。
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => out.push(other),
        }
    }
    out
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => {
            if number.fract() == 0.0 && number.abs() < 9.0e15 {
                out.push_str(&format!("{}", *number as i64));
            } else {
                out.push_str(&format!("{number}"));
            }
        }
        Value::String(text) => {
            out.push('"');
            out.push_str(&escape(text));
            out.push('"');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (index, (key, item)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push('"');
                out.push_str(&escape(key));
                out.push_str("\":");
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

/// 解析一段 JSON（UTF-8 已由调用方解好）。
pub fn parse(bytes: &[u8]) -> Result<Value, String> {
    let mut parser = Parser { bytes, index: 0 };
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.index != parser.bytes.len() {
        return Err("JSON 末尾有多余字节".to_owned());
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl Parser<'_> {
    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.bytes.get(self.index) {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.index += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.index += 1;
            Ok(())
        } else {
            Err(format!("JSON 期望 `{}`", byte as char))
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Value::String(self.parse_string()?)),
            Some(b't') => self.parse_literal("true", Value::Bool(true)),
            Some(b'f') => self.parse_literal("false", Value::Bool(false)),
            Some(b'n') => self.parse_literal("null", Value::Null),
            Some(byte) if byte == b'-' || byte.is_ascii_digit() => self.parse_number(),
            _ => Err("JSON 值不合法".to_owned()),
        }
    }

    fn parse_literal(&mut self, text: &str, value: Value) -> Result<Value, String> {
        let end = self.index + text.len();
        if self.bytes.get(self.index..end) == Some(text.as_bytes()) {
            self.index = end;
            Ok(value)
        } else {
            Err(format!("JSON 字面量 `{text}` 不合法"))
        }
    }

    fn parse_object(&mut self) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.index += 1;
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.index += 1;
                }
                Some(b'}') => {
                    self.index += 1;
                    return Ok(Value::Object(entries));
                }
                _ => return Err("JSON 对象缺 `,` 或 `}`".to_owned()),
            }
        }
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.index += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.index += 1;
                }
                Some(b']') => {
                    self.index += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err("JSON 数组缺 `,` 或 `]`".to_owned()),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err("JSON 字符串未闭合".to_owned());
            };
            self.index += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(escaped) = self.peek() else {
                        return Err("JSON 转义未闭合".to_owned());
                    };
                    self.index += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let high = self.parse_hex4()?;
                            if (0xD800..=0xDBFF).contains(&high) {
                                // 代理对：期望紧跟 `\uXXXX` 的低代理
                                if self.peek() == Some(b'\\')
                                    && self.bytes.get(self.index + 1) == Some(&b'u')
                                {
                                    self.index += 2;
                                    let low = self.parse_hex4()?;
                                    if (0xDC00..=0xDFFF).contains(&low) {
                                        let scalar = 0x10000
                                            + ((high - 0xD800) << 10)
                                            + (low - 0xDC00);
                                        out.push(
                                            char::from_u32(scalar).unwrap_or('\u{fffd}'),
                                        );
                                    } else {
                                        out.push('\u{fffd}');
                                    }
                                } else {
                                    out.push('\u{fffd}');
                                }
                            } else if (0xDC00..=0xDFFF).contains(&high) {
                                out.push('\u{fffd}');
                            } else {
                                out.push(char::from_u32(high).unwrap_or('\u{fffd}'));
                            }
                        }
                        _ => return Err("JSON 未知转义".to_owned()),
                    }
                }
                // 裸 UTF-8：把这一段字节收进 out（字符串其余部分按字节走）
                byte if byte < 0x80 => out.push(byte as char),
                _ => {
                    let start = self.index - 1;
                    let width = utf8_width(byte);
                    let end = start + width;
                    if end > self.bytes.len() {
                        return Err("JSON 字符串的 UTF-8 不完整".to_owned());
                    }
                    let slice = &self.bytes[start..end];
                    out.push_str(
                        core::str::from_utf8(slice)
                            .map_err(|_| "JSON 字符串不是合法 UTF-8".to_owned())?,
                    );
                    self.index = end;
                }
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let Some(byte) = self.peek() else {
                return Err("JSON \\u 转义不完整".to_owned());
            };
            self.index += 1;
            let digit = (byte as char)
                .to_digit(16)
                .ok_or_else(|| "JSON \\u 转义不是十六进制".to_owned())?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.index;
        if self.peek() == Some(b'-') {
            self.index += 1;
        }
        while let Some(byte) = self.peek() {
            if byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.index += 1;
            } else {
                break;
            }
        }
        let text = core::str::from_utf8(&self.bytes[start..self.index])
            .map_err(|_| "JSON 数字不是 UTF-8".to_owned())?;
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| format!("JSON 数字不合法：{text}"))
    }
}

fn utf8_width(first: u8) -> usize {
    if first >= 0xF0 {
        4
    } else if first >= 0xE0 {
        3
    } else {
        2
    }
}
