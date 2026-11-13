//! **LSP MVP 端到端验收**（PLAN §10 梯②）：真起 `pyawa-lsp` 子进程，走 `Content-Length` 帧：
//! 初始化 → 打开错误 `.pyawa` ⇒ ≥2 条 `publishDiagnostics` → `didChange` 修正 ⇒ 0 条 →
//! `hover` 命中声明类型 → `.py` 只有语法类诊断 → `shutdown`／`exit` 干净退出。

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use pyawa_lsp::json::{self, Value};

struct Client {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
}

impl Client {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pyawa-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("起 pyawa-lsp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let Some(frame) = read_frame(&mut reader) else {
                    break;
                };
                let Ok(value) = json::parse(frame.as_bytes()) else {
                    break;
                };
                if sender.send(value).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin,
            messages: receiver,
        }
    }

    fn send(&mut self, body: &str) {
        write!(
            self.stdin,
            "Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("写 LSP 帧");
        self.stdin.flush().expect("flush");
    }

    fn recv(&self) -> Value {
        self.messages
            .recv_timeout(Duration::from_secs(10))
            .expect("等 LSP 消息超时")
    }

    /// 等一条通知（`method` 匹配）。
    fn wait_notification(&self, method: &str) -> Value {
        loop {
            let message = self.recv();
            if message.get("method").and_then(Value::as_str) == Some(method) {
                return message;
            }
        }
    }

    /// 发请求并等 `id` 对应的响应。
    fn request(&mut self, id: i64, method: &str, params: &str) -> Value {
        self.send(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":{},\"params\":{params}}}",
            json::escape_quoted(method)
        ));
        loop {
            let message = self.recv();
            if message.get("id").and_then(Value::as_u32) == Some(id as u32) {
                return message;
            }
        }
    }

    fn notify(&mut self, method: &str, params: &str) {
        self.send(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":{},\"params\":{params}}}",
            json::escape_quoted(method)
        ));
    }

    fn shutdown(mut self) {
        self.request(999, "shutdown", "null");
        self.notify("exit", "null");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return;
                }
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_frame(reader: &mut impl BufRead) -> Option<String> {
    let mut length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            length = rest.trim().parse().ok();
        }
    }
    let mut body = vec![0u8; length?];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

fn diagnostics_of(message: &Value) -> &Vec<Value> {
    match message
        .get("params")
        .and_then(|params| params.get("diagnostics"))
    {
        Some(Value::Array(items)) => items,
        _ => panic!("publishDiagnostics 缺 diagnostics 数组：{message:?}"),
    }
}

#[test]
fn diagnostics_and_hover_end_to_end() {
    let mut client = Client::start();
    let initialized = client.request(
        1,
        "initialize",
        "{\"processId\":null,\"rootUri\":null,\"capabilities\":{}}",
    );
    let result = initialized.get("result").expect("initialize 有 result");
    let capabilities = result.get("capabilities").expect("capabilities");
    assert_eq!(
        capabilities.get("hoverProvider").and_then(Value::as_bool),
        Some(true),
        "应声明 hoverProvider"
    );
    let sync = capabilities.get("textDocumentSync").expect("textDocumentSync");
    assert_eq!(sync.get("change").and_then(Value::as_u32), Some(1), "全量同步");
    client.notify("initialized", "{}");

    // 打开一个故意写错的 `.pyawa` ⇒ 一次 ≥2 条诊断。
    let broken = "{\"textDocument\":{\"uri\":\"file:///tmp/pyawa-lsp-demo.pyawa\",\
                  \"languageId\":\"pyawa\",\"version\":1,\
                  \"text\":\"a: int = \\\"x\\\"\\nb: str = 1\\n\"}}";
    client.notify("textDocument/didOpen", broken);
    let published = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(
        published
            .get("params")
            .and_then(|params| params.get("uri"))
            .and_then(Value::as_str),
        Some("file:///tmp/pyawa-lsp-demo.pyawa")
    );
    let diagnostics = diagnostics_of(&published);
    assert_eq!(diagnostics.len(), 2, "应一次报 2 条：{diagnostics:?}");
    let first = &diagnostics[0];
    assert_eq!(first.get("severity").and_then(Value::as_u32), Some(1));
    assert_eq!(first.get("code").and_then(Value::as_str), Some("type"));
    assert_eq!(first.get("source").and_then(Value::as_str), Some("pyawa"));
    let start_line = first
        .get("range")
        .and_then(|range| range.get("start"))
        .and_then(|start| start.get("line"))
        .and_then(Value::as_u32);
    assert_eq!(start_line, Some(0), "LSP 行是 0 基：{first:?}");

    // 全量 didChange 修正后 ⇒ 0 条诊断。
    client.notify(
        "textDocument/didChange",
        "{\"textDocument\":{\"uri\":\"file:///tmp/pyawa-lsp-demo.pyawa\",\"version\":2},\
          \"contentChanges\":[{\"text\":\"ho: int = 0\\nother: str = \\\"x\\\"\\n\"}]}",
    );
    let published = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(diagnostics_of(&published).len(), 0, "修正后应 0 条");

    // hover 命中已标注名字 ⇒ markdown 里有声明类型。
    let hovered = client.request(
        2,
        "textDocument/hover",
        "{\"textDocument\":{\"uri\":\"file:///tmp/pyawa-lsp-demo.pyawa\"},\
          \"position\":{\"line\":0,\"character\":1}}",
    );
    let value = hovered
        .get("result")
        .and_then(|result| result.get("contents"))
        .and_then(|contents| contents.get("value"))
        .and_then(Value::as_str)
        .expect("hover 应有 markdown");
    assert!(value.contains("ho: int"), "hover 内容：{value}");
    // 找不到时 ⇒ null。
    let missed = client.request(
        3,
        "textDocument/hover",
        "{\"textDocument\":{\"uri\":\"file:///tmp/pyawa-lsp-demo.pyawa\"},\
          \"position\":{\"line\":9,\"character\":0}}",
    );
    assert_eq!(missed.get("result"), Some(&Value::Null));

    // didClose ⇒ 清空诊断。
    client.notify(
        "textDocument/didClose",
        "{\"textDocument\":{\"uri\":\"file:///tmp/pyawa-lsp-demo.pyawa\"}}",
    );
    let cleared = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(diagnostics_of(&cleared).len(), 0);

    client.shutdown();
}

