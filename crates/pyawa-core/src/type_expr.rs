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

// ============================================================================
// **求值层**（`TS-52`／`TS-53`；M3）：名称解析 ＋ `typing` 构造识别 → 语义类型。
//
// 与语法层严格分开：本层**需要实例**（类型注册表/globals/builtins/属性链），
// 且**不改**语法树本身。输出 [`SemanticType`] 与 `AB-69` 的 `TyExpr` 同形（M4 接 ABI 前先落在
// core，避免 core→abi 反向依赖）；`to_static_label()` 提供 `BC-24`/`T-TS-13` 的静态标签投影。
// ============================================================================

use core::ptr::NonNull;

use crate::executor::runtime::discard_error;
use crate::header::Header;
use crate::instance::Instance;
use crate::type_object::TypeObject;

/// 求值后的**语义类型**（`TS-53`；与 `AB-69` 的 `TyExpr` 同形，M4 接 ABI 时同一表示）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticType {
    /// 显式 `Any`（`TS-16`：来源可枚举；与一切类型双向相容）。
    Any,
    /// `None`。
    NoneType,
    /// 具名类型：**同一性＝类型对象**（`TS-29`）。
    Named(NonNull<TypeObject>),
    /// `list[T]`：边界按 `TS-35` **擦除**（只核容器身份）。
    ListOf(Box<SemanticType>),
    /// `dict[K, V]`：同上（只核 dict 身份）。
    DictOf(Box<SemanticType>, Box<SemanticType>),
    /// `T?`：`None` 或 `T`（`TS-46`）。
    Optional(Box<SemanticType>),
    /// `T | U | …`（`TS-46`／`BC-24` 的 `"Union"` 标签）。
    Union(Vec<SemanticType>),
    /// `Callable[[P…], R]`：边界只核**可调用**（v1，同 `AB-69`）。
    Callable {
        /// 形参类型（v1 不参与边界核对）。
        params: Vec<SemanticType>,
        /// 返回类型（v1 不参与边界核对）。
        ret: Box<SemanticType>,
    },
    /// 类型变量（`AB-69` 的同名统一口径）。
    Var(String),
    /// `Base[Args…]`（宿主泛型／内置容器之外的泛型）：边界**只核基底**（`TS-35` 擦除）。
    Apply(Box<SemanticType>, Vec<SemanticType>),
}

impl SemanticType {
    /// `BC-24` 的**静态标签**投影（与 `AB-69` 的 `static_label` 同口径；`T-TS-13`）。
    ///
    /// 只投影**静态可判**的简单标签；泛型参数、可空/联合、`Callable`、类型变量 ⇒ `None`
    /// （编译期放行、边界运行期兜底）。
    pub fn to_static_label(&self) -> Option<String> {
        match self {
            // `BC-24` 的 `"Any"` 标签（与一切类型相容）。
            SemanticType::Any => Some("Any".to_owned()),
            SemanticType::NoneType => Some("NoneType".to_owned()),
            // SAFETY: 类型对象由实例注册表持有，语义类型存活期间实例存活。
            SemanticType::Named(ty) => Some(unsafe { ty.as_ref() }.name().to_owned()),
            SemanticType::ListOf(_) => Some("list".to_owned()),
            SemanticType::DictOf(_, _) => Some("dict".to_owned()),
            SemanticType::Apply(base, _) => base.to_static_label(),
            SemanticType::Optional(_)
            | SemanticType::Union(_)
            | SemanticType::Callable { .. }
            | SemanticType::Var(_) => None,
        }
    }
}

/// 求值失败的形态（`TS-54`：宿主侧映射成 `PA_ERR_INVALID`；**不**静默降级）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolveError {
    /// 名字在当前作用域里找不到（且不是"延迟解析"的前向引用）。
    Unknown(String),
    /// **前向引用**暂未解析到（`TS-52`：可以延迟重试——M4 在检查/调用时机重解析）。
    Deferred(String),
    /// 认识但形态不合法／不支持（如实拒绝，不猜）。
    Invalid(String),
}

