//! **`TS-46` 可空类型标注 `T?`**（扩展模式；等价于 `T | None`；纯 Python 模式必须 `SyntaxError`，`BC-15`）。
//!
//! 验收口径（任务书 ①-⑤）：
//! - 扩展模式下同一标注的 `T?` 与 `T | None` 两种写法**编译产物逐项等价**；
//! - 相邻规则两例：`list[int]?` ＝ `list[int] | None`、`list[int?]` ＝ `list[int | None]`；
//! - 类属性标注的 `__annotate__` 单元同样等价；
//! - 纯 Python 模式下两种写法都报 `CompileError::Syntax`；
//! - 不带 `?` 的既有源码两模式产物与改动前逐字节一致。
//!
//! **比较口径**：只比 `code` ＋ `constants` ＋ `names`（本文件把它们和同层其它"形状"字段一起
//! 投影成字符串，递归进嵌套 `Code` 常量）——**不比位置表**：`int?` 的位点终点落在 `?`、
//! `int | None` 落在 `None` 末尾，`BC-18` 的位点本来就不可能相同；`TS-46` 的等价只要求
//! 指令／常量／名字一致。

use pyawa_core::compile::{compile, CheckTier, CompileError, CompiledUnit, Constant, Mode};

/// 扩展模式编译（浅层档位、优化级 0：两者都不改发射，固定死以消掉无关变量）。
fn extension(source: &str) -> CompiledUnit {
    compile(source, "<t>", Mode::Extension, CheckTier::Shallow, 0)
        .unwrap_or_else(|error| panic!("扩展模式应当编得过 {source:?}，却报了 {error:?}"))
}

/// 纯 Python 模式编译（同上两个入参）。
fn pure_python(source: &str) -> Result<CompiledUnit, CompileError> {
    compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0)
}

/// **产物投影**：`code` ＋ `constants` ＋ `names`（另含同类"形状"字段），**不含位置表**。
///
/// 嵌套的 `Constant::Code`（函数／类体／`__annotate__` 单元）递归投影 —— 否则 `Debug` 会把
/// 它们的位置表也印出来，把"位置不可比"带进比较里。
fn project(unit: &CompiledUnit) -> String {
    format!(
        "unit{{name={:?},qualname={:?},flags={},argcount={},posonly={},kwonly={},nlocals={},\
         names={:?},varnames={:?},cellvars={:?},freevars={:?},demanded={:?},code={:?},\
         exceptiontable={:?},consts=[{}]}}",
        unit.name,
        unit.qualname,
        unit.flags,
        unit.argcount,
        unit.posonlyargcount,
        unit.kwonlyargcount,
        unit.nlocals,
        unit.names,
        unit.varnames,
        unit.cellvars,
        unit.freevars,
        unit.demanded,
        unit.code,
        unit.exceptiontable,
        unit.constants
            .iter()
            .map(project_constant)
            .collect::<Vec<_>>()
            .join(","),
    )
}

/// 常量投影：嵌套 `Code` 递归走 [`project`]，其余照 `Debug`（它们不带位置表）。
fn project_constant(constant: &Constant) -> String {
    match constant {
        Constant::Code(inner) => format!("Code({})", project(inner)),
        other => format!("{other:?}"),
    }
}

/// 递归找嵌套的 `__annotate__` 单元（类体 `__annotate_func__` 的那个常量）。
fn find_annotate(unit: &CompiledUnit) -> Option<&CompiledUnit> {
    for constant in &unit.constants {
        let Constant::Code(inner) = constant else {
            continue;
        };
        if inner.name == "__annotate__" {
            return Some(inner);
        }
        if let Some(found) = find_annotate(inner) {
            return Some(found);
        }
    }
    None
}

/// ① 扩展模式：`int?` 与 `int | None` 的编译产物等价（形参 ＋ 返回值两处都验）。
#[test]
fn nullable_annotation_equals_the_written_union() {
    let sugar = extension("def f(x: int?) -> int?: return x\n");
    let written = extension("def f(x: int | None) -> int | None: return x\n");
    assert_eq!(
        project(&sugar),
        project(&written),
        "`int?` 必须与 `int | None` 同码"
    );
    // 两种写法都必须真的产出了 `__annotate__` 单元（防"两边都没编注解"式的空转相等）。
    assert!(
        find_annotate(&sugar).is_some(),
        "函数注解应当进 `__annotate__` 单元"
    );
    // 非空转的反证：把联合的另一边换掉，投影必须变。
    let different = extension("def f(x: int) -> int?: return x\n");
    assert_ne!(project(&sugar), project(&different));
}

