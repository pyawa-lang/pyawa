//! **LSP 服务主体**（stdio ＋ `Content-Length` 帧的 JSON-RPC 2.0）。
//!
//! v1 范围（与 `crates/pyawa-lsp/README.md` 一处真相）：`initialize`／`initialized`／
//! `shutdown`／`exit`；`textDocument/didOpen`／`didChange`（**全量**）／`didClose`；
//! `textDocument/publishDiagnostics`（`check_source` 投影）；`textDocument/hover`。
//! **只编译不执行**：不建 VM 实例、不跑字节码 ✓。

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};

use pyawa_core::compile::{check_source, Mode};

use crate::json::{self, Value};

/// 在 stdin／stdout 上跑 LSP 循环（阻塞到 `exit` 通知或 stdin 关闭）。
pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut server = Server {
        documents: HashMap::new(),
    };
    loop {
        let Some(frame) = read_frame(&mut reader)? else {
            break;
        };
        let Ok(message) = json::parse(frame.as_bytes()) else {
            write_frame(
                &mut writer,
                r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}"#,
            )?;
            continue;
        };
        let mut outgoing: Vec<String> = Vec::new();
        let keep_running = server.handle(&message, &mut outgoing);
        for body in outgoing {
            write_frame(&mut writer, &body)?;
        }
        if !keep_running {
            break;
        }
    }
    Ok(())
}

/// 读一个 LSP 帧（`Content-Length` 头 ＋ 空行 ＋ 恰好 N 字节）；EOF ⇒ `None`。
fn read_frame<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    let mut length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            length = rest.trim().parse().ok();
        }
    }
    let Some(length) = length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LSP 帧缺 Content-Length",
        ));
    };
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    String::from_utf8(body)
        .map(Some)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "LSP 帧体不是 UTF-8"))
}

/// 写一个 LSP 帧（`Content-Length` 用的是**字节数** ✓）。
fn write_frame<W: Write>(writer: &mut W, body: &str) -> io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()
}

/// 服务器状态（打开的文档文本；全量同步 ⇒ 每文档只存**最新全文**）。
struct Server {
    documents: HashMap<String, String>,
}