impl core::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ResolveError::Unknown(name) => write!(f, "注解里的名字 `{name}` 在当前作用域里找不到"),
            ResolveError::Deferred(name) => {
                write!(f, "注解里的前向引用 `{name}` 暂未解析到（可以延迟重试）")
            }
            ResolveError::Invalid(message) => f.write_str(message),
        }
    }
}

/// 求值作用域（`TS-52` 的求值层输入）。
pub struct ResolveScope<'a> {
    /// 实例（类型注册表/builtins/属性链都从它取）。
    pub instance: &'a Instance,
    /// 名称解析**第一档**：模块 globals（可选）。
    pub globals: Option<NonNull<Header>>,
    /// 调用方声明的**类型变量名**（`AB-69` 的 `tparams` 同口径；命中即 `Var`）。
    pub tparams: &'a [String],
}

impl<'a> ResolveScope<'a> {
    /// 只有实例（builtins/类型注册表可查）。
    pub fn new(instance: &'a Instance) -> Self {
        Self {
            instance,
            globals: None,
            tparams: &[],
        }
    }
}

/// `typing` 构造的**裸名别名**（没有下标时）：`List` ⇒ `list` 等（`TS-53` 的"按名字识别"）。
fn typing_bare_alias(name: &str) -> Option<&'static str> {
    match name {
        "List" => Some("list"),
        "Dict" => Some("dict"),
        "Set" => Some("set"),
        "FrozenSet" => Some("frozenset"),
        "Tuple" => Some("tuple"),
        _ => None,
    }
}

/// 语法树的**点号路径**（`typing.Optional` ⇒ `"typing.Optional"`；非名字/点号 ⇒ `None`）。
fn path_of(expr: &TypeExpr) -> Option<String> {
    match expr {
        TypeExpr::Name(name) => Some(name.clone()),
        TypeExpr::Attribute { base, name } => Some(format!("{}.{}", path_of(base)?, name)),
        _ => None,
    }
}

fn last_segment(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// **解析并求值**一段注解字符串（语法层 + 求值层；M3 的对外入口，M4 会用于 `AB-52`）。
pub fn evaluate_type_expression(
    scope: &ResolveScope<'_>,
    text: &str,
) -> Result<SemanticType, TypeAnnotationError> {
    let tree = crate::compile::parse_type_expression(text).map_err(TypeAnnotationError::Syntax)?;
    resolve_type_expr(scope, &tree).map_err(TypeAnnotationError::Resolve)
}

/// `evaluate_type_expression` 的失败面（语法错 vs 求值错，供 `TS-54` 的宿主错误映射区分）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeAnnotationError {
    /// 语法层拒绝（`TS-51`）。
    Syntax(crate::compile::CompileError),
    /// 求值层拒绝（`TS-54`）。
    Resolve(ResolveError),
}

impl core::fmt::Display for TypeAnnotationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TypeAnnotationError::Syntax(error) => write!(f, "注解语法错：{error:?}"),
            TypeAnnotationError::Resolve(error) => write!(f, "{error}"),
        }
    }
}

