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


/// 把一个**字符**按 `repr` 规则追加到 `out`（引号与转义规则见 [`quote_str`] 的说明 ✓）。
fn push_repr_character(out: &mut String, character: char, quote: char, ascii: bool) {
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

/// **`str` 的 `repr`**：`bytes` 是**已校验的 WTF-8**（P3-38 S2a-2）——
/// **孤立代理**按 CPython 口径转义成 `\udXXX`（结果仍是**合法 UTF-8** ✓）。
///
/// 合法 UTF-8 输入与旧的 [`quote_str`] **逐字节相同** ✓（后者现在只是本函数对
/// `text.as_bytes()` 的转调 ✓）。代理的转义口径照参照实测：
/// `repr('\ud800')` ⇒ `"'\\ud800'"`、`repr('a\ud800b')` ⇒ `"'a\\ud800b'"`。
pub(crate) fn quote_wtf8(bytes: &[u8], ascii: bool) -> String {
    // 引号选择与旧口径一致：内容里有 `'` 且没有 `"` ⇒ 用双引号 ✓（代理不参与引号判断 ✓）。
    let mut has_single = false;
    let mut has_double = false;
    let mut text = String::new();
    for point in crate::wtf8::code_points(bytes) {
        match point {
            Ok(code_point) if code_point == u32::from(b'\'') => has_single = true,
            Ok(code_point) if code_point == u32::from(b'"') => has_double = true,
            _ => {}
        }
    }
    let quote = if has_single && !has_double { '"' } else { '\'' };
    text.push(quote);
    for point in crate::wtf8::code_points(bytes) {
        match point {
            // **孤立代理**：`\udXXX`（小写十六进制，CPython 同形 ✓；ASCII 结果 ⇒ 仍是合法 UTF-8）
            Ok(code_point) if crate::wtf8::is_surrogate(code_point) => {
                text.push_str(&format!("\\u{code_point:04x}"));
            }
            Ok(code_point) => match char::from_u32(code_point) {
                Some(character) => push_repr_character(&mut text, character, quote, ascii),
                // 校验过的 WTF-8 不会走到这里（超界码点在 `encode`/`validate` 就拒了）⇒ 如实转义兜底
                None => text.push_str(&format!("\\U{code_point:08x}")),
            },
            // 同上：校验过的 WTF-8 不该有坏字节 ⇒ 替换符兜底（不 panic ✓）
            Err(_) => text.push(crate::wtf8::REPLACEMENT_CHARACTER),
        }
    }
    text.push(quote);
    text
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

