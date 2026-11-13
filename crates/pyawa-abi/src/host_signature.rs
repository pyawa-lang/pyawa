//! **`AB-69` 的签名数据模型与边界检查**（`pa_ty` 句柄版；round 272）。
//!
//! 口径（一处真相＝`SPEC-c-abi.md` 的 `AB-69`）：
//! * 类型表达式是**不透明句柄** `pa_ty`（`pa_ty_runtime_tag`／`named`／`list_of`／`dict_of`／
//!   `optional`／`union`／`callable`／`var`／`apply` 构造）；句柄**按实例**存，`pa_destroy` 后失效；
//! * **边界运行期检查**：宿主函数**声明过**签名 ⇒ 调用边界按声明核参数（与返回）；不符 ⇒
//!   **脚本侧可捕获**的 `TypeError`（归责插件：消息带宿主函数名）✓；未声明 ⇒ 与今天完全一致
//!   （不检查，`AB-69` ③）✓；
//! * **泛型擦除**（`TS-35`／`TS-30`）：`list[int]`／`dict[str,int]` 的**容器身份**在边界核
//!   （是 list／dict），**元素类型不核** ✓；宿主泛型 `pa_ty_apply` 同样只核基底 ✓（不做
//!   reified generics ✗）；
//! * **类型变量**（`pa_ty_var`）：同一次调用里同名变量**统一**（第一个实参定下实际类型，
//!   后续同名参数必须相容）✓；未在参数里出现的变量不做额外限制 ✓。

use core::ptr::NonNull;
use std::collections::{HashMap, HashSet};

use pyawa_core::{ExecError, Header, Instance, TypeObject};

use crate::host::{self, param_flags};
use crate::stack::tag_of;

/// 一个 `pa_ty` 句柄的实际内容（实例私有；宿主只能当不透明指针传回）。
#[derive(Clone)]
pub enum TyExpr {
    /// `AB-63` 的运行时标签（`int` 与 `bool` 在标签上分得开 ⇒ 精确匹配）。
    RuntimeTag(i32),
    /// 具名类型：**同一性＝类型对象**（内置类型按名字解析／宿主类型先 `pa_newtype` 注册）。
    Named(NonNull<TypeObject>),
    /// `list[T]`：边界只核**容器身份**（元素类型按 `TS-35` 擦除 ✓）。
    ListOf(Box<TyExpr>),
    /// `dict[K, V]`：同 `ListOf`（只核 dict 身份）。
    DictOf(Box<TyExpr>, Box<TyExpr>),
    /// `T?`：`None` 或 `T`。
    Optional(Box<TyExpr>),
    /// `T | U | …`。
    Union(Vec<TyExpr>),
    /// `Callable[[P…], R]`：v1 只核**可调用**（`PA_TFUNCTION`／内建函数标签）。
    Callable {
        /// 形参类型（v1 不参与边界核对）。
        params: Vec<TyExpr>,
        /// 返回类型（v1 不参与边界核对）。
        ret: Box<TyExpr>,
    },
    /// 类型变量（`T`；配合 `pa_func_set_signature` 的 `tparams`）。
    Var(String),
    /// 宿主泛型 `Base[Args…]`：边界**只核基底**（擦除类型身份 ✓）。
    Apply(Box<TyExpr>, Vec<TyExpr>),
}

/// 一个宿主函数的**已声明签名**（`pa_func_set_signature` 成功后存进实例）。
#[derive(Clone)]
pub struct TypedSignature {
    /// 宿主 C 函数（签名按它查）。
    pub function: host::PaHostFn,
    /// 形参类型（与注册时的 `pa_sig.nparams` **一一对应**）。
    pub params: Vec<TyExpr>,
    /// 返回类型；`None` ＝ 不声明（不检查返回值）。
    pub ret: Option<TyExpr>,
    /// 类型变量名（`pa_ty_var` 的集合；只作文档与一致性检查用）。
    pub tparams: Vec<String>,
    /// 签名级标志（v1 保留：必须 0）。
    pub flags: u32,
}

/// 类型变量统一时的实际类型。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    /// 类型对象（`pa_ty_var` 的统一只按类型对象走；`TS-35` 的擦除口径）。
    Type(NonNull<TypeObject>),
}