/// **语法树 → 语义类型**（`TS-52` 的求值层；`TS-53` 的 `BC-24` 映射）。
pub fn resolve_type_expr(
    scope: &ResolveScope<'_>,
    expr: &TypeExpr,
) -> Result<SemanticType, ResolveError> {
    match expr {
        TypeExpr::Any => Ok(SemanticType::Any),
        TypeExpr::NoneType => Ok(SemanticType::NoneType),
        TypeExpr::Ellipsis => Err(ResolveError::Invalid(
            "`...` 只能出现在 `Callable`／`tuple` 的泛型实参里".to_owned(),
        )),
        TypeExpr::List(_) => Err(ResolveError::Invalid(
            "列表形态只能出现在 `Callable[[…], …]` 的形参表位置".to_owned(),
        )),
        TypeExpr::ForwardRef(text) => resolve_forward_ref(scope, text),
        TypeExpr::Name(name) => resolve_name(scope, name),
        TypeExpr::Attribute { .. } => resolve_dotted_name(scope, expr),
        TypeExpr::Union(items) => {
            let mut members = Vec::with_capacity(items.len());
            for item in items {
                members.push(resolve_type_expr(scope, item)?);
            }
            Ok(normalize_union(members))
        }
        TypeExpr::Subscript { base, arguments } => resolve_subscript(scope, base, arguments),
    }
}

fn resolve_forward_ref(scope: &ResolveScope<'_>, text: &str) -> Result<SemanticType, ResolveError> {
    // 前向引用按"注解字符串里的名字/点号"解析：先按普通名字路径尝试，解析不到 ⇒ **延迟**
    // （`TS-52`：M4 在检查/调用时机重解析；这里如实给 `Deferred`，不猜成 `Any`）。
    let tree = synthesize_path_tree(text)?;
    match resolve_type_expr(scope, &tree) {
        Ok(value) => Ok(value),
        // 名字本身是 `Optional` 之类需要下标的 typing 构造 ⇒ 前向引用里不带下属语法（如实拒绝）。
        Err(ResolveError::Invalid(message)) => Err(ResolveError::Invalid(format!(
            "前向引用 `{text}` 里的注解不合法：{message}"
        ))),
        Err(ResolveError::Unknown(_)) | Err(ResolveError::Deferred(_)) => {
            Err(ResolveError::Deferred(text.to_owned()))
        }
    }
}

/// 把 `"mod.X"` / `"X"` 这样的前向引用文本还原成点号名字树（非标识符/非点号 ⇒ 如实拒绝）。
fn synthesize_path_tree(text: &str) -> Result<TypeExpr, ResolveError> {
    let mut parts = text.split('.');
    let Some(first) = parts.next() else {
        return Err(ResolveError::Invalid("空的前向引用".to_owned()));
    };
    if !is_identifier(first) {
        return Err(ResolveError::Invalid(format!(
            "前向引用 `{text}` 不是名字/点号路径（任意表达式请走 `__annotate__` 通道）"
        )));
    }
    let mut tree = TypeExpr::Name(first.to_owned());
    for part in parts {
        if !is_identifier(part) {
            return Err(ResolveError::Invalid(format!(
                "前向引用 `{text}` 不是名字/点号路径（任意表达式请走 `__annotate__` 通道）"
            )));
        }
        tree = TypeExpr::Attribute {
            base: Box::new(tree),
            name: part.to_owned(),
        };
    }
    Ok(tree)
}

fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn resolve_name(scope: &ResolveScope<'_>, name: &str) -> Result<SemanticType, ResolveError> {
    // 0) 类型变量（`AB-69` 的 `tparams` 同口径）。
    if scope.tparams.iter().any(|parameter| parameter == name) {
        return Ok(SemanticType::Var(name.to_owned()));
    }
    // 1) `typing` 裸名别名（无下标）：`List` ⇒ `list` 等。`Optional`/`Union`/`Callable`/`Annotated`
    //    没有"无实参"的合法形态 ⇒ 如实拒绝。
    if let Some(alias) = typing_bare_alias(name) {
        return resolve_named_type(scope, alias);
    }
    if matches!(name, "Optional" | "Union" | "Callable" | "Annotated") {
        return Err(ResolveError::Invalid(format!(
            "`{name}` 必须带实参（`{name}[…]`）；裸名没有类型含义"
        )));
    }
    resolve_named_type(scope, name)
}

fn resolve_named_type(scope: &ResolveScope<'_>, name: &str) -> Result<SemanticType, ResolveError> {
    let object = lookup_named_object(scope, name)?;
    named_from_object(scope, object, name)
}

