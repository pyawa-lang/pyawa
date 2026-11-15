//! `_ast` 模块（第 402 轮）：**按参照的继承关系建类** ✓ ＋ 常量 ✓ ＋ 其余名字占位 ✓。
//!
//! 为什么需要它：`Lib/ast.py` 第 23 行 `from _ast import *` ✓，且 577-613 行把 `AST`／`mod`／
//! `expr_context` **当基类**用 ✓（`class Suite(mod)` 一类）⇒ 它们**必须是真类** ✗，占位函数不够 ✗。
//!
//! **如实偏差**：本层只保证 **导入期** 与 `isinstance` 一类的形状 ✓ —— AST 的**构造／解析**
//! （`_ast.parse`／`literal_eval` 一族）仍是**占位**，调用即如实报未接线 ✗，不静默给假值 ✓。
//! 名字、继承关系与常量值都取自**同版本参照**的 `dir(_ast)` ✓。

use core::ptr::NonNull;

use pyawa_core::{build_class_from_parts, Header, Instance, TypeObject};

/// 模块名（`_ast`）。
pub const NAME: &str = "_ast";

/// 类与它们的基类（**基类在前** ✓，共 126 个）。
const CLASSES: [(&str, &[&str]); 126] = [
    ("AST", &[]),
    ("operator", &["AST"]),
    ("Add", &["operator"]),
    ("boolop", &["AST"]),
    ("And", &["boolop"]),
    ("stmt", &["AST"]),
    ("AnnAssign", &["stmt"]),
    ("Assert", &["stmt"]),
    ("Assign", &["stmt"]),
    ("AsyncFor", &["stmt"]),
    ("AsyncFunctionDef", &["stmt"]),
    ("AsyncWith", &["stmt"]),
    ("expr", &["AST"]),
    ("Attribute", &["expr"]),
    ("AugAssign", &["stmt"]),
    ("Await", &["expr"]),
    ("BinOp", &["expr"]),
    ("BitAnd", &["operator"]),
    ("BitOr", &["operator"]),
    ("BitXor", &["operator"]),
    ("BoolOp", &["expr"]),
    ("Break", &["stmt"]),
    ("Call", &["expr"]),
    ("ClassDef", &["stmt"]),
    ("Compare", &["expr"]),
    ("Constant", &["expr"]),
    ("Continue", &["stmt"]),
    ("expr_context", &["AST"]),
    ("Del", &["expr_context"]),
    ("Delete", &["stmt"]),
    ("Dict", &["expr"]),
    ("DictComp", &["expr"]),
    ("Div", &["operator"]),
    ("cmpop", &["AST"]),
    ("Eq", &["cmpop"]),
    ("excepthandler", &["AST"]),
    ("ExceptHandler", &["excepthandler"]),
    ("Expr", &["stmt"]),
    ("mod", &["AST"]),
    ("Expression", &["mod"]),
    ("FloorDiv", &["operator"]),
    ("For", &["stmt"]),
    ("FormattedValue", &["expr"]),
    ("FunctionDef", &["stmt"]),
    ("FunctionType", &["mod"]),
    ("GeneratorExp", &["expr"]),
    ("Global", &["stmt"]),
    ("Gt", &["cmpop"]),
    ("GtE", &["cmpop"]),
    ("If", &["stmt"]),
    ("IfExp", &["expr"]),
    ("Import", &["stmt"]),
    ("ImportFrom", &["stmt"]),
    ("In", &["cmpop"]),
    ("Interactive", &["mod"]),
    ("Interpolation", &["expr"]),
    ("unaryop", &["AST"]),
    ("Invert", &["unaryop"]),
    ("Is", &["cmpop"]),
    ("IsNot", &["cmpop"]),
    ("JoinedStr", &["expr"]),
    ("LShift", &["operator"]),
    ("Lambda", &["expr"]),
    ("List", &["expr"]),
    ("ListComp", &["expr"]),
    ("Load", &["expr_context"]),
    ("Lt", &["cmpop"]),
    ("LtE", &["cmpop"]),
    ("MatMult", &["operator"]),
    ("Match", &["stmt"]),
    ("pattern", &["AST"]),
    ("MatchAs", &["pattern"]),
    ("MatchClass", &["pattern"]),
    ("MatchMapping", &["pattern"]),
    ("MatchOr", &["pattern"]),
    ("MatchSequence", &["pattern"]),
    ("MatchSingleton", &["pattern"]),
    ("MatchStar", &["pattern"]),
    ("MatchValue", &["pattern"]),
    ("Mod", &["operator"]),
    ("Module", &["mod"]),
    ("Mult", &["operator"]),
    ("Name", &["expr"]),
    ("NamedExpr", &["expr"]),
    ("Nonlocal", &["stmt"]),
    ("Not", &["unaryop"]),
    ("NotEq", &["cmpop"]),
    ("NotIn", &["cmpop"]),
    ("Or", &["boolop"]),
    ("type_param", &["AST"]),
    ("ParamSpec", &["type_param"]),
    ("Pass", &["stmt"]),
    ("Pow", &["operator"]),
    ("RShift", &["operator"]),
    ("Raise", &["stmt"]),
    ("Return", &["stmt"]),
    ("Set", &["expr"]),
    ("SetComp", &["expr"]),
    ("Slice", &["expr"]),
    ("Starred", &["expr"]),
    ("Store", &["expr_context"]),
    ("Sub", &["operator"]),
    ("Subscript", &["expr"]),
    ("TemplateStr", &["expr"]),
    ("Try", &["stmt"]),
    ("TryStar", &["stmt"]),
    ("Tuple", &["expr"]),
    ("TypeAlias", &["stmt"]),
    ("type_ignore", &["AST"]),
    ("TypeIgnore", &["type_ignore"]),
    ("TypeVar", &["type_param"]),
    ("TypeVarTuple", &["type_param"]),
    ("UAdd", &["unaryop"]),
    ("USub", &["unaryop"]),
    ("UnaryOp", &["expr"]),
    ("While", &["stmt"]),
    ("With", &["stmt"]),
    ("Yield", &["expr"]),
    ("YieldFrom", &["expr"]),
    ("alias", &["AST"]),
    ("arg", &["AST"]),
    ("arguments", &["AST"]),
    ("comprehension", &["AST"]),
    ("keyword", &["AST"]),
    ("match_case", &["AST"]),
    ("withitem", &["AST"]),
];

