//! 字符串／字节串的引号与转义（`repr` 用；从 `instance.rs` 原样搬来）。

/// 字符串的引号形态（实测参照实现：能用单引号就用单引号；内容里有单引号而**没有**双引号时
/// 改用双引号）。`ascii` 为真时把非 ASCII 字符转义（`ascii()` 的语义）。
///
/// **临时**：`repr` 的完整规则属于类型自己的槽位（`OM-11`），接线后由那里说了算。
/// **`str` 的 `repr` 转义** ✓（task-19 第 2 轮真 bug ✗：先前只转 `\\ \n \r \t` ＋ 引号 ✓，
/// **其余控制字符一律原样吐** ✗ —— 实测 `repr('a\x0bb')` 给原始 VT ✓、参照给 `"'a\\x0bb'"` ✓）。
///
/// 参照口径（本机 `python3` 3.14 实测 ✓）：**不可打印**字符按码位宽度转义 ✓ ——
/// `< 0x100` ⇒ `\xNN` ✓（`\x80`／`\x85`／`\xa0` 都算 ✓）、`< 0x10000` ⇒ `\uNNNN` ✓
/// （`\u200b`／`\u2028`／`\u3000`／`\uffff` ✓）、其余 ⇒ `\UNNNNNNNN` ✓；
/// **可打印的非 ASCII 原样** ✓（`é`／`中`／`😀` ✓）；`ascii=true` 时非 ASCII **一律**转义 ✓。
fn str_character_is_printable(character: char) -> bool {
    if character == ' ' {
        return true;
    }
    if character.is_control() {
        // Cc（C0／C1 ✓）
        return false;
    }
    let code = character as u32;
    // **非字符**（每个平面末尾两个 ＋ `FDD0..=FDEF` ✓）也是"不可打印" ✓
    if (0xFDD0..=0xFDEF).contains(&code) || code & 0xFFFE == 0xFFFE {
        return false;
    }
    !matches!(
        code,
        // Cf（格式字符 ✓）
        0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x0890..=0x0891 | 0x08E2
            | 0x180E | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F
            | 0xFEFF | 0xFFF9..=0xFFFB | 0x110BD | 0x110CD | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F
            // Zs（空格除外 ✓）／Zl／Zp
            | 0x00A0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000
            // Co（私用区 ✓）
            | 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
    )
}


pub(crate) fn quote_str(text: &str, ascii: bool) -> String {
    let has_single = text.contains('\'');
    let has_double = text.contains('"');
    let quote = if has_single && !has_double { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for character in text.chars() {
        // **ASCII 也要看可打印性** ✓（`\x00`／`\x7f` 都得转义 ✓ —— 先前写成"ASCII 一律不转" ✗）；
        // `ascii=true` 只**额外**把可打印的非 ASCII 也转掉 ✓。
        let escape_code = if ascii && !character.is_ascii() {
            true
        } else {
            !str_character_is_printable(character)
        };
        match character {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if character == quote => {
                out.push('\\');
                out.push(character);
            }
            _ if escape_code => {
                let code = character as u32;
                if code <= 0xFF {
                    out.push_str(&format!("\\x{code:02x}"));
                } else if code <= 0xFFFF {
                    out.push_str(&format!("\\u{code:04x}"));
                } else {
                    out.push_str(&format!("\\U{code:08x}"));
                }
            }
            _ => out.push(character),
        }
    }
    out.push(quote);
    out
}


/// **`bytes` 的 `repr` 引号与转义**（`P1-12`；规则与 `str` 同源但按**字节**判断，照参照实测）：
/// 能用单引号就用单引号（内容有 `'` 而无 `"` 时改用双引号）；`\t`／`\n`／`\r`／`\\` 用转义；
/// 可打印 ASCII（`0x20..=0x7e`）原样；**其余一律** `\xNN`（含 `0x7f` 与所有高位字节——
/// 实测 `repr(b'caf\xc3\xa9') == "b'caf\\xc3\\xa9'"`，即使那是合法的 UTF-8）。
pub(crate) fn quote_bytes(value: &[u8]) -> String {
    let has_single = value.contains(&b'\'');
    let has_double = value.contains(&b'"');
    let quote = if has_single && !has_double { b'"' } else { b'\'' };
    let mut out = String::from("b");
    out.push(char::from(quote));
    for byte in value {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            _ if *byte == quote => {
                out.push('\\');
                out.push(char::from(*byte));
            }
            _ if (0x20..=0x7e).contains(byte) => out.push(char::from(*byte)),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out.push(char::from(quote));
    out
}

