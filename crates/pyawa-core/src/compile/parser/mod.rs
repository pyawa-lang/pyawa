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
