//! `pyexpat` 模块（`P3-37` 剩余；**形状面 ＋ 诚实解析桩** ✗）。
//!
//! **动因**：官方套件 `test_xml_etree`／`test_xml_etree_c`／`test_minidom`／`test_docxmlrpc`
//! 在**导入期**就要 `pyexpat`（`Lib/xml/parsers/expat.py` 的 `from pyexpat import *` ＋ 两个
//! `sys.modules[...] = model/errors` 子模块）；`test_pyexpat` 则直接调它。此前整个模块缺 ⇒
//! 5 个测试卡在 `ModuleNotFoundError: No module named 'pyexpat'`。
//!
//! **本片给什么**（`CM-6`：名字齐、真做不了的**调用时如实报未实现**）：
//! - 模块级常量：`EXPAT_VERSION`／`native_encoding`／`version_info`／`features`／三个
//!   `XML_PARAM_ENTITY_PARSING_*`；`ErrorString(code)`（44 条错误消息表，值照参照实测）；
//! - `errors`／`model` 两个**子模块**（顶层名字与 `sys.modules['pyexpat.errors'…]` 都挂上，
//!   `xml.parsers.expat` 的重导出才成立 ✓）——`errors` 的 44 条 `XML_ERROR_*`（值是**消息串** ✓）、
//!   `model` 的 10 条 `XML_*` 常量；
//! - `ExpatError`／`error`：`OSError` 的**别名子类**（参照里是 `Exception` 子类；本层异常子类的
//!   载荷布局限制见 `_socket.gaierror` 同一处说明 ⇒ 先按 `OSError` 家族落名，如实记）；
//! - `XMLParserType` 类 ＋ `ParserCreate(encoding=None, namespace_separator=None)`：实例可建、
//!   处理器属性可读写（`StartElementHandler` 一族默认 `None`）、`SetParamEntityParsing` 返回 1；
//!   **`Parse`／`ParseFile` 如实报 `NotImplementedError`** ✗ —— 真解析要移植整块 expat C 扩展
//!   （XML/DTD/实体/命名空间/错误码语义），属**独立评估**的活，不在本批硬塞。
//!
//! 因此：**导入面缺口消失** ✓（4 个测试从"缺模块"推进到各自的下一层缺口）；
//! `test_pyexpat` 由"缺模块"转为**如实未实现**（到不了断言层——整块 expat 的语义面未接，如实记 ✗）。

use core::ptr::NonNull;

use pyawa_core::{build_class_from_parts, Header, Instance, NativeFn};

/// 模块名（`pyexpat`）。
pub const NAME: &str = "pyexpat";
/// 子模块名（`pyexpat.errors`）。
pub const ERRORS_NAME: &str = "pyexpat.errors";
/// 子模块名（`pyexpat.model`）。
pub const MODEL_NAME: &str = "pyexpat.model";

/// `__doc__`。
pub const DOC: &str = "Interface to the Expat non-validating XML parser.";

/// `features`（参照实测的 expat 编译期特征表）✓。
const FEATURES: &[(&str, i64)] = &[("sizeof(XML_Char)", 1), ("sizeof(XML_LChar)", 1), ("XML_DTD", 0), ("XML_CONTEXT_BYTES", 1024), ("XML_NS", 0), ("XML_BLAP_MAX_AMP", 100), ("XML_BLAP_ACT_THRES", 8388608), ("XML_GE", 0), ("XML_AT_MAX_AMP", 100), ("XML_AT_ACT_THRES", 67108864)];

/// **`ErrorString(code)` 的码表**（`code` 从 1 起；索引 = `code - 1`；参照实测逐条抄录 ✓）。
///
/// 注意：它**不是** `errors` 子模块的名字表 —— 参照的码序与字母序不同（如 `code == 1` 是
/// `out of memory`，而 `XML_ERROR_ABORTED` 的码是 35）⇒ 两张表分开 ✓。
const ERROR_MESSAGES: &[&str] = &[
    "out of memory",
    "syntax error",
    "no element found",
    "not well-formed (invalid token)",
    "unclosed token",
    "partial character",
    "mismatched tag",
    "duplicate attribute",
    "junk after document element",
    "illegal parameter entity reference",
    "undefined entity",
    "recursive entity reference",
    "asynchronous entity",
    "reference to invalid character number",
    "reference to binary entity",
    "reference to external entity in attribute",
    "XML or text declaration not at start of entity",
    "unknown encoding",
    "encoding specified in XML declaration is incorrect",
    "unclosed CDATA section",
    "error in processing external entity reference",
    "document is not standalone",
    "unexpected parser state - please send a bug report",
    "entity declared in parameter entity",
    "requested feature requires XML_DTD support in Expat",
    "cannot change setting once parsing has begun",
    "unbound prefix",
    "must not undeclare prefix",
    "incomplete markup in parameter entity",
    "XML declaration not well-formed",
    "text declaration not well-formed",
    "illegal character(s) in public id",
    "parser suspended",
    "parser not suspended",
    "parsing aborted",
    "parsing finished",
    "cannot suspend in external parameter entity",
    "reserved prefix (xml) must not be undeclared or bound to another namespace name",
    "reserved prefix (xmlns) must not be declared or undeclared",
    "prefix must not be bound to one of the reserved namespace names",
    "invalid argument",
    "a successful prior call to function XML_GetBuffer is required",
    "limit on input amplification factor (from DTD and entities) breached",
    "parser not started",
];

