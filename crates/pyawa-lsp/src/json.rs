//! **JSON 编解码**：`serde_json`（`CX-27` 允许第三方出现在本 crate：协议／序列化 ✓）。
//!
//! round 276 曾手写 JSON 解析／序列化；解析器的重复键、数字精度、嵌套深度等边界自己实现
//! 且无单元测试，round 281 起改用 `serde_json`（解析、转义、序列化都由它承担）。
//! 本模块只保留 LSP 层用到的极薄适配：解析、字符串转义、`u32` 视图。

pub use serde_json::Value;

/// 解析一段 JSON（UTF-8 已由调用方解好）。
pub fn parse(bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

/// 把 `Value` 序列化成 JSON 文本（发响应／通知时用）。
pub fn to_json(value: &Value) -> String {
    serde_json::to_string(value).expect("serde_json 序列化 Value 不会失败")
}

/// 把字符串编成**带引号的** JSON 字符串。
pub fn escape_quoted(text: &str) -> String {
    serde_json::to_string(text).expect("serde_json 序列化 str 不会失败")
}

/// 把字符串转义成 JSON 字符串**内容**（不含首尾引号）。
pub fn escape(text: &str) -> String {
    let quoted = escape_quoted(text);
    // `to_string` 的结果必然以 `"` 开头、以 `"` 结尾（各 1 字节）。
    quoted[1..quoted.len() - 1].to_owned()
}

/// 无符号 32 位整数视图（LSP 的 `id`／`line`／`character` 都是小整数）。
pub fn as_u32(value: &Value) -> Option<u32> {
    u32::try_from(value.as_u64()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lsp_shaped_message() {
        let value = parse(br#"{"jsonrpc":"2.0","id":1,"params":{"text":"x\ny"}}"#)
            .expect("合法 JSON");
        assert_eq!(value.get("jsonrpc").and_then(Value::as_str), Some("2.0"));
        assert_eq!(value.get("id").and_then(as_u32), Some(1));
        assert_eq!(
            value
                .get("params")
                .and_then(|params| params.get("text"))
                .and_then(Value::as_str),
            Some("x\ny")
        );
    }

    #[test]
    fn rejects_malformed_json() {
        for bad in [
            &b"{"[..],
            br#"{"a":}"#,
            br#"{"a" 1}"#,
            br#""unterminated"#,
            br#"{"a":1} trailing"#,
        ] {
            assert!(parse(bad).is_err(), "应拒绝：{:?}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn supports_unicode_escapes_and_surrogate_pairs() {
        let value = parse(br#""\ud83d\ude00 \u4e2d""#).expect("合法转义");
        assert_eq!(value.as_str(), Some("😀 中"));
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_stack_overflow() {
        // `serde_json` 默认递归上限 128：恶意深嵌套是 `Err`，不会打爆 Rust 栈。
        let deep = "[".repeat(10_000) + &"]".repeat(10_000);
        assert!(parse(deep.as_bytes()).is_err());
    }

    #[test]
    fn escape_round_trips_through_parse() {
        let text = "引号\" 反斜杠\\ 换行\n 制表\t 😀";
        let quoted = escape_quoted(text);
        assert_eq!(parse(quoted.as_bytes()).expect("合法 JSON").as_str(), Some(text));
    }
}
