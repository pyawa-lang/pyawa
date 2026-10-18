//! `parser` 的 `types` 域：类型语法（`T?`／下标／PEP 695 类型参数）与注解来源判定。
//!
//! 从单文件 `parser.rs` **原样搬来**；父模块 `pub(crate) use` 再导出。

use super::*;

/// 解析**变量／类属性标注位置**的注解，返回「来源 ＋ 游标 ＋ 注解跨度」（`TS-47`／`TS-46`）。
///
/// 扩展模式下**先试**类型语法 [`parse_type_at`]：解析成功、且紧跟一个合法的"注解结束"词素
/// （`=`／语句结尾）⇒ [`AnnotationSource::Type`] —— 这份**编译期类型标签**既用于类体
/// `__annotate__` 的发射，也是 `TS-47` 运行期赋值检查（`CHECK_ASSIGNMENT`）的标签来源。
/// 这一条同时**收口** `TS-46` round 4 的留缺：嵌套在变量／类属性标注里的 `?`
/// （`x: list[int?]`）现在由 [`parse_type_at`] 自己吃掉。
///
/// 判不出类型（例如 `x: 1 + 2`／`x: (int)`）⇒ **回退**表达式通道 ⇒ `.pyawa` 仍接受 `.py`
/// 能接受的任意注解表达式（纯增量）。纯 Python 模式**永远**走表达式通道（`TS-5`）。
pub(crate) fn parse_annotation_source(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(AnnotationSource, usize, Span), CompileError> {
    if lexed.mode == Mode::Extension {
        if let Ok((label, after)) = parse_type_at(lexed, cursor) {
            // "注解结束"：`=`（带值形式）或语句结尾。别的词素 ⇒ 这段类型语法只是**前缀**，
            // 整条注解其实是更大的表达式 ⇒ 回退（不静默截断）。
            let terminated = matches!(
                lexed.lexemes.get(after),
                Some(Lexeme::Assign)
                    | Some(Lexeme::Newline)
                    | Some(Lexeme::Semicolon)
                    | Some(Lexeme::Dedent)
                    | Some(Lexeme::End)
            );
            if terminated && after > cursor {
                let span = lexed.spans[cursor].to(lexed.spans[after - 1]);
                return Ok((AnnotationSource::Type(label), after, span));
            }
        }
    }
    let (expression, after) = parse_annotation_expression(lexed, cursor)?;
    let span = expression.span();
    Ok((AnnotationSource::Expression(expression), after, span))
}

/// 解析**表达式形态**的注解（变量／类属性标注位置的兼容通道）。
///
/// `TS-46`：扩展模式下，表达式解析完**紧跟** `?` 时把整段包成 `T | None` —— **复用 `|` 表达式
/// 节点** ⇒ 与手写 `T | None` 同码。纯 Python 模式走不到这一支
/// （`?` 在词法期就报 `Syntax`，`BC-15` ✓），既有源码**逐字节不变** ✓。
pub(crate) fn parse_annotation_expression(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(Expression, usize), CompileError> {
    let (expression, cursor) = parse_expression(lexed, cursor)?;
    if lexed.mode == Mode::Extension && lexed.lexemes.get(cursor) == Some(&Lexeme::Question) {
        // 合成 `None` 的跨度取 `?` 那一格（只影响位置表；指令／常量／名字与手写 `T | None` 同）。
        let question_span = lexed.spans[cursor];
        let span = expression.span().to(question_span);
        return Ok((
            Expression::Binary(
                BinaryOperator::BitOr,
                Box::new(expression),
                Box::new(Expression::Constant(Constant::None, question_span)),
                span,
            ),
            cursor + 1,
        ));
    }
    Ok((expression, cursor))
}

/// 解析一个**注解类型**（`TS-31` 的边界标签；`BC-24` 的签名条目就是它）。
///
/// - 名字 ⇒ 类型标签（`Constant::Type`）：`int`／`str`／`list` 一类
/// - `Any` ⇒ 执行器认识的 `"Any"` 标签（`TS-28` 的双向相容）
/// - `None` ⇒ `NoneType`（`-> None` 的值就是 `None`；`NoneType` 在内建表里）
/// - `名字[内层]` ⇒ 复合标签 `(外类型, 内标签)`（深层档位按它递归，`TS-30` 的不变性落在外类型上）
pub(crate) fn parse_type_at(lexed: &Lexed, cursor: usize) -> Result<(Constant, usize), CompileError> {
    let (mut value, mut cursor) = parse_type_primary(lexed, cursor)?;
    // **`|` 联合**（PEP 604；第 287 轮）：参照发 `BINARY_OP 7`（`dis` 实测 `int | None` ✓）
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Pipe) {
        let (right, next) = parse_type_primary(lexed, cursor + 1)?;
        value = Constant::AnnUnion {
            left: Box::new(value),
            right: Box::new(right),
        };
        cursor = next;
    }
    Ok((value, cursor))
}

/// 注解里的**基本项** ＋ **可空后缀**（`TS-46`）：`T?` ⇒ `T | None`。
///
/// **相邻规则**靠"后缀只作用于紧邻的那一个类型表达式"实现：后缀加在 [`parse_type_primary_core`]
/// 整段之后 ⇒ `list[int]?` ＝ `list[int] | None`，而 `list[int?]` 里的 `?` 由实参表的
/// `parse_type_at` 自己吃掉 ⇒ `list[int | None]` ✓。`A | B?` 同样成立（`|` 的右操作数走这里）。
pub(crate) fn parse_type_primary(lexed: &Lexed, cursor: usize) -> Result<(Constant, usize), CompileError> {
    let (value, cursor) = parse_type_primary_core(lexed, cursor)?;
    // 后缀**只吃一个** `?`（`T??` 不在规格里 ⇒ 第二个 `?` 留给调用方报语法错，不猜语义）。
    if lexed.lexemes.get(cursor) == Some(&Lexeme::Question) {
        return Ok((
            Constant::AnnUnion {
                left: Box::new(value),
                right: Box::new(Constant::Type("NoneType".to_owned())),
            },
            cursor + 1,
        ));
    }
    Ok((value, cursor))
}

/// 注解里的**基本项**：名字（含 `None`／`Any`）／`...`／`[项表]`／`名字[实参表]` ✓。
pub(crate) fn parse_type_primary_core(lexed: &Lexed, cursor: usize) -> Result<(Constant, usize), CompileError> {
    let mut value = match lexed.lexemes.get(cursor) {
        Some(Lexeme::Name(name)) => {
            let base = match name.as_str() {
                "Any" => Constant::Str("Any".to_owned()),
                "None" => Constant::Type("NoneType".to_owned()),
                _ => Constant::Type(name.clone()),
            };
            base
        }
        // **前向引用**（`def f(a: "X")` ✓，第 287 轮）：发 `LOAD_CONST`（不是 `LOAD_GLOBAL` ✓）
        Some(Lexeme::Str(text)) => return Ok((Constant::AnnString(text.clone()), cursor + 1)),
        // `Callable[..., int]` 里的 `...` ✓（参照发 `LOAD_CONST Ellipsis` ✓）——
        // 词法层它是**三个 `Dot`** ✓（没有单独的省略号词素 ✓）。
        Some(Lexeme::Dot)
            if lexed.lexemes.get(cursor + 1) == Some(&Lexeme::Dot)
                && lexed.lexemes.get(cursor + 2) == Some(&Lexeme::Dot) =>
        {
            return Ok((Constant::Ellipsis, cursor + 3));
        }
        // `Callable[[int, str], None]` 里那个**列表** ✓（参照发 `BUILD_LIST n` ✓）
        Some(Lexeme::LeftBracket) => {
            let (items, next) = parse_type_arguments(lexed, cursor + 1)?;
            if lexed.lexemes.get(next) != Some(&Lexeme::RightBracket) {
                return Err(CompileError::Syntax("注解的 `[` 没有收尾 `]`".to_owned()));
            }
            return Ok((Constant::AnnList(items), next + 1));
        }
        other => {
            return Err(CompileError::Syntax(format!(
                "注解里要一个类型名，实际 {other:?}"
            )))
        }
    };
    let mut cursor = cursor + 1;
    // **点号**（`types.FunctionType` 一类 ✓，第 287 轮）：参照发 `LOAD_ATTR`（`dis` 实测 ✓）。
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Dot) {
        let Some(Lexeme::Name(attribute)) = lexed.lexemes.get(cursor + 1) else {
            return Err(CompileError::Syntax(format!(
                "注解里的 `.` 后面要名字，实际 {:?}",
                lexed.lexemes.get(cursor + 1)
            )));
        };
        value = Constant::AnnAttribute {
            base: Box::new(value),
            name: attribute.clone(),
        };
        cursor += 2;
    }
    // **下标**（`TS-31` 的复合标签）：`X[a]` ⇒ 一个实参（参照**不**发 `BUILD_TUPLE` ✓）、
    // `X[a, b, …]` ⇒ `BUILD_TUPLE n` ✓（第 287 轮把"只认一层、只认一个实参"的旧界线拆掉 ✓ ——
    // `Lib/test/support/__init__.py:729` 的 `dict[str, object] | None` 正是撞在这里 ✓）。
    if lexed.lexemes.get(cursor) == Some(&Lexeme::LeftBracket) {
        let (arguments, next) = parse_type_arguments(lexed, cursor + 1)?;
        if lexed.lexemes.get(next) != Some(&Lexeme::RightBracket) {
            return Err(CompileError::Syntax("注解的 `[` 没有收尾 `]`".to_owned()));
        }
        // **一个实参**沿用**旧的** `Tuple([外, 内])` 形状 ✓（第 287 轮）：边界检查那一套
        // （`CHECK_BOUNDARY_IN` 的标签 ✓ `TS-31` 的深层档位 ✓）就是按这个二元组读的 ✓ ——
        // 换成新形状会**动了已落地的语义** ✗（`tests/boundary.rs` 当场红 ✓）。
        // **两个及以上**才走新形状 ✓（旧代码在这里本来就报错 ✗ ⇒ 没有既有语义可动 ✓）。
        value = if arguments.len() == 1 && !matches!(arguments[0], Constant::AnnList(_)) {
            Constant::Tuple(vec![value, arguments.into_iter().next().expect("刚判过非空")])
        } else {
            Constant::AnnSubscript {
                base: Box::new(value),
                arguments,
            }
        };
        cursor = next + 1;
    }
    Ok((value, cursor))
}