/// `errors` 子模块的 44 个 `XML_ERROR_*`（值是**消息串**，参照实测）✓。
const ERROR_NAMES: &[(&str, &str)] = &[
    ("XML_ERROR_ABORTED", "parsing aborted"),
    ("XML_ERROR_AMPLIFICATION_LIMIT_BREACH", "limit on input amplification factor (from DTD and entities) breached"),
    ("XML_ERROR_ASYNC_ENTITY", "asynchronous entity"),
    ("XML_ERROR_ATTRIBUTE_EXTERNAL_ENTITY_REF", "reference to external entity in attribute"),
    ("XML_ERROR_BAD_CHAR_REF", "reference to invalid character number"),
    ("XML_ERROR_BINARY_ENTITY_REF", "reference to binary entity"),
    ("XML_ERROR_CANT_CHANGE_FEATURE_ONCE_PARSING", "cannot change setting once parsing has begun"),
    ("XML_ERROR_DUPLICATE_ATTRIBUTE", "duplicate attribute"),
    ("XML_ERROR_ENTITY_DECLARED_IN_PE", "entity declared in parameter entity"),
    ("XML_ERROR_EXTERNAL_ENTITY_HANDLING", "error in processing external entity reference"),
    ("XML_ERROR_FEATURE_REQUIRES_XML_DTD", "requested feature requires XML_DTD support in Expat"),
    ("XML_ERROR_FINISHED", "parsing finished"),
    ("XML_ERROR_INCOMPLETE_PE", "incomplete markup in parameter entity"),
    ("XML_ERROR_INCORRECT_ENCODING", "encoding specified in XML declaration is incorrect"),
    ("XML_ERROR_INVALID_ARGUMENT", "invalid argument"),
    ("XML_ERROR_INVALID_TOKEN", "not well-formed (invalid token)"),
    ("XML_ERROR_JUNK_AFTER_DOC_ELEMENT", "junk after document element"),
    ("XML_ERROR_MISPLACED_XML_PI", "XML or text declaration not at start of entity"),
    ("XML_ERROR_NOT_STANDALONE", "document is not standalone"),
    ("XML_ERROR_NOT_STARTED", "parser not started"),
    ("XML_ERROR_NOT_SUSPENDED", "parser not suspended"),
    ("XML_ERROR_NO_BUFFER", "a successful prior call to function XML_GetBuffer is required"),
    ("XML_ERROR_NO_ELEMENTS", "no element found"),
    ("XML_ERROR_NO_MEMORY", "out of memory"),
    ("XML_ERROR_PARAM_ENTITY_REF", "illegal parameter entity reference"),
    ("XML_ERROR_PARTIAL_CHAR", "partial character"),
    ("XML_ERROR_PUBLICID", "illegal character(s) in public id"),
    ("XML_ERROR_RECURSIVE_ENTITY_REF", "recursive entity reference"),
    ("XML_ERROR_RESERVED_NAMESPACE_URI", "prefix must not be bound to one of the reserved namespace names"),
    ("XML_ERROR_RESERVED_PREFIX_XML", "reserved prefix (xml) must not be undeclared or bound to another namespace name"),
    ("XML_ERROR_RESERVED_PREFIX_XMLNS", "reserved prefix (xmlns) must not be declared or undeclared"),
    ("XML_ERROR_SUSPENDED", "parser suspended"),
    ("XML_ERROR_SUSPEND_PE", "cannot suspend in external parameter entity"),
    ("XML_ERROR_SYNTAX", "syntax error"),
    ("XML_ERROR_TAG_MISMATCH", "mismatched tag"),
    ("XML_ERROR_TEXT_DECL", "text declaration not well-formed"),
    ("XML_ERROR_UNBOUND_PREFIX", "unbound prefix"),
    ("XML_ERROR_UNCLOSED_CDATA_SECTION", "unclosed CDATA section"),
    ("XML_ERROR_UNCLOSED_TOKEN", "unclosed token"),
    ("XML_ERROR_UNDECLARING_PREFIX", "must not undeclare prefix"),
    ("XML_ERROR_UNDEFINED_ENTITY", "undefined entity"),
    ("XML_ERROR_UNEXPECTED_STATE", "unexpected parser state - please send a bug report"),
    ("XML_ERROR_UNKNOWN_ENCODING", "unknown encoding"),
    ("XML_ERROR_XML_DECL", "XML declaration not well-formed"),
];