impl Server {
    /// 处理一条消息；返回 `false` 表示收到 `exit`（应结束进程）。响应／通知追加进 `outgoing`。
    fn handle(&mut self, message: &Value, outgoing: &mut Vec<String>) -> bool {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // 客户端对我们发出的响应（本服务不发请求）：忽略。
            return true;
        };
        let id = message.get("id");
        let params = message.get("params");
        match method {
            "initialize" => respond(outgoing, id, initialize_result()),
            "initialized" => {}
            "shutdown" => respond(outgoing, id, "null".to_owned()),
            "exit" => return false,
            "textDocument/didOpen" => {
                if let Some(params) = params {
                    self.did_open(params, outgoing);
                }
            }
            "textDocument/didChange" => {
                if let Some(params) = params {
                    self.did_change(params, outgoing);
                }
            }
            "textDocument/didClose" => {
                if let Some(params) = params {
                    self.did_close(params, outgoing);
                }
            }
            "textDocument/hover" => {
                let result = self.hover(params);
                respond(outgoing, id, result);
            }
            "textDocument/completion" => {
                let result = self.completion(params);
                respond(outgoing, id, result);
            }
            "textDocument/definition" => {
                let result = self.definition(params);
                respond(outgoing, id, result);
            }
            "textDocument/rename" => {
                let result = self.rename(params);
                respond(outgoing, id, result);
            }
            _ => {
                if id.is_some() {
                    respond_error(outgoing, id, -32601, "method not found");
                }
            }
        }
        true
    }

    fn did_open(&mut self, params: &Value, outgoing: &mut Vec<String>) {
        let Some(document) = params.get("textDocument") else {
            return;
        };
        let (Some(uri), Some(text)) = (
            document.get("uri").and_then(Value::as_str),
            document.get("text").and_then(Value::as_str),
        ) else {
            return;
        };
        let language_id = document
            .get("languageId")
            .and_then(Value::as_str)
            .unwrap_or("");
        self.documents.insert(uri.to_owned(), text.to_owned());
        self.publish(outgoing, uri, text, language_id);
    }

    fn did_change(&mut self, params: &Value, outgoing: &mut Vec<String>) {
        let Some(document) = params.get("textDocument") else {
            return;
        };
        let Some(uri) = document.get("uri").and_then(Value::as_str) else {
            return;
        };
        // 全量同步（`textDocumentSync.change = 1`）：取第一个 contentChange 的 `text`。
        let new_text = params
            .get("contentChanges")
            .and_then(|changes| match changes {
                Value::Array(items) => items.first(),
                _ => None,
            })
            .and_then(|change| change.get("text"))
            .and_then(Value::as_str);
        let Some(new_text) = new_text else {
            return;
        };
        let uri = uri.to_owned();
        self.documents.insert(uri.clone(), new_text.to_owned());
        let language_id = if uri.ends_with(".pyawa") { "pyawa" } else { "python" };
        self.publish(outgoing, &uri, new_text, language_id);
    }

    fn did_close(&mut self, params: &Value, outgoing: &mut Vec<String>) {
        let Some(uri) = params
            .get("textDocument")
            .and_then(|document| document.get("uri"))
            .and_then(Value::as_str)
        else {
            return;
        };
        self.documents.remove(uri);
        // 关闭 ⇒ 清空诊断（LSP 约定：发空列表）。
        let params = format!(
            "{{\"uri\":{},\"diagnostics\":[]}}",
            json_string(uri)
        );
        outgoing.push(notification("textDocument/publishDiagnostics", params));
    }

    /// 诊断投影（`check_source` → `publishDiagnostics`）。
    ///
    /// 模式：URI 以 `.pyawa` 结尾或 `languageId == "pyawa"` ⇒ 扩展模式（语法＋类型），否则纯
    /// Python 模式（**只语法**，`MS-19` ✓）。LSP 的 `line`／`character` 是 **0 基**；本层给 1 基
    /// 诊断 ⇒ 减 1（无位点时 0）。**如实边界**：列按 `char` 计（非严格 UTF-16 code unit；纯 ASCII
    /// 等价，非 BMP 字符会偏），LSP 梯②之后的补齐面。
    fn publish(&self, outgoing: &mut Vec<String>, uri: &str, text: &str, language_id: &str) {
        let extension_mode = uri.ends_with(".pyawa") || language_id == "pyawa";
        let mode = if extension_mode {
            Mode::Extension
        } else {
            Mode::PurePython
        };
        let diagnostics = check_source(text, uri, mode);
        let mut items: Vec<String> = Vec::with_capacity(diagnostics.len());
        for diagnostic in &diagnostics {
            let (start_line, start_character) = if diagnostic.line > 0 {
                (
                    diagnostic.line - 1,
                    diagnostic.col.saturating_sub(1),
                )
            } else {
                (0, 0)
            };
            let end_line = start_line;
            let end_character = start_character.saturating_add(1);
            items.push(format!(
                "{{\"range\":{{\"start\":{{\"line\":{start_line},\"character\":{start_character}}},\
                 \"end\":{{\"line\":{end_line},\"character\":{end_character}}}}},\
                 \"severity\":1,\"code\":{},\"source\":\"pyawa\",\"message\":{}}}",
                json_string(&diagnostic.code),
                json_string(&diagnostic.message)
            ));
        }
        let params = format!(
            "{{\"uri\":{},\"diagnostics\":[{}]}}",
            json_string(uri),
            items.join(",")
        );
        outgoing.push(notification("textDocument/publishDiagnostics", params));
    }

    /// `textDocument/completion`：**文档内符号**补全（已标注的名字／`def`／`class`／顶层赋值）。
    ///
    /// v1 如实边界：不做上下文敏感／成员补全、也没接标准库；只从当前文档文本里搜集符号 ✓。
    fn completion(&self, params: Option<&Value>) -> String {
        let Some((uri, _line, _character)) = text_position(params) else {
            return "null".to_owned();
        };
        let Some(source) = self.documents.get(uri) else {
            return "null".to_owned();
        };
        let mut items: Vec<String> = Vec::new();
        for symbol in document_symbols(source) {
            let kind = match symbol.kind {
                SymbolKind::Function | SymbolKind::Class => 3, // CompletionItemKind::Function
                SymbolKind::Variable => 6,                    // CompletionItemKind::Variable
            };
            let detail = symbol
                .type_text
                .as_ref()
                .map(|type_text| format!(",\"detail\":{}", json_string(type_text)))
                .unwrap_or_default();
            items.push(format!(
                "{{\"label\":{},\"kind\":{kind}{detail}}}",
                json_string(&symbol.name)
            ));
        }
        format!("{{\"isIncomplete\":false,\"items\":[{}]}}", items.join(","))
    }

    /// `textDocument/definition`：光标下名字的**定义处**（注解／`def`／`class`／顶层赋值的第一处）。
    fn definition(&self, params: Option<&Value>) -> String {
        let Some((uri, line, character)) = text_position(params) else {
            return "null".to_owned();
        };
        let Some(source) = self.documents.get(uri) else {
            return "null".to_owned();
        };
        let Some(token) = token_at(source, line, character) else {
            return "null".to_owned();
        };
        let Some(symbol) = document_symbols(source)
            .into_iter()
            .find(|symbol| symbol.name == token)
        else {
            return "null".to_owned();
        };
        let start_character = symbol.character;
        let end_character = start_character + symbol.name.chars().count() as u32;
        format!(
            "{{\"uri\":{},\"range\":{{\"start\":{{\"line\":{},\"character\":{start_character}}},\
             \"end\":{{\"line\":{},\"character\":{end_character}}}}}}}",
            json_string(uri),
            symbol.line,
            symbol.line
        )
    }

    /// `textDocument/rename`：**单文件**整词重命名（返回 `WorkspaceEdit`，由客户端应用）。
    ///
    /// v1 如实边界：不做跨文件／作用域分析（同名单文件内全部替换）；名字必须是合法标识符。
    fn rename(&self, params: Option<&Value>) -> String {
        let Some(params) = params else {
            return "null".to_owned();
        };
        let Some((uri, line, character)) = text_position(Some(params)) else {
            return "null".to_owned();
        };
        let Some(new_name) = params.get("newName").and_then(Value::as_str) else {
            return "null".to_owned();
        };
        if !is_identifier(new_name) {
            return "null".to_owned();
        }
        let Some(source) = self.documents.get(uri) else {
            return "null".to_owned();
        };
        let Some(token) = token_at(source, line, character) else {
            return "null".to_owned();
        };
        let mut edits: Vec<String> = Vec::new();
        for (line_index, line_text) in source.lines().enumerate() {
            for (start, end) in word_occurrences(line_text, &token) {
                edits.push(format!(
                    "{{\"range\":{{\"start\":{{\"line\":{line_index},\"character\":{start}}},\
                     \"end\":{{\"line\":{line_index},\"character\":{end}}}}},\"newText\":{}}}",
                    json_string(new_name)
                ));
            }
        }
        if edits.is_empty() {
            return "null".to_owned();
        }
        format!(
            "{{\"changes\":{{{}}}}}",
            format!(
                "{}:[{}]",
                json_string(uri),
                edits.join(",")
            )
        )
    }

    /// `textDocument/hover`：对**已标注的名字**返回声明类型 markdown；找不到 ⇒ `null`。
    fn hover(&self, params: Option<&Value>) -> String {
        let Some(params) = params else {
            return "null".to_owned();
        };
        let Some(uri) = params
            .get("textDocument")
            .and_then(|document| document.get("uri"))
            .and_then(Value::as_str)
        else {
            return "null".to_owned();
        };
        let Some(position) = params.get("position") else {
            return "null".to_owned();
        };
        let (Some(line), Some(character)) = (
            position.get("line").and_then(Value::as_u32),
            position.get("character").and_then(Value::as_u32),
        ) else {
            return "null".to_owned();
        };
        let Some(source) = self.documents.get(uri) else {
            return "null".to_owned();
        };
        let Some(markdown) = hover_markdown(source, line, character) else {
            return "null".to_owned();
        };
        format!(
            "{{\"contents\":{{\"kind\":\"markdown\",\"value\":{}}}}}",
            json_string(&markdown)
        )
    }
}

