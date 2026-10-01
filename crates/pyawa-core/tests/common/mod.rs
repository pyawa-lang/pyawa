//! 集成测试共用的小工具：够用的 JSON 解析器（`pyawa-core` 不带 JSON 依赖）。
//!
//! 期望值一律来自 `tests/fixture-*.json`（生成器见 `tools/gen_*_fixture.py`）。

// --------------------------------------------------------------------------- #
// 极简 JSON（够读夹具：对象／数组／字符串／整数／bool／null）
// --------------------------------------------------------------------------- #

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(entries) => entries.iter().find(|(name, _)| name == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn key(&self, key: &str) -> &Json {
        self.get(key).unwrap_or_else(|| panic!("夹具缺字段 {key}"))
    }

    pub fn as_i64(&self) -> i64 {
        match self {
            Json::Num(value) => *value,
            other => panic!("期望整数，得到 {other:?}"),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Json::Str(value) => value,
            other => panic!("期望字符串，得到 {other:?}"),
        }
    }

    pub fn as_arr(&self) -> &[Json] {
        match self {
            Json::Arr(items) => items,
            other => panic!("期望数组，得到 {other:?}"),
        }
    }

    pub fn as_obj(&self) -> &[(String, Json)] {
        match self {
            Json::Obj(entries) => entries,
            other => panic!("期望对象，得到 {other:?}"),
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self { bytes: text.as_bytes(), position: 0 }
    }

    fn parse(mut self) -> Json {
        self.skip_whitespace();
        let value = self.value();
        self.skip_whitespace();
        value
    }

    fn peek(&self) -> u8 {
        *self.bytes.get(self.position).unwrap_or(&0)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), b' ' | b'\n' | b'\r' | b'\t') {
            self.position += 1;
        }
    }

    fn value(&mut self) -> Json {
        match self.peek() {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Json::Str(self.string()),
            b't' => {
                self.literal("true");
                Json::Bool(true)
            }
            b'f' => {
                self.literal("false");
                Json::Bool(false)
            }
            b'n' => {
                self.literal("null");
                Json::Null
            }
            _ => self.number(),
        }
    }

    fn literal(&mut self, text: &str) {
        assert!(
            self.bytes[self.position..].starts_with(text.as_bytes()),
            "字面量 {text} 不匹配"
        );
        self.position += text.len();
    }

    fn object(&mut self) -> Json {
        self.position += 1; // '{'
        let mut entries = Vec::new();
        self.skip_whitespace();
        if self.peek() == b'}' {
            self.position += 1;
            return Json::Obj(entries);
        }
        loop {
            self.skip_whitespace();
            let key = self.string();
            self.skip_whitespace();
            assert_eq!(self.peek(), b':');
            self.position += 1;
            self.skip_whitespace();
            entries.push((key, self.value()));
            self.skip_whitespace();
            match self.peek() {
                b',' => self.position += 1,
                b'}' => {
                    self.position += 1;
                    break;
                }
                other => panic!("对象里出现意外字节 {other}"),
            }
        }
        Json::Obj(entries)
    }

    fn array(&mut self) -> Json {
        self.position += 1; // '['
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == b']' {
            self.position += 1;
            return Json::Arr(items);
        }
        loop {
            self.skip_whitespace();
            items.push(self.value());
            self.skip_whitespace();
            match self.peek() {
                b',' => self.position += 1,
                b']' => {
                    self.position += 1;
                    break;
                }
                other => panic!("数组里出现意外字节 {other}"),
            }
        }
        Json::Arr(items)
    }

    fn string(&mut self) -> String {
        assert_eq!(self.peek(), b'"');
        self.position += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                b'"' => {
                    self.position += 1;
                    break;
                }
                b'\\' => {
                    self.position += 1;
                    match self.peek() {
                        b'n' => {
                            out.push('\n');
                            self.position += 1;
                        }
                        b't' => {
                            out.push('\t');
                            self.position += 1;
                        }
                        b'"' => {
                            out.push('"');
                            self.position += 1;
                        }
                        b'\\' => {
                            out.push('\\');
                            self.position += 1;
                        }
                        b'/' => {
                            out.push('/');
                            self.position += 1;
                        }
                        b'u' => {
                            let hex = std::str::from_utf8(&self.bytes[self.position + 1..self.position + 5])
                                .expect("转义不是 UTF-8");
                            let code = u32::from_str_radix(hex, 16).expect("转义不是十六进制");
                            out.push(char::from_u32(code).expect("转义不是合法码点"));
                            self.position += 5;
                        }
                        other => panic!("未知转义 \\{other}"),
                    }
                }
                0 => panic!("字符串未闭合"),
                _ => {
                    let rest = std::str::from_utf8(&self.bytes[self.position..]).expect("不是 UTF-8");
                    let character = rest.chars().next().expect("空字符串");
                    out.push(character);
                    self.position += character.len_utf8();
                }
            }
        }
        out
    }

    fn number(&mut self) -> Json {
        let start = self.position;
        if self.peek() == b'-' {
            self.position += 1;
        }
        while self.peek().is_ascii_digit() {
            self.position += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.position]).expect("数字不是 UTF-8");
        Json::Num(text.parse().expect("不是整数"))
    }
}

/// 解析夹具文本（调用方用 `include_str!` 把夹具读进来）。
pub fn parse(text: &str) -> Json {
    Parser::new(text).parse()
}
