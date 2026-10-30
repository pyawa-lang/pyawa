//! **P3-38 S2b**：源码里的孤立代理转义（`'\ud800'`／`'a\uDC80'`）能编译成 WTF-8 常量；
//! **合法 UTF-8 字面量仍走旧的 `Constant::Str`**（编译产物/夹具零变化 ✓）；`.pyac` 新增
//! tag 21 的 round-trip；f-string 字面段同样支持；非法转义照旧是语法错。
//!
//! 运行期语义（`len`／`repr`／`==`／下标等）在 `tests/str_wtf8.rs` 与 e2e 探针里钉 ✓。

use pyawa_core::compile::{compile, CheckTier, Constant, Mode};

fn compile_probe(source: &str) -> pyawa_core::compile::CompiledUnit {
    compile(source, "probe.py", Mode::PurePython, CheckTier::Shallow, 0).expect("应当编得过")
}

/// `a` + U+D800 + `b` 的 WTF-8 字节。
const LONE_SURROGATE_AB: [u8; 5] = [b'a', 0xED, 0xA0, 0x80, b'b'];

#[test]
fn surrogate_literal_becomes_str_wtf8_constant() {
    let unit = compile_probe("s = '\\ud800'\n");
    assert!(
        unit.constants
            .contains(&Constant::StrWtf8(vec![0xED, 0xA0, 0x80])),
        "`'\\ud800'` 应编成 WTF-8 常量：{:?}",
        unit.constants
    );
    assert!(
        !unit.constants.contains(&Constant::StrWtf8(vec![])),
        "不该有空字节常量"
    );
    // 混合字面量
    let unit = compile_probe("t = 'a\\ud800b'\n");
    assert!(unit
        .constants
        .contains(&Constant::StrWtf8(LONE_SURROGATE_AB.to_vec())));
    // `\uDC80`（surrogateescape 那一段的低代理）同样合法
    let unit = compile_probe("u = 'a\\uDC80'\n");
    assert!(unit
        .constants
        .contains(&Constant::StrWtf8(vec![b'a', 0xED, 0xB2, 0x80])));
}

#[test]
fn adjacent_literal_merging_happens_on_wtf8_bytes() {
    // `"x" "\ud800" "y"` ⇒ **一个** WTF-8 常量（隐式拼接不产生运行期拼接指令 ✓）
    let unit = compile_probe("v = \"x\" \"\\ud800\" \"y\"\n");
    assert!(unit
        .constants
        .contains(&Constant::StrWtf8(vec![b'x', 0xED, 0xA0, 0x80, b'y'])));
}

#[test]
fn legal_utf8_literals_keep_the_old_string_constant() {
    // 合法 UTF-8（含非 ASCII / 混合转义）⇒ 仍是 `Constant::Str(String)`（产物形状零变化 ✓）
    for (source, expected) in [
        ("s = 'abc'\n", "abc"),
        ("s = 'héllo'\n", "héllo"),
        ("s = '\\n\\t'\n", "\n\t"),
        ("s = 'compose'\n", "compose"),
    ] {
        let unit = compile_probe(source);
        assert!(
            unit.constants.contains(&Constant::Str(expected.to_owned())),
            "{source:?} 应给 Str({expected:?})：{:?}",
            unit.constants
        );
        // 不该出现任何 WTF-8 常量
        assert!(
            !unit
                .constants
                .iter()
                .any(|constant| matches!(constant, Constant::StrWtf8(_))),
            "{source:?} 不该有 StrWtf8"
        );
    }
}

#[test]
fn fstring_literal_segments_support_lone_surrogates() {
    // `f"p{t}q"` 里的字面段含代理 ⇒ 常量里出现 WTF-8 段（表达式段不变 ✓）
    let unit = compile_probe("x = 1\nr = f\"p\\ud800{x}\"\n");
    assert!(
        unit.constants
            .contains(&Constant::StrWtf8(vec![b'p', 0xED, 0xA0, 0x80])),
        "f-string 字面段应是 WTF-8 常量：{:?}",
        unit.constants
    );
    // 纯字面量的 f-string（含代理）会降级成普通字符串表达式 ✓
    let unit = compile_probe("r = f\"\\ud800\"\n");
    assert!(unit
        .constants
        .contains(&Constant::StrWtf8(vec![0xED, 0xA0, 0x80])));
}

#[test]
fn pyac_round_trips_the_new_constant_tag() {
    // `.pyac` 新 tag 21：长度 ＋ WTF-8 字节；往返后常量池完全相同 ✓
    let unit = compile_probe("s = '\\ud800'\nt = 'héllo'\n");
    let bytes = pyawa_core::pyac::encode_unit(&unit);
    let back = pyawa_core::pyac::decode_unit(&bytes).expect("代码段应解得开");
    assert_eq!(back.constants, unit.constants, "WTF-8 常量的 .pyac 往返");
    assert_eq!(back.code, unit.code);
    // 旧 tag（2 ＝ 普通 Str）不变 ✓
    assert!(unit
        .constants
        .iter()
        .any(|constant| matches!(constant, Constant::Str(_))));
}

#[test]
fn invalid_escapes_stay_syntax_errors() {
    // 未闭合／坏的 `\u` 十六进制／超界 `\U` 仍是语法错（不是静默接受 ✓）
    for source in [
        "s = '\\ud800\n",         // 未闭合
        "s = '\\uD80'\n",          // 十六进制不足 4 位
        "s = '\\U00110000'\n",     // 超出 U+10FFFF
    ] {
        assert!(
            compile(source, "probe.py", Mode::PurePython, CheckTier::Shallow, 0).is_err(),
            "{source:?} 应报语法错"
        );
    }
}