fn initialize_result() -> String {
    // 只声明**已实现**的能力（②：全量同步＋hover；③：completion／definition／rename）。
    "{\"capabilities\":{\"textDocumentSync\":{\"openClose\":true,\"change\":1},\"hoverProvider\":true,\
     \"completionProvider\":{\"triggerCharacters\":[]},\"definitionProvider\":true,\
     \"renameProvider\":true},\
     \"serverInfo\":{\"name\":\"pyawa-lsp\",\"version\":\"0.0.0\"}}"
        .to_owned()
}

fn notification(method: &str, params: String) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":{},\"params\":{params}}}",
        json_string(method)
    )
}

fn respond(outgoing: &mut Vec<String>, id: Option<&Value>, result: String) {
    let id = id.map(Value::to_json).unwrap_or_else(|| "null".to_owned());
    outgoing.push(format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}"
    ));
}

fn respond_error(outgoing: &mut Vec<String>, id: Option<&Value>, code: i32, message: &str) {
    let id = id.map(Value::to_json).unwrap_or_else(|| "null".to_owned());
    outgoing.push(format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":{code},\"message\":{}}}}}",
        json_string(message)
    ));
}

fn json_string(text: &str) -> String {
    format!("\"{}\"", json::escape(text))
}

/// 从一行的 `名字: 类型` 注解里取出 `(名字, 类型文本, 名字起始 char 下标)`。
fn annotation_on_line(line: &str) -> Option<(String, String, usize)> {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut depth = 0i32;
    let mut colon = None;
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'"' => in_string = !in_string,
            b'#' if !in_string => break,
            b'(' | b'[' | b'{' if !in_string => depth += 1,
            b')' | b']' | b'}' if !in_string => depth -= 1,
            b':' if !in_string && depth == 0 => {
                colon = Some(index);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let before = &line[..colon];
    let name_end = before
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map(|(index, character)| index + character.len_utf8())
        .unwrap_or(0);
    let name = before[name_end..].trim();
    if name.is_empty() || !name.chars().all(|character| character.is_alphanumeric() || character == '_') {
        return None;
    }
    let name_start_char = line[..name_end].chars().count();
    let after = &line[colon + 1..];
    let type_end = after
        .find(['=', '#', ','])
        .unwrap_or(after.len());
    let type_text = after[..type_end].trim();
    if type_text.is_empty() {
        return None;
    }
    Some((name.to_owned(), type_text.to_owned(), name_start_char))
}

/// 某位置上的标识符（近似按 char 计；见 `publish` 的如实边界）。
fn identifier_at(line: &str, character: usize) -> Option<String> {
    let characters: Vec<char> = line.chars().collect();
    let is_identifier =
        |character: char| character.is_alphanumeric() || character == '_';
    let index = character.min(characters.len().saturating_sub(1));
    let mut start = index;
    while start > 0 && is_identifier(characters[start - 1]) {
        start -= 1;
    }
    let mut end = index;
    while end < characters.len() && is_identifier(characters[end]) {
        end += 1;
    }
    if start == end && index > 0 && is_identifier(characters[index - 1]) {
        start = index - 1;
        while start > 0 && is_identifier(characters[start - 1]) {
            start -= 1;
        }
        end = index;
    }
    if start < end {
        Some(characters[start..end].iter().collect())
    } else {
        None
    }
}

/// `hover` 的 markdown：优先本行注解；否则在全文里按光标下的名字找注解。
fn hover_markdown(source: &str, line: u32, character: u32) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let line_text = lines.get(line as usize)?;
    let token = identifier_at(line_text, character as usize);
    if let Some((name, type_text, name_start)) = annotation_on_line(line_text) {
        let on_name = token.as_deref() == Some(name.as_str());
        if on_name || (character as usize) >= name_start {
            return Some(hover_text(&name, &type_text));
        }
    }
    let token = token?;
    for candidate in &lines {
        if let Some((name, type_text, _)) = annotation_on_line(candidate) {
            if name == token {
                return Some(hover_text(&name, &type_text));
            }
        }
    }
    None
}