/// **投影成编译期静态标签**（`AB-69` 的检查期那一半用）：只投影**静态可判**的简单标签；
/// 复杂/未知（泛型参数、宿主不透明类型、callable、union…）⇒ `None`（编译期放行、边界运行期兜底 ✓）。
pub fn static_label(expression: &TyExpr) -> Option<String> {
    match expression {
        TyExpr::RuntimeTag(tag) => match *tag {
            crate::stack::tag::PA_TNIL => Some("NoneType".to_owned()),
            crate::stack::tag::PA_TBOOLEAN => Some("bool".to_owned()),
            crate::stack::tag::PA_TINTEGER => Some("int".to_owned()),
            crate::stack::tag::PA_TNUMBER => Some("float".to_owned()),
            crate::stack::tag::PA_TSTRING => Some("str".to_owned()),
            crate::stack::tag::PA_TBYTES => Some("bytes".to_owned()),
            // `TABLE`（list/dict 歧义）／`FUNCTION`／`HANDLE` ⇒ 不投影
            _ => None,
        },
        TyExpr::Named(ty) => {
            // SAFETY: 类型对象由实例注册表持有，实例存活期间有效。
            Some(unsafe { ty.as_ref() }.name().to_owned())
        }
        TyExpr::ListOf(_) => Some("list".to_owned()),
        TyExpr::DictOf(_, _) => Some("dict".to_owned()),
        TyExpr::Apply(base, _) => static_label(base),
        TyExpr::Optional(_) | TyExpr::Union(_) | TyExpr::Callable { .. } | TyExpr::Var(_) => None,
    }
}

/// 标签的显示名（错误消息用；与 `AB-63` 的取值域一致）。
fn tag_name(tag: i32) -> &'static str {
    match tag {
        crate::stack::tag::PA_TNIL => "None",
        crate::stack::tag::PA_TBOOLEAN => "bool",
        crate::stack::tag::PA_TINTEGER => "int",
        crate::stack::tag::PA_TNUMBER => "float",
        crate::stack::tag::PA_TSTRING => "str",
        crate::stack::tag::PA_TTABLE => "list/dict",
        crate::stack::tag::PA_TFUNCTION => "function",
        crate::stack::tag::PA_THANDLE => "host object",
        crate::stack::tag::PA_TBYTES => "bytes",
        _ => "unknown",
    }
}

