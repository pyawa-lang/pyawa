//! `compile` 的子模块：源码解析（拆分自单文件时期；按域拆成 `parser/` 目录）。
//!
//! 入口 [`parse_module`]；语句／表达式／类型／目标／模式各域在同一目录。

use super::*;

mod statements;
mod expressions;
mod atoms;
mod fstrings;
mod lambdas;
mod types;
mod targets;
mod patterns;

pub(crate) use self::statements::*;
pub(crate) use self::expressions::*;
pub(crate) use self::atoms::*;
pub(crate) use self::fstrings::*;
pub(crate) use self::lambdas::*;
pub(crate) use self::types::*;
pub(crate) use self::targets::*;
pub(crate) use self::patterns::*;

/// **硬关键字表**（CPython 3.14 的 `keyword.kwlist`；**不含**软关键字 `_`/`case`/`match`/`type` ✓）。
///
/// 词法器只把其中一部分给了独立 `Lexeme` 变体（`if`/`while`/…）；其余（`and`/`or`/`not`/`is`/
/// `as`/`elif`/`except`/`finally`/`lambda`/`True`/`False`/`None`）以 `Lexeme::Name` 形态流进
/// 解析器（**运算符/字面量位置**先被上层吃掉 ✓）。先前**名字位置**不查 ⇒ `exec("and = 42")`
/// 被当成合法赋值 ✗（`test_keyword.test_all_keywords_fail_to_be_used_as_names` 一条钉住 ✓）。
pub(crate) fn is_hard_keyword(name: &str) -> bool {
    matches!(
        name,
        "False" | "None" | "True" | "and" | "as" | "assert" | "async" | "await" | "break"
            | "class" | "continue" | "def" | "del" | "elif" | "else" | "except" | "finally"
            | "for" | "from" | "global" | "if" | "import" | "in" | "is" | "lambda" | "nonlocal"
            | "not" | "or" | "pass" | "raise" | "return" | "try" | "while" | "with" | "yield"
    )
}