fn hover_text(name: &str, type_text: &str) -> String {
    format!("```python\n{name}: {type_text}\n```")
}


/// 光标处位置（`uri`／`line`／`character`；`params.textDocument.uri` ＋ `params.position`）。
fn text_position(params: Option<&Value>) -> Option<(&str, u32, u32)> {
    let params = params?;
    let uri = params
        .get("textDocument")
        .and_then(|document| document.get("uri"))
        .and_then(Value::as_str)?;
    let position = params.get("position")?;
    let line = position.get("line").and_then(Value::as_u32)?;
    let character = position.get("character").and_then(Value::as_u32)?;
    Some((uri, line, character))
}

/// 源码里的一类符号（补全／跳转的最小来源；**不做**语义分析）。
struct DocumentSymbol {
    name: String,
    kind: SymbolKind,
    line: u32,
    character: u32,
    type_text: Option<String>,
}

enum SymbolKind {
    Function,
    Class,
    Variable,
}

/// 从文档文本里搜集符号：注解（`name: type`）、`def`／`class`、顶层赋值（`name = …`）。
fn document_symbols(source: &str) -> Vec<DocumentSymbol> {
    let mut symbols: Vec<DocumentSymbol> = Vec::new();
    for (line_index, line_text) in source.lines().enumerate() {
        let line = line_index as u32;
        let trimmed = line_text.trim_start();
        let indent = line_text.len() - trimmed.len();
        if let Some((name, type_text, name_start)) = annotation_on_line(line_text) {
            push_symbol(
                &mut symbols,
                DocumentSymbol {
                    name,
                    kind: SymbolKind::Variable,
                    line,
                    character: (name_start) as u32,
                    type_text: Some(type_text),
                },
            );
            continue;
        }
        for (keyword, kind) in [("def ", SymbolKind::Function), ("class ", SymbolKind::Class)] {
            if let Some(rest) = trimmed.strip_prefix(keyword) {
                let name: String = rest
                    .chars()
                    .take_while(|character| character.is_alphanumeric() || *character == '_')
                    .collect();
                if !name.is_empty() {
                    push_symbol(
                        &mut symbols,
                        DocumentSymbol {
                            name,
                            kind,
                            line,
                            character: (indent + keyword.chars().count()) as u32,
                            type_text: None,
                        },
                    );
                }
                break;
            }
        }
        // 顶层赋值（缩进为 0；`name = …`）——定义跳转的兜底。
        if indent == 0 {
            if let Some((name, _value)) = trimmed.split_once('=') {
                let name = name.trim();
                if is_identifier(name) {
                    push_symbol(
                        &mut symbols,
                        DocumentSymbol {
                            name: name.to_owned(),
                            kind: SymbolKind::Variable,
                            line,
                            character: 0,
                            type_text: None,
                        },
                    );
                }
            }
        }
    }
    symbols
}