/// 按 `TS-52` 的次序查名字：globals → builtins → 类型注册表；返回**新引用**。
fn lookup_named_object(
    scope: &ResolveScope<'_>,
    name: &str,
) -> Result<NonNull<Header>, ResolveError> {
    if let Some(globals) = scope.globals {
        if let Some(value) = scope.instance.dict_get(globals, name) {
            return Ok(scope.instance.retain(value));
        }
    }
    if let Some(builtins) = scope.instance.builtins() {
        if let Some(value) = scope.instance.dict_get(builtins, name) {
            return Ok(scope.instance.retain(value));
        }
    }
    if let Some(ty) = scope.instance.type_named(name) {
        return Ok(scope.instance.type_value(ty));
    }
    Err(ResolveError::Unknown(name.to_owned()))
}

fn named_from_object(
    scope: &ResolveScope<'_>,
    object: NonNull<Header>,
    name: &str,
) -> Result<SemanticType, ResolveError> {
    if !scope.instance.is_type_object(object) {
        // SAFETY: 调用方交回的是一份新引用。
        unsafe { scope.instance.release_object(object.as_ptr()) };
        return Err(ResolveError::Invalid(format!(
            "`{name}` 解析到的不是类型对象（类型表达式只收类型）"
        )));
    }
    let ty = object.cast::<TypeObject>();
    // SAFETY: 栈/字典/输入交回的是一份新引用；类型注册表另持一份。
    unsafe { scope.instance.release_object(object.as_ptr()) };
    Ok(SemanticType::Named(ty))
}

fn resolve_dotted_name(
    scope: &ResolveScope<'_>,
    expr: &TypeExpr,
) -> Result<SemanticType, ResolveError> {
    let Some(path) = path_of(expr) else {
        return Err(ResolveError::Invalid("点号路径不合法".to_owned()));
    };
    // `typing` 构造的裸名别名（`typing.List` ⇒ list；`typing.Optional` 仍要求下标）。
    if let Some(alias) = typing_bare_alias(last_segment(&path)) {
        return resolve_named_type(scope, alias);
    }
    let object = resolve_object_chain(scope, expr)?;
    named_from_object(scope, object, &path)
}

/// 把名字/点号树解析成一个**对象**（新引用）：用于点号属性链与前向引用。
fn resolve_object_chain(
    scope: &ResolveScope<'_>,
    expr: &TypeExpr,
) -> Result<NonNull<Header>, ResolveError> {
    match expr {
        TypeExpr::Name(name) => {
            if let Some(alias) = typing_bare_alias(name) {
                return lookup_named_object(scope, alias);
            }
            lookup_named_object(scope, name)
        }
        TypeExpr::Attribute { base, name } => {
            let base_object = resolve_object_chain(scope, base)?;
            match crate::attribute_read(scope.instance, base_object, name) {
                Ok(value) => {
                    // SAFETY: base_object 是这里持有的新引用。
                    unsafe { scope.instance.release_object(base_object.as_ptr()) };
                    Ok(value)
                }
                Err(error) => {
                    // SAFETY: base_object 是这里持有的新引用。
                    unsafe { scope.instance.release_object(base_object.as_ptr()) };
                    discard_error(scope.instance, &error);
                    Err(ResolveError::Unknown(format!(
                        "{}（点号属性链解析失败）",
                        path_of(expr).unwrap_or_default()
                    )))
                }
            }
        }
        _ => Err(ResolveError::Invalid(
            "点号路径里只允许名字/属性".to_owned(),
        )),
    }
}

