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