/// 注解里的一串实参（逗号分隔，允许尾随逗号 ✓；空表给空 `Vec` ✓）。
pub(crate) fn parse_type_arguments(
    lexed: &Lexed,
    cursor: usize,
) -> Result<(Vec<Constant>, usize), CompileError> {
    let mut items: Vec<Constant> = Vec::new();
    let mut cursor = cursor;
    loop {
        if lexed.lexemes.get(cursor) == Some(&Lexeme::RightBracket) {
            return Ok((items, cursor));
        }
        let (item, next) = parse_type_at(lexed, cursor)?;
        items.push(item);
        cursor = next;
        match lexed.lexemes.get(cursor) {
            Some(Lexeme::Comma) => cursor += 1,
            _ => return Ok((items, cursor)),
        }
    }
}

/// **PEP 695 类型参数表**（task-24）：`[T, U]` ⇒ `[(名字, 名字 token 的跨度), …]`。
///
/// 只接**裸名字** ✓：`*Ts`（TypeVarTuple）／`**P`（ParamSpec）／`T: bound`／`T = default`
/// **如实报未接线** ✗（不静默丢掉 `[T]` 的语义 ✓）。当前位置不是 `[` ⇒ 返回空表（普通 `class`／`def` ✓）。
pub(crate) fn parse_type_params(
    lexed: &Lexed,
    cursor: &mut usize,
) -> Result<Vec<TypeParam>, CompileError> {
    if lexed.lexemes.get(*cursor) != Some(&Lexeme::LeftBracket) {
        return Ok(Vec::new());
    }
    *cursor += 1;
    let mut params: Vec<TypeParam> = Vec::new();
    loop {
        match lexed.lexemes.get(*cursor) {
            Some(Lexeme::RightBracket) => {
                *cursor += 1;
                break;
            }
            Some(Lexeme::Comma) => {
                *cursor += 1;
            }
            Some(Lexeme::Name(name)) => {
                let name = name.clone();
                let name_span = lexed.spans[*cursor];
                *cursor += 1;
                // **`[T: bound]`**（task-24 收口 ① ✓）：bound 表达式 —— 参照把它编成**惰性 thunk** ✓。
                let mut bound = None;
                if lexed.lexemes.get(*cursor) == Some(&Lexeme::Colon) {
                    let (expression, next) = parse_expression(lexed, *cursor + 1)?;
                    bound = Some(expression);
                    *cursor = next;
                }
                // **`[T = default]`**（同上 ✓）。
                let mut default = None;
                if lexed.lexemes.get(*cursor) == Some(&Lexeme::Assign) {
                    let (expression, next) = parse_value_expression(lexed, *cursor + 1)?;
                    default = Some(expression);
                    *cursor = next;
                }
                // **整条参数的跨度**（末项用 default，其次 bound，都没有就是名字本身 ✓）。
                let span = match (bound.as_ref(), default.as_ref()) {
                    (_, Some(value)) => name_span.to(value.span()),
                    (Some(value), None) => name_span.to(value.span()),
                    (None, None) => name_span,
                };
                params.push(TypeParam {
                    name,
                    name_span,
                    span,
                    bound,
                    default,
                });
            }
            Some(Lexeme::Star) => {
                return Err(CompileError::Unsupported(
                    "PEP 695 的 `*Ts`（TypeVarTuple）尚未接线".to_owned(),
                ))
            }
            Some(Lexeme::DoubleStar) => {
                return Err(CompileError::Unsupported(
                    "PEP 695 的 `**P`（ParamSpec）尚未接线".to_owned(),
                ))
            }
            other => {
                return Err(CompileError::Syntax(format!(
                    "类型参数表里出现 {other:?}（只接裸名字）"
                )))
            }
        }
    }
    Ok(params)
}

