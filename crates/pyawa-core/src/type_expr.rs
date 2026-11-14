//! **TS 注解表达式的语法模型**（`SPEC-type-system.md` §16 的 `TS-52`…`TS-56`；2026-11-14 起）。
//!
//! 定位：把"注释里的类型表达式"从**源码常量形状**（`compile::Constant` 的 `Ann*` 变体，供字节码
//! 发射用）抽象成**与实例无关、可复用、可逆**的语法树。它是全口径「TS 注解解析器」的第一层：
//!
//! ```text
//! 注解字符串 ──词法+语法（复用 compile/parser/types.rs）──▶ TypeExpr（本文件）
//!                                                        │
//!                                                        ├─ to_source()：Pyawa 源形（可空/联合统一成
//!                                                        │              `A | B` 规范形；`T?` 已降级）
//!                                                        ├─ to_python_source()：纯 Python 源形
//!                                                        └─（M3）名称解析/求值 ──▶ 语义类型 / BC-24 标签
//! ```
//!
//! **`T?` 的表示**：`TS-46` 规定它等价于 `T | None` ⇒ 语法层**降级成联合**（同树、同语义）；
//! 回写时统一给规范形 `T | None`（`to_source`），不保留"这是 `?` 写的"这一来源信息。
//!
//! **本层不碰实例**：没有 `Instance`、没有类型对象解析、没有 `typing` 求值——同一段字符串在任何
//! 实例上都解析出**同一棵树**（可缓存、可对拍）。名称解析与求值属下一层（`TS-53`／`TS-54`）。
//!
//! 语法范围（`TS-52`）：名字（含点号）、`|` 联合、`?` 可空后缀（`TS-46`）、`X[实参…]`、
//! `[实参…]`（`Callable` 的形参表）、`...`、字符串前向引用。**任意表达式**（`1 + 2`、调用）不在
//! 本层：那是纯 Python 注解协议（`__annotate__`）的表达式通道，不是"类型表达式"。

use core::fmt;

/// 注解表达式的**语法树**（与实例无关；`TS-52`）。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeExpr {
    /// 显式 `Any`（`TS-16`：`Any` 只有"未标注"与"显式写 `Any`"两种来源，本层只表达后者）。
    Any,
    /// `None`（源码写作 `None`，语义上是 `NoneType` 的单例类型）。
    NoneType,
    /// 具名类型（未解析）：`int`、`Widget`。
    Name(String),
    /// 点号属性：`types.FunctionType`、`mymod.Widget`。
    Attribute {
        /// 基底表达式。
        base: Box<TypeExpr>,
        /// 属性名。
        name: String,
    },
    /// `T | U | …`（**平坦**存：`A | (B | C)` 与 `A | B | C` 同树）。
    Union(Vec<TypeExpr>),
    /// `X[实参…]`（泛型下标；`TS-30` 不变、`TS-35` 擦除等在**求值**层定义）。
    Subscript {
        /// 基底表达式。
        base: Box<TypeExpr>,
        /// 实参表（按源码顺序）。
        arguments: Vec<TypeExpr>,
    },
    /// 方括号实参表：`Callable[[int, str], None]` 的第一个实参。
    List(Vec<TypeExpr>),
    /// 字符串**前向引用**：`"X"`（延迟到求值期解析；`TS-53`）。
    ForwardRef(String),
    /// `...`（`Callable[..., int]` 与 `tuple[int, ...]` 的省略号）。
    Ellipsis,
}

impl TypeExpr {
    /// 把语法树渲染成 **Pyawa 源形**（`T?` 已降级为 `T | None` 的规范形；供显示/回写）。
    pub fn to_source(&self) -> String {
        match self {
            TypeExpr::Any => "Any".to_owned(),
            TypeExpr::NoneType => "None".to_owned(),
            TypeExpr::Name(name) => name.clone(),
            TypeExpr::Attribute { base, name } => format!("{}.{}", base.to_source(), name),
            TypeExpr::Union(items) => items
                .iter()
                .map(TypeExpr::to_source)
                .collect::<Vec<_>>()
                .join(" | "),
            TypeExpr::Subscript { base, arguments } => format!(
                "{}[{}]",
                base.to_source(),
                arguments
                    .iter()
                    .map(TypeExpr::to_source)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TypeExpr::List(items) => format!(
                "[{}]",
                items.iter().map(TypeExpr::to_source).collect::<Vec<_>>().join(", ")
            ),
            TypeExpr::ForwardRef(text) => quote_forward_ref(text),
            TypeExpr::Ellipsis => "...".to_owned(),
        }
    }

    /// 把语法树渲染成 **纯 Python 源形**（`T?` 展开成 `T | None`；`.pyi`／外部工具用）。
    pub fn to_python_source(&self) -> String {
        match self {
            TypeExpr::Any => "Any".to_owned(),
            TypeExpr::NoneType => "None".to_owned(),
            TypeExpr::Name(name) => name.clone(),
            TypeExpr::Attribute { base, name } => {
                format!("{}.{}", base.to_python_source(), name)
            }
            TypeExpr::Union(items) => items
                .iter()
                .map(TypeExpr::to_python_source)
                .collect::<Vec<_>>()
                .join(" | "),
            TypeExpr::Subscript { base, arguments } => format!(
                "{}[{}]",
                base.to_python_source(),
                arguments
                    .iter()
                    .map(TypeExpr::to_python_source)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TypeExpr::List(items) => format!(
                "[{}]",
                items.iter().map(TypeExpr::to_python_source).collect::<Vec<_>>().join(", ")
            ),
            TypeExpr::ForwardRef(text) => quote_forward_ref(text),
            TypeExpr::Ellipsis => "...".to_owned(),
        }
    }
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_source())
    }
}

/// 前向引用字符串的**规范引号形态**（双引号；内部双引号/反斜杠转义）。
fn quote_forward_ref(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