/// ② 相邻规则：`?` 只作用于**紧邻**的类型表达式。
#[test]
fn adjacent_rule_binds_the_suffix_to_the_nearest_type_expression() {    // `list[int]?` ＝ `list[int] | None`
    assert_eq!(
        project(&extension("def f(x: list[int]?) -> None: return x\n")),
        project(&extension("def f(x: list[int] | None) -> None: return x\n")),
        "`list[int]?` 必须等于 `list[int] | None`"
    );
    // `list[int?]` ＝ `list[int | None]`
    assert_eq!(
        project(&extension("def f(x: list[int?]) -> None: return x\n")),
        project(&extension("def f(x: list[int | None]) -> None: return x\n")),
        "`list[int?]` 必须等于 `list[int | None]`"
    );
    // 两例**不是一回事**（否则"相邻"这一条只对了一半也看不出来）。
    assert_ne!(
        project(&extension("def f(x: list[int]?) -> None: return x\n")),
        project(&extension("def f(x: list[int?]) -> None: return x\n")),
        "`list[int]?` 与 `list[int?]` 必须是两种不同的类型"
    );
}

/// ② 的补充：`|` 的**右操作数**也吃后缀（`A | B?`）。
///
/// **`TS-47` 起**变量／类属性标注也统一成**编译期类型标签** ⇒ `A | B?` 的 `?` 落在**紧邻的
/// `B`** 上，联合成员与手写 `A | B | None` **相同**（`|` 的结合方向只改变 `BINARY_OP` 的先后，
/// 运行期结果是同一个 `types.UnionType`；本测试按"成员相同 ＋ 指令数相同"验，不按逐字节同码）。
#[test]
fn union_right_operand_takes_the_nullable_suffix() {
    let sugar = extension("class C:\n    x: int | list[str]?\n");
    let written = extension("class C:\n    x: int | list[str] | None\n");
    let sugar_annotate = find_annotate(&sugar).expect("类体里应当有 `__annotate__` 单元");
    let written_annotate = find_annotate(&written).expect("类体里应当有 `__annotate__` 单元");
    // `?` 真的落到了 `B` 上（否则成员里会少一个 `None` ⇒ 指令数不同）。
    assert_ne!(
        project(&sugar),
        project(&extension("class C:\n    x: int | list[str]\n")),
        "`A | B?` 必须比 `A | B` 多一个联合成员"
    );
    assert_eq!(
        sugar_annotate.code.len(),
        written_annotate.code.len(),
        "`A | B?` 与 `A | B | None` 是同一个三成员联合（指令数相同）"
    );
    assert_eq!(
        sugar_annotate.names, written_annotate.names,
        "`A | B?` 与手写联合的成员名字必须相同"
    );
    // 类型位置（形参／返回值）：后缀确实落在 `B` 上（比 `A | B` 多一次 `BINARY_OP 7`）。
    let with_suffix = extension("def f(x: int | list[str]?) -> None: return x\n");
    let without = extension("def f(x: int | list[str]) -> None: return x\n");
    let with_annotate = find_annotate(&with_suffix).expect("应当有 `__annotate__` 单元");
    let without_annotate = find_annotate(&without).expect("应当有 `__annotate__` 单元");
    assert_ne!(project(&with_suffix), project(&without));
    assert!(
        with_annotate.code.len() > without_annotate.code.len(),
        "`A | B?` 应当比 `A | B` 多一条 `BINARY_OP 7`"
    );
}

/// 变量／类属性**赋值**形式的标注（`x: T? = …`）：同样按编译期类型标签收，等价。
#[test]
fn nullable_suffix_on_annotated_assignment() {
    let sugar = extension("class C:\n    xs: list[int]? = None\n");
    let written = extension("class C:\n    xs: list[int] | None = None\n");
    assert_eq!(
        project(&sugar),
        project(&written),
        "`xs: list[int]? = None` 必须与手写联合一致"
    );
    // 模块级与函数局部的标注位置同样能收（`TS-47` 起它们还登记进赋值检查表）。
    extension("x: int? = 1\n");
    extension("def g():\n    y: int? = 1\n    return y\n");
}

/// **`TS-47` 收口**：**嵌套**在变量／类属性标注里的 `?`（`x: list[int?]`）现在按类型标签解析
/// ⇒ 扩展模式编得过、且与手写 `list[int | None]` 等价；纯 Python 模式仍报 `Syntax`（`BC-15`）。
#[test]
fn nested_suffix_inside_variable_annotation_is_supported() {
    let sugar = extension("x: list[int?] = None\n");
    let written = extension("x: list[int | None] = None\n");
    assert_eq!(
        project(&sugar),
        project(&written),
        "`x: list[int?]` 必须与手写 `x: list[int | None]` 同码"
    );
    // 纯 Python 模式：`?` 根本不是合法词素（`BC-15`）——嵌套位置也一样。
    assert!(
        matches!(
            pure_python("x: list[int?] = None\n"),
            Err(CompileError::Syntax(_))
        ),
        "纯 Python 模式下嵌套位置同样必须报 `Syntax`"
    );
}