fn resolve_subscript(
    scope: &ResolveScope<'_>,
    base: &TypeExpr,
    arguments: &[TypeExpr],
) -> Result<SemanticType, ResolveError> {
    let Some(path) = path_of(base) else {
        return Err(ResolveError::Invalid(
            "下标的基底必须是名字/点号（类型表达式）".to_owned(),
        ));
    };
    let constructor = last_segment(&path);
    match constructor {
        "Optional" => {
            if arguments.len() != 1 {
                return Err(ResolveError::Invalid(format!(
                    "`{path}[…]` 只能有 1 个实参，实际 {}",
                    arguments.len()
                )));
            }
            let inner = resolve_type_expr(scope, &arguments[0])?;
            Ok(normalize_union(vec![inner, SemanticType::NoneType]))
        }
        "Union" => {
            if arguments.is_empty() {
                return Err(ResolveError::Invalid(format!("`{path}[…]` 至少要 1 个实参")));
            }
            let mut members = Vec::with_capacity(arguments.len());
            for argument in arguments {
                members.push(resolve_type_expr(scope, argument)?);
            }
            Ok(normalize_union(members))
        }
        "Callable" => {
            if arguments.len() != 2 {
                return Err(ResolveError::Invalid(format!(
                    "`{path}[[形参…], 返回]` 必须恰有 2 个实参，实际 {}",
                    arguments.len()
                )));
            }
            let params = match &arguments[0] {
                // `Callable[..., R]`：形参**可变**（v1 与 `AB-69` 一致：只核可调用）。
                TypeExpr::Ellipsis => Vec::new(),
                TypeExpr::List(items) => {
                    let mut params = Vec::with_capacity(items.len());
                    for item in items {
                        params.push(resolve_type_expr(scope, item)?);
                    }
                    params
                }
                other => {
                    return Err(ResolveError::Invalid(format!(
                        "`{path}` 的第一个实参必须是 `[形参…]` 或 `...`，实际 {}",
                        other.to_source()
                    )))
                }
            };
            let ret = resolve_type_expr(scope, &arguments[1])?;
            Ok(SemanticType::Callable {
                params,
                ret: Box::new(ret),
            })
        }
        "Annotated" => {
            // `TS-53`：`Annotated[T, …]` 只看第一个实参；元数据不参与类型判定（如实记录）。
            let Some(first) = arguments.first() else {
                return Err(ResolveError::Invalid(format!("`{path}[…]` 至少要 1 个实参")));
            };
            resolve_type_expr(scope, first)
        }
        "List" => {
            expect_single_argument(scope, &path, arguments).map(|inner| {
                SemanticType::ListOf(Box::new(inner))
            })
        }
        "Dict" => {
            if arguments.len() != 2 {
                return Err(ResolveError::Invalid(format!(
                    "`{path}[K, V]` 必须恰有 2 个实参，实际 {}",
                    arguments.len()
                )));
            }
            let key = resolve_type_expr(scope, &arguments[0])?;
            let value = resolve_type_expr(scope, &arguments[1])?;
            Ok(SemanticType::DictOf(Box::new(key), Box::new(value)))
        }
        "Set" | "FrozenSet" => {
            let inner = expect_single_argument(scope, &path, arguments)?;
            let base_name = if constructor == "Set" { "set" } else { "frozenset" };
            let base = resolve_named_type(scope, base_name)?;
            Ok(SemanticType::Apply(Box::new(base), vec![inner]))
        }
        "Tuple" => {
            let base = resolve_named_type(scope, "tuple")?;
            let mut members = Vec::with_capacity(arguments.len());
            for argument in arguments {
                // `tuple[int, ...]` 的 `...`：擦除（边界只核基底）⇒ 不进语义实参表。
                if matches!(argument, TypeExpr::Ellipsis) {
                    continue;
                }
                members.push(resolve_type_expr(scope, argument)?);
            }
            Ok(SemanticType::Apply(Box::new(base), members))
        }
        // 内置小写容器名也接受（`list[int]`／`dict[K,V]`／`set[int]`／`tuple[…]`）——
        // 与 `typing` 大写名同一映射，避免两套口径。
        "list" => expect_single_argument(scope, &path, arguments)
            .map(|inner| SemanticType::ListOf(Box::new(inner))),
        "dict" => {
            if arguments.len() != 2 {
                return Err(ResolveError::Invalid(format!(
                    "`{path}[K, V]` 必须恰有 2 个实参，实际 {}",
                    arguments.len()
                )));
            }
            let key = resolve_type_expr(scope, &arguments[0])?;
            let value = resolve_type_expr(scope, &arguments[1])?;
            Ok(SemanticType::DictOf(Box::new(key), Box::new(value)))
        }
        "set" | "frozenset" => {
            let inner = expect_single_argument(scope, &path, arguments)?;
            let base = resolve_named_type(scope, constructor)?;
            Ok(SemanticType::Apply(Box::new(base), vec![inner]))
        }
        "tuple" => {
            let base = resolve_named_type(scope, "tuple")?;
            let mut members = Vec::with_capacity(arguments.len());
            for argument in arguments {
                if matches!(argument, TypeExpr::Ellipsis) {
                    continue;
                }
                members.push(resolve_type_expr(scope, argument)?);
            }
            Ok(SemanticType::Apply(Box::new(base), members))
        }
        // 其余 `X[实参…]`：宿主泛型/用户泛型 ⇒ `Apply`（`TS-35` 擦除：边界只核基底）。
        _ => {
            let base_value = resolve_type_expr(scope, base)?;
            let mut members = Vec::with_capacity(arguments.len());
            for argument in arguments {
                if matches!(argument, TypeExpr::Ellipsis) {
                    continue;
                }
                members.push(resolve_type_expr(scope, argument)?);
            }
            Ok(SemanticType::Apply(Box::new(base_value), members))
        }
    }
}