/// `model` 子模块的 10 个 `XML_*` 常量（参照实测）✓。
const MODEL_CONSTANTS: &[(&str, i64)] = &[("XML_CQUANT_NONE", 0), ("XML_CQUANT_OPT", 1), ("XML_CQUANT_PLUS", 3), ("XML_CQUANT_REP", 2), ("XML_CTYPE_ANY", 2), ("XML_CTYPE_CHOICE", 5), ("XML_CTYPE_EMPTY", 1), ("XML_CTYPE_MIXED", 3), ("XML_CTYPE_NAME", 4), ("XML_CTYPE_SEQ", 6)];

fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

fn type_error(instance: &Instance, message: String) -> pyawa_core::ExecError {
    instance.raise_builtin_error("TypeError", &message)
}

/// **`ErrorString(code)`**：错误码 ⇒ 消息串；未知码 ⇒ `None`（参照实测 ✓）。
fn error_string_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(code) = args.first().copied() else {
        return Err(type_error(
            instance,
            "ErrorString() takes exactly one argument (0 given)".to_owned(),
        ));
    };
    let Some(code) = instance.int_value(code) else {
        return Err(type_error(instance, "ErrorString() argument must be an integer".to_owned()));
    };
    // 参照：错误码从 1 起（0 与超界 ⇒ None ✓）。
    let message = usize::try_from(code)
        .ok()
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| ERROR_MESSAGES.get(index).copied());
    Ok(match message {
        Some(text) => instance.new_str(text),
        None => instance.new_none(),
    })
}

/// 解析器实例的默认处理器面（参照的 `XMLParserType` 公开属性；默认全是 `None` ✓）。
const HANDLER_ATTRIBUTES: &[&str] = &[
    "AttlistDeclHandler",
    "CharacterDataHandler",
    "CommentHandler",
    "DefaultHandler",
    "DefaultHandlerExpand",
    "ElementDeclHandler",
    "EndCdataSectionHandler",
    "EndDoctypeDeclHandler",
    "EndElementHandler",
    "EndNamespaceDeclHandler",
    "EntityDeclHandler",
    "ExternalEntityRefHandler",
    "NotStandaloneHandler",
    "NotationDeclHandler",
    "ProcessingInstructionHandler",
    "SkippedEntityHandler",
    "StartCdataSectionHandler",
    "StartDoctypeDeclHandler",
    "StartElementHandler",
    "StartNamespaceDeclHandler",
    "UnparsedEntityDeclHandler",
    "XmlDeclHandler",
];

/// 解析器实例的布尔属性（默认 `False` ✓）。
const PARSER_FLAGS: &[&str] = &[
    "ordered_attributes",
    "specified_attributes",
    "buffer_text",
    "namespace_prefixes",
];

/// **`XMLParserType` 的 `__init__`**：只做形状（处理器默认 `None`、布尔属性默认 `False`、
/// `buffer_size` 默认 `8192`、`intern` 空集）✓。真状态机**未接** ✗（见 `Parse`）。
fn parser_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (this, _rest) = match bound {
        Some(this) => (this, args),
        None => match args.split_first() {
            Some((this, rest)) => (*this, rest),
            None => return Err(type_error(instance, "XMLParserType.__init__ 缺少 self".to_owned())),
        },
    };
    for name in HANDLER_ATTRIBUTES {
        let none = instance.new_none();
        let outcome = instance.set_attribute_value(this, name, none);
        instance.release(none);
        outcome?;
    }
    for name in PARSER_FLAGS {
        let flag = instance.new_bool(false);
        let outcome = instance.set_attribute_value(this, name, flag);
        instance.release(flag);
        outcome?;
    }
    let buffer_size = instance.new_int(8192);
    let outcome = instance.set_attribute_value(this, "buffer_size", buffer_size);
    instance.release(buffer_size);
    outcome?;
    let intern = instance.new_set(Vec::new());
    let outcome = instance.set_attribute_value(this, "intern", intern);
    instance.release(intern);
    outcome?;
    Ok(instance.retain(instance.singletons().none()))
}