/// 整数常量（共 4 个）。
const CONSTS: [(&str, i64); 4] = [
    ("PyCF_ALLOW_TOP_LEVEL_AWAIT", 8192),
    ("PyCF_ONLY_AST", 1024),
    ("PyCF_OPTIMIZED_AST", 33792),
    ("PyCF_TYPE_COMMENTS", 4096),
];


/// **字段面**（第 407 轮补 ✓）：`_fields`／`_attributes`／`__match_args__` 取自**同版本参照** ✓
const FIELDS: [(&str, &[&str], &[&str], &[&str]); 86] = [
    ("AnnAssign", &["target", "annotation", "value", "simple"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["target", "annotation", "value", "simple"]),
    ("Assert", &["test", "msg"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["test", "msg"]),
    ("Assign", &["targets", "value", "type_comment"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["targets", "value", "type_comment"]),
    ("AsyncFor", &["target", "iter", "body", "orelse", "type_comment"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["target", "iter", "body", "orelse", "type_comment"]),
    ("AsyncFunctionDef", &["name", "args", "body", "decorator_list", "returns", "type_comment", "type_params"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name", "args", "body", "decorator_list", "returns", "type_comment", "type_params"]),
    ("AsyncWith", &["items", "body", "type_comment"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["items", "body", "type_comment"]),
    ("Attribute", &["value", "attr", "ctx"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value", "attr", "ctx"]),
    ("AugAssign", &["target", "op", "value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["target", "op", "value"]),
    ("Await", &["value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value"]),
    ("BinOp", &["left", "op", "right"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["left", "op", "right"]),
    ("BoolOp", &["op", "values"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["op", "values"]),
    ("Break", &[], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &[]),
    ("Call", &["func", "args", "keywords"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["func", "args", "keywords"]),
    ("ClassDef", &["name", "bases", "keywords", "body", "decorator_list", "type_params"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name", "bases", "keywords", "body", "decorator_list", "type_params"]),
    ("Compare", &["left", "ops", "comparators"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["left", "ops", "comparators"]),
    ("Constant", &["value", "kind"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value", "kind"]),
    ("Continue", &[], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &[]),
    ("Delete", &["targets"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["targets"]),
    ("Dict", &["keys", "values"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["keys", "values"]),
    ("DictComp", &["key", "value", "generators"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["key", "value", "generators"]),
    ("ExceptHandler", &["type", "name", "body"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["type", "name", "body"]),
    ("Expr", &["value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value"]),
    ("Expression", &["body"], &[], &["body"]),
    ("For", &["target", "iter", "body", "orelse", "type_comment"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["target", "iter", "body", "orelse", "type_comment"]),
    ("FormattedValue", &["value", "conversion", "format_spec"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value", "conversion", "format_spec"]),
    ("FunctionDef", &["name", "args", "body", "decorator_list", "returns", "type_comment", "type_params"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name", "args", "body", "decorator_list", "returns", "type_comment", "type_params"]),
    ("FunctionType", &["argtypes", "returns"], &[], &["argtypes", "returns"]),
    ("GeneratorExp", &["elt", "generators"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["elt", "generators"]),
    ("Global", &["names"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["names"]),
    ("If", &["test", "body", "orelse"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["test", "body", "orelse"]),
    ("IfExp", &["test", "body", "orelse"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["test", "body", "orelse"]),
    ("Import", &["names"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["names"]),
    ("ImportFrom", &["module", "names", "level"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["module", "names", "level"]),
    ("Interactive", &["body"], &[], &["body"]),
    ("Interpolation", &["value", "str", "conversion", "format_spec"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value", "str", "conversion", "format_spec"]),
    ("JoinedStr", &["values"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["values"]),
    ("Lambda", &["args", "body"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["args", "body"]),
    ("List", &["elts", "ctx"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["elts", "ctx"]),
    ("ListComp", &["elt", "generators"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["elt", "generators"]),
    ("Match", &["subject", "cases"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["subject", "cases"]),
    ("MatchAs", &["pattern", "name"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["pattern", "name"]),
    ("MatchClass", &["cls", "patterns", "kwd_attrs", "kwd_patterns"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["cls", "patterns", "kwd_attrs", "kwd_patterns"]),
    ("MatchMapping", &["keys", "patterns", "rest"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["keys", "patterns", "rest"]),
    ("MatchOr", &["patterns"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["patterns"]),
    ("MatchSequence", &["patterns"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["patterns"]),
    ("MatchSingleton", &["value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value"]),
    ("MatchStar", &["name"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name"]),
    ("MatchValue", &["value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value"]),
    ("Module", &["body", "type_ignores"], &[], &["body", "type_ignores"]),
    ("Name", &["id", "ctx"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["id", "ctx"]),
    ("NamedExpr", &["target", "value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["target", "value"]),
    ("Nonlocal", &["names"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["names"]),
    ("ParamSpec", &["name", "default_value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name", "default_value"]),
    ("Pass", &[], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &[]),
    ("Raise", &["exc", "cause"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["exc", "cause"]),
    ("Return", &["value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value"]),
    ("Set", &["elts"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["elts"]),
    ("SetComp", &["elt", "generators"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["elt", "generators"]),
    ("Slice", &["lower", "upper", "step"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["lower", "upper", "step"]),
    ("Starred", &["value", "ctx"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value", "ctx"]),
    ("Subscript", &["value", "slice", "ctx"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value", "slice", "ctx"]),
    ("TemplateStr", &["values"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["values"]),
    ("Try", &["body", "handlers", "orelse", "finalbody"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["body", "handlers", "orelse", "finalbody"]),
    ("TryStar", &["body", "handlers", "orelse", "finalbody"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["body", "handlers", "orelse", "finalbody"]),
    ("Tuple", &["elts", "ctx"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["elts", "ctx"]),
    ("TypeAlias", &["name", "type_params", "value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name", "type_params", "value"]),
    ("TypeIgnore", &["lineno", "tag"], &[], &["lineno", "tag"]),
    ("TypeVar", &["name", "bound", "default_value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name", "bound", "default_value"]),
    ("TypeVarTuple", &["name", "default_value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name", "default_value"]),
    ("UnaryOp", &["op", "operand"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["op", "operand"]),
    ("While", &["test", "body", "orelse"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["test", "body", "orelse"]),
    ("With", &["items", "body", "type_comment"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["items", "body", "type_comment"]),
    ("Yield", &["value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value"]),
    ("YieldFrom", &["value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["value"]),
    ("alias", &["name", "asname"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["name", "asname"]),
    ("arg", &["arg", "annotation", "type_comment"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["arg", "annotation", "type_comment"]),
    ("arguments", &["posonlyargs", "args", "vararg", "kwonlyargs", "kw_defaults", "kwarg", "defaults"], &[], &["posonlyargs", "args", "vararg", "kwonlyargs", "kw_defaults", "kwarg", "defaults"]),
    ("comprehension", &["target", "iter", "ifs", "is_async"], &[], &["target", "iter", "ifs", "is_async"]),
    ("excepthandler", &[], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &[]),
    ("expr", &[], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &[]),
    ("keyword", &["arg", "value"], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &["arg", "value"]),
    ("match_case", &["pattern", "guard", "body"], &[], &["pattern", "guard", "body"]),
    ("pattern", &[], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &[]),
    ("stmt", &[], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &[]),
    ("type_param", &[], &["lineno", "col_offset", "end_lineno", "end_col_offset"], &[]),
    ("withitem", &["context_expr", "optional_vars"], &[], &["context_expr", "optional_vars"]),
];

/// 建 `_ast` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, value) in CONSTS {
        let object = instance.new_int(value);
        instance.dict_set(namespace, name, object);
    }
    for (name, bases) in CLASSES {
        let mut base_objects: Vec<NonNull<Header>> = Vec::new();
        for base in bases.iter() {
            if let Some(base_type) = instance.type_named(base) {
                base_objects.push(base_type.cast::<Header>());
            }
        }
        // **字段面**（第 407 轮 ✓）：参照在 C 层给的 `_fields`／`_attributes`／`__match_args__`
        // 现在**如实补上** ✓（先前只有名字壳 ✗ ⇒ `node._fields` 直接 `AttributeError` ✗）。
        let class_namespace = instance.new_dict();
        if let Some((_, fields, attributes, match_args)) =
            FIELDS.iter().find(|(class_name, _, _, _)| *class_name == name)
        {
            let fields_object = instance.new_tuple(
                fields.iter().map(|field| instance.new_str(field)).collect(),
            );
            instance.dict_set(class_namespace, "_fields", fields_object);
            let attributes_object = instance.new_tuple(
                attributes
                    .iter()
                    .map(|attribute| instance.new_str(attribute))
                    .collect(),
            );
            instance.dict_set(class_namespace, "_attributes", attributes_object);
            let match_args_object = instance.new_tuple(
                match_args
                    .iter()
                    .map(|argument| instance.new_str(argument))
                    .collect(),
            );
            instance.dict_set(class_namespace, "__match_args__", match_args_object);
        }
        // 元类取默认（`type`）✓。
        if let Ok(class) = build_class_from_parts(
            instance,
            (*name).to_owned(),
            base_objects,
            class_namespace,
            None::<NonNull<TypeObject>>,
        ) {
            instance.dict_set(namespace, name, class);
        }
    }
    let exports = instance.new_list(
        CLASSES
            .iter()
            .map(|(name, _)| instance.new_str(name))
            .chain(CONSTS.iter().map(|(name, _)| instance.new_str(name)))
            .collect(),
    );
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    // **AST 解析回调**（round 86）：`compile(..., PyCF_ONLY_AST)` 的落点 ✓（core 不依赖 stdlib）。
    instance.set_ast_parser(parse_signature_ast);
    namespace
}


// --------------------------------------------------------------------------- #
// **`compile(..., PyCF_ONLY_AST)` 的最小 AST 解析面**（round 86）
// --------------------------------------------------------------------------- #
//
// 目标只有一个：让 `inspect.signature` 解析**文本签名**（它把
// `def foo<清理后的签名>: pass` 交给 `ast.parse`，再读 `module.body[0].args` 一族）。
// 本层没有完整 AST 解析器（`compile` 直接出字节码）⇒ 这里实现**签名语法**能造出的那几类节点：
// `Module`／`FunctionDef`／`arguments`／`arg` ✓；其余形状**如实 Err**（不造假值 ✓）。

/// 建一个 `_ast` 类实例（无参构造）。
fn new_node(
    instance: &Instance,
    class_name: &str,
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(class) = instance.type_named(class_name) else {
        return Err(instance.raise_builtin_error(
            "RuntimeError",
            &format!("_ast.{class_name} 不可用"),
        ));
    };
    pyawa_core::call_value(instance, class.cast::<Header>(), &[], &[])
}

/// 给节点写一个字段（`set_attribute_value` 是**借用**语义 ⇒ 这里写完归还自己那份 ✓）。
fn set_field(
    instance: &Instance,
    node: NonNull<Header>,
    field: &str,
    value: NonNull<Header>,
) -> Result<(), pyawa_core::ExecError> {
    instance.set_attribute_value(node, field, value)?;
    instance.release(value);
    Ok(())
}

/// 造 `arg` 节点。
fn make_arg(instance: &Instance, name: &str) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let node = new_node(instance, "arg")?;
    let name_object = instance.new_str(name);
    set_field(instance, node, "arg", name_object)?;
    let none = instance.new_none();
    set_field(instance, node, "annotation", none)?;
    let none = instance.new_none();
    set_field(instance, node, "type_comment", none)?;
    Ok(node)
}

/// 按逗号切参数（**顶层**逗号；括号深度 > 0 的逗号属默认值里的表达式 ⇒ 不切）。
fn split_parameters(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for character in text.chars() {
        match character {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(character);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(character);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_owned());
                current = String::new();
            }
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_owned());
    }
    parts
}

/// **声明**给 compile 的 AST 解析回调（只支持 `def <名字>(<参数表>): pass`）✓。
pub fn parse_signature_ast(
    instance: &Instance,
    source: &str,
    _filename: &str,
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let unsupported = || pyawa_core::ExecError::Unsupported {
        opcode: 0,
        what: "compile()：`PyCF_ONLY_AST` 目前只接线了签名解析（`def 名字(<参数表>): pass`）",
    };
    // 形如 `def foo(<参数>): pass` —— 取**配对**的圆括号内容 ✓。
    let Some(open) = source.find('(') else {
        return Err(unsupported());
    };
    let mut depth = 0i32;
    let mut close = None;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        return Err(unsupported());
    };
    let parameters = &source[open + 1..close];

    let mut posonly: Vec<NonNull<Header>> = Vec::new();
    let mut positional: Vec<NonNull<Header>> = Vec::new();
    let mut kwonly: Vec<NonNull<Header>> = Vec::new();
    let mut vararg: Option<NonNull<Header>> = None;
    let mut kwarg: Option<NonNull<Header>> = None;
    let mut star_seen = false;
    for part in split_parameters(parameters) {
        if part.is_empty() {
            continue;
        }
        if part == "/" {
            // `/` 之前的位置参数变成 **仅位置** ✓。
            posonly.append(&mut positional);
            positional = Vec::new();
            continue;
        }
        if part == "*" {
            star_seen = true;
            continue;
        }
        if let Some(name) = part.strip_prefix("**") {
            let name = name.trim();
            if name.is_empty() {
                return Err(unsupported());
            }
            kwarg = Some(make_arg(instance, name)?);
            continue;
        }
        if let Some(name) = part.strip_prefix('*') {
            let name = name.trim();
            if name.is_empty() {
                return Err(unsupported());
            }
            vararg = Some(make_arg(instance, name)?);
            star_seen = true;
            continue;
        }
        // 带默认值的参数：本层还不造 `Constant`／表达式节点 ⇒ 如实拒绝（`inspect` 会退化成 '(' ✓）。
        if part.contains('=') {
            return Err(unsupported());
        }
        let node = make_arg(instance, part.trim())?;
        if star_seen {
            kwonly.push(node);
        } else {
            positional.push(node);
        }
    }

    // `arguments` 节点：七个字段 ✓（`posonlyargs, args, vararg, kwonlyargs, kw_defaults, kwarg, defaults`）。
    let arguments = new_node(instance, "arguments")?;
    let posonly_list = instance.new_list(posonly);
    set_field(instance, arguments, "posonlyargs", posonly_list)?;
    let args_list = instance.new_list(positional);
    set_field(instance, arguments, "args", args_list)?;
    match vararg {
        Some(node) => set_field(instance, arguments, "vararg", node)?,
        None => {
            let none = instance.new_none();
            set_field(instance, arguments, "vararg", none)?;
        }
    }
    // `kw_defaults` 的长度必须与 `kwonlyargs` 一致 ⇒ 先记长度（列表接手后就拿不到 len 了）✓；
    // 本层不接受默认值 ⇒ 每格 `None` ✓。
    let kwonly_count = kwonly.len();
    let kwonly_list = instance.new_list(kwonly);
    set_field(instance, arguments, "kwonlyargs", kwonly_list)?;
    let kw_defaults: Vec<NonNull<Header>> =
        (0..kwonly_count).map(|_| instance.new_none()).collect();
    let kw_defaults_list = instance.new_list(kw_defaults);
    set_field(instance, arguments, "kw_defaults", kw_defaults_list)?;
    match kwarg {
        Some(node) => set_field(instance, arguments, "kwarg", node)?,
        None => {
            let none = instance.new_none();
            set_field(instance, arguments, "kwarg", none)?;
        }
    }
    let defaults = instance.new_list(Vec::new());
    set_field(instance, arguments, "defaults", defaults)?;

    // `FunctionDef`：`name, args, body, decorator_list, returns, type_comment, type_params` ✓
    let function = new_node(instance, "FunctionDef")?;
    let name = instance.new_str("foo");
    set_field(instance, function, "name", name)?;
    set_field(instance, function, "args", arguments)?;
    let body = instance.new_list(Vec::new());
    set_field(instance, function, "body", body)?;
    let decorators = instance.new_list(Vec::new());
    set_field(instance, function, "decorator_list", decorators)?;
    let none = instance.new_none();
    set_field(instance, function, "returns", none)?;
    let none = instance.new_none();
    set_field(instance, function, "type_comment", none)?;
    let type_params = instance.new_list(Vec::new());
    set_field(instance, function, "type_params", type_params)?;

    // `Module`：`body, type_ignores` ✓
    let module = new_node(instance, "Module")?;
    let body = instance.new_list(vec![function]);
    set_field(instance, module, "body", body)?;
    let type_ignores = instance.new_list(Vec::new());
    set_field(instance, module, "type_ignores", type_ignores)?;
    Ok(module)
}