fn expect_single_argument(
    scope: &ResolveScope<'_>,
    path: &str,
    arguments: &[TypeExpr],
) -> Result<SemanticType, ResolveError> {
    if arguments.len() != 1 {
        return Err(ResolveError::Invalid(format!(
            "`{path}[…]` 只能有 1 个实参，实际 {}",
            arguments.len()
        )));
    }
    resolve_type_expr(scope, &arguments[0])
}

/// **联合归一化**（`TS-46`／`BC-24`）：
/// - `Union[A]` ⇒ `A`（参照同义）；
/// - 恰为 `[T, None]`（任一序）⇒ `Optional(T)`（`TS-46` 的 `T?`）；
/// - 其余 ⇒ 摊平的 `Union`（嵌套联合/`Optional` 成员展开）。
fn normalize_union(members: Vec<SemanticType>) -> SemanticType {
    let mut flattened: Vec<SemanticType> = Vec::new();
    for member in members {
        match member {
            SemanticType::Union(items) => {
                for item in items {
                    push_union_member(&mut flattened, item);
                }
            }
            SemanticType::Optional(inner) => {
                push_union_member(&mut flattened, *inner);
                push_union_member(&mut flattened, SemanticType::NoneType);
            }
            other => push_union_member(&mut flattened, other),
        }
    }
    if flattened.is_empty() {
        return SemanticType::Any;
    }
    if flattened.len() == 1 {
        return flattened.pop().expect("刚判过非空");
    }
    let none_count = flattened
        .iter()
        .filter(|member| matches!(member, SemanticType::NoneType))
        .count();
    if flattened.len() == 2 && none_count == 1 {
        let other = flattened
            .into_iter()
            .find(|member| !matches!(member, SemanticType::NoneType))
            .expect("恰有一个非 None 成员");
        return SemanticType::Optional(Box::new(other));
    }
    SemanticType::Union(flattened)
}

fn push_union_member(out: &mut Vec<SemanticType>, member: SemanticType) {
    // 去重（同一类型对象/同名字变量）：`int | int` ≡ `int`。
    if !out.contains(&member) {
        out.push(member);
    }
}