/// **`Parse(data, isfinal=False)`** ⇒ 如实 `NotImplementedError`（整块 expat 未移植 ✗）。
fn parser_parse_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`pyexpat` 的 XML 解析器本体：整块 expat C 扩展（XML/DTD/实体/命名空间/错误码）尚未移植，本片只落形状面",
    ))
}

/// **`ParseFile(file)`** ⇒ 同上。
fn parser_parse_file_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    parser_parse_native(instance, None, &[], &[])
}

/// **`SetParamEntityParsing(flag)`**：形状面返回 `1`（参照成功口径 ✓；没有真解析器状态）。
fn parser_set_param_entity_parsing_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if args.is_empty() {
        return Err(type_error(
            instance,
            "SetParamEntityParsing() takes exactly one argument (0 given)".to_owned(),
        ));
    }
    Ok(instance.new_int(1))
}

fn build_parser_type(instance: &Instance) -> Option<NonNull<Header>> {
    let base = instance.type_named("object")?.cast::<Header>();
    let namespace = instance.new_dict();
    for (name, handler) in [
        ("__init__", parser_init_native as NativeFn),
        ("Parse", parser_parse_native as NativeFn),
        ("ParseFile", parser_parse_file_native as NativeFn),
        (
            "SetParamEntityParsing",
            parser_set_param_entity_parsing_native as NativeFn,
        ),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    build_class_from_parts(
        instance,
        "xmlparser".to_owned(),
        vec![base],
        namespace,
        None,
    )
    .ok()
}

/// **`ParserCreate(encoding=None, namespace_separator=None)`**：建一个形状面解析器实例。
fn parser_create_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    for (key, _value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            return Err(type_error(instance, "ParserCreate() keywords must be strings".to_owned()));
        };
        if !matches!(name, "encoding" | "namespace_separator") {
            return Err(type_error(
                instance,
                format!("ParserCreate() got an unexpected keyword argument '{name}'"),
            ));
        }
    }
    let mut positional: Vec<Option<NonNull<Header>>> = vec![args.first().copied(), args.get(1).copied()];
    for (key, value) in kwargs {
        match instance.text_of(*key).as_deref() {
            Some("encoding") => positional[0] = Some(*value),
            Some("namespace_separator") => positional[1] = Some(*value),
            _ => {}
        }
    }
    let none = instance.singletons().none();
    let encoding = positional[0].filter(|value| *value != none);
    if let Some(value) = encoding {
        if instance.text_of(value).is_none() {
            let name = instance.type_name(instance.type_of(value));
            return Err(type_error(
                instance,
                format!("ParserCreate() argument 'encoding' must be str or None, not {name}"),
            ));
        }
    }
    let separator = positional[1].filter(|value| *value != none);
    if let Some(value) = separator {
        match instance.text_of(value) {
            Some(text) if text.chars().count() == 1 => {}
            Some(_) => {
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    "namespace_separator must be at most one character, omitted, or None",
                ))
            }
            None => {
                let name = instance.type_name(instance.type_of(value));
                return Err(type_error(
                    instance,
                    format!(
                        "ParserCreate() argument 'namespace_separator' must be str or None, not {name}"
                    ),
                ));
            }
        }
    }
    let class = instance
        .modules()
        .and_then(|modules| instance.dict_get(modules, NAME))
        .and_then(|module| pyawa_core::mounted_instance_dict(instance, module))
        .and_then(|namespace| instance.dict_get(namespace, "XMLParserType"));
    let Some(class) = class else {
        return Err(instance.raise_builtin_error(
            "RuntimeError",
            "`XMLParserType` 未登记（`pyexpat` 模块的类面缺失）",
        ));
    };
    let outcome = pyawa_core::call_value(instance, class, &[], &[])?;
    Ok(outcome)
}