#[test]
fn python_files_get_only_syntax_diagnostics() {
    let mut client = Client::start();
    client.request(1, "initialize", "{\"capabilities\":{}}");
    client.notify("initialized", "{}");
    // `.py` 里同样的错误标注 ⇒ **0 条类型诊断**（`MS-19`）。
    client.notify(
        "textDocument/didOpen",
        "{\"textDocument\":{\"uri\":\"file:///tmp/pyawa-lsp-demo.py\",\
          \"languageId\":\"python\",\"version\":1,\
          \"text\":\"a: int = \\\"x\\\"\\n\"}}",
    );
    let published = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(diagnostics_of(&published).len(), 0, "`.py` 不该有类型诊断");
    // 语法错仍报（`.py` 只语法类）。
    client.notify(
        "textDocument/didChange",
        "{\"textDocument\":{\"uri\":\"file:///tmp/pyawa-lsp-demo.py\",\"version\":2},\
          \"contentChanges\":[{\"text\":\"def broken(:\\n    pass\\n\"}]}",
    );
    let published = client.wait_notification("textDocument/publishDiagnostics");
    let diagnostics = diagnostics_of(&published);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].get("code").and_then(Value::as_str),
        Some("syntax")
    );
    client.shutdown();
}

#[test]
fn unknown_method_gets_a_jsonrpc_error() {
    let mut client = Client::start();
    client.request(1, "initialize", "{\"capabilities\":{}}");
    let response = client.request(2, "foo/bar", "null");
    let code = response
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(|code| match code {
            Value::Number(number) => Some(*number as i32),
            _ => None,
        });
    assert_eq!(code, Some(-32601), "{response:?}");
    client.shutdown();
}

#[test]
fn completion_definition_and_rename() {
    let mut client = Client::start();
    let initialized = client.request(1, "initialize", "{\"capabilities\":{}}");
    let capabilities = initialized
        .get("result")
        .and_then(|result| result.get("capabilities"))
        .expect("capabilities");
    assert!(capabilities.get("completionProvider").is_some());
    assert_eq!(
        capabilities.get("definitionProvider").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        capabilities.get("renameProvider").and_then(Value::as_bool),
        Some(true)
    );
    client.notify("initialized", "{}");
    let uri = "file:///tmp/pyawa-lsp-features.pyawa";
    client.notify(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"pyawa\",\"version\":1,\
              \"text\":\"x: int = 1\\ny: str = \\\"a\\\"\\ndef f():\\n    return 1\\nz = x\\n\"}}}}"
        ),
    );
    // 打开时可能先来一条 publishDiagnostics（无误报则 0 条），消费掉。
    let _ = client.wait_notification("textDocument/publishDiagnostics");

    // 补全：文档内符号都在。
    let completion = client.request(
        2,
        "textDocument/completion",
        &format!("{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":4,\"character\":0}}}}"),
    );
    let items = match completion
        .get("result")
        .and_then(|result| result.get("items"))
    {
        Some(Value::Array(items)) => items,
        other => panic!("completion 结果不对：{other:?}"),
    };
    let labels: Vec<&str> = items
        .iter()
        .filter_map(|item| item.get("label").and_then(Value::as_str))
        .collect();
    for expected in ["x", "y", "f", "z"] {
        assert!(labels.contains(&expected), "缺符号 {expected}：{labels:?}");
    }

    // 定义：第 4 行的 `x` ⇒ 第 0 行注解处。
    let definition = client.request(
        3,
        "textDocument/definition",
        &format!("{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":4,\"character\":4}}}}"),
    );
    let result = definition.get("result").expect("definition result");
    assert_eq!(result.get("uri").and_then(Value::as_str), Some(uri));
    let line = result
        .get("range")
        .and_then(|range| range.get("start"))
        .and_then(|start| start.get("line"))
        .and_then(Value::as_u32);
    assert_eq!(line, Some(0), "应跳到注释（定义）行：{result:?}");

    // 重命名：`x` ⇒ `counter`，单文件两处（注解行 ＋ 使用行）。
    let rename = client.request(
        4,
        "textDocument/rename",
        &format!(
            "{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":4,\"character\":4}},\
              \"newName\":\"counter\"}}"
        ),
    );
    let edits = match rename
        .get("result")
        .and_then(|result| result.get("changes"))
        .and_then(|changes| changes.get(uri))
    {
        Some(Value::Array(edits)) => edits,
        other => panic!("rename 结果不对：{other:?}"),
    };
    assert_eq!(edits.len(), 2, "单文件两处整词替换：{edits:?}");
    for edit in edits {
        assert_eq!(
            edit.get("newText").and_then(Value::as_str),
            Some("counter")
        );
    }
    // 非法新名字 ⇒ null。
    let bad_rename = client.request(
        5,
        "textDocument/rename",
        &format!(
            "{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":4,\"character\":4}},\
              \"newName\":\"1bad\"}}"
        ),
    );
    assert_eq!(bad_rename.get("result"), Some(&Value::Null));
    client.shutdown();
}

