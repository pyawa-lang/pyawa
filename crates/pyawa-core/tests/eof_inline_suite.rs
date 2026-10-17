//! **行内 suite ＋ 源文件末尾无换行 ⇒ 正常收尾，绝不 panic**（真 bug 修 ✓）。
//!
//! 实测原形 ✓：
//! ```text
//! printf 'for i in range(2): print(i)' > p.py && pyawa p.py
//! ⇒ panicked at crates/pyawa-core/src/compile/parser.rs:1814:43:
//!   index out of bounds: the len is 13 but the index is 13   (rc=101)
//! ```
//! 参照 `python3` 正常输出 `0`／`1` ✓ ⇒ **硬验收**就是这条命令输出 `0`／`1` 且 `rc=0` ✓。
//!
//! **根因**（`parse_statements` ＋ 四处行内体收尾 ✓）：词法层在文件尾发 `End` ✓，但
//! `for`／`while`／`def`／`class` 的行内体收尾把收尾词当"行尾换行"**无条件**多前移一格 ✗ ⇒
//! 末尾无换行时游标越过 `End` ✗ ⇒ ① `other` 臂**裸索引** `lexed.spans[*cursor]` panic ✗；
//! ② `parse_module` 报"模块结尾多出了 None" ✗。
//! 修法（两半同一逻辑改动 ✓）：① `parse_statements` 循环顶 EOF 正常收尾 ＋ `other` 臂跨度加下界保护 ✓；
//! ② 四处收尾**只在真停在 `Newline`（行内体）／`Dedent`（缩进体）时才前移** ✓，且**行内空体必须报错** ✓
//! （`if` 走 `parse_body_after_colon`，同补空体检查 ✓）。
//!
//! **为什么不走 conformance 夹具**：夹具／对拍 harness 会先给源码**补末尾换行** ✗ ⇒ 这一格在
//! corpus 里**测不到** ✓ ⇒ 必须在这里直接调编译入口 ✓（`crates/pyawa-core/tests/` ✓）。

use pyawa_core::compile::{compile, CheckTier, CompileError, Mode};

/// 调编译入口并**把 panic 变成测试失败** ✓：`Ok`＝编译成功 ✓、`Err`＝普通 `CompileError` ✓。
fn compile_eof(source: &str) -> Result<(), CompileError> {
    assert!(!source.ends_with('\n'), "这条用例的要点就是**末尾无换行**：{source:?}");
    let outcome = std::panic::catch_unwind(|| {
        compile(source, "<eof>", Mode::PurePython, CheckTier::Shallow, 0)
    });
    match outcome {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(error),
        Err(_) => panic!("编译入口 panic 了（必须正常收尾）\n源码：{source:?}"),
    }
}

/// ① 合法：五种行内体 ＋ `if` 对照，**末尾无换行也照常编译** ✓（参照同样接受 ✓）。
const INLINE_SUITES: &[(&str, &str)] = &[
    ("for_inline", "for i in range(2): print(i)"),
    ("while_inline", "while False: pass"),
    ("def_inline", "def f(): pass"),
    ("class_inline", "class C: pass"),
    ("async_def_inline", "async def f(): pass"),
    ("if_inline_control", "if True: pass"),
];

#[test]
fn inline_suite_at_eof_compiles() {
    for (name, source) in INLINE_SUITES {
        let result = compile_eof(source);
        assert!(
            result.is_ok(),
            "{name}：末尾无换行的行内体应编译成功（参照接受），实际 {result:?}\n源码：{source:?}"
        );
    }
}

/// ② 空体：冒号后直接文件尾 ⇒ 参照报 `IndentationError` 一族 ✓ ⇒ 我们给**普通 `CompileError`** ✓，
/// **不许静默编成空体** ✗（那才是"只让语法过却丢语义" ✓）。
const EMPTY_SUITES: &[(&str, &str)] = &[
    ("for_empty", "for i in range(2):"),
    ("while_empty", "while False:"),
    ("def_empty", "def f():"),
    ("class_empty", "class C:"),
    ("if_empty", "if True:"),
];

#[test]
fn empty_inline_suite_at_eof_is_a_plain_readable_error() {
    for (name, source) in EMPTY_SUITES {
        match compile_eof(source) {
            Ok(()) => panic!("{name}：空体必须报普通 CompileError，不许静默接受\n源码：{source:?}"),
            Err(CompileError::Syntax(message)) | Err(CompileError::Unsupported(message)) => {
                assert!(
                    message.contains("缩进"),
                    "{name}：错误信息要可读（应指明\"体要缩进\"），实际 {message:?}"
                );
            }
        }
    }
}