/// 建 `pyexpat` 模块命名空间。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    instance.dict_set(namespace, "EXPAT_VERSION", instance.new_str("expat_2.7.4"));
    instance.dict_set(namespace, "native_encoding", instance.new_str("UTF-8"));
    let version_info = instance.new_tuple(vec![
        instance.new_int(2),
        instance.new_int(7),
        instance.new_int(4),
    ]);
    instance.dict_set(namespace, "version_info", version_info);
    let features: Vec<NonNull<Header>> = FEATURES
        .iter()
        .map(|(name, value)| {
            instance.new_tuple(vec![instance.new_str(name), instance.new_int(*value)])
        })
        .collect();
    let features = instance.new_list(features);
    instance.dict_set(namespace, "features", features);
    for (name, value) in [
        ("XML_PARAM_ENTITY_PARSING_NEVER", 0i64),
        ("XML_PARAM_ENTITY_PARSING_UNLESS_STANDALONE", 1),
        ("XML_PARAM_ENTITY_PARSING_ALWAYS", 2),
    ] {
        instance.dict_set(namespace, name, instance.new_int(value));
    }
    let error_string = make_native(instance, "ErrorString", error_string_native as NativeFn);
    instance.dict_set(namespace, "ErrorString", error_string);
    if let Some(parser_type) = build_parser_type(instance) {
        instance.dict_set(namespace, "XMLParserType", parser_type);
        let parser_create = make_native(instance, "ParserCreate", parser_create_native as NativeFn);
        instance.dict_set(namespace, "ParserCreate", parser_create);
    }
    // **异常面**：`ExpatError`／`error`（参照二者同一对象 ✓）。载荷布局限制见 `_socket.gaierror`。
    if let Some(base) = instance.type_named("OSError") {
        let class_namespace = instance.new_dict();
        if let Ok(class) = build_class_from_parts(
            instance,
            "ExpatError".to_owned(),
            vec![base.cast::<Header>()],
            class_namespace,
            None,
        ) {
            instance.dict_set(namespace, "ExpatError", class);
            instance.dict_set(namespace, "error", class);
        }
    }
    let exports: Vec<NonNull<Header>> = [
        "EXPAT_VERSION",
        "ErrorString",
        "ExpatError",
        "ParserCreate",
        "XMLParserType",
        "XML_PARAM_ENTITY_PARSING_ALWAYS",
        "XML_PARAM_ENTITY_PARSING_NEVER",
        "XML_PARAM_ENTITY_PARSING_UNLESS_STANDALONE",
        "error",
        "errors",
        "model",
        "features",
        "native_encoding",
        "version_info",
    ]
    .iter()
    .map(|name| instance.new_str(name))
    .collect();
    let exports = instance.new_list(exports);
    instance.dict_set(namespace, "__all__", exports);
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    namespace
}

/// 建 `pyexpat.errors` 子模块命名空间（44 条 `XML_ERROR_*`，值是消息串 ✓）。
pub fn build_errors(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let mut names: Vec<NonNull<Header>> = Vec::new();
    for (name, message) in ERROR_NAMES {
        instance.dict_set(namespace, name, instance.new_str(message));
        names.push(instance.new_str(name));
    }
    let exports = instance.new_list(names);
    instance.dict_set(namespace, "__all__", exports);
    instance.dict_set(namespace, "__name__", instance.new_str(ERRORS_NAME));
    namespace
}

/// 建 `pyexpat.model` 子模块命名空间（10 条 `XML_*` 常量 ✓）。
pub fn build_model(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let mut names: Vec<NonNull<Header>> = Vec::new();
    for (name, value) in MODEL_CONSTANTS {
        instance.dict_set(namespace, name, instance.new_int(*value));
        names.push(instance.new_str(name));
    }
    let exports = instance.new_list(names);
    instance.dict_set(namespace, "__all__", exports);
    instance.dict_set(namespace, "__name__", instance.new_str(MODEL_NAME));
    namespace
}

/// **把 `errors`／`model` 两个模块对象挂进 `pyexpat` 的命名空间**（`xml.parsers.expat` 的
/// `sys.modules['xml.parsers.expat.model'] = model` 要求它们**是模块对象** ✓）。
///
/// 必须在模块表装配**之后**调用（那时两个模块对象才存在 ✓）。
pub fn install_submodules(instance: &Instance) {
    let Some(modules) = instance.modules() else { return };
    let Some(pyexpat_module) = instance.dict_get(modules, NAME) else { return };
    let Some(namespace) = pyawa_core::mounted_instance_dict(instance, pyexpat_module) else {
        return;
    };
    for name in [ERRORS_NAME, MODEL_NAME] {
        if let Some(submodule) = instance.dict_get(modules, name) {
            let short = name.rsplit('.').next().unwrap_or(name);
            instance.dict_set(namespace, short, submodule);
        }
    }
}