#[test]
fn unicode_positions_use_utf16_code_units() {
    // 测试侧的 JSON 字符串拼装（避免手写多层转义）。
    fn quoted(text: &str) -> String {
        format!("\"{}\"", json::escape(text))
    }
    let mut client = Client::start();
    client.request(1, "initialize", "{\"capabilities\":{}}");
    client.notify("initialized", "{}");
    let uri = "file:///tmp/pyawa-lsp-unicode.pyawa";
    // 行 0 以字符串字面量 `"😀"` 开头（UTF-16 占 3 个 unit：引号+emoji+引号）：
    // `x` 在 **UTF-16 列 6**，而 char 下标是 5。
    let text = "\"😀\"; x: int = 1\ny = x\n";
    let open = format!(
        "{{\"textDocument\":{{\"uri\":{},\"languageId\":\"pyawa\",\"version\":1,\"text\":{}}}}}",
        quoted(uri),
        quoted(text)
    );
    client.notify("textDocument/didOpen", &open);
    let published = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(diagnostics_of(&published).len(), 0, "合法源不该有诊断");

    // definition：第 1 行的 `x`（LSP 列 4）⇒ 第 0 行注解，起始列必须是 **6**（UTF-16）。
    let definition = client.request(
        2,
        "textDocument/definition",
        &format!(
            "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":1,\"character\":4}}}}",
            quoted(uri)
        ),
    );
    let start = definition
        .get("result")
        .and_then(|result| result.get("range"))
        .and_then(|range| range.get("start"))
        .expect("definition start");
    assert_eq!(start.get("line").and_then(Value::as_u32), Some(0));
    assert_eq!(
        start.get("character").and_then(Value::as_u32),
        Some(6),
        "emoji 之后的名字在 UTF-16 里是第 6 列：{start:?}"
    );

    // rename：两处编辑的列也要按 UTF-16 报（0 行 6..7；1 行 4..5）。
    let rename = client.request(
        3,
        "textDocument/rename",
        &format!(
            "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":1,\"character\":4}},\"newName\":\"z\"}}",
            quoted(uri)
        ),
    );
    let edits = match rename
        .get("result")
        .and_then(|result| result.get("changes"))
        .and_then(|changes| changes.get(uri))
    {
        Some(Value::Array(edits)) => edits,
        other => panic!("rename 结果不对：{other:?}"),
    };
    assert_eq!(edits.len(), 2, "{edits:?}");
    let first_start = edits[0]
        .get("range")
        .and_then(|range| range.get("start"))
        .and_then(|start| start.get("character"))
        .and_then(Value::as_u32);
    assert_eq!(first_start, Some(6), "第一处 UTF-16 列：{edits:?}");
    let second_start = edits[1]
        .get("range")
        .and_then(|range| range.get("start"))
        .and_then(|start| start.get("character"))
        .and_then(Value::as_u32);
    assert_eq!(second_start, Some(4), "第二处 UTF-16 列：{edits:?}");

    // 诊断：emoji 后的 `a` 类型错，LSP 起始列同样应为 6。
    let broken = "\"😀\"; a: int = \"x\"\n";
    let change = format!(
        "{{\"textDocument\":{{\"uri\":{},\"version\":2}},\"contentChanges\":[{{\"text\":{}}}]}}",
        quoted(uri),
        quoted(broken)
    );
    client.notify("textDocument/didChange", &change);
    let published = client.wait_notification("textDocument/publishDiagnostics");
    let diagnostics = diagnostics_of(&published);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let start = diagnostics[0]
        .get("range")
        .and_then(|range| range.get("start"))
        .expect("diagnostic start");
    assert_eq!(start.get("line").and_then(Value::as_u32), Some(0));
    assert_eq!(
        start.get("character").and_then(Value::as_u32),
        Some(6),
        "诊断列应按 UTF-16 报：{start:?}"
    );
    client.shutdown();
}
