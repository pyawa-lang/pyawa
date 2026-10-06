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
        // 元类取默认（`type`）✓；命名空间是空 dict ✓（字段由参照在 C 层给，本层暂不给字段 ✗）。
        if let Ok(class) = build_class_from_parts(
            instance,
            (*name).to_owned(),
            base_objects,
            instance.new_dict(),
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
    namespace
}