/// 声明类型的显示名（错误消息用）。
fn expected_name(expr: &TyExpr) -> String {
    match expr {
        TyExpr::RuntimeTag(tag) => tag_name(*tag).to_owned(),
        TyExpr::Named(ty) => {
            // SAFETY: 类型对象由实例注册表持有，实例存活期间有效。
            unsafe { ty.as_ref() }.name().to_owned()
        }
        TyExpr::ListOf(inner) => format!("list[{}]", expected_name(inner)),
        TyExpr::DictOf(key, value) => {
            format!("dict[{}, {}]", expected_name(key), expected_name(value))
        }
        TyExpr::Optional(inner) => format!("{}?", expected_name(inner)),
        TyExpr::Union(members) => members
            .iter()
            .map(expected_name)
            .collect::<Vec<_>>()
            .join(" | "),
        TyExpr::Callable { params, ret } => format!(
            "Callable[[{}], {}]",
            params
                .iter()
                .map(expected_name)
                .collect::<Vec<_>>()
                .join(", "),
            expected_name(ret)
        ),
        TyExpr::Var(name) => format!("T({name})"),
        TyExpr::Apply(base, args) => format!(
            "{}[{}]",
            expected_name(base),
            args.iter().map(expected_name).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// 实参的显示名（错误消息用）。
fn actual_name(instance: &Instance, object: NonNull<Header>) -> String {
    instance.type_name(instance.type_of(object)).to_owned()
}

/// 收集表达式里用到的类型变量名（`tparams` 一致性检查用）。
fn collect_variables(expression: &TyExpr, out: &mut HashSet<String>) {
    match expression {
        TyExpr::Var(name) => {
            out.insert(name.clone());
        }
        TyExpr::ListOf(inner) | TyExpr::Optional(inner) => collect_variables(inner, out),
        TyExpr::DictOf(key, value) => {
            collect_variables(key, out);
            collect_variables(value, out);
        }
        TyExpr::Union(members) => {
            for member in members {
                collect_variables(member, out);
            }
        }
        TyExpr::Callable { params, ret } => {
            for parameter in params {
                collect_variables(parameter, out);
            }
            collect_variables(ret, out);
        }
        TyExpr::Apply(base, args) => {
            collect_variables(base, out);
            for argument in args {
                collect_variables(argument, out);
            }
        }
        TyExpr::RuntimeTag(_) | TyExpr::Named(_) => {}
    }
}

/// 判断一个实参是否符合声明类型（`TS-35` 的擦除口径见模块文档）。
fn matches(
    instance: &Instance,
    expected: &TyExpr,
    object: NonNull<Header>,
    bound: &mut HashMap<String, Bound>,
) -> bool {
    match expected {
        TyExpr::RuntimeTag(tag) => tag_of(instance, object) == *tag,
        TyExpr::Named(ty) => instance.is_subtype(instance.type_of(object), *ty),
        TyExpr::ListOf(_) => {
            let Some(list) = instance.type_named("list") else {
                return false;
            };
            instance.is_subtype(instance.type_of(object), list)
        }
        TyExpr::DictOf(_, _) => {
            let Some(dict) = instance.type_named("dict") else {
                return false;
            };
            instance.is_subtype(instance.type_of(object), dict)
        }
        TyExpr::Optional(inner) => {
            tag_of(instance, object) == crate::stack::tag::PA_TNIL
                || matches(instance, inner, object, bound)
        }
        TyExpr::Union(members) => members
            .iter()
            .any(|member| matches(instance, member, object, bound)),
        TyExpr::Callable { .. } => {
            matches!(tag_of(instance, object), crate::stack::tag::PA_TFUNCTION)
        }
        TyExpr::Var(name) => {
            let Some(binding) = bound.get(name) else {
                bound.insert(name.clone(), Bound::Type(instance.type_of(object)));
                return true;
            };
            match binding {
                Bound::Type(ty) => instance.is_subtype(instance.type_of(object), *ty)
                    || instance.is_subtype(*ty, instance.type_of(object)),
            }
        }
        TyExpr::Apply(base, _) => matches(instance, base, object, bound),
    }
}

/// **边界参数检查**（`AB-69` ②）：参数个数按注册时 `pa_sig` 的 flags（可选／变参）判，
/// 类型按声明核；失败 ⇒ `TypeError`（脚本可捕获，消息带宿主函数名 ⇒ **归责插件**）。
pub fn check_arguments(
    instance: &Instance,
    name: &str,
    param_flags: &[u32],
    signature: &TypedSignature,
    args: &[NonNull<Header>],
) -> Result<(), ExecError> {
    let required = param_flags
        .iter()
        .enumerate()
        .filter(|(index, flags)| {
            let variadic = *flags & param_flags::PA_PARAM_VARARGS != 0
                || *flags & param_flags::PA_PARAM_VARKW != 0;
            let optional = *flags & param_flags::PA_PARAM_HAS_DEFAULT != 0;
            !variadic && !optional && *index < signature.params.len()
        })
        .count();
    let variadic = param_flags
        .iter()
        .any(|flags| *flags & param_flags::PA_PARAM_VARARGS != 0);
    let max = if variadic {
        usize::MAX
    } else {
        signature.params.len()
    };
    if signature.flags != 0 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "AB-69：签名 flags 非 0（内部不一致）",
        ));
    }
    // **类型变量必须先声明**（`tparams`）：参数里出现未声明的 `T` ⇒ 归责插件（声明写坏了）。
    let mut used_variables = HashSet::new();
    for expression in &signature.params {
        collect_variables(expression, &mut used_variables);
    }
    for name in &used_variables {
        if !signature.tparams.iter().any(|declared| declared == name) {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("AB-69：宿主函数 `{name}` 使用了未在 tparams 声明的类型变量 `{name}`"),
            ));
        }
    }
    if args.len() < required || args.len() > max {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "宿主函数 `{name}` 的参数个数不符（声明 {required}..{} 个，实际 {} 个）——AB-69 边界检查",
                if variadic { "∞".to_owned() } else { max.to_string() },
                args.len()
            ),
        ));
    }
    let mut bound: HashMap<String, Bound> = HashMap::new();
    for (index, argument) in args.iter().enumerate() {
        // 变参位（最后一个形参声明为 `*args`）⇒ 每个多出来的实参都按**元素类型**核。
        let declared = signature.params.get(index).or_else(|| {
            param_flags.last().and_then(|flags| {
                if *flags & param_flags::PA_PARAM_VARARGS != 0 {
                    signature.params.last()
                } else {
                    None
                }
            })
        });
        let Some(declared) = declared else {
            continue;
        };
        if !matches(instance, declared, *argument, &mut bound) {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "宿主函数 `{name}` 的第 {} 个参数类型不符（声明 {}，实际 {}）——AB-69 边界检查（归责插件）",
                    index,
                    expected_name(declared),
                    actual_name(instance, *argument)
                ),
            ));
        }
    }
    Ok(())
}

/// **边界返回检查**：声明了返回类型 ⇒ 核返回值；不符 ⇒ `TypeError`。
pub fn check_return(
    instance: &Instance,
    name: &str,
    signature: &TypedSignature,
    value: NonNull<Header>,
) -> Result<(), ExecError> {
    let Some(ret) = &signature.ret else {
        return Ok(());
    };
    let mut bound = HashMap::new();
    if matches(instance, ret, value, &mut bound) {
        Ok(())
    } else {
        Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "宿主函数 `{name}` 的返回值类型不符（声明 {}，实际 {}）——AB-69 边界检查（归责插件）",
                expected_name(ret),
                actual_name(instance, value)
            ),
        ))
    }
}
