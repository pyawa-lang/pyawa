//! **TS 注解表达式语法树**（`SPEC-type-system.md` §16；M1/M2，2026-11-14）。
//!
//! 钉住三件事：
//! 1. `parse_type_expression(&str)` 覆盖 `TS-52` 的语法范围（名字/点号/`|`/`?`/下标/列表/`...`/
//!    字符串前向引用），并把**源码常量形状**（`Constant::Ann*`）规范成语法树；
//! 2. `to_source()`／`to_python_source()` 的**可逆投影**（再解析回来得到同一棵树）；
//! 3. 非类型表达式／残缺输入 ⇒ `CompileError::Syntax`（不静默截断）。

use pyawa_core::compile::parse_type_expression;
use pyawa_core::TypeExpr as T;

fn parse(source: &str) -> T {
    parse_type_expression(source)
        .unwrap_or_else(|error| panic!("{source:?} 应当解析成功，实际 {error:?}"))
}

fn name(text: &str) -> T {
    T::Name(text.to_owned())
}

#[test]
fn parses_the_ts52_grammar() {
    assert_eq!(parse("int"), name("int"));
    assert_eq!(parse("Any"), T::Any);
    assert_eq!(parse("None"), T::NoneType);
    assert_eq!(parse("types.FunctionType"), T::Attribute {
        base: Box::new(name("types")),
        name: "FunctionType".to_owned(),
    });
    // `|` 的右结合形状被**摊平**（同树 ⇒ 同语义）。
    assert_eq!(
        parse("int | str | None"),
        T::Union(vec![name("int"), name("str"), T::NoneType])
    );
    // `TS-46` 的 `?` 等价 `T | None`。
    assert_eq!(
        parse("int?"),
        T::Union(vec![name("int"), T::NoneType])
    );
    // 单实参下标（源码常量里是 `Tuple([base, arg])`）与多实参下标都要规范化。
    assert_eq!(
        parse("list[int]"),
        T::Subscript {
            base: Box::new(name("list")),
            arguments: vec![name("int")],
        }
    );
    assert_eq!(
        parse("dict[str, int]"),
        T::Subscript {
            base: Box::new(name("dict")),
            arguments: vec![name("str"), name("int")],
        }
    );
    assert_eq!(
        parse("typing.Callable[[int, str], None]"),
        T::Subscript {
            base: Box::new(T::Attribute {
                base: Box::new(name("typing")),
                name: "Callable".to_owned(),
            }),
            arguments: vec![T::List(vec![name("int"), name("str")]), T::NoneType],
        }
    );
    assert_eq!(
        parse("tuple[int, ...]"),
        T::Subscript {
            base: Box::new(name("tuple")),
            arguments: vec![name("int"), T::Ellipsis],
        }
    );
    assert_eq!(parse("\"X\""), T::ForwardRef("X".to_owned()));
    // 前向引用里允许点号字符串（参照 `eval('"m.X"')` 语义，解析仍是一棵树）。
    assert_eq!(parse("\"mod.X\""), T::ForwardRef("mod.X".to_owned()));
}

#[test]
fn canonical_rendering_round_trips() {
    let cases = [
        "int",
        "Any",
        "None",
        "int | str | None",
        "list[int]",
        "dict[str, int]",
        "typing.Callable[[int, str], None]",
        "tuple[int, ...]",
        "\"X\"",
    ];
    for source in cases {
        let tree = parse(source);
        assert_eq!(tree.to_source(), source, "Pyawa 源形回写：{source}");
        // 再解析回来必须得**同一棵树**（可逆投影的核心判据）。
        assert_eq!(parse(&tree.to_source()), tree, "往返：{source}");
        assert_eq!(parse(&tree.to_python_source()), tree, "Python 形往返：{source}");
    }
    // `T?` 归一成 `T | None` 的规范形。
    assert_eq!(parse("int?").to_source(), "int | None");
}

#[test]
fn non_type_expressions_are_rejected_as_syntax_errors() {
    for source in [
        "",
        "   ",
        "1",
        "int + 2",
        "f(x)",
        "int |",
        "list[",
        "Callable[[int]",
        "int? ?",
        "\"unterminated",
    ] {
        let outcome = parse_type_expression(source);
        assert!(
            matches!(outcome, Err(pyawa_core::compile::CompileError::Syntax(_))),
            "{source:?} 应当是 Syntax 错误，实际 {outcome:?}"
        );
    }
}