/// 扩展模式下 `?` 出现在**非标注位置**：如实报语法错，**不** panic。
///
/// 词法层现在在扩展模式下处处认得 `?`（同 `BC-63` 的 token 复用），所以这条守的是
/// "解析器遇到不该出现的 `?` 时给出错误而不是越界／猜语义"。
#[test]
fn question_outside_annotation_is_reported_not_a_panic() {
    for source in ["x = 1 ? 2\n", "x = 1 ?\n", "x: int?? = 1\n"] {
        let result = compile(source, "<t>", Mode::Extension, CheckTier::Shallow, 0);
        assert!(
            matches!(
                result,
                Err(CompileError::Syntax(_)) | Err(CompileError::Unsupported(_))
            ),
            "非标注位置的 `?` 应报错：{source:?} ⇒ {result:?}"
        );
    }
}

/// ③ 类属性标注：`next: Node?` 的 `__annotate__` 单元与 `next: Node | None` 等价。
#[test]
fn class_attribute_nullable_annotation_matches_the_written_union() {
    let sugar = extension("class Node:\n    next: Node?\n");
    let written = extension("class Node:\n    next: Node | None\n");
    assert_eq!(
        project(&sugar),
        project(&written),
        "类属性 `next: Node?` 的 `__annotate__` 单元必须与手写联合一致"
    );
    let annotate = find_annotate(&sugar).expect("类体里应当有 `__annotate__` 单元");
    assert!(!annotate.code.is_empty(), "`__annotate__` 单元不能是空体");
    // 非空转反证：换成别的类型，投影必须变。
    let different = extension("class Node:\n    next: Node | int\n");
    assert_ne!(project(&sugar), project(&different));
}

/// ④ 纯 Python 模式：`T?` 的三种标注位置都报 `CompileError::Syntax`（`BC-15`）。
#[test]
fn pure_python_rejects_the_nullable_suffix() {
    let sources = [
        // 变量标注
        "x: int? = 1\n",
        // 形参 ＋ 返回值标注
        "def f(x: int?) -> int?: return x\n",
        // 类属性标注
        "class Node:\n    next: Node?\n",
    ];
    for source in sources {
        let result = pure_python(source);
        assert!(
            matches!(result, Err(CompileError::Syntax(_))),
            "纯 Python 模式必须报 `Syntax`（BC-15）：{source:?} ⇒ {result:?}"
        );
        // 同一源码在扩展模式编得过（否则上面"被拒"可能只是别的原因）。
        let _ = extension(source);
    }
}

/// ⑤ 不带 `?` 的既有源码：两模式产物与改动前逐字节一致（拿 `x = 1` 的已知产物钉住）。
#[test]
fn sources_without_the_suffix_are_unchanged_in_both_modes() {
    let op = |name: &str| pyawa_core::opcode::opcode(name).expect("opcode 表里有") as u8;
    // 改动前 `x = 1` 的产物（模块：RESUME／LOAD_SMALL_INT 1／STORE_NAME 0／LOAD_CONST None／RETURN）
    let expected_code = vec![
        op("RESUME"),
        0,
        op("LOAD_SMALL_INT"),
        1,
        op("STORE_NAME"),
        0,
        op("LOAD_CONST"),
        1,
        op("RETURN_VALUE"),
        0,
    ];
    let pure_unit = pure_python("x = 1").expect("纯模式编得过");
    let extension_unit = extension("x = 1");
    assert_eq!(pure_unit.code, expected_code, "`x = 1` 的纯模式产物变了");
    assert_eq!(
        extension_unit.code, expected_code,
        "`x = 1` 的扩展模式产物变了"
    );
    assert_eq!(pure_unit.names, vec!["x".to_owned()]);
    assert_eq!(
        pure_unit.constants,
        vec![Constant::Int(1), Constant::None],
        "常量表变了"
    );
    assert_eq!(project(&pure_unit), project(&extension_unit));

    // 模式只影响 `?` 的词素识别 ＋ `TS-47` 的赋值检查发射 ⇒ **没有变量标注赋值**的源码
    // 两模式必须同产物。`x: int = 1` 不在此列：`TS-47` 起扩展模式会多一条 `CHECK_ASSIGNMENT`
    // （那是**有意的**代码生成差异，`BC-65`；发射面的验收在 `tests/assignment_emission.rs`）。
    let sources = [
        "x = 1\n",
        "def f(x: int | None) -> int: return x\n",
        "class C:\n    x: int | None\n",
    ];
    for source in sources {
        let pure = pure_python(source)
            .unwrap_or_else(|error| panic!("纯模式应当编得过 {source:?}，却报了 {error:?}"));
        assert_eq!(
            project(&pure),
            project(&extension(source)),
            "不带 `?` 的源码两模式产物必须相同：{source:?}"
        );
    }
}