fn push_symbol(symbols: &mut Vec<DocumentSymbol>, symbol: DocumentSymbol) {
    if symbols.iter().any(|existing| existing.name == symbol.name) {
        return;
    }
    symbols.push(symbol);
}

/// 光标下的标识符（`source` 的 `line` 行 `character` 列）。
fn token_at(source: &str, line: u32, character: u32) -> Option<String> {
    let line_text = source.lines().nth(line as usize)?;
    identifier_at(line_text, character as usize)
}

fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_')
        && characters.all(|character| character.is_alphanumeric() || character == '_')
}

/// 一行里某标识符的**整词**出现区间（char 下标）——重命名用。
fn word_occurrences(line: &str, word: &str) -> Vec<(u32, u32)> {
    let characters: Vec<char> = line.chars().collect();
    let word_chars: Vec<char> = word.chars().collect();
    let mut occurrences = Vec::new();
    if word_chars.is_empty() || characters.len() < word_chars.len() {
        return occurrences;
    }
    let is_identifier_character =
        |character: char| character.is_alphanumeric() || character == '_';
    for start in 0..=characters.len() - word_chars.len() {
        let end = start + word_chars.len();
        if characters[start..end] != word_chars[..] {
            continue;
        }
        if start > 0 && is_identifier_character(characters[start - 1]) {
            continue;
        }
        if end < characters.len() && is_identifier_character(characters[end]) {
            continue;
        }
        occurrences.push((start as u32, end as u32));
    }
    occurrences
}
